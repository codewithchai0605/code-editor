use git2::{BranchType, Repository, StatusOptions};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// Directories never worth descending into while scanning for repos —
/// either because a nested `.git` there is essentially never intentional
/// (a vendored dependency checked out with its own repo) or because
/// descending into them at all is expensive for no benefit (thousands of
/// files deep in `node_modules`).
const VENDOR_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    ".dart_tool",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".nuxt",
    "vendor",
    ".gradle",
    ".cargo",
];
/// How many directory levels deep to scan from the workspace root. Deep
/// enough for any realistic monorepo layout, shallow enough that a huge
/// tree with no repos in it doesn't turn "open folder" into a slow walk
/// of the entire filesystem under it.
const MAX_SCAN_DEPTH: usize = 6;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: Box<str>,
    /// "modified" | "added" | "deleted" | "renamed" | "typechange"
    pub status: Box<str>,
}

/// Lightweight per-repo header info — enough to render every detected
/// repo's collapsed sidebar header (name, branch, ahead/behind) without
/// paying for a full status walk of each one up front.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub id: Box<str>,
    pub path: Box<str>,
    pub name: Box<str>,
    pub current_branch: Option<Box<str>>,
    pub ahead: usize,
    pub behind: usize,
}

/// The heavier per-repo payload — every changed file, categorized the way
/// VS Code's own Source Control view groups them. Recomputed and replaced
/// wholesale on every refresh (see `start_watcher`'s callback), never
/// mutated field-by-field, which is exactly the case `Box<[T]>` is for
/// rather than carrying `Vec`'s spare capacity around between refreshes
/// that will just discard it anyway.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub repo_id: Box<str>,
    pub branch: Option<Box<str>>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Box<[FileEntry]>,
    pub unstaged: Box<[FileEntry]>,
    pub untracked: Box<[FileEntry]>,
    pub conflicted: Box<[FileEntry]>,
}

struct RepoInstance {
    path: PathBuf,
    #[allow(dead_code)] // kept alive only to keep the watcher running — dropped, not read
    watcher: Option<Debouncer<RecommendedWatcher>>,
}

#[derive(Default)]
pub struct GitManagerState {
    repos: Mutex<HashMap<String, RepoInstance>>,
}

/// Recursively finds every repo root under `root` — a directory containing
/// a `.git` entry, which may be a real directory (a normal repo) or a
/// *file* containing `gitdir: ...` (a linked worktree or submodule).
/// Doesn't stop at the first repo found in a subtree: a repo containing a
/// submodule checked out as an ordinary subfolder has its own nested
/// `.git`, which is worth surfacing as its own section too (per
/// AGENT-git.md §2 — "submodules should behave like an independent repo
/// in the sidebar").
pub fn scan_for_repos(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_dir(root, 0, &mut found);
    found
}

fn scan_dir(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut has_git_marker = false;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == ".git" {
            has_git_marker = true;
            continue;
        }
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_dir() {
                let name_str = name.to_string_lossy();
                if !VENDOR_DIRS.contains(&name_str.as_ref()) {
                    subdirs.push(entry.path());
                }
            }
        }
    }

    if has_git_marker {
        found.push(dir.to_path_buf());
        // Deliberately not `return`ing here — still worth recursing for a
        // nested repo (e.g. a submodule) inside this one.
    }
    for subdir in subdirs {
        scan_dir(&subdir, depth + 1, found);
    }
}

/// git2 always reports status paths with `/` separators internally, even
/// on Windows — joining as-is onto a native Windows `PathBuf` would
/// produce a mixed-separator string that silently fails to string-match
/// this app's other path values (see the file-tree badge bug this exact
/// normalization originally fixed).
fn to_native_path(root: &Path, git_relative_path: &str) -> Box<str> {
    let normalized: Cow<str> = if cfg!(windows) {
        Cow::Owned(git_relative_path.replace('/', "\\"))
    } else {
        Cow::Borrowed(git_relative_path)
    };
    root.join(normalized.as_ref())
        .to_string_lossy()
        .into_owned()
        .into_boxed_str()
}

fn branch_and_ahead_behind(repo: &Repository) -> (Option<Box<str>>, usize, usize) {
    let head = repo.head().ok();

    let branch: Option<Box<str>> = head
        .as_ref()
        .and_then(|head| head.shorthand().ok().map(|s| s.to_owned().into_boxed_str()))
        .or_else(|| {
            // Detached HEAD — AGENT-git.md §11 calls for showing the short
            // SHA rather than a blank/broken label in this state.
            head.as_ref().and_then(|head| head.target()).map(|oid| {
                let full = oid.to_string();
                full[..7.min(full.len())].to_string().into_boxed_str()
            })
        });

    let (ahead, behind) = head
        .as_ref()
        .and_then(|head| head.shorthand().ok())
        .and_then(|name| {
            let local_branch = repo.find_branch(name, BranchType::Local).ok()?;
            let upstream = local_branch.upstream().ok()?;
            let local_oid = local_branch.get().target()?;
            let upstream_oid = upstream.get().target()?;
            repo.graph_ahead_behind(local_oid, upstream_oid).ok()
        })
        .unwrap_or((0, 0));

    (branch, ahead, behind)
}

fn compute_repo_info(id: &str, path: &Path) -> Result<RepoInfo, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let (branch, ahead, behind) = branch_and_ahead_behind(&repo);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(RepoInfo {
        id: id.into(),
        path: path.to_string_lossy().into_owned().into_boxed_str(),
        name: name.into_boxed_str(),
        current_branch: branch,
        ahead,
        behind,
    })
}

fn compute_repo_status(id: &str, path: &Path) -> Result<RepoStatus, String> {
    // `open()`, not `discover()` — the caller already knows the exact repo
    // root from the scanner, and `discover()` walking upward on a path
    // that turned out not to be a repo root itself could silently resolve
    // to a different, unrelated ancestor repo.
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let (branch, ahead, behind) = branch_and_ahead_behind(&repo);

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut conflicted = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();
        let Ok(rel_path) = entry.path() else {
            continue;
        };
        let native_path = to_native_path(path, rel_path);

        if status.is_conflicted() {
            conflicted.push(FileEntry {
                path: native_path,
                status: "conflicted".into(),
            });
            continue;
        }
        if status.is_wt_new() && !status.is_index_new() {
            untracked.push(FileEntry {
                path: native_path,
                status: "untracked".into(),
            });
            continue;
        }

        let is_staged = status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange();
        let is_unstaged = status.is_wt_modified()
            || status.is_wt_deleted()
            || status.is_wt_renamed()
            || status.is_wt_typechange();

        // A file can legitimately appear in both lists at once — staged
        // one change, then modified it again — matching VS Code's own
        // "Staged Changes" / "Changes" split rather than picking one.
        if is_staged {
            let kind = if status.is_index_new() {
                "added"
            } else if status.is_index_deleted() {
                "deleted"
            } else if status.is_index_renamed() {
                "renamed"
            } else {
                "modified"
            };
            staged.push(FileEntry {
                path: if is_unstaged {
                    native_path.clone()
                } else {
                    native_path
                },
                status: kind.into(),
            });
        }
        if is_unstaged {
            let kind = if status.is_wt_deleted() {
                "deleted"
            } else if status.is_wt_renamed() {
                "renamed"
            } else {
                "modified"
            };
            unstaged.push(FileEntry {
                path: to_native_path(path, rel_path),
                status: kind.into(),
            });
        }
    }

    Ok(RepoStatus {
        repo_id: id.into(),
        branch,
        ahead,
        behind,
        staged: staged.into_boxed_slice(),
        unstaged: unstaged.into_boxed_slice(),
        untracked: untracked.into_boxed_slice(),
        conflicted: conflicted.into_boxed_slice(),
    })
}

/// Starts a `.git`-internals watcher for one repo — same approach as the
/// original single-repo version (watch the `.git` dir non-recursively for
/// HEAD/index/packed-refs, plus `refs/` recursively for loose ref
/// updates), just scoped per-`RepoInstance` now, and emitting
/// `git://status-changed` tagged with the specific repo that changed
/// rather than a bare signal to re-fetch everything.
fn start_watcher(
    app: AppHandle,
    id: String,
    repo_path: PathBuf,
) -> Option<Debouncer<RecommendedWatcher>> {
    let git_dir = Repository::open(&repo_path).ok()?.path().to_path_buf();

    let watched_path = repo_path.clone();
    let debouncer = new_debouncer(
        Duration::from_millis(300),
        move |result: DebounceEventResult| {
            if result.is_err() {
                return;
            }
            match compute_repo_status(&id, &watched_path) {
                Ok(status) => {
                    let _ = app.emit("git://status-changed", &status);
                }
                Err(message) => {
                    let _ = app.emit(
                    "git://operation-error",
                    serde_json::json!({ "repoId": id, "operation": "status", "message": message }),
                );
                }
            }
        },
    );

    let mut debouncer = debouncer.ok()?;
    let watcher = debouncer.watcher();
    let _ = watcher.watch(&git_dir, RecursiveMode::NonRecursive);
    let refs_dir = git_dir.join("refs");
    if refs_dir.exists() {
        let _ = watcher.watch(&refs_dir, RecursiveMode::Recursive);
    }
    Some(debouncer)
}

/// Scans the opened workspace for every repo under it, starts a watcher
/// for each, and returns their lightweight header info (see `RepoInfo`) —
/// the sidebar renders these immediately, then fetches each repo's full
/// `RepoStatus` separately via `git_status`. Replaces whatever repos were
/// previously tracked (e.g. from a folder opened earlier in the session);
/// dropping the old `RepoInstance`s here is what stops their watchers.
#[tauri::command]
pub fn git_list_repos(
    app: AppHandle,
    state: State<GitManagerState>,
    workspace_path: String,
) -> Vec<RepoInfo> {
    let root = PathBuf::from(&workspace_path);
    let found_paths = scan_for_repos(&root);

    let mut repos = state.repos.lock().unwrap();
    repos.clear();

    let mut infos = Vec::new();
    for path in found_paths {
        let id = path.to_string_lossy().into_owned();
        if let Ok(info) = compute_repo_info(&id, &path) {
            let watcher = start_watcher(app.clone(), id.clone(), path.clone());
            repos.insert(id, RepoInstance { path, watcher });
            infos.push(info);
        }
        // Not actually a usable repo (e.g. a corrupt .git) — skip it
        // rather than failing the whole scan over one bad entry.
    }
    infos
}

#[tauri::command]
pub fn git_status(state: State<GitManagerState>, repo_id: String) -> Result<RepoStatus, String> {
    let repos = state.repos.lock().unwrap();
    let repo = repos
        .get(&repo_id)
        .ok_or_else(|| "Unknown repository.".to_string())?;
    compute_repo_status(&repo_id, &repo.path)
}

/// Stops every tracked repo's watcher — call when closing the workspace
/// (opening a *different* folder already handles this via `git_list_repos`
/// clearing and replacing the map; this covers the case where no new
/// folder is opened right after).
#[tauri::command]
pub fn git_unwatch_all(state: State<GitManagerState>) {
    state.repos.lock().unwrap().clear();
}
