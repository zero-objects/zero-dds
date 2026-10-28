// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ReliableStatelessWriter` — DDSI-RTPS 2.5 §8.4.8.2 (Reliable
//! StatelessWriter, Table 8.64 T1 to T12) with the `ReaderLocator` of
//! §8.4.7.3 (Tables 8.57 and 8.58).
//!
//! A StatelessWriter keeps no state per matched reader but one
//! [`ReaderLocator`] per locator it sends to. Two machines run for each:
//!
//! - **push**: `announcing` and `pushing` (T1 to T5). A locator that has
//!   unsent changes is pushed them (T4: a GAP for the sequence numbers that
//!   are no longer in the cache, then the DATA); one that has none is
//!   announced to with a HEARTBEAT, FinalFlag set, every
//!   `heartbeatPeriod` (T5).
//! - **repair**: `waiting`, `must_repair`, `repairing` (T6 to T11). An
//!   ACKNACK is answered at the reply locators of the receiver state
//!   (T6, T8), which "is the only source of information for the
//!   StatelessWriter to determine where to send the reply to"; the
//!   requested changes go out after `nackResponseDelay` (T9, T10), a GAP for
//!   one that is no longer in the cache.
//!
//! Cyclone DDS and Fast DDS use only the stateful path; the type is used
//! outside this file by nobody yet.

extern crate alloc;
use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::history_cache::{CacheChange, ChangeKind, HistoryCache, HistoryKind};
use crate::message_builder::OutboundDatagram;
use crate::parameter_list::ParameterList;
use crate::receiver_state::ReceiverState;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::submessages::{
    AckNackSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage, SequenceNumberSet,
};
use crate::wire_types::{EntityId, Guid, GuidPrefix, Locator, SequenceNumber, VendorId};

/// The `nackResponseDelay` a writer starts with (§8.4.7.1).
pub const DEFAULT_NACK_RESPONSE_DELAY: Duration =
    crate::reliable_writer::DEFAULT_NACK_RESPONSE_DELAY;

/// State of a ReaderLocator's push machine (Table 8.64 T1 to T5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushState {
    /// Nothing unsent: HEARTBEATs every `heartbeatPeriod` (T5).
    Announcing,
    /// Unsent changes: the writer sends them (T2, T4).
    Pushing,
}

/// State of a ReaderLocator's repair machine (Table 8.64 T6 to T11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairState {
    /// No change requested.
    Waiting,
    /// Changes requested; the writer waits `nackResponseDelay` (T7, T8).
    MustRepair,
    /// The delay is over; the writer sends the requested changes (T9, T10).
    Repairing,
}

/// DDSI-RTPS 2.5 Table 8.57: the locator a StatelessWriter sends to, with
/// what it has sent and what was requested there.
#[derive(Debug, Clone)]
pub struct ReaderLocator {
    locator: Locator,
    expects_inline_qos: bool,
    /// `highestSentChangeSN`: the highest sequence number sent.
    highest_sent_seq_num: i64,
    requested: BTreeSet<SequenceNumber>,
    push: PushState,
    repair: RepairState,
    must_repair_since: Option<Duration>,
    announcing_since: Option<Duration>,
    /// Highest `readerSNState.bitmapBase` an ACKNACK named here: every
    /// sequence number below it is received.
    acked_below: i64,
}

impl ReaderLocator {
    fn new(locator: Locator, expects_inline_qos: bool) -> Self {
        Self {
            locator,
            expects_inline_qos,
            highest_sent_seq_num: 0,
            requested: BTreeSet::new(),
            push: PushState::Announcing,
            repair: RepairState::Waiting,
            must_repair_since: None,
            announcing_since: None,
            acked_below: 0,
        }
    }

    /// The locator.
    #[must_use]
    pub fn locator(&self) -> &Locator {
        &self.locator
    }

    /// Whether the reader behind it expects inline QoS.
    #[must_use]
    pub fn expects_inline_qos(&self) -> bool {
        self.expects_inline_qos
    }

    /// `highestSentChangeSN`.
    #[must_use]
    pub fn highest_sent_seq_num(&self) -> i64 {
        self.highest_sent_seq_num
    }

    /// `requested_changes()`: the sequence numbers requested and not yet
    /// answered, ascending.
    #[must_use]
    pub fn requested_changes(&self) -> Vec<SequenceNumber> {
        self.requested.iter().copied().collect()
    }

    /// `requested_changes_set(req_seq_num_set)`.
    pub fn requested_changes_set(&mut self, set: impl IntoIterator<Item = SequenceNumber>) {
        self.requested.extend(set);
    }

    /// `next_requested_change()`: the lowest requested sequence number; it
    /// leaves the requested set.
    pub fn next_requested_change(&mut self) -> Option<SequenceNumber> {
        let sn = self.requested.iter().next().copied()?;
        self.requested.remove(&sn);
        Some(sn)
    }

    /// `unsent_changes()`: the sequence numbers above `highestSentChangeSN`
    /// that are in `cache`.
    #[must_use]
    pub fn unsent_changes(&self, cache: &HistoryCache) -> bool {
        cache
            .max_sn()
            .is_some_and(|max| max.0 > self.highest_sent_seq_num)
    }

    /// The state of the push machine.
    #[must_use]
    pub fn push_state(&self) -> PushState {
        self.push
    }

    /// The state of the repair machine.
    #[must_use]
    pub fn repair_state(&self) -> RepairState {
        self.repair
    }
}

/// `ReliableStatelessWriter` — Spec §8.4.8.2.
pub struct ReliableStatelessWriter {
    guid: Guid,
    vendor_id: VendorId,
    cache: HistoryCache,
    next_sn: i64,
    reader_locators: Vec<ReaderLocator>,
    heartbeat_count: u32,
    heartbeat_period: Duration,
    nack_response_delay: Duration,
    /// `related_dds_writer.qos` as inline QoS (T4, T10), for a
    /// ReaderLocator that expects it.
    related_qos: Option<ParameterList>,
    /// Maximum datagrams per locator and tick (resource bound: the
    /// `RL::can_send()` guard of T4 and T10).
    max_per_tick: usize,
    /// ACKNACKs whose reply locators matched no ReaderLocator.
    unmatched_acknacks: u64,
}

/// Statistics snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReliableStatelessStats {
    /// Number of cache entries.
    pub cached_changes: usize,
    /// Changes requested and not yet answered, over all ReaderLocators.
    pub pending_retransmits: usize,
    /// Number of ReaderLocators.
    pub reader_locators: usize,
    /// Heartbeat counter (modulo u32).
    pub heartbeat_count: u32,
    /// ACKNACKs that named no known ReaderLocator.
    pub unmatched_acknacks: u64,
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
            reader_locators: Vec::new(),
            heartbeat_count: 0,
            heartbeat_period,
            nack_response_delay: DEFAULT_NACK_RESPONSE_DELAY,
            related_qos: None,
            max_per_tick: 16,
            unmatched_acknacks: 0,
        }
    }

    /// Own GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Sets `nackResponseDelay` (§8.4.7.1).
    pub fn set_nack_response_delay(&mut self, delay: Duration) {
        self.nack_response_delay = delay;
    }

    /// Sets the inline QoS of the related DDS writer, which the DATA to a
    /// ReaderLocator that expects inline QoS carries.
    pub fn set_related_qos(&mut self, qos: Option<ParameterList>) {
        self.related_qos = qos;
    }

    /// Sets the bound on datagrams per locator and tick.
    pub fn set_max_per_tick(&mut self, n: usize) {
        self.max_per_tick = n;
    }

    /// T1 — `reader_locator_add`. `false` if the locator is a
    /// ReaderLocator already.
    pub fn reader_locator_add(&mut self, locator: Locator, expects_inline_qos: bool) -> bool {
        if self.reader_locator_lookup(&locator).is_some() {
            return false;
        }
        self.reader_locators
            .push(ReaderLocator::new(locator, expects_inline_qos));
        true
    }

    /// T12 — `reader_locator_remove`. `false` if there is no such
    /// ReaderLocator.
    pub fn reader_locator_remove(&mut self, locator: &Locator) -> bool {
        let before = self.reader_locators.len();
        self.reader_locators.retain(|rl| rl.locator != *locator);
        self.reader_locators.len() != before
    }

    /// `unsent_changes_reset` (Table 8.56): sets `highestSentChangeSN` of
    /// every ReaderLocator back to 0, so each change in the cache counts as
    /// unsent again and the next [`Self::tick`] pushes them all.
    pub fn unsent_changes_reset(&mut self) {
        for rl in &mut self.reader_locators {
            rl.highest_sent_seq_num = 0;
        }
    }

    /// `reader_locator_lookup`.
    #[must_use]
    pub fn reader_locator_lookup(&self, locator: &Locator) -> Option<&ReaderLocator> {
        self.reader_locators
            .iter()
            .find(|rl| rl.locator == *locator)
    }

    /// `new_change`: stores a sample in the cache. Returns the assigned
    /// sequence number.
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
        let mut change = CacheChange::alive(sn, payload);
        change.kind = kind;
        self.cache
            .insert(change)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "reliable stateless writer cache full",
            })?;
        Ok(sn)
    }

    /// T6, T8 — an ACKNACK destined to this writer. The reply goes to the
    /// locators of the receiver state: for each of its unicast and multicast
    /// reply locators that names a port and is a ReaderLocator of this
    /// writer, `requested_changes_set(readerSNState.set)`. `readerSNState`'s
    /// `bitmapBase` also tells that every sequence number below it is
    /// received there. An ACKNACK none of whose reply locators is a
    /// ReaderLocator is counted (`unmatched_acknacks`): the reader did not
    /// send the InfoReply the protocol needs.
    pub fn handle_acknack(&mut self, ack: &AckNackSubmessage, receiver: &ReceiverState) {
        let base = ack.reader_sn_state.bitmap_base.0;
        let mut matched = false;
        for reply in receiver
            .unicast_reply_locator_list
            .iter()
            .chain(receiver.multicast_reply_locator_list.iter())
        {
            if reply.port == Locator::PORT_INVALID {
                continue;
            }
            if let Some(rl) = self
                .reader_locators
                .iter_mut()
                .find(|rl| rl.locator == *reply)
            {
                matched = true;
                rl.acked_below = rl.acked_below.max(base);
                rl.requested.retain(|sn| sn.0 >= base);
                rl.requested_changes_set(ack.reader_sn_state.iter_set());
            }
        }
        if !matched {
            self.unmatched_acknacks = self.unmatched_acknacks.saturating_add(1);
        }
    }

    /// Whether every ReaderLocator has acknowledged every sequence number up
    /// to and including `sn`. False without a ReaderLocator.
    #[must_use]
    pub fn is_acked_to(&self, sn: SequenceNumber) -> bool {
        !self.reader_locators.is_empty()
            && self.reader_locators.iter().all(|rl| sn.0 < rl.acked_below)
    }

    /// Removes the cache entries every ReaderLocator has acknowledged.
    /// Returns the number removed.
    pub fn purge_acked(&mut self) -> usize {
        let Some(low) = self.reader_locators.iter().map(|rl| rl.acked_below).min() else {
            return 0;
        };
        if low <= 1 {
            return 0;
        }
        self.cache.remove_up_to(SequenceNumber(low - 1))
    }

    /// Drives both machines of every ReaderLocator (T2 to T5, T7 to T11).
    /// Returns the datagrams to send, each to its ReaderLocator's locator.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        let header = RtpsHeader::new(self.vendor_id, self.guid.prefix);
        for idx in 0..self.reader_locators.len() {
            let targets = Arc::new(alloc::vec![self.reader_locators[idx].locator]);
            self.push_machine(idx, now, header, &targets, &mut out)?;
            self.repair_machine(idx, now, header, &targets, &mut out)?;
        }
        Ok(out)
    }

    fn push_machine(
        &mut self,
        idx: usize,
        now: Duration,
        header: RtpsHeader,
        targets: &Arc<Vec<Locator>>,
        out: &mut Vec<OutboundDatagram>,
    ) -> Result<(), WireError> {
        let mut sent = 0usize;
        // T2: unsent changes; T4 while the writer can send; T3: none left.
        if self.reader_locators[idx].unsent_changes(&self.cache) {
            self.reader_locators[idx].push = PushState::Pushing;
        }
        while self.reader_locators[idx].push == PushState::Pushing {
            if sent >= self.max_per_tick {
                return Ok(());
            }
            let highest = self.reader_locators[idx].highest_sent_seq_num;
            let Some(next) = self.next_cached_above(highest) else {
                let rl = &mut self.reader_locators[idx];
                rl.push = PushState::Announcing;
                rl.announcing_since = Some(now);
                break;
            };
            if next.0 > highest + 1 {
                // The changes in between are no longer in the cache.
                out.push(self.gap_datagram(
                    header,
                    SequenceNumber(highest + 1),
                    SequenceNumber(next.0 - 1),
                    targets,
                )?);
            }
            let expects = self.reader_locators[idx].expects_inline_qos;
            out.push(self.data_datagram(header, next, expects, false, targets)?);
            self.reader_locators[idx].highest_sent_seq_num = next.0;
            sent += 1;
        }
        // T5: announcing; after(heartbeatPeriod).
        let rl = &mut self.reader_locators[idx];
        if rl.push == PushState::Announcing {
            match rl.announcing_since {
                None => rl.announcing_since = Some(now),
                Some(since) if now >= since + self.heartbeat_period => {
                    rl.announcing_since = Some(now);
                    out.push(self.heartbeat_datagram(header, targets)?);
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    fn repair_machine(
        &mut self,
        idx: usize,
        now: Duration,
        header: RtpsHeader,
        targets: &Arc<Vec<Locator>>,
        out: &mut Vec<OutboundDatagram>,
    ) -> Result<(), WireError> {
        // T7: requested changes.
        {
            let rl = &mut self.reader_locators[idx];
            if rl.repair == RepairState::Waiting && !rl.requested.is_empty() {
                rl.repair = RepairState::MustRepair;
                rl.must_repair_since = Some(now);
            }
            // T9: after(nackResponseDelay).
            if rl.repair == RepairState::MustRepair
                && rl
                    .must_repair_since
                    .is_some_and(|since| now >= since + self.nack_response_delay)
            {
                rl.repair = RepairState::Repairing;
            }
        }
        // T10 while the writer can send; T11 when nothing is requested.
        let mut sent = 0usize;
        while self.reader_locators[idx].repair == RepairState::Repairing {
            if sent >= self.max_per_tick {
                break;
            }
            let Some(sn) = self.reader_locators[idx].next_requested_change() else {
                let rl = &mut self.reader_locators[idx];
                rl.repair = RepairState::Waiting;
                rl.must_repair_since = None;
                break;
            };
            if self.cache.get(sn).is_some() {
                let expects = self.reader_locators[idx].expects_inline_qos;
                out.push(self.data_datagram(header, sn, expects, true, targets)?);
            } else {
                // No longer in the cache: a GAP that counts no filtered
                // change.
                out.push(self.gap_datagram(header, sn, sn, targets)?);
            }
            sent += 1;
        }
        Ok(())
    }

    /// The lowest sequence number in the cache above `after`.
    fn next_cached_above(&self, after: i64) -> Option<SequenceNumber> {
        let max = self.cache.max_sn()?.0;
        let mut sn = after.checked_add(1)?;
        while sn <= max {
            if self.cache.get(SequenceNumber(sn)).is_some() {
                return Some(SequenceNumber(sn));
            }
            sn = sn.checked_add(1)?;
        }
        None
    }

    fn data_datagram(
        &self,
        header: RtpsHeader,
        sn: SequenceNumber,
        expects_inline_qos: bool,
        repair: bool,
        targets: &Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let change = self.cache.get(sn).ok_or(WireError::ValueOutOfRange {
            message: "reliable stateless writer: change left the cache",
        })?;
        let mut inline = ParameterList::new();
        if expects_inline_qos {
            if let Some(qos) = &self.related_qos {
                for p in &qos.parameters {
                    inline.push(p.clone());
                }
            }
            // T10: "DATA.inlineQos += a_change.inlineQos".
            if repair {
                if let Some(own) = &change.inline_qos {
                    for p in &own.parameters {
                        inline.push(p.clone());
                    }
                }
            }
        }
        // A dispose or unregister names its instance and says what happened
        // whatever the reader expects (§9.6.4.8, §9.6.4.9).
        let status = match change.kind {
            ChangeKind::Alive => None,
            ChangeKind::AliveFiltered => Some(crate::inline_qos::status_info::FILTERED),
            ChangeKind::NotAliveDisposed => Some(crate::inline_qos::status_info::DISPOSED),
            ChangeKind::NotAliveUnregistered => Some(crate::inline_qos::status_info::UNREGISTERED),
            ChangeKind::NotAliveDisposedUnregistered => Some(
                crate::inline_qos::status_info::DISPOSED
                    | crate::inline_qos::status_info::UNREGISTERED,
            ),
        };
        let lifecycle = matches!(
            change.kind,
            ChangeKind::NotAliveDisposed
                | ChangeKind::NotAliveUnregistered
                | ChangeKind::NotAliveDisposedUnregistered
        );
        if let Some(bits) = status {
            inline.push(crate::inline_qos::status_info_param(bits));
            if let Some(hash) = change.key_hash {
                inline.push(crate::parameter_list::Parameter::new(
                    crate::parameter_list::pid::KEY_HASH,
                    hash.to_vec(),
                ));
            }
        }
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::UNKNOWN,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            inline_qos: if inline.parameters.is_empty() {
                None
            } else {
                Some(inline)
            },
            data_flag: !lifecycle,
            key_flag: lifecycle,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::whole(Arc::clone(&change.payload)),
        };
        Ok(OutboundDatagram {
            bytes: crate::datagram::encode_data_datagram(header, &[data])?,
            targets: Arc::clone(targets),
        })
    }

    fn gap_datagram(
        &self,
        header: RtpsHeader,
        first: SequenceNumber,
        last: SequenceNumber,
        targets: &Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let gap = GapSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: self.guid.entity_id,
            gap_start: first,
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(last.0 + 1),
                num_bits: 0,
                bitmap: Vec::new(),
            },
            group_info: None,
            filtered_count: Some(0),
        };
        let (body, flags) = gap.write_body(true);
        Ok(OutboundDatagram {
            bytes: single_submessage(header, SubmessageId::Gap, flags, &body)?,
            targets: Arc::clone(targets),
        })
    }

    fn heartbeat_datagram(
        &mut self,
        header: RtpsHeader,
        targets: &Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        self.heartbeat_count = self.heartbeat_count.wrapping_add(1);
        // An empty cache announces firstSN = lastSN + 1 (§8.3.8.6).
        let last = self
            .cache
            .max_sn()
            .unwrap_or(SequenceNumber(self.next_sn - 1));
        let first = self.cache.min_sn().unwrap_or(SequenceNumber(last.0 + 1));
        let hb = HeartbeatSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: self.guid.entity_id,
            first_sn: first,
            last_sn: last,
            count: self.heartbeat_count as i32,
            // T5: "HEARTBEAT.FinalFlag := SET".
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        Ok(OutboundDatagram {
            bytes: single_submessage(header, SubmessageId::Heartbeat, flags, &body)?,
            targets: Arc::clone(targets),
        })
    }

    /// Clears the cache and every ReaderLocator's requests.
    pub fn shutdown(&mut self) {
        if let Some(max) = self.cache.max_sn() {
            let _ = self.cache.remove_up_to(max);
        }
        for rl in &mut self.reader_locators {
            rl.requested.clear();
        }
    }

    /// Diagnosis snapshot.
    #[must_use]
    pub fn stats(&self) -> ReliableStatelessStats {
        ReliableStatelessStats {
            cached_changes: self.cache.len(),
            pending_retransmits: self
                .reader_locators
                .iter()
                .map(|rl| rl.requested.len())
                .sum(),
            reader_locators: self.reader_locators.len(),
            heartbeat_count: self.heartbeat_count,
            unmatched_acknacks: self.unmatched_acknacks,
        }
    }
}

fn single_submessage(
    header: RtpsHeader,
    id: SubmessageId,
    flags: u8,
    body: &[u8],
) -> Result<Vec<u8>, WireError> {
    let len = u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
        message: "submessage body exceeds u16::MAX",
    })?;
    let sh = SubmessageHeader {
        submessage_id: id,
        flags: flags | FLAG_E_LITTLE_ENDIAN,
        octets_to_next_header: len,
    };
    let mut bytes = header.to_bytes().to_vec();
    bytes.extend_from_slice(&sh.to_bytes());
    bytes.extend_from_slice(body);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::datagram::{ParsedSubmessage, decode_datagram};

    fn loc(n: u8) -> Locator {
        Locator::udp_v4([10, 0, 0, n], 7400)
    }

    fn make_writer() -> ReliableStatelessWriter {
        let mut w = ReliableStatelessWriter::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([1, 2, 3]),
            VendorId::ZERODDS,
            HistoryKind::KeepAll,
            32,
            Duration::from_millis(100),
        );
        // The tests drive the clock by milliseconds; the default is the
        // spec's 200 ms.
        w.set_nack_response_delay(Duration::ZERO);
        w
    }

    /// A receiver state as an InfoReply gave it: replies go to `at`.
    fn receiver_replying_at(at: Locator) -> ReceiverState {
        let mut st = ReceiverState::new(GuidPrefix::from_bytes([1; 12]));
        st.apply_info_reply(alloc::vec![at], None);
        st
    }

    fn acknack(w: &ReliableStatelessWriter, base: i64, missing: &[i64]) -> AckNackSubmessage {
        let missing: Vec<SequenceNumber> = missing.iter().map(|n| SequenceNumber(*n)).collect();
        AckNackSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: w.guid.entity_id,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(base), &missing),
            count: 1,
            final_flag: false,
        }
    }

    fn contents(dg: &OutboundDatagram) -> Vec<ParsedSubmessage> {
        decode_datagram(&dg.bytes)
            .unwrap()
            .contents()
            .cloned()
            .collect()
    }

    fn gap_of(dg: &OutboundDatagram) -> GapSubmessage {
        contents(dg)
            .into_iter()
            .find_map(|s| match s {
                ParsedSubmessage::Gap(g) => Some(g),
                _ => None,
            })
            .expect("a GAP")
    }

    fn heartbeat_of(dg: &OutboundDatagram) -> HeartbeatSubmessage {
        contents(dg)
            .into_iter()
            .find_map(|s| match s {
                ParsedSubmessage::Heartbeat(h) => Some(h),
                _ => None,
            })
            .expect("a HEARTBEAT")
    }

    fn data_of(dg: &OutboundDatagram) -> DataSubmessage {
        contents(dg)
            .into_iter()
            .find_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .expect("a DATA")
    }

    fn kinds(dgs: &[OutboundDatagram]) -> Vec<&'static str> {
        dgs.iter()
            .flat_map(contents)
            .map(|s| match s {
                ParsedSubmessage::Data(_) => "DATA",
                ParsedSubmessage::Gap(_) => "GAP",
                ParsedSubmessage::Heartbeat(_) => "HEARTBEAT",
                _ => "other",
            })
            .collect()
    }

    #[test]
    fn new_change_assigns_monotonic_sn() {
        let mut w = make_writer();
        let sns: Vec<i64> = (0..3)
            .map(|i| w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap().0)
            .collect();
        assert_eq!(sns, alloc::vec![1, 2, 3]);
    }

    /// T1 and T12: a ReaderLocator is added once and removed again.
    #[test]
    fn reader_locators_are_added_looked_up_and_removed() {
        let mut w = make_writer();
        assert!(w.reader_locator_add(loc(1), true));
        assert!(
            !w.reader_locator_add(loc(1), false),
            "a locator is one ReaderLocator"
        );
        assert!(
            w.reader_locator_lookup(&loc(1))
                .unwrap()
                .expects_inline_qos()
        );
        assert!(w.reader_locator_lookup(&loc(2)).is_none());
        assert!(w.reader_locator_remove(&loc(1)));
        assert!(!w.reader_locator_remove(&loc(1)));
        assert_eq!(w.stats().reader_locators, 0);
    }

    /// T2, T4, T3: unsent changes are pushed; after them the locator is
    /// announced to.
    #[test]
    fn unsent_changes_are_pushed_in_order_then_announced() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        let dgs = w.tick(Duration::ZERO).unwrap();
        assert_eq!(kinds(&dgs), alloc::vec!["DATA", "DATA", "DATA"]);
        let sns: Vec<i64> = dgs
            .iter()
            .flat_map(contents)
            .filter_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                _ => None,
            })
            .collect();
        assert_eq!(sns, alloc::vec![1, 2, 3]);
        let rl = w.reader_locator_lookup(&loc(1)).unwrap();
        assert_eq!(rl.highest_sent_seq_num(), 3);
        assert_eq!(rl.push_state(), PushState::Announcing);
        // Nothing is sent twice.
        assert!(kinds(&w.tick(Duration::from_millis(10)).unwrap()).is_empty());
        assert!(dgs.iter().all(|d| d.targets.as_slice() == [loc(1)]));
    }

    /// Table 8.56 `unsent_changes_reset`: `highestSentChangeSN` of every
    /// ReaderLocator goes back to 0, and the next tick sends all changes of
    /// the cache again, to every locator.
    #[test]
    fn unsent_changes_reset_makes_every_change_unsent_again() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.reader_locator_add(loc(2), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        let first = w.tick(Duration::ZERO).unwrap();
        assert_eq!(kinds(&first).len(), 6, "three DATA to each locator");
        assert!(kinds(&w.tick(Duration::from_millis(1)).unwrap()).is_empty());
        w.unsent_changes_reset();
        for l in [loc(1), loc(2)] {
            assert_eq!(
                w.reader_locator_lookup(&l).unwrap().highest_sent_seq_num(),
                0
            );
        }
        let again = w.tick(Duration::from_millis(2)).unwrap();
        let sns: Vec<i64> = again
            .iter()
            .flat_map(contents)
            .filter_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                _ => None,
            })
            .collect();
        assert_eq!(sns, alloc::vec![1, 2, 3, 1, 2, 3]);
    }

    /// T4: a sequence number that is no longer in the cache is announced by
    /// a GAP with `filteredCount` 0 before the next DATA.
    #[test]
    fn a_gap_precedes_the_data_after_a_hole_in_the_cache() {
        let mut w = ReliableStatelessWriter::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([1, 2, 3]),
            VendorId::ZERODDS,
            HistoryKind::KeepLast { depth: 1 },
            1,
            Duration::from_millis(100),
        );
        w.reader_locator_add(loc(1), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        // KEEP_LAST(1): only SN 3 is in the cache; SNs 1 and 2 are a hole.
        let dgs = w.tick(Duration::ZERO).unwrap();
        assert_eq!(kinds(&dgs), alloc::vec!["GAP", "DATA"]);
        let g = gap_of(&dgs[0]);
        assert_eq!(g.gap_start.0, 1);
        assert_eq!(
            g.gap_list.bitmap_base.0, 3,
            "SNs 1 and 2: below the list's base"
        );
        assert_eq!(g.filtered_count, Some(0));
        assert_eq!(g.reader_id, EntityId::UNKNOWN);
    }

    /// T5: HEARTBEAT every heartbeatPeriod to an announcing locator,
    /// FinalFlag set, readerId UNKNOWN.
    #[test]
    fn an_announcing_locator_gets_a_final_heartbeat_each_period() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        w.tick(Duration::ZERO).unwrap(); // pushes SN 1, starts announcing
        assert!(
            w.tick(Duration::from_millis(50)).unwrap().is_empty(),
            "before the period"
        );
        let dgs = w.tick(Duration::from_millis(100)).unwrap();
        assert_eq!(kinds(&dgs), alloc::vec!["HEARTBEAT"]);
        let hb = heartbeat_of(&dgs[0]);
        assert!(hb.final_flag, "T5: FinalFlag := SET");
        assert_eq!((hb.first_sn.0, hb.last_sn.0), (1, 1));
        assert_eq!(hb.reader_id, EntityId::UNKNOWN);
        assert_eq!(w.stats().heartbeat_count, 1);
        // The next one a period later.
        assert!(w.tick(Duration::from_millis(150)).unwrap().is_empty());
        assert_eq!(
            kinds(&w.tick(Duration::from_millis(200)).unwrap()),
            alloc::vec!["HEARTBEAT"]
        );
    }

    /// An empty cache announces firstSN = lastSN + 1.
    #[test]
    fn a_heartbeat_of_an_empty_cache_has_first_after_last() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.tick(Duration::ZERO).unwrap();
        let dgs = w.tick(Duration::from_millis(100)).unwrap();
        let hb = heartbeat_of(&dgs[0]);
        assert_eq!((hb.first_sn.0, hb.last_sn.0), (1, 0));
    }

    /// T6, T7, T9, T10, T11: an ACKNACK is answered at the reply locator of
    /// the receiver state, after nackResponseDelay.
    #[test]
    fn an_acknack_is_answered_at_its_reply_locator_after_the_delay() {
        let mut w = make_writer();
        w.set_nack_response_delay(Duration::from_millis(30));
        w.reader_locator_add(loc(1), false);
        w.reader_locator_add(loc(2), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.tick(Duration::ZERO).unwrap(); // pushes to both
        w.handle_acknack(&acknack(&w, 1, &[2, 3]), &receiver_replying_at(loc(2)));
        // Only the locator the reply names has requests.
        assert_eq!(
            w.reader_locator_lookup(&loc(1))
                .unwrap()
                .requested_changes(),
            alloc::vec![]
        );
        assert_eq!(
            w.reader_locator_lookup(&loc(2))
                .unwrap()
                .requested_changes(),
            alloc::vec![SequenceNumber(2), SequenceNumber(3)]
        );
        // T7: must_repair; T9 not yet.
        assert!(w.tick(Duration::from_millis(10)).unwrap().is_empty());
        assert_eq!(
            w.reader_locator_lookup(&loc(2)).unwrap().repair_state(),
            RepairState::MustRepair
        );
        // T9, T10, T11 after the delay.
        let dgs = w.tick(Duration::from_millis(40)).unwrap();
        assert_eq!(kinds(&dgs), alloc::vec!["DATA", "DATA"]);
        assert!(dgs.iter().all(|d| d.targets.as_slice() == [loc(2)]));
        let rl = w.reader_locator_lookup(&loc(2)).unwrap();
        assert_eq!(rl.repair_state(), RepairState::Waiting);
        assert!(rl.requested_changes().is_empty());
    }

    /// T10: a requested change that left the cache is answered by a GAP.
    #[test]
    fn a_requested_change_that_left_the_cache_is_answered_by_a_gap() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.tick(Duration::ZERO).unwrap();
        w.handle_acknack(&acknack(&w, 1, &[2]), &receiver_replying_at(loc(1)));
        // SNs up to 2 leave the cache; the reader asks for 2 again.
        let _ = w.cache.remove_up_to(SequenceNumber(2));
        w.handle_acknack(&acknack(&w, 1, &[2]), &receiver_replying_at(loc(1)));
        let dgs = w.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(kinds(&dgs), alloc::vec!["GAP"]);
        let g = gap_of(&dgs[0]);
        assert_eq!((g.gap_start.0, g.gap_list.bitmap_base.0), (2, 3));
        assert_eq!(g.filtered_count, Some(0));
    }

    /// The reply locators of the receiver state are the only source: an
    /// ACKNACK whose reply names no ReaderLocator, or names no port (Table
    /// 8.17 initial value), requests nothing and is counted.
    #[test]
    fn an_acknack_without_a_usable_reply_locator_is_counted() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        // No InfoReply: the initial lists carry no port.
        let plain = ReceiverState::new(GuidPrefix::from_bytes([1; 12]));
        w.handle_acknack(&acknack(&w, 1, &[1]), &plain);
        // An InfoReply to a locator the writer does not send to.
        w.handle_acknack(&acknack(&w, 1, &[1]), &receiver_replying_at(loc(9)));
        assert_eq!(w.stats().unmatched_acknacks, 2);
        assert_eq!(w.stats().pending_retransmits, 0);
    }

    #[test]
    fn the_acknack_base_acknowledges_and_the_cache_is_purged() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.reader_locator_add(loc(2), false);
        for i in 1..=5 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        assert!(!w.is_acked_to(SequenceNumber(1)));
        w.handle_acknack(&acknack(&w, 4, &[]), &receiver_replying_at(loc(1)));
        assert!(
            !w.is_acked_to(SequenceNumber(3)),
            "locator 2 has acknowledged nothing"
        );
        w.handle_acknack(&acknack(&w, 3, &[]), &receiver_replying_at(loc(2)));
        assert!(w.is_acked_to(SequenceNumber(2)));
        assert!(!w.is_acked_to(SequenceNumber(3)));
        assert_eq!(w.purge_acked(), 2);
        // A lower base later does not take back what was acknowledged.
        w.handle_acknack(&acknack(&w, 1, &[]), &receiver_replying_at(loc(1)));
        assert!(w.is_acked_to(SequenceNumber(2)));
    }

    #[test]
    fn the_resend_per_tick_is_bounded() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.set_max_per_tick(2);
        for i in 1..=5 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        assert_eq!(w.tick(Duration::ZERO).unwrap().len(), 2);
        assert_eq!(w.tick(Duration::ZERO).unwrap().len(), 2);
        assert_eq!(w.tick(Duration::ZERO).unwrap().len(), 1);
        assert!(w.tick(Duration::ZERO).unwrap().is_empty());
    }

    /// T4: inline QoS only for a locator that expects it.
    #[test]
    fn the_related_qos_goes_out_with_the_data_only_where_it_is_expected() {
        let mut w = make_writer();
        let mut qos = ParameterList::new();
        qos.push(crate::parameter_list::Parameter::new(
            crate::parameter_list::pid::DURABILITY,
            alloc::vec![0, 0, 0, 0],
        ));
        w.set_related_qos(Some(qos));
        w.reader_locator_add(loc(1), true);
        w.reader_locator_add(loc(2), false);
        w.new_change(ChangeKind::Alive, alloc::vec![1]).unwrap();
        let dgs = w.tick(Duration::ZERO).unwrap();
        let qos_of = |dg: &OutboundDatagram| data_of(dg).inline_qos.map(|p| p.parameters.len());
        let to = |l: Locator| dgs.iter().find(|d| d.targets.as_slice() == [l]).unwrap();
        assert_eq!(qos_of(to(loc(1))), Some(1));
        assert_eq!(qos_of(to(loc(2))), None);
    }

    /// A dispose is DATA with the key flag and the status info.
    #[test]
    fn a_dispose_goes_out_as_a_key_marker_with_status_info() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        w.new_change(ChangeKind::NotAliveDisposed, alloc::vec![9, 9, 9, 9])
            .unwrap();
        let dgs = w.tick(Duration::ZERO).unwrap();
        let d = data_of(&dgs[0]);
        assert!(d.key_flag && !d.data_flag);
        assert_eq!(
            crate::inline_qos::find_status_info(d.inline_qos.as_ref().unwrap()),
            Some(crate::inline_qos::status_info::DISPOSED)
        );
    }

    #[test]
    fn the_heartbeat_count_wraps_at_u32_max() {
        let mut w = make_writer();
        w.heartbeat_count = u32::MAX - 1;
        w.reader_locator_add(loc(1), false);
        w.tick(Duration::ZERO).unwrap();
        w.tick(Duration::from_millis(100)).unwrap();
        assert_eq!(w.stats().heartbeat_count, u32::MAX);
        w.tick(Duration::from_millis(200)).unwrap();
        assert_eq!(w.stats().heartbeat_count, 0);
    }

    #[test]
    fn shutdown_clears_the_cache_and_the_requests() {
        let mut w = make_writer();
        w.reader_locator_add(loc(1), false);
        for i in 1..=3 {
            w.new_change(ChangeKind::Alive, alloc::vec![i]).unwrap();
        }
        w.handle_acknack(&acknack(&w, 1, &[2, 3]), &receiver_replying_at(loc(1)));
        w.shutdown();
        assert_eq!(w.stats().cached_changes, 0);
        assert_eq!(w.stats().pending_retransmits, 0);
    }
}
