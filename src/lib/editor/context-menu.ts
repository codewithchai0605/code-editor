import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import type { MenuItem } from "$lib/types/menu";
import { toastStore } from "$lib/stores/toast.svelte";
import { lspLanguageIdFor } from "$lib/lsp/languageId";
import { lspGoToDefinitionCommand } from "$lib/lsp/definition";
import { lspRenameCommand } from "$lib/lsp/rename";
import { lspCodeActionCommand } from "$lib/lsp/code-action";
import { formatActiveDocument } from "$lib/lsp/format";
import { editorContextMenuState } from "./context-menu.svelte";

/** The current selection, or — VS Code's own convention for Cut/Copy with
 *  nothing selected — the whole current line including its trailing
 *  newline. Without this, right-clicking with no selection and choosing
 *  "Copy" would silently copy an empty string. */
function selectionOrLine(view: EditorView): { from: number; to: number } {
    const sel = view.state.selection.main;
    if (!sel.empty) return { from: sel.from, to: sel.to };
    const line = view.state.doc.lineAt(sel.head);
    const to = Math.min(line.to + 1, view.state.doc.length);
    return { from: line.from, to };
}

async function copySelection(view: EditorView): Promise<void> {
    const { from, to } = selectionOrLine(view);
    try {
        await navigator.clipboard.writeText(view.state.sliceDoc(from, to));
    } catch (err) {
        toastStore.error(`Copy failed: ${String(err)}`);
    }
}

async function cutSelection(view: EditorView): Promise<void> {
    const { from, to } = selectionOrLine(view);
    try {
        await navigator.clipboard.writeText(view.state.sliceDoc(from, to));
        view.dispatch({ changes: { from, to, insert: "" }, selection: { anchor: from } });
        view.focus();
    } catch (err) {
        toastStore.error(`Cut failed: ${String(err)}`);
    }
}

async function pasteClipboard(view: EditorView): Promise<void> {
    try {
        const text = await navigator.clipboard.readText();
        const sel = view.state.selection.main;
        view.dispatch({
            changes: { from: sel.from, to: sel.to, insert: text },
            selection: { anchor: sel.from + text.length },
        });
        view.focus();
    } catch {
        toastStore.error("Paste failed — this window may not have clipboard-read permission.");
    }
}

function buildMenuItems(filePath: string, view: EditorView): MenuItem[] {
    const items: MenuItem[] = [
        { label: "Cut", action: () => void cutSelection(view) },
        { label: "Copy", action: () => void copySelection(view) },
        { label: "Paste", action: () => void pasteClipboard(view) },
    ];

    // Only offered when this file actually has a language server behind
    // it (see lspLanguageIdFor) — greying these out would need a live
    // per-server capabilities check, which isn't worth it for a menu
    // that's cheap to just not show at all on a plain-text file.
    if (lspLanguageIdFor(filePath)) {
        items.push(
            {
                label: "Go to Definition",
                separatorAbove: true,
                action: () => {
                    lspGoToDefinitionCommand(filePath)(view);
                },
            },
            {
                label: "Rename Symbol",
                action: () => {
                    lspRenameCommand(filePath)(view);
                },
            },
            {
                label: "Quick Fix…",
                action: () => {
                    lspCodeActionCommand(filePath)(view);
                },
            },
            {
                label: "Format Document",
                action: () => void formatActiveDocument(),
            },
        );
    }

    return items;
}

/** Right-click support for the editor. CodeMirror's own content area isn't
 *  a native `<textarea>`/`contenteditable` form control with a built-in
 *  menu of its own — without this, right-clicking falls through to the
 *  surrounding webview's *native* context menu instead (on Chromium/WRY,
 *  the generic "editable text field" one, complete with items like
 *  "Emoji" and "Voice typing" that have nothing to do with a code
 *  editor).
 *
 *  Reuses the same `ContextMenu.svelte` the file tree's and the
 *  code-action menu's right-click already render through, via the
 *  dedicated `editorContextMenuState` store (mounted once in
 *  `Shell.svelte` — see that store's doc comment for why this needs a
 *  global store rather than a component-owned one).
 *
 *  A right-click outside the current selection moves the caret there
 *  first, matching the native/VS Code convention, so Cut/Copy act on
 *  what was actually right-clicked rather than a leftover selection
 *  elsewhere in the file. */
export function editorContextMenu(filePath: string): Extension {
    return EditorView.domEventHandlers({
        contextmenu(event, view) {
            event.preventDefault();
            const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
            if (pos != null) {
                const sel = view.state.selection.main;
                if (pos < sel.from || pos > sel.to) {
                    view.dispatch({ selection: { anchor: pos } });
                }
            }
            editorContextMenuState.open(
                event.clientX,
                event.clientY,
                buildMenuItems(filePath, view),
            );
            return true;
        },
    });
}
