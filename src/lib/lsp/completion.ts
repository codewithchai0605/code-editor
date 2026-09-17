import { EditorState, type Extension } from "@codemirror/state";
import type {
    Completion,
    CompletionContext,
    CompletionResult,
    CompletionSource,
} from "@codemirror/autocomplete";
import { lspCompletion } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri } from "./uri";
import { offsetToPos, posToOffset } from "./position";

/** A completion item's own `label`/`filterText` is already meant to be
 *  matched against whatever the user typed since `from` — trigger on any
 *  word character *or* one of a language-agnostic default set of LSP
 *  trigger characters (member access, string/attribute openers, etc.)
 *  rather than only word characters, so e.g. `obj.` triggers member
 *  completions the moment `.` is typed instead of needing another
 *  keystroke first. A real per-server trigger-character list (from
 *  `completionProvider.triggerCharacters` in its capabilities) would be
 *  more precise, but capabilities aren't guaranteed to have arrived by
 *  the time typing starts — this default set covers the common cases
 *  without that dependency.
 */
const DEFAULT_TRIGGER_CHARS = new Set([".", ":", "<", '"', "'", "/", "@", "$", "-", ">"]);

/** Rough LSP `CompletionItemKind` (1-25) -> CodeMirror `Completion.type`
 *  mapping, for icon rendering. Not exhaustive — an unmapped kind just
 *  falls back to no icon rather than a wrong one. */
const KIND_TO_TYPE: Record<number, string> = {
    2: "method",
    3: "function",
    4: "constructor",
    5: "property",
    6: "variable",
    7: "class",
    8: "interface",
    9: "namespace",
    10: "property",
    12: "value",
    13: "enum",
    14: "keyword",
    15: "text",
    18: "text",
    20: "enumMember",
    21: "constant",
    22: "type",
    25: "typeParameter",
};

interface LspCompletionItem {
    label: string;
    kind?: number;
    detail?: string;
    documentation?: string | { value: string };
    sortText?: string;
    filterText?: string;
    insertText?: string;
    insertTextFormat?: number; // 1 = PlainText, 2 = Snippet
    textEdit?:
        | { range: { start: unknown; end: unknown }; newText: string }
        | { insert: { start: unknown; end: unknown }; newText: string };
}

/** Strips the common LSP snippet placeholder shapes (`$0`, `$1`,
 *  `${1:default}`, `${1|a,b,c|}`) down to plain inserted text — not a
 *  real snippet mode (no tab-stops, no live editing of the placeholder),
 *  just enough to avoid literally inserting `${1:foo}` into the buffer
 *  for the (common) case of a server returning `insertTextFormat: 2`. */
function stripSnippetPlaceholders(text: string, format: number | undefined): string {
    if (format !== 2) return text;
    return text
        .replace(/\$\{\d+:([^}]*)\}/g, "$1")
        .replace(/\$\{\d+\|([^,}]*)(?:,[^}]*)?\|\}/g, "$1")
        .replace(/\$\{\d+\}/g, "")
        .replace(/\$\d+/g, "");
}

function documentationText(doc: LspCompletionItem["documentation"]): string | undefined {
    if (!doc) return undefined;
    return typeof doc === "string" ? doc : doc.value;
}

function toCompletion(item: LspCompletionItem): Completion {
    return {
        label: item.label,
        detail: item.detail,
        info: documentationText(item.documentation),
        type: item.kind ? KIND_TO_TYPE[item.kind] : undefined,
        boost: item.sortText ? undefined : 0,
        apply(view, _completion, from, to) {
            let insertFrom = from;
            let insertTo = to;
            let text = item.insertText ?? item.label;

            if (item.textEdit) {
                const range = "range" in item.textEdit ? item.textEdit.range : item.textEdit.insert;
                insertFrom = posToOffset(view.state.doc, range.start as never);
                insertTo = posToOffset(view.state.doc, range.end as never);
                text = item.textEdit.newText;
            }

            text = stripSnippetPlaceholders(text, item.insertTextFormat);
            view.dispatch({
                changes: { from: insertFrom, to: insertTo, insert: text },
                selection: { anchor: insertFrom + text.length },
            });
        },
    };
}

function buildSource(filePath: string): CompletionSource {
    return async (context: CompletionContext): Promise<CompletionResult | null> => {
        const languageId = lspLanguageIdFor(filePath);
        const workspaceRoot = workspace.rootPath;
        if (!languageId || !workspaceRoot) return null;

        const typedChar = context.state.sliceDoc(Math.max(0, context.pos - 1), context.pos);
        const word = context.matchBefore(/[\w$]*/);
        if (!context.explicit && !word?.text && !DEFAULT_TRIGGER_CHARS.has(typedChar)) return null;

        const root = lspStore.resolveRootFor(filePath, languageId, workspaceRoot);
        const uri = pathToFileUri(filePath);
        const position = offsetToPos(context.state.doc, context.pos);

        let result: unknown;
        try {
            result = await lspCompletion(root, languageId, uri, position);
        } catch {
            return null;
        }
        if (context.aborted || !result) return null;

        const items: LspCompletionItem[] = Array.isArray(result)
            ? result
            : ((result as { items?: LspCompletionItem[] }).items ?? []);
        if (items.length === 0) return null;

        return {
            from: word?.from ?? context.pos,
            to: context.pos,
            options: items.map(toCompletion),
            validFor: /^[\w$]*$/,
        };
    };
}

/** Wires an LSP completion source into the given file's language data —
 *  additive with, not a replacement for, per-language snippet completions
 *  (see `editor/languages.ts`): CodeMirror's `languageData` facet
 *  accumulates every provider that applies at a position rather than
 *  only using the closest one, so both sources' results get merged by
 *  `autocompletion()`. */
export function lspCompletionSource(filePath: string): Extension {
    const source = buildSource(filePath);
    return EditorState.languageData.of(() => [{ autocomplete: source }]);
}
