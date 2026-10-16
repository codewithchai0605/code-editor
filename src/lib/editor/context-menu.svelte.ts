import type { MenuItem } from "$lib/types/menu";

/** The editor's right-click menu (Cut/Copy/Paste plus, when available,
 *  Go to Definition/Rename Symbol/Format/Quick Fix) — a small global
 *  store with a single mount point (`Shell.svelte`), same reasoning as
 *  `code-action-menu.svelte.ts`'s `codeActionMenuState`: it's triggered
 *  from inside a CodeMirror `domEventHandlers` callback, not from a
 *  specific place in the Svelte component tree.
 *
 *  Kept as its own store rather than reusing `codeActionMenuState`: the
 *  two are shaped identically ({x, y, items}) but are conceptually
 *  different menus that can each be open independently, and sharing one
 *  store would make "is the code-action menu open" ambiguous. */
class EditorContextMenuState {
    request = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

    open(x: number, y: number, items: MenuItem[]): void {
        this.request = { x, y, items };
    }

    close(): void {
        this.request = null;
    }
}

export const editorContextMenuState = new EditorContextMenuState();
