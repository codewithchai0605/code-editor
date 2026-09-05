<script lang="ts">
  import { workspace } from "$lib/stores/workspace.svelte";

  const conflict = $derived(workspace.pendingConflict);
  let dialogEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (conflict) dialogEl?.focus();
  });

  function itemLabel(count: number, mode: "copy" | "move"): string {
    const verb = mode === "copy" ? "copying" : "moving";
    return count === 1 ? `${verb} 1 item` : `${verb} ${count} items`;
  }

  function onBackdropKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") workspace.resolveConflict(false);
  }
</script>

{#if conflict}
  <div
    class="backdrop"
    role="presentation"
    onclick={() => workspace.resolveConflict(false)}
    onkeydown={onBackdropKeydown}
  >
    <div
      class="dialog"
      bind:this={dialogEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="conflict-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={onBackdropKeydown}
    >
      <h2 id="conflict-title">Replace existing items?</h2>
      <p>
        {itemLabel(conflict.sources.length, conflict.mode)} to this location will replace
        {conflict.conflictingPaths.length === 1 ? "an item that" : `${conflict.conflictingPaths.length} items that`}
        already exist{conflict.conflictingPaths.length === 1 ? "s" : ""} there:
      </p>
      <ul class="paths">
        {#each conflict.conflictingPaths.slice(0, 6) as path (path)}
          <li>{path}</li>
        {/each}
        {#if conflict.conflictingPaths.length > 6}
          <li>&hellip; and {conflict.conflictingPaths.length - 6} more</li>
        {/if}
      </ul>
      <div class="actions">
        <button class="btn" onclick={() => workspace.resolveConflict(false)}>Cancel</button>
        <button class="btn danger" onclick={() => workspace.resolveConflict(true)}>Replace</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }
  .dialog {
    width: min(420px, 100%);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 20px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  p {
    margin: 0 0 8px;
    font-size: 13px;
    color: var(--color-text-muted);
    line-height: 1.5;
  }
  .paths {
    margin: 0 0 16px;
    padding: 8px 10px;
    max-height: 140px;
    overflow-y: auto;
    background: var(--color-bg);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text-muted);
    list-style: none;
  }
  .paths li {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn {
    padding: 6px 14px;
    font-size: 13px;
    border-radius: 4px;
    background: var(--color-bg-hover);
    cursor: pointer;
  }
  .btn.danger {
    background: #a1260d;
    color: #fff;
  }
</style>
