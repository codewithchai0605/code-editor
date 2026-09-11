use crate::state::{PtyState, RunningPty, TerminalSession};
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager, State};

fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        CommandBuilder::new("pwsh")
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
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
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(
            id.to_string(),
            match &reason {
                ExitReason::Exited => TerminalSession::Exited,
                ExitReason::Failed { .. } => TerminalSession::Failed,
            },
        );
        sessions.remove(id);
    }
    let _ = app.emit(&format!("pty:exit:{id}"), reason);
}

fn spawn_reader_thread(
    app: AppHandle,
    id: String,
    mut reader: Box<dyn Read + Send>,
    already_reported: Arc<AtomicBool>,
) {
    let buffer = Arc::new(Mutex::new(Vec::<u8>::with_capacity(16 * 1024)));
    let alive = Arc::new(AtomicBool::new(true));

    {
        let buffer = Arc::clone(&buffer);
        let alive = Arc::clone(&alive);
        let app = app.clone();
        let id = id.clone();
        thread::spawn(move || {
            while alive.load(Ordering::Relaxed) {
                thread::sleep(FLUSH_INTERVAL);
                let mut buf = buffer.lock().unwrap();
                if buf.is_empty() {
                    continue;
                }
                let chunk = std::mem::take(&mut *buf);
                drop(buf);
                let _ = app.emit(&format!("pty:data:{id}"), chunk);
            }
        });
    }

    thread::spawn(move || {
        let mut read_buf = [0u8; 8192];
        let exit_reason = loop {
            match reader.read(&mut read_buf) {
                Ok(0) => break ExitReason::Exited,
                Ok(n) => {
                    let mut buf = buffer.lock().unwrap();
                    buf.extend_from_slice(&read_buf[..n]);
                    if buf.len() >= MAX_BUFFERED {
                        let chunk = std::mem::take(&mut *buf);
                        drop(buf);
                        let _ = app.emit(&format!("pty:data:{id}"), chunk);
                    }
                }
                Err(e) => {
                    break ExitReason::Failed {
                        message: e.to_string(),
                    }
                }
            }
        };

        {
            let mut buf = buffer.lock().unwrap();
            if !buf.is_empty() {
                let chunk = std::mem::take(&mut *buf);
                drop(buf);
                let _ = app.emit(&format!("pty:data:{id}"), chunk);
            }
        }
        alive.store(false, Ordering::Relaxed);

        finish_session(&app, &id, exit_reason, &already_reported);
    });
}

/// Belt-and-suspenders exit detection alongside the reader thread's own
/// EOF/error handling above. On Windows, ConPTY does not reliably signal
/// EOF on the read side just because the foreground process exited — so a
/// shell exiting normally (typing `exit`) could leave `reader.read()`
/// simply never returning again, even though the process is genuinely
/// gone. Polling whether the PID itself is still alive is
/// platform-independent and doesn't depend on PTY-specific EOF semantics.
fn spawn_exit_watcher(
    app: AppHandle,
    id: String,
    pid: Option<u32>,
    already_reported: Arc<AtomicBool>,
) {
    let Some(pid) = pid else { return };
    thread::spawn(move || {
        let sys_pid = Pid::from(pid as usize);
        let mut sys = System::new();
        loop {
            thread::sleep(EXIT_POLL_INTERVAL);
            if already_reported.load(Ordering::SeqCst) {
                return;
            }
            sys.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
            if sys.process(sys_pid).is_none() {
                finish_session(&app, &id, ExitReason::Exited, &already_reported);
                return;
            }
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
    let mut sessions = pty_state.sessions.lock().unwrap();
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
        .lock()
        .unwrap()
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
            .lock()
            .unwrap()
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
        let mut sessions = pty_state.sessions.lock().unwrap();
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
    spawn_exit_watcher(app.clone(), id.clone(), pid, Arc::clone(&already_reported));
    spawn_reader_thread(app, id, reader, already_reported);

    Ok(())
}

#[tauri::command]
pub fn write_to_terminal(
    pty_state: State<PtyState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = pty_state.sessions.lock().unwrap();
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
    let sessions = pty_state.sessions.lock().unwrap();
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
    let mut sessions = pty_state.sessions.lock().unwrap();
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
