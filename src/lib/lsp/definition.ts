import { EditorView } from "@codemirror/view";
import type { Extension, Text } from "@codemirror/state";
import { lspDefinition } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { editorStore } from "$lib/stores/editor.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri, fileUriToPath } from "./uri";
import { offsetToPos } from "./position";
import type { LspLocation, LspRange } from "./types";

interface LspLocationLink {
    targetUri: string;
    targetRange: LspRange;
    targetSelectionRange: LspRange;
}

function isLocationLink(item: LspLocation | LspLocationLink): item is LspLocationLink {
    return "targetUri" in item;
}

function normalizeLocations(result: unknown): LspLocation[] {
    if (!result) return [];
    const items: Array<LspLocation | LspLocationLink> = Array.isArray(result) ? result : [result];
    return items.map((item) =>
        isLocationLink(item) ? { uri: item.targetUri, range: item.targetSelectionRange ?? item.targetRange } : item,
    );
}

/** Requests `textDocument/definition` at `pos` in `filePath` and jumps to
 *  the first result — used by both the Ctrl/Cmd+Click handler and the
 *  "Go to Definition" command (F12 / command palette). Several results
 *  just jump to the first one; a peek/picker UI for "go to any of these"
 *  would be a reasonable future addition, not attempted here. */
export async function goToDefinition(filePath: string, pos: number, doc: Text): Promise<void> {
    const languageId = lspLanguageIdFor(filePath);
    const workspaceRoot = workspace.rootPath;
    if (!languageId || !workspaceRoot) return;

    const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
    const uri = pathToFileUri(filePath);
    const position = offsetToPos(doc, pos);

    let result: unknown;
    try {
        result = await lspDefinition(root, languageId, uri, position);
    } catch (err) {
        console.error("Go to Definition failed:", err);
        return;
    }

    const locations = normalizeLocations(result);
    if (locations.length === 0) return;

    const target = locations[0];
    // LSP's `range.start`/`end` are already `{line, character}` in the
    // *target* file's own coordinates — no need to load that file's `Text`
    // first just to convert a range, the way `posToOffset` elsewhere needs
    // to for the *current* file.
    await editorStore.openAndReveal(fileUriToPath(target.uri), target.range.start.line + 1, target.range.start.character, target.range.end.character);
}

/** Ctrl/Cmd+Click-to-definition — the mouse-driven counterpart to the F12
 *  keybinding registered alongside `baseExtensions`' other keymap
 *  entries in `editor/setup.ts`. */
export function lspDefinitionHandler(filePath: string): Extension {
    return EditorView.domEventHandlers({
        mousedown(event, view) {
            if (!(event.metaKey || event.ctrlKey)) return false;
            const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
            if (pos == null) return false;
            event.preventDefault();
            void goToDefinition(filePath, pos, view.state.doc);
            return true;
        },
    });
}

/** A CodeMirror command (for the F12 keymap entry) closed over `filePath`
 *  — reads the live cursor position at invocation time rather than
 *  taking one as an argument. */
export function lspGoToDefinitionCommand(filePath: string): (view: EditorView) => boolean {
    return (view) => {
        void goToDefinition(filePath, view.state.selection.main.head, view.state.doc);
        return true;
    };
}