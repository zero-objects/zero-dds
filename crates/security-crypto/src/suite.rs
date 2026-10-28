// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crypto suite selection (AES-GCM and AES-GMAC, 128 / 256).
//!
//! OMG DDS-Security 1.2 §10.5.2 Table 77 defines the `CryptoTransform-
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

use crate::backend::aead;

/// `CryptoTransformKind` constants as low-byte IDs (spec-1.2 §10.5
/// Tab.79). On the wire it is the 4-byte big-endian array
/// `[0, 0, 0, ID]`.
///
/// `NONE` is no `Suite` variant — the constant is still here for decoder
/// paths that must recognize the spec values ("supported" vs "known but
/// not implemented").
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
/// The default (`AesGcmCryptoPlugin::new()`, `Suite::default()`) is
/// `Aes256Gcm`: DDS-Security 1.2 §10.5.1.1 reads a missing or `AUTO`
/// `symmetric_cipher_algorithm` as `AES256-GCM`. `Aes128Gcm` is chosen with
/// `AesGcmCryptoPlugin::with_suite(Suite::Aes128Gcm)` or the property.
///
/// The two GMAC suites are **auth-only** (no confidentiality), for a
/// `*_protection_kind=SIGN` — the payload stays plain, authenticated by a
/// 16-byte AES-GMAC tag.
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
    /// AES-128 in GMAC mode (auth-only, spec kind 0x01). 16-byte master
    /// key, 16-byte AES-GMAC tag (DDS-Security 1.2 §10.5.2 Table 77). The
    /// slot carried HMAC-SHA256, and an AES128-GMAC tag of another vendor
    /// was checked with HMAC.
    Aes128Gmac,
}

impl Suite {
    /// Required master-key length in bytes.
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            Self::Aes128Gcm | Self::Aes128Gmac => 16,
            Self::Aes256Gcm | Self::Aes256Gmac => 32,
        }
    }

    /// The AES-GCM algorithm of the suite; GMAC is AES-GCM over an empty
    /// plaintext with the data as additional authenticated data.
    #[must_use]
    pub(crate) fn algorithm(self) -> Option<&'static aead::Algorithm> {
        match self {
            Self::Aes128Gcm | Self::Aes128Gmac => Some(&aead::AES_128_GCM),
            Self::Aes256Gcm | Self::Aes256Gmac => Some(&aead::AES_256_GCM),
        }
    }

    /// `true` if the suite provides confidentiality (otherwise auth-only).
    #[must_use]
    pub const fn is_aead(self) -> bool {
        matches!(self, Self::Aes128Gcm | Self::Aes256Gcm)
    }

    /// One-byte transform-kind ID for the wire format (SEC_PREFIX).
    /// Spec DDS-Security 1.2 §10.5.2 Table 77 — we return the low-byte
    /// part; the wire codec packs it into `[0, 0, 0, id]` BE.
    ///
    #[must_use]
    pub fn transform_kind_id(self) -> u8 {
        match self {
            Self::Aes128Gcm => transform_kind::AES128_GCM, // 0x02
            Self::Aes256Gcm => transform_kind::AES256_GCM, // 0x04
            Self::Aes256Gmac => transform_kind::AES256_GMAC, // 0x03
            Self::Aes128Gmac => transform_kind::AES128_GMAC, // 0x01
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
            transform_kind::AES128_GMAC => Some(Self::Aes128Gmac),
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

impl Suite {
    /// The suite of the same kind (GMAC or GCM) with keys of `bits` bits
    /// (128 or 256; any other value is 256).
    #[must_use]
    pub const fn with_key_bits(self, bits: u16) -> Self {
        match (self.is_aead(), bits) {
            (true, 128) => Self::Aes128Gcm,
            (true, _) => Self::Aes256Gcm,
            (false, 128) => Self::Aes128Gmac,
            (false, _) => Self::Aes256Gmac,
        }
    }
}

/// The key width the property `dds.sec.crypto.symmetric_cipher_algorithm`
/// (and `rtps_psk_symmetric_cipher_algorithm`) names (DDS-Security 1.2
/// §10.5.1 Table 71, §10.5.1.1): `AUTO`, or none, is `AES256-GCM`; the names
/// of Table 22 select 128 or 256 bits. The text of the specification spells
/// them `AES128+GCM` (the constants `CNAME_AES128_GMAC` and
/// `CNAME_AES128_GCM` are the same string) and `AES256-GCM`; both spellings
/// of the four names are taken, GMAC as well as GCM. Whether the mode is
/// authentication only or authenticated encryption follows the Protection
/// Kind (§10.5.3.1), not the name.
///
/// # Errors
/// `InvalidConfiguration` for any other value.
pub fn symmetric_cipher_key_bits(
    property: &str,
    value: Option<&str>,
) -> zerodds_security::error::SecurityResult<u16> {
    match value.map(str::trim) {
        None | Some("AUTO") => Ok(256),
        Some("AES256-GCM" | "AES256+GCM" | "AES256-GMAC" | "AES256+GMAC") => Ok(256),
        Some("AES128-GCM" | "AES128+GCM" | "AES128-GMAC" | "AES128+GMAC") => Ok(128),
        Some(other) => Err(zerodds_security::error::SecurityError::new(
            zerodds_security::error::SecurityErrorKind::InvalidConfiguration,
            alloc::format!(
                "{property} = '{other}' is AUTO, AES128+GCM, AES128-GMAC, AES256+GCM or AES256-GMAC"
            ),
        )),
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::Aes256Gcm
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn spec_constants_match_table_79() {
        // Spec DDS-Security 1.2 §10.5.2 Table 77 — the wire values must
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

    /// DDS-Security 1.2 §10.5.2 Table 77: 0x01 is AES128-GMAC, with a
    /// 16-octet key. The slot carried HMAC-SHA256.
    #[test]
    fn aes128_gmac_uses_spec_id_1() {
        assert_eq!(Suite::Aes128Gmac.transform_kind_id(), 0x01);
        assert_eq!(Suite::Aes128Gmac.key_len(), 16);
        assert!(!Suite::Aes128Gmac.is_aead());
    }

    #[test]
    fn from_id_roundtrip_for_all_supported() {
        assert_eq!(Suite::from_transform_kind_id(0x01), Some(Suite::Aes128Gmac));
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
        for s in [
            Suite::Aes128Gcm,
            Suite::Aes256Gcm,
            Suite::Aes128Gmac,
            Suite::Aes256Gmac,
        ] {
            let id = s.transform_kind_id();
            assert_eq!(Suite::from_transform_kind_id(id), Some(s));
        }
    }
}
