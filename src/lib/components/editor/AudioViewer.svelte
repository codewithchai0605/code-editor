<script lang="ts">
    import { convertFileSrc } from "@tauri-apps/api/core";
    import { baseName } from "$lib/utils/path";

    interface Props {
        path: string;
    }
    let { path }: Props = $props();

    const src = $derived(convertFileSrc(path));
</script>

<div class="audio-viewer">
    <svg
        class="art"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--color-text-faint)"
        stroke-width="1.2"
    >
        <path d="M9 18V5l12-2v13" />
        <circle cx="6" cy="18" r="3" />
        <circle cx="18" cy="16" r="3" />
    </svg>
    <p class="filename">{baseName(path)}</p>
    <!-- svelte-ignore a11y_media_has_caption -->
    <audio controls {src}>Your browser doesn't support playing this file.</audio>
</div>

<style>
    .audio-viewer {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 16px;
        padding: 24px;
    }
    .art {
        width: 96px;
        height: 96px;
    }
    .filename {
        margin: 0;
        font-size: 13px;
        color: var(--color-text-muted);
    }
    audio {
        width: min(360px, 100%);
    }
</style>
