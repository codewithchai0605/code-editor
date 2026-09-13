import { gitDiff, type DiffContent } from "$lib/api/git";
import { baseName } from "$lib/utils/path";

export interface DiffTab {
    id: string;
    repoId: string;
    path: string;
    staged: boolean;
    title: string;
    content: DiffContent | null;
    loading: boolean;
    error: string | null;
}

/**
 * Diff tabs are deliberately a completely separate store from
 * `editorStore` rather than a new `OpenTab` kind — a diff view is
 * read-only, has no save/dirty lifecycle, and (crucially) has no LSP
 * involvement, so keeping it fully independent means never needing to
 * touch `editor.svelte.ts`'s LSP-instrumented `open`/`save`/`requestClose`
 * paths to add this feature.
 *
 * `EditorArea.svelte` treats `activeId` here as taking priority over
 * `editorStore.activePath` when non-null: opening a diff doesn't disturb
 * which file tab was active, so switching back to file tabs resumes
 * exactly where you left off.
 */
class DiffTabStore {
    tabs = $state<DiffTab[]>([]);
    activeId = $state<string | null>(null);

    get activeTab(): DiffTab | undefined {
        return this.tabs.find((t) => t.id === this.activeId);
    }

    async open(repoId: string, path: string, staged: boolean): Promise<void> {
        const id = `${repoId}:${path}:${staged}`;
        const existing = this.tabs.find((t) => t.id === id);
        if (existing) {
            this.activeId = id;
            // Refresh on reopen — the file may have changed since this diff was
            // last viewed, and reusing a stale diff would be actively misleading.
            void this.load(existing);
            return;
        }
        const tab: DiffTab = {
            id,
            repoId,
            path,
            staged,
            title: `${baseName(path)} (${staged ? "Staged" : "Working Tree"})`,
            content: null,
            loading: true,
            error: null,
        };
        this.tabs.push(tab);
        this.activeId = id;
        await this.load(tab);
    }

    private async load(tab: DiffTab): Promise<void> {
        tab.loading = true;
        tab.error = null;
        try {
            tab.content = await gitDiff(tab.repoId, tab.path, tab.staged);
        } catch (err) {
            tab.error = err instanceof Error ? err.message : String(err);
        } finally {
            tab.loading = false;
        }
    }

    setActive(id: string): void {
        if (this.tabs.some((t) => t.id === id)) this.activeId = id;
    }

    /** Falls back to whatever file tab was active — doesn't touch
     *  `editorStore` at all, since it was never disturbed in the first place. */
    deactivate(): void {
        this.activeId = null;
    }

    close(id: string): void {
        const idx = this.tabs.findIndex((t) => t.id === id);
        if (idx === -1) return;
        this.tabs.splice(idx, 1);
        if (this.activeId === id) {
            const next = this.tabs[idx] ?? this.tabs[idx - 1];
            this.activeId = next ? next.id : null;
        }
    }
}

export const diffTabStore = new DiffTabStore();
