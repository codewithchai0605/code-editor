import { commandRegistry } from "./registry.svelte";
import { layout } from "$lib/stores/layout.svelte";
import { workspace } from "$lib/stores/workspace.svelte";
import { editorStore } from "$lib/stores/editor.svelte";
import { paletteState } from "$lib/stores/palette.svelte";
import { markdownPreviewStore } from "$lib/stores/markdownPreview.svelte";
import {
  moveLineUp,
  moveLineDown,
  copyLineUp,
  copyLineDown,
} from "@codemirror/commands";
import { selectNextOccurrence } from "@codemirror/search";
import { goToNextGitChange, goToPreviousGitChange } from "$lib/editor/git-gutter";
import type { EditorView } from "@codemirror/view";

/** Runs a CodeMirror command against the currently-visible editor, if
 *  there is one — the shared wrapper every editor-scoped command below
 *  goes through, since (unlike `workspace`/`layout` commands) these need
 *  a live `EditorView` rather than just app state. See editorStore.view. */
function onActiveView(run: (view: EditorView) => boolean) {
  return () => {
    if (editorStore.view) run(editorStore.view);
  };
}

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
      id: "workbench.findInFiles",
      title: "View: Find in Files",
      keybinding: "Ctrl+Shift+F",
      run: () => {
        if (workspace.rootPath) layout.setActiveView("search");
      },
    }),
    commandRegistry.register({
      id: "workbench.togglePanel",
      title: "View: Toggle Panel",
      keybinding: "Ctrl+`",
      run: () => layout.togglePanel(),
    }),
    commandRegistry.register({
      id: "workbench.selectTheme",
      title: "Preferences: Color Theme",
      run: () => paletteState.open("theme"),
    }),
    commandRegistry.register({
      id: "editor.moveLineUp",
      title: "Line: Move Up",
      keybinding: "Alt+↑",
      run: onActiveView(moveLineUp),
    }),
    commandRegistry.register({
      id: "editor.moveLineDown",
      title: "Line: Move Down",
      keybinding: "Alt+↓",
      run: onActiveView(moveLineDown),
    }),
    commandRegistry.register({
      id: "editor.copyLineUp",
      title: "Line: Copy Up",
      keybinding: "Shift+Alt+↑",
      run: onActiveView(copyLineUp),
    }),
    commandRegistry.register({
      id: "editor.copyLineDown",
      title: "Line: Copy Down",
      keybinding: "Shift+Alt+↓",
      run: onActiveView(copyLineDown),
    }),
    commandRegistry.register({
      id: "editor.selectNextOccurrence",
      title: "Selection: Select Next Occurrence",
      keybinding: "Ctrl+D",
      run: onActiveView(selectNextOccurrence),
    }),
    commandRegistry.register({
      id: "editor.gitGoToNextChange",
      title: "Source Control: Go to Next Change",
      keybinding: "Alt+F3",
      run: onActiveView(goToNextGitChange),
    }),
    commandRegistry.register({
      id: "editor.gitGoToPreviousChange",
      title: "Source Control: Go to Previous Change",
      keybinding: "Shift+Alt+F3",
      run: onActiveView(goToPreviousGitChange),
    }),
    commandRegistry.register({
      id: "markdown.togglePreview",
      title: "Markdown: Toggle Preview",
      keybinding: "Ctrl+Shift+V",
      run: () => markdownPreviewStore.toggle(),
    }),
  ];

  return () => unregisters.forEach((unregister) => unregister());
}