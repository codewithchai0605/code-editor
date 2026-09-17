<script lang="ts">
    import { onMount } from "svelte";
    import { lspStore } from "$lib/lsp/manager.svelte";
    import { workspace } from "$lib/stores/workspace.svelte";
    import { baseName } from "$lib/utils/path";
    import type { LspServerStatusKind } from "$lib/lsp/types";

    interface Props {
        anchorX: number;
        anchorY: number;
        onClose: () => void;
        onShowLogs: (root: string, languageId: string) => void;
    }

    let { anchorX, anchorY, onClose, onShowLogs }: Props = $props();

    let panelEl: HTMLDivElement | undefined = $state();
    let pos = $state({ left: 0, bottom: 0 });

    interface Row {
        root: string;
        languageId: string;
        status: LspServerStatusKind;
        error: string | null;
    }

    const rows = $derived<Row[]>(
        Array.from(lspStore.statuses.entries())
            .map(([key, entry]) => {
                const [root, languageId] = key.split("\u0000");
                return { root, languageId, status: entry.status, error: entry.error };
            })
            .sort(
                (a, b) => a.languageId.localeCompare(b.languageId) || a.root.localeCompare(b.root),
            ),
    );

    // languageId::root -> variant info, populated lazily as rows render —
    // best-effort (a language with no Multi entry just never gets an entry
    // here, and the row renders without a variant switcher).
    let variantInfo = $state<Map<string, { variants: string[]; active: string }>>(new Map());

    function variantKey(root: string, languageId: string): string {
        return `${languageId}::${root}`;
    }

    async function loadVariants(root: string, languageId: string): Promise<void> {
        const key = variantKey(root, languageId);
        if (variantInfo.has(key) || !workspace.rootPath) return;
        const info = await lspStore.listVariants(workspace.rootPath, root, languageId);
        if (info) {
            const next = new Map(variantInfo);
            next.set(key, info);
            variantInfo = next;
        }
    }

    $effect(() => {
        for (const row of rows) void loadVariants(row.root, row.languageId);
    });

    function statusColor(status: LspServerStatusKind): string {
        switch (status) {
            case "running":
                return "#3fb950";
            case "starting":
                return "#e2c08d";
            case "crashed":
                return "#f85149";
            case "stopped":
                return "#6e7681";
        }
    }

    function statusLabel(status: LspServerStatusKind): string {
        switch (status) {
            case "running":
                return "Running";
            case "starting":
                return "Starting…";
            case "crashed":
                return "Crashed";
            case "stopped":
                return "Stopped";
        }
    }

    async function onVariantChange(
        root: string,
        languageId: string,
        variant: string,
    ): Promise<void> {
        if (!workspace.rootPath) return;
        await lspStore.setVariant(workspace.rootPath, root, languageId, variant);
        const key = variantKey(root, languageId);
        const next = new Map(variantInfo);
        next.delete(key);
        variantInfo = next;
        void loadVariants(root, languageId);
    }

    onMount(() => {
        if (panelEl) {
            const rect = panelEl.getBoundingClientRect();
            const maxLeft = window.innerWidth - rect.width - 4;
            pos = {
                left: Math.min(anchorX, Math.max(4, maxLeft)),
                bottom: window.innerHeight - anchorY + 6,
            };
        }

        function onPointerDown(e: PointerEvent) {
            if (panelEl && !panelEl.contains(e.target as Node)) onClose();
        }
        function onKeydown(e: KeyboardEvent) {
            if (e.key === "Escape") onClose();
        }
        window.addEventListener("pointerdown", onPointerDown, true);
        window.addEventListener("keydown", onKeydown);
        return () => {
            window.removeEventListener("pointerdown", onPointerDown, true);
            window.removeEventListener("keydown", onKeydown);
        };
    });
</script>

<div class="panel" bind:this={panelEl} style:left="{pos.left}px" style:bottom="{pos.bottom}px">
    <div class="header">Language Servers</div>
    {#if rows.length === 0}
        <p class="empty">
            No language servers started yet — one starts automatically the first time you open a
            file it supports.
        </p>
    {:else}
        <ul class="rows">
            {#each rows as row (row.root + "::" + row.languageId)}
                <li class="row">
                    <div class="row-main">
                        <span class="dot" style:background={statusColor(row.status)}></span>
                        <span class="lang">{row.languageId}</span>
                        <span class="root" title={row.root}>{baseName(row.root) || row.root}</span>
                        <span class="status">{statusLabel(row.status)}</span>
                    </div>
                    {#if row.error}
                        <p class="error" title={row.error}>{row.error}</p>
                    {/if}
                    {#if variantInfo.get(variantKey(row.root, row.languageId))}
                        {@const info = variantInfo.get(variantKey(row.root, row.languageId))!}
                        <div class="variant-row">
                            <span class="variant-label">Server:</span>
                            <select
                                value={info.active}
                                onchange={(e) =>
                                    onVariantChange(
                                        row.root,
                                        row.languageId,
                                        e.currentTarget.value,
                                    )}
                            >
                                {#each info.variants as variant (variant)}
                                    <option value={variant}>{variant}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}
                    <div class="actions">
                        <button onclick={() => lspStore.restartServer(row.root, row.languageId)}
                            >Restart</button
                        >
                        <button
                            onclick={() => lspStore.stopServer(row.root, row.languageId)}
                            disabled={row.status === "stopped"}>Stop</button
                        >
                        <button onclick={() => onShowLogs(row.root, row.languageId)}>Output</button>
                    </div>
                </li>
            {/each}
        </ul>
        <div class="footer">
            <button class="stop-all" onclick={() => lspStore.stopAllServers()}
                >Stop All Servers</button
            >
        </div>
    {/if}
</div>

<style>
    .panel {
        position: fixed;
        z-index: 100;
        width: 340px;
        max-height: 60vh;
        overflow-y: auto;
        background: var(--color-bg-elevated);
        border: 1px solid var(--color-border);
        border-radius: 6px;
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
        font-size: 12px;
        color: var(--color-text);
    }
    .header {
        padding: 8px 10px 6px;
        font-weight: 600;
        border-bottom: 1px solid var(--color-border);
    }
    .empty {
        padding: 10px;
        color: var(--color-text-faint);
        line-height: 1.5;
    }
    .rows {
        list-style: none;
    }
    .row {
        padding: 8px 10px;
        border-bottom: 1px solid var(--color-border);
    }
    .row:last-child {
        border-bottom: none;
    }
    .row-main {
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        flex-shrink: 0;
    }
    .lang {
        font-weight: 600;
        text-transform: capitalize;
    }
    .root {
        color: var(--color-text-faint);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }
    .status {
        color: var(--color-text-faint);
        flex-shrink: 0;
    }
    .error {
        margin-top: 4px;
        color: #f85149;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .variant-row {
        margin-top: 6px;
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .variant-label {
        color: var(--color-text-faint);
    }
    .variant-row select {
        background: var(--color-bg);
        color: var(--color-text);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 2px 4px;
        font-size: 12px;
    }
    .actions {
        margin-top: 6px;
        display: flex;
        gap: 6px;
    }
    .actions button {
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 3px 8px;
        color: var(--color-text);
        cursor: pointer;
    }
    .actions button:hover:not(:disabled) {
        background: var(--color-accent);
    }
    .actions button:disabled {
        color: var(--color-text-faint);
        cursor: default;
    }
    .footer {
        padding: 8px 10px;
        border-top: 1px solid var(--color-border);
    }
    .stop-all {
        width: 100%;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 5px 8px;
        color: var(--color-text);
        cursor: pointer;
    }
    .stop-all:hover {
        background: #a1260d;
    }
</style>
