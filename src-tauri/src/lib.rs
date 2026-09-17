mod commands;
mod lsp;
mod state;

use commands::git::GitManagerState;
use state::{AppState, ProcessMonitorState, PtyState, SearchState};
use tauri::Manager;

use crate::lsp::manager::LspManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(PtyState::default())
        .manage(ProcessMonitorState::default())
        .manage(GitManagerState::default())
        .manage(LspManager::default())
        .manage(SearchState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app::app_version,
            commands::fs::open_workspace,
            commands::fs::read_dir,
            commands::fs::create_entry,
            commands::fs::rename_entry,
            commands::fs::delete_entries,
            commands::fs::copy_paths,
            commands::fs::move_paths,
            commands::fs::read_text_file,
            commands::fs::write_text_file,
            commands::fs::read_binary_preview,
            commands::fs::list_workspace_files,
            commands::pty::reserve_terminal_id,
            commands::pty::create_terminal,
            commands::pty::write_to_terminal,
            commands::pty::resize_terminal,
            commands::pty::close_terminal,
            commands::git::git_list_repos,
            commands::git::git_status,
            commands::git::git_diff,
            commands::git::git_file_baseline,
            commands::git::git_stage_file,
            commands::git::git_stage_all,
            commands::git::git_unstage_file,
            commands::git::git_unstage_all,
            commands::git::git_discard_file,
            commands::git::git_discard_all,
            commands::git::git_commit,
            commands::git::git_unwatch_all,
            commands::process_monitor::list_tracked_processes,
            commands::process_monitor::register_process,
            commands::process_monitor::unregister_process,
            commands::process_monitor::kill_process,
            commands::lsp::lsp_detect_roots,
            commands::lsp::lsp_start,
            commands::lsp::lsp_stop,
            commands::lsp::lsp_restart,
            commands::lsp::lsp_stop_all,
            commands::lsp::lsp_list_servers,
            commands::lsp::lsp_list_variants,
            commands::lsp::lsp_set_variant_override,
            commands::lsp::lsp_get_logs,
            commands::lsp::lsp_did_open,
            commands::lsp::lsp_did_change,
            commands::lsp::lsp_did_close,
            commands::lsp::lsp_completion,
            commands::lsp::lsp_hover,
            commands::lsp::lsp_definition,
            commands::lsp::lsp_references,
            commands::lsp::lsp_rename,
            commands::lsp::lsp_code_action,
            commands::lsp::lsp_execute_command,
            commands::lsp::lsp_signature_help,
            commands::lsp::lsp_document_symbols,
            commands::lsp::lsp_formatting,
            commands::lsp::lsp_workspace_symbol,
            commands::search::workspace_search,
            commands::search::stop_workspace_search,
            commands::search::workspace_replace_all,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(pty_state) = app_handle.try_state::<PtyState>() {
                    commands::pty::kill_all_sessions(&pty_state);
                }
                if let Some(lsp_manager) = app_handle.try_state::<LspManager>() {
                    lsp_manager.kill_all_blocking();
                }
            }
        });
}
