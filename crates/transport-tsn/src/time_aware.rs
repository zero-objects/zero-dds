// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TimeAware Stream — Spec §7.2.3 Tab 7.17 (IEEE 802.1Qbv).

/// Spec §7.2.3 Tab 7.17 — `TimeAware` configuration of a TSN stream.
///
/// Guarantees that frames are only sent within a time window
/// `[earliest_transmit_offset, latest_transmit_offset]` (relative to the
/// period start). Jitter describes the permissible
/// variation.
///
/// Precondition: talker + listener must be IEEE 802.1AS time-
/// synchronized (typically via gPTP / `linuxptp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeAware {
    /// Spec — `earliest_transmit_offset` (ns) from the period start.
    pub earliest_transmit_offset_ns: u64,
    /// Spec — `latest_transmit_offset` (ns) from the period start.
    pub latest_transmit_offset_ns: u64,
    /// Spec — `jitter` (ns).
    pub jitter_ns: u64,
}

impl TimeAware {
    /// Window length (latest - earliest), or `None` if invalid.
    #[must_use]
    pub fn window_ns(self) -> Option<u64> {
        if self.latest_transmit_offset_ns >= self.earliest_transmit_offset_ns {
            Some(self.latest_transmit_offset_ns - self.earliest_transmit_offset_ns)
        } else {
            None
        }
    }

    /// `true` if the configuration is consistent:
    /// `earliest <= latest` and `jitter <= window`.
    #[must_use]
    pub fn is_valid(self) -> bool {
        let Some(w) = self.window_ns() else {
            return false;
        };
        self.jitter_ns <= w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_window_is_latest_minus_earliest() {
        let t = TimeAware {
            earliest_transmit_offset_ns: 1_000,
            latest_transmit_offset_ns: 2_500,
            jitter_ns: 500,
        };
        assert_eq!(t.window_ns(), Some(1_500));
        assert!(t.is_valid());
    }

    #[test]
    fn invalid_when_earliest_above_latest() {
        let t = TimeAware {
            earliest_transmit_offset_ns: 2_000,
            latest_transmit_offset_ns: 1_000,
            jitter_ns: 100,
        };
        assert_eq!(t.window_ns(), None);
        assert!(!t.is_valid());
    }

    #[test]
    fn invalid_when_jitter_exceeds_window() {
        let t = TimeAware {
            earliest_transmit_offset_ns: 0,
            latest_transmit_offset_ns: 1_000,
            jitter_ns: 1_500,
        };
        assert_eq!(t.window_ns(), Some(1_000));
        assert!(!t.is_valid());
    }

    #[test]
    fn zero_jitter_is_valid_boundary() {
        let t = TimeAware {
            earliest_transmit_offset_ns: 0,
            latest_transmit_offset_ns: 1_000,
            jitter_ns: 0,
        };
        assert!(t.is_valid());
    }
}
