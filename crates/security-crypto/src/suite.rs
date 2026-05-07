// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crypto-Suite-Auswahl (AES-GCM 128 / 256).
//!
//! OMG DDS-Security 1.2 §10.5 Tab.79 definiert die `CryptoTransform-
//! Kind`-Konstanten — 4-byte Big-Endian-Werte:
//!
//! | Spec-Konstante                              | Hex-Low | Algorithmus       | Key |
//! |---------------------------------------------|--------:|-------------------|----:|
//! | `CRYPTO_TRANSFORMATION_KIND_NONE`           |  `0x00` | (kein Schutz)     |  -  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES128_GMAC`    |  `0x01` | AES-128 (GMAC)    | 16  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES128_GCM`     |  `0x02` | AES-128 (GCM)     | 16  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES256_GMAC`    |  `0x03` | AES-256 (GMAC)    | 32  |
//! | `CRYPTO_TRANSFORMATION_KIND_AES256_GCM`     |  `0x04` | AES-256 (GCM)     | 32  |
//!
//! Beide GCM-Varianten liefern Integrity + Confidentiality; die GMAC-
//! Varianten sind Auth-only (Payload bleibt plain, Tag wird angehaengt).
//!
//! **C3.6 Wire-Konflikt-Fix (2026-04-25):** vor diesem Commit hatte
//! `Suite::transform_kind_id()` ein nicht-spec-konformes Mapping
//! (Aes128Gcm=0x01, Aes256Gcm=0x02, HmacSha256=0x03), wodurch ein
//! Cyclone DDS-Receiver unsere Submessages mit "wrong algorithm"
//! abwies. Jetzt spec-konform — Wire-breaking Aenderung gegenueber
//! v0.x ZeroDDS-Peers.

use ring::aead;

/// `CryptoTransformKind`-Konstanten als Low-Byte-IDs (Spec-1.2 §10.5
/// Tab.79). Auf der Wire steht jeweils das 4-byte Big-Endian-Array
/// `[0, 0, 0, ID]`.
///
/// `NONE` und `AES256_GMAC` werden von ZeroDDS aktuell nicht als
/// `Suite`-Variante exposed — die Konstanten sind dennoch da fuer
/// Decoder-Pfade die Spec-Werte erkennen muessen ("supported" vs
/// "known but not implemented").
#[allow(dead_code)]
pub mod transform_kind {
    /// `CRYPTO_TRANSFORMATION_KIND_NONE` — kein Schutz.
    pub const NONE: u8 = 0x00;
    /// `CRYPTO_TRANSFORMATION_KIND_AES128_GMAC` — AES-128 Auth-only.
    pub const AES128_GMAC: u8 = 0x01;
    /// `CRYPTO_TRANSFORMATION_KIND_AES128_GCM` — AES-128 Auth+Encrypt.
    pub const AES128_GCM: u8 = 0x02;
    /// `CRYPTO_TRANSFORMATION_KIND_AES256_GMAC` — AES-256 Auth-only.
    pub const AES256_GMAC: u8 = 0x03;
    /// `CRYPTO_TRANSFORMATION_KIND_AES256_GCM` — AES-256 Auth+Encrypt.
    pub const AES256_GCM: u8 = 0x04;
}

/// Verfuegbare Crypto-Suites im `AesGcmCryptoPlugin`.
///
/// Default (`AesGcmCryptoPlugin::new()`) ist `Aes128Gcm` — leichtgewichtig,
/// ausreichend fuer < 10-Jahres-Vertraulichkeit. Fuer hoehere Stufen
/// `AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm)` waehlen.
///
/// `HmacSha256` ist **Auth-only** (keine Confidentiality). Wird
/// eingesetzt wenn Governance-XML `metadata_protection_kind=SIGN`
/// vorgibt — die Payload bleibt plain, aber per HMAC-Tag
/// authentifiziert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// AES-128 im GCM-Mode. 16-byte Master-Key. Auth+Encrypt.
    Aes128Gcm,
    /// AES-256 im GCM-Mode. 32-byte Master-Key. Auth+Encrypt.
    Aes256Gcm,
    /// HMAC-SHA256 Auth-only. 32-byte Master-Key. Payload bleibt
    /// plain; 32-byte-HMAC wird angehaengt (Spec-Kind
    /// `NONE + HMAC_SHA256` — "SIGN" im Governance-XML).
    HmacSha256,
}

impl Suite {
    /// Benötigte Master-Key-Laenge in Bytes.
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            Self::Aes128Gcm => 16,
            Self::Aes256Gcm | Self::HmacSha256 => 32,
        }
    }

    /// ring-Algorithm-Referenz fuer AEAD. Fuer `HmacSha256` gibts
    /// keine AEAD-Algo → Caller muss diese Suite separat handlen.
    #[must_use]
    pub(crate) fn algorithm(self) -> Option<&'static aead::Algorithm> {
        match self {
            Self::Aes128Gcm => Some(&aead::AES_128_GCM),
            Self::Aes256Gcm => Some(&aead::AES_256_GCM),
            Self::HmacSha256 => None,
        }
    }

    /// `true` wenn die Suite Confidentiality liefert (sonst Auth-only).
    #[must_use]
    pub const fn is_aead(self) -> bool {
        matches!(self, Self::Aes128Gcm | Self::Aes256Gcm)
    }

    /// Ein-Byte-Transform-Kind-Id fuers Wire-Format (SEC_PREFIX).
    /// Spec DDS-Security 1.2 §10.5 Tab.79 — wir liefern den Low-Byte-
    /// Anteil; der Wire-Codec packt das in `[0, 0, 0, id]` BE.
    ///
    /// `HmacSha256` belegt den `AES128_GMAC`-Slot (`0x01`) — die
    /// ZeroDDS-Implementation laeuft mit HMAC-SHA256 statt AES-GMAC,
    /// d.h. fuer SIGN-only-Topics ist der Cyclone-Interop noch nicht
    /// gegeben (siehe C3.7). GCM-Varianten sind voll spec-konform.
    #[must_use]
    pub fn transform_kind_id(self) -> u8 {
        match self {
            Self::Aes128Gcm => transform_kind::AES128_GCM, // 0x02
            Self::Aes256Gcm => transform_kind::AES256_GCM, // 0x04
            Self::HmacSha256 => transform_kind::AES128_GMAC, // 0x01 (Slot)
        }
    }

    /// 4-byte `CryptoTransformKind` Big-Endian-Wire-Repraesentation
    /// (Spec §10.5 Tab.79).
    #[must_use]
    pub fn transform_kind(self) -> [u8; 4] {
        [0, 0, 0, self.transform_kind_id()]
    }

    /// Inverse von [`Self::transform_kind_id`]. Liefert `None` fuer
    /// Spec-Werte die ZeroDDS nicht als Suite anbietet (z.B.
    /// `AES256_GMAC = 0x03`).
    #[must_use]
    pub fn from_transform_kind_id(id: u8) -> Option<Self> {
        match id {
            transform_kind::AES128_GMAC => Some(Self::HmacSha256),
            transform_kind::AES128_GCM => Some(Self::Aes128Gcm),
            transform_kind::AES256_GCM => Some(Self::Aes256Gcm),
            _ => None,
        }
    }

    /// Maximum Encrypts pro Key, bevor Key-Refresh noetig wird. Spec
    /// §9.5.3.3.4 empfiehlt ≤ 2^32 fuer GCM — wir cappen bei 2^48
    /// (konservativ unter Soft-Limit, viel unter harter Nonce-Bound).
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
        // Spec DDS-Security 1.2 §10.5 Tab.79 — Wire-Werte muessen
        // unveraenderlich sein. Cyclone DDS / FastDDS verlassen sich
        // auf genau diese Bytes.
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
        // ZeroDDS-Drift: Implementation ist HMAC-SHA256, der Wire-Slot
        // ist aber AES128_GMAC=0x01. SIGN-only-Cross-Vendor (z.B.
        // Cyclone) ist hier noch nicht spec-konform; siehe C3.7.
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
        // 0x00 NONE — wir bieten keine None-Suite an
        assert!(Suite::from_transform_kind_id(0x00).is_none());
        // 0x03 AES256_GMAC — Spec-konform, aber ZeroDDS bietet sie nicht
        assert!(Suite::from_transform_kind_id(0x03).is_none());
        // Reservierter / unbekannter Wert
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
