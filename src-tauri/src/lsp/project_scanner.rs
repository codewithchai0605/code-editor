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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn registry() -> HashMap<String, ServerEntry> {
        use crate::lsp::registry::ServerConfig;
        HashMap::from([
            (
                "typescript".to_string(),
                ServerEntry::Single(ServerConfig {
                    command: "typescript-language-server".into(),
                    args: vec![],
                    root_markers: vec!["package.json".into()],
                    initialization_options: None,
                }),
            ),
            (
                "dart".to_string(),
                ServerEntry::Single(ServerConfig {
                    command: "dart".into(),
                    args: vec![],
                    root_markers: vec!["pubspec.yaml".into()],
                    initialization_options: None,
                }),
            ),
            (
                "rust".to_string(),
                ServerEntry::Single(ServerConfig {
                    command: "rust-analyzer".into(),
                    args: vec![],
                    root_markers: vec!["Cargo.toml".into()],
                    initialization_options: None,
                }),
            ),
        ])
    }

    /// Each test gets its own throwaway directory under the system temp
    /// dir, named after the test plus the current time — good enough
    /// uniqueness without pulling in a `tempfile` dev-dependency for what
    /// is otherwise a very small test suite.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "codeeditor-scan-{label}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn touch(&self, relative: &str) {
            let full = self.0.join(relative);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, "").unwrap();
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn single_root_workspace_matches_at_depth_zero() {
        let dir = TempDir::new("single-root");
        dir.touch("Cargo.toml");
        dir.touch("src/main.rs");

        let roots = scan_workspace(dir.path(), &registry());
        assert_eq!(
            roots,
            vec![ProjectRoot {
                root: dir.path().to_path_buf(),
                language_id: "rust".into()
            }]
        );
    }

    #[test]
    fn multi_root_node_backend_and_flutter_app() {
        let dir = TempDir::new("multi-root");
        dir.touch("backend/package.json");
        dir.touch("backend/src/index.ts");
        dir.touch("app/pubspec.yaml");
        dir.touch("app/lib/main.dart");

        let mut roots = scan_workspace(dir.path(), &registry());
        roots.sort_by(|a, b| a.root.cmp(&b.root));

        assert_eq!(
            roots,
            vec![
                ProjectRoot {
                    root: dir.path().join("app"),
                    language_id: "dart".into()
                },
                ProjectRoot {
                    root: dir.path().join("backend"),
                    language_id: "typescript".into()
                },
            ]
        );
    }

    #[test]
    fn single_directory_can_match_multiple_languages() {
        let dir = TempDir::new("polyglot");
        dir.touch("package.json");
        dir.touch("Cargo.toml");

        let mut roots = scan_workspace(dir.path(), &registry());
        roots.sort_by(|a, b| a.language_id.cmp(&b.language_id));

        assert_eq!(
            roots,
            vec![
                ProjectRoot {
                    root: dir.path().to_path_buf(),
                    language_id: "rust".into()
                },
                ProjectRoot {
                    root: dir.path().to_path_buf(),
                    language_id: "typescript".into()
                },
            ]
        );
    }

    #[test]
    fn does_not_descend_past_a_matched_root() {
        let dir = TempDir::new("no-nested-descent");
        dir.touch("backend/package.json");
        // A stray marker file deep inside the already-detected root should
        // NOT produce a second, spurious root.
        dir.touch("backend/vendor/some-lib/package.json");

        let roots = scan_workspace(dir.path(), &registry());
        assert_eq!(
            roots,
            vec![ProjectRoot {
                root: dir.path().join("backend"),
                language_id: "typescript".into()
            }]
        );
    }

    #[test]
    fn skips_noise_directories() {
        let dir = TempDir::new("skip-noise");
        dir.touch("node_modules/some-dep/package.json");
        dir.touch(".git/package.json");

        let roots = scan_workspace(dir.path(), &registry());
        assert!(roots.is_empty(), "expected no roots, got {roots:?}");
    }

    #[test]
    fn empty_registry_yields_no_roots() {
        let dir = TempDir::new("empty-registry");
        dir.touch("Cargo.toml");
        assert!(scan_workspace(dir.path(), &HashMap::new()).is_empty());
    }
}
