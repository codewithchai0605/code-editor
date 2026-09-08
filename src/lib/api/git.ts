import { invoke } from "@tauri-apps/api/core";

export interface GitFileStatus {
  path: string;
  status: "modified" | "added" | "deleted" | "renamed" | "untracked" | "conflicted";
  staged: boolean;
}

export interface GitStatusResult {
  isRepo: boolean;
  branch: string | null;
  ahead: number;
  behind: number;
  files: GitFileStatus[];
}

export function gitStatus(path: string): Promise<GitStatusResult> {
  return invoke<GitStatusResult>("git_status", { path });
}

/** Starts (or re-points, if already watching a different repo) the
 *  backend's `.git`-internals watcher — see git.rs. Safe to call on a
 *  folder that isn't a git repo at all; the backend just returns without
 *  starting a watch. */
export function gitWatch(path: string): Promise<void> {
  return invoke("git_watch", { path });
}

export function gitUnwatch(): Promise<void> {
  return invoke("git_unwatch");
}
