import { invoke } from "@tauri-apps/api/core";
import type { FsEntry } from "$lib/types/fs";

/**
 * Tauri auto-converts Rust's snake_case command parameters to camelCase for
 * the JS `invoke()` payload (e.g. `dest_dir` -> `destDir`). Centralizing
 * every call here means that mapping only has to be gotten right once.
 */

export function openWorkspace(path: string): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("open_workspace", { path });
}

export function readDir(path: string): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("read_dir", { path });
}

export function createEntry(parentDir: string, name: string, isDir: boolean): Promise<FsEntry> {
  return invoke<FsEntry>("create_entry", { parentDir, name, isDir });
}

export function renameEntry(path: string, newName: string): Promise<FsEntry> {
  return invoke<FsEntry>("rename_entry", { path, newName });
}

export function deleteEntries(paths: string[]): Promise<void> {
  return invoke("delete_entries", { paths });
}

export function copyPaths(sources: string[], destDir: string, overwrite = false): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("copy_paths", { sources, destDir, overwrite });
}

export function movePaths(sources: string[], destDir: string, overwrite = false): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("move_paths", { sources, destDir, overwrite });
}

/** Real file open/save — used by the CodeMirror-backed text editor. */
export function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path });
}

export function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke("write_text_file", { path, contents });
}

/** Capped raw-byte read for the binary/hex viewer fallback. */
export function readBinaryPreview(path: string): Promise<number[]> {
  return invoke<number[]>("read_binary_preview", { path });
}

/** Flat, gitignore-aware file index for Quick Open. */
export function listWorkspaceFiles(root: string): Promise<string[]> {
  return invoke<string[]>("list_workspace_files", { root });
}
