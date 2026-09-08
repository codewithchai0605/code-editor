/**
 * The published `emmet` package declares an `exports` map with no `types`
 * condition, so TypeScript (under `moduleResolution: "bundler"`) can't
 * find its real .d.ts files and falls back to `any` — this is a gap in
 * the package's own packaging, not something fixable from our side except
 * by shimming it. Only the two exports this codebase actually uses
 * (emmet.ts) are declared, typed against the real definitions shipped in
 * `node_modules/emmet/dist/*.d.ts` — see the note in emmet.ts.
 */
declare module "emmet" {
  export interface EmmetUserConfig {
    syntax?: string;
    [key: string]: unknown;
  }

  export interface ExtractedAbbreviation {
    abbreviation: string;
    location: number;
    start: number;
    end: number;
  }

  export interface ExtractOptions {
    lookAhead: boolean;
    type: "markup" | "stylesheet";
    prefix: string;
  }

  export function extract(
    line: string,
    pos?: number,
    options?: Partial<ExtractOptions>,
  ): ExtractedAbbreviation | undefined;

  export default function expandAbbreviation(abbr: string, config?: EmmetUserConfig): string;
}
