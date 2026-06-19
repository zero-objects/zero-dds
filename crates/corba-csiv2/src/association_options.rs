// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AssociationOptions — Spec §24.2.4 (spec Table 24-1).
//!
//! Bitmask that is set in `target_supports` and `target_requires`
//! (`unsigned short`).

/// Spec §24.2.4 — AssociationOptions bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AssociationOptions(pub u16);

impl AssociationOptions {
    /// `NoProtection = 1`.
    pub const NO_PROTECTION: u16 = 1;
    /// `Integrity = 2`.
    pub const INTEGRITY: u16 = 2;
    /// `Confidentiality = 4`.
    pub const CONFIDENTIALITY: u16 = 4;
    /// `DetectReplay = 8`.
    pub const DETECT_REPLAY: u16 = 8;
    /// `DetectMisordering = 16`.
    pub const DETECT_MISORDERING: u16 = 16;
    /// `EstablishTrustInTarget = 32`.
    pub const ESTABLISH_TRUST_IN_TARGET: u16 = 32;
    /// `EstablishTrustInClient = 64`.
    pub const ESTABLISH_TRUST_IN_CLIENT: u16 = 64;
    /// `NoDelegation = 128`.
    pub const NO_DELEGATION: u16 = 128;
    /// `SimpleDelegation = 256`.
    pub const SIMPLE_DELEGATION: u16 = 256;
    /// `CompositeDelegation = 512`.
    pub const COMPOSITE_DELEGATION: u16 = 512;
    /// `IdentityAssertion = 1024`.
    pub const IDENTITY_ASSERTION: u16 = 1024;
    /// `DelegationByClient = 2048`.
    pub const DELEGATION_BY_CLIENT: u16 = 2048;

    /// Constructor from bits.
    #[must_use]
    pub const fn from_bits(b: u16) -> Self {
        Self(b)
    }

    /// `true` if the flag is set.
    #[must_use]
    pub const fn has(&self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }

    /// Sets a flag.
    #[must_use]
    pub const fn with(mut self, flag: u16) -> Self {
        self.0 |= flag;
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn flag_values_match_spec_table_24_1() {
        // Spec §24.2.4 Table 24-1: Powers of 2.
        assert_eq!(AssociationOptions::NO_PROTECTION, 1);
        assert_eq!(AssociationOptions::INTEGRITY, 2);
        assert_eq!(AssociationOptions::CONFIDENTIALITY, 4);
        assert_eq!(AssociationOptions::DETECT_REPLAY, 8);
        assert_eq!(AssociationOptions::DETECT_MISORDERING, 16);
        assert_eq!(AssociationOptions::ESTABLISH_TRUST_IN_TARGET, 32);
        assert_eq!(AssociationOptions::ESTABLISH_TRUST_IN_CLIENT, 64);
        assert_eq!(AssociationOptions::NO_DELEGATION, 128);
        assert_eq!(AssociationOptions::SIMPLE_DELEGATION, 256);
        assert_eq!(AssociationOptions::COMPOSITE_DELEGATION, 512);
        assert_eq!(AssociationOptions::IDENTITY_ASSERTION, 1024);
        assert_eq!(AssociationOptions::DELEGATION_BY_CLIENT, 2048);
    }

    #[test]
    fn has_and_with_round_trip() {
        let ao = AssociationOptions::default()
            .with(AssociationOptions::CONFIDENTIALITY)
            .with(AssociationOptions::ESTABLISH_TRUST_IN_TARGET);
        assert!(ao.has(AssociationOptions::CONFIDENTIALITY));
        assert!(ao.has(AssociationOptions::ESTABLISH_TRUST_IN_TARGET));
        assert!(!ao.has(AssociationOptions::IDENTITY_ASSERTION));
        assert_eq!(ao.0, 4 | 32);
    }
}
