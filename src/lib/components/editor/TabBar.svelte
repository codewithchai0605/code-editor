<script lang="ts">
  import { editorStore } from "$lib/stores/editor.svelte";
  import { baseName } from "$lib/utils/path";
  import { fileIconColor } from "$lib/utils/fileIcons";

  function onAuxClick(e: MouseEvent, path: string) {
    if (e.button === 1) {
      e.preventDefault();
      editorStore.requestClose(path);
    }
  }

  function onKeydown(e: KeyboardEvent, path: string) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      editorStore.activate(path);
    }
  }
</script>

<div class="tab-bar" role="tablist" aria-label="Open files">
  {#each editorStore.tabs as tab (tab.path)}
    <div
      class="tab"
      class:active={tab.path === editorStore.activePath}
      role="tab"
      aria-selected={tab.path === editorStore.activePath}
      tabindex="0"
      title={tab.path}
      onclick={() => editorStore.activate(tab.path)}
      onkeydown={(e) => onKeydown(e, tab.path)}
      onauxclick={(e) => onAuxClick(e, tab.path)}
    >
      <span class="dot" style:background={tab.kind === "text" ? fileIconColor(tab.path) : "var(--color-text-faint)"}></span>
      <span class="name">{baseName(tab.path)}</span>
      {#if tab.isDirty}<span class="dirty" aria-hidden="true"></span>{/if}
      <button
        class="close"
        aria-label="Close {baseName(tab.path)}"
        onclick={(e) => {
          e.stopPropagation();
          editorStore.requestClose(tab.path);
        }}
      >
        &times;
      </button>
    </div>
  {/each}
</div>

<style>
  .tab-bar {
    flex-shrink: 0;
    height: var(--tabbar-height);
    display: flex;
    overflow-x: auto;
    overflow-y: hidden;
    border-bottom: 1px solid var(--color-border);
  }
  .tab {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px 0 10px;
    max-width: 200px;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg);
    font-size: 13px;
    color: var(--color-text-muted);
    cursor: pointer;
    user-select: none;
  }
  .tab:hover {
    background: var(--color-bg-hover);
  }
  .tab.active {
    color: var(--color-text);
    background: var(--color-bg);
    box-shadow: inset 0 -1px 0 var(--color-accent);
  }
  .dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dirty {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-text-muted);
  }
  .close {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    line-height: 1;
    font-size: 15px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .close:hover {
    background: var(--color-bg-hover);
    color: var(--color-text);
  }
</style>
