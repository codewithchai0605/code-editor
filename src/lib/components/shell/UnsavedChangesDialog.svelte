<script lang="ts">
  import { editorStore } from "$lib/stores/editor.svelte";
  import { baseName } from "$lib/utils/path";

  const pending = $derived(editorStore.pendingClose);
  let dialogEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (pending) dialogEl?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") editorStore.resolvePendingClose("cancel");
  }
</script>

{#if pending}
  <div class="backdrop" role="presentation" onclick={() => editorStore.resolvePendingClose("cancel")} onkeydown={onKeydown}>
    <div
      class="dialog"
      bind:this={dialogEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="unsaved-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={onKeydown}
    >
      <h2 id="unsaved-title">Save changes?</h2>
      <p>"{baseName(pending.path)}" has unsaved changes. Save them before closing?</p>
      <div class="actions">
        <button class="btn" onclick={() => editorStore.resolvePendingClose("discard")}>Don't Save</button>
        <button class="btn" onclick={() => editorStore.resolvePendingClose("cancel")}>Cancel</button>
        <button class="btn primary" onclick={() => editorStore.resolvePendingClose("save")}>Save</button>
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
    width: min(400px, 100%);
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
    margin: 0 0 16px;
    font-size: 13px;
    color: var(--color-text-muted);
    line-height: 1.5;
    overflow-wrap: break-word;
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
  .btn.primary {
    background: var(--color-accent);
    color: #fff;
  }
</style>
