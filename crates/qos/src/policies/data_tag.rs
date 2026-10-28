// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DataTagQosPolicy` — the tags of a DataWriter or DataReader.
//!
//! DDS-Security 1.2 §7.3.21: `struct Tag { string name; string value; }`,
//! `typedef sequence<Tag> TagSeq;`, `struct DataTags { TagSeq tags; }`,
//! `typedef DataTags DataTagQosPolicy;`. §9.7.2: the tags are implemented
//! as an immutable member of `DataWriterQos` and `DataReaderQos`.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

/// DoS cap: maximum number of tags in a `DataTagQosPolicy` that is decoded.
pub const MAX_DATA_TAGS: usize = 1024;

/// DoS cap: maximum length of a tag name or value that is decoded.
pub const MAX_DATA_TAG_STRING_LEN: usize = 256;

/// One tag: a name and a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Tag name.
    pub name: String,
    /// Tag value.
    pub value: String,
}

impl Tag {
    /// A tag.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// The tags of an endpoint, in the order given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataTagQosPolicy {
    /// `TagSeq tags`.
    pub tags: Vec<Tag>,
}

impl DataTagQosPolicy {
    /// Without tags.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: appends a tag.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push(Tag::new(name, value));
        self
    }

    /// `true` without tags.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Wire encoding: `sequence<Tag>`, each tag two strings
    /// (DDS-Security 1.2 §7.3.21).
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a list no `u32` count describes.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        let len = u32::try_from(self.tags.len())
            .map_err(|_| WireError::refused(w.position(), "a tag count a u32 can carry"))?;
        w.write_u32(len);
        for tag in &self.tags {
            w.write_string(&tag.name);
            w.write_string(&tag.value);
        }
        Ok(())
    }

    /// Wire decoding, capped at [`MAX_DATA_TAGS`] tags of at most
    /// [`MAX_DATA_TAG_STRING_LEN`] octets per string.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] above a cap, [`WireError::ImpossibleLength`]
    /// for a count the stream cannot hold, [`WireError::Truncated`] for a
    /// stream that ends early.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let at = r.position();
        let len = r.read_u32()? as usize;
        if len > MAX_DATA_TAGS {
            return Err(WireError::refused(at, "at most MAX_DATA_TAGS tags"));
        }
        // Each tag is at least two strings of 4 octets.
        if len > r.remaining() / 8 {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: len,
                available: r.remaining(),
            });
        }
        let mut tags = Vec::with_capacity(len);
        for _ in 0..len {
            let string = |r: &mut Reader<'_>| -> Result<String, WireError> {
                let string_at = r.position();
                let s = r.read_string()?;
                if s.len() > MAX_DATA_TAG_STRING_LEN {
                    return Err(WireError::refused(
                        string_at,
                        "a tag string of at most MAX_DATA_TAG_STRING_LEN octets",
                    ));
                }
                Ok(s)
            };
            let name = string(r)?;
            let value = string(r)?;
            tags.push(Tag { name, value });
        }
        Ok(Self { tags })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn tags_survive_the_wire_in_order() {
        let p = DataTagQosPolicy::new()
            .with("level", "secret")
            .with("a", "");
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(DataTagQosPolicy::decode_from(&mut r).unwrap(), p);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_count_the_stream_cannot_hold_is_refused() {
        let mut w = Writer::new(Endianness::Little);
        w.write_u32(1000);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert!(DataTagQosPolicy::decode_from(&mut r).is_err());
    }

    #[test]
    fn tags_keep_their_order() {
        let p = DataTagQosPolicy::new().with("a", "1").with("b", "2");
        assert_eq!(p.tags, [Tag::new("a", "1"), Tag::new("b", "2")]);
        assert!(!p.is_empty());
        assert!(DataTagQosPolicy::default().is_empty());
    }
}
