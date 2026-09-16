<script lang="ts">
  import { layout, type ActivityView } from "$lib/stores/layout.svelte";
  import { gitStore } from "$lib/stores/git.svelte";

  const topItems: { id: ActivityView; label: string }[] = [
    { id: "explorer", label: "Explorer" },
    { id: "search", label: "Search" },
    { id: "scm", label: "Source Control" },
    { id: "processes", label: "Task Manager" },
    { id: "extensions", label: "Extensions" },
  ];
</script>

{#snippet icon(id: ActivityView)}
  {#if id === "explorer"}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
      <path
        d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z"
      />
    </svg>
  {:else if id === "search"}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
      <circle cx="10.5" cy="10.5" r="6.5" />
      <line x1="20" y1="20" x2="15.2" y2="15.2" />
    </svg>
  {:else if id === "scm"}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
      <circle cx="6" cy="6" r="2.3" />
      <circle cx="6" cy="18" r="2.3" />
      <circle cx="18" cy="9" r="2.3" />
      <path d="M6 8.3v7.4" />
      <path d="M6 8.3c0 4 3 5.7 7 5.7h3" stroke-linecap="round" />
    </svg>
  {:else if id === "extensions"}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
      <rect x="3" y="3" width="7.5" height="7.5" rx="1.2" />
      <rect x="13.5" y="3" width="7.5" height="7.5" rx="1.2" />
      <rect x="8.25" y="13.5" width="7.5" height="7.5" rx="1.2" />
    </svg>
  {:else if id === "processes"}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
      <path
        d="M3 12h4l2-7 4 14 2-7h6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  {:else}
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
    >
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
        {#if item.id === "scm" && gitStore.totalChangeCount > 0}
          <span class="badge-count">{gitStore.totalChangeCount}</span>
        {/if}
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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--activitybar-width);
    height: 48px;
    cursor: pointer;
    border-left: 2px solid transparent;
    color: var(--color-text-muted);
  }
  .badge-count {
    position: absolute;
    bottom: 6px;
    right: 9px;
    min-width: 15px;
    height: 15px;
    padding: 0 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 999px;
    background: var(--color-accent);
    color: #fff;
    font-size: 9px;
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
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
