<script lang="ts">
  import { onMount } from "svelte";
  import "@xterm/xterm/css/xterm.css";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { listen } from "@tauri-apps/api/event";
  import { writeToTerminal, resizeTerminal } from "$lib/api/pty";
  import { terminalStore } from "$lib/stores/terminal.svelte";
  import { themeStore } from "$lib/stores/theme.svelte";
  import { buildTerminalTheme } from "$lib/editor/themeEngine";

  interface Props {
    id: string;
    active: boolean;
  }
  let { id, active }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;

  onMount(() => {
    term = new Terminal({
      fontFamily: "var(--font-mono)",
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: buildTerminalTheme(themeStore.theme),
    });
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    if (container) term.open(container);

    term.onData((data) => {
      writeToTerminal(id, data);
    });

    let lastCols = 0;
    let lastRows = 0;
    function doFit() {
      // Fitting a hidden (display:none, zero-size) terminal produces a
      // nonsensical size — only the active one should ever be fit. The
      // effect below re-fits this instance the moment it becomes active.
      if (!term || !fitAddon || !active) return;
      fitAddon.fit();
      if (term.cols !== lastCols || term.rows !== lastRows) {
        lastCols = term.cols;
        lastRows = term.rows;
        resizeTerminal(id, term.cols, term.rows);
      }
    }
    doFit();

    const resizeObserver = new ResizeObserver(doFit);
    if (container) resizeObserver.observe(container);

    let unlistenData: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    (async () => {
      unlistenData = await listen<number[]>(`pty:data:${id}`, (event) => {
        term?.write(new Uint8Array(event.payload));
      });
      unlistenExit = await listen(`pty:exit:${id}`, () => {
        terminalStore.markExited(id);
      });
    })();

    return () => {
      resizeObserver.disconnect();
      unlistenData?.();
      unlistenExit?.();
      term?.dispose();
    };
  });

  // Re-fit + refocus whenever this session becomes the visible one — its
  // container may have been display:none (zero size) while inactive, so
  // dimensions need recomputing now that it's actually on screen.
  $effect(() => {
    if (active) {
      queueMicrotask(() => {
        fitAddon?.fit();
        term?.focus();
      });
    }
  });

  // Live re-theme: xterm.js supports reassigning `.options.theme` after
  // construction to repaint immediately, same as the CodeMirror side.
  $effect(() => {
    if (term) term.options.theme = buildTerminalTheme(themeStore.theme);
  });
</script>

<div class="terminal-container" bind:this={container}></div>

<style>
  .terminal-container {
    height: 100%;
    padding: 4px 8px;
  }
</style>
