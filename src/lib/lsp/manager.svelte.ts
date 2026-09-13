import { listen } from "@tauri-apps/api/event";
import { forceLinting } from "@codemirror/lint";
import { lspStart } from "$lib/api/lsp";
import { setDiagnosticsForUri } from "./diagnostics";
import { pathToFileUri } from "./uri";
import { editorStore } from "$lib/stores/editor.svelte";
import type { LspDiagnosticsEvent, LspServerStatusKind, LspStatusEvent } from "./types";

interface StatusEntry {
  status: LspServerStatusKind;
  error: string | null;
}

class LspStore {
  /** languageId -> status, read by LspStatusBar.svelte. */
  statuses = $state<Map<string, StatusEntry>>(new Map());

  private listening = false;

  /** Starts listening for LSP events (once, app-wide) and starts the
   *  TypeScript server for `root`. Safe to call every time a workspace
   *  opens — Rust's `lsp_start` is itself idempotent per (root,
   *  languageId), and the event listeners are only ever registered once. */
  async startForWorkspace(root: string): Promise<void> {
    await this.ensureListening();
    try {
      await lspStart(root);
    } catch (err) {
      console.error("Failed to start TypeScript language server:", err);
    }
  }

  private async ensureListening(): Promise<void> {
    if (this.listening) return;
    this.listening = true;

    await listen<LspStatusEvent>("lsp:status", (event) => {
      const { languageId, status } = event.payload;
      const next = new Map(this.statuses);
      next.set(languageId, { status: status.kind, error: status.message ?? null });
      this.statuses = next;
    });

    await listen<LspDiagnosticsEvent>("lsp:diagnostics", (event) => {
      const { uri, diagnostics } = event.payload.params;
      setDiagnosticsForUri(uri, diagnostics);

      // Refresh immediately if the affected file is the one currently on
      // screen — otherwise the (correct) diagnostics just sit in the
      // cache and are picked up automatically next time that tab is
      // activated, since `linter()` runs once when its extension first
      // becomes part of the active state.
      const activeTab = editorStore.activeTab;
      const view = editorStore.view;
      if (activeTab && view && pathToFileUri(activeTab.path) === uri) {
        forceLinting(view);
      }
    });
  }
}

export const lspStore = new LspStore();
