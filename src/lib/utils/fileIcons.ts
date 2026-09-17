/**
 * A full per-filetype icon set is a nice later addition (and a natural fit
 * for an extension-contributed "icon theme" once the plugin system lands).
 * For now we keep it to zero assets/dependencies: one generic file glyph,
 * tinted by extension so the tree is still easy to scan.
 */

const COLOR_BY_EXT: Record<string, string> = {
    // Scripts / code
    js: "#f1e05a",
    mjs: "#f1e05a",
    cjs: "#f1e05a",
    jsx: "#f1e05a",
    ts: "#3178c6",
    tsx: "#3178c6",
    rs: "#dea584",
    py: "#3572a5",
    go: "#00add8",
    java: "#b07219",
    kt: "#a97bff",
    c: "#555555",
    h: "#555555",
    cpp: "#f34b7d",
    cc: "#f34b7d",
    hpp: "#f34b7d",
    cs: "#178600",
    rb: "#701516",
    php: "#4f5d95",
    sh: "#89e051",
    bash: "#89e051",
    // Data / config
    json: "#cbcb41",
    toml: "#9c4221",
    yaml: "#cb171e",
    yml: "#cb171e",
    xml: "#0060ac",
    // Docs
    md: "#83a3c1",
    mdx: "#83a3c1",
    txt: "#969696",
    // Web
    html: "#e34c26",
    css: "#563d7c",
    scss: "#c6538c",
    less: "#1d365d",
    svelte: "#ff3e00",
    vue: "#41b883",
    // Media
    png: "#a074c4",
    jpg: "#a074c4",
    jpeg: "#a074c4",
    gif: "#a074c4",
    svg: "#ffb13b",
    webp: "#a074c4",
    mp3: "#e07a5f",
    wav: "#e07a5f",
    ogg: "#e07a5f",
    flac: "#e07a5f",
};

export function extensionOf(fileName: string): string {
    const idx = fileName.lastIndexOf(".");
    return idx > 0 ? fileName.slice(idx + 1).toLowerCase() : "";
}

export function fileIconColor(fileName: string): string {
    return COLOR_BY_EXT[extensionOf(fileName)] ?? "var(--color-text-faint)";
}
