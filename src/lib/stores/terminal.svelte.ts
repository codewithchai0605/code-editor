import { createTerminal, closeTerminal } from "$lib/api/pty";
import { layout } from "./layout.svelte";

export interface TerminalSession {
  id: string;
  title: string;
  exited: boolean;
}

class TerminalStore {
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<string | null>(null);
  private counter = 0;

  async createSession(cwd?: string): Promise<void> {
    const id = await createTerminal(cwd);
    this.counter++;
    this.sessions.push({ id, title: `Terminal ${this.counter}`, exited: false });
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
      await closeTerminal(id);
    } catch (err) {
      console.error("Failed to close terminal session:", err);
    }
  }

  markExited(id: string): void {
    const session = this.sessions.find((s) => s.id === id);
    if (session) session.exited = true;
  }
}

export const terminalStore = new TerminalStore();
