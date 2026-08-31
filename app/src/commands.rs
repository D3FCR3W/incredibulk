//! The surface the HUD and settings windows call into.
//!
//! These are deliberately thin: they validate their input, delegate to
//! `actions`, and translate errors into strings the UI can show. No behaviour
//! lives here that the tray and the shortcuts cannot also reach.

use incredibulk_core::{
    ClipItem, ClipKind, Config, HistoryView, MoveDirection, Preset, SessionSnapshot, Template,
    Warning, presets, render,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::actions;
use crate::hud;
use crate::state::{AppState, lock};

/// Everything a window needs to draw itself, in one round trip.
#[derive(Serialize)]
pub struct UiState {
    snapshot: SessionSnapshot,
    config: Config,
    warnings: Vec<Warning>,
    presets: Vec<Preset>,
    defaults: Config,
    platform: PlatformInfo,
}

#[derive(Serialize)]
pub struct PlatformInfo {
    os: &'static str,
    /// Whether `{source}` will actually be filled in here.
    source_tracking: bool,
    /// Set when the config file on disk could not be read at startup.
    config_error: Option<String>,
    /// Set when the OS refused one or more shortcuts.
    hotkey_error: Option<String>,
    /// Whether copied file lists can be read here at all.
    file_capture: bool,
}

#[tauri::command]
pub fn get_state(app: AppHandle) -> UiState {
    let state = app.state::<AppState>();
    let config = lock(&state.config).clone();
    UiState {
        snapshot: lock(&state.session).snapshot(),
        warnings: config.warnings(),
        config,
        presets: presets(),
        defaults: Config::default(),
        platform: PlatformInfo {
            os: std::env::consts::OS,
            source_tracking: crate::platform::foreground_title().is_some(),
            config_error: lock(&state.config_error).clone(),
            hotkey_error: lock(&state.hotkey_error).clone(),
            file_capture: cfg!(target_os = "windows"),
        },
    }
}

#[tauri::command]
pub fn start_session(app: AppHandle) {
    actions::start_session(&app);
}

#[tauri::command]
pub fn cancel_session(app: AppHandle) {
    actions::cancel_session(&app);
}

#[tauri::command]
pub fn toggle_session(app: AppHandle) {
    actions::toggle_session(&app);
}

#[tauri::command]
pub fn flush_session(app: AppHandle) -> Result<usize, String> {
    actions::flush(&app)
}

#[tauri::command]
pub fn undo_last(app: AppHandle) {
    actions::undo_last(&app);
}

#[tauri::command]
pub fn remove_item(app: AppHandle, id: u64) {
    actions::remove_item(&app, id);
}

#[tauri::command]
pub fn move_item(app: AppHandle, id: u64, direction: MoveDirection) {
    actions::move_item(&app, id, direction);
}

#[tauri::command]
pub fn clear_items(app: AppHandle) {
    actions::clear_items(&app);
}

#[tauri::command]
pub fn rename_session(app: AppHandle, name: String) {
    actions::rename_session(&app, name);
}

#[tauri::command]
pub fn rename_history_entry(app: AppHandle, id: u64, name: String) {
    actions::rename_history_entry(&app, id, name);
}

#[tauri::command]
pub fn reorder_item(app: AppHandle, id: u64, index: usize) {
    actions::reorder_item(&app, id, index);
}

#[tauri::command]
pub fn set_included(app: AppHandle, id: u64, included: bool) {
    actions::set_included(&app, id, included);
}

#[tauri::command]
pub fn set_all_included(app: AppHandle, included: bool) {
    actions::set_all_included(&app, included);
}

#[tauri::command]
pub fn edit_item(app: AppHandle, id: u64, text: String) -> Result<(), String> {
    actions::edit_item(&app, id, text)
}

#[tauri::command]
pub fn add_text(app: AppHandle, text: String) -> Result<(), String> {
    actions::add_text(&app, text)
}

/// The full text of one fragment, fetched only when an editor opens for it.
///
/// The snapshot carries short previews so that pushing it on every capture
/// stays cheap; a fragment can be a whole page, and the list shows dozens.
#[tauri::command]
pub fn item_text(app: AppHandle, id: u64) -> Option<String> {
    let state = app.state::<AppState>();
    let guard = lock(&state.session);
    let session = guard.session()?;
    session.items().iter().find(|i| i.id == id).and_then(|i| i.text()).map(str::to_owned)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: Config) -> Result<Vec<Warning>, String> {
    let warnings = config.warnings();
    let autostart = config.behavior.autostart;
    actions::apply_config(&app, config)?;
    // Reported rather than raised: an autostart entry that cannot be written
    // is a nuisance, not a reason to reject the whole settings save.
    if let Err(e) = sync_autostart(&app, autostart) {
        actions::notify(&app, actions::Notice::warn(format!("Autostart: {e}")));
    }
    Ok(warnings)
}

#[tauri::command]
pub fn reset_config(app: AppHandle) -> Result<Vec<Warning>, String> {
    let config = Config::default();
    let warnings = config.warnings();
    actions::apply_config(&app, config)?;
    Ok(warnings)
}

/// Render a template against sample fragments, so the settings pane can show
/// the shape of the output before anything is captured.
#[tauri::command]
pub fn preview(app: AppHandle, template: Template) -> String {
    let state = app.state::<AppState>();
    let live = lock(&state.session);

    // Prefer the real stack: seeing your own fragments beats seeing samples.
    match live.session() {
        Some(session) if !session.is_empty() => session.render(&template),
        _ => render(&sample_items().iter().collect::<Vec<_>>(), &template),
    }
}

fn sample_items() -> Vec<ClipItem> {
    let now = incredibulk_core::now_ms();
    vec![
        ClipItem::new(1, ClipKind::text("first fragment"), now, Some("Editor".into())),
        ClipItem::new(2, ClipKind::text("second fragment"), now, Some("Browser".into())),
        ClipItem::new(3, ClipKind::text("third fragment"), now, Some("Terminal".into())),
    ]
}

/// Every archived session, newest first.
#[tauri::command]
pub fn get_history(app: AppHandle) -> Vec<HistoryView> {
    let state = app.state::<AppState>();
    lock(&state.history).views()
}

/// Paste the ticked sessions as one block.
#[tauri::command]
pub fn replay_history(app: AppHandle, ids: Vec<u64>) -> Result<usize, String> {
    actions::replay(&app, ids)
}

/// Mirror the ticked rows into the backend, so the paste shortcut can
/// honour them while the history window is closed.
#[tauri::command]
pub fn set_history_selection(app: AppHandle, ids: Vec<u64>) {
    actions::set_history_selection(&app, ids);
}

#[tauri::command]
pub fn forget_history(app: AppHandle, ids: Vec<u64>) {
    actions::forget(&app, ids);
}

#[tauri::command]
pub fn clear_history(app: AppHandle) {
    actions::clear_history(&app);
}

/// What the ticked sessions would paste as, without pasting them.
#[tauri::command]
pub fn preview_history(app: AppHandle, ids: Vec<u64>) -> String {
    actions::preview_history(&app, ids)
}

#[tauri::command]
pub fn open_history(app: AppHandle) {
    hud::open_history(&app);
}

#[tauri::command]
pub fn open_home(app: AppHandle) {
    hud::open_home(&app);
}

/// Bring the stack window back, whether or not it was put away.
/// Ask whether a newer version exists. Never installs anything.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> crate::update::UpdateStatus {
    crate::update::check(&app).await
}

/// Download, verify and install. Refused while a session is collecting.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    crate::update::install(&app).await
}

#[tauri::command]
pub fn open_stack(app: AppHandle) {
    let state = app.state::<AppState>();
    state.set_hud_dismissed(false);
    let corner = lock(&state.config).behavior.hud_corner;
    hud::show(&app, corner);
}

/// Mark the introduction as seen and save, so it appears once.
#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = lock(&state.config).clone();
    config.onboarded = true;
    drop(config.warnings());
    actions::apply_config(&app, config)
}

/// Put the stack window away.
///
/// This is what the close button in its header does. It has to be remembered,
/// or the very next capture would show the window again.
#[tauri::command]
pub fn hide_hud(app: AppHandle) {
    app.state::<AppState>().set_hud_dismissed(true);
    hud::hide(&app);
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    hud::open_settings(&app);
}

#[tauri::command]
pub fn close_window(app: AppHandle, label: String) {
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.hide();
    }
}

/// Put the login entry in the state the settings ask for.
pub fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| e.to_string())
}
