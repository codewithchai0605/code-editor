use crate::state::AppState;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use serde::Serialize;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Types shared with the frontend
//
// `name`/`path` use `Box<str>` rather than `String`, and the collections
// returned by commands use `Box<[T]>` rather than `Vec<T>`: every one of
// these values is built once, serialized to JSON, and dropped — they never
// grow after construction, so the extra capacity field `String`/`Vec` carry
// (and the reallocation slack that can come with it) is pure overhead here.
// Serde serializes both exactly like their unboxed counterparts, so this is
// invisible to the frontend — see src/lib/types/fs.ts, unchanged.
//
// Command *parameters* below (e.g. `sources: Vec<String>`) deliberately stay
// as `String`/`Vec<String>`: they're deserialized fresh per call and
// immediately consumed into `PathBuf`s, so there's nothing long-lived to
// optimize and the standard owned types keep the command signatures
// idiomatic and friction-free.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub name: Box<str>,
    pub path: Box<str>,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Structured so the frontend can branch on `kind` instead of pattern-matching
/// an opaque string — the conflict variant in particular carries enough info
/// to drive an overwrite-confirmation dialog without a second round trip.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsError {
    Conflict { paths: Box<[Box<str>]> },
    InvalidName { message: Box<str> },
    Unsupported { message: Box<str> },
    Io { message: Box<str> },
}

impl From<std::io::Error> for FsError {
    fn from(err: std::io::Error) -> Self {
        FsError::Io {
            message: err.to_string().into_boxed_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyProgress {
    // Plain scalars — nothing here allocates, so there's no Box to apply.
    done: usize,
    total: usize,
}

/// Payload for the `fs:changed` event. `dirs` is the parent directories that
/// might need re-listing (the tree's job — a no-op for anything not
/// currently expanded). `paths` is every individual path notify actually
/// saw touched, *not* reduced to parents — this is what lets an open editor
/// tab match itself against the event directly (see `EditorStore`'s
/// `handleExternalChange` on the frontend) instead of only ever finding out
/// its file changed the next time someone happens to reopen it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FsChangedPayload {
    // Same reasoning as `CopyProgress` above for deriving `Clone`:
    // `Emitter::emit` requires it.
    dirs: Box<[Box<str>]>,
    paths: Box<[Box<str>]>,
}

// ---------------------------------------------------------------------------
// Name validation (mirrors src/lib/utils/validateName.ts on the frontend —
// that copy gives instant feedback, this one is the actual security
// boundary since the frontend check can always be bypassed)
// ---------------------------------------------------------------------------

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn validate_name(name: &str) -> Result<(), FsError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FsError::InvalidName {
            message: "Name can't be empty.".into(),
        });
    }
    if trimmed == "." || trimmed == ".." {
        return Err(FsError::InvalidName {
            message: "That name isn't allowed.".into(),
        });
    }
    if trimmed.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
        || trimmed.chars().any(|c| (c as u32) < 0x20)
    {
        return Err(FsError::InvalidName {
            message: "Name can't contain: < > : \" / \\ | ? *".into(),
        });
    }
    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err(FsError::InvalidName {
            message: "Name can't end with a space or period on Windows.".into(),
        });
    }
    let base = trimmed.split('.').next().unwrap_or(trimmed).to_uppercase();
    if WINDOWS_RESERVED.contains(&base.as_str()) {
        return Err(FsError::InvalidName {
            message: format!("\"{base}\" is a reserved name on Windows.").into_boxed_str(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

fn entry_for(path: &Path) -> Result<FsEntry, FsError> {
    let metadata = std::fs::symlink_metadata(path)?;
    let is_symlink = metadata.file_type().is_symlink();
    // For a symlink we still want to know whether it *points at* a
    // directory (so the tree can render it as expandable), which needs a
    // second, link-following stat call.
    let is_dir = if is_symlink {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    } else {
        metadata.is_dir()
    };
    let name: Box<str> = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
        .into_boxed_str();
    let path_str: Box<str> = path.to_string_lossy().into_owned().into_boxed_str();
    Ok(FsEntry {
        name,
        path: path_str,
        is_dir,
        is_symlink,
    })
}

fn list_dir_sorted(dir: &Path) -> std::io::Result<Box<[FsEntry]>> {
    // Sorting needs a growable, in-place-mutable buffer, so we build a `Vec`
    // first and only box it up (trimming any excess capacity) once it's
    // final.
    let mut entries: Vec<FsEntry> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| entry_for(&e.path()).ok())
        .collect();

    // Folders first, then alphabetical (case-insensitive, so "Zeta" sorts
    // next to "apple" the way people expect rather than before it).
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries.into_boxed_slice())
}

#[tauri::command]
pub fn read_dir(path: String) -> Result<Box<[FsEntry]>, FsError> {
    Ok(list_dir_sorted(Path::new(&path))?)
}

// ---------------------------------------------------------------------------
// Workspace + file watcher
// ---------------------------------------------------------------------------

/// True if any path component of `path` is one of `ALWAYS_SKIP_DIRS` (see
/// below — the same list `list_workspace_files` prunes for Quick Open).
/// Applied to every raw watcher event *before* it costs anything else: no
/// string allocation for the hashsets, no emit, nothing. This is what keeps
/// `npm install`/`cargo build`/a `.git` fetch from turning into tens of
/// thousands of debounced events on every keystroke-adjacent moment — those
/// directories are never shown in the tree unless explicitly expanded, and
/// nothing in the app indexes into them either (search and Quick Open skip
/// them the same way), so their churn is pure overhead today.
///
/// This only filters what the *debouncer* does with an event, not whether
/// the OS-level watch exists in the first place — `notify`'s recursive mode
/// still places an inotify/FSEvents/ReadDirectoryChangesW watch on every
/// subdirectory, `node_modules` included. On very large trees that can
/// still hit a platform's watch-descriptor ceiling; avoiding that requires
/// hand-rolling non-recursive, dynamically-added-and-removed per-directory
/// watches instead of `RecursiveMode::Recursive`, which is a much bigger
/// (and riskier) change than this pass makes — worth a dedicated follow-up
/// if it turns out to matter in practice, but the filtering here already
/// removes the actual CPU/memory/IPC cost for the common case.
fn is_ignored(path: &Path) -> bool {
    path.components().any(|c| {
        if let std::path::Component::Normal(name) = c {
            if let Some(name) = name.to_str() {
                return ALWAYS_SKIP_DIRS.contains(&name);
            }
        }
        false
    })
}

/// Watches `root` and emits `fs:changed` with both the parent directories
/// that might need re-listing and the exact paths notify saw touched (see
/// `FsChangedPayload`). We don't try to fully diff what changed — the
/// frontend already knows which directories it currently has expanded and
/// which files it currently has open, so "these might be stale, act on them
/// if you care" is all either side needs, and it's far simpler than trying
/// to replicate notify's rename-matching logic on the JS side too.
fn start_watcher(app: &AppHandle, state: &State<AppState>, root: &Path) {
    let app_handle = app.clone();
    let debouncer = new_debouncer(
        // Short enough that a single external edit (another editor, an
        // agent, a script) shows up as close to instantly as a debounce can
        // get away with, while still collapsing a burst of rapid writes
        // (a save that touches several files, a fast `git checkout`) into
        // one event instead of one per file.
        Duration::from_millis(200),
        move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let mut dirs: HashSet<String> = HashSet::new();
            let mut paths: HashSet<String> = HashSet::new();
            for e in events.iter().filter(|e| !is_ignored(&e.path)) {
                if let Some(parent) = e.path.parent() {
                    dirs.insert(parent.to_string_lossy().to_string());
                }
                paths.insert(e.path.to_string_lossy().to_string());
            }
            if dirs.is_empty() && paths.is_empty() {
                return;
            }
            let payload = FsChangedPayload {
                dirs: dirs
                    .into_iter()
                    .map(String::into_boxed_str)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                paths: paths
                    .into_iter()
                    .map(String::into_boxed_str)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            };
            let _ = app_handle.emit("fs:changed", payload);
        },
    );

    let mut watcher_slot = state.watcher.lock().unwrap();
    *watcher_slot = None; // drop any previous watcher first

    if let Ok(mut debouncer) = debouncer {
        if debouncer
            .watcher()
            .watch(root, RecursiveMode::Recursive)
            .is_ok()
        {
            *watcher_slot = Some(debouncer);
        }
    }
}

#[tauri::command]
pub fn open_workspace(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<Box<[FsEntry]>, FsError> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(FsError::InvalidName {
            message: "That doesn't look like a folder.".into(),
        });
    }

    let entries = list_dir_sorted(&root)?;
    *state.workspace_root.lock().unwrap() = Some(root.clone());
    start_watcher(&app, &state, &root);

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Create / rename / delete
// ---------------------------------------------------------------------------

/// Splits a possibly-nested `name` like `"lib/date.ts"` or `"lib\\date.ts"`
/// into validated path segments, so `create_entry` can create any missing
/// intermediate folders in one call instead of requiring "New Folder" once
/// per level. Accepts either separator regardless of host OS, and collapses
/// repeated/leading/trailing separators (`"/lib/date.ts"`, `"lib//date.ts"`)
/// rather than erroring on them.
///
/// Every segment goes through the same `validate_name` a single-level
/// create/rename does, which is also what keeps this from ever resolving
/// outside `parent_dir`: a `".."` segment is rejected outright ("That name
/// isn't allowed"), and a bare drive letter like `"C:"` is rejected for
/// containing `:`, so there's no spelling of `name` that can climb above or
/// switch out from under `parent_dir`.
fn split_relative_segments(name: &str) -> Result<Vec<String>, FsError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FsError::InvalidName {
            message: "Name can't be empty.".into(),
        });
    }
    let segments: Vec<String> = trimmed
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if segments.is_empty() {
        return Err(FsError::InvalidName {
            message: "Name can't be empty.".into(),
        });
    }
    for seg in &segments {
        validate_name(seg)?;
    }
    Ok(segments)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntryResult {
    pub entry: FsEntry,
    /// Every intermediate folder along `name`'s path that didn't already
    /// exist, root-to-leaf — e.g. creating `"lib/utils/date.ts"` in an
    /// empty `parent_dir` reports both `lib` and `lib/utils` here. Lets the
    /// frontend expand and populate the tree straight down to the new
    /// entry instead of leaving a freshly-made `lib` collapsed (and, until
    /// expanded, looking empty) for the user to notice and open by hand.
    /// Folders that already existed aren't included — nothing about them
    /// changed, so nothing needs refreshing.
    pub created_dirs: Box<[FsEntry]>,
}

#[tauri::command]
pub fn create_entry(
    parent_dir: String,
    name: String,
    is_dir: bool,
) -> Result<CreateEntryResult, FsError> {
    let segments = split_relative_segments(&name)?;
    let (dir_segments, last_segment) = segments.split_at(segments.len() - 1);

    let mut target = PathBuf::from(&parent_dir);
    let mut created_dirs = Vec::new();
    for seg in dir_segments {
        target.push(seg);
        if target.is_file() {
            return Err(FsError::InvalidName {
                message: format!(
                    "\"{}\" already exists and isn't a folder.",
                    target.file_name().map_or_else(
                        || target.to_string_lossy().into_owned(),
                        |n| n.to_string_lossy().into_owned()
                    )
                )
                .into_boxed_str(),
            });
        }
        if !target.exists() {
            std::fs::create_dir(&target)?;
            created_dirs.push(entry_for(&target)?);
        }
    }

    // `last_segment` is exactly one element — `segments.split_at(len - 1)`
    // guarantees that for any non-empty `segments`.
    target.push(&last_segment[0]);
    if target.exists() {
        return Err(FsError::Conflict {
            paths: Box::from([target.to_string_lossy().into_owned().into_boxed_str()]),
        });
    }
    if is_dir {
        std::fs::create_dir(&target)?;
    } else {
        std::fs::File::create(&target)?;
    }

    Ok(CreateEntryResult {
        entry: entry_for(&target)?,
        created_dirs: created_dirs.into_boxed_slice(),
    })
}

#[tauri::command]
pub fn rename_entry(path: String, new_name: String) -> Result<FsEntry, FsError> {
    validate_name(&new_name)?;
    let old = PathBuf::from(&path);
    let parent = old.parent().ok_or_else(|| FsError::InvalidName {
        message: "Can't rename the workspace root.".into(),
    })?;
    let new_path = parent.join(new_name.trim());
    if new_path != old && new_path.exists() {
        return Err(FsError::Conflict {
            paths: Box::from([new_path.to_string_lossy().into_owned().into_boxed_str()]),
        });
    }
    std::fs::rename(&old, &new_path)?;
    entry_for(&new_path)
}

#[tauri::command]
pub fn delete_entries(paths: Vec<String>) -> Result<(), FsError> {
    trash::delete_all(&paths).map_err(|e| FsError::Io {
        message: e.to_string().into_boxed_str(),
    })
}

// ---------------------------------------------------------------------------
// Copy / move
// ---------------------------------------------------------------------------

/// Counts files + directories under `path` (inclusive) so progress events
/// can report "done / total" instead of an indeterminate spinner.
fn count_entries(path: &Path) -> usize {
    if path.is_dir() {
        let mut n = 1;
        if let Ok(rd) = std::fs::read_dir(path) {
            for entry in rd.filter_map(|e| e.ok()) {
                n += count_entries(&entry.path());
            }
        }
        n
    } else {
        1
    }
}

fn check_conflicts(sources: &[PathBuf], dest_dir: &Path) -> Box<[Box<str>]> {
    sources
        .iter()
        .filter_map(|src| {
            let name = src.file_name()?;
            let target = dest_dir.join(name);
            target
                .exists()
                .then(|| target.to_string_lossy().into_owned().into_boxed_str())
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn guard_against_self_nesting(
    sources: &[PathBuf],
    dest_dir: &Path,
    verb: &str,
) -> Result<(), FsError> {
    for src in sources {
        if src.is_dir() && dest_dir.starts_with(src) {
            return Err(FsError::InvalidName {
                message: format!("Can't {verb} \"{}\" into itself.", src.display())
                    .into_boxed_str(),
            });
        }
    }
    Ok(())
}

fn copy_recursive(
    src: &Path,
    dest: &Path,
    app: &AppHandle,
    done: &mut usize,
    total: usize,
) -> Result<(), FsError> {
    if src.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(
                &entry.path(),
                &dest.join(entry.file_name()),
                app,
                done,
                total,
            )?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest)?;
    }
    *done += 1;
    // Emit roughly every 32 items (plus always the final one) so a huge
    // folder copy doesn't flood IPC with an event per file.
    if *done % 32 == 0 || *done == total {
        let _ = app.emit("fs:copy-progress", CopyProgress { done: *done, total });
    }
    Ok(())
}

#[tauri::command]
pub fn copy_paths(
    app: AppHandle,
    sources: Vec<String>,
    dest_dir: String,
    overwrite: bool,
) -> Result<Box<[FsEntry]>, FsError> {
    let dest_dir = PathBuf::from(dest_dir);
    let sources: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();

    guard_against_self_nesting(&sources, &dest_dir, "copy")?;

    if !overwrite {
        let conflicts = check_conflicts(&sources, &dest_dir);
        if !conflicts.is_empty() {
            return Err(FsError::Conflict { paths: conflicts });
        }
    }

    let total = sources
        .iter()
        .map(|p| count_entries(p))
        .sum::<usize>()
        .max(1);
    let mut done = 0usize;
    let mut results = Vec::with_capacity(sources.len());

    for src in &sources {
        let name = src.file_name().ok_or_else(|| FsError::InvalidName {
            message: "Source has no file name.".into(),
        })?;
        let target = dest_dir.join(name);
        copy_recursive(src, &target, &app, &mut done, total)?;
        results.push(entry_for(&target)?);
    }

    Ok(results.into_boxed_slice())
}

#[tauri::command]
pub fn move_paths(
    app: AppHandle,
    sources: Vec<String>,
    dest_dir: String,
    overwrite: bool,
) -> Result<Box<[FsEntry]>, FsError> {
    let dest_dir = PathBuf::from(dest_dir);
    let sources: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();

    guard_against_self_nesting(&sources, &dest_dir, "move")?;

    if !overwrite {
        let conflicts = check_conflicts(&sources, &dest_dir);
        if !conflicts.is_empty() {
            return Err(FsError::Conflict { paths: conflicts });
        }
    }

    let total = sources
        .iter()
        .map(|p| count_entries(p))
        .sum::<usize>()
        .max(1);
    let mut done = 0usize;
    let mut results = Vec::with_capacity(sources.len());

    for src in &sources {
        let name = src.file_name().ok_or_else(|| FsError::InvalidName {
            message: "Source has no file name.".into(),
        })?;
        let target = dest_dir.join(name);

        if overwrite && target.exists() {
            if target.is_dir() {
                std::fs::remove_dir_all(&target)?;
            } else {
                std::fs::remove_file(&target)?;
            }
        }

        // Fast path: same filesystem, atomic rename. Falls back to
        // copy-then-delete for cross-device moves (e.g. between drives on
        // Windows), which `fs::rename` can't do.
        if std::fs::rename(src, &target).is_err() {
            copy_recursive(src, &target, &app, &mut done, total)?;
            if src.is_dir() {
                std::fs::remove_dir_all(src)?;
            } else {
                std::fs::remove_file(src)?;
            }
        } else {
            done += count_entries(&target);
            let _ = app.emit("fs:copy-progress", CopyProgress { done, total });
        }
        results.push(entry_for(&target)?);
    }

    Ok(results.into_boxed_slice())
}

// ---------------------------------------------------------------------------
// Text file read/write. Read is capped — a proper chunked/streaming path for
// very large files is future work, not needed for a code editor's typical
// file sizes. Non-UTF-8 or oversized files fall back to the binary preview
// below instead.
// ---------------------------------------------------------------------------

const MAX_TEXT_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[tauri::command]
pub fn read_text_file(path: String) -> Result<Box<str>, FsError> {
    let path = PathBuf::from(path);
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > MAX_TEXT_FILE_BYTES {
        return Err(FsError::Unsupported {
            message: "File is too large to open here (over 5 MB).".into(),
        });
    }
    let bytes = std::fs::read(&path)?;
    let text = String::from_utf8(bytes).map_err(|_| FsError::Unsupported {
        message: "This doesn't look like a text file.".into(),
    })?;
    Ok(text.into_boxed_str())
}

#[tauri::command]
pub fn write_text_file(path: String, contents: String) -> Result<(), FsError> {
    std::fs::write(path, contents)?;
    Ok(())
}

/// Capped raw-byte read for the binary hex viewer — this is a preview, not a
/// full read, so even a multi-gigabyte file only ever costs us this much.
const MAX_BINARY_PREVIEW_BYTES: usize = 256 * 1024;

#[tauri::command]
pub fn read_binary_preview(path: String) -> Result<Box<[u8]>, FsError> {
    let mut file = std::fs::File::open(PathBuf::from(path))?;
    let mut buf = vec![0u8; MAX_BINARY_PREVIEW_BYTES];
    let bytes_read = file.read(&mut buf)?;
    buf.truncate(bytes_read);
    Ok(buf.into_boxed_slice())
}

// ---------------------------------------------------------------------------
// Workspace file index for Quick Open. Fuzzy matching itself happens on the
// frontend (see src/lib/utils/fuzzyMatch.ts) — this just needs to hand back
// a reasonably-sized, reasonably-clean list of file paths.
// ---------------------------------------------------------------------------

/// Directories that are always pruned during the walk (not just filtered
/// after the fact) — `filter_entry` skips descending into them entirely, so
/// a huge `node_modules` never actually gets enumerated. `.gitignore` rules
/// are respected on top of this via `ignore::WalkBuilder`'s defaults.
pub(crate) const ALWAYS_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "venv",
];

const MAX_INDEXED_FILES: usize = 20_000;

#[tauri::command]
pub fn list_workspace_files(root: String) -> Result<Box<[Box<str>]>, FsError> {
    let mut out: Vec<Box<str>> = Vec::new();

    let walker = ignore::WalkBuilder::new(&root)
        // Keep dotfiles (`.env`, `.eslintrc`, etc.) — they're common,
        // legitimate project files a project's .gitignore doesn't usually
        // hide them from `git` itself, just from search-by-default tools.
        .hidden(false)
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| !ALWAYS_SKIP_DIRS.contains(&name))
                .unwrap_or(true)
        })
        .build();

    for result in walker {
        if out.len() >= MAX_INDEXED_FILES {
            break;
        }
        let Ok(entry) = result else { continue };
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            out.push(entry.path().to_string_lossy().into_owned().into_boxed_str());
        }
    }

    Ok(out.into_boxed_slice())
}

// ---------------------------------------------------------------------------
// "Reveal in File Explorer" / "Reveal in Finder" — opens the host OS's file
// manager with `path` selected, for either a file or a folder (the file
// tree's context menu offers this for both). Implemented with plain
// `std::process::Command` calls rather than through `tauri-plugin-opener`'s
// `revealItemInDir`: this crate already spawns child processes elsewhere
// (pty.rs's shells, git.rs's `git` invocations), so doing the same here
// keeps this command running under whatever capability setup the rest of
// this file's commands already have, rather than needing a separate plugin
// permission granted on top.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> Result<(), FsError> {
    let path = PathBuf::from(path);

    #[cfg(target_os = "windows")]
    {
        // `/select,` highlights `path` itself inside its parent folder's
        // Explorer window — the same call for a file or a folder. Explorer
        // quite often exits non-zero even after a completely successful
        // reveal (a long-standing quirk of explorer.exe, unrelated to this
        // flag specifically), so a successful *spawn* is treated as success
        // here rather than checking the exit status.
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        // `-R` reveals `path` selected in Finder — again, the same call
        // works for a file or a folder.
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        reveal_on_linux(&path)?;
    }

    Ok(())
}

/// Minimal RFC 3986 percent-encoding for turning a filesystem path into a
/// `file://` URI — only escapes what isn't in the unreserved set, leaving
/// `/` alone as the path separator. Good enough for the one place this is
/// used just below; not a general-purpose URI encoder.
#[cfg(target_os = "linux")]
fn path_to_file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// Linux has no single, universally-supported "reveal and select this item"
/// API the way Windows/macOS do. Tries the `org.freedesktop.FileManager1`
/// D-Bus interface first — implemented by Nautilus, Nemo, and several other
/// file managers, and the same mechanism VS Code itself uses here — to
/// select `path` inside its parent folder. If that isn't available (no
/// `dbus-send` binary, nothing listening on the other end, a sandboxed
/// environment, ...), falls back to just opening the containing folder with
/// `xdg-open`, which is close to universal but can't highlight the specific
/// item within it.
#[cfg(target_os = "linux")]
fn reveal_on_linux(path: &Path) -> Result<(), FsError> {
    let uri = path_to_file_uri(path);
    let dbus_ok = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("array:string:{uri}"),
            "string:",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if dbus_ok {
        return Ok(());
    }

    let parent = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open").arg(parent).spawn()?;
    Ok(())
}
