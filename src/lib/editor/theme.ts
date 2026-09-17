import { EditorView } from "@codemirror/view";
import { HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

/**
 * Colors approximate VS Code's "Dark+" palette. Referencing our own CSS
 * custom properties (var(--color-...)) for the editor chrome means this
 * stays in sync with app.css automatically; the syntax colors below are
 * fixed hex values since they're theme-specific rather than UI-chrome
 * colors — a real theme *system* is later (settings) phase work.
 */
export const highlightStyle = HighlightStyle.define([
    { tag: [t.comment, t.lineComment, t.blockComment], color: "#6a9955", fontStyle: "italic" },
    { tag: t.keyword, color: "#569cd6" },
    { tag: t.controlKeyword, color: "#c586c0" },
    { tag: t.operatorKeyword, color: "#569cd6" },
    { tag: [t.name, t.deleted, t.character, t.propertyName, t.macroName], color: "#9cdcfe" },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "#dcdcaa" },
    { tag: [t.typeName, t.className, t.namespace, t.annotation], color: "#4ec9b0" },
    { tag: [t.number, t.bool, t.null], color: "#b5cea8" },
    { tag: [t.string, t.special(t.string)], color: "#ce9178" },
    { tag: t.regexp, color: "#d16969" },
    { tag: [t.operator, t.punctuation], color: "#d4d4d4" },
    { tag: t.definition(t.variableName), color: "#9cdcfe" },
    { tag: t.self, color: "#569cd6" },
    { tag: t.tagName, color: "#569cd6" },
    { tag: t.attributeName, color: "#9cdcfe" },
    { tag: t.heading, fontWeight: "bold", color: "#569cd6" },
    { tag: t.strong, fontWeight: "bold" },
    { tag: t.emphasis, fontStyle: "italic" },
    { tag: t.strikethrough, textDecoration: "line-through" },
    { tag: t.link, color: "#4ec9b0", textDecoration: "underline" },
    { tag: t.invalid, color: "#f44747" },
]);

export const editorTheme = EditorView.theme(
    {
        "&": {
            color: "var(--color-text)",
            backgroundColor: "var(--color-bg)",
            height: "100%",
            fontSize: "13px",
        },
        "&.cm-focused": { outline: "none" },
        ".cm-content": {
            fontFamily: "var(--font-mono)",
            caretColor: "var(--color-text)",
            padding: "8px 0",
        },
        ".cm-scroller": {
            fontFamily: "var(--font-mono)",
            lineHeight: "1.5",
        },
        ".cm-selectionBackground, .cm-content ::selection": {
            backgroundColor: "rgba(38, 79, 120, 0.6) !important",
        },
        ".cm-activeLine": { backgroundColor: "rgba(255, 255, 255, 0.04)" },
        ".cm-activeLineGutter": { backgroundColor: "rgba(255, 255, 255, 0.06)" },
        ".cm-gutters": {
            backgroundColor: "var(--color-bg)",
            color: "var(--color-text-faint)",
            border: "none",
        },
        ".cm-lineNumbers .cm-gutterElement": { padding: "0 12px 0 8px" },
        ".cm-searchMatch": { backgroundColor: "rgba(255, 200, 0, 0.25)" },
        ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "rgba(255, 200, 0, 0.5)" },
        ".cm-matchingBracket, .cm-nonmatchingBracket": {
            backgroundColor: "rgba(255, 255, 255, 0.15)",
            outline: "1px solid var(--color-text-faint)",
        },
        ".cm-tooltip": {
            backgroundColor: "var(--color-bg-elevated)",
            border: "1px solid var(--color-border)",
        },
        ".cm-tooltip-autocomplete ul li[aria-selected]": {
            backgroundColor: "var(--color-accent)",
            color: "#fff",
        },
    },
    { dark: true },
);
