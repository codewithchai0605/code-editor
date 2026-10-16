import { canonicalPathKey } from "$lib/utils/path";

/** Converts a filesystem path (as returned by the Rust fs commands,
 *  possibly backslash-separated on Windows) to a `file://` URI matching
 *  what `lsp_types::Uri` parses on the Rust side. */
export function pathToFileUri(path: string): string {
    const normalized = path.replace(/\\/g, "/");
    const withLeadingSlash = normalized.startsWith("/") ? normalized : `/${normalized}`;
    return `file://${withLeadingSlash}`;
}

/** The inverse of `pathToFileUri` — a `file://` URI (as sent back by a
 *  language server, e.g. in a `WorkspaceEdit` or `Location`) to a plain
 *  filesystem path suitable for the `fs`/editor APIs. Percent-decodes,
 *  since a server-provided URI may escape characters (spaces, unicode)
 *  that `pathToFileUri` above never needed to escape in the first place
 *  (paths handed to it come straight from the Rust fs commands, never
 *  through a URL).
 *
 *  Also strips the extra leading slash a Windows path picks up from
 *  `pathToFileUri`'s own `file://` + `/C:/...` construction (a Windows
 *  file URI is `file:///C:/Users/...` — three slashes — since the drive
 *  letter needs its own leading slash *in addition to* the two from the
 *  `file://` scheme). Without this, every URI a server sends back for a
 *  Windows path round-trips as `/C:/Users/...`, which every OS API
 *  rejects outright (this was the "os error 123" behind Ctrl+Click). */
export function fileUriToPath(uri: string): string {
    const withoutScheme = uri.replace(/^file:\/\//, "");
    let path: string;
    try {
        path = decodeURIComponent(withoutScheme);
    } catch {
        path = withoutScheme;
    }
    return /^\/[a-zA-Z]:\//.test(path) ? path.slice(1) : path;
}

/** Canonical string key for a `file://` URI, for use as a `Map`/cache key
 *  (diagnostics, anything else keyed by "which file is this about") —
 *  never hand this back to an LSP request, it's a lookup key, not a valid
 *  URI. Two URIs naming the same file always produce the same key even if
 *  a server returned one with different drive-letter case or separators
 *  than this app used when it opened the file — see `canonicalPathKey`
 *  (`$lib/utils/path`) for why that happens. */
export function normalizeUriKey(uri: string): string {
    return canonicalPathKey(fileUriToPath(uri));
}
