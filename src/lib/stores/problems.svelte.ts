import { SvelteMap } from "svelte/reactivity";
import { baseName } from "$lib/utils/path";

/** A single row in the Problems panel — one diagnostic, tagged with the
 *  file it came from. Mirrors VS Code's Problems view entries. */
export interface ProblemEntry {
    id: string;
    uri: string;
    /** Absolute path derived from `uri` — what click-to-reveal feeds into
     *  `editorStore.openAndReveal`. */
    path: string;
    fileName: string;
    /** 1-based line for display (LSP ranges are 0-based). */
    line: number;
    /** 1-based column for display. */
    column: number;
    severity: 1 | 2 | 3 | 4;
    source?: string;
    message: string;
}

type Listener = () => void;

/** Bridge between the LSP diagnostics cache (`lsp/diagnostics.ts`, a plain
 *  Map polled by CodeMirror's linter) and the Problems panel, which needs
 *  Svelte-reactive state. The cache can't itself become `$state` without
 *  dragging reactivity into the hot per-keystroke lint path, so instead
 *  `diagnostics.ts` fires this store's `sync()` whenever a URI's set
 *  changes, and we maintain a reactive per-file list here. */
class ProblemsStore {
    /** uriKey -> that file's rows. Reactive map so the panel updates live. */
    byFile = new SvelteMap<string, ProblemEntry[]>();

    /** Total counts across every file, kept as plain $state numbers so the
     *  status-bar chip is a trivially cheap read (no iteration on render). */
    errorCount = $state(0);
    warningCount = $state(0);
    infoCount = $state(0);

    /** Set to a file's uriKey when something wants the panel open *and*
     *  scrolled to that file (clicking the Problems status-bar chip).
     *  Cleared by ProblemsPanel once it has handled it. */
    revealFilter = $state<string | null>(null);

    private listeners = new Set<Listener>();

    subscribe(fn: Listener): () => void {
        this.listeners.add(fn);
        return () => this.listeners.delete(fn);
    }

    private notify(): void {
        for (const fn of [...this.listeners]) fn();
    }

    /** Recomputes one file's rows + refreshes the aggregate counts. Called
     *  from `lsp/diagnostics.ts` after any `cache.set`/`cache.delete`. */
    sync(uri: string, entries: ProblemEntry[]): void {
        const key = uri;
        if (entries.length === 0) this.byFile.delete(key);
        else this.byFile.set(key, entries);
        this.recount();
        this.notify();
    }

    removeFile(key: string): void {
        if (this.byFile.delete(key)) {
            this.recount();
            this.notify();
        }
    }

    private recount(): void {
        let errors = 0;
        let warnings = 0;
        let infos = 0;
        for (const rows of this.byFile.values()) {
            for (const r of rows) {
                if (r.severity === 1) errors++;
                else if (r.severity === 2) warnings++;
                else infos++;
            }
        }
        this.errorCount = errors;
        this.warningCount = warnings;
        this.infoCount = infos;
    }

    /** All rows, grouped by file (file name + first-seen order), matching
     *  how VS Code nests problems under their file in the panel. */
    get groups(): { key: string; fileName: string; path: string; rows: ProblemEntry[] }[] {
        const out: { key: string; fileName: string; path: string; rows: ProblemEntry[] }[] = [];
        for (const [key, rows] of this.byFile) {
            if (rows.length === 0) continue;
            out.push({ key, fileName: rows[0].fileName, path: rows[0].path, rows });
        }
        out.sort((a, b) => a.fileName.localeCompare(b.fileName));
        return out;
    }
}

export const problemsStore = new ProblemsStore();

/** Builds ProblemEntry rows from raw LSP diagnostics for one file. Lives
 *  here rather than in diagnostics.ts so both the initial publish and any
 *  later reconstruction share one definition of "row". */
export function toProblemEntries(
    uri: string,
    path: string,
    diagnostics: { range: { start: { line: number; character: number } }; severity?: 1 | 2 | 3 | 4; source?: string; message: string }[],
): ProblemEntry[] {
    const fileName = baseName(path);
    return diagnostics.map((d, i) => ({
        id: `${uri}:${i}`,
        uri,
        path,
        fileName,
        line: d.range.start.line + 1,
        column: d.range.start.character + 1,
        severity: d.severity ?? 1,
        source: d.source,
        message: d.message,
    }));
}
