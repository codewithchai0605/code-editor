<script lang="ts">
  import { getContext } from "svelte";
  import type { FsEntry } from "$lib/types/fs";
  import { workspace } from "$lib/stores/workspace.svelte";
  import { editorStore } from "$lib/stores/editor.svelte";
  import { dnd } from "$lib/stores/dnd.svelte";
  import { gitStore } from "$lib/stores/git.svelte";
  import { fileIconColor } from "$lib/utils/fileIcons";
  import { fileIconUrl, folderIconUrl } from "$lib/utils/iconTheme";
  import { parentOf } from "$lib/utils/path";
  import { validateEntryName } from "$lib/utils/validateName";
  import { TREE_ACTIONS_KEY, type TreeActions } from "./context";
  import NewEntryRow from "./NewEntryRow.svelte";
  import FileTreeNode from "./FileTreeNode.svelte";

  interface Props {
    entry: FsEntry;
    depth: number;
  }
  let { entry, depth }: Props = $props();

  const actions = getContext<TreeActions>(TREE_ACTIONS_KEY);

  const isExpanded = $derived(workspace.expanded.has(entry.path));
  const isSelected = $derived(workspace.selected.has(entry.path));
  const isRenaming = $derived(workspace.renamingPath === entry.path);
  const isDropTarget = $derived(
    entry.isDir && dnd.dropTargetPath === entry.path,
  );
  const isDragging = $derived(dnd.draggingPaths?.includes(entry.path) ?? false);
  const children = $derived(
    entry.isDir ? workspace.childrenOf(entry.path) : undefined,
  );
  const loading = $derived(
    entry.isDir ? workspace.isLoading(entry.path) : false,
  );
  const creatingHere = $derived(
    entry.isDir && workspace.creating?.parentDir === entry.path
      ? workspace.creating
      : null,
  );
  // Folders show the aggregated (highest-priority) status among every
  // changed file underneath them — computed in gitStore from the flat
  // status list itself, not from this component's lazily-loaded tree
  // state, so it's correct even for a folder whose children haven't been
  // expanded/fetched yet.
  const gitStatus = $derived(
    entry.isDir
      ? gitStore.folderStatusFor(entry.path)
      : gitStore.statusFor(entry.path)?.status,
  );
  const GIT_BADGE: Record<string, string> = {
    modified: "M",
    added: "A",
    deleted: "D",
    renamed: "R",
    untracked: "U",
    conflicted: "!",
  };
  // Themed SVG icon for this entry, if the current icon set covers it —
  // `undefined` falls back to the generic tinted-glyph icon below, so this
  // degrades gracefully as icons get added to src/lib/assets/icons over
  // time rather than requiring full coverage before any of it can be used.
  const themedIconUrl = $derived(
    entry.isDir
      ? folderIconUrl(entry.name, isExpanded)
      : fileIconUrl(entry.name),
  );

  let renameValue = $state("");
  let renameError = $state<string | null>(null);
  let renameInputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (isRenaming) {
      renameValue = entry.name;
      renameError = null;
      queueMicrotask(() => renameInputEl?.select());
    }
  });

  function open() {
    if (entry.isDir) {
      workspace.toggleExpand(entry);
    } else {
      editorStore.open(entry.path);
    }
  }

  function handleClick(e: MouseEvent) {
    if (isRenaming) return;
    workspace.select(entry.path, { toggle: e.ctrlKey || e.metaKey });
    open();
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    if (!workspace.selected.has(entry.path)) workspace.select(entry.path);
    actions.openContextMenu(entry, e.clientX, e.clientY);
  }

  function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0 || isRenaming) return;
    // Without this, the browser's default text-selection/native-drag
    // behavior fires alongside our own pointer-based drag, which is what
    // was causing text to get selected while dragging. preventDefault()
    // here does NOT stop the click event, so normal select/open still work.
    e.preventDefault();
    (e.currentTarget as HTMLElement).focus();
    actions.startDrag(entry, e);
  }

  async function submitRename() {
    const error = validateEntryName(renameValue);
    if (error) {
      renameError = error;
      return;
    }
    if (renameValue.trim() === entry.name) {
      workspace.cancelRename();
      return;
    }
    await workspace.confirmRename(entry.path, renameValue.trim());
  }

  function onRenameKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter") submitRename();
    else if (e.key === "Escape") workspace.cancelRename();
  }

  function onRowKeydown(e: KeyboardEvent) {
    if (isRenaming) return;
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === "F2") {
      e.preventDefault();
      workspace.startRename(entry.path);
    } else if (e.key === "Delete") {
      e.preventDefault();
      if (!workspace.selected.has(entry.path)) workspace.select(entry.path);
      workspace.deleteSelected();
    } else if (mod && e.key.toLowerCase() === "c") {
      e.preventDefault();
      workspace.copySelection();
    } else if (mod && e.key.toLowerCase() === "x") {
      e.preventDefault();
      workspace.cutSelection();
    } else if (mod && e.key.toLowerCase() === "v" && entry.isDir) {
      e.preventDefault();
      workspace.pasteInto(entry.path);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (!workspace.selected.has(entry.path)) workspace.select(entry.path);
      open();
    }
  }
</script>

<div
  class="row"
  class:selected={isSelected}
  class:drop-target={isDropTarget}
  class:dragging={isDragging}
  style:padding-left="{depth * 14 + 8}px"
  data-tree-path={entry.path}
  data-tree-dir={entry.isDir}
  data-tree-parent={parentOf(entry.path)}
  role="treeitem"
  aria-selected={isSelected}
  aria-expanded={entry.isDir ? isExpanded : undefined}
  tabindex="0"
  onclick={handleClick}
  oncontextmenu={handleContextMenu}
  onpointerdown={handlePointerDown}
  onkeydown={onRowKeydown}
>
  {#if entry.isDir}
    <svg
      class="chevron"
      class:expanded={isExpanded}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
    >
      <path d="M6 3l5 5-5 5" />
    </svg>
  {:else}
    <span class="chevron-spacer"></span>
  {/if}

  {#if themedIconUrl}
    <img class="icon" src={themedIconUrl} alt="" />
  {:else}
    <svg
      class="icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke={entry.isDir
        ? "var(--color-text-muted)"
        : fileIconColor(entry.name)}
      stroke-width="1.6"
    >
      {#if entry.isDir}
        <path
          d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z"
        />
      {:else}
        <path
          d="M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"
        />
        <path d="M14 3v4a1 1 0 0 0 1 1h4" />
      {/if}
    </svg>
  {/if}

  {#if isRenaming}
    <input
      class="rename-input"
      bind:this={renameInputEl}
      bind:value={renameValue}
      onkeydown={onRenameKeydown}
      onblur={submitRename}
      onclick={(e) => e.stopPropagation()}
    />
  {:else}
    <span
      class="name"
      class:git-changed={!!gitStatus}
      data-git-status={gitStatus}>{entry.name}</span
    >
    {#if gitStatus}
      <span class="git-badge" data-git-status={gitStatus}
        >{GIT_BADGE[gitStatus]}</span
      >
    {/if}
  {/if}
</div>
{#if isRenaming && renameError}
  <p class="rename-error" style:padding-left="{depth * 14 + 30}px">
    {renameError}
  </p>
{/if}

{#if entry.isDir && isExpanded}
  <div role="group">
    {#if loading}
      <p class="hint" style:padding-left="{(depth + 1) * 14 + 30}px">
        Loading&hellip;
      </p>
    {:else}
      {#each children ?? [] as child (child.path)}
        <FileTreeNode entry={child} depth={depth + 1} />
      {/each}
      {#if creatingHere}
        <NewEntryRow depth={depth + 1} isDir={creatingHere.isDir} />
      {/if}
      {#if (children ?? []).length === 0 && !creatingHere}
        <p class="hint" style:padding-left="{(depth + 1) * 14 + 30}px">
          Empty folder
        </p>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    cursor: pointer;
    border: 1px solid transparent;
    touch-action: none;
    user-select: none;
  }
  .row:hover {
    background: var(--color-bg-hover);
  }
  .row.selected {
    background: rgba(0, 122, 204, 0.25);
  }
  .row.drop-target {
    border-color: var(--color-accent);
  }
  .row.dragging {
    opacity: 0.5;
  }
  .chevron,
  .chevron-spacer {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }
  .chevron {
    transition: transform 0.1s ease;
    color: var(--color-text-muted);
  }
  .chevron.expanded {
    transform: rotate(90deg);
  }
  .icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    object-fit: contain;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  /* Colors follow VS Code's own convention, so it reads the same way to
     anyone coming from there: orange for a modification in progress,
     green for something new (added/untracked), red for something gone or
     in conflict. */
  .name.git-changed[data-git-status="modified"] {
    color: #e2c08d;
  }
  .name.git-changed[data-git-status="added"],
  .name.git-changed[data-git-status="untracked"] {
    color: #73c991;
  }
  .name.git-changed[data-git-status="deleted"],
  .name.git-changed[data-git-status="conflicted"] {
    color: #f48771;
  }
  .name.git-changed[data-git-status="renamed"] {
    color: #73c991;
  }
  .git-badge {
    flex-shrink: 0;
    margin-left: auto;
    padding-right: 8px;
    font-size: 11px;
    font-weight: 600;
    opacity: 0.85;
  }
  .git-badge[data-git-status="modified"] {
    color: #e2c08d;
  }
  .git-badge[data-git-status="added"],
  .git-badge[data-git-status="untracked"],
  .git-badge[data-git-status="renamed"] {
    color: #73c991;
  }
  .git-badge[data-git-status="deleted"],
  .git-badge[data-git-status="conflicted"] {
    color: #f48771;
  }
  .rename-input {
    flex: 1;
    min-width: 0;
    background: var(--color-bg);
    border: 1px solid var(--color-focus-ring);
    border-radius: 2px;
    padding: 0 4px;
    font-size: 13px;
    height: 20px;
  }
  .rename-error {
    margin: 2px 0 0;
    font-size: 11px;
    color: #f48771;
  }
  .hint {
    margin: 2px 0;
    font-size: 12px;
    color: var(--color-text-faint);
    font-style: italic;
  }
</style>
