/** Works on both `/`- and `\`-separated paths, since the backend can hand us
 *  either depending on platform. */

const WINDOWS_ABSOLUTE_PATH = /^[a-zA-Z]:[\\/]/;

/** Canonical string key for a filesystem path: forward-slashed, and (only
 *  for paths that look like a Windows absolute path — a drive letter, e.g.
 *  `C:\...` or `c:/...`) lowercased, since only Windows filesystems are
 *  case-insensitive. This is a *comparison/lookup key*, not a path to hand
 *  back to the filesystem or display to the user — callers that need an
 *  equality check or a map key should use this (or `pathsEqual`/
 *  `isPathUnder` below); callers that need an actual path should keep
 *  using the original string.
 *
 *  This exists because a language server doesn't promise to echo back a
 *  path/URI with the same drive-letter case or separator style it was
 *  given — e.g. typescript-language-server's own definition/reference
 *  results commonly lowercase the drive letter. Comparing those with `===`
 *  makes the *same* file look like a different one (duplicate tabs, a
 *  workspace-edit landing on disk instead of an already-open buffer,
 *  diagnostics keyed to the wrong cache entry, ...). This mirrors
 *  `paths_equal`/`workspace_key` in the Rust backend
 *  (`src-tauri/src/lsp/manager.rs` / `registry.rs`), which fixed the same
 *  class of bug for workspace roots but never had an equivalent here. */
export function canonicalPathKey(path: string): string {
    const slashed = path.replace(/\\/g, "/");
    return WINDOWS_ABSOLUTE_PATH.test(path) ? slashed.toLowerCase() : slashed;
}

/** True if `a` and `b` name the same file, tolerant of `/` vs `\` and (on
 *  a Windows-looking path only) drive-letter/segment case. Use this
 *  instead of `===` anywhere a path might have come from a language
 *  server or another external source instead of this app's own file
 *  dialogs/explorer. */
export function pathsEqual(a: string, b: string): boolean {
    return canonicalPathKey(a) === canonicalPathKey(b);
}

/** True if `path` is `root` itself or nested under it, with the same
 *  tolerance as `pathsEqual`. */
export function isPathUnder(root: string, path: string): boolean {
    const r = canonicalPathKey(root).replace(/\/+$/, "");
    const p = canonicalPathKey(path);
    return p === r || p.startsWith(`${r}/`);
}

export function baseName(path: string): string {
    const trimmed = path.replace(/[\\/]+$/, "");
    const idx = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    return idx >= 0 ? trimmed.slice(idx + 1) : trimmed;
}

export function parentOf(path: string): string {
    const trimmed = path.replace(/[\\/]+$/, "");
    const idx = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    return idx > 0 ? trimmed.slice(0, idx) : trimmed;
}
