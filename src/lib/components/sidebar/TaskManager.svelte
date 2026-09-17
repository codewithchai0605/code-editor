<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { processMonitorStore } from "$lib/stores/processMonitor.svelte";
    import { terminalStore } from "$lib/stores/terminal.svelte";

    onMount(() => processMonitorStore.start());
    onDestroy(() => processMonitorStore.stop());

    function formatMemory(bytes: number): string {
        const mb = bytes / (1024 * 1024);
        return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
    }

    /** The backend only knows terminal sessions by id ("term-3"); the
     *  human-readable "Terminal 3" title lives in terminalStore, keyed by
     *  the same id — enrich here rather than teaching process_monitor.rs
     *  about terminal-naming conventions it has no other reason to know. */
    function labelFor(kind: string, id: string, fallback: string): string {
        if (kind === "terminal") {
            return terminalStore.sessions.find((s) => s.id === id)?.title ?? fallback;
        }
        return fallback;
    }

    const totalMemory = $derived(
        processMonitorStore.processes.reduce((sum, p) => sum + p.memoryBytes, 0),
    );

    async function handleKill(pid: number) {
        await processMonitorStore.kill(pid);
    }
</script>

<div class="task-manager">
    <p class="summary">
        {processMonitorStore.processes.length} process{processMonitorStore.processes.length === 1
            ? ""
            : "es"}
        &middot; {formatMemory(totalMemory)} total
    </p>

    {#if processMonitorStore.lastError}
        <p class="error">{processMonitorStore.lastError}</p>
    {/if}

    <div class="list" role="table">
        <div class="row header" role="row">
            <span class="col-name" role="columnheader">Process</span>
            <span class="col-mem" role="columnheader">Memory</span>
            <span class="col-cpu" role="columnheader">CPU</span>
            <span class="col-action" role="columnheader"></span>
        </div>
        {#each processMonitorStore.processes as proc (proc.id)}
            <div class="row" role="row">
                <span class="col-name" role="cell">
                    <span class="kind-dot" data-kind={proc.kind}></span>
                    {labelFor(proc.kind, proc.id, proc.label)}
                    <span class="pid">PID {proc.pid}</span>
                </span>
                <span class="col-mem" role="cell">{formatMemory(proc.memoryBytes)}</span>
                <span class="col-cpu" role="cell">{proc.cpuPercent.toFixed(1)}%</span>
                <span class="col-action" role="cell">
                    {#if proc.kind !== "main"}
                        <button
                            class="kill-btn"
                            title="End process"
                            onclick={() => handleKill(proc.pid)}>&times;</button
                        >
                    {/if}
                </span>
            </div>
        {/each}
    </div>
</div>

<style>
    .task-manager {
        display: flex;
        flex-direction: column;
        gap: 8px;
        font-size: 12px;
    }
    .summary {
        color: var(--color-text-muted);
        margin: 0;
    }
    .error {
        color: #f48771;
        margin: 0;
    }
    .list {
        display: flex;
        flex-direction: column;
    }
    .row {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 5px 2px;
        border-bottom: 1px solid var(--color-border);
    }
    .row.header {
        color: var(--color-text-faint);
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        border-bottom: 1px solid var(--color-border);
    }
    .col-name {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
    }
    .col-mem {
        width: 64px;
        flex-shrink: 0;
        text-align: right;
    }
    .col-cpu {
        width: 42px;
        flex-shrink: 0;
        text-align: right;
    }
    .col-action {
        width: 18px;
        flex-shrink: 0;
    }
    .kind-dot {
        flex-shrink: 0;
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--color-text-faint);
    }
    .kind-dot[data-kind="main"] {
        background: var(--color-accent);
    }
    .kind-dot[data-kind="terminal"] {
        background: #73c991;
    }
    .kind-dot[data-kind="helper"] {
        background: #e2c08d;
    }
    .pid {
        flex-shrink: 0;
        color: var(--color-text-faint);
        font-size: 10px;
    }
    .kill-btn {
        width: 16px;
        height: 16px;
        line-height: 1;
        border-radius: 4px;
        color: var(--color-text-muted);
        cursor: pointer;
    }
    .kill-btn:hover {
        background: rgba(244, 135, 113, 0.2);
        color: #f48771;
    }
</style>
