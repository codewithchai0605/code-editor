// pty.ts
import { invoke } from "@tauri-apps/api/core";

export function reserveTerminalId(): Promise<string> {
  return invoke<string>("reserve_terminal_id");
}

export function createTerminal(id: string, cwd?: string): Promise<void> {
  return invoke("create_terminal", { id, cwd });
}

export function writeToTerminal(id: string, data: string): Promise<void> {
  // Convert string to byte array (Uint8Array)
  const encoder = new TextEncoder();
  const bytes = encoder.encode(data);
  return invoke("write_to_terminal", { id, data: Array.from(bytes) });
}

export function resizeTerminal(id: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_terminal", { id, cols, rows });
}

export function closeTerminal(id: string): Promise<void> {
  return invoke("close_terminal", { id });
}