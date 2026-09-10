// A console window behind a tray utility would be a bug, so release builds
// detach from it. Debug builds keep it: the clipboard and shortcut layers
// report platform failures on stderr and those are the ones worth seeing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod clipboard;
mod commands;
mod hotkeys;
mod hud;
mod imaging;
mod paste;
mod platform;
mod state;
mod store;
mod tray;
mod uninstall;
mod update;

use incredibulk_core::Config;
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::state::AppState;

/// Write panics to a file next to the settings.
///
/// A release build has no console, so a crash is completely silent: the tray
/// icon simply disappears and there is nothing to look at. This is the only
/// thing standing between "it closed itself" and an actual diagnosis.
fn install_crash_log(dir: std::path::PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let where_ = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".into());
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned());
        let line = format!(
            "{} v{} panicked at {}: {}\n",
            when,
            env!("CARGO_PKG_VERSION"),
            where_,
            what.unwrap_or_else(|| "no message".into())
        );

        if std::fs::create_dir_all(&dir).is_ok() {
            use std::io::Write;
            if let Ok(mut file) =
                std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log"))
            {
                let _ = file.write_all(line.as_bytes());
            }
        }
        eprintln!("Incredibulk: {line}");
        previous(info);
    }));
}

fn main() {
    let app = tauri::Builder::default()
        // Registered first, as the plugin requires. Two instances would fight
        // over the same global shortcuts: whichever lost would sit there doing
        // nothing while its tray icon suggested otherwise. A second launch
        // opens settings on the instance already running instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            hud::open_settings(app);
            actions::notify(app, actions::Notice::info("Incredibulk is already running"));
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_session,
            commands::cancel_session,
            commands::toggle_session,
            commands::flush_session,
            commands::undo_last,
            commands::remove_item,
            commands::move_item,
            commands::clear_items,
            commands::reorder_item,
            commands::set_included,
            commands::set_all_included,
            commands::edit_item,
            commands::add_text,
            commands::item_text,
            commands::save_config,
            commands::reset_config,
            commands::preview,
            commands::hide_hud,
            commands::open_settings,
            commands::open_history,
            commands::open_home,
            commands::open_stack,
            commands::check_for_update,
            commands::install_update,
            commands::finish_onboarding,
            commands::rename_session,
            commands::rename_history_entry,
            commands::get_history,
            commands::search_history,
            commands::replay_history,
            commands::set_history_selection,
            commands::forget_history,
            commands::clear_history,
            commands::preview_history,
            commands::close_window,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            let config_dir = app.path().app_config_dir()?;
            install_crash_log(config_dir.clone());
            let config_path = config_dir.join("config.json");
            // A broken settings file must not stop the app from starting: fall
            // back to defaults and carry the reason through to the settings
            // pane, where the user can see it and fix it.
            let (config, config_error) = match store::load_config(&config_path) {
                Ok(c) => (c, None),
                Err(e) => {
                    eprintln!("Incredibulk: {e}");
                    (Config::default(), Some(e.to_string()))
                }
            };
            let hotkeys = config.hotkeys.clone();
            let wants_autostart = config.behavior.autostart;
            let first_run = !config.onboarded;

            let history_path = config_path.with_file_name("history.json");
            let mut history = store::load_history(&history_path);
            history.set_limit(config.behavior.history_limit);

            app.manage(AppState::new(config, config_path, config_error, history, history_path));

            // The clipboard thread needs the handle that managing produced, so
            // it can only start now.
            let clipboard = clipboard::spawn(handle.clone());
            let _ = app.state::<AppState>().clipboard.set(clipboard);

            // Only now, with the state in place, may a window load a page that
            // can call back into the app.
            hud::create_windows(app)?;

            tray::build(&handle)?;

            if let Err(e) = hotkeys::reregister(&handle, &hotkeys) {
                eprintln!("Incredibulk: {e}");
                // Kept, not just announced: at this point no window is open to
                // receive the notice, and a shortcut that silently does nothing
                // is the failure a user is least able to diagnose.
                *state::lock(&app.state::<AppState>().hotkey_error) = Some(e.clone());
                actions::notify(&handle, actions::Notice::warn(e));
            }

            // Keep the login entry in step with the setting. Without this, a
            // config that says autostart is on stays a lie until the user opens
            // settings and saves again.
            if let Err(e) = commands::sync_autostart(&handle, wants_autostart) {
                eprintln!("Incredibulk: autostart could not be set: {e}");
            }

            // No dock icon on macOS: this is a tray utility, and a bouncing
            // dock entry for a window that is normally hidden is noise.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Looked for a few seconds after launch, so it never competes
            // with claiming the shortcuts or registering the tray icon.
            update::check_in_background(&handle);

            actions::broadcast(&handle);

            // Nothing about a tray icon tells a first-time user which keys to
            // press, so the front door opens itself the first time. It carries
            // the introduction, and finishing or skipping it writes the config
            // file, which is what stops it happening again.
            if first_run {
                hud::open_home(&handle);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing a window hides it. The app lives in the tray, and the
            // stack has to survive the user tidying their screen.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|_handle, event| {
            // Every window being hidden is the normal resting state, so it must
            // not end the process. An explicit Quit carries an exit code and is
            // allowed through.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        }),
        Err(e) => {
            eprintln!("Incredibulk could not start: {e}");
            std::process::exit(1);
        }
    }
}
