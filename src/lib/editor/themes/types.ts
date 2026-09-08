/**
 * A single source of truth per theme. `colors` feeds two different
 * consumers:
 *  - The app shell (Sidebar, ActivityBar, StatusBar, tabs, etc.) already
 *    reads everything through CSS custom properties (see app.css) — see
 *    `themeEngine.ts`'s `applyAppColors`, which just writes these values
 *    onto `:root`.
 *  - CodeMirror's editor chrome + syntax highlighting, built once per
 *    theme by `themeEngine.ts`'s `buildCmTheme`.
 *
 * Keeping one data shape for both is what makes "switch theme -> the whole
 * app re-tints instantly" a small function instead of two parallel
 * hand-maintained theme systems.
 */
export interface ThemeColors {
  // --- App shell -----------------------------------------------------
  bg: string;
  bgElevated: string;
  bgSidebar: string;
  bgActivitybar: string;
  bgStatusbar: string;
  statusbarText: string;
  bgHover: string;
  border: string;
  text: string;
  textMuted: string;
  textFaint: string;
  accent: string;
  focusRing: string;

  // --- Editor-only chrome (not shared with the app shell) -------------
  selection: string;
  activeLine: string;

  // --- Syntax highlighting --------------------------------------------
  comment: string;
  keyword: string;
  controlKeyword: string;
  variable: string;
  function: string;
  type: string;
  number: string;
  string: string;
  regexp: string;
  operator: string;
  tag: string;
  attribute: string;
  invalid: string;
}

export interface Theme {
  id: string;
  name: string;
  dark: boolean;
  colors: ThemeColors;
}
