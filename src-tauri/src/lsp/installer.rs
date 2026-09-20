//! Auto-installs a language server when it isn't found on PATH, mirroring
//! what VS Code's per-language *extensions* do invisibly (rust-analyzer's
//! extension downloads its binary on first activation; most others `npm
//! install`/`pip install` a wrapped tool under the hood) rather than
//! requiring the user to type an install command themselves first — see
//! `registry.rs`'s `InstallStrategy` for what's actually installable this
//! way and what isn't.
//!
//! Every install writes into `<app data dir>/lsp-tools/<language_id>/
//! <command>/`, never globally: two different projects wanting different
//! server versions never fight over one shared install, nothing here
//! needs admin/sudo rights, and a broken/partial install can be wiped by
//! deleting one directory rather than hunting through a global npm/pip
//! environment.

use crate::lsp::manager::LogEvent;
use crate::lsp::registry::{InstallStrategy, ServerConfig};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn log(app: &AppHandle, root: &Path, language_id: &str, line: impl Into<String>) {
    let _ = app.emit(
        "lsp:log",
        LogEvent {
            root: root.to_string_lossy().into_owned(),
            language_id: language_id.to_string(),
            line: line.into(),
        },
    );
}

/// One directory per `(language_id, command)` pair — e.g. switching
/// TypeScript's variant between `typescript-language-server` and `tsc`
/// gets two independent installs rather than one fighting the other.
fn tool_dir(app: &AppHandle, language_id: &str, command: &str) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let safe_command: String = command
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(base.join("lsp-tools").join(language_id).join(safe_command))
}

const MARKER_FILE: &str = ".codeeditor-installed-binary";

/// The binary path recorded by a previous successful `install()` for this
/// `(language_id, command)`, if the file it points to still exists —
/// checked by `manager.rs`'s `resolve_program` *before* ever falling back
/// to `which` on PATH, so an app-managed install always wins over (and
/// never conflicts with) anything else the user separately has on PATH.
pub fn installed_binary_path(app: &AppHandle, language_id: &str, command: &str) -> Option<PathBuf> {
    let dir = tool_dir(app, language_id, command).ok()?;
    let recorded = std::fs::read_to_string(dir.join(MARKER_FILE)).ok()?;
    let path = PathBuf::from(recorded.trim());
    path.is_file().then_some(path)
}

fn record_installed_binary(dir: &Path, binary_path: &Path) -> Result<(), String> {
    std::fs::write(
        dir.join(MARKER_FILE),
        binary_path.to_string_lossy().as_bytes(),
    )
    .map_err(|e| e.to_string())
}

/// `true` if `config` has an install strategy that can actually attempt
/// something automatic — the status bar's cue for whether a "not found"
/// server should offer an "Install" button at all (`Manual`, and no
/// strategy configured, both mean "nothing to click").
pub fn is_auto_installable(config: &ServerConfig) -> bool {
    !matches!(config.install, None | Some(InstallStrategy::Manual { .. }))
}

/// Runs the configured install strategy for `config`, streaming progress
/// as `lsp:log` events tagged with `(root, language_id)` — the same event
/// an already-running server's own stderr capture uses, so install
/// progress shows up in the same "LSP Output" panel a user might already
/// have open for it. Returns the path `manager.rs` should launch on
/// success; the caller is responsible for actually starting the server
/// afterward (this only installs it).
pub async fn install(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    config: &ServerConfig,
) -> Result<PathBuf, String> {
    let Some(strategy) = &config.install else {
        return Err(format!(
            "No automatic install is available for '{language_id}' — install '{}' manually and make sure it's on PATH.",
            config.command
        ));
    };

    let dir = tool_dir(app, language_id, &config.command)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;

    log(
        app,
        root,
        language_id,
        format!("Installing {} …", config.command),
    );

    let binary_path = match strategy {
        InstallStrategy::Npm { packages } => {
            install_npm(app, root, language_id, &dir, packages, &config.command).await
        }
        InstallStrategy::Pip { packages } => {
            install_pip(app, root, language_id, &dir, packages, &config.command).await
        }
        InstallStrategy::Go { packages } => {
            install_go(app, root, language_id, &dir, packages, &config.command).await
        }
        InstallStrategy::RustAnalyzer => install_rust_analyzer(app, root, language_id, &dir).await,
        InstallStrategy::Manual { instructions } => Err(instructions.clone()),
    }?;

    record_installed_binary(&dir, &binary_path)?;
    log(
        app,
        root,
        language_id,
        format!("Installed at {}", binary_path.display()),
    );
    Ok(binary_path)
}

/// Spawns `cmd`, streams its stdout+stderr live as `lsp:log` events, and
/// waits for it to exit — the shared plumbing behind every command-based
/// strategy (`Npm`/`Pip`/`Go`) below. Mirrors `LspManager::spawn_instance`'s
/// own stdout/stderr reader-task pattern.
async fn run_streaming(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    mut cmd: Command,
) -> Result<(), String> {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let spawn_reader = |pipe: Option<tokio::process::ChildStdout>,
                        app: &AppHandle,
                        root: &Path,
                        language_id: &str| {
        pipe.map(|out| {
            let app = app.clone();
            let root = root.to_path_buf();
            let language_id = language_id.to_string();
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log(&app, &root, &language_id, line);
                }
            })
        })
    };
    let spawn_err_reader = |pipe: Option<tokio::process::ChildStderr>,
                            app: &AppHandle,
                            root: &Path,
                            language_id: &str| {
        pipe.map(|err| {
            let app = app.clone();
            let root = root.to_path_buf();
            let language_id = language_id.to_string();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log(&app, &root, &language_id, line);
                }
            })
        })
    };

    let stdout_task = spawn_reader(stdout, app, root, language_id);
    let stderr_task = spawn_err_reader(stderr, app, root, language_id);

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if let Some(task) = stdout_task {
        let _ = task.await;
    }
    if let Some(task) = stderr_task {
        let _ = task.await;
    }

    if !status.success() {
        return Err(format!(
            "Command exited with {status} — see the log above for details."
        ));
    }
    Ok(())
}

fn require_on_path(name: &str, install_hint: &str) -> Result<PathBuf, String> {
    which::which(name).map_err(|_| format!("'{name}' wasn't found on PATH. {install_hint}"))
}

async fn install_npm(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    dir: &Path,
    packages: &[String],
    command: &str,
) -> Result<PathBuf, String> {
    let npm = require_on_path(
        "npm",
        "Install Node.js (which includes npm) first: https://nodejs.org",
    )?;

    let mut cmd = Command::new(npm);
    cmd.arg("install")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--prefix")
        .arg(dir)
        .args(packages);
    run_streaming(app, root, language_id, cmd).await?;

    let bin_name = if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_string()
    };
    let binary = dir.join("node_modules").join(".bin").join(&bin_name);
    if !binary.is_file() {
        return Err(format!(
            "npm install succeeded, but {} wasn't created — double check the package name.",
            binary.display()
        ));
    }
    Ok(binary)
}

async fn install_pip(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    dir: &Path,
    packages: &[String],
    command: &str,
) -> Result<PathBuf, String> {
    let python = require_on_path("python3", "Install Python first: https://python.org")
        .or_else(|_| require_on_path("python", "Install Python first: https://python.org"))?;

    // An isolated venv, not `pip install --user`/global — same isolation
    // rationale as npm's `--prefix` above, and avoids `pip`'s well-known
    // trouble exposing console-script entry points reliably when
    // installed with `--target` instead.
    let venv_dir = dir.join("venv");
    let venv_python = if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python3")
    };
    if !venv_python.is_file() {
        let mut cmd = Command::new(&python);
        cmd.arg("-m").arg("venv").arg(&venv_dir);
        run_streaming(app, root, language_id, cmd).await?;
    }

    let mut cmd = Command::new(&venv_python);
    cmd.arg("-m").arg("pip").arg("install").args(packages);
    run_streaming(app, root, language_id, cmd).await?;

    let bin_dir = if cfg!(windows) {
        venv_dir.join("Scripts")
    } else {
        venv_dir.join("bin")
    };
    let bin_name = if cfg!(windows) {
        format!("{command}.exe")
    } else {
        command.to_string()
    };
    let binary = bin_dir.join(&bin_name);
    if !binary.is_file() {
        return Err(format!(
            "pip install succeeded, but {} wasn't created — double check the package name.",
            binary.display()
        ));
    }
    Ok(binary)
}

async fn install_go(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    dir: &Path,
    packages: &[String],
    command: &str,
) -> Result<PathBuf, String> {
    let go = require_on_path("go", "Install Go first: https://go.dev/dl/")?;

    let mut cmd = Command::new(go);
    cmd.arg("install").args(packages);
    // Keeps the binary isolated in our own dir instead of `$GOPATH/bin`
    // (which may not even be on PATH) — `go install` respects `GOBIN` as
    // the install destination.
    cmd.env("GOBIN", dir);
    run_streaming(app, root, language_id, cmd).await?;

    let bin_name = if cfg!(windows) {
        format!("{command}.exe")
    } else {
        command.to_string()
    };
    let binary = dir.join(&bin_name);
    if !binary.is_file() {
        return Err(format!(
            "go install succeeded, but {} wasn't created — double check the package path.",
            binary.display()
        ));
    }
    Ok(binary)
}

/// rust-analyzer's actual release asset naming (verified against its live
/// releases, not just its docs): raw single-file `.gz` for Linux/macOS,
/// `.zip` (containing `rust-analyzer.exe`) for Windows. See
/// <https://rust-analyzer.github.io/book/rust_analyzer_binary.html>.
fn rust_analyzer_asset() -> Result<(&'static str, bool), String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(("rust-analyzer-x86_64-unknown-linux-gnu.gz", false)),
        ("linux", "aarch64") => Ok(("rust-analyzer-aarch64-unknown-linux-gnu.gz", false)),
        ("macos", "x86_64") => Ok(("rust-analyzer-x86_64-apple-darwin.gz", false)),
        ("macos", "aarch64") => Ok(("rust-analyzer-aarch64-apple-darwin.gz", false)),
        ("windows", "x86_64") => Ok(("rust-analyzer-x86_64-pc-windows-msvc.zip", true)),
        ("windows", "aarch64") => Ok(("rust-analyzer-aarch64-pc-windows-msvc.zip", true)),
        (os, arch) => Err(format!(
            "No prebuilt rust-analyzer binary is published for {os}/{arch} — install it manually: \
             https://rust-analyzer.github.io/book/rust_analyzer_binary.html"
        )),
    }
}

async fn install_rust_analyzer(
    app: &AppHandle,
    root: &Path,
    language_id: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    let (asset, is_zip) = rust_analyzer_asset()?;
    // A stable URL that always redirects to the matching asset of
    // whichever release is currently "latest" — no GitHub API call (and
    // its stricter unauthenticated rate limit) needed just to find it.
    let url =
        format!("https://github.com/rust-lang/rust-analyzer/releases/latest/download/{asset}");
    log(app, root, language_id, format!("Downloading {url}"));

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    log(
        app,
        root,
        language_id,
        format!("Downloaded {} bytes, extracting…", bytes.len()),
    );

    let binary_path = dir.join(if is_zip {
        "rust-analyzer.exe"
    } else {
        "rust-analyzer"
    });
    if is_zip {
        let cursor = std::io::Cursor::new(&bytes[..]);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| format!("Bad zip archive: {e}"))?;
        let mut entry = archive
            .by_name("rust-analyzer.exe")
            .map_err(|e| format!("Archive is missing rust-analyzer.exe: {e}"))?;
        let mut out = std::fs::File::create(&binary_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    } else {
        let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("Failed to decompress: {e}"))?;
        std::fs::write(&binary_path, &decompressed).map_err(|e| e.to_string())?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary_path, perms).map_err(|e| e.to_string())?;
    }

    Ok(binary_path)
}
