<script lang="ts">
    import { onMount } from "svelte";
    import { workspace } from "$lib/stores/workspace.svelte";
    import { editorStore } from "$lib/stores/editor.svelte";
    import { paletteState } from "$lib/stores/palette.svelte";
    import { listWorkspaceFiles } from "$lib/api/fs";
    import Palette from "./Palette.svelte";

    let files = $state<string[]>([]);
    let loading = $state(true);
    const root = workspace.rootPath;

    onMount(async () => {
        if (!root) {
            loading = false;
            return;
        }
        try {
            files = await listWorkspaceFiles(root);
        } catch (err) {
            console.error("Failed to index workspace files:", err);
        } finally {
            loading = false;
        }
    });

    /** Path relative to the workspace root, for both matching and display. */
    function relativeOf(path: string): string {
        return root && path.startsWith(root) ? path.slice(root.length).replace(/^[\\/]/, "") : path;
    }

    /** Splits a relative path into name + containing directory, for the
     *  two-line result display. Deliberately separate from utils/path.ts's
     *  parentOf(), which returns the whole string (not "") for a path with
     *  no separator — correct for its own callers, wrong for a root-level
     *  file's "directory" here. */
    function splitRelative(rel: string): { name: string; dir: string } {
        const idx = Math.max(rel.lastIndexOf("/"), rel.lastIndexOf("\\"));
        return idx === -1
            ? { name: rel, dir: "" }
            : { name: rel.slice(idx + 1), dir: rel.slice(0, idx) };
    }
</script>

<Palette
    placeholder="Search files by name..."
    items={files}
    getText={relativeOf}
    getKey={(path) => path}
    onSelect={(path) => editorStore.open(path)}
    onClose={() => paletteState.close()}
    emptyLabel={loading ? "Indexing files\u2026" : "No matching files"}
>
    {#snippet row(result)}
        {@const { name, dir } = splitRelative(relativeOf(result.item))}
        <div class="quick-open-row">
            <span class="name">{name}</span>
            {#if dir}<span class="dir">{dir}</span>{/if}
        </div>
    {/snippet}
</Palette>

<style>
    .quick-open-row {
        display: flex;
        align-items: baseline;
        gap: 10px;
        overflow: hidden;
    }
    .name {
        flex-shrink: 0;
        max-width: 60%;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 13px;
        color: var(--color-text);
    }
    .dir {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 12px;
        color: var(--color-text-faint);
    }
</style>
