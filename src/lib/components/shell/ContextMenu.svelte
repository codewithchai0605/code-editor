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
    pos = { x, y };
    // Clamp so the menu never renders partly off-screen, regardless of
    // where the click happened or how small the window is.
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const maxX = window.innerWidth - rect.width - 4;
      const maxY = window.innerHeight - rect.height - 4;
      pos = { x: Math.min(x, Math.max(4, maxX)), y: Math.min(y, Math.max(4, maxY)) };
    }

    function onPointerDown(e: PointerEvent) {
      if (menuEl && !menuEl.contains(e.target as Node)) onClose();
    }
    function onKeydown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeydown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeydown);
    };
  });

  function run(item: MenuItem) {
    if (item.disabled) return;
    onClose();
    item.action();
  }
</script>

<div class="menu" role="menu" bind:this={menuEl} style:left="{pos.x}px" style:top="{pos.y}px">
  {#each items as item (item.label)}
    {#if item.separatorAbove}
      <div class="separator"></div>
    {/if}
    <button
      class="item"
      class:danger={item.danger}
      disabled={item.disabled}
      role="menuitem"
      onclick={() => run(item)}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 180px;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 4px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
  .item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    font-size: 13px;
    border-radius: 4px;
    color: var(--color-text);
    cursor: pointer;
  }
  .item:hover:not(:disabled) {
    background: var(--color-accent);
  }
  .item:disabled {
    color: var(--color-text-faint);
    cursor: default;
  }
  .item.danger:hover:not(:disabled) {
    background: #a1260d;
  }
  .separator {
    height: 1px;
    margin: 4px 6px;
    background: var(--color-border);
  }
</style>
