import type { EditorView, Panel, ViewUpdate } from "@codemirror/view";
import {
    search,
    getSearchQuery,
    setSearchQuery,
    SearchQuery,
    closeSearchPanel,
    findNext,
    findPrevious,
    replaceAll,
    replaceNext,
    selectMatches,
} from "@codemirror/search";
import { mount, unmount } from "svelte";
import SearchPanel from "$lib/components/shell/SearchPanel.svelte";

/**
 * Bridges CodeMirror's search/replace state (a StateField living inside the
 * EditorView) to a plain Svelte 5 reactive object, so SearchPanel.svelte can
 * read/write it with $state semantics instead of reaching into CodeMirror
 * transactions directly. One instance is shared module-wide since only one
 * editor can have focus/an open search panel at a time.
 */
class SearchPanelBridge {
    view = $state<EditorView | null>(null);
    query = $state<SearchQuery | null>(null);

    term = $state("");
    replacement = $state("");
    caseSensitive = $state(false);
    regexp = $state(false);
    wholeWord = $state(false);

    /** Total match count and the 1-based index of the match at/after the cursor. */
    matchCount = $state<number | null>(null);
    currentMatch = $state<number | null>(null);

    /** Pushes the current field values into CodeMirror as a new SearchQuery. */
    runQuery() {
        if (!this.view) return;
        this.query = new SearchQuery({
            search: this.term,
            caseSensitive: this.caseSensitive,
            regexp: this.regexp,
            wholeWord: this.wholeWord,
            replace: this.replacement,
        });
        this.view.dispatch({ effects: setSearchQuery.of(this.query) });
        this.updateMatches();
    }

    /** Re-syncs every field from CodeMirror's own state (used when the panel opens/reopens). */
    syncFromView() {
        if (!this.view) return;
        const q = getSearchQuery(this.view.state);
        this.query = q;
        this.term = q.search;
        this.replacement = q.replace;
        this.caseSensitive = q.caseSensitive;
        this.regexp = q.regexp;
        this.wholeWord = q.wholeWord;
        this.updateMatches();
    }

    /** Recomputes the total match count and which one the cursor currently sits at/after. */
    updateMatches() {
        if (!this.view || !this.query || !this.query.search) {
            this.matchCount = null;
            this.currentMatch = null;
            return;
        }
        if (!this.query.valid) {
            // e.g. a regexp query with an unterminated group — nothing to count.
            this.matchCount = 0;
            this.currentMatch = null;
            return;
        }
        const state = this.view.state;
        const cursorPos = state.selection.main.head;
        const cursor = this.query.getCursor(state);
        let count = 0;
        let current: number | null = null;
        for (let result = cursor.next(); !result.done; result = cursor.next()) {
            count++;
            if (current === null && result.value.to >= cursorPos) current = count;
        }
        this.matchCount = count;
        this.currentMatch = count === 0 ? null : (current ?? 1);
    }

    next() {
        if (this.view) findNext(this.view);
        this.updateMatches();
    }
    prev() {
        if (this.view) findPrevious(this.view);
        this.updateMatches();
    }
    doReplaceNext() {
        if (this.view) replaceNext(this.view);
        this.updateMatches();
    }
    doReplaceAll() {
        if (this.view) replaceAll(this.view);
        this.updateMatches();
    }
    selectAll() {
        if (this.view) selectMatches(this.view);
    }
    close() {
        if (this.view) closeSearchPanel(this.view);
    }
}

export const searchPanelBridge = new SearchPanelBridge();

const createSearchPanel = (view: EditorView): Panel => {
    const dom = document.createElement("div");

    searchPanelBridge.view = view;
    searchPanelBridge.syncFromView();

    let instance: ReturnType<typeof mount>;
    return {
        dom,
        top: true,
        mount() {
            instance = mount(SearchPanel, { target: dom });
        },
        update(update: ViewUpdate) {
            searchPanelBridge.query = getSearchQuery(view.state);
            if (update.docChanged || update.selectionSet) searchPanelBridge.updateMatches();
        },
        destroy() {
            if (instance) unmount(instance);
            if (searchPanelBridge.view === view) searchPanelBridge.view = null;
        },
    };
};

export function searchExtension() {
    return search({ createPanel: createSearchPanel, top: true });
}
