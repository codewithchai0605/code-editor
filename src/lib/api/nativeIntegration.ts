import { invoke } from "@tauri-apps/api/core";

/** Mirrors `ContextMenuStatus` in native_integration.rs. */
export interface ContextMenuStatus {
    /** "windows" | "macos" | "linux" | whatever `std::env::consts::OS` says
     *  on a target this app hasn't been specifically taught about. */
    platform: string;
    /** Whether this can be turned on/off at runtime at all — false only on
     *  macOS, where folder support is a build-time Info.plist declaration. */
    toggleable: boolean;
    /** Only meaningful when `toggleable` is true. */
    registered: boolean;
    /** User-facing explanation of what "registered" means on this OS, or
     *  why there's nothing to toggle. Safe to show directly. */
    note: string;
}

/** The folder path this launch was asked to open, if any — a CLI argument
 *  on cold start, already captured once found on a later single-instance
 *  or macOS "Opened" launch. Call once on startup; resolves to `null` if
 *  this wasn't launched via "Open with <App>" on a folder. See
 *  `take_pending_open_path` in native_integration.rs. */
export function takePendingOpenPath(): Promise<string | null> {
    return invoke<string | null>("take_pending_open_path");
}

export function contextMenuStatus(): Promise<ContextMenuStatus> {
    return invoke<ContextMenuStatus>("context_menu_status");
}

/** Adds "Open with <App>" to a folder's right-click menu. Rejects with a
 *  plain string, not an `FsError` — these commands return `Result<_,
 *  String>` on the Rust side (see native_integration.rs), most notably on
 *  macOS, where this always rejects explaining that folder support there
 *  is set at build time with nothing to toggle from inside the app. */
export function registerFolderContextMenu(): Promise<ContextMenuStatus> {
    return invoke<ContextMenuStatus>("register_folder_context_menu");
}

/** See `registerFolderContextMenu` — same rejection shape. */
export function unregisterFolderContextMenu(): Promise<ContextMenuStatus> {
    return invoke<ContextMenuStatus>("unregister_folder_context_menu");
}