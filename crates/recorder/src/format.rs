// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Format constants + wire structures for `.zddsrec`.

use alloc::string::String;
use alloc::vec::Vec;

/// File magic "ZDDS".
pub const ZDDSREC_MAGIC: [u8; 4] = *b"ZDDS";
/// Frame marker within the stream.
pub const FRAME_MAGIC: u8 = b'F';
/// Format version. Incompatible changes bump the value.
pub const ZDDSREC_VERSION: u32 = 1;

/// Sample kind per DDS spec §2.2.4.4.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SampleKind {
    /// `ALIVE` — the reader sees a live sample.
    Alive = 0,
    /// `NOT_ALIVE_DISPOSED` — the writer called dispose().
    NotAliveDisposed = 1,
    /// `NOT_ALIVE_UNREGISTERED` — the writer called unregister().
    NotAliveUnregistered = 2,
}

impl SampleKind {
    /// Wire value.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parses the u8 wire byte. Returns None on unknown.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Alive),
            1 => Some(Self::NotAliveDisposed),
            2 => Some(Self::NotAliveUnregistered),
            _ => None,
        }
    }
}

/// Participant entry in the header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantEntry {
    /// 16-byte RTPS GUID prefix + EntityId (full GUID).
    pub guid: [u8; 16],
    /// Logical name (e.g. ROS 2 node name).
    pub name: String,
}

/// Topic entry in the header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicEntry {
    /// DDS topic name.
    pub name: String,
    /// Type name (e.g. "std_msgs::msg::String").
    pub type_name: String,
}

/// Header of a `.zddsrec` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// UNIX epoch timestamp in nanoseconds — frame timestamps are
    /// deltas relative to it.
    pub time_base_unix_ns: i64,
    /// Known participants (order is the frame index).
    pub participants: Vec<ParticipantEntry>,
    /// Known topics (order is the frame index).
    pub topics: Vec<TopicEntry>,
}

impl Header {
    /// Serializes the header incl. magic + version.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&ZDDSREC_MAGIC);
        out.extend_from_slice(&ZDDSREC_VERSION.to_le_bytes());
        out.extend_from_slice(&self.time_base_unix_ns.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(self.participants.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(self.topics.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for p in &self.participants {
            out.extend_from_slice(&p.guid);
            write_string(out, &p.name);
        }
        for t in &self.topics {
            write_string(out, &t.type_name);
            write_string(out, &t.name);
        }
    }
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Frame in the sample stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Nanosecond delta to the header `time_base_unix_ns`.
    pub timestamp_delta_ns: i64,
    /// Index in [`Header::participants`].
    pub participant_idx: u32,
    /// Index in [`Header::topics`].
    pub topic_idx: u32,
    /// DDS sample kind (Alive/Disposed/Unregistered).
    pub sample_kind: SampleKind,
    /// CDR-encoded payload bytes.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Serializes the frame.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.push(FRAME_MAGIC);
        out.extend_from_slice(&self.timestamp_delta_ns.to_le_bytes());
        out.extend_from_slice(&self.participant_idx.to_le_bytes());
        out.extend_from_slice(&self.topic_idx.to_le_bytes());
        out.push(self.sample_kind.to_u8());
        let len = u32::try_from(self.payload.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&self.payload);
    }
}

/// Borrowed variant of the frame — useful for replay without a copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameView<'a> {
    /// Nanosecond delta to the header `time_base_unix_ns`.
    pub timestamp_delta_ns: i64,
    /// Index in [`Header::participants`].
    pub participant_idx: u32,
    /// Index in [`Header::topics`].
    pub topic_idx: u32,
    /// DDS sample kind.
    pub sample_kind: SampleKind,
    /// Borrowed payload.
    pub payload: &'a [u8],
}

impl FrameView<'_> {
    /// Converts into an owned [`Frame`].
    #[must_use]
    pub fn to_owned(&self) -> Frame {
        Frame {
            timestamp_delta_ns: self.timestamp_delta_ns,
            participant_idx: self.participant_idx,
            topic_idx: self.topic_idx,
            sample_kind: self.sample_kind,
            payload: self.payload.to_vec(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    #[test]
    fn sample_kind_roundtrip() {
        for k in [
            SampleKind::Alive,
            SampleKind::NotAliveDisposed,
            SampleKind::NotAliveUnregistered,
        ] {
            assert_eq!(SampleKind::from_u8(k.to_u8()), Some(k));
        }
        assert_eq!(SampleKind::from_u8(99), None);
    }

    #[test]
    fn header_writes_magic_and_version() {
        let h = Header {
            time_base_unix_ns: 1_700_000_000_000_000_000,
            participants: Vec::new(),
            topics: Vec::new(),
        };
        let mut out = Vec::new();
        h.write(&mut out);
        assert_eq!(&out[0..4], &ZDDSREC_MAGIC);
        assert_eq!(
            u32::from_le_bytes(out[4..8].try_into().unwrap()),
            ZDDSREC_VERSION
        );
    }
}
