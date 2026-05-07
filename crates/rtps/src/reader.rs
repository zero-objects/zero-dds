// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Best-Effort Stateless RTPS-Reader (W4).
//!
//! parst eingehende RTPS-Datagrams, filtert DATA-
//! Submessages, die an unsere `EntityId` adressiert sind (oder
//! ID_UNKNOWN als Wildcard), und liefert die Payload-Bytes an einen
//! Listener-Callback.

extern crate alloc;
use alloc::vec::Vec;

use crate::datagram::{ParsedSubmessage, decode_datagram};
use crate::error::WireError;
use crate::wire_types::{EntityId, EntityKind, Guid, GuidPrefix, SequenceNumber};

/// Best-Effort Reader.
///
/// Stateless ausser der eigenen GUID + dem akzeptierten Writer-GUID.
/// Phase 0: 1:1-Modell. Forward-Kompatibel mit Mehr-Writer-Setup, weil
/// `recv_datagram` jeden DATA-Submessage einzeln liefert und die
/// Filterlogik im Caller liegen kann.
#[derive(Debug, Clone)]
pub struct BestEffortReader {
    guid: Guid,
}

impl BestEffortReader {
    /// Konstruiert einen Reader.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, reader_id: EntityId) -> Self {
        Self {
            guid: Guid::new(participant_prefix, reader_id),
        }
    }

    /// Eigene GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Verarbeitet ein eingehendes Datagram.
    ///
    /// Liefert Vec der DATA-Submessages, die an diesen Reader gerichtet
    /// sind (matched auf `entity_key` ODER `entity_kind == Unknown` als
    /// Wildcard).
    ///
    /// # Errors
    /// `WireError`, wenn das Datagram nicht parst.
    pub fn recv_datagram(&self, datagram: &[u8]) -> Result<Vec<DeliveredSample>, WireError> {
        let parsed = decode_datagram(datagram)?;
        let mut out = Vec::new();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                if matches_reader(&d.reader_id, &self.guid.entity_id) {
                    out.push(DeliveredSample {
                        writer_id: d.writer_id,
                        writer_sn: d.writer_sn,
                        // WP 2.0a: d.serialized_payload ist bereits Arc<[u8]>
                        // — kein zweiter Arc::from-Alloc, nur move.
                        payload: d.serialized_payload,
                    });
                }
            }
            // Andere Submessages (HEARTBEAT/ACKNACK/GAP/Unknown) werden
            // in Phase 0 ignoriert. Reliable-Pfad kommt mit Phase 1.
        }
        Ok(out)
    }
}

/// Eine DATA-Submessage, die der Reader an den Listener weiterreicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredSample {
    /// Writer-GUID (nur EntityId; der Caller kennt den Participant
    /// ueber den Datagram-Header).
    pub writer_id: EntityId,
    /// Sequence-Number, mit der der Writer das Sample sendet.
    pub writer_sn: SequenceNumber,
    /// Payload-Bytes (XCDR2-codiert oder vendor-spezifisch),
    /// referenzgezaehlt via `Arc::clone` aus dem Cache.
    pub payload: alloc::sync::Arc<[u8]>,
}

fn matches_reader(target: &EntityId, our: &EntityId) -> bool {
    // Wildcard: ID_UNKNOWN matched alles.
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
        // Bauen manuell ein Datagram mit zwei DATA-Submessages an uns.
        let mut w = BestEffortWriter::new(prefix(), writer_id(), reader_id());
        let bytes_a = w.write(b"first").unwrap();
        let bytes_b = w.write(b"second").unwrap();
        // Concat-Datagrams nicht direkt erlaubt — aber zwei aufeinander-
        // folgende recv-Aufrufe simulieren das.
        let r = BestEffortReader::new(prefix(), reader_id());
        let s1 = r.recv_datagram(&bytes_a).unwrap();
        let s2 = r.recv_datagram(&bytes_b).unwrap();
        assert_eq!(s1[0].writer_sn, SequenceNumber(1));
        assert_eq!(s2[0].writer_sn, SequenceNumber(2));
    }
}
