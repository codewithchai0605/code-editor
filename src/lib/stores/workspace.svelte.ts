import { SvelteMap, SvelteSet } from "svelte/reactivity";
import {
  openWorkspace,
  readDir,
  createEntry,
  renameEntry,
  deleteEntries,
  copyPaths,
  movePaths,
} from "$lib/api/fs";
import { isFsError, type FsEntry } from "$lib/types/fs";
import { baseName, parentOf } from "$lib/utils/path";

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

function describeError(err: unknown): string {
  if (isFsError(err)) {
    return err.message ?? "Something went wrong.";
  }
  return err instanceof Error ? err.message : String(err);
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

  childrenOf(path: string): FsEntry[] | undefined {
    return this.dirs.get(path)?.children;
  }

  isLoading(path: string): boolean {
    return this.dirs.get(path)?.loading ?? false;
  }

  // --- open / refresh ----------------------------------------------------

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
    } catch (err) {
      this.lastError = describeError(err);
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
      this.lastError = describeError(err);
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
    const { parentDir } = this.creating;
    try {
      await createEntry(parentDir, name, this.creating.isDir);
      this.creating = null;
      await this.refresh(parentDir);
    } catch (err) {
      this.lastError = describeError(err);
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
      this.lastError = describeError(err);
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
      this.lastError = describeError(err);
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
      this.lastError = describeError(err);
      return false;
    }
  }
}

export const workspace = new WorkspaceState();
