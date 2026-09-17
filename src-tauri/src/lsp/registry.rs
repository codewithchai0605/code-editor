//! Per-workspace, user-editable registry mapping a language ID to the
//! command that runs its language server. Generalizes Step 1's hardcoded
//! `typescript-language-server --stdio` into "any language, any command" —
//! see AGENT.md §3 and §4.
//!
//! The registry lives at `<workspace root>/.codeeditor/lsp-servers.json`,
//! written with built-in defaults the first time a workspace is opened so
//! it's immediately discoverable and editable, exactly like the file
//! didn't exist as a hidden/implicit default. Once it exists, its content
//! is the *sole* source of truth on every later load — defaults are never
//! silently merged back in, so removing an entry keeps it removed.
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
//! Go-based compiler/language-service (`tsgo`) alongside the classic
//! Node-based `typescript-language-server`, and *which one actually works*
//! depends on which TypeScript a given project has installed (see
//! `typescript_version.rs`). [`resolve_server_config`] is where a
//! `Multi` entry's several possible commands get reconciled down to the
//! one to actually launch for a given project root, in this order:
//! 1. A root-specific override (`rootOverrides` below) — "use this LSP
//!    for this folder specifically".
//! 2. The entry's `preferVariant` — "always use this LSP for every
//!    `<language>` project in this workspace, regardless of version".
//! 3. Auto-detection, where one exists for this language.
//! 4. The entry's static `default` variant, as a final fallback.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

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

/// The parsed, in-memory form of a workspace's registry file — what
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

const CONFIG_RELATIVE_PATH: &str = ".codeeditor/lsp-servers.json";

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
                            root_markers: vec![
                                "package.json".to_string(),
                                "tsconfig.json".to_string(),
                            ],
                            initialization_options: None,
                        },
                    ),
                    (
                        // TypeScript 7's native, Go-based compiler/language
                        // service (package `@typescript/native-preview`,
                        // binary `tsgo`). `tsgo` is expected to be renamed
                        // to plain `tsc` once it graduates out of preview
                        // (TS 7.0 RC+) — if that's what's on your PATH
                        // instead, just change `command` below to `"tsc"`;
                        // everything else about the variant (the `--lsp
                        // --stdio` flags, root markers) stays the same. See
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
                            root_markers: vec![
                                "package.json".to_string(),
                                "tsconfig.json".to_string(),
                            ],
                            initialization_options: None,
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
            }),
        ),
        (
            "rust".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                root_markers: vec!["Cargo.toml".to_string()],
                initialization_options: None,
            }),
        ),
        (
            "dart".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "dart".to_string(),
                args: vec!["language-server".to_string(), "--protocol=lsp".to_string()],
                root_markers: vec!["pubspec.yaml".to_string()],
                initialization_options: None,
            }),
        ),
        (
            "go".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "gopls".to_string(),
                args: vec![],
                root_markers: vec!["go.mod".to_string()],
                initialization_options: None,
            }),
        ),
        (
            "cpp".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "clangd".to_string(),
                args: vec![],
                root_markers: vec![
                    "CMakeLists.txt".to_string(),
                    "compile_commands.json".to_string(),
                ],
                initialization_options: None,
            }),
        ),
        (
            "php".to_string(),
            ServerEntry::Single(ServerConfig {
                command: "intelephense".to_string(),
                args: vec!["--stdio".to_string()],
                root_markers: vec!["composer.json".to_string()],
                initialization_options: None,
            }),
        ),
    ])
}

/// Loads `<root>/.codeeditor/lsp-servers.json`, creating it with
/// `default_servers()` on first use. A malformed file falls back to
/// defaults *for this session only* (without overwriting the broken file —
/// better to surface the parse error than silently clobber whatever the
/// user was mid-edit on).
pub fn load_registry(root: &Path) -> Registry {
    let path = root.join(CONFIG_RELATIVE_PATH);

    if let Ok(text) = std::fs::read_to_string(&path) {
        return match serde_json::from_str::<RegistryFile>(&text) {
            Ok(parsed) => Registry {
                servers: parsed.servers,
                root_overrides: parsed.root_overrides,
            },
            Err(e) => {
                eprintln!(
                    "[lsp] Failed to parse {}: {e} — using built-in defaults for this session only \
                     (the file itself is left untouched)",
                    path.display()
                );
                Registry {
                    servers: default_servers(),
                    root_overrides: HashMap::new(),
                }
            }
        };
    }

    let servers = default_servers();
    write_registry_file(
        &path,
        &RegistryFile {
            servers: servers.clone(),
            root_overrides: HashMap::new(),
        },
    );
    Registry {
        servers,
        root_overrides: HashMap::new(),
    }
}

fn write_registry_file(path: &Path, file: &RegistryFile) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string_pretty(file) {
        let _ = std::fs::write(path, json);
    }
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
    workspace_root: &Path,
    language_id: &str,
    project_root: &Path,
) -> Option<ServerConfig> {
    let registry = load_registry(workspace_root);
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
    workspace_root: &Path,
    language_id: &str,
    project_root: &Path,
) -> Option<VariantInfo> {
    let registry = load_registry(workspace_root);
    let ServerEntry::Multi(multi) = registry.servers.get(language_id)? else {
        return None;
    };

    let mut variants: Vec<String> = multi.variants.keys().cloned().collect();
    variants.sort();
    let active = resolve_variant_name(&registry, multi, language_id, workspace_root, project_root);

    Some(VariantInfo { variants, active })
}

/// Writes a per-folder LSP variant override into the workspace's registry
/// file — "use this LSP for this folder specifically" (AGENT.md's manual-
/// controls surface for it is the status bar's server popover). Round-
/// trips through the *whole* file, not just the touched key, so a user's
/// manual edits to `servers`/other overrides are preserved — consistent
/// with `load_registry`'s "the file is the sole source of truth" contract.
/// Applying the new choice to an already-running server is the caller's
/// job (an `lsp_restart` after this succeeds) — this only ever changes
/// what a *future* resolution picks.
pub fn set_root_override(
    workspace_root: &Path,
    project_root: &Path,
    language_id: &str,
    variant: &str,
) -> Result<(), String> {
    let path = workspace_root.join(CONFIG_RELATIVE_PATH);
    let registry = load_registry(workspace_root);

    let Some(key) = relative_root_key(workspace_root, project_root) else {
        return Err("That folder isn't inside the current workspace".to_string());
    };

    let mut root_overrides = registry.root_overrides;
    root_overrides
        .entry(key)
        .or_default()
        .insert(language_id.to_string(), variant.to_string());

    let file = RegistryFile {
        servers: registry.servers,
        root_overrides,
    };
    write_registry_file(&path, &file);
    Ok(())
}
