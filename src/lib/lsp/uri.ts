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
 *  through a URL). */
export function fileUriToPath(uri: string): string {
    const withoutScheme = uri.replace(/^file:\/\//, "");
    try {
        return decodeURIComponent(withoutScheme);
    } catch {
        return withoutScheme;
    }
}
