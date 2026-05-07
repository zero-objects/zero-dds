// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Participant-Security-Info Wire-Format fuer
//! `PID_PARTICIPANT_SECURITY_INFO` (0x1005, DDS-Security 1.2 §7.4.1.6).
//!
//! Zwei u32-Bitmasken auf Participant-Ebene:
//! ```text
//!   u32 participant_security_attributes
//!   u32 plugin_participant_security_attributes
//! ```
//!
//! MSB ist `IS_VALID`-Flag — wie bei [`crate::endpoint_security_info`].
//! Dieses Modul liefert nur den Wire-Codec; Policy-Bindings sind im
//! `security-runtime`-Crate.

use crate::error::WireError;

/// Bit-Masks fuer `participant_security_attributes` (Spec §7.4.1.6
/// Tab.18).
pub mod attrs {
    /// MSB — Receiver muss pruefen.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// `is_rtps_protected` — RTPS-Submessage-Layer-Schutz aktiv.
    pub const IS_RTPS_PROTECTED: u32 = 0x0000_0001;
    /// `is_discovery_protected` — SEDP wird verschluesselt.
    pub const IS_DISCOVERY_PROTECTED: u32 = 0x0000_0002;
    /// `is_liveliness_protected` — WLP-Heartbeats werden geschuetzt.
    pub const IS_LIVELINESS_PROTECTED: u32 = 0x0000_0004;
}

/// Bit-Masks fuer `plugin_participant_security_attributes` (Spec §7.4.1.6
/// Tab.19).
pub mod plugin_attrs {
    /// MSB — Receiver muss pruefen.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// `is_rtps_encrypted` — RTPS-Submessages sind GCM-verschluesselt
    /// (sonst nur GMAC).
    pub const IS_RTPS_ENCRYPTED: u32 = 0x0000_0001;
    /// `is_discovery_encrypted`.
    pub const IS_DISCOVERY_ENCRYPTED: u32 = 0x0000_0002;
    /// `is_liveliness_encrypted`.
    pub const IS_LIVELINESS_ENCRYPTED: u32 = 0x0000_0004;
    /// `is_rtps_origin_authenticated` — Receiver-Specific-MAC fuer
    /// Sender-Auth (Spec §7.4.1.6).
    pub const IS_RTPS_ORIGIN_AUTHENTICATED: u32 = 0x0000_0008;
    /// `is_discovery_origin_authenticated`.
    pub const IS_DISCOVERY_ORIGIN_AUTHENTICATED: u32 = 0x0000_0010;
    /// `is_liveliness_origin_authenticated`.
    pub const IS_LIVELINESS_ORIGIN_AUTHENTICATED: u32 = 0x0000_0020;
}

/// Wire-Repraesentation von `PID_PARTICIPANT_SECURITY_INFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityInfo {
    /// `participant_security_attributes` (Spec §7.4.1.6 Tab.18).
    pub participant_security_attributes: u32,
    /// `plugin_participant_security_attributes` (Spec §7.4.1.6 Tab.19).
    pub plugin_participant_security_attributes: u32,
}

impl ParticipantSecurityInfo {
    /// Spec-Wire-Size (LE).
    pub const WIRE_SIZE: usize = 8;

    /// Encode zu PL_CDR-Value-Bytes (8 Bytes LE).
    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&self.participant_security_attributes.to_le_bytes());
        out[4..8].copy_from_slice(&self.plugin_participant_security_attributes.to_le_bytes());
        out
    }

    /// Decode aus 8 Byte (PL_CDR-Value).
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` bei zu kurzem Buffer.
    pub fn from_le_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE - bytes.len(),
                offset: 0,
            });
        }
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[0..4]);
        let mut b = [0u8; 4];
        b.copy_from_slice(&bytes[4..8]);
        Ok(Self {
            participant_security_attributes: u32::from_le_bytes(a),
            plugin_participant_security_attributes: u32::from_le_bytes(b),
        })
    }

    /// Receiver-Side-Check: ist die Info ueberhaupt gueltig (`IS_VALID`-
    /// Bit gesetzt in beiden Masken)?
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (self.participant_security_attributes & attrs::IS_VALID) != 0
            && (self.plugin_participant_security_attributes & plugin_attrs::IS_VALID) != 0
    }

    /// Spec §7.4.1.6: Convenience-Builder fuer "alles RTPS-protected,
    /// Discovery-protected, Liveliness-protected" — der haeufigste
    /// Default fuer Production.
    #[must_use]
    pub fn fully_protected_default() -> Self {
        Self {
            participant_security_attributes: attrs::IS_VALID
                | attrs::IS_RTPS_PROTECTED
                | attrs::IS_DISCOVERY_PROTECTED
                | attrs::IS_LIVELINESS_PROTECTED,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_RTPS_ENCRYPTED
                | plugin_attrs::IS_DISCOVERY_ENCRYPTED
                | plugin_attrs::IS_LIVELINESS_ENCRYPTED,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default() {
        let info = ParticipantSecurityInfo::default();
        let bytes = info.to_le_bytes();
        let back = ParticipantSecurityInfo::from_le_bytes(&bytes).unwrap();
        assert_eq!(back, info);
    }

    #[test]
    fn round_trip_fully_protected() {
        let info = ParticipantSecurityInfo::fully_protected_default();
        let bytes = info.to_le_bytes();
        let back = ParticipantSecurityInfo::from_le_bytes(&bytes).unwrap();
        assert_eq!(back, info);
        assert!(back.is_valid());
    }

    #[test]
    fn is_valid_requires_both_masks() {
        let info = ParticipantSecurityInfo {
            participant_security_attributes: attrs::IS_VALID,
            plugin_participant_security_attributes: 0,
        };
        assert!(!info.is_valid(), "second mask not valid");
        let info = ParticipantSecurityInfo {
            participant_security_attributes: 0,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID,
        };
        assert!(!info.is_valid(), "first mask not valid");
    }

    #[test]
    fn short_buffer_rejected() {
        assert!(matches!(
            ParticipantSecurityInfo::from_le_bytes(&[0; 4]),
            Err(WireError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn wire_size_is_8() {
        assert_eq!(ParticipantSecurityInfo::WIRE_SIZE, 8);
    }

    #[test]
    fn fully_protected_sets_all_protection_bits() {
        let info = ParticipantSecurityInfo::fully_protected_default();
        assert_ne!(
            info.participant_security_attributes & attrs::IS_RTPS_PROTECTED,
            0
        );
        assert_ne!(
            info.participant_security_attributes & attrs::IS_DISCOVERY_PROTECTED,
            0
        );
        assert_ne!(
            info.participant_security_attributes & attrs::IS_LIVELINESS_PROTECTED,
            0
        );
    }

    #[test]
    fn fully_protected_sets_all_encrypt_bits() {
        let info = ParticipantSecurityInfo::fully_protected_default();
        assert_ne!(
            info.plugin_participant_security_attributes & plugin_attrs::IS_RTPS_ENCRYPTED,
            0
        );
        assert_ne!(
            info.plugin_participant_security_attributes & plugin_attrs::IS_DISCOVERY_ENCRYPTED,
            0
        );
    }

    #[test]
    fn empty_default_is_not_valid() {
        let info = ParticipantSecurityInfo::default();
        assert!(!info.is_valid());
    }
}
