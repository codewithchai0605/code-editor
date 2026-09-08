import type { Theme } from "./types";

export const vscodeDarkTheme: Theme = {
  id: "vscode-dark",
  name: "VS Code Dark+",
  dark: true,
  colors: {
    bg: "#1e1e1e",
    bgElevated: "#2d2d2d",
    bgSidebar: "#252526",
    bgActivitybar: "#333333",
    bgStatusbar: "#007acc",
    statusbarText: "#ffffff",
    bgHover: "rgba(255, 255, 255, 0.08)",
    border: "#2b2b2b",
    text: "#cccccc",
    textMuted: "#969696",
    textFaint: "#6b6b6b",
    accent: "#007acc",
    focusRing: "#007fd4",

    selection: "rgba(38, 79, 120, 0.6)",
    activeLine: "rgba(255, 255, 255, 0.04)",

    comment: "#6a9955",
    keyword: "#569cd6",
    controlKeyword: "#c586c0",
    variable: "#9cdcfe",
    function: "#dcdcaa",
    type: "#4ec9b0",
    number: "#b5cea8",
    string: "#ce9178",
    regexp: "#d16969",
    operator: "#d4d4d4",
    tag: "#569cd6",
    attribute: "#9cdcfe",
    invalid: "#f44747",
  },
};
