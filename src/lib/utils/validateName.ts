/**
 * Mirrors `validate_name` in `src-tauri/src/commands/fs.rs`. This copy only
 * exists to give instant inline feedback in the rename/create input before
 * a round trip — the Rust copy is the actual boundary, since this one can
 * always be bypassed.
 */

const INVALID_CHARS = /[<>:"/\\|?*\x00-\x1F]/;

const WINDOWS_RESERVED = new Set([
    "CON",
    "PRN",
    "AUX",
    "NUL",
    "COM1",
    "COM2",
    "COM3",
    "COM4",
    "COM5",
    "COM6",
    "COM7",
    "COM8",
    "COM9",
    "LPT1",
    "LPT2",
    "LPT3",
    "LPT4",
    "LPT5",
    "LPT6",
    "LPT7",
    "LPT8",
    "LPT9",
]);

/** Returns an error message, or `null` if the name is valid. */
export function validateEntryName(name: string): string | null {
    const trimmed = name.trim();
    if (!trimmed) return "Name can't be empty.";
    if (trimmed === "." || trimmed === "..") return "That name isn't allowed.";
    if (INVALID_CHARS.test(trimmed)) return "Name can't contain: < > : \" / \\ | ? *";
    if (trimmed.endsWith(".") || trimmed.endsWith(" ")) {
        return "Name can't end with a space or period on Windows.";
    }
    const base = trimmed.split(".")[0].toUpperCase();
    if (WINDOWS_RESERVED.has(base)) return `"${base}" is a reserved name on Windows.`;
    return null;
}
