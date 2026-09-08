<script lang="ts">
  import { layout } from "$lib/stores/layout.svelte";
  import FileTree from "$lib/components/explorer/FileTree.svelte";
  import TaskManager from "$lib/components/sidebar/TaskManager.svelte";

  const titles: Record<string, string> = {
    explorer: "Explorer",
    search: "Search",
    processes: "Task Manager",
    extensions: "Extensions",
    settings: "Settings",
  };
</script>

<aside class="sidebar" style:width="{layout.sidebarWidth}px">
  <header class="sidebar-header">
    <span>{titles[layout.activeView]}</span>
  </header>
  <div class="sidebar-content" class:explorer-mode={layout.activeView === "explorer"}>
    {#if layout.activeView === "explorer"}
      <FileTree />
    {:else if layout.activeView === "search"}
      <p class="placeholder">Workspace search is coming in a later phase.</p>
    {:else if layout.activeView === "processes"}
      <TaskManager />
    {:else if layout.activeView === "extensions"}
      <p class="placeholder">Installed native extensions will be listed here once the plugin host lands.</p>
    {:else}
      <p class="placeholder">A settings.json-backed settings UI is coming in a later phase.</p>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    flex-shrink: 0;
    min-width: 170px;
    max-width: 600px;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-sidebar);
    border-right: 1px solid var(--color-border);
    overflow: hidden;
  }
  .sidebar-header {
    flex-shrink: 0;
    height: 35px;
    display: flex;
    align-items: center;
    padding: 0 16px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-text-muted);
    user-select: none;
  }
  .sidebar-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 16px 16px;
  }
  .sidebar-content.explorer-mode {
    overflow: hidden;
  }
  .placeholder {
    color: var(--color-text-muted);
    font-size: 13px;
    line-height: 1.6;
  }
</style>
