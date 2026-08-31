//! What a single capture is.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Milliseconds since the unix epoch. The model never calls the clock itself
/// except here, so tests can inject their own timestamps.
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// An image lifted off the clipboard, in the three shapes it is needed in.
///
/// None of it is ever logged: it is the user's screen content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePayload {
    pub width: usize,
    pub height: usize,
    /// Raw pixels, for putting the image back on the clipboard. Never stored:
    /// a screenshot is tens of megabytes uncompressed and the PNG below says
    /// the same thing.
    #[serde(skip)]
    pub rgba: Vec<u8>,
    /// PNG encoding. This is what a rich paste embeds and what the history
    /// file keeps, so an image survives a restart.
    #[serde(default, with = "crate::b64::bytes")]
    pub png: Vec<u8>,
    /// A small `data:` URI for the list. Computed once at capture rather than
    /// per redraw, because the list redraws on every single copy.
    #[serde(default)]
    pub thumbnail: String,
}

impl ImagePayload {
    /// A payload with pixels but nothing derived from them yet. The app layer
    /// fills in `png` and `thumbnail`, which need an encoder the model has no
    /// business carrying.
    pub fn raw(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        Self { width, height, rgba, png: Vec::new(), thumbnail: String::new() }
    }

    /// Whether this image can appear in a rich paste. Without the PNG it can
    /// only be described, not shown.
    pub fn is_embeddable(&self) -> bool {
        !self.png.is_empty()
    }

    pub fn data_uri(&self) -> Option<String> {
        if self.png.is_empty() {
            return None;
        }
        Some(format!("data:image/png;base64,{}", crate::b64::encode(&self.png)))
    }
}

/// The three things a clipboard can hold that we know how to accumulate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClipKind {
    Text { text: String },
    Files { paths: Vec<PathBuf> },
    Image { image: ImagePayload },
}

impl ClipKind {
    pub fn text(s: impl Into<String>) -> Self {
        ClipKind::Text { text: s.into() }
    }

    /// True when this capture carries nothing worth keeping. Whitespace-only
    /// text counts as empty: it is almost always a stray selection.
    pub fn is_blank(&self) -> bool {
        match self {
            ClipKind::Text { text } => text.trim().is_empty(),
            ClipKind::Files { paths } => paths.is_empty(),
            ClipKind::Image { image } => image.width == 0 || image.height == 0,
        }
    }

    /// Content hash, used both for dedupe inside a session and to tell a real
    /// clipboard change from the same value being re-announced by the OS.
    pub fn digest(&self) -> u64 {
        let mut h = DefaultHasher::new();
        match self {
            ClipKind::Text { text } => {
                0u8.hash(&mut h);
                text.hash(&mut h);
            }
            ClipKind::Files { paths } => {
                1u8.hash(&mut h);
                paths.hash(&mut h);
            }
            ClipKind::Image { image } => {
                2u8.hash(&mut h);
                image.width.hash(&mut h);
                image.height.hash(&mut h);
                image.rgba.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Short single-line label for the HUD list. Never the full payload.
    pub fn preview(&self, max_chars: usize) -> String {
        let raw = match self {
            ClipKind::Text { text } => text.trim().replace(['\n', '\r', '\t'], " "),
            ClipKind::Files { paths } => paths
                .iter()
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.to_string_lossy().into_owned())
                })
                .collect::<Vec<_>>()
                .join(", "),
            ClipKind::Image { image } => {
                return format!("Image {}x{}", image.width, image.height);
            }
        };
        let collapsed = collapse_spaces(&raw);
        truncate_chars(&collapsed, max_chars)
    }

    /// Rough size indicator shown next to each row.
    pub fn weight(&self) -> usize {
        match self {
            ClipKind::Text { text } => text.chars().count(),
            ClipKind::Files { paths } => paths.len(),
            ClipKind::Image { image } => image.width * image.height,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            ClipKind::Text { .. } => "text",
            ClipKind::Files { .. } => "files",
            ClipKind::Image { .. } => "image",
        }
    }
}

/// One captured fragment, in the order it was grabbed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipItem {
    pub id: u64,
    pub kind: ClipKind,
    pub captured_at_ms: u64,
    /// Window or application the fragment came from, when the platform will
    /// tell us. Purely informational, available to the template as `{source}`.
    pub source: Option<String>,
    /// Whether this fragment goes into the block.
    ///
    /// Unticking is not deleting. Half of what you grab in a real session is
    /// a maybe, and a maybe you can put back is worth far more than a clean
    /// list: the alternative is deciding at capture time, which is exactly
    /// when you do not yet know.
    #[serde(default = "yes")]
    pub included: bool,
    /// True once the text has been changed by hand, so the display can say
    /// that what will be pasted is no longer what was copied.
    #[serde(default)]
    pub edited: bool,
}

fn yes() -> bool {
    true
}

impl ClipItem {
    pub fn new(id: u64, kind: ClipKind, captured_at_ms: u64, source: Option<String>) -> Self {
        Self { id, kind, captured_at_ms, source, included: true, edited: false }
    }

    /// Only text can be edited in place. Replacing an image or a file list
    /// with typed text would quietly change what the item is.
    pub fn is_editable(&self) -> bool {
        matches!(self.kind, ClipKind::Text { .. })
    }

    /// The full text of a text fragment, for handing to an editor.
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            ClipKind::Text { text } => Some(text),
            _ => None,
        }
    }
}

fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        let is_space = c.is_whitespace();
        if is_space && prev_space {
            continue;
        }
        out.push(if is_space { ' ' } else { c });
        prev_space = is_space;
    }
    out.trim().to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_text_is_blank() {
        assert!(ClipKind::text("   \n\t ").is_blank());
        assert!(!ClipKind::text(" x ").is_blank());
    }

    #[test]
    fn digest_separates_kinds_with_equal_payloads() {
        let a = ClipKind::text("a.txt");
        let b = ClipKind::Files { paths: vec![PathBuf::from("a.txt")] };
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn preview_collapses_and_truncates() {
        let k = ClipKind::text("line one\n\n   line two\t\tend");
        assert_eq!(k.preview(80), "line one line two end");
        assert_eq!(k.preview(9).chars().count(), 9);
    }

    #[test]
    fn preview_is_char_safe_on_multibyte() {
        let k = ClipKind::text("éàüñ漢字テスト");
        let p = k.preview(4);
        assert_eq!(p.chars().count(), 4);
    }
}
