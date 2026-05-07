// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Endpoint-Security-Info Wire-Format fuer `PID_ENDPOINT_SECURITY_INFO`
//! (0x1004, DDS-Security 1.1 §7.4.1.5).
//!
//! Zwei u32-Bitmasken:
//! ```text
//!   u32  endpoint_security_attributes          (masks §7.4.1.5)
//!   u32  plugin_endpoint_security_attributes   (masks §7.4.1.6)
//! ```
//!
//! Beide Masken verwenden das MSB als `IS_VALID`-Flag: ein
//! Receiver darf die Werte nur interpretieren, wenn das Bit gesetzt
//! ist (sonst → als "no security info" behandeln).
//!
//! Dieses Modul ist **Wire-only** — Policy-Entscheidungen (Match
//! Writer ↔ Reader, Encryption-Level-Enforcement) gehoeren in den
//! `security-runtime`-Crate.

use crate::error::WireError;

// ============================================================================
// EndpointSecurityAttributes (DDS-Security 1.1 §7.4.1.5 Tabelle 32)
// ============================================================================

/// Bit-Masks fuer `endpoint_security_attributes`.
pub mod attrs {
    /// Set wenn die Bits ueberhaupt interpretiert werden duerfen.
    /// Receiver muss diesen Bit pruefen, sonst Werte ignorieren.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// Read-Access-Control ist fuer dieses Endpoint aktiv.
    pub const IS_READ_PROTECTED: u32 = 0x0000_0001;
    /// Write-Access-Control ist aktiv.
    pub const IS_WRITE_PROTECTED: u32 = 0x0000_0002;
    /// SEDP-Discovery fuer dieses Endpoint wird geschuetzt.
    pub const IS_DISCOVERY_PROTECTED: u32 = 0x0000_0004;
    /// Submessage-Level-Protection (SEC_PREFIX wrapping) aktiv.
    pub const IS_SUBMESSAGE_PROTECTED: u32 = 0x0000_0008;
    /// Payload-Level-Protection (SEC_BODY-Encoding) aktiv.
    pub const IS_PAYLOAD_PROTECTED: u32 = 0x0000_0010;
    /// Key-Attribute (z.B. Partition-Keys) werden geschuetzt.
    pub const IS_KEY_PROTECTED: u32 = 0x0000_0020;
    /// Liveliness-Submessages werden authentifiziert.
    pub const IS_LIVELINESS_PROTECTED: u32 = 0x0000_0040;
}

// ============================================================================
// PluginEndpointSecurityAttributes (§7.4.1.6 Tabelle 33)
// ============================================================================

/// Bit-Masks fuer `plugin_endpoint_security_attributes`.
///
/// Diese zweite Maske ist Plugin-spezifisch: sie sagt *was konkret*
/// an Protection passiert, waehrend [`attrs`] sagt *welche Klasse* von
/// Schutz aktiv ist.
pub mod plugin_attrs {
    /// Set wenn die Plugin-Bits gesetzt sind.
    pub const IS_VALID: u32 = 0x8000_0000;
    /// Submessage ist AEAD-verschluesselt (nicht nur signiert).
    pub const IS_SUBMESSAGE_ENCRYPTED: u32 = 0x0000_0001;
    /// Submessage traegt Origin-Authentication-Tag (receiver-spezifisch).
    pub const IS_SUBMESSAGE_ORIGIN_AUTHENTICATED: u32 = 0x0000_0002;
    /// Payload (SEC_BODY) ist verschluesselt (sonst nur authentifiziert).
    pub const IS_PAYLOAD_ENCRYPTED: u32 = 0x0000_0004;
}

// ============================================================================
// EndpointSecurityInfo-Struct
// ============================================================================

/// Wire-Repraesentation von `PID_ENDPOINT_SECURITY_INFO`.
///
/// Die rohen Masken werden unmodifiziert durchgereicht — die
/// Policy-Layer-Konversion (z.B. "is_submessage_protected bedeutet
/// ProtectionLevel::Sign/Encrypt") sitzt im `security-runtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EndpointSecurityInfo {
    /// Standard-Endpoint-Security-Attribute (siehe [`attrs`]).
    pub endpoint_security_attributes: u32,
    /// Plugin-spezifische Endpoint-Security-Attribute (siehe
    /// [`plugin_attrs`]).
    pub plugin_endpoint_security_attributes: u32,
}

impl EndpointSecurityInfo {
    /// Wire-Size (2 * u32).
    pub const WIRE_SIZE: usize = 8;

    /// `true` wenn der Spec-konforme `IS_VALID`-Bit in beiden Masken
    /// gesetzt ist. Andernfalls soll der Receiver die Werte ignorieren
    /// (§7.4.1.5 Satz 2).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_VALID) != 0
            && (self.plugin_endpoint_security_attributes & plugin_attrs::IS_VALID) != 0
    }

    /// Builder fuer ein "plain-Legacy"-Endpoint (alle Bits 0 ausser
    /// den IS_VALID-Flags) — entspricht: der Peer unterstuetzt die
    /// Security-PID, will aber keinen Schutz fuer dieses Endpoint.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            endpoint_security_attributes: attrs::IS_VALID,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID,
        }
    }

    /// `true` wenn Submessage-Level-Protection gesetzt ist.
    #[must_use]
    pub const fn is_submessage_protected(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_SUBMESSAGE_PROTECTED) != 0
    }

    /// `true` wenn Payload-Level-Protection gesetzt ist.
    #[must_use]
    pub const fn is_payload_protected(&self) -> bool {
        (self.endpoint_security_attributes & attrs::IS_PAYLOAD_PROTECTED) != 0
    }

    /// `true` wenn Plugin AEAD-Encryption fuer Submessages anmeldet.
    #[must_use]
    pub const fn is_submessage_encrypted(&self) -> bool {
        (self.plugin_endpoint_security_attributes & plugin_attrs::IS_SUBMESSAGE_ENCRYPTED) != 0
    }

    /// `true` wenn Plugin Origin-Authentication-Tag meldet (Stufe 7
    /// Receiver-Specific-MACs).
    #[must_use]
    pub const fn is_submessage_origin_authenticated(&self) -> bool {
        (self.plugin_endpoint_security_attributes
            & plugin_attrs::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED)
            != 0
    }

    /// `true` wenn Plugin Payload-Ciphertext meldet.
    #[must_use]
    pub const fn is_payload_encrypted(&self) -> bool {
        (self.plugin_endpoint_security_attributes & plugin_attrs::IS_PAYLOAD_ENCRYPTED) != 0
    }

    /// Encode zu 8 Byte (2 * u32 LE oder BE).
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

    /// Decode aus 8 Byte (Value eines `PID_ENDPOINT_SECURITY_INFO`-
    /// Parameters).
    ///
    /// # Errors
    /// `UnexpectedEof` wenn Input < 8 Byte.
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
            "default == alle null == is_valid muss false sein"
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
