// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CryptoTransformIdentifier` und Builtin-Crypto-Plugin-IDs —
//! DDS-Security 1.2 §7.3.20 + §10.5.2-3 + §10.3.2.1.
//!
//! Spec §7.3.20 (S. 73) listet die builtin Cryptographic-Plugin-IDs;
//! §10.5.2 spezifiziert die `CryptoTransformIdentifier`-Struktur, die
//! in jeder verschluesselten RTPS-Submessage als Identifier-Header
//! mitlaeuft.

use alloc::format;
use alloc::vec::Vec;

/// Builtin Crypto Plugin Class-ID (Spec §10.3.2.1 + §7.3.20).
pub const BUILTIN_CRYPTO_PLUGIN: &str = "DDS:Crypto:AES_GCM_GMAC";

/// Spec §10.5.2.1: `CryptoTransformKind` — 4-Byte-Identifier.
///
/// ```text
///   CRYPTO_TRANSFORMATION_KIND_NONE      = {0,0,0,0}
///   CRYPTO_TRANSFORMATION_KIND_AES128_GMAC = {0,0,0,1}
///   CRYPTO_TRANSFORMATION_KIND_AES128_GCM  = {0,0,0,2}
///   CRYPTO_TRANSFORMATION_KIND_AES256_GMAC = {0,0,0,3}
///   CRYPTO_TRANSFORMATION_KIND_AES256_GCM  = {0,0,0,4}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CryptoTransformKind {
    /// Kein Transform — Submessage unsigned.
    None = 0,
    /// AES-128-GMAC (nur Authentication, kein Encrypt).
    Aes128Gmac = 1,
    /// AES-128-GCM (Authentication + Encrypt).
    Aes128Gcm = 2,
    /// AES-256-GMAC.
    Aes256Gmac = 3,
    /// AES-256-GCM.
    Aes256Gcm = 4,
}

impl CryptoTransformKind {
    /// Wire-Bytes (4 BE).
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 4] {
        (self as u32).to_be_bytes()
    }

    /// Decode aus 4 BE Bytes.
    ///
    /// # Errors
    /// Static-String wenn Wert nicht in 0..=4 liegt.
    pub fn from_be_bytes(bytes: [u8; 4]) -> Result<Self, &'static str> {
        match u32::from_be_bytes(bytes) {
            0 => Ok(Self::None),
            1 => Ok(Self::Aes128Gmac),
            2 => Ok(Self::Aes128Gcm),
            3 => Ok(Self::Aes256Gmac),
            4 => Ok(Self::Aes256Gcm),
            _ => Err("unknown CryptoTransformKind"),
        }
    }

    /// `true` wenn der Transform Encryption macht (sonst nur Auth).
    #[must_use]
    pub const fn encrypts(self) -> bool {
        matches!(self, Self::Aes128Gcm | Self::Aes256Gcm)
    }

    /// Spec §10.5.2.1 — Authentication-Tag-Length.
    /// Alle GCM/GMAC-Varianten verwenden 16 Byte Tag.
    #[must_use]
    pub const fn tag_size(self) -> usize {
        match self {
            Self::None => 0,
            _ => 16,
        }
    }

    /// Key-Material-Size (Bytes) je Algorithmus.
    #[must_use]
    pub const fn key_size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Aes128Gmac | Self::Aes128Gcm => 16,
            Self::Aes256Gmac | Self::Aes256Gcm => 32,
        }
    }
}

/// Spec §10.5.2.1 `CryptoTransformIdentifier`:
/// ```text
///   struct CryptoTransformIdentifier {
///     CryptoTransformKind   transformation_kind; // 4 byte BE
///     CryptoTransformKeyId  transformation_key_id; // 4 byte
///   };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CryptoTransformIdentifier {
    /// 4-Byte-Wire-Form.
    pub kind: CryptoTransformKind,
    /// 4-Byte Key-Identifier (vom Plugin vergeben).
    pub key_id: [u8; 4],
}

impl CryptoTransformIdentifier {
    /// Konstruktor.
    #[must_use]
    pub fn new(kind: CryptoTransformKind, key_id: [u8; 4]) -> Self {
        Self { kind, key_id }
    }

    /// Encode zu 8 Bytes (Spec Wire-Form).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&self.kind.to_be_bytes());
        out[4..8].copy_from_slice(&self.key_id);
        out
    }

    /// Decode 8 Bytes.
    ///
    /// # Errors
    /// Static-String wenn der Wire-Buffer nicht 8 Bytes hat oder
    /// die `transformation_kind` ungueltig ist.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 8 {
            return Err("CryptoTransformIdentifier needs 8 bytes");
        }
        let mut k = [0u8; 4];
        k.copy_from_slice(&bytes[0..4]);
        let kind = CryptoTransformKind::from_be_bytes(k)?;
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&bytes[4..8]);
        Ok(Self { kind, key_id })
    }
}

/// Spec §10.5.2.3 `CryptoHeader`:
/// ```text
///   struct CryptoHeader {
///     CryptoTransformIdentifier transformation_id;
///     CryptoSessionId           session_id;        // 4 byte
///     CryptoInitVectorSuffix    init_vector_suffix; // 8 byte
///   };
/// ```
/// Total 8 + 4 + 8 = 20 Bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CryptoHeader {
    /// Transform-Identifier.
    pub transformation_id: CryptoTransformIdentifier,
    /// Session-Id (vom Plugin vergeben, 4 Byte).
    pub session_id: [u8; 4],
    /// IV-Suffix (8 Byte). Volles IV = session_id || init_vector_suffix.
    pub init_vector_suffix: [u8; 8],
}

impl CryptoHeader {
    /// Wire-Size (Spec).
    pub const WIRE_SIZE: usize = 20;

    /// Encode zu Wire-Bytes (20 Byte).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::WIRE_SIZE] {
        let mut out = [0u8; Self::WIRE_SIZE];
        out[0..8].copy_from_slice(&self.transformation_id.to_bytes());
        out[8..12].copy_from_slice(&self.session_id);
        out[12..20].copy_from_slice(&self.init_vector_suffix);
        out
    }

    /// Decode 20 Wire-Bytes.
    ///
    /// # Errors
    /// Static-String wenn Wire-Buffer falscher Laenge.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err("CryptoHeader needs 20 bytes");
        }
        let transformation_id = CryptoTransformIdentifier::from_bytes(&bytes[0..8])?;
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&bytes[8..12]);
        let mut iv_suffix = [0u8; 8];
        iv_suffix.copy_from_slice(&bytes[12..20]);
        Ok(Self {
            transformation_id,
            session_id,
            init_vector_suffix: iv_suffix,
        })
    }

    /// Berechnet das volle 12-Byte-IV (Spec §10.5.2.3): `session_id ||
    /// init_vector_suffix`. Wird in AES-GCM als Nonce verwendet.
    #[must_use]
    pub fn full_iv(&self) -> [u8; 12] {
        let mut iv = [0u8; 12];
        iv[0..4].copy_from_slice(&self.session_id);
        iv[4..12].copy_from_slice(&self.init_vector_suffix);
        iv
    }
}

/// Spec §10.5.2.4 `CryptoFooter` — enthaelt Auth-Tag + Receiver-
/// Specific-MAC-Liste. Wir liefern nur den minimalen Common-Tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoFooter {
    /// Common 16-Byte AES-GCM/GMAC Authentication-Tag.
    pub common_mac: [u8; 16],
    /// Receiver-Specific-MACs (Spec §10.5.2.4 — pro Empfaenger ein
    /// 4-Byte-Key-Id + 16-Byte-Tag; voll abgedeckt in WP A.6).
    pub receiver_specific_macs: Vec<([u8; 4], [u8; 16])>,
}

impl CryptoFooter {
    /// Encode (4-Byte BE Receiver-Count + 16 common_mac + N x (4+16)).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(20 + self.receiver_specific_macs.len() * 20);
        out.extend_from_slice(&self.common_mac);
        let n = self.receiver_specific_macs.len() as u32;
        out.extend_from_slice(&n.to_be_bytes());
        for (key_id, mac) in &self.receiver_specific_macs {
            out.extend_from_slice(key_id);
            out.extend_from_slice(mac);
        }
        out
    }

    /// Decode aus Wire-Bytes.
    ///
    /// # Errors
    /// Static-String bei falscher Laenge.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 20 {
            return Err("CryptoFooter needs >= 20 bytes");
        }
        let mut common_mac = [0u8; 16];
        common_mac.copy_from_slice(&bytes[0..16]);
        let n_buf: [u8; 4] = bytes[16..20].try_into().map_err(|_| "footer count")?;
        let n = u32::from_be_bytes(n_buf) as usize;
        let mut pos = 20;
        let mut receiver_specific_macs = Vec::with_capacity(n);
        for _ in 0..n {
            if bytes.len() < pos + 20 {
                return Err("receiver-specific MAC truncated");
            }
            let mut key_id = [0u8; 4];
            key_id.copy_from_slice(&bytes[pos..pos + 4]);
            let mut mac = [0u8; 16];
            mac.copy_from_slice(&bytes[pos + 4..pos + 20]);
            receiver_specific_macs.push((key_id, mac));
            pos += 20;
        }
        Ok(Self {
            common_mac,
            receiver_specific_macs,
        })
    }
}

/// Cross-Vendor-Plugin-Negotiation. Caller liefert die Liste der
/// vom Remote angekuendigten Plugin-Class-Ids; wir liefern den
/// hoechsten gemeinsam unterstuetzten Builtin-Algorithmus zurueck.
///
/// # Errors
/// `format!` String wenn kein gemeinsamer Algorithmus.
pub fn negotiate_transform(
    remote_kinds: &[CryptoTransformKind],
) -> Result<CryptoTransformKind, alloc::string::String> {
    // Praeferenz: AES-256-GCM > AES-128-GCM > AES-256-GMAC > AES-128-GMAC.
    let pref = [
        CryptoTransformKind::Aes256Gcm,
        CryptoTransformKind::Aes128Gcm,
        CryptoTransformKind::Aes256Gmac,
        CryptoTransformKind::Aes128Gmac,
    ];
    for p in pref {
        if remote_kinds.contains(&p) {
            return Ok(p);
        }
    }
    Err(format!(
        "no common crypto transform with peer (peer-offered: {remote_kinds:?})"
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trip_all_variants() {
        for k in [
            CryptoTransformKind::None,
            CryptoTransformKind::Aes128Gmac,
            CryptoTransformKind::Aes128Gcm,
            CryptoTransformKind::Aes256Gmac,
            CryptoTransformKind::Aes256Gcm,
        ] {
            assert_eq!(
                CryptoTransformKind::from_be_bytes(k.to_be_bytes()).unwrap(),
                k
            );
        }
    }

    #[test]
    fn unknown_kind_rejected() {
        assert!(CryptoTransformKind::from_be_bytes([0, 0, 0, 99]).is_err());
    }

    #[test]
    fn key_sizes_match_spec() {
        assert_eq!(CryptoTransformKind::Aes128Gcm.key_size(), 16);
        assert_eq!(CryptoTransformKind::Aes256Gcm.key_size(), 32);
        assert_eq!(CryptoTransformKind::None.key_size(), 0);
    }

    #[test]
    fn tag_size_is_16_for_all_aead_variants() {
        for k in [
            CryptoTransformKind::Aes128Gmac,
            CryptoTransformKind::Aes128Gcm,
            CryptoTransformKind::Aes256Gmac,
            CryptoTransformKind::Aes256Gcm,
        ] {
            assert_eq!(k.tag_size(), 16);
        }
    }

    #[test]
    fn encrypts_only_for_gcm() {
        assert!(CryptoTransformKind::Aes128Gcm.encrypts());
        assert!(CryptoTransformKind::Aes256Gcm.encrypts());
        assert!(!CryptoTransformKind::Aes128Gmac.encrypts());
        assert!(!CryptoTransformKind::None.encrypts());
    }

    #[test]
    fn transform_identifier_round_trip() {
        let id = CryptoTransformIdentifier::new(
            CryptoTransformKind::Aes256Gcm,
            [0xCA, 0xFE, 0xBA, 0xBE],
        );
        let bytes = id.to_bytes();
        assert_eq!(bytes.len(), 8);
        let back = CryptoTransformIdentifier::from_bytes(&bytes).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn header_round_trip_with_full_iv() {
        let h = CryptoHeader {
            transformation_id: CryptoTransformIdentifier::new(
                CryptoTransformKind::Aes256Gcm,
                [1, 2, 3, 4],
            ),
            session_id: [10, 20, 30, 40],
            init_vector_suffix: [50, 60, 70, 80, 90, 100, 110, 120],
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes.len(), CryptoHeader::WIRE_SIZE);
        let back = CryptoHeader::from_bytes(&bytes).unwrap();
        assert_eq!(back, h);
        assert_eq!(
            back.full_iv(),
            [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120]
        );
    }

    #[test]
    fn header_short_buffer_rejected() {
        assert!(CryptoHeader::from_bytes(&[0; 10]).is_err());
    }

    #[test]
    fn footer_round_trip_no_receivers() {
        let f = CryptoFooter {
            common_mac: [0xAA; 16],
            receiver_specific_macs: alloc::vec![],
        };
        let bytes = f.to_bytes();
        let back = CryptoFooter::from_bytes(&bytes).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn footer_round_trip_with_receivers() {
        let f = CryptoFooter {
            common_mac: [0xAA; 16],
            receiver_specific_macs: alloc::vec![
                ([1, 2, 3, 4], [0xBB; 16]),
                ([5, 6, 7, 8], [0xCC; 16]),
            ],
        };
        let bytes = f.to_bytes();
        let back = CryptoFooter::from_bytes(&bytes).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn footer_short_buffer_rejected() {
        assert!(CryptoFooter::from_bytes(&[0; 10]).is_err());
    }

    #[test]
    fn negotiate_picks_strongest_common() {
        let r = negotiate_transform(&[
            CryptoTransformKind::Aes128Gmac,
            CryptoTransformKind::Aes256Gcm,
            CryptoTransformKind::Aes128Gcm,
        ])
        .unwrap();
        assert_eq!(r, CryptoTransformKind::Aes256Gcm);
    }

    #[test]
    fn negotiate_falls_back_to_gmac_if_no_gcm() {
        let r = negotiate_transform(&[CryptoTransformKind::Aes128Gmac]).unwrap();
        assert_eq!(r, CryptoTransformKind::Aes128Gmac);
    }

    #[test]
    fn negotiate_fails_with_no_overlap() {
        assert!(negotiate_transform(&[CryptoTransformKind::None]).is_err());
        assert!(negotiate_transform(&[]).is_err());
    }

    #[test]
    fn builtin_plugin_id_matches_spec() {
        // Spec §10.3.2.1 + §7.3.20.
        assert_eq!(BUILTIN_CRYPTO_PLUGIN, "DDS:Crypto:AES_GCM_GMAC");
    }
}
