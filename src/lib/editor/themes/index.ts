import type { Theme } from "./types";
import { vscodeDarkTheme } from "./vscode-dark";
import { draculaTheme } from "./dracula";
import { monokaiTheme } from "./monokai";
import { oneDarkTheme } from "./one-dark";
import { nordTheme } from "./nord";
import { tokyoNightTheme } from "./tokyo-night";
import { githubLightTheme } from "./github-light";
import { solarizedLightTheme } from "./solarized-light";
import { oneLightTheme } from "./one-light";
import { ayuLightTheme } from "./ayu-light";

export type { Theme, ThemeColors } from "./types";

export const themes: Theme[] = [
    vscodeDarkTheme,
    draculaTheme,
    monokaiTheme,
    ayuLightTheme,
    oneDarkTheme,
    oneLightTheme,
    nordTheme,
    tokyoNightTheme,
    githubLightTheme,
    solarizedLightTheme,
];

export const themesById: Record<string, Theme> = Object.fromEntries(
    themes.map((theme) => [theme.id, theme]),
);

export const defaultTheme: Theme = vscodeDarkTheme;
