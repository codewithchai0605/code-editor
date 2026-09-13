import { invoke } from "@tauri-apps/api/core";
import type { ReplaceFileResult, SearchOptions } from "$lib/types/search";

/**
 * Fire-and-forget: the command spawns a background thread and returns
 * immediately (see search.rs) — all actual matches arrive via the
 * `search:results` / `search:done` events, not this call's resolution.
 * Rejects synchronously only for a bad regex/glob, caught before any
 * thread is spawned.
 */
export function workspaceSearch(searchId: string, root: string, options: SearchOptions): Promise<void> {
    return invoke("workspace_search", { searchId, root, options });
}

/** Cancels whatever search is currently in flight without starting a new one. */
export function stopWorkspaceSearch(): Promise<void> {
    return invoke("stop_workspace_search");
}

export function workspaceReplaceAll(
    paths: string[],
    options: SearchOptions,
    replacement: string,
): Promise<ReplaceFileResult[]> {
    return invoke<ReplaceFileResult[]>("workspace_replace_all", { paths, options, replacement });
}