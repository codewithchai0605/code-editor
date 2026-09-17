import { linter, type Diagnostic as CmDiagnostic } from "@codemirror/lint";
import type { Extension } from "@codemirror/state";
import type { LspDiagnostic } from "./types";
import { posToOffset } from "./position";

/** uri -> latest diagnostics for that file. A plain module-level Map, not
 *  Svelte `$state` — what needs to observe this is CodeMirror's own
 *  linter() pull cycle (see lspDiagnostics() below), not Svelte's
 *  reactivity. manager.ts writes to this as `lsp:diagnostics` events
 *  arrive. */
const cache = new Map<string, LspDiagnostic[]>();

export function setDiagnosticsForUri(uri: string, diagnostics: LspDiagnostic[]): void {
  cache.set(uri, diagnostics);
}

export function clearDiagnosticsForUri(uri: string): void {
  cache.delete(uri);
}

/** Synchronous read of the current cache — used by `codeAction.ts` to
 *  find diagnostics overlapping the requested range (servers commonly
 *  only attach quick-fixes to a code action when the request's range
 *  includes the diagnostic they fix). */
export function getDiagnosticsForUri(uri: string): LspDiagnostic[] {
  return cache.get(uri) ?? [];
}

function severityToString(severity: LspDiagnostic["severity"]): CmDiagnostic["severity"] {
  switch (severity) {
    case 1:
      return "error";
    case 2:
      return "warning";
    case 3:
      return "info";
    case 4:
      return "hint";
    default:
      // LSP spec: an absent severity is left to the client to interpret —
      // treating it as an error is the safer default (more visible than
      // silently downgrading a potentially serious issue).
      return "error";
  }
}

/**
 * Per-tab linter extension. `uri` is fixed at construction time (one call
 * per open tab, from editor/setup.ts) rather than derived from the live
 * view — this app shares one `EditorView` across every open tab (see
 * editor.svelte.ts), so a closure over "which file is *this* extension
 * instance for" is simpler and unambiguous.
 *
 * Diagnostics reach the editor two ways: this pull-based `source`
 * (invoked by CodeMirror after edits, debounced by `delay`), and an
 * immediate `forceLinting()` call from lsp/manager.ts the moment a
 * `lsp:diagnostics` event arrives — so the squiggles show up promptly
 * rather than waiting for the next keystroke.
 */
export function lspDiagnostics(uri: string): Extension {
  return linter(
    (view) => {
      const raw = cache.get(uri) ?? [];
      const doc = view.state.doc;
      const result: CmDiagnostic[] = [];
      for (const d of raw) {
        const from = posToOffset(doc, d.range.start);
        const to = Math.max(from, posToOffset(doc, d.range.end));
        result.push({ from, to, severity: severityToString(d.severity), message: d.message, source: d.source });
      }
      return result;
    },
    { delay: 200 },
  );
}