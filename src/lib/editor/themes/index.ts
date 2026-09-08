import type { Theme } from "./types";
import { vscodeDarkTheme } from "./vscode-dark";
import { draculaTheme } from "./dracula";
import { monokaiTheme } from "./monokai";

export type { Theme, ThemeColors } from "./types";

export const themes: Theme[] = [vscodeDarkTheme, draculaTheme, monokaiTheme];

export const themesById: Record<string, Theme> = Object.fromEntries(
  themes.map((theme) => [theme.id, theme]),
);

export const defaultTheme: Theme = vscodeDarkTheme;
