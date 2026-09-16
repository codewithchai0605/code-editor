import { listen } from "@tauri-apps/api/event";
import { forceLinting } from "@codemirror/lint";
import { lspStart } from "$lib/api/lsp";
import { setDiagnosticsForUri } from "./diagnostics";
import { pathToFileUri } from "./uri";
import { editorStore } from "$lib/stores/editor.svelte";
import type { LspDiagnosticsEvent, LspServerStatusKind, LspStatusEvent } from "./types";

interface StatusEntry {
  status: LspServerStatusKind;
  error: string | null;
}

class LspStore {
  /** languageId -> status, read by LspStatusBar.svelte. */
  statuses = $state<Map<string, StatusEntry>>(new Map());

  private listeningPromise: Promise<void> | null = null;

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

  /** Registers the lsp:status/lsp:diagnostics event listeners exactly
   *  once, no matter how many times or how concurrently this is called —
   *  `listeningPromise` is shared across all callers so a second call
   *  that arrives while the first is still awaiting `listen()` doesn't
   *  race ahead of registration completing. */
  private ensureListening(): Promise<void> {
    if (!this.listeningPromise) {
      this.listeningPromise = this.registerListeners();
    }
    return this.listeningPromise;
  }

  private async registerListeners(): Promise<void> {
    await listen<LspStatusEvent>("lsp:status", (event) => {
      const { languageId, status } = event.payload;
      const next = new Map(this.statuses);
      next.set(languageId, { status: status.kind, error: status.message ?? null });
      this.statuses = next;
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
  }
}

export const lspStore = new LspStore();
