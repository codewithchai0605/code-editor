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
    /** TypeScript's non-standard "Expandable Hover" extension (mirroring
     *  VS Code's own experimental `typescript.experimental.expandableHover`
     *  — the `+`/`-` control that expands a bare `interface Foo` hover
     *  into its full member list). `undefined` on any server that doesn't
     *  support it — including plain-spec-compliant `textDocument/hover`
     *  responses — which is exactly what's used below to decide whether
     *  to show the expand/collapse control at all.
     *
     *  Note there is no `canDecreaseVerbosityLevel` counterpart:
     *  typescript-language-server's own wire protocol for this only ever
     *  reports the increase direction (see its docs/configuration.md) —
     *  whether "-" should be enabled is tracked client-side instead (see
     *  `createVerbosityToolbar`), the same way VS Code's own built-in TS
     *  hover provider tracks its own `lastHoverAndLevel` rather than
     *  asking the server. */
    canIncreaseVerbosityLevel?: boolean;
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

/** The small "-"/"+" footer VS Code shows on an expandable hover. Only
 *  ever constructed once the *first* response has already told us the
 *  server supports this (see the `undefined` check at the call site) —
 *  every server that doesn't gets exactly the same hover UI as before
 *  this feature existed.
 *
 *  `update` takes the *current* level as well as the latest result: "-"
 *  is enabled purely from the client's own tracked level (anything above
 *  the level-0 baseline can always be decreased back down), since the
 *  server never reports a "can decrease" flag of its own — see
 *  `LspHoverResult`'s doc comment. */
function createVerbosityToolbar(
    onAdjust: (delta: 1 | -1) => void,
): { dom: HTMLElement; update: (result: LspHoverResult, level: number) => void } {
    const dom = document.createElement("div");
    dom.className = "cm-lsp-hover-toolbar";

    const decreaseBtn = document.createElement("button");
    decreaseBtn.type = "button";
    decreaseBtn.className = "cm-lsp-hover-verbosity-btn";
    decreaseBtn.textContent = "−";
    decreaseBtn.title = "Show less";
    decreaseBtn.addEventListener("click", (event) => {
        event.preventDefault();
        onAdjust(-1);
    });

    const increaseBtn = document.createElement("button");
    increaseBtn.type = "button";
    increaseBtn.className = "cm-lsp-hover-verbosity-btn";
    increaseBtn.textContent = "+";
    increaseBtn.title = "Show more";
    increaseBtn.addEventListener("click", (event) => {
        event.preventDefault();
        onAdjust(1);
    });

    dom.append(decreaseBtn, increaseBtn);

    return {
        dom,
        update(result, level) {
            decreaseBtn.disabled = level <= 0;
            increaseBtn.disabled = !result.canIncreaseVerbosityLevel;
        },
    };
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

            // Only offered when the server actually signaled it supports
            // adjustable verbosity on *this* hover — a request with no
            // `verbosityLevel` is assumed to have come back at level 0,
            // which is what the first increase/decrease request is
            // computed relative to.
            const supportsVerbosity = result.canIncreaseVerbosityLevel !== undefined;
            let level = 0;

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

                    if (supportsVerbosity) {
                        const toolbar = createVerbosityToolbar((delta) => {
                            void (async () => {
                                const nextLevel = level + delta;
                                let next: LspHoverResult | null;
                                try {
                                    next = await lspHover(
                                        root,
                                        languageId,
                                        uri,
                                        position,
                                        nextLevel,
                                    );
                                } catch {
                                    return;
                                }
                                if (!next?.contents) return;
                                level = nextLevel;
                                renderLspMarkdown(scroll, hoverText(next.contents));
                                toolbar.update(next, level);
                            })();
                        });
                        toolbar.update(result, level);
                        dom.appendChild(toolbar.dom);
                    }

                    return { dom };
                },
            };
        },
        { hoverTime: 300 },
    );
}
