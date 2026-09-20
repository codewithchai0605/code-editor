/**
 * Mirrors `src-tauri/src/commands/fs.rs`. Keep these two in sync by hand —
 * there's no codegen wiring them together yet.
 */

export interface FsEntry {
    name: string;
    path: string;
    isDir: boolean;
    isSymlink: boolean;
}

/** `create_entry`'s return shape — `name` can be a nested path like
 *  `"lib/date.ts"`, so the entry alone doesn't say whether any folders had
 *  to be created along the way. `createdDirs` is exactly those (root to
 *  leaf, already-existing folders excluded) — see `create_entry` in fs.rs. */
export interface CreateEntryResult {
    entry: FsEntry;
    createdDirs: FsEntry[];
}

/** The `fs:changed` event payload — see `start_watcher` in fs.rs. `dirs` is
 *  parent directories that might need re-listing; `paths` is every exact
 *  path notify saw touched, for matching against open editor tabs. */
export interface FsChangedPayload {
    dirs: string[];
    paths: string[];
}

export type FsErrorKind = "conflict" | "invalidName" | "unsupported" | "io";

/** The `Err` payload Tauri delivers to a rejected `invoke()` call — see the
 *  `#[serde(tag = "kind", ...)]` enum on the Rust side. */
export interface FsError {
    kind: FsErrorKind;
    paths?: string[];
    message?: string;
}

export function isFsError(value: unknown): value is FsError {
    return typeof value === "object" && value !== null && "kind" in value;
}

export interface CopyProgress {
    done: number;
    total: number;
}