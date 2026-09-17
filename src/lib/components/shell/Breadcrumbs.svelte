<script lang="ts">
    import { workspace } from "$lib/stores/workspace.svelte";
    import { layout } from "$lib/stores/layout.svelte";

    interface Props {
        path: string;
    }
    let { path }: Props = $props();

    // Segments between the workspace root and the file itself — clicking one
    // reveals + selects that folder in the Explorer, matching VS Code's own
    // breadcrumb behavior. Falls back to showing the whole path as one
    // segment if it isn't actually inside the open workspace for some reason
    // (shouldn't normally happen, but a bare crumb beats a broken one).
    const segments = $derived.by(() => {
        const root = workspace.rootPath;
        if (!root || !path.startsWith(root)) return [{ name: path, fullPath: path, isDir: false }];

        const rest = path.slice(root.length).replace(/^[\\/]+/, "");
        const parts = rest.split(/[\\/]+/).filter(Boolean);
        const crumbs: { name: string; fullPath: string; isDir: boolean }[] = [];
        let current = root;
        for (let i = 0; i < parts.length; i++) {
            current = `${current}${current.endsWith("/") || current.endsWith("\\") ? "" : "/"}${parts[i]}`;
            crumbs.push({
                name: parts[i],
                fullPath: current,
                isDir: i < parts.length - 1,
            });
        }
        return crumbs;
    });

    function reveal(fullPath: string, isDir: boolean) {
        if (isDir) workspace.expanded.add(fullPath);
        workspace.select(fullPath);
        layout.setActiveView("explorer");
    }
</script>

{#if segments.length > 0}
    <div class="breadcrumbs" aria-label="File path">
        {#each segments as segment, i (segment.fullPath)}
            {#if i > 0}<span class="sep">/</span>{/if}
            <button class="crumb" onclick={() => reveal(segment.fullPath, segment.isDir)}
                >{segment.name}</button
            >
        {/each}
    </div>
{/if}

<style>
    .breadcrumbs {
        flex-shrink: 0;
        display: flex;
        align-items: center;
        height: 24px;
        padding: 0 10px;
        gap: 2px;
        overflow-x: auto;
        overflow-y: hidden;
        border-bottom: 1px solid var(--color-border);
        font-size: 12px;
        color: var(--color-text-muted);
        white-space: nowrap;
    }
    .crumb {
        flex-shrink: 0;
        padding: 1px 4px;
        border-radius: 3px;
        color: inherit;
    }
    .crumb:hover {
        background: var(--color-bg-hover);
        color: var(--color-text);
    }
    .sep {
        flex-shrink: 0;
        color: var(--color-text-faint);
    }
</style>
