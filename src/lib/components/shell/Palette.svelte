<script lang="ts" generics="T">
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";
  import { fuzzyFilter, type FuzzyResult } from "$lib/utils/fuzzyMatch";

  interface Props {
    placeholder: string;
    items: T[];
    getText: (item: T) => string;
    getKey: (item: T) => string;
    onSelect: (item: T) => void;
    onClose: () => void;
    emptyLabel?: string;
    row: Snippet<[result: FuzzyResult<T>, active: boolean]>;
    /** Optional: called with the current query on every change (including
     *  once on mount, with `""`). Filtering itself always stays client-side
     *  over `items` (see `results` below) — this is only for a caller that
     *  wants to *re-fetch* `items` itself as the query changes (e.g.
     *  `SymbolPalette`'s workspace-symbol search, which re-queries the
     *  language server rather than filtering a single fixed list the way
     *  `QuickOpenPalette`'s static file index does). */
    onQueryChange?: (query: string) => void;
  }

  let {
    placeholder,
    items,
    getText,
    getKey,
    onSelect,
    onClose,
    emptyLabel = "No results",
    row,
    onQueryChange,
  }: Props = $props();

  let query = $state("");
  let activeIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  const results = $derived(fuzzyFilter(query, items, getText));

  $effect(() => {
    onQueryChange?.(query);
  });

  // Reset the selection whenever the result set changes shape (new query),
  // so it never points past the end of a shorter list.
  $effect(() => {
    void results;
    activeIndex = 0;
  });

  onMount(() => inputEl?.focus());

  function scrollActiveIntoView() {
    queueMicrotask(() => {
      listEl?.querySelector(".active")?.scrollIntoView({ block: "nearest" });
    });
  }

  function choose(item: T) {
    onClose();
    onSelect(item);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = Math.min(results.length - 1, activeIndex + 1);
      scrollActiveIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = Math.max(0, activeIndex - 1);
      scrollActiveIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results[activeIndex];
      if (r) choose(r.item);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="backdrop"
  role="presentation"
  onclick={onClose}
  onkeydown={(e) => e.key === "Escape" && onClose()}
>
  <div
    class="palette"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === "Escape" && onClose()}
  >
    <input
      bind:this={inputEl}
      bind:value={query}
      {placeholder}
      aria-label={placeholder}
      onkeydown={onKeydown}
    />
    <div
      class="results"
      bind:this={listEl}
      role="listbox"
      aria-label={placeholder}
    >
      {#each results as r, i (getKey(r.item))}
        <button
          class="result"
          class:active={i === activeIndex}
          role="option"
          aria-selected={i === activeIndex}
          onmouseenter={() => (activeIndex = i)}
          onclick={() => choose(r.item)}
        >
          {@render row(r, i === activeIndex)}
        </button>
      {:else}
        <p class="empty">{emptyLabel}</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 250;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
  }
  .palette {
    width: min(560px, calc(100vw - 32px));
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  input {
    flex-shrink: 0;
    height: 44px;
    padding: 0 14px;
    font-size: 14px;
    background: transparent;
    border-bottom: 1px solid var(--color-border);
  }
  input::placeholder {
    color: var(--color-text-faint);
  }
  .results {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
  }
  .result {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border-radius: 4px;
    cursor: pointer;
  }
  .result.active {
    background: var(--color-accent);
  }
  .empty {
    margin: 0;
    padding: 16px 10px;
    text-align: center;
    font-size: 13px;
    color: var(--color-text-faint);
  }
</style>
