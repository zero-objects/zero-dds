// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `GroupDigest_t` — DDSI-RTPS 2.5 §8.3.5.10 (16-byte CDR-encoded
//! 128-bit digest for group membership). Carried in heartbeats with the
//! GroupInfo flag (`HeartbeatSubmessage.writer_set` /
//! `secure_writer_set`), to prove to readers the consistency of the shared
//! writer/reader group per tick.
//!
//! # Spec-Layout
//!
//! ```text
//! struct GroupDigest_t {
//!     octet[16] value;  // MD5 over {GuidPrefix*}
//! };
//! ```
//!
//! The hash algorithm is MD5 (RFC 1321) over the concatenated
//! 12-byte `GuidPrefix` values of all group members, sorted
//! ascending. The spec leaves the sorting open but fixes it
//! per vendor; ZeroDDS sorts lexicographically (stable +
//! deterministic), so that the hash is byte-identical to Cyclone DDS
//! (until Cyclone vendor hashing is documented, then
//! possibly a vendor switch).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::wire_types::GuidPrefix;

/// `GroupDigest_t` (Spec §8.3.5.10). 16-byte value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroupDigest(pub [u8; 16]);

impl GroupDigest {
    /// Wire size: 16 bytes.
    pub const WIRE_SIZE: usize = 16;

    /// `GROUPDIGEST_UNKNOWN` (Spec): 16 null bytes.
    pub const UNKNOWN: Self = Self([0; 16]);

    /// `true` if the value equals the `UNKNOWN` sentinel.
    #[must_use]
    pub fn is_unknown(self) -> bool {
        self.0 == [0u8; 16]
    }

    /// Computes the digest from a list of GuidPrefixes. Sorts
    /// the input before hashing — the result is thus independent
    /// of the caller's iteration order.
    #[must_use]
    pub fn from_prefixes(prefixes: &[GuidPrefix]) -> Self {
        let mut sorted: Vec<&GuidPrefix> = prefixes.iter().collect();
        sorted.sort();
        let mut concat = Vec::with_capacity(sorted.len() * GuidPrefix::WIRE_SIZE);
        for p in sorted {
            concat.extend_from_slice(&p.0);
        }
        Self(md5_128(&concat))
    }

    /// LE bytes (the spec specifies no endianness — the value is a
    /// pure byte sequence from the hash).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        self.0
    }

    /// Roundtrip identity.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Decodes from a slice. Truncated → error.
    ///
    /// # Errors
    /// `UnexpectedEof` if `bytes.len() < 16`.
    pub fn read_from(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 16 {
            return Err(WireError::UnexpectedEof {
                needed: 16,
                offset: 0,
            });
        }
        let mut out = [0u8; 16];
        out.copy_from_slice(&bytes[..16]);
        Ok(Self(out))
    }
}

/// MD5 via `zerodds_foundation::md5` (pure-Rust no_std). MD5 here is
/// not security-relevant — only for group-digest consistency with
/// Cyclone DDS and other DDS stacks.
fn md5_128(input: &[u8]) -> [u8; 16] {
    zerodds_foundation::md5(input)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn md5_empty_input_matches_rfc1321_test_vector() {
        // RFC 1321 test vector: MD5("") = d41d8cd98f00b204e9800998ecf8427e
        let h = md5_128(b"");
        assert_eq!(
            h,
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
    }

    #[test]
    fn md5_abc_matches_rfc1321_test_vector() {
        // MD5("abc") = 900150983cd24fb0d6963f7d28e17f72
        let h = md5_128(b"abc");
        assert_eq!(
            h,
            [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
                0x7f, 0x72
            ]
        );
    }

    #[test]
    fn md5_message_digest_matches_rfc1321_test_vector() {
        // RFC 1321 test vector 5: MD5("message digest")
        //   = f96b697d7cb7938d525a2f31aaf161d0
        let h = md5_128(b"message digest");
        assert_eq!(
            h,
            [
                0xf9, 0x6b, 0x69, 0x7d, 0x7c, 0xb7, 0x93, 0x8d, 0x52, 0x5a, 0x2f, 0x31, 0xaa, 0xf1,
                0x61, 0xd0
            ]
        );
    }

    #[test]
    fn md5_long_input_matches_rfc1321_test_vector() {
        // RFC 1321 test vector 7 (62 byte, 2-block path). Input is
        // UPPER-cased first, then lowercase, then digits:
        //   "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
        //   = d174ab98d277d9f5a5611c2c9f419d9f
        let h = md5_128(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789");
        assert_eq!(
            h,
            [
                0xd1, 0x74, 0xab, 0x98, 0xd2, 0x77, 0xd9, 0xf5, 0xa5, 0x61, 0x1c, 0x2c, 0x9f, 0x41,
                0x9d, 0x9f
            ]
        );
    }

    #[test]
    fn group_digest_handles_more_than_5_prefixes_two_block_md5() {
        // 6 GuidPrefixes = 72 bytes input → 2-block MD5 path. Ensures
        // that GroupDigest also works correctly for larger groups.
        let prefixes: alloc::vec::Vec<GuidPrefix> =
            (1u8..=6).map(|b| GuidPrefix::from_bytes([b; 12])).collect();
        let d = GroupDigest::from_prefixes(&prefixes);
        // Expected output is deterministic (for regression).
        assert!(!d.is_unknown());
        // Order independence also for 6 prefixes.
        let mut reversed = prefixes.clone();
        reversed.reverse();
        let d2 = GroupDigest::from_prefixes(&reversed);
        assert_eq!(d, d2);
    }

    #[test]
    fn group_digest_unknown_is_zero() {
        assert!(GroupDigest::UNKNOWN.is_unknown());
        assert_eq!(GroupDigest::UNKNOWN.0, [0u8; 16]);
    }

    #[test]
    fn group_digest_from_empty_prefixes_is_md5_of_empty() {
        let d = GroupDigest::from_prefixes(&[]);
        // MD5("") as 16 bytes
        assert_eq!(
            d.0,
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
    }

    #[test]
    fn group_digest_independent_of_input_order() {
        let p1 = GuidPrefix::from_bytes([1; 12]);
        let p2 = GuidPrefix::from_bytes([2; 12]);
        let p3 = GuidPrefix::from_bytes([3; 12]);
        let a = GroupDigest::from_prefixes(&[p1, p2, p3]);
        let b = GroupDigest::from_prefixes(&[p3, p2, p1]);
        assert_eq!(a, b);
    }

    #[test]
    fn group_digest_distinguishes_different_groups() {
        let g1 = GroupDigest::from_prefixes(&[GuidPrefix::from_bytes([1; 12])]);
        let g2 = GroupDigest::from_prefixes(&[GuidPrefix::from_bytes([2; 12])]);
        assert_ne!(g1, g2);
    }

    #[test]
    fn group_digest_roundtrip() {
        let d = GroupDigest::from_prefixes(&[
            GuidPrefix::from_bytes([7; 12]),
            GuidPrefix::from_bytes([8; 12]),
        ]);
        let bytes = d.to_bytes();
        assert_eq!(GroupDigest::from_bytes(bytes), d);
        assert_eq!(GroupDigest::read_from(&bytes).unwrap(), d);
    }

    #[test]
    fn group_digest_read_from_truncated_rejects() {
        let r = GroupDigest::read_from(&[0u8; 8]);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }
}
