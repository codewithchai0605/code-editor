<script lang="ts">
    interface Props {
        /** "vertical" = a vertical bar the user drags left/right (sidebar edge). */
        orientation: "vertical" | "horizontal";
        onDrag: (deltaPx: number) => void;
    }

    let { orientation, onDrag }: Props = $props();

    let dragging = $state(false);

    function startDrag(e: PointerEvent) {
        dragging = true;
        const target = e.currentTarget as HTMLElement;
        target.setPointerCapture(e.pointerId);

        let lastPos = orientation === "vertical" ? e.clientX : e.clientY;

        function onMove(ev: PointerEvent) {
            const pos = orientation === "vertical" ? ev.clientX : ev.clientY;
            onDrag(pos - lastPos);
            lastPos = pos;
        }

        function stopDrag() {
            dragging = false;
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", stopDrag);
        }

        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", stopDrag);
    }

    const KEY_STEP = 20;

    function onKeydown(e: KeyboardEvent) {
        const forwardKey = orientation === "vertical" ? "ArrowRight" : "ArrowDown";
        const backwardKey = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
        if (e.key === forwardKey) {
            e.preventDefault();
            onDrag(KEY_STEP);
        } else if (e.key === backwardKey) {
            e.preventDefault();
            onDrag(-KEY_STEP);
        }
    }
</script>

<!-- This is the WAI-ARIA "window splitter" pattern (role=separator, focusable,
     arrow-key resizable) — the linter doesn't recognize it as interactive. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
    class="handle"
    class:vertical={orientation === "vertical"}
    class:horizontal={orientation === "horizontal"}
    class:dragging
    role="separator"
    aria-orientation={orientation}
    aria-label={orientation === "vertical" ? "Resize sidebar" : "Resize panel"}
    tabindex="0"
    onpointerdown={startDrag}
    onkeydown={onKeydown}
></div>

<style>
    .handle {
        flex-shrink: 0;
        position: relative;
        z-index: 5;
        background: transparent;
        touch-action: none;
    }
    .handle.vertical {
        width: 4px;
        margin: 0 -2px;
        cursor: col-resize;
    }
    .handle.horizontal {
        height: 4px;
        margin: -2px 0;
        cursor: row-resize;
    }
    .handle:hover,
    .handle.dragging {
        background: var(--color-accent);
        opacity: 0.5;
        transition: background 0.1s ease 100ms;
    }
</style>
