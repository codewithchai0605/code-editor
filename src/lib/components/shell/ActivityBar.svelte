<script lang="ts">
  import { layout, type ActivityView } from "$lib/stores/layout.svelte";

  const topItems: { id: ActivityView; label: string }[] = [
    { id: "explorer", label: "Explorer" },
    { id: "search", label: "Search" },
    { id: "extensions", label: "Extensions" },
  ];
</script>

{#snippet icon(id: ActivityView)}
  {#if id === "explorer"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
      <path d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z" />
    </svg>
  {:else if id === "search"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
      <circle cx="10.5" cy="10.5" r="6.5" />
      <line x1="20" y1="20" x2="15.2" y2="15.2" />
    </svg>
  {:else if id === "extensions"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
      <rect x="3" y="3" width="7.5" height="7.5" rx="1.2" />
      <rect x="13.5" y="3" width="7.5" height="7.5" rx="1.2" />
      <rect x="8.25" y="13.5" width="7.5" height="7.5" rx="1.2" />
    </svg>
  {:else}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
      <circle cx="12" cy="12" r="3" />
      <circle cx="12" cy="12" r="8" stroke-dasharray="3 3.2" />
    </svg>
  {/if}
{/snippet}

<nav class="activity-bar" aria-label="Primary">
  <div class="items">
    {#each topItems as item (item.id)}
      <button
        class="item"
        class:active={layout.activeView === item.id && layout.sidebarOpen}
        title={item.label}
        aria-label={item.label}
        aria-pressed={layout.activeView === item.id && layout.sidebarOpen}
        onclick={() => layout.setActiveView(item.id)}
      >
        {@render icon(item.id)}
      </button>
    {/each}
  </div>

  <div class="items">
    <button
      class="item"
      class:active={layout.activeView === "settings" && layout.sidebarOpen}
      title="Settings"
      aria-label="Settings"
      aria-pressed={layout.activeView === "settings" && layout.sidebarOpen}
      onclick={() => layout.setActiveView("settings")}
    >
      {@render icon("settings")}
    </button>
  </div>
</nav>

<style>
  .activity-bar {
    flex-shrink: 0;
    width: var(--activitybar-width);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    background: var(--color-bg-activitybar);
    border-right: 1px solid var(--color-border);
    user-select: none;
  }
  .items {
    display: flex;
    flex-direction: column;
  }
  .item {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--activitybar-width);
    height: 48px;
    cursor: pointer;
    border-left: 2px solid transparent;
    color: var(--color-text-muted);
  }
  .item svg {
    width: 22px;
    height: 22px;
  }
  .item:hover {
    color: var(--color-text);
  }
  .item.active {
    color: var(--color-text);
    border-left-color: var(--color-accent);
  }
</style>
