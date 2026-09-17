import { EditorState } from "@codemirror/state";
import { readTextFile, writeTextFile, createEntry, renameEntry, deleteEntries } from "$lib/api/fs";
import { editorStore } from "$lib/stores/editor.svelte";
import { fileUriToPath } from "./uri";
import { posToOffset } from "./position";
import type { LspTextEdit, LspWorkspaceEdit } from "./types";

interface Change {
    from: number;
    to: number;
    insert: string;
}

function textEditsToChanges(text: string, edits: LspTextEdit[]): Change[] {
    // A full `EditorState` is more than this needs, but it's the one place
    // that already handles every line-ending flavor (`\n`, `\r\n`, `\r`)
    // correctly and consistently with how every open tab's own `Text` was
    // built in the first place (`editor.svelte.ts`'s `open()`) — not worth
    // re-deriving that logic here just to avoid one extra object.
    const doc = EditorState.create({ doc: text }).doc;
    return edits.map((edit) => {
        const from = posToOffset(doc, edit.range.start);
        const to = Math.max(from, posToOffset(doc, edit.range.end));
        return { from, to, insert: edit.newText };
    });
}

async function applyEditsToFile(path: string, edits: LspTextEdit[]): Promise<void> {
    if (edits.length === 0) return;

    const openText = editorStore.docTextIfOpen(path);
    if (openText !== null) {
        // Every change's `from`/`to` is computed against the *same*, original
        // `openText` — CodeMirror's `changes` array applies a batch of
        // non-overlapping edits against one base document in one step, so
        // there's no need to sort or offset-adjust them relative to each
        // other the way the disk-file path below does.
        editorStore.applyChangesToTab(path, textEditsToChanges(openText, edits));
        return;
    }

    // Not open: read-modify-write directly on disk. Applied back-to-front
    // by start offset so each splice doesn't invalidate the offsets the
    // next one still needs.
    const text = await readTextFile(path);
    const changes = textEditsToChanges(text, edits).sort((a, b) => b.from - a.from);
    let next = text;
    for (const change of changes) {
        next = next.slice(0, change.from) + change.insert + next.slice(change.to);
    }
    await writeTextFile(path, next);
}

/** Applies a `WorkspaceEdit` — the result of an accepted rename or code
 *  action — across however many files it touches, whether or not each one
 *  currently has an open tab (an open tab's edit goes through a real
 *  CodeMirror transaction and is left dirty for review, matching how any
 *  other edit behaves; a file with no open tab is read, patched, and
 *  written straight back since there's no buffer to show as dirty).
 *
 *  Prefers `documentChanges` (the versioned, ordered form some servers
 *  send) over the older flat `changes` map when both happen to be
 *  present, per the LSP spec's own guidance that `documentChanges` wins
 *  when a client supports it (this one does).
 *
 *  Resource operations (`create`/`rename`/`delete` entries inside
 *  `documentChanges`) are applied too, via the same `fs` commands the
 *  file tree uses — but without the spec's optional per-operation
 *  `options` (`overwrite`, `ignoreIfExists`, `recursive`) honored, since
 *  the current `fs` API surface doesn't expose those knobs yet. Uncommon
 *  in practice (most servers' code actions only ever edit existing
 *  files' text), so this is a deliberate, documented gap rather than a
 *  silent one — a failed resource operation is logged and skipped rather
 *  than aborting the rest of the edit. */
export async function applyWorkspaceEdit(edit: LspWorkspaceEdit): Promise<void> {
    if (edit.documentChanges && edit.documentChanges.length > 0) {
        for (const change of edit.documentChanges) {
            if ("textDocument" in change) {
                await applyEditsToFile(fileUriToPath(change.textDocument.uri), change.edits);
                continue;
            }
            try {
                if (change.kind === "create") {
                    const path = fileUriToPath(change.uri);
                    const name = path.split("/").pop() ?? path;
                    const parent = path.slice(0, path.length - name.length - 1);
                    await createEntry(parent, name, false);
                } else if (change.kind === "rename") {
                    const oldPath = fileUriToPath(change.oldUri);
                    const newName = fileUriToPath(change.newUri).split("/").pop() ?? "";
                    await renameEntry(oldPath, newName);
                } else if (change.kind === "delete") {
                    await deleteEntries([fileUriToPath(change.uri)]);
                }
            } catch (err) {
                console.error("Failed to apply a workspace-edit resource operation:", change, err);
            }
        }
        return;
    }

    for (const [uri, edits] of Object.entries(edit.changes ?? {})) {
        await applyEditsToFile(fileUriToPath(uri), edits);
    }
}
