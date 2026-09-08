<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { editorStore, type OpenTab } from "$lib/stores/editor.svelte";

  interface Props {
    tab: OpenTab;
  }
  let { tab }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | undefined;

  onMount(() => {
    if (container && tab.cmState) {
      view = new EditorView({ state: tab.cmState, parent: container });
      editorStore.registerActiveView(view);
      view.focus();
    }
    return () => {
      editorStore.registerActiveView(null);
      view?.destroy();
    };
  });

  // Runs whenever `tab` changes — including switching between two text
  // tabs, since Svelte keeps this component instance alive across that
  // (only non-text tabs unmount it). Swapping state instead of recreating
  // the view is what preserves each document's undo history, selection,
  // and keeps memory flat regardless of tab count.
  $effect(() => {
    if (view && tab.cmState && view.state !== tab.cmState) {
      // See stores/editor.svelte.ts: this stops the state swap itself from
      // being mistaken for a real edit by the per-tab updateListener.
      editorStore.suppressDirtyTracking = true;
      view.setState(tab.cmState);
      editorStore.suppressDirtyTracking = false;
      view.focus();
    }
  });
</script>

<div class="cm-host" bind:this={container}></div>

<style>
  .cm-host {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .cm-host :global(.cm-editor) {
    height: 100%;
  }
</style>
