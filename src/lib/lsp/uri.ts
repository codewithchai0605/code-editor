/** Converts a filesystem path (as returned by the Rust fs commands,
 *  possibly backslash-separated on Windows) to a `file://` URI matching
 *  what `lsp_types::Uri` parses on the Rust side. */
export function pathToFileUri(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const withLeadingSlash = normalized.startsWith("/") ? normalized : `/${normalized}`;
  return `file://${withLeadingSlash}`;
}
