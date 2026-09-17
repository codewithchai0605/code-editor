use crate::lsp::manager::{LspManager, ProjectRootInfo, ServerInfo};
use crate::lsp::registry::{self, VariantInfo};
use lsp_types::{Position, Range};
use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, State};

// --- Lifecycle -------------------------------------------------------

/// Scans `workspace` for project roots and records it as the active
/// workspace (stopping any servers left over from a previously-open one).
/// Called once when a folder is opened — see `LspManager::detect_roots`.
/// Doesn't itself start any server: AGENT.md §9's "lazy mode" means a
/// server only starts the first time a file of its language is opened.
#[tauri::command]
pub async fn lsp_detect_roots(
    app: AppHandle,
    manager: State<'_, LspManager>,
    workspace: String,
) -> Result<Box<[ProjectRootInfo]>, String> {
    Ok(manager.detect_roots(&app, PathBuf::from(workspace)).await)
}

/// Starts (or reuses) the server for `language_id` at `root`, looked up
/// from the current workspace's `.codeeditor/lsp-servers.json` registry.
#[tauri::command]
pub async fn lsp_start(
    app: AppHandle,
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
) -> Result<(), String> {
    manager.start(app, PathBuf::from(root), language_id).await
}

#[tauri::command]
pub async fn lsp_stop(
    app: AppHandle,
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
) -> Result<(), String> {
    manager.stop(&app, &PathBuf::from(root), &language_id).await
}

/// Stops then starts a server, replaying any documents the editor still
/// has open under it. Also resets the crash-backoff counter, so a server
/// that gave up auto-restarting after repeated crashes gets a clean slate.
#[tauri::command]
pub async fn lsp_restart(
    app: AppHandle,
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
) -> Result<(), String> {
    manager.restart(app, PathBuf::from(root), language_id).await
}

/// Stops every server this manager knows about — backs both "close
/// workspace" cleanup and the "LSP: Stop All Servers" command.
#[tauri::command]
pub async fn lsp_stop_all(app: AppHandle, manager: State<'_, LspManager>) -> Result<(), String> {
    manager.stop_all(&app).await;
    Ok(())
}

/// Every server this manager currently knows about (running, starting,
/// crashed, or stopped) — what the status bar's server list is built
/// from.
#[tauri::command]
pub async fn lsp_list_servers(manager: State<'_, LspManager>) -> Result<Box<[ServerInfo]>, String> {
    Ok(manager.list_servers().await)
}

/// Available variants for `language_id` at `root` (e.g. TypeScript's
/// `classic`/`native`, see `registry.rs`) and which one is currently in
/// effect — `None` for a language that only ever has one server, the
/// frontend's cue not to offer a picker for it at all.
#[tauri::command]
pub fn lsp_list_variants(
    workspace: String,
    root: String,
    language_id: String,
) -> Option<VariantInfo> {
    registry::list_variants(
        &PathBuf::from(workspace),
        &language_id,
        &PathBuf::from(root),
    )
}

/// Records "use this variant for this folder specifically" in the
/// workspace's registry file. Doesn't itself restart anything running —
/// call `lsp_restart` after this to apply the new choice, which is what
/// makes switching safe: the existing graceful-shutdown-then-replay-open-
/// documents path `restart` already goes through, not a special case.
#[tauri::command]
pub fn lsp_set_variant_override(
    workspace: String,
    root: String,
    language_id: String,
    variant: String,
) -> Result<(), String> {
    registry::set_root_override(
        &PathBuf::from(workspace),
        &PathBuf::from(root),
        &language_id,
        &variant,
    )
}

/// This server's captured stderr / `window/logMessage` output, most
/// recent `LOG_BUFFER_CAP` lines — backs the "LSP Output" panel's
/// backlog when it's first opened (live lines after that arrive via the
/// `lsp:log` event).
#[tauri::command]
pub async fn lsp_get_logs(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
) -> Result<Box<[Box<str>]>, String> {
    manager.get_logs(&PathBuf::from(root), &language_id).await
}

// --- Document sync -----------------------------------------------------

#[tauri::command]
pub async fn lsp_did_open(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    text: String,
) -> Result<(), String> {
    manager
        .did_open(&PathBuf::from(root), &language_id, &uri, text)
        .await
}

#[tauri::command]
pub async fn lsp_did_change(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    text: String,
    version: i32,
) -> Result<(), String> {
    manager
        .did_change(&PathBuf::from(root), &language_id, &uri, text, version)
        .await
}

#[tauri::command]
pub async fn lsp_did_close(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
) -> Result<(), String> {
    manager
        .did_close(&PathBuf::from(root), &language_id, &uri)
        .await
}

// --- Language features --------------------------------------------------
// Each of these returns the server's raw JSON-RPC `result` — see
// `manager.rs`'s module doc comment for why these aren't re-typed through
// their matching `lsp_types` response struct first.

#[tauri::command]
pub async fn lsp_completion(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
) -> Result<Value, String> {
    manager
        .completion(&PathBuf::from(root), &language_id, &uri, position)
        .await
}

#[tauri::command]
pub async fn lsp_hover(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
) -> Result<Value, String> {
    manager
        .hover(&PathBuf::from(root), &language_id, &uri, position)
        .await
}

#[tauri::command]
pub async fn lsp_definition(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
) -> Result<Value, String> {
    manager
        .definition(&PathBuf::from(root), &language_id, &uri, position)
        .await
}

#[tauri::command]
pub async fn lsp_references(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
    include_declaration: bool,
) -> Result<Value, String> {
    manager
        .references(
            &PathBuf::from(root),
            &language_id,
            &uri,
            position,
            include_declaration,
        )
        .await
}

#[tauri::command]
pub async fn lsp_rename(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
    new_name: String,
) -> Result<Value, String> {
    manager
        .rename(&PathBuf::from(root), &language_id, &uri, position, new_name)
        .await
}

/// `diagnostics` is the raw array of currently-known diagnostics for this
/// file (the frontend's own `lsp:diagnostics`-fed cache) — forwarded so
/// servers that only attach quick-fixes to diagnostics overlapping the
/// requested range can find them.
#[tauri::command]
pub async fn lsp_code_action(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    range: Range,
    diagnostics: Value,
) -> Result<Value, String> {
    manager
        .code_action(&PathBuf::from(root), &language_id, &uri, range, diagnostics)
        .await
}

/// Runs a server-defined command — used when a chosen code action carries
/// a `.command` rather than a directly-appliable `.edit`.
#[tauri::command]
pub async fn lsp_execute_command(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    command: String,
    arguments: Value,
) -> Result<Value, String> {
    manager
        .execute_command(&PathBuf::from(root), &language_id, command, arguments)
        .await
}

#[tauri::command]
pub async fn lsp_signature_help(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    position: Position,
) -> Result<Value, String> {
    manager
        .signature_help(&PathBuf::from(root), &language_id, &uri, position)
        .await
}

#[tauri::command]
pub async fn lsp_document_symbols(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
) -> Result<Value, String> {
    manager
        .document_symbols(&PathBuf::from(root), &language_id, &uri)
        .await
}

#[tauri::command]
pub async fn lsp_formatting(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    tab_size: u32,
    insert_spaces: bool,
) -> Result<Value, String> {
    manager
        .formatting(
            &PathBuf::from(root),
            &language_id,
            &uri,
            tab_size,
            insert_spaces,
        )
        .await
}

/// Fans out to every currently-running server across every project root
/// and merges the results — see `LspManager::workspace_symbol`.
#[tauri::command]
pub async fn lsp_workspace_symbol(
    manager: State<'_, LspManager>,
    query: String,
) -> Result<Box<[Value]>, String> {
    Ok(manager.workspace_symbol(query).await)
}
