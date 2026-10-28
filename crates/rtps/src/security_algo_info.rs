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
//! Spec defaults (used when a peer sends no algorithm-info PID — §7.3.11.2,
//! §7.3.12.2, §7.3.13.2, §7.3.15.2):
//! - sig.trust_chain: supported = PSS | PKCS1 | ECDSA-P256, required = ECDSA-P256
//! - sig.message_auth: supported = PSS | ECDSA-P256, required = ECDSA-P256
//! - kx.shared_secret: supported = DHE-MODP | ECDHE-P256, required = ECDHE-P256
//! - sym.supported_mask: AES128 | AES256 (the AEAD bits of Table 22)
//! - sym.builtin_endpoints_required_mask, builtin_kx_endpoints_required_mask,
//!   user_endpoints_default_required_mask: AES256
//! - endpoint required_mask: the participant's user_endpoints_default_required_mask
//!
//! Two sets are compatible by `check_crypto_algorithm_compatibility` (§7.3.10.1),
//! including the compatibility-mode bit `0x80000000`.

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

/// `CRYPTO_ALGORITHM_COMPATIBILITY_MODE` (DDS-Security 1.2 §7.3.10): in a
/// `required_mask` it lets one algorithm in common be enough.
pub const CRYPTO_ALGORITHM_COMPATIBILITY_MODE: u32 = 0x8000_0000;

/// `CheckCryptoAlgorithmCompatibility(supported_mask, required_mask)`
/// (DDS-Security 1.2 §7.3.10.1): every required algorithm is supported, or
/// at least one is and the compatibility mode bit is set in `required_mask`.
#[must_use]
pub const fn check_crypto_algorithm_compatibility(supported_mask: u32, required_mask: u32) -> bool {
    let common = required_mask & supported_mask;
    common == required_mask
        || (common != 0 && (required_mask & CRYPTO_ALGORITHM_COMPATIBILITY_MODE) != 0)
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

    /// `CheckCryptoAlgorithmCompatibility(remote_supported, self.required)`
    /// (Spec §7.3.10.1): the algorithms this side requires are supported by
    /// the remote one.
    #[must_use]
    pub fn is_compatible_with(&self, remote_supported: u32) -> bool {
        check_crypto_algorithm_compatibility(remote_supported, self.required)
    }

    /// The `CryptoAlgorithmRequirements` of two participants are compatible
    /// (Spec §7.3.10.1) if each one's required algorithms are supported by
    /// the other.
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.is_compatible_with(other.supported) && other.is_compatible_with(self.supported)
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

    /// Spec default (§7.3.11.2): the value of a participant that sends none.
    #[must_use]
    pub fn spec_default() -> Self {
        Self {
            trust_chain: AlgorithmRequirements {
                supported: digital_signature_bit::RSASSA_PSS_2048_SHA256
                    | digital_signature_bit::RSASSA_PKCS1_V15_2048_SHA256
                    | digital_signature_bit::ECDSA_P256_SHA256,
                required: digital_signature_bit::ECDSA_P256_SHA256,
            },
            message_auth: AlgorithmRequirements {
                supported: digital_signature_bit::RSASSA_PSS_2048_SHA256
                    | digital_signature_bit::ECDSA_P256_SHA256,
                required: digital_signature_bit::ECDSA_P256_SHA256,
            },
        }
    }

    /// Compatibility (§7.3.11.1): `trust_chain` and `message_auth` both
    /// compatible (§7.3.10.1).
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.trust_chain.compatible_with(&other.trust_chain)
            && self.message_auth.compatible_with(&other.message_auth)
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

    /// Spec default (§7.3.12.2): the value of a participant that sends none.
    #[must_use]
    pub fn spec_default() -> Self {
        Self {
            shared_secret: AlgorithmRequirements {
                supported: key_establishment_bit::DHE_MODP_2048_256
                    | key_establishment_bit::ECDHE_CEUM_P256,
                required: key_establishment_bit::ECDHE_CEUM_P256,
            },
        }
    }

    /// Compatibility (§7.3.12.1): `shared_secret` compatible (§7.3.10.1).
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.shared_secret.compatible_with(&other.shared_secret)
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

    /// Spec default (§7.3.13.2): the value of a participant that sends none.
    #[must_use]
    pub fn spec_default() -> Self {
        Self {
            supported_mask: symmetric_bit::AES128 | symmetric_bit::AES256,
            builtin_endpoints_required_mask: symmetric_bit::AES256,
            builtin_kx_endpoints_required_mask: symmetric_bit::AES256,
            user_endpoints_default_required_mask: symmetric_bit::AES256,
        }
    }

    /// Compatibility (§7.3.13.1): the builtin endpoint and builtin Kx
    /// endpoint requirements of each side are supported by the other. The
    /// `user_endpoints_default_required_mask` is not considered.
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        let one_way = |from: &Self, to: &Self| {
            check_crypto_algorithm_compatibility(
                to.supported_mask,
                from.builtin_endpoints_required_mask,
            ) && check_crypto_algorithm_compatibility(
                to.supported_mask,
                from.builtin_kx_endpoints_required_mask,
            )
        };
        one_way(self, other) && one_way(other, self)
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
    /// `supported_mask` (`@non_serialized`, §7.3.15): the
    /// `symmetric_cipher.supported_mask` of the DomainParticipant that
    /// contains the endpoint. It is not part of
    /// `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO`; the implementation sets it
    /// ([`Self::with_supported_mask`], [`Self::spec_default`]). A decoded value
    /// has 0 until it is set.
    pub supported_mask: u32,
}

impl EndpointSecuritySymmetricCipherAlgorithmInfo {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Value of an endpoint that sends none (§7.3.15.2): the
    /// `user_endpoints_default_required_mask` of its participant.
    #[must_use]
    pub fn spec_default(participant: &ParticipantSecuritySymmetricCipherAlgorithmInfo) -> Self {
        Self {
            required_mask: participant.user_endpoints_default_required_mask,
            supported_mask: participant.supported_mask,
        }
    }

    /// The same info with another `required_mask`.
    #[must_use]
    pub fn with_required_mask(mut self, required: u32) -> Self {
        self.required_mask = required;
        self
    }

    /// The same info with the `supported_mask` of the participant that
    /// contains the endpoint (§7.3.15).
    #[must_use]
    pub fn with_supported_mask(mut self, participant_supported: u32) -> Self {
        self.supported_mask = participant_supported;
        self
    }

    /// Compatibility of endpoint E1 of participant P1 with endpoint E2 of
    /// participant P2 (§7.3.15.1): what E1 requires is supported by P2, and
    /// what E2 requires is supported by P1. The `supported_mask` of each is
    /// that of its participant.
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        check_crypto_algorithm_compatibility(other.supported_mask, self.required_mask)
            && check_crypto_algorithm_compatibility(self.supported_mask, other.required_mask)
    }

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
            supported_mask: 0,
        })
    }
}

// ----------------------------------------------------------------------
// §7.3.14 and §7.3.16 — the aggregates
// ----------------------------------------------------------------------

/// Spec §7.3.14 `ParticipantSecurityAlgorithmInfo`: the algorithms the
/// Participant SPIs support and require, the three parts together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantSecurityAlgorithmInfo {
    /// `digital_signature` (§7.3.11).
    pub digital_signature: ParticipantSecurityDigitalSignatureAlgorithmInfo,
    /// `key_establishment` (§7.3.12).
    pub key_establishment: ParticipantSecurityKeyEstablishmentAlgorithmInfo,
    /// `symmetric_cipher` (§7.3.13).
    pub symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo,
}

impl ParticipantSecurityAlgorithmInfo {
    /// Wire size of the three members one after the other: 16 + 8 + 16.
    pub const WIRE_SIZE: usize = 40;

    /// The value of a participant that sends none of the three (§7.3.11.2,
    /// §7.3.12.2, §7.3.13.2).
    #[must_use]
    pub fn spec_default() -> Self {
        Self {
            digital_signature: ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default(),
            key_establishment: ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default(),
            symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default(),
        }
    }

    /// Compatible if and only if each of the three parts is (§7.3.11.1,
    /// §7.3.12.1, §7.3.13.1).
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.digital_signature
            .compatible_with(&other.digital_signature)
            && self
                .key_establishment
                .compatible_with(&other.key_establishment)
            && self
                .symmetric_cipher
                .compatible_with(&other.symmetric_cipher)
    }

    /// Encode the three members in the order of the IDL.
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; Self::WIRE_SIZE] {
        let mut out = [0u8; Self::WIRE_SIZE];
        out[..16].copy_from_slice(&self.digital_signature.to_bytes(little_endian));
        out[16..24].copy_from_slice(&self.key_establishment.to_bytes(little_endian));
        out[24..].copy_from_slice(&self.symmetric_cipher.to_bytes(little_endian));
        out
    }

    /// Decode the three members.
    ///
    /// # Errors
    /// `ValueOutOfRange` on fewer than [`Self::WIRE_SIZE`] bytes.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantSecurityAlgorithmInfo: < 40 bytes",
            });
        }
        Ok(Self {
            digital_signature: ParticipantSecurityDigitalSignatureAlgorithmInfo::from_bytes(
                &bytes[..16],
                little_endian,
            )?,
            key_establishment: ParticipantSecurityKeyEstablishmentAlgorithmInfo::from_bytes(
                &bytes[16..24],
                little_endian,
            )?,
            symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo::from_bytes(
                &bytes[24..40],
                little_endian,
            )?,
        })
    }
}

/// Spec §7.3.16 `EndpointSecurityAlgorithmInfo`: the algorithms the Endpoint
/// SPIs require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointSecurityAlgorithmInfo {
    /// `symmetric_cipher` (§7.3.15).
    pub symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo,
}

impl EndpointSecurityAlgorithmInfo {
    /// The value of an endpoint that sends none (§7.3.15.2).
    #[must_use]
    pub fn spec_default(participant: &ParticipantSecurityAlgorithmInfo) -> Self {
        Self {
            symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo::spec_default(
                &participant.symmetric_cipher,
            ),
        }
    }

    /// Compatible if and only if the symmetric cipher infos are (§7.3.15.1).
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        self.symmetric_cipher
            .compatible_with(&other.symmetric_cipher)
    }

    /// Encode the one member (the required mask, four octets).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; 4] {
        self.symmetric_cipher.to_bytes(little_endian)
    }

    /// Decode the one member.
    ///
    /// # Errors
    /// `ValueOutOfRange` on fewer than four bytes.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        Ok(Self {
            symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo::from_bytes(
                bytes,
                little_endian,
            )?,
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

    /// DDS-Security 1.2 §7.3.10.1, `CheckCryptoAlgorithmCompatibility`:
    /// `(required & supported) == required`, or some common algorithm and
    /// `CRYPTO_ALGORITHM_COMPATIBILITY_MODE` (0x80000000) in `required`.
    #[test]
    fn the_compatibility_mode_bit_makes_one_algorithm_enough() {
        assert_eq!(CRYPTO_ALGORITHM_COMPATIBILITY_MODE, 0x8000_0000);
        // Required {1, 2}: both are needed without the mode bit.
        assert!(!check_crypto_algorithm_compatibility(0b001, 0b011));
        assert!(check_crypto_algorithm_compatibility(0b011, 0b011));
        // With the mode bit one in common is enough ...
        let mode = CRYPTO_ALGORITHM_COMPATIBILITY_MODE;
        assert!(check_crypto_algorithm_compatibility(0b001, 0b011 | mode));
        // ... and none in common is not (the mode bit alone is no algorithm).
        assert!(!check_crypto_algorithm_compatibility(0b100, 0b011 | mode));
        // The mode bit is not an algorithm the supported side must have.
        assert!(check_crypto_algorithm_compatibility(0b011, 0b011 | mode));
        // Nothing required: compatible.
        assert!(check_crypto_algorithm_compatibility(0, 0));
    }

    /// §7.3.10.1: two participants are compatible if each one's required
    /// algorithms are supported by the other.
    #[test]
    fn two_requirements_are_compatible_both_ways() {
        let p1 = AlgorithmRequirements {
            supported: 0b011,
            required: 0b001,
        };
        let p2 = AlgorithmRequirements {
            supported: 0b110,
            required: 0b100,
        };
        // P1 requires 0b001, P2 supports 0b110: no.
        assert!(!p1.compatible_with(&p2));
        let p3 = AlgorithmRequirements {
            supported: 0b111,
            required: 0b010,
        };
        assert!(p1.compatible_with(&p3));
        assert!(p3.compatible_with(&p1));
    }

    /// DDS-Security 1.2 §7.3.11.2.
    #[test]
    fn sig_info_spec_default() {
        let d = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
        assert_eq!(
            d.trust_chain.supported,
            digital_signature_bit::RSASSA_PSS_2048_SHA256
                | digital_signature_bit::RSASSA_PKCS1_V15_2048_SHA256
                | digital_signature_bit::ECDSA_P256_SHA256
        );
        assert_eq!(
            d.trust_chain.required,
            digital_signature_bit::ECDSA_P256_SHA256
        );
        assert_eq!(
            d.message_auth.supported,
            digital_signature_bit::RSASSA_PSS_2048_SHA256
                | digital_signature_bit::ECDSA_P256_SHA256
        );
        assert_eq!(
            d.message_auth.required,
            digital_signature_bit::ECDSA_P256_SHA256
        );
    }

    #[test]
    fn sig_info_roundtrip() {
        let d = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
        let bytes = d.to_bytes(true);
        let back =
            ParticipantSecurityDigitalSignatureAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(d, back);
    }

    /// DDS-Security 1.2 §7.3.12.2.
    #[test]
    fn kx_info_spec_default() {
        let k = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
        assert_eq!(
            k.shared_secret.supported,
            key_establishment_bit::DHE_MODP_2048_256 | key_establishment_bit::ECDHE_CEUM_P256
        );
        assert_eq!(
            k.shared_secret.required,
            key_establishment_bit::ECDHE_CEUM_P256
        );
    }

    #[test]
    fn kx_info_roundtrip() {
        let k = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
        let bytes = k.to_bytes(true);
        let back =
            ParticipantSecurityKeyEstablishmentAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(k, back);
    }

    /// DDS-Security 1.2 §7.3.13.2 and §7.3.15.2.
    #[test]
    fn sym_info_spec_default() {
        let s = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
        assert_eq!(
            s.supported_mask,
            symmetric_bit::AES128 | symmetric_bit::AES256
        );
        assert_eq!(s.builtin_endpoints_required_mask, symmetric_bit::AES256);
        assert_eq!(s.builtin_kx_endpoints_required_mask, symmetric_bit::AES256);
        assert_eq!(
            s.user_endpoints_default_required_mask,
            symmetric_bit::AES256
        );
        assert_eq!(
            EndpointSecuritySymmetricCipherAlgorithmInfo::spec_default(&s).required_mask,
            symmetric_bit::AES256
        );
    }

    /// §7.3.13.1: the builtin and builtin-Kx requirements of each side must
    /// be supported by the other; the user endpoints default is not
    /// considered.
    #[test]
    fn symmetric_cipher_info_compatibility() {
        let aes256_only = ParticipantSecuritySymmetricCipherAlgorithmInfo {
            supported_mask: symmetric_bit::AES256,
            builtin_endpoints_required_mask: symmetric_bit::AES256,
            builtin_kx_endpoints_required_mask: symmetric_bit::AES256,
            user_endpoints_default_required_mask: symmetric_bit::AES256,
        };
        let aes128_only = ParticipantSecuritySymmetricCipherAlgorithmInfo {
            supported_mask: symmetric_bit::AES128,
            builtin_endpoints_required_mask: symmetric_bit::AES128,
            builtin_kx_endpoints_required_mask: symmetric_bit::AES128,
            user_endpoints_default_required_mask: symmetric_bit::AES128,
        };
        let default = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
        assert!(default.compatible_with(&default));
        assert!(default.compatible_with(&aes256_only));
        // The default requires AES256 for the builtin endpoints.
        assert!(!default.compatible_with(&aes128_only));
        assert!(!aes256_only.compatible_with(&aes128_only));
        // The user endpoints default alone does not break it.
        let mut other_user_default = aes256_only;
        other_user_default.user_endpoints_default_required_mask = symmetric_bit::AES128;
        assert!(aes256_only.compatible_with(&other_user_default));
    }

    /// §7.3.15.1.
    #[test]
    fn endpoint_symmetric_cipher_compatibility() {
        let e = |required, supported| EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: required,
            supported_mask: supported,
        };
        let both = symmetric_bit::AES128 | symmetric_bit::AES256;
        assert!(e(symmetric_bit::AES256, both).compatible_with(&e(symmetric_bit::AES128, both)));
        // E2 requires AES256, P1 supports AES128 only.
        assert!(
            !e(symmetric_bit::AES128, symmetric_bit::AES128)
                .compatible_with(&e(symmetric_bit::AES256, both))
        );
        // E1 requires AES256, P2 supports AES128 only.
        assert!(
            !e(symmetric_bit::AES256, both)
                .compatible_with(&e(symmetric_bit::AES128, symmetric_bit::AES128))
        );
    }

    /// §7.3.11.1 and §7.3.12.1.
    #[test]
    fn signature_and_key_establishment_compatibility() {
        let default = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
        assert!(default.compatible_with(&default));
        let p384_only = ParticipantSecurityDigitalSignatureAlgorithmInfo {
            trust_chain: AlgorithmRequirements {
                supported: digital_signature_bit::ECDSA_P384_SHA384,
                required: digital_signature_bit::ECDSA_P384_SHA384,
            },
            message_auth: AlgorithmRequirements {
                supported: digital_signature_bit::ECDSA_P384_SHA384,
                required: digital_signature_bit::ECDSA_P384_SHA384,
            },
        };
        assert!(!default.compatible_with(&p384_only));
        let kx = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
        assert!(kx.compatible_with(&kx));
        let p384 = ParticipantSecurityKeyEstablishmentAlgorithmInfo {
            shared_secret: AlgorithmRequirements {
                supported: key_establishment_bit::ECDHE_CEUM_P384,
                required: key_establishment_bit::ECDHE_CEUM_P384,
            },
        };
        assert!(!kx.compatible_with(&p384));
    }

    #[test]
    fn sym_info_roundtrip() {
        let s = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
        let bytes = s.to_bytes(true);
        let back =
            ParticipantSecuritySymmetricCipherAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(s, back);
    }

    /// §7.3.14 and §7.3.16: the aggregates are the three, respectively the one,
    /// member of the IDL in order; the default is that of the parts.
    #[test]
    fn the_aggregates_hold_their_members_in_order() {
        let participant = ParticipantSecurityAlgorithmInfo::spec_default();
        let bytes = participant.to_bytes(true);
        assert_eq!(bytes.len(), 40);
        assert_eq!(
            bytes[..16],
            ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default().to_bytes(true)
        );
        assert_eq!(
            bytes[16..24],
            ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default().to_bytes(true)
        );
        assert_eq!(
            bytes[24..],
            ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default().to_bytes(true)
        );
        assert_eq!(
            ParticipantSecurityAlgorithmInfo::from_bytes(&bytes, true).unwrap(),
            participant
        );
        assert!(participant.compatible_with(&participant));
        let mut other = participant;
        other.symmetric_cipher.supported_mask = symmetric_bit::AES128;
        assert!(!participant.compatible_with(&other));
        assert!(ParticipantSecurityAlgorithmInfo::from_bytes(&bytes[..39], true).is_err());
        let endpoint = EndpointSecurityAlgorithmInfo::spec_default(&participant);
        assert_eq!(endpoint.to_bytes(true), [0x02, 0, 0, 0]);
        assert_eq!(
            endpoint.symmetric_cipher.supported_mask,
            participant.symmetric_cipher.supported_mask
        );
        assert!(endpoint.compatible_with(&endpoint));
        assert_eq!(
            EndpointSecurityAlgorithmInfo::from_bytes(&[0x02, 0, 0, 0], true)
                .unwrap()
                .symmetric_cipher
                .required_mask,
            symmetric_bit::AES256
        );
    }

    #[test]
    fn endpoint_sym_info_roundtrip() {
        let e = EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: symmetric_bit::AES256,
            supported_mask: symmetric_bit::AES128 | symmetric_bit::AES256,
        };
        let bytes = e.to_bytes(true);
        // `supported_mask` is `@non_serialized` (§7.3.15): four octets, the
        // required mask only; a decoded info has no supported mask yet.
        assert_eq!(bytes, [0x02, 0, 0, 0]);
        let back = EndpointSecuritySymmetricCipherAlgorithmInfo::from_bytes(&bytes, true).unwrap();
        assert_eq!(back.required_mask, e.required_mask);
        assert_eq!(back.supported_mask, 0);
        assert_eq!(back.with_supported_mask(e.supported_mask), e);
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
