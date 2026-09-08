import { invoke } from "@tauri-apps/api/core";

/** Spawns a new PTY + shell, returns its session id. */
export function createTerminal(cwd?: string): Promise<string> {
  return invoke<string>("create_terminal", { cwd });
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
