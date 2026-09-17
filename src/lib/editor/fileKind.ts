import { extensionOf } from "$lib/utils/fileIcons";

export type FileKind = "text" | "image" | "audio" | "binary";

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"]);
const AUDIO_EXTS = new Set(["mp3", "wav", "ogg", "flac", "m4a", "aac", "weba"]);

/** "binary" is never guessed up front — it's only reached after a text
 *  read actually fails (non-UTF-8 or too large). Everything else defaults
 *  to "text" and lets the backend be the judge. */
export function guessFileKind(fileName: string): "image" | "audio" | "text" {
    const ext = extensionOf(fileName);
    if (IMAGE_EXTS.has(ext)) return "image";
    if (AUDIO_EXTS.has(ext)) return "audio";
    return "text";
}
