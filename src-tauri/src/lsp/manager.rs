//! Owns the lifecycle of language server processes and the JSON-RPC
//! request/response correlation for talking to them.
//!
//! Design notes (see project AGENT.md for the full spec this builds
//! toward):
//! - Keyed by `(root, language_id)` — two different projects open in the
//!   same app instance (or two different languages in the same project)
//!   each get their own server and their own entry in `servers`.
//! - Diagnostics, progress, and every request-shaped response (completion,
//!   hover, etc.) are forwarded to/from the frontend as raw
//!   `serde_json::Value`, not re-typed through the matching `lsp_types`
//!   response struct — the wire JSON already matches what the frontend
//!   needs, so there's no reason to deserialize into a Rust struct just to
//!   immediately re-serialize the same shape back out. Only *outgoing*
//!   request params are built through `lsp_types` structs, since those
//!   have to match the spec's wire shape exactly and getting a field name
//!   wrong there is a real (and easy to make) mistake.
//! - `pending` (request-id -> response channel) is a plain
//!   `std::sync::Mutex`, not `tokio::sync::Mutex`: every critical section
//!   touching it is a single insert-then-drop or remove-then-drop, never
//!   held across an `.await`. `servers` and each server's `stdin`/`child`,
//!   by contrast, ARE held across awaits (writing a message, killing a
//!   process), so those use `tokio::sync::Mutex`.
//! - A crashed or manually-stopped server's entry is kept in `servers`
//!   (with its status updated in place) rather than removed. This is
//!   deliberate: `list_servers` — and therefore the status bar's "Restart"
//!   button — needs to know about a dead server, not just live ones. The
//!   entry is only ever *replaced* (by a later `start`) or wiped out
//!   wholesale when the workspace changes (`set_workspace`).
//! - `open_docs` tracks, per `(root, language_id)`, the current text of
//!   every file the editor has open under that root — independent of
//!   whether the server backing it happens to be up. That's what lets
//!   `start`/`restart` replay `textDocument/didOpen` for already-open
//!   files after a (re)start, per AGENT.md §5.5/§9, without the frontend
//!   needing to know or care that a restart happened.

use crate::lsp::project_scanner;
use crate::lsp::protocol::{read_message, write_message};
use crate::lsp::registry::{self, ServerConfig};
use lsp_types::{
    ClientCapabilities, CodeActionContext, CodeActionParams, CompletionParams, Diagnostic,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbolParams, ExecuteCommandParams, FormattingOptions,
    GotoDefinitionParams, HoverParams, InitializeParams, InitializedParams, PartialResultParams,
    Position, Range, ReferenceContext, ReferenceParams, RenameParams, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    WorkspaceFolder, WorkspaceSymbolParams,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, watch, Mutex as AsyncMutex};
use tokio::task::JoinSet;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long `stop()` waits for a `shutdown` response before giving up and
/// sending `exit` anyway — a server that doesn't answer `shutdown`
/// shouldn't be able to block the user from stopping it.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
/// How long `stop()` waits, after sending `exit`, for the process to have
/// actually terminated on its own before force-killing it.
const KILL_GRACE_PERIOD: Duration = Duration::from_secs(2);
/// Ring buffer size for a server's captured stderr / `window/logMessage`
/// output — enough scrollback for the "LSP Output" panel to be useful
/// without holding a crashed server's full lifetime of logs in memory.
const LOG_BUFFER_CAP: usize = 500;
/// Capped exponential backoff for auto-restarting a server that exits
/// unexpectedly (AGENT.md §11). After the 5th crash in a row without a
/// successful `Running` status in between, auto-restart gives up and
/// leaves the server `Crashed` until the user restarts it manually — a
/// server that can't stay up for 5 attempts in a row needs a human to
/// look at it, not a 6th silent retry.
const AUTO_RESTART_BACKOFFS: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
];

/// Resolves a command name (e.g. "typescript-language-server") to its real
/// executable path via `PATH`, correctly handling Windows.
///
/// **This is the fix for "program not found" even when the tool is
/// installed and on PATH.** `Command::new("foo")` calls `CreateProcessW`
/// directly on Windows, which does *not* consult the `PATHEXT` environment
/// variable — so it only ever finds `foo.exe`/`foo.com`, never
/// `foo.cmd`/`foo.bat`. Global npm packages on Windows install as `.cmd`
/// shim scripts (e.g. `typescript-language-server.cmd`), not `.exe`
/// files, which is exactly why this worked fine typed into PowerShell
/// (the shell *does* do PATHEXT-aware lookup) but failed when spawned
/// from Rust. The `which` crate performs the same PATH × PATHEXT search a
/// shell would.
///
/// Falls back to the bare name if resolution fails, so the *original*,
/// more actionable "not installed?" error still surfaces from the actual
/// spawn attempt below, rather than a different error from here.
///
/// Security note: the registry's `command`/`args` come from a
/// user-editable, project-local config file (`.codeeditor/lsp-servers.json`)
/// — trusted-but-reviewable input, the same trust level as any other
/// project-local config file that can run commands (compare a repo's
/// `package.json` scripts or `.vscode/tasks.json`).
fn resolve_program(name: &str) -> String {
    which::which(name)
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned))
        .unwrap_or_else(|| name.to_string())
}

/// Identifies one running server instance: a language on a specific
/// project root. Two different projects open in the same app instance
/// (or two different languages in the same project) each get their own
/// server and their own entry here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerKey {
    pub root: PathBuf,
    pub language_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ServerStatus {
    Starting,
    Running,
    Crashed { message: String },
    Stopped,
}

/// One in-flight JSON-RPC request awaiting its response.
struct PendingRequest {
    sender: oneshot::Sender<Result<Value, Value>>,
}

/// The current text of one file the editor has open under a given
/// `(root, language_id)` — see the module doc comment on `open_docs`.
#[derive(Clone)]
struct OpenDoc {
    text: String,
    version: i32,
}

pub struct LspServerInstance {
    stdin: AsyncMutex<ChildStdin>,
    child: AsyncMutex<Child>,
    pending: StdMutex<HashMap<i64, PendingRequest>>,
    next_id: AtomicI64,
    /// Single source of truth for this instance's current status —
    /// updated via `set_status`, read by `list_servers` and by `get`'s
    /// "is it actually running" check. A `watch` channel (rather than a
    /// plain `StdMutex<ServerStatus>`) so a later increment could
    /// subscribe to changes directly if needed, though nothing does yet.
    status: watch::Sender<ServerStatus>,
    /// Raw `ServerCapabilities` JSON from this server's `initialize`
    /// response — `Value::Null` until that completes. Exposed via
    /// `list_servers` so the frontend can gate which features it offers
    /// per-server (e.g. hide "Rename" if `renameProvider` is absent)
    /// without the backend needing to understand every capability flag.
    capabilities: StdMutex<Value>,
    /// Set by `stop()` *before* it tells the process to exit, so the
    /// stdout-reader task's cleanup can tell "we did this on purpose"
    /// (-> `Stopped`) apart from "it went away on its own" (-> `Crashed`,
    /// with auto-restart).
    stopping: AtomicBool,
    /// Ring buffer of this server's stderr lines and `window/logMessage`
    /// notifications, for the "LSP Output" panel. Capped at
    /// `LOG_BUFFER_CAP` lines.
    logs: StdMutex<VecDeque<String>>,
}

impl LspServerInstance {
    async fn write_raw(&self, body: &str) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, body).await
    }

    /// Sends a JSON-RPC request and awaits its response, subject to
    /// `REQUEST_TIMEOUT`. On timeout the pending entry is cleaned up so a
    /// slow/hung server can't leak entries indefinitely.
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap()
            .insert(id, PendingRequest { sender: tx });

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        if let Err(e) = self.write_raw(&body).await {
            self.pending.lock().unwrap().remove(&id);
            return Err(e.to_string());
        }

        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(error))) => Err(error.to_string()),
            Ok(Err(_)) => Err("LSP request cancelled (server disconnected)".to_string()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!(
                    "LSP request '{method}' timed out after {REQUEST_TIMEOUT:?}"
                ))
            }
        }
    }

    /// Sends a JSON-RPC notification — fire-and-forget, no response expected.
    async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        })
        .to_string();
        self.write_raw(&body).await.map_err(|e| e.to_string())
    }

    /// Updates the in-memory status and emits it to the frontend as a
    /// `lsp:status` event, in one place so the two never drift apart.
    fn set_status(&self, app: &AppHandle, key: &ServerKey, status: ServerStatus) {
        self.status.send_replace(status.clone());
        emit_status(app, key, status);
    }

    fn push_log(&self, line: String) {
        let mut logs = self.logs.lock().unwrap();
        logs.push_back(line);
        if logs.len() > LOG_BUFFER_CAP {
            logs.pop_front();
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsEvent {
    root: String,
    language_id: String,
    /// Raw `PublishDiagnosticsParams` JSON — `{ uri, diagnostics, version? }` —
    /// passed through unchanged. See the module doc comment for why this
    /// isn't deserialized into a typed Rust struct first.
    params: Value,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    root: String,
    language_id: String,
    status: ServerStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    root: String,
    language_id: String,
    line: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    root: String,
    language_id: String,
    /// Raw `$/progress` params (`{token, value}`) — same passthrough
    /// rationale as `DiagnosticsEvent`.
    params: Value,
}

/// One entry in `lsp_list_servers`' result — see AGENT.md §6/§10: this is
/// what the status bar's server list and manual restart/stop controls are
/// built from.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub root: Box<str>,
    pub language_id: Box<str>,
    pub status: ServerStatus,
    pub capabilities: Value,
}

/// One entry in `lsp_detect_roots`' result — see `project_scanner`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRootInfo {
    pub root: Box<str>,
    pub language_id: Box<str>,
}

#[derive(Default)]
pub struct LspManager {
    servers: AsyncMutex<HashMap<ServerKey, Arc<LspServerInstance>>>,
    /// The workspace folder `detect_roots` was last called with — used to
    /// find `.codeeditor/lsp-servers.json` (which lives at the workspace
    /// root, not necessarily at any individual server's project root) and
    /// to know when the workspace has actually changed vs. being
    /// re-scanned.
    workspace_root: StdMutex<Option<PathBuf>>,
    /// Bumped every time the workspace changes. A crash's scheduled
    /// auto-restart captures this at crash time and checks it again right
    /// before firing, so a stale timer from a workspace the user has since
    /// closed doesn't spawn a server for it.
    generation: AtomicU64,
    /// Consecutive-crash counter per server, for the auto-restart backoff.
    /// Reset to zero on a successful `Running` status or an explicit
    /// manual restart.
    restart_attempts: StdMutex<HashMap<ServerKey, u32>>,
    open_docs: AsyncMutex<HashMap<ServerKey, HashMap<String, OpenDoc>>>,
}

impl LspManager {
    /// Scans `workspace_root` for project roots (see `project_scanner`)
    /// and records it as the current workspace — stopping any servers
    /// left over from a *previous, different* workspace first. Called
    /// once when a folder is opened (AGENT.md §9's "lazy mode": this only
    /// detects roots, it never itself starts a server — that still only
    /// happens the first time a file of that language is opened).
    pub async fn detect_roots(
        &self,
        app: &AppHandle,
        workspace_root: PathBuf,
    ) -> Box<[ProjectRootInfo]> {
        self.set_workspace(app, workspace_root.clone()).await;
        let registry = registry::load_registry(&workspace_root);
        project_scanner::scan_workspace(&workspace_root, &registry.servers)
            .into_iter()
            .map(|r| ProjectRootInfo {
                root: r.root.to_string_lossy().into_owned().into_boxed_str(),
                language_id: r.language_id.into_boxed_str(),
            })
            .collect()
    }

    async fn set_workspace(&self, app: &AppHandle, root: PathBuf) {
        let previous = self.workspace_root.lock().unwrap().clone();
        if previous.as_ref() == Some(&root) {
            return; // re-detecting the same workspace — leave running servers alone
        }
        *self.workspace_root.lock().unwrap() = Some(root);
        self.generation.fetch_add(1, Ordering::SeqCst);
        if previous.is_some() {
            self.stop_all(app).await;
        }
        self.restart_attempts.lock().unwrap().clear();
        self.open_docs.lock().await.clear();
    }

    /// Starts (or, if already `Starting`/`Running`, reuses) the server for
    /// `language_id` at `root`, looking up its command from the registry
    /// for the current workspace. Replaces any existing `Crashed`/`Stopped`
    /// entry for the same key with a fresh instance.
    pub async fn start(
        &self,
        app: AppHandle,
        root: PathBuf,
        language_id: String,
    ) -> Result<(), String> {
        let key = ServerKey {
            root: root.clone(),
            language_id: language_id.clone(),
        };

        {
            let servers = self.servers.lock().await;
            if let Some(existing) = servers.get(&key) {
                if matches!(
                    *existing.status.borrow(),
                    ServerStatus::Starting | ServerStatus::Running
                ) {
                    return Ok(());
                }
            }
        }

        emit_status(&app, &key, ServerStatus::Starting);

        let workspace_root = self
            .workspace_root
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| root.clone());
        let Some(config) = registry::resolve_server_config(&workspace_root, &language_id, &root)
        else {
            let message = format!(
                "No LSP server configured for '{language_id}'. Add an entry to {}/.codeeditor/lsp-servers.json",
                workspace_root.display()
            );
            emit_status(
                &app,
                &key,
                ServerStatus::Crashed {
                    message: message.clone(),
                },
            );
            return Err(message);
        };

        self.spawn_instance(&app, key, config, &root).await
    }

    /// The actual process-spawn + handshake, split out from `start` only
    /// so `start`'s registry lookup / idempotency guard don't get buried
    /// in the middle of it.
    async fn spawn_instance(
        &self,
        app: &AppHandle,
        key: ServerKey,
        config: ServerConfig,
        root: &Path,
    ) -> Result<(), String> {
        let program = resolve_program(&config.command);
        let mut cmd = Command::new(&program);
        cmd.args(&config.args);
        cmd.current_dir(root);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let message = format!(
                    "Failed to launch {} language server (resolved '{}' to '{program}'): {e}. \
                     Is it installed and on PATH?",
                    key.language_id, config.command
                );
                emit_status(
                    app,
                    &key,
                    ServerStatus::Crashed {
                        message: message.clone(),
                    },
                );
                return Err(message);
            }
        };

        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture server stdout")?;
        let stdin = child.stdin.take().ok_or("Failed to capture server stdin")?;
        let stderr = child
            .stderr
            .take()
            .ok_or("Failed to capture server stderr")?;

        // Surface this server in the Task Manager view alongside terminal
        // shells — register_process/unregister_process were built with
        // exactly this in mind (see their doc comments in
        // commands/process_monitor.rs). Called directly against
        // ProcessMonitorState rather than through the Tauri command
        // wrapper, since this isn't itself a command invocation.
        if let Some(pid) = child.id() {
            if let Some(monitor) = app.try_state::<crate::state::ProcessMonitorState>() {
                monitor.extra.lock().unwrap().insert(
                    process_registry_id(&key),
                    (format!("{} language server", key.language_id), pid),
                );
            }
        }

        let (status_tx, _status_rx) = watch::channel(ServerStatus::Starting);
        let instance = Arc::new(LspServerInstance {
            stdin: AsyncMutex::new(stdin),
            child: AsyncMutex::new(child),
            pending: StdMutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
            status: status_tx,
            capabilities: StdMutex::new(Value::Null),
            stopping: AtomicBool::new(false),
            logs: StdMutex::new(VecDeque::new()),
        });

        self.servers
            .lock()
            .await
            .insert(key.clone(), instance.clone());

        // stdout reader: owns stdout for the lifetime of the process, runs
        // on Tauri's async runtime (its own thread pool — never the
        // webview's JS main thread). Its cleanup, once stdout hits EOF, is
        // the *single* place that decides "was this a deliberate stop or
        // an unexpected crash" and acts accordingly.
        let reader_app = app.clone();
        let reader_key = key.clone();
        let reader_instance = instance.clone();
        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader).await {
                    Ok(Some(text)) => {
                        route_message(&reader_app, &reader_key, &reader_instance, &text)
                    }
                    Ok(None) | Err(_) => break,
                }
            }
            if let Some(monitor) = reader_app.try_state::<crate::state::ProcessMonitorState>() {
                monitor
                    .extra
                    .lock()
                    .unwrap()
                    .remove(&process_registry_id(&reader_key));
            }
            if reader_instance.stopping.load(Ordering::SeqCst) {
                reader_instance.set_status(&reader_app, &reader_key, ServerStatus::Stopped);
            } else {
                let manager = reader_app.state::<LspManager>();
                manager
                    .mark_crashed_and_maybe_restart(
                        &reader_app,
                        &reader_key,
                        "Language server exited unexpectedly".to_string(),
                    )
                    .await;
            }
        });

        // stderr reader: forwards each line into the ring buffer and as a
        // live `lsp:log` event, for the "LSP Output" panel.
        let log_app = app.clone();
        let log_key = key.clone();
        let log_instance = instance.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log_instance.push_log(line.clone());
                let _ = log_app.emit(
                    "lsp:log",
                    LogEvent {
                        root: log_key.root.to_string_lossy().into_owned(),
                        language_id: log_key.language_id.clone(),
                        line,
                    },
                );
            }
        });

        // Handshake: initialize -> initialized.
        let root_uri = path_to_uri(root)?;

        #[allow(deprecated)] // root_uri/root_path are deprecated in favor of
        // workspace_folders, but many servers still expect root_uri to be
        // set — sending both matches what mainstream LSP clients do.
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri.clone()),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: "workspace".to_string(),
            }]),
            capabilities: ClientCapabilities::default(),
            initialization_options: config.initialization_options.clone(),
            ..Default::default()
        };

        let init_params = serde_json::to_value(&params).map_err(|e| e.to_string())?;

        match instance.request("initialize", init_params).await {
            Ok(result) => {
                *instance.capabilities.lock().unwrap() =
                    result.get("capabilities").cloned().unwrap_or(Value::Null);
                let initialized =
                    serde_json::to_value(InitializedParams {}).map_err(|e| e.to_string())?;
                let _ = instance.notify("initialized", initialized).await;
                instance.set_status(app, &key, ServerStatus::Running);
                self.restart_attempts.lock().unwrap().remove(&key);

                // Replay didOpen for every file the editor already has open
                // under this root — covers both the ordinary "first start,
                // but the user had already opened files before the server
                // finished starting" race and a restart mid-session.
                let open_docs = self
                    .open_docs
                    .lock()
                    .await
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();
                for (uri, doc) in open_docs {
                    if let Ok(parsed) = parse_uri(&uri) {
                        let replay_params = DidOpenTextDocumentParams {
                            text_document: TextDocumentItem::new(
                                parsed,
                                key.language_id.clone(),
                                doc.version,
                                doc.text,
                            ),
                        };
                        if let Ok(value) = serde_json::to_value(&replay_params) {
                            let _ = instance.notify("textDocument/didOpen", value).await;
                        }
                    }
                }

                Ok(())
            }
            Err(e) => {
                // Don't touch status or `servers` here — kill the process
                // and let the stdout-reader task's own cleanup (triggered
                // by the EOF this causes) uniformly handle the
                // Crashed-status-plus-auto-restart transition, exactly as
                // it would for any other unexpected exit. This still
                // returns the specific handshake error to *this* caller
                // immediately, rather than the reader task's generic
                // "exited unexpectedly" message.
                let _ = instance.child.lock().await.start_kill();
                Err(e)
            }
        }
    }

    async fn mark_crashed_and_maybe_restart(
        &self,
        app: &AppHandle,
        key: &ServerKey,
        message: String,
    ) {
        if let Some(instance) = self.servers.lock().await.get(key).cloned() {
            instance.set_status(app, key, ServerStatus::Crashed { message });
        } else {
            emit_status(app, key, ServerStatus::Crashed { message });
        }
        self.schedule_auto_restart(app.clone(), key.clone());
    }

    fn schedule_auto_restart(&self, app: AppHandle, key: ServerKey) {
        let attempt = {
            let mut attempts = self.restart_attempts.lock().unwrap();
            let counter = attempts.entry(key.clone()).or_insert(0);
            *counter += 1;
            *counter
        };
        let Some(&delay) = AUTO_RESTART_BACKOFFS.get((attempt - 1) as usize) else {
            eprintln!(
                "[lsp] '{}' at {} crashed {attempt} times in a row — giving up on auto-restart; use manual restart.",
                key.language_id,
                key.root.display()
            );
            return;
        };
        let generation_at_crash = self.generation.load(Ordering::SeqCst);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(delay).await;
            let manager = app.state::<LspManager>();
            if manager.generation.load(Ordering::SeqCst) != generation_at_crash {
                return; // the workspace changed since this crash — stale restart, drop it
            }
            {
                let servers = manager.servers.lock().await;
                if let Some(existing) = servers.get(&key) {
                    if matches!(
                        *existing.status.borrow(),
                        ServerStatus::Starting | ServerStatus::Running
                    ) {
                        return; // already restarted some other way in the meantime
                    }
                }
            }
            let _ = manager
                .start(app.clone(), key.root.clone(), key.language_id.clone())
                .await;
        });
    }

    pub async fn did_open(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        text: String,
    ) -> Result<(), String> {
        let key = ServerKey {
            root: root.to_path_buf(),
            language_id: language_id.to_string(),
        };
        self.open_docs.lock().await.entry(key).or_default().insert(
            uri.to_string(),
            OpenDoc {
                text: text.clone(),
                version: 1,
            },
        );

        let instance = self.get(root, language_id).await?;
        let parsed = parse_uri(uri)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(parsed, language_id.to_string(), 1, text),
        };
        instance
            .notify(
                "textDocument/didOpen",
                serde_json::to_value(&params).map_err(|e| e.to_string())?,
            )
            .await
    }

    /// Full-document sync — the whole new text is sent every time, no
    /// incremental ranges. Simpler and safer to get right than computing
    /// minimal diffs, and every server that speaks LSP accepts it (it's
    /// what a client advertising no incremental-sync capability falls
    /// back to per the spec). The frontend debounces calls to this so
    /// it isn't invoked on literally every keystroke.
    pub async fn did_change(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        text: String,
        version: i32,
    ) -> Result<(), String> {
        let key = ServerKey {
            root: root.to_path_buf(),
            language_id: language_id.to_string(),
        };
        {
            let mut open_docs = self.open_docs.lock().await;
            let docs = open_docs.entry(key).or_default();
            docs.insert(
                uri.to_string(),
                OpenDoc {
                    text: text.clone(),
                    version,
                },
            );
        }

        let instance = self.get(root, language_id).await?;
        let parsed = parse_uri(uri)?;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: parsed,
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        };
        instance
            .notify(
                "textDocument/didChange",
                serde_json::to_value(&params).map_err(|e| e.to_string())?,
            )
            .await
    }

    pub async fn did_close(&self, root: &Path, language_id: &str, uri: &str) -> Result<(), String> {
        let key = ServerKey {
            root: root.to_path_buf(),
            language_id: language_id.to_string(),
        };
        if let Some(docs) = self.open_docs.lock().await.get_mut(&key) {
            docs.remove(uri);
        }

        let instance = self.get(root, language_id).await?;
        let parsed = parse_uri(uri)?;
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: parsed },
        };
        instance
            .notify(
                "textDocument/didClose",
                serde_json::to_value(&params).map_err(|e| e.to_string())?,
            )
            .await
    }

    /// Gracefully shuts a server down: `shutdown` request (bounded by
    /// `SHUTDOWN_TIMEOUT`), then `exit` notification, then a grace period
    /// before force-killing it if it hasn't exited on its own. A no-op if
    /// the server is already stopped/crashed or doesn't exist.
    pub async fn stop(
        &self,
        app: &AppHandle,
        root: &Path,
        language_id: &str,
    ) -> Result<(), String> {
        let key = ServerKey {
            root: root.to_path_buf(),
            language_id: language_id.to_string(),
        };
        let Some(instance) = self.servers.lock().await.get(&key).cloned() else {
            return Ok(());
        };

        if matches!(
            *instance.status.borrow(),
            ServerStatus::Stopped | ServerStatus::Crashed { .. }
        ) {
            return Ok(());
        }

        instance.stopping.store(true, Ordering::SeqCst);
        // Optimistic immediate UI feedback — the stdout-reader task
        // re-confirms this once the process has actually exited, which is
        // harmless (the frontend just receives the same status twice).
        instance.set_status(app, &key, ServerStatus::Stopped);

        let _ =
            tokio::time::timeout(SHUTDOWN_TIMEOUT, instance.request("shutdown", Value::Null)).await;
        let _ = instance.notify("exit", Value::Null).await;

        let exited_on_own = tokio::time::timeout(KILL_GRACE_PERIOD, async {
            loop {
                if instance
                    .child
                    .lock()
                    .await
                    .try_wait()
                    .ok()
                    .flatten()
                    .is_some()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .is_ok();

        if !exited_on_own {
            let _ = instance.child.lock().await.start_kill();
        }

        if let Some(monitor) = app.try_state::<crate::state::ProcessMonitorState>() {
            monitor
                .extra
                .lock()
                .unwrap()
                .remove(&process_registry_id(&key));
        }

        Ok(())
    }

    /// Stops then starts a server, replaying any open documents on the
    /// way back up (see `spawn_instance`'s replay step). Resets the
    /// crash-backoff counter, since a manual restart is an explicit "try
    /// again right now" regardless of how many times it crashed on its
    /// own before this.
    pub async fn restart(
        &self,
        app: AppHandle,
        root: PathBuf,
        language_id: String,
    ) -> Result<(), String> {
        self.stop(&app, &root, &language_id).await?;
        self.restart_attempts.lock().unwrap().remove(&ServerKey {
            root: root.clone(),
            language_id: language_id.clone(),
        });
        self.start(app, root, language_id).await
    }

    /// Best-effort, synchronous, non-graceful kill of every server
    /// process — used only from the app's exit handler, where there's no
    /// async runtime context to do `stop()`'s polite `shutdown`/`exit`
    /// handshake (and no time to wait for one anyway). Mirrors
    /// `pty::kill_all_sessions`'s same "the app is closing, just make sure
    /// nothing is left running" role for terminal sessions.
    pub fn kill_all_blocking(&self) {
        let Ok(servers) = self.servers.try_lock() else {
            return;
        };
        for instance in servers.values() {
            if let Ok(mut child) = instance.child.try_lock() {
                let _ = child.start_kill();
            }
        }
    }

    pub async fn stop_all(&self, app: &AppHandle) {
        let keys: Vec<ServerKey> = self.servers.lock().await.keys().cloned().collect();
        for key in keys {
            let _ = self.stop(app, &key.root, &key.language_id).await;
        }
    }

    /// Every server this manager currently knows about, alive or not —
    /// the source of truth for the status bar's server list.
    pub async fn list_servers(&self) -> Box<[ServerInfo]> {
        self.servers
            .lock()
            .await
            .iter()
            .map(|(key, instance)| ServerInfo {
                root: key.root.to_string_lossy().into_owned().into_boxed_str(),
                language_id: key.language_id.clone().into_boxed_str(),
                status: instance.status.borrow().clone(),
                capabilities: instance.capabilities.lock().unwrap().clone(),
            })
            .collect()
    }

    pub async fn get_logs(
        &self,
        root: &Path,
        language_id: &str,
    ) -> Result<Box<[Box<str>]>, String> {
        let instance = self.get_any(root, language_id).await?;
        let logs = instance
            .logs
            .lock()
            .unwrap()
            .iter()
            .map(|line| line.clone().into_boxed_str())
            .collect();
        Ok(logs)
    }

    /// Looks up a server regardless of its status — for operations (like
    /// reading logs, or stopping it) that make sense even on a crashed
    /// server. Feature requests go through `get` instead, which also
    /// requires the server to actually be `Running`.
    async fn get_any(
        &self,
        root: &Path,
        language_id: &str,
    ) -> Result<Arc<LspServerInstance>, String> {
        let key = ServerKey {
            root: root.to_path_buf(),
            language_id: language_id.to_string(),
        };
        self.servers
            .lock()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("No LSP server recorded for '{language_id}' at this root"))
    }

    async fn get(&self, root: &Path, language_id: &str) -> Result<Arc<LspServerInstance>, String> {
        let instance = self.get_any(root, language_id).await?;
        let status = instance.status.borrow().clone();
        match status {
            ServerStatus::Running => Ok(instance),
            ServerStatus::Starting => Err(format!(
                "Language server for '{language_id}' is still starting"
            )),
            ServerStatus::Crashed { message } => Err(format!(
                "Language server for '{language_id}' crashed: {message}"
            )),
            ServerStatus::Stopped => Err(format!("Language server for '{language_id}' is stopped")),
        }
    }

    async fn send_request(
        &self,
        root: &Path,
        language_id: &str,
        method: &str,
        params: impl Serialize,
    ) -> Result<Value, String> {
        let instance = self.get(root, language_id).await?;
        let value = serde_json::to_value(params).map_err(|e| e.to_string())?;
        instance.request(method, value).await
    }

    pub async fn completion(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
    ) -> Result<Value, String> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        };
        self.send_request(root, language_id, "textDocument/completion", params)
            .await
    }

    pub async fn hover(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
    ) -> Result<Value, String> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request(root, language_id, "textDocument/hover", params)
            .await
    }

    pub async fn definition(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
    ) -> Result<Value, String> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request(root, language_id, "textDocument/definition", params)
            .await
    }

    pub async fn references(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
        include_declaration: bool,
    ) -> Result<Value, String> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };
        self.send_request(root, language_id, "textDocument/references", params)
            .await
    }

    pub async fn rename(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
        new_name: String,
    ) -> Result<Value, String> {
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            new_name,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request(root, language_id, "textDocument/rename", params)
            .await
    }

    /// `diagnostics` is the raw JSON array of currently-known diagnostics
    /// for this file (the frontend already caches these from
    /// `lsp:diagnostics` events) — passed through so servers that only
    /// attach quick-fixes to diagnostics overlapping the requested range
    /// can find them, matching how a real LSP client behaves.
    pub async fn code_action(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        range: Range,
        diagnostics: Value,
    ) -> Result<Value, String> {
        let diagnostics: Vec<Diagnostic> = serde_json::from_value(diagnostics).unwrap_or_default();
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: parse_uri(uri)?,
            },
            range,
            context: CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request(root, language_id, "textDocument/codeAction", params)
            .await
    }

    /// Executes a server-defined command — used when a code action's
    /// `.command` (rather than a directly-appliable `.edit`) is chosen.
    pub async fn execute_command(
        &self,
        root: &Path,
        language_id: &str,
        command: String,
        arguments: Value,
    ) -> Result<Value, String> {
        let arguments: Vec<Value> = serde_json::from_value(arguments).unwrap_or_default();
        let params = ExecuteCommandParams {
            command,
            arguments,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request(root, language_id, "workspace/executeCommand", params)
            .await
    }

    pub async fn signature_help(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        position: Position,
    ) -> Result<Value, String> {
        let params = SignatureHelpParams {
            context: None,
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request(root, language_id, "textDocument/signatureHelp", params)
            .await
    }

    pub async fn document_symbols(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
    ) -> Result<Value, String> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: parse_uri(uri)?,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request(root, language_id, "textDocument/documentSymbol", params)
            .await
    }

    pub async fn formatting(
        &self,
        root: &Path,
        language_id: &str,
        uri: &str,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Value, String> {
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier {
                uri: parse_uri(uri)?,
            },
            options: FormattingOptions {
                tab_size,
                insert_spaces,
                properties: HashMap::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request(root, language_id, "textDocument/formatting", params)
            .await
    }

    /// Fans a workspace-symbol query out to every currently-`Running`
    /// server (across every root/language) and merges the results into
    /// one flat list — the natural shape for "Go to Symbol in Workspace"
    /// when a workspace can have several independent project roots
    /// (AGENT.md §3/§9). A server that errors or times out just
    /// contributes nothing; it doesn't fail the whole query.
    pub async fn workspace_symbol(&self, query: String) -> Box<[Value]> {
        let instances: Vec<Arc<LspServerInstance>> = {
            let servers = self.servers.lock().await;
            servers
                .values()
                .filter(|i| matches!(*i.status.borrow(), ServerStatus::Running))
                .cloned()
                .collect()
        };

        let mut set = JoinSet::new();
        for instance in instances {
            let query = query.clone();
            set.spawn(async move {
                let params = WorkspaceSymbolParams {
                    partial_result_params: PartialResultParams::default(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    query,
                };
                let value = serde_json::to_value(&params).ok()?;
                instance.request("workspace/symbol", value).await.ok()
            });
        }

        let mut results = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Some(value)) = joined {
                // Each server responds with a flat JSON array
                // (SymbolInformation[] or, per 3.17, WorkspaceSymbol[]) —
                // flatten rather than nesting per-server arrays, since the
                // caller just wants one flat, mergeable symbol list.
                if let Some(array) = value.as_array() {
                    results.extend(array.iter().cloned());
                }
            }
        }
        results.into_boxed_slice()
    }
}

/// Dispatches one incoming JSON-RPC message: resolves a pending request if
/// the message carries a matching `id`, or forwards a recognized
/// notification to the frontend as a Tauri event. Unrecognized
/// notifications and server-to-client *requests* (which also carry an
/// `id`, just one we never registered a pending entry for) are
/// intentionally dropped — see AGENT.md §6 for the fuller event surface
/// this deliberately doesn't attempt (e.g. answering
/// `workspace/configuration`).
fn route_message(app: &AppHandle, key: &ServerKey, instance: &LspServerInstance, text: &str) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };

    if let Some(id) = value.get("id").and_then(Value::as_i64) {
        if let Some(pending) = instance.pending.lock().unwrap().remove(&id) {
            let result = if let Some(error) = value.get("error") {
                Err(error.clone())
            } else {
                Ok(value.get("result").cloned().unwrap_or(Value::Null))
            };
            let _ = pending.sender.send(result);
        }
        return;
    }

    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return;
    };
    let Some(params) = value.get("params") else {
        return;
    };

    match method {
        "textDocument/publishDiagnostics" => {
            let _ = app.emit(
                "lsp:diagnostics",
                DiagnosticsEvent {
                    root: key.root.to_string_lossy().into_owned(),
                    language_id: key.language_id.clone(),
                    params: params.clone(),
                },
            );
        }
        "$/progress" => {
            let _ = app.emit(
                "lsp:progress",
                ProgressEvent {
                    root: key.root.to_string_lossy().into_owned(),
                    language_id: key.language_id.clone(),
                    params: params.clone(),
                },
            );
        }
        "window/logMessage" => {
            let message = params.get("message").and_then(Value::as_str).unwrap_or("");
            let line = format!("[window/logMessage] {message}");
            instance.push_log(line.clone());
            let _ = app.emit(
                "lsp:log",
                LogEvent {
                    root: key.root.to_string_lossy().into_owned(),
                    language_id: key.language_id.clone(),
                    line,
                },
            );
        }
        _ => {}
    }
}

/// Key used to register/unregister this server with `ProcessMonitorState`.
/// Includes the root (not just the language) so two same-language servers
/// under different project roots don't collide under the same id.
fn process_registry_id(key: &ServerKey) -> String {
    format!("lsp:{}:{}", key.language_id, key.root.display())
}

fn emit_status(app: &AppHandle, key: &ServerKey, status: ServerStatus) {
    let _ = app.emit(
        "lsp:status",
        StatusEvent {
            root: key.root.to_string_lossy().into_owned(),
            language_id: key.language_id.clone(),
            status,
        },
    );
}

fn path_to_uri(path: &Path) -> Result<Uri, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let with_leading_slash = if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    };
    format!("file://{with_leading_slash}")
        .parse::<Uri>()
        .map_err(|e| format!("Invalid path for URI: {e}"))
}

fn parse_uri(s: &str) -> Result<Uri, String> {
    s.parse::<Uri>()
        .map_err(|e| format!("Invalid URI '{s}': {e}"))
}
