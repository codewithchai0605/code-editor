use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub workspace_root: Mutex<Option<PathBuf>>,
    pub watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
    // pub git_watcher: Mutex<Option<Debouncer<RecommendedWatcher>>>,
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
    Exited,
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
            TerminalSession::Exited => "Terminal has exited.",
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