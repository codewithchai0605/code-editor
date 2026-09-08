use crate::state::AppState;
use git2::{BranchType, Repository, Status, StatusOptions};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: Box<str>,
    /// One of "modified" | "added" | "deleted" | "renamed" | "untracked" |
    /// "conflicted" — deliberately a string rather than a tagged enum:
    /// the frontend only ever needs it as a lookup key for a badge
    /// letter/color (see FileTreeNode.svelte), never branches deeply on it.
    pub status: Box<str>,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusResult {
    pub is_repo: bool,
    /// `None` covers both "not a repo" and an unborn branch (a fresh repo
    /// before its first commit) — `repo.head()` errors in the latter case,
    /// which isn't a real failure worth surfacing as one.
    pub branch: Option<Box<str>>,
    pub ahead: usize,
    pub behind: usize,
    pub files: Box<[GitFileStatus]>,
}

/// Collapses git2's bitflag `Status` into the single label the file tree
/// actually renders. Where a path has both an index (staged) and a
/// worktree (unstaged) change at once, the worktree side wins for the
/// *label* shown — "still has unstaged changes on top" is the more
/// actionable thing to surface — while `staged` independently reports
/// whether *any* of it is already staged.
fn classify(status: Status) -> Option<(&'static str, bool)> {
    let staged = status.is_index_new()
        || status.is_index_modified()
        || status.is_index_deleted()
        || status.is_index_renamed()
        || status.is_index_typechange();

    if status.is_conflicted() {
        Some(("conflicted", staged))
    } else if status.is_wt_deleted() || status.is_index_deleted() {
        Some(("deleted", staged))
    } else if status.is_wt_renamed() || status.is_index_renamed() {
        Some(("renamed", staged))
    } else if status.is_wt_modified() || status.is_index_modified() || status.is_wt_typechange() {
        Some(("modified", staged))
    } else if status.is_wt_new() {
        // A file only in the worktree (never staged) is untracked; one
        // that's also `is_index_new` has been `git add`-ed as a new file.
        Some(("untracked", status.is_index_new()))
    } else if status.is_index_new() {
        Some(("added", true))
    } else {
        None
    }
}

fn compute_status(root: &Path) -> Result<GitStatusResult, String> {
    let repo = match Repository::discover(root) {
        Ok(repo) => repo,
        Err(_) => {
            // Not inside a git repo at all — a normal, common state, not
            // an error the caller needs to react to.
            return Ok(GitStatusResult {
                is_repo: false,
                branch: None,
                ahead: 0,
                behind: 0,
                files: Box::new([]),
            });
        }
    };

    let branch: Option<Box<str>> = repo
        .head()
        .ok()
        // `shorthand()` returns a `Result` (it can fail on non-UTF8 ref
        // names), not an `Option` — `.ok()` bridges it into the `Option`
        // chain the rest of this is built on.
        .and_then(|head| head.shorthand().ok().map(|s| s.to_owned().into_boxed_str()));

    // Ahead/behind needs a configured upstream — most local-only branches
    // don't have one, which isn't an error, just nothing to report.
    let (ahead, behind) = branch
        .as_deref()
        .and_then(|name| {
            let local_branch = repo.find_branch(name, BranchType::Local).ok()?;
            let upstream = local_branch.upstream().ok()?;
            let local_oid = local_branch.get().target()?;
            let upstream_oid = upstream.get().target()?;
            repo.graph_ahead_behind(local_oid, upstream_oid).ok()
        })
        .unwrap_or((0, 0));

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;

    // git2 reports status paths relative to the repo's workdir, not
    // relative to `root` (which may be a subdirectory the user opened,
    // not the repo root) — join back onto the workdir so the frontend can
    // match these against the absolute paths it already has.
    let workdir = repo.workdir().unwrap_or(root);

    let files: Vec<GitFileStatus> = statuses
        .iter()
        .filter_map(|entry| {
            let (kind, staged) = classify(entry.status())?;
            let rel_path = entry.path().ok()?;
            // git always reports this path with `/` separators internally,
            // even on Windows — joining it as-is onto `workdir` (a native
            // Windows `PathBuf`) produces a *mixed*-separator string
            // (`C:\repo\src/components/App.svelte`), which then silently
            // never string-matches `FsEntry.path` from fs.rs (built via
            // native `PathBuf` joins throughout, so pure backslashes on
            // Windows). That mismatch is why file-tree badges never
            // rendered on Windows despite the branch/status data itself
            // being fetched correctly — normalizing to the OS's own
            // separator first makes the two sides compare equal.
            let normalized: std::borrow::Cow<str> = if cfg!(windows) {
                std::borrow::Cow::Owned(rel_path.replace('/', "\\"))
            } else {
                std::borrow::Cow::Borrowed(rel_path)
            };
            Some(GitFileStatus {
                path: workdir
                    .join(normalized.as_ref())
                    .to_string_lossy()
                    .into_owned()
                    .into_boxed_str(),
                status: kind.into(),
                staged,
            })
        })
        .collect();

    Ok(GitStatusResult {
        is_repo: true,
        branch,
        ahead,
        behind,
        files: files.into_boxed_slice(),
    })
}

#[tauri::command]
pub fn git_status(path: String) -> Result<GitStatusResult, String> {
    compute_status(Path::new(&path))
}

/// Watches the handful of paths directly inside `.git` that every git
/// operation touches — `HEAD` (checkout/commit), `index` (stage/unstage),
/// `packed-refs`/`MERGE_HEAD`/etc — plus `refs/**` recursively for loose
/// branch/tag updates, and emits `git:changed` so the frontend can re-run
/// `git_status` without polling. Mirrors `commands::fs::start_watcher`'s
/// shape closely on purpose.
///
/// Watching `.git` itself non-recursively (rather than only the literal
/// `HEAD` file) is deliberate: it also catches `packed-refs`, `MERGE_HEAD`
/// appearing mid-merge, etc., in one watch, and — since it does *not*
/// recurse — never gets flooded by the constant small writes under
/// `.git/objects/` that every commit produces.
#[tauri::command]
pub fn git_watch(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    let mut watcher_slot = state.git_watcher.lock().unwrap();
    *watcher_slot = None; // drop any previous watcher first

    // Opening a plain, non-git folder is the common case, not an error —
    // matches `compute_status`'s handling of the same situation. Returning
    // `Err` here would surface as an unhandled rejection on the frontend's
    // fire-and-forget `gitStore.setRoot()` call for every non-repo folder.
    let repo = match Repository::discover(Path::new(&path)) {
        Ok(repo) => repo,
        Err(_) => return Ok(()),
    };
    // `repo.path()` is the real `.git` directory, correctly resolved even
    // for a linked worktree or submodule (where it isn't literally
    // `<root>/.git`) — using it instead of hand-joining ".git" is what
    // makes this correct in those cases too.
    let git_dir = repo.path().to_path_buf();

    let app_handle = app.clone();
    let debouncer = new_debouncer(
        Duration::from_millis(300),
        move |result: DebounceEventResult| {
            if result.is_ok() {
                let _ = app_handle.emit("git:changed", ());
            }
        },
    );

    if let Ok(mut debouncer) = debouncer {
        let watcher = debouncer.watcher();
        let _ = watcher.watch(&git_dir, RecursiveMode::NonRecursive);
        let refs_dir = git_dir.join("refs");
        if refs_dir.exists() {
            let _ = watcher.watch(&refs_dir, RecursiveMode::Recursive);
        }
        *watcher_slot = Some(debouncer);
    }

    Ok(())
}

#[tauri::command]
pub fn git_unwatch(state: State<AppState>) -> Result<(), String> {
    *state.git_watcher.lock().unwrap() = None;
    Ok(())
}
