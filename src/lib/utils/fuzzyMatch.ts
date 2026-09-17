export interface FuzzyMatch {
    score: number;
    /** Indices into `target` that matched, in order — for highlighting. */
    positions: number[];
}

const SEPARATORS = "/-_. \\";

/**
 * Subsequence fuzzy match: every character of `query` must appear in
 * `target`, in order, but not necessarily contiguously. Returns `null` if
 * `query` isn't a subsequence of `target` at all.
 *
 * Scoring rewards (roughly, in order of weight): consecutive-character
 * runs, matches right after a path/word separator or at a camelCase
 * boundary, and an earlier first-match position as a tie-breaker.
 */
export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
    if (query.length === 0) return { score: 0, positions: [] };
    if (query.length > target.length) return null;

    const q = query.toLowerCase();
    const t = target.toLowerCase();

    let qi = 0;
    let score = 0;
    let prevMatchIndex = -2;
    let consecutive = 0;
    const positions: number[] = [];

    for (let ti = 0; ti < t.length && qi < q.length; ti++) {
        if (t[ti] !== q[qi]) continue;

        let charScore = 1;

        if (prevMatchIndex === ti - 1) {
            consecutive++;
            charScore += consecutive * 3;
        } else {
            consecutive = 0;
        }

        if (ti === 0 || SEPARATORS.includes(target[ti - 1])) {
            charScore += 8;
        } else if (/[a-z0-9]/.test(target[ti - 1]) && /[A-Z]/.test(target[ti])) {
            charScore += 8;
        }

        score += charScore;
        positions.push(ti);
        prevMatchIndex = ti;
        qi++;
    }

    if (qi < q.length) return null;

    score += Math.max(0, 20 - positions[0]);
    score -= target.length * 0.05;

    return { score, positions };
}

export interface FuzzyResult<T> {
    item: T;
    positions: number[];
}

/** Filters + ranks `items` by fuzzy match against `getText(item)`. An empty
 *  query returns the first `limit` items unranked (positions all empty). */
export function fuzzyFilter<T>(
    query: string,
    items: readonly T[],
    getText: (item: T) => string,
    limit = 200,
): FuzzyResult<T>[] {
    const trimmed = query.trim();
    if (!trimmed) {
        return items.slice(0, limit).map((item) => ({ item, positions: [] }));
    }

    const scored: { item: T; score: number; positions: number[] }[] = [];
    for (const item of items) {
        const m = fuzzyMatch(trimmed, getText(item));
        if (m) scored.push({ item, score: m.score, positions: m.positions });
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, limit).map(({ item, positions }) => ({ item, positions }));
}
