<script lang="ts">
  import { paletteState } from "$lib/stores/palette.svelte";
  import QuickOpenPalette from "./QuickOpenPalette.svelte";
  import CommandPalette from "./CommandPalette.svelte";
  import ThemePalette from "./ThemePalette.svelte";
  import SymbolPalette from "./SymbolPalette.svelte";
</script>

{#if paletteState.active === "quickOpen"}
  <QuickOpenPalette />
{:else if paletteState.active === "commandPalette"}
  <CommandPalette />
{:else if paletteState.active === "theme"}
  <ThemePalette />
{:else if paletteState.active === "documentSymbols"}
  <SymbolPalette scope="document" />
{:else if paletteState.active === "workspaceSymbols"}
  <SymbolPalette scope="workspace" />
{/if}
