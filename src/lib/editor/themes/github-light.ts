import type { Theme } from "./types";

export const githubLightTheme: Theme = {
    id: "github-light",
    name: "GitHub Light",
    dark: false,
    colors: {
        bg: "#ffffff",
        bgElevated: "#f6f8fa",
        bgSidebar: "#f6f8fa",
        bgActivitybar: "#f6f8fa",
        bgStatusbar: "#0969da",
        statusbarText: "#ffffff",
        bgHover: "rgba(0, 0, 0, 0.05)",
        border: "#d0d7de",
        text: "#1f2328",
        textMuted: "#656d76",
        textFaint: "#8c959f",
        accent: "#0969da",
        focusRing: "#0969da",

        selection: "#b6e3ff",
        activeLine: "rgba(0, 0, 0, 0.03)",

        comment: "#6e7781",
        keyword: "#cf222e",
        controlKeyword: "#cf222e",
        variable: "#953800",
        function: "#8250df",
        type: "#0550ae",
        number: "#0550ae",
        string: "#0a3069",
        regexp: "#116329",
        operator: "#cf222e",
        tag: "#116329",
        attribute: "#953800",
        invalid: "#82071e",
    },
};