<script lang="ts">
    import { onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { layout } from "$lib/stores/layout.svelte";
    import { gitStore } from "$lib/stores/git.svelte";
    import { editorStore } from "$lib/stores/editor.svelte";
    import { problemsStore } from "$lib/stores/problems.svelte";
    import { lspLanguageIdFor } from "$lib/lsp/languageId";
    import { lspStore } from "$lib/lsp/manager.svelte";
    import LspServerPopover from "./LspServerPopover.svelte";
    import LspOutputPanel from "./LspOutputPanel.svelte";

    let version = $state("");

    onMount(async () => {
        try {
            version = await invoke<string>("app_version");
        } catch (err) {
            console.error("Failed to read app version:", err);
        }
    });

    // With multiple repos possible in one workspace, the status bar shows
    // whichever one is most relevant right now: the repo containing the
    // active editor tab's file, falling back to the first detected repo.
    // The Source Control sidebar (opened by clicking this chip) is where
    // every repo gets its own section — this chip is a summary, not the
    // full picture.
    const primaryRepo = $derived(
        (editorStore.activeTab && gitStore.repoFor(editorStore.activeTab.path)) ??
            gitStore.repos[0],
    );
    const primaryStatus = $derived(primaryRepo ? gitStore.statusOf(primaryRepo.id) : undefined);
    const changedCount = $derived(
        primaryStatus
            ? primaryStatus.staged.length +
                  primaryStatus.unstaged.length +
                  primaryStatus.untracked.length +
                  primaryStatus.conflicted.length
            : 0,
    );

    // Summarizes every known server into one dot: red if anything crashed,
    // amber while anything's still starting, green once everything that
    // exists is running, hidden entirely if no server has ever started —
    // matching the git chip's "summary, not the full picture" role above.
    const lspSummary = $derived.by(() => {
        const statuses = Array.from(lspStore.statuses.values());
        if (statuses.length === 0) return null;
        if (statuses.some((s) => s.status === "crashed"))
            return { color: "#f85149", label: "Language servers: error" };
        if (statuses.some((s) => s.status === "starting"))
            return { color: "#e2c08d", label: "Language servers: starting" };
        return { color: "#3fb950", label: "Language servers: running" };
    });

    let lspPopoverAnchor = $state<{ x: number; y: number } | null>(null);
    let lspOutput = $state<{ root: string; languageId: string } | null>(null);

    function toggleLspPopover(e: MouseEvent) {
        if (lspPopoverAnchor) {
            lspPopoverAnchor = null;
            return;
        }
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        lspPopoverAnchor = { x: rect.left, y: rect.top };
    }

    // --- VS Code parity chips (Phase 3) ---------------------------------
    // Cursor position + selection size, fed live by cursorTrackingExtension
    // through editorStore. Only rendered when a text tab is active — the
    // chip would be meaningless (and stale) over image/binary/diff tabs.
    const hasTextTab = $derived(
        editorStore.activeTab !== undefined && editorStore.activeTab.kind === "text",
    );
    const cursorLabel = $derived(
        `Ln ${editorStore.cursorLine}, Col ${editorStore.cursorColumn}` +
            (editorStore.selectionLength > 0
                ? ` (${editorStore.selectionLength} selected)`
                : ""),
    );
    const multiCursorHint = $derived(
        editorStore.selectionCount > 1
            ? `${editorStore.selectionCount} cursors · `
            : "",
    );

    // Problems chip: hidden entirely at zero (VS Code shows nothing until
    // there's something to show); red-tinted once any file has an error.
    const problemTotal = $derived(
        problemsStore.errorCount + problemsStore.warningCount,
    );

    // Language chip mirrors what the LSP layer actually recognizes for the
    // active file — clicking it jumps to the Explorer, matching VS Code's
    // "change language mode" entry point's discoverability role (a full
    // per-file override picker is a later phase).
    const languageLabel = $derived.by(() => {
        const tab = editorStore.activeTab;
        if (!tab || tab.kind !== "text") return null;
        return lspLanguageIdFor(tab.path) ?? "Plain Text";
    });
</script>

<footer class="status-bar">
    <div class="segment">
        <button class="chip" onclick={() => layout.toggleSidebar()}>Explorer</button>
        <button class="chip" onclick={() => layout.togglePanel()}>Terminal</button>
        {#if primaryRepo?.currentBranch}
            <button
                class="chip"
                title={changedCount
                    ? `${changedCount} changed file${changedCount === 1 ? "" : "s"}`
                    : "No changes"}
                onclick={() => layout.setActiveView("scm")}
            >
                <svg
                    class="branch-icon"
                    viewBox="0 0 16 16"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.4"
                >
                    <circle cx="4.5" cy="3.5" r="1.5" />
                    <circle cx="4.5" cy="12.5" r="1.5" />
                    <circle cx="11.5" cy="7.5" r="1.5" />
                    <path d="M4.5 5v6" />
                    <path d="M4.5 8c0-2.5 1.5-4 4-4h1.5" stroke-linecap="round" />
                </svg>
                {primaryRepo.currentBranch}{#if primaryRepo.ahead || primaryRepo.behind}
                    &nbsp;{primaryRepo.ahead ? `↑${primaryRepo.ahead}` : ""}{primaryRepo.behind
                        ? `↓${primaryRepo.behind}`
                        : ""}
                {/if}
                {#if changedCount}
                    &nbsp;<span class="dirty-dot">●</span>
                {/if}
            </button>
        {/if}
        {#if lspSummary}
            <button class="chip" title={lspSummary.label} onclick={toggleLspPopover}>
                <span class="lsp-dot" style:background={lspSummary.color}></span>
                LSP
            </button>
        {/if}
    </div>
    <div class="segment">
        {#if problemTotal > 0}
            <button
                class="chip problems"
                class:has-errors={problemsStore.errorCount > 0}
                title={`${problemsStore.errorCount} error${problemsStore.errorCount === 1 ? "" : "s"}, ${problemsStore.warningCount} warning${problemsStore.warningCount === 1 ? "" : "s"} — click to open Problems, F8 to jump`}
                onclick={() => layout.showProblems()}
            >
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" class="problem-icon">
                    <circle cx="8" cy="8" r="6.25" />
                    <path d="M8 4.75v3.5" stroke-linecap="round" />
                    <circle cx="8" cy="10.9" r="0.7" fill="currentColor" stroke="none" />
                </svg>
                <span>⊗ {problemsStore.errorCount}</span>
                <span>⚠ {problemsStore.warningCount}</span>
            </button>
        {/if}
        {#if languageLabel}
            <button
                class="chip"
                title="Language mode (detected from file type; drives which LSP server runs)"
                onclick={() => layout.setActiveView("explorer")}
            >{languageLabel}</button>
        {/if}
        {#if !layout.isNarrow}
            <span class="chip static" title="File encoding">UTF-8</span>
        {/if}
        {#if hasTextTab && !layout.isNarrow}
            <span class="chip static" title="Indentation of the current file">{editorStore.indentLabel}</span>
            <span class="chip static" title="Line ending of the current file">{editorStore.documentEol}</span>
        {:else if !layout.isNarrow}
            <span class="chip static" title="Line ending of the current file">{editorStore.documentEol}</span>
        {/if}
        {#if hasTextTab}
            <span class="chip static" title="{multiCursorHint}cursor position">
                {multiCursorHint}{cursorLabel}
            </span>
        {/if}
        {#if version}
            <span class="chip static">v{version}</span>
        {/if}
    </div>
</footer>

{#if lspPopoverAnchor}
    <LspServerPopover
        anchorX={lspPopoverAnchor.x}
        anchorY={lspPopoverAnchor.y}
        onClose={() => (lspPopoverAnchor = null)}
        onShowLogs={(root, languageId) => {
            lspOutput = { root, languageId };
            lspPopoverAnchor = null;
        }}
    />
{/if}

{#if lspOutput}
    <LspOutputPanel
        root={lspOutput.root}
        languageId={lspOutput.languageId}
        onClose={() => (lspOutput = null)}
    />
{/if}

<style>
    .status-bar {
        flex-shrink: 0;
        height: var(--statusbar-height);
        display: flex;
        align-items: center;
        justify-content: space-between;
        background: var(--color-bg-statusbar);
        color: var(--color-statusbar-text);
        font-size: 12px;
        padding: 0 8px;
        user-select: none;
    }
    .segment {
        display: flex;
        align-items: center;
        gap: 2px;
        min-width: 0;
        overflow: hidden;
    }
    .chip {
        cursor: pointer;
        height: var(--statusbar-height);
        padding: 0 8px;
        display: flex;
        align-items: center;
        border-radius: 4px;
        white-space: nowrap;
        color: inherit;
    }
    .chip:hover {
        background: rgba(255, 255, 255, 0.08);
    }
    .chip.static {
        cursor: default;
    }
    .chip.static:hover {
        background: none;
    }
    .branch-icon {
        width: 13px;
        height: 13px;
        margin-right: 4px;
    }
    .dirty-dot {
        color: #e2c08d;
        font-size: 8px;
    }
    .lsp-dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        margin-right: 5px;
    }
</style>
