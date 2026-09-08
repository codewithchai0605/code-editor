import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";
import type { Theme } from "./themes/types";

/**
 * Builds the CodeMirror side (editor chrome + syntax colors) from a
 * `Theme`. Every theme goes through this same function — there's no way
 * for a new theme to end up with mismatched chrome/syntax colors the way
 * the original dracula.ts did (its `dracula()` export combined the
 * *default* theme's chrome with Dracula's syntax HighlightStyle, since
 * building the EditorView.theme by hand made it easy to reference the
 * wrong constant).
 */
export function buildCmTheme(theme: Theme): Extension {
  const c = theme.colors;

  const editorTheme = EditorView.theme(
    {
      "&": {
        color: c.text,
        backgroundColor: c.bg,
        height: "100%",
        fontSize: "13px",
      },
      "&.cm-focused": { outline: "none" },
      ".cm-content": {
        fontFamily: "var(--font-mono)",
        caretColor: c.text,
        padding: "8px 0",
      },
      ".cm-scroller": {
        fontFamily: "var(--font-mono)",
        lineHeight: "1.5",
      },
      ".cm-selectionBackground, .cm-content ::selection": {
        backgroundColor: `${c.selection} !important`,
      },
      ".cm-activeLine": { backgroundColor: c.activeLine },
      ".cm-activeLineGutter": { backgroundColor: c.activeLine },
      ".cm-gutters": {
        backgroundColor: c.bg,
        color: c.textFaint,
        border: "none",
      },
      ".cm-lineNumbers .cm-gutterElement": { padding: "0 12px 0 8px" },
      ".cm-searchMatch": { backgroundColor: "rgba(255, 200, 0, 0.25)" },
      ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "rgba(255, 200, 0, 0.5)" },
      ".cm-matchingBracket, .cm-nonmatchingBracket": {
        backgroundColor: "rgba(255, 255, 255, 0.15)",
        outline: `1px solid ${c.textFaint}`,
      },
      ".cm-tooltip": {
        backgroundColor: c.bgElevated,
        border: `1px solid ${c.border}`,
      },
      ".cm-tooltip-autocomplete ul li[aria-selected]": {
        backgroundColor: c.accent,
        color: "#fff",
      },
      ".cm-panels": {
        backgroundColor: c.bgElevated,
        color: c.text,
      },
      ".cm-panels.cm-panels-top": { borderBottom: `1px solid ${c.border}` },
    },
    { dark: theme.dark },
  );

  const highlightStyle = HighlightStyle.define([
    { tag: [t.comment, t.lineComment, t.blockComment], color: c.comment, fontStyle: "italic" },
    { tag: t.keyword, color: c.keyword },
    { tag: t.controlKeyword, color: c.controlKeyword },
    { tag: t.operatorKeyword, color: c.keyword },
    { tag: [t.name, t.deleted, t.character, t.propertyName, t.macroName], color: c.variable },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], color: c.function },
    { tag: [t.typeName, t.className, t.namespace, t.annotation], color: c.type },
    { tag: [t.number, t.bool, t.null], color: c.number },
    { tag: [t.string, t.special(t.string)], color: c.string },
    { tag: t.regexp, color: c.regexp },
    { tag: [t.operator, t.punctuation], color: c.operator },
    { tag: t.definition(t.variableName), color: c.variable },
    { tag: t.self, color: c.keyword },
    { tag: t.tagName, color: c.tag },
    { tag: t.attributeName, color: c.attribute },
    { tag: t.heading, fontWeight: "bold", color: c.keyword },
    { tag: t.strong, fontWeight: "bold" },
    { tag: t.emphasis, fontStyle: "italic" },
    { tag: t.strikethrough, textDecoration: "line-through" },
    { tag: t.link, color: c.type, textDecoration: "underline" },
    { tag: t.invalid, color: c.invalid },
  ]);

  return [editorTheme, syntaxHighlighting(highlightStyle)];
}

/** Derives an xterm.js `ITheme`-shaped object from the same palette, so the
 *  integrated terminal re-tints along with everything else. ANSI colors
 *  (used by programs that print in color) are a fixed, reasonably
 *  standard dark palette shared across themes for now — only the base
 *  background/foreground/cursor/selection vary per theme. */
export function buildTerminalTheme(theme: Theme): Record<string, string> {
  const c = theme.colors;
  return {
    background: c.bg,
    foreground: c.text,
    cursor: c.text,
    cursorAccent: c.bg,
    selectionBackground: c.selection,
    black: "#1e1e1e",
    red: "#f44747",
    green: "#6a9955",
    yellow: "#d7ba7d",
    blue: "#569cd6",
    magenta: "#c586c0",
    cyan: "#4ec9b0",
    white: c.text,
    brightBlack: c.textFaint,
    brightRed: "#f44747",
    brightGreen: "#b5cea8",
    brightYellow: "#dcdcaa",
    brightBlue: "#9cdcfe",
    brightMagenta: "#c586c0",
    brightCyan: "#4ec9b0",
    brightWhite: "#ffffff",
  };
}

/** Maps `Theme.colors` onto the same CSS custom properties app.css defines
 *  on `:root` — every shell component (Sidebar, ActivityBar, StatusBar,
 *  tabs, dialogs...) already reads colors through these variables, so
 *  overriding them here is what makes the *whole app*, not just the code
 *  editor, re-tint instantly. */
export function applyAppColors(theme: Theme): void {
  const c = theme.colors;
  const root = document.documentElement.style;
  root.setProperty("--color-bg", c.bg);
  root.setProperty("--color-bg-elevated", c.bgElevated);
  root.setProperty("--color-bg-sidebar", c.bgSidebar);
  root.setProperty("--color-bg-activitybar", c.bgActivitybar);
  root.setProperty("--color-bg-statusbar", c.bgStatusbar);
  root.setProperty("--color-statusbar-text", c.statusbarText);
  root.setProperty("--color-bg-hover", c.bgHover);
  root.setProperty("--color-border", c.border);
  root.setProperty("--color-text", c.text);
  root.setProperty("--color-text-muted", c.textMuted);
  root.setProperty("--color-text-faint", c.textFaint);
  root.setProperty("--color-accent", c.accent);
  root.setProperty("--color-focus-ring", c.focusRing);
}
