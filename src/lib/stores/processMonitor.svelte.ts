import { listTrackedProcesses, killProcess, type ProcessInfo } from "$lib/api/processes";

/** Polls this app's own process list for the Task Manager view. Polling
 *  (rather than push-based events) is deliberate — memory/CPU numbers are
 *  cheap to re-fetch on a timer and don't need push-precision, unlike e.g.
 *  git or filesystem changes where an external event actually happened. */
class ProcessMonitorStore {
  processes = $state<ProcessInfo[]>([]);
  lastError = $state<string | null>(null);

  private pollHandle: ReturnType<typeof setInterval> | undefined;

  async refresh(): Promise<void> {
    try {
      this.processes = await listTrackedProcesses();
      this.lastError = null;
    } catch (err) {
      this.lastError = err instanceof Error ? err.message : String(err);
    }
  }

  /** Call when the Task Manager view becomes visible; pair with `stop()`
   *  when it's hidden. No reason to keep sampling memory/CPU for a panel
   *  nobody's looking at. Safe to call repeatedly — a second call while
   *  already polling is a no-op. */
  start(): void {
    if (this.pollHandle) return;
    void this.refresh();
    this.pollHandle = setInterval(() => this.refresh(), 1500);
  }

  stop(): void {
    if (this.pollHandle) clearInterval(this.pollHandle);
    this.pollHandle = undefined;
  }

  async kill(pid: number): Promise<void> {
    try {
      await killProcess(pid);
      await this.refresh();
    } catch (err) {
      this.lastError = err instanceof Error ? err.message : String(err);
    }
  }
}

export const processMonitorStore = new ProcessMonitorStore();
