use crate::lsp::manager::LspManager;
use std::path::PathBuf;
use tauri::{AppHandle, State};

/// Starts the TypeScript language server for a workspace root (idempotent
/// if already running). Currently the only server the registry knows
/// about — see `LspManager::start_typescript` doc comment for why this is
/// hardcoded rather than looked up from a config file yet.
#[tauri::command]
pub async fn lsp_start(app: AppHandle, manager: State<'_, LspManager>, root: String) -> Result<(), String> {
    manager.start_typescript(app, PathBuf::from(root)).await
}

#[tauri::command]
pub async fn lsp_did_open(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
    text: String,
) -> Result<(), String> {
    manager.did_open(&PathBuf::from(root), &language_id, &uri, text).await
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
    manager.did_change(&PathBuf::from(root), &language_id, &uri, text, version).await
}

#[tauri::command]
pub async fn lsp_did_close(
    manager: State<'_, LspManager>,
    root: String,
    language_id: String,
    uri: String,
) -> Result<(), String> {
    manager.did_close(&PathBuf::from(root), &language_id, &uri).await
}

#[tauri::command]
pub async fn lsp_stop(app: AppHandle, manager: State<'_, LspManager>, root: String, language_id: String) -> Result<(), String> {
    manager.stop(&app, &PathBuf::from(root), &language_id).await
}
