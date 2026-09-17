import { lspDocumentSymbols, lspWorkspaceSymbol } from "$lib/api/lsp";
import { lspStore } from "./manager.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { editorStore } from "$lib/stores/editor.svelte";
import { lspLanguageIdFor } from "./languageId";
import { pathToFileUri, fileUriToPath } from "./uri";
import type { LspDocumentSymbol, LspRange, LspSymbolInformation } from "./types";

export interface SymbolEntry {
    name: string;
    detail?: string;
    kind: number;
    path: string;
    range: LspRange;
    containerName?: string;
}

function isSymbolInformationArray(result: unknown): result is LspSymbolInformation[] {
    return Array.isArray(result) && result.length > 0 && "location" in (result[0] as object);
}

function flattenDocumentSymbols(
    symbols: LspDocumentSymbol[],
    path: string,
    containerName?: string,
): SymbolEntry[] {
    const out: SymbolEntry[] = [];
    for (const symbol of symbols) {
        out.push({
            name: symbol.name,
            detail: symbol.detail,
            kind: symbol.kind,
            path,
            range: symbol.selectionRange ?? symbol.range,
            containerName,
        });
        if (symbol.children?.length)
            out.push(...flattenDocumentSymbols(symbol.children, path, symbol.name));
    }
    return out;
}

function fromSymbolInformation(symbols: LspSymbolInformation[]): SymbolEntry[] {
    return symbols.map((symbol) => ({
        name: symbol.name,
        kind: symbol.kind,
        path: fileUriToPath(symbol.location.uri),
        range: symbol.location.range,
        containerName: symbol.containerName,
    }));
}

/** For "Go to Symbol in File" (Ctrl+Shift+O) — a server can return either
 *  the nested `DocumentSymbol[]` shape or the flatter `SymbolInformation[]`
 *  shape; both are normalized to the same flat `SymbolEntry[]`. */
export async function fetchDocumentSymbols(): Promise<SymbolEntry[]> {
    const tab = editorStore.activeTab;
    const workspaceRoot = workspace.rootPath;
    if (!tab || !workspaceRoot) return [];

    const languageId = lspLanguageIdFor(tab.path);
    if (!languageId) return [];

    const root = lspStore.resolveRootFor(tab.path, languageId, workspaceRoot);
    const uri = pathToFileUri(tab.path);

    let result: unknown;
    try {
        result = await lspDocumentSymbols(root, languageId, uri);
    } catch {
        return [];
    }
    if (!Array.isArray(result) || result.length === 0) return [];

    return isSymbolInformationArray(result)
        ? fromSymbolInformation(result)
        : flattenDocumentSymbols(result as LspDocumentSymbol[], tab.path);
}

/** For "Go to Symbol in Workspace" (Ctrl+T) — fans out to every running
 *  server across every project root (see `LspManager::workspace_symbol`
 *  on the Rust side) and returns one flat, mergeable list. */
export async function fetchWorkspaceSymbols(query: string): Promise<SymbolEntry[]> {
    let result: LspSymbolInformation[];
    try {
        result = await lspWorkspaceSymbol(query);
    } catch {
        return [];
    }
    return fromSymbolInformation(result);
}
