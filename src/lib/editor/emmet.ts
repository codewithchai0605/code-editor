// The `emmet` package ships real .d.ts files but its package.json exports
// map doesn't declare a `types` condition, so TS can't resolve them under
// `moduleResolution: "bundler"` — see src/lib/types/emmet.d.ts for a
// small hand-written shim covering just what's used here.
import expandAbbreviation, { extract } from "emmet";
import type { Command } from "@codemirror/view";

/** Extension -> emmet syntax name, split by which of emmet's two abbreviation
 *  grammars applies (markup vs. stylesheet — they parse differently, e.g.
 *  stylesheet syntax doesn't support markup's `[]`/`{}` attribute syntax). */
const MARKUP_SYNTAX: Record<string, string> = {
  html: "html",
  htm: "html",
  xml: "xml",
  svg: "html",
  jsx: "jsx",
  tsx: "jsx",
};
const STYLESHEET_SYNTAX: Record<string, string> = {
  css: "css",
};

/** Builds a Tab-triggered Emmet-expand command for the given file
 *  extension, or `null` if Emmet doesn't apply to it — callers should
 *  fall through to normal Tab handling (indent) in that case rather than
 *  binding a command with nothing to do. */
export function emmetExpandCommand(extension: string): Command | null {
  const markupSyntax = MARKUP_SYNTAX[extension];
  const stylesheetSyntax = STYLESHEET_SYNTAX[extension];
  if (!markupSyntax && !stylesheetSyntax) return null;

  const syntax = markupSyntax ?? stylesheetSyntax;
  const type: "markup" | "stylesheet" = markupSyntax ? "markup" : "stylesheet";

  return (view) => {
    const { from, to } = view.state.selection.main;
    if (from !== to) return false; // let Tab handle an active selection normally (indent-more)

    const line = view.state.doc.lineAt(from);
    const posInLine = from - line.from;

    // `extract` finds the abbreviation immediately before the cursor —
    // e.g. in "<body>ul>li*3|</body>" (cursor at `|`) it correctly pulls
    // out just "ul>li*3", handling emmet's own bracket/quote nesting far
    // more reliably than a hand-rolled regex would.
    const extracted = extract(line.text, posInLine, { type });
    if (!extracted) return false;

    let expanded: string;
    try {
      expanded = expandAbbreviation(extracted.abbreviation, { syntax });
    } catch {
      return false; // not a valid abbreviation -- let Tab fall through to normal indent
    }
    if (expanded === extracted.abbreviation) return false; // nothing meaningful to expand

    const insertFrom = line.from + extracted.start;
    const insertTo = line.from + extracted.end;
    view.dispatch({
      changes: { from: insertFrom, to: insertTo, insert: expanded },
      selection: { anchor: insertFrom + expanded.length },
    });
    return true;
  };
}
