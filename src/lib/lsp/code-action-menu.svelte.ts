import type { MenuItem } from "$lib/types/menu";

/** The code-action menu (Ctrl+.) is triggered from deep inside a
 *  CodeMirror command, not from a specific place in the Svelte component
 *  tree the way `FileTree`'s own right-click menu is — so, like
 *  `paletteState`, this is a small global store with a single mount
 *  point (`Shell.svelte`) rather than each caller owning its own
 *  `<ContextMenu>` instance. */
class CodeActionMenuState {
    request = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

    open(x: number, y: number, items: MenuItem[]): void {
        this.request = { x, y, items };
    }

    close(): void {
        this.request = null;
    }
}

export const codeActionMenuState = new CodeActionMenuState();