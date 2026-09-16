import { listen } from "@tauri-apps/api/event";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { workspaceSearch, stopWorkspaceSearch, workspaceReplaceAll } from "$lib/api/search";
import { isSearchError, type SearchDoneEvent, type SearchMatch, type SearchOptions, type SearchResultBatch } from "$lib/types/search";
import { workspace } from "./workspace.svelte";
import { editorStore } from "./editor.svelte";

/** Debounce window between the last keystroke/option change and the search
 *  actually firing — long enough that a fast typist doesn't spawn a walk
 *  per keystroke, short enough to still feel instant. See search.rs's own
 *  doc comment for how the backend cooperates: a new search here always
 *  supersedes (and cancels) whatever the previous one was still doing. */
const DEBOUNCE_MS = 180;

function describeError(err: unknown): string {
    if (isSearchError(err)) {
        if (err.message) return err.message;
        return err.kind === "invalidRegex" ? "Invalid regular expression." : "Invalid glob pattern.";
    }
    return err instanceof Error ? err.message : String(err);
}

type Status = "idle" | "searching" | "done";

class WorkspaceSearchStore {
    term = $state("");
    replaceTerm = $state("");
    caseSensitive = $state(false);
    wholeWord = $state(false);
    regex = $state(false);
    include = $state("");
    exclude = $state("");
    showReplace = $state(false);
    showFilters = $state(false);

    status = $state<Status>("idle");
    truncated = $state(false);
    lastError = $state<string | null>(null);

    isReplacing = $state(false);
    confirmingReplaceAll = $state(false);
    replaceSummary = $state<string | null>(null);

    /** Keyed by absolute file path, insertion order preserved (a plain
     *  `SvelteMap` — not a `$state`-wrapped Map — so `.set()`/`.delete()`
     *  are independently reactive; see workspace.svelte.ts's own note on
     *  why). Every match for a file arrives together in one batch, so a
     *  whole-array replace on `.set()` is all that's ever needed here. */
    results = new SvelteMap<string, SearchMatch[]>();
    collapsedFiles = new SvelteSet<string>();

    private currentSearchId: string | null = null;
    private searchCounter = 0;
    private debounceTimer: ReturnType<typeof setTimeout> | null = null;
    private listenersStarted = false;

    get options(): SearchOptions {
        return {
            query: this.term,
            caseSensitive: this.caseSensitive,
            wholeWord: this.wholeWord,
            regex: this.regex,
            include: this.include,
            exclude: this.exclude,
        };
    }

    get orderedFiles(): string[] {
        return [...this.results.keys()];
    }

    get totalMatches(): number {
        let n = 0;
        for (const matches of this.results.values()) n += matches.length;
        return n;
    }

    get fileCount(): number {
        return this.results.size;
    }

    get hasQuery(): boolean {
        return this.term.trim().length > 0;
    }

    /** Wires up the two backend events. Call once (see Shell.svelte) — same
     *  one-shot-singleton idiom as `workspace.initFsWatcherListener()`. */
    initListeners(): void {
        if (this.listenersStarted) return;
        this.listenersStarted = true;
        listen<SearchResultBatch>("search:results", (event) => this.handleBatch(event.payload));
        listen<SearchDoneEvent>("search:done", (event) => this.handleDone(event.payload));
    }

    private handleBatch(batch: SearchResultBatch): void {
        if (batch.searchId !== this.currentSearchId) return; // stale — superseded by a newer search
        for (const file of batch.files) this.results.set(file.path, file.matches);
    }

    private handleDone(done: SearchDoneEvent): void {
        if (done.searchId !== this.currentSearchId) return;
        this.status = "done";
        this.truncated = done.truncated;
    }

    // --- option setters — every one reschedules the debounced search --------

    setTerm(value: string): void {
        this.term = value;
        this.scheduleSearch();
    }
    setReplaceTerm(value: string): void {
        this.replaceTerm = value;
    }
    toggleCaseSensitive(): void {
        this.caseSensitive = !this.caseSensitive;
        this.scheduleSearch();
    }
    toggleWholeWord(): void {
        this.wholeWord = !this.wholeWord;
        this.scheduleSearch();
    }
    toggleRegex(): void {
        this.regex = !this.regex;
        this.scheduleSearch();
    }
    setInclude(value: string): void {
        this.include = value;
        this.scheduleSearch();
    }
    setExclude(value: string): void {
        this.exclude = value;
        this.scheduleSearch();
    }
    toggleShowReplace(): void {
        this.showReplace = !this.showReplace;
    }
    toggleShowFilters(): void {
        this.showFilters = !this.showFilters;
    }

    clearQuery(): void {
        this.term = "";
        this.scheduleSearch();
    }

    // --- search lifecycle -----------------------------------------------------

    private scheduleSearch(): void {
        if (this.debounceTimer) clearTimeout(this.debounceTimer);
        this.replaceSummary = null;
        this.confirmingReplaceAll = false;
        if (!this.hasQuery) {
            this.stopAndClear();
            return;
        }
        this.debounceTimer = setTimeout(() => void this.runSearch(), DEBOUNCE_MS);
    }

    /** Skips the debounce and searches right now — used by Enter in the
     *  search box, where waiting the usual 180ms after a keypress the user
     *  explicitly used to say "go" would feel laggy. */
    searchImmediately(): void {
        if (this.debounceTimer) clearTimeout(this.debounceTimer);
        this.debounceTimer = null;
        if (!this.hasQuery) {
            this.stopAndClear();
            return;
        }
        void this.runSearch();
    }

    private stopAndClear(): void {
        // Bumping past whatever search id was last issued means a late
        // in-flight batch/done event for it will fail the `searchId` check
        // above and be dropped, even though the backend's own cancellation
        // (see search.rs's generation counter) is what actually stops the work.
        this.currentSearchId = null;
        this.results.clear();
        this.collapsedFiles.clear();
        this.status = "idle";
        this.truncated = false;
        this.lastError = null;
        void stopWorkspaceSearch();
    }

    private async runSearch(): Promise<void> {
        const root = workspace.rootPath;
        if (!root || !this.hasQuery) return;

        this.searchCounter += 1;
        const id = `s${this.searchCounter}`;
        this.currentSearchId = id;
        this.results.clear();
        this.collapsedFiles.clear();
        this.status = "searching";
        this.truncated = false;
        this.lastError = null;

        try {
            await workspaceSearch(id, root, this.options);
        } catch (err) {
            if (this.currentSearchId !== id) return;
            this.lastError = describeError(err);
            this.status = "done";
        }
    }

    // --- results interaction ---------------------------------------------------

    toggleCollapse(path: string): void {
        if (this.collapsedFiles.has(path)) this.collapsedFiles.delete(path);
        else this.collapsedFiles.add(path);
    }

    navigate(path: string, match: SearchMatch): void {
        void editorStore.openAndReveal(path, match.line, match.colStart, match.colEnd);
    }

    // --- replace ---------------------------------------------------------------

    requestReplaceAll(): void {
        if (this.results.size === 0) return;
        this.confirmingReplaceAll = true;
    }

    cancelReplaceAll(): void {
        this.confirmingReplaceAll = false;
    }

    async confirmReplaceAll(): Promise<void> {
        if (this.results.size === 0) return;
        const paths = this.orderedFiles;
        this.isReplacing = true;
        this.confirmingReplaceAll = false;
        try {
            const results = await workspaceReplaceAll(paths, this.options, this.replaceTerm);
            const failed = results.filter((r) => r.error);
            const totalReplaced = results.reduce((sum, r) => sum + r.replaced, 0);
            const filesTouched = results.length - failed.length;
            this.replaceSummary =
                failed.length > 0
                    ? `Replaced ${totalReplaced} occurrence(s) in ${filesTouched} file(s) — ${failed.length} file(s) failed.`
                    : `Replaced ${totalReplaced} occurrence(s) in ${filesTouched} file(s).`;
            // Re-running is simpler and more trustworthy than patching `results`
            // locally to guess at post-replace state (overlapping matches,
            // partial per-file failures, etc. all fall out for free this way).
            await this.runSearch();
        } finally {
            this.isReplacing = false;
        }
    }
}

export const workspaceSearchStore = new WorkspaceSearchStore();