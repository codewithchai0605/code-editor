<script lang="ts">
  import { onMount } from "svelte";
  import "@xterm/xterm/css/xterm.css";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { listen } from "@tauri-apps/api/event";
  import { createTerminal, writeToTerminal, resizeTerminal } from "$lib/api/pty";
  import { terminalStore } from "$lib/stores/terminal.svelte";
  import { themeStore } from "$lib/stores/theme.svelte";
  import { layout } from "$lib/stores/layout.svelte";
  import { buildTerminalTheme } from "$lib/editor/themeEngine";
  import { resolveCssVar } from "$lib/utils/cssVar";

  interface Props {
    id: string;
    active: boolean;
    cwd?: string;
  }
  let { id, active, cwd }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;

  onMount(() => {
    term = new Terminal({
      // xterm renders to a <canvas>, whose 2D context `font` property
      // doesn't understand `var(...)` the way a real CSS rule does — pass
      // the resolved literal font stack instead. See cssVar.ts.
      fontFamily: resolveCssVar("--font-mono", "monospace"),
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: buildTerminalTheme(themeStore.theme),
    });
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    if (container) term.open(container);

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
        // The very first call here happens synchronously in onMount, before
        // the async spawn below has told the backend this session exists —
        // that one resize is expected to 404 against `resize_terminal`'s
        // session lookup. Harmless (the PTY opens at a sane default size
        // regardless, and every subsequent real resize succeeds normally
        // once the session exists), so it's swallowed rather than left to
        // surface as an unhandled rejection in the console.
        resizeTerminal(id, term.cols, term.rows).catch(() => {});
      }
    }
    doFit();

    const resizeObserver = new ResizeObserver(doFit);
    if (container) resizeObserver.observe(container);

    let unlistenData: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    (async () => {
      // Attach both listeners — and *await* that attachment completing —
      // before telling the backend to actually spawn anything. The old
      // flow spawned the shell first (as part of the single `create_terminal`
      // call that also generated the id) and only attached this listener
      // afterwards; a fast-starting shell could write its entire initial
      // prompt in that window, and since Tauri doesn't buffer or replay
      // events for a listener that attaches late, that output was simply
      // gone — an xterm instance with a blinking cursor and nothing else.
      // `id` here was already reserved by terminal.svelte.ts's
      // `createSession` before this component ever mounted, specifically
      // so there's a stable id to listen on before anything exists to spawn.
      unlistenData = await listen<number[]>(`pty:data:${id}`, (event) => {
        term?.write(new Uint8Array(event.payload));
      });
      unlistenExit = await listen(`pty:exit:${id}`, () => {
        terminalStore.markExited(id);
      });

      term?.onData((data) => {
        writeToTerminal(id, data);
      });

      try {
        await createTerminal(id, cwd);
      } catch (err) {
        term?.write(`\r\n\x1b[31mFailed to start shell: ${String(err)}\x1b[0m\r\n`);
      }
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
  // dimensions need recomputing now that it's actually on screen. Also
  // re-runs when the whole panel opens/closes: Panel.svelte now stays
  // mounted permanently (see its comment), toggling visibility via CSS,
  // and a container going from display:none straight to its final size
  // does fire ResizeObserver in every engine tested — but re-fitting here
  // too costs nothing and removes any dependency on that timing being
  // reliable across every WebView this ships on.
  $effect(() => {
    if (active && layout.panelOpen) {
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
