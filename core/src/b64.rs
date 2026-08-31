//! Base64, because two callers need it and neither justifies a dependency.
//!
//! Images reach a rich paste as `data:` URIs and reach the history file as
//! text, and both of those are base64 or they are nothing.

use serde::{Deserialize, Deserializer, Serializer};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 63] as char
        } else {
            PAD as char
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 63] as char
        } else {
            PAD as char
        });
    }
    out
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut lookup = [255u8; 256];
    for (i, c) in ALPHABET.iter().enumerate() {
        lookup[*c as usize] = i as u8;
    }

    let cleaned: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace() && *b != PAD).collect();

    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        // A lone trailing character encodes nothing: it is a truncated file.
        if chunk.len() < 2 {
            return None;
        }
        let mut acc = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            let v = lookup[*b as usize];
            if v == 255 {
                return None;
            }
            acc |= (v as u32) << (18 - 6 * i);
        }
        out.push((acc >> 16) as u8);
        if chunk.len() > 2 {
            out.push((acc >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(acc as u8);
        }
    }
    Some(out)
}

/// Serde adapter so image bytes survive a round trip through the history file.
pub mod bytes {
    use super::*;

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        // A corrupt image in a history file should cost that one thumbnail,
        // not the whole file, so this degrades to empty rather than failing.
        Ok(decode(&text).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_known_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn round_trips_every_byte_value() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)).as_deref(), Some(all.as_slice()));
    }

    #[test]
    fn round_trips_at_every_padding_length() {
        for len in 0..16 {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
            assert_eq!(decode(&encode(&data)), Some(data.clone()), "length {len}");
        }
    }

    #[test]
    fn rejects_characters_outside_the_alphabet() {
        assert_eq!(decode("Zm9v!!!!"), None);
    }

    #[test]
    fn tolerates_whitespace_from_a_wrapped_file() {
        assert!(decode("Zm9v\n Ymfy").is_some());
    }
}
