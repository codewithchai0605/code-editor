import type { Text } from "@codemirror/state";
import type { LspPosition } from "./types";

/** LSP positions are 0-indexed (line, character, in UTF-16 code units);
 *  CodeMirror's `Text.line()` is 1-indexed and works in document offsets.
 *  Clamped to the document's actual bounds — a server can (rarely) report
 *  a position that's stale relative to what's currently on screen, e.g.
 *  if the file changed since the response was computed. */
export function posToOffset(doc: Text, pos: LspPosition): number {
    if (pos.line < 0) return 0;
    if (pos.line >= doc.lines) return doc.length;
    const line = doc.line(pos.line + 1);
    return Math.max(line.from, Math.min(line.from + pos.character, line.to));
}

/** The inverse of `posToOffset` — a CodeMirror document offset to an LSP
 *  `{line, character}` position, for requests that need to tell the
 *  server where the cursor/selection currently is. */
export function offsetToPos(doc: Text, offset: number): LspPosition {
    const clamped = Math.max(0, Math.min(offset, doc.length));
    const line = doc.lineAt(clamped);
    return { line: line.number - 1, character: clamped - line.from };
}