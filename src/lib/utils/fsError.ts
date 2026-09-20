import { isFsError } from "$lib/types/fs";
import { baseName } from "./path";

/**
 * Turns whatever a failed `fs.ts` command rejected with into a message
 * worth showing someone. Was previously duplicated (and subtly
 * inconsistent) across `workspace.svelte.ts` and `editor.svelte.ts`.
 *
 * The `conflict` kind is the one case that needs real handling here rather
 * than just forwarding `message`: `FsError::Conflict` on the Rust side only
 * ever carries `paths`, never a `message` (see fs.rs — it's also used by
 * copy/move, which build their own dialog from `paths` and never render a
 * string at all), so reading `.message` for it was always going to fall
 * through to the generic fallback below. That's the exact "something went
 * wrong" a name collision was showing instead of "that name already
 * exists".
 */
export function describeFsError(err: unknown): string {
    if (isFsError(err)) {
        if (err.kind === "conflict") {
            const names = (err.paths ?? []).map(baseName);
            if (names.length === 1) return `"${names[0]}" already exists in this folder.`;
            if (names.length > 1) {
                return `${names.length} items already exist in the destination folder.`;
            }
            return "That name already exists in this folder.";
        }
        return err.message ?? "Something went wrong.";
    }
    return err instanceof Error ? err.message : String(err);
}