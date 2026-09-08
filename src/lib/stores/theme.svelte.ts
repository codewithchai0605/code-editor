import type { Extension } from "@codemirror/state";
import { themes, themesById, defaultTheme, type Theme } from "$lib/editor/themes";
import { buildCmTheme, applyAppColors } from "$lib/editor/themeEngine";

const STORAGE_KEY = "code-editor:theme";

function loadPersistedThemeId(): string {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved && themesById[saved] ? saved : defaultTheme.id;
  } catch {
    // Persistence is a nice-to-have — a locked-down/private profile that
    // blocks localStorage shouldn't stop the app from picking a theme.
    return defaultTheme.id;
  }
}

class ThemeStore {
  themeId = $state(loadPersistedThemeId());

  get theme(): Theme {
    return themesById[this.themeId] ?? defaultTheme;
  }

  get list(): Theme[] {
    return themes;
  }

  /** Fresh CodeMirror extension for the current theme — read whenever a
   *  new tab is opened (see editor.svelte.ts) or the theme changes. */
  get cmExtension(): Extension {
    return buildCmTheme(this.theme);
  }

  /** Call once on app start so the shell matches the saved theme even
   *  before any editor tab exists. */
  applyInitial(): void {
    applyAppColors(this.theme);
  }

  /** Switches the theme everywhere: app shell colors immediately, and
   *  every already-open CodeMirror tab via editorStore's theme
   *  compartment. The editorStore import is dynamic (not a top-level
   *  `import`) specifically to avoid a circular dependency — editor.svelte.ts
   *  needs `themeStore.cmExtension` when constructing a brand-new tab's
   *  initial state, so a static import back here would form a cycle. */
  async setTheme(id: string): Promise<void> {
    if (!themesById[id] || id === this.themeId) return;
    this.themeId = id;
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      // Same reasoning as loadPersistedThemeId — non-fatal either way.
    }
    applyAppColors(this.theme);
    const { editorStore } = await import("./editor.svelte");
    editorStore.applyThemeToOpenTabs(this.cmExtension);
  }
}

export const themeStore = new ThemeStore();
