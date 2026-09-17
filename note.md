# LSP integration — what's here

34 files, all under the same relative paths they live at in your project
(e.g. `src-tauri/src/lsp/manager.rs` → drop it in at
`your-project/src-tauri/src/lsp/manager.rs`). Everything below "already
completed" in your `CLAUDE.md` is finished: registry-driven multi-language
spawning, multi-root project detection, the full feature set (completion,
hover, go-to-definition, references, rename, code actions, signature help,
formatting, document/workspace symbols), manual controls (status bar,
restart/stop, output panel), crash recovery with backoff, and — from your
follow-up — TypeScript 7 (`tsgo`) support with per-folder overrides.

**New files** are new modules. **Everything else listed** is an edit to an
existing file (a diff, not a rewrite of unrelated parts of it) — the two
big rewrites are `src-tauri/src/lsp/manager.rs` and
`src/lib/lsp/manager.svelte.ts`, since those are where almost everything
above actually lives.

## Verification

I don't have your exact toolchain here (no `rustup`, and your project's
`typescript@7` blocks `svelte-check` without the dual-install workaround —
see below), so I built two throwaway harnesses to check this for real
rather than by eye:

- **Rust**: a mock crate standing in for `tauri` (same call patterns your
  existing Step 1 code already used — `app.emit`, `app.try_state`,
  `tauri::async_runtime::spawn`), compiled against the *real* `lsp-types`,
  `tokio`, and `ignore` crates. Caught two real bugs before you'd have hit
  them: a borrow-checker issue in `get_logs`, and — more importantly —
  `project_scanner.rs` silently detecting *zero* project roots for the
  single-root case (opening a folder that directly contains `Cargo.toml`),
  because `ignore::WalkBuilder`'s `filter_entry` is never invoked for the
  walk's own root. Both fixed; 13 Rust unit tests pass, plus an end-to-end
  test that spawns a real fake LSP process and exercises the full
  handshake → completion → graceful stop → simulated crash → auto-restart
  → TS-variant-override lifecycle.
- **Frontend**: `npm install`, `svelte-check` (0 errors, 0 warnings), and a
  full `npm run build` — all clean.

I can't run the actual `typescript-language-server`/`rust-analyzer`/etc.
processes or click through the UI here, so give the interactive bits
(hover tooltip positioning, the rename input, the code-action menu) a real
try — they're the parts most likely to want a small polish pass once
you're looking at them on screen.

## Your TypeScript 7 setup

Your `package.json` already has `"typescript": "7"` — this is exactly the
case the new `typescript_version.rs` module handles. On first open, a
project with TS 7 installed auto-routes to a `native` variant
(`tsgo --lsp --stdio`); TS ≤6 gets the classic `typescript-language-server`.
Practically:

1. Install `tsgo`: `npm install --save-dev @typescript/native-preview`
   (ships the `tsgo` binary). If you're later on a TS 7.0 RC/stable release
   where the binary is renamed to plain `tsc`, just edit
   `.codeeditor/lsp-servers.json` → `servers.typescript.variants.native.command`
   to `"tsc"` — no rebuild needed.
2. Everything else is automatic. To override the auto-pick for a specific
   folder ("use the fast one even on a TS5 project"), or workspace-wide,
   use the status bar's server popover (click the "LSP" chip → the
   dropdown next to that server) — it writes to
   `.codeeditor/lsp-servers.json` and restarts safely (graceful shutdown,
   replays whatever files you had open).
3. **Known gap, not a bug**: `tsgo` doesn't push
   `textDocument/publishDiagnostics` yet
   (microsoft/typescript-go#2362) — hover/completion/definition/rename all
   work, but you won't see live red squiggles from the native variant
   until upstream adds that. Switching back to `classic` gets diagnostics
   back immediately.

## Other known, documented simplifications

(each has a comment at the exact spot in the code, this is just the index)

- Completion: LSP snippet placeholders (`${1:foo}`) are stripped to plain
  text, not really expanded — no tab-stops.
- Code actions / rename: applied via `changes` or `documentChanges`;
  resource operations (`create`/`rename`/`delete` file) are applied but
  without the spec's optional `overwrite`/`ignoreIfExists` options, since
  the existing `fs` command surface doesn't expose them.
- Format Document always requests `insertSpaces: true` — there's no
  per-file tabs-vs-spaces setting tracked yet, only `tabSize` (read from
  the real per-tab facet).
- Auto-restart backs off 1s/2s/4s/8s/16s, then gives up after 5
  consecutive crashes and waits for a manual restart.
- Workspace-root detection stops descending once a directory matches (see
  `project_scanner.rs`'s doc comment) — a monorepo's own nested packages
  are still found (each gets checked independently on the way down before
  the "stop" rule kicks in for *their* subfolders), but this doesn't
  special-case e.g. a `package.json` `workspaces` field.

## Everything registered

New commands (Command Palette + a real keybinding, not just a palette
entry): Go to Definition (F12), Rename Symbol (F2), Code Action (Ctrl+.),
Format Document (Ctrl+Shift+Alt+F), Go to Symbol in File (Ctrl+Shift+O),
Go to Symbol in Workspace (Ctrl+T), LSP: Restart Server for Current File,
LSP: Stop All Servers. Status bar gets an "LSP" chip (click for the
server list — status, restart/stop, variant picker, output log).