use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct AppState {
    pub workspace_root: Mutex<Option<PathBuf>>,
    pub watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
    /// A folder path the OS asked this app to open — from a command-line
    /// argument on cold start (Windows/Linux), a `RunEvent::Opened` launch
    /// (macOS), or a later launch that `tauri-plugin-single-instance`
    /// redirected into this already-running instance. Set once by whichever
    /// of those fired, and read (and cleared) exactly once by the frontend's
    /// `take_pending_open_path` call on startup — see
    /// `commands::native_integration`. A plain `Mutex<Option<String>>`
    /// rather than an event-only handoff because the frontend's listener
    /// for a *live* "already running" launch is registered well after the
    /// window exists, so a launch that raced ahead of that registration
    /// (in particular every cold start, and macOS's `Opened` event, which
    /// can fire before the page has finished loading) would otherwise be
    /// silently missed.
    pub pending_open_path: Mutex<Option<String>>,
}

/// The PTY/process handles for a session that's actually alive. Split out
/// from `TerminalSession` so the enum below can represent "doesn't have
/// these yet" (`Creating`) and "no longer has these" (`Closing`/`Exited`/
/// `Failed`) states where calling `.write()`/`.resize()` simply isn't
/// possible to express, rather than needing a runtime check against
/// fields that are always present but sometimes meaningless.
pub struct RunningPty {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
}

/// Explicit lifecycle for one terminal tab's backing process:
///
/// ```text
/// Creating -> Running -> Closing -> Exited
///                    \              /
///                     ----> Failed -
/// ```
///
/// `Creating` covers the brief window between `reserve_terminal_id` and
/// the PTY/shell actually spawning inside `create_terminal`; `Closing`
/// covers the window between asking the child to die (`close_terminal`,
/// or app shutdown) and the reader thread actually observing it gone.
/// `Exited`/`Failed` are intentionally short-lived — see pty.rs, which
/// removes the entry from the map shortly after reaching either, so a
/// long session opening/closing many terminals never accumulates dead
/// entries in memory.
pub enum TerminalSession {
    Creating,
    Running(RunningPty),
    Closing,
    Failed,
}

impl TerminalSession {
    /// `None` for every state except `Running` — the single place every
    /// write/resize/close/PID-lookup call goes through, so "what do I do
    /// if this session isn't actually running" only needs answering once.
    pub fn running(&self) -> Option<&RunningPty> {
        match self {
            TerminalSession::Running(pty) => Some(pty),
            _ => None,
        }
    }

    /// Human-readable reason a `running()` call came back empty.
    pub fn describe(&self) -> &'static str {
        match self {
            TerminalSession::Creating => "Terminal is still starting.",
            TerminalSession::Running(_) => "Terminal is running.",
            TerminalSession::Closing => "Terminal is closing.",
            TerminalSession::Failed => "Terminal failed to start.",
        }
    }
}

#[derive(Default)]
pub struct PtyState {
    pub sessions: Mutex<HashMap<String, TerminalSession>>,
    next_id: AtomicU32,
}

impl PtyState {
    pub fn next_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("term-{n}")
    }
}

#[derive(Default)]
pub struct ProcessMonitorState {
    pub system: Mutex<sysinfo::System>,
    pub extra: Mutex<HashMap<String, (String, u32)>>,
}

/// A single monotonic counter shared with every in-flight workspace search.
///
/// Bumped once per `workspace_search`/`stop_workspace_search` call. Each
/// search's parallel walker threads capture the post-bump value as their own
/// "generation" and compare it against this shared counter's *current* value
/// on every visited entry; the moment a newer search (or an explicit stop)
/// bumps it again, every thread from the older search sees a mismatch and
/// quits within one entry — a lock-free way to keep a debounced, rapidly
/// re-triggered search from ever running two workspace walks at once, which
/// matters for both wasted CPU and (since results are buffered in-flight)
/// memory. Wrapped in an `Arc` rather than a bare `AtomicU64` so a clone of
/// just the counter — not the whole `State` handle, which doesn't outlive
/// the command call — can be moved into the search's background thread.
#[derive(Default)]
pub struct SearchState {
    generation: Arc<AtomicU64>,
}

impl SearchState {
    /// Advances to a new generation and returns it — the value any search
    /// started *by this call* should tag itself with.
    pub fn bump(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// A cheap, 'static-lifetime clone of the shared counter for a
    /// background thread to poll against.
    pub fn handle(&self) -> Arc<AtomicU64> {
        self.generation.clone()
    }
}
