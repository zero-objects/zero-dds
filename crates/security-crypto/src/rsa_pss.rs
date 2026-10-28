// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RSASSA-PSS-MGF1SHA256+2048+SHA256` (DDS-Security 1.2 §8.2 Table 25,
//! `CryptoAlgorithm Id 0x10`): the verification of a signature "using the
//! RSASSA-PSS algorithm specified in PKCS #1 ... using SHA256 as hash
//! function, and MGF1 with SHA256. ... The length of the salt is not
//! specified. ... The validation of the signature shall detect the salt
//! length from the signature."
//!
//! [`verify_rsassa_pss_sha256`] reads the salt length out of the signature
//! as RFC 8017 §9.1.2 says a verifier may: `DB` is `PS | 0x01 | salt`, the
//! length of the salt is what follows the first non-zero octet of `DB`. A
//! verifier with a fixed salt length (the `ring` and `aws-lc` ones take the
//! length of the hash) refuses the signatures of the signers that use another
//! length, which Table 25 allows.
//!
//! The modular exponentiation is `crypto-bigint`'s, in Montgomery form, with
//! the public exponent of the key.

use alloc::vec::Vec;

use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
use crypto_bigint::{Odd, U64, U2048, U3072, U4096, U6144, U8192};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use crate::backend::digest;

const H_LEN: usize = 32;

fn refused(detail: &'static str) -> SecurityError {
    SecurityError::new(SecurityErrorKind::CryptoFailed, detail)
}

/// One DER element: its tag, its content, and what follows it.
fn der_element(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *input.first()?;
    let first = *input.get(1)?;
    let (len, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for b in input.get(2..2 + n)? {
            len = (len << 8) | usize::from(*b);
        }
        (len, 2 + n)
    };
    let end = header.checked_add(len)?;
    Some((tag, input.get(header..end)?, input.get(end..)?))
}

/// The modulus and the public exponent of an `RSAPublicKey` (RFC 8017
/// A.1.1: `SEQUENCE { modulus INTEGER, publicExponent INTEGER }`).
fn rsa_public_key(der: &[u8]) -> Option<(Vec<u8>, u64)> {
    let (tag, body, rest) = der_element(der)?;
    if tag != 0x30 || !rest.is_empty() {
        return None;
    }
    let (t_n, n, rest) = der_element(body)?;
    let (t_e, e, rest) = der_element(rest)?;
    if t_n != 0x02 || t_e != 0x02 || !rest.is_empty() {
        return None;
    }
    let strip = |v: &[u8]| -> Vec<u8> {
        let skip = v.iter().take_while(|b| **b == 0).count();
        v[skip..].to_vec()
    };
    let n = strip(n);
    let e = strip(e);
    if n.is_empty() || e.is_empty() || e.len() > 8 {
        return None;
    }
    let e = e.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    Some((n, e))
}

/// `s^e mod n` as big-endian octets of the length of `n`, for the smallest
/// width among the ones `crypto-bigint` has that holds `n`.
fn mod_pow(n: &[u8], e: u64, s: &[u8]) -> Option<Vec<u8>> {
    macro_rules! with_width {
        ($uint:ty) => {{
            if n.len() > <$uint>::BYTES || s.len() > <$uint>::BYTES {
                None
            } else {
                let pad = |v: &[u8]| {
                    let mut buf = alloc::vec![0u8; <$uint>::BYTES];
                    buf[<$uint>::BYTES - v.len()..].copy_from_slice(v);
                    buf
                };
                let modulus = <$uint>::from_be_slice(&pad(n));
                let base = <$uint>::from_be_slice(&pad(s));
                // RSAVP1: 0 <= s < n; an even modulus is no RSA modulus.
                if base >= modulus {
                    return None;
                }
                let modulus: Odd<$uint> = Option::from(Odd::new(modulus))?;
                // n, e and s are public: verification may take variable time.
                let params = FixedMontyParams::new_vartime(modulus);
                let result = FixedMontyForm::new(&base, &params)
                    .pow_vartime(&U64::from_u64(e))
                    .retrieve();
                let bytes = result.to_be_bytes();
                let bytes: &[u8] = bytes.as_ref();
                Some(bytes[bytes.len() - n.len()..].to_vec())
            }
        }};
    }
    let bits = n.len();
    if bits <= U2048::BYTES {
        with_width!(U2048)
    } else if bits <= U3072::BYTES {
        with_width!(U3072)
    } else if bits <= U4096::BYTES {
        with_width!(U4096)
    } else if bits <= U6144::BYTES {
        with_width!(U6144)
    } else {
        with_width!(U8192)
    }
}

/// MGF1 with SHA-256 (RFC 8017 B.2.1).
fn mgf1_sha256(seed: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + H_LEN);
    let mut counter = 0u32;
    while out.len() < len {
        let mut input = Vec::with_capacity(seed.len() + 4);
        input.extend_from_slice(seed);
        input.extend_from_slice(&counter.to_be_bytes());
        out.extend_from_slice(digest::digest(&digest::SHA256, &input).as_ref());
        counter += 1;
    }
    out.truncate(len);
    out
}

/// EMSA-PSS-VERIFY (RFC 8017 §9.1.2) with SHA-256 and MGF1-SHA256, the salt
/// length read from `em`. Returns the length of the salt.
fn emsa_pss_verify(message: &[u8], em: &[u8], em_bits: usize) -> Option<usize> {
    let m_hash = digest::digest(&digest::SHA256, message);
    let em_len = em_bits.div_ceil(8);
    if em.len() != em_len || em_len < H_LEN + 2 || em[em_len - 1] != 0xbc {
        return None;
    }
    let (masked_db, rest) = em.split_at(em_len - H_LEN - 1);
    let h = &rest[..H_LEN];
    // The leftmost 8 * emLen - emBits bits of maskedDB are zero.
    let unused = 8 * em_len - em_bits;
    if unused > 0 && masked_db[0] >> (8 - unused) != 0 {
        return None;
    }
    let mask = mgf1_sha256(h, em_len - H_LEN - 1);
    let mut db: Vec<u8> = masked_db.iter().zip(&mask).map(|(a, b)| a ^ b).collect();
    if unused > 0 {
        db[0] &= 0xff >> unused;
    }
    // DB = PS | 0x01 | salt, PS zeros: the salt length is what follows the
    // first non-zero octet, which has to be 0x01.
    let one = db.iter().position(|b| *b != 0)?;
    if db[one] != 0x01 {
        return None;
    }
    let salt = &db[one + 1..];
    let mut m_prime = Vec::with_capacity(8 + H_LEN + salt.len());
    m_prime.extend_from_slice(&[0u8; 8]);
    m_prime.extend_from_slice(m_hash.as_ref());
    m_prime.extend_from_slice(salt);
    let h_prime = digest::digest(&digest::SHA256, &m_prime);
    let diff = h
        .iter()
        .zip(h_prime.as_ref())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    (diff == 0).then_some(salt.len())
}

/// Verifies an RSASSA-PSS signature (SHA-256, MGF1-SHA256) over `message`
/// with the `RSAPublicKey` `public_key_der` (PKCS #1), of any salt length: it
/// "shall detect the salt length from the signature" (Table 25). The modulus
/// has 2048 to 8192 bits.
///
/// Returns the length of the salt the signature has.
///
/// # Errors
/// `CryptoFailed` for a key that is no RSA public key of 2048 to 8192 bits, a
/// signature of another length than the modulus, or one that does not
/// verify.
pub fn verify_rsassa_pss_sha256(
    public_key_der: &[u8],
    message: &[u8],
    signature: &[u8],
) -> SecurityResult<usize> {
    let (n, e) =
        rsa_public_key(public_key_der).ok_or_else(|| refused("rsa-pss: not an RSAPublicKey"))?;
    // The modulus without leading zeros: its bit length decides emBits.
    let mod_bits = n.len() * 8 - n[0].leading_zeros() as usize;
    if !(2048..=8192).contains(&mod_bits) {
        return Err(refused("rsa-pss: the modulus has 2048 to 8192 bits"));
    }
    if e < 3 || e & 1 == 0 {
        return Err(refused("rsa-pss: an odd public exponent of at least 3"));
    }
    // RSASSA-PSS-VERIFY: the signature has the length of the modulus.
    if signature.len() != n.len() {
        return Err(refused(
            "rsa-pss: the signature has not the length of the modulus",
        ));
    }
    let em_full =
        mod_pow(&n, e, signature).ok_or_else(|| refused("rsa-pss: signature out of range"))?;
    // EM = I2OSP(m, emLen) with emLen = ceil((modBits - 1) / 8): when
    // modBits - 1 is a multiple of 8 the leading octet of the k octets is 0.
    let em_bits = mod_bits - 1;
    let em_len = em_bits.div_ceil(8);
    let skip = em_full.len() - em_len;
    if em_full[..skip].iter().any(|b| *b != 0) {
        return Err(refused("rsa-pss: the encoded message is longer than emLen"));
    }
    emsa_pss_verify(message, &em_full[skip..], em_bits)
        .ok_or_else(|| refused("rsa-pss: the signature does not verify"))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const MSG: &[u8] = include_bytes!("../tests/fixtures/rsa_pss/msg.bin");
    const PUB2048: &[u8] = include_bytes!("../tests/fixtures/rsa_pss/pub2048.der");
    const PUB3072: &[u8] = include_bytes!("../tests/fixtures/rsa_pss/pub3072.der");

    fn sig2048(salt: usize) -> &'static [u8] {
        match salt {
            0 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_0.bin"),
            8 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_8.bin"),
            20 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_20.bin"),
            32 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_32.bin"),
            64 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_64.bin"),
            222 => include_bytes!("../tests/fixtures/rsa_pss/sig2048_222.bin"),
            _ => panic!("no vector"),
        }
    }

    /// Table 25: "The validation of the signature shall detect the salt
    /// length from the signature": signatures OpenSSL made with salts of 0, 8,
    /// 20, 32, 64 and the largest, 222, octets all verify, and the length
    /// that is returned is the one it signed with.
    #[test]
    fn the_salt_length_is_read_from_the_signature() {
        for salt in [0usize, 8, 20, 32, 64, 222] {
            let got = verify_rsassa_pss_sha256(PUB2048, MSG, sig2048(salt)).unwrap();
            assert_eq!(got, salt, "salt {salt}");
        }
    }

    /// The same for a key of 3072 bits.
    #[test]
    fn a_3072_bit_key_verifies_signatures_of_any_salt_length() {
        for (salt, sig) in [
            (
                0usize,
                &include_bytes!("../tests/fixtures/rsa_pss/sig3072_0.bin")[..],
            ),
            (
                32,
                &include_bytes!("../tests/fixtures/rsa_pss/sig3072_32.bin")[..],
            ),
            (
                94,
                &include_bytes!("../tests/fixtures/rsa_pss/sig3072_94.bin")[..],
            ),
        ] {
            assert_eq!(
                verify_rsassa_pss_sha256(PUB3072, MSG, sig).unwrap(),
                salt,
                "salt {salt}"
            );
        }
    }

    /// A signature over another message, a signature with a changed octet,
    /// one of the wrong length, and a key of another signer do not verify.
    #[test]
    fn what_does_not_verify_is_refused() {
        let sig = sig2048(20);
        assert!(verify_rsassa_pss_sha256(PUB2048, b"another message", sig).is_err());
        for at in [0usize, 100, 255] {
            let mut bad = sig.to_vec();
            bad[at] ^= 0x01;
            assert!(
                verify_rsassa_pss_sha256(PUB2048, MSG, &bad).is_err(),
                "octet {at}"
            );
        }
        assert!(verify_rsassa_pss_sha256(PUB2048, MSG, &sig[1..]).is_err());
        let mut long = sig.to_vec();
        long.push(0);
        assert!(verify_rsassa_pss_sha256(PUB2048, MSG, &long).is_err());
        assert!(verify_rsassa_pss_sha256(PUB3072, MSG, sig).is_err());
        // Not a public key, and a modulus below 2048 bits.
        assert!(verify_rsassa_pss_sha256(&[1, 2, 3], MSG, sig).is_err());
        let small = [0x30, 0x08, 0x02, 0x03, 0x00, 0xc3, 0x61, 0x02, 0x01, 0x03];
        assert!(verify_rsassa_pss_sha256(&small, MSG, sig).is_err());
        // The signature of a PKCS #1 v1.5 padding is no PSS signature (the
        // 0xbc trailer is absent): a modified top byte stands for it.
        let mut zero = alloc::vec![0u8; 256];
        zero[255] = 1;
        assert!(verify_rsassa_pss_sha256(PUB2048, MSG, &zero).is_err());
    }

    /// The parts of the verification, RFC 8017: MGF1 and the DER of the key.
    #[test]
    fn the_public_key_is_read_from_its_der() {
        let (n, e) = rsa_public_key(PUB2048).unwrap();
        assert_eq!(n.len(), 256);
        assert_eq!(e, 65537);
        assert!(rsa_public_key(&PUB2048[..PUB2048.len() - 1]).is_none());
        // MGF1-SHA256 of the empty seed for 3 octets: the first octets of
        // SHA-256(00000000) (e3b0c442... is SHA-256 of nothing; of four zero
        // octets it is df3f6198...).
        let mask = mgf1_sha256(&[], 3);
        assert_eq!(mask, [0xdf, 0x3f, 0x61]);
    }
}
