//! Detects which TypeScript compiler/language-service flavor a project is
//! actually using, so the registry's `typescript` entry (see
//! `registry.rs`'s `default_servers`) can pick a *working* default
//! between the classic, Node-based `typescript-language-server` and
//! TypeScript 7's native, Go-based `tsgo`/`tsc --lsp` preview, without the
//! user having to say so explicitly for every project.
//!
//! This isn't just a quality-of-life pick between two servers that both
//! basically work: TS 7's package ships without `lib/tsserver.js` at all
//! (what `typescript-language-server` wraps), so launching the classic
//! server against a TS 7 project doesn't degrade gracefully — it fails to
//! do anything. Getting this detection right is the difference between
//! "LSP support" and "no LSP support" for a TS 7 project, not a
//! nice-to-have.
//!
//! Known limitation worth being upfront about: as of this writing, `tsgo`
//! doesn't push `textDocument/publishDiagnostics` the way
//! `typescript-language-server` does (see
//! <https://github.com/microsoft/typescript-go/issues/2362>), so a
//! project resolved to `Native` gets working hover/completion/definition/
//! etc. but not live diagnostic squiggles yet. Still strictly better than
//! a server that can't start at all.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypescriptFlavor {
    /// TypeScript <= 6.x — `tsserver.js`-based, spoken via
    /// `typescript-language-server`.
    Classic,
    /// TypeScript 7's native port (package `@typescript/native-preview`,
    /// binary `tsgo`). The command name is expected to become plain `tsc`
    /// once the native compiler graduates out of preview — see the
    /// `native` variant's doc comment in `registry.rs`'s
    /// `default_servers` for what that means for the registry entry.
    Native,
}

/// Best-effort detection, in order of how trustworthy the signal is:
/// 1. The actually-installed `typescript` package's version — unambiguous
///    ground truth whenever `node_modules` exists.
/// 2. `lib/tsserver.js`'s presence/absence, if installed but the version
///    string itself didn't parse — TS 7 doesn't ship that file at all
///    *regardless* of exactly which preview build it is, so this alone
///    is enough to tell flavors apart even when (1) can't.
/// 3. `package.json`'s *declared* version range, for a project that
///    hasn't been `npm install`-ed yet (no `node_modules` to inspect).
/// 4. `Classic`, if none of the above yield a signal at all — the long-
///    established, fully-capable server is the safe default; `Native` is
///    still an evolving preview, not something to guess into.
pub fn detect_flavor(project_root: &Path) -> TypescriptFlavor {
    let package_dir = project_root.join("node_modules").join("typescript");

    if let Some(version) = installed_version(&package_dir) {
        if let Some(major) = major_version(&version) {
            return flavor_for_major(major);
        }
    }

    if package_dir.is_dir() {
        return if package_dir.join("lib").join("tsserver.js").is_file() {
            TypescriptFlavor::Classic
        } else {
            TypescriptFlavor::Native
        };
    }

    if let Some(range) = declared_range(project_root) {
        if let Some(major) = minimum_major_from_range(&range) {
            return flavor_for_major(major);
        }
    }

    TypescriptFlavor::Classic
}

fn flavor_for_major(major: u32) -> TypescriptFlavor {
    if major >= 7 {
        TypescriptFlavor::Native
    } else {
        TypescriptFlavor::Classic
    }
}

fn installed_version(package_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(package_dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

fn declared_range(project_root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(project_root.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    for section in ["dependencies", "devDependencies"] {
        if let Some(range) = value
            .get(section)
            .and_then(|deps| deps.get("typescript"))
            .and_then(|v| v.as_str())
        {
            return Some(range.to_string());
        }
    }
    None
}

/// Extracts a major version number from a semver-*ish* string — not a
/// full range solver, just enough for the shapes a `package.json` and an
/// installed package's own `version` field actually use in practice:
/// `"7.0.0"`, `"7.0.0-dev.20260115.1"`, `"^5.4.2"`, `"~5.4.2"`, `"5.x"`,
/// `">=5.0.0"`, bare `"5"`. Anything it can't confidently parse returns
/// `None` rather than guessing, so callers fall through to the next, more
/// conservative signal instead of trusting a bad parse.
fn major_version(version: &str) -> Option<u32> {
    let trimmed = version
        .trim()
        .trim_start_matches(['^', '~', '=', '>', '<', ' ']);
    let first_segment = trimmed.split(['.', '-', '+']).next()?;
    first_segment.parse::<u32>().ok()
}

/// A range can list several space-separated comparators (e.g.
/// `">=5.0.0 <6.0.0"`); the first one carries the version this needs.
fn minimum_major_from_range(range: &str) -> Option<u32> {
    let first = range.split_whitespace().next()?;
    major_version(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "codeeditor-tsver-{label}-{}",
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
        fn write(&self, relative: &str, contents: &str) {
            let full = self.0.join(relative);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, contents).unwrap();
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn installed_classic_version_wins() {
        let dir = TempDir::new("classic");
        dir.write(
            "node_modules/typescript/package.json",
            r#"{"version": "5.6.3"}"#,
        );
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Classic);
    }

    #[test]
    fn installed_native_major_version_wins() {
        let dir = TempDir::new("native-version");
        dir.write(
            "node_modules/typescript/package.json",
            r#"{"version": "7.0.0-dev.20260115.1"}"#,
        );
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Native);
    }

    #[test]
    fn missing_tsserver_implies_native_even_if_version_unparseable() {
        let dir = TempDir::new("no-tsserver");
        dir.write(
            "node_modules/typescript/package.json",
            r#"{"version": "not-a-real-semver"}"#,
        );
        dir.write("node_modules/typescript/lib/tsc.js", "");
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Native);
    }

    #[test]
    fn present_tsserver_implies_classic_even_if_version_unparseable() {
        let dir = TempDir::new("has-tsserver");
        dir.write(
            "node_modules/typescript/package.json",
            r#"{"version": "not-a-real-semver"}"#,
        );
        dir.write("node_modules/typescript/lib/tsserver.js", "");
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Classic);
    }

    #[test]
    fn falls_back_to_declared_range_without_node_modules() {
        let dir = TempDir::new("declared-only");
        dir.write(
            "package.json",
            r#"{"devDependencies": {"typescript": "^7.0.0"}}"#,
        );
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Native);
    }

    #[test]
    fn declared_range_classic() {
        let dir = TempDir::new("declared-classic");
        dir.write(
            "package.json",
            r#"{"devDependencies": {"typescript": "^5.4.2"}}"#,
        );
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Classic);
    }

    #[test]
    fn no_signal_at_all_defaults_to_classic() {
        let dir = TempDir::new("no-signal");
        assert_eq!(detect_flavor(dir.path()), TypescriptFlavor::Classic);
    }
}
