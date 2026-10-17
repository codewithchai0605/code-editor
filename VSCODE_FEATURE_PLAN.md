# VS Code Feature Parity — Plan & Tasks

## Phase 0 — Foundation (already landed, verified)
- [x] Workspace search: `lineTextOffset` windowed-line column fix (backend + frontend)
- [x] Reveal flash highlight on navigation
- [x] LSP root detection restored (`startForWorkspace`)
- [x] Rich `client_capabilities` (utf-16 positions, diagnostics, workspace folders)
- [x] Lightweight-LSP gate for standalone html/css/json

## Phase 1 — Navigation that earns its keep
- [ ] **Go to References** (Shift+Alt+F12): backend already exposes
      `textDocument/references`; add a peek-style panel listing every hit,
      click-to-reveal via the existing `openAndReveal`.
- [ ] **Breadcrumbs → symbol picker**: clicking the file crumb opens the
      existing document-symbol palette (Ctrl+Shift+O equivalent).
- [ ] **Auto-reveal in Explorer**: opening/activating a tab expands and
      selects its folder path in the tree (VS Code's default behavior).
- [ ] **Command palette polish**: recent commands pinned to top.

## Phase 2 — Editor muscle memory
- [ ] **Toggle comment** (`Mod-/`) per language via `@codemirror/language`'s
      `commentBlock`/`comment` commands.
- [ ] **Select all occurrences** (`Mod-Shift-L`).
- [ ] **Delete line** (`Mod-Shift-K`), **join lines** (`Mod-j`),
      **select line** (`Mod-l`).
- [ ] **Fold all / unfold all / fold region** commands + palette entries.
- [ ] **Go to Line/Column** (`Ctrl+G`) — reuse SymbolPalette pattern.

## Phase 3 — Status bar becomes useful
- [ ] **Cursor position chip** (Ln, Col) — live from the active view.
- [ ] **Selection count chip** ("N selected") when multiple cursors/selection.
- [ ] **Problems chip** (errors/warnings of active file) — opens Problems panel.
- [ ] **Indentation chip** (spaces/tabs size) — clickable to change.
- [ ] **Real encoding/EOL chips** derived from the active buffer.

## Phase 4 — Problems panel
- [ ] Bottom-panel "Problems" tab aggregating the diagnostics cache across
      open files; click row → reveal (reuses openAndReveal + flash).

## Phase 5 — Search power-ups
- [ ] Replace-all in workspace with preview count (backend replace exists).
- [ ] Preserve case on replace (frontend transform).
- [ ] Results limit indicator + "show more" per file (backend truncation flag
      already surfaced).

## Verification
- `cargo check` after every Rust change; `npm run build` after TS/Svelte.
