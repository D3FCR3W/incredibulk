//! Sending the paste keystroke into whatever window has focus.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// The paste modifier: Command on macOS, Control everywhere else.
#[cfg(target_os = "macos")]
const PASTE_MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const PASTE_MODIFIER: Key = Key::Control;

/// Put the block into the focused window.
///
/// The flush is reached by a shortcut, which means the user is physically
/// holding modifiers at the moment this runs. Sending Ctrl+V while Alt is
/// still down produces Ctrl+Alt+V, which is not paste in any application, so
/// the held modifiers are explicitly released first. Releasing a key that is
/// not down is harmless; not releasing one that is loses the paste.
pub fn send_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("input simulation is unavailable: {e}"))?;

    for key in [Key::Alt, Key::Shift, Key::Meta, Key::Control] {
        let _ = enigo.key(key, Direction::Release);
    }

    enigo
        .key(PASTE_MODIFIER, Direction::Press)
        .map_err(|e| format!("could not press the paste modifier: {e}"))?;

    // The modifier is down from here on, so the result is held rather than
    // returned early: leaving Ctrl stuck down would break the user's keyboard
    // until they pressed and released it themselves.
    let pressed = enigo.key(Key::Unicode('v'), Direction::Click);
    let released = enigo.key(PASTE_MODIFIER, Direction::Release);

    pressed.map_err(|e| format!("could not send the paste key: {e}"))?;
    released.map_err(|e| format!("could not release the paste modifier: {e}"))?;
    Ok(())
}
