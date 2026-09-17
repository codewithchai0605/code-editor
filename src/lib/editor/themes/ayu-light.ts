import type { Theme } from "./types";

export const ayuLightTheme: Theme = {
  id: "ayu-light",
  name: "Ayu Light",
  dark: false,
  colors: {
    bg: "#fafafa",
    bgElevated: "#f3f3f3",
    bgSidebar: "#f3f3f3",
    bgActivitybar: "#f3f3f3",
    bgStatusbar: "#ff9940",
    statusbarText: "#fafafa",
    bgHover: "rgba(0, 0, 0, 0.05)",
    border: "#e7e8e9",
    text: "#5c6166",
    textMuted: "#abb0b6",
    textFaint: "#abb0b6",
    accent: "#ff9940",
    focusRing: "#399ee6",

    selection: "#f0eee4",
    activeLine: "rgba(0, 0, 0, 0.03)",

    comment: "#abb0b6",
    keyword: "#fa8d3e",
    controlKeyword: "#fa8d3e",
    variable: "#5c6166",
    function: "#f2ae49",
    type: "#399ee6",
    number: "#a37acc",
    string: "#86b300",
    regexp: "#4cbf99",
    operator: "#ed9366",
    tag: "#55b4d4",
    attribute: "#f2ae49",
    invalid: "#f51818",
  },
};