<script lang="ts">
    import { toastStore } from "$lib/stores/toast.svelte";
</script>

<div class="toast-host" aria-live="polite">
    {#each toastStore.toasts as toast (toast.id)}
        <div class="toast" data-kind={toast.kind}>
            <span class="dot"></span>
            <span class="message">{toast.message}</span>
            <button class="close" aria-label="Dismiss" onclick={() => toastStore.dismiss(toast.id)}
                >&times;</button
            >
        </div>
    {/each}
</div>

<style>
    .toast-host {
        position: fixed;
        bottom: 16px;
        right: 16px;
        z-index: 300;
        display: flex;
        flex-direction: column-reverse;
        gap: 8px;
        max-width: 340px;
        pointer-events: none;
    }
    .toast {
        pointer-events: auto;
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 9px 10px;
        border-radius: 6px;
        background: var(--color-bg-elevated);
        border: 1px solid var(--color-border);
        box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
        font-size: 12.5px;
        color: var(--color-text);
        animation: toast-in 0.15s ease-out;
    }
    .dot {
        flex-shrink: 0;
        width: 7px;
        height: 7px;
        border-radius: 50%;
    }
    .toast[data-kind="success"] .dot {
        background: #73c991;
    }
    .toast[data-kind="error"] .dot {
        background: #f48771;
    }
    .toast[data-kind="info"] .dot {
        background: var(--color-accent);
    }
    .message {
        flex: 1;
        min-width: 0;
        line-height: 1.4;
    }
    .close {
        flex-shrink: 0;
        color: var(--color-text-faint);
        font-size: 15px;
        line-height: 1;
        width: 16px;
        height: 16px;
    }
    .close:hover {
        color: var(--color-text);
    }
    @keyframes toast-in {
        from {
            opacity: 0;
            transform: translateY(6px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
</style>
