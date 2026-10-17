<script lang="ts">
    import type { Component } from "svelte";
    import { layout } from "$lib/stores/layout.svelte";
    import { workspace } from "$lib/stores/workspace.svelte";
    import { terminalStore } from "$lib/stores/terminal.svelte";
    import ProblemsPanel from "./ProblemsPanel.svelte";

    /** Which bottom-panel tab is showing — VS Code's TERMINAL/PROBLEMS split.
     *  Lives on the layout store rather than as component state because the
     *  status bar's Problems chip (Phase 3) needs to switch it from outside
     *  this component. */
    let activeTab = $derived(layout.panelTab);

    // xterm.js is a substantial library that most sessions may never need
    // (the panel starts closed) — deferring the import until the panel is
    // actually opened keeps it out of the main bundle entirely.
    let TerminalView = $state<Component<{ id: string; active: boolean; cwd?: string }> | null>(
        null,
    );

    let creatingFirst = false;
    $effect(() => {
        // Panel is now always mounted (see Shell.svelte) so this effect keeps
        // running after the panel is first closed — bail out each time rather
        // than only guarding the one-time setup below.
        if (!layout.panelOpen) return;

        if (!TerminalView) {
            import("$lib/components/terminal/TerminalView.svelte").then((mod) => {
                TerminalView = mod.default;
            });
        }

        // Lazily create the first terminal session the first time the panel
        // opens. `creatingFirst` guards against a rapid open/close/open of the
        // panel firing this twice before the first createSession() resolves.
        if (terminalStore.sessions.length === 0 && !creatingFirst) {
            creatingFirst = true;
            terminalStore.createSession(workspace.rootPath ?? undefined).finally(() => {
                creatingFirst = false;
            });
        }
    });
</script>

<section
    class="panel"
    class:closed={!layout.panelOpen}
    style:height="{layout.panelOpen ? layout.panelHeight : 0}px"
>
    <header class="panel-header">
        <div class="view-tabs" role="tablist" aria-label="Panel views">
            <button
                class="view-tab"
                class:active={activeTab === "problems"}
                role="tab"
                aria-selected={activeTab === "problems"}
                onclick={() => (layout.panelTab = "problems")}
            >PROBLEMS</button>
            <button
                class="view-tab"
                class:active={activeTab === "terminal"}
                role="tab"
                aria-selected={activeTab === "terminal"}
                onclick={() => (layout.panelTab = "terminal")}
            >TERMINAL</button>
        </div>
        <div class="tabs" role="tablist" aria-label="Terminal sessions">
            {#each terminalStore.sessions as session (session.id)}
                <div
                    class="tab"
                    class:active={session.id === terminalStore.activeId}
                    role="tab"
                    aria-selected={session.id === terminalStore.activeId}
                    tabindex="0"
                    onclick={() => terminalStore.setActive(session.id)}
                    onkeydown={(e) => e.key === "Enter" && terminalStore.setActive(session.id)}
                >
                    <span class="name" class:exited={session.exited}>{session.title}</span>
                    <button
                        class="close"
                        aria-label="Close {session.title}"
                        onclick={(e) => {
                            e.stopPropagation();
                            terminalStore.closeSession(session.id);
                        }}
                    >
                        &times;
                    </button>
                </div>
            {/each}
        </div>
        <button
            class="new-terminal"
            aria-label="New Terminal"
            onclick={() => terminalStore.createSession(workspace.rootPath ?? undefined)}
        >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
                <path d="M12 5v14M5 12h14" />
            </svg>
        </button>
    </header>

    <div class="panel-content">
        {#if activeTab === "problems"}
            <ProblemsPanel />
        {:else if TerminalView}
            {#each terminalStore.sessions as session (session.id)}
                <div class="terminal-slot" class:hidden={session.id !== terminalStore.activeId}>
                    <TerminalView
                        id={session.id}
                        active={session.id === terminalStore.activeId}
                        cwd={session.cwd}
                    />
                </div>
            {/each}
        {:else if terminalStore.sessions.length > 0}
            <p class="placeholder">Loading terminal&hellip;</p>
        {/if}
        {#if terminalStore.sessions.length === 0}
            <p class="placeholder">No terminal sessions.</p>
        {/if}
    </div>
</section>

<style>
    .panel {
        flex-shrink: 0;
        min-height: 100px;
        display: flex;
        flex-direction: column;
        background: var(--color-bg);
        border-top: 1px solid var(--color-border);
        overflow: hidden;
    }
    /* Hidden this way (rather than unmounted via `{#if}` in Shell.svelte) so
     every TerminalView instance underneath stays alive across a close/open
     of the panel — see the comment on <Panel /> in Shell.svelte. `min-height`
     above would otherwise still reserve 100px of dead space once `height`
     is driven to 0, so it needs its own override here. */
    .panel.closed {
        display: none;
    }
    .panel-header {
        flex-shrink: 0;
        height: 30px;
        display: flex;
        align-items: center;
        gap: 12px;
        justify-content: space-between;
        padding: 0 6px 0 12px;
        border-bottom: 1px solid var(--color-border);
        user-select: none;
    }
    .view-tabs {
        display: flex;
        align-items: center;
        gap: 2px;
    }
    .view-tab {
        background: none;
        border: none;
        color: inherit;
        opacity: 0.6;
        font-size: 11px;
        letter-spacing: 0.04em;
        padding: 4px 8px;
        cursor: pointer;
        border-bottom: 2px solid transparent;
    }
    .view-tab:hover { opacity: 0.9; }
    .view-tab.active {
        opacity: 1;
        border-bottom-color: var(--color-accent, #4d9eff);
    }
    .tabs {
        display: flex;
        align-items: center;
        gap: 4px;
        height: 100%;
        overflow-x: auto;
    }
    .tab {
        flex-shrink: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        height: 100%;
        padding: 0 4px 0 8px;
        font-size: 12px;
        color: var(--color-text-muted);
        border-bottom: 1px solid transparent;
        cursor: pointer;
    }
    .tab.active {
        color: var(--color-text);
        border-bottom-color: var(--color-accent);
    }
    .name.exited {
        text-decoration: line-through;
        opacity: 0.6;
    }
    .close {
        width: 16px;
        height: 16px;
        line-height: 1;
        font-size: 13px;
        border-radius: 3px;
        color: var(--color-text-muted);
    }
    .close:hover {
        background: var(--color-bg-hover);
        color: var(--color-text);
    }
    .new-terminal {
        flex-shrink: 0;
        width: 22px;
        height: 22px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 3px;
        color: var(--color-text-muted);
    }
    .new-terminal:hover {
        background: var(--color-bg-hover);
        color: var(--color-text);
    }
    .new-terminal svg {
        width: 15px;
        height: 15px;
    }
    .panel-content {
        flex: 1;
        min-height: 0;
        position: relative;
    }
    .terminal-slot {
        position: absolute;
        inset: 0;
    }
    .terminal-slot.hidden {
        display: none;
    }
    .placeholder {
        margin: 0;
        padding: 12px;
        font-size: 13px;
        color: var(--color-text-muted);
    }
</style>
