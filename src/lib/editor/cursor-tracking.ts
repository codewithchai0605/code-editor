import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { editorStore } from "$lib/stores/editor.svelte";

/**
 * Shared extension that mirrors the active editor's cursor/selection into
 * `editorStore.cursorLine`/`cursorColumn`/`selectionLength`/`selectionCount`
 * so the status bar's Ln/Col chip (VS Code parity, Phase 3) updates live.
 *
 * Deliberately a module-level singleton: every tab's state includes it, and
 * it only does two O(selection-count) reads per dispatched update — no
 * allocations, no async work — keeping the per-keystroke path as cheap as
 * the rest of the updateListener chain in setup.ts.
 */
let shared: Extension | null = null;

export function cursorTrackingExtension(): Extension {
    if (!shared) {
        shared = EditorView.updateListener.of((update) => {
            // Only care when the cursor or document actually moved; pure
            // viewport/theme changes dispatch updates too.
            if (update.selectionSet || update.docChanged) {
                editorStore.reportCursorState(update.view.state);
            }
        });
    }
    return shared;
}
