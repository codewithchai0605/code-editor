<script lang="ts">
  import { onMount } from "svelte";
  import type { MenuItem } from "$lib/types/menu";

  interface Props {
    x: number;
    y: number;
    items: MenuItem[];
    onClose: () => void;
  }

  let { x, y, items, onClose }: Props = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  let pos = $state({ x: 0, y: 0 });

  onMount(() => {
    // Initial position
    pos = { x, y };

    // Boundary detection - ensure menu stays on screen
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      // Adjust X if menu would go off right edge
      if (x + rect.width > viewportWidth) {
        pos.x = Math.max(4, viewportWidth - rect.width - 8);
      }

      // Adjust Y if menu would go off bottom edge
      if (y + rect.height > viewportHeight) {
        pos.y = Math.max(4, viewportHeight - rect.height - 8);
      }
    }

    function onPointerDown(e: PointerEvent) {
      if (menuEl && !menuEl.contains(e.target as Node)) onClose();
    }

    function onKeydown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }

    // Close on scroll or resize to prevent menu from floating away
    function onScrollOrResize() {
      onClose();
    }

    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeydown);
    window.addEventListener("scroll", onScrollOrResize, true);
    window.addEventListener("resize", onScrollOrResize);

    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("scroll", onScrollOrResize, true);
      window.removeEventListener("resize", onScrollOrResize);
    };
  });

  function run(item: MenuItem) {
    if (item.disabled) return;
    onClose();
    // Use queueMicrotask to ensure menu closes before action runs
    queueMicrotask(() => item.action());
  }
</script>

<!-- Prevent native context menu while ours is open -->
<svelte:window on:contextmenu|preventDefault />

<div
  class="menu"
  role="menu"
  bind:this={menuEl}
  style:left="{pos.x}px"
  style:top="{pos.y}px"
>
  {#each items as item (item.label)}
    {#if item.separatorAbove}
      <div class="separator" role="separator"></div>
    {/if}
    <button
      class="item"
      class:danger={item.danger}
      class:disabled={item.disabled}
      role="menuitem"
      disabled={item.disabled}
      onclick={() => run(item)}
    >
      <span class="label">{item.label}</span>
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 9999;
    min-width: 180px;
    padding: 4px;
    background: var(--color-bg-elevated, #1e1e1e);
    border: 1px solid var(--color-border, #454545);
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    font-size: 13px;
    color: var(--color-text, #cccccc);
    animation: fadeIn 0.08s ease-out;
    backdrop-filter: blur(8px);
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: scale(0.98) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 24px;
    padding: 0 8px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: inherit;
    font-size: 13px;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    outline: none;
    transition:
      background-color 0.1s,
      color 0.1s;
  }

  .item:hover:not(.disabled) {
    background: var(--color-bg-hover, #2a2d2e);
    color: var(--color-text, #ffffff);
  }

  .item.danger:hover:not(.disabled) {
    background: var(--color-danger-bg, #5a1d1d);
    color: var(--color-danger, #f48771);
  }

  .item.disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .separator {
    height: 1px;
    margin: 4px 0;
    background: var(--color-border, #454545);
  }
</style>
