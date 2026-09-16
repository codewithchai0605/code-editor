import { EditorSelection, EditorState, Text, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { readTextFile, writeTextFile, readBinaryPreview } from "$lib/api/fs";
import { isFsError } from "$lib/types/fs";
import { guessFileKind, type FileKind } from "$lib/editor/fileKind";
import { loadLanguage } from "$lib/editor/languages";
import { baseExtensions, themeCompartment } from "$lib/editor/setup";
import { themeStore } from "./theme.svelte";
import { workspace } from "./workspace.svelte";
import { lspLanguageIdFor } from "$lib/lsp/languageId";
import { pathToFileUri } from "$lib/lsp/uri";
import { lspDidOpen, lspDidChange, lspDidClose } from "$lib/api/lsp";
import { clearDiagnosticsForUri } from "$lib/lsp/diagnostics";
import { lspStore } from "$lib/lsp/manager.svelte";
import { gitStore } from "./git.svelte";
import { gitFileBaseline } from "$lib/api/git"
import { setGitBaseline } from "$lib/editor/git-gutter";

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
}

function describeError(err: unknown): string {
  if (isFsError(err)) return err.message ?? "Something went wrong.";
  return err instanceof Error ? err.message : String(err);
}

/** `Text.of` splits on `\n` only, which would leave a stray `\r` at the
 *  end of every line of a CRLF file's baseline — harmless for the diff
 *  itself (both sides would carry it), but this normalizes so a
 *  CRLF-in-git/LF-in-buffer (or vice versa) mismatch doesn't manufacture
 *  a "changed" line out of nothing but a line-ending difference. */
function textFromString(value: string): Text {
  return Text.of(value.length ? value.split(/\r\n|\r|\n/) : [""]);
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

  constructor() {
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

  private findTab(path: string): OpenTab | undefined {
    return this.tabs.find((t) => t.path === path);
  }

  async open(path: string): Promise<void> {
    const existing = this.findTab(path);
    if (existing) {
      this.activePath = path;
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
          };
        }),
      ];
      this.updateTab(path, {
        cmState: EditorState.create({ doc: content, extensions }),
        loading: false,
        lspVersion: 1
      });
      this.notifyLspOpen(path, content);
      void this.refreshGitBaseline(path);
    } catch (err) {
      if (isFsError(err) && err.kind === "unsupported") {
        await this.loadAsBinary(path);
      } else {
        this.updateTab(path, { loading: false, error: describeError(err) });
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
  async openAndReveal(path: string, line: number, colStart: number, colEnd: number): Promise<void> {
    // Captured *before* `open()` runs: if `path` is already the active tab,
    // `open()`'s early-return path touches nothing about it, so neither of
    // TextEditor's own hooks below has anything to react to and this method
    // must apply the reveal itself. Every other case (a brand-new tab, or
    // switching in from a different tab) genuinely changes what TextEditor
    // is rendering, so its hooks pick it up on their own — applying it here
    // too, before Svelte's reactivity has actually flushed, would risk
    // landing before that tab-swap `setState` and then being clobbered by
    // it (setState replaces the selection along with everything else).
    const alreadyActive = this.activePath === path && this.findTab(path) !== undefined;
    this.pendingReveal = { path, line, colStart, colEnd };
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
    const from = Math.min(lineInfo.from + target.colStart, lineInfo.to);
    const to = Math.min(lineInfo.from + target.colEnd, lineInfo.to);

    view.dispatch({
      selection: EditorSelection.range(from, to),
      effects: EditorView.scrollIntoView(from, { y: "center" }),
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
      this.updateTab(path, { loading: false, error: describeError(err) });
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

  /** Fire-and-forget notifications to the LSP backend — errors are logged,
   *  not surfaced to the user, since a missing/crashed language server
   *  should degrade to "no diagnostics/completions for this file", never
   *  block editing. `workspace.rootPath` is guaranteed non-null by the
   *  time any tab can be open (there's no way to open a file without a
   *  workspace first), but the check is kept explicit rather than
   *  asserted, since "silently skip" is the right failure mode here too. */

  private notifyLspOpen(path: string, text: string, attemptsLeft = 3): void {
    const languageId = lspLanguageIdFor(path);
    const root = workspace.rootPath;
    if (!languageId || !root) return;

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
      const root = workspace.rootPath;
      if (!tab?.cmState || !root) return;
      const version = (tab.lspVersion ?? 1) + 1;
      tab.lspVersion = version;
      lspDidChange(root, languageId, pathToFileUri(path), tab.cmState.doc.toString(), version).catch((err) =>
        console.error(`LSP didChange failed for ${path}:`, err),
      );
    }, LSP_CHANGE_DEBOUNCE_MS);

    this.lspChangeTimers.set(path, timer);
  }

  private notifyLspClose(path: string): void {
    const languageId = lspLanguageIdFor(path);
    const root = workspace.rootPath;
    const timer = this.lspChangeTimers.get(path);
    if (timer) {
      clearTimeout(timer);
      this.lspChangeTimers.delete(path);
    }
    clearDiagnosticsForUri(pathToFileUri(path));
    if (!languageId || !root) return;
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
    } catch (err) {
      tab.error = describeError(err);
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