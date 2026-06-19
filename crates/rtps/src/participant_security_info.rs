// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Participant security info wire format for
//! `PID_PARTICIPANT_SECURITY_INFO` (0x1005, DDS-Security 1.2 §7.4.1.6).
//!
//! Two u32 bitmasks at participant level:
//! ```text
//!   u32 participant_security_attributes
//!   u32 plugin_participant_security_attributes
//! ```
//!
//! The MSB is the `IS_VALID` flag — like in [`crate::endpoint_security_info`].
//! This module provides only the wire codec; policy bindings are in the
//! `security-runtime` crate.

use crate::error::WireError;

/// Bit masks for `participant_security_attributes` (Spec §7.4.1.6
/// Tab.18).
pub mod attrs {
    /// MSB — the receiver must check.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// `is_rtps_protected` — RTPS submessage-layer protection active.
    pub const IS_RTPS_PROTECTED: u32 = 0x0000_0001;
    /// `is_discovery_protected` — SEDP is encrypted.
    pub const IS_DISCOVERY_PROTECTED: u32 = 0x0000_0002;
    /// `is_liveliness_protected` — WLP heartbeats are protected.
    pub const IS_LIVELINESS_PROTECTED: u32 = 0x0000_0004;
}

/// Bit masks for `plugin_participant_security_attributes` (Spec §7.4.1.6
/// Tab.19).
pub mod plugin_attrs {
    /// MSB — the receiver must check.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// `is_rtps_encrypted` — RTPS submessages are GCM-encrypted
    /// (otherwise only GMAC).
    pub const IS_RTPS_ENCRYPTED: u32 = 0x0000_0001;
    /// `is_discovery_encrypted`.
    pub const IS_DISCOVERY_ENCRYPTED: u32 = 0x0000_0002;
    /// `is_liveliness_encrypted`.
    pub const IS_LIVELINESS_ENCRYPTED: u32 = 0x0000_0004;
    /// `is_rtps_origin_authenticated` — receiver-specific MAC for
    /// sender auth (Spec §7.4.1.6).
    pub const IS_RTPS_ORIGIN_AUTHENTICATED: u32 = 0x0000_0008;
    /// `is_discovery_origin_authenticated`.
    pub const IS_DISCOVERY_ORIGIN_AUTHENTICATED: u32 = 0x0000_0010;
    /// `is_liveliness_origin_authenticated`.
    pub const IS_LIVELINESS_ORIGIN_AUTHENTICATED: u32 = 0x0000_0020;
}

/// Wire representation of `PID_PARTICIPANT_SECURITY_INFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityInfo {
    /// `participant_security_attributes` (Spec §7.4.1.6 Tab.18).
    pub participant_security_attributes: u32,
    /// `plugin_participant_security_attributes` (Spec §7.4.1.6 Tab.19).
    pub plugin_participant_security_attributes: u32,
}

impl ParticipantSecurityInfo {
    /// Spec wire size (LE).
    pub const WIRE_SIZE: usize = 8;

    /// Encode to PL_CDR value bytes (8 bytes LE).
    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&self.participant_security_attributes.to_le_bytes());
        out[4..8].copy_from_slice(&self.plugin_participant_security_attributes.to_le_bytes());
        out
    }

    /// Encode to PL_CDR value bytes (8 bytes BE) — for PL_CDR_BE payloads
    /// like the handshake `c.pdata`.
    #[must_use]
    pub fn to_be_bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&self.participant_security_attributes.to_be_bytes());
        out[4..8].copy_from_slice(&self.plugin_participant_security_attributes.to_be_bytes());
        out
    }

    /// Decode from 8 bytes (PL_CDR value).
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` on a too-short buffer.
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

    /// Decode from 8 bytes BE (PL_CDR_BE value, e.g. handshake `c.pdata`).
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` on a too-short buffer.
    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, WireError> {
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
            participant_security_attributes: u32::from_be_bytes(a),
            plugin_participant_security_attributes: u32::from_be_bytes(b),
        })
    }

    /// Receiver-side check: is the info valid at all (`IS_VALID`
    /// bit set in both masks)?
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (self.participant_security_attributes & attrs::IS_VALID) != 0
            && (self.plugin_participant_security_attributes & plugin_attrs::IS_VALID) != 0
    }

    /// Spec §7.4.1.6: convenience builder for "all RTPS-protected,
    /// discovery-protected, liveliness-protected" — the most common
    /// default for production.
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
