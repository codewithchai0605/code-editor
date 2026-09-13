import { invoke } from "@tauri-apps/api/core";

export function lspStart(root: string): Promise<void> {
  return invoke("lsp_start", { root });
}

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

export function lspStop(root: string, languageId: string): Promise<void> {
  return invoke("lsp_stop", { root, languageId });
}
