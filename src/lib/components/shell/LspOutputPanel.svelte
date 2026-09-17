<script lang="ts">
  import { onMount } from "svelte";
  import { lspStore } from "$lib/lsp/manager.svelte";
  import { baseName } from "$lib/utils/path";

  interface Props {
    root: string;
    languageId: string;
    onClose: () => void;
  }

  let { root, languageId, onClose }: Props = $props();

  let panelEl: HTMLDivElement | undefined = $state();
  let scrollEl: HTMLDivElement | undefined = $state();

  // Seeds the backlog once; live lines after that flow in reactively via
  // `lspStore.logs`, which the `lsp:log` listener updates directly — see
  // `getCachedLogs`'s doc comment.
  onMount(() => {
    void lspStore.getLogs(root, languageId);

    function onKeydown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  const lines = $derived(lspStore.getCachedLogs(root, languageId));

  // Keeps the view pinned to the bottom as new lines arrive, unless the
  // user has scrolled up to read something earlier.
  $effect(() => {
    void lines;
    if (
      scrollEl &&
      scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 40
    ) {
      scrollEl.scrollTop = scrollEl.scrollHeight;
    }
  });
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
  onkeydown={(e) => e.key === "Escape" && onClose()}
>
  <div class="panel" bind:this={panelEl} role="dialog" aria-label="LSP Output">
    <div class="header">
      <span class="title">{languageId} — {baseName(root) || root}</span>
      <button class="close" onclick={onClose} aria-label="Close">✕</button>
    </div>
    <div class="body" bind:this={scrollEl}>
      {#if lines.length === 0}
        <p class="empty">No output captured yet.</p>
      {:else}
        {#each lines as line, i (i)}
          <div class="line">{line}</div>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: rgba(0, 0, 0, 0.25);
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 40px;
  }
  .panel {
    width: min(720px, 92vw);
    height: min(420px, 60vh);
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-bottom: 1px solid var(--color-border);
    font-size: 12px;
    color: var(--color-text);
  }
  .title {
    font-weight: 600;
    text-transform: capitalize;
  }
  .close {
    color: var(--color-text-faint);
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }
  .close:hover {
    background: var(--color-accent);
    color: var(--color-text);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 10px;
    font-family: var(--font-mono, monospace);
    font-size: 12px;
  }
  .empty {
    color: var(--color-text-faint);
  }
  .line {
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.5;
    color: var(--color-text);
  }
</style>
