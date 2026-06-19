// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TrafficSpecification — Spec §7.2.3 Tab 7.16 (IEEE 802.1Qcc).

/// Spec — `TransmissionSelection` algorithm per stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransmissionSelection {
    /// `0` Strict-Priority (Default IEEE 802.1Q).
    StrictPriority,
    /// `1` CBS (Credit-Based Shaper, IEEE 802.1Qav).
    CreditBasedShaper,
    /// `2` ETS (Enhanced Transmission Selection, IEEE 802.1Qaz).
    EnhancedTransmissionSelection,
    /// `3` ATS (Asynchronous Traffic Shaping, IEEE 802.1Qcr).
    AsynchronousTrafficShaping,
}

/// Spec §7.2.3 Tab 7.16 — `TrafficSpecification`.
///
/// Describes a TSN stream's traffic-pattern requirements on the
/// bridge network: inter-frame interval + max frame size +
/// max frames per interval + transmission-selection algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficSpecification {
    /// Spec — `interval` (ns) between frames.
    pub interval_nanoseconds: u64,
    /// Spec — `max_frame_size` (bytes) incl. L2 header.
    pub max_frame_size: u32,
    /// Spec — `max_frames_per_interval`.
    pub max_frames_per_interval: u32,
    /// Spec — `transmission_selection`.
    pub transmission_selection: TransmissionSelection,
}

impl TrafficSpecification {
    /// Converts the specified data rate to bytes/second.
    /// Helpful for a sanity check on whether the TSN bridge can
    /// reserve the bandwidth.
    #[must_use]
    pub const fn bytes_per_second(self) -> u64 {
        if self.interval_nanoseconds == 0 {
            return u64::MAX; // unrealistic — the caller should check this.
        }
        // bytes/interval * 1e9 / interval_ns = bytes/s
        let bytes_per_interval = self.max_frame_size as u64 * self.max_frames_per_interval as u64;
        let scaled = bytes_per_interval.saturating_mul(1_000_000_000);
        match scaled.checked_div(self.interval_nanoseconds) {
            Some(v) => v,
            None => u64::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transmission_selection_distinct_values() {
        // Spec §7.2.3.
        assert_ne!(
            TransmissionSelection::StrictPriority,
            TransmissionSelection::CreditBasedShaper
        );
        assert_ne!(
            TransmissionSelection::CreditBasedShaper,
            TransmissionSelection::EnhancedTransmissionSelection
        );
        assert_ne!(
            TransmissionSelection::EnhancedTransmissionSelection,
            TransmissionSelection::AsynchronousTrafficShaping
        );
    }

    #[test]
    fn bytes_per_second_calculation_for_1ms_interval_1500_byte_frame() {
        // 1500 bytes/frame @ 1 frame/ms = 1.5MB/s.
        let t = TrafficSpecification {
            interval_nanoseconds: 1_000_000, // 1ms.
            max_frame_size: 1500,
            max_frames_per_interval: 1,
            transmission_selection: TransmissionSelection::StrictPriority,
        };
        assert_eq!(t.bytes_per_second(), 1_500_000);
    }

    #[test]
    fn bytes_per_second_for_125us_interval_8x_64_byte_frame() {
        // 64 bytes * 8 frames / 125us = 512 bytes / 125us = 4.096 MB/s.
        let t = TrafficSpecification {
            interval_nanoseconds: 125_000,
            max_frame_size: 64,
            max_frames_per_interval: 8,
            transmission_selection: TransmissionSelection::CreditBasedShaper,
        };
        assert_eq!(t.bytes_per_second(), 4_096_000);
    }

    #[test]
    fn bytes_per_second_zero_interval_returns_max() {
        let t = TrafficSpecification {
            interval_nanoseconds: 0,
            max_frame_size: 100,
            max_frames_per_interval: 1,
            transmission_selection: TransmissionSelection::StrictPriority,
        };
        assert_eq!(t.bytes_per_second(), u64::MAX);
    }
}
