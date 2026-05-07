// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Best-Effort Stateless RTPS-Writer (W4).
//!
//! ein einziges 1:1-Modell mit einem Reader. Reliable-
//! AckNack-Loop, Heartbeat-Timer, History-Cache mit Resend-Logik
//! folgen mit Phase 1.
//!
//! Der Writer ist eine reine Datenstruktur ohne I/O — er produziert
//! Datagramme als `Vec<u8>`, der Caller leitet sie an seinen Transport
//! weiter. Diese Trennung erleichtert Testing (no fakes needed).

extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::datagram::encode_data_datagram;
use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::submessages::DataSubmessage;
use crate::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};

/// Stateless Best-Effort Writer.
///
/// Pflegt nur den naechsten zu vergebenden `SequenceNumber`. Jeder
/// `write()`-Call inkrementiert die SN, baut eine DATA-Submessage und
/// liefert das fertige Datagram. Der Caller sendet das via Transport.
#[derive(Debug, Clone)]
pub struct BestEffortWriter {
    guid: Guid,
    vendor_id: VendorId,
    next_sn: i64,
    /// EntityId des einen erlaubten Reader-Endpoints.
    target_reader: EntityId,
}

impl BestEffortWriter {
    /// Konstruiert einen Writer.
    ///
    /// `next_sn` startet bei 1 (Spec-Konvention: erste valid SN).
    #[must_use]
    pub fn new(
        participant_prefix: GuidPrefix,
        writer_id: EntityId,
        target_reader: EntityId,
    ) -> Self {
        Self {
            guid: Guid::new(participant_prefix, writer_id),
            vendor_id: VendorId::ZERODDS,
            next_sn: 1,
            target_reader,
        }
    }

    /// Setzt die VendorId (Default `VendorId::ZERODDS`).
    pub fn set_vendor_id(&mut self, vendor: VendorId) {
        self.vendor_id = vendor;
    }

    /// Eigene GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// SequenceNumber, die beim naechsten `write()` vergeben wird.
    #[must_use]
    pub fn next_sequence_number(&self) -> SequenceNumber {
        SequenceNumber(self.next_sn)
    }

    /// Encoded eine DATA-Submessage mit `payload` und liefert das
    /// fertige RTPS-Datagram. SequenceNumber wird vor Return
    /// inkrementiert.
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange`, wenn der Body groesser als
    /// `u16::MAX` Bytes wird.
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<u8>, WireError> {
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "writer sequence number overflow",
            })?;
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: self.target_reader,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: Arc::from(payload),
        };
        let header = RtpsHeader::new(self.vendor_id, self.guid.prefix);
        encode_data_datagram(header, &[data])
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::datagram::{ParsedSubmessage, decode_datagram};

    fn writer() -> BestEffortWriter {
        BestEffortWriter::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        )
    }

    #[test]
    fn writer_starts_at_sequence_number_one() {
        let w = writer();
        assert_eq!(w.next_sequence_number(), SequenceNumber(1));
    }

    #[test]
    fn writer_increments_sequence_number_per_write() {
        let mut w = writer();
        w.write(b"a").unwrap();
        assert_eq!(w.next_sequence_number(), SequenceNumber(2));
        w.write(b"b").unwrap();
        assert_eq!(w.next_sequence_number(), SequenceNumber(3));
    }

    #[test]
    fn writer_produces_decodable_datagram() {
        let mut w = writer();
        let bytes = w.write(b"hello world").unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(d) => {
                assert_eq!(d.writer_sn, SequenceNumber(1));
                assert_eq!(d.serialized_payload.as_ref(), b"hello world");
                assert_eq!(d.writer_id.entity_key, [0x10, 0x20, 0x30]);
                assert_eq!(d.reader_id.entity_key, [0xA0, 0xB0, 0xC0]);
            }
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn writer_sets_header_with_zerodds_vendor() {
        let mut w = writer();
        let bytes = w.write(b"x").unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.header.vendor_id, VendorId::ZERODDS);
    }

    #[test]
    fn writer_set_vendor_id_overrides_default() {
        let mut w = writer();
        w.set_vendor_id(VendorId([0xAB, 0xCD]));
        let bytes = w.write(b"x").unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.header.vendor_id, VendorId([0xAB, 0xCD]));
    }

    #[test]
    fn writer_sn_overflow_is_error() {
        let mut w = writer();
        w.next_sn = i64::MAX;
        let res = w.write(b"x");
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn writer_three_writes_have_increasing_sn_in_decoded() {
        use alloc::format;
        let mut w = writer();
        let mut sns = Vec::new();
        for i in 0..3 {
            let bytes = w.write(format!("msg-{i}").as_bytes()).unwrap();
            let parsed = decode_datagram(&bytes).unwrap();
            if let ParsedSubmessage::Data(d) = &parsed.submessages[0] {
                sns.push(d.writer_sn);
            }
        }
        assert_eq!(
            sns,
            alloc::vec![SequenceNumber(1), SequenceNumber(2), SequenceNumber(3)]
        );
    }
}
