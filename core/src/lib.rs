//! Core model for incredibulk, the accumulating clipboard.
//!
//! The clipboard is normally a register of size one: every copy destroys the
//! previous one. A session turns it into an append-only stack with an explicit
//! begin and commit. Open a session, copy as many fragments as you like from
//! anywhere, then flush them into the focused window as a single rendered
//! block.
//!
//! Everything here is pure: no clipboard, no OS, no threads. The `incredibulk`
//! binary owns those and drives this model.

pub mod b64;
pub mod config;
pub mod history;
pub mod html;
pub mod item;
pub mod session;
pub mod template;

pub use config::{
    Behavior, CapturePolicy, Config, ConfigError, Hotkeys, HudCorner, PasteFormat, Warning,
};
pub use history::{History, HistoryEntry, HistoryView, replay_target};
pub use html::render_html;
pub use item::{ClipItem, ClipKind, ImagePayload, now_ms};
pub use session::{
    CaptureOutcome, ItemView, MoveDirection, Session, SessionSnapshot, SessionStatus,
};
pub use template::{Preset, Template, presets, render};
