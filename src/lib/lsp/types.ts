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

export interface LspLogEvent {
    root: string;
    languageId: string;
    line: string;
}

/** One project root detected by `lsp_detect_roots` — see
 *  `src-tauri/src/lsp/project_scanner.rs`. */
export interface ProjectRootInfo {
    root: string;
    languageId: string;
}

/** One entry from `lsp_list_servers` — every server the backend currently
 *  knows about, alive or not (see manager.rs's module doc comment on why
 *  crashed/stopped entries are kept rather than removed). */
export interface ServerInfo {
    root: string;
    languageId: string;
    status: LspServerStatus;
    /** Raw `ServerCapabilities` JSON from this server's `initialize`
     *  response, `null` until that's completed. Used to gate which
     *  features (rename, formatting, ...) are offered per-server without
     *  needing a typed model of every capability flag. */
    capabilities: Record<string, unknown> | null;
}

export interface LspTextEdit {
    range: LspRange;
    newText: string;
}

/** Matches `lsp_types::WorkspaceEdit`'s wire JSON — only the `changes`
 *  shape is modeled (a plain `{uri: TextEdit[]}` map); `documentChanges`
 *  (the versioned/resource-operation variant some servers prefer) is a
 *  fully-discriminated union (each variant has its own distinct required
 *  fields, no catch-all) deliberately, rather than a looser `{kind, ...}`
 *  shape with an index signature — the latter defeats `"textDocument" in
 *  change`-style narrowing, since TypeScript can't tell an index
 *  signature branch *doesn't* also have a `textDocument` key. */
export interface LspWorkspaceEdit {
    changes?: Record<string, LspTextEdit[]>;
    documentChanges?: Array<
        | { textDocument: { uri: string; version: number | null }; edits: LspTextEdit[] }
        | { kind: "create"; uri: string }
        | { kind: "rename"; oldUri: string; newUri: string }
        | { kind: "delete"; uri: string }
    >;
}

export interface LspCommand {
    title: string;
    command: string;
    arguments?: unknown[];
}

export interface LspCodeAction {
    title: string;
    kind?: string;
    isPreferred?: boolean;
    edit?: LspWorkspaceEdit;
    command?: LspCommand;
}

/** A `CodeActionOrCommand` entry — servers may return either shape
 *  directly in the array. */
export type LspCodeActionOrCommand = LspCodeAction | LspCommand;

export interface LspLocation {
    uri: string;
    range: LspRange;
}

export interface LspDocumentSymbol {
    name: string;
    detail?: string;
    kind: number;
    range: LspRange;
    selectionRange: LspRange;
    children?: LspDocumentSymbol[];
}

/** The flat (`SymbolInformation`) shape — what `workspace/symbol` and,
 *  from some servers, `textDocument/documentSymbol` return instead of the
 *  nested `LspDocumentSymbol` tree. */
export interface LspSymbolInformation {
    name: string;
    kind: number;
    location: LspLocation;
    containerName?: string;
}
