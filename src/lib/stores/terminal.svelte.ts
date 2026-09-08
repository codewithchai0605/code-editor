import { reserveTerminalId, closeTerminal } from "$lib/api/pty";
import { layout } from "./layout.svelte";

export interface TerminalSession {
  id: string;
  title: string;
  exited: boolean;
  /** Read by TerminalView.svelte, which is what actually calls
   *  `createTerminal(id, cwd)` — see its onMount for why the spawn itself
   *  doesn't happen here. */
  cwd?: string;
}

class TerminalStore {
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<string | null>(null);
  private counter = 0;

  /** Reserves an id and adds a session placeholder — it does *not* spawn
   *  the actual shell process. That happens in TerminalView.svelte's
   *  onMount, once its `pty:data:{id}` listener is confirmed attached, so
   *  the shell can never write output before anything is listening for
   *  it. This method itself only awaits a cheap id-allocation call (no
   *  process spawn), so there's no meaningful delay before the session
   *  appears and its view starts mounting. */
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
      await closeTerminal(id);
    } catch (err) {
      console.error("Failed to close terminal session:", err);
    }
  }

  markExited(id: string): void {
    void this.closeSession(id);
  }
}

export const terminalStore = new TerminalStore();
