import type { Theme } from "./types";

export const tokyoNightTheme: Theme = {
    id: "tokyo-night",
    name: "Tokyo Night",
    dark: true,
    colors: {
        bg: "#1a1b26",
        bgElevated: "#16161e",
        bgSidebar: "#16161e",
        bgActivitybar: "#16161e",
        bgStatusbar: "#7aa2f7",
        statusbarText: "#1a1b26",
        bgHover: "rgba(255, 255, 255, 0.06)",
        border: "#292e42",
        text: "#a9b1d6",
        textMuted: "#565f89",
        textFaint: "#565f89",
        accent: "#7aa2f7",
        focusRing: "#bb9af7",

        selection: "#33467c",
        activeLine: "rgba(255, 255, 255, 0.04)",

        comment: "#565f89",
        keyword: "#bb9af7",
        controlKeyword: "#bb9af7",
        variable: "#c0caf5",
        function: "#7aa2f7",
        type: "#2ac3de",
        number: "#ff9e64",
        string: "#9ece6a",
        regexp: "#b4f9f8",
        operator: "#89ddff",
        tag: "#f7768e",
        attribute: "#e0af68",
        invalid: "#db4b4b",
    },
};