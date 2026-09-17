import type { Theme } from "./types";

export const nordTheme: Theme = {
    id: "nord",
    name: "Nord",
    dark: true,
    colors: {
        bg: "#2e3440",
        bgElevated: "#3b4252",
        bgSidebar: "#2e3440",
        bgActivitybar: "#2e3440",
        bgStatusbar: "#5e81ac",
        statusbarText: "#eceff4",
        bgHover: "rgba(255, 255, 255, 0.06)",
        border: "#3b4252",
        text: "#d8dee9",
        textMuted: "#4c566a",
        textFaint: "#4c566a",
        accent: "#88c0d0",
        focusRing: "#81a1c1",

        selection: "#434c5e",
        activeLine: "rgba(255, 255, 255, 0.04)",

        comment: "#616e88",
        keyword: "#81a1c1",
        controlKeyword: "#81a1c1",
        variable: "#d8dee9",
        function: "#88c0d0",
        type: "#8fbcbb",
        number: "#b48ead",
        string: "#a3be8c",
        regexp: "#ebcb8b",
        operator: "#81a1c1",
        tag: "#81a1c1",
        attribute: "#8fbcbb",
        invalid: "#bf616a",
    },
};