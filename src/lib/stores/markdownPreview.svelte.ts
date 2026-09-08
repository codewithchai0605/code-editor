/** Whether the Markdown preview pane is open. A single global toggle
 *  (mirroring `layout.panelOpen`) rather than one flag per tab — simpler,
 *  and it's the same convention VS Code's own "open the last preview
 *  state" default roughly matches for a single active editor. */
class MarkdownPreviewStore {
  open = $state(false);

  toggle(): void {
    this.open = !this.open;
  }
}

export const markdownPreviewStore = new MarkdownPreviewStore();
