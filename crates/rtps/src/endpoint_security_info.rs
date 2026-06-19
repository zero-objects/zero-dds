// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Endpoint security info wire format for `PID_ENDPOINT_SECURITY_INFO`
//! (0x1004, DDS-Security 1.1 §7.4.1.5).
//!
//! Two u32 bitmasks:
//! ```text
//!   u32  endpoint_security_attributes          (masks §7.4.1.5)
//!   u32  plugin_endpoint_security_attributes   (masks §7.4.1.6)
//! ```
//!
//! Both masks use the MSB as the `IS_VALID` flag: a
//! receiver may interpret the values only if the bit is set
//! (otherwise → treat as "no security info").
//!
//! This module is **wire-only** — policy decisions (matching
//! writer ↔ reader, encryption-level enforcement) belong in the
//! `security-runtime` crate.

use crate::error::WireError;

// ============================================================================
// EndpointSecurityAttributes (DDS-Security 1.1 §7.4.1.5 Table 32)
// ============================================================================

/// Bit masks for `endpoint_security_attributes`.
pub mod attrs {
    /// Set if the bits may be interpreted at all.
    /// The receiver must check this bit, otherwise ignore the values.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// Read access control is active for this endpoint.
    pub const IS_READ_PROTECTED: u32 = 0x0000_0001;
    /// Write access control is active.
    pub const IS_WRITE_PROTECTED: u32 = 0x0000_0002;
    /// SEDP discovery for this endpoint is protected.
    pub const IS_DISCOVERY_PROTECTED: u32 = 0x0000_0004;
    /// Submessage-level protection (SEC_PREFIX wrapping) active.
    pub const IS_SUBMESSAGE_PROTECTED: u32 = 0x0000_0008;
    /// Payload-level protection (SEC_BODY encoding) active.
    pub const IS_PAYLOAD_PROTECTED: u32 = 0x0000_0010;
    /// Key attributes (e.g. partition keys) are protected.
    pub const IS_KEY_PROTECTED: u32 = 0x0000_0020;
    /// Liveliness submessages are authenticated.
    pub const IS_LIVELINESS_PROTECTED: u32 = 0x0000_0040;
}

// ============================================================================
// PluginEndpointSecurityAttributes (§7.4.1.6 Table 33)
// ============================================================================

/// Bit masks for `plugin_endpoint_security_attributes`.
///
/// This second mask is plugin-specific: it says *what concretely*
/// happens in terms of protection, while [`attrs`] says *which class* of
/// protection is active.
pub mod plugin_attrs {
    /// Set if the plugin bits are set.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// Submessage is AEAD-encrypted (not just signed). Bit 0.
    pub const IS_SUBMESSAGE_ENCRYPTED: u32 = 0x0000_0001;
    /// Payload (SEC_BODY) is encrypted (otherwise only authenticated).
    /// Bit 1 — DDS-Security §9.4.2.5 (previously wrongly 0x4; cyclone/OpenDDS
    /// read `payload_encrypted` at bit 1 → cross-vendor endpoint mismatch).
    pub const IS_PAYLOAD_ENCRYPTED: u32 = 0x0000_0002;
    /// Submessage carries an origin-authentication tag (receiver-specific).
    /// Bit 2 — DDS-Security §9.4.2.5 (previously wrongly 0x2).
    pub const IS_SUBMESSAGE_ORIGIN_AUTHENTICATED: u32 = 0x0000_0004;
}

// ============================================================================
// EndpointSecurityInfo-Struct
// ============================================================================

/// Wire representation of `PID_ENDPOINT_SECURITY_INFO`.
///
/// The raw masks are passed through unmodified — the
/// policy-layer conversion (e.g. "is_submessage_protected means
/// ProtectionLevel::Sign/Encrypt") lives in the `security-runtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EndpointSecurityInfo {
    /// Standard endpoint security attributes (see [`attrs`]).
    pub endpoint_security_attributes: u32,
    /// Plugin-specific endpoint security attributes (see
    /// [`plugin_attrs`]).
    pub plugin_endpoint_security_attributes: u32,
}

impl EndpointSecurityInfo {
    /// Wire-Size (2 * u32).
    pub const WIRE_SIZE: usize = 8;

    /// `true` if the spec-conformant `IS_VALID` bit is set in both masks.
    /// Otherwise the receiver should ignore the values
    /// (§7.4.1.5 sentence 2).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_VALID) != 0
            && (self.plugin_endpoint_security_attributes & plugin_attrs::IS_VALID) != 0
    }

    /// Builder for a "plain legacy" endpoint (all bits 0 except
    /// the IS_VALID flags) — corresponds to: the peer supports the
    /// security PID, but wants no protection for this endpoint.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            endpoint_security_attributes: attrs::IS_VALID,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID,
        }
    }

    /// `true` if submessage-level protection is set.
    #[must_use]
    pub const fn is_submessage_protected(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_SUBMESSAGE_PROTECTED) != 0
    }

    /// `true` if payload-level protection is set.
    #[must_use]
    pub const fn is_payload_protected(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_PAYLOAD_PROTECTED) != 0
    }

    /// `true` if the plugin announces AEAD encryption for submessages.
    #[must_use]
    pub const fn is_submessage_encrypted(&self) -> bool {
        (self.plugin_endpoint_security_attributes & plugin_attrs::IS_SUBMESSAGE_ENCRYPTED) != 0
    }

    /// `true` if the plugin reports an origin-authentication tag (stage 7
    /// receiver-specific MACs).
    #[must_use]
    pub const fn is_submessage_origin_authenticated(&self) -> bool {
        (self.plugin_endpoint_security_attributes
            & plugin_attrs::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED)
            != 0
    }

    /// `true` if the plugin reports payload ciphertext.
    #[must_use]
    pub const fn is_payload_encrypted(&self) -> bool {
        (self.plugin_endpoint_security_attributes & plugin_attrs::IS_PAYLOAD_ENCRYPTED) != 0
    }

    /// Encode to 8 bytes (2 * u32 LE or BE).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; Self::WIRE_SIZE] {
        let mut out = [0u8; Self::WIRE_SIZE];
        let (a, p) = if little_endian {
            (
                self.endpoint_security_attributes.to_le_bytes(),
                self.plugin_endpoint_security_attributes.to_le_bytes(),
            )
        } else {
            (
                self.endpoint_security_attributes.to_be_bytes(),
                self.plugin_endpoint_security_attributes.to_be_bytes(),
            )
        };
        out[..4].copy_from_slice(&a);
        out[4..].copy_from_slice(&p);
        out
    }

    /// Decode from 8 bytes (the value of a `PID_ENDPOINT_SECURITY_INFO`
    /// parameter).
    ///
    /// # Errors
    /// `UnexpectedEof` if the input is < 8 bytes.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[..4]);
        let mut p = [0u8; 4];
        p.copy_from_slice(&bytes[4..8]);
        let (attrs_raw, plugin_raw) = if little_endian {
            (u32::from_le_bytes(a), u32::from_le_bytes(p))
        } else {
            (u32::from_be_bytes(a), u32::from_be_bytes(p))
        };
        Ok(Self {
            endpoint_security_attributes: attrs_raw,
            plugin_endpoint_security_attributes: plugin_raw,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn plain_is_valid_with_no_protection_bits() {
        let p = EndpointSecurityInfo::plain();
        assert!(p.is_valid());
        assert!(!p.is_submessage_protected());
        assert!(!p.is_payload_protected());
        assert!(!p.is_submessage_encrypted());
        assert!(!p.is_payload_encrypted());
        assert!(!p.is_submessage_origin_authenticated());
    }

    #[test]
    fn default_is_not_valid() {
        let d = EndpointSecurityInfo::default();
        assert!(
            !d.is_valid(),
            "default == all zero == is_valid must be false"
        );
    }

    #[test]
    fn roundtrip_le() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID
                | attrs::IS_SUBMESSAGE_PROTECTED
                | attrs::IS_PAYLOAD_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_SUBMESSAGE_ENCRYPTED
                | plugin_attrs::IS_PAYLOAD_ENCRYPTED,
        };
        let bytes = info.to_bytes(true);
        let decoded = EndpointSecurityInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn roundtrip_be() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_SUBMESSAGE_ENCRYPTED,
        };
        let bytes = info.to_bytes(false);
        let decoded = EndpointSecurityInfo::from_bytes(&bytes, false).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn wire_size_is_eight_bytes() {
        let info = EndpointSecurityInfo::plain();
        assert_eq!(info.to_bytes(true).len(), 8);
    }

    #[test]
    fn decode_rejects_short_input() {
        let err = EndpointSecurityInfo::from_bytes(&[0u8; 7], true).unwrap_err();
        assert!(matches!(err, WireError::UnexpectedEof { .. }));
    }

    #[test]
    fn encoded_bytes_le_match_spec_layout() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: 0x8000_0008,
            plugin_endpoint_security_attributes: 0x8000_0001,
        };
        let bytes = info.to_bytes(true);
        // attrs LE: 08 00 00 80; plugin LE: 01 00 00 80
        assert_eq!(bytes, [0x08, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x80]);
    }

    #[test]
    fn protection_bit_accessors_read_correctly() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_SUBMESSAGE_ENCRYPTED
                | plugin_attrs::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
        };
        assert!(info.is_submessage_protected());
        assert!(!info.is_payload_protected());
        assert!(info.is_submessage_encrypted());
        assert!(info.is_submessage_origin_authenticated());
        assert!(!info.is_payload_encrypted());
    }

    #[test]
    fn is_valid_requires_both_masks() {
        let only_attrs = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID,
            plugin_endpoint_security_attributes: 0,
        };
        assert!(!only_attrs.is_valid());

        let only_plugin = EndpointSecurityInfo {
            endpoint_security_attributes: 0,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID,
        };
        assert!(!only_plugin.is_valid());

        let both = EndpointSecurityInfo::plain();
        assert!(both.is_valid());
    }
}
