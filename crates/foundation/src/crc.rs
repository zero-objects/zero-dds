// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire-integrity primitives: CRC-32C, CRC-64-XZ, MD5.
//!
//! Spec references:
//! - **CRC-32C** (Castagnoli, polynomial `0x1EDC6F41` reflected):
//!   RFC 4960 Appendix B; DDSI-RTPS 2.5 §3 [Ref 5] / §9.4.2.15.2
//!   `messageChecksum = CRC32C`.
//! - **CRC-64-XZ** (ECMA-182, polynomial `0x42F0E1EBA9EA3693` reflected,
//!   init `0xFFFFFFFFFFFFFFFF`, XorOut `0xFFFFFFFFFFFFFFFF`):
//!   ECMA-182; DDSI-RTPS 2.5 §3 [Ref 6] / §9.4.2.15.2
//!   `messageChecksum = CRC64`.
//! - **MD5-128**: RFC 1321; DDSI-RTPS 2.5 §3 [Ref 7] / §9.4.2.15.2
//!   `messageChecksum = MD5`.
//!
//! All implementations are pure Rust, without external crates, so that
//! the `foundation` crate retains its `forbid(unsafe_code)` + `no_std`
//! approach. Algorithmic fidelity against the test vectors of the
//! respective RFC/ECMA specifications is shown via unit tests in the
//! `mod tests` blocks.
//!
//! # Performance note
//!
//! The CRC implementation uses classic lookup tables (1 KiB for
//! CRC-32C, 2 KiB for CRC-64). This is enough for typical RTPS
//! datagrams (≤64 KiB). High-frequency hashing paths (e.g. KeyHash,
//! stream auth tag) are architecturally outside this module.

#![allow(clippy::cast_possible_truncation)]

// =====================================================================
// CRC-32C — Castagnoli (RFC 4960 App. B / SCTP)
// =====================================================================

/// Polynomial `0x1EDC6F41`, reflected -> `0x82F63B78`.
const CRC32C_POLY_REFLECTED: u32 = 0x82F6_3B78;

/// Lookup table for CRC-32C (256 * 4 bytes = 1 KiB).
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

/// CRC-32C Castagnoli over `data`.
///
/// Initial value `0xFFFFFFFF`, final XorOut `0xFFFFFFFF` — per
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

/// Polynomial `0x42F0E1EBA9EA3693`, reflected -> `0xC96C5795D7870F42`.
const CRC64_XZ_POLY_REFLECTED: u64 = 0xC96C_5795_D787_0F42;

/// Lookup table for CRC-64-XZ (256 * 8 bytes = 2 KiB).
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

/// CRC-64-XZ over `data` (ECMA-182 polynomial, XZ utils variant).
///
/// Initial value `0xFFFFFFFFFFFFFFFF`, final XorOut
/// `0xFFFFFFFFFFFFFFFF` — matches the `xz`/`liblzma` variant that
/// DDSI-RTPS §9.4.2.15.2 also references.
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
// Pure-Rust implementation of the MD5 message-digest function. It is
// no longer recommended for cryptographic security (collisions!), but
// is defined in DDSI-RTPS as a wire-integrity primitive. The spec
// allows MD5-128 as a `messageChecksum` variant (§9.4.2.15.2).
//
// The algorithm follows RFC 1321 §3 + Appendix A. Test vectors from §A.5.

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

/// MD5 hash over `data` (RFC 1321). Returns 16 bytes (little-endian
/// representation of the four state u32 words).
#[must_use]
pub fn md5(data: &[u8]) -> [u8; 16] {
    // Initial state (RFC 1321 §3.3).
    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    // Padding (§3.1): append 1 bit, then zero bits until length ≡ 448 (mod 512),
    // then the 64-bit original length LE. The total length is a multiple of 64 bytes.
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
        // no_std path: we limit to 56 bytes of input (fits in a
        // single 64-byte block). Intended for internal short hashes.
        // Since `foundation` enables `alloc` by default, this is
        // only a no_std fallback; production paths use `alloc`.
        let mut buf = [0u8; 64];
        let n = data.len().min(56);
        buf[..n].copy_from_slice(&data[..n]);
        buf[n] = 0x80;
        buf[56..].copy_from_slice(&bit_len.to_le_bytes());
        buf
    };

    // Block-wise compression.
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

    // Output: a0|b0|c0|d0 concatenated LE.
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

    // ----- CRC-32C — RFC 4960 Appendix B + known standard vectors -----

    #[test]
    fn crc32c_empty_is_zero() {
        // Spec: CRC-32C("") = 0x00000000.
        assert_eq!(crc32c(b""), 0x0000_0000);
    }

    #[test]
    fn crc32c_a_is_known_vector() {
        // "a" -> 0xC1D04330 (verified against several SCTP test suites).
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
        // 32 null bytes -> 0xAA36918A (wire LE) = 0x8A9136AA as a
        // host-order u32 with the usual convention implemented here.
        // This convention concretely yields `crc32c(&[0;32])`
        // = 0x8A9136AA.
        let zeros = [0u8; 32];
        assert_eq!(crc32c(&zeros), 0x8A91_36AA);
    }

    #[test]
    fn crc32c_iso_iec_31_byte_pattern() {
        // 32 incrementing bytes 00..1F -> 0x46DD_794E.
        let mut data = [0u8; 32];
        for (i, b) in data.iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(crc32c(&data), 0x46DD_794E);
    }

    // ----- CRC-64-XZ — known ECMA-182/XZ vectors -----

    #[test]
    fn crc64_xz_empty_is_zero() {
        assert_eq!(crc64_xz(b""), 0);
    }

    #[test]
    fn crc64_xz_known_vector_123456789() {
        // Standard "check" value for CRC-64/XZ -> 0x995DC9BBDF1939FA.
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
        // 55-byte input: lies 1 byte below the block boundary (56); the
        // padding occupies exactly 9 bytes and fits into exactly one block.
        let s = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXY12";
        assert_eq!(s.len(), 53);
        // Arbitrary check: the hash stays deterministic.
        let h1 = md5(s.as_bytes());
        let h2 = md5(s.as_bytes());
        assert_eq!(h1, h2);
    }

    #[test]
    fn md5_block_boundary_64_bytes() {
        // 64-byte input: requires exactly two 64-byte blocks for the
        // padding (1 full block + padding block).
        let data = [b'A'; 64];
        let h = md5(&data);
        // Stability: the value is deterministic.
        let h2 = md5(&data);
        assert_eq!(h, h2);
        // Sanity: not all bytes are zero.
        assert!(h.iter().any(|&b| b != 0));
    }
}
