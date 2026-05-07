// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MessageBuilder — Submessage-Aggregation in ein UDP-Datagramm.
//!
//! Analog zu Fast-DDS `RTPSMessageGroup` (Recherche WP 1.4). Der Writer
//! oeffnet einen Builder pro Ziel-Locator-Set, haengt mehrere Submessages
//! an, und finalisiert zu einem [`OutboundDatagram`]. Aggregation spart
//! RTPS-Header + UDP-Overhead bei SEDP-Announce-all-Runden und kleinen
//! Samples.
//!
//! # Flush-Regeln
//!
//! 1. **Size-Trigger**: `try_add_submessage` lehnt ab, wenn der Body
//!    nicht mehr ins MTU passt. Caller muss `finish()` + neuen Builder.
//! 2. **DATA_FRAG geht alleine**: Aufrufer soll DATA_FRAG-Submessages
//!    nicht mit anderen bundlen (Fragment ist typisch MTU-nah).
//! 3. **Piggyback-HEARTBEAT am Ende**: Aufrufer haengt HB nach allen
//!    DATAs an, vor `finish()`.
//! 4. **Kein INFO_DST**: Phase 1 baut ein Datagramm pro Proxy — GuidPrefix
//!    ist statisch. INFO_DST wird fuer Multicast-Fan-out mit gemischten
//!    Zielen in Phase 2 ergaenzt.
//! 5. **Kein INFO_TS**: Writer hat heute keine Source-Timestamps.
//!
//! # Ziel-Locators
//!
//! Ein Datagramm geht an **alle** `targets`. Typisch: die Unicast-Locators
//! des Remote-Readers, oder ein Multicast-Locator. Transport-Layer kippt
//! es einmal pro Locator auf die Leitung.

extern crate alloc;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::header::RtpsHeader;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::wire_types::Locator;

/// Default-MTU fuer Aggregation (Ethernet 1500 − 20 IP − 8 UDP).
pub const DEFAULT_MTU: usize = 1472;

/// Ein fertig aggregiertes Datagramm mit Zielen.
///
/// `targets` ist als `Rc<Vec<Locator>>` geteilt, um Allocation-Overhead
/// bei Multi-Reader-tick-Loops zu vermeiden — der gleiche Proxy-
/// Locator-Set wird ueber alle Submessages eines Proxies wiederverwendet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundDatagram {
    /// Wire-Bytes (RTPS-Header + N Submessages).
    pub bytes: Vec<u8>,
    /// Ziel-Locators. Transport-Layer sendet an alle.
    pub targets: Rc<Vec<Locator>>,
}

/// Grund, warum [`MessageBuilder::try_add_submessage`] ablehnt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    /// Submessage passt nicht mehr ins MTU-Budget.
    WouldExceedMtu {
        /// Byte-Anzahl, die nicht mehr passt (inkl. Submessage-Header).
        needed: usize,
        /// Verbleibendes Budget.
        remaining: usize,
    },
    /// Submessage-Body > u16::MAX (Wire-Feld `octetsToNextHeader`).
    BodyTooLarge,
}

/// Submessage-Aggregator.
///
/// Wird bei `open` mit einer vor-allokierten Byte-Liste beginnend beim
/// RTPS-Header initialisiert. Submessages werden per `try_add_submessage`
/// angehaengt; bei Full muss Caller `finish()` + neuen Builder.
#[derive(Debug)]
pub struct MessageBuilder {
    bytes: Vec<u8>,
    targets: Rc<Vec<Locator>>,
    mtu: usize,
    submsg_count: usize,
}

impl MessageBuilder {
    /// Oeffnet einen neuen Builder mit gegebenem RTPS-Header,
    /// Zielen und MTU-Budget.
    ///
    /// Panics: wenn `mtu` kleiner als der RTPS-Header (20 Byte).
    #[must_use]
    pub fn open(header: RtpsHeader, targets: Rc<Vec<Locator>>, mtu: usize) -> Self {
        assert!(
            mtu >= 20,
            "MTU must accommodate at least the 20-byte RTPS header"
        );
        let mut bytes = Vec::with_capacity(mtu);
        bytes.extend_from_slice(&header.to_bytes());
        Self {
            bytes,
            targets,
            mtu,
            submsg_count: 0,
        }
    }

    /// Anzahl bisher eingefuegter Submessages.
    #[must_use]
    pub fn submsg_count(&self) -> usize {
        self.submsg_count
    }

    /// True wenn der Builder nur den RTPS-Header enthaelt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.submsg_count == 0
    }

    /// Aktuelle Gesamt-Byte-Zahl (Header + bereits angehaengte
    /// Submessages).
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Verbleibendes Budget in Bytes.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.mtu.saturating_sub(self.bytes.len())
    }

    /// Versucht, eine Submessage anzuhaengen. Liefert
    /// [`AddError::WouldExceedMtu`], wenn sie nicht mehr reinpasst —
    /// dann ist `finish()` + neuer Builder faellig.
    ///
    /// `flags` enthaelt nur die **submessage-spezifischen** Flags
    /// (F, L, Q, H, K, N etc.). Das E-Bit (Little-Endian) setzt der
    /// Builder selbst, konsistent fuer das ganze Datagramm.
    ///
    /// # Errors
    /// - [`AddError::WouldExceedMtu`] bei Size-Overflow.
    /// - [`AddError::BodyTooLarge`] wenn `body.len() > u16::MAX`.
    pub fn try_add_submessage(
        &mut self,
        id: SubmessageId,
        flags: u8,
        body: &[u8],
    ) -> Result<(), AddError> {
        let body_len = u16::try_from(body.len()).map_err(|_| AddError::BodyTooLarge)?;
        let needed = SubmessageHeader::WIRE_SIZE + body.len();
        if self.bytes.len() + needed > self.mtu {
            return Err(AddError::WouldExceedMtu {
                needed,
                remaining: self.remaining(),
            });
        }
        let sh = SubmessageHeader {
            submessage_id: id,
            flags: flags | FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: body_len,
        };
        self.bytes.extend_from_slice(&sh.to_bytes());
        self.bytes.extend_from_slice(body);
        self.submsg_count += 1;
        Ok(())
    }

    /// Wandelt in ein fertiges [`OutboundDatagram`] um.
    ///
    /// Liefert `None` bei leerem Builder (nur RTPS-Header ohne
    /// Submessages) — das erlaubt Aufrufern, unbenutzte Builder
    /// einfach zu verwerfen, ohne vorher `is_empty()` pruefen zu
    /// muessen.
    #[must_use]
    pub fn finish(self) -> Option<OutboundDatagram> {
        if self.submsg_count == 0 {
            return None;
        }
        Some(OutboundDatagram {
            bytes: self.bytes,
            targets: self.targets,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::datagram::{ParsedSubmessage, decode_datagram};
    use crate::submessages::{DataSubmessage, HeartbeatSubmessage};
    use crate::wire_types::{EntityId, GuidPrefix, Locator, SequenceNumber, VendorId};

    fn sample_header() -> RtpsHeader {
        RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([1; 12]))
    }

    fn sample_data(sn: i64, payload_len: usize) -> DataSubmessage {
        DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            writer_id: EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            writer_sn: SequenceNumber(sn),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![0xAB; payload_len]),
        }
    }

    fn targets() -> Rc<Vec<Locator>> {
        Rc::new(alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7400)])
    }

    #[test]
    fn fresh_builder_contains_only_rtps_header() {
        let b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        assert!(b.is_empty());
        assert_eq!(b.len(), 20, "only RTPS header");
        assert_eq!(b.submsg_count(), 0);
        assert_eq!(b.remaining(), DEFAULT_MTU - 20);
    }

    #[test]
    fn single_data_submessage_fits_and_decodes() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body, flags) = sample_data(1, 10).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        let dg = b.finish().unwrap();
        assert_eq!(dg.targets.len(), 1);
        let parsed = decode_datagram(&dg.bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        assert!(matches!(&parsed.submessages[0], ParsedSubmessage::Data(_)));
    }

    #[test]
    fn four_small_datas_aggregate_into_one_datagram() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        for sn in 1..=4i64 {
            let (body, flags) = sample_data(sn, 10).write_body(true);
            b.try_add_submessage(SubmessageId::Data, flags, &body)
                .unwrap();
        }
        let dg = b.finish().unwrap();
        let parsed = decode_datagram(&dg.bytes).unwrap();
        // 4 DATA-Submessages in einem Datagramm
        let data_count = parsed
            .submessages
            .iter()
            .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
            .count();
        assert_eq!(data_count, 4);
    }

    #[test]
    fn overflow_rejects_with_would_exceed_mtu() {
        let mtu = 100; // sehr klein
        let mut b = MessageBuilder::open(sample_header(), targets(), mtu);
        // 1 DATA mit 50-Byte-Payload passt (20 hdr + 4 sub_hdr + 20 body + 50 payload = 94)
        let (body, flags) = sample_data(1, 50).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        // 2. DATA wuerde sprengen
        let (body2, flags2) = sample_data(2, 50).write_body(true);
        let res = b.try_add_submessage(SubmessageId::Data, flags2, &body2);
        assert!(matches!(res, Err(AddError::WouldExceedMtu { .. })));
        assert_eq!(b.submsg_count(), 1, "first add must still be counted");
    }

    #[test]
    fn overflow_allows_caller_to_open_new_builder() {
        let mtu = 100;
        let (body, flags) = sample_data(1, 50).write_body(true);
        let mut out: Vec<OutboundDatagram> = Vec::new();
        let mut b = MessageBuilder::open(sample_header(), targets(), mtu);

        for sn in 1..=3i64 {
            let (body_n, flags_n) = sample_data(sn, 50).write_body(true);
            if b.try_add_submessage(SubmessageId::Data, flags_n, &body_n)
                .is_err()
            {
                out.push(b.finish().unwrap());
                b = MessageBuilder::open(sample_header(), targets(), mtu);
                b.try_add_submessage(SubmessageId::Data, flags_n, &body_n)
                    .unwrap();
            }
        }
        if !b.is_empty() {
            out.push(b.finish().unwrap());
        }
        let _ = flags;
        let _ = body;
        // 3 DATAs, je 1 pro Datagramm (weil MTU 100 nur 1 passt)
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn finish_on_empty_builder_returns_none() {
        let b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        assert!(b.finish().is_none());
    }

    #[test]
    fn piggyback_heartbeat_after_data_aggregates() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body, flags) = sample_data(1, 10).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        let hb = HeartbeatSubmessage {
            reader_id: EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            writer_id: EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(1),
            count: 1,
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        };
        let (hb_body, hb_flags) = hb.write_body(true);
        b.try_add_submessage(SubmessageId::Heartbeat, hb_flags, &hb_body)
            .unwrap();
        let dg = b.finish().unwrap();
        let parsed = decode_datagram(&dg.bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        assert!(matches!(&parsed.submessages[0], ParsedSubmessage::Data(_)));
        assert!(matches!(
            &parsed.submessages[1],
            ParsedSubmessage::Heartbeat(h) if h.final_flag
        ));
    }

    #[test]
    fn builder_propagates_little_endian_flag_e() {
        // Wir schreiben eine DATA mit Body-LE. Der Builder soll
        // automatisch das E-Bit im Submessage-Header setzen.
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body, _flags_from_write) = sample_data(1, 10).write_body(true);
        // Caller uebergibt flags ohne E-Bit; Builder muss es setzen.
        b.try_add_submessage(SubmessageId::Data, 0, &body).unwrap();
        let dg = b.finish().unwrap();
        // Submessage-Header-Byte 1 (Flags) muss E-Bit gesetzt haben
        let sub_header_flags = dg.bytes[21]; // 20 bytes RTPS header + 1 byte id
        assert_eq!(
            sub_header_flags & FLAG_E_LITTLE_ENDIAN,
            FLAG_E_LITTLE_ENDIAN
        );
    }

    #[test]
    #[should_panic(expected = "MTU must accommodate")]
    fn open_panics_on_mtu_below_header() {
        let _ = MessageBuilder::open(sample_header(), targets(), 10);
    }

    #[test]
    fn body_too_large_rejected() {
        let mut b = MessageBuilder::open(sample_header(), targets(), 100_000);
        let oversize = alloc::vec![0u8; u16::MAX as usize + 1];
        let res = b.try_add_submessage(SubmessageId::Data, 0, &oversize);
        assert!(matches!(res, Err(AddError::BodyTooLarge)));
    }
}
