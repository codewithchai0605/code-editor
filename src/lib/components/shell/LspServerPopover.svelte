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
        installable: boolean;
    }

    const rows = $derived<Row[]>(
        Array.from(lspStore.statuses.entries())
            .map(([key, entry]) => {
                const [root, languageId] = key.split("\u0000");
                return {
                    root,
                    languageId,
                    status: entry.status,
                    error: entry.error,
                    installable: entry.installable,
                };
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

    let restarting = $state<Set<string>>(new Set());
    async function onRestart(root: string, languageId: string): Promise<void> {
        const key = variantKey(root, languageId);
        restarting = new Set(restarting).add(key);
        try {
            await lspStore.restartServer(root, languageId);
        } finally {
            const next = new Set(restarting);
            next.delete(key);
            restarting = next;
        }
    }

    let installing = $state<Set<string>>(new Set());
    async function onInstall(root: string, languageId: string): Promise<void> {
        const key = variantKey(root, languageId);
        installing = new Set(installing).add(key);
        onShowLogs(root, languageId); // install progress streams in as lsp:log — show it live
        try {
            await lspStore.installServer(root, languageId);
        } finally {
            const next = new Set(installing);
            next.delete(key);
            installing = next;
        }
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
    <div class="header">
        <span>Language Servers</span>
        <button class="icon-btn" onclick={onClose} aria-label="Close">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"
                ><path d="M4 4l8 8M12 4l-8 8" /></svg
            >
        </button>
    </div>
    {#if rows.length === 0}
        <p class="empty">
            No language servers started yet — one starts automatically the first time you open a
            file it supports. Configure or add one from Settings.
        </p>
    {:else}
        <ul class="rows">
            {#each rows as row (row.root + "::" + row.languageId)}
                <li class="row">
                    <div class="row-main">
                        <span class="status-pill status-{row.status}">
                            <span class="dot"></span>
                            {statusLabel(row.status)}
                        </span>
                        <span class="lang">{row.languageId}</span>
                        <span class="root" title={row.root}>{baseName(row.root) || row.root}</span>
                    </div>

                    {#if row.error}
                        <p class="error" title={row.error}>{row.error}</p>
                    {/if}

                    {#if variantInfo.get(variantKey(row.root, row.languageId))}
                        {@const info = variantInfo.get(variantKey(row.root, row.languageId))!}
                        <div class="variant-row">
                            <span class="variant-label">Server</span>
                            <div class="segmented">
                                {#each info.variants as variant (variant)}
                                    <button
                                        class="segment"
                                        class:active={info.active === variant}
                                        onclick={() =>
                                            onVariantChange(row.root, row.languageId, variant)}
                                    >
                                        {variant}
                                    </button>
                                {/each}
                            </div>
                        </div>
                    {/if}

                    <div class="actions">
                        {#if row.installable}
                            <button
                                class="action-btn install-btn"
                                title="Install"
                                disabled={installing.has(variantKey(row.root, row.languageId))}
                                onclick={() => onInstall(row.root, row.languageId)}
                            >
                                <svg
                                    class:spinning={installing.has(
                                        variantKey(row.root, row.languageId),
                                    )}
                                    viewBox="0 0 16 16"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.4"
                                >
                                    {#if installing.has(variantKey(row.root, row.languageId))}
                                        <path
                                            d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9"
                                            stroke-linecap="round"
                                        />
                                        <path
                                            d="M13.5 2.5v3.2h-3.2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                        />
                                    {:else}
                                        <path
                                            d="M8 2.5v8M4.5 7l3.5 3.5L11.5 7"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                        />
                                        <path d="M3 13.5h10" stroke-linecap="round" />
                                    {/if}
                                </svg>
                                {installing.has(variantKey(row.root, row.languageId))
                                    ? "Installing…"
                                    : "Install"}
                            </button>
                        {/if}
                        <button
                            class="action-btn"
                            title="Restart"
                            disabled={restarting.has(variantKey(row.root, row.languageId))}
                            onclick={() => onRestart(row.root, row.languageId)}
                        >
                            <svg
                                class:spinning={restarting.has(
                                    variantKey(row.root, row.languageId),
                                )}
                                viewBox="0 0 16 16"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.4"
                            >
                                <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" stroke-linecap="round" />
                                <path
                                    d="M13.5 2.5v3.2h-3.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                />
                            </svg>
                            Restart
                        </button>
                        <button
                            class="action-btn"
                            title="Stop"
                            disabled={row.status === "stopped"}
                            onclick={() => lspStore.stopServer(row.root, row.languageId)}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor"
                                ><rect x="4" y="4" width="8" height="8" rx="1" /></svg
                            >
                            Stop
                        </button>
                        <button
                            class="action-btn"
                            title="Show Output"
                            onclick={() => onShowLogs(row.root, row.languageId)}
                        >
                            <svg
                                viewBox="0 0 16 16"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.4"
                            >
                                <rect x="2" y="3" width="12" height="10" rx="1.5" />
                                <path
                                    d="M4.5 6.5l2 1.5-2 1.5M8 9.5h3.5"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                />
                            </svg>
                            Output
                        </button>
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
        width: 360px;
        max-height: 60vh;
        overflow-y: auto;
        background: var(--color-bg-elevated);
        border: 1px solid var(--color-border);
        border-radius: 8px;
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
        font-size: 12px;
        color: var(--color-text);
    }
    .header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 9px 8px 9px 12px;
        font-weight: 600;
        font-size: 12.5px;
        border-bottom: 1px solid var(--color-border);
        position: sticky;
        top: 0;
        background: var(--color-bg-elevated);
    }
    .icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 22px;
        height: 22px;
        padding: 0;
        border-radius: 4px;
        color: var(--color-text-faint);
        cursor: pointer;
    }
    .icon-btn:hover {
        background: var(--color-bg-hover);
        color: var(--color-text);
    }
    .icon-btn svg {
        width: 12px;
        height: 12px;
    }
    .empty {
        padding: 14px 12px;
        color: var(--color-text-faint);
        line-height: 1.5;
    }
    .rows {
        list-style: none;
    }
    .row {
        padding: 10px 12px;
        border-bottom: 1px solid var(--color-border);
    }
    .row:last-child {
        border-bottom: none;
    }
    .row-main {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .status-pill {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        flex-shrink: 0;
        padding: 2px 7px;
        border-radius: 10px;
        font-size: 10.5px;
        font-weight: 600;
        letter-spacing: 0.02em;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
    }
    .status-pill .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
    }
    .status-running {
        color: #3fb950;
    }
    .status-running .dot {
        background: #3fb950;
        box-shadow: 0 0 4px #3fb950;
    }
    .status-starting {
        color: #e2c08d;
    }
    .status-starting .dot {
        background: #e2c08d;
        animation: pulse 1.1s ease-in-out infinite;
    }
    .status-crashed {
        color: #f85149;
    }
    .status-crashed .dot {
        background: #f85149;
    }
    .status-stopped {
        color: var(--color-text-faint);
    }
    .status-stopped .dot {
        background: var(--color-text-faint);
    }
    @keyframes pulse {
        0%,
        100% {
            opacity: 1;
        }
        50% {
            opacity: 0.35;
        }
    }
    .lang {
        font-weight: 600;
        text-transform: capitalize;
        flex-shrink: 0;
    }
    .root {
        color: var(--color-text-faint);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
        text-align: right;
    }
    .error {
        margin-top: 6px;
        padding: 5px 7px;
        border-radius: 4px;
        background: rgba(248, 81, 73, 0.12);
        color: #f85149;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .variant-row {
        margin-top: 8px;
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .variant-label {
        color: var(--color-text-faint);
        flex-shrink: 0;
    }
    .segmented {
        display: inline-flex;
        border: 1px solid var(--color-border);
        border-radius: 5px;
        overflow: hidden;
    }
    .segment {
        padding: 3px 9px;
        color: var(--color-text-faint);
        background: var(--color-bg);
        cursor: pointer;
        border-right: 1px solid var(--color-border);
    }
    .segment:last-child {
        border-right: none;
    }
    .segment:hover {
        color: var(--color-text);
    }
    .segment.active {
        background: var(--color-accent);
        color: #fff;
    }
    .actions {
        margin-top: 8px;
        display: flex;
        gap: 6px;
    }
    .action-btn {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 4px;
        padding: 4px 9px;
        color: var(--color-text);
        cursor: pointer;
    }
    .action-btn svg {
        width: 11px;
        height: 11px;
        flex-shrink: 0;
    }
    .action-btn svg.spinning {
        animation: spin 0.8s linear infinite;
    }
    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }
    .action-btn:hover:not(:disabled) {
        background: var(--color-bg-hover);
        border-color: var(--color-accent);
    }
    .action-btn:disabled {
        color: var(--color-text-faint);
        cursor: default;
        opacity: 0.6;
    }
    .install-btn {
        background: var(--color-accent);
        border-color: var(--color-accent);
        color: #fff;
    }
    .install-btn:hover:not(:disabled) {
        filter: brightness(1.1);
        background: var(--color-accent);
    }
    .footer {
        padding: 8px 12px;
        border-top: 1px solid var(--color-border);
    }
    .stop-all {
        width: 100%;
        background: var(--color-bg);
        border: 1px solid var(--color-border);
        border-radius: 5px;
        padding: 6px 8px;
        color: var(--color-text);
        cursor: pointer;
        font-weight: 500;
    }
    .stop-all:hover {
        background: #a1260d;
        border-color: #a1260d;
        color: #fff;
    }
</style>
