//! Scans an opened workspace folder for **project roots**: directories
//! that contain one of the registry's configured `rootMarkers` for some
//! language — see AGENT.md §3. This is the piece that turns a folder
//! containing both a Node backend and a Flutter app (§9's walkthrough)
//! into two independent `ProjectRoot`s instead of one.
//!
//! Reuses `ignore::WalkBuilder` (same crate `commands::fs::list_workspace_files`
//! already depends on) so this gets `.gitignore`-awareness and the same
//! `ALWAYS_SKIP_DIRS` noise filter for free, rather than re-implementing
//! directory-walking rules a second time.

use crate::commands::fs::ALWAYS_SKIP_DIRS;
use crate::lsp::registry::ServerEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    pub root: PathBuf,
    pub language_id: String,
}

/// How many directory levels deep to search from the workspace root.
/// Generous enough for realistic monorepo layouts (`apps/mobile/...`)
/// without wandering arbitrarily far into a huge tree — and largely
/// moot in practice anyway, since matching a root stops further descent
/// into that branch (see `scan_workspace`'s doc comment below).
const MAX_SCAN_DEPTH: usize = 6;

/// Scans `workspace_root` for every directory matching one of `registry`'s
/// `root_markers` lists, returning one `ProjectRoot` per (directory,
/// language) match — a single directory can legitimately match more than
/// one language (e.g. a Rust crate with an embedded Node-based doc site).
///
/// Detection stops descending into a directory the moment it matches *any*
/// language's markers: a project's own source tree essentially never nests
/// another, independent project root inside it, and the common case (a
/// single-root workspace, matching Step 1/2's original behavior exactly)
/// resolves at depth 0 without walking the rest of the tree at all. This
/// doesn't yet special-case monorepo layouts where that assumption breaks
/// down (e.g. a top-level `package.json` with a `workspaces` field whose
/// member packages are each independently LSP-relevant) — see AGENT.md
/// §3's note on config-overridable detection for the natural extension
/// point once that's needed.
pub fn scan_workspace(
    workspace_root: &Path,
    registry: &HashMap<String, ServerEntry>,
) -> Vec<ProjectRoot> {
    // Sorted for a deterministic result order — HashMap iteration order
    // isn't stable, and a stable order matters here since it's surfaced
    // directly to the user (status bar, server list).
    let mut languages: Vec<(String, Vec<String>)> = registry
        .iter()
        .filter(|(_, entry)| !entry.root_markers().is_empty())
        .map(|(language_id, entry)| (language_id.clone(), entry.root_markers().to_vec()))
        .collect();
    languages.sort_by(|a, b| a.0.cmp(&b.0));

    if languages.is_empty() || !workspace_root.is_dir() {
        return Vec::new();
    }

    let found: Arc<Mutex<Vec<ProjectRoot>>> = Arc::new(Mutex::new(Vec::new()));

    // `ignore::WalkBuilder`'s `filter_entry` hook (used below to check
    // every *descendant* directory) is never invoked for the walk's own
    // root entry — only its children. That would silently miss the most
    // common case of all: a workspace opened *at* its project root (e.g.
    // opening a folder that directly contains `Cargo.toml`). So the root
    // is checked directly, up front, using the same match-and-stop logic
    // `filter_entry` applies to everything below it.
    let mut root_matched = false;
    for (language_id, markers) in &languages {
        if markers
            .iter()
            .any(|marker| workspace_root.join(marker).is_file())
        {
            found.lock().unwrap().push(ProjectRoot {
                root: workspace_root.to_path_buf(),
                language_id: language_id.clone(),
            });
            root_matched = true;
        }
    }
    if root_matched {
        return Arc::try_unwrap(found)
            .expect("no other Arc clone outlives this function")
            .into_inner()
            .expect("single-threaded here; the mutex is never poisoned");
    }

    let found_for_closure = found.clone();

    // `filter_entry`'s closure is called for every entry *before* the
    // walker decides whether to descend into it — exactly the hook needed
    // to both (a) test a directory against every language's markers and
    // (b) prevent further descent the moment it matches, in one pass. Its
    // `F: Fn(...) + Send + Sync + 'static` bound (shared with the parallel
    // walker `WalkBuilder` also supports) is why matches are collected
    // into a `Mutex`, not a plain `RefCell` — this walk itself is
    // sequential, but the bound doesn't know that.
    let walker = ignore::WalkBuilder::new(workspace_root)
        .max_depth(Some(MAX_SCAN_DEPTH))
        .filter_entry(move |entry| {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_dir {
                return true;
            }
            if let Some(name) = entry.file_name().to_str() {
                if ALWAYS_SKIP_DIRS.contains(&name) {
                    return false;
                }
            }

            let dir = entry.path();
            let mut matched = false;
            for (language_id, markers) in &languages {
                if markers.iter().any(|marker| dir.join(marker).is_file()) {
                    found_for_closure.lock().unwrap().push(ProjectRoot {
                        root: dir.to_path_buf(),
                        language_id: language_id.clone(),
                    });
                    matched = true;
                }
            }
            !matched
        })
        .build();

    // The walker does all its work as a side effect of iteration (via
    // `filter_entry` above) — nothing to do with each yielded entry itself.
    for _ in walker {}

    Arc::try_unwrap(found)
        .expect("no other Arc clone outlives the walk above")
        .into_inner()
        .expect("walker is single-threaded here; the mutex is never poisoned")
}
