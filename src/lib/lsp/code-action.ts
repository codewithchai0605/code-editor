import type { EditorView } from "@codemirror/view";
import { lspCodeAction, lspExecuteCommand } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos } from "./position";
import { getDiagnosticsForUri } from "./diagnostics";
import { applyWorkspaceEdit } from "./workspace-edit";
import { codeActionMenuState } from "./code-action-menu.svelte";
import { toastStore } from "$lib/stores/toast.svelte";
import type { LspCodeActionOrCommand, LspCommand, LspRange } from "./types";
import type { MenuItem } from "$lib/types/menu";

/** A bare `Command` (`{title, command, arguments?}`, `command` a string)
 *  vs. a `CodeAction` (`command` — if present at all — is a nested
 *  `Command` *object*, not a string) are the two shapes a
 *  `CodeActionOrCommand` entry can be; `typeof .command === "string"` is
 *  what actually distinguishes them per the LSP spec, not anything
 *  TypeScript's structural typing alone can narrow. */
function isBareCommand(item: LspCodeActionOrCommand): item is LspCommand {
    return typeof (item as LspCommand).command === "string";
}

function rangesOverlap(a: LspRange, b: LspRange): boolean {
    const key = (p: { line: number; character: number }) => p.line * 100_000 + p.character;
    return key(a.start) <= key(b.end) && key(b.start) <= key(a.end);
}

async function runCommand(
    root: string,
    languageId: string,
    command: LspCommand,
    actionTitle: string,
): Promise<void> {
    try {
        await lspExecuteCommand(root, languageId, command.command, command.arguments ?? []);
    } catch (err) {
        toastStore.error(`Failed to run "${actionTitle}": ${String(err)}`);
    }
}

async function applyChosenAction(
    root: string,
    languageId: string,
    action: LspCodeActionOrCommand,
): Promise<void> {
    if (isBareCommand(action)) {
        await runCommand(root, languageId, action, action.title);
        return;
    }

    if (action.edit) {
        try {
            await applyWorkspaceEdit(action.edit);
        } catch (err) {
            toastStore.error(`Failed to apply "${action.title}": ${String(err)}`);
            return;
        }
    }
    if (action.command) {
        await runCommand(root, languageId, action.command, action.title);
    }
}

async function showCodeActions(filePath: string, view: EditorView): Promise<void> {
    const languageId = lspLanguageIdFor(filePath);
    const workspaceRoot = workspace.rootPath;
    if (!languageId || !workspaceRoot) return;

    const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
    const uri = pathToFileUri(filePath);
    const selection = view.state.selection.main;
    const range: LspRange = {
        start: offsetToPos(view.state.doc, selection.from),
        end: offsetToPos(view.state.doc, selection.to || selection.from),
    };
    const diagnostics = getDiagnosticsForUri(uri).filter((d) => rangesOverlap(d.range, range));

    let actions: LspCodeActionOrCommand[];
    try {
        actions = (await lspCodeAction(root, languageId, uri, range, diagnostics)) ?? [];
    } catch (err) {
        console.error("textDocument/codeAction failed:", err);
        return;
    }
    if (actions.length === 0) {
        toastStore.info("No code actions available here.");
        return;
    }

    const coords = view.coordsAtPos(selection.head) ?? { left: 0, bottom: 0 };
    const items: MenuItem[] = actions.map((action) => ({
        label: action.title,
        action: () => void applyChosenAction(root, languageId, action),
    }));
    codeActionMenuState.open(coords.left, coords.bottom, items);
}

/** A CodeMirror command (for the Ctrl+. keymap entry): fetches
 *  `textDocument/codeAction` for the current selection (or just the
 *  cursor position, collapsed to a zero-width range) and shows the
 *  results in a context menu — reusing the same `ContextMenu.svelte` the
 *  file tree's right-click menu uses, via the global `codeActionMenuState`
 *  store (see its doc comment for why this needs a global store rather
 *  than a component-owned one). */
export function lspCodeActionCommand(filePath: string): (view: EditorView) => boolean {
    return (view) => {
        void showCodeActions(filePath, view);
        return true;
    };
}
