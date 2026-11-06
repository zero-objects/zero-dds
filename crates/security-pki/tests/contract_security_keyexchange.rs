// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-pki` with `zerodds-security-keyexchange`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-keyexchange`, from its non-test
//! sources:
//! - `KxSuite` (re-exported by `lib.rs` for `set_preferred_kx_suite`):
//!   `EcdhP256` as the preferred suite, `EcdhP384`, `DhModp2048_256` and
//!   `X25519` chosen by the `c.kagree_algo` of a request (`plugin.rs`).
//! - `KeyExchange::with_suite(suite)`, `public_key()` (sent as `dh1` or
//!   `dh2`) and `derive_shared_secret(peer)` with the `dh1`/`dh2` of the
//!   peer; `plugin.rs` hashes the raw result with SHA-256 into the
//!   SharedSecret.
//!
//! What it relies on: the public key has the form a peer reads (SEC 1
//! uncompressed points, the 32 octets of X25519, the big-endian number of the
//! MODP group), the raw agreement is the x-coordinate (or the number) of
//! the peer's key raised to the own private key, and a key that is not on
//! the curve or not in the group is refused.
//!
//! Oracles: DDS-Security 1.2 §8.3 Table 26 (`CID_DHE_MODP_2048_256` = 0x20,
//! `CID_ECDHE_CEUM_P256` = 0x21, `CID_ECDHE_CEUM_P384` = 0x22); SEC 1 §2.3.3
//! (uncompressed points), the base points of P-256 and P-384 (FIPS 186-4
//! D.1.2.3 and D.1.2.4); RFC 7748 §4.1 (the X25519 base point u = 9, the
//! public key is X25519(k, 9)) and §6.1 (the all-zero result is refused);
//! RFC 5114 §2.3 (the 2048-bit MODP group, `tests/fixtures/rfc5114_2_3.txt`
//! of the producer); RFC 5903 §8.1 (the responder's public value of the
//! P-256 example); SEC 1 §3.3.1 (the ECDH shared secret is the
//! x-coordinate). Taking a base point as the peer's key gives the own
//! public key as the agreement: g^x is both.
#![allow(clippy::unwrap_used)]

use zerodds_security::error::SecurityErrorKind;
use zerodds_security_keyexchange::{KeyExchange, KxSuite};

/// FIPS 186-4 D.1.2.3: the base point of P-256, uncompressed.
fn p256_base_point() -> Vec<u8> {
    let mut g = vec![0x04];
    g.extend(unhex(
        "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296\
         4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
    ));
    g
}

/// FIPS 186-4 D.1.2.4: the base point of P-384, uncompressed.
fn p384_base_point() -> Vec<u8> {
    let mut g = vec![0x04];
    g.extend(unhex(
        "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a38\
         5502f25dbf55296c3a545e3872760ab7\
         3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c0\
         0a60b1ce1d7e819d7a431d7c90ea0e5f",
    ));
    g
}

/// RFC 5903 §8.1: the responder's public value `gr` (x, y), uncompressed.
fn rfc5903_p256_gr() -> Vec<u8> {
    let mut g = vec![0x04];
    g.extend(unhex(
        "d12dfb5289c8d4f81208b70270398c342296970a0bccb74c736fc7554494bf63\
         56fbf3ca366cc23e8157854c13c58d6aac23f046ada30f8353e74f33039872ab",
    ));
    g
}

/// The value after `= ` and up to the next blank line of `name` in the
/// RFC 5114 §2.3 text, as octets.
fn rfc5114_value(name: &str) -> Vec<u8> {
    let text = include_str!("../../security-keyexchange/tests/fixtures/rfc5114_2_3.txt");
    let start = text.find(&format!("{name} = ")).unwrap() + name.len() + 3;
    let hex: String = text[start..]
        .lines()
        .take_while(|l| !l.contains("The "))
        .flat_map(|l| l.chars())
        .filter(char::is_ascii_hexdigit)
        .collect();
    unhex(&hex)
}

fn unhex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(u8::is_ascii_hexdigit)
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            _ => b - b'A' + 10,
        })
        .collect();
    digits.chunks(2).map(|p| (p[0] << 4) | p[1]).collect()
}

#[test]
fn security_8_3_the_suites_carry_the_ids_of_table_26() {
    assert_eq!(KxSuite::DhModp2048_256.crypto_algorithm_id(), Some(0x20));
    assert_eq!(KxSuite::EcdhP256.crypto_algorithm_id(), Some(0x21));
    assert_eq!(KxSuite::EcdhP384.crypto_algorithm_id(), Some(0x22));
    // X25519 is not in Table 26 and has no id to announce.
    assert_eq!(KxSuite::X25519.crypto_algorithm_id(), None);
    // `plugin.rs` prefers P-256, the default of the suite.
    assert_eq!(KxSuite::default(), KxSuite::EcdhP256);
    assert_eq!(KeyExchange::new().unwrap().suite(), KxSuite::EcdhP256);
}

#[test]
fn the_public_keys_have_the_form_a_peer_reads() {
    // SEC 1 §2.3.3: 0x04 || X || Y with 32 or 48 octet coordinates.
    for (suite, len) in [(KxSuite::EcdhP256, 65), (KxSuite::EcdhP384, 97)] {
        assert_eq!(suite.public_key_len(), len);
        let kx = KeyExchange::with_suite(suite).unwrap();
        assert_eq!(kx.suite(), suite);
        assert_eq!(kx.public_key().len(), len);
        assert_eq!(kx.public_key()[0], 0x04);
    }
    // RFC 7748: 32 octets.
    assert_eq!(KxSuite::X25519.public_key_len(), 32);
    let kx = KeyExchange::with_suite(KxSuite::X25519).unwrap();
    assert_eq!(kx.public_key().len(), 32);
    // RFC 5114 §2.3: a number below p, at most 256 octets, no leading zero.
    let p = rfc5114_value("p");
    assert_eq!(p.len(), 256);
    assert_eq!(KxSuite::DhModp2048_256.public_key_len(), 256);
    let kx = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
    let public = kx.public_key();
    assert!(!public.is_empty() && public.len() <= 256);
    assert_ne!(public[0], 0);
    let mut padded = vec![0u8; 256 - public.len()];
    padded.extend_from_slice(public);
    assert!(padded < p);
}

#[test]
fn security_8_3_the_agreement_with_the_base_point_is_the_own_public_key() {
    // ECDH (SEC 1 §3.3.1): the shared secret is the x-coordinate of x·G,
    // which is the x-coordinate of the own public key.
    for (suite, g, coordinate) in [
        (KxSuite::EcdhP256, p256_base_point(), 32),
        (KxSuite::EcdhP384, p384_base_point(), 48),
    ] {
        let kx = KeyExchange::with_suite(suite).unwrap();
        let x = kx.public_key()[1..=coordinate].to_vec();
        assert_eq!(kx.derive_shared_secret(&g).unwrap(), x, "{suite:?}");
    }
    // RFC 7748 §4.1: X25519(k, 9) is the public key.
    let kx = KeyExchange::with_suite(KxSuite::X25519).unwrap();
    let public = kx.public_key().to_vec();
    let mut nine = [0u8; 32];
    nine[0] = 9;
    assert_eq!(kx.derive_shared_secret(&nine).unwrap(), public);
    // RFC 5114 §2.3: g^x mod p is the public key.
    let kx = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
    let public = kx.public_key().to_vec();
    assert_eq!(
        kx.derive_shared_secret(&rfc5114_value("g")).unwrap(),
        public
    );
}

#[test]
fn rfc_5903_8_1_the_public_value_of_another_implementation_is_taken() {
    // Two sides agree on one secret of 32 octets (the x-coordinate) for a
    // public value from RFC 5903.
    let kx = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
    assert_eq!(
        kx.derive_shared_secret(&rfc5903_p256_gr()).unwrap().len(),
        32
    );

    let a = KeyExchange::new().unwrap();
    let b = KeyExchange::new().unwrap();
    let (a_public, b_public) = (a.public_key().to_vec(), b.public_key().to_vec());
    assert_eq!(
        a.derive_shared_secret(&b_public).unwrap(),
        b.derive_shared_secret(&a_public).unwrap()
    );
}

#[test]
fn keys_off_the_curve_or_outside_the_group_are_refused() {
    // A point with a changed y-coordinate is not on P-256.
    let mut off_curve = rfc5903_p256_gr();
    off_curve[64] ^= 0x01;
    let kx = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
    assert_eq!(
        kx.derive_shared_secret(&off_curve).unwrap_err().kind,
        SecurityErrorKind::CryptoFailed
    );
    // Rustdoc: a key of the wrong length is `BadArgument`.
    let kx = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
    assert_eq!(
        kx.derive_shared_secret(&p384_base_point())
            .unwrap_err()
            .kind,
        SecurityErrorKind::BadArgument
    );
    // RFC 7748 §6.1: the all-zero result of a low-order point is refused.
    let kx = KeyExchange::with_suite(KxSuite::X25519).unwrap();
    assert_eq!(
        kx.derive_shared_secret(&[0u8; 32]).unwrap_err().kind,
        SecurityErrorKind::CryptoFailed
    );
    // RFC 5114 §2.3 with SP 800-56A §5.6.2.3.1: 1 and p - 1 are no keys of
    // the subgroup.
    let mut p_minus_1 = rfc5114_value("p");
    let last = p_minus_1.len() - 1;
    p_minus_1[last] -= 1;
    for peer in [vec![1u8], p_minus_1] {
        let kx = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
        assert_eq!(
            kx.derive_shared_secret(&peer).unwrap_err().kind,
            SecurityErrorKind::CryptoFailed
        );
    }
}
