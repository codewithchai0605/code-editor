import { invoke } from "@tauri-apps/api/core";

/** Allocates a terminal id without spawning anything yet — call this,
 *  attach a `pty:data:{id}` listener, and only *then* call
 *  `createTerminal`. See TerminalView.svelte for why the ordering matters:
 *  spawning first (the old single-step flow) let a fast shell write its
 *  whole initial prompt before anything was listening for it. */
export function reserveTerminalId(): Promise<string> {
  return invoke<string>("reserve_terminal_id");
}

/** Spawns the PTY + shell for a previously-reserved `id`. */
export function createTerminal(id: string, cwd?: string): Promise<void> {
  return invoke("create_terminal", { id, cwd });
}

export function writeToTerminal(id: string, data: string): Promise<void> {
  return invoke("write_to_terminal", { id, data });
}

export function resizeTerminal(id: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_terminal", { id, cols, rows });
}

export function closeTerminal(id: string): Promise<void> {
  return invoke("close_terminal", { id });
}
