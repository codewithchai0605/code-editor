# What's in this archive

Only the files that were changed or added — drop them into your project at
the same relative paths (they'll overwrite the originals). Nothing else in
your project was touched.

## 1. Duplicate tabs / broken breadcrumb / cursor-at-top

Root cause: paths/URIs coming back from the language server aren't
guaranteed to keep the same drive-letter case/separator style your app
used to open the file — comparing them with `===` made the *same* file
look like a different one.

- `src/lib/utils/path.ts` — new `canonicalPathKey`/`pathsEqual`/
  `isPathUnder` helpers, mirroring the `paths_equal`/`workspace_key` fix
  already in your Rust backend, extended to files and to the frontend.
- `src/lib/lsp/uri.ts` — new `normalizeUriKey`.
- `src/lib/stores/editor.svelte.ts` — `findTab`, `open`, `openAndReveal`,
  `handleExternalChange`, `applyChangesToTab` now use these instead of raw
  `===`.
- `src/lib/components/shell/Breadcrumbs.svelte` — breadcrumb segmenting
  tolerant of the same differences.
- `src/lib/lsp/manager.svelte.ts` — project-root resolution and the
  diagnostics force-refresh check fixed the same way.
- `src/lib/lsp/diagnostics.ts` — diagnostics cache keyed by
  `normalizeUriKey`, not the raw URI string.
- `src/lib/stores/git.svelte.ts` — reuses the shared canonical key
  (also fixes a latent bug where it unconditionally lowercased paths).

## 2. Native right-click menu (Emoji/Voice typing) inside the editor

- New `src/lib/editor/context-menu.svelte.ts` + `src/lib/editor/context-menu.ts`
  — a real right-click menu: Cut/Copy/Paste, plus Go to Definition/Rename
  Symbol/Quick Fix/Format Document when the file has a language server.
- `src/lib/editor/setup.ts`, `src/lib/components/shell/Shell.svelte` — wired in.

## 3. Hover expand/collapse ("+"/"-")

TypeScript's non-standard "Expandable Hover" extension to
`textDocument/hover`. Needed two things neither of which is obvious from
the LSP spec:

- `src-tauri/src/lsp/registry.rs` — the "classic" TypeScript variant now
  sets `supportsHoverVerbosity: true` in `initializationOptions`.
  typescript-language-server keeps this feature *off* by default; without
  it, the server silently ignores `verbosityLevel` and never returns
  `canIncreaseVerbosityLevel`, no matter how correct the request-side code
  is. Also adds a one-time self-healing backfill (`load_registry`) so a
  workspace you'd already opened before this field existed picks it up
  automatically rather than needing a hand-edited config file.
- `src/lib/lsp/hover.ts` — tracks the hover's current verbosity level
  client-side, since typescript-language-server only ever reports the
  *increase* direction, never "can decrease" (found this reading its
  actual source — see the comment on `LspHoverResult`).
- `src-tauri/src/lsp/manager.rs`, `src-tauri/src/commands/lsp.rs`,
  `src/lib/api/lsp.ts` — the `verbosityLevel` request plumbing.
- `src/lib/editor/themeEngine.ts` — matching theme-aware CSS.

**To see it:** rebuild, then run "LSP: Restart Server for Current File"
from the Command Palette (a plain file reopen isn't enough — the server
only reads `initializationOptions` once, at startup).

## 4. "Failing to find tsserver" with no `node_modules` (this round)

What was happening: typescript-language-server needs a `typescript`
install to load `tsserver.js` from. With no project `node_modules`, and
this app's own TypeScript install deliberately *not* on the system PATH
(kept isolated per `installer.rs`'s own design — see its module doc
comment), the server had nowhere to look and hard-crashed on startup:
*"Could not find a valid TypeScript installation ... Exiting."* VS Code
doesn't hit this because it ships its own bundled TypeScript and falls
back to it automatically.

- `src-tauri/src/lsp/manager.rs` — new `build_initialization_options`,
  called right before the server starts. For TypeScript's classic variant
  specifically, if this app's own bundled `typescript` is already
  installed, it sets `tsserver.fallbackPath` to point at it —
  **`fallbackPath`, not `path`**: `path` would always override a
  project's own pinned TypeScript version; `fallbackPath` only ever
  kicks in when the workspace has none of its own, which is what
  actually matches VS Code's real behavior (workspace version wins,
  bundled version is just the safety net). Computed dynamically at
  startup, not baked into the persisted config file, since your app's
  install location is a local-machine detail that would be wrong if that
  JSON file were ever copied to another machine.
- `src-tauri/src/lsp/installer.rs` — new `bundled_typescript_lib_dir`,
  the actual lookup this reads from.

This should just work automatically the next time the TypeScript server
starts — no config file edit needed, unlike the hover fix (this one isn't
stored in `lsp-config.json` at all).

## 5. LSP "bundles" + HTML/CSS/JSON support (this round)

Your ask: a plain static site (html/css/js, no `package.json`) should
still get full LSP features, and a "bundle" should mean an actual shared
install, not three separate ones.

- `src-tauri/src/lsp/registry.rs` — added `html`, `css`, `json`, using
  `vscode-html-language-server`/`vscode-css-language-server`/
  `vscode-json-language-server` (all three ship in one npm package,
  `vscode-langservers-extracted` — the actively-maintained fork of VS
  Code's own bundled language servers, ~1.5M downloads/month, used by
  several other editors including Zed). All three set `rootMarkers: []`
  deliberately — unlike TypeScript/Rust/Go, there's no manifest file a
  static site is expected to have, and none of these servers need a
  "project" in that sense anyway; with no marker to detect, every
  html/css/json file just resolves to the workspace root, which is
  already the existing, intentional fallback in
  `lsp/manager.svelte.ts`'s `resolveRootFor` — nothing needed changing
  there.
- **New `installGroup` field on `ServerConfig`** (`registry.rs`) — this is
  the actual "bundle" mechanism: every `ServerConfig` naming the same
  group shares one install directory and one `npm install` run, rather
  than each independently re-installing the identical package. Opening an
  html file triggers the one shared install; opening a css or json file
  afterward finds its own binary already sitting in that same shared
  directory (see `installer.rs`'s `expected_binary_path` — it recognizes
  a sibling's already-installed binary directly, so the status bar
  doesn't wrongly show "not installed" for the second and third language).
  This is a real, reusable mechanism — a future bundle (say, an ESLint +
  Prettier combo) is just another `installGroup` string away, no new
  plumbing needed.
- `src-tauri/src/lsp/installer.rs` — `tool_dir` now branches on
  `install_group`; marker files became per-command (`.codeeditor-installed-
  <command>`) instead of one fixed name, since a shared directory now
  legitimately holds more than one installed binary at once.
- `src/lib/lsp/languageId.ts` — routes `.html`/`.htm`/`.css`/`.json` to
  their language IDs. **Also fixes a pre-existing, unrelated gap I found
  while I was in this file**: `dart`/`go`/`cpp`/`php` already had full
  server configs in `registry.rs` but *no entry here at all* — meaning
  `.dart`/`.go`/`.cpp`/`.h`/`.php` files got zero LSP features no matter
  what, since this map is what turns a file extension into a language ID
  in the first place. Filled those in too while adding html/css/json.
- No other frontend changes needed for hover/completion/diagnostics/
  format/rename to start working on these three languages —
  `editor/setup.ts` already wires every LSP feature generically off
  `lspLanguageIdFor(filePath)`, so they just light up once the language ID
  and registry entry exist.

**Scope note:** I didn't wire up `vscode-eslint-language-server` (also in
that same npm package) — ESLint needs a project's own `.eslintrc`
and installed rules to do anything useful, so it doesn't fit the
"works with zero config" goal this round the way html/css/json do. Same
`installGroup` mechanism would cover it if you want it later.

## Validation done / not done

- All `.ts` files: full project `tsc --noEmit` passes clean (checked the
  checker actually catches errors, not just skipping silently).
- Both edited `.svelte` files (from the earlier round): compile cleanly
  through Svelte's own compiler.
- **Rust changes are not compiler-verified.** This sandbox's available
  Rust toolchain (1.75, apt) is too old for your project's own dependency
  tree regardless of my changes (`tauri-plugin-fs` needs `toml ^1.0`,
  which needs edition2024 / cargo ≥~1.85), and I have no network access to
  install a newer one here (confirmed by trying — `rustup`'s own installer
  domain isn't reachable, and downgrading the toml dependency chain to
  something 1.75 could build hits a hard version floor from
  `tauri-plugin-fs` itself). I reviewed every diff by hand — ownership/
  move-checked the new functions, matched the file's own existing
  patterns (let-else, match ergonomics, `serde_json::Map::entry`), and
  ran brace/paren balance checks across every touched file — but please
  run `cargo check` on your own machine before trusting it fully. This
  round touched more Rust than the last one (`registry.rs`,
  `installer.rs`, `manager.rs`), so I'd treat this build as more likely to
  have a typo than the last.
- The `vscode-langservers-extracted` package name and its three binary
  names (`vscode-html-language-server`/`vscode-css-language-server`/
  `vscode-json-language-server`) are verified against its actual npm
  listing and GitHub README, not assumed.
- The `typescript-language-server` behavior (the `supportsHoverVerbosity`
  opt-in, only reporting the increase direction, and the
  `tsserver.path`/`fallbackPath` distinction) is taken directly from that
  project's own source and docs on GitHub.
- I still can't click-test any of this — no GUI in this environment, and
  the html/css/json servers additionally need `npm` reachable on your
  machine the same way typescript-language-server's install already does.
  Please smoke-test: opening a plain html/css/json file (watch the status
  bar for each installing once, and confirm css/json don't show a
  separate "Install" prompt after html's install finishes), and a React
  project with no `node_modules` (should no longer show a hard failure
  finding tsserver).
