<script lang="ts">
  import { themeStore } from "$lib/stores/theme.svelte";
  import { paletteState } from "$lib/stores/palette.svelte";
  import Palette from "./Palette.svelte";
</script>

<Palette
  placeholder="Select a color theme..."
  items={themeStore.list}
  getText={(theme) => theme.name}
  getKey={(theme) => theme.id}
  onSelect={(theme) => themeStore.setTheme(theme.id)}
  onClose={() => paletteState.close()}
  emptyLabel="No matching themes"
>
  {#snippet row(result)}
    <div class="theme-row">
      <span class="swatch" style:background={result.item.colors.accent}></span>
      <span class="name">{result.item.name}</span>
      {#if result.item.id === themeStore.themeId}<span class="current">Current</span>{/if}
    </div>
  {/snippet}
</Palette>

<style>
  .theme-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .swatch {
    flex-shrink: 0;
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    color: var(--color-text);
  }
  .current {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--color-text-faint);
  }
</style>
