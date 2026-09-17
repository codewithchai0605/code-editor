import { EditorState } from "@codemirror/state";
import { lspFormatting } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { editorStore } from "$lib/stores/editor.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { posToOffset } from "./position";
import { toastStore } from "$lib/stores/toast.svelte";
import type { LspTextEdit } from "./types";

/** "Format Document" — registered as a command (see `coreCommands.ts`)
 *  rather than a CodeMirror keymap entry, since it needs to act on
 *  whichever tab is active without needing a `view`/`filePath` closed
 *  over ahead of time the way the per-tab LSP extensions do. */
export async function formatActiveDocument(): Promise<void> {
    const tab = editorStore.activeTab;
    const workspaceRoot = workspace.rootPath;
    if (!tab || tab.kind !== "text" || !tab.cmState || !workspaceRoot) return;

    const languageId = lspLanguageIdFor(tab.path);
    if (!languageId) {
        toastStore.info("No language server available to format this file.");
        return;
    }

    const root = lspStore.resolveRootFor(tab.path, languageId, workspaceRoot);
    const uri = pathToFileUri(tab.path);
    // `insertSpaces: true` is a simplification — there's no per-file
    // tabs-vs-spaces setting tracked anywhere yet (only `EditorState.tabSize`,
    // which is what a tab *displays* as, not what indentation a formatter
    // should *write*). `tabSize` itself is read from the real per-tab
    // facet rather than a hardcoded default.
    const tabSize = tab.cmState.facet(EditorState.tabSize);

    let edits: LspTextEdit[] | null;
    try {
        edits = await lspFormatting(root, languageId, uri, tabSize, true);
    } catch (err) {
        toastStore.error(`Format failed: ${String(err)}`);
        return;
    }
    if (!edits || edits.length === 0) return;

    const doc = tab.cmState.doc;
    const changes = edits.map((edit) => {
        const from = posToOffset(doc, edit.range.start);
        const to = Math.max(from, posToOffset(doc, edit.range.end));
        return { from, to, insert: edit.newText };
    });
    editorStore.applyChangesToTab(tab.path, changes);
}
