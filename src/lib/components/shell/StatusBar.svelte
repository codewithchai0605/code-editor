<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { layout } from "$lib/stores/layout.svelte";
  import { gitStore } from "$lib/stores/git.svelte";

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
    {#if gitStore.isRepo && gitStore.branch}
      <button
        class="chip"
        title={gitStore.changedCount
          ? `${gitStore.changedCount} changed file${gitStore.changedCount === 1 ? "" : "s"}`
          : "No changes"}
        onclick={() => layout.setActiveView("explorer")}
      >
        <svg class="branch-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
          <circle cx="4.5" cy="3.5" r="1.5" />
          <circle cx="4.5" cy="12.5" r="1.5" />
          <circle cx="11.5" cy="7.5" r="1.5" />
          <path d="M4.5 5v6" />
          <path d="M4.5 8c0-2.5 1.5-4 4-4h1.5" stroke-linecap="round" />
        </svg>
        {gitStore.branch}{#if gitStore.ahead || gitStore.behind}
          &nbsp;{gitStore.ahead ? `↑${gitStore.ahead}` : ""}{gitStore.behind ? `↓${gitStore.behind}` : ""}
        {/if}
        {#if gitStore.changedCount}
          &nbsp;<span class="dirty-dot">●</span>
        {/if}
      </button>
    {/if}
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
  .branch-icon {
    width: 13px;
    height: 13px;
    margin-right: 4px;
  }
  .dirty-dot {
    color: #e2c08d;
    font-size: 8px;
  }
</style>
