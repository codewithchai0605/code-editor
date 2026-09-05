<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { layout } from "$lib/stores/layout.svelte";

  let version = $state("");

  onMount(async () => {
    try {
      version = await invoke<string>("app_version");
    } catch (err) {
      console.error("Failed to read app version:", err);
    }
  });
</script>

<footer class="status-bar">
  <div class="segment">
    <button class="chip" onclick={() => layout.toggleSidebar()}>Explorer</button>
    <button class="chip" onclick={() => layout.togglePanel()}>Terminal</button>
  </div>
  <div class="segment">
    {#if !layout.isNarrow}
      <span class="chip static">UTF-8</span>
      <span class="chip static">LF</span>
    {/if}
    {#if version}
      <span class="chip static">v{version}</span>
    {/if}
  </div>
</footer>

<style>
  .status-bar {
    flex-shrink: 0;
    height: var(--statusbar-height);
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--color-bg-statusbar);
    color: var(--color-statusbar-text);
    font-size: 12px;
    padding: 0 8px;
    user-select: none;
  }
  .segment {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    overflow: hidden;
  }
  .chip {
    cursor: pointer;
    height: var(--statusbar-height);
    padding: 0 8px;
    display: flex;
    align-items: center;
    border-radius: 2px;
    white-space: nowrap;
    color: inherit;
  }
  .chip:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .chip.static {
    cursor: default;
  }
  .chip.static:hover {
    background: none;
  }
</style>
