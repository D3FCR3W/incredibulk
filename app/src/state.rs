//! Shared application state.
//!
//! ## Locking rule
//!
//! Never call into [`ClipboardHandle`](crate::clipboard::ClipboardHandle)
//! while holding `session` or `config`. Those calls block until the clipboard
//! thread answers, and that thread takes the `session` lock when it detects a
//! copy. Holding a lock across the call would deadlock the pair. Every
//! operation in `actions` therefore computes what it needs under the lock,
//! drops it, and only then talks to the clipboard.
//!
//! The hot values the clipboard thread reads on every tick are mirrored into
//! atomics precisely so that the polling path never has to take a lock at all.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use incredibulk_core::{ClipKind, Config, History, SessionStatus};

use crate::clipboard::ClipboardHandle;

pub struct AppState {
    pub session: Mutex<SessionStatus>,
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    /// Set once, immediately after the clipboard thread starts. It cannot be
    /// built before the state is managed, because the thread needs the app
    /// handle that managing produces.
    pub clipboard: OnceLock<ClipboardHandle>,
    /// What was on the clipboard before the session, so it can be put back
    /// after the flush instead of leaving the pasted block behind.
    pub saved_clipboard: Mutex<Option<ClipKind>>,
    /// Why the settings file on disk was unusable, if it was. Surfaced in
    /// the settings pane instead of being swallowed, so a hand-edited file
    /// with a typo does not look like the app ignoring the user.
    pub config_error: Mutex<Option<String>>,
    /// Finished sessions, kept so a paste can be replayed.
    pub history: Mutex<History>,
    pub history_path: PathBuf,
    /// Sessions ticked in the history list.
    ///
    /// Held here rather than only in the window, because the paste
    /// shortcut has to honour it and the shortcut works whether or not
    /// that window is open. Deliberately not saved: a tick is about what
    /// you are doing now, not a preference.
    pub history_selection: Mutex<Vec<u64>>,
    /// Which shortcuts the OS refused to give us, usually because another
    /// application already owns them. Kept so the settings pane can say so:
    /// a shortcut that silently does nothing is the worst failure mode this
    /// app has.
    pub hotkey_error: Mutex<Option<String>>,

    /// Set when the user closes the stack window during a live session.
    ///
    /// Without this the next capture would put it straight back: the window
    /// is shown whenever a session is running, so dismissing it would last
    /// exactly until the next copy. Cleared when a session starts.
    hud_dismissed: AtomicBool,

    active: AtomicBool,
    poll_interval_ms: AtomicU64,
    capture_images: AtomicBool,
    capture_files: AtomicBool,
}

impl AppState {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        config_error: Option<String>,
        history: History,
        history_path: PathBuf,
    ) -> Self {
        let poll = config.behavior.poll_interval_ms.clamp(30, 5_000);
        let images = config.capture.capture_images;
        let files = config.capture.capture_files;
        Self {
            session: Mutex::new(SessionStatus::new()),
            config: Mutex::new(config),
            config_path,
            clipboard: OnceLock::new(),
            saved_clipboard: Mutex::new(None),
            config_error: Mutex::new(config_error),
            history: Mutex::new(history),
            history_path,
            history_selection: Mutex::new(Vec::new()),
            hud_dismissed: AtomicBool::new(false),
            hotkey_error: Mutex::new(None),
            active: AtomicBool::new(false),
            poll_interval_ms: AtomicU64::new(poll),
            capture_images: AtomicBool::new(images),
            capture_files: AtomicBool::new(files),
        }
    }

    pub fn clipboard(&self) -> Option<&ClipboardHandle> {
        self.clipboard.get()
    }

    pub fn hud_dismissed(&self) -> bool {
        self.hud_dismissed.load(Ordering::Relaxed)
    }

    pub fn set_hud_dismissed(&self, value: bool) {
        self.hud_dismissed.store(value, Ordering::Relaxed);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, value: bool) {
        self.active.store(value, Ordering::Relaxed);
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms.load(Ordering::Relaxed)
    }

    pub fn capture_images(&self) -> bool {
        self.capture_images.load(Ordering::Relaxed)
    }

    pub fn capture_files(&self) -> bool {
        self.capture_files.load(Ordering::Relaxed)
    }

    /// Refresh the mirrored values after the config changes.
    pub fn refresh_mirrors(&self, config: &Config) {
        self.poll_interval_ms
            .store(config.behavior.poll_interval_ms.clamp(30, 5_000), Ordering::Relaxed);
        self.capture_images.store(config.capture.capture_images, Ordering::Relaxed);
        self.capture_files.store(config.capture.capture_files, Ordering::Relaxed);
    }
}

/// Take a lock, ignoring poisoning.
///
/// A panic in one operation leaves the session or config mutex poisoned. That
/// is not a reason to take the whole app down: the data behind these locks is
/// a list of captured fragments and a settings struct, both of which are
/// perfectly usable afterwards. Refusing to unlock would strand the user with
/// a tray icon that no longer responds.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
