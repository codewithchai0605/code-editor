use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// Shared across every command via `tauri::State<AppState>`.
///
/// The watcher fields are behind a `Mutex` because Tauri commands can run
/// concurrently on different threads — opening a second workspace while a
/// copy from the first is still in flight must not tear either one down
/// half-finished.
#[derive(Default)]
pub struct AppState {
    pub workspace_root: Mutex<Option<PathBuf>>,
    /// Keeping the debouncer alive is what keeps the watch active — dropping
    /// it (e.g. by replacing this with `None`) stops watching immediately.
    pub watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
    /// Same idea as `watcher` above, but scoped to a repo's `.git` internals
    /// (HEAD/index/refs) rather than the workspace's files — see
    /// commands/git.rs. A separate field (rather than reusing `watcher`)
    /// since a workspace can be watched for file changes without being a
    /// git repo at all, and the two watchers have independent lifetimes.
    pub git_watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
}

/// Everything needed to keep one terminal tab alive: the PTY handle (for
/// resize), the writer half (for input), and the child process (so we can
/// kill it on close). The read half lives only inside that session's
/// background reader thread — see commands/pty.rs — since a blocking
/// `Read` can't usefully be shared beyond the thread that owns it.
pub struct TerminalSession {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct PtyState {
    pub sessions: Mutex<HashMap<String, TerminalSession>>,
    next_id: AtomicU32,
}

impl PtyState {
    /// Simple incrementing id ("term-1", "term-2", ...) rather than a UUID
    /// dependency — uniqueness within one running app instance is all this
    /// needs.
    pub fn next_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("term-{n}")
    }
}

/// Backs the Task Manager view. `system` is a *long-lived* `sysinfo::System`
/// rather than one built fresh per call — sysinfo computes CPU-usage deltas
/// against the previous refresh of the same instance, so recreating it
/// every poll would make every reading come back as 0%.
///
/// `extra` lets a future subsystem (a language server, a formatter daemon,
/// anything spawned outside `PtyState`) show up in the same list without
/// this module needing to know about it in advance — see
/// commands/process_monitor.rs's `register_process`/`unregister_process`.
#[derive(Default)]
pub struct ProcessMonitorState {
    pub system: Mutex<sysinfo::System>,
    pub extra: Mutex<HashMap<String, (String, u32)>>,
}
