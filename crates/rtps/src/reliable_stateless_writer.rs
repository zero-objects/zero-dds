// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ReliableStatelessWriter` — DDSI-RTPS 2.5 §8.4.8.2 (Reliable
//! StatelessWriter, T1-T12).
//!
//! In contrast to the StatefulWriter (`reliable_writer.rs`), the
//! StatelessWriter carries **no per-reader proxy state**. Instead
//! there is exactly one list of reply locators (typically multicast)
//! and a single shared acked state, which derives the
//! `lowest_unacked_sn` from the pool of all incoming ACKNACKs.
//!
//! Real-world use cases are rare — the DDSI-RTPS spec lists
//! reliable-stateless as optional, and Cyclone DDS / FastDDS
//! use only the stateful path. ZeroDDS implements the
//! stateless-reliable path for spec completeness (K3b-D).
//!
//! # T1-T12 Transitions (Spec Tab. 8.46)
//!
//! The spec defines a 12-state transition matrix. We map
//! it to the following API methods:
//!
//! | T  | Trigger                            | API                         |
//! |----|------------------------------------|-----------------------------|
//! | T1 | new_change(kind, payload)          | [`Self::new_change`]        |
//! | T2 | RESPONSIVE-Tick → DATA an alle     | [`Self::tick`] → DATA-Burst |
//! | T3 | RESPONSIVE-Tick → HEARTBEAT        | [`Self::tick`] → HB         |
//! | T4 | ACKNACK received (FinalFlag=0)    | [`Self::handle_acknack`]    |
//! | T5 | ACKNACK FinalFlag=1 (no NACK)       | [`Self::handle_acknack`]    |
//! | T6 | requested-Retransmit               | [`Self::tick`] (NACK-Drain) |
//! | T7 | unsent_changes empty + ACKED-bound  | [`Self::is_acked_to`]       |
//! | T8 | sample purge (Cache-LowWater)      | [`Self::purge_acked`]       |
//! | T9 | new_change-Boundary (KeepLast)     | [`HistoryCache::insert`]    |
//! | T10| Lease-Timeout / shutdown           | [`Self::shutdown`]          |
//! | T11| Locator-List Update                | [`Self::set_locators`]      |
//! | T12| Stats-Snapshot (Diagnose)          | [`Self::stats`]             |

extern crate alloc;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::history_cache::{CacheChange, ChangeKind, HistoryCache, HistoryKind};
use crate::message_builder::OutboundDatagram;
use crate::submessages::{AckNackSubmessage, DataSubmessage, HeartbeatSubmessage};
use crate::wire_types::{EntityId, Guid, GuidPrefix, Locator, SequenceNumber, VendorId};

/// `ReliableStatelessWriter` — Spec §8.4.8.2.
pub struct ReliableStatelessWriter {
    /// Own GUID (prefix + EntityId).
    guid: Guid,
    /// VendorId (RTPS header).
    vendor_id: VendorId,
    /// HistoryCache (KeepAll or KeepLast — the spec allows both).
    cache: HistoryCache,
    /// Next sequence number (writer-assigned, monotonic).
    next_sn: i64,
    /// Reply locator list (typically multicast).
    locators: Vec<Locator>,
    /// Heartbeat counter (Spec §8.3.8.6 — wraps u32).
    heartbeat_count: u32,
    /// Pending ACKNACK-requested SNs (shared pool of all readers).
    requested: BTreeSet<SequenceNumber>,
    /// Lowest un-acked SN (`min(base)` of all ACKNACKs ever seen).
    /// `0` means: no ACKNACK seen yet — no assumptions.
    lowest_unacked: i64,
    /// Heartbeat period (tick-driven).
    heartbeat_period: Duration,
    /// Last heartbeat tick.
    last_heartbeat: Duration,
    /// Maximum packets per tick (DoS cap for retransmits).
    max_per_tick: usize,
}

/// Statistics snapshot (T12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReliableStatelessStats {
    /// Number of cache entries.
    pub cached_changes: usize,
    /// Number of pending retransmits (`requested.len()`).
    pub pending_retransmits: usize,
    /// Current `lowest_unacked` (0 = still unknown).
    pub lowest_unacked: i64,
    /// Heartbeat counter (modulo u32).
    pub heartbeat_count: u32,
}

impl ReliableStatelessWriter {
    /// Constructor.
    #[must_use]
    pub fn new(
        prefix: GuidPrefix,
        entity_id: EntityId,
        vendor_id: VendorId,
        history: HistoryKind,
        capacity: usize,
        heartbeat_period: Duration,
    ) -> Self {
        Self {
            guid: Guid::new(prefix, entity_id),
            vendor_id,
            cache: HistoryCache::new_with_kind(history, capacity),
            next_sn: 1,
            locators: Vec::new(),
            heartbeat_count: 0,
            requested: BTreeSet::new(),
            lowest_unacked: 0,
            heartbeat_period,
            last_heartbeat: Duration::ZERO,
            max_per_tick: 16,
        }
    }

    /// Own GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Sets a new locator list (T11).
    pub fn set_locators(&mut self, locators: Vec<Locator>) {
        self.locators = locators;
    }

    /// Sets the DoS cap for retransmits per tick.
    pub fn set_max_per_tick(&mut self, n: usize) {
        self.max_per_tick = n;
    }

    /// T1 — `new_change`: stores a sample in the cache. Returns the
    /// assigned sequence number.
    ///
    /// # Errors
    /// `ValueOutOfRange` on SN overflow or cache capacity limit.
    pub fn new_change(
        &mut self,
        kind: ChangeKind,
        payload: Vec<u8>,
    ) -> Result<SequenceNumber, WireError> {
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "reliable stateless writer SN overflow",
            })?;
        let change = match kind {
            ChangeKind::Alive => CacheChange::alive(sn, payload),
            other => {
                // Dispose/Unregister + Filtered use the same
                // constructor; the kind is preserved in the cache.
                let mut c = CacheChange::alive(sn, payload);
                c.kind = other;
                c
            }
        };
        self.cache
            .insert(change)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "reliable stateless writer cache full",
            })?;
        Ok(sn)
    }

    /// T4/T5 — processes an incoming ACKNACK. Updates
    /// `lowest_unacked` (max of all `base` ever seen — once-acked-
    /// always-acked) and takes `requested` SNs into the retransmit pool.
    pub fn handle_acknack(&mut self, ack: &AckNackSubmessage) {
        let base = ack.reader_sn_state.bitmap_base.0;
        if base > self.lowest_unacked {
            self.lowest_unacked = base;
            // Remove acked SNs from the retransmit pool (everything < base).
            self.requested.retain(|sn| sn.0 >= base);
        }
        for sn in ack.reader_sn_state.iter_set() {
            self.requested.insert(sn);
        }
    }

    /// T7 — `is_acked_to(sn)`: all SNs up to and including `sn` are
    /// acknowledged by at least one reader.
    #[must_use]
    pub fn is_acked_to(&self, sn: SequenceNumber) -> bool {
        sn.0 < self.lowest_unacked
    }

    /// T8 — purges all cache entries that are `lowest_unacked - 1` or
    /// smaller. Returns the number of removed samples.
    pub fn purge_acked(&mut self) -> usize {
        if self.lowest_unacked <= 1 {
            return 0;
        }
        let cutoff = SequenceNumber(self.lowest_unacked - 1);
        self.cache.remove_up_to(cutoff)
    }

    /// T2/T3/T6 — tick. Returns a list of datagrams the
    /// caller should send via UDP to all `locators`. Order:
    /// 1. Pending retransmits (max `max_per_tick`).
    /// 2. If `now >= last_heartbeat + heartbeat_period` AND the cache is
    ///    not empty: one HEARTBEAT.
    ///
    /// # Errors
    /// Wire encode error on DATA/HEARTBEAT encoding.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        use alloc::rc::Rc;
        let mut out = Vec::new();
        let targets = Rc::new(self.locators.clone());
        let header = RtpsHeader::new(self.vendor_id, self.guid.prefix);
        let mut sent = 0usize;

        // 1. Retransmits.
        let retransmits: Vec<SequenceNumber> = self
            .requested
            .iter()
            .take(self.max_per_tick)
            .copied()
            .collect();
        for sn in &retransmits {
            if let Some(change) = self.cache.get(*sn) {
                let data = DataSubmessage {
                    extra_flags: 0,
                    reader_id: EntityId::UNKNOWN, // Stateless: to all readers.
                    writer_id: self.guid.entity_id,
                    writer_sn: *sn,
                    inline_qos: None,
                    key_flag: false,
                    non_standard_flag: false,
                    serialized_payload: alloc::sync::Arc::clone(&change.payload),
                };
                let bytes = crate::datagram::encode_data_datagram(header, &[data])?;
                out.push(OutboundDatagram {
                    bytes,
                    targets: Rc::clone(&targets),
                });
                sent += 1;
            }
            self.requested.remove(sn);
            if sent >= self.max_per_tick {
                break;
            }
        }

        // 2. Heartbeat tick.
        if now >= self.last_heartbeat + self.heartbeat_period && !self.cache.is_empty() {
            self.last_heartbeat = now;
            self.heartbeat_count = self.heartbeat_count.wrapping_add(1);
            let first = self
                .cache
                .min_sn()
                .unwrap_or(SequenceNumber(self.lowest_unacked));
            let last = self
                .cache
                .max_sn()
                .unwrap_or(SequenceNumber(self.next_sn - 1));
            let hb = HeartbeatSubmessage {
                reader_id: EntityId::UNKNOWN,
                writer_id: self.guid.entity_id,
                first_sn: first,
                last_sn: last,
                count: self.heartbeat_count as i32,
                final_flag: false,
                liveliness_flag: false,
                group_info: None,
            };
            let (body, flags) = hb.write_body(true);
            let sh = crate::submessage_header::SubmessageHeader {
                submessage_id: crate::submessage_header::SubmessageId::Heartbeat,
                flags,
                octets_to_next_header: body.len() as u16,
            };
            let mut bytes = header.to_bytes().to_vec();
            bytes.extend_from_slice(&sh.to_bytes());
            bytes.extend_from_slice(&body);
            out.push(OutboundDatagram {
                bytes,
                targets: Rc::clone(&targets),
            });
        }

        Ok(out)
    }

    /// T10 — shutdown: clears the cache + retransmit pool.
    pub fn shutdown(&mut self) {
        if let Some(max) = self.cache.max_sn() {
            let _ = self.cache.remove_up_to(max);
        }
        self.requested.clear();
    }

    /// T12 — diagnosis snapshot.
    #[must_use]
    pub fn stats(&self) -> ReliableStatelessStats {
        ReliableStatelessStats {
            cached_changes: self.cache.len(),
            pending_retransmits: self.requested.len(),
            lowest_unacked: self.lowest_unacked,
            heartbeat_count: self.heartbeat_count,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::submessages::SequenceNumberSet;

    fn make_writer() -> ReliableStatelessWriter {
        ReliableStatelessWriter::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([1, 2, 3]),
            VendorId::ZERODDS,
            HistoryKind::KeepAll,
            32,
            Duration::from_millis(100),
        )
    }

    #[test]
    fn new_change_assigns_monotonic_sn_t1() {
        let mut w = make_writer();
        let sn1 = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        let sn2 = w.new_change(ChangeKind::Alive, alloc::vec![2]).unwrap();
        let sn3 = w.new_change(ChangeKind::Alive, alloc::vec![3]).unwrap();
        assert_eq!(sn1.0, 1);
        assert_eq!(sn2.0, 2);
        assert_eq!(sn3.0, 3);
    }

    #[test]
    fn handle_acknack_advances_lowest_unacked_t4() {
        let mut w = make_writer();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![2]).unwrap();
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(2), &[]),
            count: 1,
            final_flag: true,
        };
        w.handle_acknack(&ack);
        assert_eq!(w.stats().lowest_unacked, 2);
    }

    #[test]
    fn handle_acknack_only_advances_t4_once_acked_always_acked() {
        let mut w = make_writer();
        let ack_high = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(10), &[]),
            count: 1,
            final_flag: true,
        };
        w.handle_acknack(&ack_high);
        let ack_low = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(3), &[]),
            count: 2,
            final_flag: true,
        };
        // A lower ACKNACK must not regress.
        w.handle_acknack(&ack_low);
        assert_eq!(w.stats().lowest_unacked, 10);
    }

    #[test]
    fn handle_acknack_with_requested_bits_queues_retransmits_t6() {
        let mut w = make_writer();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![2]).unwrap();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![3]).unwrap();
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(
                SequenceNumber(1),
                &[SequenceNumber(2), SequenceNumber(3)],
            ),
            count: 1,
            final_flag: false,
        };
        w.handle_acknack(&ack);
        assert_eq!(w.stats().pending_retransmits, 2);
    }

    #[test]
    fn is_acked_to_t7() {
        let mut w = make_writer();
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(5), &[]),
            count: 1,
            final_flag: true,
        };
        w.handle_acknack(&ack);
        assert!(w.is_acked_to(SequenceNumber(4)));
        assert!(w.is_acked_to(SequenceNumber(1)));
        assert!(!w.is_acked_to(SequenceNumber(5)));
    }

    #[test]
    fn purge_acked_t8_removes_acked_samples() {
        let mut w = make_writer();
        for i in 1..=5 {
            let _ = w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(4), &[]),
            count: 1,
            final_flag: true,
        };
        w.handle_acknack(&ack);
        let purged = w.purge_acked();
        assert_eq!(purged, 3);
        assert_eq!(w.stats().cached_changes, 2);
    }

    #[test]
    fn tick_emits_heartbeat_t3() {
        let mut w = make_writer();
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        w.set_locators(alloc::vec![Locator::udp_v4([10, 0, 0, 1], 7400)]);
        let datagrams = w.tick(Duration::from_millis(150)).unwrap();
        assert!(!datagrams.is_empty(), "tick should emit HB");
        assert_eq!(w.stats().heartbeat_count, 1);
    }

    #[test]
    fn tick_does_not_emit_heartbeat_when_cache_empty() {
        let mut w = make_writer();
        w.set_locators(alloc::vec![Locator::udp_v4([10, 0, 0, 1], 7400)]);
        let datagrams = w.tick(Duration::from_millis(150)).unwrap();
        assert!(datagrams.is_empty(), "empty cache → no HB");
    }

    #[test]
    fn tick_emits_retransmits_for_requested_sns_t6() {
        let mut w = make_writer();
        for i in 1..=3 {
            let _ = w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.set_locators(alloc::vec![Locator::udp_v4([10, 0, 0, 1], 7400)]);
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(
                SequenceNumber(1),
                &[SequenceNumber(2), SequenceNumber(3)],
            ),
            count: 1,
            final_flag: false,
        };
        w.handle_acknack(&ack);
        let datagrams = w.tick(Duration::from_millis(0)).unwrap();
        // 2 retransmits — no HB (tick=0).
        assert_eq!(datagrams.len(), 2);
        assert_eq!(w.stats().pending_retransmits, 0);
    }

    #[test]
    fn tick_caps_retransmits_at_max_per_tick() {
        let mut w = make_writer();
        for i in 1..=5 {
            let _ = w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.set_locators(alloc::vec![Locator::udp_v4([10, 0, 0, 1], 7400)]);
        w.set_max_per_tick(2);
        let ack = AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(
                SequenceNumber(1),
                &[
                    SequenceNumber(2),
                    SequenceNumber(3),
                    SequenceNumber(4),
                    SequenceNumber(5),
                ],
            ),
            count: 1,
            final_flag: false,
        };
        w.handle_acknack(&ack);
        let datagrams = w.tick(Duration::from_millis(0)).unwrap();
        assert!(datagrams.len() <= 2, "max_per_tick cap respected");
        assert!(w.stats().pending_retransmits >= 2, "rest stays queued");
    }

    #[test]
    fn shutdown_clears_state_t10() {
        let mut w = make_writer();
        for i in 1..=3 {
            let _ = w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.shutdown();
        assert_eq!(w.stats().cached_changes, 0);
        assert_eq!(w.stats().pending_retransmits, 0);
    }

    #[test]
    fn set_locators_t11_replaces_list() {
        let mut w = make_writer();
        w.set_locators(alloc::vec![Locator::udp_v4([1, 1, 1, 1], 100)]);
        w.set_locators(alloc::vec![Locator::udp_v4([2, 2, 2, 2], 200)]);
        // Roundtrip: only 1 locator, the second.
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        let datagrams = w.tick(Duration::from_millis(150)).unwrap();
        assert!(!datagrams.is_empty());
        assert_eq!(datagrams[0].targets.len(), 1);
    }

    #[test]
    fn heartbeat_count_wraps_at_u32_max_t3_modular() {
        // Spec §8.4.15.7: count modulo u32. Set heartbeat_count near
        // u32::MAX and verify the wrap.
        let mut w = make_writer();
        w.heartbeat_count = u32::MAX - 1;
        let _ = w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        w.set_locators(alloc::vec![Locator::udp_v4([1, 2, 3, 4], 7400)]);
        let _ = w.tick(Duration::from_millis(150)).unwrap();
        assert_eq!(w.stats().heartbeat_count, u32::MAX);
        // Reset last_heartbeat so the next tick fires again.
        w.last_heartbeat = Duration::ZERO;
        let _ = w.tick(Duration::from_millis(150)).unwrap();
        // Wrap-around: u32::MAX + 1 → 0.
        assert_eq!(w.stats().heartbeat_count, 0);
    }

    #[test]
    fn stats_snapshot_t12() {
        let mut w = make_writer();
        for i in 1..=4 {
            let _ = w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        let s = w.stats();
        assert_eq!(s.cached_changes, 4);
        assert_eq!(s.pending_retransmits, 0);
        assert_eq!(s.lowest_unacked, 0);
        assert_eq!(s.heartbeat_count, 0);
    }
}
