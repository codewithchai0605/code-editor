import type { Extension } from "@codemirror/state";
import { LanguageDescription, LanguageSupport } from "@codemirror/language";
import { languages as registeredLanguages } from "@codemirror/language-data";
import { extensionOf } from "$lib/utils/fileIcons";
import { snippetSourceFor } from "./snippets";

type LanguageLoader = () => Promise<Extension>;

/**
 * `.env` files are exactly the case `extensionOf()` isn't built for: a
 * plain `.env` has no "extension" by that function's definition, and
 * `.env.local`/`.env.production` etc. would resolve to `local`/`production`
 * — the *last* dot-segment — not `env`. Matched by filename shape here
 * rather than bending `extensionOf`'s general rule to cover this one case.
 */
const DOTENV_PATTERN = /^\.env(\..+)?$/i;

function languageKeyFor(fileName: string): string {
  const baseName = fileName.split(/[\\/]/).pop() ?? fileName;
  if (DOTENV_PATTERN.test(baseName)) return "env";
  return extensionOf(fileName);
}

/**
 * Hand-tuned loaders for the handful of languages worth actual
 * configuration beyond their package's own default — JSX on for every JS
 * flavor, auto-closing tags for markup languages, a shared HTML base for
 * Vue/PHP — plus the two fully custom languages (`env`, `prisma`) that
 * aren't in any public registry at all.
 *
 * Everything else falls through to `@codemirror/language-data`'s own
 * ~190-language registry in `loadLanguage` below, so this map only needs
 * to grow when a language specifically needs configuring, not every time
 * a new one should merely be *supported*.
 */
const loaders: Record<string, LanguageLoader> = {
  js: async () => (await import("@codemirror/lang-javascript")).javascript({ jsx: true }),
  mjs: async () => (await import("@codemirror/lang-javascript")).javascript({ jsx: true }),
  cjs: async () => (await import("@codemirror/lang-javascript")).javascript({ jsx: true }),
  jsx: async () => (await import("@codemirror/lang-javascript")).javascript({ jsx: true }),
  ts: async () => (await import("@codemirror/lang-javascript")).javascript({ typescript: true }),
  tsx: async () =>
    (await import("@codemirror/lang-javascript")).javascript({ jsx: true, typescript: true }),
  html: async () => (await import("@codemirror/lang-html")).html({ autoCloseTags: true }),
  htm: async () => (await import("@codemirror/lang-html")).html({ autoCloseTags: true }),
  xml: async () => (await import("@codemirror/lang-xml")).xml({ autoCloseTags: true }),
  svg: async () => (await import("@codemirror/lang-xml")).xml({ autoCloseTags: true }),
  vue: async () => {
    const [{ vue }, { html }] = await Promise.all([
      import("@codemirror/lang-vue"),
      import("@codemirror/lang-html"),
    ]);
    return vue({ base: html({ autoCloseTags: true }) });
  },
  php: async () => {
    const [{ php }, { html }] = await Promise.all([
      import("@codemirror/lang-php"),
      import("@codemirror/lang-html"),
    ]);
    const htmlSupport = html({ autoCloseTags: true, matchClosingTags: false });
    return [php({ baseLanguage: htmlSupport.language }), htmlSupport.support];
  },
  env: async () => (await import("./languages/env")).envLanguage,
  prisma: async () => (await import("./languages/prisma")).prismaLanguage,
};

/** Returns `null` for a file nothing recognizes — it still opens and edits
 *  fine, just without syntax highlighting.
 *
 *  Resolution order: the hand-tuned `loaders` above (checked by extension
 *  key) first, then `@codemirror/language-data`'s registry via
 *  `LanguageDescription.matchFilename` — which itself checks *filename*
 *  patterns before extensions, so it correctly matches things like
 *  `Dockerfile` or `Makefile` that have no extension at all. That one
 *  fallback is what took this from ~20 hand-maintained extensions to the
 *  registry's full ~190 languages (Go, Rust... already hand-tuned above,
 *  but also Kotlin, Swift, Ruby, Shell, TOML, YAML variants, CMake,
 *  GraphQL, LaTeX, PowerShell, Nginx/Apache configs, and more) for free —
 *  new entries in that package's own registry need nothing added here. */
export async function loadLanguage(fileName: string): Promise<Extension | null> {
  const key = languageKeyFor(fileName);
  const loader = loaders[key];

  let support: Extension | null = null;
  try {
    if (loader) {
      support = await loader();
    } else {
      const baseName = fileName.split(/[\\/]/).pop() ?? fileName;
      const desc = LanguageDescription.matchFilename(registeredLanguages, baseName);
      // `LanguageDescription` caches its own `.load()` result internally
      // (via its private `support`/`loading` fields) — reopening the same
      // file type repeatedly doesn't re-trigger the dynamic import.
      if (desc) support = await desc.load();
    }
  } catch (err) {
    console.error("Failed to load language support:", err);
    return null;
  }
  if (!support) return null;

  // Snippets are attached to the *language's own* data facet (rather than
  // via `autocompletion()`'s `override` back in setup.ts) so they show up
  // alongside each language package's own built-in completions instead of
  // silently replacing them — `override` takes over the entire completion
  // source list, built-ins included. Only applies to the hand-tuned set
  // above (`snippetSourceFor` has no entries for the registry fallback's
  // languages), which is exactly the set snippets.ts covers anyway.
  const snippetSource = snippetSourceFor(key);
  if (snippetSource && support instanceof LanguageSupport) {
    return [support, support.language.data.of({ autocomplete: snippetSource })];
  }
  return support;
}
