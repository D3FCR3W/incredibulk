//! The session: an append-only clipboard with an explicit begin and commit.

use serde::{Deserialize, Serialize};

use crate::config::CapturePolicy;
use crate::item::{ClipItem, ClipKind, now_ms};
use crate::template::{Template, render};

/// What happened to a capture the watcher handed us.
///
/// Every rejection is named so the UI can say *why* nothing was added instead
/// of silently dropping the fragment. A user who copies something and sees the
/// counter stay put has to be told which rule ate it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CaptureOutcome {
    /// Added to the stack.
    Appended { id: u64, count: usize },
    /// Identical content already in this session, and dedupe is on.
    Duplicate,
    /// Empty or whitespace-only.
    Blank,
    /// This kind of content is switched off in the capture policy.
    Filtered { kind: String },
    /// The session is at `max_items`.
    Full { limit: usize },
    /// No session is open.
    Inactive,
}

impl CaptureOutcome {
    pub fn accepted(&self) -> bool {
        matches!(self, CaptureOutcome::Appended { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveDirection {
    Up,
    Down,
}

/// A live capture session. Holds items in capture order.
#[derive(Clone, Debug)]
pub struct Session {
    items: Vec<ClipItem>,
    next_id: u64,
    started_at_ms: u64,
    /// What the user calls this session, if anything.
    ///
    /// Collecting quotes for one email and links for another in the same
    /// afternoon leaves a history of timestamps that all look alike. A name is
    /// the only thing that tells them apart later.
    name: Option<String>,
    /// Snapshotted when the session opened, so changing settings mid-session
    /// cannot retroactively change what was already accepted.
    policy: CapturePolicy,
}

impl Session {
    pub fn start(policy: CapturePolicy, started_at_ms: u64) -> Self {
        Self { items: Vec::new(), next_id: 1, started_at_ms, name: None, policy }
    }

    pub fn items(&self) -> &[ClipItem] {
        &self.items
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Name the session, or clear the name by passing something blank.
    pub fn set_name(&mut self, name: &str) {
        let trimmed = name.trim();
        self.name = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    pub fn policy(&self) -> &CapturePolicy {
        &self.policy
    }

    /// Offer a clipboard value to the session.
    pub fn capture(
        &mut self,
        kind: ClipKind,
        source: Option<String>,
        captured_at_ms: u64,
    ) -> CaptureOutcome {
        let allowed = match &kind {
            ClipKind::Text { .. } => true,
            ClipKind::Files { .. } => self.policy.capture_files,
            ClipKind::Image { .. } => self.policy.capture_images,
        };
        if !allowed {
            return CaptureOutcome::Filtered { kind: kind.type_name().to_string() };
        }
        if kind.is_blank() {
            return CaptureOutcome::Blank;
        }
        if self.policy.max_items > 0 && self.items.len() >= self.policy.max_items {
            return CaptureOutcome::Full { limit: self.policy.max_items };
        }
        if self.policy.dedupe {
            let d = kind.digest();
            if self.items.iter().any(|i| i.kind.digest() == d) {
                return CaptureOutcome::Duplicate;
            }
        }

        let id = self.next_id;
        self.next_id += 1;
        self.items.push(ClipItem::new(id, kind, captured_at_ms, source));
        CaptureOutcome::Appended { id, count: self.items.len() }
    }

    /// Drop one item the user grabbed by mistake.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != before
    }

    /// Drop the most recent capture. Bound to a hotkey so a misfire can be
    /// undone without opening the HUD.
    pub fn undo_last(&mut self) -> Option<ClipItem> {
        self.items.pop()
    }

    /// Reorder within the stack. Out-of-range moves are a no-op, not an error:
    /// the first item cannot go up.
    pub fn move_item(&mut self, id: u64, dir: MoveDirection) -> bool {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else {
            return false;
        };
        let target = match dir {
            MoveDirection::Up if pos > 0 => pos - 1,
            MoveDirection::Down if pos + 1 < self.items.len() => pos + 1,
            _ => return false,
        };
        self.items.swap(pos, target);
        true
    }

    /// Move a fragment to an absolute position in the stack.
    ///
    /// This is what dragging a row produces. Swapping neighbours cannot
    /// express "take this to the top", which is most of what restructuring a
    /// block before pasting actually is.
    pub fn move_to(&mut self, id: u64, index: usize) -> bool {
        let Some(from) = self.items.iter().position(|i| i.id == id) else {
            return false;
        };
        // A drop past the end means the end, not an error.
        let to = index.min(self.items.len() - 1);
        if from == to {
            return false;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        true
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Include or exclude one fragment. Excluded fragments stay in the list
    /// and can be brought back.
    pub fn set_included(&mut self, id: u64, included: bool) -> bool {
        match self.items.iter_mut().find(|i| i.id == id) {
            Some(item) => {
                item.included = included;
                true
            }
            None => false,
        }
    }

    /// Include or exclude everything at once.
    pub fn set_all_included(&mut self, included: bool) {
        for item in &mut self.items {
            item.included = included;
        }
    }

    /// Replace the text of one fragment.
    ///
    /// Editing to nothing removes the fragment rather than leaving a blank
    /// line in the block: clearing the field is how a user says they are
    /// done with it.
    pub fn edit_text(&mut self, id: u64, text: String) -> Result<bool, EditError> {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else {
            return Err(EditError::NoSuchItem);
        };
        if !item.is_editable() {
            return Err(EditError::NotText);
        }
        if text.trim().is_empty() {
            self.remove(id);
            return Ok(false);
        }
        item.edited = item.edited || item.text() != Some(text.as_str());
        item.kind = ClipKind::Text { text };
        Ok(true)
    }

    /// Append a fragment the user typed rather than copied.
    ///
    /// This deliberately skips the dedupe rule: a repeat that someone typed
    /// on purpose is not the accident that rule exists to catch.
    pub fn add_text(&mut self, text: String, captured_at_ms: u64) -> CaptureOutcome {
        if text.trim().is_empty() {
            return CaptureOutcome::Blank;
        }
        if self.policy.max_items > 0 && self.items.len() >= self.policy.max_items {
            return CaptureOutcome::Full { limit: self.policy.max_items };
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut item = ClipItem::new(id, ClipKind::Text { text }, captured_at_ms, None);
        item.edited = true;
        self.items.push(item);
        CaptureOutcome::Appended { id, count: self.items.len() }
    }

    /// The fragments that will actually be pasted, in order.
    pub fn included(&self) -> Vec<&ClipItem> {
        self.items.iter().filter(|i| i.included).collect()
    }

    pub fn included_count(&self) -> usize {
        self.items.iter().filter(|i| i.included).count()
    }

    /// Render the stack into the single block that gets pasted. Excluded
    /// fragments are skipped, and the numbering closes up behind them.
    pub fn render(&self, template: &Template) -> String {
        render(&self.included(), template)
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            active: true,
            name: self.name.clone(),
            started_at_ms: self.started_at_ms,
            count: self.items.len(),
            included: self.included_count(),
            max_items: self.policy.max_items,
            items: {
                // Positions are numbered over the included fragments only, so
                // the list shows the same numbers the pasted block will.
                let mut next = 0usize;
                self.items
                    .iter()
                    .map(|i| {
                        let position = i.included.then(|| {
                            next += 1;
                            next
                        });
                        ItemView::from_item(i, position)
                    })
                    .collect()
            },
        }
    }
}

/// What the session holds, flattened for the UI. Never carries payloads: the
/// HUD shows previews, and image bytes have no business crossing into a
/// webview on every tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub active: bool,
    /// What the user called this session, if anything.
    pub name: Option<String>,
    pub started_at_ms: u64,
    /// Everything captured, whether or not it will be pasted.
    pub count: usize,
    /// How many of those are ticked, which is what a paste will contain.
    pub included: usize,
    pub max_items: usize,
    pub items: Vec<ItemView>,
}

impl SessionSnapshot {
    pub fn idle() -> Self {
        Self {
            active: false,
            name: None,
            started_at_ms: 0,
            count: 0,
            included: 0,
            max_items: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemView {
    pub id: u64,
    pub kind: String,
    pub preview: String,
    pub weight: usize,
    pub captured_at_ms: u64,
    pub source: Option<String>,
    pub included: bool,
    pub edited: bool,
    pub editable: bool,
    /// A small `data:` URI for an image fragment, so the list can show
    /// the picture rather than a line of text describing it.
    pub thumbnail: Option<String>,
    /// Position in the pasted block, or none when the fragment is excluded.
    pub position: Option<usize>,
}

impl ItemView {
    fn from_item(i: &ClipItem, position: Option<usize>) -> Self {
        Self {
            id: i.id,
            kind: i.kind.type_name().to_string(),
            preview: i.kind.preview(120),
            weight: i.kind.weight(),
            captured_at_ms: i.captured_at_ms,
            source: i.source.clone(),
            included: i.included,
            edited: i.edited,
            editable: i.is_editable(),
            thumbnail: match &i.kind {
                ClipKind::Image { image } if !image.thumbnail.is_empty() => {
                    Some(image.thumbnail.clone())
                }
                _ => None,
            },
            position,
        }
    }
}

/// Why an edit could not be applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    NoSuchItem,
    NotText,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NoSuchItem => write!(f, "that fragment is no longer in the session"),
            EditError::NotText => write!(f, "only text fragments can be edited"),
        }
    }
}

impl std::error::Error for EditError {}

/// Owns the at-most-one live session.
#[derive(Clone, Debug, Default)]
pub struct SessionStatus {
    current: Option<Session>,
}

impl SessionStatus {
    pub fn new() -> Self {
        Self { current: None }
    }

    pub fn is_active(&self) -> bool {
        self.current.is_some()
    }

    pub fn session(&self) -> Option<&Session> {
        self.current.as_ref()
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        self.current.as_mut()
    }

    /// Open a session. Restarting over a live one is intentional: the hotkey is
    /// a toggle at the app layer, so reaching here twice means the caller
    /// really wants a fresh stack.
    pub fn start(&mut self, policy: CapturePolicy) -> &mut Session {
        self.current = Some(Session::start(policy, now_ms()));
        self.current.as_mut().expect("just assigned")
    }

    /// Throw the session away without pasting.
    pub fn cancel(&mut self) -> Option<Session> {
        self.current.take()
    }

    /// Render the block to paste. `keep_open` decides whether the same stack
    /// can be pasted again, which is the difference between committing once and
    /// stamping the same block in several places.
    pub fn flush(&mut self, template: &Template, keep_open: bool) -> Option<String> {
        let block = self.current.as_ref()?.render(template);
        if !keep_open {
            self.current = None;
        }
        Some(block)
    }

    pub fn capture(&mut self, kind: ClipKind, source: Option<String>) -> CaptureOutcome {
        match self.current.as_mut() {
            Some(s) => s.capture(kind, source, now_ms()),
            None => CaptureOutcome::Inactive,
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.current.as_ref().map(Session::snapshot).unwrap_or_else(SessionSnapshot::idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ImagePayload;
    use crate::template::Template;

    fn policy() -> CapturePolicy {
        CapturePolicy::default()
    }

    fn sess() -> Session {
        Session::start(policy(), 1_000)
    }

    fn cap(s: &mut Session, text: &str) -> CaptureOutcome {
        s.capture(ClipKind::text(text), None, 1_000)
    }

    #[test]
    fn captures_accumulate_in_order() {
        let mut s = sess();
        cap(&mut s, "one");
        cap(&mut s, "two");
        cap(&mut s, "three");
        assert_eq!(s.len(), 3);
        assert_eq!(s.render(&Template::default()), "one\ntwo\nthree");
    }

    #[test]
    fn blank_captures_are_rejected_not_stored() {
        let mut s = sess();
        assert_eq!(cap(&mut s, "   \n "), CaptureOutcome::Blank);
        assert!(s.is_empty());
    }

    #[test]
    fn dedupe_rejects_repeat_content() {
        let mut s = sess();
        assert!(cap(&mut s, "same").accepted());
        assert_eq!(cap(&mut s, "same"), CaptureOutcome::Duplicate);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn dedupe_off_keeps_repeats() {
        let mut p = policy();
        p.dedupe = false;
        let mut s = Session::start(p, 0);
        cap(&mut s, "same");
        cap(&mut s, "same");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn max_items_caps_the_stack() {
        let mut p = policy();
        p.max_items = 2;
        let mut s = Session::start(p, 0);
        cap(&mut s, "a");
        cap(&mut s, "b");
        assert_eq!(cap(&mut s, "c"), CaptureOutcome::Full { limit: 2 });
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn images_can_be_filtered_out() {
        let mut p = policy();
        p.capture_images = false;
        let mut s = Session::start(p, 0);
        let img = ClipKind::Image { image: ImagePayload::raw(2, 2, vec![0; 16]) };
        assert_eq!(s.capture(img, None, 0), CaptureOutcome::Filtered { kind: "image".to_string() });
    }

    #[test]
    fn undo_last_removes_only_the_newest() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        let dropped = s.undo_last().expect("an item");
        assert_eq!(dropped.kind, ClipKind::text("b"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn ids_are_not_reused_after_removal() {
        let mut s = sess();
        cap(&mut s, "a");
        s.undo_last();
        cap(&mut s, "b");
        assert_eq!(s.items()[0].id, 2);
    }

    #[test]
    fn reordering_moves_items_and_refuses_the_edges() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        assert!(s.move_item(2, MoveDirection::Up));
        assert_eq!(s.render(&Template::default()), "b\na");
        assert!(!s.move_item(2, MoveDirection::Up));
        assert!(!s.move_item(999, MoveDirection::Down));
    }

    #[test]
    fn removing_an_item_frees_its_content_for_dedupe() {
        let mut s = sess();
        cap(&mut s, "a");
        assert_eq!(cap(&mut s, "a"), CaptureOutcome::Duplicate);
        s.remove(1);
        assert!(cap(&mut s, "a").accepted());
    }

    #[test]
    fn capture_without_a_session_is_inactive() {
        let mut st = SessionStatus::new();
        assert_eq!(st.capture(ClipKind::text("x"), None), CaptureOutcome::Inactive);
    }

    #[test]
    fn flush_closes_the_session_by_default() {
        let mut st = SessionStatus::new();
        st.start(policy()).capture(ClipKind::text("x"), None, 0);
        assert_eq!(st.flush(&Template::default(), false).as_deref(), Some("x"));
        assert!(!st.is_active());
        assert_eq!(st.flush(&Template::default(), false), None);
    }

    #[test]
    fn flush_can_keep_the_stack_for_repeat_pastes() {
        let mut st = SessionStatus::new();
        st.start(policy()).capture(ClipKind::text("x"), None, 0);
        assert_eq!(st.flush(&Template::default(), true).as_deref(), Some("x"));
        assert!(st.is_active());
        assert_eq!(st.flush(&Template::default(), true).as_deref(), Some("x"));
    }

    #[test]
    fn flushing_an_empty_session_yields_an_empty_block_not_none() {
        let mut st = SessionStatus::new();
        st.start(policy());
        assert_eq!(st.flush(&Template::default(), false).as_deref(), Some(""));
    }

    #[test]
    fn cancel_discards_without_rendering() {
        let mut st = SessionStatus::new();
        st.start(policy()).capture(ClipKind::text("x"), None, 0);
        assert!(st.cancel().is_some());
        assert!(!st.is_active());
        assert!(st.cancel().is_none());
    }

    #[test]
    fn a_session_starts_unnamed() {
        assert_eq!(sess().name(), None);
    }

    #[test]
    fn naming_a_session_trims_it() {
        let mut s = sess();
        s.set_name("  research notes  ");
        assert_eq!(s.name(), Some("research notes"));
        assert_eq!(s.snapshot().name.as_deref(), Some("research notes"));
    }

    #[test]
    fn a_blank_name_clears_it() {
        let mut s = sess();
        s.set_name("temporary");
        s.set_name("   ");
        assert_eq!(s.name(), None);
    }

    #[test]
    fn move_to_takes_a_fragment_to_the_top() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        cap(&mut s, "c");
        assert!(s.move_to(3, 0));
        assert_eq!(
            s.render(&Template::default()),
            "c
a
b"
        );
    }

    #[test]
    fn move_to_takes_a_fragment_to_the_bottom() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        cap(&mut s, "c");
        assert!(s.move_to(1, 2));
        assert_eq!(
            s.render(&Template::default()),
            "b
c
a"
        );
    }

    #[test]
    fn move_to_past_the_end_lands_at_the_end() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        assert!(s.move_to(1, 99));
        assert_eq!(
            s.render(&Template::default()),
            "b
a"
        );
    }

    #[test]
    fn move_to_the_same_place_changes_nothing() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        assert!(!s.move_to(1, 0));
        assert_eq!(
            s.render(&Template::default()),
            "a
b"
        );
    }

    #[test]
    fn move_to_an_unknown_fragment_is_refused() {
        let mut s = sess();
        cap(&mut s, "a");
        assert!(!s.move_to(99, 0));
    }

    #[test]
    fn moving_an_excluded_fragment_keeps_it_excluded() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        cap(&mut s, "c");
        s.set_included(3, false);
        s.move_to(3, 0);
        let snap = s.snapshot();
        assert!(!snap.items[0].included);
        assert_eq!(snap.items[0].position, None);
        assert_eq!(snap.items[1].position, Some(1));
        assert_eq!(
            s.render(&Template::default()),
            "a
b"
        );
    }

    #[test]
    fn unticked_fragments_stay_in_the_list_but_leave_the_block() {
        let mut s = sess();
        cap(&mut s, "keep");
        cap(&mut s, "drop");
        cap(&mut s, "keep too");
        assert!(s.set_included(2, false));
        assert_eq!(s.len(), 3, "unticking is not deleting");
        assert_eq!(s.included_count(), 2);
        assert_eq!(
            s.render(&Template::default()),
            "keep
keep too"
        );
    }

    #[test]
    fn numbering_closes_up_behind_an_unticked_fragment() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        cap(&mut s, "c");
        s.set_included(2, false);
        let t = Template { item_format: "{index}/{count} {content}".into(), ..Default::default() };
        assert_eq!(
            s.render(&t),
            "1/2 a
2/2 c"
        );
    }

    #[test]
    fn an_unticked_fragment_can_be_brought_back() {
        let mut s = sess();
        cap(&mut s, "a");
        s.set_included(1, false);
        assert_eq!(s.render(&Template::default()), "");
        s.set_included(1, true);
        assert_eq!(s.render(&Template::default()), "a");
    }

    #[test]
    fn set_all_included_covers_the_whole_stack() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        s.set_all_included(false);
        assert_eq!(s.included_count(), 0);
        s.set_all_included(true);
        assert_eq!(s.included_count(), 2);
    }

    #[test]
    fn snapshot_positions_follow_the_pasted_order() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        cap(&mut s, "c");
        s.set_included(1, false);
        let snap = s.snapshot();
        assert_eq!(snap.items[0].position, None);
        assert_eq!(snap.items[1].position, Some(1));
        assert_eq!(snap.items[2].position, Some(2));
        assert_eq!(snap.included, 2);
        assert_eq!(snap.count, 3);
    }

    #[test]
    fn editing_replaces_the_text_and_marks_it() {
        let mut s = sess();
        cap(&mut s, "typo");
        assert_eq!(s.edit_text(1, "fixed".into()), Ok(true));
        assert_eq!(s.render(&Template::default()), "fixed");
        assert!(s.snapshot().items[0].edited);
    }

    #[test]
    fn an_edit_that_changes_nothing_leaves_the_item_unmarked() {
        let mut s = sess();
        cap(&mut s, "same");
        assert_eq!(s.edit_text(1, "same".into()), Ok(true));
        assert!(!s.snapshot().items[0].edited);
    }

    #[test]
    fn editing_back_to_the_original_still_counts_as_edited() {
        let mut s = sess();
        cap(&mut s, "one");
        s.edit_text(1, "two".into()).expect("edits");
        s.edit_text(1, "one".into()).expect("edits");
        assert!(s.snapshot().items[0].edited, "the fragment was still tampered with");
    }

    #[test]
    fn editing_to_nothing_removes_the_fragment() {
        let mut s = sess();
        cap(&mut s, "a");
        cap(&mut s, "b");
        assert_eq!(s.edit_text(1, "   ".into()), Ok(false));
        assert_eq!(s.len(), 1);
        assert_eq!(s.render(&Template::default()), "b");
    }

    #[test]
    fn images_and_files_refuse_to_be_edited() {
        let mut s = sess();
        let img = ClipKind::Image { image: ImagePayload::raw(2, 2, vec![0; 16]) };
        s.capture(img, None, 0);
        assert_eq!(s.edit_text(1, "text".into()), Err(EditError::NotText));
    }

    #[test]
    fn editing_a_gone_fragment_says_so() {
        let mut s = sess();
        assert_eq!(s.edit_text(99, "x".into()), Err(EditError::NoSuchItem));
    }

    #[test]
    fn a_typed_line_joins_the_stack() {
        let mut s = sess();
        cap(&mut s, "copied");
        assert!(s.add_text("typed".into(), 0).accepted());
        assert_eq!(
            s.render(&Template::default()),
            "copied
typed"
        );
        assert!(s.snapshot().items[1].edited);
    }

    #[test]
    fn a_typed_line_may_repeat_something_already_there() {
        let mut s = sess();
        cap(&mut s, "same");
        assert_eq!(cap(&mut s, "same"), CaptureOutcome::Duplicate);
        assert!(s.add_text("same".into(), 0).accepted(), "typing it is deliberate");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn a_blank_typed_line_is_refused() {
        let mut s = sess();
        assert_eq!(
            s.add_text(
                "  
 "
                .into(),
                0
            ),
            CaptureOutcome::Blank
        );
        assert!(s.is_empty());
    }

    #[test]
    fn typed_lines_respect_the_item_limit() {
        let mut p = policy();
        p.max_items = 1;
        let mut s = Session::start(p, 0);
        cap(&mut s, "a");
        assert_eq!(s.add_text("b".into(), 0), CaptureOutcome::Full { limit: 1 });
    }

    #[test]
    fn a_typed_line_gets_a_fresh_id() {
        let mut s = sess();
        cap(&mut s, "a");
        s.add_text("b".into(), 0);
        let ids: Vec<u64> = s.items().iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn snapshot_reports_counts_without_payloads() {
        let mut st = SessionStatus::new();
        assert!(!st.snapshot().active);
        st.start(policy()).capture(ClipKind::text("hello world"), None, 0);
        let snap = st.snapshot();
        assert!(snap.active);
        assert_eq!(snap.count, 1);
        assert_eq!(snap.items[0].preview, "hello world");
    }
}
