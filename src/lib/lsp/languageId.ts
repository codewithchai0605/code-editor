import { extensionOf } from "$lib/utils/fileIcons";

/**
 * Maps a file extension to the language ID used both as the registry key
 * (this app's own LSP config, managed from Settings — see
 * `src-tauri/src/lsp/registry.rs`) and in the LSP `textDocument/didOpen`
 * notification. `null` means "no LSP integration for this file" — a
 * normal, expected state (most extensions won't have a server configured),
 * not an error. Extend this alongside the registry's default entries in
 * `src-tauri/src/lsp/registry.rs` when adding a new language.
 */
const LANGUAGE_IDS: Record<string, string> = {
    ts: "typescript",
    tsx: "typescript",
    js: "typescript",
    jsx: "typescript",
    mjs: "typescript",
    cjs: "typescript",
    py: "python",
    rs: "rust",
};

export function lspLanguageIdFor(fileName: string): string | null {
    return LANGUAGE_IDS[extensionOf(fileName)] ?? null;
}