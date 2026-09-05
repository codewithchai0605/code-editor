<script lang="ts">
  import { layout } from "$lib/stores/layout.svelte";
</script>

<section class="panel" style:height="{layout.panelHeight}px">
  <header class="panel-header">
    <span class="panel-tab active">Terminal</span>
  </header>
  <div class="panel-content">
    <p class="placeholder">The integrated terminal (PTY + xterm.js) lands in a later phase.</p>
  </div>
</section>

<style>
  .panel {
    flex-shrink: 0;
    min-height: 100px;
    display: flex;
    flex-direction: column;
    background: var(--color-bg);
    border-top: 1px solid var(--color-border);
    overflow: hidden;
  }
  .panel-header {
    flex-shrink: 0;
    height: 30px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    border-bottom: 1px solid var(--color-border);
    user-select: none;
  }
  .panel-tab {
    font-size: 12px;
    padding: 4px 0;
    color: var(--color-text-muted);
    border-bottom: 1px solid transparent;
  }
  .panel-tab.active {
    color: var(--color-text);
    border-bottom-color: var(--color-accent);
  }
  .panel-content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px;
  }
  .placeholder {
    margin: 0;
    font-size: 13px;
    color: var(--color-text-muted);
  }
</style>
