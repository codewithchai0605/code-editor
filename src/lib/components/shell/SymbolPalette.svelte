<script lang="ts">
  import { onMount } from "svelte";
  import { editorStore } from "$lib/stores/editor.svelte";
  import { paletteState } from "$lib/stores/palette.svelte";
  import {
    fetchDocumentSymbols,
    fetchWorkspaceSymbols,
    type SymbolEntry,
  } from "$lib/lsp/symbols";
  import { baseName } from "$lib/utils/path";
  import Palette from "./Palette.svelte";

  interface Props {
    scope: "document" | "workspace";
  }
  let { scope }: Props = $props();

  let symbols = $state<SymbolEntry[]>([]);
  let loading = $state(true);
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  // A rough label for LSP's `SymbolKind` (1-26) — good enough for a
  // palette row, not attempting the full enum's every value.
  const KIND_LABELS: Record<number, string> = {
    5: "class",
    6: "method",
    7: "property",
    8: "field",
    9: "constructor",
    10: "enum",
    11: "interface",
    12: "function",
    13: "variable",
    14: "constant",
    23: "struct",
  };

  function kindLabel(kind: number): string {
    return KIND_LABELS[kind] ?? "symbol";
  }

  onMount(() => {
    if (scope === "document") {
      void fetchDocumentSymbols().then((result) => {
        symbols = result;
        loading = false;
      });
    } else {
      loading = false; // workspace scope starts empty until a query comes in via onQueryChange
    }
    return () => clearTimeout(debounceTimer);
  });

  function onQueryChange(query: string): void {
    if (scope !== "workspace") return;
    clearTimeout(debounceTimer);
    if (!query.trim()) {
      symbols = [];
      return;
    }
    loading = true;
    debounceTimer = setTimeout(() => {
      void fetchWorkspaceSymbols(query).then((result) => {
        symbols = result;
        loading = false;
      });
    }, 200);
  }

  function onSelect(symbol: SymbolEntry): void {
    void editorStore.openAndReveal(
      symbol.path,
      symbol.range.start.line + 1,
      symbol.range.start.character,
      symbol.range.end.character,
    );
  }
</script>

<Palette
  placeholder={scope === "document"
    ? "Go to symbol in file..."
    : "Go to symbol in workspace..."}
  items={symbols}
  getText={(s) => s.name}
  getKey={(s) =>
    `${s.path}:${s.range.start.line}:${s.range.start.character}:${s.name}`}
  {onSelect}
  onClose={() => paletteState.close()}
  {onQueryChange}
  emptyLabel={loading
    ? "Searching\u2026"
    : scope === "workspace"
      ? "Type to search symbols across the workspace"
      : "No symbols found"}
>
  {#snippet row(result)}
    <div class="symbol-row">
      <span class="kind">{kindLabel(result.item.kind)}</span>
      <span class="name">{result.item.name}</span>
      {#if result.item.containerName}<span class="container"
          >{result.item.containerName}</span
        >{/if}
      {#if scope === "workspace"}<span class="path"
          >{baseName(result.item.path)}</span
        >{/if}
    </div>
  {/snippet}
</Palette>

<style>
  .symbol-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow: hidden;
  }
  .kind {
    flex-shrink: 0;
    font-size: 11px;
    text-transform: uppercase;
    color: var(--color-text-faint);
    width: 56px;
  }
  .name {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    color: var(--color-text);
  }
  .container,
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--color-text-faint);
  }
</style>
