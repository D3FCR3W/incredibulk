//! User settings, and the rules the session reads out of them.
//!
//! Every struct is `#[serde(default)]` so a config file written by an older
//! build still loads: unknown fields are ignored, missing ones fall back. A
//! settings file is not worth a startup failure.

use serde::{Deserialize, Serialize};

use crate::template::Template;

/// What goes onto the clipboard when a session is flushed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PasteFormat {
    /// Plain text and nothing else. Pastes identically everywhere, and an
    /// image can only be described, never shown.
    Plain,
    /// Rich text with the images embedded, and the plain block written
    /// alongside it rather than instead of it. Editors and terminals still
    /// receive exactly the plain text they would have received; only
    /// applications that ask for formatting see the images.
    #[default]
    Rich,
}

impl PasteFormat {
    pub fn is_rich(self) -> bool {
        matches!(self, PasteFormat::Rich)
    }
}

/// Where the HUD parks itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HudCorner {
    TopLeft,
    TopRight,
    #[default]
    BottomRight,
    BottomLeft,
}

/// Global shortcuts, in the accelerator syntax the OS layer understands.
/// `CmdOrCtrl` resolves to Command on macOS and Control elsewhere.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    /// Open a session, or close and discard the open one.
    pub toggle_session: String,
    /// Render the stack and paste it into the focused window.
    pub flush: String,
    /// Discard the session without pasting.
    pub cancel: String,
    /// Drop the most recent capture.
    pub undo_last: String,
    /// Show or hide the stack window.
    pub toggle_hud: String,
    /// Open the list of past sessions.
    pub history: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        // Ctrl+Alt is deliberate. Ctrl+Shift+C and Ctrl+Shift+V are already
        // copy and paste in most terminals, and stealing them globally would
        // break the tool for exactly the people most likely to want it.
        Self {
            toggle_session: "CmdOrCtrl+Alt+C".into(),
            flush: "CmdOrCtrl+Alt+V".into(),
            cancel: "CmdOrCtrl+Alt+X".into(),
            undo_last: "CmdOrCtrl+Alt+Z".into(),
            toggle_hud: "CmdOrCtrl+Alt+S".into(),
            history: "CmdOrCtrl+Alt+H".into(),
        }
    }
}

impl Hotkeys {
    pub fn all(&self) -> [(&'static str, &str); 6] {
        [
            ("toggle_session", &self.toggle_session),
            ("flush", &self.flush),
            ("cancel", &self.cancel),
            ("undo_last", &self.undo_last),
            ("toggle_hud", &self.toggle_hud),
            ("history", &self.history),
        ]
    }
}

/// What a session will and will not accept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapturePolicy {
    /// Skip a fragment already present in this session.
    pub dedupe: bool,
    /// Accept images off the clipboard. They cannot join a text block, so they
    /// render through the template placeholder.
    pub capture_images: bool,
    /// Accept copied file lists.
    pub capture_files: bool,
    /// Hard cap on stack size. Zero means no limit.
    pub max_items: usize,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self { dedupe: true, capture_images: true, capture_files: true, max_items: 0 }
    }
}

/// How the app behaves around a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Behavior {
    /// Leave the stack intact after a flush so the same block can be pasted
    /// again. Off by default: flush reads as commit.
    pub keep_open_after_flush: bool,
    /// Send the paste keystroke after putting the block on the clipboard. With
    /// this off, the block is on the clipboard and the user presses paste.
    pub auto_paste: bool,
    /// Put back whatever was on the clipboard before the session, once the
    /// paste has landed.
    pub restore_clipboard_after_flush: bool,
    /// Take whatever is already on the clipboard as the first item when a
    /// session opens. Off by default: it usually grabs something stale.
    pub capture_clipboard_on_start: bool,
    /// Clipboard poll period while a session is open, in milliseconds.
    pub poll_interval_ms: u64,
    /// Milliseconds to wait between setting the clipboard and sending the
    /// paste keystroke. Some applications read the clipboard asynchronously
    /// and will paste the previous value if this is too tight.
    pub paste_delay_ms: u64,
    /// Show the floating stack window while a session is open.
    pub show_hud: bool,
    pub hud_corner: HudCorner,
    /// Record the window title a fragment came from, for `{source}`.
    pub track_source_window: bool,
    /// Launch on login. On by default: a clipboard tool that is not
    /// running when you reach for it is worse than no clipboard tool,
    /// and the first-run tour offers to turn it off.
    pub autostart: bool,
    /// Whether the pasted block carries formatting and images, or is text
    /// and text alone.
    pub paste_format: PasteFormat,
    /// How many finished sessions to keep for replaying.
    pub history_limit: usize,
    /// Archive a session when it is pasted. Off means nothing is kept.
    pub keep_history: bool,
    /// Look for a new version on startup, and offer it rather than
    /// installing it behind the user's back.
    pub check_for_updates: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            keep_open_after_flush: false,
            auto_paste: true,
            restore_clipboard_after_flush: true,
            capture_clipboard_on_start: false,
            poll_interval_ms: 180,
            paste_delay_ms: 120,
            show_hud: true,
            hud_corner: HudCorner::default(),
            track_source_window: true,
            autostart: true,
            paste_format: PasteFormat::Rich,
            history_limit: crate::history::DEFAULT_LIMIT,
            keep_history: true,
            check_for_updates: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub hotkeys: Hotkeys,
    pub capture: CapturePolicy,
    pub template: Template,
    pub behavior: Behavior,
    /// Whether the introduction has been seen. Written the first time
    /// the app starts, so the tour appears once rather than at every
    /// launch.
    #[serde(default)]
    pub onboarded: bool,
}

/// A setting that is wrong in a way the app can survive. Reported to the
/// settings pane rather than raised as an error: a bad accelerator should
/// disable one shortcut, not stop the app from starting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub field: String,
    pub message: String,
}

impl Config {
    /// Parse settings from the text of a config file.
    ///
    /// Reading that file is the caller's job. Keeping it that way is what lets
    /// this whole crate be tested without touching a disk, and stops the model
    /// from having an opinion about where a user's files live.
    pub fn from_json(raw: &str) -> Result<Self, ConfigError> {
        // A byte order mark is not valid JSON, but it is what several Windows
        // editors write by default. Rejecting settings over an invisible
        // prefix would look like the app ignoring them.
        serde_json::from_str(raw.strip_prefix('\u{feff}').unwrap_or(raw))
            .map_err(ConfigError::Parse)
    }

    /// Render settings as the text of a config file.
    pub fn to_json(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self).map_err(ConfigError::Parse)
    }

    /// Settings that are usable but probably not what the user meant.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out = Vec::new();
        let keys = self.hotkeys.all();

        for (i, (name, accel)) in keys.iter().enumerate() {
            if accel.trim().is_empty() {
                out.push(Warning {
                    field: format!("hotkeys.{name}"),
                    message: "No shortcut set, so this action can only be reached from the tray."
                        .into(),
                });
                continue;
            }
            if let Some((other, _)) = keys[..i].iter().find(|(_, a)| a == accel) {
                out.push(Warning {
                    field: format!("hotkeys.{name}"),
                    message: format!("Same shortcut as {other}; only one of them will fire."),
                });
            }
        }

        if self.behavior.poll_interval_ms < 30 {
            out.push(Warning {
                field: "behavior.poll_interval_ms".into(),
                message: "Below 30 ms the watcher burns CPU for no gain.".into(),
            });
        }
        if self.behavior.poll_interval_ms > 1000 {
            out.push(Warning {
                field: "behavior.poll_interval_ms".into(),
                message: "Above 1 s, fast successive copies can be missed.".into(),
            });
        }
        if self.behavior.auto_paste && self.behavior.paste_delay_ms < 30 {
            out.push(Warning {
                field: "behavior.paste_delay_ms".into(),
                message: "Too short: some applications will paste the previous clipboard value."
                    .into(),
            });
        }
        if self.behavior.keep_history && self.behavior.history_limit == 0 {
            out.push(Warning {
                field: "behavior.history_limit".into(),
                message: "Zero means nothing is kept, so history is effectively off.".into(),
            });
        }
        if self.capture.capture_images && !self.behavior.paste_format.is_rich() {
            out.push(Warning {
                field: "behavior.paste_format".into(),
                message: "Plain text cannot carry an image, so captured images will paste as their placeholder."
                    .into(),
            });
        }
        if !self.capture.capture_images && !self.capture.capture_files {
            out.push(Warning {
                field: "capture".into(),
                message: "Only text will be captured.".into(),
            });
        }
        out
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Parse(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Parse(e) => write!(f, "settings are not valid: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_clean() {
        assert!(Config::default().warnings().is_empty());
    }

    #[test]
    fn defaults_survive_a_round_trip() {
        let c = Config::default();
        let raw = serde_json::to_string(&c).expect("serialises");
        let back: Config = serde_json::from_str(&raw).expect("deserialises");
        assert_eq!(c, back);
    }

    #[test]
    fn a_partial_file_fills_in_the_rest() {
        let raw = r#"{"behavior":{"auto_paste":false}}"#;
        let c: Config = serde_json::from_str(raw).expect("deserialises");
        assert!(!c.behavior.auto_paste);
        assert_eq!(c.behavior.poll_interval_ms, Behavior::default().poll_interval_ms);
        assert_eq!(c.hotkeys, Hotkeys::default());
    }

    #[test]
    fn unknown_fields_from_a_newer_build_are_ignored() {
        let raw = r#"{"behavior":{"from_the_future":42}}"#;
        let c: Config = serde_json::from_str(raw).expect("deserialises");
        assert_eq!(c.behavior, Behavior::default());
    }

    #[test]
    fn colliding_hotkeys_are_reported_once() {
        let mut c = Config::default();
        c.hotkeys.flush = c.hotkeys.toggle_session.clone();
        let w = c.warnings();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].field, "hotkeys.flush");
    }

    #[test]
    fn empty_hotkey_is_a_warning_not_a_collision() {
        let mut c = Config::default();
        c.hotkeys.cancel = String::new();
        c.hotkeys.undo_last = String::new();
        let w = c.warnings();
        assert_eq!(w.len(), 2);
        assert!(w.iter().all(|x| x.message.contains("tray")));
    }

    #[test]
    fn every_default_shortcut_is_distinct() {
        let hotkeys = Hotkeys::default();
        let all = hotkeys.all();
        for (i, (name, accel)) in all.iter().enumerate() {
            assert!(!accel.is_empty(), "{name} has no default");
            for (other, previous) in all[..i].iter() {
                assert_ne!(accel, previous, "{name} collides with {other}");
            }
        }
    }

    #[test]
    fn absurd_poll_intervals_are_flagged() {
        let mut c = Config::default();
        c.behavior.poll_interval_ms = 5;
        assert!(c.warnings().iter().any(|w| w.field == "behavior.poll_interval_ms"));
        c.behavior.poll_interval_ms = 5_000;
        assert!(c.warnings().iter().any(|w| w.field == "behavior.poll_interval_ms"));
    }

    #[test]
    fn a_byte_order_mark_does_not_stop_settings_loading() {
        let c = Config::from_json("\u{feff}{\"behavior\":{\"auto_paste\":false}}")
            .expect("loads despite the mark");
        assert!(!c.behavior.auto_paste);
    }

    #[test]
    fn nonsense_is_an_error_not_a_silent_reset() {
        assert!(matches!(Config::from_json("{ not json"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn settings_survive_a_round_trip_through_text() {
        let mut c = Config::default();
        c.template.separator = "\\n\\n".into();
        c.behavior.poll_interval_ms = 250;
        let text = c.to_json().expect("renders");
        assert_eq!(Config::from_json(&text).expect("parses"), c);
    }
}
