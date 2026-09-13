import JSZip from "jszip";
import { readFileSync, writeFileSync, statSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";

const files = [
    "./src/",                 // folder
    "./src-tauri/src",        // folder
    "./scripts",              // folder
    "./ss",                   // folder
    "./src-tauri/Cargo.toml", // file
    "./package.json",         // file
    "./README.md",            // file
    "./vite.config.js",       // file
    "./svelte.config.js",     // file
    "./tsconfig.json",        // file
];

const zip = new JSZip();
const IGNORE = new Set([
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".svelte-kit",
    "icons"
]);

function addPath(path, baseDir = "..") {
    const stat = statSync(path);

    if (stat.isDirectory()) {
        const name = path.split(/[\\/]/).pop();

        if (name && IGNORE.has(name)) {
            console.log(`Skipping: ${path}`);
            return;
        }

        for (const entry of readdirSync(path)) {
            addPath(join(path, entry), baseDir);
        }

        return;
    }

    if (stat.isFile()) {
        const zipPath = relative(baseDir, path).replaceAll("\\", "/");

        console.log(`Adding: ${zipPath}`);

        zip.file(zipPath, readFileSync(path));
    }
}

for (const path of files) {
    addPath(path);
}

const output = join(process.cwd(), "code-editor.zip");

const buffer = await zip.generateAsync({
    type: "nodebuffer",
    compression: "DEFLATE",
    compressionOptions: {
        level: 9,
    },
});

writeFileSync(output, buffer);

console.log(`\nZIP created: ${output}`);