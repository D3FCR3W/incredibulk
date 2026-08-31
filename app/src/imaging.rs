//! Turning clipboard pixels into something that can be shown and pasted.
//!
//! An image arrives as raw RGBA, which is useless to both the list and the
//! clipboard: a webview needs a `data:` URI and a rich paste needs a PNG. Both
//! are derived once, here, at the moment of capture, rather than every time
//! the list redraws.

use std::io::Cursor;

use image::{ImageFormat, RgbaImage};
use incredibulk_core::{ImagePayload, b64};

/// Longest edge of the thumbnail shown in the list.
///
/// Small on purpose. The snapshot carrying these is pushed to the window on
/// every capture, and a session can hold a dozen screenshots.
const THUMBNAIL_EDGE: u32 = 96;

/// Fill in the derived forms of a freshly captured image.
///
/// Failure is not fatal: an image that cannot be encoded still counts as a
/// capture, it just shows and pastes as its description.
pub fn prepare(payload: &mut ImagePayload) {
    let Some(source) = to_image(payload) else {
        return;
    };

    if let Some(png) = encode_png(&source) {
        payload.png = png;
    }

    // `thumbnail` preserves the aspect ratio and never enlarges, so a 32px
    // favicon stays 32px instead of being blown up into a blurry square.
    let small = image::DynamicImage::ImageRgba8(source).thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE);
    if let Some(png) = encode_png(&small.to_rgba8()) {
        payload.thumbnail = format!("data:image/png;base64,{}", b64::encode(&png));
    }
}

fn to_image(payload: &ImagePayload) -> Option<RgbaImage> {
    let width = u32::try_from(payload.width).ok()?;
    let height = u32::try_from(payload.height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    // from_raw returns None when the buffer does not match the dimensions,
    // which is the one thing that would otherwise panic here.
    RgbaImage::from_raw(width, height, payload.rgba.clone())
}

fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    image.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).ok().map(|_| out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize) -> ImagePayload {
        ImagePayload::raw(width, height, vec![120; width * height * 4])
    }

    #[test]
    fn a_captured_image_gains_a_png_and_a_thumbnail() {
        let mut payload = solid(8, 6);
        prepare(&mut payload);
        assert!(payload.is_embeddable());
        assert!(payload.png.starts_with(&[137, 80, 78, 71]), "PNG magic number");
        assert!(payload.thumbnail.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn a_large_image_is_thumbnailed_down() {
        let mut payload = solid(400, 300);
        prepare(&mut payload);
        // The thumbnail must be far smaller than the full encoding, otherwise
        // pushing the list on every capture would carry the whole screenshot.
        assert!(
            payload.thumbnail.len() < payload.png.len(),
            "thumbnail {} should be smaller than png {}",
            payload.thumbnail.len(),
            payload.png.len()
        );
    }

    #[test]
    fn a_buffer_that_does_not_match_its_dimensions_is_left_alone() {
        let mut payload = ImagePayload::raw(10, 10, vec![0; 4]);
        prepare(&mut payload);
        assert!(!payload.is_embeddable(), "no half-decoded image is invented");
        assert!(payload.thumbnail.is_empty());
    }

    #[test]
    fn an_empty_image_is_left_alone() {
        let mut payload = ImagePayload::raw(0, 0, Vec::new());
        prepare(&mut payload);
        assert!(payload.png.is_empty());
    }

    #[test]
    fn the_data_uri_round_trips_through_base64() {
        let mut payload = solid(4, 4);
        prepare(&mut payload);
        let uri = payload.data_uri().expect("an embeddable image");
        let encoded = uri.strip_prefix("data:image/png;base64,").expect("the prefix");
        assert_eq!(b64::decode(encoded).as_deref(), Some(payload.png.as_slice()));
    }
}
