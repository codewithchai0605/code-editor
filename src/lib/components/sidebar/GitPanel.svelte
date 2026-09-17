<script lang="ts">
    import { gitStore } from "$lib/stores/git.svelte";
    import { editorStore } from "$lib/stores/editor.svelte";
    import { diffTabStore } from "$lib/stores/diffTab.svelte";
    import { toastStore } from "$lib/stores/toast.svelte";
    import { baseName, parentOf } from "$lib/utils/path";
    import { fileIconColor } from "$lib/utils/fileIcons";
    import type { GitFileEntry } from "$lib/api/git";
    import DiscardConfirmDialog from "./DiscardConfirmDialog.svelte";

    const GIT_BADGE: Record<GitFileEntry["status"], string> = {
        modified: "M",
        added: "A",
        deleted: "D",
        renamed: "R",
        typechange: "T",
        untracked: "U",
        conflicted: "!",
    };

    // Collapsed-by-id set — repos start expanded, matching VS Code's default
    // for a workspace with only a handful of repos in it.
    let collapsed = $state(new Set<string>());
    function toggle(repoId: string) {
        const next = new Set(collapsed);
        if (next.has(repoId)) next.delete(repoId);
        else next.add(repoId);
        collapsed = next;
    }

    // Commit message + inline commit-error text, both keyed per repo so
    // switching between multiple open repos doesn't clobber either.
    let commitMessages = $state<Record<string, string>>({});
    let commitErrors = $state<Record<string, string>>({});

    /** Conflicted files open as a plain file (so the person can edit the
     *  conflict markers directly) — everything else opens its diff, matching
     *  VS Code's own Source Control click behavior. A 3-way merge-specific
     *  view is future work; plain editing is the reasonable fallback until
     *  then, same as how a conflict is actually resolved either way. */
    function openRow(
        repoId: string,
        path: string,
        staged: boolean,
        status: GitFileEntry["status"],
    ) {
        if (status === "conflicted") {
            void editorStore.open(path);
        } else {
            void diffTabStore.open(repoId, path, staged);
        }
    }

    function relativeDir(path: string, repoPath: string): string {
        const dir = parentOf(path);
        if (dir === repoPath) return "";
        // Strip the repo root prefix so nested paths read like VS Code's own
        // SCM view ("src/lib/foo.ts" rather than the full absolute path).
        const stripped = dir.startsWith(repoPath) ? dir.slice(repoPath.length) : dir;
        return stripped.replace(/^[\\/]+/, "");
    }

    async function handleCommit(repoId: string) {
        const message = commitMessages[repoId] ?? "";
        try {
            const result = await gitStore.commit(repoId, message);
            commitMessages = { ...commitMessages, [repoId]: "" };
            commitErrors = { ...commitErrors, [repoId]: "" };
            toastStore.success(`Committed ${result.oid.slice(0, 7)}`);
        } catch (err) {
            commitErrors = {
                ...commitErrors,
                [repoId]: err instanceof Error ? err.message : String(err),
            };
        }
    }
</script>

{#snippet plusIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
    >
        <path d="M8 3v10M3 8h10" />
    </svg>
{/snippet}
{#snippet minusIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
    >
        <path d="M3 8h10" />
    </svg>
{/snippet}
{#snippet discardIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
    >
        <path d="M4 4l8 8M12 4l-8 8" />
    </svg>
{/snippet}
{#snippet refreshIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <path d="M13 3v4h-4" />
        <path d="M3 13v-4h4" />
        <path d="M3.5 8a4.5 4.5 0 0 1 7.5-3.2l2 2" />
        <path d="M12.5 8a4.5 4.5 0 0 1-7.5 3.2l-2-2" />
    </svg>
{/snippet}
{#snippet branchIcon()}
    <svg
        class="branch-icon"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <circle cx="4.5" cy="3.5" r="1.5" />
        <circle cx="4.5" cy="12.5" r="1.5" />
        <circle cx="11.5" cy="6.5" r="1.5" />
        <path d="M4.5 5v6" />
        <path d="M4.5 8c0-1.5 1.2-2.5 3-2.8l2.2-.3" />
    </svg>
{/snippet}
{#snippet arrowUpIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <path d="M8 12.5V3.5M4.5 7l3.5-3.5L11.5 7" />
    </svg>
{/snippet}
{#snippet arrowDownIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <path d="M8 3.5v9M4.5 9l3.5 3.5L11.5 9" />
    </svg>
{/snippet}
{#snippet checkIcon()}
    <svg
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <path d="M3.5 8.5l3 3 6-7" />
    </svg>
{/snippet}
{#snippet emptyStateIcon()}
    <svg
        viewBox="0 0 32 32"
        fill="none"
        stroke="currentColor"
        stroke-width="1.3"
        stroke-linecap="round"
        stroke-linejoin="round"
    >
        <circle cx="9" cy="7" r="3" />
        <circle cx="9" cy="25" r="3" />
        <circle cx="23" cy="13" r="3" />
        <path d="M9 10v12" />
        <path d="M9 16c0-3 2.4-5 6-5.6l4.6-.7" />
    </svg>
{/snippet}

<div class="git-panel">
    {#if !gitStore.hasAnyRepo}
        <div class="empty-state">
            {@render emptyStateIcon()}
            <p class="empty-title">No Source Control Providers</p>
            <p class="empty-desc">
                Open a folder containing a Git repository to track and commit changes here.
            </p>
        </div>
    {:else}
        {#each gitStore.repos as repo (repo.id)}
            {@const status = gitStore.statusOf(repo.id)}
            {@const isCollapsed = collapsed.has(repo.id)}
            {@const changeCount = status ? status.unstaged.length + status.untracked.length : 0}
            {@const totalChanges = status
                ? status.staged.length + changeCount + status.conflicted.length
                : 0}
            <section class="repo">
                <div class="repo-header">
                    <button class="repo-toggle" onclick={() => toggle(repo.id)}>
                        <svg
                            class="chevron"
                            class:collapsed={isCollapsed}
                            viewBox="0 0 16 16"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.6"
                        >
                            <path d="M6 4l4 4-4 4" />
                        </svg>
                        <span class="repo-name">{repo.name}</span>
                        {#if repo.currentBranch}
                            <span class="repo-branch">
                                {@render branchIcon()}
                                <span class="repo-branch-name">{repo.currentBranch}</span>
                            </span>
                        {/if}
                        {#if repo.ahead || repo.behind}
                            <span class="repo-sync">
                                {#if repo.behind}
                                    <span class="sync-part"
                                        >{@render arrowDownIcon()}{repo.behind}</span
                                    >
                                {/if}
                                {#if repo.ahead}
                                    <span class="sync-part"
                                        >{@render arrowUpIcon()}{repo.ahead}</span
                                    >
                                {/if}
                            </span>
                        {/if}
                    </button>
                    {#if totalChanges > 0}
                        <span
                            class="repo-change-count"
                            title="{totalChanges} change{totalChanges === 1 ? '' : 's'}"
                            >{totalChanges}</span
                        >
                    {/if}
                    <button
                        class="icon-btn"
                        title="Refresh"
                        onclick={() => gitStore.refreshRepo(repo.id)}
                    >
                        {@render refreshIcon()}
                    </button>
                </div>

                {#if !isCollapsed}
                    {#if !status}
                        <p class="loading">Loading…</p>
                    {:else}
                        <div class="commit-box">
                            <textarea
                                placeholder="Message ({repo.name})"
                                rows="2"
                                value={commitMessages[repo.id] ?? ""}
                                oninput={(e) =>
                                    (commitMessages = {
                                        ...commitMessages,
                                        [repo.id]: e.currentTarget.value,
                                    })}
                                onkeydown={(e) => {
                                    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                                        e.preventDefault();
                                        handleCommit(repo.id);
                                    }
                                }}></textarea>
                            {#if commitErrors[repo.id]}
                                <p class="commit-error">{commitErrors[repo.id]}</p>
                            {/if}
                            <button
                                class="commit-btn"
                                title="Commit (Ctrl+Enter)"
                                disabled={status.staged.length === 0 ||
                                    !(commitMessages[repo.id] ?? "").trim()}
                                onclick={() => handleCommit(repo.id)}
                            >
                                {@render checkIcon()}
                                Commit{status.staged.length ? ` (${status.staged.length})` : ""}
                            </button>
                        </div>

                        {#if status.conflicted.length}
                            <div class="group">
                                <p class="group-title">Merge Conflicts</p>
                                {#each status.conflicted as file (file.path)}
                                    <div class="file-row">
                                        <button
                                            class="file-open"
                                            onclick={() =>
                                                openRow(repo.id, file.path, false, file.status)}
                                        >
                                            <span
                                                class="file-dot"
                                                style:background={fileIconColor(file.path)}
                                            ></span>
                                            <span class="file-name">{baseName(file.path)}</span>
                                            {#if relativeDir(file.path, repo.path)}
                                                <span class="file-dir"
                                                    >{relativeDir(file.path, repo.path)}</span
                                                >
                                            {/if}
                                        </button>
                                        <span class="badge" data-status={file.status}
                                            >{GIT_BADGE[file.status]}</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        {/if}

                        {#if status.staged.length}
                            <div class="group">
                                <div class="group-header">
                                    <p class="group-title">
                                        Staged Changes ({status.staged.length})
                                    </p>
                                    <button
                                        class="icon-btn"
                                        title="Unstage All"
                                        onclick={() => gitStore.unstageAll(repo.id)}
                                        >{@render minusIcon()}</button
                                    >
                                </div>
                                {#each status.staged as file (file.path)}
                                    <div class="file-row">
                                        <button
                                            class="file-open"
                                            onclick={() =>
                                                openRow(repo.id, file.path, true, file.status)}
                                        >
                                            <span
                                                class="file-dot"
                                                style:background={fileIconColor(file.path)}
                                            ></span>
                                            <span class="file-name">{baseName(file.path)}</span>
                                            {#if relativeDir(file.path, repo.path)}
                                                <span class="file-dir"
                                                    >{relativeDir(file.path, repo.path)}</span
                                                >
                                            {/if}
                                        </button>
                                        <button
                                            class="row-action"
                                            title="Unstage"
                                            onclick={() => gitStore.unstageFile(repo.id, file.path)}
                                            >{@render minusIcon()}</button
                                        >
                                        <span class="badge" data-status={file.status}
                                            >{GIT_BADGE[file.status]}</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        {/if}

                        {#if status.unstaged.length}
                            <div class="group">
                                <div class="group-header">
                                    <p class="group-title">Changes ({status.unstaged.length})</p>
                                    <button
                                        class="icon-btn"
                                        title="Stage All"
                                        onclick={() => gitStore.stageAll(repo.id)}
                                        >{@render plusIcon()}</button
                                    >
                                    <button
                                        class="icon-btn"
                                        title="Discard All"
                                        onclick={() => gitStore.requestDiscardAll(repo.id)}
                                        >{@render discardIcon()}</button
                                    >
                                </div>
                                {#each status.unstaged as file (file.path)}
                                    <div class="file-row">
                                        <button
                                            class="file-open"
                                            onclick={() =>
                                                openRow(repo.id, file.path, false, file.status)}
                                        >
                                            <span
                                                class="file-dot"
                                                style:background={fileIconColor(file.path)}
                                            ></span>
                                            <span class="file-name">{baseName(file.path)}</span>
                                            {#if relativeDir(file.path, repo.path)}
                                                <span class="file-dir"
                                                    >{relativeDir(file.path, repo.path)}</span
                                                >
                                            {/if}
                                        </button>
                                        <button
                                            class="row-action"
                                            title="Discard"
                                            onclick={() =>
                                                gitStore.requestDiscardFile(repo.id, file.path)}
                                            >{@render discardIcon()}</button
                                        >
                                        <button
                                            class="row-action"
                                            title="Stage"
                                            onclick={() => gitStore.stageFile(repo.id, file.path)}
                                            >{@render plusIcon()}</button
                                        >
                                        <span class="badge" data-status={file.status}
                                            >{GIT_BADGE[file.status]}</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        {/if}

                        {#if status.untracked.length}
                            <div class="group">
                                <div class="group-header">
                                    <p class="group-title">
                                        Untracked ({status.untracked.length})
                                    </p>
                                    {#if !status.unstaged.length}
                                        <button
                                            class="icon-btn"
                                            title="Stage All"
                                            onclick={() => gitStore.stageAll(repo.id)}
                                            >{@render plusIcon()}</button
                                        >
                                        <button
                                            class="icon-btn"
                                            title="Discard All"
                                            onclick={() => gitStore.requestDiscardAll(repo.id)}
                                            >{@render discardIcon()}</button
                                        >
                                    {/if}
                                </div>
                                {#each status.untracked as file (file.path)}
                                    <div class="file-row">
                                        <button
                                            class="file-open"
                                            onclick={() =>
                                                openRow(repo.id, file.path, false, file.status)}
                                        >
                                            <span
                                                class="file-dot"
                                                style:background={fileIconColor(file.path)}
                                            ></span>
                                            <span class="file-name">{baseName(file.path)}</span>
                                            {#if relativeDir(file.path, repo.path)}
                                                <span class="file-dir"
                                                    >{relativeDir(file.path, repo.path)}</span
                                                >
                                            {/if}
                                        </button>
                                        <button
                                            class="row-action"
                                            title="Discard"
                                            onclick={() =>
                                                gitStore.requestDiscardFile(repo.id, file.path)}
                                            >{@render discardIcon()}</button
                                        >
                                        <button
                                            class="row-action"
                                            title="Stage"
                                            onclick={() => gitStore.stageFile(repo.id, file.path)}
                                            >{@render plusIcon()}</button
                                        >
                                        <span class="badge" data-status={file.status}
                                            >{GIT_BADGE[file.status]}</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        {/if}

                        {#if !status.conflicted.length && !status.staged.length && !changeCount}
                            <p class="clean">{@render checkIcon()} No changes</p>
                        {/if}
                    {/if}
                {/if}
            </section>
        {/each}
    {/if}
</div>

<DiscardConfirmDialog />

<style>
    .git-panel {
        display: flex;
        flex-direction: column;
        font-size: 12px;
    }
    .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        text-align: center;
        gap: 8px;
        padding: 32px 20px 16px;
        color: var(--color-text-faint);
    }
    .empty-state svg {
        width: 32px;
        height: 32px;
        opacity: 0.5;
    }
    .empty-title {
        margin: 0;
        font-size: 12px;
        font-weight: 600;
        color: var(--color-text-muted);
    }
    .empty-desc {
        margin: 0;
        font-size: 11.5px;
        line-height: 1.5;
    }
    .loading {
        color: var(--color-text-faint);
        padding: 3px 2px 3px 20px;
        margin: 4px 0;
    }
    .repo {
        display: flex;
        flex-direction: column;
        margin-bottom: 8px;
    }
    .repo:not(:first-child) {
        border-top: 1px solid var(--color-border);
        padding-top: 6px;
    }
    .repo-header {
        display: flex;
        align-items: center;
        gap: 2px;
        padding: 2px;
    }
    .repo-toggle {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 3px 0;
        font-weight: 600;
        text-align: left;
        border-radius: 4px;
    }
    .repo-toggle:hover {
        background: rgba(255, 255, 255, 0.08);
    }
    .chevron {
        width: 12px;
        height: 12px;
        flex-shrink: 0;
        transition: transform 0.1s ease;
    }
    .chevron.collapsed {
        transform: rotate(-90deg);
    }
    .repo-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .repo-branch {
        display: flex;
        align-items: center;
        gap: 3px;
        flex-shrink: 0;
        font-weight: 400;
        color: var(--color-text-muted);
        background: rgba(255, 255, 255, 0.06);
        border-radius: 10px;
        padding: 1px 7px 1px 5px;
    }
    .branch-icon {
        width: 11px;
        height: 11px;
        flex-shrink: 0;
    }
    .repo-branch-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 12ch;
    }
    .repo-sync {
        display: flex;
        align-items: center;
        gap: 6px;
        flex-shrink: 0;
        font-weight: 400;
        font-variant-numeric: tabular-nums;
        color: var(--color-text-faint);
    }
    .sync-part {
        display: flex;
        align-items: center;
        gap: 1px;
    }
    .sync-part svg {
        width: 10px;
        height: 10px;
    }
    .repo-change-count {
        flex-shrink: 0;
        font-size: 10px;
        font-weight: 600;
        line-height: 1;
        color: var(--color-text-muted);
        background: rgba(255, 255, 255, 0.1);
        border-radius: 9px;
        padding: 2.5px 5.5px;
        font-variant-numeric: tabular-nums;
    }
    .commit-box {
        display: flex;
        flex-direction: column;
        gap: 4px;
        margin: 2px 0 8px;
    }
    .commit-box textarea {
        resize: vertical;
        min-height: 40px;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 6px 8px;
        font-size: 12px;
        font-family: inherit;
        color: var(--color-text);
    }
    .commit-error {
        color: #f48771;
        margin: 0;
        font-size: 11px;
    }
    .commit-btn {
        display: flex;
        align-items: center;
        gap: 5px;
        align-self: stretch;
        justify-content: center;
        padding: 5px 12px;
        border-radius: 4px;
        background: var(--color-accent);
        color: #fff;
        font-size: 12px;
        font-weight: 600;
    }
    .commit-btn svg {
        width: 12px;
        height: 12px;
        flex-shrink: 0;
    }
    .commit-btn:not(:disabled):hover {
        filter: brightness(1.08);
    }
    .commit-btn:disabled {
        opacity: 0.4;
        cursor: default;
    }
    .icon-btn {
        flex-shrink: 0;
        width: 20px;
        height: 20px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 4px;
        color: var(--color-text-muted);
    }
    .icon-btn:hover {
        background: rgba(255, 255, 255, 0.08);
        color: var(--color-text);
    }
    .icon-btn svg {
        width: 13px;
        height: 13px;
    }
    .group {
        display: flex;
        flex-direction: column;
        margin: 2px 0 4px;
    }
    .group-header {
        display: flex;
        align-items: center;
        gap: 2px;
        padding: 0 2px;
    }
    .group-title {
        flex: 1;
        min-width: 0;
        color: var(--color-text-faint);
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        padding: 3px 0 3px 18px;
        margin: 0;
    }
    .file-row {
        display: flex;
        align-items: center;
        gap: 2px;
        padding: 0 2px 0 20px;
        border-radius: 4px;
    }
    .file-row:hover {
        background: rgba(255, 255, 255, 0.08);
    }
    .file-row:hover .row-action {
        display: flex;
    }
    .file-open {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 3px 0;
        text-align: left;
    }
    .file-dot {
        flex-shrink: 0;
        width: 7px;
        height: 7px;
        border-radius: 50%;
    }
    .file-name {
        flex-shrink: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .file-dir {
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: var(--color-text-faint);
        font-size: 11px;
    }
    .row-action {
        display: none;
        flex-shrink: 0;
        width: 20px;
        height: 20px;
        align-items: center;
        justify-content: center;
        border-radius: 4px;
        color: var(--color-text-muted);
    }
    .row-action:hover {
        background: rgba(255, 255, 255, 0.12);
        color: var(--color-text);
    }
    .row-action svg {
        width: 13px;
        height: 13px;
    }
    .badge {
        flex-shrink: 0;
        font-weight: 600;
        font-size: 11px;
        opacity: 0.85;
        padding: 0 4px;
    }
    .badge[data-status="modified"] {
        color: #e2c08d;
    }
    .badge[data-status="added"],
    .badge[data-status="untracked"],
    .badge[data-status="renamed"] {
        color: #73c991;
    }
    .badge[data-status="deleted"],
    .badge[data-status="conflicted"] {
        color: #f48771;
    }
    .clean {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--color-text-faint);
        padding: 4px 2px 4px 20px;
        margin: 0;
    }
    .clean svg {
        width: 12px;
        height: 12px;
        flex-shrink: 0;
    }
</style>
