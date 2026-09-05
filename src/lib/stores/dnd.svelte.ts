/**
 * In-tree drag state, read by FileTreeNode for hover highlighting and
 * written by FileTree's pointer-based drag orchestration.
 *
 * This is deliberately NOT implemented with native HTML5 `draggable`/
 * `dragstart`/`drop` events: Tauri's native OS drag-and-drop (needed to get
 * real filesystem paths when dragging files in from Explorer — see
 * `dragDropEnabled` in tauri.conf.json) intercepts drag gestures at the
 * WebView level, which stops in-page HTML5 DnD events from firing
 * reliably. Pointer events sidestep the conflict entirely and are what
 * FileTree.svelte uses for both in-tree reordering and (via
 * `document.elementFromPoint`) resolving OS drop targets.
 */
class DndState {
  draggingPaths = $state<string[] | null>(null);
  dropTargetPath = $state<string | null>(null);
  /** Cursor position + label while dragging, for the floating ghost badge
   *  that shows what's being dragged (see FileTree.svelte). */
  pointer = $state<{ x: number; y: number } | null>(null);
  label = $state("");
}

export const dnd = new DndState();
