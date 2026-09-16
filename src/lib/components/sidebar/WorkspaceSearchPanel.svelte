<script lang="ts">
  import { onMount } from "svelte";
  import { workspaceSearchStore as store } from "$lib/stores/workspaceSearch.svelte";
  import { workspace } from "$lib/stores/workspace.svelte";
  import { baseName, parentOf } from "$lib/utils/path";
  import { fileIconColor } from "$lib/utils/fileIcons";
  import SearchMatchRow from "./SearchMatchRow.svelte";

  let searchInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    searchInput?.focus();
  });

  function relativeDir(path: string): string {
    const root = workspace.rootPath;
    const dir = parentOf(path);
    if (!root || dir === root) return "";
    const stripped = dir.startsWith(root) ? dir.slice(root.length) : dir;
    return stripped.replace(/^[\\/]+/, "");
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      store.searchImmediately();
    } else if (e.key === "Escape" && store.term) {
      e.preventDefault();
      store.clearQuery();
    }
  }

  function onReplaceKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && store.fileCount > 0) {
      e.preventDefault();
      store.requestReplaceAll();
    }
  }

  const matchWord = $derived(store.totalMatches === 1 ? "result" : "results");
  const fileWord = $derived(store.fileCount === 1 ? "file" : "files");
</script>

<div class="wsearch-panel">
  <div class="controls">
    <div class="row">
      <button
        type="button"
        class="replace-toggle"
        class:collapsed={!store.showReplace}
        title={store.showReplace ? "Hide Replace" : "Toggle Replace"}
        aria-label="Toggle replace"
        onclick={() => store.toggleShowReplace()}
      >
        <svg
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
        >
          <path d="M6 4l4 4-4 4" />
        </svg>
      </button>
      <div class="field">
        <input
          bind:this={searchInput}
          class="text-input"
          type="text"
          placeholder="Search"
          aria-label="Search across files"
          value={store.term}
          oninput={(e) => store.setTerm(e.currentTarget.value)}
          onkeydown={onSearchKeydown}
        />
        {#if store.term}
          <button
            type="button"
            class="clear-btn"
            title="Clear"
            aria-label="Clear search"
            onclick={() => store.clearQuery()}
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"><path d="M6 6l12 12M18 6L6 18" /></svg
            >
          </button>
        {/if}
        <div class="field-toggles">
          <button
            type="button"
            class="toggle"
            class:active={store.caseSensitive}
            aria-pressed={store.caseSensitive}
            title="Match Case"
            onclick={() => store.toggleCaseSensitive()}>Aa</button
          >
          <button
            type="button"
            class="toggle"
            class:active={store.wholeWord}
            aria-pressed={store.wholeWord}
            title="Match Whole Word"
            onclick={() => store.toggleWholeWord()}>ab</button
          >
          <button
            type="button"
            class="toggle"
            class:active={store.regex}
            aria-pressed={store.regex}
            title="Use Regular Expression"
            onclick={() => store.toggleRegex()}>.*</button
          >
        </div>
      </div>
    </div>

    {#if store.showReplace}
      <div class="row replace-row">
        <div class="field replace-field">
          <input
            class="text-input"
            type="text"
            placeholder="Replace"
            aria-label="Replace"
            value={store.replaceTerm}
            oninput={(e) => store.setReplaceTerm(e.currentTarget.value)}
            onkeydown={onReplaceKeydown}
          />
        </div>
        {#if store.confirmingReplaceAll}
          <button
            type="button"
            class="text-btn danger"
            onclick={() => store.confirmReplaceAll()}>Confirm</button
          >
          <button
            type="button"
            class="text-btn"
            onclick={() => store.cancelReplaceAll()}>Cancel</button
          >
        {:else}
          <button
            type="button"
            class="text-btn"
            disabled={store.fileCount === 0 || store.isReplacing}
            title="Replace All (Ctrl+Enter)"
            onclick={() => store.requestReplaceAll()}
            >{store.isReplacing ? "Replacing…" : "Replace All"}</button
          >
        {/if}
      </div>
    {/if}

    <button
      type="button"
      class="filters-toggle"
      onclick={() => store.toggleShowFilters()}
    >
      <svg
        class="chevron"
        class:collapsed={!store.showFilters}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
      >
        <path d="M6 4l4 4-4 4" />
      </svg>
      files to include/exclude
    </button>
    {#if store.showFilters}
      <div class="row">
        <input
          class="filter-input"
          type="text"
          placeholder="files to include, e.g. **/*.ts, src/**"
          aria-label="Files to include"
          value={store.include}
          oninput={(e) => store.setInclude(e.currentTarget.value)}
        />
      </div>
      <div class="row">
        <input
          class="filter-input"
          type="text"
          placeholder="files to exclude, e.g. **/*.spec.ts"
          aria-label="Files to exclude"
          value={store.exclude}
          oninput={(e) => store.setExclude(e.currentTarget.value)}
        />
      </div>
    {/if}
  </div>

  {#if store.lastError}
    <p class="error-banner">{store.lastError}</p>
  {/if}

  {#if store.replaceSummary}
    <div class="replace-summary">
      <span>{store.replaceSummary}</span>
      <button
        type="button"
        class="icon-btn"
        title="Dismiss"
        onclick={() => (store.replaceSummary = null)}>×</button
      >
    </div>
  {/if}

  <p class="summary">
    {#if store.status === "searching"}
      Searching…
    {:else if store.hasQuery && store.fileCount > 0}
      {store.totalMatches}
      {matchWord} in {store.fileCount}
      {fileWord}{store.truncated
        ? " — showing partial results, narrow your search"
        : ""}
    {:else if store.hasQuery && store.status === "done"}
      No results found.
    {:else}
      Search across every file in the workspace.
    {/if}
  </p>

  <div class="results">
    {#each store.orderedFiles as path (path)}
      {@const matches = store.results.get(path) ?? []}
      {@const collapsed = store.collapsedFiles.has(path)}
      <div class="file-group">
        <button
          type="button"
          class="file-header"
          onclick={() => store.toggleCollapse(path)}
        >
          <svg
            class="chevron"
            class:collapsed
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
          >
            <path d="M6 4l4 4-4 4" />
          </svg>
          <svg
            class="file-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke={fileIconColor(baseName(path))}
            stroke-width="1.6"
          >
            <path
              d="M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"
            />
            <path d="M14 3v4a1 1 0 0 0 1 1h4" />
          </svg>
          <span class="file-name">{baseName(path)}</span>
          {#if relativeDir(path)}
            <span class="file-dir">{relativeDir(path)}</span>
          {/if}
          <span class="match-count">{matches.length}</span>
        </button>
        {#if !collapsed}
          {#each matches as match, i (i)}
            <SearchMatchRow
              {path}
              {match}
              onNavigate={() => store.navigate(path, match)}
            />
          {/each}
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .wsearch-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    font-size: 12px;
  }
  .controls {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .replace-toggle {
    flex-shrink: 0;
    width: 16px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
  }
  .replace-toggle svg {
    width: 10px;
    height: 10px;
    transition: transform 0.1s ease;
  }
  .replace-toggle.collapsed svg {
    transform: rotate(0deg);
  }
  .replace-toggle:not(.collapsed) svg {
    transform: rotate(90deg);
  }
  .replace-toggle:hover {
    color: var(--color-text);
  }
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 0 4px;
  }
  .field:focus-within {
    border-color: var(--color-focus-ring);
  }
  .replace-field {
    margin-left: 20px;
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
  .clear-btn {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    border-radius: 3px;
  }
  .clear-btn svg {
    width: 12px;
    height: 12px;
  }
  .clear-btn:hover {
    color: var(--color-text);
    background: var(--color-bg-hover);
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
  .text-btn {
    flex-shrink: 0;
    height: 26px;
    padding: 0 10px;
    font-size: 11px;
    border-radius: 4px;
    color: var(--color-text-muted);
    background: var(--color-bg);
    border: 1px solid var(--color-border);
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
  .text-btn.danger {
    color: #f48771;
    border-color: #f48771;
  }
  .filters-toggle {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--color-text-muted);
    font-size: 11px;
    padding: 2px 0;
  }
  .filters-toggle:hover {
    color: var(--color-text);
  }
  .filters-toggle .chevron {
    width: 9px;
    height: 9px;
  }
  .filter-input {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 8px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-size: 12px;
    font-family: var(--font-mono);
  }
  .filter-input:focus {
    border-color: var(--color-focus-ring);
  }
  .filter-input::placeholder {
    color: var(--color-text-faint);
  }
  .error-banner {
    flex-shrink: 0;
    margin: 0 0 6px;
    padding: 6px 8px;
    font-size: 12px;
    color: #f48771;
    background: rgba(244, 135, 113, 0.12);
    border-radius: 4px;
  }
  .replace-summary {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 0 6px;
    padding: 6px 8px;
    font-size: 12px;
    color: var(--color-text);
    background: rgba(115, 201, 145, 0.12);
    border-radius: 4px;
  }
  .replace-summary .icon-btn {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    line-height: 1;
    color: var(--color-text-muted);
  }
  .replace-summary .icon-btn:hover {
    color: var(--color-text);
  }
  .summary {
    flex-shrink: 0;
    margin: 0 0 4px;
    color: var(--color-text-faint);
    font-size: 11px;
  }
  .results {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 0 -16px;
    padding: 0 0 8px;
  }
  .file-group {
    display: flex;
    flex-direction: column;
  }
  .file-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px 3px 16px;
    width: 100%;
    text-align: left;
    font-weight: 600;
  }
  .file-header:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .chevron {
    flex-shrink: 0;
    width: 12px;
    height: 12px;
    transition: transform 0.1s ease;
  }
  .chevron.collapsed {
    transform: rotate(-90deg);
  }
  .file-icon {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
  }
  .file-name {
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-dir {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 400;
    color: var(--color-text-faint);
    font-size: 11px;
  }
  .match-count {
    flex-shrink: 0;
    font-weight: 400;
    font-size: 11px;
    color: var(--color-text-faint);
    background: var(--color-bg-hover);
    border-radius: 8px;
    padding: 0 6px;
    min-width: 16px;
    text-align: center;
  }
</style>
