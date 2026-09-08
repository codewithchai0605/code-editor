mod commands;
mod state;

use state::{AppState, ProcessMonitorState, PtyState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(PtyState::default())
        .manage(ProcessMonitorState::default())
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
            commands::pty::create_terminal,
            commands::pty::write_to_terminal,
            commands::pty::resize_terminal,
            commands::pty::close_terminal,
            commands::git::git_status,
            commands::git::git_watch,
            commands::git::git_unwatch,
            commands::process_monitor::list_tracked_processes,
            commands::process_monitor::register_process,
            commands::process_monitor::unregister_process,
            commands::process_monitor::kill_process,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
