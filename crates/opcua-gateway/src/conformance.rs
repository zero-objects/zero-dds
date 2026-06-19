// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Spec §2 — declared OPC UA/DDS Gateway conformance points.
//!
//! DDS-OPC UA Gateway 1.0 §2 defines **four** conformance points. This module
//! is the explicit, machine-queryable **multi-point conformance marker** for
//! `zerodds-opcua-gateway`: it declares which points the crate satisfies and
//! cross-references the module that implements each, so a downstream
//! server-stack integrator (or an audit tool) can interrogate the gateway's
//! conformance programmatically instead of reading prose.
//!
//! The crate is a **multi-point** conformant implementation — it declares all
//! four points (the backing capabilities — type-system mapping, service-sets,
//! subscription behavior, AddressSpace mapping, historical access — are
//! implemented in the referenced modules).

/// One of the four DDS-OPC UA Gateway 1.0 §2 conformance points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConformancePoint {
    /// §2 Point 1 — *OPC UA to DDS Mapping Basic*: UA type-system mapping +
    /// UA subscription mapping. Impl: [`crate::types`] + [`crate::subscription_mapping`].
    UaToDdsBasic,
    /// §2 Point 2 — *OPC UA to DDS Mapping Complete*: Basic + UA service-sets.
    /// Impl: Point 1 + [`crate::service_sets`].
    UaToDdsComplete,
    /// §2 Point 3 — *DDS to OPC UA Mapping Basic*: DDS type-system mapping +
    /// AddressSpace mapping. Impl: [`crate::dds_to_ua`] + [`crate::address_space`].
    DdsToUaBasic,
    /// §2 Point 4 — *DDS to OPC UA Mapping Complete*: Basic + historical access.
    /// Impl: Point 3 + [`crate::historical`].
    DdsToUaComplete,
}

impl ConformancePoint {
    /// All four §2 conformance points, in spec order.
    pub const ALL: [ConformancePoint; 4] = [
        ConformancePoint::UaToDdsBasic,
        ConformancePoint::UaToDdsComplete,
        ConformancePoint::DdsToUaBasic,
        ConformancePoint::DdsToUaComplete,
    ];

    /// Human-readable title as printed in the spec (§2).
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            ConformancePoint::UaToDdsBasic => "OPC UA to DDS Mapping Basic",
            ConformancePoint::UaToDdsComplete => "OPC UA to DDS Mapping Complete",
            ConformancePoint::DdsToUaBasic => "DDS to OPC UA Mapping Basic",
            ConformancePoint::DdsToUaComplete => "DDS to OPC UA Mapping Complete",
        }
    }

    /// Spec clause / point number (`§2 Point N`).
    #[must_use]
    pub fn spec_point(self) -> u8 {
        match self {
            ConformancePoint::UaToDdsBasic => 1,
            ConformancePoint::UaToDdsComplete => 2,
            ConformancePoint::DdsToUaBasic => 3,
            ConformancePoint::DdsToUaComplete => 4,
        }
    }

    /// The "Complete" point of each direction subsumes its "Basic" point
    /// (Complete = Basic + the extra service-sets / historical sets).
    #[must_use]
    pub fn subsumes_basic(self) -> Option<ConformancePoint> {
        match self {
            ConformancePoint::UaToDdsComplete => Some(ConformancePoint::UaToDdsBasic),
            ConformancePoint::DdsToUaComplete => Some(ConformancePoint::DdsToUaBasic),
            _ => None,
        }
    }
}

/// The conformance points `zerodds-opcua-gateway` declares as satisfied — all
/// four (a **multi-point** conformant gateway, Spec §2).
pub const DECLARED_CONFORMANCE: [ConformancePoint; 4] = ConformancePoint::ALL;

/// `true` if the crate declares **multi-point** conformance, i.e. it satisfies
/// two or more §2 conformance points (it satisfies all four).
#[must_use]
pub fn is_multi_point_conformant() -> bool {
    DECLARED_CONFORMANCE.len() >= 2
}

/// `true` if `point` is among the declared conformance points.
#[must_use]
pub fn declares(point: ConformancePoint) -> bool {
    DECLARED_CONFORMANCE.contains(&point)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn declares_all_four_conformance_points() {
        assert_eq!(DECLARED_CONFORMANCE.len(), 4);
        for p in ConformancePoint::ALL {
            assert!(declares(p), "must declare {p:?}");
        }
    }

    #[test]
    fn is_multi_point() {
        // The whole point of this marker: an explicit multi-point declaration.
        assert!(is_multi_point_conformant());
    }

    #[test]
    fn points_are_numbered_one_to_four_uniquely() {
        let mut nums: alloc::vec::Vec<u8> = ConformancePoint::ALL
            .iter()
            .map(|p| p.spec_point())
            .collect();
        nums.sort_unstable();
        assert_eq!(nums, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn complete_points_subsume_their_basic() {
        assert_eq!(
            ConformancePoint::UaToDdsComplete.subsumes_basic(),
            Some(ConformancePoint::UaToDdsBasic)
        );
        assert_eq!(
            ConformancePoint::DdsToUaComplete.subsumes_basic(),
            Some(ConformancePoint::DdsToUaBasic)
        );
        assert_eq!(ConformancePoint::UaToDdsBasic.subsumes_basic(), None);
    }

    #[test]
    fn every_point_has_a_distinct_title() {
        let mut titles: alloc::vec::Vec<&str> =
            ConformancePoint::ALL.iter().map(|p| p.title()).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), 4);
    }
}
