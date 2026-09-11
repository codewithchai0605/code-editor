import { listen } from "@tauri-apps/api/event";
import { SvelteMap } from "svelte/reactivity";
import { gitListRepos, gitStatus, gitUnwatchAll, type GitFileEntry, type RepoInfo, type RepoStatus } from "$lib/api/git";

/** Lower number = takes priority when a file (or folder aggregating one)
 *  matches more than one status at once. Same convention VS Code's own
 *  folder decorations use. */
const STATUS_PRIORITY: Record<GitFileEntry["status"], number> = {
  conflicted: 0,
  modified: 1,
  added: 2,
  deleted: 3,
  renamed: 4,
  typechange: 5,
  untracked: 6,
};

/**
 * Multi-repo git status for the current workspace — every repo found
 * under it (monorepos, or a workspace that's just several independent
 * repos side by side, plus submodules — see git.rs's scanner), each with
 * its own branch/ahead-behind/file lists, kept live via the backend's
 * per-repo `.git`-internals watchers.
 *
 * Also maintains two *merged*, path-keyed views across every repo at
 * once — `byPath` and `folderStatus` — which is what the Explorer tree's
 * per-entry badges actually read, since a file tree entry doesn't know or
 * care which repo (if any) owns the path it's rendering.
 */
class GitStore {
  repos = $state<RepoInfo[]>([]);
  private statuses = new SvelteMap<string, RepoStatus>();
  private byPath = new SvelteMap<string, GitFileEntry>();
  private folderStatus = new SvelteMap<string, GitFileEntry["status"]>();
  private listenerStarted = false;

  get hasAnyRepo(): boolean {
    return this.repos.length > 0;
  }

  statusOf(repoId: string): RepoStatus | undefined {
    return this.statuses.get(repoId);
  }

  /** Windows paths are case-insensitive, and libgit2 is known to
   *  normalize a repo's workdir to a lowercase drive letter regardless of
   *  how the path was originally opened — lowercasing both sides of every
   *  lookup sidesteps that. Backslash/forward-slash is normalized too. */
  private normalizeKey(path: string): string {
    return path.toLowerCase().replace(/\\/g, "/");
  }

  statusFor(path: string): GitFileEntry | undefined {
    return this.byPath.get(this.normalizeKey(path));
  }

  /** Aggregated status for a folder — undefined if nothing changed
   *  anywhere underneath it, across any repo. */
  folderStatusFor(path: string): GitFileEntry["status"] | undefined {
    return this.folderStatus.get(this.normalizeKey(path));
  }

  /** The repo (if any) that owns the given absolute path — the most
   *  specific (longest-prefix) match, so a submodule nested inside a
   *  parent repo correctly resolves to the submodule itself, not the
   *  parent it's nested in. */
  repoFor(path: string): RepoInfo | undefined {
    const key = this.normalizeKey(path);
    let best: RepoInfo | undefined;
    let bestLen = -1;
    for (const repo of this.repos) {
      const repoKey = this.normalizeKey(repo.path);
      if ((key === repoKey || key.startsWith(`${repoKey}/`)) && repoKey.length > bestLen) {
        best = repo;
        bestLen = repoKey.length;
      }
    }
    return best;
  }

  /** Scans the workspace for every repo under it and fetches each one's
   *  full status. Call once per workspace open (see workspace.svelte.ts). */
  async scanWorkspace(workspacePath: string): Promise<void> {
    this.ensureListener();
    try {
      this.repos = await gitListRepos(workspacePath);
    } catch {
      this.repos = [];
    }
    this.statuses.clear();
    this.byPath.clear();
    this.folderStatus.clear();
    await Promise.all(this.repos.map((repo) => this.refreshRepo(repo.id)));
  }

  async refreshRepo(repoId: string): Promise<void> {
    try {
      const status = await gitStatus(repoId);
      this.applyStatus(status);
    } catch {
      // Leave whatever was last known rather than erroring loudly — a
      // mid-operation git state elsewhere can transiently fail a status
      // call without anything actually being wrong.
    }
  }

  private applyStatus(status: RepoStatus): void {
    this.statuses.set(status.repoId, status);

    const repo = this.repos.find((r) => r.id === status.repoId);
    if (!repo) return;
    const prefix = this.normalizeKey(repo.path);

    // Clear this repo's previous contribution to the merged map before
    // re-adding, so a file that's no longer changed doesn't leave a stale
    // badge behind. Folder aggregation is then a full rebuild from the
    // (already deduplicated) merged file map — simpler and still cheap at
    // realistic change-set sizes than tracking per-folder reference counts.
    for (const key of [...this.byPath.keys()]) {
      if (key === prefix || key.startsWith(`${prefix}/`)) this.byPath.delete(key);
    }

    for (const file of [...status.staged, ...status.unstaged, ...status.untracked, ...status.conflicted]) {
      const key = this.normalizeKey(file.path);
      const existing = this.byPath.get(key);
      if (!existing || STATUS_PRIORITY[file.status] < STATUS_PRIORITY[existing.status]) {
        this.byPath.set(key, file);
      }
    }

    this.folderStatus.clear();
    for (const [key, entry] of this.byPath) {
      this.bubbleToAncestors(key, entry.status);
    }
  }

  private bubbleToAncestors(normalizedFilePath: string, status: GitFileEntry["status"]): void {
    const segments = normalizedFilePath.split("/");
    segments.pop();
    let current = "";
    for (const segment of segments) {
      current = current ? `${current}/${segment}` : segment;
      const existing = this.folderStatus.get(current);
      if (!existing || STATUS_PRIORITY[status] < STATUS_PRIORITY[existing]) {
        this.folderStatus.set(current, status);
      }
    }
  }

  private ensureListener(): void {
    if (this.listenerStarted) return;
    this.listenerStarted = true;
    listen<RepoStatus>("git://status-changed", (event) => this.applyStatus(event.payload));
  }

  /** Call when the workspace is being closed with nothing reopened right
   *  after — opening a *different* folder handles this itself via
   *  `scanWorkspace`, which replaces the backend's tracked repos anyway. */
  async teardown(): Promise<void> {
    this.repos = [];
    this.statuses.clear();
    this.byPath.clear();
    this.folderStatus.clear();
    try {
      await gitUnwatchAll();
    } catch {
      // best-effort
    }
  }
}

export const gitStore = new GitStore();