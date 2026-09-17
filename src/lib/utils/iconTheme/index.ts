import { fileIcons } from "./file-icons-data.js";
import { folderIcons } from "./folder-icons-data.js";

export interface FileIconRule {
    name: string;
    file_name?: string[];
    file_extensions?: string[];
}
export interface FolderIconRule {
    name: string;
    folder_name: string[];
}

const iconUrls = import.meta.glob("/src/lib/assets/icons/*.svg", {
    eager: true,
    query: "?url",
    import: "default",
}) as Record<string, string>;

// Keyed by lowercased filename stem, not the raw glob path — a lookup like
// urlForIconName("typescript") is an exact property access against
// whatever case the data file uses for `name`, while the actual SVG on
// disk might be `TypeScript.svg`. Lookups are case-sensitive regardless
// of the OS's own filesystem case-sensitivity, so that mismatch would
// silently resolve to nothing.
const iconUrlByLowerName = new Map<string, string>();
for (const [path, url] of Object.entries(iconUrls)) {
    const stem = path.slice(path.lastIndexOf("/") + 1, -".svg".length).toLowerCase();
    iconUrlByLowerName.set(stem, url);
}

function urlForIconName(name: string): string | undefined {
    return iconUrlByLowerName.get(name.toLowerCase());
}

interface FileIconIndex {
    byExactName: Map<string, string>;
    byExtension: Map<string, string>;
}

function buildFileIndex(rules: FileIconRule[]): FileIconIndex {
    const byExactName = new Map<string, string>();
    const byExtension = new Map<string, string>();
    for (const rule of rules) {
        for (const name of rule.file_name ?? []) byExactName.set(name.toLowerCase(), rule.name);
        for (const ext of rule.file_extensions ?? []) byExtension.set(ext.toLowerCase(), rule.name);
    }
    return { byExactName, byExtension };
}

function buildFolderIndex(rules: FolderIconRule[]): Map<string, string> {
    const byName = new Map<string, string>();
    for (const rule of rules) {
        for (const name of rule.folder_name) byName.set(name.toLowerCase(), rule.name);
    }
    return byName;
}

const fileIndex = buildFileIndex(fileIcons as FileIconRule[]);
const folderIndex = buildFolderIndex(folderIcons as FolderIconRule[]);

export function resolveFileIconName(fileName: string): string | undefined {
    const lower = fileName.toLowerCase();

    // 1. Exact filename rule wins.
    const exact = fileIndex.byExactName.get(lower);
    if (exact) return exact;

    // 2. Try compound extensions from longest to shortest.
    const parts = lower.split(".");

    for (let i = 1; i < parts.length; i++) {
        const extension = parts.slice(i).join(".");
        const iconName = fileIndex.byExtension.get(extension);

        if (iconName) return iconName;
    }

    return undefined;
}

export function fileIconUrl(fileName: string): string | undefined {
    const name = resolveFileIconName(fileName);
    return name ? urlForIconName(name) : undefined;
}

export function folderIconUrl(folderName: string, isOpen: boolean): string | undefined {
    const name = folderIndex.get(folderName.toLowerCase());
    if (!name) return undefined;
    if (isOpen) return urlForIconName(`${name}-open`) ?? urlForIconName(name);
    return urlForIconName(name);
}
