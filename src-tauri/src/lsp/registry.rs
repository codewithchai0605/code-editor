//! Per-workspace, user-editable registry mapping a language ID to the
//! command that runs its language server. Generalizes Step 1's hardcoded
//! `typescript-language-server --stdio` into "any language, any command" —
//! see AGENT.md §3.
//!
//! The registry lives at `<workspace root>/.codeeditor/lsp-servers.json`,
//! written with built-in defaults the first time a workspace is opened so
//! it's immediately discoverable and editable, exactly like the file
//! didn't exist as a hidden/implicit default. Once it exists, its content
//! is the *sole* source of truth on every later load — defaults are never
//! silently merged back in, so removing an entry keeps it removed.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegistryFile {
    servers: HashMap<String, ServerConfig>,
}

const CONFIG_RELATIVE_PATH: &str = ".codeeditor/lsp-servers.json";

/// Starting point written to a fresh workspace's config file. Deliberately
/// small — three well-known servers as a working example, not an attempt
/// to cover every language up front. Add more by editing the file directly
/// once it exists; no rebuild or restart is needed beyond reopening the
/// workspace (or whatever a future "restart server" command ends up doing
/// — see AGENT.md §5, not built yet).
fn default_servers() -> HashMap<String, ServerConfig> {
    HashMap::from([
        (
            "typescript".to_string(),
            ServerConfig { command: "typescript-language-server".to_string(), args: vec!["--stdio".to_string()] },
        ),
        ("python".to_string(), ServerConfig { command: "pylsp".to_string(), args: vec![] }),
        ("rust".to_string(), ServerConfig { command: "rust-analyzer".to_string(), args: vec![] }),
    ])
}

/// Loads `<root>/.codeeditor/lsp-servers.json`, creating it with
/// `default_servers()` on first use. A malformed file falls back to
/// defaults *for this session only* (without overwriting the broken file —
/// better to surface the parse error than silently clobber whatever the
/// user was mid-edit on).
pub fn load_registry(root: &Path) -> HashMap<String, ServerConfig> {
    let path = root.join(CONFIG_RELATIVE_PATH);

    if let Ok(text) = std::fs::read_to_string(&path) {
        return match serde_json::from_str::<RegistryFile>(&text) {
            Ok(parsed) => parsed.servers,
            Err(e) => {
                eprintln!(
                    "[lsp] Failed to parse {}: {e} — using built-in defaults for this session only \
                     (the file itself is left untouched)",
                    path.display()
                );
                default_servers()
            }
        };
    }

    let defaults = default_servers();
    write_registry_file(&path, &defaults);
    defaults
}

fn write_registry_file(path: &Path, servers: &HashMap<String, ServerConfig>) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let file = RegistryFile { servers: servers.clone() };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, json);
    }
}
