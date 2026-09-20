import { listen } from "@tauri-apps/api/event";
import { forceLinting } from "@codemirror/lint";
import {
    lspDetectRoots,
    lspGetLogs,
    lspInstall,
    lspListServers,
    lspListVariants,
    lspRestart,
    lspSetVariantOverride,
    lspStart,
    lspStop,
    lspStopAll,
} from "$lib/api/lsp";
import { setDiagnosticsForUri } from "./diagnostics";
import { pathToFileUri } from "./uri";
import { editorStore } from "$lib/stores/editor.svelte";
import { toastStore } from "$lib/stores/toast.svelte";
import type {
    LspDiagnosticsEvent,
    LspLogEvent,
    LspServerStatusKind,
    LspStatusEvent,
    ProjectRootInfo,
    ServerInfo,
} from "./types";

interface StatusEntry {
    status: LspServerStatusKind;
    error: string | null;
    installable: boolean;
    capabilities: Record<string, unknown> | null;
}

const LOG_CAP = 500;

function serverKey(root: string, languageId: string): string {
    return `${root}\u0000${languageId}`;
}

/** True if `path` is `root` itself or somewhere underneath it — both
 *  compared with backslashes normalized to forward slashes so this works
 *  the same regardless of platform. */
function isUnderRoot(root: string, path: string): boolean {
    const r = root.replace(/\\/g, "/").replace(/\/+$/, "");
    const p = path.replace(/\\/g, "/");
    return p === r || p.startsWith(r + "/");
}

class LspStore {
    /** `root\0languageId` -> status, read by the status bar's server list. */
    statuses = $state<Map<string, StatusEntry>>(new Map());

    /** Every project root detected in the current workspace — see
     *  `startForWorkspace`. Used by `resolveRootFor` to route a given file
     *  to the nearest matching root rather than always the workspace root,
     *  so a workspace containing more than one project (a Node backend next
     *  to a Flutter app, say) gets one independent server per project. */
    roots = $state<ProjectRootInfo[]>([]);

    /** `root\0languageId` -> captured log lines, populated from
     *  `lsp_get_logs` (backlog, on demand) and `lsp:log` events (live).
     *  `$state` (like `statuses`) so the "LSP Output" panel updates live
     *  without polling — updated via the same immutable-replace pattern as
     *  `statuses`, for the same reason: explicit over relying on deep Map
     *  proxying. */
    logs = $state<Map<string, string[]>>(new Map());

    private listeningPromise: Promise<void> | null = null;

    /** Scans `workspacePath` for project roots and records it as the active
     *  workspace, stopping any servers left over from a previously-open one
     *  (all handled server-side by `lsp_detect_roots` — see
     *  `LspManager::detect_roots`). Called once from `workspace.svelte.ts`
     *  right after a folder is opened. Doesn't itself start any server:
     *  detection only, matching AGENT.md §9's "lazy mode" — a server starts
     *  the first time a file of its language is actually opened. */
    async startForWorkspace(workspacePath: string): Promise<void> {
        await this.ensureListening();
        this.statuses = new Map();
        this.logs.clear();
        try {
            this.roots = await lspDetectRoots(workspacePath);
        } catch (err) {
            console.error(`Failed to detect project roots for ${workspacePath}:`, err);
            this.roots = [];
        }
    }

    /** The project root that should back `path` (an LSP `languageId`-typed
     *  file) — the nearest ancestor among `roots` matching `languageId`, or
     *  `fallbackRoot` (the workspace root) if none was detected. Detection
     *  can legitimately come up empty (no marker files match anything, or
     *  it hasn't finished running yet when this races the very first file
     *  open) without that meaning "no LSP support" — falling back to the
     *  workspace root preserves the original single-root behavior exactly
     *  in that case. */
    resolveRootFor(path: string, languageId: string, fallbackRoot: string): string {
        let best: string | null = null;
        for (const candidate of this.roots) {
            if (candidate.languageId !== languageId) continue;
            if (!isUnderRoot(candidate.root, path)) continue;
            if (!best || candidate.root.length > best.length) best = candidate.root;
        }
        return best ?? fallbackRoot;
    }

    /** Starts (or reuses) the server configured for `languageId` in `root`.
     *  Called lazily from editor.svelte.ts right before a file of that
     *  language is opened — not eagerly for the whole workspace, since most
     *  projects only use a handful of whatever languages the registry has
     *  configured, and starting everything up front would spawn servers
     *  nobody's using yet. Safe to call repeatedly and concurrently: Rust's
     *  `lsp_start` is itself idempotent per (root, languageId). */
    async ensureServerRunning(root: string, languageId: string): Promise<void> {
        await this.ensureListening();
        try {
            await lspStart(root, languageId);
        } catch (err) {
            console.error(`Failed to start '${languageId}' language server:`, err);
        }
    }

    async restartServer(root: string, languageId: string): Promise<void> {
        try {
            await lspRestart(root, languageId);
        } catch (err) {
            toastStore.error(`Failed to restart the ${languageId} language server: ${String(err)}`);
        }
    }

    async stopServer(root: string, languageId: string): Promise<void> {
        try {
            await lspStop(root, languageId);
        } catch (err) {
            toastStore.error(`Failed to stop the ${languageId} language server: ${String(err)}`);
        }
    }

    /** Runs the configured auto-install (npm/pip/go install, or a direct
   *  download for rust-analyzer) then starts the server — what the
   *  status bar's "Install" button calls, shown when a server's status is
   *  `{kind: "crashed", installable: true}`. Install progress streams
   *  into the same `lsp:log`/Output-panel plumbing as a running server's
   *  own output. */
    async installServer(root: string, languageId: string): Promise<void> {
        try {
            await lspInstall(root, languageId);
        } catch (err) {
            toastStore.error(`Failed to install the ${languageId} language server: ${String(err)}`);
        }
    }

    async stopAllServers(): Promise<void> {
        try {
            await lspStopAll();
        } catch (err) {
            toastStore.error(`Failed to stop language servers: ${String(err)}`);
        }
    }

    /** Full backlog for one server's "LSP Output" view — `lsp_get_logs`'s
     *  ring buffer, plus anything that's arrived over `lsp:log` since. Re-
     *  fetching the backlog each call (rather than trusting a cache) keeps
     *  this correct across a restart, which resets the server-side ring
     *  buffer — callers that just want the current reactive value without
     *  triggering a re-fetch should read `logs.get(key)` directly instead. */
    async getLogs(root: string, languageId: string): Promise<string[]> {
        const key = serverKey(root, languageId);
        try {
            const backlog = await lspGetLogs(root, languageId);
            const live = this.logs.get(key) ?? [];
            const combined = [...backlog, ...live].slice(-LOG_CAP);
            const next = new Map(this.logs);
            next.set(key, combined);
            this.logs = next;
            return combined;
        } catch {
            return this.logs.get(key) ?? [];
        }
    }

    /** Synchronous, reactive read of whatever's currently cached for this
     *  server — for the "LSP Output" panel's live view, which calls
     *  `getLogs` once on mount to seed the backlog and reads this
     *  thereafter so new `lsp:log` lines show up without polling. */
    getCachedLogs(root: string, languageId: string): string[] {
        return this.logs.get(serverKey(root, languageId)) ?? [];
    }

    /** Available variants for `languageId` at `root` (e.g. TypeScript's
     *  `classic`/`native` — see AGENT.md and `src-tauri/src/lsp/registry.rs`)
     *  and which one is currently in effect. `null` if this language only
     *  ever has one server — the status bar's cue not to show a picker. */
    async listVariants(
        workspaceRoot: string,
        root: string,
        languageId: string,
    ): Promise<{ variants: string[]; active: string } | null> {
        try {
            return await lspListVariants(workspaceRoot, root, languageId);
        } catch (err) {
            console.error(`Failed to list LSP variants for '${languageId}':`, err);
            return null;
        }
    }

    /** Switches `languageId` at `root` to `variant` — "use this LSP for
     *  this folder specifically" (AGENT.md). Writes the override, then
     *  restarts through the same graceful-shutdown-and-replay-open-
     *  documents path a manual restart already uses, which is what makes
     *  switching safe rather than a special case. */
    async setVariant(
        workspaceRoot: string,
        root: string,
        languageId: string,
        variant: string,
    ): Promise<void> {
        try {
            await lspSetVariantOverride(workspaceRoot, root, languageId, variant);
            await this.restartServer(root, languageId);
        } catch (err) {
            toastStore.error(`Failed to switch the ${languageId} language server: ${String(err)}`);
        }
    }

    /** Re-populates `statuses` from the backend's own bookkeeping —
     *  `lsp_list_servers` is the source of truth; the `lsp:status` listener
     *  below keeps this fresh incrementally between calls, but the explicit
     *  refresh (called after a status transitions to "running", when
     *  capabilities first become available) is what actually picks up
     *  `capabilities`, which the lighter-weight status event doesn't carry. */
    async refreshServers(): Promise<void> {
        let servers: ServerInfo[];
        try {
            servers = await lspListServers();
        } catch (err) {
            console.error("Failed to list LSP servers:", err);
            return;
        }
        const next = new Map<string, StatusEntry>();
        for (const server of servers) {
            next.set(serverKey(server.root, server.languageId), {
                status: server.status.kind,
                error: server.status.message ?? null,
                capabilities: server.capabilities,
                installable: server.status.installable ?? false
            });
        }
        this.statuses = next;
    }

    /** Registers the lsp:status/lsp:diagnostics/lsp:log event listeners
     *  exactly once, no matter how many times or how concurrently this is
     *  called — `listeningPromise` is shared across all callers so a second
     *  call that arrives while the first is still awaiting `listen()`
     *  doesn't race ahead of registration completing. */
    private ensureListening(): Promise<void> {
        if (!this.listeningPromise) {
            this.listeningPromise = this.registerListeners();
        }
        return this.listeningPromise;
    }

    private async registerListeners(): Promise<void> {
        await listen<LspStatusEvent>("lsp:status", (event) => {
            const { root, languageId, status } = event.payload;
            const key = serverKey(root, languageId);
            const next = new Map(this.statuses);
            const previous = next.get(key);
            next.set(key, {
                status: status.kind,
                error: status.message ?? null,
                capabilities: previous?.capabilities ?? null,
                installable: status.installable ?? false
            });
            this.statuses = next;

            if (status.kind === "running") {
                void this.refreshServers();
            }
        });

        await listen<LspDiagnosticsEvent>("lsp:diagnostics", (event) => {
            const { uri, diagnostics } = event.payload.params;
            setDiagnosticsForUri(uri, diagnostics);

            // Refresh immediately if the affected file is the one currently on
            // screen — otherwise the (correct) diagnostics just sit in the
            // cache and are picked up automatically next time that tab is
            // activated, since `linter()` runs once when its extension first
            // becomes part of the active state.
            const activeTab = editorStore.activeTab;
            const view = editorStore.view;
            if (activeTab && view && pathToFileUri(activeTab.path) === uri) {
                forceLinting(view);
            }
        });

        await listen<LspLogEvent>("lsp:log", (event) => {
            const { root, languageId, line } = event.payload;
            const key = serverKey(root, languageId);
            const existing = this.logs.get(key) ?? [];
            const updated =
                existing.length >= LOG_CAP ? [...existing.slice(1), line] : [...existing, line];
            const next = new Map(this.logs);
            next.set(key, updated);
            this.logs = next;
        });
    }
}

export const lspStore = new LspStore();
