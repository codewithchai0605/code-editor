mod commands;
mod lsp;
mod state;

use commands::git::GitManagerState;
use state::{AppState, ProcessMonitorState, PtyState, SearchState};
use tauri::Manager;

use crate::lsp::manager::LspManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be registered before any other plugin — see the plugin's own
        // docs. Its callback fires in the *original* instance whenever a
        // second launch (e.g. a folder's "Open with <App>") happens while
        // this one is already running; extracting a folder path from argv
        // here and funneling it through `set_pending_open_path` is what
        // makes that redirect actually open the folder instead of just
        // focusing an unchanged window.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = commands::native_integration::extract_folder_path(&argv) {
                commands::native_integration::set_pending_open_path(app, path);
            } else if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
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
            commands::fs::reveal_in_file_manager,
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
            commands::lsp::lsp_install,
            commands::lsp::lsp_stop_all,
            commands::lsp::lsp_list_servers,
            commands::lsp::lsp_list_variants,
            commands::lsp::lsp_set_variant_override,
            commands::lsp::lsp_list_server_configs,
            commands::lsp::lsp_upsert_server_config,
            commands::lsp::lsp_delete_server_config,
            commands::lsp::lsp_reset_workspace_config,
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
            commands::native_integration::take_pending_open_path,
            commands::native_integration::context_menu_status,
            commands::native_integration::register_folder_context_menu,
            commands::native_integration::unregister_folder_context_menu,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Cold start: if the OS launched us with a folder argument (the other
    // half of "Open with <App>" — see the single-instance callback above
    // for the "already running" half), record it the same way so the
    // frontend picks it up once it's loaded. `std::env::args()` is the
    // right source on Windows/Linux; macOS never puts it here at all (a
    // Finder "Open With" launch arrives as a `RunEvent::Opened` below,
    // cold start included), so this is simply a no-op there.
    if let Some(path) =
        commands::native_integration::extract_folder_path(&std::env::args().collect::<Vec<_>>())
    {
        commands::native_integration::set_pending_open_path(app.handle(), path);
    }

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            if let Some(pty_state) = app_handle.try_state::<PtyState>() {
                commands::pty::kill_all_sessions(&pty_state);
            }
            if let Some(lsp_manager) = app_handle.try_state::<LspManager>() {
                lsp_manager.kill_all_blocking();
            }
        }
        // The macOS/iOS equivalent of a folder argument: fires both for a
        // cold start (Finder "Open With" launching the app fresh) and,
        // redundantly with but harmlessly alongside the single-instance
        // callback above, while already running. Desktop-only here since
        // this app doesn't target iOS.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            if let Some(path) = urls
                .iter()
                .find_map(|url| commands::native_integration::file_uri_to_path(url.as_str()))
            {
                commands::native_integration::set_pending_open_path(app_handle, path);
            }
        }
        _ => {}
    });
}
