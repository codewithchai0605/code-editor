import type { Theme } from "./types";

export const draculaTheme: Theme = {
    id: "dracula",
    name: "Dracula",
    dark: true,
    colors: {
        bg: "#282a36",
        bgElevated: "#21222c",
        bgSidebar: "#21222c",
        bgActivitybar: "#191a21",
        bgStatusbar: "#ff79c6",
        statusbarText: "#282a36",
        bgHover: "rgba(255, 255, 255, 0.08)",
        border: "#44475a",
        text: "#f8f8f2",
        textMuted: "#6272a4",
        textFaint: "#6272a4",
        accent: "#ff79c6",
        focusRing: "#bd93f9",

        selection: "#44475a",
        activeLine: "rgba(255, 255, 255, 0.05)",

        comment: "#6272a4",
        keyword: "#ff79c6",
        controlKeyword: "#ff79c6",
        variable: "#8be9fd",
        function: "#50fa7b",
        type: "#8be9fd",
        number: "#bd93f9",
        string: "#f1fa8c",
        regexp: "#ff79c6",
        operator: "#ff79c6",
        tag: "#ff79c6",
        attribute: "#50fa7b",
        invalid: "#ff5555",
    },
};
