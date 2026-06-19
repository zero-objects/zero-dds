// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security 1.2 §7.3.11-§7.3.15 — Algorithm-Info-Strukturen + PIDs
//! 0x1010-0x1013 (C3.5-Rest).
//!
//! The spec requires three new properties in the SPDP announce so that peers
//! can negotiate over different algorithm families:
//!
//! | PID    | Spec                       | Content                                 |
//! |--------|----------------------------|-----------------------------------------|
//! | 0x1010 | §7.3.11                    | DigitalSignatureAlgorithmInfo (sig)     |
//! | 0x1011 | §7.3.12                    | KeyEstablishmentAlgorithmInfo (kx)      |
//! | 0x1012 | §7.3.13                    | SymmetricCipherAlgorithmInfo (sym)      |
//! | 0x1013 | §7.3.15 (endpoint level)   | EndpointSymmetricCipherAlgorithmInfo    |
//!
//! Bit constants (Spec §8.1 Tab.22 + §8.2 + §8.3, "CBIT" = CryptoBit):
//!
//! Symmetric (`SymmetricCipherBitId`):
//! - bit 0 = AES128 (covers GCM + GMAC)
//! - bit 1 = AES256 (covers GCM + GMAC)
//!
//! DigitalSignature (`DigitalSignatureBitId`):
//! - bit 0 = RSASSA-PSS-MGF1SHA256+2048+SHA256
//! - bit 1 = RSASSA-PKCS1-V1_5+2048+SHA256
//! - bit 2 = ECDSA+P256+SHA256
//! - bit 3 = ECDSA+P384+SHA384
//!
//! KeyEstablishment (`KeyEstablishmentBitId`):
//! - bit 0 = DHE+MODP-2048-256
//! - bit 1 = ECDHE-CEUM+P256
//! - bit 2 = ECDHE-CEUM+P384
//!
//! Spec defaults (used when a peer sends no algorithm-info PID
//! — backwards compat with DDS-Security 1.1):
//! - sig.trust_chain: supported = required = (RSASSA_PSS | ECDSA_P256)
//! - sig.message_auth: supported = required = (RSASSA_PSS | ECDSA_P256)
//! - kx.shared_secret: supported = required = (DHE_MODP | ECDHE_P256)
//! - sym.supported_mask: AES128 | AES256
//! - sym.builtin_endpoints_required_mask: AES128
//! - sym.builtin_kx_endpoints_required_mask: AES128
//! - sym.user_endpoints_default_required_mask: AES128

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;

// ----------------------------------------------------------------------
// Bit constants (Spec §8.1 Tab.22, §8.2, §8.3)
// ----------------------------------------------------------------------

/// Symmetric-cipher bit IDs (Spec §8.1).
pub mod symmetric_bit {
    /// AES-128 (GCM or GMAC).
    pub const AES128: u32 = 1 << 0;
    /// AES-256 (GCM or GMAC).
    pub const AES256: u32 = 1 << 1;
}

/// Digital-signature bit IDs (Spec §8.2).
pub mod digital_signature_bit {
    /// RSASSA-PSS-MGF1SHA256+2048+SHA256.
    pub const RSASSA_PSS_2048_SHA256: u32 = 1 << 0;
    /// RSASSA-PKCS1-V1_5+2048+SHA256.
    pub const RSASSA_PKCS1_V15_2048_SHA256: u32 = 1 << 1;
    /// ECDSA+P256+SHA256.
    pub const ECDSA_P256_SHA256: u32 = 1 << 2;
    /// ECDSA+P384+SHA384.
    pub const ECDSA_P384_SHA384: u32 = 1 << 3;
}

/// Key-establishment bit IDs (Spec §8.3).
pub mod key_establishment_bit {
    /// DHE+MODP-2048-256 (RFC 5114 group).
    pub const DHE_MODP_2048_256: u32 = 1 << 0;
    /// ECDHE-CEUM+P256 (NIST P-256 ephemeral).
    pub const ECDHE_CEUM_P256: u32 = 1 << 1;
    /// ECDHE-CEUM+P384.
    pub const ECDHE_CEUM_P384: u32 = 1 << 2;
}

// ----------------------------------------------------------------------
// AlgorithmRequirements (Spec §7.3.10)
// ----------------------------------------------------------------------

/// Spec §7.3.10 — `AlgorithmRequirements { supported_mask, required_mask }`.
/// Both u32 BE on the wire (8 bytes total).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlgorithmRequirements {
    /// Mask of all algorithms supported by this participant.
    pub supported: u32,
    /// Mask of the algorithms the participant *requires*. A
    /// compatibility check (Spec §7.3.10) requires
    /// `(remote.supported & local.required) == local.required`.
    pub required: u32,
}

impl AlgorithmRequirements {
    /// Wire size: 4 byte supported + 4 byte required = 8 bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Encode (4 byte u32 LE/BE × 2).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 8] {
        let mut out = [0u8; 8];
        if little_endian {
            out[0..4].copy_from_slice(&self.supported.to_le_bytes());
            out[4..8].copy_from_slice(&self.required.to_le_bytes());
        } else {
            out[0..4].copy_from_slice(&self.supported.to_be_bytes());
            out[4..8].copy_from_slice(&self.required.to_be_bytes());
        }
        out
    }

    /// Decode from 8 bytes.
    ///
    /// # Errors
    /// `ValueOutOfRange` on slice mismatch.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < 8 {
            return Err(WireError::ValueOutOfRange {
                message: "AlgorithmRequirements: < 8 bytes",
            });
        }
        let mut s = [0u8; 4];
        let mut r = [0u8; 4];
        s.copy_from_slice(&bytes[0..4]);
        r.copy_from_slice(&bytes[4..8]);
        Ok(if little_endian {
            Self {
                supported: u32::from_le_bytes(s),
                required: u32::from_le_bytes(r),
            }
        } else {
            Self {
                supported: u32::from_be_bytes(s),
                required: u32::from_be_bytes(r),
            }
        })
    }

    /// Compatibility check (Spec §7.3.10): check whether `remote.supported`
    /// contains all bits of `self.required`.
    #[must_use]
    pub fn is_compatible_with(&self, remote_supported: u32) -> bool {
        (remote_supported & self.required) == self.required
    }
}

// ----------------------------------------------------------------------
// PID 0x1010 — ParticipantSecurityDigitalSignatureAlgorithmInfo
// ----------------------------------------------------------------------

/// Spec §7.3.11 — sig algorithm info per participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantSecurityDigitalSignatureAlgorithmInfo {
    /// Trust-chain algorithms (cert sig verify).
    pub trust_chain: AlgorithmRequirements,
    /// Message-authentication algorithms (handshake signatures, OCSP).
    pub message_auth: AlgorithmRequirements,
}

impl ParticipantSecurityDigitalSignatureAlgorithmInfo {
    /// Wire size: 16 bytes (2 × `AlgorithmRequirements`).
    pub const WIRE_SIZE: usize = 16;

    /// Spec default (see module docs).
    #[must_use]
    pub fn spec_default() -> Self {
        let mask = digital_signature_bit::RSASSA_PSS_2048_SHA256
            | digital_signature_bit::ECDSA_P256_SHA256;
        Self {
            trust_chain: AlgorithmRequirements {
                supported: mask,
                required: mask,
            },
            message_auth: AlgorithmRequirements {
                supported: mask,
                required: mask,
            },
        }
    }

    /// Encode (16 bytes).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..8].copy_from_slice(&self.trust_chain.to_bytes(little_endian));
        out[8..16].copy_from_slice(&self.message_auth.to_bytes(little_endian));
        out
    }

    /// Decode (16 bytes).
    ///
    /// # Errors
    /// `ValueOutOfRange` on slice mismatch.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < 16 {
            return Err(WireError::ValueOutOfRange {
                message: "DigitalSignatureAlgorithmInfo: < 16 bytes",
            });
        }
        Ok(Self {
            trust_chain: AlgorithmRequirements::from_bytes(&bytes[0..8], little_endian)?,
            message_auth: AlgorithmRequirements::from_bytes(&bytes[8..16], little_endian)?,
        })
    }
}

// ----------------------------------------------------------------------
// PID 0x1011 — ParticipantSecurityKeyEstablishmentAlgorithmInfo
// ----------------------------------------------------------------------

/// Spec §7.3.12 — key-establishment algorithm info per participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantSecurityKeyEstablishmentAlgorithmInfo {
    /// Shared-secret algorithms (DH/ECDH).
    pub shared_secret: AlgorithmRequirements,
}

impl ParticipantSecurityKeyEstablishmentAlgorithmInfo {
    /// Wire size: 8 bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Spec default.
    #[must_use]
    pub fn spec_default() -> Self {
        let mask =
            key_establishment_bit::DHE_MODP_2048_256 | key_establishment_bit::ECDHE_CEUM_P256;
        Self {
            shared_secret: AlgorithmRequirements {
                supported: mask,
                required: mask,
            },
        }
    }

    /// Encode (8 bytes).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 8] {
        self.shared_secret.to_bytes(little_endian)
    }

    /// Decode (8 bytes).
    ///
    /// # Errors
    /// `ValueOutOfRange` on slice mismatch.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        Ok(Self {
            shared_secret: AlgorithmRequirements::from_bytes(bytes, little_endian)?,
        })
    }
}

// ----------------------------------------------------------------------
// PID 0x1012 — ParticipantSecuritySymmetricCipherAlgorithmInfo
// ----------------------------------------------------------------------

/// Spec §7.3.13 — symmetric-cipher algorithm info per participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantSecuritySymmetricCipherAlgorithmInfo {
    /// Mask of all supported symmetric-cipher families
    /// (see [`symmetric_bit`]).
    pub supported_mask: u32,
    /// Mask of the required algos for builtin endpoints (discovery,
    /// liveliness, volatile, stateless).
    pub builtin_endpoints_required_mask: u32,
    /// Mask of the required algos for builtin KX endpoints (crypto-token
    /// exchange via VolatileMessageSecure).
    pub builtin_kx_endpoints_required_mask: u32,
    /// Default mask for user endpoints (can be overwritten by the
    /// user-endpoint PID 0x1013).
    pub user_endpoints_default_required_mask: u32,
}

impl ParticipantSecuritySymmetricCipherAlgorithmInfo {
    /// Wire size: 16 bytes (4 × u32).
    pub const WIRE_SIZE: usize = 16;

    /// Spec default.
    #[must_use]
    pub fn spec_default() -> Self {
        Self {
            supported_mask: symmetric_bit::AES128 | symmetric_bit::AES256,
            builtin_endpoints_required_mask: symmetric_bit::AES128,
            builtin_kx_endpoints_required_mask: symmetric_bit::AES128,
            user_endpoints_default_required_mask: symmetric_bit::AES128,
        }
    }

    /// Encode (16 bytes).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 16] {
        let mut out = [0u8; 16];
        let fields = [
            self.supported_mask,
            self.builtin_endpoints_required_mask,
            self.builtin_kx_endpoints_required_mask,
            self.user_endpoints_default_required_mask,
        ];
        for (i, v) in fields.iter().enumerate() {
            let bytes = if little_endian {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            };
            out[i * 4..i * 4 + 4].copy_from_slice(&bytes);
        }
        out
    }

    /// Decode (16 bytes).
    ///
    /// # Errors
    /// `ValueOutOfRange` on slice mismatch.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < 16 {
            return Err(WireError::ValueOutOfRange {
                message: "SymmetricCipherAlgorithmInfo: < 16 bytes",
            });
        }
        let read = |off: usize| -> u32 {
            let mut a = [0u8; 4];
            a.copy_from_slice(&bytes[off..off + 4]);
            if little_endian {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        };
        Ok(Self {
            supported_mask: read(0),
            builtin_endpoints_required_mask: read(4),
            builtin_kx_endpoints_required_mask: read(8),
            user_endpoints_default_required_mask: read(12),
        })
    }
}

// ----------------------------------------------------------------------
// PID 0x1013 — EndpointSecuritySymmetricCipherAlgorithmInfo
// ----------------------------------------------------------------------

/// Spec §7.3.15 — symmetric-cipher algorithm info per endpoint
/// (DataWriter / DataReader). Kept in Pub/SubscriptionBuiltinTopicData
/// — overrides the participant default for this slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointSecuritySymmetricCipherAlgorithmInfo {
    /// Required mask for this endpoint (see [`symmetric_bit`]).
    pub required_mask: u32,
}

impl EndpointSecuritySymmetricCipherAlgorithmInfo {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Encode (4 bytes).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 4] {
        if little_endian {
            self.required_mask.to_le_bytes()
        } else {
            self.required_mask.to_be_bytes()
        }
    }

    /// Decode (4 bytes).
    ///
    /// # Errors
    /// `ValueOutOfRange` on slice mismatch.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::ValueOutOfRange {
                message: "EndpointSymmetricCipherAlgorithmInfo: < 4 bytes",
            });
        }
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[0..4]);
        Ok(Self {
            required_mask: if little_endian {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            },
        })
    }
}

/// Suppress warning for the `Vec` import (used in tests).
#[allow(dead_code)]
fn _vec_keepalive(v: Vec<u8>) -> Vec<u8> {
    v
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_requirements_roundtrip_le() {
        let a = AlgorithmRequirements {
            supported: 0xCAFE_BABE,
            required: 0xDEAD_BEEF,
        };
        let bytes = a.to_bytes(true);
        let back = AlgorithmRequirements::from_bytes(&bytes, true).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn algorithm_requirements_roundtrip_be() {
        let a = AlgorithmRequirements {
            supported: 0x0102_0304,
            required: 0x0506_0708,
        };
        let bytes = a.to_bytes(false);
        let back = AlgorithmRequirements::from_bytes(&bytes, false).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn algorithm_requirements_layout_be() {
        // Spec-conformant BE: 4 byte supported + 4 byte required.
        let a = AlgorithmRequirements {
            supported: 0x01,
            required: 0x02,
        };
        assert_eq!(a.to_bytes(false), [0, 0, 0, 0x01, 0, 0, 0, 0x02]);
    }

    #[test]
    fn compatibility_check_strict() {
        let local = AlgorithmRequirements {
            supported: 0,
            required: 0b101,
        };
        // Remote supports all 3 → compatible.
        assert!(local.is_compatible_with(0b111));
        // Remote supports bit 0 + 2 (exactly the required bits) → ok.
        assert!(local.is_compatible_with(0b101));
        // Remote supports bit 0 alone → missing bit 2 → reject.
        assert!(!local.is_compatible_with(0b001));
        // Remote supports nothing → reject.
        assert!(!local.is_compatible_with(0));
    }

    #[test]
    fn sig_info_spec_default() {
        let d = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
        let expected = digital_signature_bit::RSASSA_PSS_2048_SHA256
            | digital_signature_bit::ECDSA_P256_SHA256;
        assert_eq!(d.trust_chain.supported, expected);
        assert_eq!(d.trust_chain.required, expected);
        assert_eq!(d.message_auth.supported, expected);
        assert_eq!(d.message_auth.required, expected);
    }

    #[test]
    fn sig_info_roundtrip() {
        let d = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
        let bytes = d.to_bytes(true);
        let back =
            ParticipantSecurityDigitalSignatureAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn kx_info_spec_default() {
        let k = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
        let expected =
            key_establishment_bit::DHE_MODP_2048_256 | key_establishment_bit::ECDHE_CEUM_P256;
        assert_eq!(k.shared_secret.supported, expected);
        assert_eq!(k.shared_secret.required, expected);
    }

    #[test]
    fn kx_info_roundtrip() {
        let k = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
        let bytes = k.to_bytes(true);
        let back =
            ParticipantSecurityKeyEstablishmentAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(k, back);
    }

    #[test]
    fn sym_info_spec_default() {
        let s = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
        assert_eq!(
            s.supported_mask,
            symmetric_bit::AES128 | symmetric_bit::AES256
        );
        assert_eq!(s.builtin_endpoints_required_mask, symmetric_bit::AES128);
        assert_eq!(s.builtin_kx_endpoints_required_mask, symmetric_bit::AES128);
        assert_eq!(
            s.user_endpoints_default_required_mask,
            symmetric_bit::AES128
        );
    }

    #[test]
    fn sym_info_roundtrip() {
        let s = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
        let bytes = s.to_bytes(true);
        let back =
            ParticipantSecuritySymmetricCipherAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn endpoint_sym_info_roundtrip() {
        let e = EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: symmetric_bit::AES256,
        };
        let bytes = e.to_bytes(true);
        let back = EndpointSecuritySymmetricCipherAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn truncated_buffer_rejected() {
        assert!(AlgorithmRequirements::from_bytes(&[1, 2, 3], true).is_err());
        assert!(
            ParticipantSecurityDigitalSignatureAlgorithmInfo::from_bytes(&[0u8; 15], true).is_err()
        );
        assert!(
            ParticipantSecuritySymmetricCipherAlgorithmInfo::from_bytes(&[0u8; 15], true).is_err()
        );
        assert!(EndpointSecuritySymmetricCipherAlgorithmInfo::from_bytes(&[0u8; 3], true).is_err());
    }

    #[test]
    fn spec_bit_constants_match() {
        // Spec §8.1 Tab.22 + §8.2 + §8.3 — these constants must
        // NEVER drift, otherwise a Cyclone peer sees our caps wrong.
        assert_eq!(symmetric_bit::AES128, 0x01);
        assert_eq!(symmetric_bit::AES256, 0x02);
        assert_eq!(digital_signature_bit::RSASSA_PSS_2048_SHA256, 0x01);
        assert_eq!(digital_signature_bit::RSASSA_PKCS1_V15_2048_SHA256, 0x02);
        assert_eq!(digital_signature_bit::ECDSA_P256_SHA256, 0x04);
        assert_eq!(digital_signature_bit::ECDSA_P384_SHA384, 0x08);
        assert_eq!(key_establishment_bit::DHE_MODP_2048_256, 0x01);
        assert_eq!(key_establishment_bit::ECDHE_CEUM_P256, 0x02);
        assert_eq!(key_establishment_bit::ECDHE_CEUM_P384, 0x04);
    }
}
