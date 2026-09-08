<script lang="ts">
  import { onMount } from "svelte";
  import { searchPanelBridge } from "$lib/editor/search.svelte";

  let searchInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    searchInput?.focus();
    searchInput?.select();
  });

  function setTerm(value: string) {
    searchPanelBridge.term = value;
    searchPanelBridge.runQuery();
  }
  function setReplacement(value: string) {
    searchPanelBridge.replacement = value;
    searchPanelBridge.runQuery();
  }
  function toggle(field: "caseSensitive" | "regexp" | "wholeWord") {
    searchPanelBridge[field] = !searchPanelBridge[field];
    searchPanelBridge.runQuery();
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.altKey) searchPanelBridge.selectAll();
      else if (e.shiftKey) searchPanelBridge.prev();
      else searchPanelBridge.next();
    } else if (e.key === "Escape") {
      e.preventDefault();
      searchPanelBridge.close();
    }
  }
  function onReplaceKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.altKey || e.metaKey || e.ctrlKey) searchPanelBridge.doReplaceAll();
      else searchPanelBridge.doReplaceNext();
    } else if (e.key === "Escape") {
      e.preventDefault();
      searchPanelBridge.close();
    }
  }

  const status = $derived.by(() => {
    if (!searchPanelBridge.term) return "";
    if (searchPanelBridge.query && !searchPanelBridge.query.valid) return "Invalid pattern";
    if (searchPanelBridge.matchCount === 0) return "No results";
    if (searchPanelBridge.matchCount == null) return "";
    return `${searchPanelBridge.currentMatch ?? 0} of ${searchPanelBridge.matchCount}`;
  });
</script>

<div class="search-panel" role="group" aria-label="Find and replace">
  <div class="row">
    <div class="field">
      <input
        bind:this={searchInput}
        class="text-input"
        type="text"
        placeholder="Find"
        aria-label="Find"
        value={searchPanelBridge.term}
        oninput={(e) => setTerm(e.currentTarget.value)}
        onkeydown={onSearchKeydown}
      />
      <div class="field-toggles">
        <button
          type="button"
          class="toggle"
          class:active={searchPanelBridge.caseSensitive}
          aria-pressed={searchPanelBridge.caseSensitive}
          title="Match Case"
          onclick={() => toggle("caseSensitive")}
        >Aa</button>
        <button
          type="button"
          class="toggle"
          class:active={searchPanelBridge.wholeWord}
          aria-pressed={searchPanelBridge.wholeWord}
          title="Match Whole Word"
          onclick={() => toggle("wholeWord")}
        >ab</button>
        <button
          type="button"
          class="toggle"
          class:active={searchPanelBridge.regexp}
          aria-pressed={searchPanelBridge.regexp}
          title="Use Regular Expression"
          onclick={() => toggle("regexp")}
        >.*</button>
      </div>
    </div>

    <span class="status" class:muted={!status}>{status || "\u00A0"}</span>

    <button
      type="button"
      class="icon-btn"
      title="Previous Match (Shift+Enter)"
      aria-label="Previous match"
      disabled={!searchPanelBridge.matchCount}
      onclick={() => searchPanelBridge.prev()}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M6 15l6-6 6 6" /></svg>
    </button>
    <button
      type="button"
      class="icon-btn"
      title="Next Match (Enter)"
      aria-label="Next match"
      disabled={!searchPanelBridge.matchCount}
      onclick={() => searchPanelBridge.next()}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M6 9l6 6 6-6" /></svg>
    </button>
    <button
      type="button"
      class="icon-btn"
      title="Close (Esc)"
      aria-label="Close find and replace"
      onclick={() => searchPanelBridge.close()}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <div class="row">
    <div class="field replace-field">
      <input
        class="text-input"
        type="text"
        placeholder="Replace"
        aria-label="Replace"
        value={searchPanelBridge.replacement}
        oninput={(e) => setReplacement(e.currentTarget.value)}
        onkeydown={onReplaceKeydown}
      />
    </div>
    <button
      type="button"
      class="text-btn"
      disabled={!searchPanelBridge.matchCount}
      title="Replace (Enter)"
      onclick={() => searchPanelBridge.doReplaceNext()}
    >Replace</button>
    <button
      type="button"
      class="text-btn"
      disabled={!searchPanelBridge.matchCount}
      title="Replace All (Ctrl+Enter)"
      onclick={() => searchPanelBridge.doReplaceAll()}
    >Replace All</button>
  </div>
</div>

<style>
  .search-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    background: var(--color-bg-elevated);
    border-bottom: 1px solid var(--color-border);
    font-size: 12px;
    user-select: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1 1 260px;
    max-width: 420px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 0 4px;
  }
  .field:focus-within {
    border-color: var(--color-focus-ring);
  }
  .text-input {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 4px;
    background: transparent;
    font-size: 12px;
    font-family: var(--font-mono);
  }
  .text-input::placeholder {
    color: var(--color-text-faint);
  }
  .field-toggles {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .toggle {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-family: var(--font-mono);
    border-radius: 3px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .toggle:hover {
    color: var(--color-text);
    background: var(--color-bg-hover);
  }
  .toggle.active {
    color: var(--color-text);
    background: rgba(0, 122, 204, 0.35);
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
  }
  .status {
    flex-shrink: 0;
    min-width: 72px;
    font-size: 11px;
    color: var(--color-text-muted);
    text-align: right;
  }
  .status.muted {
    color: transparent;
  }
  .icon-btn {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn svg {
    width: 16px;
    height: 16px;
  }
  .icon-btn:hover:not(:disabled) {
    color: var(--color-text);
    background: var(--color-bg-hover);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .replace-field {
    max-width: 420px;
  }
  .text-btn {
    flex-shrink: 0;
    height: 24px;
    padding: 0 10px;
    font-size: 11px;
    border-radius: 4px;
    color: var(--color-text-muted);
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    cursor: pointer;
    white-space: nowrap;
  }
  .text-btn:hover:not(:disabled) {
    color: var(--color-text);
    border-color: var(--color-text-faint);
  }
  .text-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
