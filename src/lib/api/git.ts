import { invoke } from "@tauri-apps/api/core";

export interface GitFileEntry {
  path: string;
  status: "modified" | "added" | "deleted" | "renamed" | "typechange" | "untracked" | "conflicted";
}

export interface RepoInfo {
  id: string;
  path: string;
  name: string;
  currentBranch: string | null;
  ahead: number;
  behind: number;
}

export interface RepoStatus {
  repoId: string;
  branch: string | null;
  ahead: number;
  behind: number;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
  conflicted: GitFileEntry[];
}

/** Scans the given workspace root for every repo under it (handles
 *  monorepos / multiple independent repos in one workspace, and
 *  submodules — see git.rs), starts a watcher for each, and returns their
 *  lightweight header info. Call `gitStatus` per repo id for the full
 *  file lists. */
export function gitListRepos(workspacePath: string): Promise<RepoInfo[]> {
  return invoke<RepoInfo[]>("git_list_repos", { workspacePath });
}

export function gitStatus(repoId: string): Promise<RepoStatus> {
  return invoke<RepoStatus>("git_status", { repoId });
}

export function gitStageFile(repoId: string, path: string): Promise<void> {
  return invoke("git_stage_file", { repoId, path });
}

export function gitStageAll(repoId: string): Promise<void> {
  return invoke("git_stage_all", { repoId });
}

export function gitUnstageFile(repoId: string, path: string): Promise<void> {
  return invoke("git_unstage_file", { repoId, path });
}

export function gitUnstageAll(repoId: string): Promise<void> {
  return invoke("git_unstage_all", { repoId });
}

/** Irreversible — caller must confirm with the user before invoking this. */
export function gitDiscardFile(repoId: string, path: string): Promise<void> {
  return invoke("git_discard_file", { repoId, path });
}

/** Same irreversibility note as `gitDiscardFile`. */
export function gitDiscardAll(repoId: string): Promise<void> {
  return invoke("git_discard_all", { repoId });
}

export interface CommitResult {
  oid: string;
}

export function gitCommit(repoId: string, message: string): Promise<CommitResult> {
  return invoke<CommitResult>("git_commit", { repoId, message });
}

export function gitUnwatchAll(): Promise<void> {
  return invoke("git_unwatch_all");
}