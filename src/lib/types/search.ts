/**
 * Mirrors `src-tauri/src/commands/search.rs`. Keep these two in sync by
 * hand, same convention as `types/fs.ts`.
 */

export interface SearchOptions {
    query: string;
    caseSensitive: boolean;
    wholeWord: boolean;
    regex: boolean;
    /** Comma-separated glob patterns, e.g. `"**\/*.ts, morefile"`. Empty means "no restriction". */
    include: string;
    exclude: string;
}

export type SearchErrorKind = "invalidRegex" | "invalidGlob";

/** The `Err` payload Tauri delivers to a rejected `invoke()` call — see the
 *  `#[serde(tag = "kind", ...)]` enum on the Rust side. */
export interface SearchError {
    kind: SearchErrorKind;
    message?: string;
}

export function isSearchError(value: unknown): value is SearchError {
    return typeof value === "object" && value !== null && "kind" in value;
}

export interface SearchMatch {
    /** 1-based line number. */
    line: number;
    /** UTF-16 code unit offsets into `lineText` — directly usable as
     *  CodeMirror/JS string positions, no re-encoding needed. */
    colStart: number;
    colEnd: number;
    lineText: string;
}

export interface FileSearchResult {
    path: string;
    matches: SearchMatch[];
}

/** Payload of the backend's `search:results` event — one or more files'
 *  worth of matches, batched together (see search.rs's BATCH_FLUSH_*). */
export interface SearchResultBatch {
    searchId: string;
    files: FileSearchResult[];
}

/** Payload of the backend's `search:done` event. */
export interface SearchDoneEvent {
    searchId: string;
    filesMatched: number;
    matchesFound: number;
    truncated: boolean;
}

export interface ReplaceFileResult {
    path: string;
    replaced: number;
    error?: string | null;
}