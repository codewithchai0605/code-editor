<script lang="ts">
    import { onMount } from "svelte";
    import { workspace } from "$lib/stores/workspace.svelte";
    import { validateEntryPath } from "$lib/utils/validateName";

    interface Props {
        depth: number;
        isDir: boolean;
    }
    let { depth, isDir }: Props = $props();

    let value = $state("");
    let error = $state<string | null>(null);
    let inputEl: HTMLInputElement | undefined = $state();
    let submitted = false;

    onMount(() => inputEl?.focus());

    async function submit() {
        if (submitted) return;
        const err = validateEntryPath(value);
        if (err) {
            error = err;
            return;
        }
        submitted = true;
        await workspace.confirmCreate(value.trim());
    }

    function onKeydown(e: KeyboardEvent) {
        e.stopPropagation();
        if (e.key === "Enter") submit();
        else if (e.key === "Escape") workspace.cancelCreate();
    }

    function onBlur() {
        // A blur with no name typed is a cancel, not an error nag.
        if (!value.trim()) workspace.cancelCreate();
        else submit();
    }
</script>

<div class="row" style:padding-left="{depth * 14 + 8}px">
    <span class="chevron-spacer"></span>
    <svg
        class="icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--color-text-muted)"
        stroke-width="1.6"
    >
        {#if isDir}
            <path
                d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z"
            />
        {:else}
            <path d="M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z" />
            <path d="M14 3v4a1 1 0 0 0 1 1h4" />
        {/if}
    </svg>
    <input
        class="entry-input"
        bind:this={inputEl}
        bind:value
        placeholder={isDir ? "Folder name (e.g. nested/folder)" : "File name (e.g. lib/file.ts)"}
        onkeydown={onKeydown}
        onblur={onBlur}
    />
</div>
{#if error}
    <p class="entry-error" style:padding-left="{depth * 14 + 30}px">{error}</p>
{/if}

<style>
    .row {
        display: flex;
        align-items: center;
        gap: 4px;
        height: 22px;
    }
    .chevron-spacer {
        width: 16px;
        flex-shrink: 0;
    }
    .icon {
        width: 16px;
        height: 16px;
        flex-shrink: 0;
    }
    .entry-input {
        flex: 1;
        min-width: 0;
        background: var(--color-bg);
        border: 1px solid var(--color-focus-ring);
        border-radius: 2px;
        padding: 0 4px;
        font-size: 13px;
        height: 20px;
    }
    .entry-error {
        margin: 2px 0 0;
        font-size: 11px;
        color: #f48771;
    }
</style>
