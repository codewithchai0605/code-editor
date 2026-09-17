import type { FsEntry } from "$lib/types/fs";

export interface TreeActions {
    startDrag: (entry: FsEntry, e: PointerEvent) => void;
    openContextMenu: (entry: FsEntry, x: number, y: number) => void;
}

/** Symbol key so this can't collide with an unrelated context elsewhere. */
export const TREE_ACTIONS_KEY = Symbol("file-tree-actions");
