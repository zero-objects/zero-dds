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
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::header::RtpsHeader;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::wire_types::Locator;

/// Default MTU for aggregation (Ethernet 1500 − 20 IP − 8 UDP).
pub const DEFAULT_MTU: usize = 1472;

/// A fully aggregated datagram with targets.
///
/// `targets` is shared as `Arc<Vec<Locator>>`: a writer keeps one per
/// matched reader and every datagram to that reader holds it, so a write
/// does not copy the locator list. `Arc` and not `Rc`, because the writer
/// that keeps it moves between threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundDatagram {
    /// Wire bytes (RTPS header + N submessages).
    pub bytes: Vec<u8>,
    /// Target locators. The transport layer sends to all.
    pub targets: Arc<Vec<Locator>>,
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
    /// The datagram does not end on a 32-bit boundary: the last submessage's
    /// length is not a multiple of four, which only the last submessage of a
    /// message may be (DDSI-RTPS 2.5 §9.4.1, §9.4.5.1.3). The next
    /// submessage belongs in a new datagram.
    Unaligned,
}

/// Submessage aggregator.
///
/// Initialized on `open` with a pre-allocated byte list starting at the
/// RTPS header. Submessages are appended via `try_add_submessage`;
/// when full the caller must `finish()` + open a new builder.
#[derive(Debug)]
pub struct MessageBuilder {
    bytes: Vec<u8>,
    targets: Arc<Vec<Locator>>,
    mtu: usize,
    submsg_count: usize,
    content_count: usize,
}

/// True for the submessages that only set receiver state and carry no
/// information of their own (RTPS 2.5 §8.3.8.8 `InfoDestination`, §8.3.8.11
/// `InfoTimestamp`, §8.3.8.10 `InfoSource`, §8.3.8.9 `InfoReply`, §8.3.8.13
/// `Pad`). A message built from these alone is a no-op at the receiver: it
/// updates the receiver state, which is then discarded at the end of the
/// message because no submessage ever consumed it.
fn is_framing_only(id: SubmessageId) -> bool {
    matches!(
        id,
        SubmessageId::Pad
            | SubmessageId::InfoTs
            | SubmessageId::InfoSrc
            | SubmessageId::InfoReplyIp4
            | SubmessageId::InfoDst
            | SubmessageId::InfoReply
    )
}

impl MessageBuilder {
    /// Opens a new builder with the given RTPS header,
    /// targets and MTU budget.
    ///
    /// Panics: if `mtu` is smaller than the RTPS header (20 bytes).
    #[must_use]
    pub fn open(header: RtpsHeader, targets: Arc<Vec<Locator>>, mtu: usize) -> Self {
        assert!(
            mtu >= 20,
            "MTU must accommodate at least the 20-byte RTPS header"
        );
        // Reserved up to an Ethernet datagram, not up to `mtu`: a writer
        // whose readers are all on this host opens every message with the
        // 64 000-octet loopback MTU, and reserving that per message was a
        // 64 kB allocation and release per sample (measured on the Linux
        // test host). A larger message grows its buffer once, when the
        // submessage that needs it is appended.
        let mut bytes = Vec::with_capacity(mtu.min(DEFAULT_MTU));
        bytes.extend_from_slice(&header.to_bytes());
        Self {
            bytes,
            targets,
            mtu,
            submsg_count: 0,
            content_count: 0,
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

    /// True once at least one submessage was appended that is not pure
    /// framing (`is_framing_only`) — i.e. once the message would actually
    /// tell the receiver something. [`Self::finish`] produces a datagram only
    /// in that case.
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.content_count > 0
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
    /// - [`AddError::Unaligned`] after a submessage whose length is not a
    ///   multiple of four.
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
        // derives the payload length from `octetsToNextHeader`). A body
        // that is not a multiple of four is therefore the last submessage
        // of its datagram, and the builder refuses to append after it.
        // It used to leave that check to the callers, and the reliable
        // writer's resend path put a HEARTBEAT behind a TypeLookup request
        // of 123 octets: Cyclone DDS dropped the message as malformed and
        // never answered the request.
        self.check_aligned()?;
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
        if !is_framing_only(id) {
            self.content_count += 1;
        }
        Ok(())
    }

    /// [`AddError::Unaligned`] when the datagram so far does not end on a
    /// 32-bit boundary — its last submessage has to stay the last.
    fn check_aligned(&self) -> Result<(), AddError> {
        if self.bytes.len() % 4 == 0 {
            Ok(())
        } else {
            Err(AddError::Unaligned)
        }
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
        self.check_aligned()?;
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
        if !is_framing_only(id) {
            self.content_count += 1;
        }
        Ok(())
    }

    /// Converts into a finished [`OutboundDatagram`].
    ///
    /// Returns `None` for a builder that carries no content — either nothing
    /// at all (just the RTPS header), or nothing but framing submessages
    /// (`is_framing_only`). This lets callers simply discard unused builders
    /// without having to check [`Self::has_content`] first.
    ///
    /// The framing case is not hypothetical. `ReliableWriter::tick` opens its
    /// per-proxy builder through `open_builder`, which prepends INFO_DST
    /// straight away; when the tick then finds no resend to serve and no
    /// HEARTBEAT due, the builder still held one submessage. Every such tick
    /// put a 36-byte header+INFO_DST datagram on the wire, per reader proxy
    /// and per announced locator, that told the receiver nothing. Because a
    /// received user datagram raises a tick wake, the SEDP writers ran this at
    /// the sample rate, and the two participants kept each other awake with
    /// them: measured on the Linux test host, a 1 kB best-effort stream cost four `sendto`
    /// per sample, of which two carried no information.
    #[must_use]
    pub fn finish(self) -> Option<OutboundDatagram> {
        if self.content_count == 0 {
            return None;
        }
        Some(OutboundDatagram {
            bytes: self.bytes,
            targets: self.targets,
        })
    }
}

/// Length of the RTPS message header (DDSI-RTPS 2.5 §8.3.3): protocol id,
/// version, vendor id, GUID prefix.
pub const RTPS_HEADER_LEN: usize = 20;

/// Appends the submessages of the finished message `next` to the already
/// finished message `pending`, so both travel in a single datagram.
///
/// DDSI-RTPS 2.5 §8.3.3 defines a Message as one Header followed by a
/// sequence of Submessages, and §8.3.7 gives every Submessage its own
/// self-delimiting header — so a receiver walks the submessages of a message
/// one after the other and does not care how many arrived together. Two
/// messages that carry the **same** header therefore concatenate into one
/// legal message by dropping the second header, and the receiver processes
/// exactly the same submessage sequence it would have seen in two datagrams.
///
/// This is the merge step behind writer-side batching: a `write()` produces a
/// finished per-proxy datagram, and the batch keeps merging the next one into
/// it until `max_message_size` is reached.
///
/// Returns `false` and leaves `pending` untouched when the merge is not
/// allowed:
///
/// * either side is shorter than a header,
/// * the two headers differ — a different GUID prefix, vendor or protocol
///   version is a different message, and folding it in would relabel its
///   submessages as someone else's,
/// * or the result would exceed `max_message_size`.
///
/// The receiver state a merged message carries is the concatenation of both:
/// an INFO_DST or INFO_TS in the second part still precedes the submessages
/// it belongs to, and one in the first part still applies to the second — the
/// same reading order §8.3.4 prescribes for submessages of one message.
#[must_use]
pub fn try_merge_message(pending: &mut Vec<u8>, next: &[u8], max_message_size: usize) -> bool {
    if pending.len() < RTPS_HEADER_LEN || next.len() < RTPS_HEADER_LEN {
        return false;
    }
    if pending[..RTPS_HEADER_LEN] != next[..RTPS_HEADER_LEN] {
        return false;
    }
    let body = &next[RTPS_HEADER_LEN..];
    if body.is_empty() {
        // A header with nothing behind it adds no submessage; merging it in
        // would silently claim a sample went out that carries nothing.
        return false;
    }
    if pending.len() + body.len() > max_message_size {
        return false;
    }
    pending.extend_from_slice(body);
    true
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from(alloc::vec![0xAB; payload_len]),
        }
    }

    fn targets() -> Arc<Vec<Locator>> {
        Arc::new(alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7400)])
    }

    /// A submessage whose length is not a multiple of four ends its
    /// datagram (DDSI-RTPS 2.5 §9.4.1): the builder refuses to append after
    /// it, and appends again once it is aligned.
    #[test]
    fn nothing_is_appended_after_an_unaligned_submessage() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        b.try_add_submessage(SubmessageId::Data, 0, &[0u8; 23])
            .unwrap();
        assert_eq!(
            b.try_add_submessage(SubmessageId::Heartbeat, 0, &[0u8; 28]),
            Err(AddError::Unaligned)
        );
        assert_eq!(
            b.try_add_submessage_split(SubmessageId::Data, 0, &[0u8; 20], &[1, 2, 3]),
            Err(AddError::Unaligned)
        );
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        b.try_add_submessage(SubmessageId::Data, 0, &[0u8; 24])
            .unwrap();
        b.try_add_submessage(SubmessageId::Heartbeat, 0, &[0u8; 28])
            .unwrap();
    }

    /// RTPS 2.5 §8.3.8.8: a message whose only submessage is INFO_DST sets the
    /// receiver's destination prefix and then ends, so the receiver discards
    /// the state again without ever having used it. `finish` must not hand
    /// such a message to the send path.
    ///
    /// This is the shape `ReliableWriter::tick` produces on an idle proxy:
    /// `open_builder` prepends INFO_DST, and when nothing else is due the
    /// builder is finished as-is.
    #[test]
    fn framing_only_builder_produces_no_datagram() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        b.try_add_submessage(SubmessageId::InfoDst, 0, &[7u8; 12])
            .unwrap();
        assert_eq!(b.submsg_count(), 1, "INFO_DST is on the wire buffer");
        assert!(!b.has_content(), "but it is not content");
        assert!(
            b.finish().is_none(),
            "a header+INFO_DST datagram tells the receiver nothing"
        );
    }

    /// The same builder must still produce a datagram once real content lands
    /// behind the framing — INFO_DST + HEARTBEAT is the normal idle-writer
    /// message and must keep going out unchanged.
    #[test]
    fn framing_plus_content_still_produces_a_datagram() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        b.try_add_submessage(SubmessageId::InfoDst, 0, &[7u8; 12])
            .unwrap();
        let hb = HeartbeatSubmessage {
            reader_id: EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            writer_id: EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(3),
            count: 1,
            final_flag: false,
            liveliness_flag: false,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        b.try_add_submessage(SubmessageId::Heartbeat, flags, &body)
            .unwrap();
        assert!(b.has_content());
        let dg = b.finish().expect("INFO_DST + HEARTBEAT must go out");
        let parsed = decode_datagram(&dg.bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2, "framing is preserved");
    }

    /// INFO_TS is framing too: an INFO_TS that never got its DATA is the same
    /// no-op message, and `append_info_ts` runs before the DATA is known to
    /// fit.
    #[test]
    fn info_ts_alone_produces_no_datagram() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        b.try_add_submessage(SubmessageId::InfoTs, 0, &[0u8; 8])
            .unwrap();
        assert!(b.finish().is_none());
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
        assert!(matches!(
            &parsed.submessages[0].submessage,
            ParsedSubmessage::Data(_)
        ));
    }

    #[test]
    fn four_small_datas_aggregate_into_one_datagram() {
        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        // Twelve octets each: an aligned DATA, which another may follow.
        for sn in 1..=4i64 {
            let (body, flags) = sample_data(sn, 12).write_body(true);
            b.try_add_submessage(SubmessageId::Data, flags, &body)
                .unwrap();
        }
        let dg = b.finish().unwrap();
        let parsed = decode_datagram(&dg.bytes).unwrap();
        // 4 DATA submessages in one datagram
        let data_count = parsed
            .contents()
            .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
            .count();
        assert_eq!(data_count, 4);
    }

    #[test]
    fn overflow_rejects_with_would_exceed_mtu() {
        let mtu = 100; // very small
        let mut b = MessageBuilder::open(sample_header(), targets(), mtu);
        // 1 DATA with a 48-byte payload fits (20 hdr + 4 sub_hdr + 20 body + 48 payload = 92)
        let (body, flags) = sample_data(1, 48).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        // 2nd DATA would overflow
        let (body2, flags2) = sample_data(2, 48).write_body(true);
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
        let (body, flags) = sample_data(1, 12).write_body(true);
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
        assert!(matches!(
            &parsed.submessages[0].submessage,
            ParsedSubmessage::Data(_)
        ));
        assert!(matches!(
            &parsed.submessages[1].submessage,
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
    fn merge_folds_a_second_message_into_the_first() {
        // Two finished single-DATA messages with the same header merge into
        // one message carrying both DATAs — what the receiver walks per
        // §8.3.4 is the same submessage sequence, in the same order.
        let mut a = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body_a, flags_a) = sample_data(1, 10).write_body(true);
        a.try_add_submessage(SubmessageId::Data, flags_a, &body_a)
            .unwrap();
        let mut merged = a.finish().unwrap().bytes;

        let mut b = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body_b, flags_b) = sample_data(2, 10).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags_b, &body_b)
            .unwrap();
        let second = b.finish().unwrap().bytes;

        assert!(try_merge_message(&mut merged, &second, 14_720));
        let parsed = decode_datagram(&merged).expect("merged message decodes");
        assert_eq!(parsed.submessages.len(), 2);
        for (i, want_sn) in [1i64, 2].iter().enumerate() {
            match &parsed.submessages[i].submessage {
                ParsedSubmessage::Data(d) => assert_eq!(d.writer_sn.0, *want_sn),
                other => panic!("submessage {i} is not a DATA: {other:?}"),
            }
        }
    }

    #[test]
    fn merge_refuses_a_foreign_header() {
        // A different GUID prefix is a different sender. Folding it in would
        // hand its submessages to the receiver under our prefix.
        let mut a = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body_a, flags_a) = sample_data(1, 10).write_body(true);
        a.try_add_submessage(SubmessageId::Data, flags_a, &body_a)
            .unwrap();
        let mut merged = a.finish().unwrap().bytes;
        let before = merged.clone();

        let foreign = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([9u8; 12]));
        let mut b = MessageBuilder::open(foreign, targets(), DEFAULT_MTU);
        let (body_b, flags_b) = sample_data(2, 10).write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags_b, &body_b)
            .unwrap();
        let second = b.finish().unwrap().bytes;

        assert!(!try_merge_message(&mut merged, &second, 14_720));
        assert_eq!(merged, before, "a refused merge leaves the batch untouched");
    }

    #[test]
    fn merge_refuses_to_exceed_the_message_size() {
        let mut a = MessageBuilder::open(sample_header(), targets(), DEFAULT_MTU);
        let (body_a, flags_a) = sample_data(1, 400).write_body(true);
        a.try_add_submessage(SubmessageId::Data, flags_a, &body_a)
            .unwrap();
        let mut merged = a.finish().unwrap().bytes;
        let second = merged.clone();
        let cap = merged.len() + 1;
        assert!(
            !try_merge_message(&mut merged, &second, cap),
            "a merge that would pass the cap is refused"
        );
        assert!(try_merge_message(&mut merged, &second, 14_720));
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
