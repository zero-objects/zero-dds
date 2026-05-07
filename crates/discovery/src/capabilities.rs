// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Peer-Capabilities aus dem `BuiltinEndpointSet`-Bitfield.
//!
//! Klassifiziert eine remote `ParticipantBuiltinTopicData::builtin_
//! endpoint_set` u32-Maske in eine high-level Capability-Struct, die
//! der Caller direkt fuers Matching nutzen kann. Zusaetzlich gibt es
//! Boolean-Helpers, um pro-Bit-Prufungen lesbar zu halten.
//!
//! # Spec-Referenzen
//!
//! - DDSI-RTPS 2.5 §9.3.2.12 (Standard-Bits 0..5, 10..11, 28..29)
//! - DDS-Security 1.2 §7.4.7.1 (Secure-Discovery-Bits 16..27)

extern crate alloc;
use zerodds_rtps::participant_data::endpoint_flag;

/// High-level Klassifikation eines Peer-`BuiltinEndpointSet`s.
/// Wird vom DCPS-Runtime aus dem SPDP-Beacon eines Peers errechnet
/// und an die SEDP-/WLP-/Security-Match-Logic weitergegeben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerCapabilities {
    /// Roh-Bitmaske, wie sie vom Peer announced wurde — fuer
    /// Audit-/Diagnose-Pfade.
    pub raw: u32,
    /// SPDP-Endpoints (Bits 0/1).
    pub has_spdp: bool,
    /// SEDP-Publications-Endpoints (Bits 2/3).
    pub has_sedp_publications: bool,
    /// SEDP-Subscriptions-Endpoints (Bits 4/5).
    pub has_sedp_subscriptions: bool,
    /// Writer-Liveliness-Protocol (Bits 10/11).
    pub has_wlp: bool,
    /// TypeLookup-Service (Bits 12/13, XTypes 1.3 §7.6.3.3.4).
    pub has_type_lookup: bool,
    /// XTypes-Topics-Discovery (Bits 28/29).
    pub has_topics_discovery: bool,
    /// Secure-Publications-Endpoints (Bits 16/17).
    pub has_secure_publications: bool,
    /// Secure-Subscriptions-Endpoints (Bits 18/19).
    pub has_secure_subscriptions: bool,
    /// Secure-WLP-Endpoints (Bits 20/21).
    pub has_secure_wlp: bool,
    /// Auth-Stateless-Endpoints (Bits 22/23).
    pub has_stateless_auth: bool,
    /// Crypto-KeyExchange-Endpoints (Bits 24/25).
    pub has_volatile_secure: bool,
    /// Secure-Participant-Discovery (Bits 26/27).
    pub has_secure_participant: bool,
}

impl PeerCapabilities {
    /// Klassifiziert eine Peer-Bitmaske.
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

    /// `true` wenn der Peer mindestens ein Secure-Discovery-Bit
    /// gesetzt hat (Sub-Bits 16..27). Wird vom Security-Pfad genutzt,
    /// um zu entscheiden, ob ein Secure-Handshake versucht werden
    /// kann oder ob der Peer Plain-Discovery erwartet.
    #[must_use]
    pub fn supports_security(&self) -> bool {
        self.raw & endpoint_flag::ALL_SECURE != 0
    }

    /// `true` wenn der Peer alle Standard-Bits gesetzt hat
    /// (Erwartung an einen Spec-konformen DDSI-2.5-Stack ohne
    /// Security).
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
        // SEDP-Topics-Endpoints (Bits 28/29) sind per RTPS 2.5 §8.5.4.4
        // optional und nicht in `ALL_STANDARD` enthalten — DCPSTopic-
        // Samples werden synthetisch aus Pub/Sub abgeleitet.
        assert!(!c.has_topics_discovery);
        assert!(c.fully_standard());
        // Keine Security-Bits.
        assert!(!c.supports_security());
        assert!(!c.has_secure_publications);
    }

    #[test]
    fn capabilities_topics_discovery_when_explicitly_added() {
        // Vendors, die die nativen SEDP-Topics-Endpoints implementieren,
        // koennen die Bits zur Maske hinzumixen — `has_topics_discovery`
        // bleibt also korrekt detektierbar.
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
        // Standard-Bits sind nicht im Bundle.
        assert!(!c.has_spdp);
        assert!(!c.has_wlp);
    }

    #[test]
    fn capabilities_partial_pair_does_not_count() {
        // Nur PUBLICATIONS_ANNOUNCER (kein DETECTOR) → has_sedp_publications=false.
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
        // Legacy-ZeroDDS und alte Cyclone-Builds setzen nur Bits 0..5.
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
        assert!(!c.fully_standard()); // WLP/Topics fehlen
    }

    #[test]
    fn capabilities_raw_is_preserved() {
        let mask = 0xDEAD_BEEF & 0x3FFF_FFFFu32; // valid 32-bit-ish
        let c = PeerCapabilities::from_bits(mask);
        assert_eq!(c.raw, mask);
    }
}
