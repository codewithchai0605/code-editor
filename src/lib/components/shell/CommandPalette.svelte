<script lang="ts">
    import { commandRegistry } from "$lib/commands/registry.svelte";
    import { paletteState } from "$lib/stores/palette.svelte";
    import Palette from "./Palette.svelte";
</script>

<Palette
    placeholder="Type a command..."
    items={commandRegistry.commands}
    getText={(command) => command.title}
    getKey={(command) => command.id}
    onSelect={(command) => command.run()}
    onClose={() => paletteState.close()}
    emptyLabel="No matching commands"
>
    {#snippet row(result)}
        <div class="command-row">
            <span class="title">{result.item.title}</span>
            {#if result.item.keybinding}<span class="keybinding">{result.item.keybinding}</span
                >{/if}
        </div>
    {/snippet}
</Palette>

<style>
    .command-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }
    .title {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 13px;
        color: var(--color-text);
    }
    .keybinding {
        flex-shrink: 0;
        font-size: 11px;
        color: var(--color-text-faint);
    }
</style>
