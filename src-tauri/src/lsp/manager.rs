//! Owns the lifecycle of language server processes and the JSON-RPC
//! request/response correlation for talking to them.
//!
//! Design notes (see project AGENT.md for the full spec this incrementally
//! builds toward):
//! - Keyed by `(root, language_id)` from the start, even though Step 1
//!   only ever has one entry — this is additive when multi-root support
//!   lands later rather than a rework.
//! - Diagnostics are forwarded to the frontend as raw `serde_json::Value`,
//!   not re-typed through `lsp_types::PublishDiagnosticsParams` — the
//!   wire JSON already matches what the frontend needs (`{uri,
//!   diagnostics}`), so there is no reason to deserialize into a Rust
//!   struct just to immediately re-serialize the same shape back out.
//!   This also means the whole `Diagnostic`/`Range`/`Position` type
//!   surface doesn't need to round-trip exactly through this file.
//! - `pending` (request-id -> response channel) is a plain
//!   `std::sync::Mutex`, not `tokio::sync::Mutex`: every critical section
//!   touching it is a single insert-then-drop or remove-then-drop, never
//!   held across an `.await`, so the lighter std mutex is the right tool.
//!   `servers` and each server's `stdin`/`child`, by contrast, ARE held
//!   across awaits (writing a message, killing a process), so those use
//!   `tokio::sync::Mutex`.

use crate::lsp::protocol::{read_message, write_message};
use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializedParams, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    Uri, VersionedTextDocumentIdentifier, WorkspaceFolder,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

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
/// Security note for later steps: Windows' `CreateProcess` has
/// undocumented-but-real special handling that silently routes `.bat`/
/// `.cmd` targets through `cmd.exe` — which is exactly what makes this
/// fix work, but historically made argument escaping a real attack
/// surface (see CVE-2024-24576, fixed upstream in Rust 1.77.2). The only
/// argument passed here (`--stdio`) is a hardcoded literal, not
/// user-controlled, so there's nothing to inject today — but once the
/// server registry (§3) reads `args` from user-editable config, that
/// input should be treated as trusted-but-reviewable, the same trust
/// level as any other project-local config file that can run commands.
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

pub struct LspServerInstance {
    stdin: AsyncMutex<ChildStdin>,
    child: AsyncMutex<Child>,
    pending: StdMutex<HashMap<i64, PendingRequest>>,
    next_id: AtomicI64,
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
        self.pending.lock().unwrap().insert(id, PendingRequest { sender: tx });

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
                Err(format!("LSP request '{method}' timed out after {REQUEST_TIMEOUT:?}"))
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

#[derive(Default)]
pub struct LspManager {
    servers: AsyncMutex<HashMap<ServerKey, Arc<LspServerInstance>>>,
}

impl LspManager {
    /// Starts (or, if already running, reuses) the TypeScript language
    /// server for `root`. Hardcoded to `typescript-language-server
    /// --stdio` for now — Step 1 of the incremental build order is a
    /// single working server end-to-end; a pluggable registry (matching
    /// arbitrary languages to arbitrary commands) is the next increment.
    pub async fn start_typescript(&self, app: AppHandle, root: PathBuf) -> Result<(), String> {
        let key = ServerKey { root: root.clone(), language_id: "typescript".to_string() };

        if self.servers.lock().await.contains_key(&key) {
            return Ok(());
        }

        emit_status(&app, &key, ServerStatus::Starting);

        let program = resolve_program("typescript-language-server");
        let mut cmd = Command::new(&program);
        cmd.arg("--stdio");
        cmd.current_dir(&root);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit()); // surfaces server logs to the host console

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let message = format!(
                    "Failed to launch typescript-language-server (resolved to '{program}'): {e}. \
                     Is it installed and on PATH? (npm i -g typescript-language-server typescript)"
                );
                emit_status(&app, &key, ServerStatus::Crashed { message: message.clone() });
                return Err(message);
            }
        };

        let stdout = child.stdout.take().ok_or("Failed to capture server stdout")?;
        let stdin = child.stdin.take().ok_or("Failed to capture server stdin")?;

        // Surface this server in the Task Manager view alongside terminal
        // shells — register_process/unregister_process were built with
        // exactly this in mind (see their doc comments in
        // commands/process_monitor.rs). Called directly against
        // ProcessMonitorState rather than through the Tauri command
        // wrapper, since this isn't itself a command invocation.
        if let Some(pid) = child.id() {
            if let Some(monitor) = app.try_state::<crate::state::ProcessMonitorState>() {
                monitor
                    .extra
                    .lock()
                    .unwrap()
                    .insert(process_registry_id(&key), ("TypeScript language server".to_string(), pid));
            }
        }

        let instance = Arc::new(LspServerInstance {
            stdin: AsyncMutex::new(stdin),
            child: AsyncMutex::new(child),
            pending: StdMutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        });

        self.servers.lock().await.insert(key.clone(), instance.clone());

        // Reader task: owns stdout for the lifetime of the process, runs
        // on Tauri's async runtime (its own thread pool — never the
        // webview's JS main thread).
        let reader_app = app.clone();
        let reader_key = key.clone();
        let reader_instance = instance.clone();
        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader).await {
                    Ok(Some(text)) => route_message(&reader_app, &reader_key, &reader_instance, &text),
                    Ok(None) | Err(_) => break,
                }
            }
            if let Some(monitor) = reader_app.try_state::<crate::state::ProcessMonitorState>() {
                monitor.extra.lock().unwrap().remove(&process_registry_id(&reader_key));
            }
            emit_status(&reader_app, &reader_key, ServerStatus::Stopped);
        });

        // Handshake: initialize -> initialized.
        let root_uri = path_to_uri(&root)?;

        #[allow(deprecated)] // root_uri/root_path are deprecated in favor of
        // workspace_folders, but many servers (including older
        // typescript-language-server releases) still expect root_uri to
        // be set — sending both matches what mainstream LSP clients do.
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri.clone()),
            workspace_folders: Some(vec![WorkspaceFolder { uri: root_uri, name: "workspace".to_string() }]),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };

        let init_params = serde_json::to_value(&params).map_err(|e| e.to_string())?;

        match instance.request("initialize", init_params).await {
            Ok(_result) => {
                let initialized = serde_json::to_value(InitializedParams {}).map_err(|e| e.to_string())?;
                let _ = instance.notify("initialized", initialized).await;
                emit_status(&app, &key, ServerStatus::Running);
                Ok(())
            }
            Err(e) => {
                emit_status(&app, &key, ServerStatus::Crashed { message: e.clone() });
                self.servers.lock().await.remove(&key);
                let _ = instance.child.lock().await.start_kill();
                if let Some(monitor) = app.try_state::<crate::state::ProcessMonitorState>() {
                    monitor.extra.lock().unwrap().remove(&process_registry_id(&key));
                }
                Err(e)
            }
        }
    }

    pub async fn did_open(&self, root: &Path, language_id: &str, uri: &str, text: String) -> Result<(), String> {
        let instance = self.get(root, language_id).await?;
        let uri = parse_uri(uri)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(uri, language_id.to_string(), 1, text),
        };
        instance
            .notify("textDocument/didOpen", serde_json::to_value(&params).map_err(|e| e.to_string())?)
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
        let instance = self.get(root, language_id).await?;
        let uri = parse_uri(uri)?;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent { range: None, range_length: None, text }],
        };
        instance
            .notify("textDocument/didChange", serde_json::to_value(&params).map_err(|e| e.to_string())?)
            .await
    }

    pub async fn did_close(&self, root: &Path, language_id: &str, uri: &str) -> Result<(), String> {
        let instance = self.get(root, language_id).await?;
        let uri = parse_uri(uri)?;
        let params = DidCloseTextDocumentParams { text_document: TextDocumentIdentifier { uri } };
        instance
            .notify("textDocument/didClose", serde_json::to_value(&params).map_err(|e| e.to_string())?)
            .await
    }

    pub async fn stop(&self, app: &AppHandle, root: &Path, language_id: &str) -> Result<(), String> {
        let key = ServerKey { root: root.to_path_buf(), language_id: language_id.to_string() };
        if let Some(instance) = self.servers.lock().await.remove(&key) {
            let _ = instance.child.lock().await.start_kill();
        }
        if let Some(monitor) = app.try_state::<crate::state::ProcessMonitorState>() {
            monitor.extra.lock().unwrap().remove(&process_registry_id(&key));
        }
        Ok(())
    }

    async fn get(&self, root: &Path, language_id: &str) -> Result<Arc<LspServerInstance>, String> {
        let key = ServerKey { root: root.to_path_buf(), language_id: language_id.to_string() };
        self.servers
            .lock()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("No running LSP server for '{language_id}' at this root"))
    }
}

/// Dispatches one incoming JSON-RPC message: resolves a pending request if
/// the message carries a matching `id`, or forwards a recognized
/// notification (currently just diagnostics) to the frontend as a Tauri
/// event. Unrecognized notifications and server-to-client requests are
/// intentionally dropped for now — see AGENT.md §6 for the fuller event
/// surface planned for a later increment.
fn route_message(app: &AppHandle, key: &ServerKey, instance: &LspServerInstance, text: &str) {
    let Ok(value) = serde_json::from_str::<Value>(text) else { return };

    if let Some(id) = value.get("id").and_then(Value::as_i64) {
        // A response to one of our own requests (not a server-initiated
        // request — we don't send an "id" in requests *to* us to check
        // against, and this app doesn't yet handle server->client
        // requests like workspace/configuration).
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

    if let Some(method) = value.get("method").and_then(Value::as_str) {
        if method == "textDocument/publishDiagnostics" {
            if let Some(params) = value.get("params") {
                let _ = app.emit(
                    "lsp:diagnostics",
                    DiagnosticsEvent {
                        root: key.root.to_string_lossy().into_owned(),
                        language_id: key.language_id.clone(),
                        params: params.clone(),
                    },
                );
            }
        }
    }
}

/// Key used to register/unregister this server with `ProcessMonitorState`.
/// Doesn't yet incorporate `key.root` since Step 1 only ever has one
/// server running at a time — once multi-root support lands (§3), this
/// needs the root folded in too so two projects' TypeScript servers don't
/// collide under the same id.
fn process_registry_id(key: &ServerKey) -> String {
    format!("lsp:{}", key.language_id)
}

fn emit_status(app: &AppHandle, key: &ServerKey, status: ServerStatus) {
    let _ = app.emit(
        "lsp:status",
        StatusEvent { root: key.root.to_string_lossy().into_owned(), language_id: key.language_id.clone(), status },
    );
}

fn path_to_uri(path: &Path) -> Result<Uri, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let with_leading_slash = if normalized.starts_with('/') { normalized } else { format!("/{normalized}") };
    format!("file://{with_leading_slash}")
        .parse::<Uri>()
        .map_err(|e| format!("Invalid path for URI: {e}"))
}

fn parse_uri(s: &str) -> Result<Uri, String> {
    s.parse::<Uri>().map_err(|e| format!("Invalid URI '{s}': {e}"))
}
