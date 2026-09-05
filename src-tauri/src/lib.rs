mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
