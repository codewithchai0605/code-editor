<script lang="ts">
  import { onMount } from "svelte";
  import { layout } from "$lib/stores/layout.svelte";
  import { editorStore } from "$lib/stores/editor.svelte";
  import ActivityBar from "./ActivityBar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import EditorArea from "./EditorArea.svelte";
  import Panel from "./Panel.svelte";
  import StatusBar from "./StatusBar.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";

  onMount(() => {
    const updateSize = () => layout.setWindowSize(window.innerWidth, window.innerHeight);
    updateSize();
    window.addEventListener("resize", updateSize);

    // Base keybindings. A real command registry (so both core commands and
    // extension-contributed commands share one keybinding dispatch table)
    // arrives with the Command Palette phase — this is intentionally just
    // enough to make the shell feel usable today.
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
      }
    }
    window.addEventListener("keydown", onKeydown);

    return () => {
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
        <Panel />
      {/if}
    </div>
  </div>

  <StatusBar />
</div>

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
