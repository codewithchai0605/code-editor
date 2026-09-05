import { readTextFile } from "$lib/api/fs";
import { isFsError } from "$lib/types/fs";

/**
 * Temporary bridge until the next phase's EditorFile/tabs land: clicking a
 * file shows a single read-only preview instead of opening a real tab.
 * Deliberately minimal — no multi-file state, no editing.
 */
class PreviewState {
  path = $state<string | null>(null);
  content = $state<string | null>(null);
  error = $state<string | null>(null);
  loading = $state(false);

  async open(path: string): Promise<void> {
    this.path = path;
    this.content = null;
    this.error = null;
    this.loading = true;
    try {
      this.content = await readTextFile(path);
    } catch (err) {
      this.error = isFsError(err) ? (err.message ?? "Couldn't open this file.") : String(err);
    } finally {
      this.loading = false;
    }
  }

  close(): void {
    this.path = null;
    this.content = null;
    this.error = null;
  }
}

export const preview = new PreviewState();
