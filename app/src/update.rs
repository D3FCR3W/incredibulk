//! Checking for, and installing, a new version.
//!
//! The rule here is that an update is offered, never imposed. This program
//! sits between someone and their clipboard all day; restarting it without
//! asking, in the middle of a session, would lose what they had collected.
//! So the check is automatic, the install is a button, and an open session
//! blocks the restart until it is finished.
//!
//! Every downloaded update is verified against the public key compiled into
//! the binary before a single byte of it is run. That is the whole reason the
//! updater can be trusted to replace the program with something it fetched
//! off the internet.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::state::{AppState, lock};

/// What the interface needs to decide whether to show anything.
#[derive(Clone, Serialize)]
pub struct UpdateStatus {
    pub available: bool,
    /// The version on offer, when there is one.
    pub version: Option<String>,
    pub notes: Option<String>,
    /// Set when the check could not be made at all: offline, rate limited, no
    /// release published yet. Reported rather than hidden, because "no update"
    /// and "could not tell" are different answers.
    pub error: Option<String>,
}

impl UpdateStatus {
    fn none() -> Self {
        Self { available: false, version: None, notes: None, error: None }
    }

    fn failed(reason: String) -> Self {
        Self { available: false, version: None, notes: None, error: Some(reason) }
    }
}

/// Ask whether a newer version exists.
pub async fn check(app: &AppHandle) -> UpdateStatus {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(e) => return UpdateStatus::failed(e.to_string()),
    };

    match updater.check().await {
        Ok(Some(update)) => UpdateStatus {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            error: None,
        },
        Ok(None) => UpdateStatus::none(),
        Err(e) => UpdateStatus::failed(e.to_string()),
    }
}

/// Download, verify and install, then restart.
///
/// Refused while a session is collecting. Everything in that session lives in
/// memory, so restarting would throw it away, and no version is worth losing
/// what someone spent ten minutes gathering.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if app.state::<AppState>().is_active() {
        return Err("A session is open. Paste or discard it first, then update.".into());
    }

    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "There is no update to install.".to_string())?;

    // The progress callbacks are where a progress bar would be wired. Left
    // empty on purpose: the installer shows its own, and two progress bars for
    // one operation is worse than one.
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;

    // Not reached on Windows, where the installer stops the program itself.
    // Kept for the platforms where it does return.
    app.restart();
}

/// Look for an update shortly after launch, and tell the windows if there is
/// one.
///
/// Deliberately not at the very first instant: the app has a tray icon to
/// register and shortcuts to claim, and a network round trip has no business
/// competing with either.
pub fn check_in_background(app: &AppHandle) {
    let wanted = lock(&app.state::<AppState>().config).behavior.check_for_updates;
    if !wanted {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(std::time::Duration::from_secs(5)).await;
        let status = check(&app).await;
        if status.available {
            let _ = tauri::Emitter::emit(&app, "update", &status);
        } else if let Some(reason) = &status.error {
            // Not shown to anyone: a failed check on startup is usually just a
            // laptop that has not found the wifi yet.
            eprintln!("Incredibulk: update check failed: {reason}");
        }
    });
}

async fn tokio_sleep(duration: std::time::Duration) {
    tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await.ok();
}
