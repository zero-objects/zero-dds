// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Participant security info wire format for
//! `PID_PARTICIPANT_SECURITY_INFO` (0x1005, DDS-Security 1.2 §7.5.1.4 Table 10).
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
    /// `is_rtps_axk_protected` (DDS-Security 1.2 §9.4.2.5 Table 34, bit 0):
    /// the RTPS messages are protected with the participant key exchanged
    /// after authentication.
    pub const IS_RTPS_AXK_PROTECTED: u32 = 0x0000_0001;
    /// `is_discovery_protected` — SEDP is encrypted.
    pub const IS_DISCOVERY_PROTECTED: u32 = 0x0000_0002;
    /// `is_liveliness_protected` — WLP heartbeats are protected.
    pub const IS_LIVELINESS_PROTECTED: u32 = 0x0000_0004;
    /// `is_key_revision_enabled` (DDS-Security 1.2 §9.4.2.5 Table 34, bit 3).
    pub const IS_KEY_REVISION_ENABLED: u32 = 0x0000_0008;
    /// `is_rtps_psk_protected` (Table 34, bit 4).
    pub const IS_RTPS_PSK_PROTECTED: u32 = 0x0000_0010;
}

/// Bits of `ParticipantSecurityAttributesMaskExt` (DDS-Security 1.2 §9.4.2.5
/// Table 35; they apply to `is_set` and to `value`).
pub mod opt_attrs {
    /// `allow_unauthenticated_participants`.
    pub const ALLOW_UNAUTHENTICATED_PARTICIPANTS: u16 = 0x0001;
    /// `is_access_protected`.
    pub const IS_ACCESS_PROTECTED: u16 = 0x0002;
}

/// Bit masks for `plugin_participant_security_attributes` (Spec §7.4.1.6
/// Tab.19).
pub mod plugin_attrs {
    /// MSB — the receiver must check.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// `is_rtps_axk_encrypted` (DDS-Security 1.2 §10.4.2.4 Table 67, bit 0):
    /// the RTPS messages protected with the exchanged participant key are
    /// GCM-encrypted (otherwise only GMAC).
    pub const IS_RTPS_AXK_ENCRYPTED: u32 = 0x0000_0001;
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
    /// `is_rtps_psk_encrypted` (DDS-Security 1.2 §10.4.2.4 Table 67, bit 6).
    pub const IS_RTPS_PSK_ENCRYPTED: u32 = 0x0000_0040;
}

/// `ParticipantSecurityAttributesMaskExt` (DDS-Security 1.2 §7.3.23): `is_set`
/// says which bits of `value` are set at all; configuration "that does not
/// need to be set consistently for two Participants to authenticate", so it
/// is not part of the compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityAttributesMaskExt {
    /// Which bits of [`Self::value`] carry a setting.
    pub is_set: u16,
    /// The settings (see [`opt_attrs`]).
    pub value: u16,
}

/// Wire representation of `PID_PARTICIPANT_SECURITY_INFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityInfo {
    /// `participant_security_attributes` (Spec §7.4.1.6 Tab.18).
    pub participant_security_attributes: u32,
    /// `plugin_participant_security_attributes` (Spec §7.4.1.6 Tab.19).
    pub plugin_participant_security_attributes: u32,
    /// `participant_security_optional_attributes` (§7.3.23): sent only when
    /// `is_set` is not 0 (the member is absent otherwise, the value of a
    /// peer that does not know it).
    pub optional_attributes: ParticipantSecurityAttributesMaskExt,
}

impl ParticipantSecurityInfo {
    /// Spec wire size (LE) of the two masks; the third member adds 4.
    pub const WIRE_SIZE: usize = 8;

    /// The value of `PID_PARTICIPANT_SECURITY_INFO`, LE: the two masks, and
    /// `participant_security_optional_attributes` (`is_set`, `value`) when
    /// `is_set` is not 0.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_wire_le(&self) -> alloc::vec::Vec<u8> {
        let mut out = self.to_le_bytes().to_vec();
        if self.optional_attributes.is_set != 0 {
            out.extend_from_slice(&self.optional_attributes.is_set.to_le_bytes());
            out.extend_from_slice(&self.optional_attributes.value.to_le_bytes());
        }
        out
    }

    /// As [`Self::to_wire_le`], big endian.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_wire_be(&self) -> alloc::vec::Vec<u8> {
        let mut out = self.to_be_bytes().to_vec();
        if self.optional_attributes.is_set != 0 {
            out.extend_from_slice(&self.optional_attributes.is_set.to_be_bytes());
            out.extend_from_slice(&self.optional_attributes.value.to_be_bytes());
        }
        out
    }

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
        let optional_attributes = match bytes.get(8..12) {
            Some(ext) => ParticipantSecurityAttributesMaskExt {
                is_set: u16::from_le_bytes([ext[0], ext[1]]),
                value: u16::from_le_bytes([ext[2], ext[3]]),
            },
            None => ParticipantSecurityAttributesMaskExt::default(),
        };
        Ok(Self {
            participant_security_attributes: u32::from_le_bytes(a),
            plugin_participant_security_attributes: u32::from_le_bytes(b),
            optional_attributes,
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
        let optional_attributes = match bytes.get(8..12) {
            Some(ext) => ParticipantSecurityAttributesMaskExt {
                is_set: u16::from_be_bytes([ext[0], ext[1]]),
                value: u16::from_be_bytes([ext[2], ext[3]]),
            },
            None => ParticipantSecurityAttributesMaskExt::default(),
        };
        Ok(Self {
            participant_security_attributes: u32::from_be_bytes(a),
            plugin_participant_security_attributes: u32::from_be_bytes(b),
            optional_attributes,
        })
    }

    /// Receiver-side check: is the info valid at all (`IS_VALID`
    /// bit set in both masks)?
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (self.participant_security_attributes & attrs::IS_VALID) != 0
            && (self.plugin_participant_security_attributes & plugin_attrs::IS_VALID) != 0
    }

    /// Whether two participants have a compatible configuration (DDS-Security
    /// 1.2 §7.3.23): the same value for both masks, except that a mask whose
    /// `IS_VALID` bit is zero on either side is not compared. Participants
    /// that are not compatible "shall not attempt authentication".
    #[must_use]
    pub fn is_compatible_with(&self, other: Option<&Self>) -> bool {
        let Some(other) = other else {
            return true;
        };
        crate::endpoint_security_info::masks_compatible(
            self.participant_security_attributes,
            other.participant_security_attributes,
            attrs::IS_VALID,
        ) && crate::endpoint_security_info::masks_compatible(
            self.plugin_participant_security_attributes,
            other.plugin_participant_security_attributes,
            plugin_attrs::IS_VALID,
        )
    }

    /// Spec §7.4.1.6: convenience builder for "all RTPS-protected,
    /// discovery-protected, liveliness-protected" — the most common
    /// default for production.
    #[must_use]
    pub fn fully_protected_default() -> Self {
        Self {
            participant_security_attributes: attrs::IS_VALID
                | attrs::IS_RTPS_AXK_PROTECTED
                | attrs::IS_DISCOVERY_PROTECTED
                | attrs::IS_LIVELINESS_PROTECTED,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_RTPS_AXK_ENCRYPTED
                | plugin_attrs::IS_DISCOVERY_ENCRYPTED
                | plugin_attrs::IS_LIVELINESS_ENCRYPTED,
            ..Default::default()
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// DDS-Security 1.2 §7.3.23, §9.4.2.5 Tables 34 and 35: the third member
    /// `participant_security_optional_attributes` (`is_set`, `value`) is read
    /// when it is there (12 octets), written only when set, and not part of
    /// the compatibility.
    #[test]
    fn the_optional_attributes_are_the_third_member() {
        assert_eq!(attrs::IS_KEY_REVISION_ENABLED, 1 << 3);
        assert_eq!(attrs::IS_RTPS_PSK_PROTECTED, 1 << 4);
        assert_eq!(plugin_attrs::IS_RTPS_PSK_ENCRYPTED, 1 << 6);
        assert_eq!(opt_attrs::ALLOW_UNAUTHENTICATED_PARTICIPANTS, 1);
        assert_eq!(opt_attrs::IS_ACCESS_PROTECTED, 2);
        let mut info = ParticipantSecurityInfo::fully_protected_default();
        assert_eq!(info.to_wire_le().len(), 8, "absent unless set");
        info.optional_attributes = ParticipantSecurityAttributesMaskExt {
            is_set: opt_attrs::ALLOW_UNAUTHENTICATED_PARTICIPANTS | opt_attrs::IS_ACCESS_PROTECTED,
            value: opt_attrs::IS_ACCESS_PROTECTED,
        };
        let le = info.to_wire_le();
        assert_eq!(le.len(), 12);
        assert_eq!(&le[8..], &[3, 0, 2, 0]);
        assert_eq!(ParticipantSecurityInfo::from_le_bytes(&le).unwrap(), info);
        let be = info.to_wire_be();
        assert_eq!(&be[8..], &[0, 3, 0, 2]);
        assert_eq!(ParticipantSecurityInfo::from_be_bytes(&be).unwrap(), info);
        // 8 octets: the member is absent.
        let eight = ParticipantSecurityInfo::from_le_bytes(&le[..8]).unwrap();
        assert_eq!(
            eight.optional_attributes,
            ParticipantSecurityAttributesMaskExt::default()
        );
        // Not part of the compatibility.
        let mut other = info;
        other.optional_attributes = ParticipantSecurityAttributesMaskExt::default();
        assert!(info.is_compatible_with(Some(&other)));
    }

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

    /// DDS-Security 1.2 §7.3.23: compatible means the same value of both
    /// masks; a mask without `IS_VALID` on either side is not compared; the
    /// default `{0, 0}` is never compared.
    #[test]
    fn compatibility_is_equality_of_the_valid_masks() {
        let a = ParticipantSecurityInfo::fully_protected_default();
        assert!(a.is_compatible_with(Some(&a)));
        let mut rtps_off = a;
        rtps_off.participant_security_attributes &= !attrs::IS_RTPS_AXK_PROTECTED;
        assert!(!a.is_compatible_with(Some(&rtps_off)));
        let mut plugin_differs = a;
        plugin_differs.plugin_participant_security_attributes &=
            !plugin_attrs::IS_RTPS_AXK_ENCRYPTED;
        assert!(!a.is_compatible_with(Some(&plugin_differs)));
        // The default (both masks without IS_VALID) and a missing one: ignored.
        assert!(a.is_compatible_with(Some(&ParticipantSecurityInfo::default())));
        assert!(a.is_compatible_with(None));
        // Only the participant mask valid on the remote: the plugin mask is
        // not compared, the participant mask is.
        let only_standard = ParticipantSecurityInfo {
            participant_security_attributes: rtps_off.participant_security_attributes,
            plugin_participant_security_attributes: 0,
            ..Default::default()
        };
        assert!(!a.is_compatible_with(Some(&only_standard)));
        let only_plugin_differs = ParticipantSecurityInfo {
            participant_security_attributes: 0,
            plugin_participant_security_attributes: plugin_differs
                .plugin_participant_security_attributes,
            ..Default::default()
        };
        assert!(!a.is_compatible_with(Some(&only_plugin_differs)));
    }

    #[test]
    fn is_valid_requires_both_masks() {
        let info = ParticipantSecurityInfo {
            participant_security_attributes: attrs::IS_VALID,
            plugin_participant_security_attributes: 0,
            ..Default::default()
        };
        assert!(!info.is_valid(), "second mask not valid");
        let info = ParticipantSecurityInfo {
            participant_security_attributes: 0,
            plugin_participant_security_attributes: plugin_attrs::IS_VALID,
            ..Default::default()
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
            info.participant_security_attributes & attrs::IS_RTPS_AXK_PROTECTED,
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
            info.plugin_participant_security_attributes & plugin_attrs::IS_RTPS_AXK_ENCRYPTED,
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
