// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MessageBuilder — submessage aggregation into a UDP datagram.
//!
//! Analogous to Fast-DDS `RTPSMessageGroup` (research WP 1.4). The writer
//! opens a builder per target locator set, appends several submessages,
//! and finalizes into an [`OutboundDatagram`]. Aggregation saves the
//! RTPS header + UDP overhead on SEDP announce-all rounds and small
//! samples.
//!
//! # Flush rules
//!
//! 1. **Size trigger**: `try_add_submessage` rejects if the body
//!    no longer fits the MTU. The caller must `finish()` + open a new builder.
//! 2. **DATA_FRAG goes alone**: the caller should not bundle DATA_FRAG
//!    submessages with others (a fragment is typically near the MTU).
//! 3. **Piggyback HEARTBEAT at the end**: the caller appends the HB after all
//!    DATAs, before `finish()`.
//! 4. **No INFO_DST**: phase 1 builds one datagram per proxy — the GuidPrefix
//!    is static. INFO_DST is added for multicast fan-out with mixed
//!    targets in phase 2.
//! 5. **No INFO_TS**: the writer has no source timestamps today.
//!
//! # Target locators
//!
//! A datagram goes to **all** `targets`. Typically: the unicast locators
//! of the remote reader, or a multicast locator. The transport layer puts
//! it on the wire once per locator.

extern crate alloc;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::header::RtpsHeader;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::wire_types::Locator;

/// Default MTU for aggregation (Ethernet 1500 − 20 IP − 8 UDP).
pub const DEFAULT_MTU: usize = 1472;

/// A fully aggregated datagram with targets.
///
/// `targets` is shared as `Rc<Vec<Locator>>` to avoid allocation overhead
/// in multi-reader tick loops — the same proxy
/// locator set is reused across all submessages of a proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundDatagram {
    /// Wire bytes (RTPS header + N submessages).
    pub bytes: Vec<u8>,
    /// Target locators. The transport layer sends to all.
    pub targets: Rc<Vec<Locator>>,
}

impl OutboundDatagram {
    /// Opt-6 (Spec `zerodds-zero-copy-1.0` §9): converts the
    /// internal Vec into a shared `Arc<[u8]>` representation
    /// **without a copy** (boxed-slice conversion is O(1) if `bytes`
    /// has no excess capacity; otherwise a single realloc).
    ///
    /// Useful for multi-reader hot paths: instead of passing `&dg.bytes` to
    /// each reader send and implicitly letting it `Vec::clone`,
    /// the caller takes the `Arc<[u8]>` out once and
    /// clones only the refcount. Eliminates the per-reader Vec clone.
    #[must_use]
    pub fn into_shared_bytes(self) -> alloc::sync::Arc<[u8]> {
        alloc::sync::Arc::from(self.bytes.into_boxed_slice())
    }
}

/// Reason why [`MessageBuilder::try_add_submessage`] rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    /// The submessage no longer fits the MTU budget.
    WouldExceedMtu {
        /// Number of bytes that no longer fit (incl. submessage header).
        needed: usize,
        /// Remaining budget.
        remaining: usize,
    },
    /// Submessage body > u16::MAX (wire field `octetsToNextHeader`).
    BodyTooLarge,
}

/// Submessage aggregator.
///
/// Initialized on `open` with a pre-allocated byte list starting at the
/// RTPS header. Submessages are appended via `try_add_submessage`;
/// when full the caller must `finish()` + open a new builder.
#[derive(Debug)]
pub struct MessageBuilder {
    bytes: Vec<u8>,
    targets: Rc<Vec<Locator>>,
    mtu: usize,
    submsg_count: usize,
}

impl MessageBuilder {
    /// Opens a new builder with the given RTPS header,
    /// targets and MTU budget.
    ///
    /// Panics: if `mtu` is smaller than the RTPS header (20 bytes).
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

    /// Number of submessages inserted so far.
    #[must_use]
    pub fn submsg_count(&self) -> usize {
        self.submsg_count
    }

    /// True if the builder contains only the RTPS header.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.submsg_count == 0
    }

    /// Current total byte count (header + already appended
    /// submessages).
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Remaining budget in bytes.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.mtu.saturating_sub(self.bytes.len())
    }

    /// Tries to append a submessage. Returns
    /// [`AddError::WouldExceedMtu`] if it no longer fits —
    /// then `finish()` + a new builder is due.
    ///
    /// `flags` contains only the **submessage-specific** flags
    /// (F, L, Q, H, K, N etc.). The E bit (little-endian) is set by the
    /// builder itself, consistently for the whole datagram.
    ///
    /// # Errors
    /// - [`AddError::WouldExceedMtu`] on size overflow.
    /// - [`AddError::BodyTooLarge`] if `body.len() > u16::MAX`.
    pub fn try_add_submessage(
        &mut self,
        id: SubmessageId,
        flags: u8,
        body: &[u8],
    ) -> Result<(), AddError> {
        // RTPS 2.5 §8.3.4.1: every submessage starts on a 32-bit
        // boundary. Since the submessage header measures 4 bytes, `body`
        // itself must be a multiple of 4 bytes so that a *following*
        // submessage starts aligned. The builder deliberately does NOT pad:
        // a pad counted in `octetsToNextHeader` would bleed through as
        // serializedData slack into the user payload (the receiver
        // derives the payload length from `octetsToNextHeader`). Whoever
        // co-frames multiple submessages checks the alignment themselves —
        // see `ReliableWriter::write_with_heartbeat`: it appends a
        // piggyback HEARTBEAT only to a DATA that already ends 4-aligned
        // and otherwise puts it into its own datagram.
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

    /// Like [`Self::try_add_submessage`], but the body in two slices —
    /// avoids an intermediate Vec when the caller already has `header_body`
    /// and `payload_tail` separately. Exactly the hot-
    /// path case for the DATA submessage: 20-byte fixed header (the caller
    /// writes into a stack buffer) + N byte serializedData (borrowed from
    /// the cache `Arc<[u8]>`). Without this variant the caller needs
    /// `Vec::with_capacity(20+N) + extend(header) + extend(payload)` —
    /// a heap alloc + copy of N bytes, which is fully eliminated here.
    /// `octets_to_next_header = header_body.len() + payload_tail.len()`.
    ///
    /// # Errors
    /// Same as [`Self::try_add_submessage`].
    pub fn try_add_submessage_split(
        &mut self,
        id: SubmessageId,
        flags: u8,
        header_body: &[u8],
        payload_tail: &[u8],
    ) -> Result<(), AddError> {
        let total_body = header_body.len() + payload_tail.len();
        let body_len = u16::try_from(total_body).map_err(|_| AddError::BodyTooLarge)?;
        let needed = SubmessageHeader::WIRE_SIZE + total_body;
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
        self.bytes.extend_from_slice(header_body);
        self.bytes.extend_from_slice(payload_tail);
        self.submsg_count += 1;
        Ok(())
    }

    /// Converts into a finished [`OutboundDatagram`].
    ///
    /// Returns `None` for an empty builder (only the RTPS header without
    /// submessages) — this lets callers simply discard unused builders
    /// without having to check `is_empty()`
    /// first.
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

    /// Opt-6 — into_shared_bytes returns Arc<[u8]> with identical
    /// wire content; an Arc::clone across multiple reader sends shares
    /// the buffer without a Vec clone.
    #[test]
    fn into_shared_bytes_preserves_wire_content_and_shares_via_arc() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body, flags) = sample_data(7, 16).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        let dg = b.finish().unwrap();
        let bytes_copy = dg.bytes.clone();
        let shared = dg.into_shared_bytes();
        // Content identical.
        assert_eq!(&shared[..], &bytes_copy[..]);
        // Arc::clone shares the buffer (one refcount, no realloc).
        let c1 = alloc::sync::Arc::clone(&shared);
        let c2 = alloc::sync::Arc::clone(&shared);
        assert_eq!(alloc::sync::Arc::strong_count(&shared), 3);
        assert_eq!(&c1[..], &c2[..]);
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
        // 4 DATA submessages in one datagram
        let data_count = parsed
            .submessages
            .iter()
            .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
            .count();
        assert_eq!(data_count, 4);
    }

    #[test]
    fn overflow_rejects_with_would_exceed_mtu() {
        let mtu = 100; // very small
        let mut b = MessageBuilder::open(sample_header(), targets(), mtu);
        // 1 DATA with a 50-byte payload fits (20 hdr + 4 sub_hdr + 20 body + 50 payload = 94)
        let (body, flags) = sample_data(1, 50).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        // 2nd DATA would overflow
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
        // 3 DATAs, 1 per datagram each (because MTU 100 fits only 1)
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
        // We write a DATA with body LE. The builder should
        // automatically set the E bit in the submessage header.
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body, _flags_from_write) = sample_data(1, 10).write_body(true);
        // The caller passes flags without the E bit; the builder must set it.
        b.try_add_submessage(SubmessageId::Data, 0, &body).unwrap();
        let dg = b.finish().unwrap();
        // Submessage header byte 1 (flags) must have the E bit set
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
