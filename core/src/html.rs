//! Rendering the same stack as HTML, so a rich paste can carry the images.
//!
//! A text block cannot contain a picture; it can only describe one. This is
//! the answer to that. Applications that understand HTML get the images
//! inline, and everything else falls back to the plain block, which goes onto
//! the clipboard alongside the markup rather than instead of it.
//!
//! The template still drives the shape. Literal text from the template is
//! escaped, so a separator of `<hr>` appears as those four characters rather
//! than as a rule. Only an image fragment ever becomes markup.

use chrono::Local;

use crate::item::{ClipItem, ClipKind};
use crate::template::{Template, bookend_token, content_of, stamp_of, unescape};

/// Render a stack as an HTML fragment.
pub fn render_html(items: &[&ClipItem], t: &Template) -> String {
    let count = items.len();
    let now = Local::now();
    // Escaped like every other literal part of the template. A separator is
    // text the user typed, not markup they are authoring.
    let separator = breaks(&escape(&unescape(&t.separator)));

    let body = items
        .iter()
        .enumerate()
        .map(|(index, item)| render_item(item, index, count, t))
        .collect::<Vec<_>>()
        .join(&separator);

    let head = breaks(&expand_escaped(&unescape(&t.prefix), |token| {
        bookend_token(token, count, &now, t).map(|v| escape(&v))
    }));
    let tail = breaks(&expand_escaped(&unescape(&t.suffix), |token| {
        bookend_token(token, count, &now, t).map(|v| escape(&v))
    }));

    format!("<div>{head}{body}{tail}</div>")
}

fn render_item(item: &ClipItem, index: usize, count: usize, t: &Template) -> String {
    let content = content_html(item, t);
    let stamp = stamp_of(item);
    let rendered = expand_escaped(&unescape(&t.item_format), |token| match token {
        "content" => Some(content.clone()),
        "index" => Some((index + 1).to_string()),
        "index0" => Some(index.to_string()),
        "count" => Some(count.to_string()),
        "kind" => Some(item.kind.type_name().to_string()),
        "source" => Some(escape(&item.source.clone().unwrap_or_default())),
        "time" => Some(escape(&stamp.format(&t.time_format).to_string())),
        "date" => Some(escape(&stamp.format(&t.date_format).to_string())),
        _ => None,
    });
    breaks(&rendered)
}

fn content_html(item: &ClipItem, t: &Template) -> String {
    match &item.kind {
        ClipKind::Text { text } => {
            let body = if t.trim_items { text.trim() } else { text.as_str() };
            breaks(&escape(body))
        }
        ClipKind::Image { image } => match image.data_uri() {
            // The placeholder becomes the alt text, so an image that will not
            // load still says what it was.
            Some(uri) => format!(
                "<img src=\"{}\" alt=\"{}\" width=\"{}\" height=\"{}\" />",
                uri,
                escape(&content_of(item, t)),
                image.width,
                image.height
            ),
            // No PNG behind it, so there is nothing to show: fall back to the
            // same description the plain block would have used.
            None => escape(&content_of(item, t)),
        },
        ClipKind::Files { .. } => breaks(&escape(&content_of(item, t))),
    }
}

/// Newlines mean nothing in HTML unless they are turned into breaks.
fn breaks(text: &str) -> String {
    let normalised = text.replace("\r\n", "\n");
    normalised.replace('\n', "<br />")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Token substitution where the literal parts of the template are escaped and
/// the substituted values are inserted as they are.
///
/// Callers escape anything they hand back that is not meant to be markup,
/// which is everything except an image.
fn expand_escaped<F>(fmt: &str, lookup: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(fmt.len());
    let mut rest = fmt;

    while let Some(open) = rest.find(['{', '}']) {
        let (before, tail) = rest.split_at(open);
        out.push_str(&escape(before));

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
            // A token never contains an opening brace.
            Some(close) if after[..close].contains('{') => {
                out.push('{');
                rest = after;
            }
            Some(close) => {
                let token = &after[..close];
                match lookup(token) {
                    Some(value) => out.push_str(&value),
                    None => out.push_str(&escape(&format!("{{{token}}}"))),
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(&escape(rest));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ClipKind, ImagePayload};

    fn text_item(id: u64, text: &str) -> ClipItem {
        ClipItem::new(id, ClipKind::text(text), 0, None)
    }

    fn image_item(id: u64, png: Vec<u8>) -> ClipItem {
        let mut image = ImagePayload::raw(40, 30, Vec::new());
        image.png = png;
        ClipItem::new(id, ClipKind::Image { image }, 0, None)
    }

    fn refs(items: &[ClipItem]) -> Vec<&ClipItem> {
        items.iter().collect()
    }

    #[test]
    fn text_is_escaped_so_a_copied_tag_stays_text() {
        let items = vec![text_item(1, "<script>alert(1)</script> & \"quotes\"")];
        let out = render_html(&refs(&items), &Template::default());
        assert!(!out.contains("<script>"), "a copied tag must not become markup");
        assert!(out.contains("&lt;script&gt;"));
        assert!(out.contains("&amp;"));
        assert!(out.contains("&quot;"));
    }

    #[test]
    fn an_image_becomes_an_img_with_its_data_uri() {
        let items = vec![image_item(1, vec![1, 2, 3])];
        let out = render_html(&refs(&items), &Template::default());
        assert!(out.contains("<img src=\"data:image/png;base64,AQID\""));
        assert!(out.contains("width=\"40\""));
        assert!(out.contains("height=\"30\""));
    }

    #[test]
    fn an_image_with_no_png_falls_back_to_the_placeholder() {
        let image = ImagePayload::raw(40, 30, Vec::new());
        let items = vec![ClipItem::new(1, ClipKind::Image { image }, 0, None)];
        let out = render_html(&refs(&items), &Template::default());
        assert!(!out.contains("<img"));
        assert!(out.contains("[image 40x30]"));
    }

    #[test]
    fn the_alt_text_describes_the_image() {
        let items = vec![image_item(1, vec![9])];
        let out = render_html(&refs(&items), &Template::default());
        assert!(out.contains("alt=\"[image 40x30]\""));
    }

    #[test]
    fn newlines_inside_a_fragment_become_breaks() {
        let items = vec![text_item(1, "one\ntwo")];
        let out = render_html(&refs(&items), &Template::default());
        assert!(out.contains("one<br />two"));
    }

    #[test]
    fn the_separator_becomes_a_break_not_a_newline() {
        let items = vec![text_item(1, "a"), text_item(2, "b")];
        let out = render_html(&refs(&items), &Template::default());
        assert!(out.contains("a<br />b"));
    }

    #[test]
    fn markup_typed_into_the_template_is_shown_not_rendered() {
        let t = Template { separator: "<hr>".into(), ..Default::default() };
        let items = vec![text_item(1, "a"), text_item(2, "b")];
        let out = render_html(&refs(&items), &t);
        assert!(out.contains("&lt;hr&gt;"), "the template is text, not markup");
        assert!(!out.contains("<hr>"));
    }

    #[test]
    fn the_template_shape_is_kept() {
        let t = Template { item_format: "{index}. {content}".into(), ..Default::default() };
        let items = vec![text_item(1, "a"), text_item(2, "b")];
        let out = render_html(&refs(&items), &t);
        assert!(out.contains("1. a"));
        assert!(out.contains("2. b"));
    }

    #[test]
    fn an_unknown_token_survives_as_visible_text() {
        let t = Template { item_format: "{nope} {content}".into(), ..Default::default() };
        let items = vec![text_item(1, "a")];
        let out = render_html(&refs(&items), &t);
        assert!(out.contains("{nope} a"));
    }

    #[test]
    fn an_empty_stack_still_produces_a_container() {
        assert_eq!(render_html(&[], &Template::default()), "<div></div>");
    }
}
