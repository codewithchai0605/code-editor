<script lang="ts">
    import { onMount } from "svelte";
    import { workspace } from "$lib/stores/workspace.svelte";
    import {
        lspDeleteServerConfig,
        lspListServerConfigs,
        lspResetWorkspaceConfig,
        lspUpsertServerConfig,
    } from "$lib/api/lsp";
    import { toastStore } from "$lib/stores/toast.svelte";
    import type { ServerConfigInfo } from "$lib/lsp/types";

    interface Draft {
        command: string;
        argsText: string;
        rootMarkersText: string;
    }

    let configs = $state<ServerConfigInfo[]>([]);
    let loading = $state(true);
    let drafts = $state<Map<string, Draft>>(new Map());
    let dirty = $state<Set<string>>(new Set());
    let saving = $state<Set<string>>(new Set());
    let resetting = $state(false);

    let addingNew = $state(false);
    let newLanguageId = $state("");
    let newCommand = $state("");
    let newArgsText = $state("");
    let newRootMarkersText = $state("");

    function rowKey(c: { languageId: string; variant: string | null }): string {
        return `${c.languageId}::${c.variant ?? ""}`;
    }

    function toDraft(c: ServerConfigInfo): Draft {
        return {
            command: c.command,
            argsText: c.args.join(" "),
            rootMarkersText: c.rootMarkers.join(", "),
        };
    }

    function parseArgs(text: string): string[] {
        return text.trim() ? text.trim().split(/\s+/) : [];
    }

    function parseRootMarkers(text: string): string[] {
        return text
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean);
    }

    async function load(): Promise<void> {
        if (!workspace.rootPath) {
            configs = [];
            loading = false;
            return;
        }
        loading = true;
        try {
            configs = await lspListServerConfigs(workspace.rootPath);
            const next = new Map<string, Draft>();
            for (const c of configs) next.set(rowKey(c), toDraft(c));
            drafts = next;
            dirty = new Set();
        } catch (err) {
            toastStore.error(`Failed to load language server settings: ${String(err)}`);
        } finally {
            loading = false;
        }
    }

    onMount(load);

    function markDirty(key: string): void {
        dirty = new Set(dirty).add(key);
    }

    async function save(c: ServerConfigInfo): Promise<void> {
        const key = rowKey(c);
        const draft = drafts.get(key);
        if (!draft || !workspace.rootPath) return;

        saving = new Set(saving).add(key);
        try {
            await lspUpsertServerConfig(
                workspace.rootPath,
                c.languageId,
                c.variant,
                draft.command.trim(),
                parseArgs(draft.argsText),
                parseRootMarkers(draft.rootMarkersText),
            );
            toastStore.success(`Saved ${c.languageId}${c.variant ? ` (${c.variant})` : ""}`);
            await load();
        } catch (err) {
            toastStore.error(`Failed to save: ${String(err)}`);
        } finally {
            const next = new Set(saving);
            next.delete(key);
            saving = next;
        }
    }

    async function remove(c: ServerConfigInfo): Promise<void> {
        if (!workspace.rootPath) return;
        const label = `${c.languageId}${c.variant ? ` (${c.variant})` : ""}`;
        if (
            !confirm(
                `Remove the ${label} language server config?${c.variant ? " This removes every variant of it, not just this one." : ""}`,
            )
        )
            return;
        try {
            await lspDeleteServerConfig(workspace.rootPath, c.languageId);
            toastStore.success(`Removed ${label}`);
            await load();
        } catch (err) {
            toastStore.error(`Failed to remove: ${String(err)}`);
        }
    }

    async function addNew(): Promise<void> {
        if (!workspace.rootPath || !newLanguageId.trim() || !newCommand.trim()) return;
        try {
            await lspUpsertServerConfig(
                workspace.rootPath,
                newLanguageId.trim(),
                null,
                newCommand.trim(),
                parseArgs(newArgsText),
                parseRootMarkers(newRootMarkersText),
            );
            toastStore.success(`Added ${newLanguageId.trim()}`);
            newLanguageId = "";
            newCommand = "";
            newArgsText = "";
            newRootMarkersText = "";
            addingNew = false;
            await load();
        } catch (err) {
            toastStore.error(`Failed to add: ${String(err)}`);
        }
    }

    async function restoreDefaults(): Promise<void> {
        if (!workspace.rootPath) return;
        if (
            !confirm(
                "Restore all language servers in this workspace to their built-in defaults? Any custom commands, args, or added languages will be lost.",
            )
        )
            return;
        resetting = true;
        try {
            await lspResetWorkspaceConfig(workspace.rootPath);
            toastStore.success("Restored built-in defaults");
            await load();
        } catch (err) {
            toastStore.error(`Failed to reset: ${String(err)}`);
        } finally {
            resetting = false;
        }
    }
</script>

<div class="settings-section">
    <h3>Language Servers</h3>
    {#if !workspace.rootPath}
        <p class="hint">Open a folder to configure its language servers.</p>
    {:else if loading}
        <p class="hint">Loading…</p>
    {:else}
        <p class="hint">
            Configured per-workspace, stored by this app (not in your project). Most of these
            install themselves automatically the first time you open a matching file — the status
            bar's "LSP" chip shows an Install button when one isn't set up yet.
        </p>

        <div class="rows">
            {#each configs as c (rowKey(c))}
                {@const key = rowKey(c)}
                {@const draft = drafts.get(key)}
                {#if draft}
                    <div class="row">
                        <div class="row-header">
                            <span class="lang">{c.languageId}</span>
                            {#if c.variant}<span class="variant-badge">{c.variant}</span>{/if}
                            {#if c.hasInstallStrategy}<span
                                    class="install-badge"
                                    title="This server can auto-install itself"
                                    >auto-installable</span
                                >{/if}
                            <button class="remove-btn" title="Remove" onclick={() => remove(c)}>
                                <svg
                                    viewBox="0 0 16 16"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.4"><path d="M4 4l8 8M12 4l-8 8" /></svg
                                >
                            </button>
                        </div>
                        <div class="fields">
                            <label>
                                <span>Command</span>
                                <input
                                    type="text"
                                    bind:value={draft.command}
                                    oninput={() => markDirty(key)}
                                    spellcheck="false"
                                />
                            </label>
                            <label>
                                <span>Arguments</span>
                                <input
                                    type="text"
                                    bind:value={draft.argsText}
                                    oninput={() => markDirty(key)}
                                    placeholder="--stdio"
                                    spellcheck="false"
                                />
                            </label>
                            <label>
                                <span>Root markers</span>
                                <input
                                    type="text"
                                    bind:value={draft.rootMarkersText}
                                    oninput={() => markDirty(key)}
                                    placeholder="package.json, tsconfig.json"
                                    spellcheck="false"
                                />
                            </label>
                        </div>
                        {#if dirty.has(key)}
                            <button
                                class="save-btn"
                                disabled={saving.has(key)}
                                onclick={() => save(c)}
                            >
                                {saving.has(key) ? "Saving…" : "Save"}
                            </button>
                        {/if}
                    </div>
                {/if}
            {/each}
        </div>

        {#if addingNew}
            <div class="row new-row">
                <div class="fields">
                    <label>
                        <span>Language ID</span>
                        <input
                            type="text"
                            bind:value={newLanguageId}
                            placeholder="e.g. haskell"
                            spellcheck="false"
                        />
                    </label>
                    <label>
                        <span>Command</span>
                        <input
                            type="text"
                            bind:value={newCommand}
                            placeholder="e.g. haskell-language-server-wrapper"
                            spellcheck="false"
                        />
                    </label>
                    <label>
                        <span>Arguments</span>
                        <input
                            type="text"
                            bind:value={newArgsText}
                            placeholder="--lsp"
                            spellcheck="false"
                        />
                    </label>
                    <label>
                        <span>Root markers</span>
                        <input
                            type="text"
                            bind:value={newRootMarkersText}
                            placeholder="*.cabal, package.yaml"
                            spellcheck="false"
                        />
                    </label>
                </div>
                <div class="new-row-actions">
                    <button
                        class="save-btn"
                        disabled={!newLanguageId.trim() || !newCommand.trim()}
                        onclick={addNew}>Add</button
                    >
                    <button class="cancel-btn" onclick={() => (addingNew = false)}>Cancel</button>
                </div>
            </div>
        {:else}
            <button class="add-btn" onclick={() => (addingNew = true)}>+ Add Language Server</button
            >
        {/if}

        <button class="restore-btn" disabled={resetting} onclick={restoreDefaults}>
            {resetting ? "Restoring…" : "Restore Defaults"}
        </button>
    {/if}
</div>

<style>
    .settings-section h3 {
        font-size: 13px;
        font-weight: 600;
        margin-bottom: 6px;
        color: var(--color-text);
    }
    .hint {
        color: var(--color-text-faint);
        font-size: 12px;
        line-height: 1.5;
        margin-bottom: 12px;
    }
    .rows {
        display: flex;
        flex-direction: column;
        gap: 10px;
        margin-bottom: 10px;
    }
    .row {
        border: 1px solid var(--color-border);
        border-radius: 6px;
        padding: 8px 10px;
        background: var(--color-bg-elevated);
    }
    .row-header {
        display: flex;
        align-items: center;
        gap: 6px;
        margin-bottom: 8px;
    }
    .lang {
        font-weight: 600;
        font-size: 12.5px;
        text-transform: capitalize;
    }
    .variant-badge {
        font-size: 10px;
        padding: 1px 6px;
        border-radius: 8px;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        color: var(--color-text-faint);
    }
    .install-badge {
        font-size: 10px;
        padding: 1px 6px;
        border-radius: 8px;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        color: #3fb950;
    }
    .remove-btn {
        margin-left: auto;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        padding: 0;
        border-radius: 4px;
        color: var(--color-text-faint);
        cursor: pointer;
    }
    .remove-btn:hover {
        background: #a1260d;
        color: #fff;
    }
    .remove-btn svg {
        width: 10px;
        height: 10px;
    }
    .fields {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .fields label {
        display: flex;
        flex-direction: column;
        gap: 2px;
        font-size: 11px;
        color: var(--color-text-faint);
    }
    .fields input {
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 4px 6px;
        color: var(--color-text);
        font-size: 12px;
        font-family: var(--font-mono);
    }
    .fields input:focus {
        outline: 1px solid var(--color-focus-ring);
        outline-offset: -1px;
    }
    .save-btn {
        margin-top: 8px;
        background: var(--color-accent);
        color: #fff;
        border: none;
        border-radius: 4px;
        padding: 4px 10px;
        font-size: 12px;
        cursor: pointer;
    }
    .save-btn:disabled {
        opacity: 0.6;
        cursor: default;
    }
    .cancel-btn {
        margin-top: 8px;
        margin-left: 6px;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 4px 10px;
        font-size: 12px;
        color: var(--color-text);
        cursor: pointer;
    }
    .add-btn {
        width: 100%;
        background: var(--color-bg-elevated);
        border: 1px dashed var(--color-border);
        border-radius: 6px;
        padding: 8px;
        color: var(--color-text-faint);
        font-size: 12px;
        cursor: pointer;
        margin-bottom: 10px;
    }
    .add-btn:hover {
        color: var(--color-text);
        border-color: var(--color-accent);
    }
    .new-row {
        margin-bottom: 10px;
    }
    .new-row-actions {
        display: flex;
    }
    .restore-btn {
        width: 100%;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 5px;
        padding: 6px 8px;
        color: var(--color-text-faint);
        font-size: 12px;
        cursor: pointer;
    }
    .restore-btn:hover:not(:disabled) {
        color: var(--color-text);
    }
    .restore-btn:disabled {
        opacity: 0.6;
        cursor: default;
    }
</style>
