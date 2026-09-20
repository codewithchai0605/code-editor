<script lang="ts">
    import { editorStore } from "$lib/stores/editor.svelte";
    import { diffTabStore } from "$lib/stores/diffTab.svelte";
    import { gitStore } from "$lib/stores/git.svelte";
    import { baseName } from "$lib/utils/path";
    import { fileIconColor } from "$lib/utils/fileIcons";

    // Same convention (letters + colors) as the Explorer's own per-entry
    // badge in FileTreeNode.svelte and the Source Control panel's file
    // rows — keeping all three in sync is what makes a file's git status
    // legible no matter which of them you're looking at, matching how VS
    // Code decorates its own editor tabs.
    const GIT_BADGE: Record<string, string> = {
        modified: "M",
        added: "A",
        deleted: "D",
        renamed: "R",
        untracked: "U",
        conflicted: "!",
    };

    function onAuxClick(e: MouseEvent, path: string) {
        if (e.button === 1) {
            e.preventDefault();
            editorStore.requestClose(path);
        }
    }

    function onKeydown(e: KeyboardEvent, activate: () => void) {
        if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            activate();
        }
    }

    function activateFileTab(path: string) {
        diffTabStore.deactivate();
        editorStore.activate(path);
    }
</script>

<div class="tab-bar" role="tablist" aria-label="Open files">
    {#each editorStore.tabs as tab (tab.path)}
        {@const isActive = !diffTabStore.activeId && tab.path === editorStore.activePath}
        {@const gitStatus = gitStore.statusFor(tab.path)?.status}
        <div
            class="tab"
            class:active={isActive}
            role="tab"
            aria-selected={isActive}
            tabindex="0"
            title={tab.path}
            onclick={() => activateFileTab(tab.path)}
            onkeydown={(e) => onKeydown(e, () => activateFileTab(tab.path))}
            onauxclick={(e) => onAuxClick(e, tab.path)}
        >
            <span
                class="dot"
                style:background={tab.kind === "text"
                    ? fileIconColor(tab.path)
                    : "var(--color-text-faint)"}
            ></span>
            <span class="name" data-git-status={gitStatus}>{baseName(tab.path)}</span>
            {#if tab.isDirty}<span class="dirty" aria-hidden="true"></span>{/if}
            {#if tab.changedOnDisk}
                <span
                    class="changed-on-disk"
                    title="Changed on disk since you started editing here — saving will overwrite that change."
                    aria-label="Changed on disk since you started editing">⚠</span
                >
            {/if}
            {#if gitStatus}
                <span class="git-badge" data-git-status={gitStatus}>{GIT_BADGE[gitStatus]}</span>
            {/if}
            <button
                class="close"
                aria-label="Close {baseName(tab.path)}"
                onclick={(e) => {
                    e.stopPropagation();
                    editorStore.requestClose(tab.path);
                }}
            >
                &times;
            </button>
        </div>
    {/each}
    {#each diffTabStore.tabs as diffTab (diffTab.id)}
        {@const isActive = diffTabStore.activeId === diffTab.id}
        <div
            class="tab diff-tab"
            class:active={isActive}
            role="tab"
            aria-selected={isActive}
            tabindex="0"
            title={diffTab.path}
            onclick={() => diffTabStore.setActive(diffTab.id)}
            onkeydown={(e) => onKeydown(e, () => diffTabStore.setActive(diffTab.id))}
        >
            <svg
                class="diff-icon"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
            >
                <circle cx="4.5" cy="3.5" r="1.5" />
                <circle cx="4.5" cy="12.5" r="1.5" />
                <path d="M4.5 5v6" />
            </svg>
            <span class="name">{diffTab.title}</span>
            <button
                class="close"
                aria-label="Close {diffTab.title}"
                onclick={(e) => {
                    e.stopPropagation();
                    diffTabStore.close(diffTab.id);
                }}
            >
                &times;
            </button>
        </div>
    {/each}
</div>

<style>
    .tab-bar {
        flex-shrink: 0;
        height: var(--tabbar-height);
        display: flex;
        overflow-x: auto;
        overflow-y: hidden;
        border-bottom: 1px solid var(--color-border);
    }
    .tab {
        flex-shrink: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 0 6px 0 10px;
        max-width: 200px;
        border-right: 1px solid var(--color-border);
        background: var(--color-bg);
        font-size: 13px;
        color: var(--color-text-muted);
        cursor: pointer;
        user-select: none;
    }
    .tab:hover {
        background: var(--color-bg-hover);
    }
    .tab.active {
        color: var(--color-text);
        background: var(--color-bg);
        box-shadow: inset 0 -1px 0 var(--color-accent);
    }
    .dot {
        flex-shrink: 0;
        width: 8px;
        height: 8px;
        border-radius: 50%;
    }
    .diff-icon {
        flex-shrink: 0;
        width: 14px;
        height: 14px;
        color: var(--color-text-faint);
    }
    .name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .name[data-git-status="modified"] {
        color: #e2c08d;
    }
    .name[data-git-status="added"],
    .name[data-git-status="untracked"],
    .name[data-git-status="renamed"] {
        color: #73c991;
    }
    .name[data-git-status="deleted"],
    .name[data-git-status="conflicted"] {
        color: #f48771;
    }
    .git-badge {
        flex-shrink: 0;
        font-size: 10.5px;
        font-weight: 700;
        opacity: 0.85;
    }
    .git-badge[data-git-status="modified"] {
        color: #e2c08d;
    }
    .git-badge[data-git-status="added"],
    .git-badge[data-git-status="untracked"],
    .git-badge[data-git-status="renamed"] {
        color: #73c991;
    }
    .git-badge[data-git-status="deleted"],
    .git-badge[data-git-status="conflicted"] {
        color: #f48771;
    }
    .dirty {
        flex-shrink: 0;
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: var(--color-text-muted);
    }
    .changed-on-disk {
        flex-shrink: 0;
        font-size: 11px;
        line-height: 1;
        /* Same amber as .name[data-git-status="modified"] above — both mean
           "differs from a known-good baseline", just a different baseline
           (disk vs. the git index). */
        color: #e2c08d;
    }
    .close {
        flex-shrink: 0;
        width: 18px;
        height: 18px;
        border-radius: 3px;
        line-height: 1;
        font-size: 15px;
        color: var(--color-text-muted);
        cursor: pointer;
    }
    .close:hover {
        background: var(--color-bg-hover);
        color: var(--color-text);
    }
</style>
