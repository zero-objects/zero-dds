// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR profile tags — spec §13.6.7.1.
//!
//! `ProfileId` is an `unsigned long`. OMG-registered values:
//! * `TAG_INTERNET_IOP = 0` — IIOP profile (spec §15.7.2 ProfileBody).
//! * `TAG_MULTIPLE_COMPONENTS = 1` — profile from components without its
//!   own transport mapping (spec §13.6.4).
//! * `TAG_SCCP_IOP = 2` — telco SCCP.
//! * `TAG_UIPMC = 3` — unreliable IP multicast.
//! * `TAG_MOB_INET_IOP = 4` — mobile IIOP.

/// `ProfileId` — spec §13.6.7.1. We model it as an enum but keep a
/// sentinel `Other(u32)` for vendor/unknown tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileId {
    /// `TAG_INTERNET_IOP = 0`.
    InternetIop,
    /// `TAG_MULTIPLE_COMPONENTS = 1`.
    MultipleComponents,
    /// `TAG_SCCP_IOP = 2`.
    SccpIop,
    /// `TAG_UIPMC = 3`.
    Uipmc,
    /// `TAG_MOB_INET_IOP = 4`.
    MobInetIop,
    /// Other/vendor-specific tag value.
    Other(u32),
}

impl ProfileId {
    /// Raw `unsigned long` value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::InternetIop => 0,
            Self::MultipleComponents => 1,
            Self::SccpIop => 2,
            Self::Uipmc => 3,
            Self::MobInetIop => 4,
            Self::Other(v) => v,
        }
    }

    /// Constructs from an `unsigned long` (all values are valid).
    #[must_use]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::InternetIop,
            1 => Self::MultipleComponents,
            2 => Self::SccpIop,
            3 => Self::Uipmc,
            4 => Self::MobInetIop,
            v => Self::Other(v),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn standard_profile_tag_values_match_spec() {
        // Spec §13.6.7.1 Table 13-1.
        assert_eq!(ProfileId::InternetIop.as_u32(), 0);
        assert_eq!(ProfileId::MultipleComponents.as_u32(), 1);
        assert_eq!(ProfileId::SccpIop.as_u32(), 2);
        assert_eq!(ProfileId::Uipmc.as_u32(), 3);
        assert_eq!(ProfileId::MobInetIop.as_u32(), 4);
    }

    #[test]
    fn round_trip_all_known_tags() {
        for v in 0u32..=4 {
            assert_eq!(ProfileId::from_u32(v).as_u32(), v);
        }
    }

    #[test]
    fn unknown_tag_round_trips_via_other() {
        assert_eq!(ProfileId::from_u32(99).as_u32(), 99);
        assert!(matches!(ProfileId::from_u32(99), ProfileId::Other(99)));
    }
}
