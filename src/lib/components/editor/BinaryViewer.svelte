<script lang="ts">
    import { onMount } from "svelte";

    interface Props {
        bytes: Uint8Array;
    }
    let { bytes }: Props = $props();

    const BYTES_PER_ROW = 16;
    const ROW_HEIGHT = 20;
    const OVERSCAN = 8;

    const totalRows = $derived(Math.max(1, Math.ceil(bytes.length / BYTES_PER_ROW)));

    let container: HTMLDivElement | undefined = $state();
    let scrollTop = $state(0);
    let viewportHeight = $state(400);

    function onScroll() {
        if (container) scrollTop = container.scrollTop;
    }

    onMount(() => {
        if (container) viewportHeight = container.clientHeight;
        const observer = new ResizeObserver(() => {
            if (container) viewportHeight = container.clientHeight;
        });
        if (container) observer.observe(container);
        return () => observer.disconnect();
    });

    const firstRow = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
    const visibleCount = $derived(Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2);
    const lastRow = $derived(Math.min(totalRows, firstRow + visibleCount));

    const rows = $derived.by(() => {
        const out: { offset: number; hex: string; ascii: string }[] = [];
        for (let r = firstRow; r < lastRow; r++) {
            const start = r * BYTES_PER_ROW;
            const end = Math.min(bytes.length, start + BYTES_PER_ROW);
            if (start >= end) break;
            let hex = "";
            let ascii = "";
            for (let i = start; i < end; i++) {
                const b = bytes[i];
                hex += b.toString(16).padStart(2, "0") + " ";
                ascii += b >= 32 && b < 127 ? String.fromCharCode(b) : ".";
            }
            out.push({ offset: start, hex: hex.trimEnd(), ascii });
        }
        return out;
    });
</script>

<div class="binary-viewer">
    <p class="notice">
        Binary file &mdash; showing the first {bytes.length.toLocaleString()} bytes as hex.
    </p>
    <div class="hex-scroll" bind:this={container} onscroll={onScroll}>
        <div class="hex-spacer" style:height="{totalRows * ROW_HEIGHT}px">
            {#each rows as row (row.offset)}
                <div class="hex-row" style:top="{(row.offset / BYTES_PER_ROW) * ROW_HEIGHT}px">
                    <span class="offset">{row.offset.toString(16).padStart(8, "0")}</span>
                    <span class="hex">{row.hex}</span>
                    <span class="ascii">{row.ascii}</span>
                </div>
            {/each}
        </div>
    </div>
</div>

<style>
    .binary-viewer {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
    .notice {
        flex-shrink: 0;
        margin: 0;
        padding: 8px 16px;
        font-size: 12px;
        color: var(--color-text-muted);
        border-bottom: 1px solid var(--color-border);
    }
    .hex-scroll {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        position: relative;
    }
    .hex-spacer {
        position: relative;
    }
    .hex-row {
        position: absolute;
        left: 0;
        right: 0;
        height: 20px;
        display: flex;
        align-items: center;
        gap: 16px;
        padding: 0 16px;
        white-space: pre;
        font-family: var(--font-mono);
        font-size: 12px;
    }
    .offset {
        color: var(--color-text-faint);
        flex-shrink: 0;
    }
    .hex {
        color: var(--color-text);
        letter-spacing: 1px;
        flex-shrink: 0;
        width: 336px;
    }
    .ascii {
        color: var(--color-text-muted);
    }
</style>
