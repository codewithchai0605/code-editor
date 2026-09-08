import { listen } from "@tauri-apps/api/event";
import { SvelteMap } from "svelte/reactivity";
import { gitStatus, gitWatch, type GitFileStatus } from "$lib/api/git";

/**
 * Live git status for the current workspace — branch, ahead/behind, and a
 * per-file status map the Explorer tree decorates with. Refreshed both on
 * demand and automatically whenever the backend's `.git`-internals watcher
 * reports a change (a commit, checkout, or stage from *any* program, not
 * just this app — see git.rs's `git_watch`).
 *
 * `byPath` uses `SvelteMap` rather than a plain `$state(new Map())` for the
 * same reason `workspace.svelte.ts` does: a `$state`-wrapped Map isn't
 * deeply reactive to `.set()`/`.clear()`, only to reassignment.
 */
class GitStore {
  isRepo = $state(false);
  branch = $state<string | null>(null);
  ahead = $state(0);
  behind = $state(0);

  private byPath = new SvelteMap<string, GitFileStatus>();
  private currentRoot: string | null = null;
  private listenerStarted = false;

  get changedCount(): number {
    return this.byPath.size;
  }

  statusFor(path: string): GitFileStatus | undefined {
    return this.byPath.get(path);
  }

  async refresh(): Promise<void> {
    if (!this.currentRoot) return;
    try {
      const result = await gitStatus(this.currentRoot);
      this.isRepo = result.isRepo;
      this.branch = result.branch;
      this.ahead = result.ahead;
      this.behind = result.behind;
      this.byPath.clear();
      for (const file of result.files) this.byPath.set(file.path, file);
    } catch {
      // Leave the last known status rather than erroring loudly — a
      // mid-operation git state (e.g. a rebase in progress elsewhere) can
      // transiently make a status call fail without anything being wrong.
    }
  }

  /** Call whenever the workspace root changes — see workspace.svelte.ts's
   *  `open()`. Points the backend's `.git` watcher at the new root (a
   *  harmless no-op there if it isn't a repo) and refreshes immediately. */
  async setRoot(path: string): Promise<void> {
    this.currentRoot = path;
    this.ensureListener();
    await gitWatch(path);
    await this.refresh();
  }

  private ensureListener(): void {
    if (this.listenerStarted) return;
    this.listenerStarted = true;
    listen("git:changed", () => this.refresh());
  }
}

export const gitStore = new GitStore();
