import type { Theme } from "./types";

export const monokaiTheme: Theme = {
    id: "monokai",
    name: "Monokai",
    dark: true,
    colors: {
        bg: "#272822",
        bgElevated: "#1e1f1c",
        bgSidebar: "#1e1f1c",
        bgActivitybar: "#1e1f1c",
        bgStatusbar: "#a6e22e",
        statusbarText: "#272822",
        bgHover: "rgba(255, 255, 255, 0.08)",
        border: "#3e3d32",
        text: "#f8f8f2",
        textMuted: "#75715e",
        textFaint: "#75715e",
        accent: "#a6e22e",
        focusRing: "#fd971f",

        selection: "#49483e",
        activeLine: "rgba(255, 255, 255, 0.05)",

        comment: "#75715e",
        keyword: "#f92672",
        controlKeyword: "#f92672",
        variable: "#fd971f",
        function: "#a6e22e",
        type: "#66d9ef",
        number: "#ae81ff",
        string: "#e6db74",
        regexp: "#f92672",
        operator: "#f92672",
        tag: "#f92672",
        attribute: "#a6e22e",
        invalid: "#f8f8f0",
    },
};
