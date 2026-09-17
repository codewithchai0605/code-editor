import { invoke } from "@tauri-apps/api/core";
import type { LspPosition, LspRange, ProjectRootInfo, ServerInfo } from "$lib/lsp/types";

// --- Lifecycle -----------------------------------------------------------

export function lspDetectRoots(workspace: string): Promise<ProjectRootInfo[]> {
  return invoke("lsp_detect_roots", { workspace });
}

export function lspStart(root: string, languageId: string): Promise<void> {
  return invoke("lsp_start", { root, languageId });
}

export function lspStop(root: string, languageId: string): Promise<void> {
  return invoke("lsp_stop", { root, languageId });
}

export function lspRestart(root: string, languageId: string): Promise<void> {
  return invoke("lsp_restart", { root, languageId });
}

export function lspStopAll(): Promise<void> {
  return invoke("lsp_stop_all");
}

export function lspListServers(): Promise<ServerInfo[]> {
  return invoke("lsp_list_servers");
}

export function lspGetLogs(root: string, languageId: string): Promise<string[]> {
  return invoke("lsp_get_logs", { root, languageId });
}

/** Available variants for `languageId` at `root` (e.g. TypeScript's
 *  `classic`/`native`) and which one is currently in effect — `null` if
 *  this language only ever has one server. See `src-tauri/src/lsp/registry.rs`. */
export function lspListVariants(workspace: string, root: string, languageId: string): Promise<{ variants: string[]; active: string } | null> {
  return invoke("lsp_list_variants", { workspace, root, languageId });
}

/** Records "use this variant for this folder specifically" in the
 *  workspace's registry file. Doesn't restart anything running — pair
 *  with `lspRestart` to apply it. */
export function lspSetVariantOverride(workspace: string, root: string, languageId: string, variant: string): Promise<void> {
  return invoke("lsp_set_variant_override", { workspace, root, languageId, variant });
}

// --- Document sync ---------------------------------------------------------

export function lspDidOpen(root: string, languageId: string, uri: string, text: string): Promise<void> {
  return invoke("lsp_did_open", { root, languageId, uri, text });
}

export function lspDidChange(
  root: string,
  languageId: string,
  uri: string,
  text: string,
  version: number,
): Promise<void> {
  return invoke("lsp_did_change", { root, languageId, uri, text, version });
}

export function lspDidClose(root: string, languageId: string, uri: string): Promise<void> {
  return invoke("lsp_did_close", { root, languageId, uri });
}

// --- Language features ------------------------------------------------
// Each resolves with the server's raw JSON-RPC `result` — untyped `any` at
// this boundary, refined by each individual feature module (completion.ts,
// hover.ts, ...) into whatever shape it actually reads.

export function lspCompletion(root: string, languageId: string, uri: string, position: LspPosition): Promise<any> {
  return invoke("lsp_completion", { root, languageId, uri, position });
}

export function lspHover(root: string, languageId: string, uri: string, position: LspPosition): Promise<any> {
  return invoke("lsp_hover", { root, languageId, uri, position });
}

export function lspDefinition(root: string, languageId: string, uri: string, position: LspPosition): Promise<any> {
  return invoke("lsp_definition", { root, languageId, uri, position });
}

export function lspReferences(
  root: string,
  languageId: string,
  uri: string,
  position: LspPosition,
  includeDeclaration: boolean,
): Promise<any> {
  return invoke("lsp_references", { root, languageId, uri, position, includeDeclaration });
}

export function lspRename(
  root: string,
  languageId: string,
  uri: string,
  position: LspPosition,
  newName: string,
): Promise<any> {
  return invoke("lsp_rename", { root, languageId, uri, position, newName });
}

export function lspCodeAction(
  root: string,
  languageId: string,
  uri: string,
  range: LspRange,
  diagnostics: unknown[],
): Promise<any> {
  return invoke("lsp_code_action", { root, languageId, uri, range, diagnostics });
}

export function lspExecuteCommand(root: string, languageId: string, command: string, args: unknown[]): Promise<any> {
  return invoke("lsp_execute_command", { root, languageId, command, arguments: args });
}

export function lspSignatureHelp(root: string, languageId: string, uri: string, position: LspPosition): Promise<any> {
  return invoke("lsp_signature_help", { root, languageId, uri, position });
}

export function lspDocumentSymbols(root: string, languageId: string, uri: string): Promise<any> {
  return invoke("lsp_document_symbols", { root, languageId, uri });
}

export function lspFormatting(
  root: string,
  languageId: string,
  uri: string,
  tabSize: number,
  insertSpaces: boolean,
): Promise<any> {
  return invoke("lsp_formatting", { root, languageId, uri, tabSize, insertSpaces });
}

export function lspWorkspaceSymbol(query: string): Promise<any[]> {
  return invoke("lsp_workspace_symbol", { query });
}