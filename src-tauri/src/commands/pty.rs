use crate::state::{PtyState, TerminalSession};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

/// Picks a sensible default shell per platform. On Windows this tries
/// PowerShell first (matching what most current-generation editors default
/// to) and falls back to whatever `ComSpec` points at — typically
/// `cmd.exe` — if PowerShell isn't on PATH. On Unix, `$SHELL` if set,
/// otherwise `/bin/bash`.
fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        CommandBuilder::new("powershell.exe")
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
    }
}

/// Reads PTY output on a dedicated OS thread (the reader is a blocking
/// `Read`, so this can't be an async task) and forwards each chunk to the
/// frontend as raw bytes — not a decoded `String`. Converting to UTF-8 in
/// Rust here would risk splitting a multi-byte character across two reads;
/// xterm.js's own `write()` accepts a `Uint8Array` and does streaming
/// UTF-8 decoding correctly, so the raw bytes are handed over as-is.
fn spawn_reader_thread(app: AppHandle, id: String, mut reader: Box<dyn Read + Send>) {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = app.emit(&format!("pty:data:{id}"), buf[..n].to_vec());
                }
                Err(_) => break,
            }
        }
        // The shell exited (or the pipe broke) — drop the session so it
        // doesn't linger, and let the frontend know to mark the tab closed.
        if let Some(state) = app.try_state::<PtyState>() {
            state.sessions.lock().unwrap().remove(&id);
        }
        let _ = app.emit(&format!("pty:exit:{id}"), ());
    });
}

#[tauri::command]
pub fn create_terminal(app: AppHandle, pty_state: State<PtyState>, cwd: Option<String>) -> Result<String, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = default_shell_command();
    if let Some(dir) = &cwd {
        cmd.cwd(dir);
    }

    let child = match pair.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(_) if cfg!(windows) => {
            // PowerShell wasn't available — fall back to ComSpec (cmd.exe).
            let mut fallback = CommandBuilder::new_default_prog();
            if let Some(dir) = &cwd {
                fallback.cwd(dir);
            }
            pair.slave.spawn_command(fallback).map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
    };

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let id = pty_state.next_id();
    pty_state.sessions.lock().unwrap().insert(
        id.clone(),
        TerminalSession {
            master: pair.master,
            writer,
            child,
        },
    );

    spawn_reader_thread(app, id.clone(), reader);

    Ok(id)
}

#[tauri::command]
pub fn write_to_terminal(pty_state: State<PtyState>, id: String, data: String) -> Result<(), String> {
    let mut sessions = pty_state.sessions.lock().unwrap();
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| "Terminal session not found.".to_string())?;
    session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_terminal(pty_state: State<PtyState>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    let sessions = pty_state.sessions.lock().unwrap();
    let session = sessions.get(&id).ok_or_else(|| "Terminal session not found.".to_string())?;
    session
        .master
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
    if let Some(mut session) = pty_state.sessions.lock().unwrap().remove(&id) {
        let _ = session.child.kill();
    }
    Ok(())
}
