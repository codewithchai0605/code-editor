import { invoke } from "@tauri-apps/api/core";

export interface ProcessInfo {
    id: string;
    label: string;
    kind: "main" | "terminal" | "helper";
    pid: number;
    memoryBytes: number;
    cpuPercent: number;
}

export function listTrackedProcesses(): Promise<ProcessInfo[]> {
    return invoke<ProcessInfo[]>("list_tracked_processes");
}

export function killProcess(pid: number): Promise<boolean> {
    return invoke<boolean>("kill_process", { pid });
}

/** For a future subsystem (a language server, a formatter daemon) to
 *  announce itself to the Task Manager. Not called anywhere yet — LSP is
 *  deferred — but wiring the API now means adding that later is just a
 *  `registerProcess(...)` call at spawn time, not new plumbing. */
export function registerProcess(id: string, label: string, pid: number): Promise<void> {
    return invoke("register_process", { id, label, pid });
}

export function unregisterProcess(id: string): Promise<void> {
    return invoke("unregister_process", { id });
}
