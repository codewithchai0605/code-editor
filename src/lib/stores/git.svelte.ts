import { listen } from "@tauri-apps/api/event";
import { SvelteMap } from "svelte/reactivity";
import { gitStatus, gitWatch, type GitFileStatus } from "$lib/api/git";

/** Lower number = takes priority when a folder contains several kinds of
 *  changes at once — e.g. a folder with both a modified and an untracked
 *  file inside shows as "modified", the more attention-worthy of the two.
 *  Same convention VS Code's own folder decorations use. */
const STATUS_PRIORITY: Record<GitFileStatus["status"], number> = {
  conflicted: 0,
  modified: 1,
  added: 2,
  deleted: 3,
  renamed: 4,
  untracked: 5,
};

class GitStore {
  isRepo = $state(false);
  branch = $state<string | null>(null);
  ahead = $state(0);
  behind = $state(0);

  private byPath = new SvelteMap<string, GitFileStatus>();
  private folderStatus = new SvelteMap<string, GitFileStatus["status"]>();
  private currentRoot: string | null = null;
  private listenerStarted = false;

  get changedCount(): number {
    return this.byPath.size;
  }

  private normalizeKey(path: string): string {
    return path.toLowerCase().replace(/\\/g, "/");
  }

  statusFor(path: string): GitFileStatus | undefined {
    return this.byPath.get(this.normalizeKey(path));
  }

  /** Aggregated status for a folder — undefined if nothing changed
   *  anywhere underneath it. */
  folderStatusFor(path: string): GitFileStatus["status"] | undefined {
    return this.folderStatus.get(this.normalizeKey(path));
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
      this.folderStatus.clear();
      for (const file of result.files) {
        const key = this.normalizeKey(file.path);
        this.byPath.set(key, file);
        this.bubbleToAncestors(key, file.status);
      }
    } catch {
      // Leave the last known status rather than erroring loudly.
    }
  }

  /** Walks every ancestor directory of a changed file's path and records
   *  this status there if it outranks whatever that ancestor already
   *  has — so a folder several levels above a single changed file still
   *  lights up, all the way to the workspace root. */
  private bubbleToAncestors(normalizedFilePath: string, status: GitFileStatus["status"]): void {
    const segments = normalizedFilePath.split("/");
    segments.pop(); // the file itself — only its ancestor dirs get marked
    let current = "";
    for (const segment of segments) {
      current = current ? `${current}/${segment}` : segment;
      const existing = this.folderStatus.get(current);
      if (!existing || STATUS_PRIORITY[status] < STATUS_PRIORITY[existing]) {
        this.folderStatus.set(current, status);
      }
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