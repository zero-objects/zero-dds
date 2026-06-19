// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RFC 7541 Appendix B Static Huffman Code.
//!
//! We implement the wire-format-compatible variant: the complete
//! code book is embedded as a `(code, bit_length)` array
//! over the 256 octets + EOS (index 256 — used only for padding).

use alloc::vec::Vec;

/// Huffman code table from RFC 7541 Appendix B.
/// Tuple `(code_bits, bit_length)`. Index 0..=255 = octet, 256 = EOS.
const TABLE: [(u32, u8); 257] = [
    (0x1ff8, 13),
    (0x7fffd8, 23),
    (0xfffffe2, 28),
    (0xfffffe3, 28),
    (0xfffffe4, 28),
    (0xfffffe5, 28),
    (0xfffffe6, 28),
    (0xfffffe7, 28),
    (0xfffffe8, 28),
    (0xffffea, 24),
    (0x3ffffffc, 30),
    (0xfffffe9, 28),
    (0xfffffea, 28),
    (0x3ffffffd, 30),
    (0xfffffeb, 28),
    (0xfffffec, 28),
    (0xfffffed, 28),
    (0xfffffee, 28),
    (0xfffffef, 28),
    (0xffffff0, 28),
    (0xffffff1, 28),
    (0xffffff2, 28),
    (0x3ffffffe, 30),
    (0xffffff3, 28),
    (0xffffff4, 28),
    (0xffffff5, 28),
    (0xffffff6, 28),
    (0xffffff7, 28),
    (0xffffff8, 28),
    (0xffffff9, 28),
    (0xffffffa, 28),
    (0xffffffb, 28),
    (0x14, 6),
    (0x3f8, 10),
    (0x3f9, 10),
    (0xffa, 12),
    (0x1ff9, 13),
    (0x15, 6),
    (0xf8, 8),
    (0x7fa, 11),
    (0x3fa, 10),
    (0x3fb, 10),
    (0xf9, 8),
    (0x7fb, 11),
    (0xfa, 8),
    (0x16, 6),
    (0x17, 6),
    (0x18, 6),
    (0x0, 5),
    (0x1, 5),
    (0x2, 5),
    (0x19, 6),
    (0x1a, 6),
    (0x1b, 6),
    (0x1c, 6),
    (0x1d, 6),
    (0x1e, 6),
    (0x1f, 6),
    (0x5c, 7),
    (0xfb, 8),
    (0x7ffc, 15),
    (0x20, 6),
    (0xffb, 12),
    (0x3fc, 10),
    (0x1ffa, 13),
    (0x21, 6),
    (0x5d, 7),
    (0x5e, 7),
    (0x5f, 7),
    (0x60, 7),
    (0x61, 7),
    (0x62, 7),
    (0x63, 7),
    (0x64, 7),
    (0x65, 7),
    (0x66, 7),
    (0x67, 7),
    (0x68, 7),
    (0x69, 7),
    (0x6a, 7),
    (0x6b, 7),
    (0x6c, 7),
    (0x6d, 7),
    (0x6e, 7),
    (0x6f, 7),
    (0x70, 7),
    (0x71, 7),
    (0x72, 7),
    (0xfc, 8),
    (0x73, 7),
    (0xfd, 8),
    (0x1ffb, 13),
    (0x7fff0, 19),
    (0x1ffc, 13),
    (0x3ffc, 14),
    (0x22, 6),
    (0x7ffd, 15),
    (0x3, 5),
    (0x23, 6),
    (0x4, 5),
    (0x24, 6),
    (0x5, 5),
    (0x25, 6),
    (0x26, 6),
    (0x27, 6),
    (0x6, 5),
    (0x74, 7),
    (0x75, 7),
    (0x28, 6),
    (0x29, 6),
    (0x2a, 6),
    (0x7, 5),
    (0x2b, 6),
    (0x76, 7),
    (0x2c, 6),
    (0x8, 5),
    (0x9, 5),
    (0x2d, 6),
    (0x77, 7),
    (0x78, 7),
    (0x79, 7),
    (0x7a, 7),
    (0x7b, 7),
    (0x7ffe, 15),
    (0x7fc, 11),
    (0x3ffd, 14),
    (0x1ffd, 13),
    (0xffffffc, 28),
    (0xfffe6, 20),
    (0x3fffd2, 22),
    (0xfffe7, 20),
    (0xfffe8, 20),
    (0x3fffd3, 22),
    (0x3fffd4, 22),
    (0x3fffd5, 22),
    (0x7fffd9, 23),
    (0x3fffd6, 22),
    (0x7fffda, 23),
    (0x7fffdb, 23),
    (0x7fffdc, 23),
    (0x7fffdd, 23),
    (0x7fffde, 23),
    (0xffffeb, 24),
    (0x7fffdf, 23),
    (0xffffec, 24),
    (0xffffed, 24),
    (0x3fffd7, 22),
    (0x7fffe0, 23),
    (0xffffee, 24),
    (0x7fffe1, 23),
    (0x7fffe2, 23),
    (0x7fffe3, 23),
    (0x7fffe4, 23),
    (0x1fffdc, 21),
    (0x3fffd8, 22),
    (0x7fffe5, 23),
    (0x3fffd9, 22),
    (0x7fffe6, 23),
    (0x7fffe7, 23),
    (0xffffef, 24),
    (0x3fffda, 22),
    (0x1fffdd, 21),
    (0xfffe9, 20),
    (0x3fffdb, 22),
    (0x3fffdc, 22),
    (0x7fffe8, 23),
    (0x7fffe9, 23),
    (0x1fffde, 21),
    (0x7fffea, 23),
    (0x3fffdd, 22),
    (0x3fffde, 22),
    (0xfffff0, 24),
    (0x1fffdf, 21),
    (0x3fffdf, 22),
    (0x7fffeb, 23),
    (0x7fffec, 23),
    (0x1fffe0, 21),
    (0x1fffe1, 21),
    (0x3fffe0, 22),
    (0x1fffe2, 21),
    (0x7fffed, 23),
    (0x3fffe1, 22),
    (0x7fffee, 23),
    (0x7fffef, 23),
    (0xfffea, 20),
    (0x3fffe2, 22),
    (0x3fffe3, 22),
    (0x3fffe4, 22),
    (0x7ffff0, 23),
    (0x3fffe5, 22),
    (0x3fffe6, 22),
    (0x7ffff1, 23),
    (0x3ffffe0, 26),
    (0x3ffffe1, 26),
    (0xfffeb, 20),
    (0x7fff1, 19),
    (0x3fffe7, 22),
    (0x7ffff2, 23),
    (0x3fffe8, 22),
    (0x1ffffec, 25),
    (0x3ffffe2, 26),
    (0x3ffffe3, 26),
    (0x3ffffe4, 26),
    (0x7ffffde, 27),
    (0x7ffffdf, 27),
    (0x3ffffe5, 26),
    (0xfffff1, 24),
    (0x1ffffed, 25),
    (0x7fff2, 19),
    (0x1fffe3, 21),
    (0x3ffffe6, 26),
    (0x7ffffe0, 27),
    (0x7ffffe1, 27),
    (0x3ffffe7, 26),
    (0x7ffffe2, 27),
    (0xfffff2, 24),
    (0x1fffe4, 21),
    (0x1fffe5, 21),
    (0x3ffffe8, 26),
    (0x3ffffe9, 26),
    (0xffffffd, 28),
    (0x7ffffe3, 27),
    (0x7ffffe4, 27),
    (0x7ffffe5, 27),
    (0xfffec, 20),
    (0xfffff3, 24),
    (0xfffed, 20),
    (0x1fffe6, 21),
    (0x3fffe9, 22),
    (0x1fffe7, 21),
    (0x1fffe8, 21),
    (0x7ffff3, 23),
    (0x3fffea, 22),
    (0x3fffeb, 22),
    (0x1ffffee, 25),
    (0x1ffffef, 25),
    (0xfffff4, 24),
    (0xfffff5, 24),
    (0x3ffffea, 26),
    (0x7ffff4, 23),
    (0x3ffffeb, 26),
    (0x7ffffe6, 27),
    (0x3ffffec, 26),
    (0x3ffffed, 26),
    (0x7ffffe7, 27),
    (0x7ffffe8, 27),
    (0x7ffffe9, 27),
    (0x7ffffea, 27),
    (0x7ffffeb, 27),
    (0xffffffe, 28),
    (0x7ffffec, 27),
    (0x7ffffed, 27),
    (0x7ffffee, 27),
    (0x7ffffef, 27),
    (0x7fffff0, 27),
    (0x3ffffee, 26),
    (0x3fffffff, 30),
];

/// Huffman decode error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HuffmanError;

/// Encode `bytes` with Huffman.
#[must_use]
pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut acc: u64 = 0;
    let mut acc_bits: u8 = 0;
    for &b in bytes {
        let (code, len) = TABLE[b as usize];
        acc = (acc << len) | u64::from(code);
        acc_bits += len;
        while acc_bits >= 8 {
            acc_bits -= 8;
            out.push((acc >> acc_bits) as u8);
        }
    }
    // Pad with EOS-prefix (1-bits) per RFC 7541 §5.2.
    if acc_bits > 0 {
        let pad = 8 - acc_bits;
        acc = (acc << pad) | ((1u64 << pad) - 1);
        out.push(acc as u8);
    }
    out
}

/// Decode Huffman-encoded input.
///
/// # Errors
/// `HuffmanError` when the code stream is invalid (spec §5.2: pad
/// bits with zeros, or the sequence does not decode).
pub fn decode(bytes: &[u8]) -> Result<Vec<u8>, HuffmanError> {
    let mut out = Vec::new();
    let mut acc: u64 = 0;
    let mut acc_bits: u8 = 0;
    for &b in bytes {
        acc = (acc << 8) | u64::from(b);
        acc_bits += 8;
        // Try to consume codes from MSB.
        while acc_bits > 0 {
            let mut found = None;
            for (sym, (code, len)) in TABLE.iter().enumerate().take(256) {
                if *len <= acc_bits {
                    let shift = acc_bits - len;
                    let candidate = (acc >> shift) & ((1u64 << len) - 1);
                    if candidate == u64::from(*code) {
                        found = Some((sym as u8, *len));
                        break;
                    }
                }
            }
            if let Some((sym, len)) = found {
                out.push(sym);
                acc_bits -= len;
                acc &= (1u64 << acc_bits) - 1;
            } else {
                break;
            }
        }
    }
    // Pad bits must be 1 (EOS-prefix) per RFC 7541.
    if acc_bits > 0 {
        let pad_mask = (1u64 << acc_bits) - 1;
        if (acc & pad_mask) != pad_mask {
            return Err(HuffmanError);
        }
        if acc_bits >= 8 {
            return Err(HuffmanError);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ascii() {
        for s in ["www.example.com", "no-cache", "custom-key", "/index.html"] {
            let enc = encode(s.as_bytes());
            let dec = decode(&enc).unwrap();
            assert_eq!(dec, s.as_bytes());
        }
    }

    #[test]
    fn round_trip_empty() {
        let enc = encode(b"");
        assert!(enc.is_empty());
        let dec = decode(&enc).unwrap();
        assert!(dec.is_empty());
    }

    #[test]
    fn round_trip_single_char() {
        for c in [b'a', b'A', b'0', b'!', b' '] {
            let enc = encode(&[c]);
            let dec = decode(&enc).unwrap();
            assert_eq!(dec, alloc::vec![c]);
        }
    }

    #[test]
    fn rfc_appendix_c_4_1_www_example_com_compresses() {
        // Spec §C.4.1: "www.example.com" → 12 bytes Huffman-encoded
        // (`f1e3 c2e5 f23a 6ba0 ab90 f4ff`).
        let enc = encode(b"www.example.com");
        let expected = [
            0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff,
        ];
        assert_eq!(enc, expected);
    }

    #[test]
    fn rfc_appendix_c_4_2_no_cache_compresses() {
        let enc = encode(b"no-cache");
        let expected = [0xa8, 0xeb, 0x10, 0x64, 0x9c, 0xbf];
        assert_eq!(enc, expected);
    }

    #[test]
    fn invalid_pad_with_zeros_rejected() {
        // Encode normally then flip pad bits to 0.
        let mut enc = encode(b"a");
        // Force pad bits to 0 by clearing low bits.
        if let Some(last) = enc.last_mut() {
            // 'a' is 6 bits (0x21), so pad is 2 bits → mask 0xfc.
            *last &= 0xfc;
        }
        assert_eq!(decode(&enc), Err(HuffmanError));
    }
}
