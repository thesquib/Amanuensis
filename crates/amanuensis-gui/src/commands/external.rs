//! Opening links in the user's browser, and folders in their file manager.
//!
//! Deliberately not `tauri-plugin-shell`'s `open` (nor its replacement,
//! `tauri-plugin-opener` — they share one implementation): both call
//! `open::that_detached`, which hands the system opener *our* environment. Inside an
//! AppImage that environment is AppImageKit's: `LD_LIBRARY_PATH`, `QT_PLUGIN_PATH`,
//! `GTK_PATH`, `XDG_DATA_DIRS` and friends all have `$APPDIR` entries prepended, pointing
//! at the Ubuntu-22.04 libraries we ship. `xdg-open` is a shell script that runs system
//! binaries — on KDE it delegates to Qt ones via `kde-open`/`kioclient` — and those load
//! the bundled libraries against a much newer host and die. Nothing opens, and because the
//! spawn is detached the error is never seen.
//!
//! See tauri-apps/tauri#6172, #10078, #10617 and tauri-apps/plugins-workspace#2315.

#[cfg(not(windows))]
use std::process::Stdio;

/// What to do with one inherited environment variable before spawning a child.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EnvEdit {
    /// Keep the variable, but with every `$APPDIR` entry stripped out.
    Set(String),
    /// Drop the variable entirely — everything in it came from the bundle.
    Remove,
}

/// Variables the AppImage runtime owns outright. A child that sees these believes it is
/// itself running inside the bundle.
const APPIMAGE_MARKERS: &[&str] = &["APPDIR", "APPIMAGE", "APPIMAGE_GTK_THEME", "ARGV0", "OWD"];

/// Used when a bundle leaves nothing but its own directories on `PATH`, so the child still
/// has somewhere to resolve programs from.
const FALLBACK_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Computes the edits that undo the AppImage runtime's environment changes.
///
/// Works entry-by-entry rather than from a list of known variable names: AppImageKit's
/// `AppRun` and Tauri's linuxdeploy GTK/GStreamer hooks between them touch a good two dozen
/// variables, and the list grows with each bundler release. Anything holding a path inside
/// `$APPDIR` came from the bundle and is wrong for a child process; anything else
/// (`GDK_BACKEND=x11`, say) is left alone.
pub(crate) fn appimage_env_edits<'a>(
    vars: impl IntoIterator<Item = (&'a str, &'a str)>,
    appdir: &str,
) -> Vec<(String, EnvEdit)> {
    if appdir.is_empty() {
        return Vec::new();
    }
    let prefix = format!("{}/", appdir.trim_end_matches('/'));

    let mut edits = Vec::new();
    for (key, value) in vars {
        if APPIMAGE_MARKERS.contains(&key) {
            edits.push((key.to_string(), EnvEdit::Remove));
            continue;
        }
        let kept: Vec<&str> = value
            .split(':')
            .filter(|entry| !entry.starts_with(&prefix) && *entry != appdir)
            .collect();
        if kept.len() == value.split(':').count() {
            continue; // nothing of ours in there
        }
        edits.push((
            key.to_string(),
            match (kept.is_empty(), key) {
                (true, "PATH") => EnvEdit::Set(FALLBACK_PATH.to_string()),
                (true, _) => EnvEdit::Remove,
                (false, _) => EnvEdit::Set(kept.join(":")),
            },
        ));
    }
    edits
}

/// How long to give a spawned opener before deciding it survived.
///
/// `xdg-open` returns as soon as it has handed the URL to a handler, and the failure we
/// care about (a bundled library blowing up in the child) happens immediately — so this is
/// long enough to catch it and short enough that the click still feels instant.
#[cfg(not(windows))]
const SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

/// The edits to apply to child processes, or empty when we're not in an AppImage.
///
/// `APPDIR` is set by the AppImage runtime and nothing else, so this is inert on macOS and
/// on distro packages — whose environment is the user's own and must be passed through
/// untouched.
#[cfg(not(windows))]
fn current_env_edits() -> Vec<(String, EnvEdit)> {
    let Ok(appdir) = std::env::var("APPDIR") else {
        return Vec::new();
    };
    let vars: Vec<(String, String)> = std::env::vars().collect();
    appimage_env_edits(
        vars.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        &appdir,
    )
}

#[cfg(not(windows))]
fn apply(cmd: &mut std::process::Command, edits: &[(String, EnvEdit)]) {
    for (key, edit) in edits {
        match edit {
            EnvEdit::Set(value) => cmd.env(key, value),
            EnvEdit::Remove => cmd.env_remove(key),
        };
    }
}

/// Hand `target` (a URL or a directory) to the desktop's opener.
///
/// Walks the same candidate list the `open` crate uses (`xdg-open`, `gio open`,
/// `gnome-open`, `kde-open`, … — just `open` on macOS) but spawns each one itself, so the
/// AppImage's variables can be scrubbed first and so a candidate that dies on startup falls
/// through to the next instead of failing silently.
///
/// Windows keeps the crate's own `ShellExecuteW` path: it has neither problem, and going
/// through `cmd /c start` instead would risk a console flash.
#[cfg(not(windows))]
fn open_target(target: &str) -> Result<(), String> {
    let edits = current_env_edits();
    let mut failures = Vec::new();

    for mut cmd in open::commands(target) {
        let program = cmd.get_program().to_string_lossy().into_owned();
        apply(&mut cmd, &edits);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match cmd.spawn() {
            Ok(mut child) => {
                std::thread::sleep(SETTLE);
                match child.try_wait() {
                    // Still running, or finished cleanly: it took the URL.
                    Ok(None) => return Ok(()),
                    Ok(Some(status)) if status.success() => return Ok(()),
                    Ok(Some(status)) => failures.push(format!("{program} exited with {status}")),
                    Err(e) => failures.push(format!("{program}: {e}")),
                }
            }
            Err(e) => failures.push(format!("{program}: {e}")),
        }
    }

    Err(if failures.is_empty() {
        "no program is installed to open links (try installing xdg-utils)".to_string()
    } else {
        format!("could not open a browser: {}", failures.join("; "))
    })
}

#[cfg(windows)]
fn open_target(target: &str) -> Result<(), String> {
    open::that_detached(target).map_err(|e| e.to_string())
}

/// Spawn a specific program with the AppImage's environment scrubbed, for callers that
/// can't go through the opener candidate list (see `reveal_database`).
#[cfg(not(windows))]
pub(crate) fn spawn_clean(program: &str, args: &[&str]) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    apply(&mut cmd, &current_env_edits());
    cmd.spawn().map(|_| ())
}

/// Open an `http(s)` link in the user's default browser.
#[tauri::command]
pub async fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(format!("refusing to open non-web link: {url}"));
    }
    // The Linux path waits briefly on the child, so keep it off the main thread.
    tauri::async_runtime::spawn_blocking(move || open_target(&url))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPDIR: &str = "/tmp/.mount_AmanueXYZ";

    fn edits(vars: &[(&str, &str)]) -> Vec<(String, EnvEdit)> {
        appimage_env_edits(vars.iter().copied(), APPDIR)
    }

    #[test]
    fn strips_bundle_entries_and_keeps_system_ones() {
        assert_eq!(
            edits(&[(
                "LD_LIBRARY_PATH",
                "/tmp/.mount_AmanueXYZ/usr/lib:/tmp/.mount_AmanueXYZ/usr/lib/x86_64-linux-gnu:/usr/lib"
            )]),
            vec![(
                "LD_LIBRARY_PATH".to_string(),
                EnvEdit::Set("/usr/lib".to_string())
            )]
        );
    }

    #[test]
    fn removes_variables_that_are_entirely_ours() {
        // The GTK hook sets these wholesale to a path inside the bundle.
        assert_eq!(
            edits(&[(
                "GDK_PIXBUF_MODULE_FILE",
                "/tmp/.mount_AmanueXYZ/usr/lib/gdk-pixbuf-2.0/loaders.cache"
            )]),
            vec![("GDK_PIXBUF_MODULE_FILE".to_string(), EnvEdit::Remove)]
        );
    }

    #[test]
    fn keeps_path_usable_when_every_entry_was_ours() {
        assert_eq!(
            edits(&[("PATH", "/tmp/.mount_AmanueXYZ/usr/bin:/tmp/.mount_AmanueXYZ/bin")]),
            vec![("PATH".to_string(), EnvEdit::Set(FALLBACK_PATH.to_string()))]
        );
    }

    #[test]
    fn leaves_untouched_variables_alone() {
        // No edit at all, so the child inherits these unchanged.
        assert!(edits(&[
            ("GDK_BACKEND", "x11"),
            ("HOME", "/home/player"),
            ("PATH", "/usr/local/bin:/usr/bin:/bin"),
        ])
        .is_empty());
    }

    #[test]
    fn drops_the_appimage_markers() {
        assert_eq!(
            edits(&[("APPDIR", APPDIR), ("APPIMAGE", "/home/player/Amanuensis.AppImage")]),
            vec![
                ("APPDIR".to_string(), EnvEdit::Remove),
                ("APPIMAGE".to_string(), EnvEdit::Remove),
            ]
        );
    }

    #[test]
    fn a_name_that_merely_starts_with_appdir_is_not_ours() {
        // `/tmp/.mount_AmanueXYZ-notours` is a different directory.
        assert!(edits(&[("LD_LIBRARY_PATH", "/tmp/.mount_AmanueXYZ-notours/lib")]).is_empty());
    }

    #[test]
    fn no_appdir_means_no_edits() {
        assert!(appimage_env_edits([("LD_LIBRARY_PATH", "/tmp/.mount_X/usr/lib")], "").is_empty());
    }

    #[test]
    fn outside_an_appimage_nothing_is_edited() {
        assert!(current_env_edits().is_empty(), "APPDIR is set outside an AppImage?");
    }

    /// Covers the spawn path itself — `reveal_database` reaches the desktop through this on
    /// both macOS and Linux.
    #[test]
    fn spawn_clean_runs_a_program() {
        spawn_clean("/usr/bin/true", &[]).expect("spawn failed");
    }

    #[test]
    fn only_web_links_are_opened() {
        let err = tauri::async_runtime::block_on(open_external("file:///etc/passwd".into()))
            .expect_err("a file:// link should be refused");
        assert!(err.contains("non-web link"), "{err}");
    }
}
