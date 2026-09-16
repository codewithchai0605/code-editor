import {
    EditorSelection,
    StateEffect,
    StateField,
    Text,
    type Extension,
} from "@codemirror/state";
import { EditorView, gutter, GutterMarker } from "@codemirror/view";
import { Chunk } from "@codemirror/merge";
import { gitStore } from "$lib/stores/git.svelte";
import { diffTabStore } from "$lib/stores/diffTab.svelte";

/**
 * VS Code's own "dirty diff" gutter — the thin colored bar next to the
 * line numbers that shows which lines differ from git's index, live as
 * you type (see the `dark_modern` theme's `editorGutter.*` colors, which
 * these match exactly). Deliberately its own module, independent of
 * `setup.ts`'s other editor features: the diffing itself is generic
 * (any two `Text`s), and everything here that *is* git-specific (opening
 * a diff tab on click) is confined to the click handler at the bottom.
 *
 * The "original" side of the diff is set from outside via `setGitBaseline`
 * — see `editor.svelte.ts`'s `refreshGitBaseline`, which fetches it from
 * the backend's index blob (empty for an untracked file, so the whole
 * buffer reads as newly added) and re-fetches it whenever that file's repo
 * reports a status change (stage/unstage/commit/external change).
 */
export const setGitBaseline = StateEffect.define<Text | null>();

interface GitGutterState {
    baseline: Text | null;
    chunks: readonly Chunk[];
}

const DIFF_CONFIG = { scanLimit: 750, timeout: 300 };

const emptyState: GitGutterState = { baseline: null, chunks: [] };

const gitGutterField = StateField.define<GitGutterState>({
    create() {
        return emptyState;
    },
    update(value, tr) {
        let next = value;
        let baselineChanged = false;
        for (const effect of tr.effects) {
            if (effect.is(setGitBaseline)) {
                const baseline = effect.value;
                next = {
                    baseline,
                    chunks: baseline ? Chunk.build(baseline, tr.state.doc, DIFF_CONFIG) : [],
                };
                baselineChanged = true;
            }
        }
        if (!baselineChanged && tr.docChanged && next.baseline) {
            next = {
                baseline: next.baseline,
                chunks: Chunk.updateB(next.chunks, next.baseline, tr.state.doc, tr.changes, DIFF_CONFIG),
            };
        }
        return next;
    },
});

/** Whether `path` has an active baseline in `view` — used to skip the
 *  fetch-and-dispatch round trip for files that can never have one (no
 *  owning repo), and by `TextEditor.svelte`/commands that only make sense
 *  once there's something to compare against. */
export function hasGitChanges(view: EditorView): boolean {
    return (view.state.field(gitGutterField, false)?.chunks.length ?? 0) > 0;
}

const GUTTER_CLASS = "cm-git-gutter-marker";

class ChangeMarker extends GutterMarker {
    constructor(
        private readonly kind: "added" | "modified" | "deleted",
        private readonly atDocEnd = false,
    ) {
        super();
    }
    eq(other: GutterMarker): boolean {
        return (
            other instanceof ChangeMarker && other.kind === this.kind && other.atDocEnd === this.atDocEnd
        );
    }
    toDOM(): HTMLElement {
        const el = document.createElement("div");
        el.className = `${GUTTER_CLASS} ${GUTTER_CLASS}-${this.kind}`;
        if (this.atDocEnd) el.className += ` ${GUTTER_CLASS}-end`;
        el.title =
            this.kind === "added"
                ? "Added line"
                : this.kind === "modified"
                    ? "Modified line"
                    : "Line(s) removed here";
        return el;
    }
}

// One instance per (kind, atDocEnd) combination — GutterMarker instances
// are compared with `eq`, not identity, but reusing these avoids
// allocating a fresh marker on every single-line redraw.
const addedMarker = new ChangeMarker("added");
const modifiedMarker = new ChangeMarker("modified");
const deletedMarker = new ChangeMarker("deleted");
const deletedEndMarker = new ChangeMarker("deleted", true);

function markerForLine(
    chunks: readonly Chunk[],
    lineFrom: number,
    lineTo: number,
    docLength: number,
): GutterMarker | null {
    let changeKind: "added" | "modified" | null = null;
    let deletionAtStart = false;
    let deletionAtDocEnd = false;

    for (const chunk of chunks) {
        if (chunk.fromB === chunk.toB) {
            // A pure deletion has no lines of its own in B — it's a "point"
            // sitting right where the removed lines used to be. Pin its marker
            // to the line that now immediately follows that point, same as
            // VS Code's own wedge indicator; if the deletion is the very last
            // thing in the document, there's no following line, so it goes on
            // the bottom edge of the last line instead.
            if (chunk.fromB === lineFrom) {
                deletionAtStart = true;
            } else if (chunk.fromB >= docLength && lineTo >= docLength) {
                deletionAtDocEnd = true;
            }
            continue;
        }
        if (lineFrom < chunk.toB && lineTo > chunk.fromB) {
            changeKind = chunk.fromA === chunk.toA ? "added" : "modified";
        }
    }

    if (changeKind === "added") return addedMarker;
    if (changeKind === "modified") return modifiedMarker;
    if (deletionAtStart) return deletedMarker;
    if (deletionAtDocEnd) return deletedEndMarker;
    return null;
}

const changeGutter = gutter({
    class: "cm-git-gutter",
    lineMarker(view, line) {
        const field = view.state.field(gitGutterField, false);
        if (!field || !field.chunks.length) return null;
        return markerForLine(field.chunks, line.from, line.to, view.state.doc.length);
    },
    lineMarkerChange(update) {
        return update.state.field(gitGutterField, false) !== update.startState.field(gitGutterField, false);
    },
    domEventHandlers: {
        mousedown(view) {
            openDiffFor(view);
            return true;
        },
    },
});

/** Opens the working-tree diff for `path` in a new diff tab — the same
 *  view the Source Control panel's own row click opens, so clicking a
 *  change in the gutter and clicking the same file in the sidebar land in
 *  the same place. A no-op if `path` isn't inside any known repo (the
 *  gutter has nothing to show in that case anyway). */
function openDiffFor(view: EditorView): void {
    const path = view.state.field(gitGutterPath, false);
    if (!path) return;
    const repo = gitStore.repoFor(path);
    if (!repo) return;
    void diffTabStore.open(repo.id, path, false);
}

/** Carries this editor's own file path into the gutter's click handler —
 *  a StateField rather than a closure so it survives `setState` (a fresh
 *  closure captured at extension-build time would go stale the moment a
 *  different tab's state gets swapped into a shared `EditorView`, which is
 *  exactly what `TextEditor.svelte` does on every tab switch). */
const gitGutterPath = StateField.define<string | null>({
    create: () => null,
    update: (value) => value,
});

const gitGutterTheme = EditorView.baseTheme({
    ".cm-git-gutter": {
        width: "7px",
    },
    ".cm-git-gutter .cm-gutterElement": {
        padding: "0",
    },
    [`.${GUTTER_CLASS}`]: {
        boxSizing: "border-box",
        width: "3px",
        height: "100%",
        margin: "0 auto",
        cursor: "pointer",
        borderRadius: "1px",
        background: "transparent",
    },
    [`.${GUTTER_CLASS}:hover`]: {
        width: "5px",
    },
    [`.${GUTTER_CLASS}-added`]: {
        background: "#2ea043",
    },
    [`.${GUTTER_CLASS}-modified`]: {
        background: "#0078d4",
    },
    [`.${GUTTER_CLASS}-deleted`]: {
        position: "relative",
    },
    [`.${GUTTER_CLASS}-deleted::before`]: {
        content: '""',
        position: "absolute",
        top: "-4px",
        left: "-2px",
        width: "0",
        height: "0",
        borderStyle: "solid",
        borderWidth: "4px 0 4px 6px",
        borderColor: "transparent transparent transparent #f85149",
    },
    [`.${GUTTER_CLASS}-deleted-end::before`]: {
        top: "auto",
        bottom: "-4px",
    },
});

/** Everything a text tab needs for its dirty-diff gutter: the state that
 *  tracks the baseline/chunks, the gutter column itself, and its theme.
 *  Included unconditionally for every text tab (not just ones that
 *  already know they're in a repo at open time) — `gitStore`'s workspace
 *  scan can still be in flight when a file opens, and a `StateField`
 *  can't be added to an `EditorState` after the fact, so the extension is
 *  always present and simply renders nothing until a baseline effect
 *  actually arrives. */
export function gitGutter(filePath: string): Extension[] {
    return [
        gitGutterField,
        gitGutterPath.init(() => filePath),
        changeGutter,
        gitGutterTheme,
    ];
}

/** Moves the cursor to the start of the next changed chunk (wrapping back
 *  to the first once past the last), same behavior as VS Code's Go to
 *  Next Change. Returns `false` (a no-op) if this file has no baseline or
 *  no changes, matching the other `@codemirror/commands`-style command
 *  functions this app already binds directly in `setup.ts`'s keymap. */
export function goToNextGitChange(view: EditorView): boolean {
    const field = view.state.field(gitGutterField, false);
    if (!field?.chunks.length) return false;
    const pos = view.state.selection.main.head;
    const target = field.chunks.find((c) => c.fromB > pos) ?? field.chunks[0];
    return jumpTo(view, target.fromB);
}

/** The reverse of `goToNextGitChange` — VS Code's Go to Previous Change. */
export function goToPreviousGitChange(view: EditorView): boolean {
    const field = view.state.field(gitGutterField, false);
    if (!field?.chunks.length) return false;
    const pos = view.state.selection.main.head;
    const chunks = field.chunks;
    let target = chunks[chunks.length - 1];
    for (let i = chunks.length - 1; i >= 0; i--) {
        if (chunks[i].fromB < pos) {
            target = chunks[i];
            break;
        }
    }
    return jumpTo(view, target.fromB);
}

function jumpTo(view: EditorView, docPos: number): boolean {
    const pos = Math.min(docPos, view.state.doc.length);
    view.dispatch({
        selection: EditorSelection.cursor(pos),
        effects: EditorView.scrollIntoView(pos, { y: "center" }),
    });
    view.focus();
    return true;
}