import { EditorView, showTooltip, type Tooltip } from "@codemirror/view";
import { StateField, StateEffect, type Extension } from "@codemirror/state";
import { lspRename } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos } from "./position";
import { applyWorkspaceEdit } from "./workspace-edit";
import { toastStore } from "$lib/stores/toast.svelte";
import type { LspWorkspaceEdit } from "./types";

const setRenameTooltip = StateEffect.define<Tooltip | null>();

/** Module-level singleton — same reasoning as `signatureHelp.ts`'s
 *  `signatureTooltipField`: every tab needs to share the same field
 *  instance for `setRenameTooltip` to be a valid effect target regardless
 *  of which tab is active when F2 is pressed. */
const renameTooltipField = StateField.define<Tooltip | null>({
    create: () => null,
    update(value, tr) {
        for (const effect of tr.effects) {
            if (effect.is(setRenameTooltip)) return effect.value;
        }
        // Any other doc change — including the rename's own applied edit —
        // invalidates whatever position this was anchored to.
        if (tr.docChanged) return null;
        return value;
    },
    provide: (field) => showTooltip.from(field),
});

function wordRangeAt(view: EditorView, pos: number): { from: number; to: number } {
    const line = view.state.doc.lineAt(pos);
    const text = line.text;
    const offset = pos - line.from;
    const isWordChar = (ch: string) => /[\w$]/.test(ch);
    let from = offset;
    let to = offset;
    while (from > 0 && isWordChar(text[from - 1])) from--;
    while (to < text.length && isWordChar(text[to])) to++;
    return { from: line.from + from, to: line.from + to };
}

async function performRename(view: EditorView, filePath: string, pos: number, newName: string): Promise<void> {
    const languageId = lspLanguageIdFor(filePath);
    const workspaceRoot = workspace.rootPath;
    if (!languageId || !workspaceRoot) return;

    const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
    const uri = pathToFileUri(filePath);
    const position = offsetToPos(view.state.doc, pos);

    let edit: LspWorkspaceEdit | null;
    try {
        edit = await lspRename(root, languageId, uri, position, newName);
    } catch (err) {
        toastStore.error(`Rename failed: ${String(err)}`);
        return;
    }
    if (!edit) return;

    try {
        await applyWorkspaceEdit(edit);
        toastStore.success(`Renamed to "${newName}"`);
    } catch (err) {
        toastStore.error(`Failed to apply the rename: ${String(err)}`);
    }
}

/** A CodeMirror command (for the F2 keymap entry): shows a small inline
 *  input, anchored at the symbol under the cursor via the same `Tooltip`
 *  mechanism hover/signature-help use, pre-filled with its current name.
 *  Enter requests `textDocument/rename` and applies the result; Escape or
 *  losing focus cancels without asking the server anything. */
export function lspRenameCommand(filePath: string): (view: EditorView) => boolean {
    return (view) => {
        const pos = view.state.selection.main.head;
        const { from, to } = wordRangeAt(view, pos);
        const currentName = view.state.sliceDoc(from, to);

        const input = document.createElement("input");
        input.className = "cm-lsp-rename-input";
        input.value = currentName;
        input.size = Math.max(4, currentName.length + 2);

        const close = () => view.dispatch({ effects: setRenameTooltip.of(null) });

        input.addEventListener("keydown", (event) => {
            event.stopPropagation();
            if (event.key === "Enter") {
                event.preventDefault();
                const newName = input.value.trim();
                close();
                if (newName && newName !== currentName) void performRename(view, filePath, from, newName);
            } else if (event.key === "Escape") {
                event.preventDefault();
                close();
            }
        });
        input.addEventListener("blur", close);

        view.dispatch({
            effects: setRenameTooltip.of({
                pos: from,
                above: true,
                create: () => {
                    // The tooltip's DOM isn't attached/focusable synchronously
                    // within `create()` itself — deferring to a microtask is the
                    // standard CodeMirror pattern for "focus a tooltip's own input
                    // the moment it appears".
                    queueMicrotask(() => {
                        input.focus();
                        input.select();
                    });
                    return { dom: input };
                },
            }),
        });

        return true;
    };
}

/** Included once per tab (alongside `signatureTooltipField` etc.) in
 *  `editor/setup.ts`'s `baseExtensions` — see the module doc comment on
 *  why this needs to be a shared singleton rather than built fresh
 *  per-file. */
export function lspRenameExtension(): Extension {
    return renameTooltipField;
}