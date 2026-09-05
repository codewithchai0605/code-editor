# Code Editor

A lightweight code editor built on **Tauri + Rust**, with **CodeMirror 6** (not Monaco) for editing, native Rust plugins for extensions, and a generic user-configured LSP client.

Full architecture + phased roadmap: see the implementation plan doc from planning (`vscode-clone-implementation-plan.md`).

## Status

**Phase 0–1 complete:** project scaffolding + the app shell (Activity Bar, Sidebar, Editor area, bottom Panel, Status Bar), fully resizable and responsive down to the window's minimum size.

Not built yet (upcoming phases): file tree/file operations, CodeMirror-backed EditorFile with image/audio viewers, Quick Open, Command Palette, integrated terminal, LSP client, native plugin host.

## Running it

```bash
npm install
npm run doctor      # sanity-checks that Rust/cargo are installed
npm run tauri dev   # launches the app
```

Requires Node.js, and a Rust toolchain (`rustup.rs`) — on Windows you also need the "Desktop development with C++" workload from the Visual Studio Build Tools, for the MSVC linker.

## Project structure

```
src/                              SvelteKit frontend
├── app.css                       Global design tokens (colors, fonts, scrollbars) + reset
├── routes/
│   ├── +layout.svelte            Imports app.css
│   └── +page.svelte              Renders <Shell />
└── lib/
    ├── components/shell/         App chrome: ActivityBar, Sidebar, EditorArea, Panel,
    │                              StatusBar, ResizeHandle, Shell (composes all of them)
    ├── stores/
    │   └── layout.svelte.ts      Svelte 5 runes store: pane sizes, visibility,
    │                              responsive breakpoint logic
    └── types/                    Shared TS types (grows with later phases)

src-tauri/                        Rust backend
└── src/
    ├── lib.rs                    Tauri builder entry point
    ├── main.rs                   Binary entry point (unchanged from template)
    └── commands/
        ├── mod.rs                One file per concern; fs.rs, pty.rs, lsp.rs,
        │                          plugins.rs land here in later phases
        └── app.rs                app_version command (proves the IPC round trip)

scripts/
└── check-env.mjs                 `npm run doctor` — checks for cargo/rustc before dev
```

## Design notes

- **Responsiveness**: the shell is flexbox-based, not fixed pixel layout. The sidebar and
  panel are user-resizable via drag handles, but their widths/heights are continuously
  clamped against the *current* window size (see `layout.svelte.ts`), and the sidebar
  auto-collapses below a width breakpoint. The window itself has a `minWidth`/`minHeight`
  floor set in `tauri.conf.json` so the layout never has to render in an impossible space.
- **Theming**: one dark theme for now, entirely driven by CSS custom properties in
  `app.css`. A theme system is a later (settings) phase — swapping themes later means
  swapping variable values, not rewriting components.
- **Commands module**: `src-tauri/src/commands/` is split one-file-per-concern from the
  start, since it'll hold dozens of commands (fs, pty, lsp, plugin host) by the end.
