// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP-Reliability — RFC 7252 §4.2-§4.7.
//!
//! Spec §4.2: CON-Messages erwarten ACK; bei Timeout wird mit
//! exponentiellem Backoff retransmittet bis `MAX_RETRANSMIT`.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Retransmit-Default-Konstanten — RFC 7252 §4.8.
pub const ACK_TIMEOUT_MS: u64 = 2000;
/// `ACK_RANDOM_FACTOR` (Spec §4.8) — wir verwenden 1.5 gerundet auf
/// einen Multiplikator von 3/2.
pub const ACK_RANDOM_FACTOR_NUM: u64 = 3;
/// Nenner zu `ACK_RANDOM_FACTOR_NUM`.
pub const ACK_RANDOM_FACTOR_DEN: u64 = 2;
/// `MAX_RETRANSMIT` (Spec §4.8).
pub const MAX_RETRANSMIT: u32 = 4;

/// Pending-CON-Eintrag in der Reliability-Queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmable {
    /// Message-Id (16-Bit).
    pub message_id: u16,
    /// Token-Bytes (0..=8).
    pub token: Vec<u8>,
    /// Encoded Message-Bytes — werden bei Retransmit re-emittiert.
    pub bytes: Vec<u8>,
    /// Verbleibende Retransmits (start = `MAX_RETRANSMIT`).
    pub retransmits_left: u32,
    /// Naechstes Timeout (in Millisekunden seit Start).
    pub next_timeout_ms: u64,
    /// Aktuelles Timeout-Intervall (verdoppelt sich pro Retransmit).
    pub current_interval_ms: u64,
}

/// Reliability-Tracker — Caller pumpt Zeit + Empfangs-Events rein,
/// Tracker liefert Retransmit/Drop-Decisions.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReliabilityTracker {
    pending: BTreeMap<u16, PendingConfirmable>,
}

/// Output eines `tick`-Schritts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TickOutput {
    /// Messages, die retransmittiert werden muessen (Bytes-Slice
    /// bereits enthalten).
    pub retransmit: Vec<PendingConfirmable>,
    /// Message-Ids, deren `MAX_RETRANSMIT` erschoepft sind und
    /// deren Verbindung als verloren angesehen wird.
    pub timed_out: Vec<u16>,
}

impl ReliabilityTracker {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sendet eine CON — registriert sie zur Retransmission.
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

    /// ACK empfangen — entferne Pending-Eintrag.
    pub fn receive_ack(&mut self, message_id: u16) -> bool {
        self.pending.remove(&message_id).is_some()
    }

    /// RST empfangen — entferne Pending-Eintrag (Spec §4.2: RST
    /// terminiert ohne Retransmit).
    pub fn receive_rst(&mut self, message_id: u16) -> bool {
        self.pending.remove(&message_id).is_some()
    }

    /// Tick — pruefe ob ein Pending faellig ist.
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

    /// Anzahl pending CONs.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
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
}
