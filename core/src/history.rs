//! Past sessions, kept so a paste can be replayed.
//!
//! A session ends the moment it is pasted, and until now that was the end of
//! it. Collecting eight fragments and then wanting them again an hour later is
//! ordinary, and re-collecting them by hand is exactly the work this tool
//! exists to remove. So a flushed session is archived, and several archived
//! sessions can be ticked and pasted as one block.
//!
//! The cap is on entries, not bytes, with a separate ceiling on how large an
//! image may be before only its description is kept. An unbounded history of
//! screenshots would quietly become the largest file in the user's profile.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::item::{ClipItem, ClipKind, now_ms};
use crate::template::{Template, render};

/// How many past sessions to keep when nothing says otherwise.
pub const DEFAULT_LIMIT: usize = 50;

/// Beyond this, an archived image keeps its dimensions but not its pixels.
/// A replayed paste then describes it instead of showing it, which is a better
/// trade than a history file measured in gigabytes.
pub const MAX_ARCHIVED_IMAGE_BYTES: usize = 4 * 1024 * 1024;

/// One finished session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u64,
    pub ended_at_ms: u64,
    /// What the session was called. Sessions are told apart by name long
    /// before they are told apart by the minute they ended.
    #[serde(default)]
    pub name: Option<String>,
    pub items: Vec<ClipItem>,
}

impl HistoryEntry {
    /// Fragments that were actually pasted. Unticked ones are not archived:
    /// the entry records what happened, not what was considered.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    entries: VecDeque<HistoryEntry>,
    next_id: u64,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self { entries: VecDeque::new(), next_id: 1, limit: DEFAULT_LIMIT }
    }
}

impl History {
    pub fn with_limit(limit: usize) -> Self {
        Self { limit: limit.max(1), ..Default::default() }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        self.trim();
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Newest first, which is the order anyone looking for "the one I just
    /// did" expects.
    pub fn entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.entries.iter().rev()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The most recent session, which is what "paste that again" means.
    pub fn latest(&self) -> Option<&HistoryEntry> {
        self.entries.back()
    }

    /// Keep only the ids that still refer to something.
    ///
    /// A selection outlives the list it was made from: entries can be deleted,
    /// or pushed off the end by the cap, while their ids are still ticked. A
    /// stale id is not an error, it is simply no longer part of the selection.
    pub fn retain_existing(&self, ids: &[u64]) -> Vec<u64> {
        self.entries.iter().filter(|e| ids.contains(&e.id)).map(|e| e.id).collect()
    }

    /// Archive a finished session. Returns the new entry's id, or `None` when
    /// there was nothing worth keeping.
    pub fn record(
        &mut self,
        items: Vec<ClipItem>,
        name: Option<String>,
        ended_at_ms: u64,
    ) -> Option<u64> {
        if items.is_empty() {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let items = items.into_iter().map(shrink).collect();
        self.entries.push_back(HistoryEntry { id, ended_at_ms, name, items });
        self.trim();
        Some(id)
    }

    /// Name an archived session, or clear the name by passing something blank.
    ///
    /// Naming after the fact matters as much as naming during: what a batch of
    /// fragments was for is often only obvious once it is finished.
    pub fn rename(&mut self, id: u64, name: &str) -> bool {
        let trimmed = name.trim();
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(entry) => {
                entry.name = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The fragments of the given entries, oldest session first.
    ///
    /// Ticking three sessions and pasting should read the way they happened,
    /// not the way the list happens to be sorted, so this ignores the order
    /// the ids arrive in.
    pub fn combined(&self, ids: &[u64]) -> Vec<&ClipItem> {
        self.entries.iter().filter(|e| ids.contains(&e.id)).flat_map(|e| e.items.iter()).collect()
    }

    pub fn render(&self, ids: &[u64], template: &Template) -> String {
        render(&self.combined(ids), template)
    }

    /// A short description of each entry, for the list.
    pub fn views(&self) -> Vec<HistoryView> {
        self.entries().map(HistoryView::of).collect()
    }

    /// Match every search word against names, complete text, paths and sources.
    /// Return only IDs: searching must not ship clipboard payloads to the UI.
    pub fn search(&self, query: &str) -> Vec<u64> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.entries()
            .filter(|entry| {
                if words.is_empty() {
                    return true;
                }
                let mut fields = vec![entry.name.as_deref().unwrap_or_default().to_lowercase()];
                for item in &entry.items {
                    if let Some(source) = &item.source {
                        fields.push(source.to_lowercase());
                    }
                    match &item.kind {
                        ClipKind::Text { text } => fields.push(text.to_lowercase()),
                        ClipKind::Files { paths } => {
                            fields.extend(paths.iter().map(|p| p.to_string_lossy().to_lowercase()));
                        }
                        ClipKind::Image { image } => {
                            fields.push(format!("image {}x{}", image.width, image.height));
                        }
                    }
                }
                words.iter().all(|word| fields.iter().any(|field| field.contains(word)))
            })
            .map(|entry| entry.id)
            .collect()
    }

    fn trim(&mut self) {
        while self.entries.len() > self.limit {
            self.entries.pop_front();
        }
    }

    /// Parse a history file.
    ///
    /// History is a convenience, not a document: text that cannot be read
    /// yields an empty history rather than an error nobody could act on.
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw.strip_prefix('\u{feff}').unwrap_or(raw)).unwrap_or_default()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Drop what is too big to keep, and the raw pixels either way: only the PNG
/// is ever needed again.
fn shrink(mut item: ClipItem) -> ClipItem {
    if let ClipKind::Image { image } = &mut item.kind {
        image.rgba = Vec::new();
        if image.png.len() > MAX_ARCHIVED_IMAGE_BYTES {
            image.png = Vec::new();
            image.thumbnail = String::new();
        }
    }
    item
}

/// One row of the history list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryView {
    pub id: u64,
    pub ended_at_ms: u64,
    pub name: Option<String>,
    pub count: usize,
    /// First line or so of the first fragment, to recognise the session by.
    pub preview: String,
    /// How many of each kind, so a session of screenshots is obvious at a
    /// glance rather than reading as empty text.
    pub images: usize,
    pub files: usize,
    /// Thumbnail of the first image in the session, if it kept one.
    pub thumbnail: Option<String>,
}

impl HistoryView {
    fn of(entry: &HistoryEntry) -> Self {
        let preview =
            entry.items.iter().map(|i| i.kind.preview(60)).collect::<Vec<_>>().join(" · ");

        Self {
            id: entry.id,
            ended_at_ms: entry.ended_at_ms,
            name: entry.name.clone(),
            count: entry.items.len(),
            preview: truncate(&preview, 160),
            images: entry.items.iter().filter(|i| matches!(i.kind, ClipKind::Image { .. })).count(),
            files: entry.items.iter().filter(|i| matches!(i.kind, ClipKind::Files { .. })).count(),
            thumbnail: entry.items.iter().find_map(|i| match &i.kind {
                ClipKind::Image { image } if !image.thumbnail.is_empty() => {
                    Some(image.thumbnail.clone())
                }
                _ => None,
            }),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

/// Which sessions a paste should use when there is no live session to flush.
///
/// The rule in one place, and a pure function, because it is the part that has
/// to be right: it decides what lands in someone's document when they press a
/// key expecting the obvious thing.
///
/// Ticked sessions win. Ticking is an explicit choice made in a list that says
/// what the shortcut will do, so overriding it would make that list a lie.
/// With nothing ticked there is only one sensible answer left, the session
/// that just happened.
pub fn replay_target(history: &History, ticked: &[u64]) -> Vec<u64> {
    let live = history.retain_existing(ticked);
    if !live.is_empty() {
        return live;
    }
    history.latest().map(|e| vec![e.id]).unwrap_or_default()
}

/// Convenience for the common case of archiving right now.
pub fn record_now(
    history: &mut History,
    items: Vec<ClipItem>,
    name: Option<String>,
) -> Option<u64> {
    history.record(items, name, now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ImagePayload;

    fn items(texts: &[&str]) -> Vec<ClipItem> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| ClipItem::new(i as u64 + 1, ClipKind::text(*t), 0, None))
            .collect()
    }

    #[test]
    fn search_reads_beyond_previews_and_matches_words_across_fields() {
        let mut h = History::default();
        let long_text = format!("{} Décision finale", "intro ".repeat(50));
        let mut fragments = items(&[&long_text, "second fragment"]);
        fragments[0].source = Some("Firefox".into());
        h.record(fragments, Some("Projet été".into()), 1);
        h.record(items(&["unrelated"]), None, 2);

        assert!(!h.views()[1].preview.contains("Décision"));
        assert_eq!(h.search("  ÉTÉ\tDÉCISION firefox\nsecond "), vec![1]);
        assert!(h.search("décision missing").is_empty());
        assert_eq!(h.search(" \n "), vec![2, 1]);
        assert_eq!(h.search(""), vec![2, 1]);
    }

    #[test]
    fn search_includes_full_paths_and_image_descriptions_but_not_image_bytes() {
        let mut h = History::default();
        h.record(
            vec![ClipItem::new(
                1,
                ClipKind::Files { paths: vec!["/projects/private/report.pdf".into()] },
                0,
                None,
            )],
            None,
            1,
        );
        let mut image = ImagePayload::raw(640, 480, Vec::new());
        image.png = b"secretpixels".to_vec();
        image.thumbnail = "secretthumbnail".into();
        h.record(vec![ClipItem::new(1, ClipKind::Image { image }, 0, None)], None, 2);

        assert_eq!(h.search("/PROJECTS/PRIVATE PDF"), vec![1]);
        assert_eq!(h.search("IMAGE 640x480"), vec![2]);
        assert!(h.search("secret").is_empty());
    }

    #[test]
    fn search_tracks_renames_deletions_and_persisted_history() {
        let mut h = History::default();
        h.record(items(&["shared"]), Some("old name".into()), 1);
        h.record(items(&["shared"]), None, 2);
        assert_eq!(h.search("shared"), vec![2, 1]);
        h.rename(1, "new name");
        assert!(h.search("old").is_empty());
        let mut loaded = History::from_json(&h.to_json().unwrap());
        assert_eq!(loaded.search("new shared"), vec![1]);
        loaded.remove(1);
        assert!(loaded.search("new").is_empty());
        loaded.clear();
        assert!(loaded.search("").is_empty());
    }

    #[test]
    fn a_flushed_session_is_archived() {
        let mut h = History::default();
        let id = h.record(items(&["a", "b"]), None, 1_000).expect("archived");
        assert_eq!(h.len(), 1);
        assert_eq!(h.get(id).expect("present").len(), 2);
    }

    #[test]
    fn an_empty_session_is_not_worth_keeping() {
        let mut h = History::default();
        assert_eq!(h.record(Vec::new(), None, 0), None);
        assert!(h.is_empty());
    }

    #[test]
    fn the_newest_session_comes_first() {
        let mut h = History::default();
        h.record(items(&["old"]), None, 1);
        h.record(items(&["new"]), None, 2);
        let ids: Vec<u64> = h.entries().map(|e| e.id).collect();
        assert_eq!(ids, vec![2, 1]);
    }

    #[test]
    fn the_oldest_session_falls_off_the_end() {
        let mut h = History::with_limit(3);
        for i in 0..5 {
            h.record(items(&[&format!("session {i}")]), None, i as u64);
        }
        assert_eq!(h.len(), 3);
        let ids: Vec<u64> = h.entries().map(|e| e.id).collect();
        assert_eq!(ids, vec![5, 4, 3], "the three most recent survive");
    }

    #[test]
    fn lowering_the_limit_trims_immediately() {
        let mut h = History::default();
        for i in 0..10 {
            h.record(items(&[&format!("s{i}")]), None, i as u64);
        }
        h.set_limit(4);
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn a_limit_of_zero_is_refused() {
        let mut h = History::with_limit(0);
        assert_eq!(h.limit(), 1);
        h.set_limit(0);
        assert_eq!(h.limit(), 1);
    }

    #[test]
    fn ids_are_not_reused_after_the_cap_drops_entries() {
        let mut h = History::with_limit(2);
        h.record(items(&["a"]), None, 0);
        h.record(items(&["b"]), None, 0);
        let third = h.record(items(&["c"]), None, 0).expect("archived");
        assert_eq!(third, 3);
        assert!(h.get(1).is_none(), "the first was dropped");
    }

    #[test]
    fn the_latest_session_is_the_newest_one() {
        let mut h = History::default();
        assert!(h.latest().is_none());
        h.record(items(&["old"]), None, 1);
        let newest = h.record(items(&["new"]), None, 2).expect("archived");
        assert_eq!(h.latest().map(|e| e.id), Some(newest));
    }

    #[test]
    fn the_latest_session_follows_a_deletion() {
        let mut h = History::default();
        let first = h.record(items(&["a"]), None, 1).expect("archived");
        let second = h.record(items(&["b"]), None, 2).expect("archived");
        h.remove(second);
        assert_eq!(h.latest().map(|e| e.id), Some(first));
    }

    #[test]
    fn a_selection_drops_ids_that_are_gone() {
        let mut h = History::default();
        let a = h.record(items(&["a"]), None, 1).expect("archived");
        let b = h.record(items(&["b"]), None, 2).expect("archived");
        h.remove(a);
        assert_eq!(h.retain_existing(&[a, b, 999]), vec![b]);
    }

    #[test]
    fn a_selection_comes_back_in_the_order_the_sessions_happened() {
        let mut h = History::default();
        let a = h.record(items(&["a"]), None, 1).expect("archived");
        let b = h.record(items(&["b"]), None, 2).expect("archived");
        // Ticked newest first, but the block still reads oldest first.
        assert_eq!(h.retain_existing(&[b, a]), vec![a, b]);
    }

    #[test]
    fn an_empty_selection_stays_empty() {
        let mut h = History::default();
        h.record(items(&["a"]), None, 1);
        assert!(h.retain_existing(&[]).is_empty());
    }

    #[test]
    fn with_nothing_ticked_a_paste_repeats_the_newest() {
        let mut h = History::default();
        h.record(items(&["old"]), None, 1);
        let newest = h.record(items(&["new"]), None, 2).expect("archived");
        assert_eq!(replay_target(&h, &[]), vec![newest]);
    }

    #[test]
    fn ticked_sessions_beat_the_newest() {
        let mut h = History::default();
        let first = h.record(items(&["a"]), None, 1).expect("archived");
        h.record(items(&["b"]), None, 2);
        assert_eq!(replay_target(&h, &[first]), vec![first]);
    }

    #[test]
    fn every_ticked_session_is_pasted_oldest_first() {
        let mut h = History::default();
        let a = h.record(items(&["a"]), None, 1).expect("archived");
        let b = h.record(items(&["b"]), None, 2).expect("archived");
        let c = h.record(items(&["c"]), None, 3).expect("archived");
        // Ticked in a jumbled order, pasted in the order they happened.
        assert_eq!(replay_target(&h, &[c, a]), vec![a, c]);
        assert_eq!(h.render(&replay_target(&h, &[c, a]), &Template::default()), "a\nc");
        let _ = b;
    }

    #[test]
    fn ticks_pointing_at_deleted_sessions_fall_back_to_the_newest() {
        let mut h = History::default();
        let gone = h.record(items(&["a"]), None, 1).expect("archived");
        let newest = h.record(items(&["b"]), None, 2).expect("archived");
        h.remove(gone);
        assert_eq!(replay_target(&h, &[gone]), vec![newest]);
    }

    #[test]
    fn a_partly_stale_selection_keeps_what_is_left() {
        let mut h = History::default();
        let gone = h.record(items(&["a"]), None, 1).expect("archived");
        let kept = h.record(items(&["b"]), None, 2).expect("archived");
        h.record(items(&["c"]), None, 3);
        h.remove(gone);
        assert_eq!(replay_target(&h, &[gone, kept]), vec![kept]);
    }

    #[test]
    fn an_empty_history_gives_nothing_to_paste() {
        let h = History::default();
        assert!(replay_target(&h, &[]).is_empty());
        assert!(replay_target(&h, &[1, 2, 3]).is_empty());
    }

    #[test]
    fn removing_one_leaves_the_rest() {
        let mut h = History::default();
        let a = h.record(items(&["a"]), None, 0).expect("archived");
        h.record(items(&["b"]), None, 0);
        assert!(h.remove(a));
        assert!(!h.remove(a));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn replaying_one_session_gives_back_its_block() {
        let mut h = History::default();
        let id = h.record(items(&["one", "two"]), None, 0).expect("archived");
        assert_eq!(h.render(&[id], &Template::default()), "one\ntwo");
    }

    #[test]
    fn several_sessions_combine_oldest_first() {
        let mut h = History::default();
        let first = h.record(items(&["a"]), None, 1).expect("archived");
        let second = h.record(items(&["b"]), None, 2).expect("archived");
        // Ticked in either order, the block reads the way they happened.
        assert_eq!(h.render(&[second, first], &Template::default()), "a\nb");
        assert_eq!(h.render(&[first, second], &Template::default()), "a\nb");
    }

    #[test]
    fn combining_renumbers_across_the_whole_block() {
        let mut h = History::default();
        let a = h.record(items(&["a", "b"]), None, 1).expect("archived");
        let b = h.record(items(&["c"]), None, 2).expect("archived");
        let t = Template { item_format: "{index}/{count} {content}".into(), ..Default::default() };
        assert_eq!(h.render(&[a, b], &t), "1/3 a\n2/3 b\n3/3 c");
    }

    #[test]
    fn selecting_nothing_renders_nothing() {
        let mut h = History::default();
        h.record(items(&["a"]), None, 0);
        assert_eq!(h.render(&[], &Template::default()), "");
    }

    #[test]
    fn an_unknown_id_is_skipped_rather_than_failing() {
        let mut h = History::default();
        let id = h.record(items(&["a"]), None, 0).expect("archived");
        assert_eq!(h.render(&[id, 999], &Template::default()), "a");
    }

    #[test]
    fn archived_images_drop_their_pixels_but_keep_the_png() {
        let mut image = ImagePayload::raw(4, 4, vec![7; 64]);
        image.png = vec![1, 2, 3];
        let item = ClipItem::new(1, ClipKind::Image { image }, 0, None);

        let mut h = History::default();
        let id = h.record(vec![item], None, 0).expect("archived");
        let stored = h.get(id).expect("present");
        match &stored.items[0].kind {
            ClipKind::Image { image } => {
                assert!(image.rgba.is_empty(), "raw pixels are never archived");
                assert_eq!(image.png, vec![1, 2, 3]);
            }
            _ => panic!("expected an image"),
        }
    }

    #[test]
    fn an_oversized_image_keeps_only_its_description() {
        let mut image = ImagePayload::raw(4, 4, Vec::new());
        image.png = vec![0; MAX_ARCHIVED_IMAGE_BYTES + 1];
        image.thumbnail = "data:image/png;base64,AA".into();
        let item = ClipItem::new(1, ClipKind::Image { image }, 0, None);

        let mut h = History::default();
        let id = h.record(vec![item], None, 0).expect("archived");
        match &h.get(id).expect("present").items[0].kind {
            ClipKind::Image { image } => {
                assert!(image.png.is_empty());
                assert!(image.thumbnail.is_empty());
                assert_eq!((image.width, image.height), (4, 4), "it still says what it was");
            }
            _ => panic!("expected an image"),
        }
    }

    #[test]
    fn views_describe_what_is_in_each_session() {
        let mut h = History::default();
        h.record(items(&["hello", "world"]), None, 1_700_000_000_000);
        let views = h.views();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].count, 2);
        assert!(views[0].preview.contains("hello"));
        assert_eq!(views[0].images, 0);
    }

    #[test]
    fn a_history_survives_a_round_trip_through_text() {
        let mut h = History::with_limit(7);
        h.record(items(&["kept"]), None, 42);

        let back = History::from_json(&h.to_json().expect("renders"));
        assert_eq!(back.len(), 1);
        assert_eq!(back.limit(), 7);
        assert_eq!(back.render(&[1], &Template::default()), "kept");
    }

    #[test]
    fn image_bytes_survive_the_round_trip() {
        let mut image = ImagePayload::raw(2, 2, Vec::new());
        image.png = vec![137, 80, 78, 71];
        let item = ClipItem::new(1, ClipKind::Image { image }, 0, None);

        let mut h = History::default();
        h.record(vec![item], None, 0);

        let back = History::from_json(&h.to_json().expect("renders"));
        match &back.get(1).expect("present").items[0].kind {
            ClipKind::Image { image } => assert_eq!(image.png, vec![137, 80, 78, 71]),
            _ => panic!("expected an image"),
        }
    }

    #[test]
    fn corrupt_history_text_starts_empty_rather_than_failing() {
        assert!(History::from_json("{ not json").is_empty());
        assert!(History::from_json("").is_empty());
    }

    #[test]
    fn a_byte_order_mark_does_not_stop_history_loading() {
        let mut h = History::default();
        h.record(items(&["kept"]), None, 1);
        let text = format!("\u{feff}{}", h.to_json().expect("renders"));
        assert_eq!(History::from_json(&text).len(), 1);
    }
}
