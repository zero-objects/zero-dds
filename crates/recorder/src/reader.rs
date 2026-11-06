// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RecordReader` — parses `.zddsrec` streams into a `Header` plus
//! a sequence of [`crate::format::Frame`].
//!
//! Reads from a `&[u8]` buffer (all in memory). Streaming
//! `std::io::Read` is envisaged as an additive major-2.0 extension.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::format::{
    FIRST_VERSION_WITH_REPRESENTATION, FRAME_MAGIC, Frame, FrameView, Header, ParticipantEntry,
    SampleKind, TopicEntry, ZDDSREC_MAGIC, ZDDSREC_VERSION,
};

/// Error while reading.
#[derive(Debug)]
pub enum ReadError {
    /// File magic does not match — not a `.zddsrec`.
    BadMagic,
    /// Format version above the supported range.
    UnsupportedVersion(u32),
    /// Stream too short for the expected field.
    Truncated {
        /// Which field.
        what: &'static str,
        /// How many bytes are expected.
        need: usize,
        /// How many are left.
        have: usize,
    },
    /// String contains no valid UTF-8.
    InvalidUtf8 {
        /// Which field.
        what: &'static str,
    },
    /// Sample-kind byte is outside {0,1,2}.
    BadSampleKind(u8),
    /// Frame magic does not match.
    BadFrameMagic(u8),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "bad file magic (not a zddsrec)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported zddsrec version: {v}"),
            Self::Truncated { what, need, have } => {
                write!(f, "truncated at {what}: need {need} bytes, have {have}")
            }
            Self::InvalidUtf8 { what } => write!(f, "invalid utf-8 in {what}"),
            Self::BadSampleKind(b) => write!(f, "unknown sample-kind byte {b}"),
            Self::BadFrameMagic(b) => write!(f, "expected frame-magic 'F', got 0x{b:02x}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Reader for `.zddsrec` buffers.
pub struct RecordReader<'a> {
    /// Topics that `TopicAdd` frames put into the index after the
    /// header. Their indices continue the header's list.
    added_topics: Vec<TopicEntry>,
    bytes: &'a [u8],
    cursor: usize,
    /// The version the header names; the frames are read in its layout.
    version: u32,
}

impl<'a> RecordReader<'a> {
    /// Creates a reader. `parse_header` must be called first,
    /// then `next_frame` until None.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            cursor: 0,
            added_topics: Vec::new(),
            version: ZDDSREC_VERSION,
        }
    }

    /// The format version the header named (after [`Self::parse_header`]).
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The topics added after the header, in the order they were added.
    ///
    /// Their indices continue the header's list: the first addition is
    /// `header.topics.len()`. Only filled once the frames carrying them
    /// have been read, so this answers the state as of the last frame
    /// returned, not the state of the whole file.
    #[must_use]
    pub fn added_topics(&self) -> &[TopicEntry] {
        &self.added_topics
    }

    /// Bytes not yet consumed.
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn need(&self, what: &'static str, n: usize) -> Result<&'a [u8], ReadError> {
        if self.cursor + n > self.bytes.len() {
            return Err(ReadError::Truncated {
                what,
                need: n,
                have: self.bytes.len().saturating_sub(self.cursor),
            });
        }
        Ok(&self.bytes[self.cursor..self.cursor + n])
    }

    fn read_u32(&mut self, what: &'static str) -> Result<u32, ReadError> {
        let s = self.need(what, 4)?;
        // `need(.., 4)` guarantees 4 bytes; try_into is therefore
        // infallible — we still map the theoretical Err path
        // explicitly instead of expect/panic in the runtime path.
        let arr: [u8; 4] = s.try_into().map_err(|_| ReadError::Truncated {
            what,
            need: 4,
            have: s.len(),
        })?;
        let v = u32::from_le_bytes(arr);
        self.cursor += 4;
        Ok(v)
    }

    fn read_u64(&mut self, what: &'static str) -> Result<u64, ReadError> {
        let s = self.need(what, 8)?;
        let arr: [u8; 8] = s.try_into().map_err(|_| ReadError::Truncated {
            what,
            need: 8,
            have: s.len(),
        })?;
        let v = u64::from_le_bytes(arr);
        self.cursor += 8;
        Ok(v)
    }

    fn read_i64(&mut self, what: &'static str) -> Result<i64, ReadError> {
        Ok(self.read_u64(what)? as i64)
    }

    fn read_u8(&mut self, what: &'static str) -> Result<u8, ReadError> {
        let s = self.need(what, 1)?;
        let v = s[0];
        self.cursor += 1;
        Ok(v)
    }

    fn read_string(&mut self, what: &'static str) -> Result<String, ReadError> {
        let len = self.read_u32(what)? as usize;
        let s = self.need(what, len)?;
        let owned = core::str::from_utf8(s)
            .map_err(|_| ReadError::InvalidUtf8 { what })?
            .to_string();
        self.cursor += len;
        Ok(owned)
    }

    fn read_bytes(&mut self, what: &'static str, n: usize) -> Result<&'a [u8], ReadError> {
        let s = self.need(what, n)?;
        self.cursor += n;
        Ok(s)
    }

    /// Parses the header. The cursor then sits at the first frame.
    ///
    /// # Errors
    /// [`ReadError::BadMagic`], `UnsupportedVersion`, `Truncated`.
    pub fn parse_header(&mut self) -> Result<Header, ReadError> {
        let magic = self.read_bytes("magic", 4)?;
        if magic != ZDDSREC_MAGIC {
            return Err(ReadError::BadMagic);
        }
        let version = self.read_u32("version")?;
        if version > ZDDSREC_VERSION {
            return Err(ReadError::UnsupportedVersion(version));
        }
        self.version = version;
        let time_base = self.read_i64("time_base")?;
        let pn = self.read_u32("participant_count")? as usize;
        let tn = self.read_u32("topic_count")? as usize;
        // Each ParticipantEntry needs >= 16 bytes (GUID) + 4 bytes (the
        // participant_name length prefix); reject a wire-announced count
        // that cannot possibly fit the bytes remaining *before*
        // `Vec::with_capacity` — mirrors `crates/cdr/src/composite.rs`'s
        // `len > reader.remaining()` guard.
        const MIN_PARTICIPANT_BYTES: usize = 20;
        if pn > self.remaining() / MIN_PARTICIPANT_BYTES {
            return Err(ReadError::Truncated {
                what: "participant_count",
                need: pn.saturating_mul(MIN_PARTICIPANT_BYTES),
                have: self.remaining(),
            });
        }
        let mut participants = Vec::with_capacity(pn);
        for _ in 0..pn {
            let guid_slice = self.read_bytes("participant_guid", 16)?;
            let mut guid = [0u8; 16];
            guid.copy_from_slice(guid_slice);
            let name = self.read_string("participant_name")?;
            participants.push(ParticipantEntry { guid, name });
        }
        // Each TopicEntry needs >= 4 + 4 bytes (the two string length
        // prefixes).
        const MIN_TOPIC_BYTES: usize = 8;
        if tn > self.remaining() / MIN_TOPIC_BYTES {
            return Err(ReadError::Truncated {
                what: "topic_count",
                need: tn.saturating_mul(MIN_TOPIC_BYTES),
                have: self.remaining(),
            });
        }
        let mut topics = Vec::with_capacity(tn);
        for _ in 0..tn {
            let type_name = self.read_string("topic_type_name")?;
            let name = self.read_string("topic_name")?;
            topics.push(TopicEntry { name, type_name });
        }
        Ok(Header {
            time_base_unix_ns: time_base,
            participants,
            topics,
        })
    }

    /// Reads the next frame. Returns `Ok(None)` when EOF is reached.
    ///
    /// # Errors
    /// `BadFrameMagic`, `Truncated`, `BadSampleKind`.
    pub fn next_frame_view(&mut self) -> Result<Option<FrameView<'a>>, ReadError> {
        // Topic additions are consumed here rather than answered to the
        // caller. They carry no sample, and a caller that walks frames to
        // read data would have to know about them to skip them — which
        // every caller would then do the same way. What they change is
        // this reader's own topic list, which is where the change
        // belongs. More than one may stand in a row.
        loop {
            if self.cursor >= self.bytes.len() {
                return Ok(None);
            }
            if self.bytes[self.cursor] != crate::format::TOPIC_ADD_MAGIC {
                break;
            }
            self.cursor += 1;
            let type_name = self.read_string("topic_add_type")?;
            let name = self.read_string("topic_add_name")?;
            self.added_topics.push(TopicEntry { name, type_name });
        }
        let magic = self.read_u8("frame_magic")?;
        if magic != FRAME_MAGIC {
            return Err(ReadError::BadFrameMagic(magic));
        }
        let ts = self.read_i64("frame_timestamp")?;
        let pidx = self.read_u32("frame_participant_idx")?;
        let tidx = self.read_u32("frame_topic_idx")?;
        let kind_byte = self.read_u8("frame_sample_kind")?;
        let sample_kind =
            SampleKind::from_u8(kind_byte).ok_or(ReadError::BadSampleKind(kind_byte))?;
        let representation_identifier = if self.version >= FIRST_VERSION_WITH_REPRESENTATION {
            let present = self.read_u8("frame_representation_present")?;
            let id = self.read_bytes("frame_representation_identifier", 2)?;
            (present != 0).then(|| [id[0], id[1]])
        } else {
            None
        };
        let plen = self.read_u32("frame_payload_len")? as usize;
        let payload = self.read_bytes("frame_payload", plen)?;
        Ok(Some(FrameView {
            timestamp_delta_ns: ts,
            participant_idx: pidx,
            topic_idx: tidx,
            sample_kind,
            representation_identifier,
            payload,
        }))
    }

    /// Convenience: returns an owned [`Frame`].
    ///
    /// # Errors
    /// As [`RecordReader::next_frame_view`].
    pub fn next_frame(&mut self) -> Result<Option<Frame>, ReadError> {
        Ok(self.next_frame_view()?.map(|v| v.to_owned()))
    }
}
