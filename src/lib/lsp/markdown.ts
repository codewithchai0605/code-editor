import { renderMarkdownToSafeHtml } from "$lib/markdown/render";

/** Renders LSP hover/signature-help content (Markdown, per the
 *  `hover.content_format` capability this client declares — see
 *  `src-tauri/src/lsp/manager.rs`'s `client_capabilities`) into `container`.
 *
 *  Reuses the app's existing Markdown pipeline (`markdown/render.ts` —
 *  `markdown-it` + `highlight.js`, sanitized through DOMPurify) rather
 *  than a second, LSP-specific one: it's a real CommonMark parser (lists,
 *  headers, links, tables — a hand-rolled regex renderer only ever covers
 *  "bold and inline code and fenced blocks"), it already covers more
 *  languages for fenced-code highlighting than this app's own CodeMirror
 *  language packages do (bash/shell in particular, common in hover docs),
 *  and its output already renders through the same `.hljs-*` classes
 *  `app.css` colors to match the active editor theme — so a hover
 *  tooltip's code blocks and the Markdown preview's code blocks are
 *  visually consistent with each other for free, not just internally
 *  consistent with themselves. `innerHTML` here is safe for the same
 *  reason it's safe in `MarkdownPreview.svelte`'s `{@html ...}` binding:
 *  `renderMarkdownToSafeHtml` already ran the output through DOMPurify. */
export function renderLspMarkdown(container: HTMLElement, text: string): void {
    container.innerHTML = renderMarkdownToSafeHtml(text);
}
