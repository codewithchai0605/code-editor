use git2::build::CheckoutBuilder;
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
const MAX_SCAN_DEPTH: usize = 6;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: Box<str>,
    pub status: Box<str>,
}

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
    #[allow(dead_code)]
    watcher: Option<Debouncer<RecommendedWatcher>>,
}

#[derive(Default)]
pub struct GitManagerState {
    repos: Mutex<HashMap<String, RepoInstance>>,
}

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
    }
    for subdir in subdirs {
        scan_dir(&subdir, depth + 1, found);
    }
}

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffContent {
    pub old_text: String,
    pub new_text: String,
    pub old_label: Box<str>,
    pub new_label: Box<str>,
}

fn tree_blob_content(repo: &Repository, tree: &git2::Tree, rel_str: &str) -> Option<String> {
    let entry = tree.get_path(Path::new(rel_str)).ok()?;
    let object = entry.to_object(repo).ok()?;
    let blob = object.as_blob()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

fn index_blob_content(repo: &Repository, index: &git2::Index, rel_str: &str) -> Option<String> {
    // `Index::get_path` expects forward-slash paths regardless of platform
    // (verified against git2's own source — `IndexEntry::path` says the
    // separator is always ASCII `/`), same as `Tree::get_path` above.
    let entry = index.get_path(Path::new(rel_str), 0)?; // stage 0 = the normal, non-conflicted entry
    let blob = repo.find_blob(entry.id).ok()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

/// Reads the two sides of a diff for one file — never touches the
/// filesystem's actual diff/patch machinery, just reads whichever blobs
/// are relevant and hands both full texts to the frontend, which renders
/// them with `@codemirror/merge`'s own diffing rather than parsing a unified
/// patch format.
///
/// - Staged diff: HEAD's blob -> the index's blob.
/// - Unstaged diff: the index's blob -> the working tree file on disk.
///
/// An untracked file has no index entry at all, so its "old" side comes
/// back empty automatically; a deleted file (working tree side gone, or
/// staged as removed) has its "new" side come back empty the same way —
/// both fall out of this naturally without special-casing.
#[tauri::command]
pub fn git_diff(
    state: State<GitManagerState>,
    repo_id: String,
    path: String,
    staged: bool,
) -> Result<DiffContent, String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let absolute = PathBuf::from(&path);
    let rel = relative_to_repo(&repo_path, &absolute)?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");

    let index = repo.index().map_err(|e| e.to_string())?;

    let (old_text, new_text) = if staged {
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let old = head_tree
            .as_ref()
            .and_then(|t| tree_blob_content(&repo, t, &rel_str))
            .unwrap_or_default();
        let new = index_blob_content(&repo, &index, &rel_str).unwrap_or_default();
        (old, new)
    } else {
        let old = index_blob_content(&repo, &index, &rel_str).unwrap_or_default();
        let new = std::fs::read_to_string(&absolute).unwrap_or_default();
        (old, new)
    };

    Ok(DiffContent {
        old_text,
        new_text,
        old_label: if staged { "HEAD" } else { "Index" }.into(),
        new_label: if staged { "Staged" } else { "Working Tree" }.into(),
    })
}

/// Returns just the "original" side a live editor buffer should be diffed
/// against for its gutter decorations — the same index blob `git_diff`'s
/// unstaged mode reads its old side from (so staging a hunk and continuing
/// to edit only shows the *remaining* delta, matching VS Code's own
/// dirty-diff behavior). An untracked file has no index entry, which
/// `unwrap_or_default()` turns into an empty string — i.e. the whole
/// buffer diffs as newly added, exactly like `git_diff` already does for
/// that case.
#[tauri::command]
pub fn git_file_baseline(
    state: State<GitManagerState>,
    repo_id: String,
    path: String,
) -> Result<String, String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let absolute = PathBuf::from(&path);
    let rel = relative_to_repo(&repo_path, &absolute)?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let index = repo.index().map_err(|e| e.to_string())?;
    Ok(index_blob_content(&repo, &index, &rel_str).unwrap_or_default())
}

#[tauri::command]
pub fn git_unwatch_all(state: State<GitManagerState>) {
    state.repos.lock().unwrap().clear();
}

fn open_tracked_repo(
    state: &GitManagerState,
    repo_id: &str,
) -> Result<(Repository, PathBuf), String> {
    let path = {
        let repos = state.repos.lock().unwrap();
        repos
            .get(repo_id)
            .ok_or_else(|| "Unknown repository.".to_string())?
            .path
            .clone()
    };
    let repo = Repository::open(&path).map_err(|e| e.to_string())?;
    Ok((repo, path))
}

fn relative_to_repo<'a>(repo_root: &Path, absolute: &'a Path) -> Result<&'a Path, String> {
    absolute
        .strip_prefix(repo_root)
        .map_err(|_| "Path is not inside this repository.".to_string())
}

fn emit_status(app: &AppHandle, repo_id: &str, repo_path: &Path) {
    if let Ok(status) = compute_repo_status(repo_id, repo_path) {
        let _ = app.emit("git://status-changed", &status);
    }
}

fn stage_path(repo: &Repository, rel: &Path, absolute: &Path) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    if absolute.exists() {
        index.add_path(rel).map_err(|e| e.to_string())?;
    } else {
        index.remove_path(rel).map_err(|e| e.to_string())?;
    }
    index.write().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_stage_file(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
    path: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let absolute = PathBuf::from(&path);
    let rel = relative_to_repo(&repo_path, &absolute)?;
    stage_path(&repo, rel, &absolute)?;
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

#[tauri::command]
pub fn git_stage_all(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let status = compute_repo_status(&repo_id, &repo_path)?;
    for file in status.unstaged.iter().chain(status.untracked.iter()) {
        let absolute = PathBuf::from(file.path.as_ref());
        let rel = relative_to_repo(&repo_path, &absolute)?;
        stage_path(&repo, rel, &absolute)?;
    }
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

#[tauri::command]
pub fn git_unstage_file(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
    path: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let absolute = PathBuf::from(&path);
    let rel = relative_to_repo(&repo_path, &absolute)?;
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    repo.reset_default(
        head_commit.as_ref().map(|c| c.as_object()),
        [rel.to_string_lossy().into_owned()],
    )
    .map_err(|e| e.to_string())?;
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

#[tauri::command]
pub fn git_unstage_all(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let status = compute_repo_status(&repo_id, &repo_path)?;
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let paths: Vec<String> = status
        .staged
        .iter()
        .filter_map(|f| {
            let absolute = PathBuf::from(f.path.as_ref());
            relative_to_repo(&repo_path, &absolute)
                .ok()
                .map(|r| r.to_string_lossy().into_owned())
        })
        .collect();
    if !paths.is_empty() {
        repo.reset_default(head_commit.as_ref().map(|c| c.as_object()), paths)
            .map_err(|e| e.to_string())?;
    }
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

fn discard_path(repo: &Repository, rel: &Path, absolute: &Path) -> Result<(), String> {
    let status = repo.status_file(rel).map_err(|e| e.to_string())?;
    if status.is_wt_new() && !status.is_index_new() {
        std::fs::remove_file(absolute).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let mut checkout = CheckoutBuilder::new();
    checkout.force();
    checkout.path(rel.to_string_lossy().into_owned());
    repo.checkout_index(None, Some(&mut checkout))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_discard_file(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
    path: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let absolute = PathBuf::from(&path);
    let rel = relative_to_repo(&repo_path, &absolute)?;
    discard_path(&repo, rel, &absolute)?;
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

#[tauri::command]
pub fn git_discard_all(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
) -> Result<(), String> {
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;
    let status = compute_repo_status(&repo_id, &repo_path)?;
    for file in status.unstaged.iter().chain(status.untracked.iter()) {
        let absolute = PathBuf::from(file.path.as_ref());
        let rel = relative_to_repo(&repo_path, &absolute)?;
        discard_path(&repo, rel, &absolute)?;
    }
    emit_status(&app, &repo_id, &repo_path);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub oid: Box<str>,
}

#[tauri::command]
pub fn git_commit(
    app: AppHandle,
    state: State<GitManagerState>,
    repo_id: String,
    message: String,
) -> Result<CommitResult, String> {
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty.".to_string());
    }
    let (repo, repo_path) = open_tracked_repo(&state, &repo_id)?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if let Some(parent) = &parent_commit {
        if parent.tree_id() == tree_oid {
            return Err("Nothing staged to commit.".to_string());
        }
    }

    let signature = repo.signature().map_err(|e| e.to_string())?;
    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    let oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            &message,
            &tree,
            &parents,
        )
        .map_err(|e| e.to_string())?;

    emit_status(&app, &repo_id, &repo_path);
    Ok(CommitResult {
        oid: oid.to_string().into_boxed_str(),
    })
}
