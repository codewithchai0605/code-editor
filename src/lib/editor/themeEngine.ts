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
            ".cm-searchMatch.cm-searchMatch-selected": {
                backgroundColor: "rgba(255, 200, 0, 0.5)",
            },
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
            ".cm-lsp-hover": {
                padding: "6px 8px",
                maxWidth: "480px",
                fontSize: "12.5px",
                lineHeight: "1.5",
            },
            ".cm-lsp-hover pre": {
                margin: "4px 0",
                padding: "4px 6px",
                backgroundColor: c.bg,
                borderRadius: "4px",
                overflowX: "auto",
            },
            ".cm-lsp-hover code": {
                fontFamily: "var(--font-mono)",
                fontSize: "12px",
            },
            ".cm-lsp-signature-help": {
                padding: "6px 8px",
                fontSize: "12.5px",
                fontFamily: "var(--font-mono)",
                maxWidth: "480px",
            },
            ".cm-lsp-signature-help .cm-lsp-sig-active-param": {
                fontWeight: "bold",
                color: c.accent,
            },
            ".cm-lsp-signature-help .cm-lsp-sig-doc": {
                marginTop: "4px",
                fontFamily: "var(--font-ui)",
                color: c.textMuted,
                whiteSpace: "normal",
            },
            ".cm-lsp-rename-input": {
                fontFamily: "var(--font-mono)",
                fontSize: "13px",
                padding: "2px 4px",
                border: `1px solid ${c.accent}`,
                borderRadius: "3px",
                backgroundColor: c.bg,
                color: c.text,
                outline: "none",
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
 *  integrated terminal re-tints along with everything else — including
 *  ANSI colors (used by programs that print in color), which previously
 *  stayed a fixed dark palette regardless of the active theme. A light
 *  theme with a dark-terminal ANSI set looked broken (dark text on a
 *  light background with no contrast tuning); deriving every slot from
 *  `theme.colors` — reusing the same hues the syntax highlighter already
 *  uses for keywords/strings/etc. — keeps the terminal visually part of
 *  the same theme instead of a bolted-on VS-Code-flavored constant. */
export function buildTerminalTheme(theme: Theme): Record<string, string> {
    const c = theme.colors;
    return {
        background: c.bg,
        foreground: c.text,
        cursor: c.text,
        cursorAccent: c.bg,
        selectionBackground: c.selection,
        // "Black" still needs to read as text against the terminal's own
        // background, so it's a muted foreground shade rather than literally
        // black (which would be invisible on a dark theme and match the
        // background exactly on some light ones).
        black: c.textFaint,
        red: c.invalid,
        green: c.string,
        yellow: c.number,
        blue: c.keyword,
        magenta: c.controlKeyword,
        cyan: c.type,
        white: c.text,
        brightBlack: c.textMuted,
        brightRed: c.invalid,
        brightGreen: c.string,
        brightYellow: c.number,
        // Distinct from `blue` the same way VS Code's own scheme separates
        // keyword-blue from variable-blue — `variable` reads as a lighter,
        // more saturated blue in every shipped theme.
        brightBlue: c.variable,
        brightMagenta: c.controlKeyword,
        brightCyan: c.type,
        brightWhite: theme.dark ? "#ffffff" : c.text,
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
