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
    FRAME_MAGIC, Frame, FrameView, Header, ParticipantEntry, SampleKind, TopicEntry, ZDDSREC_MAGIC,
    ZDDSREC_VERSION,
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
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> RecordReader<'a> {
    /// Creates a reader. `parse_header` must be called first,
    /// then `next_frame` until None.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
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
        let time_base = self.read_i64("time_base")?;
        let pn = self.read_u32("participant_count")? as usize;
        let tn = self.read_u32("topic_count")? as usize;
        let mut participants = Vec::with_capacity(pn);
        for _ in 0..pn {
            let guid_slice = self.read_bytes("participant_guid", 16)?;
            let mut guid = [0u8; 16];
            guid.copy_from_slice(guid_slice);
            let name = self.read_string("participant_name")?;
            participants.push(ParticipantEntry { guid, name });
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
        if self.cursor >= self.bytes.len() {
            return Ok(None);
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
        let plen = self.read_u32("frame_payload_len")? as usize;
        let payload = self.read_bytes("frame_payload", plen)?;
        Ok(Some(FrameView {
            timestamp_delta_ns: ts,
            participant_idx: pidx,
            topic_idx: tidx,
            sample_kind,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;
    use crate::writer::{header_with, write_all};

    fn alloc_string(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn header_roundtrip() {
        let h = header_with(
            1_700_000_000_000_000_000,
            vec![
                (alloc_string("talker"), [1u8; 16]),
                (alloc_string("listener"), [2u8; 16]),
            ],
            vec![(
                alloc_string("/chatter"),
                alloc_string("std_msgs::msg::String"),
            )],
        );
        let w = write_all(Vec::<u8>::new(), &h, std::iter::empty()).unwrap();
        let bytes = w.into_inner();
        let mut r = RecordReader::new(&bytes);
        let parsed = r.parse_header().unwrap();
        assert_eq!(parsed, h);
        assert!(r.next_frame_view().unwrap().is_none());
    }

    #[test]
    fn frame_roundtrip() {
        let h = header_with(
            0,
            vec![(alloc_string("p"), [1u8; 16])],
            vec![(alloc_string("/t"), alloc_string("T"))],
        );
        let frames: Vec<Frame> = (0..3u8)
            .map(|i| Frame {
                timestamp_delta_ns: (i as i64) * 1_000_000,
                participant_idx: 0,
                topic_idx: 0,
                sample_kind: SampleKind::Alive,
                payload: vec![i, i + 1, i + 2, 0xab],
            })
            .collect();
        let w = write_all(Vec::<u8>::new(), &h, frames.clone()).unwrap();
        let bytes = w.into_inner();
        let mut r = RecordReader::new(&bytes);
        let _ = r.parse_header().unwrap();
        let mut got = Vec::new();
        while let Some(f) = r.next_frame().unwrap() {
            got.push(f);
        }
        assert_eq!(got, frames);
    }

    #[test]
    fn bad_magic_rejected() {
        let bytes = [0u8; 32];
        let mut r = RecordReader::new(&bytes);
        let res = r.parse_header();
        assert!(matches!(res, Err(ReadError::BadMagic)));
    }

    #[test]
    fn truncated_header_detected() {
        let bytes = b"ZDD".as_slice(); // 3 bytes, magic needs 4
        let mut r = RecordReader::new(bytes);
        assert!(matches!(r.parse_header(), Err(ReadError::Truncated { .. })));
    }

    #[test]
    fn truncated_frame_detected() {
        let h = header_with(
            0,
            vec![(alloc_string("p"), [1u8; 16])],
            vec![(alloc_string("/t"), alloc_string("T"))],
        );
        let mut buf = Vec::new();
        h.write(&mut buf);
        // Add half a frame.
        buf.push(b'F');
        buf.extend_from_slice(&0i64.to_le_bytes()); // timestamp ok
        buf.extend_from_slice(&0u32.to_le_bytes()); // participant
        // missing topic + sample_kind + payload-len
        let mut r = RecordReader::new(&buf);
        let _ = r.parse_header().unwrap();
        assert!(matches!(
            r.next_frame_view(),
            Err(ReadError::Truncated { .. })
        ));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&ZDDSREC_MAGIC);
        buf.extend_from_slice(&999u32.to_le_bytes());
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let mut r = RecordReader::new(&buf);
        assert!(matches!(
            r.parse_header(),
            Err(ReadError::UnsupportedVersion(999))
        ));
    }

    #[test]
    fn bad_sample_kind_rejected() {
        let h = header_with(
            0,
            vec![(alloc_string("p"), [0u8; 16])],
            vec![(alloc_string("/t"), alloc_string("T"))],
        );
        let mut buf = Vec::new();
        h.write(&mut buf);
        buf.push(b'F');
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.push(99); // invalid sample-kind
        buf.extend_from_slice(&0u32.to_le_bytes());
        let mut r = RecordReader::new(&buf);
        let _ = r.parse_header().unwrap();
        assert!(matches!(
            r.next_frame_view(),
            Err(ReadError::BadSampleKind(99))
        ));
    }
}
