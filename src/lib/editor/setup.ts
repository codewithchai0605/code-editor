import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    highlightSpecialChars,
    drawSelection,
    dropCursor,
    rectangularSelection,
    crosshairCursor,
} from "@codemirror/view";
import {
    defaultKeymap,
    history,
    historyKeymap,
    indentWithTab,
    moveLineUp,
    moveLineDown,
    copyLineUp,
    copyLineDown,
    toggleComment,
    deleteLine,
    joinLines,
    selectLine,
    foldAll,
    unfoldAll,
} from "@codemirror/commands";
import {
    indentOnInput,
    bracketMatching,
    foldGutter,
    foldKeymap,
    lineAt,
} from "@codemirror/language";
import {
    searchKeymap,
    highlightSelectionMatches,
    openSearchPanel,
    selectNextOccurrence,
} from "@codemirror/search";
import {
    closeBrackets,
    closeBracketsKeymap,
    autocompletion,
    completionKeymap,
} from "@codemirror/autocomplete";
import { lintGutter, lintKeymap } from "@codemirror/lint";
import type { Theme } from "./themes/types";
import { buildCmTheme } from "./themeEngine";
import { searchExtension } from "./search.svelte";
import { indentGuides } from "./indent-guide";
import { rainbowBrackets } from "./rainbow-brackets";
import tagAutoRename from "./tag-auto-rename";
import indentationFolding from "./indentation-folding";
import { colorChips } from "./color-chip";
import { gitGutter, goToNextGitChange, goToPreviousGitChange } from "./git-gutter";
import { emmetExpandCommand } from "./emmet";
import { extensionOf } from "$lib/utils/fileIcons";
import { lspDiagnostics } from "$lib/lsp/diagnostics";
import { pathToFileUri } from "$lib/lsp/uri";
import { lspCompletionSource } from "$lib/lsp/completion";
import { lspHoverTooltip } from "$lib/lsp/hover";
import { lspSignatureHelpTooltip } from "$lib/lsp/signature-help";
import { lspDefinitionHandler, lspGoToDefinitionCommand } from "$lib/lsp/definition";
import { lspRenameCommand, lspRenameExtension } from "$lib/lsp/rename";
import { lspCodeActionCommand } from "$lib/lsp/code-action";
import { editorContextMenu } from "./context-menu";
import { revealHighlightExtension } from "./reveal-highlight";
import { cursorTrackingExtension } from "./cursor-tracking";

/**
 * Holds the theme's CodeMirror extension (chrome + syntax highlighting)
 * for every open tab. A single module-level `Compartment` instance is
 * required here — reconfiguring only works if every tab's `EditorState`
 * was built from the *same* Compartment object, which is why this is a
 * shared export rather than something created fresh per call.
 * `stores/editor.svelte.ts`'s `applyThemeToOpenTabs` is what actually
 * dispatches a reconfigure against it, for every open tab, when the theme
 * changes.
 */
export const themeCompartment = new Compartment();

function createFoldMarker(open: boolean) {
    const marker = document.createElement("span");
    marker.className = "fold-marker";

    marker.innerHTML = `
    <svg
      viewBox="0 0 16 16"
      width="14"
      height="14"
      aria-hidden="true"
    >
      <path
        d="${open ? "M4 6l4 4 4-4" : "M6 4l4 4-4 4"}"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  `;

    return marker;
}

/**
 * This deliberately mirrors CodeMirror's own `basicSetup` ingredient list
 * (see the `codemirror` package's doc comment). Hand-picking these
 * individually (rather than depending on the `codemirror` meta-package)
 * means we control exactly what ships, and can layer our own theme + a
 * real save keybinding on top. `@codemirror/lint` — omitted from
 * `basicSetup` upstream — is included here directly via the per-file
 * `lspDiagnostics()` extension below, now that there's a real LSP backend
 * to feed it.
 *
 * Beyond that base set, this also includes the extra editing features
 * layered on top of the original scaffold: multi-theme support (via
 * `themeCompartment`), indent guides, rainbow bracket matching, paired
 * HTML/JSX tag renaming, indentation-based folding (a fallback for any
 * legacy `StreamLanguage`-based language — a no-op for the Lezer-grammar
 * languages this app currently ships), inline color swatches, and a
 * custom find/replace panel (`search.ts` + `SearchPanel.svelte`) in place
 * of CodeMirror's default DOM-built one, so it can match the app's design
 * system, and (when `filePath` is given) live diagnostics from the LSP
 * backend — see lsp/diagnostics.ts for why this is a *push* model (Rust
 * emits events as the server publishes them) bridged onto @codemirror/lint's
 * *pull* API, rather than the lint package's default polling behavior.
 *
 * `filePath` (optional — omitted for e.g. an unsaved/untitled buffer) is
 * used both to pick an extension-appropriate Tab behavior (Emmet-expand
 * for markup/stylesheet files, ordinary indent everywhere else) and to
 * fix this tab's diagnostics linter to its own file URI. Snippets are
 * attached separately, per-language, in languages.ts — they need a loaded
 * `Language` instance to hang off of, which isn't available yet at this
 * point for a freshly-opened tab.
 */
export function baseExtensions(
    onSave: () => void,
    initialTheme: Theme,
    filePath?: string,
): Extension[] {
    const emmetExpand = filePath ? emmetExpandCommand(extensionOf(filePath)) : null;

    return [
        lineNumbers(),
        ...(filePath ? gitGutter(filePath) : []),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        foldGutter({
            markerDOM(open) {
                return createFoldMarker(open);
            },
        }),
        drawSelection(),
        dropCursor(),
        EditorState.allowMultipleSelections.of(true),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        autocompletion(),
        rectangularSelection(),
        crosshairCursor(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        searchExtension(),
        indentGuides(),
        rainbowBrackets(),
        tagAutoRename(),
        indentationFolding,
        colorChips(),
        revealHighlightExtension(),
        cursorTrackingExtension(),
        lintGutter(),
        themeCompartment.of(buildCmTheme(initialTheme)),
        ...(filePath
            ? [
                  lspDiagnostics(pathToFileUri(filePath)),
                  lspCompletionSource(filePath),
                  lspHoverTooltip(filePath),
                  lspSignatureHelpTooltip(filePath),
                  lspDefinitionHandler(filePath),
                  lspRenameExtension(),
                  editorContextMenu(filePath),
              ]
            : []),
        keymap.of([
            {
                key: "Mod-s",
                preventDefault: true,
                run: () => {
                    onSave();
                    return true;
                },
            },
            { key: "Mod-f", run: openSearchPanel },
            { key: "Mod-d", run: selectNextOccurrence },
            // --- VS Code parity keybindings (Phase 2) ---
            // Mod-/ toggles line or block comment depending on selection.
            { key: "Mod-slash", preventDefault: true, run: toggleComment },
            // Mod-Shift-L: select every occurrence of the current selection
            // (or the word under an empty cursor), like VS Code's
            // "Select All Occurrences of Find Match".
            {
                key: "Mod-Shift-l",
                preventDefault: true,
                run(view) {
                    const { state } = view;
                    const sel = state.selection.main;
                    const query = sel.empty
                        ? state.wordAt(sel.head)?.sliceString(state.doc) ?? ""
                        : state.sliceDoc(sel.from, sel.to);
                    if (!query) return false;
                    const ranges: { from: number; to: number }[] = [];
                    const re = new RegExp(
                        query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
                        "g",
                    );
                    const text = state.doc.sliceString(0, state.doc.length);
                    let m: RegExpExecArray | null;
                    while ((m = re.exec(text)) !== null) {
                        ranges.push({ from: m.index, to: m.index + m[0].length });
                        if (m[0].length === 0) re.lastIndex++;
                    }
                    if (ranges.length === 0) return false;
                    view.dispatch({
                        selections: ranges.map((r) => ({ anchor: r.from, head: r.to })),
                    });
                    return true;
                },
            },
            // Shift-Alt-K: keep only the lines matching the current selection.
            {
                key: "Shift-Alt-k",
                preventDefault: true,
                run(view) {
                    const { state } = view;
                    const sel = state.selection.main;
                    if (sel.empty) return false;
                    const needle = state.sliceDoc(sel.from, sel.to);
                    const first = lineAt(state.doc, sel.from).number;
                    const last = lineAt(state.doc, Math.max(sel.from, sel.to - 1)).number;
                    const kept: string[] = [];
                    for (let n = first; n <= last; n++) {
                        const line = state.doc.line(n);
                        if (line.text.includes(needle)) kept.push(line.text);
                    }
                    if (kept.length === 0) return false;
                    view.dispatch({
                        changes: {
                            from: state.doc.line(first).from,
                            to: last < state.doc.lines ? state.doc.line(last + 1).from : state.doc.length,
                            insert: kept.join("\n") + "\n",
                        },
                    });
                    return true;
                },
            },
            // Mod-Shift-K / Mod-Shift-J: delete and join lines, as in VS Code.
            { key: "Mod-Shift-k", preventDefault: true, run: deleteLine },
            { key: "Mod-j", preventDefault: true, run: joinLines },
            // Mod-Shift-i: place a cursor at the end of every selected line.
            { key: "Mod-Shift-i", preventDefault: true, run: selectLine },
            // Mod-K Mod-0 / Mod-K Mod-1: fold everything / unfold everything.
            { key: "Mod-k Mod-0", preventDefault: true, run: unfoldAll },
            { key: "Mod-k Mod-1", preventDefault: true, run: foldAll },
            { key: "Alt-F3", run: goToNextGitChange },
            { key: "Shift-Alt-F3", run: goToPreviousGitChange },
            { key: "Alt-ArrowUp", run: moveLineUp },
            { key: "Alt-ArrowDown", run: moveLineDown },
            { key: "Shift-Alt-ArrowUp", run: copyLineUp },
            { key: "Shift-Alt-ArrowDown", run: copyLineDown },
            ...(filePath
                ? [
                      { key: "F12", preventDefault: true, run: lspGoToDefinitionCommand(filePath) },
                      { key: "F2", preventDefault: true, run: lspRenameCommand(filePath) },
                      { key: "Mod-.", preventDefault: true, run: lspCodeActionCommand(filePath) },
                  ]
                : []),
            ...closeBracketsKeymap,
            ...defaultKeymap,
            ...searchKeymap.filter((binding) => binding.key !== "Mod-f"),
            ...historyKeymap,
            ...foldKeymap,
            ...completionKeymap,
            ...lintKeymap,
            // Tried before `indentWithTab` so an Emmet-expandable file gets a
            // real expansion on Tab when the cursor sits after a valid
            // abbreviation; `emmetExpand` itself returns false (falling through
            // to ordinary indent) for anything that isn't one, an active
            // selection, or a non-markup/stylesheet file (where it's `null` and
            // never added at all).
            ...(emmetExpand ? [{ key: "Tab", run: emmetExpand }] : []),
            indentWithTab,
        ]),
    ];
}
