import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
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
} from "@codemirror/commands";
import {
  indentOnInput,
  bracketMatching,
  foldGutter,
  foldKeymap,
} from "@codemirror/language";
import {
  searchKeymap,
  highlightSelectionMatches,
  openSearchPanel,
  selectNextOccurrence,
} from "@codemirror/search";
import { closeBrackets, closeBracketsKeymap, autocompletion, completionKeymap } from "@codemirror/autocomplete";
import type { Theme } from "./themes/types";
import { buildCmTheme } from "./themeEngine";
import { searchExtension } from "./search.svelte";
import { indentGuides } from "./indent-guide";
import { rainbowBrackets } from "./rainbow-brackets";
import tagAutoRename from "./tag-auto-rename";
import indentationFolding from "./indentation-folding";
import { colorChips } from "./color-chip";
import { emmetExpandCommand } from "./emmet";
import { extensionOf } from "$lib/utils/fileIcons";

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

/**
 * This deliberately mirrors CodeMirror's own `basicSetup` ingredient list
 * (see the `codemirror` package's doc comment) minus `@codemirror/lint` —
 * there's no LSP yet to feed it, so pulling in the lint package now would
 * just be unused weight. Hand-picking these individually (rather than
 * depending on the `codemirror` meta-package) also means we control exactly
 * what ships, and can layer our own theme + a real save keybinding on top.
 *
 * Beyond that base set, this also includes the extra editing features
 * layered on top of the original scaffold: multi-theme support (via
 * `themeCompartment`), indent guides, rainbow bracket matching, paired
 * HTML/JSX tag renaming, indentation-based folding (a fallback for any
 * legacy `StreamLanguage`-based language — a no-op for the Lezer-grammar
 * languages this app currently ships), inline color swatches, and a
 * custom find/replace panel (`search.ts` + `SearchPanel.svelte`) in place
 * of CodeMirror's default DOM-built one, so it can match the app's design
 * system.
 *
 * `filePath` (optional — omitted for e.g. an unsaved/untitled buffer) is
 * only used to pick an extension-appropriate Tab behavior: Emmet-expand
 * for markup/stylesheet files, ordinary indent everywhere else. Snippets
 * are attached separately, per-language, in languages.ts — they need a
 * loaded `Language` instance to hang off of, which isn't available yet at
 * this point for a freshly-opened tab.
 */
export function baseExtensions(onSave: () => void, initialTheme: Theme, filePath?: string): Extension[] {
  const emmetExpand = filePath ? emmetExpandCommand(extensionOf(filePath)) : null;

  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    foldGutter(),
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
    themeCompartment.of(buildCmTheme(initialTheme)),
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
      { key: "Alt-ArrowUp", run: moveLineUp },
      { key: "Alt-ArrowDown", run: moveLineDown },
      { key: "Shift-Alt-ArrowUp", run: copyLineUp },
      { key: "Shift-Alt-ArrowDown", run: copyLineDown },
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...searchKeymap.filter((binding) => binding.key !== "Mod-f"),
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
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
