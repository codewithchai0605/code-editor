<script lang="ts">
  import { onDestroy } from "svelte";
  import { MergeView } from "@codemirror/merge";
  import { EditorState } from "@codemirror/state";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import { foldGutter } from "@codemirror/language";
  import { loadLanguage } from "$lib/editor/languages";
  import { buildCmTheme } from "$lib/editor/themeEngine";
  import { themeStore } from "$lib/stores/theme.svelte";
  import type { DiffTab } from "$lib/stores/diffTab.svelte";

  interface Props {
    tab: DiffTab;
  }
  let { tab }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let mergeView: MergeView | undefined;

  async function build() {
    mergeView?.destroy();
    mergeView = undefined;
    if (!container || !tab.content) return;

    // Same language loader the real editor uses (so a diff on a .rs file
    // gets Rust highlighting, etc.) — deliberately *not* reusing
    // setup.ts's `baseExtensions()` for the rest, since that bundles
    // autocomplete/history/Emmet/LSP-diagnostics wiring meant for a live,
    // editable buffer, none of which applies to a read-only diff pane.
    const language = await loadLanguage(tab.path);
    const shared = [
      lineNumbers(),
      foldGutter(),
      buildCmTheme(themeStore.theme),
      EditorView.editable.of(false),
      EditorState.readOnly.of(true),
      ...(language ? [language] : []),
    ];

    mergeView = new MergeView({
      a: { doc: tab.content.oldText, extensions: shared },
      b: { doc: tab.content.newText, extensions: shared },
      parent: container,
      gutter: true,
      highlightChanges: true,
      collapseUnchanged: { margin: 3, minSize: 4 },
    });
  }

  $effect(() => {
    // Explicit synchronous reads of every dependency right here (rather
    // than only inside `build()`, which is async) — guarantees all three
    // are tracked regardless of exactly where in `build()`'s pre-`await`
    // prefix they'd otherwise first be touched.
    const currentContainer = container;
    const content = tab.content;
    const theme = themeStore.theme;
    void currentContainer;
    void content;
    void theme;
    void build();
  });

  onDestroy(() => mergeView?.destroy());
</script>

<div class="diff-view">
  {#if tab.loading}
    <p class="status">Loading diff…</p>
  {:else if tab.error}
    <p class="status error">{tab.error}</p>
  {:else}
    <div class="labels">
      <span>{tab.content?.oldLabel}</span>
      <span>{tab.content?.newLabel}</span>
    </div>
    <div class="merge-container" bind:this={container}></div>
  {/if}
</div>

<style>
  .diff-view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .status {
    padding: 16px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
  .status.error {
    color: #f48771;
  }
  .labels {
    flex-shrink: 0;
    display: flex;
    border-bottom: 1px solid var(--color-border);
  }
  .labels span {
    flex: 1;
    min-width: 0;
    padding: 4px 12px;
    font-size: 11px;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .merge-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  /* @codemirror/merge's own default styles already make `.cm-mergeViewEditors`
     a flex row with `.cm-mergeViewEditor` (one per side) growing to fill it —
     verified against the package's actual bundled CSS rather than assumed.
     The only thing genuinely missing for our layout is giving the outer
     `.cm-mergeView` a real height to stretch within, since its own default
     is just `overflow-y: auto` on a block sized to content. */
  .merge-container :global(.cm-mergeView) {
    height: 100%;
  }
</style>
