<script lang="ts">
    import { gitStore } from "$lib/stores/git.svelte";
    import { baseName } from "$lib/utils/path";

    const pending = $derived(gitStore.pendingDiscard);
    let dialogEl: HTMLDivElement | undefined = $state();

    $effect(() => {
        if (pending) dialogEl?.focus();
    });

    function onBackdropKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") gitStore.resolveDiscard(false);
    }
</script>

{#if pending}
    <div
        class="backdrop"
        role="presentation"
        onclick={() => gitStore.resolveDiscard(false)}
        onkeydown={onBackdropKeydown}
    >
        <div
            class="dialog"
            bind:this={dialogEl}
            role="dialog"
            aria-modal="true"
            aria-labelledby="discard-title"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={onBackdropKeydown}
        >
            <h2 id="discard-title">
                {pending.path
                    ? `Discard changes in ${baseName(pending.path)}?`
                    : `Discard ${pending.count} change${pending.count === 1 ? "" : "s"}?`}
            </h2>
            <p>
                This can't be undone. Any uncommitted changes {pending.path
                    ? "to this file"
                    : "to these files"} will be permanently lost.
            </p>
            <div class="actions">
                <button class="btn" onclick={() => gitStore.resolveDiscard(false)}>Cancel</button>
                <button class="btn danger" onclick={() => gitStore.resolveDiscard(true)}
                    >Discard</button
                >
            </div>
        </div>
    </div>
{/if}

<style>
    .backdrop {
        position: fixed;
        inset: 0;
        z-index: 200;
        background: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 16px;
    }
    .dialog {
        width: min(420px, 100%);
        display: flex;
        flex-direction: column;
        background: var(--color-bg-elevated);
        border: 1px solid var(--color-border);
        border-radius: 8px;
        padding: 20px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    }
    h2 {
        margin: 0 0 8px;
        font-size: 15px;
    }
    p {
        margin: 0 0 16px;
        font-size: 13px;
        color: var(--color-text-muted);
        line-height: 1.5;
    }
    .actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
    }
    .btn {
        padding: 6px 14px;
        font-size: 13px;
        border-radius: 4px;
        background: var(--color-bg-hover);
        cursor: pointer;
    }
    .btn.danger {
        background: #a1260d;
        color: #fff;
    }
</style>
