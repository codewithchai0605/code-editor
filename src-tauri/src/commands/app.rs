/// Returns the running application's version, as declared in `Cargo.toml`.
///
/// This is deliberately trivial — its job right now is just to prove the
/// frontend/backend IPC round trip works before the filesystem, LSP, PTY and
/// plugin commands land in later phases. The status bar calls this on mount.
#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
