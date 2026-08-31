//! Taking Incredibulk back off the machine.
//!
//! Incredibulk ships two ways: as a single portable file, and through an installer.
//! Both leave the same traces behind, and those traces are easy to miss, which
//! is the whole reason this exists. Only the last step differs, so the wording
//! adapts rather than asserting one story and being wrong half the time.
//!
//! - The login entry, which is the only thing that is genuinely *installed*.
//! - Settings and saved sessions, a few kilobytes in the roaming profile.
//! - The webview's cache, which is tens of megabytes and which nobody would
//!   ever think to look for.
//!
//! The executable itself cannot delete itself while it is running, and the
//! cache folder is held open by the webview, so both are reported rather than
//! promised. Saying what is left is better than claiming a clean removal that
//! did not happen.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::state::AppState;

/// What removal will touch, worked out before anything is destroyed so the
/// question can be specific about it.
struct Footprint {
    settings: Option<PathBuf>,
    cache: Option<PathBuf>,
    cache_bytes: u64,
    executable: Option<PathBuf>,
    autostart: bool,
    /// Whether this copy was put here by an installer rather than dropped in
    /// by hand. It is the difference between "delete this file" and "use
    /// Windows to remove the program", and getting it wrong sends someone to
    /// delete a file out from under an install that still believes it is
    /// there.
    installed: bool,
}

impl Footprint {
    fn survey(app: &AppHandle) -> Self {
        let settings =
            app.state::<AppState>().config_path.parent().map(PathBuf::from).filter(|p| p.exists());

        let cache = app.path().app_local_data_dir().ok().filter(|p| p.exists());
        let cache_bytes = cache.as_deref().map(directory_size).unwrap_or(0);

        let executable = std::env::current_exe().ok();
        let installed = executable.as_deref().is_some_and(was_installed);

        Self {
            settings,
            cache,
            cache_bytes,
            executable,
            autostart: app.autolaunch().is_enabled().unwrap_or(false),
            installed,
        }
    }

    fn question(&self) -> String {
        let opening = if self.installed {
            "This clears what Incredibulk leaves in your profile. The program itself \
             is removed through Windows, in Settings under Installed apps."
        } else {
            "Incredibulk was never installed: it is a single file you can delete."
        };

        let mut lines =
            vec![opening.to_string(), String::new(), "Removing it will take away:".to_string()];

        if self.autostart {
            lines.push("  the entry that starts it when you log in".into());
        }
        if let Some(path) = &self.settings {
            lines.push(format!("  your settings and saved sessions, in {}", path.display()));
        }
        if self.cache_bytes > 0 {
            lines.push(format!("  {} of browser cache it built up", human(self.cache_bytes)));
        }

        lines.push(String::new());
        lines.push("This cannot be undone. Remove it and quit?".into());
        lines.join("\n")
    }
}

/// Ask first. This throws away the user's saved sessions, and there is no
/// putting them back.
pub fn ask(app: &AppHandle) {
    let footprint = Footprint::survey(app);
    let handle = app.clone();

    app.dialog()
        .message(footprint.question())
        .title("Remove Incredibulk")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Remove and quit".into(),
            "Keep Incredibulk".into(),
        ))
        .show(move |confirmed| {
            if confirmed {
                perform(&handle, footprint);
            }
        });
}

fn perform(app: &AppHandle, footprint: Footprint) {
    let mut leftovers: Vec<PathBuf> = Vec::new();

    // The login entry first. If everything after this fails, at least the app
    // has stopped coming back on its own.
    if let Err(e) = app.autolaunch().disable() {
        eprintln!("Incredibulk: could not remove the login entry: {e}");
    }

    if let Some(path) = &footprint.settings
        && let Err(e) = std::fs::remove_dir_all(path)
    {
        eprintln!("Incredibulk: could not remove {}: {e}", path.display());
        leftovers.push(path.clone());
    }

    // The webview holds this open while the app is running, so it usually
    // survives. Attempted anyway: on some machines it does go.
    if let Some(path) = &footprint.cache
        && std::fs::remove_dir_all(path).is_err()
        && path.exists()
    {
        leftovers.push(path.clone());
    }

    // With an installer in play, the program is Windows' to remove, so
    // pointing at the executable would send the user to delete a file out from
    // under an install that still thinks it is there.
    let installed = footprint.installed;
    if !installed && let Some(exe) = &footprint.executable {
        leftovers.push(exe.clone());
    }

    let report = if leftovers.is_empty() {
        if installed {
            "Settings and the login entry are gone. Remove the program itself from \
             Windows Settings, under Installed apps."
                .to_string()
        } else {
            "Incredibulk is gone.".to_string()
        }
    } else {
        // The paths go on the clipboard, which is the one thing this app is
        // certain to be good at, because nobody can copy text out of a message
        // box.
        let listed =
            leftovers.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
        let copied = app
            .state::<AppState>()
            .clipboard()
            .map(|c| c.write_text(listed.clone()).is_ok())
            .unwrap_or(false);

        let tail = if installed {
            "Remove the program itself from Windows Settings, under Installed apps."
        } else if copied {
            "Those paths are on your clipboard."
        } else {
            "Copy the paths above before closing this."
        };

        format!(
            "Settings and the login entry are gone.\n\n\
             These are still in use and have to go by hand:\n{listed}\n\n{tail}"
        )
    };

    app.dialog()
        .message(report)
        .title("Incredibulk removed")
        .buttons(MessageDialogButtons::OkCustom("Close".into()))
        .show({
            let handle = app.clone();
            move |_| handle.exit(0)
        });
}

/// Whether an executable at this path was put there by an installer.
///
/// Two signals, because the two installers leave different traces. NSIS drops
/// an `uninstall.exe` beside the program. An MSI leaves nothing next to it at
/// all and registers with Windows instead, but it installs into a program
/// directory, which a portable copy has no reason to be in.
///
/// A false negative here is mild: the user is told to delete a file that
/// Windows will remove for them. A false positive is worse, so this only
/// treats the standard program directories as installed, not merely any path
/// the user cannot write to.
fn was_installed(exe: &std::path::Path) -> bool {
    if exe.parent().map(|dir| dir.join("uninstall.exe").exists()).unwrap_or(false) {
        return true;
    }

    let program_dirs = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"];
    program_dirs.iter().filter_map(std::env::var_os).any(|dir| exe.starts_with(PathBuf::from(dir)))
}

fn directory_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(t) if t.is_dir() => directory_size(&entry.path()),
            Ok(_) => entry.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

fn human(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes >= MB { format!("{} MB", bytes / MB) } else { format!("{} KB", (bytes / 1024).max(1)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_portable_copy_is_not_an_installed_one() {
        let loose = std::env::temp_dir().join("incredibulk-portable-4f2a.exe");
        assert!(!was_installed(&loose));
    }

    #[test]
    fn an_uninstaller_beside_the_program_means_installed() {
        let dir = std::env::temp_dir().join("incredibulk-nsis-4f2a");
        std::fs::create_dir_all(&dir).expect("dirs");
        std::fs::write(dir.join("uninstall.exe"), b"").expect("write");

        assert!(was_installed(&dir.join("incredibulk.exe")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_directory_means_installed() {
        // Skipped where the variable is absent, which is every platform that
        // does not have a Program Files to begin with.
        let Some(dir) = std::env::var_os("ProgramFiles") else {
            return;
        };
        let exe = PathBuf::from(dir).join("Incredibulk").join("incredibulk.exe");
        assert!(was_installed(&exe));
    }

    #[test]
    fn sizes_read_the_way_a_person_would_say_them() {
        assert_eq!(human(5 * 1024 * 1024), "5 MB");
        assert_eq!(human(2048), "2 KB");
        // Never "0 KB" for something that exists: that reads as nothing there.
        assert_eq!(human(1), "1 KB");
    }

    #[test]
    fn measuring_a_missing_directory_is_not_an_error() {
        assert_eq!(directory_size(std::path::Path::new("no-such-directory-4f2a")), 0);
    }

    #[test]
    fn a_directory_tree_is_measured_whole() {
        let root = std::env::temp_dir().join("incredibulk-size-4f2a");
        let nested = root.join("deep");
        std::fs::create_dir_all(&nested).expect("dirs");
        std::fs::write(root.join("a"), vec![0u8; 1000]).expect("write");
        std::fs::write(nested.join("b"), vec![0u8; 2000]).expect("write");

        assert_eq!(directory_size(&root), 3000);
        let _ = std::fs::remove_dir_all(&root);
    }
}
