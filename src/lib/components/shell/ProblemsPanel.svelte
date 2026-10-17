<script lang="ts">
    import { problemsStore, type ProblemEntry } from "$lib/stores/problems.svelte";
    import { editorStore } from "$lib/stores/editor.svelte";

    /** Collapsed-file state for the Problems panel — a file's uriKey when
     *  its rows are hidden. Defaults to expanded (empty set). */
    let collapsed = $state(new Set<string>());

    function toggleGroup(key: string): void {
        const next = new Set(collapsed);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        collapsed = next;
    }

    const SEVERITY_ICON: Record<number, { glyph: string; color: string; label: string }> = {
        1: { glyph: "✕", color: "#f85149", label: "Error" },
        2: { glyph: "▲", color: "#e2c08d", label: "Warning" },
        3: { glyph: "ℹ", color: "#79c0ff", label: "Info" },
        4: { glyph: "◇", color: "#8b949e", label: "Hint" },
    };

    function reveal(entry: ProblemEntry): void {
        // openAndReveal opens the tab if needed and jumps to the exact
        // range, reusing the same flash-highlight as workspace search.
        void editorStore.openAndReveal(
            entry.path,
            entry.line,
            entry.column - 1,
            entry.column, // end col unknown per-row; single-char reveal is fine
        );
    }

    function collapseAll(): void {
        collapsed = new Set(problemsStore.byFile.keys());
    }
    function expandAll(): void {
        collapsed = new Set();
    }
</script>

<div class="problems">
    <div class="toolbar">
        <span class="title">Problems</span>
        <span class="counts">
            {#if problemsStore.errorCount > 0}<span class="count error">{problemsStore.errorCount} errors</span>{/if}
            {#if problemsStore.warningCount > 0}<span class="count warning">{problemsStore.warningCount} warnings</span>{/if}
            {#if problemsStore.infoCount > 0}<span class="count info">{problemsStore.infoCount} infos</span>{/if}
        </span>
        <span class="spacer"></span>
        <button class="tool" title="Collapse All" onclick={collapseAll}>⊟</button>
        <button class="tool" title="Expand All" onclick={expandAll}>⊞</button>
    </div>

    {#if problemsStore.groups.length === 0}
        <p class="empty">No problems have been detected in the workspace.</p>
    {:else}
        <div class="tree" role="tree">
            {#each problemsStore.groups as group (group.key)}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                    class="file-row"
                    role="treeitem"
                    aria-expanded={!collapsed.has(group.key)}
                    tabindex="0"
                    onclick={() => toggleGroup(group.key)}
                    onkeydown={(e) => e.key === "Enter" && toggleGroup(group.key)}
                >
                    <span class="chevron" class:collapsed={collapsed.has(group.key)}>▾</span>
                    <span class="file-name">{group.fileName}</span>
                    <span class="file-count">{group.rows.length}</span>
                </div>
                {#if !collapsed.has(group.key)}
                    {#each group.rows as row (row.id)}
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <div
                            class="problem-row"
                            role="treeitem"
                            tabindex="0"
                            title={`${row.message}${row.source ? ` (${row.source})` : ""}`}
                            onclick={() => reveal(row)}
                            onkeydown={(e) => e.key === "Enter" && reveal(row)}
                        >
                            <span
                                class="sev"
                                style:color={SEVERITY_ICON[row.severity].color}
                            >{SEVERITY_ICON[row.severity].glyph}</span>
                            <span class="msg">{row.message}</span>
                            <span class="code">{row.source ?? ""}</span>
                            <span class="loc">{row.line}:{row.column}</span>
                        </div>
                    {/each}
                {/if}
            {/each}
        </div>
    {/if}
</div>

<style>
    .problems {
        height: 100%;
        overflow: auto;
        font-size: 12px;
        display: flex;
        flex-direction: column;
    }
    .toolbar {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 4px 10px;
        border-bottom: 1px solid var(--color-border);
        position: sticky;
        top: 0;
        background: var(--color-bg);
        z-index: 1;
    }
    .title {
        font-weight: 600;
        opacity: 0.9;
    }
    .counts {
        display: flex;
        gap: 8px;
        opacity: 0.8;
    }
    .count.error { color: #f85149; }
    .count.warning { color: #e2c08d; }
    .count.info { color: #79c0ff; }
    .spacer { flex: 1; }
    .tool {
        background: none;
        border: none;
        color: inherit;
        cursor: pointer;
        opacity: 0.7;
        padding: 2px 4px;
    }
    .tool:hover { opacity: 1; }
    .empty {
        padding: 16px;
        opacity: 0.6;
    }
    .tree { padding: 2px 0 8px; }
    .file-row {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 2px 10px;
        cursor: pointer;
        user-select: none;
    }
    .file-row:hover { background: var(--color-hover, rgba(128, 128, 128, 0.1)); }
    .chevron { transition: transform 0.1s ease; display: inline-block; width: 12px; }
    .chevron.collapsed { transform: rotate(-90deg); }
    .file-name { font-weight: 500; }
    .file-count { opacity: 0.6; }
    .problem-row {
        display: flex;
        align-items: baseline;
        gap: 8px;
        padding: 2px 10px 2px 32px;
        cursor: pointer;
        white-space: nowrap;
        overflow: hidden;
    }
    .problem-row:hover { background: var(--color-hover, rgba(128, 128, 128, 0.1)); }
    .sev { flex-shrink: 0; width: 14px; text-align: center; }
    .msg { overflow: hidden; text-overflow: ellipsis; flex: 1; }
    .code { opacity: 0.55; flex-shrink: 0; }
    .loc { opacity: 0.7; flex-shrink: 0; font-variant-numeric: tabular-nums; }
</style>
