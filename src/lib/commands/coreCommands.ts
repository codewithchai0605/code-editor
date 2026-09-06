import { commandRegistry } from "./registry.svelte";
import { layout } from "$lib/stores/layout.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { editorStore } from "$lib/stores/editor.svelte";

/**
 * The `keybinding` field on several of these is a *display hint* for the
 * palette, matching a shortcut that's already bound directly (Ctrl+S in
 * both Shell.svelte and editor/setup.ts; Ctrl+B/Ctrl+` in Shell.svelte) —
 * it does not itself bind the key. A fully unified "the registry is the
 * single source of truth for every keybinding" design would mean routing
 * CodeMirror's own internal key handling through this registry too, which
 * is a bigger refactor than Quick Open + Command Palette need in order to
 * ship. Worth doing later if/when the keybinding surface grows (e.g. once
 * extensions can contribute their own shortcuts).
 */
export function registerCoreCommands(): () => void {
  const unregisters = [
    commandRegistry.register({
      id: "workbench.openFolder",
      title: "File: Open Folder...",
      run: () => workspace.pickAndOpenFolder(),
    }),
    commandRegistry.register({
      id: "workbench.newFile",
      title: "File: New File",
      run: () => {
        if (workspace.rootPath) workspace.startCreate(workspace.rootPath, false);
      },
    }),
    commandRegistry.register({
      id: "workbench.newFolder",
      title: "File: New Folder",
      run: () => {
        if (workspace.rootPath) workspace.startCreate(workspace.rootPath, true);
      },
    }),
    commandRegistry.register({
      id: "workbench.saveFile",
      title: "File: Save",
      keybinding: "Ctrl+S",
      run: () => editorStore.save(),
    }),
    commandRegistry.register({
      id: "workbench.closeEditor",
      title: "View: Close Editor",
      run: () => {
        if (editorStore.activePath) editorStore.requestClose(editorStore.activePath);
      },
    }),
    commandRegistry.register({
      id: "workbench.toggleSidebar",
      title: "View: Toggle Sidebar",
      keybinding: "Ctrl+B",
      run: () => layout.toggleSidebar(),
    }),
    commandRegistry.register({
      id: "workbench.togglePanel",
      title: "View: Toggle Panel",
      keybinding: "Ctrl+`",
      run: () => layout.togglePanel(),
    }),
  ];

  return () => unregisters.forEach((unregister) => unregister());
}
