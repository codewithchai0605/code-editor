//! Per-workspace, user-editable registry mapping a language ID to the
//! command that runs its language server. Generalizes Step 1's hardcoded
//! `typescript-language-server --stdio` into "any language, any command" —
//! see AGENT.md §3 and §4.
//!
//! The registry lives in *this app's own data directory*
//! (`lsp-config.json`, alongside `installer.rs`'s `lsp-tools/`), keyed by
//! workspace — not inside the project itself. Earlier versions of this
//! wrote a `.codeeditor/lsp-servers.json` file into every opened project;
//! that's deliberately gone now (see `migrate_legacy_config_if_present`
//! for what happens to a project that already has one) in favor of
//! managing it entirely from the app's own UI (the status bar's server
//! popover, and a full editor in Settings) — a project folder shouldn't
//! need an app-specific dotfile just because this editor was used on it,
//! and a config file living with the project meant every teammate's
//! local tool preferences (or absence of a tool at all) ended up either
//! committed to the repo or `.gitignore`d by convention nobody agreed on.
//!
//! `rootMarkers` doubles as both "what command to run" (this file) and
//! "where to run it" — `project_scanner.rs` reads the very same
//! `root_markers` list to find project roots, so a user who adds a new
//! language here automatically gets multi-root detection for it too,
//! with no second place to configure.
//!
//! Most languages resolve to exactly one command ([`ServerEntry::Single`]).
//! A language can instead offer several interchangeable **variants** —
//! `typescript` is the motivating case: TypeScript 7 ships a native,
//! Go-based compiler/language-service (`tsgo`/`tsc --lsp`) alongside the
//! classic Node-based `typescript-language-server`, and *which one
//! actually works* depends on which TypeScript a given project has
//! installed (see `typescript_version.rs`). [`resolve_server_config`] is
//! where a `Multi` entry's several possible commands get reconciled down
//! to the one to actually launch for a given project root, in this order:
//! 1. A root-specific override (`rootOverrides` below) — "use this LSP
//!    for this folder specifically".
//! 2. The entry's `preferVariant` — "always use this LSP for every
//!    `<language>` project in this workspace, regardless of version".
//! 3. Auto-detection, where one exists for this language.
//! 4. The entry's static `default` variant, as a final fallback.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// How to auto-install a server that isn't found on PATH — mirroring what
/// VS Code's per-language extensions do invisibly (rust-analyzer's
/// extension downloads a binary on first activation; most others `npm
/// install`/`pip install` a wrapped tool under the hood) rather than
/// requiring the user to run an install command themselves first. See
/// `installer.rs` for the actual install logic each variant runs.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InstallStrategy {
    /// `npm install --prefix <app-managed dir> <packages>` — isolated per
    /// server, never global, so this never fights with (or needs) the
    /// user's own global npm packages. Needs `npm` (i.e. Node.js) on
    /// PATH; nothing here can bootstrap Node itself.
    Npm { packages: Vec<String> },
    /// Creates an isolated virtualenv under the app-managed dir and `pip
    /// install`s into *that*, rather than the user's global/user Python
    /// packages — same isolation rationale as `Npm`. Needs `python3` (or
    /// `python`) on PATH.
    Pip { packages: Vec<String> },
    /// `go install <packages>` with `GOBIN` pointed at the app-managed
    /// dir, so the binary lands there instead of `$GOPATH/bin` (which
    /// may not even be on PATH). Needs `go` on PATH.
    Go { packages: Vec<String> },
    /// rust-analyzer publishes standalone prebuilt binaries per-platform
    /// directly on its GitHub releases — no package manager needed, just
    /// a download. Specific to rust-analyzer (not a generic "any GitHub
    /// release" strategy) since that's the one consumer today; see
    /// `installer.rs`'s `install_rust_analyzer`.
    RustAnalyzer,
    /// No automatic path exists (or hasn't been built yet) — surfaces
    /// `instructions` to the user instead of pretending this can be
    /// automated. Used today for Dart (the LSP ships with the Dart SDK
    /// itself — there's nothing separate to install, just the SDK) and
    /// C/C++ (`clangd` has no single reliably-cross-platform package
    /// manager story worth committing to yet).
    Manual { instructions: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Marker files (checked relative to a candidate directory) that
    /// identify a project root for this language — see AGENT.md §3's
    /// table. Empty means "never auto-detected", which is a legitimate
    /// choice for a server a user only ever wants to start manually.
    #[serde(default, rename = "rootMarkers")]
    pub root_markers: Vec<String>,
    /// Passed through verbatim as `InitializeParams.initialization_options`
    /// — kept as raw JSON (see AGENT.md's Memory Conventions on why
    /// `lsp-types`-adjacent, server-defined shapes aren't worth modeling
    /// as Rust structs) so any server-specific tuning (e.g. rust-analyzer's
    /// `cargo`/`checkOnSave` knobs) is just JSON a user can drop in without
    /// a rebuild.
    #[serde(
        default,
        rename = "initializationOptions",
        skip_serializing_if = "Option::is_none"
    )]
    pub initialization_options: Option<Value>,
    /// How to auto-install this server if `command` isn't found — `None`
    /// means "no auto-install known", same end result as `Manual` but
    /// without even instructions to show (a user's own hand-written
    /// entry, most likely). See `InstallStrategy` and `installer.rs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install: Option<InstallStrategy>,
    /// Groups this server's install with every other `ServerConfig` that
    /// names the same group, so they share **one** install directory and
    /// **one** `npm install`/`pip install`/etc run instead of each
    /// re-running its own — for a package that legitimately provides
    /// several *different* language servers as separate binaries, like
    /// `vscode-langservers-extracted` (html/css/json all come from one
    /// npm install). `None` (the default, and every entry before this
    /// field existed) keeps today's behavior exactly: one install
    /// directory per `(language_id, command)`, entirely independent of
    /// every other entry. See `installer.rs`'s `tool_dir` for where this
    /// is actually read.
    ///
    /// Only meaningful between entries that use the *same* `install`
    /// strategy and `packages` — this doesn't merge or reconcile
    /// different strategies, it just points them at the same directory,
    /// so a mismatched group (different packages under the same group
    /// name) would just mean whichever installs first "wins" the
    /// directory and the other's install overwrites it. `default_servers`
    /// only ever sets this for entries that are already known to agree.
    #[serde(
        default,
        rename = "installGroup",
        skip_serializing_if = "Option::is_none"
    )]
    pub install_group: Option<String>,
}

/// A language entry with more than one possible server — see the module
/// doc comment. `variants` is keyed by an arbitrary, user-visible name
/// (`"classic"`/`"native"` for the built-in `typescript` entry); nothing
/// about the name is special except that it's what `preferVariant` and
/// `rootOverrides` refer to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiServerConfig {
    pub variants: HashMap<String, ServerConfig>,
    /// Used whenever nothing else picks a variant (no override, no
    /// auto-detection signal for this language) — always a name present
    /// in `variants`, enforced by `default_servers()` for the built-in
    /// entry and simply the safest assumption to make for a user-edited
    /// one (see `resolve_server_config`'s fallback-of-last-resort).
    pub default: String,
    /// "Always use this variant for every project, regardless of what
    /// auto-detection would otherwise pick." Distinct from `default`:
    /// `default` is the fallback when *nothing else* decides, this is a
    /// standing preference that pre-empts auto-detection itself.
    #[serde(
        default,
        rename = "preferVariant",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefer_variant: Option<String>,
}

/// One `servers` entry — either a single fixed command, or several
/// interchangeable ones to choose between (see the module doc comment).
/// `#[serde(untagged)]`: a JSON object is tried against `Single` first
/// (matches whenever it has a `command` key) and falls back to `Multi`
/// only when it doesn't (matches whenever it has `variants`+`default`
/// instead) — the two shapes' required fields are disjoint by
/// construction, so this is never ambiguous.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ServerEntry {
    Single(ServerConfig),
    Multi(MultiServerConfig),
}

impl ServerEntry {
    /// Root markers to use for *detecting* a project root for this
    /// language (see `project_scanner.rs`) — for a `Multi` entry, taken
    /// from its `default` variant, since in every realistic case the
    /// different variants are different ways to talk to the *same*
    /// project (same marker files), not different kinds of project.
    pub fn root_markers(&self) -> &[String] {
        match self {
            ServerEntry::Single(config) => &config.root_markers,
            ServerEntry::Multi(multi) => multi
                .variants
                .get(&multi.default)
                .map(|c| c.root_markers.as_slice())
                .unwrap_or(&[]),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct RegistryFile {
    servers: HashMap<String, ServerEntry>,
    /// Path (relative to the workspace root, forward-slash-separated,
    /// `"."` for the workspace root itself) -> language ID -> variant
    /// name. The per-folder half of variant resolution — see the module
    /// doc comment and `resolve_server_config`.
    #[serde(
        default,
        rename = "rootOverrides",
        skip_serializing_if = "HashMap::is_empty"
    )]
    root_overrides: HashMap<String, HashMap<String, String>>,
}

/// The whole app-data-dir config file's shape: every workspace this app
/// has ever seen, each with its own `RegistryFile`. Keyed by
/// `workspace_key` (a normalized workspace-root path string), not the
/// raw path, so the same workspace reported with different casing/
/// separators still lands on the same entry.
#[derive(Debug, Default, Deserialize, Serialize)]
struct RegistryStore {
    #[serde(default)]
    workspaces: HashMap<String, RegistryFile>,
}

/// The parsed, in-memory form of a workspace's registry entry — what
/// `load_registry` returns and every resolution function in this module
/// works from.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub servers: HashMap<String, ServerEntry>,
    pub root_overrides: HashMap<String, HashMap<String, String>>,
}

/// What a variant picker (the status bar's server popover) needs: every
/// available name, and which one is currently in effect.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantInfo {
    pub variants: Vec<String>,
    pub active: String,
}

/// One row of what a Settings "Language Servers" editor works from — see
/// `list_server_configs`. A `Multi` entry (today, just `typescript`)
/// contributes one row per variant, each independently editable;
/// `variant` is `None` for an ordinary `Single` entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigInfo {
    pub language_id: String,
    pub variant: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub root_markers: Vec<String>,
    /// Whether this entry has an `InstallStrategy` at all — the frontend
    /// doesn't need to know *which*, just whether an "auto-installable"
    /// badge applies (edited via Settings, this is preserved from
    /// whatever it already was; nothing in the simple form can set an
    /// install strategy from scratch — see `upsert_server_config`).
    pub has_install_strategy: bool,
}

const CONFIG_FILE_NAME: &str = "lsp-config.json";
/// Where this used to live, in every individual project — read once per
/// workspace for a one-time migration (`migrate_legacy_config_if_present`)
/// and then removed from the project entirely.
const LEGACY_CONFIG_RELATIVE_PATH: &str = ".codeeditor/lsp-servers.json";

fn config_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    Ok(dir.join(CONFIG_FILE_NAME))
}

/// The key a workspace is stored under — its root path, forward-slash-
/// normalized and (on Windows, where paths are case-insensitive at the
/// filesystem level but Rust string comparison isn't) lowercased, so the
/// same workspace reported with different casing or separators still
/// round-trips to the same entry. Mirrors `manager.rs`'s `paths_equal` —
/// same underlying reason.
fn workspace_key(workspace_root: &Path) -> String {
    let normalized = workspace_root.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn read_store(app: &AppHandle) -> RegistryStore {
    let Ok(path) = config_store_path(app) else {
        return RegistryStore::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return RegistryStore::default();
    };
    match serde_json::from_str(&text) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("[lsp] Failed to parse {}: {e} — using built-in defaults for this session only (the file itself is left untouched)", path.display());
            RegistryStore::default()
        }
    }
}

fn write_store(app: &AppHandle, store: &RegistryStore) -> Result<(), String> {
    let path = config_store_path(app)?;
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// One-time migration: if `workspace_root` has an old project-local
/// `.codeeditor/lsp-servers.json` and nothing yet in the app's own store,
/// copies its content over and deletes the old file (and its
/// `.codeeditor` directory, if that's now empty) — a project that
/// predates the move to app-managed config gets cleaned up automatically
/// the first time it's reopened, rather than left with a stale file
/// nothing reads anymore.
fn migrate_legacy_config_if_present(
    app: &AppHandle,
    workspace_root: &Path,
    store: &mut RegistryStore,
) {
    let key = workspace_key(workspace_root);
    if store.workspaces.contains_key(&key) {
        return;
    }
    let legacy_path = workspace_root.join(LEGACY_CONFIG_RELATIVE_PATH);
    let Ok(text) = std::fs::read_to_string(&legacy_path) else {
        return;
    };
    let Ok(parsed) = serde_json::from_str::<RegistryFile>(&text) else {
        return;
    };

    store.workspaces.insert(key, parsed);
    if write_store(app, store).is_ok() {
        let _ = std::fs::remove_file(&legacy_path);
        if let Some(parent) = legacy_path.parent() {
            let _ = std::fs::remove_dir(parent); // silently does nothing unless it's now empty
        }
        eprintln!(
            "[lsp] Migrated {} into the app's own settings — language server config for this \
             project now lives outside the project folder (manage it from the status bar or Settings).",
            legacy_path.display()
        );
    }
}

/// One-time repair for a workspace whose `lsp-config.json` entry predates
/// `supportsHoverVerbosity` support (see `default_servers()`'s "classic"
/// TypeScript variant, below) — without this, a workspace that was
/// already opened once before that field existed keeps its old,
/// persisted `initializationOptions: null` forever, since
/// `default_servers()` only ever runs the *first* time a workspace is
/// seen (see `load_registry`). Only touches the field when it's still
/// completely unset and the command is still the stock
/// `typescript-language-server`, so a user's own deliberate
/// `initializationOptions` for this variant (added by hand, or a
/// `command` pointed at something else entirely) is left untouched.
fn backfill_hover_verbosity_support(servers: &mut HashMap<String, ServerEntry>) -> bool {
    let Some(ServerEntry::Multi(multi)) = servers.get_mut("typescript") else {
        return false;
    };
    let Some(classic) = multi.variants.get_mut("classic") else {
        return false;
    };
    if classic.command != "typescript-language-server" || classic.initialization_options.is_some()
    {
        return false;
    }
    classic.initialization_options = Some(serde_json::json!({ "supportsHoverVerbosity": true }));
    true
}

/// Loads (and, the very first time a workspace is seen, seeds and
/// persists) this workspace's registry entry from the app's own config
/// store — see the module doc comment for why that's a central,
/// app-managed file rather than one written into the project.
pub fn load_registry(app: &AppHandle, workspace_root: &Path) -> Registry {
    let mut store = read_store(app);
    migrate_legacy_config_if_present(app, workspace_root, &mut store);

    let key = workspace_key(workspace_root);
    if let Some(mut file) = store.workspaces.remove(&key) {
        if backfill_hover_verbosity_support(&mut file.servers) {
            store.workspaces.insert(key, file.clone());
            let _ = write_store(app, &store);
        }
        return Registry {
            servers: file.servers,
            root_overrides: file.root_overrides,
        };
    }

    // First time this workspace has been seen at all: seed it with the
    // defaults and persist that immediately, so it shows up in Settings
    // right away rather than only materializing on first edit.
    let servers = default_servers();
    store.workspaces.insert(
        key,
        RegistryFile {
            servers: servers.clone(),
            root_overrides: HashMap::new(),
        },
    );
    let _ = write_store(app, &store);
    Registry {
        servers,
        root_overrides: HashMap::new(),
    }
}

/// Starting point written to a fresh workspace's config file. Seven
/// well-known languages spanning AGENT.md §3's full marker-file table —
/// broad, pluggable coverage out of the box, not an exhaustive list. Add
/// more by editing the file directly once it exists; no rebuild is
/// needed, just an `lsp_detect_roots` re-scan (triggered automatically
/// next time the workspace is opened, or manually via "LSP: Restart
/// Server").
fn default_servers() -> HashMap<String, ServerEntry> {
    HashMap::from([
        (
            "typescript".to_string(),
            ServerEntry::Multi(MultiServerConfig {
                variants: HashMap::from([
                    (
                        "classic".to_string(),
                        ServerConfig {
                            command: "typescript-language-server".to_string(),
                            args: vec!["--stdio".to_string()],
                            root_markers: vec!["package.json".to_string(), "tsconfig.json".to_string()],
                            // `supportsHoverVerbosity` is a required opt-in,
                            // not just a capability the server advertises on
                            // its own: typescript-language-server keeps its
                            // internal `hoverVerbositySupport` flag false
                            // until the client explicitly sets this in
                            // `initializationOptions`, and while it's false
                            // the server silently drops any `verbosityLevel`
                            // sent on a hover request and never adds
                            // `canIncreaseVerbosityLevel`/
                            // `canDecreaseVerbosityLevel` to the response —
                            // so the "+`/`-`" expand-hover control
                            // (lsp/hover.ts) would stay hidden forever
                            // without this, even though the per-request
                            // wiring is otherwise correct. Requires
                            // TypeScript 5.9+ and typescript-language-server
                            // >=5.3.0; harmless (silently ignored) on an
                            // older pair of either, since a hover response
                            // simply won't include the verbosity fields and
                            // lsp/hover.ts only shows the control when they
                            // do. See typescript-language-server's own
                            // docs/configuration.md, "supportsHoverVerbosity
                            // option".
                            initialization_options: Some(serde_json::json!({
                                "supportsHoverVerbosity": true,
                            })),
                            install: Some(InstallStrategy::Npm {
                                packages: vec!["typescript-language-server".to_string(), "typescript".to_string()],
                            }),
                            install_group: None,
                        },
                    ),
                    (
                        // TypeScript 7's native, Go-based compiler/language
                        // service. Early preview builds shipped this under
                        // the package `@typescript/native-preview` with a
                        // separate `tsgo` binary; as of TS 7's actual
                        // release the native compiler *is* `tsc` — this
                        // entry runs whichever `tsc` is resolved first on
                        // PATH (a project's own `node_modules/.bin/tsc` if
                        // present, else whatever's globally installed), the
                        // same way every other variant/language here does.
                        // If you're specifically still on an older preview
                        // that only ships a separate `tsgo` binary, change
                        // `command` below back to `"tsgo"`. See
                        // `typescript_version.rs` for how a project gets
                        // auto-routed to this variant in the first place,
                        // and its module doc comment for a known gap: this
                        // preview doesn't yet push diagnostics the way the
                        // classic server does, so expect working
                        // hover/completion/definition/etc. but not yet live
                        // squiggles.
                        "native".to_string(),
                        ServerConfig {
                            command: "tsc".to_string(),
                            args: vec!["--lsp".to_string(), "--stdio".to_string()],
                            root_markers: vec!["package.json".to_string(), "tsconfig.json".to_string()],
                            initialization_options: None,
                            // Pinned to ^7 deliberately: an un-pinned
                            // `npm install typescript` here could resolve
                            // to a <7 release, which has no `--lsp` mode
                            // at all (that's the *point* of this variant).
                            install: Some(InstallStrategy::Npm { packages: vec!["typescript@^7".to_string()] }),
                            install_group: None,
                        },
                    ),
                ]),
                default: "classic".to_string(),
                prefer_variant: None,
            }),
        ),
        (
            "python".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "pylsp".to_string(),
                args: vec![],
                root_markers: vec!["pyproject.toml".to_string(), "requirements.txt".to_string()],
                initialization_options: None,
                install: Some(InstallStrategy::Pip { packages: vec!["python-lsp-server".to_string()] }),
                install_group: None,
            }),
        ),
        (
            "rust".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                root_markers: vec!["Cargo.toml".to_string()],
                initialization_options: None,
                install: Some(InstallStrategy::RustAnalyzer),
                install_group: None,
            }),
        ),
        (
            "dart".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "dart".to_string(),
                args: vec!["language-server".to_string(), "--protocol=lsp".to_string()],
                root_markers: vec!["pubspec.yaml".to_string()],
                initialization_options: None,
                // The LSP ships as part of the Dart SDK itself (`dart
                // language-server` is a subcommand, not a separate tool)
                // — there's nothing to install *beyond* the SDK, and
                // auto-installing an entire SDK is out of scope here the
                // same way auto-installing Node or Python would be.
                install: Some(InstallStrategy::Manual {
                    instructions: "Install the Dart SDK, which includes its language server: https://dart.dev/get-dart".to_string(),
                }),
                install_group: None,
            }),
        ),
        (
            "go".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "gopls".to_string(),
                args: vec![],
                root_markers: vec!["go.mod".to_string()],
                initialization_options: None,
                install: Some(InstallStrategy::Go { packages: vec!["golang.org/x/tools/gopls@latest".to_string()] }),
                install_group: None,
            }),
        ),
        (
            "cpp".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "clangd".to_string(),
                args: vec![],
                root_markers: vec!["CMakeLists.txt".to_string(), "compile_commands.json".to_string()],
                initialization_options: None,
                // No single package manager reliably covers every
                // platform for a standalone `clangd` binary the way
                // rust-analyzer's own releases do — installing LLVM
                // itself is the common path (`apt install clangd`, `brew
                // install llvm`, or the LLVM releases page on Windows).
                install: Some(InstallStrategy::Manual {
                    instructions: "Install clangd (usually via LLVM): apt install clangd (Linux), brew install llvm (macOS), or https://github.com/llvm/llvm-project/releases (Windows)".to_string(),
                }),
                install_group: None,
            }),
        ),
        (
            "php".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "intelephense".to_string(),
                args: vec!["--stdio".to_string()],
                root_markers: vec!["composer.json".to_string()],
                initialization_options: None,
                install: Some(InstallStrategy::Npm { packages: vec!["intelephense".to_string()] }),
                install_group: None,
            }),
        ),
        // The three below all come from one npm package
        // (`vscode-langservers-extracted`, the actively-maintained fork
        // of VS Code's own bundled html/css/json language servers — see
        // <https://www.npmjs.com/package/vscode-langservers-extracted>),
        // so they share one `installGroup`: opening the first of an
        // html/css/json file triggers one `npm install`, and the other
        // two find their binary already sitting in that same shared
        // install directory the next time *they're* opened, rather than
        // each independently re-installing the same package. Deliberately
        // empty `rootMarkers` for all three — unlike TypeScript or a
        // Cargo/Go project, there's no manifest file a plain static
        // html/css/js site is expected to have at all, and these servers
        // don't need a "project" in that sense anyway: each document is
        // handled independently. With no marker to detect, every file of
        // these languages simply resolves to the workspace root (see
        // `lsp/manager.svelte.ts`'s `resolveRootFor` — that's the
        // intended fallback, not a gap).
        (
            "html".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "vscode-html-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                root_markers: vec![],
                initialization_options: None,
                install: Some(InstallStrategy::Npm {
                    packages: vec!["vscode-langservers-extracted".to_string()],
                }),
                install_group: Some("vscode-langservers-extracted".to_string()),
            }),
        ),
        (
            "css".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "vscode-css-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                root_markers: vec![],
                initialization_options: None,
                install: Some(InstallStrategy::Npm {
                    packages: vec!["vscode-langservers-extracted".to_string()],
                }),
                install_group: Some("vscode-langservers-extracted".to_string()),
            }),
        ),
        (
            "json".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "vscode-json-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                root_markers: vec![],
                initialization_options: None,
                install: Some(InstallStrategy::Npm {
                    packages: vec!["vscode-langservers-extracted".to_string()],
                }),
                install_group: Some("vscode-langservers-extracted".to_string()),
            }),
        ),
    ])
}

/// `project_root` relative to `workspace_root`, forward-slash-separated
/// regardless of platform (so the config file is portable), `"."` for the
/// workspace root itself — the key `rootOverrides` is keyed by. `None`
/// only if `project_root` somehow isn't under `workspace_root` at all,
/// which shouldn't happen given how callers obtain `project_root` (either
/// `project_scanner::scan_workspace`'s own output, or the workspace root
/// passed straight through for a single-root project).
fn relative_root_key(workspace_root: &Path, project_root: &Path) -> Option<String> {
    let relative = project_root.strip_prefix(workspace_root).ok()?;
    if relative.as_os_str().is_empty() {
        Some(".".to_string())
    } else {
        Some(relative.to_string_lossy().replace('\\', "/"))
    }
}

/// Auto-detection dispatch for languages that have one — currently just
/// `typescript` (see `typescript_version.rs`). Returns `None` for every
/// other language, which falls through to the entry's static `default` in
/// `resolve_server_config`.
fn auto_detect_variant(language_id: &str, project_root: &Path) -> Option<String> {
    match language_id {
        "typescript" => Some(
            match super::typescript_version::detect_flavor(project_root) {
                super::typescript_version::TypescriptFlavor::Native => "native",
                super::typescript_version::TypescriptFlavor::Classic => "classic",
            }
            .to_string(),
        ),
        _ => None,
    }
}

/// Picks which variant of `language_id` is currently in effect at
/// `project_root`, without needing every caller to re-implement the
/// override precedence — shared by `resolve_server_config` (which needs
/// the winning name) and `list_variants` (which needs it too, to tell the
/// frontend which entry to show as selected).
fn resolve_variant_name(
    registry: &Registry,
    multi: &MultiServerConfig,
    language_id: &str,
    workspace_root: &Path,
    project_root: &Path,
) -> String {
    let override_choice = relative_root_key(workspace_root, project_root)
        .and_then(|key| registry.root_overrides.get(&key))
        .and_then(|langs| langs.get(language_id))
        .cloned();

    override_choice
        .or_else(|| multi.prefer_variant.clone())
        .or_else(|| auto_detect_variant(language_id, project_root))
        .unwrap_or_else(|| multi.default.clone())
}

/// Resolves the effective `ServerConfig` to actually launch for
/// `language_id` at `project_root` — the single place a `Multi` entry's
/// several possible commands get reconciled down to one. See the module
/// doc comment for the precedence order. A `Single`-shape entry (every
/// language except, by default, TypeScript) skips all of this and is
/// returned as-is.
pub fn resolve_server_config(
    app: &AppHandle,
    workspace_root: &Path,
    language_id: &str,
    project_root: &Path,
) -> Option<ServerConfig> {
    let registry = load_registry(app, workspace_root);
    match registry.servers.get(language_id)? {
        ServerEntry::Single(config) => Some(config.clone()),
        ServerEntry::Multi(multi) => {
            let chosen =
                resolve_variant_name(&registry, multi, language_id, workspace_root, project_root);
            multi
                .variants
                .get(&chosen)
                .or_else(|| multi.variants.get(&multi.default))
                .cloned()
        }
    }
}

/// `None` if `language_id` isn't a `Multi` entry at all (nothing to pick
/// between) — the frontend's cue not to show a variant picker for it.
pub fn list_variants(
    app: &AppHandle,
    workspace_root: &Path,
    language_id: &str,
    project_root: &Path,
) -> Option<VariantInfo> {
    let registry = load_registry(app, workspace_root);
    let ServerEntry::Multi(multi) = registry.servers.get(language_id)? else {
        return None;
    };

    let mut variants: Vec<String> = multi.variants.keys().cloned().collect();
    variants.sort();
    let active = resolve_variant_name(&registry, multi, language_id, workspace_root, project_root);

    Some(VariantInfo { variants, active })
}

/// Writes a per-folder LSP variant override into the workspace's config —
/// "use this LSP for this folder specifically" (the status bar's server
/// popover is the UI surface for it). Round-trips through the *whole*
/// store, not just the touched key, so nothing else gets clobbered.
/// Applying the new choice to an already-running server is the caller's
/// job (an `lsp_restart` after this succeeds) — this only ever changes
/// what a *future* resolution picks.
pub fn set_root_override(
    app: &AppHandle,
    workspace_root: &Path,
    project_root: &Path,
    language_id: &str,
    variant: &str,
) -> Result<(), String> {
    let mut store = read_store(app);
    let key = workspace_key(workspace_root);
    let entry = store.workspaces.entry(key).or_insert_with(|| RegistryFile {
        servers: default_servers(),
        root_overrides: HashMap::new(),
    });

    let Some(relative_key) = relative_root_key(workspace_root, project_root) else {
        return Err("That folder isn't inside the current workspace".to_string());
    };
    entry
        .root_overrides
        .entry(relative_key)
        .or_default()
        .insert(language_id.to_string(), variant.to_string());

    write_store(app, &store)
}

/// Flattens a workspace's registry into one row per editable server —
/// what a Settings "Language Servers" page lists and edits. A `Multi`
/// entry contributes one row per variant rather than trying to cram
/// several commands into one row.
pub fn list_server_configs(app: &AppHandle, workspace_root: &Path) -> Vec<ServerConfigInfo> {
    let registry = load_registry(app, workspace_root);
    let mut language_ids: Vec<&String> = registry.servers.keys().collect();
    language_ids.sort();

    let mut out = Vec::new();
    for language_id in language_ids {
        match &registry.servers[language_id] {
            ServerEntry::Single(config) => out.push(ServerConfigInfo {
                language_id: language_id.clone(),
                variant: None,
                command: config.command.clone(),
                args: config.args.clone(),
                root_markers: config.root_markers.clone(),
                has_install_strategy: config.install.is_some(),
            }),
            ServerEntry::Multi(multi) => {
                let mut variant_names: Vec<&String> = multi.variants.keys().collect();
                variant_names.sort();
                for variant_name in variant_names {
                    let config = &multi.variants[variant_name];
                    out.push(ServerConfigInfo {
                        language_id: language_id.clone(),
                        variant: Some(variant_name.clone()),
                        command: config.command.clone(),
                        args: config.args.clone(),
                        root_markers: config.root_markers.clone(),
                        has_install_strategy: config.install.is_some(),
                    });
                }
            }
        }
    }
    out
}

/// Creates or updates one server entry (or, for a `Multi` language, one
/// of its variants) from Settings — the UI counterpart to hand-editing
/// the old project-local JSON file this replaced. `variant: None` always
/// means "the whole entry is a single fixed command"; passing `Some` for
/// a language that doesn't have a `Multi` entry yet starts one (with the
/// new variant as its `default`), rather than requiring the UI to already
/// know the entry's current shape.
///
/// Preserves whatever this exact `(language_id, variant)` already had for
/// `install`/`initializationOptions`/`installGroup` — fields this simple
/// command/args/root-markers form doesn't expose — so tweaking, say,
/// rust-analyzer's args doesn't silently drop its auto-install capability,
/// and editing the CSS server's command doesn't silently un-share its
/// install directory from the HTML/JSON servers it came bundled with.
pub fn upsert_server_config(
    app: &AppHandle,
    workspace_root: &Path,
    language_id: &str,
    variant: Option<&str>,
    command: String,
    args: Vec<String>,
    root_markers: Vec<String>,
) -> Result<(), String> {
    let mut store = read_store(app);
    let key = workspace_key(workspace_root);
    let entry = store.workspaces.entry(key).or_insert_with(|| RegistryFile {
        servers: default_servers(),
        root_overrides: HashMap::new(),
    });

    let existing = match (variant, entry.servers.get(language_id)) {
        (None, Some(ServerEntry::Single(config))) => Some(config.clone()),
        (Some(name), Some(ServerEntry::Multi(multi))) => multi.variants.get(name).cloned(),
        _ => None,
    };

    let new_config = ServerConfig {
        command,
        args,
        root_markers,
        initialization_options: existing
            .as_ref()
            .and_then(|c| c.initialization_options.clone()),
        install: existing.as_ref().and_then(|c| c.install.clone()),
        install_group: existing.and_then(|c| c.install_group),
    };

    match variant {
        None => {
            entry
                .servers
                .insert(language_id.to_string(), ServerEntry::Single(new_config));
        }
        Some(variant_name) => match entry.servers.get_mut(language_id) {
            Some(ServerEntry::Multi(multi)) => {
                multi.variants.insert(variant_name.to_string(), new_config);
            }
            _ => {
                let mut variants = HashMap::new();
                variants.insert(variant_name.to_string(), new_config);
                entry.servers.insert(
                    language_id.to_string(),
                    ServerEntry::Multi(MultiServerConfig {
                        variants,
                        default: variant_name.to_string(),
                        prefer_variant: None,
                    }),
                );
            }
        },
    }

    write_store(app, &store)
}

/// Removes a language entirely (every variant, for a `Multi` one) from a
/// workspace's registry — a no-op if it's already absent.
pub fn delete_server_config(
    app: &AppHandle,
    workspace_root: &Path,
    language_id: &str,
) -> Result<(), String> {
    let mut store = read_store(app);
    let key = workspace_key(workspace_root);
    if let Some(entry) = store.workspaces.get_mut(&key) {
        entry.servers.remove(language_id);
    }
    write_store(app, &store)
}

/// Discards every customization for this workspace (both `servers` edits
/// and `rootOverrides`) — the next `load_registry` reseeds it fresh from
/// `default_servers()`. Settings' "Restore Defaults".
pub fn reset_workspace_config(app: &AppHandle, workspace_root: &Path) -> Result<(), String> {
    let mut store = read_store(app);
    store.workspaces.remove(&workspace_key(workspace_root));
    write_store(app, &store)
}
