import type { Theme } from "./types";

export const oneDarkTheme: Theme = {
    id: "one-dark",
    name: "One Dark",
    dark: true,
    colors: {
        bg: "#282c34",
        bgElevated: "#21252b",
        bgSidebar: "#21252b",
        bgActivitybar: "#21252b",
        bgStatusbar: "#61afef",
        statusbarText: "#282c34",
        bgHover: "rgba(255, 255, 255, 0.06)",
        border: "#3b4048",
        text: "#abb2bf",
        textMuted: "#5c6370",
        textFaint: "#5c6370",
        accent: "#61afef",
        focusRing: "#c678dd",

        selection: "#3e4451",
        activeLine: "rgba(255, 255, 255, 0.04)",

        comment: "#5c6370",
        keyword: "#c678dd",
        controlKeyword: "#c678dd",
        variable: "#e06c75",
        function: "#61afef",
        type: "#e5c07b",
        number: "#d19a66",
        string: "#98c379",
        regexp: "#56b6c2",
        operator: "#56b6c2",
        tag: "#e06c75",
        attribute: "#d19a66",
        invalid: "#f44747",
    },
};