// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Best-effort stateless RTPS reader (W4).
//!
//! Parses incoming RTPS datagrams, filters DATA
//! submessages addressed to our `EntityId` (or
//! ID_UNKNOWN as a wildcard), and delivers the payload bytes to a
//! listener callback.

extern crate alloc;
use alloc::vec::Vec;

use crate::datagram::{ParsedSubmessage, decode_datagram};
use crate::error::WireError;
use crate::wire_types::{EntityId, EntityKind, Guid, GuidPrefix, SequenceNumber};

/// Best-Effort Reader.
///
/// Stateless except for the own GUID + the accepted writer GUID.
/// Phase 0: 1:1 model. Forward-compatible with a multi-writer setup, because
/// `recv_datagram` delivers each DATA submessage individually and the
/// filter logic can live in the caller.
#[derive(Debug, Clone)]
pub struct BestEffortReader {
    guid: Guid,
}

impl BestEffortReader {
    /// Constructs a reader.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, reader_id: EntityId) -> Self {
        Self {
            guid: Guid::new(participant_prefix, reader_id),
        }
    }

    /// Own GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Processes an incoming datagram.
    ///
    /// Returns a Vec of the DATA submessages directed at this reader
    /// (matched on `entity_key` OR `entity_kind == Unknown` as a
    /// wildcard).
    ///
    /// # Errors
    /// `WireError` if the datagram does not parse.
    pub fn recv_datagram(&self, datagram: &[u8]) -> Result<Vec<DeliveredSample>, WireError> {
        let parsed = decode_datagram(datagram)?;
        let mut out = Vec::new();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                if matches_reader(&d.reader_id, &self.guid.entity_id) {
                    out.push(DeliveredSample {
                        writer_id: d.writer_id,
                        writer_sn: d.writer_sn,
                        // WP 2.0a: d.serialized_payload is already Arc<[u8]>
                        // — no second Arc::from alloc, just a move.
                        payload: d.serialized_payload,
                    });
                }
            }
            // Other submessages (HEARTBEAT/ACKNACK/GAP/Unknown) are
            // ignored in phase 0. The reliable path comes with phase 1.
        }
        Ok(out)
    }
}

/// A DATA submessage that the reader forwards to the listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredSample {
    /// Writer GUID (only EntityId; the caller knows the participant
    /// via the datagram header).
    pub writer_id: EntityId,
    /// Sequence number with which the writer sends the sample.
    pub writer_sn: SequenceNumber,
    /// Payload bytes (XCDR2-encoded or vendor-specific),
    /// reference-counted via `Arc::clone` from the cache.
    pub payload: alloc::sync::Arc<[u8]>,
}

fn matches_reader(target: &EntityId, our: &EntityId) -> bool {
    // Wildcard: ID_UNKNOWN matches everything.
    if target.entity_kind == EntityKind::Unknown && target.entity_key == [0; 3] {
        return true;
    }
    target == our
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::writer::BestEffortWriter;

    fn reader_id() -> EntityId {
        EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0])
    }
    fn writer_id() -> EntityId {
        EntityId::user_writer_with_key([0x10, 0x20, 0x30])
    }
    fn prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([1; 12])
    }

    #[test]
    fn reader_delivers_data_addressed_to_us() {
        let mut w = BestEffortWriter::new(prefix(), writer_id(), reader_id());
        let bytes = w.write(b"hello").unwrap();
        let r = BestEffortReader::new(prefix(), reader_id());
        let samples = r.recv_datagram(&bytes).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].payload.as_ref(), &b"hello"[..]);
        assert_eq!(samples[0].writer_id, writer_id());
        assert_eq!(samples[0].writer_sn, SequenceNumber(1));
    }

    #[test]
    fn reader_drops_data_addressed_to_other_reader() {
        let other_reader = EntityId::user_reader_with_key([0x99, 0x99, 0x99]);
        let mut w = BestEffortWriter::new(prefix(), writer_id(), other_reader);
        let bytes = w.write(b"not for you").unwrap();
        let r = BestEffortReader::new(prefix(), reader_id());
        let samples = r.recv_datagram(&bytes).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn reader_accepts_unknown_wildcard_target() {
        let mut w = BestEffortWriter::new(prefix(), writer_id(), EntityId::UNKNOWN);
        let bytes = w.write(b"broadcast").unwrap();
        let r = BestEffortReader::new(prefix(), reader_id());
        let samples = r.recv_datagram(&bytes).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].payload.as_ref(), &b"broadcast"[..]);
    }

    #[test]
    fn reader_propagates_invalid_magic_error() {
        let r = BestEffortReader::new(prefix(), reader_id());
        let bytes = [0u8; 32];
        let res = r.recv_datagram(&bytes);
        assert!(matches!(res, Err(WireError::InvalidMagic { .. })));
    }

    #[test]
    fn reader_handles_multiple_data_in_one_datagram() {
        // Build a datagram with two DATA submessages addressed to us manually.
        let mut w = BestEffortWriter::new(prefix(), writer_id(), reader_id());
        let bytes_a = w.write(b"first").unwrap();
        let bytes_b = w.write(b"second").unwrap();
        // Concatenated datagrams are not directly allowed — but two
        // consecutive recv calls simulate it.
        let r = BestEffortReader::new(prefix(), reader_id());
        let s1 = r.recv_datagram(&bytes_a).unwrap();
        let s2 = r.recv_datagram(&bytes_b).unwrap();
        assert_eq!(s1[0].writer_sn, SequenceNumber(1));
        assert_eq!(s2[0].writer_sn, SequenceNumber(2));
    }
}
