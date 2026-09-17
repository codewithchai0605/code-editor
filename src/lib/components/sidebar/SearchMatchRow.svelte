<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { Decoration, EditorView } from "@codemirror/view";
    import { EditorState } from "@codemirror/state";
    import { loadLanguage } from "$lib/editor/languages";
    import { themeStore } from "$lib/stores/theme.svelte";
    import type { SearchMatch } from "$lib/types/search";

    let {
        path,
        match,
        onNavigate,
    }: {
        path: string;
        match: SearchMatch;
        onNavigate: () => void;
    } = $props();

    let rootEl: HTMLDivElement | undefined = $state();
    let container: HTMLDivElement | undefined = $state();
    let view = $state<EditorView | null>(null);

    let observer: IntersectionObserver | null = null;

    // Tracks whether this row currently wants a CodeMirror instance.
    // This is important because loadLanguage() is asynchronous and the
    // row can leave the viewport while the language is still loading.
    let wantsMount = false;
    const PREVIEW_HEIGHT = 18;

    /**
     * Compact CodeMirror theme for a single-line search preview.
     *
     * CodeMirror normally manages its own vertical dimensions. Here we
     * deliberately constrain every relevant layer to exactly one line.
     */
    const compactTheme = EditorView.theme({
        "&": {
            fontSize: "12px",
            lineHeight: `${PREVIEW_HEIGHT}px`,
            backgroundColor: "transparent",
            height: `${PREVIEW_HEIGHT}px`,
            minHeight: `${PREVIEW_HEIGHT}px`,
            maxHeight: `${PREVIEW_HEIGHT}px`,
        },

        ".cm-scroller": {
            height: `${PREVIEW_HEIGHT}px`,
            minHeight: `${PREVIEW_HEIGHT}px`,
            maxHeight: `${PREVIEW_HEIGHT}px`,
            overflowX: "auto",
            overflowY: "hidden",
            scrollbarWidth: "none",
        },

        ".cm-content": {
            padding: "0",
            height: `${PREVIEW_HEIGHT}px`,
            minHeight: `${PREVIEW_HEIGHT}px`,
            lineHeight: `${PREVIEW_HEIGHT}px`,
            fontFamily: "var(--font-mono)",
        },

        ".cm-line": {
            padding: "0",
            height: `${PREVIEW_HEIGHT}px`,
            minHeight: `${PREVIEW_HEIGHT}px`,
            lineHeight: `${PREVIEW_HEIGHT}px`,
            whiteSpace: "pre",
        },

        ".cm-cursor": {
            display: "none",
        },

        ".cm-selectionBackground": {
            display: "none",
        },

        ".cm-gutters": {
            display: "none",
        },
    });

    function highlightExtension(from: number, to: number) {
        return EditorView.decorations.of(
            Decoration.set([
                Decoration.mark({
                    class: "cm-searchMatch cm-searchMatch-selected",
                }).range(from, to),
            ]),
        );
    }

    async function mountEditor(): Promise<void> {
        if (view || !container || !wantsMount) return;

        const language = await loadLanguage(path);
        // The row may have left the viewport while the language was loading.
        if (!wantsMount || view || !container) return;
        const doc = match.lineText;
        // Clamp the match positions defensively.
        const from = Math.max(0, Math.min(match.colStart, doc.length));
        const to = Math.max(from, Math.min(match.colEnd, doc.length));

        const editor = new EditorView({
            parent: container,

            state: EditorState.create({
                doc,

                extensions: [
                    EditorView.editable.of(false),
                    EditorState.readOnly.of(true),

                    compactTheme,
                    themeStore.cmExtension,

                    highlightExtension(from, to),

                    ...(language ? [language] : []),
                ],
            }),
        });

        view = editor;

        /**
         * Wait for CodeMirror to perform its initial layout before scrolling.
         *
         * Without requestAnimationFrame(), CodeMirror may not yet know the
         * correct width of the editor, causing scrollIntoView() to calculate
         * the horizontal position incorrectly.
         */
        requestAnimationFrame(() => {
            if (view !== editor || !wantsMount) return;

            editor.dispatch({
                effects: EditorView.scrollIntoView(from, {
                    x: "center",
                    y: "nearest",
                }),
            });
        });
    }

    function disposeEditor(): void {
        if (!view) return;

        view.destroy();
        view = null;
        // Ensure no stale CodeMirror DOM remains when the row is recycled.
        container?.replaceChildren();
    }

    function handleKeydown(e: KeyboardEvent): void {
        if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onNavigate();
        }
    }

    onMount(() => {
        observer = new IntersectionObserver(
            (entries) => {
                const entry = entries[0];
                if (!entry) return;
                wantsMount = entry.isIntersecting;
                if (wantsMount) {
                    void mountEditor();
                } else {
                    disposeEditor();
                }
            },
            {
                rootMargin: "150px 0px",
                threshold: 0,
            },
        );

        if (rootEl) {
            observer.observe(rootEl);
        }
    });

    onDestroy(() => {
        wantsMount = false;

        observer?.disconnect();
        disposeEditor();
    });
</script>

<div
    class="match-row"
    bind:this={rootEl}
    role="button"
    tabindex="0"
    title={`Line ${match.line}`}
    onclick={onNavigate}
    onkeydown={handleKeydown}
>
    <span class="match-line-no">
        {match.line}
    </span>

    <span class="match-preview">
        {#if !view}
            <span class="match-preview-fallback">
                {match.lineText}
            </span>
        {/if}

        <div class="match-preview-cm" class:visible={view !== null} bind:this={container}></div>
    </span>
</div>

<style>
    .match-row {
        display: flex;
        align-items: center;
        gap: 8px;
        height: 22px;
        min-height: 22px;
        padding: 2px 2px 2px 34px;
        box-sizing: border-box;
        border-radius: 3px;
        cursor: pointer;
        min-width: 0;
    }

    .match-row:hover,
    .match-row:focus-visible {
        background: rgba(255, 255, 255, 0.06);
        outline: none;
    }

    .match-line-no {
        flex: 0 0 28px;
        width: 28px;
        text-align: right;
        color: var(--color-text-faint);
        font-size: 11px;
        line-height: 18px;
        font-variant-numeric: tabular-nums;
    }

    .match-preview {
        position: relative;
        flex: 1;
        width: 0;
        min-width: 0;
        height: 18px;
        min-height: 18px;
        max-height: 18px;
        display: block;
        overflow: hidden;
    }

    /*
   * Plain-text fallback while CodeMirror/language is loading.
   */
    .match-preview-fallback {
        position: absolute;
        inset: 0;
        display: block;
        width: 100%;
        height: 18px;
        overflow: hidden;
        white-space: pre;
        font-family: var(--font-mono);
        font-size: 12px;
        line-height: 18px;
        color: var(--color-text-muted);
        pointer-events: none;
    }

    /*
   * CodeMirror preview.
   *
   * It occupies exactly the same space as the fallback.
   */
    .match-preview-cm {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 18px;
        overflow: hidden;
        opacity: 0;
        transition: opacity 80ms ease;
    }

    .match-preview-cm.visible {
        opacity: 1;
    }

    /*
   * The row itself handles clicks.
   * CodeMirror is purely a rendering surface.
   */
    .match-preview-cm :global(.cm-editor) {
        width: 100%;
        height: 18px !important;
        min-height: 18px !important;
        max-height: 18px !important;
        overflow: hidden;
        background: transparent;
        pointer-events: none;
    }

    /*
   * Horizontal scrolling is intentionally enabled here.
   *
   * mountEditor() calls scrollIntoView() on the search match so a match
   * near the end of a very long line becomes visible.
   */
    .match-preview-cm :global(.cm-scroller) {
        width: 100%;
        height: 18px !important;
        min-height: 18px !important;
        max-height: 18px !important;
        overflow-x: auto !important;
        overflow-y: hidden !important;
        scrollbar-width: none;
    }

    .match-preview-cm :global(.cm-scroller::-webkit-scrollbar) {
        display: none;
    }

    .match-preview-cm :global(.cm-content) {
        padding: 0 !important;
        height: 18px !important;
        min-height: 18px !important;
        line-height: 18px !important;
    }

    .match-preview-cm :global(.cm-line) {
        height: 18px !important;
        min-height: 18px !important;
        padding: 0 !important;
        line-height: 18px !important;
        white-space: pre !important;
    }

    .match-preview-cm :global(.cm-gutters) {
        display: none;
    }
</style>
