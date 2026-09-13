/** Matches `lsp_types::Diagnostic`'s wire JSON exactly (camelCase). We only
 *  declare the fields Step 1 actually uses — Rust passes the full object
 *  through unchanged (see manager.rs's module doc comment for why), so
 *  more fields can be added here later without any backend change. */
export interface LspPosition {
  line: number;
  character: number;
}

export interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

export interface LspDiagnostic {
  range: LspRange;
  /** 1=Error, 2=Warning, 3=Information, 4=Hint. Absent = client's choice. */
  severity?: 1 | 2 | 3 | 4;
  source?: string;
  message: string;
}

export interface PublishDiagnosticsParams {
  uri: string;
  diagnostics: LspDiagnostic[];
  version?: number;
}

export interface LspDiagnosticsEvent {
  root: string;
  languageId: string;
  params: PublishDiagnosticsParams;
}

export type LspServerStatusKind = "starting" | "running" | "crashed" | "stopped";

export interface LspServerStatus {
  kind: LspServerStatusKind;
  message?: string;
}

export interface LspStatusEvent {
  root: string;
  languageId: string;
  status: LspServerStatus;
}
