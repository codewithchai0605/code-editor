# LSP Integration — Agent Guide

This document describes how to design and build Language Server Protocol (LSP) support
for the editor: a Rust-owned, multi-instance LSP manager capable of running several
language servers concurrently across a multi-root workspace (e.g. a Node.js backend and
a Flutter app living in sibling subfolders), with manual start/stop/restart control and
VS Code–equivalent editing features.

Decisions this doc assumes (confirmed for this project):

- **Rust owns all LSP protocol logic** — JSON-RPC framing, request/response correlation,
  notification dispatch. The TS frontend never parses raw LSP wire messages; it only
  calls Tauri commands and listens to typed Tauri events.
- **Broad, pluggable language support** (5+ languages out of the box, more addable via config).
- **Project/language detection is automatic**, based on marker files (`package.json`,
  `pubspec.yaml`, `Cargo.toml`, etc.) discovered by scanning the opened folder.

---

## Memory Conventions

This codebase favors `Box<[T]>` over `Vec<T>`, and `Box<str>`/`Box<Path>` over
`String`/`PathBuf`, for anything stored in a struct field or sent back across the Tauri
boundary — a growable container carries capacity/growth bookkeeping that's pure overhead
once nothing is being pushed to it anymore (`Vec` is a 3-word fat pointer, `Box<[T]>` is 2;
same story for `String` vs `Box<str>`). The convention used throughout this doc:

- **Build with `Vec`/`String`, freeze with `.into_boxed_slice()` / `.into_boxed_str()`**
  the moment a value is done growing and is about to be stored or returned.
- **Keep `Vec`/`HashMap`/`String` where something is genuinely mutated in place** across
  its lifetime — e.g. the pending-request map in §5.2, which inserts and removes
  constantly. Freezing something that's rebuilt every few milliseconds adds churn instead
  of removing it.
- **External crate types keep their own containers.** `lsp-types` structs (`Hover`,
  `CompletionList`, `Location`, `WorkspaceEdit`, …) use `String`/`Vec` internally because
  that's how the crate models the LSP spec — there's no value in re-implementing the
  whole protocol's types just to swap container kinds, so those pass through unchanged.
- `serde` implements `Serialize`/`Deserialize` for `Box<[T]>` and `Box<str>` directly, so
  this is a transparent swap across the Tauri IPC boundary — the frontend still just sees
  ordinary JSON arrays and strings.

---

## 1. Architecture Overview

```
┌─────────────────────────────┐        Tauri commands (invoke)        ┌───────────────────────────┐
│   Frontend (TS + CodeMirror) │ ─────────────────────────────────────▶│      Rust backend         │
│                              │                                        │                            │
│  - CodeMirror LSP extensions │        Tauri events (emit)            │  LspManager                │
│  - Diagnostics rendering     │◀───────────────────────────────────── │   ├─ ProjectScanner        │
│  - Completion / hover UI     │                                        │   ├─ ServerRegistry        │
│  - Status bar (server state) │                                        │   └─ LspServerInstance[]   │
└─────────────────────────────┘                                        │        │                    │
                                                                        │        ▼                    │
                                                                        │  child process (stdio)      │
                                                                        │  typescript-language-server │
                                                                        │  dart language server       │
                                                                        │  rust-analyzer, pyright, …  │
                                                                        └───────────────────────────┘
```

Key point: an `LspServerInstance` is **per (project root, language)**, not global. Opening
a workspace with a Node backend and a Flutter app produces two independent server
instances, each with its own process, its own `rootUri`, its own request-id space, and
its own lifecycle.

---

## 2. Core Concepts

- **Project root**: a directory containing a marker file that identifies a toolchain
  (see §3). Each root gets at most one server instance per language.
- **Server instance**: a spawned child process speaking LSP over stdio, wrapped in a
  Rust struct that owns the pipes, the pending-request table, and the current status.
- **Document routing**: every open file is mapped to exactly one server instance by
  walking up from the file's path to the nearest matching project root.
- **Capabilities negotiation**: on `initialize`, the client advertises the subset of LSP
  capabilities the editor actually implements (see §7's feature table) and stores the
  server's returned capabilities to gate which features are offered per-server (e.g.
  don't show "rename" if the server didn't advertise `renameProvider`).

---

## 3. Project / Language Detection

Scan the opened folder for marker files, up to a configurable depth (default: a few
levels, or stop descending once a root is found in a branch — don't keep scanning inside
a detected project's own subfolders unless it's a known monorepo pattern).

| Marker file(s)                             | Language / toolchain  | Suggested server                              |
| ------------------------------------------ | --------------------- | --------------------------------------------- |
| `package.json` (+ `tsconfig.json`)         | TypeScript/JavaScript | `typescript-language-server`                  |
| `pubspec.yaml`                             | Dart / Flutter        | Dart Analysis Server (`dart language-server`) |
| `Cargo.toml`                               | Rust                  | `rust-analyzer`                               |
| `pyproject.toml` / `requirements.txt`      | Python                | `pyright` (or `pylsp`)                        |
| `go.mod`                                   | Go                    | `gopls`                                       |
| `CMakeLists.txt` / `compile_commands.json` | C/C++                 | `clangd`                                      |
| `composer.json`                            | PHP                   | `intelephense`                                |

Notes:

- A single opened folder can contain **multiple roots** at different subpaths — this is
  exactly the Node + Flutter case. The scanner should return a `Vec<ProjectRoot>`, not a
  single root.
- Handle nested roots (monorepos) by picking the **closest ancestor root** for a given
  file, same resolution strategy VS Code and rust-analyzer use.
- Detection should be config-overridable: let the user pin a root manually or exclude a
  detected one (e.g. a `node_modules` package.json that shouldn't spawn its own server).
- Re-run detection (or at least re-check the affected subtree) when `fs_watcher` reports
  a new marker file being created, since you already have that watcher.

---

## 4. Server Registry (pluggable config)

Keep server definitions as data, not hardcoded Rust, so users can add servers without a
rebuild. Suggested shape (JSON or TOML), loaded at startup and mergeable with user config:

```json
{
    "typescript": {
        "command": "typescript-language-server",
        "args": ["--stdio"],
        "rootMarkers": ["package.json", "tsconfig.json"],
        "fileExtensions": ["ts", "tsx", "js", "jsx"],
        "initializationOptions": {}
    },
    "dart": {
        "command": "dart",
        "args": ["language-server", "--protocol=lsp"],
        "rootMarkers": ["pubspec.yaml"],
        "fileExtensions": ["dart"]
    },
    "rust": {
        "command": "rust-analyzer",
        "args": [],
        "rootMarkers": ["Cargo.toml"],
        "fileExtensions": ["rs"]
    }
}
```

The `ServerRegistry` maps `language_id -> ServerConfig`. The `ProjectScanner` maps
`root_path -> language_id` (via `rootMarkers`). Together they tell the `LspManager` what
to spawn and where.

---

## 5. Rust-side LSP Manager

### 5.1 Suggested crates

- `tokio` — async process I/O, tasks
- `serde` / `serde_json` — message (de)serialization
- `lsp-types` — gives you strongly-typed Rust structs for the entire LSP spec (requests,
  notifications, capabilities). Strongly recommended over hand-rolling JSON shapes.

### 5.2 Core structs (sketch)

```rust
pub struct ServerKey {
    root: Box<Path>,
    language: Box<str>,
}

pub struct LspManager {
    servers: HashMap<ServerKey, LspServerInstance>,
    registry: ServerRegistry,
}

pub struct LspServerInstance {
    key: ServerKey,
    child: tokio::process::Child,
    stdin: ChildStdin,
    // Inserted on every outgoing request, removed on the matching response — genuinely
    // mutated in place, so this stays a HashMap rather than freezing to a boxed slice.
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    next_id: AtomicI64,
    status: watch::Sender<ServerStatus>,
    capabilities: Option<ServerCapabilities>, // lsp-types crate type, left as defined
}

pub enum ServerStatus {
    Starting,
    Running,
    Crashed { message: Box<str> },
    Stopped,
}
```

### 5.3 Message framing

LSP over stdio uses `Content-Length`-prefixed JSON, not newline-delimited JSON:

```
Content-Length: 123\r\n
\r\n
{"jsonrpc":"2.0", ...}
```

Spawn a reader task per server that:

1. Reads headers until the blank line, parses `Content-Length`.
2. Reads exactly that many bytes, parses as JSON.
3. If it has an `id` matching a pending request → resolve the stored `oneshot::Sender`.
4. If it's a notification (`method` present, no matching pending id) → route to the
   frontend via a Tauri event (diagnostics, log messages, `$/progress`, etc.).

Writing works the same way in reverse: serialize the JSON body, prefix it with the
correct `Content-Length` header, write to `stdin`.

### 5.4 Request/response correlation

- Maintain an atomic counter for request ids.
- Store a `oneshot::Sender<Value>` per outstanding id in a shared map.
- `send_request()` returns a future that awaits the corresponding `oneshot::Receiver`,
  with a timeout (e.g. 10–30s) that removes the pending entry and errors out on expiry.

### 5.5 Lifecycle

- `start(root, language)` — look up config, spawn child, send `initialize` +
  `initialized`, mark `Running`, replay any buffered `didOpen` for files already open
  under that root.
- `stop(server_id)` — send `shutdown` request, then `exit` notification, kill the process
  if it doesn't exit within a grace period.
- `restart(server_id)` — `stop()` then `start()` with the same root/language, and replay
  `didOpen` for currently open documents under that root so the new instance isn't blind.
- Crash detection: watch the child's exit status; on unexpected exit, set
  `ServerStatus::Crashed`, emit an event so the UI can show a "restart" affordance
  (optionally auto-restart with capped exponential backoff — see §9).

---

## 6. Tauri Command / Event Surface

### Commands (frontend → backend)

Paths and identifiers arrive from the frontend as JSON strings, so they land as `Box<str>`
at the command boundary; convert to `Box<Path>` once past it and storing/looking up by
path internally.

- `lsp_start(root: Box<str>, language: Box<str>)`
- `lsp_stop(server_id: Box<str>)`
- `lsp_restart(server_id: Box<str>)`
- `lsp_list_servers() -> Box<[ServerInfo]>`
- `lsp_did_open(file: Box<str>, text: Box<str>)`
- `lsp_did_change(file: Box<str>, changes: Box<[TextChange]>)`
- `lsp_did_close(file: Box<str>)`
- `lsp_did_save(file: Box<str>)`
- `lsp_completion(file: Box<str>, position: Position) -> CompletionList`
- `lsp_hover(file: Box<str>, position: Position) -> Option<Hover>`
- `lsp_definition(file: Box<str>, position: Position) -> Box<[Location]>`
- `lsp_references(file: Box<str>, position: Position) -> Box<[Location]>`
- `lsp_rename(file: Box<str>, position: Position, new_name: Box<str>) -> WorkspaceEdit`
- `lsp_code_action(file: Box<str>, range: Range) -> Box<[CodeAction]>`
- `lsp_signature_help(file: Box<str>, position: Position) -> Option<SignatureHelp>`
- `lsp_document_symbols(file: Box<str>) -> Box<[DocumentSymbol]>`
- `lsp_workspace_symbol(query: Box<str>) -> Box<[SymbolInformation]>`
- `lsp_formatting(file: Box<str>) -> Box<[TextEdit]>`

(`CompletionList`, `Hover`, `WorkspaceEdit`, `SignatureHelp` are `lsp-types` structs and
keep that crate's own `String`/`Vec` fields internally — see Memory Conventions above.)

### Events (backend → frontend)

- `lsp://diagnostics` — `{ file, diagnostics: [...] }`
- `lsp://server-status-changed` — `{ server_id, status }`
- `lsp://progress` — `$/progress` passthrough (rust-analyzer indexing, etc.)
- `lsp://log` — server stderr / protocol trace lines, for a debug output panel

Each request-style command internally does: resolve the right `server_id` for the file
→ build the LSP request via `lsp-types` → `send_request()` → map the typed result back
to the frontend.

---

## 7. Feature Mapping (VS Code parity)

| LSP capability                    | Editor feature                                 | CodeMirror integration point             |
| --------------------------------- | ---------------------------------------------- | ---------------------------------------- |
| `textDocument/completion`         | Autocomplete                                   | `autocompletion()` source                |
| `textDocument/hover`              | Hover tooltips                                 | `hoverTooltip()`                         |
| `textDocument/publishDiagnostics` | Red/yellow underlines, problems panel          | `linter()` / diagnostic decorations      |
| `textDocument/definition`         | Go to Definition (Ctrl/Cmd+Click, F12)         | click handler + jump-to-location         |
| `textDocument/references`         | Find All References panel                      | custom panel, reuse location list        |
| `textDocument/rename`             | Rename Symbol (F2)                             | inline rename UI + apply `WorkspaceEdit` |
| `textDocument/codeAction`         | Lightbulb / quick fixes                        | gutter icon + menu                       |
| `textDocument/documentSymbol`     | Outline view, breadcrumbs                      | sidebar tree / breadcrumb bar            |
| `workspace/symbol`                | Go to Symbol in Workspace (Ctrl+T)             | command palette / quick-open             |
| `textDocument/signatureHelp`      | Parameter hints while typing                   | tooltip near cursor                      |
| `textDocument/formatting`         | Format Document / Format Selection             | command + apply `TextEdit[]`             |
| `textDocument/semanticTokens`     | Enhanced syntax highlighting (optional, later) | CodeMirror decoration layer              |

Only expose a feature in the UI if the connected server's `ServerCapabilities` actually
advertised it — this avoids showing "Rename" for a server that doesn't support it.

---

## 8. Document Synchronization

- On tab open → `lsp_did_open` with full text and `languageId`.
- On edit → debounce (~150–300ms) then `lsp_did_change`. Prefer **incremental sync**
  (`TextDocumentSyncKind.Incremental`, sending only the changed ranges) if the server
  supports it; fall back to full-document sync otherwise.
- Track a per-document `version` integer, incremented on every change, sent with every
  `didChange`.
- On tab close → `lsp_did_close`.
- On save → `lsp_did_save` if the server registered `save` sync options.

---

## 9. Multi-Root Walkthrough (Node backend + Flutter app)

1. User opens folder `/project` containing `/project/backend` (has `package.json`) and
   `/project/app` (has `pubspec.yaml`).
2. `ProjectScanner` walks the tree, finds both marker files, returns:
   `[{root: "/project/backend", language: "typescript"}, {root: "/project/app", language: "dart"}]`.
3. Depending on config, either:
    - **Eager mode**: `LspManager` starts both servers immediately, or
    - **Lazy mode** (recommended default): nothing starts yet.
4. User opens `backend/src/index.ts` → router walks up from the file path, finds root
   `/project/backend` registered for `typescript` → if not already running (lazy mode),
   `lsp_start("/project/backend", "typescript")` is called automatically, then
   `lsp_did_open` is sent.
5. User opens `app/lib/main.dart` → same process resolves root `/project/app`,
   language `dart`, and starts/uses the Dart server — a completely separate process and
   instance from the TypeScript one.
6. Diagnostics events carry `server_id` (or the root path) alongside the file, so the
   frontend never confuses which server produced which diagnostics.

---

## 10. Manual Controls

Surface a status-bar item (or sidebar panel) per **active** server instance, similar to
VS Code's language status item:

- Label: language + root (e.g. "TypeScript — backend").
- State indicator: Starting / Running / Crashed / Stopped.
- Actions: Restart, Stop, Show Output/Logs.
- A command-palette entry "LSP: Restart Server" / "LSP: Stop All Servers" for the power-user path.

These map directly onto `lsp_stop` / `lsp_restart` / `lsp_list_servers`.

---

## 11. Error Handling & Resilience

- **Crash recovery**: on unexpected exit, emit `server-status-changed: Crashed`. Optionally
  auto-restart with exponential backoff (e.g. 1s, 2s, 4s… capped, and capped at N attempts
  before giving up and requiring a manual restart) so a persistently broken server doesn't
  spin forever.
- **Request timeouts**: reject pending requests after a timeout so the frontend doesn't
  hang forever on e.g. a `completion` call to a wedged server.
- **Stderr capture**: pipe each server's stderr into a ring buffer, exposed via
  `lsp://log` events and an "LSP Output" panel — invaluable for debugging server startup
  failures (wrong binary path, missing dependency, etc.).
- **Missing binary**: if the configured command isn't found on the system, fail fast with
  a clear error surfaced to the user (don't retry-loop a `command not found`).

---

## 12. Performance Considerations

- **Lazy start** (default): only spawn a server when the first matching file is opened,
  not eagerly for every detected root — avoids spinning up a Dart server nobody uses in a
  session.
- **Idle shutdown** (optional): stop a server after N minutes with no open documents
  under its root, to save memory on large multi-root workspaces.
- **Debounced didChange**: avoid flooding the server with a notification per keystroke.
- **Backpressure**: if a server falls behind (e.g. large file diagnostics), don't block
  the UI thread — all of this should already be async given tokio, but double-check the
  frontend await points don't serialize on a slow server.

---

## 13. Suggested Build Order

1. **Single-server spine**: hardcode one language (e.g. TypeScript), get spawn → framing
   → `initialize` → `didOpen` → `publishDiagnostics` working end-to-end into the UI.
2. **Generalize to registry**: move server definitions into config, support swapping
   languages without new Rust code.
3. **Multi-root detection + routing**: implement `ProjectScanner`, file→server resolution,
   and prove the Node+Flutter case works with two concurrent instances.
4. **Full feature set**: completion, hover, go-to-definition, references, rename, code
   actions, formatting, document/workspace symbols.
5. **Manual controls + resilience**: status bar, restart/stop UI, crash recovery, logs panel.
6. **Performance pass**: lazy start, idle shutdown, incremental sync tuning.

---

## 14. Testing Strategy

- Unit-test the JSON-RPC framing/parsing in isolation (feed it byte chunks, including
  split-across-reads edge cases, and assert correct message boundaries).
- Spin up a trivial mock LSP server (a small script that replies to `initialize` and
  echoes diagnostics) to test `LspManager` lifecycle without depending on real,
  slow-starting servers like rust-analyzer.
- Manual test matrix: single TS project; single Dart project; the mixed
  backend+app workspace; killing a server process externally (crash path); opening a
  huge file; a file with hundreds of diagnostics; restart under load.
