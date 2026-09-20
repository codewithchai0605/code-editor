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
