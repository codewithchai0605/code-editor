import type { Theme } from "./types";

export const oneLightTheme: Theme = {
    id: "one-light",
    name: "One Light",
    dark: false,
    colors: {
        bg: "#fafafa",
        bgElevated: "#f0f0f1",
        bgSidebar: "#f0f0f1",
        bgActivitybar: "#f0f0f1",
        bgStatusbar: "#526fff",
        statusbarText: "#fafafa",
        bgHover: "rgba(0, 0, 0, 0.05)",
        border: "#e5e5e6",
        text: "#383a42",
        textMuted: "#a0a1a7",
        textFaint: "#a0a1a7",
        accent: "#526fff",
        focusRing: "#a626a4",

        selection: "#e5e5e6",
        activeLine: "rgba(0, 0, 0, 0.03)",

        comment: "#a0a1a7",
        keyword: "#a626a4",
        controlKeyword: "#a626a4",
        variable: "#e45649",
        function: "#4078f2",
        type: "#c18401",
        number: "#986801",
        string: "#50a14f",
        regexp: "#0184bc",
        operator: "#0184bc",
        tag: "#e45649",
        attribute: "#986801",
        invalid: "#ca1243",
    },
};
