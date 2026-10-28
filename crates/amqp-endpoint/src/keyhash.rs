// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! group-id canonical-key producer.
//!
//! Spec source: dds-amqp-1.0 §7.6.1 — the `group-id` value in the
//! AMQP properties section is the lowercase hexadecimal
//! representation of the SHA-256 digest over the sample's XCDR2
//! KeyHash encapsulation (X-Types 1.3 §7.6.6.4).
//!
//! The hash is opaque: subscribers must not reconstruct the key
//! fields from `group-id` (spec §7.6.2). Structured key access goes
//! through the sample body (XCDR2/JSON/AMQP-native).

use alloc::string::String;
use sha2::{Digest, Sha256};

/// Spec §7.6.1 — produce `group-id` from XCDR2 KeyHash bytes.
///
/// Expects the XCDR2 KeyHash encapsulation (X-Types §7.6.6.4)
/// — typically the XCDR2-encoded key fields of a sample
/// in IDL declaration order. Returns 64 lowercase hex
/// chars.
///
/// # Example
/// ```
/// use zerodds_amqp_endpoint::keyhash::group_id;
/// // KeyHash bytes for the sample (e.g. from Cyclone DDS).
/// let bytes = b"\x00\x00\x00\x07\x00\x00\x00\x0a\x00\x00\x00\x14"; // sensor=7, x=10, y=20
/// let id = group_id(bytes);
/// assert_eq!(id.len(), 64);
/// assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
/// ```
#[must_use]
pub fn group_id(xcdr2_keyhash: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(xcdr2_keyhash);
    let digest = h.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = core::fmt::Write::write_fmt(&mut out, core::format_args!("{b:02x}"));
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_keyhash_yields_known_sha256() {
        // Spec requires a deterministic hash; SHA-256("") is
        // `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
        let id = group_id(b"");
        assert_eq!(
            id,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn keyhash_length_always_64_lowercase_hex() {
        for sample in [b"a".as_slice(), b"abc".as_slice(), &[0u8; 64]] {
            let id = group_id(sample);
            assert_eq!(id.len(), 64);
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
    }

    /// Spec `dds-amqp-1.0` §7.6.2: the hash is one-way and "the
    /// subscriber SHALL NOT attempt to reconstruct the key fields from
    /// `group-id`". What can be measured is that nothing of the key is
    /// left in the value to reconstruct from: no byte of the key
    /// survives as a hex pair at its own position, the length is
    /// constant regardless of how long the key was, and a one-bit change
    /// in the key changes most of the digest.
    #[test]
    fn group_id_carries_no_recoverable_trace_of_the_key() {
        let key: &[u8] = b"\x00\x00\x00\x07\x00\x00\x00\x0aSENSOR-ALPHA";
        let id = group_id(key);

        // The key's own hex spelling does not occur in the digest.
        let mut key_hex = String::new();
        for b in key {
            let _ = core::fmt::Write::write_fmt(&mut key_hex, core::format_args!("{b:02x}"));
        }
        assert!(!id.contains(&key_hex), "the whole key must not appear");
        // Nor does any four-octet window of it.
        for w in key.windows(4) {
            let mut hex = String::new();
            for b in w {
                let _ = core::fmt::Write::write_fmt(&mut hex, core::format_args!("{b:02x}"));
            }
            assert!(!id.contains(&hex), "key window {hex} leaked into group-id");
        }

        // A long and a short key produce the same-length value: the
        // digest carries no length information about the key either.
        assert_eq!(group_id(b"x").len(), group_id(&[0u8; 4096]).len());

        // One flipped bit changes most of the digest — the value is not
        // a positional encoding of the key.
        let mut flipped = key.to_vec();
        flipped[3] ^= 0x01;
        let other = group_id(&flipped);
        let same = id
            .chars()
            .zip(other.chars())
            .filter(|(a, b)| a == b)
            .count();
        assert!(
            same < 32,
            "one flipped key bit left {same}/64 hex digits unchanged"
        );
    }

    #[test]
    fn distinct_keyhash_yields_distinct_group_id() {
        let a = group_id(b"\x00\x00\x00\x07");
        let b = group_id(b"\x00\x00\x00\x08");
        assert_ne!(a, b);
    }

    #[test]
    fn solidus_in_key_safe() {
        // Spec §7.6.1 motivation: the old SOLIDUS separation was
        // unsafe for strings containing `/`. SHA-256 makes the edge
        // case moot — both inputs differ only by a byte pattern, and
        // the hash separates them.
        let with_slash = group_id(b"EU/DE\x00Foo");
        let without = group_id(b"EUDEXFoo");
        assert_ne!(with_slash, without);
    }

    #[test]
    fn deterministic_across_calls() {
        let a = group_id(b"\x00\x01\x02\x03");
        let b = group_id(b"\x00\x01\x02\x03");
        assert_eq!(a, b);
    }
}
