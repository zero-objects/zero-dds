// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Peer capabilities from the `BuiltinEndpointSet` bitfield.
//!
//! Classifies a remote `ParticipantBuiltinTopicData::builtin_
//! endpoint_set` u32 mask into a high-level capability struct that the
//! caller can use directly for matching. In addition, there are boolean
//! helpers to keep per-bit checks readable.
//!
//! # Spec references
//!
//! - DDSI-RTPS 2.5 §9.3.2.12 (standard bits 0..5, 10..11, 28..29)
//! - DDS-Security 1.2 §7.4.7.1 (secure-discovery bits 16..27)

extern crate alloc;
use zerodds_rtps::participant_data::endpoint_flag;

/// High-level classification of a peer's `BuiltinEndpointSet`.
/// Computed by the DCPS runtime from a peer's SPDP beacon and passed
/// on to the SEDP/WLP/security match logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerCapabilities {
    /// Raw bitmask as announced by the peer — for audit/diagnostic
    /// paths.
    pub raw: u32,
    /// SPDP endpoints (bits 0/1).
    pub has_spdp: bool,
    /// SEDP publications endpoints (bits 2/3).
    pub has_sedp_publications: bool,
    /// SEDP subscriptions endpoints (bits 4/5).
    pub has_sedp_subscriptions: bool,
    /// Writer Liveliness Protocol (bits 10/11).
    pub has_wlp: bool,
    /// TypeLookup service (bits 12/13, XTypes 1.3 §7.6.3.3.4).
    pub has_type_lookup: bool,
    /// XTypes topics discovery (bits 28/29).
    pub has_topics_discovery: bool,
    /// Secure publications endpoints (bits 16/17).
    pub has_secure_publications: bool,
    /// Secure subscriptions endpoints (bits 18/19).
    pub has_secure_subscriptions: bool,
    /// Secure WLP endpoints (bits 20/21).
    pub has_secure_wlp: bool,
    /// Auth stateless endpoints (bits 22/23).
    pub has_stateless_auth: bool,
    /// Crypto key-exchange endpoints (bits 24/25).
    pub has_volatile_secure: bool,
    /// Secure participant discovery (bits 26/27).
    pub has_secure_participant: bool,
}

impl PeerCapabilities {
    /// Classifies a peer bitmask.
    #[must_use]
    pub fn from_bits(raw: u32) -> Self {
        let bit_pair_set = |a: u32, b: u32| -> bool { raw & a != 0 && raw & b != 0 };
        Self {
            raw,
            has_spdp: bit_pair_set(
                endpoint_flag::PARTICIPANT_ANNOUNCER,
                endpoint_flag::PARTICIPANT_DETECTOR,
            ),
            has_sedp_publications: bit_pair_set(
                endpoint_flag::PUBLICATIONS_ANNOUNCER,
                endpoint_flag::PUBLICATIONS_DETECTOR,
            ),
            has_sedp_subscriptions: bit_pair_set(
                endpoint_flag::SUBSCRIPTIONS_ANNOUNCER,
                endpoint_flag::SUBSCRIPTIONS_DETECTOR,
            ),
            has_wlp: bit_pair_set(
                endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER,
                endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER,
            ),
            has_type_lookup: (raw & endpoint_flag::TYPE_LOOKUP_REQUEST != 0)
                && (raw & endpoint_flag::TYPE_LOOKUP_REPLY != 0),
            has_topics_discovery: bit_pair_set(
                endpoint_flag::TOPICS_ANNOUNCER,
                endpoint_flag::TOPICS_DETECTOR,
            ),
            has_secure_publications: bit_pair_set(
                endpoint_flag::PUBLICATIONS_SECURE_WRITER,
                endpoint_flag::PUBLICATIONS_SECURE_READER,
            ),
            has_secure_subscriptions: bit_pair_set(
                endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER,
                endpoint_flag::SUBSCRIPTIONS_SECURE_READER,
            ),
            has_secure_wlp: bit_pair_set(
                endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER,
                endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER,
            ),
            has_stateless_auth: bit_pair_set(
                endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER,
                endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER,
            ),
            has_volatile_secure: bit_pair_set(
                endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
                endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            ),
            has_secure_participant: bit_pair_set(
                endpoint_flag::PARTICIPANT_SECURE_WRITER,
                endpoint_flag::PARTICIPANT_SECURE_READER,
            ),
        }
    }

    /// `true` if the peer has set at least one secure-discovery bit
    /// (sub-bits 16..27). Used by the security path to decide whether a
    /// secure handshake can be attempted or whether the peer expects
    /// plain discovery.
    #[must_use]
    pub fn supports_security(&self) -> bool {
        self.raw & endpoint_flag::ALL_SECURE != 0
    }

    /// `true` if the peer has set all standard bits (the expectation
    /// for a spec-conformant DDSI-2.5 stack without security).
    #[must_use]
    pub fn fully_standard(&self) -> bool {
        self.raw & endpoint_flag::ALL_STANDARD == endpoint_flag::ALL_STANDARD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_from_zero_bitmask_is_all_false() {
        let c = PeerCapabilities::from_bits(0);
        assert_eq!(c.raw, 0);
        assert!(!c.has_spdp);
        assert!(!c.has_sedp_publications);
        assert!(!c.has_sedp_subscriptions);
        assert!(!c.has_wlp);
        assert!(!c.has_topics_discovery);
        assert!(!c.supports_security());
        assert!(!c.fully_standard());
    }

    #[test]
    fn capabilities_full_standard_bundle() {
        let c = PeerCapabilities::from_bits(endpoint_flag::ALL_STANDARD);
        assert!(c.has_spdp);
        assert!(c.has_sedp_publications);
        assert!(c.has_sedp_subscriptions);
        assert!(c.has_wlp);
        // SEDP topics endpoints (bits 28/29) are optional per RTPS 2.5
        // §8.5.4.4 and not included in `ALL_STANDARD` — DCPSTopic
        // samples are derived synthetically from pub/sub.
        assert!(!c.has_topics_discovery);
        assert!(c.fully_standard());
        // No security bits.
        assert!(!c.supports_security());
        assert!(!c.has_secure_publications);
    }

    #[test]
    fn capabilities_topics_discovery_when_explicitly_added() {
        // Vendors that implement the native SEDP topics endpoints can mix
        // the bits into the mask — `has_topics_discovery` thus remains
        // correctly detectable.
        let mask = endpoint_flag::ALL_STANDARD
            | endpoint_flag::TOPICS_ANNOUNCER
            | endpoint_flag::TOPICS_DETECTOR;
        let c = PeerCapabilities::from_bits(mask);
        assert!(c.has_topics_discovery);
    }

    #[test]
    fn capabilities_full_secure_bundle() {
        let c = PeerCapabilities::from_bits(endpoint_flag::ALL_SECURE);
        assert!(c.supports_security());
        assert!(c.has_secure_publications);
        assert!(c.has_secure_subscriptions);
        assert!(c.has_secure_wlp);
        assert!(c.has_stateless_auth);
        assert!(c.has_volatile_secure);
        assert!(c.has_secure_participant);
        // Standard bits are not in the bundle.
        assert!(!c.has_spdp);
        assert!(!c.has_wlp);
    }

    #[test]
    fn capabilities_partial_pair_does_not_count() {
        // Only PUBLICATIONS_ANNOUNCER (no DETECTOR) → has_sedp_publications=false.
        let c = PeerCapabilities::from_bits(endpoint_flag::PUBLICATIONS_ANNOUNCER);
        assert!(!c.has_sedp_publications);
    }

    #[test]
    fn capabilities_combined_standard_and_secure() {
        let mask = endpoint_flag::ALL_STANDARD | endpoint_flag::ALL_SECURE;
        let c = PeerCapabilities::from_bits(mask);
        assert!(c.fully_standard());
        assert!(c.supports_security());
        assert!(c.has_wlp);
        assert!(c.has_secure_wlp);
    }

    #[test]
    fn capabilities_legacy_peer_only_spdp_sedp() {
        // Legacy ZeroDDS and old Cyclone builds set only bits 0..5.
        let mask = endpoint_flag::PARTICIPANT_ANNOUNCER
            | endpoint_flag::PARTICIPANT_DETECTOR
            | endpoint_flag::PUBLICATIONS_ANNOUNCER
            | endpoint_flag::PUBLICATIONS_DETECTOR
            | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
            | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
        let c = PeerCapabilities::from_bits(mask);
        assert!(c.has_spdp);
        assert!(c.has_sedp_publications);
        assert!(c.has_sedp_subscriptions);
        assert!(!c.has_wlp);
        assert!(!c.has_topics_discovery);
        assert!(!c.supports_security());
        assert!(!c.fully_standard()); // WLP/Topics missing
    }

    #[test]
    fn capabilities_raw_is_preserved() {
        let mask = 0xDEAD_BEEF & 0x3FFF_FFFFu32; // valid 32-bit-ish
        let c = PeerCapabilities::from_bits(mask);
        assert_eq!(c.raw, mask);
    }
}
