import type { Theme } from "./types";

export const monokaiTheme: Theme = {
  id: "monokai",
  name: "Monokai",
  dark: true,
  colors: {
    bg: "#272822",
    bgElevated: "#1e1f1c",
    bgSidebar: "#1e1f1c",
    bgActivitybar: "#171814",
    bgStatusbar: "#f92672",
    statusbarText: "#ffffff",
    bgHover: "rgba(255, 255, 255, 0.08)",
    border: "#3e3d32",
    text: "#f8f8f2",
    textMuted: "#948d77",
    textFaint: "#75715e",
    accent: "#f92672",
    focusRing: "#66d9ef",

    selection: "rgba(73, 72, 62, 0.8)",
    activeLine: "rgba(255, 255, 255, 0.05)",

    comment: "#75715e",
    keyword: "#f92672",
    controlKeyword: "#f92672",
    variable: "#f8f8f2",
    function: "#a6e22e",
    type: "#66d9ef",
    number: "#ae81ff",
    string: "#e6db74",
    regexp: "#e6db74",
    operator: "#f92672",
    tag: "#f92672",
    attribute: "#a6e22e",
    invalid: "#f44747",
  },
};
