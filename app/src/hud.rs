//! The floating stack window.

use incredibulk_core::HudCorner;
use tauri::{
    App, AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "hud";
pub const SETTINGS_LABEL: &str = "settings";
pub const HISTORY_LABEL: &str = "history";
pub const HOME_LABEL: &str = "home";

/// Margin from the screen edge, in logical pixels.
const MARGIN: f64 = 24.0;

/// Create both windows.
///
/// They are built here rather than declared in `tauri.conf.json` because
/// Tauri creates configured windows before the setup hook runs. Their pages
/// ask the backend for the session as soon as they load, and that request can
/// arrive before the shared state has been registered, which crashes the app
/// on startup roughly one launch in two. Building them after the state exists
/// makes the ordering explicit instead of a race.
pub fn create_windows(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("hud.html".into()))
        .title("Incredibulk session")
        .inner_size(360.0, 460.0)
        .min_inner_size(300.0, 240.0)
        .resizable(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .shadow(true)
        .build()?;

    WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
        .title("Incredibulk settings")
        .inner_size(820.0, 760.0)
        .min_inner_size(620.0, 520.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()?;

    WebviewWindowBuilder::new(app, HOME_LABEL, WebviewUrl::App("home.html".into()))
        .title("incredibulk")
        .inner_size(560.0, 620.0)
        .min_inner_size(460.0, 520.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()?;

    WebviewWindowBuilder::new(app, HISTORY_LABEL, WebviewUrl::App("history.html".into()))
        .title("Incredibulk history")
        .inner_size(760.0, 660.0)
        .min_inner_size(560.0, 420.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()?;

    Ok(())
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn show(app: &AppHandle, corner: HudCorner) {
    let Some(w) = window(app) else { return };

    // Already up: leave it exactly as it is. This runs again on every single
    // capture, and re-showing would drag a window the user had dragged
    // somewhere else back into its corner while they were mid-session.
    if w.is_visible().unwrap_or(false) {
        return;
    }

    // Position before showing, so it does not appear at the old spot and jump.
    let _ = place(&w, corner);
    reveal(&w);
    let _ = w.set_always_on_top(true);
}

/// Make the window visible without leaving the user typing into it.
///
/// A session opens while the user is working in another application, so a
/// stack window that keeps focus on the way up would swallow their next
/// keystroke. Showing it any way other than the toolkit's own would desync the
/// toolkit's record of what is visible, and a window it thinks is hidden can
/// never be hidden again, so the window is shown properly and the focus is
/// handed straight back to where it was.
fn reveal(w: &WebviewWindow) {
    let previous = crate::platform::foreground_window();
    let _ = w.show();
    if let Some(hwnd) = previous {
        crate::platform::restore_foreground(hwnd);
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = window(app) {
        let _ = w.hide();
    }
}

/// Show or hide on request. Unlike the automatic show when a session opens,
/// this one focuses the window: the user asked for it, and will want Escape
/// and the buttons to work without clicking first.
pub fn toggle(app: &AppHandle) {
    let Some(w) = window(app) else { return };
    match w.is_visible() {
        Ok(true) => {
            let _ = w.hide();
        }
        _ => {
            let corner = current_corner(app);
            let _ = place(&w, corner);
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn current_corner(app: &AppHandle) -> HudCorner {
    use crate::state::{AppState, lock};
    lock(&app.state::<AppState>().config).behavior.hud_corner
}

pub fn open_settings(app: &AppHandle) {
    raise(app, SETTINGS_LABEL);
}

pub fn open_history(app: &AppHandle) {
    raise(app, HISTORY_LABEL);
}

pub fn open_home(app: &AppHandle) {
    raise(app, HOME_LABEL);
}

/// Bring an ordinary window to the front. Unlike the stack window, these are
/// opened because the user asked, so taking focus is the point.
fn raise(app: &AppHandle, label: &str) {
    let Some(w) = app.get_webview_window(label) else { return };
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
}

/// Park the window in the configured corner of the monitor it is on.
fn place(w: &WebviewWindow, corner: HudCorner) -> tauri::Result<()> {
    // A hidden window may not report a monitor yet, so fall back to the
    // primary one rather than leaving it wherever it last was.
    let monitor = match w.current_monitor()? {
        Some(m) => Some(m),
        None => w.primary_monitor()?,
    };
    let Some(monitor) = monitor else { return Ok(()) };

    let screen = monitor.size();
    let origin = monitor.position();
    let size = w.outer_size()?;
    let margin = (MARGIN * monitor.scale_factor()).round() as i32;

    let right = origin.x + screen.width as i32 - size.width as i32 - margin;
    let bottom = origin.y + screen.height as i32 - size.height as i32 - margin;
    let left = origin.x + margin;
    let top = origin.y + margin;

    let (x, y) = match corner {
        HudCorner::TopLeft => (left, top),
        HudCorner::TopRight => (right, top),
        HudCorner::BottomLeft => (left, bottom),
        HudCorner::BottomRight => (right, bottom),
    };
    w.set_position(PhysicalPosition::new(x, y))
}
