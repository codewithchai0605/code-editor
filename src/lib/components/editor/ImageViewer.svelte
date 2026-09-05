<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

  interface Props {
    path: string;
  }
  let { path }: Props = $props();

  const src = $derived(convertFileSrc(path));
  let zoom = $state(1);

  function zoomIn() {
    zoom = Math.min(8, zoom * 1.25);
  }
  function zoomOut() {
    zoom = Math.max(0.1, zoom / 1.25);
  }
  function resetZoom() {
    zoom = 1;
  }
</script>

<div class="image-viewer">
  <div class="toolbar">
    <button onclick={zoomOut} aria-label="Zoom out">&minus;</button>
    <button class="zoom-label" onclick={resetZoom} aria-label="Reset zoom">{Math.round(zoom * 100)}%</button>
    <button onclick={zoomIn} aria-label="Zoom in">+</button>
  </div>
  <div class="canvas">
    <img {src} alt="" style:transform="scale({zoom})" />
  </div>
</div>

<style>
  .image-viewer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 6px;
    border-bottom: 1px solid var(--color-border);
  }
  .toolbar button {
    min-width: 28px;
    height: 24px;
    padding: 0 8px;
    border-radius: 4px;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .toolbar button:hover {
    background: var(--color-bg-hover);
    color: var(--color-text);
  }
  .zoom-label {
    min-width: 48px;
  }
  .canvas {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    background:
      linear-gradient(45deg, #2a2a2a 25%, transparent 25%),
      linear-gradient(-45deg, #2a2a2a 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #2a2a2a 75%),
      linear-gradient(-45deg, transparent 75%, #2a2a2a 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
    background-color: var(--color-bg);
  }
  img {
    max-width: none;
    transition: transform 0.1s ease;
  }
</style>
