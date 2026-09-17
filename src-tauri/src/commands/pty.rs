use crate::state::{PtyState, RunningPty, TerminalSession};
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

/// Small helper so a panic while holding a lock anywhere in the app doesn't
/// permanently poison every terminal session behind it.
trait LockRecover<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> LockRecover<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum ExitReason {
    Exited,
    Failed { message: String },
}

// Tuned for code editors: 4ms keeps the UI feeling instant,
// 32KB prevents flooding the Tauri IPC event loop during heavy output.
const FLUSH_INTERVAL: Duration = Duration::from_millis(4);
const MAX_BUFFERED: usize = 32 * 1024;
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(500);

fn finish_session(app: &AppHandle, id: &str, reason: ExitReason, already_reported: &AtomicBool) {
    if already_reported.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Some(state) = app.try_state::<PtyState>() {
        state.sessions.lock_recover().remove(id);
    }

    let _ = app.emit(&format!("pty:exit:{id}"), reason);
}

/// Reads PTY output and emits it to the frontend.
fn spawn_reader_thread(
    app: AppHandle,
    id: String,
    mut reader: Box<dyn Read + Send>,
    already_reported: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    // Emitter thread: batches chunks to minimize Tauri IPC overhead
    {
        let app = app.clone();
        let id = id.clone();
        thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut buf = first;
                let deadline = Instant::now() + FLUSH_INTERVAL;

                while buf.len() < MAX_BUFFERED {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(chunk) => buf.extend_from_slice(&chunk),
                        Err(_) => break,
                    }
                }
                // Emit as raw bytes. (Ensure your Tauri frontend expects Uint8Array)
                let _ = app.emit(&format!("pty:data:{id}"), buf);
            }
        });
    }

    // Reader thread: reads from the PTY and sends to the emitter
    thread::spawn(move || {
        let mut read_buf = [0u8; 8192];
        let exit_reason = loop {
            match reader.read(&mut read_buf) {
                Ok(0) => break ExitReason::Exited,
                Ok(n) => {
                    if tx.send(read_buf[..n].to_vec()).is_err() {
                        break ExitReason::Exited;
                    }
                }
                Err(e) => {
                    break ExitReason::Failed {
                        message: e.to_string(),
                    }
                }
            }
        };

        drop(tx);
        finish_session(&app, &id, exit_reason, &already_reported);
    });
}

/// Belt-and-suspenders exit detection for Windows ConPTY.
fn spawn_exit_watcher(app: AppHandle, id: String, already_reported: Arc<AtomicBool>) {
    thread::spawn(move || loop {
        thread::sleep(EXIT_POLL_INTERVAL);
        if already_reported.load(Ordering::SeqCst) {
            return;
        }

        let Some(state) = app.try_state::<PtyState>() else {
            return;
        };

        let exited = {
            let mut sessions = state.sessions.lock_recover();
            match sessions.get_mut(&id) {
                Some(TerminalSession::Running(pty)) => {
                    matches!(pty.child.try_wait(), Ok(Some(_)))
                }
                _ => true,
            }
        };

        if exited {
            finish_session(&app, &id, ExitReason::Exited, &already_reported);
            return;
        }
    });
}

fn kill_process_tree(pid: Option<u32>, child: &mut Box<dyn Child + Send + Sync>) {
    let tree_kill_ok = pid.is_some_and(|pid| {
        let result = if cfg!(windows) {
            std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output()
        } else {
            // Use SIGTERM instead of SIGKILL to allow processes to clean up gracefully
            let pid_arg = format!("-{}", pid);
            std::process::Command::new("kill")
                .args(["-TERM", &pid_arg])
                .output()
        };
        result.map(|o| o.status.success()).unwrap_or(false)
    });

    if !tree_kill_ok {
        let _ = child.kill();
    }
}

pub fn kill_all_sessions(pty_state: &PtyState) {
    let mut sessions = pty_state.sessions.lock_recover();
    for session in sessions.values_mut() {
        if let TerminalSession::Running(pty) = session {
            let pid = pty.child.process_id();
            kill_process_tree(pid, &mut pty.child);
        }
    }
    sessions.clear();
}

#[tauri::command]
pub fn reserve_terminal_id(pty_state: State<PtyState>) -> String {
    let id = pty_state.next_id();
    pty_state
        .sessions
        .lock_recover()
        .insert(id.clone(), TerminalSession::Creating);
    id
}

/// Smart shell detection. Prefers user env vars, falls back gracefully on Windows.
fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let shell = std::env::var("EDITOR_SHELL").unwrap_or_else(|_| {
            // Check if PowerShell Core (pwsh) is installed in default location
            if std::path::Path::new("C:\\Program Files\\PowerShell\\7\\pwsh.exe").exists() {
                "pwsh".to_string()
            } else {
                "powershell.exe".to_string()
            }
        });
        CommandBuilder::new(shell)
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
    }
}

#[tauri::command]
pub fn create_terminal(
    app: AppHandle,
    pty_state: State<PtyState>,
    id: String,
    cwd: Option<String>,
) -> Result<(), String> {
    fn fail(pty_state: &PtyState, id: &str, err: impl std::fmt::Display) -> String {
        let message = err.to_string();
        pty_state
            .sessions
            .lock_recover()
            .insert(id.to_string(), TerminalSession::Failed);
        message
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| fail(&pty_state, &id, e))?;

    let mut cmd = default_shell_command();
    if let Some(dir) = &cwd {
        cmd.cwd(dir);
    }

    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(_) if cfg!(windows) => {
            // Fallback for Windows if the preferred shell fails
            let mut fallback = CommandBuilder::new_default_prog();
            if let Some(dir) = &cwd {
                fallback.cwd(dir);
            }
            pair.slave
                .spawn_command(fallback)
                .map_err(|e| fail(&pty_state, &id, e))?
        }
        Err(e) => return Err(fail(&pty_state, &id, e)),
    };

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| fail(&pty_state, &id, e))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| fail(&pty_state, &id, e))?;
    let pid = child.process_id();

    {
        let mut sessions = pty_state.sessions.lock_recover();
        if matches!(sessions.get(&id), Some(TerminalSession::Closing)) {
            kill_process_tree(pid, &mut child);
            sessions.remove(&id);
            return Ok(());
        }
        sessions.insert(
            id.clone(),
            TerminalSession::Running(RunningPty {
                master: pair.master,
                writer,
                child,
            }),
        );
    }

    let already_reported = Arc::new(AtomicBool::new(false));
    spawn_exit_watcher(app.clone(), id.clone(), Arc::clone(&already_reported));
    spawn_reader_thread(app, id, reader, already_reported);

    Ok(())
}

/// Accepts raw bytes instead of String to prevent UTF-8 validation overhead
/// and avoid corrupting binary terminal escape sequences.
#[tauri::command]
pub fn write_to_terminal(
    pty_state: State<PtyState>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut sessions = pty_state.sessions.lock_recover();
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| "Terminal session not found.".to_string())?;

    match session {
        TerminalSession::Running(pty) => pty.writer.write_all(&data).map_err(|e| e.to_string()),
        other => Err(other.describe().to_string()),
    }
}

#[tauri::command]
pub fn resize_terminal(
    pty_state: State<PtyState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let sessions = pty_state.sessions.lock_recover();
    let session = sessions
        .get(&id)
        .ok_or_else(|| "Terminal session not found.".to_string())?;

    let pty = session
        .running()
        .ok_or_else(|| session.describe().to_string())?;

    pty.master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_terminal(pty_state: State<PtyState>, id: String) -> Result<(), String> {
    let mut sessions = pty_state.sessions.lock_recover();
    if !sessions.contains_key(&id) {
        return Ok(());
    }

    if let Some(TerminalSession::Running(mut pty)) =
        sessions.insert(id.clone(), TerminalSession::Closing)
    {
        let pid = pty.child.process_id();
        kill_process_tree(pid, &mut pty.child);
    }
    Ok(())
}
