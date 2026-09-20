import { hoverTooltip } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { lspHover } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos, posToOffset } from "./position";
import { renderLspMarkdown } from "./markdown";
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
        return contents
            .map((entry) => (typeof entry === "string" ? entry : entry.value))
            .join("\n\n---\n\n");
    }
    return contents.value;
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
                    const scroll = document.createElement("div");
                    scroll.className = "cm-lsp-hover-scroll";
                    renderLspMarkdown(scroll, text);
                    dom.appendChild(scroll);
                    return { dom };
                },
            };
        },
        { hoverTime: 300 },
    );
}
