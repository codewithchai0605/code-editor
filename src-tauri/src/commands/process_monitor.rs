use crate::state::{ProcessMonitorState, PtyState};
use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub id: String,
    pub label: String,
    /// "main" | "terminal" | "helper" — lets the frontend pick an icon and
    /// a sort position without string-matching on `label`.
    pub kind: String,
    pub pid: u32,
    pub memory_bytes: u64,
    pub cpu_percent: f32,
}

/// Registers a process this app spawned outside of `PtyState` (a future
/// language server, a formatter daemon, ripgrep for a long-running search
/// — anything) so it shows up in the Task Manager alongside the main
/// process and terminal shells. Call `unregister_process` with the same
/// `id` once it exits; an id that's already registered simply overwrites
/// the previous entry (covers a restarted server reusing the same id).
#[tauri::command]
pub fn register_process(state: State<ProcessMonitorState>, id: String, label: String, pid: u32) {
    state.extra.lock().unwrap().insert(id, (label, pid));
}

#[tauri::command]
pub fn unregister_process(state: State<ProcessMonitorState>, id: String) {
    state.extra.lock().unwrap().remove(&id);
}

/// Force-terminates a tracked process. Refuses to kill this app's own
/// main-process pid — "End Task" on yourself isn't a case worth wiring up,
/// and guarding it here (not just in the UI, which a determined caller
/// could route around) means the check can't be bypassed by hand-crafting
/// the invoke call.
#[tauri::command]
pub fn kill_process(state: State<ProcessMonitorState>, pid: u32) -> Result<bool, String> {
    if pid == std::process::id() {
        return Err("Refusing to kill the editor's own process.".to_string());
    }
    let mut sys = state.system.lock().unwrap();
    let target = Pid::from(pid as usize);
    sys.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
    Ok(sys.process(target).map(|p| p.kill()).unwrap_or(false))
}

/// This app's own process tree: the main process, every live terminal
/// shell (read straight from `PtyState`, not duplicated bookkeeping), and
/// anything registered via `register_process`. Deliberately *not* a
/// system-wide process list — refreshing sysinfo for a handful of known
/// PIDs is cheap; refreshing (or even just listing) every process on the
/// machine is not, and this app has no use for the rest of them.
#[tauri::command]
pub fn list_tracked_processes(
    state: State<ProcessMonitorState>,
    pty_state: State<PtyState>,
) -> Result<Vec<ProcessInfo>, String> {
    let mut entries: Vec<(String, String, &'static str, u32)> = Vec::new();

    entries.push((
        "main".to_string(),
        "Main process".to_string(),
        "main",
        std::process::id(),
    ));

    {
        let sessions = pty_state.sessions.lock().unwrap();
        for (id, session) in sessions.iter() {
            if let Some(pid) = session.running().and_then(|pty| pty.child.process_id()) {
                entries.push((id.clone(), "Terminal shell".to_string(), "terminal", pid));
            }
        }
    }

    {
        let extra = state.extra.lock().unwrap();
        for (id, (label, pid)) in extra.iter() {
            entries.push((id.clone(), label.clone(), "helper", *pid));
        }
    }

    let pids: Vec<Pid> = entries
        .iter()
        .map(|(_, _, _, pid)| Pid::from(*pid as usize))
        .collect();

    let mut sys = state.system.lock().unwrap();
    // Targeted refresh of just these PIDs (not `refresh_all`/`new_all`) —
    // the whole point of tracking "this app's own processes" as the scope
    // is that this stays cheap enough to poll every second or two.
    sys.refresh_processes(ProcessesToUpdate::Some(&pids), true);

    Ok(entries
        .into_iter()
        .map(|(id, label, kind, pid)| {
            let (memory_bytes, cpu_percent) = sys
                .process(Pid::from(pid as usize))
                .map(|p| (p.memory(), p.cpu_usage()))
                .unwrap_or((0, 0.0));
            ProcessInfo {
                id,
                label,
                kind: kind.to_string(),
                pid,
                memory_bytes,
                cpu_percent,
            }
        })
        .collect())
}
