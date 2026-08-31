//! Global shortcut registration.

use incredibulk_core::Hotkeys;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::actions;

/// Register every configured shortcut, replacing whatever was registered
/// before.
///
/// A shortcut another application already owns cannot be taken, and one the
/// user mistyped will not parse. Neither is fatal: the failures are collected
/// and reported so the settings pane can show which bindings are dead, while
/// the rest of the app carries on with the ones that did register.
pub fn reregister(app: &AppHandle, hotkeys: &Hotkeys) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| format!("previous shortcuts could not be released: {e}"))?;

    let mut failures = Vec::new();

    for (name, accelerator) in hotkeys.all() {
        let accelerator = accelerator.trim();
        if accelerator.is_empty() {
            continue;
        }
        let action = name;
        let registered = shortcuts.on_shortcut(accelerator, move |app, _shortcut, event| {
            // Fires on press and release. Acting on both would run every
            // action twice.
            if event.state == ShortcutState::Pressed {
                dispatch(app, action);
            }
        });
        if let Err(e) = registered {
            failures.push(format!("{name} ({accelerator}): {e}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "These shortcuts could not be registered, most likely because another application already owns them: {}",
            failures.join("; ")
        ))
    }
}

fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggle_session" => actions::toggle_session(app),
        "cancel" => actions::cancel_session(app),
        "undo_last" => actions::undo_last(app),
        "toggle_hud" => crate::hud::toggle(app),
        "history" => crate::hud::open_history(app),
        "flush" => {
            if let Err(e) = actions::flush(app) {
                actions::notify(app, actions::Notice::from_error(e));
            }
        }
        other => eprintln!("incredibulk: no action bound to {other}"),
    }
}
