# Lightweight Code Editor — Implementation Plan
### Tauri + Rust + CodeMirror 6, native-plugin extension system, Windows-first

Based on your answers: **native Rust plugins (cdylib/dlopen)** for extensions, **Windows-first**, and a **generic user-configured LSP client**. This plan is built around those three decisions.

---

## 1. Tech Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri v2 | Uses OS WebView (WebView2 on Windows) — no bundled Chromium, ~100–150MB baseline vs Electron's 300–500MB |
| Backend | Rust (tokio async runtime) | Required for Tauri; async for fs, PTY, LSP process I/O |
| Frontend framework | Svelte (or SolidJS) | Compiles away, no virtual-DOM overhead — fits the "lightweight" goal better than React |
| Editor | CodeMirror 6 | Modular/tree-shakeable, no web-worker-heavy Monaco runtime, good large-file/virtualized rendering |
| Terminal renderer | xterm.js | Standard, not Monaco, pairs with a Rust PTY backend |
| PTY backend | `portable-pty` (wezterm) | Wraps Windows ConPTY properly; swap-in ready for macOS/Linux later |
| File watching | `notify` | Cross-platform fs events, needed for tree refresh + Quick Open index invalidation |
| Fuzzy matching | `nucleo-matcher` (Helix's matcher) | Fast Rust-side fuzzy search for Quick Open / Command Palette, keeps large-repo indexing off the JS thread |
| Native plugin loading | `libloading` + `abi_stable` | See §6 — this pairing is what makes native dlopen plugins survive across compiler versions |
| Safe delete | `trash` crate | Recycle-bin-safe deletes instead of permanent `fs::remove` |
| Git (phase 2+) | `git2` | Status badges in the tree, optional |

---

## 2. High-Level Architecture

```
┌─────────────────────────────── WebView (Frontend) ───────────────────────────────┐
│  Shell UI (Svelte): Activity Bar · Sidebar · Tab Bar · Status Bar · Panel        │
│  ├─ FileTree (virtualized, lazy-loaded)                                          │
│  ├─ EditorFile host (per-tab: CodeMirror | <img> | <audio> | hex/binary view)    │
│  ├─ Quick Open (Ctrl+P) + Command Palette (fuzzy query → Tauri command)          │
│  ├─ Terminal panel (xterm.js)                                                    │
│  └─ CodeMirror LSP transport → invoke()/listen() instead of WebSocket            │
└───────────────────────────────────┬───────────────────────────────────────────────┘
                                     │ Tauri IPC (commands + events)
┌────────────────────────────────────▼──────────────────────────────────────────────┐
│  Rust Core (Tauri backend)                                                        │
│  ├─ fs module: read/write/copy/move/rename/delete (streaming for large files)     │
│  ├─ watcher: notify → debounced tree-diff events → frontend                       │
│  ├─ lsp module: spawns servers from settings.json, stdio ⇄ JSON-RPC ⇄ IPC bridge  │
│  ├─ pty module: portable-pty session per terminal tab                             │
│  ├─ search module: nucleo-matcher over cached workspace file index                │
│  ├─ command registry: id → handler, contributed by core AND by plugins           │
│  └─ plugin host: libloading + abi_stable vtable, Host API exposed to .dll files   │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. File Tree, File Browser & File Operations

- **Lazy tree**: read one directory level at a time (`read_dir` on expand), not a full recursive walk on workspace open — critical for memory/startup time on large repos.
- **Live updates**: `notify` watcher on the open workspace root(s), debounced (~150ms) and diffed before emitting `tree:changed` events, so a `git checkout` doesn't spam the frontend.
- **Clipboard model**: an explicit `ClipboardState { paths: Vec<PathBuf>, mode: Copy | Cut }` held in Rust state. Paste triggers `fs_copy` or `fs_move`; Cut visually dims the source item until paste/cancel (like VS Code).
- **Drag & drop**: two paths —
  - *In-tree* (reorder/move): HTML5 drag events in the tree view, drop handler calls `move_path` command.
  - *OS → app* (dragging files from Explorer into the window): Tauri's `onDragDropEvent`, resolved to absolute paths, then either "open" (if dropped on editor area) or "copy into folder" (if dropped on tree node).
- **Conflict handling**: copy/move commands return a `Conflict` variant when destination exists; frontend shows overwrite/skip/rename-keep-both dialog rather than the backend silently deciding.
- **Large copies**: stream + emit `progress` events (`bytes_done/total`) so big folder copies don't block or hang the UI.

## 4. EditorFile Abstraction

A tab isn't "a CodeMirror instance" — it's a generic container that picks a viewer by resolved MIME/extension:

```
EditorFile
 ├─ resolves file type (extension map + magic-byte sniff for extensionless files)
 ├─ TextViewer   → CodeMirror 6 (language pack + LSP client attached)
 ├─ ImageViewer  → <img> with zoom/pan, works for png/jpg/gif/svg/webp
 ├─ AudioViewer  → <audio> with custom transport controls
 ├─ BinaryViewer → hex dump (virtualized, only renders visible rows)
 └─ Unknown      → "no preview" placeholder + "open with system default" button
```

Registering this as a small **viewer registry** (`registerViewer(matcher, factory)`) rather than a hardcoded switch means the extension system (§6) can add new viewer types later without touching core.

## 5. Quick Open (Ctrl+P) & Command Palette

- On workspace open, build a flat file-path index in Rust (respecting `.gitignore`), kept warm and updated by the `notify` watcher — avoids re-walking the tree on every keystroke.
- Query flow: keystroke → debounced `invoke('quick_open_query', q)` → `nucleo_matcher` scores the index → top N results returned → frontend renders.
- Command Palette uses the same fuzzy component but matches against a **command registry**: `{ id, title, keybinding?, handler }`. Core commands and plugin-contributed commands live in the same registry, so plugins show up in the palette for free.

## 6. Generic LSP Client

Since it's stdio-based and the frontend can't spawn processes, Rust brokers everything:

1. `settings.json` maps language IDs to server launch config, e.g.:
   ```jsonc
   "lsp.servers": {
     "rust": { "command": "rust-analyzer", "args": [] },
     "python": { "command": "pyright-langserver", "args": ["--stdio"] },
     "typescript": { "command": "typescript-language-server", "args": ["--stdio"] }
   }
   ```
2. On first file of a given language opening, Rust spawns the configured server (idle-timeout shutdown if unused — keeps memory down), pipes stdin/stdout as JSON-RPC.
3. Rust re-emits inbound LSP messages as Tauri events (`lsp:message:<server_id>`) and accepts outbound ones via an `invoke('lsp_send', ...)` command.
4. On the frontend, write a thin custom `Transport` for `codemirror-languageserver` (or wire `@codemirror/lint` / `@codemirror/autocomplete` manually) that implements `send`/`subscribe` over `invoke`/`listen` instead of the WebSocket transport those libraries assume by default. This is the one non-trivial integration piece — budget real time for it.
5. Diagnostics, hover, completion, go-to-definition, and rename are the v1 feature set; formatting and code actions are natural v2 additions once the transport is solid.

## 7. Integrated Terminal

- One `portable-pty` session per terminal tab, spawned with the user's default shell (`%COMSPEC%`/PowerShell on Windows).
- Rust reads PTY output in a loop, emits chunks as `pty:data:<id>` events; frontend writes them straight into xterm.js.
- Input direction: xterm.js `onData` → `invoke('pty_write', id, data)`.
- Resize: xterm.js `onResize` → `invoke('pty_resize', id, cols, rows)`.

## 8. Extension System — Native Rust Plugins (cdylib/dlopen)

You picked the fastest/lightest option — worth being upfront about its real tradeoff before the design: **plain Rust has no stable ABI across compiler versions/editions**, so a naive `dlopen` + raw trait object *will* segfault if a plugin is built with a different rustc than the host. Two ways to make this safe enough to ship:

- **Recommended**: use the **`abi_stable`** crate. It gives you `#[repr(C)]`-safe, versioned trait objects and vtables specifically so native plugins can be loaded across compiler versions without matching the host's exact toolchain.
- **Leaner alternative**: hand-roll a minimal C ABI — `extern "C"` entry point returning a `#[repr(C)]` struct of raw function pointers — more boilerplate, zero extra dependency.

Either way, the host must **version-check before trusting the pointer** (a magic number + ABI version in the plugin's exported struct), and a bad extension **can crash the whole process** since it runs in-process with full privileges — there's no sandbox here by design, given the "fastest/lightest" choice. Worth documenting clearly for anyone installing third-party extensions later; an out-of-process (IPC-based) plugin host is the natural escape hatch if a marketplace with untrusted authors becomes a goal.

**Plugin lifecycle**
1. Extensions live in an `extensions/` folder, each a folder with `extension.toml` (id, name, version, entry `.dll` path, `contributes: { commands, viewers, languages, statusBarItems }`) and the compiled `.dll`.
2. On startup, host scans the folder, `libloading::Library::new()`s each `.dll`, looks up a well-known symbol (e.g. `_plugin_entry`), version-checks, then calls `init(host_api: &HostApi)`.
3. `HostApi` (v1) exposes: `register_command`, `contribute_status_bar_item`, `subscribe(event)` (file opened/saved, workspace changed), `read_file`/`write_file`, `show_notification`.
4. **UI from native code**: plugins don't touch the DOM directly. Two supported contribution shapes: (a) structured data the shell already knows how to render (status bar text, tree data provider returning a JSON node tree, quick-pick items), or (b) a "webview panel" where the plugin hands the host a static HTML/CSS/JS string to mount in an iframe — same trick VS Code uses for webview extensions, and it keeps native code from ever needing DOM access.
5. Ship a `cargo generate` template pinned to the exact rustc/crate versions the host was built with, so extension authors get a working `.dll` on the first try.

## 9. Memory/Performance Discipline (cross-cutting, not a phase)

- OS webview instead of bundled Chromium (biggest single win).
- Lazy tree expansion + virtualized lists everywhere (tree, Quick Open results, hex viewer, xterm scrollback).
- Idle-timeout LSP servers; only spawn per-language on first use.
- Stream large file reads/copies instead of loading whole buffers.
- Debounced file-watcher → diffed events, not raw event floods.
- Rust for anything CPU-heavy (fuzzy search, diffing) so the JS thread stays free.

---

## 10. Phased Roadmap

| Phase | Deliverable |
|---|---|
| 0 | Tauri v2 + Svelte scaffold, window chrome, build pipeline for Windows |
| 1 | Shell layout: activity bar, sidebar, tab bar, status bar, panel region |
| 2 | Filesystem module + file tree (lazy load, watcher) + copy/cut/paste/move/drag-drop + safe delete |
| 3 | EditorFile abstraction, CodeMirror integration, multi-tab, Image/Audio/Binary viewers |
| 4 | Quick Open (Ctrl+P) + Command Palette + keybinding system + command registry |
| 5 | Integrated terminal (portable-pty + xterm.js, multi-session) |
| 6 | Generic LSP client: settings.json config, stdio↔IPC bridge, custom Transport, diagnostics/hover/completion/go-to-def |
| 7 | Native plugin host: `abi_stable` vtable, `libloading` loader, extension.toml manifest, Host API v1, example extension + `cargo generate` template |
| 8 | Settings system (settings.json + settings UI), themes, custom keybindings |
| 9 | Perf/memory pass, Windows installer (MSI/NSIS via Tauri bundler), crash reporting opt-in |
| 10+ (stretch) | macOS/Linux port, git status in tree, DAP debugging, extension marketplace/registry, out-of-process plugin sandbox |

---

## 11. Immediate Next Step

I'd suggest starting at **Phase 0–1**: a working Tauri+Svelte shell with the layout skeleton and one CodeMirror tab wired up, before touching LSP or plugins — everything else in this plan hangs off that shell. I'm happy to scaffold that project structure and starter code next if useful.
