<script lang="ts">
    import { onMount } from "svelte";
    import "@xterm/xterm/css/xterm.css";
    import { Terminal } from "@xterm/xterm";
    import { FitAddon } from "@xterm/addon-fit";
    import { WebLinksAddon } from "@xterm/addon-web-links";
    // HIGHLY RECOMMENDED for performance: Uncomment the next two lines and install `@xterm/addon-canvas`
    import { CanvasAddon } from "@xterm/addon-canvas";

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

    let resizeTimeout: ReturnType<typeof setTimeout> | null = null;
    let resizeRaf: number | null = null;

    onMount(() => {
        term = new Terminal({
            fontFamily: resolveCssVar("--font-mono", "monospace"),
            fontSize: 13,
            cursorBlink: true,
            scrollback: 5000,
            convertEol: true, // Normalizes \n to \r\n for consistent rendering across shells
            theme: buildTerminalTheme(themeStore.theme),
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);

        // Enables Ctrl/Cmd + Click on URLs and file paths
        term.loadAddon(new WebLinksAddon());

        // UNCOMMENT for massive performance gains on large outputs (like `cat` or build logs)
        term.loadAddon(new CanvasAddon());

        if (container) term.open(container);

        let lastCols = 0;
        let lastRows = 0;

        function doFit() {
            if (!term || !fitAddon || !active || !layout.panelOpen) return;

            fitAddon.fit();
            if (term.cols !== lastCols || term.rows !== lastRows) {
                lastCols = term.cols;
                lastRows = term.rows;

                // Debounce the IPC call to avoid flooding the backend during rapid window resize
                if (resizeTimeout) clearTimeout(resizeTimeout);
                resizeTimeout = setTimeout(() => {
                    resizeTerminal(id, term!.cols, term!.rows).catch(() => {});
                }, 50);
            }
        }

        // Initial fit after DOM is ready
        queueMicrotask(() => doFit());

        const resizeObserver = new ResizeObserver(() => {
            // Debounce ResizeObserver using requestAnimationFrame to prevent layout thrashing
            if (resizeRaf) return;
            resizeRaf = requestAnimationFrame(() => {
                doFit();
                resizeRaf = null;
            });
        });

        if (container) resizeObserver.observe(container);

        let unlistenData: (() => void) | undefined;
        let unlistenExit: (() => void) | undefined;

        (async () => {
            unlistenData = await listen<number[]>(`pty:data:${id}`, (event) => {
                // Tauri v2 passes Rust's Vec<u8> as Uint8Array.
                // Fallback to array conversion just in case of serialization quirks.
                const payload =
                    event.payload instanceof Uint8Array
                        ? event.payload
                        : new Uint8Array(event.payload as number[]);
                term?.write(payload);
            });

            unlistenExit = await listen(`pty:exit:${id}`, () => {
                terminalStore.markExited(id);
            });

            term?.onData((data) => {
                // Sending string is actually optimal here: keystrokes are tiny,
                // and the Rust side efficiently handles `.as_bytes()` without JS overhead.
                writeToTerminal(id, data);
            });

            try {
                await createTerminal(id, cwd);
                // Fit again after the shell spawns and emits its initial prompt
                queueMicrotask(() => doFit());
            } catch (err) {
                term?.write(`\r\n\x1b[31mFailed to start shell: ${String(err)}\x1b[0m\r\n`);
                terminalStore.markExited(id);
            }
        })();

        return () => {
            if (resizeTimeout) clearTimeout(resizeTimeout);
            if (resizeRaf) cancelAnimationFrame(resizeRaf);
            resizeObserver.disconnect();
            unlistenData?.();
            unlistenExit?.();
            term?.dispose();
            // Note: We intentionally do NOT call closeTerminal(id) here.
            // The store manages the lifecycle and calls it via closeSession/markExited.
        };
    });

    // Re-fit + refocus whenever this session becomes the visible one
    $effect(() => {
        if (active && layout.panelOpen) {
            queueMicrotask(() => {
                fitAddon?.fit();
                term?.focus();
            });
        }
    });

    // Live re-theme
    $effect(() => {
        if (term) {
            term.options.theme = buildTerminalTheme(themeStore.theme);
        }
    });
</script>

<div class="terminal-container" bind:this={container}></div>

<style>
    .terminal-container {
        height: 100%;
        width: 100%;
        padding: 4px 8px;
        /* Prevents selecting UI chrome, making it feel like a native app */
        user-select: none;
    }
    /* Re-enable text selection specifically for the terminal output */
    .terminal-container :global(.xterm) {
        user-select: text;
    }
</style>
