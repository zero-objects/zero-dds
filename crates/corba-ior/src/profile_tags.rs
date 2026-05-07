// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR Profile-Tags — Spec §13.6.7.1.
//!
//! `ProfileId` ist ein `unsigned long`. OMG-registrierte Werte:
//! * `TAG_INTERNET_IOP = 0` — IIOP-Profile (Spec §15.7.2 ProfileBody).
//! * `TAG_MULTIPLE_COMPONENTS = 1` — Profile aus Components ohne
//!   eigenes Transport-Mapping (Spec §13.6.4).
//! * `TAG_SCCP_IOP = 2` — Telco-SCCP.
//! * `TAG_UIPMC = 3` — Unreliable IP Multicast.
//! * `TAG_MOB_INET_IOP = 4` — Mobile-IIOP.

/// `ProfileId` — Spec §13.6.7.1. Wir modellieren als Enum, halten aber
/// einen Sentinel `Other(u32)` fuer Vendor-/Unbekannt-Tags.
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
    /// Andere/Vendor-spezifische Tag-Wert.
    Other(u32),
}

impl ProfileId {
    /// Roher `unsigned long`-Wert.
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

    /// Konstruiert aus einem `unsigned long` (alle Werte zulaessig).
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
