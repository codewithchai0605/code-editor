import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";

/**
 * A brief flash highlight over a just-revealed range — the visual "you
 * landed here" cue when navigating to a workspace-search match or a
 * go-to-definition target, especially important when the file was already
 * open and on screen (where a plain selection move is easy to miss).
 *
 * Implemented as a `StateField` holding at most one decorated range:
 * `addEffect` sets it, then schedules a self-clearing effect after the
 * flash duration. The field survives `setState`/tab-switches by design
 * (it's part of every tab's state via `revealHighlightExtension()`), but
 * a pending reveal only ever dispatches against the live view, so there
 * is no cross-tab bleed in practice; the timeout clears it regardless.
 */

export const revealHighlight = StateEffect.define<{ from: number; to: number }>();

const FLASH_MS = 1600;

const revealField = StateField.define<{ from: number; to: number } | null>({
    create: () => null,
    update(value, tr) {
        for (const effect of tr.effects) {
            if (effect.is(revealHighlight)) {
                return effect.value ? { ...effect.value } : null;
            }
        }
        return value;
    },
    provide: (field) =>
        EditorView.decorations.compute([field], (state) => {
            const range = state.field(field);
            if (!range) return Decoration.none;
            // Clamp defensively — the document could have changed between
            // the dispatch that set this and the decoration computation.
            const doc = state.doc;
            const from = Math.min(range.from, doc.length);
            const to = Math.min(Math.max(range.to, from), doc.length);
            if (from >= to) return Decoration.none;
            return Decoration.set([
                Decoration.mark({ class: "cm-revealHighlight" }).range(from, to),
            ]);
        }),
});

/** Dispatched (with an empty range → `null`) by the timer below to fade
 *  the flash back out. Reuses `revealHighlight` itself rather than a
 *  second effect type — nothing reads the value except the field. */
function scheduleClear(view: EditorView, from: number, to: number): void {
    setTimeout(() => {
        // Only clear if the field still holds *this* flash — a newer
        // reveal shouldn't get cancelled by an older timer.
        const current = view.state.field(revealField);
        if (current && current.from === from && current.to === to) {
            view.dispatch({ effects: revealHighlight.of({ from: -1, to: -1 }) });
        }
    }, FLASH_MS);
}

/** Schedules the auto-clear whenever a new flash is set. Kept separate
 *  from the field (which must stay pure) — runs as a view-update side
 *  effect, exactly once per newly-set range. */
const revealScheduler = EditorView.updateListener.of((update) => {
    if (!update.transactions.some((tr) => tr.effects.some((e) => e.is(revealHighlight)))) return;
    const current = update.view.state.field(revealField);
    if (current) scheduleClear(update.view, current.from, current.to);
});

export function revealHighlightExtension(): Extension {
    return [
        revealField,
        revealScheduler,
        EditorView.baseTheme({
            ".cm-revealHighlight": {
                backgroundColor: "rgba(246, 185, 55, 0.35)",
                borderRadius: "2px",
                transition: "background-color 300ms ease",
            },
        }),
    ];
}
