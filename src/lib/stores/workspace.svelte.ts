import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
    openWorkspace,
    readDir,
    createEntry,
    renameEntry,
    deleteEntries,
    copyPaths,
    movePaths,
} from "$lib/api/fs";
import { isFsError, type FsChangedPayload, type FsEntry } from "$lib/types/fs";
import { baseName, parentOf } from "$lib/utils/path";
import { describeFsError } from "$lib/utils/fsError";
import { takePendingOpenPath } from "$lib/api/nativeIntegration";
import { gitStore } from "./git.svelte";
import { toastStore } from "./toast.svelte";
import { lspStore } from "$lib/lsp/manager.svelte";

/**
 * Central state for the Explorer: which folder is open, which directories
 * are expanded (with their lazily-fetched children), the current
 * selection/clipboard, and any pending create/rename/conflict UI.
 *
 * Note: `dirs`/`expanded`/`selected` use `SvelteMap`/`SvelteSet` from
 * `svelte/reactivity`, not plain `$state(new Map())` — a `$state`-wrapped
 * Map/Set is NOT deeply reactive to `.set()`/`.add()`/`.delete()` calls in
 * Svelte 5, only to reassignment. `SvelteMap`/`SvelteSet` are the reactive
 * versions built for exactly this in-place-mutation use.
 */

interface DirState {
    children: FsEntry[];
    loading: boolean;
}

type ClipboardMode = "copy" | "cut";

interface PendingConflict {
    sources: string[];
    destDir: string;
    mode: "copy" | "move";
    conflictingPaths: string[];
}

class WorkspaceState {
    rootPath = $state<string | null>(null);
    rootName = $state<string>("");

    private dirs = new SvelteMap<string, DirState>();
    expanded = new SvelteSet<string>();
    selected = new SvelteSet<string>();
    focusedPath = $state<string | null>(null);

    clipboard = $state<{ paths: string[]; mode: ClipboardMode } | null>(null);
    renamingPath = $state<string | null>(null);
    creating = $state<{ parentDir: string; isDir: boolean } | null>(null);
    pendingConflict = $state<PendingConflict | null>(null);
    copyProgress = $state<{ done: number; total: number } | null>(null);
    lastError = $state<string | null>(null);
    private fsListenerStarted = false;
    private openFolderListenerStarted = false;

    /** Sets `lastError` (still read directly in a couple of places) and
     *  surfaces the same message as a toast — a file operation the user
     *  just triggered (rename, delete, copy/move) failing is exactly the
     *  kind of thing worth a visible, if transient, notification. */
    private reportError(err: unknown): void {
        const message = describeFsError(err);
        this.lastError = message;
        toastStore.error(message);
    }

    childrenOf(path: string): FsEntry[] | undefined {
        return this.dirs.get(path)?.children;
    }

    isLoading(path: string): boolean {
        return this.dirs.get(path)?.loading ?? false;
    }

    /** Wires up the backend's `fs:changed` events (emitted by the workspace
     *  file watcher started in fs.rs's `open_workspace`) to re-fetch whatever
     *  directories they touch. `editorStore` listens for the same event
     *  independently (see its own `initFsWatcherListener`) to reload any
     *  open tab whose exact file was touched — kept as two separate
     *  listeners rather than this one reaching into `editorStore` so the
     *  two stores don't end up importing each other. Call once, e.g. from
     *  Shell.svelte's onMount. */
    initFsWatcherListener(): void {
        if (this.fsListenerStarted) return;
        this.fsListenerStarted = true;
        listen<FsChangedPayload>("fs:changed", (event) => {
            for (const dir of event.payload.dirs) this.refresh(dir);
        });
    }

    /** Wires up "this app was launched (or re-launched) to open a specific
     *  folder" — the whole point of a folder's right-click "Open with...".
     *  See native_integration.rs's `take_pending_open_path` / `set_pending_
     *  open_path` for why this both checks a one-shot value *and* listens
     *  for a live event: a cold start's folder argument, and in particular
     *  macOS's `RunEvent::Opened`, can arrive before this listener even
     *  exists, so a live-only listener would silently miss exactly the
     *  "double-click/right-click the app icon with a folder" case this
     *  feature is for. Call once, e.g. from Shell.svelte's onMount. */
    initOpenFolderListener(): void {
        if (this.openFolderListenerStarted) return;
        this.openFolderListenerStarted = true;
        void takePendingOpenPath().then((path) => {
            if (path) void this.open(path);
        });
        listen<{ path: string }>("open-folder-request", (event) => {
            void this.open(event.payload.path);
        });
    }

    // --- open / refresh ----------------------------------------------------

    /** Shows the native folder picker and opens whatever the user chose.
     *  Shared by FileTree's buttons and the Command Palette's "Open Folder"
     *  command, so there's exactly one place that knows how to invoke the
     *  dialog. */
    async pickAndOpenFolder(): Promise<void> {
        const selected = await openDialog({ directory: true, multiple: false });
        if (typeof selected === "string") {
            await this.open(selected);
        }
    }

    async open(path: string): Promise<void> {
        try {
            const entries = await openWorkspace(path);
            this.dirs.clear();
            this.dirs.set(path, { children: entries, loading: false });
            this.expanded.clear();
            this.expanded.add(path);
            this.selected.clear();
            this.rootPath = path;
            this.rootName = baseName(path);
            this.lastError = null;
            // Not awaited — git status shouldn't block the tree from opening
            // (a slow/huge repo's status walk is unrelated to whether the
            // *files* are ready to show), and `setRoot` handles its own errors.
            void gitStore.scanWorkspace(path);
            // Not awaited either — root detection spawns nothing by itself
            // (lazy mode: servers start on first file open, see AGENT.md §9)
            // but it does await an event-listener setup + a filesystem scan;
            // blocking `open()` on it would delay the tree for no reason.
            // This used to be commented out, which silently broke every
            // non-TypeScript/multi-root project: without it `roots` stayed
            // empty and `resolveRootFor` fell back to no root at all.
            void lspStore.startForWorkspace(path);
        } catch (err) {
            this.reportError(err);
        }
    }

    /** Re-fetches a directory's listing if it's currently loaded/expanded.
     *  A no-op for directories nobody has expanded yet — the lazy tree stays
     *  lazy even when the watcher reports changes deep in unopened folders. */
    async refresh(path: string): Promise<void> {
        if (!this.dirs.has(path)) return;
        try {
            const entries = await readDir(path);
            this.dirs.set(path, { children: entries, loading: false });
        } catch {
            // Directory may have just been deleted — leave the last known
            // listing rather than erroring loudly over a benign race.
        }
    }

    async toggleExpand(entry: FsEntry): Promise<void> {
        if (!entry.isDir) return;
        if (this.expanded.has(entry.path)) {
            this.expanded.delete(entry.path);
            return;
        }
        this.expanded.add(entry.path);
        if (this.dirs.has(entry.path)) return;
        this.dirs.set(entry.path, { children: [], loading: true });
        try {
            const entries = await readDir(entry.path);
            this.dirs.set(entry.path, { children: entries, loading: false });
        } catch (err) {
            this.dirs.set(entry.path, { children: [], loading: false });
            this.lastError = describeFsError(err);
        }
    }

    // --- selection -----------------------------------------------------------

    select(path: string, opts: { toggle?: boolean } = {}): void {
        if (opts.toggle) {
            if (this.selected.has(path)) this.selected.delete(path);
            else this.selected.add(path);
        } else {
            this.selected.clear();
            this.selected.add(path);
        }
        this.focusedPath = path;
    }

    // --- create / rename / delete -------------------------------------------

    startCreate(parentDir: string, isDir: boolean): void {
        this.expanded.add(parentDir);
        this.creating = { parentDir, isDir };
    }

    cancelCreate(): void {
        this.creating = null;
    }

    async confirmCreate(name: string): Promise<void> {
        if (!this.creating) return;
        const { parentDir, isDir } = this.creating;
        try {
            const { entry, createdDirs } = await createEntry(parentDir, name, isDir);
            this.creating = null;

            // `name` may itself be a nested path ("lib/date.ts") — fs.rs's
            // `create_entry` reports every intermediate folder it had to
            // make along the way in `createdDirs` (root to leaf, already-
            // existing folders excluded). Walk those and expand+populate
            // each so the tree opens straight down to the new entry,
            // instead of e.g. creating "lib" but leaving it collapsed
            // (and, until expanded, looking empty) for the user to notice
            // and open by hand.
            for (const dir of createdDirs) {
                this.expanded.add(dir.path);
                try {
                    this.dirs.set(dir.path, { children: await readDir(dir.path), loading: false });
                } catch {
                    // Best-effort — a refresh from the watcher will catch
                    // it if this races with something else touching the
                    // same folder.
                }
            }
            await this.refresh(parentDir);
            if (isDir) this.expanded.add(entry.path);

            this.select(entry.path);
        } catch (err) {
            this.reportError(err);
        }
    }

    startRename(path: string): void {
        this.renamingPath = path;
    }

    cancelRename(): void {
        this.renamingPath = null;
    }

    async confirmRename(path: string, newName: string): Promise<void> {
        try {
            await renameEntry(path, newName);
            this.renamingPath = null;
            await this.refresh(parentOf(path));
        } catch (err) {
            this.reportError(err);
        }
    }

    async deleteSelected(): Promise<void> {
        const paths = [...this.selected];
        if (paths.length === 0) return;
        const parents = new Set(paths.map(parentOf));
        try {
            await deleteEntries(paths);
            this.selected.clear();
            for (const parent of parents) await this.refresh(parent);
        } catch (err) {
            this.reportError(err);
        }
    }

    // --- clipboard / copy / move --------------------------------------------

    copySelection(): void {
        if (this.selected.size === 0) return;
        this.clipboard = { paths: [...this.selected], mode: "copy" };
    }

    cutSelection(): void {
        if (this.selected.size === 0) return;
        this.clipboard = { paths: [...this.selected], mode: "cut" };
    }

    async pasteInto(destDir: string): Promise<void> {
        if (!this.clipboard) return;
        const { paths, mode } = this.clipboard;
        const ok = await this.transfer(paths, destDir, mode === "cut" ? "move" : "copy", false);
        if (ok && mode === "cut") this.clipboard = null;
    }

    /** Called on an OS/in-tree drop of `sources` onto `destDir`. */
    async dropInto(sources: string[], destDir: string): Promise<void> {
        await this.transfer(sources, destDir, "move", false);
    }

    /** Files dragged in from outside the app (e.g. Windows Explorer) are
     *  copied in rather than moved — deleting someone's original file just
     *  because they dragged it into the editor would be a nasty surprise. */
    async importPaths(sources: string[], destDir: string): Promise<void> {
        await this.transfer(sources, destDir, "copy", false);
    }

    async resolveConflict(overwrite: boolean): Promise<void> {
        const pending = this.pendingConflict;
        this.pendingConflict = null;
        if (!pending || !overwrite) return;
        await this.transfer(pending.sources, pending.destDir, pending.mode, true);
    }

    /** Returns true on success (including "no-op because nothing to do"),
     *  false if it stopped early on a conflict (a dialog is now pending). */
    private async transfer(
        sources: string[],
        destDir: string,
        mode: "copy" | "move",
        overwrite: boolean,
    ): Promise<boolean> {
        // For a move, anything already directly inside destDir isn't actually
        // going anywhere — including it would make the backend see a "conflict"
        // against the item's own current location. (Copies are left as-is:
        // copying into the same folder legitimately conflicts with the
        // original and should prompt, same as any file manager.)
        const actionable =
            mode === "move" ? sources.filter((s) => parentOf(s) !== destDir) : sources;
        if (actionable.length === 0) return true;

        const affectedDirs = new Set(actionable.map(parentOf));
        affectedDirs.add(destDir);
        try {
            if (mode === "copy") await copyPaths(actionable, destDir, overwrite);
            else await movePaths(actionable, destDir, overwrite);
            for (const dir of affectedDirs) await this.refresh(dir);
            return true;
        } catch (err) {
            if (isFsError(err) && err.kind === "conflict") {
                this.pendingConflict = {
                    sources: actionable,
                    destDir,
                    mode,
                    conflictingPaths: err.paths ?? [],
                };
                return false;
            }
            this.reportError(err);
            return false;
        }
    }
}

export const workspace = new WorkspaceState();