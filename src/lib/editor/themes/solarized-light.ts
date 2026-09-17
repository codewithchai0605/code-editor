import type { Theme } from "./types";

export const solarizedLightTheme: Theme = {
    id: "solarized-light",
    name: "Solarized Light",
    dark: false,
    colors: {
        bg: "#fdf6e3",
        bgElevated: "#eee8d5",
        bgSidebar: "#eee8d5",
        bgActivitybar: "#eee8d5",
        bgStatusbar: "#268bd2",
        statusbarText: "#fdf6e3",
        bgHover: "rgba(0, 0, 0, 0.05)",
        border: "#eee8d5",
        text: "#657b83",
        textMuted: "#93a1a1",
        textFaint: "#93a1a1",
        accent: "#268bd2",
        focusRing: "#6c71c4",

        selection: "#eee8d5",
        activeLine: "rgba(0, 0, 0, 0.03)",

        comment: "#93a1a1",
        keyword: "#859900",
        controlKeyword: "#859900",
        variable: "#268bd2",
        function: "#268bd2",
        type: "#b58900",
        number: "#d33682",
        string: "#2aa198",
        regexp: "#dc322f",
        operator: "#859900",
        tag: "#268bd2",
        attribute: "#b58900",
        invalid: "#dc322f",
    },
};
