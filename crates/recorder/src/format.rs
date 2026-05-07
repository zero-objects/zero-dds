// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Format-Konstanten + Wire-Strukturen fuer `.zddsrec`.

use alloc::string::String;
use alloc::vec::Vec;

/// File-Magic "ZDDS".
pub const ZDDSREC_MAGIC: [u8; 4] = *b"ZDDS";
/// Frame-Marker innerhalb des Streams.
pub const FRAME_MAGIC: u8 = b'F';
/// Format-Version. Inkompatible Aenderungen heben den Wert.
pub const ZDDSREC_VERSION: u32 = 1;

/// Sample-Kind nach DDS-Spec §2.2.4.4.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SampleKind {
    /// `ALIVE` — Reader sieht ein lebendes Sample.
    Alive = 0,
    /// `NOT_ALIVE_DISPOSED` — Writer hat dispose()'d.
    NotAliveDisposed = 1,
    /// `NOT_ALIVE_UNREGISTERED` — Writer hat unregister()'d.
    NotAliveUnregistered = 2,
}

impl SampleKind {
    /// Wire-Wert.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parsed das u8-Wire-Byte. Returns None bei Unknown.
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

/// Participant-Entry im Header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantEntry {
    /// 16-Byte RTPS-GUID-Prefix + EntityId (full GUID).
    pub guid: [u8; 16],
    /// Logischer Name (z.B. ROS-2 Node-Name).
    pub name: String,
}

/// Topic-Entry im Header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicEntry {
    /// DDS-Topic-Name.
    pub name: String,
    /// Type-Name (z.B. "std_msgs::msg::String").
    pub type_name: String,
}

/// Header eines `.zddsrec`-Files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// UNIX-Epoch-Timestamp in Nanosekunden — Frame-Timestamps sind
    /// Deltas relativ dazu.
    pub time_base_unix_ns: i64,
    /// Bekannte Participants (Reihenfolge ist Frame-Index).
    pub participants: Vec<ParticipantEntry>,
    /// Bekannte Topics (Reihenfolge ist Frame-Index).
    pub topics: Vec<TopicEntry>,
}

impl Header {
    /// Serialisiert den Header inkl. Magic + Version.
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

/// Frame im Sample-Stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Nanosekunden-Delta zum Header-`time_base_unix_ns`.
    pub timestamp_delta_ns: i64,
    /// Index in [`Header::participants`].
    pub participant_idx: u32,
    /// Index in [`Header::topics`].
    pub topic_idx: u32,
    /// DDS-Sample-Kind (Alive/Disposed/Unregistered).
    pub sample_kind: SampleKind,
    /// CDR-encoded Payload-Bytes.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Serialisiert den Frame.
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

/// Borrowed-Variante des Frames — nuetzlich fuer Replay ohne Copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameView<'a> {
    /// Nanosekunden-Delta zum Header-`time_base_unix_ns`.
    pub timestamp_delta_ns: i64,
    /// Index in [`Header::participants`].
    pub participant_idx: u32,
    /// Index in [`Header::topics`].
    pub topic_idx: u32,
    /// DDS-Sample-Kind.
    pub sample_kind: SampleKind,
    /// Borrowed Payload.
    pub payload: &'a [u8],
}

impl FrameView<'_> {
    /// Konvertiert in eine owned [`Frame`].
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
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
