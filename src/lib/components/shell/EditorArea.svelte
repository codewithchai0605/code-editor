<script lang="ts">
  import { editorStore } from "$lib/stores/editor.svelte";
  import { markdownPreviewStore } from "$lib/stores/markdownPreview.svelte";
  import { extensionOf } from "$lib/utils/fileIcons";
  import TabBar from "$lib/components/editor/TabBar.svelte";
  import TextEditor from "$lib/components/editor/TextEditor.svelte";
  import MarkdownPreview from "$lib/components/editor/MarkdownPreview.svelte";
  import ImageViewer from "$lib/components/editor/ImageViewer.svelte";
  import AudioViewer from "$lib/components/editor/AudioViewer.svelte";
  import BinaryViewer from "$lib/components/editor/BinaryViewer.svelte";
  import UnsavedChangesDialog from "./UnsavedChangesDialog.svelte";

  const tab = $derived(editorStore.activeTab);
  const isMarkdown = $derived(
    tab?.kind === "text" && ["md", "markdown"].includes(extensionOf(tab.path)),
  );
  const showPreview = $derived(isMarkdown && markdownPreviewStore.open && !!tab?.cmState);
  // Re-derives on every doc change (`tab.cmState` is replaced on each edit —
  // see editor.svelte.ts's syncDoc) — MarkdownPreview.svelte debounces the
  // actual re-render internally, so recomputing this string per keystroke
  // doesn't translate into a full re-parse per keystroke.
  const markdownSource = $derived(tab?.cmState?.doc.toString() ?? "");
</script>

<section class="editor-area">
  {#if editorStore.tabs.length > 0}
    <TabBar />
  {/if}

  {#if !tab}
    <div class="welcome">
      <h1>Code Editor</h1>
      <p>Open a folder, then click a file to start editing.</p>
      <ul class="hints">
        <li><kbd>Ctrl</kbd>+<kbd>P</kbd> Quick Open</li>
        <li><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> Command Palette</li>
        <li><kbd>Ctrl</kbd>+<kbd>`</kbd> Toggle Terminal</li>
        <li><kbd>Ctrl</kbd>+<kbd>S</kbd> Save</li>
        <li><kbd>Ctrl</kbd>+<kbd>B</kbd> Toggle Sidebar</li>
      </ul>
    </div>
  {:else if tab.loading}
    <div class="status-message">Loading&hellip;</div>
  {:else if tab.error}
    <div class="status-message error">
      <p>{tab.error}</p>
    </div>
  {:else if tab.kind === "text" && tab.cmState}
    {#if showPreview}
      <div class="split-view">
        <div class="split-pane"><TextEditor {tab} /></div>
        <div class="split-pane divider"><MarkdownPreview content={markdownSource} /></div>
      </div>
    {:else}
      <TextEditor {tab} />
    {/if}
  {:else if tab.kind === "image"}
    <ImageViewer path={tab.path} />
  {:else if tab.kind === "audio"}
    <AudioViewer path={tab.path} />
  {:else if tab.kind === "binary" && tab.binaryBytes}
    <BinaryViewer bytes={tab.binaryBytes} />
  {/if}
</section>

<UnsavedChangesDialog />

<style>
  .editor-area {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-bg);
    overflow: hidden;
  }
  .welcome {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 24px;
    text-align: center;
    color: var(--color-text-muted);
    overflow: auto;
  }
  .welcome h1 {
    margin: 0;
    font-size: clamp(20px, 3vw, 32px);
    font-weight: 600;
    color: var(--color-text);
  }
  .welcome p {
    margin: 0;
    font-size: 13px;
  }
  .hints {
    list-style: none;
    margin: 20px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 13px;
  }
  .hints li {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: center;
  }
  kbd {
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 1px 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text);
  }
  .status-message {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }
  .status-message.error p {
    color: #f48771;
  }
  .split-view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
  }
  .split-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .split-pane.divider {
    border-left: 1px solid var(--color-border);
  }
</style>
