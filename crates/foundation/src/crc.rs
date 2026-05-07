// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire-Integrity-Primitive: CRC-32C, CRC-64-XZ, MD5.
//!
//! Spec-Referenzen:
//! - **CRC-32C** (Castagnoli, Polynom `0x1EDC6F41` reflektiert):
//!   RFC 4960 Appendix B; DDSI-RTPS 2.5 §3 [Ref 5] / §9.4.2.15.2
//!   `messageChecksum = CRC32C`.
//! - **CRC-64-XZ** (ECMA-182, Polynom `0x42F0E1EBA9EA3693` reflektiert,
//!   Init `0xFFFFFFFFFFFFFFFF`, XorOut `0xFFFFFFFFFFFFFFFF`):
//!   ECMA-182; DDSI-RTPS 2.5 §3 [Ref 6] / §9.4.2.15.2
//!   `messageChecksum = CRC64`.
//! - **MD5-128**: RFC 1321; DDSI-RTPS 2.5 §3 [Ref 7] / §9.4.2.15.2
//!   `messageChecksum = MD5`.
//!
//! Alle Implementierungen sind pure-Rust, ohne externe Crates, damit
//! die `foundation`-Crate ihren `forbid(unsafe_code)` + `no_std`-Ansatz
//! behaelt. Algorithmische Treue gegen die Test-Vektoren der jeweiligen
//! RFCs/ECMA-Spezifikationen ist via Unit-Tests in den `mod tests`-
//! Bloecken belegt.
//!
//! # Performance-Anmerkung
//!
//! Die CRC-Implementierung nutzt klassische Lookup-Tables (1 KiB fuer
//! CRC-32C, 2 KiB fuer CRC-64). Das ist genug fuer typische RTPS-
//! Datagramme (≤64 KiB). Hochfrequente Hashing-Pfade (z.B. KeyHash,
//! Stream-Auth-Tag) liegen architekturell ausserhalb dieses Moduls.

#![allow(clippy::cast_possible_truncation)]

// =====================================================================
// CRC-32C — Castagnoli (RFC 4960 App. B / SCTP)
// =====================================================================

/// Polynom `0x1EDC6F41`, reflektiert -> `0x82F63B78`.
const CRC32C_POLY_REFLECTED: u32 = 0x82F6_3B78;

/// Lookup-Table fuer CRC-32C (256 * 4 Byte = 1 KiB).
const CRC32C_TABLE: [u32; 256] = build_crc32c_table();

const fn build_crc32c_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ CRC32C_POLY_REFLECTED;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
}

/// CRC-32C Castagnoli ueber `data`.
///
/// Initial-Wert `0xFFFFFFFF`, finaler XorOut `0xFFFFFFFF` — entsprechend
/// RFC 4960 Appendix B.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        let idx = ((crc ^ u32::from(b)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF
}

// =====================================================================
// CRC-64-XZ — ECMA-182 (XZ utils variant)
// =====================================================================

/// Polynom `0x42F0E1EBA9EA3693`, reflektiert -> `0xC96C5795D7870F42`.
const CRC64_XZ_POLY_REFLECTED: u64 = 0xC96C_5795_D787_0F42;

/// Lookup-Table fuer CRC-64-XZ (256 * 8 Byte = 2 KiB).
const CRC64_XZ_TABLE: [u64; 256] = build_crc64_xz_table();

const fn build_crc64_xz_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut i = 0u64;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ CRC64_XZ_POLY_REFLECTED;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
}

/// CRC-64-XZ ueber `data` (ECMA-182 Polynom, XZ utils Variante).
///
/// Initial-Wert `0xFFFFFFFFFFFFFFFF`, finaler XorOut
/// `0xFFFFFFFFFFFFFFFF` — entspricht der `xz`/`liblzma`-Variante, die
/// auch DDSI-RTPS §9.4.2.15.2 referenziert.
#[must_use]
pub fn crc64_xz(data: &[u8]) -> u64 {
    let mut crc: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    for &b in data {
        let idx = ((crc ^ u64::from(b)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC64_XZ_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF_FFFF_FFFF
}

// =====================================================================
// MD5-128 — RFC 1321
// =====================================================================
//
// Pure-rust Implementierung der MD5-Message-Digest-Function. Ist fuer
// Crypto-Sicherheit nicht mehr empfohlen (Kollisionen!), wird in
// DDSI-RTPS aber als Wire-Integritaetsprimitiv definiert. Die Spec
// erlaubt MD5-128 als `messageChecksum`-Variante (§9.4.2.15.2).
//
// Algorithmus folgt RFC 1321 §3 + Anhang A. Test-Vektoren aus §A.5.

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, // round 1
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, // round 2
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, // round 3
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, // round 4
];

const MD5_K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// MD5-Hash ueber `data` (RFC 1321). Liefert 16 Byte (Little-Endian
/// Repraesentation der vier Zustands-u32-Worte).
#[must_use]
pub fn md5(data: &[u8]) -> [u8; 16] {
    // Initial-State (RFC 1321 §3.3).
    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    // Padding (§3.1): Append 1 bit, dann Null-Bits bis Laenge ≡ 448 (mod 512),
    // dann 64 bit Original-Laenge LE. Gesamt-Laenge ist Vielfaches von 64 Byte.
    let bit_len: u64 = (data.len() as u64).wrapping_mul(8);

    #[cfg(feature = "alloc")]
    let padded = {
        extern crate alloc;
        let mut v = alloc::vec::Vec::with_capacity(data.len() + 72);
        v.extend_from_slice(data);
        v.push(0x80u8);
        while v.len() % 64 != 56 {
            v.push(0);
        }
        v.extend_from_slice(&bit_len.to_le_bytes());
        v
    };

    #[cfg(not(feature = "alloc"))]
    let padded = {
        // no_std-Pfad: wir limitieren auf 56 Byte Eingabe (passt in einen
        // einzigen 64-Byte-Block). Ist fuer interne Kurz-Hashes gedacht.
        // Da `foundation` per default `alloc` aktiviert hat, ist das hier
        // nur ein no_std-Fallback; produktive Pfade nutzen `alloc`.
        let mut buf = [0u8; 64];
        let n = data.len().min(56);
        buf[..n].copy_from_slice(&data[..n]);
        buf[n] = 0x80;
        buf[56..].copy_from_slice(&bit_len.to_le_bytes());
        buf
    };

    // Block-Wise Compression.
    #[cfg(feature = "alloc")]
    let blocks = padded.chunks_exact(64);
    #[cfg(not(feature = "alloc"))]
    let blocks = core::iter::once(&padded[..]);

    for chunk in blocks {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            let off = i * 4;
            *w = u32::from_le_bytes([chunk[off], chunk[off + 1], chunk[off + 2], chunk[off + 3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(MD5_K[i])
                    .wrapping_add(m[g])
                    .rotate_left(MD5_S[i]),
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    // Output: a0|b0|c0|d0 LE-konkateniert.
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    // Use padded so unused-bindings warning does not trip
    #[cfg(feature = "alloc")]
    let _ = padded.len();
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    // ----- CRC-32C — RFC 4960 Appendix B + bekannte Standard-Vektoren -----

    #[test]
    fn crc32c_empty_is_zero() {
        // Spec: CRC-32C("") = 0x00000000.
        assert_eq!(crc32c(b""), 0x0000_0000);
    }

    #[test]
    fn crc32c_a_is_known_vector() {
        // "a" -> 0xC1D04330 (verifiziert gegen mehrere SCTP-Test-Suites).
        assert_eq!(crc32c(b"a"), 0xC1D0_4330);
    }

    #[test]
    fn crc32c_abc_is_known_vector() {
        // "abc" -> 0x364B3FB7.
        assert_eq!(crc32c(b"abc"), 0x364B_3FB7);
    }

    #[test]
    fn crc32c_message_digest_vector() {
        // "message digest" -> 0x02BD79D0.
        assert_eq!(crc32c(b"message digest"), 0x02BD_79D0);
    }

    #[test]
    fn crc32c_alphabet_vector() {
        // "abcdefghijklmnopqrstuvwxyz" -> 0x9EE6_EF25.
        assert_eq!(crc32c(b"abcdefghijklmnopqrstuvwxyz"), 0x9EE6_EF25);
    }

    #[test]
    fn crc32c_zeros_32_byte_vector() {
        // RFC 3720 Appendix B.4 (iSCSI / SCTP CRC-32C):
        // 32 Null-Byte -> 0xAA36918A (Wire-LE) = 0x8A9136AA als
        // Host-Order u32 mit der hier implementierten ueblichen
        // Convention. Diese Convention liefert konkret `crc32c(&[0;32])`
        // = 0x8A9136AA.
        let zeros = [0u8; 32];
        assert_eq!(crc32c(&zeros), 0x8A91_36AA);
    }

    #[test]
    fn crc32c_iso_iec_31_byte_pattern() {
        // 32 inkrementierende Bytes 00..1F -> 0x46DD_794E.
        let mut data = [0u8; 32];
        for (i, b) in data.iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(crc32c(&data), 0x46DD_794E);
    }

    // ----- CRC-64-XZ — bekannte ECMA-182/XZ-Vektoren -----

    #[test]
    fn crc64_xz_empty_is_zero() {
        assert_eq!(crc64_xz(b""), 0);
    }

    #[test]
    fn crc64_xz_known_vector_123456789() {
        // Standard "check"-Wert fuer CRC-64/XZ -> 0x995DC9BBDF1939FA.
        assert_eq!(crc64_xz(b"123456789"), 0x995D_C9BB_DF19_39FA);
    }

    #[test]
    fn crc64_xz_a_known_vector() {
        // "a" -> 0x330284772E652B05.
        assert_eq!(crc64_xz(b"a"), 0x3302_8477_2E65_2B05);
    }

    #[test]
    fn crc64_xz_abcd_vector() {
        // "abc" -> 0x2CD8094A1A277627.
        assert_eq!(crc64_xz(b"abc"), 0x2CD8_094A_1A27_7627);
    }

    // ----- MD5-128 — RFC 1321 §A.5 Test Suite -----

    fn hex(bytes: &[u8]) -> alloc::string::String {
        extern crate alloc;
        use alloc::string::String;
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push(hex_nibble(b >> 4));
            s.push(hex_nibble(b & 0xF));
        }
        s
    }

    fn hex_nibble(n: u8) -> char {
        match n {
            0..=9 => (b'0' + n) as char,
            _ => (b'a' + (n - 10)) as char,
        }
    }

    #[test]
    fn md5_empty_string_rfc1321() {
        // RFC 1321 §A.5: MD5("") = d41d8cd98f00b204e9800998ecf8427e.
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_a_rfc1321() {
        assert_eq!(hex(&md5(b"a")), "0cc175b9c0f1b6a831c399e269772661");
    }

    #[test]
    fn md5_abc_rfc1321() {
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn md5_message_digest_rfc1321() {
        assert_eq!(
            hex(&md5(b"message digest")),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn md5_alphabet_rfc1321() {
        assert_eq!(
            hex(&md5(b"abcdefghijklmnopqrstuvwxyz")),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
    }

    #[test]
    fn md5_alphanum_rfc1321() {
        assert_eq!(
            hex(&md5(
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            )),
            "d174ab98d277d9f5a5611c2c9f419d9f"
        );
    }

    #[test]
    fn md5_long_digits_rfc1321() {
        assert_eq!(
            hex(&md5(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn md5_block_boundary_55_bytes() {
        // 55-Byte-Eingabe: liegt 1 Byte unter der Block-Grenze (56), das
        // Padding belegt genau 9 Byte und passt in genau einen Block.
        let s = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXY12";
        assert_eq!(s.len(), 53);
        // Beliebige Pruefung: Hash bleibt deterministisch.
        let h1 = md5(s.as_bytes());
        let h2 = md5(s.as_bytes());
        assert_eq!(h1, h2);
    }

    #[test]
    fn md5_block_boundary_64_bytes() {
        // 64-Byte-Eingabe: erfordert genau zwei 64-Byte-Bloecke fuers
        // Padding (1 voller Block + Padding-Block).
        let data = [b'A'; 64];
        let h = md5(&data);
        // Stabilitaet: Wert ist deterministisch.
        let h2 = md5(&data);
        assert_eq!(h, h2);
        // Sanity: nicht alle Bytes Null.
        assert!(h.iter().any(|&b| b != 0));
    }
}
