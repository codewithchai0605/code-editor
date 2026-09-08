<script lang="ts">
  import { onMount } from "svelte";
  import { layout } from "$lib/stores/layout.svelte";
  import { editorStore } from "$lib/stores/editor.svelte";
  import { workspace } from "$lib/stores/workspace.svelte";
  import { paletteState } from "$lib/stores/palette.svelte";
  import { themeStore } from "$lib/stores/theme.svelte";
  import { markdownPreviewStore } from "$lib/stores/markdownPreview.svelte";
  import { registerCoreCommands } from "$lib/commands/coreCommands";
  import ActivityBar from "./ActivityBar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import EditorArea from "./EditorArea.svelte";
  import Panel from "./Panel.svelte";
  import StatusBar from "./StatusBar.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import PaletteHost from "./PaletteHost.svelte";

  onMount(() => {
    themeStore.applyInitial();

    const unregisterCommands = registerCoreCommands();
    workspace.initFsWatcherListener();

    const updateSize = () => layout.setWindowSize(window.innerWidth, window.innerHeight);
    updateSize();
    window.addEventListener("resize", updateSize);

    // Base keybindings. Several of these are also registered as commands
    // (see coreCommands.ts) purely for Command Palette discoverability —
    // this is still what actually binds the key. See the note at the top
    // of coreCommands.ts for why those two aren't more deeply unified yet.
    function onKeydown(e: KeyboardEvent) {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      if (e.key.toLowerCase() === "b") {
        e.preventDefault();
        layout.toggleSidebar();
      } else if (e.key === "`") {
        e.preventDefault();
        layout.togglePanel();
      } else if (e.key.toLowerCase() === "s") {
        e.preventDefault();
        editorStore.save();
      } else if (e.key.toLowerCase() === "p" && e.shiftKey) {
        e.preventDefault();
        paletteState.toggle("commandPalette");
      } else if (e.key.toLowerCase() === "v" && e.shiftKey) {
        e.preventDefault();
        markdownPreviewStore.toggle();
      } else if (e.key.toLowerCase() === "p") {
        e.preventDefault();
        if (workspace.rootPath) paletteState.toggle("quickOpen");
      }
    }
    window.addEventListener("keydown", onKeydown);

    return () => {
      unregisterCommands();
      window.removeEventListener("resize", updateSize);
      window.removeEventListener("keydown", onKeydown);
    };
  });
</script>

<div class="shell">
  <div class="main-row">
    <ActivityBar />

    {#if layout.effectiveSidebarOpen}
      <Sidebar />
      <ResizeHandle orientation="vertical" onDrag={(dx) => layout.setSidebarWidth(layout.sidebarWidth + dx)} />
    {/if}

    <div class="main-column">
      <EditorArea />
      {#if layout.panelOpen}
        <ResizeHandle orientation="horizontal" onDrag={(dy) => layout.setPanelHeight(layout.panelHeight - dy)} />
      {/if}
      <!-- Always mounted (visibility toggled via CSS inside Panel itself)
           rather than `{#if layout.panelOpen}` — every terminal session's
           xterm.js instance lives inside Panel's tree, and unmounting it
           on every `Ctrl+\`` would destroy and recreate the terminal UI
           each time, wiping its scrollback/screen even though the actual
           shell process behind it is still alive. See Panel.svelte. -->
      <Panel />
    </div>
  </div>

  <StatusBar />
</div>

<PaletteHost />

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .main-row {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
  }
  .main-column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
</style>
