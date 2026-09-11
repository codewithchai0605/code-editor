<script lang="ts">
  import { gitStore } from "$lib/stores/git.svelte";
  import { editorStore } from "$lib/stores/editor.svelte";
  import { baseName, parentOf } from "$lib/utils/path";
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

  function openFile(path: string) {
    void editorStore.open(path);
  }

  function relativeDir(path: string, repoPath: string): string {
    const dir = parentOf(path);
    if (dir === repoPath) return "";
    // Strip the repo root prefix so nested paths read like VS Code's own
    // SCM view ("src/lib/foo.ts" rather than the full absolute path).
    const stripped = dir.startsWith(repoPath)
      ? dir.slice(repoPath.length)
      : dir;
    return stripped.replace(/^[\\/]+/, "");
  }

  async function handleCommit(repoId: string) {
    const message = commitMessages[repoId] ?? "";
    try {
      await gitStore.commit(repoId, message);
      commitMessages = { ...commitMessages, [repoId]: "" };
      commitErrors = { ...commitErrors, [repoId]: "" };
    } catch (err) {
      commitErrors = {
        ...commitErrors,
        [repoId]: err instanceof Error ? err.message : String(err),
      };
    }
  }
</script>

<div class="git-panel">
  {#if gitStore.lastError}
    <p class="error-banner">{gitStore.lastError}</p>
  {/if}

  {#if !gitStore.hasAnyRepo}
    <p class="placeholder">No git repository detected in this workspace.</p>
  {:else}
    {#each gitStore.repos as repo (repo.id)}
      {@const status = gitStore.statusOf(repo.id)}
      {@const isCollapsed = collapsed.has(repo.id)}
      {@const canStageOrDiscard = status
        ? status.unstaged.length + status.untracked.length
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
              <span class="repo-branch">{repo.currentBranch}</span>
            {/if}
            {#if repo.ahead || repo.behind}
              <span class="repo-ahead-behind">
                {repo.ahead ? `↑${repo.ahead}` : ""}{repo.behind
                  ? `↓${repo.behind}`
                  : ""}
              </span>
            {/if}
          </button>
          <button
            class="icon-btn"
            title="Refresh"
            onclick={() => gitStore.refreshRepo(repo.id)}
          >
            <svg
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
            >
              <path d="M13 3v4h-4M3 13v-4h4" />
              <path
                d="M3.5 8a4.5 4.5 0 0 1 7.5-3.2l2 2M12.5 8a4.5 4.5 0 0 1-7.5 3.2l-2-2"
              />
            </svg>
          </button>
        </div>

        {#if !isCollapsed && status}
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
            ></textarea>
            {#if commitErrors[repo.id]}
              <p class="commit-error">{commitErrors[repo.id]}</p>
            {/if}
            <button
              class="commit-btn"
              disabled={status.staged.length === 0 ||
                !(commitMessages[repo.id] ?? "").trim()}
              onclick={() => handleCommit(repo.id)}
            >
              Commit ({status.staged.length})
            </button>
          </div>

          {#if status.conflicted.length}
            <div class="group">
              <p class="group-title">Merge Conflicts</p>
              {#each status.conflicted as file (file.path)}
                <div class="file-row">
                  <button class="file-open" onclick={() => openFile(file.path)}>
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
                  onclick={() => gitStore.unstageAll(repo.id)}>−</button
                >
              </div>
              {#each status.staged as file (file.path)}
                <div class="file-row">
                  <button class="file-open" onclick={() => openFile(file.path)}>
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
                    >−</button
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
                  onclick={() => gitStore.stageAll(repo.id)}>+</button
                >
                <button
                  class="icon-btn"
                  title="Discard All"
                  onclick={() => gitStore.requestDiscardAll(repo.id)}>⨯</button
                >
              </div>
              {#each status.unstaged as file (file.path)}
                <div class="file-row">
                  <button class="file-open" onclick={() => openFile(file.path)}>
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
                      gitStore.requestDiscardFile(repo.id, file.path)}>⨯</button
                  >
                  <button
                    class="row-action"
                    title="Stage"
                    onclick={() => gitStore.stageFile(repo.id, file.path)}
                    >+</button
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
                <p class="group-title">Untracked ({status.untracked.length})</p>
                {#if !status.unstaged.length}
                  <button
                    class="icon-btn"
                    title="Stage All"
                    onclick={() => gitStore.stageAll(repo.id)}>+</button
                  >
                  <button
                    class="icon-btn"
                    title="Discard All"
                    onclick={() => gitStore.requestDiscardAll(repo.id)}
                    >⨯</button
                  >
                {/if}
              </div>
              {#each status.untracked as file (file.path)}
                <div class="file-row">
                  <button class="file-open" onclick={() => openFile(file.path)}>
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
                      gitStore.requestDiscardFile(repo.id, file.path)}>⨯</button
                  >
                  <button
                    class="row-action"
                    title="Stage"
                    onclick={() => gitStore.stageFile(repo.id, file.path)}
                    >+</button
                  >
                  <span class="badge" data-status={file.status}
                    >{GIT_BADGE[file.status]}</span
                  >
                </div>
              {/each}
            </div>
          {/if}

          {#if !status.conflicted.length && !status.staged.length && !canStageOrDiscard}
            <p class="clean">No changes.</p>
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
  .error-banner {
    color: #f48771;
    background: rgba(244, 135, 113, 0.1);
    border-radius: 4px;
    padding: 6px 8px;
    margin: 0 0 6px;
  }
  .placeholder {
    color: var(--color-text-muted);
    padding: 4px 2px;
  }
  .repo {
    display: flex;
    flex-direction: column;
    margin-bottom: 8px;
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
    font-weight: 400;
    color: var(--color-text-muted);
  }
  .repo-ahead-behind {
    font-weight: 400;
    color: var(--color-text-faint);
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
    align-self: flex-start;
    padding: 4px 12px;
    border-radius: 4px;
    background: var(--color-accent);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
  }
  .commit-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .icon-btn {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--color-text-muted);
    font-size: 13px;
    line-height: 1;
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
    border-radius: 3px;
  }
  .file-row:hover {
    background: rgba(255, 255, 255, 0.06);
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
    width: 18px;
    height: 18px;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--color-text-muted);
    font-size: 13px;
    line-height: 1;
  }
  .row-action:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-text);
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
    color: var(--color-text-faint);
    padding: 3px 2px 3px 20px;
    margin: 0;
  }
</style>
