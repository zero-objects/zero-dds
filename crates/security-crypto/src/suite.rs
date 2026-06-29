// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crypto suite selection (AES-GCM 128 / 256).
//!
//! OMG DDS-Security 1.2 §10.5 Tab.79 defines the `CryptoTransform-
//! Kind` constants — 4-byte big-endian values:
//!
//! | Spec constant                               | Hex-Low | Algorithm         | Key |
//! |---------------------------------------------|--------:|-------------------|----:|
//! | `CRYPTO_TRANSFORMATION_KIND_NONE`           |  `0x00` | (no protection)   |  -  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES128_GMAC`    |  `0x01` | AES-128 (GMAC)    | 16  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES128_GCM`     |  `0x02` | AES-128 (GCM)     | 16  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES256_GMAC`    |  `0x03` | AES-256 (GMAC)    | 32  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES256_GCM`     |  `0x04` | AES-256 (GCM)     | 32  |
//!
//! Both GCM variants provide integrity + confidentiality; the GMAC
//! variants are auth-only (the payload stays plain, the tag is appended).
//!
//! **C3.6 wire-conflict fix (2026-04-25):** before this commit
//! `Suite::transform_kind_id()` had a non-spec-conformant mapping
//! (Aes128Gcm=0x01, Aes256Gcm=0x02, HmacSha256=0x03), which made a
//! Cyclone DDS receiver reject our submessages with "wrong algorithm".
//! Now spec-conformant — a wire-breaking change relative to
//! v0.x ZeroDDS peers.

use crate::backend::aead;

/// `CryptoTransformKind` constants as low-byte IDs (spec-1.2 §10.5
/// Tab.79). On the wire it is the 4-byte big-endian array
/// `[0, 0, 0, ID]`.
///
/// `NONE` and `AES256_GMAC` are currently not exposed by ZeroDDS as a
/// `Suite` variant — the constants are still here for
/// decoder paths that must recognize the spec values ("supported" vs
/// "known but not implemented").
#[allow(dead_code)]
pub mod transform_kind {
    /// `CRYPTO_TRANSFORMATION_KIND_NONE` — no protection.
    pub const NONE: u8 = 0x00;
    /// `CRYPTO_TRANSFORMATION_KIND_AES128_GMAC` — AES-128 auth-only.
    pub const AES128_GMAC: u8 = 0x01;
    /// `CRYPTO_TRANSFORMATION_KIND_AES128_GCM` — AES-128 auth+encrypt.
    pub const AES128_GCM: u8 = 0x02;
    /// `CRYPTO_TRANSFORMATION_KIND_AES256_GMAC` — AES-256 auth-only.
    pub const AES256_GMAC: u8 = 0x03;
    /// `CRYPTO_TRANSFORMATION_KIND_AES256_GCM` — AES-256 auth+encrypt.
    pub const AES256_GCM: u8 = 0x04;
}

/// Available crypto suites in the `AesGcmCryptoPlugin`.
///
/// The default (`AesGcmCryptoPlugin::new()`) is `Aes128Gcm` — lightweight,
/// enough for < 10-year confidentiality. For higher levels
/// choose `AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm)`.
///
/// `HmacSha256` is **auth-only** (no confidentiality). Used
/// when the governance XML mandates `metadata_protection_kind=SIGN`
/// — the payload stays plain, but is authenticated by an HMAC tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// AES-128 in GCM mode. 16-byte master key. Auth+encrypt.
    Aes128Gcm,
    /// AES-256 in GCM mode. 32-byte master key. Auth+encrypt.
    Aes256Gcm,
    /// AES-256 in GMAC mode (auth-only, spec kind 0x03). 32-byte
    /// master key; the payload stays cleartext, 16-byte AES-GMAC tag.
    /// Cyclone-conformant for `*_protection_kind=SIGN`.
    Aes256Gmac,
    /// HMAC-SHA256 auth-only. 32-byte master key. The payload stays
    /// plain; a 32-byte HMAC is appended (spec kind
    /// `NONE + HMAC_SHA256` — "SIGN" in the governance XML).
    HmacSha256,
}

impl Suite {
    /// Required master-key length in bytes.
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            Self::Aes128Gcm => 16,
            Self::Aes256Gcm | Self::HmacSha256 | Self::Aes256Gmac => 32,
        }
    }

    /// ring algorithm reference for AEAD. For `HmacSha256` there is
    /// no AEAD algo → the caller must handle this suite separately.
    #[must_use]
    pub(crate) fn algorithm(self) -> Option<&'static aead::Algorithm> {
        match self {
            Self::Aes128Gcm => Some(&aead::AES_128_GCM),
            Self::Aes256Gcm | Self::Aes256Gmac => Some(&aead::AES_256_GCM),
            Self::HmacSha256 => None,
        }
    }

    /// `true` if the suite provides confidentiality (otherwise auth-only).
    #[must_use]
    pub const fn is_aead(self) -> bool {
        matches!(self, Self::Aes128Gcm | Self::Aes256Gcm)
    }

    /// One-byte transform-kind ID for the wire format (SEC_PREFIX).
    /// Spec DDS-Security 1.2 §10.5 Tab.79 — we return the low-byte
    /// part; the wire codec packs it into `[0, 0, 0, id]` BE.
    ///
    /// `HmacSha256` occupies the `AES128_GMAC` slot (`0x01`) — the
    /// ZeroDDS implementation runs with HMAC-SHA256 instead of AES-GMAC,
    /// i.e. for SIGN-only topics the Cyclone interop is not yet
    /// in place (see C3.7). The GCM variants are fully spec-conformant.
    #[must_use]
    pub fn transform_kind_id(self) -> u8 {
        match self {
            Self::Aes128Gcm => transform_kind::AES128_GCM, // 0x02
            Self::Aes256Gcm => transform_kind::AES256_GCM, // 0x04
            Self::Aes256Gmac => transform_kind::AES256_GMAC, // 0x03
            Self::HmacSha256 => transform_kind::AES128_GMAC, // 0x01 (Slot)
        }
    }

    /// 4-byte `CryptoTransformKind` Big-Endian-Wire-Repraesentation
    /// (Spec §10.5 Tab.79).
    #[must_use]
    pub fn transform_kind(self) -> [u8; 4] {
        [0, 0, 0, self.transform_kind_id()]
    }

    /// Inverse of [`Self::transform_kind_id`]. Returns `None` for
    /// spec values that ZeroDDS does not offer as a suite (e.g.
    /// `AES256_GMAC = 0x03`).
    #[must_use]
    pub fn from_transform_kind_id(id: u8) -> Option<Self> {
        match id {
            transform_kind::AES128_GMAC => Some(Self::HmacSha256),
            transform_kind::AES128_GCM => Some(Self::Aes128Gcm),
            transform_kind::AES256_GMAC => Some(Self::Aes256Gmac),
            transform_kind::AES256_GCM => Some(Self::Aes256Gcm),
            _ => None,
        }
    }

    /// Maximum encrypts per key before a key refresh is needed. Spec
    /// §9.5.3.3.4 recommends ≤ 2^32 for GCM — we cap at 2^48
    /// (conservatively below the soft limit, far below the hard nonce bound).
    #[must_use]
    pub const fn max_encrypts(self) -> u64 {
        // 2^48 = 281_474_976_710_656
        1u64 << 48
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::Aes128Gcm
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn spec_constants_match_table_79() {
        // Spec DDS-Security 1.2 §10.5 Tab.79 — the wire values must
        // be immutable. Cyclone DDS / FastDDS rely
        // on exactly these bytes.
        assert_eq!(transform_kind::NONE, 0x00);
        assert_eq!(transform_kind::AES128_GMAC, 0x01);
        assert_eq!(transform_kind::AES128_GCM, 0x02);
        assert_eq!(transform_kind::AES256_GMAC, 0x03);
        assert_eq!(transform_kind::AES256_GCM, 0x04);
    }

    #[test]
    fn aes128_gcm_uses_spec_id_2() {
        assert_eq!(Suite::Aes128Gcm.transform_kind_id(), 0x02);
        assert_eq!(Suite::Aes128Gcm.transform_kind(), [0, 0, 0, 0x02]);
    }

    #[test]
    fn aes256_gcm_uses_spec_id_4() {
        assert_eq!(Suite::Aes256Gcm.transform_kind_id(), 0x04);
        assert_eq!(Suite::Aes256Gcm.transform_kind(), [0, 0, 0, 0x04]);
    }

    #[test]
    fn hmac_sha256_uses_aes128_gmac_slot() {
        // ZeroDDS drift: the implementation is HMAC-SHA256, but the wire slot
        // is AES128_GMAC=0x01. SIGN-only cross-vendor (e.g.
        // Cyclone) is not yet spec-conformant here; see C3.7.
        assert_eq!(Suite::HmacSha256.transform_kind_id(), 0x01);
    }

    #[test]
    fn from_id_roundtrip_for_all_supported() {
        assert_eq!(Suite::from_transform_kind_id(0x01), Some(Suite::HmacSha256));
        assert_eq!(Suite::from_transform_kind_id(0x02), Some(Suite::Aes128Gcm));
        assert_eq!(Suite::from_transform_kind_id(0x04), Some(Suite::Aes256Gcm));
    }

    #[test]
    fn from_id_rejects_unsupported() {
        // 0x00 NONE — we do not offer a None suite
        assert!(Suite::from_transform_kind_id(0x00).is_none());
        // 0x03 AES256_GMAC — supported since GMAC support (SIGN).
        assert_eq!(Suite::from_transform_kind_id(0x03), Some(Suite::Aes256Gmac));
        // Reserved / unknown value
        assert!(Suite::from_transform_kind_id(0x99).is_none());
    }

    #[test]
    fn each_suite_roundtrips_id() {
        for s in [Suite::Aes128Gcm, Suite::Aes256Gcm, Suite::HmacSha256] {
            let id = s.transform_kind_id();
            assert_eq!(Suite::from_transform_kind_id(id), Some(s));
        }
    }
}
