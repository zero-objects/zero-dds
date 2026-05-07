// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! group-id Canonical-Key Producer.
//!
//! Spec-Quelle: dds-amqp-1.0 §7.6.1 — der `group-id`-Wert in der
//! AMQP Properties-Section ist die lowercase hexadezimale
//! Repraesentation des SHA-256-Digests ueber die XCDR2-KeyHash-
//! Encapsulation des Samples (X-Types 1.3 §7.6.6.4).
//!
//! Der Hash ist opak: Subscriber duerfen die Key-Felder nicht aus
//! `group-id` rekonstruieren (Spec §7.6.2). Strukturierter
//! Key-Zugriff geht ueber den Sample-Body (XCDR2/JSON/AMQP-Native).

use alloc::string::String;
use sha2::{Digest, Sha256};

/// Spec §7.6.1 — `group-id` aus XCDR2-KeyHash-Bytes erzeugen.
///
/// Erwartet die XCDR2-KeyHash-Encapsulation (X-Types §7.6.6.4)
/// — typischerweise die XCDR2-Encoded-Key-Felder eines Samples
/// in IDL-Deklarations-Reihenfolge. Liefert 64 lowercase hex
/// chars.
///
/// # Example
/// ```
/// use zerodds_amqp_endpoint::keyhash::group_id;
/// // KeyHash-Bytes fuer den Sample (z.B. aus Cyclone DDS).
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
        // Spec verlangt deterministisches Hash; SHA-256("") ist
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

    #[test]
    fn distinct_keyhash_yields_distinct_group_id() {
        let a = group_id(b"\x00\x00\x00\x07");
        let b = group_id(b"\x00\x00\x00\x08");
        assert_ne!(a, b);
    }

    #[test]
    fn solidus_in_key_safe() {
        // Spec §7.6.1 Motivation: alte SOLIDUS-Trennung war
        // unsicher bei Strings mit `/`. SHA-256 macht den Edge-Fall
        // egal — beide Eingaben unterscheiden sich nur durch ein
        // Byte-Pattern, der Hash trennt sie.
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
