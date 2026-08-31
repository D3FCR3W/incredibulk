//! The tray icon: the app has no main window, so this is where it lives.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::actions;
use crate::hud;

pub const TRAY_ID: &str = "incredibulk";

const IDLE_ICON: &[u8] = include_bytes!("../icons/tray-idle.png");
const LIVE_ICON: &[u8] = include_bytes!("../icons/tray-live.png");

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let home = MenuItem::with_id(app, "home", "Open Incredibulk", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Start or discard session", true, None::<&str>)?;
    let flush = MenuItem::with_id(app, "flush", "Flush and paste", true, None::<&str>)?;
    let clear = MenuItem::with_id(app, "clear", "Clear the stack", true, None::<&str>)?;
    let stack = MenuItem::with_id(app, "hud", "Show the stack", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Incredibulk", true, None::<&str>)?;
    let remove = MenuItem::with_id(app, "uninstall", "Remove Incredibulk", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &home,
            &toggle,
            &flush,
            &clear,
            &PredefinedMenuItem::separator(app)?,
            &stack,
            &history,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
            &remove,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .tooltip("Incredibulk: no session open")
        // Left click belongs to the stack window. Reserving it for the menu
        // would make the one thing users do most take two clicks.
        .show_menu_on_left_click(false)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => actions::toggle_session(app),
            "clear" => actions::clear_items(app),
            "hud" => hud::toggle(app),
            "history" => hud::open_history(app),
            "home" => hud::open_home(app),
            "settings" => hud::open_settings(app),
            "quit" => app.exit(0),
            "uninstall" => crate::uninstall::ask(app),
            "flush" => {
                if let Err(e) = actions::flush(app) {
                    actions::notify(app, actions::Notice::from_error(e));
                }
            }
            other => eprintln!("Incredibulk: unhandled tray item {other}"),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state, .. } = event
                && button == MouseButton::Left
                && button_state == MouseButtonState::Up
            {
                // With a session running the stack is what you want; with
                // none, the only useful thing is the front door.
                let app = tray.app_handle();
                if app.state::<crate::state::AppState>().is_active() {
                    hud::toggle(app);
                } else {
                    hud::open_home(app);
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Reflect the session in the tray. The icon is the only always-visible
/// indicator that a session is open, so it has to change.
pub fn refresh(app: &AppHandle, active: bool, count: usize) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };

    let icon = if active { LIVE_ICON } else { IDLE_ICON };
    if let Ok(image) = Image::from_bytes(icon) {
        let _ = tray.set_icon(Some(image));
    }

    let tooltip = if !active {
        "Incredibulk: no session open".to_string()
    } else if count == 1 {
        "Incredibulk: 1 item captured".to_string()
    } else {
        format!("Incredibulk: {count} items captured")
    };
    let _ = tray.set_tooltip(Some(&tooltip));
}
