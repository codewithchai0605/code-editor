import { hoverTooltip } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { lspHover } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos, posToOffset } from "./position";
import type { LspPosition, LspRange } from "./types";

interface MarkupContent {
    kind: string;
    value: string;
}

interface LspHoverResult {
    contents: string | MarkupContent | Array<string | MarkupContent>;
    range?: LspRange;
}

function hoverText(contents: LspHoverResult["contents"]): string {
    if (typeof contents === "string") return contents;
    if (Array.isArray(contents)) {
        return contents.map((entry) => (typeof entry === "string" ? entry : entry.value)).join("\n\n---\n\n");
    }
    return contents.value;
}

/** Minimal, deliberately-not-a-real-markdown-renderer for hover content:
 *  fenced code blocks become `<pre><code>`, inline `` `code` `` and
 *  `**bold**` become their matching elements, everything else is plain
 *  text. Built with `createElement`/`textContent` throughout rather than
 *  any `innerHTML` — hover content comes from a language server process,
 *  and there's no reason to parse it as HTML just to render what's really
 *  just typed signatures and doc comments. */
function renderMarkdownish(container: HTMLElement, text: string): void {
    const withCodeBlocks = text.split(/```(?:\w*)\n?([\s\S]*?)```/g);
    // A capturing-group split interleaves [plain, code, plain, code, ..., plain].
    withCodeBlocks.forEach((chunk, i) => {
        if (i % 2 === 0) {
            if (chunk) appendInline(container, chunk);
        } else {
            const pre = document.createElement("pre");
            const code = document.createElement("code");
            code.textContent = chunk.replace(/\n$/, "");
            pre.appendChild(code);
            container.appendChild(pre);
        }
    });
}

function appendInline(container: HTMLElement, text: string): void {
    const lines = text.split("\n");
    lines.forEach((line, i) => {
        if (i > 0) container.appendChild(document.createElement("br"));
        const segments = line.split(/(`[^`]+`|\*\*[^*]+\*\*)/g);
        for (const segment of segments) {
            if (!segment) continue;
            if (segment.startsWith("`") && segment.endsWith("`")) {
                const code = document.createElement("code");
                code.textContent = segment.slice(1, -1);
                container.appendChild(code);
            } else if (segment.startsWith("**") && segment.endsWith("**")) {
                const strong = document.createElement("strong");
                strong.textContent = segment.slice(2, -2);
                container.appendChild(strong);
            } else {
                container.appendChild(document.createTextNode(segment));
            }
        }
    });
}

export function lspHoverTooltip(filePath: string): Extension {
    return hoverTooltip(
        async (view, pos) => {
            const languageId = lspLanguageIdFor(filePath);
            const workspaceRoot = workspace.rootPath;
            if (!languageId || !workspaceRoot) return null;

            const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
            const uri = pathToFileUri(filePath);
            const position: LspPosition = offsetToPos(view.state.doc, pos);

            let result: LspHoverResult | null;
            try {
                result = await lspHover(root, languageId, uri, position);
            } catch {
                return null;
            }
            if (!result?.contents) return null;

            const text = hoverText(result.contents);
            if (!text.trim()) return null;

            let start = pos;
            let end = pos;
            if (result.range) {
                start = posToOffset(view.state.doc, result.range.start);
                end = Math.max(start, posToOffset(view.state.doc, result.range.end));
            }

            return {
                pos: start,
                end,
                above: true,
                create() {
                    const dom = document.createElement("div");
                    dom.className = "cm-lsp-hover";
                    renderMarkdownish(dom, text);
                    return { dom };
                },
            };
        },
        { hoverTime: 300 },
    );
}