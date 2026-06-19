// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Exponential backoff for reconnect loops.
//!
//! Spec: `zerodds-amqp-bridge-daemon-1.0.md` §9.3 — reconnect strategy
//! with exponential backoff, capped at `max_ms`.
//!
//! Pure-Rust no_std. The caller combines this with an AMQP connect
//! attempt (TCP/TLS + AMQP-OPEN frame).

use core::time::Duration;

/// Backoff configuration.
///
/// Spec §9.3: initial=100ms, multiplier=2, max=30s; max_attempts
/// `u32::MAX` ⇒ infinite retries (the daemon loop terminates via the
/// stop flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffConfig {
    /// Initial backoff in ms.
    pub initial_ms: u64,
    /// Hard cap in ms.
    pub max_ms: u64,
    /// Multiplier per failed attempt.
    pub multiplier: u64,
    /// Max. attempts (`u32::MAX` = infinite).
    pub max_attempts: u32,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 100,
            max_ms: 30_000,
            multiplier: 2,
            max_attempts: u32::MAX,
        }
    }
}

impl BackoffConfig {
    /// Compute the delay for attempt `attempt` (0-based).
    /// Capped at `max_ms`.
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let mut d = self.initial_ms;
        for _ in 0..attempt {
            d = d.saturating_mul(self.multiplier);
            if d >= self.max_ms {
                d = self.max_ms;
                break;
            }
        }
        Duration::from_millis(d)
    }

    /// Check whether the `attempt`-th attempt is still permitted.
    #[must_use]
    pub const fn allow(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_starts_at_100ms() {
        let b = BackoffConfig::default();
        assert_eq!(b.delay_for(0), Duration::from_millis(100));
    }

    #[test]
    fn doubles_each_attempt() {
        let b = BackoffConfig {
            initial_ms: 50,
            max_ms: 10_000,
            multiplier: 2,
            max_attempts: 100,
        };
        assert_eq!(b.delay_for(0), Duration::from_millis(50));
        assert_eq!(b.delay_for(1), Duration::from_millis(100));
        assert_eq!(b.delay_for(2), Duration::from_millis(200));
        assert_eq!(b.delay_for(3), Duration::from_millis(400));
    }

    #[test]
    fn caps_at_max_ms() {
        let b = BackoffConfig {
            initial_ms: 100,
            max_ms: 1_000,
            multiplier: 4,
            max_attempts: 100,
        };
        // 100, 400, 1000 (capped), 1000, ...
        assert_eq!(b.delay_for(2), Duration::from_millis(1_000));
        assert_eq!(b.delay_for(20), Duration::from_millis(1_000));
    }

    #[test]
    fn allow_respects_max_attempts() {
        let b = BackoffConfig {
            initial_ms: 1,
            max_ms: 10,
            multiplier: 2,
            max_attempts: 3,
        };
        assert!(b.allow(0));
        assert!(b.allow(1));
        assert!(b.allow(2));
        assert!(!b.allow(3));
    }
}
