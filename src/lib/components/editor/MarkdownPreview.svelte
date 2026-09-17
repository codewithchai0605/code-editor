<script lang="ts">
    import { renderMarkdownToSafeHtml } from "$lib/markdown/render";

    interface Props {
        content: string;
    }
    let { content }: Props = $props();

    let html = $state("");

    // Debounced rather than rendered on every keystroke — markdown-it +
    // DOMPurify are fast, but there's no reason to re-run the full parse on
    // every character when a 100ms lag behind typing is imperceptible for a
    // preview pane (unlike the editor itself, which must never lag input).
    $effect(() => {
        const text = content;
        const handle = setTimeout(() => {
            html = renderMarkdownToSafeHtml(text);
        }, 150);
        return () => clearTimeout(handle);
    });
</script>

<div class="markdown-preview">
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html html}
</div>

<style>
    .markdown-preview {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        padding: 16px 24px;
        color: var(--color-text);
        font-size: 14px;
        line-height: 1.6;
    }
    .markdown-preview :global(h1),
    .markdown-preview :global(h2),
    .markdown-preview :global(h3),
    .markdown-preview :global(h4) {
        color: var(--color-text);
        border-bottom: 1px solid var(--color-border);
        padding-bottom: 6px;
        margin: 20px 0 12px;
    }
    .markdown-preview :global(h1) {
        font-size: 1.8em;
    }
    .markdown-preview :global(h2) {
        font-size: 1.4em;
    }
    .markdown-preview :global(p) {
        margin: 10px 0;
    }
    .markdown-preview :global(a) {
        color: var(--color-accent);
    }
    .markdown-preview :global(code) {
        font-family: var(--font-mono);
        font-size: 0.9em;
        background: var(--color-bg-elevated);
        border-radius: 3px;
        padding: 1px 5px;
    }
    .markdown-preview :global(pre) {
        background: var(--color-bg-elevated);
        border: 1px solid var(--color-border);
        border-radius: 6px;
        padding: 12px 14px;
        overflow-x: auto;
    }
    .markdown-preview :global(pre code) {
        background: none;
        padding: 0;
    }
    .markdown-preview :global(blockquote) {
        margin: 10px 0;
        padding: 2px 14px;
        border-left: 3px solid var(--color-border);
        color: var(--color-text-muted);
    }
    .markdown-preview :global(table) {
        border-collapse: collapse;
        margin: 10px 0;
    }
    .markdown-preview :global(th),
    .markdown-preview :global(td) {
        border: 1px solid var(--color-border);
        padding: 4px 10px;
    }
    .markdown-preview :global(img) {
        max-width: 100%;
    }
</style>
