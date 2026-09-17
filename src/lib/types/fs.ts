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
