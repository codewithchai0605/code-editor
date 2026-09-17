/**
 * Central layout state for the app shell.
 *
 * Everything about "is the app responsive" lives here rather than scattered
 * across components: pane sizes are clamped against the *current* window
 * size (not just fixed pixel bounds), and the sidebar auto-collapses once
 * the window gets narrow enough that it would otherwise crowd out the
 * editor. Components just read `layout.xxx` and call the methods below —
 * they never compute clamping themselves.
 */

export type ActivityView = "explorer" | "search" | "scm" | "extensions" | "settings" | "processes";

const SIDEBAR_MIN = 170;
const SIDEBAR_MAX = 600;
const SIDEBAR_DEFAULT = 260;
const SIDEBAR_MAX_WINDOW_RATIO = 0.6; // sidebar can never eat more than 60% of the window

const PANEL_MIN = 100;
const PANEL_DEFAULT = 220;
const PANEL_MAX_WINDOW_RATIO = 0.7;

/** Below this window width, the sidebar auto-collapses to give the editor room. */
const NARROW_BREAKPOINT = 680;

class LayoutState {
    sidebarWidth = $state(SIDEBAR_DEFAULT);
    sidebarOpen = $state(true);
    /** True once we've auto-collapsed the sidebar due to width — lets us tell
     *  "user closed it" apart from "we closed it for them" when the window
     *  grows back. */
    private sidebarCollapsedByWidth = $state(false);

    panelHeight = $state(PANEL_DEFAULT);
    panelOpen = $state(false);

    activeView = $state<ActivityView>("explorer");

    windowWidth = $state(typeof window !== "undefined" ? window.innerWidth : 1280);
    windowHeight = $state(typeof window !== "undefined" ? window.innerHeight : 800);

    get isNarrow(): boolean {
        return this.windowWidth < NARROW_BREAKPOINT;
    }

    /** What should actually render, folding in the auto-collapse rule. */
    get effectiveSidebarOpen(): boolean {
        return this.sidebarOpen;
    }

    /** Call from a `window.resize` listener (see Shell.svelte). */
    setWindowSize(width: number, height: number): void {
        this.windowWidth = width;
        this.windowHeight = height;

        // Re-clamp existing sizes against the new window — without this, a
        // sidebar/panel sized for a large monitor would overflow a smaller one.
        this.setSidebarWidth(this.sidebarWidth);
        this.setPanelHeight(this.panelHeight);

        if (width < NARROW_BREAKPOINT && this.sidebarOpen && !this.sidebarCollapsedByWidth) {
            this.sidebarCollapsedByWidth = true;
            this.sidebarOpen = false;
        } else if (width >= NARROW_BREAKPOINT) {
            this.sidebarCollapsedByWidth = false;
        }
    }

    toggleSidebar(): void {
        this.sidebarOpen = !this.sidebarOpen;
    }

    togglePanel(): void {
        this.panelOpen = !this.panelOpen;
    }

    /** Activity bar icon click: switch view, or collapse if clicking the active one. */
    setActiveView(view: ActivityView): void {
        if (this.activeView === view && this.sidebarOpen) {
            this.sidebarOpen = false;
            return;
        }
        this.activeView = view;
        this.sidebarOpen = true;
    }

    setSidebarWidth(px: number): void {
        const maxForWindow = this.windowWidth * SIDEBAR_MAX_WINDOW_RATIO;
        const max = Math.min(SIDEBAR_MAX, maxForWindow);
        this.sidebarWidth = Math.min(max, Math.max(SIDEBAR_MIN, px));
    }

    setPanelHeight(px: number): void {
        const maxForWindow = this.windowHeight * PANEL_MAX_WINDOW_RATIO;
        this.panelHeight = Math.min(maxForWindow, Math.max(PANEL_MIN, px));
    }
}

export const layout = new LayoutState();
