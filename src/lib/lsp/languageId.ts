import { extensionOf } from "$lib/utils/fileIcons";

/**
 * Maps a file extension to the language ID used both as the registry key
 * (this app's own LSP config, managed from Settings — see
 * `src-tauri/src/lsp/registry.rs`) and in the LSP `textDocument/didOpen`
 * notification. `null` means "no LSP integration for this file" — a
 * normal, expected state (most extensions won't have a server configured),
 * not an error. Extend this alongside the registry's default entries in
 * `src-tauri/src/lsp/registry.rs` when adding a new language.
 *
 * `dart`/`go`/`cpp`/`php` were already fully configured on the registry
 * side (`default_servers()`) but had no entry here at all — meaning
 * `.dart`/`.go`/`.cpp`/`.php` files got zero LSP features no matter what,
 * since this map is what turns a file into a language ID in the first
 * place. Filled in alongside adding html/css/json.
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
    dart: "dart",
    go: "go",
    cpp: "cpp",
    cc: "cpp",
    cxx: "cpp",
    c: "cpp",
    h: "cpp",
    hpp: "cpp",
    hh: "cpp",
    php: "php",
    // vscode-langservers-extracted (see registry.rs) — no project manifest
    // needed, so these work in a plain static html/css/js site same as
    // TypeScript/JS do (see the "classic" TypeScript variant's
    // `tsserver.fallbackPath` for that half of the story).
    html: "html",
    htm: "html",
    css: "css",
    json: "json",
};

export function lspLanguageIdFor(fileName: string): string | null {
    return LANGUAGE_IDS[extensionOf(fileName)] ?? null;
}