import { EditorSelection, EditorState, Text, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { listen } from "@tauri-apps/api/event";
import { readTextFile, writeTextFile, readBinaryPreview } from "$lib/api/fs";
import { isFsError, type FsChangedPayload } from "$lib/types/fs";
import { describeFsError } from "$lib/utils/fsError";
import { guessFileKind, type FileKind } from "$lib/editor/fileKind";
import { loadLanguage } from "$lib/editor/languages";
import { baseExtensions, themeCompartment } from "$lib/editor/setup";
import { themeStore } from "./theme.svelte";
import { workspace } from "./workspace.svelte";
import { lspLanguageIdFor } from "$lib/lsp/languageId";
import { pathToFileUri } from "$lib/lsp/uri";
import { lspDidOpen, lspDidChange, lspDidClose } from "$lib/api/lsp";
import { clearDiagnosticsForUri } from "$lib/lsp/diagnostics";
import { gitStore } from "./git.svelte";
import { gitFileBaseline } from "$lib/api/git";
import { setGitBaseline } from "$lib/editor/git-gutter";
import { revealHighlight } from "$lib/editor/reveal-highlight";
import { lspStore } from "$lib/lsp/manager.svelte";
import { pathsEqual, canonicalPathKey } from "$lib/utils/path";

const LSP_CHANGE_DEBOUNCE_MS = 300;

export interface OpenTab {
    path: string;
    kind: FileKind;
    loading: boolean;
    error: string | null;
    isDirty: boolean;
    /** Only set for kind === "text". Kept in sync with every keystroke via
     *  the updateListener in `open()`, so switching away and back never
     *  loses in-progress edits, undo history, or the selection. */
    cmState?: EditorState;
    /** Only set for kind === "binary" (the hex viewer). */
    binaryBytes?: Uint8Array;
    /** LSP document version — starts at 1 on open (matching the initial
     *  didOpen notification) and increments on every didChange sent. Only
     *  meaningful when this file has a recognized LSP language (see
     *  lsp/languageId.ts); otherwise unused. */
    lspVersion?: number;
    /** True when the fs watcher reported a change to this file while it had
     *  unsaved edits, so the reload that normally happens automatically
     *  (see `handleExternalChange`) was held back rather than clobbering
     *  them. Cleared on save (the buffer becomes the source of truth again)
     *  or as soon as a reload does go through. */
    changedOnDisk?: boolean;
}

/** A cursor/selection position to jump to once `path` is open and its
 *  `EditorView` exists — see `openAndReveal`/`applyPendingReveal` below,
 *  used by the workspace search panel's "click a match to navigate" and
 *  ready for any future caller that needs the same "open, then land on a
 *  specific spot" behavior (e.g. go-to-definition). */
export interface RevealTarget {
    path: string;
    /** 1-based, matching search.rs's own line numbering. */
    line: number;
    /** UTF-16 code unit offsets, matching search.rs's column units. */
    colStart: number;
    colEnd: number;
    /** Optional UTF-16 offset of the match window's start within the full
     *  line — set by workspace-search navigation for matches on lines the
     *  backend windowed down (see `SearchMatch.lineTextOffset`), so
     *  `applyPendingReveal` can translate window-relative columns back to
     *  real document columns. Other callers (e.g. go-to-definition) pass
     *  absolute positions and leave this undefined/0. */
    lineTextOffset?: number;
}

/** `Text.of` splits on `\n` only, which would leave a stray `\r` at the
 *  end of every line of a CRLF file's baseline — harmless for the diff
 *  itself (both sides would carry it), but this normalizes so a
 *  CRLF-in-git/LF-in-buffer (or vice versa) mismatch doesn't manufacture
 *  a "changed" line out of nothing but a line-ending difference. */
function textFromString(value: string): Text {
    return Text.of(value.length ? value.split(/\r\n|\r|\n/) : [""]);
}

/** Common-prefix/common-suffix diff between two full-file texts, reduced to
 *  a single `{from, to, insert}` change — `null` if identical. Used by
 *  `reloadFromDisk` to bring a tab back in sync with an externally-changed
 *  file as one minimal edit rather than replacing the whole document: for
 *  the common case (another tool added, removed, or edited a line
 *  elsewhere in the file — e.g. `npm install` rewriting a dependency
 *  version in `package.json`) this keeps the cursor, scroll position, and
 *  undo history intact instead of resetting them on every external change.
 *  Not a real diff — a full-file rewrite (a formatter reflowing
 *  everything) degenerates to "replace everything", same as it would
 *  without this — but it's O(n) and handles the localized-edit case, which
 *  is the overwhelming majority of "something external touched this file"
 *  in practice. */
function minimalChange(
    oldText: string,
    newText: string,
): { from: number; to: number; insert: string } | null {
    if (oldText === newText) return null;
    const maxPrefix = Math.min(oldText.length, newText.length);
    let start = 0;
    while (start < maxPrefix && oldText.charCodeAt(start) === newText.charCodeAt(start)) {
        start++;
    }

    let oldEnd = oldText.length;
    let newEnd = newText.length;
    while (
        oldEnd > start &&
        newEnd > start &&
        oldText.charCodeAt(oldEnd - 1) === newText.charCodeAt(newEnd - 1)
    ) {
        oldEnd--;
        newEnd--;
    }

    return { from: start, to: oldEnd, insert: newText.slice(start, newEnd) };
}

class EditorStore {
    tabs = $state<OpenTab[]>([]);
    activePath = $state<string | null>(null);
    pendingClose = $state<{ path: string } | null>(null);

    /** Set by `openAndReveal`, cleared by `applyPendingReveal` the moment it's
     *  acted on. Transient by design — nothing should read this expecting it
     *  to still describe "where the user is", only "where they're about to be
     *  taken". */
    pendingReveal = $state<RevealTarget | null>(null);

    /** True for the duration of a programmatic view.setState() call (tab
     *  switch), so the per-tab updateListener doesn't mistake the resulting
     *  update for a real edit and mark a freshly-activated tab dirty. See
     *  TextEditor.svelte. */
    suppressDirtyTracking = false;

    /** The live CodeMirror view, registered by TextEditor.svelte whenever a
     *  text tab is mounted. Only needed so a theme change can be pushed into
     *  the currently-visible editor immediately — see applyThemeToOpenTabs. */
    private activeView: EditorView | null = null;

    private fsListenerStarted = false;

    /** VS Code's `explorer.autoReveal` equivalent — when on (the default),
     *  activating a text tab expands + selects its location in the Explorer
     *  tree. Read reactively inside the `$effect` below via
     *  `editorStore.autoReveal`, so toggling the command takes effect for
     *  subsequent tab switches without any restart. */
    autoReveal = $state(true);

    /** Scroll the Explorer tree so the row for `path` is visible, if it's
     *  rendered (it won't be until revealInExplorer has finished expanding
     *  its ancestors — Svelte re-renders first, then this runs). Waits one
     *  microtask + animation frame so it lands after that render. */
    private scrollExplorerTo(path: string): void {
        queueMicrotask(() => {
            requestAnimationFrame(() => {
                const el = document.querySelector(`[data-tree-path="${CSS.escape(path)}"]`);
                el?.scrollIntoView({ block: "nearest" });
            });
        });
    }

    constructor() {
        // VS Code-style auto-reveal: whenever a different text tab becomes
        // active (and the toggle is on), expand + select its location in
        // the Explorer tree and scroll it into view. `$effect` lives here
        // rather than in a component so it runs exactly once for the store's
        // lifetime; it tracks `activePath` + `autoReveal` reactively.
        $effect(() => {
            const path = this.activePath;
            const enabled = this.autoReveal;
            if (!path || !enabled) return;
            const tab = this.findTab(path);
            if (!tab || tab.kind !== "text") return;
            void workspace.revealInExplorer(path).then(() => this.scrollExplorerTo(path));
        });

        // Keeps every open file's dirty-diff gutter pointed at the right
        // baseline: staging, unstaging, committing, or an external change to
        // the index (branch switch, pull, ...) all move what an "unchanged"
        // line means for that repo's files. Deliberately not scoped to only
        // the active tab — a background tab's gutter should be just as
        // current the moment it's switched into, not stale until some other
        // trigger refreshes it.
        gitStore.onStatusChanged((repoId) => this.refreshGitBaselinesForRepo(repoId));
    }

    registerActiveView(view: EditorView | null): void {
        this.activeView = view;
    }

    /** The live view behind the currently-visible text tab, if any — needed
     *  by any command that acts on the editor directly (move/copy line,
     *  select-next-occurrence, a future quick-fix menu, ...) rather than on
     *  `workspace`/`layout` state. See coreCommands.ts. */
    get view(): EditorView | null {
        return this.activeView;
    }

    get activeTab(): OpenTab | undefined {
        return this.tabs.find((t) => t.path === this.activePath);
    }

    /** Uses `pathsEqual`, not `===` — `path` may have come back from a
     *  language server (go-to-definition, workspace symbols, a rename's
     *  workspace edit, ...), which doesn't promise to preserve the exact
     *  drive-letter case/separator style this app used when it first
     *  opened the file. A raw `===` here is what used to make Ctrl+Click
     *  on a symbol in the *same* already-open file open a second,
     *  duplicate tab for it. See `canonicalPathKey`'s doc comment. */
    private findTab(path: string): OpenTab | undefined {
        return this.tabs.find((t) => pathsEqual(t.path, path));
    }

    /** Wires up the backend's `fs:changed` event (see fs.rs's
     *  `start_watcher`) to reload any open tab whose exact file was
     *  touched — another editor, an external agent, `npm install`
     *  rewriting `package-lock.json`, a git checkout, anything that isn't
     *  this app's own `save()`. Previously nothing did this at all: an
     *  externally-changed file only ever showed up once its tab was closed
     *  and reopened. `workspace` listens for the same event independently
     *  (see its own `initFsWatcherListener`) to refresh directory listings
     *  — kept as two separate listeners rather than one calling into the
     *  other so `editor.svelte.ts` and `workspace.svelte.ts` don't end up
     *  importing each other. Call once, e.g. from Shell.svelte's onMount. */
    initFsWatcherListener(): void {
        if (this.fsListenerStarted) return;
        this.fsListenerStarted = true;
        listen<FsChangedPayload>("fs:changed", (event) => {
            this.handleExternalChange(event.payload.paths);
        });
    }

    /** Matches every path the watcher reported against open tabs and
     *  reloads the ones that are text and *not* dirty. A dirty tab is left
     *  alone and flagged instead (`changedOnDisk`) — silently discarding
     *  someone's in-progress edit because a build script happened to touch
     *  the same file would be far worse than a stale buffer; they still
     *  get to see the file changed underneath them and decide what to do
     *  about it (the alternative here would be a merge UI, which is a lot
     *  of machinery for a case that in practice is rare — most external
     *  changes land on files nobody's mid-edit in). */
    private handleExternalChange(paths: string[]): void {
        if (paths.length === 0) return;
        const changed = new Set(paths.map(canonicalPathKey));
        for (const tab of this.tabs) {
            if (tab.kind !== "text" || !changed.has(canonicalPathKey(tab.path))) continue;
            if (tab.isDirty) {
                tab.changedOnDisk = true;
                continue;
            }
            void this.reloadFromDisk(tab.path);
        }
    }

    /** Reloads `path`'s buffer from disk. Only ever called for a *clean*
     *  tab — see `handleExternalChange`, the sole caller — so overwriting
     *  the buffer here never discards anything the user hasn't already
     *  saved. Applies the reload through `suppressDirtyTracking` (the same
     *  mechanism `applyThemeToOpenTabs` uses) so the updateListener's
     *  `markDirty` doesn't immediately re-flag a tab that, by construction,
     *  now exactly matches what's on disk — then notifies the LSP directly
     *  afterwards, since that half is still very much wanted (the server's
     *  view of the file needs to track reality regardless of who changed
     *  it, not just the user typing in this editor). */
    private async reloadFromDisk(path: string): Promise<void> {
        const tab = this.findTab(path);
        if (!tab || tab.kind !== "text" || !tab.cmState) return;

        let content: string;
        try {
            content = await readTextFile(path);
        } catch {
            // Gone, became unreadable, or turned into something too
            // large/binary in the same beat the watcher fired — leave the
            // buffer as last known-good rather than erroring over a
            // transient race (e.g. a save-as elsewhere that
            // deletes-then-recreates the file).
            return;
        }

        const oldText = tab.cmState.doc.toString();
        const change = minimalChange(oldText, content);
        tab.changedOnDisk = false;
        if (!change) return; // watcher fired but the content on disk hasn't actually changed

        this.suppressDirtyTracking = true;
        try {
            if (path === this.activePath && this.activeView) {
                this.activeView.dispatch({ changes: change });
            } else {
                tab.cmState = tab.cmState.update({ changes: change }).state;
            }
        } finally {
            this.suppressDirtyTracking = false;
        }
        tab.isDirty = false;
        this.scheduleLspChange(path);
    }

    async open(path: string): Promise<void> {
        const existing = this.findTab(path);
        if (existing) {
            // Reuse the tab's own (already-canonical) path string, not the
            // possibly differently-cased/separated `path` we were just
            // asked to open — keeps `activePath` exactly equal to some
            // `tab.path` by reference value, which every `t.path ===
            // this.activePath` check elsewhere in this store (and
            // Breadcrumbs.svelte) relies on.
            this.activePath = existing.path;
            return;
        }

        const guessed = guessFileKind(path);
        if (guessed !== "text") {
            this.tabs.push({ path, kind: guessed, loading: false, error: null, isDirty: false });
            this.activePath = path;
            return;
        }

        // Push a plain placeholder object, then treat it as write-only from
        // here on. Pushing wraps it in a *new* reactive proxy inside the
        // `$state` array — this local `tab` binding still points at the raw,
        // pre-push object, so mutating it later (e.g. after the `await` below)
        // would silently write to an object the UI never reads from, leaving
        // `loading` stuck at `true` forever. Every further mutation goes
        // through `updateTab()`, which re-fetches the live proxy each time.
        this.tabs.push({ path, kind: "text", loading: true, error: null, isDirty: false });
        this.activePath = path;

        try {
            const [content, language] = await Promise.all([readTextFile(path), loadLanguage(path)]);

            const extensions: Extension[] = [
                ...baseExtensions(() => this.save(path), themeStore.theme, path),
                ...(language ? [language] : []),
                EditorView.updateListener.of((update) => {
                    if (!update.docChanged) return;
                    this.syncDoc(path, update.state);
                    if (!this.suppressDirtyTracking) {
                        this.markDirty(path);
                        this.scheduleLspChange(path);
                    }
                }),
            ];
            this.updateTab(path, {
                cmState: EditorState.create({ doc: content, extensions }),
                loading: false,
                lspVersion: 1,
            });
            this.notifyLspOpen(path, content);
            void this.refreshGitBaseline(path);
        } catch (err) {
            if (isFsError(err) && err.kind === "unsupported") {
                await this.loadAsBinary(path);
            } else {
                this.updateTab(path, { loading: false, error: describeFsError(err) });
            }
        }
    }

    /** Opens `path` (same as `open`) and, once a live `EditorView` exists for
     *  it, moves the selection to `[colStart, colEnd)` on `line` and scrolls
     *  it into view. Used by the workspace search panel's "click a match".
     *
     *  Where the actual reveal happens depends on what `path` already is:
     *  - A brand-new tab: `TextEditor.svelte`'s `onMount` applies it, since a
     *    freshly-created view's `state` already equals `tab.cmState` (no
     *    `$effect` mismatch to trigger on).
     *  - An already-open, currently-*inactive* tab: switching `activePath` to
     *    it changes `TextEditor`'s `tab` prop, which its `$effect` picks up
     *    as a state mismatch and applies the reveal after `setState`.
     *  - An already-open, already-*active* tab: neither of the above fires
     *    (nothing about `tab` or its `cmState` actually changes), so this
     *    method applies it directly against the live view itself. */
    async openAndReveal(
        path: string,
        line: number,
        colStart: number,
        colEnd: number,
        lineTextOffset = 0,
    ): Promise<void> {
        // Captured *before* `open()` runs: if `path` is already the active tab,
        // `open()`'s early-return path touches nothing about it, so neither of
        // TextEditor's own hooks below has anything to react to and this method
        // must apply the reveal itself. Every other case (a brand-new tab, or
        // switching in from a different tab) genuinely changes what TextEditor
        // is rendering, so its hooks pick it up on their own — applying it here
        // too, before Svelte's reactivity has actually flushed, would risk
        // landing before that tab-swap `setState` and then being clobbered by
        // it (setState replaces the selection along with everything else).
        // Resolved *before* `open()` runs, and against the same tab lookup
        // `open()` itself will use — `path` here may be a differently-cased
        // /separated string for a file that's already open (see
        // `findTab`'s doc comment), in which case `open()` will resolve
        // `activePath` to the tab's own canonical path, not this one, so
        // `pendingReveal` needs to match that canonical form too or
        // `applyPendingReveal`'s `target.path !== this.activePath` guard
        // would never match.
        const existing = this.findTab(path);
        const canonicalPath = existing?.path ?? path;
        const alreadyActive = existing !== undefined && pathsEqual(this.activePath ?? "", path);
        this.pendingReveal = { path: canonicalPath, line, colStart, colEnd, lineTextOffset };
        await this.open(path);
        if (alreadyActive && this.activeView) {
            this.applyPendingReveal(this.activeView);
        }
    }

    /** Applies `pendingReveal` against `view` if it targets `view`'s tab,
     *  then clears it. A no-op otherwise — safe to call unconditionally from
     *  every place a view's document can change out from under it. */
    applyPendingReveal(view: EditorView): void {
        const target = this.pendingReveal;
        if (!target || target.path !== this.activePath) return;
        this.pendingReveal = null;

        const doc = view.state.doc;
        const lineNumber = Math.min(Math.max(target.line, 1), doc.lines);
        const lineInfo = doc.line(lineNumber);
        // `lineTextOffset` shifts window-relative columns (a match the
        // backend truncated down to a slice of an over-long line — see
        // search.rs's `windowed_line`) back to absolute ones; it's 0 for
        // every ordinary line and for non-search callers like
        // go-to-definition, so this is a no-op there.
        const offset = target.lineTextOffset ?? 0;
        const from = Math.min(lineInfo.from + offset + target.colStart, lineInfo.to);
        const to = Math.min(lineInfo.from + offset + target.colEnd, lineInfo.to);

        view.dispatch({
            selection: EditorSelection.range(from, to),
            effects: [
                EditorView.scrollIntoView(from, { y: "center" }),
                // Brief flash highlight so the user actually *sees* where
                // they landed — clicking a search result in a file that was
                // already open on screen otherwise moves only the caret,
                // which is easy to miss in a long document.
                revealHighlight.of({ from, to }),
            ].flat(),
        });
        view.focus();
    }

    /** Re-fetches the tab from the live array before patching it — see the
     *  comment in `open()` for why that re-fetch is required, not optional.
     *  Silently no-ops if the tab was closed while an async load was in
     *  flight, which is the correct behavior (nothing left to update). */
    private updateTab(path: string, patch: Partial<OpenTab>): void {
        const tab = this.findTab(path);
        if (tab) Object.assign(tab, patch);
    }

    private async loadAsBinary(path: string): Promise<void> {
        this.updateTab(path, { kind: "binary" });
        try {
            const bytes = new Uint8Array(await readBinaryPreview(path));
            this.updateTab(path, { binaryBytes: bytes, loading: false });
        } catch (err) {
            this.updateTab(path, { loading: false, error: describeFsError(err) });
        }
    }

    private syncDoc(path: string, state: EditorState): void {
        const tab = this.findTab(path);
        if (tab) tab.cmState = state;
    }

    /** Re-fetches `path`'s git-gutter baseline (the index blob it should be
     *  diffed against) and pushes it into that tab's editor state. Called
     *  once when a text tab first opens, and again — for every open tab
     *  owned by the repo in question — whenever that repo's status changes
     *  (see the constructor). Silently gives up on failure: a file outside
     *  any repo, or a transient read error, should just mean "no gutter",
     *  not a surfaced error over something this incidental. */
    private async refreshGitBaseline(path: string): Promise<void> {
        const repo = gitStore.repoFor(path);
        if (!repo) {
            this.dispatchGitBaseline(path, null);
            return;
        }
        try {
            const baseline = await gitFileBaseline(repo.id, path);
            this.dispatchGitBaseline(path, textFromString(baseline));
        } catch {
            this.dispatchGitBaseline(path, null);
        }
    }

    private refreshGitBaselinesForRepo(repoId: string): void {
        for (const tab of this.tabs) {
            if (tab.kind !== "text" || !tab.cmState) continue;
            if (gitStore.repoFor(tab.path)?.id === repoId) {
                void this.refreshGitBaseline(tab.path);
            }
        }
    }

    /** Dispatches `setGitBaseline` against whichever `EditorState` is
     *  currently canonical for `path` — the live view directly if it's the
     *  active tab (a real `dispatch`, so scroll/selection/undo history are
     *  untouched, same reasoning as `applyPendingReveal`), or the stored
     *  `cmState` otherwise. Re-fetches the tab from the live array first in
     *  case it closed while the fetch above was in flight. */
    private dispatchGitBaseline(path: string, baseline: Text | null): void {
        const tab = this.findTab(path);
        if (!tab || tab.kind !== "text" || !tab.cmState) return;
        const effects = setGitBaseline.of(baseline);
        if (tab.path === this.activePath && this.activeView) {
            this.activeView.dispatch({ effects });
            tab.cmState = this.activeView.state;
        } else {
            tab.cmState = tab.cmState.update({ effects }).state;
        }
    }

    private markDirty(path: string): void {
        const tab = this.findTab(path);
        if (tab && !tab.isDirty) tab.isDirty = true;
    }

    /** The current text of an open tab, without going through a file read —
     *  `null` if `path` isn't open (or isn't a text tab). Used by
     *  `lsp/workspaceEdit.ts` to decide whether to apply a `WorkspaceEdit`'s
     *  edits against the live buffer (open) or read-modify-write the file
     *  directly on disk (not open) — and, in the open case, as the base
     *  text LSP positions are converted against. */
    docTextIfOpen(path: string): string | null {
        const tab = this.findTab(path);
        return tab?.kind === "text" && tab.cmState ? tab.cmState.doc.toString() : null;
    }

    /** Applies a set of non-overlapping `{from, to, insert}` changes
     *  (offsets against the tab's *current* document) to an already-open
     *  tab — the "open" half of `lsp/workspaceEdit.ts`'s apply logic; see
     *  `docTextIfOpen` for why a caller would already know this tab is
     *  open before calling this. Routes through a real `view.dispatch` for
     *  the active tab so it goes through the exact same pipeline as the
     *  user typing (dirty flag, LSP didChange, undo history all included
     *  for free); an inactive tab's stored state is updated the same way
     *  `applyThemeToOpenTabs` does, but *without* suppressing dirty
     *  tracking — unlike a theme swap, this is a real content change. */
    applyChangesToTab(path: string, changes: { from: number; to: number; insert: string }[]): void {
        const tab = this.findTab(path);
        if (!tab?.cmState || changes.length === 0) return;

        // Compares `tab.path` (the tab's own canonical path), not the raw
        // `path` argument — a workspace-edit's path came from a language
        // server and isn't guaranteed to match `activePath`'s exact casing
        // even when it does refer to the active tab (see `findTab`'s doc
        // comment). Using the tab's own path keeps this in the fast,
        // real-`dispatch` branch instead of silently falling back to the
        // slower "inactive tab" one for what's actually the visible file.
        if (tab.path === this.activePath && this.activeView) {
            this.activeView.dispatch({ changes });
            return;
        }

        const nextState = tab.cmState.update({ changes }).state;
        tab.cmState = nextState;
        this.markDirty(tab.path);
        this.scheduleLspChange(tab.path);
    }

    /** Fire-and-forget notifications to the LSP backend — errors are logged,
     *  not surfaced to the user, since a missing/crashed language server
     *  should degrade to "no diagnostics/completions for this file", never
     *  block editing. `workspace.rootPath` is guaranteed non-null by the
     *  time any tab can be open (there's no way to open a file without a
     *  workspace first), but the check is kept explicit rather than
     *  asserted, since "silently skip" is the right failure mode here too.
     *  The *project* root actually passed to the LSP commands below is
     *  `lspStore.resolveRootFor`'s job, not `workspace.rootPath` directly —
     *  see its doc comment for why a multi-root workspace needs that
     *  extra step. */

    /** Lightweight-LSP gate (see AGENT.md's "lazy mode" note). The
     *  html/css/json servers come from `vscode-langservers-extracted` — a
     *  Node process each, with an npm install standing behind them if this
     *  app hasn't fetched one yet. For a plain static html/css/js folder
     *  with no project manifest anywhere, that's a lot of machinery for
     *  what CodeMirror's own modes + Emmet already cover (highlighting,
     *  tag auto-rename, snippets, color chips), so those languages only get
     *  their server when root detection actually found a project covering
     *  the file (`package.json`, `tsconfig.json`, …) — exactly how VS Code
     *  behaves for a lone `.html` opened outside any workspace. Everything
     *  else (typescript, rust, go, python, …) always starts on open: either
     *  the language is meaningless without semantic info (Rust/Go/Python)
     *  or the fallback TS server path makes it cheap enough to justify. */
    private shouldStartServer(path: string, languageId: string): boolean {
        switch (languageId) {
            case "html":
            case "css":
            case "json":
                return lspStore.hasProjectRootFor(path);
            default:
                return true;
        }
    }

    private notifyLspOpen(path: string, text: string, attemptsLeft = 3): void {
        const languageId = lspLanguageIdFor(path);
        const workspaceRoot = workspace.rootPath;
        if (!languageId || !workspaceRoot) return;
        if (!this.shouldStartServer(path, languageId)) return;
        const root = lspStore.resolveRootFor(path, languageId, workspaceRoot);

        // Starting the server here (rather than eagerly for the whole
        // workspace) is what makes this lazy per-language: most projects only
        // ever touch a few of whatever's configured in
        // .codeeditor/lsp-servers.json, so there's no reason to spawn
        // everything up front. Awaiting this before didOpen also structurally
        // closes the "server still starting" race for the first file of a
        // session — ensureServerRunning only resolves once Rust's
        // initialize/initialized handshake has actually completed.
        lspStore.ensureServerRunning(root, languageId).then(() => {
            lspDidOpen(root, languageId, pathToFileUri(path), text).catch((err) => {
                // Still possible for a *second* file of the same language opened
                // concurrently with the first: Rust marks the server "running" in
                // its map as soon as it's spawned, before the handshake finishes,
                // so a fast-following didOpen can still arrive a moment early.
                // Worth a couple of short retries rather than permanently giving
                // up on LSP support for a file that just happened to race it.
                // `stillOpen` guards against a scheduled retry firing after the
                // tab was closed in the meantime, which would otherwise send a
                // spurious didOpen after the didClose already sent.
                const stillOpen = this.findTab(path) !== undefined;
                if (!stillOpen) return;
                const message = String(err);
                if (attemptsLeft > 1 && message.includes("No running LSP server")) {
                    setTimeout(() => this.notifyLspOpen(path, text, attemptsLeft - 1), 1500);
                } else {
                    console.error(`LSP didOpen failed for ${path}:`, err);
                }
            });
        });
    }

    private lspChangeTimers = new Map<string, ReturnType<typeof setTimeout>>();

    /** Debounced — see LSP_CHANGE_DEBOUNCE_MS. Called on every keystroke via
     *  the updateListener in open(), but only actually sends after typing
     *  pauses, since full-document sync means every send ships the entire
     *  file. */
    private scheduleLspChange(path: string): void {
        const languageId = lspLanguageIdFor(path);
        if (!languageId) return;

        const existing = this.lspChangeTimers.get(path);
        if (existing) clearTimeout(existing);

        const timer = setTimeout(() => {
            this.lspChangeTimers.delete(path);
            const tab = this.findTab(path);
            const workspaceRoot = workspace.rootPath;
            if (!tab?.cmState || !workspaceRoot) return;
            const root = lspStore.resolveRootFor(path, languageId, workspaceRoot);
            const version = (tab.lspVersion ?? 1) + 1;
            tab.lspVersion = version;
            lspDidChange(
                root,
                languageId,
                pathToFileUri(path),
                tab.cmState.doc.toString(),
                version,
            ).catch((err) => console.error(`LSP didChange failed for ${path}:`, err));
        }, LSP_CHANGE_DEBOUNCE_MS);

        this.lspChangeTimers.set(path, timer);
    }

    private notifyLspClose(path: string): void {
        const languageId = lspLanguageIdFor(path);
        const workspaceRoot = workspace.rootPath;
        const timer = this.lspChangeTimers.get(path);
        if (timer) {
            clearTimeout(timer);
            this.lspChangeTimers.delete(path);
        }
        clearDiagnosticsForUri(pathToFileUri(path));
        if (!languageId || !workspaceRoot) return;
        const root = lspStore.resolveRootFor(path, languageId, workspaceRoot);
        lspDidClose(root, languageId, pathToFileUri(path)).catch((err) =>
            console.error(`LSP didClose failed for ${path}:`, err),
        );
    }

    /** Reconfigures the theme compartment in every open text tab's stored
     *  state — including tabs that aren't currently visible — so a theme
     *  switch applies everywhere at once instead of only the active tab
     *  (with the rest catching up whenever they're next activated). The
     *  active tab additionally gets pushed into the live view directly,
     *  since updating a background tab's `cmState` object alone doesn't
     *  touch anything currently on screen. */
    applyThemeToOpenTabs(extension: Extension): void {
        const effects = themeCompartment.reconfigure(extension);
        for (const tab of this.tabs) {
            if (tab.kind !== "text" || !tab.cmState) continue;
            const nextState = tab.cmState.update({ effects }).state;
            tab.cmState = nextState;
            if (tab.path === this.activePath && this.activeView) {
                this.suppressDirtyTracking = true;
                this.activeView.setState(nextState);
                this.suppressDirtyTracking = false;
            }
        }
    }

    activate(path: string): void {
        if (this.findTab(path)) this.activePath = path;
    }

    async save(path?: string): Promise<void> {
        const tab = path ? this.findTab(path) : this.activeTab;
        if (!tab || tab.kind !== "text" || !tab.cmState) return;
        try {
            await writeTextFile(tab.path, tab.cmState.doc.toString());
            tab.isDirty = false;
            tab.changedOnDisk = false;
        } catch (err) {
            tab.error = describeFsError(err);
        }
    }

    /** Asks first if the tab has unsaved changes; otherwise closes right away. */
    requestClose(path: string): void {
        const tab = this.findTab(path);
        if (tab?.isDirty) {
            this.pendingClose = { path };
        } else {
            this.closeImmediately(path);
        }
    }

    async resolvePendingClose(action: "save" | "discard" | "cancel"): Promise<void> {
        const pending = this.pendingClose;
        this.pendingClose = null;
        if (!pending || action === "cancel") return;
        if (action === "save") await this.save(pending.path);
        this.closeImmediately(pending.path);
    }

    private closeImmediately(path: string): void {
        const idx = this.tabs.findIndex((t) => t.path === path);
        if (idx === -1) return;
        this.notifyLspClose(path);
        this.tabs.splice(idx, 1);
        if (this.activePath === path) {
            const next = this.tabs[idx] ?? this.tabs[idx - 1];
            this.activePath = next ? next.path : null;
        }
    }
}

export const editorStore = new EditorStore();
