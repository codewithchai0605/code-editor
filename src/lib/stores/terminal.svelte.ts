import { reserveTerminalId, closeTerminal } from "$lib/api/pty";
import { layout } from "./layout.svelte";

export interface TerminalSession {
  id: string;
  title: string;
  exited: boolean;
  cwd?: string;
}

class TerminalStore {
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<string | null>(null);
  private counter = 0;

  async createSession(cwd?: string): Promise<void> {
    const id = await reserveTerminalId();
    this.counter++;
    this.sessions.push({ id, title: `Terminal ${this.counter}`, exited: false, cwd });
    this.activeId = id;
  }

  setActive(id: string): void {
    if (this.sessions.some((s) => s.id === id)) this.activeId = id;
  }

  async closeSession(id: string): Promise<void> {
    const idx = this.sessions.findIndex((s) => s.id === id);
    if (idx === -1) return;

    this.sessions.splice(idx, 1);

    if (this.activeId === id) {
      const next = this.sessions[idx] ?? this.sessions[idx - 1];
      this.activeId = next ? next.id : null;
    }

    if (this.sessions.length === 0) layout.panelOpen = false;

    try {
      // Fire and forget: if the backend already killed it (e.g., via crash/exit), 
      // we don't want to block the UI or spam the console.
      closeTerminal(id).catch(() => { });
    } catch (err) {
      console.error("Failed to close terminal session:", err);
    }
  }

  markExited(id: string): void {
    // Optionally: set exited: true here instead of instantly closing, 
    // if you want users to see the "process exited" state before it disappears.
    void this.closeSession(id);
  }
}

export const terminalStore = new TerminalStore();