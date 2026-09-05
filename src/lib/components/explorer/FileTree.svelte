<script lang="ts">
  import { onMount, setContext } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import type { FsEntry } from "$lib/types/fs";
  import type { CopyProgress } from "$lib/types/fs";
  import type { MenuItem } from "$lib/types/menu";
  import { workspace } from "$lib/stores/workspace.svelte";
  import { dnd } from "$lib/stores/dnd.svelte";
  import { baseName, parentOf } from "$lib/utils/path";
  import { TREE_ACTIONS_KEY, type TreeActions } from "./context";
  import FileTreeNode from "./FileTreeNode.svelte";
  import NewEntryRow from "./NewEntryRow.svelte";
  import ContextMenu from "../shell/ContextMenu.svelte";
  import ConflictDialog from "../shell/ConflictDialog.svelte";

  const rootChildren = $derived(workspace.rootPath ? workspace.childrenOf(workspace.rootPath) : undefined);
  const rootCreating = $derived(
    workspace.rootPath && workspace.creating?.parentDir === workspace.rootPath ? workspace.creating : null,
  );

  let contextMenu = $state<{ entry: FsEntry; x: number; y: number } | null>(null);

  async function pickFolder() {
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") {
      await workspace.open(selected);
    }
  }

  function buildMenuItems(entry: FsEntry): MenuItem[] {
    const hasClipboard = workspace.clipboard !== null;
    const items: MenuItem[] = [];
    if (entry.isDir) {
      items.push(
        { label: "New File", action: () => workspace.startCreate(entry.path, false) },
        { label: "New Folder", action: () => workspace.startCreate(entry.path, true) },
      );
    }
    items.push(
      { label: "Cut", action: () => workspace.cutSelection(), separatorAbove: entry.isDir },
      { label: "Copy", action: () => workspace.copySelection() },
    );
    if (entry.isDir) {
      items.push({
        label: "Paste",
        action: () => workspace.pasteInto(entry.path),
        disabled: !hasClipboard,
      });
    }
    items.push(
      { label: "Rename", action: () => workspace.startRename(entry.path), separatorAbove: true },
      { label: "Delete", action: () => workspace.deleteSelected(), danger: true },
    );
    return items;
  }

  // --- in-tree drag (pointer-based — see stores/dnd.svelte.ts for why) -----

  function elementDropTarget(clientX: number, clientY: number): string | null {
    const el = document.elementFromPoint(clientX, clientY);
    const rowEl = el?.closest<HTMLElement>("[data-tree-path]");
    if (rowEl) {
      return rowEl.dataset.treeDir === "true" ? (rowEl.dataset.treePath ?? null) : (rowEl.dataset.treeParent ?? null);
    }
    return workspace.rootPath;
  }

  /** True if every dragged path is already directly inside `target` — i.e.
   *  dropping here wouldn't actually move anything. */
  function isNoOpTarget(paths: string[], target: string): boolean {
    return paths.every((p) => parentOf(p) === target);
  }

  function startDrag(entry: FsEntry, e: PointerEvent) {
    const paths = workspace.selected.has(entry.path) ? [...workspace.selected] : [entry.path];
    const startX = e.clientX;
    const startY = e.clientY;
    let active = false;

    function onMove(ev: PointerEvent) {
      if (!active) {
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < 4) return;
        active = true;
        dnd.draggingPaths = paths;
        dnd.label = paths.length === 1 ? baseName(paths[0]) : `${paths.length} items`;
        document.body.classList.add("dragging-tree-item");
      }
      dnd.pointer = { x: ev.clientX, y: ev.clientY };
      const target = elementDropTarget(ev.clientX, ev.clientY);
      const valid = target !== null && !paths.includes(target) && !isNoOpTarget(paths, target);
      dnd.dropTargetPath = valid ? target : null;
    }

    async function onUp() {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      document.body.classList.remove("dragging-tree-item");
      const target = dnd.dropTargetPath;
      dnd.draggingPaths = null;
      dnd.dropTargetPath = null;
      dnd.pointer = null;
      // A target only ever survives onMove's filtering if it's a real,
      // non-no-op destination, so no further check is needed here.
      if (active && target) await workspace.dropInto(paths, target);
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  setContext<TreeActions>(TREE_ACTIONS_KEY, {
    startDrag,
    openContextMenu: (entry, x, y) => {
      contextMenu = { entry, x, y };
    },
  });

  onMount(() => {
    let unlistenChanged: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    let unlistenDragDrop: (() => void) | undefined;

    (async () => {
      unlistenChanged = await listen<string[]>("fs:changed", (event) => {
        for (const dir of event.payload) workspace.refresh(dir);
      });

      unlistenProgress = await listen<CopyProgress>("fs:copy-progress", (event) => {
        workspace.copyProgress = event.payload;
        if (event.payload.done >= event.payload.total) {
          setTimeout(() => {
            workspace.copyProgress = null;
          }, 500);
        }
      });

      // OS files dragged in from outside the app (e.g. Explorer). Position
      // arrives in physical pixels; elementFromPoint wants logical/CSS
      // pixels, hence the devicePixelRatio conversion.
      unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (!workspace.rootPath) return;
        const dpr = window.devicePixelRatio || 1;
        if (event.payload.type === "over") {
          const { x, y } = event.payload.position;
          dnd.dropTargetPath = elementDropTarget(x / dpr, y / dpr);
        } else if (event.payload.type === "drop") {
          const { x, y } = event.payload.position;
          const target = elementDropTarget(x / dpr, y / dpr);
          dnd.dropTargetPath = null;
          if (target) workspace.importPaths(event.payload.paths, target);
        } else {
          dnd.dropTargetPath = null;
        }
      });
    })();

    return () => {
      unlistenChanged?.();
      unlistenProgress?.();
      unlistenDragDrop?.();
    };
  });
</script>

<div class="explorer">
  {#if !workspace.rootPath}
    <div class="empty-state">
      <p>No folder opened yet.</p>
      <button class="open-btn" onclick={pickFolder}>Open Folder</button>
    </div>
  {:else}
    <div class="toolbar">
      <span class="root-name" title={workspace.rootPath}>{workspace.rootName}</span>
      <div class="toolbar-actions">
        <button title="New File" aria-label="New File" onclick={() => workspace.rootPath && workspace.startCreate(workspace.rootPath, false)}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z" /><path d="M14 3v4a1 1 0 0 0 1 1h4" /><path d="M12 12v6M9 15h6" /></svg>
        </button>
        <button title="New Folder" aria-label="New Folder" onclick={() => workspace.rootPath && workspace.startCreate(workspace.rootPath, true)}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z" /><path d="M12 11v5M9.5 13.5h5" /></svg>
        </button>
        <button title="Refresh" aria-label="Refresh" onclick={() => workspace.rootPath && workspace.refresh(workspace.rootPath)}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M20 11a8 8 0 1 0-2.5 6.5" /><path d="M20 5v6h-6" /></svg>
        </button>
        <button title="Open Different Folder" aria-label="Open Different Folder" onclick={pickFolder}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M4 5a1 1 0 0 1 1-1h4.5l2 2H19a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5z" /></svg>
        </button>
      </div>
    </div>

    {#if workspace.copyProgress}
      <div class="progress">
        <div class="progress-bar" style:width="{(100 * workspace.copyProgress.done) / Math.max(1, workspace.copyProgress.total)}%"></div>
      </div>
    {/if}

    {#if workspace.lastError}
      <p class="error-banner">{workspace.lastError}</p>
    {/if}

    <div
      class="tree"
      role="tree"
      aria-label="File Explorer"
      data-tree-path={workspace.rootPath}
      data-tree-dir="true"
      class:drop-target={dnd.dropTargetPath === workspace.rootPath}
    >
      {#each rootChildren ?? [] as entry (entry.path)}
        <FileTreeNode {entry} depth={0} />
      {/each}
      {#if rootCreating}
        <NewEntryRow depth={0} isDir={rootCreating.isDir} />
      {/if}
      {#if (rootChildren ?? []).length === 0 && !rootCreating}
        <p class="hint">This folder is empty.</p>
      {/if}
    </div>
  {/if}
</div>

{#if contextMenu}
  {#key contextMenu}
    <ContextMenu
      x={contextMenu.x}
      y={contextMenu.y}
      items={buildMenuItems(contextMenu.entry)}
      onClose={() => (contextMenu = null)}
    />
  {/key}
{/if}

{#if dnd.draggingPaths && dnd.pointer}
  <div class="drag-ghost" style:left="{dnd.pointer.x + 14}px" style:top="{dnd.pointer.y + 10}px">
    {dnd.label}
  </div>
{/if}

<ConflictDialog />

<style>
  .explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    user-select: none;
  }
  .empty-state {
    padding: 12px 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
  .open-btn {
    align-self: flex-start;
    padding: 6px 12px;
    background: var(--color-accent);
    color: #fff;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
  }
  .toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-bottom: 6px;
  }
  .root-name {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text);
  }
  .toolbar-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .toolbar-actions button {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .toolbar-actions button:hover {
    background: var(--color-bg-hover);
    color: var(--color-text);
  }
  .toolbar-actions svg {
    width: 15px;
    height: 15px;
  }
  .progress {
    flex-shrink: 0;
    height: 2px;
    background: var(--color-bg-hover);
    margin-bottom: 4px;
  }
  .progress-bar {
    height: 100%;
    background: var(--color-accent);
    transition: width 0.1s ease;
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
  .tree {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 0 -16px;
    padding: 0 0 8px;
  }
  .tree.drop-target {
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
  }
  .hint {
    margin: 4px 0 0 8px;
    font-size: 12px;
    color: var(--color-text-faint);
    font-style: italic;
  }
  .drag-ghost {
    position: fixed;
    z-index: 300;
    left: 0;
    top: 0;
    pointer-events: none;
    padding: 3px 10px;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-accent);
    border-radius: 4px;
    font-size: 12px;
    color: var(--color-text);
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  }
</style>
