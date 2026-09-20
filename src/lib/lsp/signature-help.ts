import {
    EditorView,
    ViewPlugin,
    type ViewUpdate,
    type Tooltip,
    showTooltip,
} from "@codemirror/view";
import { StateField, StateEffect, type Extension } from "@codemirror/state";
import { lspSignatureHelp } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos } from "./position";
import { renderLspMarkdown } from "./markdown";

interface MarkupContent {
    kind: string;
    value: string;
}

interface ParameterInformation {
    label: string | [number, number];
    documentation?: string | MarkupContent;
}

interface SignatureInformation {
    label: string;
    documentation?: string | MarkupContent;
    parameters?: ParameterInformation[];
    activeParameter?: number;
}

interface SignatureHelpResult {
    signatures: SignatureInformation[];
    activeSignature?: number;
    activeParameter?: number;
}

/** A reasonable language-agnostic default for "just typed a character
 *  that plausibly opens or advances an argument list" — the same
 *  trade-off `completion.ts` makes with its own default trigger set:
 *  less precise than each server's declared
 *  `signatureHelpProvider.triggerCharacters`, but not dependent on
 *  capabilities having already arrived. */
const TRIGGER_CHARS = new Set(["(", ","]);

const setSignatureTooltip = StateEffect.define<Tooltip | null>();

/** Module-level singleton, included in every tab's extensions the same
 *  way `themeCompartment` is — a `StateField` only works as an effect
 *  target if every state sharing this app's one live `EditorView`
 *  actually has the *same* field instance, not a structurally-identical
 *  but distinct one built fresh per tab. */
const signatureTooltipField = StateField.define<Tooltip | null>({
    create: () => null,
    update(value, tr) {
        for (const effect of tr.effects) {
            if (effect.is(setSignatureTooltip)) return effect.value;
        }
        // The cursor moved or the doc changed without an explicit effect this
        // time around — don't leave a tooltip pinned to what's now a stale
        // position; the plugin below will issue a fresh request and either
        // put up a new one or leave it cleared.
        if (tr.docChanged || tr.selection) return null;
        return value;
    },
    provide: (field) => showTooltip.from(field),
});

function activeParamRange(
    signature: SignatureInformation,
    activeParameter: number | undefined,
): [number, number] | null {
    if (activeParameter === undefined) return null;
    const param = signature.parameters?.[activeParameter];
    if (!param) return null;
    if (Array.isArray(param.label)) return param.label;
    const idx = signature.label.indexOf(param.label);
    return idx >= 0 ? [idx, idx + param.label.length] : null;
}

function documentationText(doc: string | MarkupContent | undefined): string | undefined {
    if (!doc) return undefined;
    return typeof doc === "string" ? doc : doc.value;
}

function buildDom(result: SignatureHelpResult): HTMLElement {
    const dom = document.createElement("div");
    dom.className = "cm-lsp-signature-help";

    const signature = result.signatures[result.activeSignature ?? 0];
    if (!signature) return dom;

    const activeParameter = signature.activeParameter ?? result.activeParameter;
    const range = activeParamRange(signature, activeParameter);

    const labelEl = document.createElement("div");
    labelEl.className = "cm-lsp-sig-label";
    if (range) {
        labelEl.append(
            document.createTextNode(signature.label.slice(0, range[0])),
            Object.assign(document.createElement("span"), {
                className: "cm-lsp-sig-active-param",
                textContent: signature.label.slice(range[0], range[1]),
            }),
            document.createTextNode(signature.label.slice(range[1])),
        );
    } else {
        labelEl.textContent = signature.label;
    }
    dom.appendChild(labelEl);

    const activeDoc =
        activeParameter !== undefined
            ? documentationText(signature.parameters?.[activeParameter]?.documentation)
            : documentationText(signature.documentation);
    if (activeDoc) {
        const docEl = document.createElement("div");
        docEl.className = "cm-lsp-sig-doc";
        renderLspMarkdown(docEl, activeDoc);
        dom.appendChild(docEl);
    }

    return dom;
}

function buildPlugin(filePath: string) {
    return ViewPlugin.fromClass(
        class {
            generation = 0;

            update(update: ViewUpdate): void {
                if (!update.docChanged && !update.selectionSet) return;

                this.generation++;
                const myGeneration = this.generation;
                const view = update.view;
                const pos = view.state.selection.main.head;

                const typedChar = update.docChanged
                    ? view.state.sliceDoc(Math.max(0, pos - 1), pos)
                    : "";
                const alreadyShowing = view.state.field(signatureTooltipField, false) != null;
                if (!TRIGGER_CHARS.has(typedChar) && !alreadyShowing) return;

                void this.fetch(view, pos, myGeneration);
            }

            async fetch(view: EditorView, pos: number, myGeneration: number): Promise<void> {
                const languageId = lspLanguageIdFor(filePath);
                const workspaceRoot = workspace.rootPath;
                if (!languageId || !workspaceRoot) return;

                const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
                const uri = pathToFileUri(filePath);
                const position = offsetToPos(view.state.doc, pos);

                let result: SignatureHelpResult | null;
                try {
                    result = await lspSignatureHelp(root, languageId, uri, position);
                } catch {
                    result = null;
                }
                if (myGeneration !== this.generation) return; // superseded by a newer request or edit

                if (!result || result.signatures.length === 0) {
                    view.dispatch({ effects: setSignatureTooltip.of(null) });
                    return;
                }

                const dom = buildDom(result);
                view.dispatch({
                    effects: setSignatureTooltip.of({ pos, above: true, create: () => ({ dom }) }),
                });
            }
        },
    );
}

export function lspSignatureHelpTooltip(filePath: string): Extension {
    return [signatureTooltipField, buildPlugin(filePath)];
}
