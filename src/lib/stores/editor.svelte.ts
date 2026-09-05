import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { readTextFile, writeTextFile, readBinaryPreview } from "$lib/api/fs";
import { isFsError } from "$lib/types/fs";
import { guessFileKind, type FileKind } from "$lib/editor/fileKind";
import { loadLanguage } from "$lib/editor/languages";
import { baseExtensions } from "$lib/editor/setup";

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
}

function describeError(err: unknown): string {
  if (isFsError(err)) return err.message ?? "Something went wrong.";
  return err instanceof Error ? err.message : String(err);
}

class EditorStore {
  tabs = $state<OpenTab[]>([]);
  activePath = $state<string | null>(null);
  pendingClose = $state<{ path: string } | null>(null);

  /** True for the duration of a programmatic view.setState() call (tab
   *  switch), so the per-tab updateListener doesn't mistake the resulting
   *  update for a real edit and mark a freshly-activated tab dirty. See
   *  TextEditor.svelte. */
  suppressDirtyTracking = false;

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
        ...baseExtensions(() => this.save(path)),
        ...(language ? [language] : []),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return;
          this.syncDoc(path, update.state);
          if (!this.suppressDirtyTracking) this.markDirty(path);
        }),
      ];
      this.updateTab(path, {
        cmState: EditorState.create({ doc: content, extensions }),
        loading: false,
      });
    } catch (err) {
      if (isFsError(err) && err.kind === "unsupported") {
        await this.loadAsBinary(path);
      } else {
        this.updateTab(path, { loading: false, error: describeError(err) });
      }
    }
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

  private markDirty(path: string): void {
    const tab = this.findTab(path);
    if (tab && !tab.isDirty) tab.isDirty = true;
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
    this.tabs.splice(idx, 1);
    if (this.activePath === path) {
      const next = this.tabs[idx] ?? this.tabs[idx - 1];
      this.activePath = next ? next.path : null;
    }
  }
}

export const editorStore = new EditorStore();