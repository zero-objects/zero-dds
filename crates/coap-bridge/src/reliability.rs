// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP reliability — RFC 7252 §4.2-§4.7.
//!
//! Spec §4.2: CON messages expect an ACK; on timeout they are
//! retransmitted with exponential backoff up to `MAX_RETRANSMIT`.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Default retransmit constants — RFC 7252 §4.8.
pub const ACK_TIMEOUT_MS: u64 = 2000;
/// `ACK_RANDOM_FACTOR` (Spec §4.8) — we use 1.5 expressed as a
/// multiplier of 3/2.
pub const ACK_RANDOM_FACTOR_NUM: u64 = 3;
/// Denominator for `ACK_RANDOM_FACTOR_NUM`.
pub const ACK_RANDOM_FACTOR_DEN: u64 = 2;
/// `MAX_RETRANSMIT` (Spec §4.8).
pub const MAX_RETRANSMIT: u32 = 4;
/// `NSTART` (Spec §4.8) — a client MUST NOT have more than `NSTART`
/// simultaneous outstanding interactions towards one server.
pub const NSTART: u32 = 1;
/// `DEFAULT_LEISURE` (Spec §4.8) in milliseconds.
pub const DEFAULT_LEISURE_MS: u64 = 5_000;
/// `PROBING_RATE` (Spec §4.8) in bytes per second.
pub const PROBING_RATE_BYTES_PER_SEC: u64 = 1;
/// `MAX_LATENCY` (Spec §4.8.2) in milliseconds.
pub const MAX_LATENCY_MS: u64 = 100_000;
/// `PROCESSING_DELAY` (Spec §4.8.2) in milliseconds — the spec sets
/// it equal to `ACK_TIMEOUT`.
pub const PROCESSING_DELAY_MS: u64 = ACK_TIMEOUT_MS;
/// `MAX_TRANSMIT_SPAN` (Spec §4.8.2) in milliseconds —
/// `ACK_TIMEOUT * ((2 ** MAX_RETRANSMIT) - 1) * ACK_RANDOM_FACTOR`.
pub const MAX_TRANSMIT_SPAN_MS: u64 = 45_000;
/// `MAX_TRANSMIT_WAIT` (Spec §4.8.2) in milliseconds —
/// `ACK_TIMEOUT * ((2 ** (MAX_RETRANSMIT + 1)) - 1) * ACK_RANDOM_FACTOR`.
pub const MAX_TRANSMIT_WAIT_MS: u64 = 93_000;
/// `EXCHANGE_LIFETIME` (Spec §4.8.2) in milliseconds —
/// `MAX_TRANSMIT_SPAN + (2 * MAX_LATENCY) + PROCESSING_DELAY`.
pub const EXCHANGE_LIFETIME_MS: u64 = 247_000;
/// `NON_LIFETIME` (Spec §4.8.2) in milliseconds —
/// `MAX_TRANSMIT_SPAN + MAX_LATENCY`.
pub const NON_LIFETIME_MS: u64 = 145_000;

/// Pending CON entry in the reliability queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmable {
    /// Message ID (16-bit).
    pub message_id: u16,
    /// Token bytes (0..=8).
    pub token: Vec<u8>,
    /// Encoded message bytes — re-emitted on retransmit.
    pub bytes: Vec<u8>,
    /// Remaining retransmits (starts at `MAX_RETRANSMIT`).
    pub retransmits_left: u32,
    /// Next timeout (in milliseconds since start).
    pub next_timeout_ms: u64,
    /// Current timeout interval (doubles per retransmit).
    pub current_interval_ms: u64,
}

/// Reliability tracker — the caller feeds in time and receive events,
/// the tracker returns retransmit/drop decisions.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReliabilityTracker {
    pending: BTreeMap<u16, PendingConfirmable>,
}

/// Output of a `tick` step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TickOutput {
    /// Messages that must be retransmitted (byte slice already
    /// included).
    pub retransmit: Vec<PendingConfirmable>,
    /// Message IDs whose `MAX_RETRANSMIT` is exhausted and whose
    /// connection is considered lost.
    pub timed_out: Vec<u16>,
}

impl ReliabilityTracker {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends a CON — registers it for retransmission.
    pub fn send_confirmable(
        &mut self,
        message_id: u16,
        token: Vec<u8>,
        bytes: Vec<u8>,
        now_ms: u64,
    ) {
        self.pending.insert(
            message_id,
            PendingConfirmable {
                message_id,
                token,
                bytes,
                retransmits_left: MAX_RETRANSMIT,
                next_timeout_ms: now_ms + initial_interval(),
                current_interval_ms: initial_interval(),
            },
        );
    }

    /// ACK received — remove the pending entry.
    pub fn receive_ack(&mut self, message_id: u16) -> bool {
        self.pending.remove(&message_id).is_some()
    }

    /// RST received — remove the pending entry (Spec §4.2: RST
    /// terminates without retransmit).
    pub fn receive_rst(&mut self, message_id: u16) -> bool {
        self.pending.remove(&message_id).is_some()
    }

    /// Tick — check whether a pending entry is due.
    pub fn tick(&mut self, now_ms: u64) -> TickOutput {
        let mut out = TickOutput::default();
        let mut to_drop = Vec::new();
        for (mid, entry) in self.pending.iter_mut() {
            if now_ms < entry.next_timeout_ms {
                continue;
            }
            if entry.retransmits_left == 0 {
                to_drop.push(*mid);
                continue;
            }
            entry.retransmits_left -= 1;
            entry.current_interval_ms *= 2;
            entry.next_timeout_ms = now_ms + entry.current_interval_ms;
            out.retransmit.push(entry.clone());
        }
        for mid in to_drop {
            self.pending.remove(&mid);
            out.timed_out.push(mid);
        }
        out
    }

    /// Number of pending CONs.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// Message kind as far as deduplication is concerned — it selects the
/// lifetime a Message ID is remembered for (Spec §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupKind {
    /// Confirmable — remembered for `EXCHANGE_LIFETIME`.
    Confirmable,
    /// Non-confirmable — remembered for `NON_LIFETIME`.
    NonConfirmable,
}

impl DedupKind {
    /// Spec §4.5 — how long the Message ID stays in the dedup table.
    #[must_use]
    pub const fn lifetime_ms(self) -> u64 {
        match self {
            Self::Confirmable => EXCHANGE_LIFETIME_MS,
            Self::NonConfirmable => NON_LIFETIME_MS,
        }
    }
}

/// Result of feeding a received message into the duplicate detector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reception {
    /// First sighting — the caller processes the message.
    Fresh,
    /// Repeat of a Message ID still inside its lifetime. Spec §4.5:
    /// the request or response "SHOULD" be processed only once, while
    /// the previous ACK "SHOULD" be echoed if one was cached.
    Duplicate {
        /// The ACK bytes remembered for the original copy, if any.
        cached_ack: Option<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeenMessage {
    expires_at_ms: u64,
    cached_ack: Option<Vec<u8>>,
}

/// Message-ID deduplication — RFC 7252 §4.5.
///
/// The recipient keeps every Message ID it has seen from an endpoint
/// for the lifetime of the exchange and answers a repeat from the
/// cache instead of processing it a second time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateDetector {
    seen: BTreeMap<(Vec<u8>, u16), SeenMessage>,
    cap: usize,
}

impl Default for DuplicateDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DuplicateDetector {
    /// Detector with a default entry cap of 1024 (DoS bound; the spec
    /// leaves the table size to the implementation).
    #[must_use]
    pub fn new() -> Self {
        Self::with_cap(1024)
    }

    /// Detector with an explicit entry cap.
    #[must_use]
    pub fn with_cap(cap: usize) -> Self {
        Self {
            seen: BTreeMap::new(),
            cap,
        }
    }

    /// Feeds a received message in. Returns whether it is a first
    /// sighting or a duplicate still inside its lifetime.
    pub fn receive(
        &mut self,
        endpoint: &[u8],
        message_id: u16,
        kind: DedupKind,
        now_ms: u64,
    ) -> Reception {
        self.evict_expired(now_ms);
        let key = (endpoint.to_vec(), message_id);
        if let Some(entry) = self.seen.get(&key) {
            return Reception::Duplicate {
                cached_ack: entry.cached_ack.clone(),
            };
        }
        if self.seen.len() >= self.cap {
            // Drop the entry that expires first — it is the one whose
            // remaining protection is worth least.
            if let Some(oldest) = self
                .seen
                .iter()
                .min_by_key(|(_, v)| v.expires_at_ms)
                .map(|(k, _)| k.clone())
            {
                self.seen.remove(&oldest);
            }
        }
        self.seen.insert(
            key,
            SeenMessage {
                expires_at_ms: now_ms + kind.lifetime_ms(),
                cached_ack: None,
            },
        );
        Reception::Fresh
    }

    /// Remembers the ACK emitted for a Message ID so a duplicate can
    /// be answered from the cache (Spec §4.5).
    pub fn remember_ack(&mut self, endpoint: &[u8], message_id: u16, ack: Vec<u8>) -> bool {
        match self.seen.get_mut(&(endpoint.to_vec(), message_id)) {
            Some(entry) => {
                entry.cached_ack = Some(ack);
                true
            }
            None => false,
        }
    }

    /// Removes every entry whose lifetime has run out.
    pub fn evict_expired(&mut self, now_ms: u64) {
        self.seen.retain(|_, v| v.expires_at_ms > now_ms);
    }

    /// Number of remembered Message IDs.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.seen.len()
    }
}

/// Outstanding-interaction accounting — RFC 7252 §4.7.
///
/// Spec §4.7: "a client SHOULD NOT have more than NSTART simultaneous
/// outstanding interactions to a given server".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutstandingInteractions {
    per_endpoint: BTreeMap<Vec<u8>, u32>,
    limit: u32,
}

impl Default for OutstandingInteractions {
    fn default() -> Self {
        Self::new()
    }
}

impl OutstandingInteractions {
    /// Accounting with the spec default `NSTART` = 1.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(NSTART)
    }

    /// Accounting with a caller-chosen `NSTART`.
    #[must_use]
    pub fn with_limit(limit: u32) -> Self {
        Self {
            per_endpoint: BTreeMap::new(),
            limit,
        }
    }

    /// Tries to start an interaction. `false` means the `NSTART` cap
    /// for that server is reached and the caller must wait.
    pub fn try_begin(&mut self, endpoint: &[u8]) -> bool {
        let slot = self.per_endpoint.entry(endpoint.to_vec()).or_insert(0);
        if *slot >= self.limit {
            return false;
        }
        *slot += 1;
        true
    }

    /// Ends an interaction (ACK, RST or final timeout).
    pub fn end(&mut self, endpoint: &[u8]) {
        if let Some(slot) = self.per_endpoint.get_mut(endpoint) {
            *slot = slot.saturating_sub(1);
            if *slot == 0 {
                self.per_endpoint.remove(endpoint);
            }
        }
    }

    /// Currently outstanding interactions towards one server.
    #[must_use]
    pub fn outstanding(&self, endpoint: &[u8]) -> u32 {
        self.per_endpoint.get(endpoint).copied().unwrap_or(0)
    }

    /// The configured `NSTART`.
    #[must_use]
    pub const fn limit(&self) -> u32 {
        self.limit
    }
}

fn initial_interval() -> u64 {
    // Spec §4.8: ACK_TIMEOUT * ACK_RANDOM_FACTOR. We use the upper
    // bound of the random range deterministically (Caller can
    // randomize externally if desired).
    ACK_TIMEOUT_MS * ACK_RANDOM_FACTOR_NUM / ACK_RANDOM_FACTOR_DEN
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn fresh_tracker_is_empty() {
        let t = ReliabilityTracker::new();
        assert_eq!(t.pending_count(), 0);
    }

    #[test]
    fn send_then_ack_clears_pending() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![1, 2], alloc::vec![0; 10], 0);
        assert_eq!(t.pending_count(), 1);
        assert!(t.receive_ack(42));
        assert_eq!(t.pending_count(), 0);
    }

    #[test]
    fn unknown_ack_returns_false() {
        let mut t = ReliabilityTracker::new();
        assert!(!t.receive_ack(99));
    }

    #[test]
    fn rst_clears_pending() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![], alloc::vec![], 0);
        assert!(t.receive_rst(42));
        assert_eq!(t.pending_count(), 0);
    }

    #[test]
    fn tick_before_timeout_does_nothing() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![], alloc::vec![0; 10], 0);
        let out = t.tick(100);
        assert!(out.retransmit.is_empty());
        assert!(out.timed_out.is_empty());
        assert_eq!(t.pending_count(), 1);
    }

    #[test]
    fn tick_after_timeout_retransmits() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![], alloc::vec![0; 5], 0);
        let out = t.tick(initial_interval() + 1);
        assert_eq!(out.retransmit.len(), 1);
        assert_eq!(out.retransmit[0].message_id, 42);
        assert!(out.timed_out.is_empty());
    }

    #[test]
    fn exhausting_retransmits_times_out() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![], alloc::vec![0; 5], 0);
        let mut now = 0u64;
        let mut interval = initial_interval();
        // Drain MAX_RETRANSMIT retransmits.
        for _ in 0..MAX_RETRANSMIT {
            now += interval + 1;
            interval *= 2;
            let _ = t.tick(now);
        }
        // One more tick should now mark the message as timed_out.
        now += interval + 1;
        let out = t.tick(now);
        assert!(!out.timed_out.is_empty(), "should be timed out");
        assert!(t.pending_count() == 0);
    }

    #[test]
    fn interval_doubles_per_retransmit() {
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(42, alloc::vec![], alloc::vec![0; 5], 0);
        let _ = t.tick(initial_interval() + 1);
        let after_first = t.pending.get(&42).unwrap().current_interval_ms;
        assert_eq!(after_first, initial_interval() * 2);
    }

    #[test]
    fn transmission_parameters_match_spec_4_8_table_2() {
        // RFC 7252 §4.8, Table 2 — the normative default values.
        assert_eq!(ACK_TIMEOUT_MS, 2_000, "ACK_TIMEOUT is 2 seconds");
        assert_eq!(
            (ACK_RANDOM_FACTOR_NUM, ACK_RANDOM_FACTOR_DEN),
            (3, 2),
            "ACK_RANDOM_FACTOR is 1.5"
        );
        assert_eq!(MAX_RETRANSMIT, 4);
        assert_eq!(NSTART, 1);
        assert_eq!(DEFAULT_LEISURE_MS, 5_000, "DEFAULT_LEISURE is 5 seconds");
        assert_eq!(PROBING_RATE_BYTES_PER_SEC, 1, "PROBING_RATE is 1 byte/s");
    }

    #[test]
    fn derived_transmission_parameters_follow_spec_4_8_2_formulas() {
        // RFC 7252 §4.8.2 states each derived value as a formula over
        // the Table 2 parameters; the constants must be that formula,
        // not a separately maintained number.
        assert_eq!(MAX_LATENCY_MS, 100_000);
        assert_eq!(PROCESSING_DELAY_MS, ACK_TIMEOUT_MS);
        // MAX_TRANSMIT_SPAN = ACK_TIMEOUT * (2^MAX_RETRANSMIT - 1) * ACK_RANDOM_FACTOR
        let span = ACK_TIMEOUT_MS * ((1u64 << MAX_RETRANSMIT) - 1) * ACK_RANDOM_FACTOR_NUM
            / ACK_RANDOM_FACTOR_DEN;
        assert_eq!(MAX_TRANSMIT_SPAN_MS, span);
        assert_eq!(MAX_TRANSMIT_SPAN_MS, 45_000);
        // MAX_TRANSMIT_WAIT = ACK_TIMEOUT * (2^(MAX_RETRANSMIT+1) - 1) * ACK_RANDOM_FACTOR
        let wait = ACK_TIMEOUT_MS * ((1u64 << (MAX_RETRANSMIT + 1)) - 1) * ACK_RANDOM_FACTOR_NUM
            / ACK_RANDOM_FACTOR_DEN;
        assert_eq!(MAX_TRANSMIT_WAIT_MS, wait);
        assert_eq!(MAX_TRANSMIT_WAIT_MS, 93_000);
        // EXCHANGE_LIFETIME = MAX_TRANSMIT_SPAN + 2*MAX_LATENCY + PROCESSING_DELAY
        assert_eq!(
            EXCHANGE_LIFETIME_MS,
            MAX_TRANSMIT_SPAN_MS + 2 * MAX_LATENCY_MS + PROCESSING_DELAY_MS
        );
        assert_eq!(EXCHANGE_LIFETIME_MS, 247_000);
        // NON_LIFETIME = MAX_TRANSMIT_SPAN + MAX_LATENCY
        assert_eq!(NON_LIFETIME_MS, MAX_TRANSMIT_SPAN_MS + MAX_LATENCY_MS);
        assert_eq!(NON_LIFETIME_MS, 145_000);
    }

    #[test]
    fn backoff_schedule_follows_ack_timeout_and_random_factor() {
        // Spec §4.2: the first timeout is ACK_TIMEOUT scaled by
        // ACK_RANDOM_FACTOR, and each retransmit doubles it. With the
        // Table 2 defaults that is 3000, 6000, 12000, 24000 ms, and
        // MAX_RETRANSMIT retransmits are emitted before the exchange
        // is given up.
        let mut t = ReliabilityTracker::new();
        t.send_confirmable(7, alloc::vec![9], alloc::vec![0; 4], 0);
        let expected = [3_000u64, 6_000, 12_000, 24_000];
        let mut now = 0u64;
        for (i, interval) in expected.iter().enumerate() {
            now += interval;
            let out = t.tick(now);
            assert_eq!(out.retransmit.len(), 1, "retransmit {i} is due at {now} ms");
            assert_eq!(out.retransmit[0].message_id, 7);
            assert_eq!(out.retransmit[0].current_interval_ms, interval * 2);
            assert!(out.timed_out.is_empty());
        }
        // The four retransmits are spent; the next due tick gives up.
        assert_eq!(expected.len(), MAX_RETRANSMIT as usize);
        now += 48_000;
        let out = t.tick(now);
        assert!(out.retransmit.is_empty());
        assert_eq!(out.timed_out, alloc::vec![7]);
        assert_eq!(t.pending_count(), 0);
    }

    #[test]
    fn duplicate_message_id_within_exchange_lifetime_is_detected() {
        // Spec §4.5: a repeat of the same Message ID from the same
        // endpoint inside EXCHANGE_LIFETIME is a duplicate and is not
        // processed a second time.
        let mut d = DuplicateDetector::new();
        let ep = b"10.0.0.1:5683";
        assert_eq!(
            d.receive(ep, 0x1234, DedupKind::Confirmable, 0),
            Reception::Fresh
        );
        assert!(d.remember_ack(ep, 0x1234, alloc::vec![0x60, 0x45, 0x12, 0x34]));
        let again = d.receive(ep, 0x1234, DedupKind::Confirmable, 1_000);
        assert_eq!(
            again,
            Reception::Duplicate {
                cached_ack: Some(alloc::vec![0x60, 0x45, 0x12, 0x34]),
            },
            "the cached ACK is echoed instead of re-processing"
        );
    }

    #[test]
    fn duplicate_entry_expires_after_its_lifetime() {
        let mut d = DuplicateDetector::new();
        let ep = b"10.0.0.1:5683";
        let _ = d.receive(ep, 1, DedupKind::Confirmable, 0);
        let _ = d.receive(ep, 2, DedupKind::NonConfirmable, 0);
        assert_eq!(d.tracked(), 2);
        // NON_LIFETIME (145 s) runs out first, EXCHANGE_LIFETIME
        // (247 s) later.
        d.evict_expired(NON_LIFETIME_MS + 1);
        assert_eq!(d.tracked(), 1);
        assert_eq!(
            d.receive(ep, 2, DedupKind::NonConfirmable, NON_LIFETIME_MS + 1),
            Reception::Fresh
        );
        d.evict_expired(EXCHANGE_LIFETIME_MS + 1);
        assert_eq!(
            d.receive(ep, 1, DedupKind::Confirmable, EXCHANGE_LIFETIME_MS + 1),
            Reception::Fresh
        );
    }

    #[test]
    fn duplicate_detection_is_per_endpoint() {
        let mut d = DuplicateDetector::new();
        let _ = d.receive(b"a", 5, DedupKind::Confirmable, 0);
        assert_eq!(
            d.receive(b"b", 5, DedupKind::Confirmable, 0),
            Reception::Fresh,
            "the same Message ID from another endpoint is a new message"
        );
    }

    #[test]
    fn duplicate_table_cap_bounds_memory() {
        let mut d = DuplicateDetector::with_cap(2);
        let _ = d.receive(b"a", 1, DedupKind::NonConfirmable, 0);
        let _ = d.receive(b"a", 2, DedupKind::Confirmable, 0);
        let _ = d.receive(b"a", 3, DedupKind::Confirmable, 0);
        assert_eq!(d.tracked(), 2);
    }

    #[test]
    fn nstart_caps_outstanding_interactions_per_server() {
        // Spec §4.7: NSTART = 1 — one outstanding interaction per
        // server until it is ACKed, RST or has timed out.
        let mut o = OutstandingInteractions::new();
        assert_eq!(o.limit(), NSTART);
        assert!(o.try_begin(b"server-a"));
        assert!(
            !o.try_begin(b"server-a"),
            "a second interaction to the same server must wait"
        );
        assert!(
            o.try_begin(b"server-b"),
            "the cap is per server, not global"
        );
        assert_eq!(o.outstanding(b"server-a"), 1);
        o.end(b"server-a");
        assert_eq!(o.outstanding(b"server-a"), 0);
        assert!(o.try_begin(b"server-a"));
    }
}
