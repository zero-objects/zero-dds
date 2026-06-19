// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! NetworkRequirements — Spec §7.2.3.1.2 Tab 7.18 (Talker) +
//! §7.2.3.2.1 Tab 7.25 (Listener).
//!
//! Presents a stream's user requirements to the TSN network: the
//! number of redundant, seamless trees (IEEE 802.1CB FRER) and the
//! maximum tolerated latency talker→listener.
//!
//! The spec models `max_latency` in Tab 7.18 as `UInt32` and in
//! Tab 7.25 as `String8` ("specified in nanoseconds"). Both
//! represent the same nanosecond value; we unify on
//! `u32` nanoseconds, which maps the talker `UInt32` form directly
//! and also fits the `max-latency` leaf in the YANG mapping (§7.3.3).

/// Spec Tab 7.18 / Tab 7.25 — `NetworkRequirements`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NetworkRequirements {
    /// `num_seamless_trees` (`UInt8`, mult 1). Number of trees the
    /// network provides for seamless redundancy of the stream. `0`
    /// means: no redundancy needed (IEEE 802.1Q §46.2.3.6.1).
    pub num_seamless_trees: u8,
    /// `max_latency` (nanoseconds, mult 1). Maximum latency talker→
    /// listener for a single stream frame (IEEE 802.1Q
    /// §46.2.3.6.2). `0` means "no latency requirement".
    pub max_latency_ns: u32,
}

impl NetworkRequirements {
    /// Constructs a `NetworkRequirements` requirement.
    #[must_use]
    pub const fn new(num_seamless_trees: u8, max_latency_ns: u32) -> Self {
        Self {
            num_seamless_trees,
            max_latency_ns,
        }
    }

    /// `true` if the stream demands seamless redundancy (≥1 tree)
    /// (IEEE 802.1CB FRER active).
    #[must_use]
    pub const fn requires_seamless_redundancy(self) -> bool {
        self.num_seamless_trees > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_both_fields() {
        let nr = NetworkRequirements::new(2, 500_000);
        assert_eq!(nr.num_seamless_trees, 2);
        assert_eq!(nr.max_latency_ns, 500_000);
    }

    #[test]
    fn zero_trees_means_no_redundancy_per_spec_46_2_3_6_1() {
        let nr = NetworkRequirements::new(0, 1_000);
        assert!(!nr.requires_seamless_redundancy());
    }

    #[test]
    fn one_or_more_trees_requests_frer() {
        assert!(NetworkRequirements::new(1, 0).requires_seamless_redundancy());
        assert!(NetworkRequirements::new(3, 0).requires_seamless_redundancy());
    }

    #[test]
    fn default_is_no_redundancy_no_latency_bound() {
        let nr = NetworkRequirements::default();
        assert_eq!(nr.num_seamless_trees, 0);
        assert_eq!(nr.max_latency_ns, 0);
        assert!(!nr.requires_seamless_redundancy());
    }
}
