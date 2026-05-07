// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 Keep-Alive-Tracker — Spec §3.1.2.10.
//!
//! "If the Keep Alive value is non-zero and the Server does not receive
//! an MQTT Control Packet from the Client within one and a half times
//! the Keep Alive time period, it MUST close the Network Connection
//! to the Client as if the network had failed."
//!
//! Wir kapseln den Keep-Alive-Timer als Wall-Clock-Tick. Caller
//! ruft `record_packet()` bei jedem eingehenden Packet und `tick()`
//! periodisch (z.B. einmal pro Sekunde).

use core::time::Duration;
use std::time::Instant;

/// Keep-Alive-Tracker pro Session.
#[derive(Debug, Clone)]
pub struct KeepAliveTracker {
    /// Spec §3.1.2.10 — `Keep Alive`-Wert in Sekunden. `0` = disabled.
    keep_alive_secs: u16,
    /// Letzter Zeitpunkt eines empfangenen Packets.
    last_packet: Instant,
}

impl KeepAliveTracker {
    /// Konstruktor — wird beim CONNECT-Empfang aktiviert.
    #[must_use]
    pub fn new(keep_alive_secs: u16) -> Self {
        Self {
            keep_alive_secs,
            last_packet: Instant::now(),
        }
    }

    /// Spec §3.1.2.10 — `0` = Keep-Alive disabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.keep_alive_secs > 0
    }

    /// Caller ruft das bei jedem eingehenden Packet (einschliesslich
    /// PINGREQ).
    pub fn record_packet(&mut self) {
        self.last_packet = Instant::now();
    }

    /// Spec §3.1.2.10 — Server schliesst Connection nach 1.5 ×
    /// Keep-Alive-Period ohne Packet.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }
        let limit = Duration::from_millis(u64::from(self.keep_alive_secs) * 1500);
        self.last_packet.elapsed() > limit
    }

    /// Verbleibende Zeit bis zum Expiry (oder `None` wenn Keep-Alive
    /// disabled).
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        if !self.is_enabled() {
            return None;
        }
        let limit = Duration::from_millis(u64::from(self.keep_alive_secs) * 1500);
        let elapsed = self.last_packet.elapsed();
        Some(limit.saturating_sub(elapsed))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn keep_alive_zero_is_disabled() {
        let t = KeepAliveTracker::new(0);
        assert!(!t.is_enabled());
        assert!(!t.is_expired());
        assert!(t.remaining().is_none());
    }

    #[test]
    fn fresh_tracker_not_expired() {
        let t = KeepAliveTracker::new(60);
        assert!(t.is_enabled());
        assert!(!t.is_expired());
    }

    #[test]
    fn record_packet_resets_timer() {
        let mut t = KeepAliveTracker::new(10);
        std::thread::sleep(Duration::from_millis(20));
        t.record_packet();
        assert!(!t.is_expired());
    }

    #[test]
    fn small_keep_alive_expires_after_1_5x() {
        // 1 second keep alive → expires after >1.5s
        let mut t = KeepAliveTracker::new(1);
        std::thread::sleep(Duration::from_millis(1600));
        assert!(t.is_expired());
        // Reset
        t.record_packet();
        assert!(!t.is_expired());
    }

    #[test]
    fn remaining_decreases_over_time() {
        let t = KeepAliveTracker::new(60);
        let r1 = t.remaining().expect("enabled");
        std::thread::sleep(Duration::from_millis(50));
        let r2 = t.remaining().expect("enabled");
        assert!(r2 < r1);
    }
}
