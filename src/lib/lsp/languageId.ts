import { extensionOf } from "$lib/utils/fileIcons";

/**
 * Step 1 of the incremental LSP build only has a single hardcoded server
 * (typescript-language-server), so this only recognizes the extensions it
 * handles. Everything else returns `null`, meaning "no LSP integration for
 * this file yet" — a normal, expected state, not an error. Extend this map
 * as the pluggable server registry (AGENT.md §3) grows.
 */
const LANGUAGE_IDS: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "typescript",
  jsx: "typescript",
  mjs: "typescript",
  cjs: "typescript",
};

export function lspLanguageIdFor(fileName: string): string | null {
  return LANGUAGE_IDS[extensionOf(fileName)] ?? null;
}
