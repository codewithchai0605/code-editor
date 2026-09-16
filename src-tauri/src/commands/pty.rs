use crate::state::{PtyState, RunningPty, TerminalSession};
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        CommandBuilder::new("pwsh")
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
    }
}

/// Small helper so a panic while holding a lock anywhere in the app doesn't
/// permanently poison every terminal session behind it. We don't have a
/// stronger invariant to protect here, so recovering the guard and carrying
/// on is preferable to every subsequent `.lock()` call panicking forever.
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

const FLUSH_INTERVAL: Duration = Duration::from_millis(8);
const MAX_BUFFERED: usize = 64 * 1024;
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(300);

fn finish_session(app: &AppHandle, id: &str, reason: ExitReason, already_reported: &AtomicBool) {
    if already_reported.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(state) = app.try_state::<PtyState>() {
        // Just remove the session - nothing reads an intermediate
        // `Exited`/`Failed` value before it's gone, so inserting that state
        // and then immediately removing it was dead code. If the frontend
        // later needs to observe a terminal's final state before it
        // disappears, add an explicit "acknowledge" command instead of
        // reintroducing an insert here.
        state.sessions.lock_recover().remove(id);
    }
    let _ = app.emit(&format!("pty:exit:{id}"), reason);
}

/// Reads PTY output and emits it to the frontend.
///
/// This spawns exactly one thread that ever calls `emit` for this session
/// (the "emitter" below), fed by a channel from the reader. Because there is
/// only ever one consumer, chunks are always emitted in the order they were
/// read - the previous design had two independent threads racing to flush
/// the same buffer, which could reorder output. `rx.recv()` blocks when
/// there's nothing to send, so an idle terminal now costs zero wakeups
/// instead of waking up ~125 times a second forever.
fn spawn_reader_thread(
    app: AppHandle,
    id: String,
    mut reader: Box<dyn Read + Send>,
    already_reported: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

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
                let _ = app.emit(&format!("pty:data:{id}"), buf);
            }
        });
    }

    thread::spawn(move || {
        let mut read_buf = [0u8; 8192];
        let exit_reason = loop {
            match reader.read(&mut read_buf) {
                Ok(0) => break ExitReason::Exited,
                Ok(n) => {
                    if tx.send(read_buf[..n].to_vec()).is_err() {
                        // Emitter thread is gone; nothing left to do but stop reading.
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

        // Drop the sender so the emitter thread's `rx.recv()` returns `Err`
        // once any already-queued chunks are drained, letting it exit
        // cleanly instead of lingering.
        drop(tx);

        finish_session(&app, &id, exit_reason, &already_reported);
    });
}

/// Belt-and-suspenders exit detection alongside the reader thread's own
/// EOF/error handling above. On Windows, ConPTY does not reliably signal
/// EOF on the read side just because the foreground process exited - so a
/// shell exiting normally (typing `exit`) could leave `reader.read()`
/// simply never returning again, even though the process is genuinely
/// gone.
///
/// This asks the child process handle itself (`Child::try_wait`) rather
/// than snapshotting the whole system process table via `sysinfo`: it's the
/// same primitive `portable_pty` already uses internally, it's one fewer
/// dependency, and it sidesteps the (rare, but real) pid-reuse race you get
/// from looking a raw pid back up in the OS process table.
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
                // Not running anymore (already cleaned up elsewhere) - nothing to watch.
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
            std::process::Command::new("kill")
                .args(["-KILL", &format!("-{pid}")])
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

#[tauri::command]
pub fn write_to_terminal(
    pty_state: State<PtyState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = pty_state.sessions.lock_recover();
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| "Terminal session not found.".to_string())?;
    match session {
        TerminalSession::Running(pty) => pty
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string()),
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
