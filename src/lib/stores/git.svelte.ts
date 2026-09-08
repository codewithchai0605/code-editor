import { listen } from "@tauri-apps/api/event";
import { SvelteMap } from "svelte/reactivity";
import { gitStatus, gitWatch, type GitFileStatus } from "$lib/api/git";

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

  /** Windows paths are case-insensitive, and libgit2 is known to normalize
   *  a repo's workdir to a lowercase drive letter regardless of how the
   *  path was originally opened — so even with matching separators,
   *  git_status's paths and the file tree's FsEntry.path can differ only
   *  in case. Lowercasing both sides of every lookup sidesteps that.
   *  Backslash/forward-slash is normalized too as a second safety net. */
  private normalizeKey(path: string): string {
    return path.toLowerCase().replace(/\\/g, "/");
  }

  statusFor(path: string): GitFileStatus | undefined {
    return this.byPath.get(this.normalizeKey(path));
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
      for (const file of result.files) this.byPath.set(this.normalizeKey(file.path), file);
    } catch {
      // Leave the last known status rather than erroring loudly.
    }
  }

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