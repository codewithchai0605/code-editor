use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use std::path::PathBuf;
use std::sync::Mutex;

/// Shared across every command via `tauri::State<AppState>`.
///
/// Both fields are behind a `Mutex` because Tauri commands can run
/// concurrently on different threads — opening a second workspace while a
/// copy from the first is still in flight must not tear either one down
/// half-finished.
#[derive(Default)]
pub struct AppState {
    pub workspace_root: Mutex<Option<PathBuf>>,
    /// Keeping the debouncer alive is what keeps the watch active — dropping
    /// it (e.g. by replacing this with `None`) stops watching immediately.
    pub watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
}
