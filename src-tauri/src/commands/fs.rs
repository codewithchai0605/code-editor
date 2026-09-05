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

/// Watches `root` and emits `fs:changed` with the set of parent directories
/// that need re-fetching. We don't try to diff exactly what changed — the
/// frontend already knows which directories it currently has expanded, so
/// "this directory might be stale, refetch it if you care" is all it needs,
/// and it's far simpler than trying to replicate notify's rename-matching
/// logic on the JS side too.
fn start_watcher(app: &AppHandle, state: &State<AppState>, root: &Path) {
    let app_handle = app.clone();
    let debouncer = new_debouncer(
        Duration::from_millis(400),
        move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let dirs: HashSet<String> = events
                .iter()
                .filter_map(|e| e.path.parent())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if !dirs.is_empty() {
                let _ = app_handle.emit("fs:changed", dirs.into_iter().collect::<Vec<_>>());
            }
        },
    );

    let mut watcher_slot = state.watcher.lock().unwrap();
    *watcher_slot = None; // drop any previous watcher first

    if let Ok(mut debouncer) = debouncer {
        if debouncer.watcher().watch(root, RecursiveMode::Recursive).is_ok() {
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

#[tauri::command]
pub fn create_entry(parent_dir: String, name: String, is_dir: bool) -> Result<FsEntry, FsError> {
    validate_name(&name)?;
    let target = Path::new(&parent_dir).join(name.trim());
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
    entry_for(&target)
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

fn guard_against_self_nesting(sources: &[PathBuf], dest_dir: &Path, verb: &str) -> Result<(), FsError> {
    for src in sources {
        if src.is_dir() && dest_dir.starts_with(src) {
            return Err(FsError::InvalidName {
                message: format!("Can't {verb} \"{}\" into itself.", src.display()).into_boxed_str(),
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
            copy_recursive(&entry.path(), &dest.join(entry.file_name()), app, done, total)?;
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
        let _ = app.emit(
            "fs:copy-progress",
            CopyProgress {
                done: *done,
                total,
            },
        );
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

    let total = sources.iter().map(|p| count_entries(p)).sum::<usize>().max(1);
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

    let total = sources.iter().map(|p| count_entries(p)).sum::<usize>().max(1);
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
