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
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  indentOnInput,
  bracketMatching,
  foldGutter,
  foldKeymap,
} from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches, openSearchPanel } from "@codemirror/search";
import { closeBrackets, closeBracketsKeymap, autocompletion, completionKeymap } from "@codemirror/autocomplete";
import type { Theme } from "./themes/types";
import { buildCmTheme } from "./themeEngine";
import { searchExtension } from "./search.svelte";
import { indentGuides } from "./indent-guide";
import { rainbowBrackets } from "./rainbow-brackets";
import tagAutoRename from "./tag-auto-rename";
import indentationFolding from "./indentation-folding";
import { colorChips } from "./color-chip";

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
 */
export function baseExtensions(onSave: () => void, initialTheme: Theme): Extension[] {
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
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...searchKeymap.filter((binding) => binding.key !== "Mod-f"),
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      indentWithTab,
    ]),
  ];
}
