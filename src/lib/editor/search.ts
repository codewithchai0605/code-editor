import type { EditorView, Panel } from "@codemirror/view";
import {
    search,
    getSearchQuery,
    setSearchQuery,
    SearchQuery,
    RegExpCursor,
    SearchCursor,
    findNext,
    findPrevious,
    gotoLine,
    highlightSelectionMatches,
    replaceAll,
    replaceNext,
    searchKeymap,
    selectMatches,
    selectNextOccurrence,
    selectSelectionMatches
} from "@codemirror/search";
import { mount, unmount } from "svelte";
import SearchPanel from "$lib/components/shell/SearchPanel.svelte";

class SearchPanelBridge {
    view = $state<EditorView | null>(null);
    query = $state<SearchQuery | null>(null);
    replacement = $state<string | null>(null);
    caseSensitive = $state<boolean>(false);

    runQuery() {
        if (!this.view) return;
        if (!this.query) return;
        this.view.dispatch({
            effects: setSearchQuery.of(
                new SearchQuery({ search: this.query.search, caseSensitive: this.caseSensitive, regexp: this.query.regexp, wholeWord: this.query.wholeWord })
            )
        })
    }
    next() { if (this.view) findNext(this.view); }
    prev() { if (this.view) findPrevious(this.view); }
    replaceNext() { if (this.view) replaceNext(this.view); }
    replaceAll() { if (this.view) replaceAll(this.view); }
    selectMatches() { if (this.view) selectMatches(this.view); }
}

export const searchPanelBridge = new SearchPanelBridge();

const createSearchPanel = (view: EditorView): Panel => {
    const dom = document.createElement("div");
    searchPanelBridge.view = view;
    searchPanelBridge.query = getSearchQuery(view.state);

    let instance: ReturnType<typeof mount>;
    return {
        dom,
        top: true,
        mount() {
            instance = mount(SearchPanel, { target: dom });
        },
        update() {
            searchPanelBridge.query = getSearchQuery(view.state);
        },
        destroy() {
            if (instance) unmount(instance);
        }
    }
}

export function searchExtension() {
    return search({ createPanel: createSearchPanel, top: true });
}