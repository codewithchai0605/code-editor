<script lang="ts">
  import { paletteState } from "$lib/stores/palette.svelte";
  import QuickOpenPalette from "./QuickOpenPalette.svelte";
  import CommandPalette from "./CommandPalette.svelte";
</script>

{#if paletteState.active === "quickOpen"}
  <QuickOpenPalette />
{:else if paletteState.active === "commandPalette"}
  <CommandPalette />
{/if}
