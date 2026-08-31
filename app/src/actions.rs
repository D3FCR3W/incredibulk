//! Every operation the app can perform, in one place.
//!
//! The shortcut handler, the tray menu and the HUD buttons all call these, so
//! there is exactly one definition of what "flush" means rather than three
//! that drift apart.
//!
//! See the locking rule in [`crate::state`]: nothing here holds a lock across
//! a clipboard call.

use std::thread;
use std::time::Duration;

use incredibulk_core::{
    CaptureOutcome, ClipItem, ClipKind, Config, MoveDirection, Template, history, render,
    render_html,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::Wanted;
use crate::state::{AppState, lock};
use crate::store;
use crate::{hud, paste, platform, tray};

/// A short message for the HUD to surface. Rejected captures are reported the
/// same way accepted ones are, because a counter that silently fails to move
/// is the one behaviour that makes the tool untrustworthy.
#[derive(Clone, Serialize)]
pub struct Notice {
    pub level: &'static str,
    pub text: String,
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Self {
        Self { level: "info", text: text.into() }
    }
    pub fn warn(text: impl Into<String>) -> Self {
        Self { level: "warn", text: text.into() }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self { level: "error", text: text.into() }
    }

    /// Every caller that surfaces a failed action funnels through here, so
    /// the wording is the same whether it came from a shortcut or a click.
    pub fn from_error(text: impl Into<String>) -> Self {
        Self::error(text)
    }
}

/// What the clipboard thread needs to know each tick, read without locking.
pub fn watch_parameters(app: &AppHandle) -> (bool, Duration, Wanted) {
    let state = app.state::<AppState>();
    (
        state.is_active(),
        Duration::from_millis(state.poll_interval_ms()),
        Wanted { images: state.capture_images(), files: state.capture_files() },
    )
}

/// A new value appeared on the clipboard while a session was open.
pub fn on_clipboard_changed(app: &AppHandle, kind: ClipKind) {
    let state = app.state::<AppState>();
    if !state.is_active() {
        return;
    }

    let track_source = lock(&state.config).behavior.track_source_window;
    let source = if track_source { platform::foreground_title() } else { None };

    let outcome = lock(&state.session).capture(kind, source);

    match &outcome {
        CaptureOutcome::Appended { count, .. } => {
            notify(app, Notice::info(format!("Captured {count}")));
        }
        CaptureOutcome::Duplicate => {
            notify(app, Notice::warn("Already in this session"));
        }
        CaptureOutcome::Full { limit } => {
            notify(app, Notice::warn(format!("Session is full at {limit} items")));
        }
        CaptureOutcome::Filtered { kind } => {
            notify(app, Notice::warn(format!("Skipped: {kind} capture is switched off")));
        }
        // Blank content and a closed session are not worth interrupting for.
        CaptureOutcome::Blank | CaptureOutcome::Inactive => {}
    }
    broadcast(app);
}

pub fn toggle_session(app: &AppHandle) {
    if app.state::<AppState>().is_active() {
        cancel_session(app);
    } else {
        start_session(app);
    }
}

pub fn start_session(app: &AppHandle) {
    let state = app.state::<AppState>();

    let (policy, capture_current) = {
        let config = lock(&state.config);
        (config.capture.clone(), config.behavior.capture_clipboard_on_start)
    };

    // No locks are held from here until the clipboard work is done.
    let clipboard = state.clipboard();
    let existing = clipboard.and_then(|c| c.read());
    if let Some(c) = clipboard {
        // Adopt what is already on the clipboard as the baseline, so the first
        // tick does not mistake it for a fresh copy.
        c.rebase();
    }

    *lock(&state.saved_clipboard) = existing.clone();
    lock(&state.session).start(policy);
    state.set_active(true);
    // Opening a session is a fresh reason to show the list, whatever the
    // user did with the window during the last one.
    state.set_hud_dismissed(false);

    let mut seeded = 0usize;
    if capture_current
        && let Some(kind) = existing
        && lock(&state.session).capture(kind, platform::foreground_title()).accepted()
    {
        seeded = 1;
    }

    notify(
        app,
        Notice::info(if seeded > 0 {
            "Session open, starting from the clipboard".to_string()
        } else {
            "Session open. Copy away.".to_string()
        }),
    );
    broadcast(app);
}

pub fn cancel_session(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dropped = lock(&state.session).cancel().map(|s| s.len()).unwrap_or(0);
    state.set_active(false);
    *lock(&state.saved_clipboard) = None;
    notify(app, Notice::info(format!("Session discarded ({dropped} items)")));
    broadcast(app);
}

/// Render the stack, put it on the clipboard, and paste it.
///
/// With nothing collecting, this pastes the last session again instead of
/// refusing. The shortcut then means one thing at all times: put that block
/// where the cursor is. Having it do nothing whenever a session happens to be
/// closed made it a key you had to think about before pressing.
///
/// Returns the number of fragments in the block.
pub fn flush(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();

    // Everything needed is read under the locks and the locks are then
    // dropped, before any clipboard call.
    let (plain, rich, archive, name, count, keep_open, auto_paste, restore, delay) = {
        let session = lock(&state.session);
        // No session, or one that has caught nothing yet, is not a refusal:
        // it is the case where the last session is what was meant.
        let Some(current) = session.session().filter(|s| !s.is_empty()) else {
            drop(session);
            return replay_last(app);
        };
        // An open session whose fragments have all been unticked is different.
        // That is a decision the user just made inside this session, and
        // quietly pasting a different one instead would undo it behind their
        // back.
        if current.included_count() == 0 {
            return Err("Every fragment is unticked, so there is nothing to paste.".into());
        }
        let config = lock(&state.config);
        let included = current.included();
        let plain = current.render(&config.template);

        // Only build the markup when it is wanted and when the stack holds
        // something plain text cannot express.
        let rich = if config.behavior.paste_format.is_rich() && carries_an_image(&included) {
            Some(render_html(&included, &config.template))
        } else {
            None
        };
        let archive: Vec<ClipItem> = if config.behavior.keep_history {
            included.iter().map(|i| (*i).clone()).collect()
        } else {
            Vec::new()
        };

        (
            plain,
            rich,
            archive,
            current.name().map(str::to_owned),
            current.included_count(),
            config.behavior.keep_open_after_flush,
            config.behavior.auto_paste,
            config.behavior.restore_clipboard_after_flush,
            config.behavior.paste_delay_ms.clamp(0, 5_000),
        )
    };

    let clipboard = state
        .clipboard()
        .ok_or_else(|| "The clipboard service is not running.".to_string())?
        .clone();

    write_block(&clipboard, &plain, rich.as_deref())?;
    archive_session(app, archive, name);

    let previous = lock(&state.saved_clipboard).clone();
    if !keep_open {
        lock(&state.session).cancel();
        state.set_active(false);
        *lock(&state.saved_clipboard) = None;
    }
    broadcast(app);

    if auto_paste {
        // Off the calling thread: this is reached from a shortcut handler on
        // the event loop, and sleeping there would freeze the tray and the HUD.
        let app = app.clone();
        thread::spawn(move || {
            // Give the target application time to notice the new clipboard
            // contents, and the user time to let go of the shortcut.
            thread::sleep(Duration::from_millis(delay.max(1)));
            if let Err(e) = paste::send_paste() {
                notify(&app, Notice::error(format!("{e} The block is on your clipboard.")));
                return;
            }
            if restore && let Some(previous) = previous {
                // The receiving application reads the clipboard
                // asynchronously, so restoring immediately would hand it
                // the old value instead of the block.
                thread::sleep(Duration::from_millis((delay * 3).max(240)));
                let _ = clipboard.write(previous);
            }
        });
    } else {
        notify(app, Notice::info("Block copied. Paste it wherever you like."));
    }

    Ok(count)
}

/// Whether the block holds something plain text can only describe.
fn carries_an_image(items: &[&ClipItem]) -> bool {
    items.iter().any(|i| matches!(i.kind, ClipKind::Image { .. }))
}

/// Put a block on the clipboard, formatted if there is a formatted version.
///
/// The plain text goes on the clipboard either way, so a platform that cannot
/// take the markup costs the images, not the paste.
fn write_block(
    clipboard: &crate::clipboard::ClipboardHandle,
    plain: &str,
    rich: Option<&str>,
) -> Result<(), String> {
    match rich {
        Some(html) => {
            if let Err(e) = clipboard.write_rich(html.to_string(), plain.to_string()) {
                eprintln!("incredibulk: rich paste unavailable, falling back to text: {e}");
                clipboard.write_text(plain.to_string())
            } else {
                Ok(())
            }
        }
        None => clipboard.write_text(plain.to_string()),
    }
}

/// Keep a finished session so it can be pasted again later.
fn archive_session(app: &AppHandle, items: Vec<ClipItem>, name: Option<String>) {
    if items.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let limit = lock(&state.config).behavior.history_limit;

    let path = state.history_path.clone();
    let saved = {
        let mut stored = lock(&state.history);
        stored.set_limit(limit);
        history::record_now(&mut stored, items, name);
        stored.clone()
    };
    // Saved outside the lock: small, but it is still disk and this runs on the
    // event loop.
    if let Err(e) = store::save_history(&saved, &path) {
        eprintln!("incredibulk: history could not be saved: {e}");
    }
    let _ = app.emit("history", saved.views());
}

/// Paste one or more archived sessions as a single block.
pub fn replay(app: &AppHandle, ids: Vec<u64>) -> Result<usize, String> {
    let state = app.state::<AppState>();
    if ids.is_empty() {
        return Err("Nothing is ticked.".into());
    }

    let (plain, rich, count, auto_paste, delay) = {
        let stored = lock(&state.history);
        let items = stored.combined(&ids);
        if items.is_empty() {
            return Err("Those sessions are no longer in the history.".into());
        }
        let config = lock(&state.config);
        let rich = if config.behavior.paste_format.is_rich() && carries_an_image(&items) {
            Some(render_html(&items, &config.template))
        } else {
            None
        };
        (
            render(&items, &config.template),
            rich,
            items.len(),
            config.behavior.auto_paste,
            config.behavior.paste_delay_ms.clamp(0, 5_000),
        )
    };

    let clipboard = state
        .clipboard()
        .ok_or_else(|| "The clipboard service is not running.".to_string())?
        .clone();
    write_block(&clipboard, &plain, rich.as_deref())?;

    if auto_paste {
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay.max(1)));
            if let Err(e) = paste::send_paste() {
                notify(&app, Notice::error(format!("{e} The block is on your clipboard.")));
            }
        });
    } else {
        notify(app, Notice::info("Block copied. Paste it wherever you like."));
    }
    Ok(count)
}

/// Remember which archived sessions are ticked.
pub fn set_history_selection(app: &AppHandle, ids: Vec<u64>) {
    let state = app.state::<AppState>();
    let kept = lock(&state.history).retain_existing(&ids);
    *lock(&state.history_selection) = kept;
}

/// Paste archived sessions with no live session to flush.
///
/// Ticked sessions win: ticking is an explicit choice, and the shortcut should
/// do what the list on screen says it will. With nothing ticked there is only
/// one sensible answer, which is the session that just happened.
fn replay_last(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();

    let (ids, label) = {
        let stored = lock(&state.history);
        let ticked = lock(&state.history_selection).clone();
        let ids = history::replay_target(&stored, &ticked);
        let was_ticked = !stored.retain_existing(&ticked).is_empty();

        let label = match ids.len() {
            0 => String::new(),
            1 => {
                let name = stored.get(ids[0]).and_then(|e| e.name.clone());
                match (name, was_ticked) {
                    (Some(name), true) => format!("Pasted \"{name}\""),
                    (Some(name), false) => format!("Pasted \"{name}\" again"),
                    (None, true) => "Pasted the ticked session".to_string(),
                    (None, false) => "Pasted the last session again".to_string(),
                }
            }
            n => format!("Pasted {n} ticked sessions"),
        };
        (ids, label)
    };

    if ids.is_empty() {
        return Err(
            "Nothing has been captured yet, and there is no earlier session to repeat.".into()
        );
    }

    let count = replay(app, ids)?;
    // Say what landed: the user pressed paste with no session open and cannot
    // otherwise tell which of these two rules applied.
    notify(app, Notice::info(format!("{label} ({count})")));
    Ok(count)
}

pub fn forget(app: &AppHandle, ids: Vec<u64>) {
    let state = app.state::<AppState>();
    let path = state.history_path.clone();
    let saved = {
        let mut stored = lock(&state.history);
        for id in &ids {
            stored.remove(*id);
        }
        stored.clone()
    };
    // A deleted session cannot stay ticked.
    lock(&state.history_selection).retain(|id| !ids.contains(id));
    let _ = store::save_history(&saved, &path);
    let _ = app.emit("history", saved.views());
}

pub fn clear_history(app: &AppHandle) {
    let state = app.state::<AppState>();
    let path = state.history_path.clone();
    let saved = {
        let mut stored = lock(&state.history);
        stored.clear();
        stored.clone()
    };
    lock(&state.history_selection).clear();
    let _ = store::save_history(&saved, &path);
    let _ = app.emit("history", saved.views());
    notify(app, Notice::info("History cleared"));
}

/// Render archived sessions without pasting, for the history preview.
pub fn preview_history(app: &AppHandle, ids: Vec<u64>) -> String {
    let state = app.state::<AppState>();
    let template: Template = lock(&state.config).template.clone();
    lock(&state.history).render(&ids, &template)
}

/// Name the open session.
pub fn rename_session(app: &AppHandle, name: String) {
    let state = app.state::<AppState>();
    if let Some(s) = lock(&state.session).session_mut() {
        s.set_name(&name);
    }
    broadcast(app);
}

/// Name an archived session.
pub fn rename_history_entry(app: &AppHandle, id: u64, name: String) {
    let state = app.state::<AppState>();
    let path = state.history_path.clone();
    let saved = {
        let mut stored = lock(&state.history);
        stored.rename(id, &name);
        stored.clone()
    };
    let _ = store::save_history(&saved, &path);
    let _ = app.emit("history", saved.views());
}

pub fn undo_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    let removed = lock(&state.session).session_mut().and_then(|s| s.undo_last());
    match removed {
        Some(item) => notify(app, Notice::info(format!("Removed {}", item.kind.preview(40)))),
        None => notify(app, Notice::warn("Nothing to undo")),
    }
    broadcast(app);
}

pub fn remove_item(app: &AppHandle, id: u64) {
    let state = app.state::<AppState>();
    lock(&state.session).session_mut().map(|s| s.remove(id));
    broadcast(app);
}

pub fn move_item(app: &AppHandle, id: u64, direction: MoveDirection) {
    let state = app.state::<AppState>();
    lock(&state.session).session_mut().map(|s| s.move_item(id, direction));
    broadcast(app);
}

/// Tick or untick one fragment.
pub fn set_included(app: &AppHandle, id: u64, included: bool) {
    let state = app.state::<AppState>();
    if let Some(s) = lock(&state.session).session_mut() {
        s.set_included(id, included);
    }
    broadcast(app);
}

/// Tick or untick everything.
pub fn set_all_included(app: &AppHandle, included: bool) {
    let state = app.state::<AppState>();
    if let Some(s) = lock(&state.session).session_mut() {
        s.set_all_included(included);
    }
    broadcast(app);
}

/// Replace the text of one fragment with what the user typed.
pub fn edit_item(app: &AppHandle, id: u64, text: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let outcome = {
        let mut guard = lock(&state.session);
        let Some(session) = guard.session_mut() else {
            return Err("No session is open.".into());
        };
        session.edit_text(id, text).map_err(|e| e.to_string())?
    };
    if !outcome {
        notify(app, Notice::info("Emptied, so the fragment was removed"));
    }
    broadcast(app);
    Ok(())
}

/// Append a fragment the user typed rather than copied.
pub fn add_text(app: &AppHandle, text: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let outcome = {
        let mut guard = lock(&state.session);
        let Some(session) = guard.session_mut() else {
            return Err("No session is open.".into());
        };
        session.add_text(text, incredibulk_core::now_ms())
    };
    match outcome {
        CaptureOutcome::Appended { .. } => {}
        CaptureOutcome::Blank => return Err("Nothing to add.".into()),
        CaptureOutcome::Full { limit } => {
            return Err(format!("The session is full at {limit} items."));
        }
        _ => return Err("The line could not be added.".into()),
    }
    broadcast(app);
    Ok(())
}

/// Drop a fragment at an absolute position.
pub fn reorder_item(app: &AppHandle, id: u64, index: usize) {
    let state = app.state::<AppState>();
    if let Some(s) = lock(&state.session).session_mut() {
        s.move_to(id, index);
    }
    broadcast(app);
}

pub fn clear_items(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(s) = lock(&state.session).session_mut() {
        s.clear();
    }
    notify(app, Notice::info("Stack cleared"));
    broadcast(app);
}

/// Persist a new configuration and make it take effect immediately.
pub fn apply_config(app: &AppHandle, config: Config) -> Result<(), String> {
    let state = app.state::<AppState>();
    store::save_config(&config, &state.config_path).map_err(|e| e.to_string())?;
    state.refresh_mirrors(&config);
    let hotkeys = config.hotkeys.clone();
    *lock(&state.config) = config;

    let outcome = crate::hotkeys::reregister(app, &hotkeys);
    *lock(&state.hotkey_error) = outcome.as_ref().err().cloned();
    broadcast(app);
    outcome
}

/// Push the current state everywhere it is displayed.
///
/// Always deferred to the main thread, even when already on it. Positioning
/// the stack window asks the event loop for the monitor and the window size,
/// and those calls block until it answers. Making them from the clipboard
/// thread would deadlock against a flush: the flush runs on the event loop and
/// waits for the clipboard thread, which would in turn be waiting for the
/// event loop. Posting the work breaks the cycle at its only crossing point.
pub fn broadcast(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || broadcast_now(&handle));
}

fn broadcast_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = lock(&state.session).snapshot();
    let show_hud = lock(&state.config).behavior.show_hud;
    let corner = lock(&state.config).behavior.hud_corner;

    tray::refresh(app, snapshot.active, snapshot.count);

    if snapshot.active && show_hud && !state.hud_dismissed() {
        hud::show(app, corner);
    } else if !snapshot.active {
        hud::hide(app);
    }

    let _ = app.emit("session", &snapshot);
}

pub fn notify(app: &AppHandle, notice: Notice) {
    let _ = app.emit("notice", notice);
}
