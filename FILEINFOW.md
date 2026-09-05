# File Info — Session Changes

Scope: **Phase 2 — file tree + file operations** (this session only; Phase 0–1 files from the
previous session aren't relisted here).

## New files

### Backend (Rust)
| File | Purpose |
|---|---|
| `src-tauri/src/state.rs` | `AppState`: the open workspace root path + the live file watcher, shared across commands |
| `src-tauri/src/commands/fs.rs` | All filesystem commands: open workspace, lazy `read_dir`, create/rename/delete, copy/move with conflict detection + progress events, file watching, minimal text-file read |

### Frontend — types / api / utils
| File | Purpose |
|---|---|
| `src/lib/types/fs.ts` | `FsEntry` / `FsError` / `CopyProgress` — mirrors the Rust types |
| `src/lib/types/menu.ts` | Shared `MenuItem` type for the context menu |
| `src/lib/api/fs.ts` | Typed `invoke()` wrapper for every fs command (centralizes the snake_case→camelCase param mapping) |
| `src/lib/utils/validateName.ts` | Client-side filename validation (instant feedback; Rust side is the real boundary) |
| `src/lib/utils/fileIcons.ts` | Extension → accent color mapping for the file glyph |
| `src/lib/utils/path.ts` | Shared `baseName`/`parentOf` path-string helpers |

### Frontend — state
| File | Purpose |
|---|---|
| `src/lib/stores/workspace.svelte.ts` | File tree state: lazy children, expand/select, clipboard, create/rename/delete, copy/move + conflict resolution |
| `src/lib/stores/preview.svelte.ts` | Minimal single-file read-only preview (bridge until real tabs land) |
| `src/lib/stores/dnd.svelte.ts` | Transient in-tree drag state (dragging paths / drop target) |

### Frontend — components
| File | Purpose |
|---|---|
| `src/lib/components/shell/ContextMenu.svelte` | Generic positioned right-click menu |
| `src/lib/components/shell/ConflictDialog.svelte` | Overwrite-confirmation modal for copy/move/paste/drop conflicts |
| `src/lib/components/explorer/context.ts` | Svelte context contract shared down the recursive tree (drag start, open context menu) |
| `src/lib/components/explorer/NewEntryRow.svelte` | Inline "new file/folder" input row |
| `src/lib/components/explorer/FileTreeNode.svelte` | A single tree row — expand, select, inline rename, drag init, recurses into children |
| `src/lib/components/explorer/FileTree.svelte` | Tree root: toolbar, empty state, drag orchestration, OS drag & drop, watcher/progress event listeners |

## Modified files

| File | Change |
|---|---|
| `src-tauri/src/commands/mod.rs` | Registered the new `fs` module |
| `src-tauri/src/lib.rs` | Registered the dialog plugin, `AppState`, and all new fs commands |
| `src-tauri/Cargo.toml` | Added `tauri-plugin-dialog`, `notify-debouncer-mini`, `trash` |
| `src-tauri/capabilities/default.json` | Added `dialog:default` permission (folder picker) |
| `src-tauri/tauri.conf.json` | Explicitly set `dragDropEnabled: true` (needed for OS file drag & drop with real paths) |
| `package.json` | Added `@tauri-apps/plugin-dialog` |
| `src/lib/components/shell/Sidebar.svelte` | Renders `<FileTree />` in the Explorer view instead of the placeholder |
| `src/lib/components/shell/EditorArea.svelte` | Shows the read-only file preview (from `preview.svelte.ts`) alongside the welcome screen |
| `README.md` | Updated status, project structure, and design notes for Phase 2 |

## Notable in-session revision

`src-tauri/src/commands/fs.rs` was rewritten partway through this session to use `Box<str>` /
`Box<[T]>` instead of `String`/`Vec<T>` for data that's constructed once and returned/serialized
(`FsEntry`, `FsError`, and every command's return collection) — per your request to apply that
optimization where it genuinely helps. Command *parameters* were deliberately left as
`String`/`Vec<String>` since they're short-lived per-call inputs; see the comment block at the
top of `fs.rs` for the full reasoning.

## Not verified by compiling

I don't have a Rust toolchain in this sandbox (no `cargo`/`rustc`, and the network here can't reach
`rustup.rs` to install one), so `src-tauri/src/**` was reviewed carefully by hand and cross-checked
against current crate docs, but not actually built. The frontend **was** fully verified: `svelte-check`
(0 errors/warnings) and a full `vite build` both pass. Please run `npm run tauri dev` as the real check
on the Rust side.