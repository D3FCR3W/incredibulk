//! Turning a stack of captures into the one block that gets pasted.
//!
//! The whole point of a session is the shape of its output, so the format is a
//! template rather than a hardcoded join. Templates are plain strings with
//! `{token}` placeholders; an unknown token is left alone rather than dropped,
//! because silently eating text the user typed is worse than showing it back.

use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::item::{ClipItem, ClipKind};

/// How a session renders. Every field is user-editable from the settings pane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Template {
    /// Emitted once before the first item. Tokens: `{count}`, `{time}`, `{date}`.
    pub prefix: String,
    /// Emitted once per item. Tokens: `{content}`, `{index}`, `{index0}`,
    /// `{count}`, `{source}`, `{kind}`, `{time}`, `{date}`.
    pub item_format: String,
    /// Placed between items. Escapes `\n`, `\t`, `\r` are honoured so the field
    /// stays usable in a single-line text input.
    pub separator: String,
    /// Emitted once after the last item. Same tokens as `prefix`.
    pub suffix: String,
    /// Trim leading and trailing whitespace off each fragment before it is
    /// placed in `item_format`. On by default: selections routinely carry a
    /// stray newline that would otherwise fight the separator.
    pub trim_items: bool,
    /// Stand-in for an image capture, which cannot be pasted into a text block.
    /// Tokens: `{width}`, `{height}`.
    pub image_format: String,
    /// Rendering of one path in a file capture. Tokens: `{path}`, `{name}`,
    /// `{stem}`, `{ext}`, `{dir}`.
    pub file_format: String,
    /// Placed between paths inside a single file capture.
    pub file_separator: String,
    /// strftime pattern behind `{time}`.
    pub time_format: String,
    /// strftime pattern behind `{date}`.
    pub date_format: String,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            item_format: "{content}".into(),
            separator: "\\n".into(),
            suffix: String::new(),
            trim_items: true,
            image_format: "[image {width}x{height}]".into(),
            file_format: "{path}".into(),
            file_separator: "\\n".into(),
            time_format: "%H:%M:%S".into(),
            date_format: "%Y-%m-%d".into(),
        }
    }
}

/// A named starting point offered in the settings pane. Users who want their
/// own shape edit the fields; these cover the common ones without anyone
/// having to learn the token vocabulary first.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub description: String,
    pub template: Template,
}

pub fn presets() -> Vec<Preset> {
    let base = Template::default;
    vec![
        Preset {
            id: "lines".into(),
            label: "One per line".into(),
            description: "Fragments joined by a single newline.".into(),
            template: base(),
        },
        Preset {
            id: "paragraphs".into(),
            label: "Blank line between".into(),
            description: "For multi-line snippets that need to breathe.".into(),
            template: Template { separator: "\\n\\n".into(), ..base() },
        },
        Preset {
            id: "numbered".into(),
            label: "Numbered list".into(),
            description: "1. first, 2. second, and so on.".into(),
            template: Template { item_format: "{index}. {content}".into(), ..base() },
        },
        Preset {
            id: "bullets".into(),
            label: "Markdown bullets".into(),
            description: "A dash in front of every fragment.".into(),
            template: Template { item_format: "- {content}".into(), ..base() },
        },
        Preset {
            id: "sourced".into(),
            label: "With source and time".into(),
            description: "Each fragment labelled with where and when it came from.".into(),
            template: Template {
                item_format: "[{index}] {source} at {time}\\n{content}".into(),
                separator: "\\n\\n".into(),
                ..base()
            },
        },
        Preset {
            id: "csv_cells".into(),
            label: "Comma separated".into(),
            description: "Fragments on one line, joined by a comma.".into(),
            template: Template { separator: ", ".into(), ..base() },
        },
        Preset {
            id: "markdown_quote".into(),
            label: "Markdown quote block".into(),
            description: "Whole stack wrapped in a fenced block.".into(),
            template: Template {
                prefix: "```\\n".into(),
                suffix: "\\n```".into(),
                separator: "\\n".into(),
                ..base()
            },
        },
    ]
}

/// Render a whole stack.
///
/// Takes references so a caller can filter out the fragments the user has
/// unticked without copying the ones that remain; an image fragment carries
/// its whole bitmap, and the settings preview re-renders on every keystroke.
pub fn render(items: &[&ClipItem], t: &Template) -> String {
    let count = items.len();
    let sep = unescape(&t.separator);
    let now = Local::now();

    let body = items
        .iter()
        .enumerate()
        .map(|(idx, item)| render_item(item, idx, count, t))
        .collect::<Vec<_>>()
        .join(&sep);

    let head = expand(&unescape(&t.prefix), |token| bookend_token(token, count, &now, t));
    let tail = expand(&unescape(&t.suffix), |token| bookend_token(token, count, &now, t));

    format!("{head}{body}{tail}")
}

fn render_item(item: &ClipItem, idx: usize, count: usize, t: &Template) -> String {
    let content = content_of(item, t);
    let stamp = stamp_of(item);
    expand(&unescape(&t.item_format), |token| match token {
        "content" => Some(content.clone()),
        "index" => Some((idx + 1).to_string()),
        "index0" => Some(idx.to_string()),
        "count" => Some(count.to_string()),
        "kind" => Some(item.kind.type_name().to_string()),
        "source" => Some(item.source.clone().unwrap_or_default()),
        "time" => Some(stamp.format(&t.time_format).to_string()),
        "date" => Some(stamp.format(&t.date_format).to_string()),
        _ => None,
    })
}

pub(crate) fn bookend_token(
    token: &str,
    count: usize,
    now: &DateTime<Local>,
    t: &Template,
) -> Option<String> {
    match token {
        "count" => Some(count.to_string()),
        "time" => Some(now.format(&t.time_format).to_string()),
        "date" => Some(now.format(&t.date_format).to_string()),
        _ => None,
    }
}

pub(crate) fn stamp_of(item: &ClipItem) -> DateTime<Local> {
    Local.timestamp_millis_opt(item.captured_at_ms as i64).single().unwrap_or_else(Local::now)
}

pub(crate) fn content_of(item: &ClipItem, t: &Template) -> String {
    match &item.kind {
        ClipKind::Text { text } => {
            if t.trim_items {
                text.trim().to_string()
            } else {
                text.clone()
            }
        }
        ClipKind::Image { image } => expand(&unescape(&t.image_format), |token| match token {
            "width" => Some(image.width.to_string()),
            "height" => Some(image.height.to_string()),
            _ => None,
        }),
        ClipKind::Files { paths } => {
            let fsep = unescape(&t.file_separator);
            paths
                .iter()
                .map(|p| {
                    expand(&unescape(&t.file_format), |token| {
                        let s = match token {
                            "path" => p.to_string_lossy().into_owned(),
                            "name" => p
                                .file_name()
                                .map(|v| v.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            "stem" => p
                                .file_stem()
                                .map(|v| v.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            "ext" => p
                                .extension()
                                .map(|v| v.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            "dir" => p
                                .parent()
                                .map(|v| v.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            _ => return None,
                        };
                        Some(s)
                    })
                })
                .collect::<Vec<_>>()
                .join(&fsep)
        }
    }
}

/// Substitute `{token}` occurrences using `lookup`.
///
/// Done as a single left-to-right scan rather than chained `str::replace` so
/// that a fragment containing something like `{index}` is never re-expanded.
/// `{{` and `}}` are literal braces; an unresolved token is passed through
/// untouched.
fn expand<F>(fmt: &str, lookup: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(fmt.len());
    let mut rest = fmt;

    while let Some(open) = rest.find(['{', '}']) {
        let (before, tail) = rest.split_at(open);
        out.push_str(before);
        let mut chars = tail.chars();
        let brace = chars.next().expect("find returned a valid index");
        let after = chars.as_str();

        if after.starts_with(brace) {
            out.push(brace);
            rest = &after[brace.len_utf8()..];
            continue;
        }
        if brace == '}' {
            out.push('}');
            rest = after;
            continue;
        }
        match after.find('}') {
            // A token never contains an opening brace. If one shows up first,
            // this brace was literal text and the real token starts later.
            Some(close) if after[..close].contains('{') => {
                out.push('{');
                rest = after;
            }
            Some(close) => {
                let token = &after[..close];
                match lookup(token) {
                    Some(value) => out.push_str(&value),
                    None => {
                        out.push('{');
                        out.push_str(token);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Turn the two-character escapes a single-line text input can carry into the
/// characters they stand for.
pub(crate) fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ClipKind, ImagePayload};
    use std::path::PathBuf;

    fn refs(items: &[ClipItem]) -> Vec<&ClipItem> {
        items.iter().collect()
    }

    fn items(texts: &[&str]) -> Vec<ClipItem> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| ClipItem::new(i as u64 + 1, ClipKind::text(*t), 0, None))
            .collect()
    }

    #[test]
    fn default_joins_with_newlines() {
        assert_eq!(render(&refs(&items(&["a", "b"])), &Template::default()), "a\nb");
    }

    #[test]
    fn empty_stack_renders_bookends_only() {
        let t = Template { prefix: "<".into(), suffix: ">".into(), ..Default::default() };
        assert_eq!(render(&[], &t), "<>");
    }

    #[test]
    fn separator_escapes_are_honoured() {
        let t = Template { separator: "\\n---\\n".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a", "b"])), &t), "a\n---\nb");
    }

    #[test]
    fn numbering_is_one_based_and_index0_is_not() {
        let t = Template { item_format: "{index}/{index0}/{count}".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a", "b"])), &t), "1/0/2\n2/1/2");
    }

    #[test]
    fn trim_can_be_switched_off() {
        let raw = vec![ClipItem::new(1, ClipKind::text("  x  "), 0, None)];
        assert_eq!(render(&refs(&raw), &Template::default()), "x");
        let t = Template { trim_items: false, ..Default::default() };
        assert_eq!(render(&refs(&raw), &t), "  x  ");
    }

    #[test]
    fn content_containing_a_token_is_never_re_expanded() {
        let raw = vec![ClipItem::new(1, ClipKind::text("literal {index} here"), 0, None)];
        let t = Template { item_format: "{index}: {content}".into(), ..Default::default() };
        assert_eq!(render(&refs(&raw), &t), "1: literal {index} here");
    }

    #[test]
    fn unknown_tokens_survive_verbatim() {
        let t = Template { item_format: "{nope} {content}".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a"])), &t), "{nope} a");
    }

    #[test]
    fn doubled_braces_are_literal() {
        let t = Template { item_format: "{{{content}}}".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a"])), &t), "{a}");
    }

    #[test]
    fn unterminated_brace_is_passed_through() {
        let t = Template { item_format: "{oops {content}".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a"])), &t), "{oops a");
    }

    #[test]
    fn missing_source_renders_empty_not_none() {
        let t = Template { item_format: "[{source}]{content}".into(), ..Default::default() };
        assert_eq!(render(&refs(&items(&["a"])), &t), "[]a");
    }

    #[test]
    fn images_render_through_their_placeholder() {
        let raw = vec![ClipItem::new(
            1,
            ClipKind::Image { image: ImagePayload::raw(800, 600, vec![]) },
            0,
            None,
        )];
        assert_eq!(render(&refs(&raw), &Template::default()), "[image 800x600]");
    }

    #[test]
    fn file_captures_expand_every_path_part() {
        let raw = vec![ClipItem::new(
            1,
            ClipKind::Files { paths: vec![PathBuf::from("/tmp/notes.md")] },
            0,
            None,
        )];
        let t = Template { file_format: "{name} ({ext}) in {dir}".into(), ..Default::default() };
        assert_eq!(render(&refs(&raw), &t), "notes.md (md) in /tmp");
    }

    #[test]
    fn multiple_files_in_one_capture_use_the_file_separator() {
        let raw = vec![ClipItem::new(
            1,
            ClipKind::Files { paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")] },
            0,
            None,
        )];
        let t = Template { file_separator: " | ".into(), ..Default::default() };
        assert_eq!(render(&refs(&raw), &t), "a.txt | b.txt");
    }

    #[test]
    fn prefix_and_suffix_see_the_count() {
        let t = Template {
            prefix: "{count} items:\\n".into(),
            suffix: "\\n(end)".into(),
            ..Default::default()
        };
        assert_eq!(render(&refs(&items(&["a", "b"])), &t), "2 items:\na\nb\n(end)");
    }

    #[test]
    fn every_preset_renders_without_leftover_tokens() {
        for p in presets() {
            let out = render(&refs(&items(&["a", "b"])), &p.template);
            assert!(!out.contains("{content}"), "preset {} left a token", p.id);
            assert!(out.contains('a') && out.contains('b'), "preset {} lost content", p.id);
        }
    }

    #[test]
    fn unescape_leaves_unknown_escapes_alone() {
        assert_eq!(unescape("a\\qb"), "a\\qb");
        assert_eq!(unescape("a\\\\b"), "a\\b");
        assert_eq!(unescape("trailing\\"), "trailing\\");
    }
}
