//! "Open with <App>" on a folder's right-click menu, and the plumbing that
//! makes launching the app *that* way actually open the folder.
//!
//! Three independent pieces live here:
//!
//! 1. Cross-platform: pulling a folder path out of a launch (a CLI argument
//!    on cold start, a `tauri-plugin-single-instance` redirect, or a macOS
//!    `RunEvent::Opened`) and handing it to the frontend exactly once. See
//!    `extract_folder_path`, `file_uri_to_path`, `set_pending_open_path`,
//!    and `take_pending_open_path` — wired up from `lib.rs`.
//! 2. Windows: registry keys under `Directory\shell` and
//!    `Directory\Background\shell`, written to `HKEY_CURRENT_USER` so
//!    neither registering nor unregistering ever needs admin rights.
//! 3. Linux: a `.desktop` file plus a Nautilus script and a Dolphin service
//!    menu, since no single mechanism covers the common file managers (see
//!    the `linux` module's own doc comment for why three).
//!
//! macOS has no runtime piece at all: folder support there is a build-time
//! `CFBundleDocumentTypes` declaration (see `src-tauri/Info.plist`) that
//! makes the app a candidate in Finder's "Open With" submenu for any
//! folder — there's nothing to toggle from inside a running app, which is
//! also why `context_menu_status` reports `toggleable: false` there.

use crate::state::AppState;
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------------------
// 1. Cross-platform: receiving a folder path from however this launch
//    happened.
// ---------------------------------------------------------------------------

/// Picks the folder path out of a set of CLI-style arguments — used both for
/// this process's own `std::env::args()` on cold start and for the argv
/// `tauri-plugin-single-instance` hands back when a second launch gets
/// redirected here. Skips `argv[0]` (the executable path itself, always
/// present) and returns the first remaining argument that both doesn't look
/// like a flag and actually exists as a directory on disk — the directory
/// check is what keeps a stray positional argument from some other launcher
/// (or a typo) from being treated as a workspace to open.
pub fn extract_folder_path(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .find(|a| !a.starts_with('-') && Path::new(a).is_dir())
        .cloned()
}

/// Converts a `file://` URL (what `RunEvent::Opened` hands us on macOS) into
/// a plain filesystem path, undoing the same percent-encoding fs.rs's
/// `path_to_file_uri` applies for the Linux reveal-in-file-manager fallback.
pub fn file_uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenFolderPayload {
    path: String,
}

/// Records `path` as a folder the OS wants opened and tells whichever
/// frontend is currently running about it. Called from three places in
/// lib.rs — the cold-start argv check, the single-instance callback, and
/// the macOS `RunEvent::Opened` handler — so every way this app can be
/// asked to open a folder funnels through one place.
///
/// Both a stored value *and* a live event, deliberately: the frontend's
/// listener for a live "another launch just happened" event is only
/// registered once the page has loaded, well after the window exists, so a
/// launch that raced ahead of that registration — which includes *every*
/// cold start, and macOS's `Opened` event in particular, which can fire
/// before the page has finished loading — would otherwise be silently
/// missed. The frontend checks the stored value once on startup (see
/// `take_pending_open_path`) to catch exactly that case, in addition to
/// listening live for anything that happens after.
pub fn set_pending_open_path(app: &AppHandle, path: String) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.pending_open_path.lock().unwrap() = Some(path.clone());
    }
    let _ = app.emit("open-folder-request", OpenFolderPayload { path });
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Returns (and clears) the pending folder path, if any — called once by
/// the frontend on startup. See `set_pending_open_path`'s doc comment for
/// why a plain take-once command exists alongside the live event.
#[tauri::command]
pub fn take_pending_open_path(state: State<AppState>) -> Option<String> {
    state.pending_open_path.lock().unwrap().take()
}

// ---------------------------------------------------------------------------
// Unified status type + command dispatch. Each platform module below
// implements `register`/`unregister`/`is_registered`; the commands here just
// pick the right one for the host OS and wrap it in a consistent shape the
// Settings UI can render without needing its own per-OS branching.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuStatus {
    /// `"windows"` | `"macos"` | `"linux"` | anything else this app hasn't
    /// been taught about — kept as a string rather than an enum so a future
    /// target doesn't need a matching frontend type change just to compile.
    pub platform: String,
    /// Whether this app can turn the integration on/off at runtime at all.
    /// `false` only on macOS, where it's a build-time `Info.plist`
    /// declaration with nothing to toggle from inside a running app.
    pub toggleable: bool,
    /// Only meaningful when `toggleable` is true.
    pub registered: bool,
    /// User-facing explanation for the Settings panel — what "registered"
    /// actually means here, or why there's nothing to toggle on this OS.
    pub note: String,
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    }
}

#[tauri::command]
pub fn context_menu_status() -> ContextMenuStatus {
    #[cfg(target_os = "windows")]
    {
        ContextMenuStatus {
            platform: current_platform().to_string(),
            toggleable: true,
            registered: windows::is_registered(),
            note: "Adds \"Open with...\" to a folder's right-click menu and to \
                   the right-click menu inside a folder."
                .to_string(),
        }
    }
    #[cfg(target_os = "linux")]
    {
        ContextMenuStatus {
            platform: current_platform().to_string(),
            toggleable: true,
            registered: linux::is_registered(),
            note: "Adds a folder \"Open with...\" entry where your file \
                   manager supports it: directly on right-click in Dolphin, \
                   under a file manager's \"Open With\" dialog generally, \
                   and under Nautilus's right-click \"Scripts\" submenu in \
                   GNOME Files (Nautilus doesn't offer arbitrary apps in its \
                   own \"Open With\" menu for folders, so Scripts is the \
                   closest equivalent there)."
                .to_string(),
        }
    }
    #[cfg(target_os = "macos")]
    {
        ContextMenuStatus {
            platform: current_platform().to_string(),
            toggleable: false,
            registered: true,
            note: "Already available whenever this app is installed as a \
                   proper .app bundle: right-click a folder in Finder, then \
                   Open With. This is declared once when the app is built, \
                   so there's nothing to turn on or off here."
                .to_string(),
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        ContextMenuStatus {
            platform: current_platform().to_string(),
            toggleable: false,
            registered: false,
            note: "Not available on this platform.".to_string(),
        }
    }
}

#[tauri::command]
pub fn register_folder_context_menu(app: AppHandle) -> Result<ContextMenuStatus, String> {
    #[cfg(target_os = "windows")]
    {
        windows::register(&app)?;
    }
    #[cfg(target_os = "linux")]
    {
        linux::register(&app)?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        return Err(
            "macOS folder support is set when the app is built, not from inside it.".to_string(),
        );
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = app;
        return Err("Not available on this platform.".to_string());
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Ok(context_menu_status())
}

#[tauri::command]
pub fn unregister_folder_context_menu() -> Result<ContextMenuStatus, String> {
    #[cfg(target_os = "windows")]
    {
        windows::unregister()?;
    }
    #[cfg(target_os = "linux")]
    {
        linux::unregister()?;
    }
    #[cfg(target_os = "macos")]
    {
        return Err(
            "macOS folder support is set when the app is built, not from inside it.".to_string(),
        );
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        return Err("Not available on this platform.".to_string());
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Ok(context_menu_status())
}

// ---------------------------------------------------------------------------
// 2. Windows — HKCU registry keys. Never HKLM/HKCR directly: everything
//    here goes through `HKEY_CURRENT_USER\Software\Classes`, which Explorer
//    reads with the same effect as `HKEY_CLASSES_ROOT` but — unlike writing
//    to the real `HKEY_CLASSES_ROOT` — never needs admin rights, whether
//    this runs from the installer or from a button inside the app.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows {
    use super::{AppHandle, Manager};

    /// Fixed, stable identifier for the registry key path itself — never
    /// shown to the user (that's `build_open_with_label`'s job) and never
    /// derived from the product name, so a rename or relocalization of the
    /// app can't turn a previously-registered key into an orphan under the
    /// old name.
    const KEY_NAME: &str = "CodeEditorOpenFolder";

    fn build_command_line(exe_path: &str, arg_placeholder: &str) -> String {
        format!("\"{exe_path}\" \"{arg_placeholder}\"")
    }

    fn build_icon_value(exe_path: &str) -> String {
        format!("{exe_path},0")
    }

    fn build_open_with_label(display_name: &str) -> String {
        format!("Open with {display_name}")
    }

    fn write_verb(
        hkcu: &winreg::RegKey,
        base: &str,
        label: &str,
        exe_path: &str,
        arg_placeholder: &str,
    ) -> std::io::Result<()> {
        let (verb_key, _) = hkcu.create_subkey(format!("{base}\\shell\\{KEY_NAME}"))?;
        verb_key.set_value("", &label.to_string())?;
        verb_key.set_value("Icon", &build_icon_value(exe_path))?;
        let (cmd_key, _) = verb_key.create_subkey("command")?;
        cmd_key.set_value("", &build_command_line(exe_path, arg_placeholder))?;
        Ok(())
    }

    pub fn register(app: &AppHandle) -> Result<(), String> {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Couldn't find this app's own executable: {e}"))?;
        let exe_path = exe_path.to_string_lossy().into_owned();
        // `package_info().name` reflects `productName` from tauri.conf.json
        // (set at build time by `tauri-build`), not the Cargo crate name —
        // this is what should show up in the menu.
        let display_name = app.package_info().name.clone();
        let label = build_open_with_label(&display_name);

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        // "Directory\shell" fires when right-clicking a folder itself
        // ("%1" — Explorer fills in that folder's path); "Directory\
        // Background\shell" fires when right-clicking empty space inside a
        // folder ("%V" — Explorer fills in the folder being viewed).
        write_verb(
            &hkcu,
            "Software\\Classes\\Directory",
            &label,
            &exe_path,
            "%1",
        )
        .map_err(|e| e.to_string())?;
        write_verb(
            &hkcu,
            "Software\\Classes\\Directory\\Background",
            &label,
            &exe_path,
            "%V",
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn unregister() -> Result<(), String> {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        for base in [
            "Software\\Classes\\Directory",
            "Software\\Classes\\Directory\\Background",
        ] {
            match hkcu.delete_subkey_all(format!("{base}\\shell\\{KEY_NAME}")) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }

    pub fn is_registered() -> bool {
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(format!("Software\\Classes\\Directory\\shell\\{KEY_NAME}"))
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// 3. Linux — no single mechanism covers the common file managers, so this
//    installs three independent things and treats "is any of them present"
//    as the registered/unregistered state:
//
//    - A `.desktop` file with `MimeType=inode/directory;`, which some file
//      managers' own "Open With" dialogs pick up directly.
//    - A Dolphin (KDE) service menu, which gets a real top-level right-click
//      entry — the closest of the three to the Windows/macOS experience.
//    - A Nautilus (GNOME Files) script, because Nautilus deliberately
//      doesn't offer arbitrary installed apps in its own folder "Open With"
//      menu (a long-standing, intentional GNOME design choice) — Scripts,
//      which appear one level down under a right-click "Scripts" submenu,
//      are the standard workaround and need no extra packages installed.
//
//    None of these ever touch `xdg-mime default` — that would make this
//    app the *default* handler for every folder, hijacking normal
//    double-click behavior everywhere, which is not what "add an option to
//    the menu" means.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::{AppHandle, Manager};
    use std::fs;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn data_home() -> Result<PathBuf, String> {
        if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
            if !dir.is_empty() {
                return Ok(PathBuf::from(dir));
            }
        }
        let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
        Ok(PathBuf::from(home).join(".local/share"))
    }

    fn desktop_file_path(data_home: &std::path::Path) -> PathBuf {
        data_home.join("applications/code-editor-open-folder.desktop")
    }

    /// Dolphin (KDE) has looked for service menus in slightly different
    /// places across Frameworks versions — writing to both the KF6 path and
    /// the older KF5 one costs nothing and covers whichever the user's
    /// Plasma version actually reads.
    fn dolphin_service_menu_paths(data_home: &std::path::Path) -> [PathBuf; 2] {
        [
            data_home.join("kio/servicemenus/code-editor-open-folder.desktop"),
            data_home.join("kservices5/ServiceMenus/code-editor-open-folder.desktop"),
        ]
    }

    fn nautilus_script_path(data_home: &std::path::Path) -> PathBuf {
        data_home.join("nautilus/scripts/Open in Code Editor")
    }

    fn write_file(path: &std::path::Path, contents: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }

    fn remove_file_if_present(path: &std::path::Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn register(app: &AppHandle) -> Result<(), String> {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Couldn't find this app's own executable: {e}"))?;
        let exe_path = exe_path.to_string_lossy().into_owned();
        let display_name = app.package_info().name.clone();
        // Convention `tauri-bundler` itself uses for the app's *own*
        // .desktop file's Icon= — reusing it means this inherits whatever
        // icon the .deb/.rpm/AppImage packaging already installed under
        // that name, without this needing to know the actual install path.
        let icon_name = display_name.to_lowercase().replace(' ', "-");
        let data_home = data_home()?;

        let desktop_entry = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Open with {display_name}\n\
             Comment=Open this folder in {display_name}\n\
             Exec=\"{exe_path}\" %f\n\
             Icon={icon_name}\n\
             Terminal=false\n\
             NoDisplay=true\n\
             MimeType=inode/directory;\n\
             Categories=Development;\n"
        );
        write_file(&desktop_file_path(&data_home), &desktop_entry).map_err(|e| e.to_string())?;

        let dolphin_entry = format!(
            "[Desktop Entry]\n\
             Type=Service\n\
             X-KDE-ServiceTypes=KonqPopupMenu/Plugin\n\
             MimeType=inode/directory;\n\
             Actions=openWithCodeEditor\n\
             X-KDE-Priority=TopLevel\n\
             \n\
             [Desktop Action openWithCodeEditor]\n\
             Name=Open with {display_name}\n\
             Icon={icon_name}\n\
             Exec=\"{exe_path}\" %f\n"
        );
        for path in dolphin_service_menu_paths(&data_home) {
            write_file(&path, &dolphin_entry).map_err(|e| e.to_string())?;
        }

        // Nautilus passes the selection via
        // $NAUTILUS_SCRIPT_SELECTED_FILE_PATHS (newline-separated, when at
        // least one item is selected) or, when nothing is selected — e.g.
        // right-clicking empty space inside a folder — the folder itself
        // via $NAUTILUS_SCRIPT_CURRENT_URI (a file:// URI, not a plain
        // path).
        let script = format!(
            "#!/bin/sh\n\
             if [ -n \"$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS\" ]; then\n\
             \ttarget=$(printf '%s\\n' \"$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS\" | head -n 1)\n\
             else\n\
             \ttarget=$(printf '%s' \"$NAUTILUS_SCRIPT_CURRENT_URI\" | sed 's#^file://##')\n\
             fi\n\
             exec \"{exe_path}\" \"$target\"\n"
        );
        let script_path = nautilus_script_path(&data_home);
        write_file(&script_path, &script).map_err(|e| e.to_string())?;
        let mut perms = fs::metadata(&script_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).map_err(|e| e.to_string())?;

        // Best-effort: refreshes the desktop-file cache so a running
        // Nautilus/GNOME session picks up the new `.desktop` file's
        // `MimeType` without a logout. Missing entirely on some minimal
        // distros — not fatal if so, just a possible relogin before the
        // `.desktop` entry (not the Dolphin/Nautilus pieces, which don't
        // depend on this cache) is picked up everywhere.
        let _ = std::process::Command::new("update-desktop-database")
            .arg(data_home.join("applications"))
            .status();

        Ok(())
    }

    pub fn unregister() -> Result<(), String> {
        let data_home = data_home()?;
        remove_file_if_present(&desktop_file_path(&data_home)).map_err(|e| e.to_string())?;
        for path in dolphin_service_menu_paths(&data_home) {
            remove_file_if_present(&path).map_err(|e| e.to_string())?;
        }
        remove_file_if_present(&nautilus_script_path(&data_home)).map_err(|e| e.to_string())?;
        let _ = std::process::Command::new("update-desktop-database")
            .arg(data_home.join("applications"))
            .status();
        Ok(())
    }

    pub fn is_registered() -> bool {
        let Ok(data_home) = data_home() else {
            return false;
        };
        desktop_file_path(&data_home).exists()
            || nautilus_script_path(&data_home).exists()
            || dolphin_service_menu_paths(&data_home)
                .iter()
                .any(|p| p.exists())
    }
}
