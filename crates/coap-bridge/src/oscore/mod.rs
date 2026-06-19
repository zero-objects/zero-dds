//! OSCORE — Object Security for Constrained RESTful Environments (RFC 8613).
//!
//! Object-level security for CoAP, an alternative to (D)TLS that protects the
//! message itself (so it survives proxies). This module implements the OSCORE
//! Security Context and its HKDF key derivation (RFC 8613 §3.2); the AEAD
//! protect/unprotect layer (COSE Encrypt0 + AES-CCM) builds on top of it.
//!
//! Supersedes ADR 0007 (which classified OSCORE as `rejected` for RC1): per the
//! spec-completeness directive, the optional §7.2 profile is implemented in full
//! rather than stubbed. Correctness is anchored to the RFC 8613 Appendix C test
//! vectors (the authoritative ground truth), verified byte-exact in the tests.
//!
//! `no_std + alloc`; the only crypto dependency for key derivation is HMAC-
//! SHA-256 (RustCrypto `hmac` + `sha2`), used to build HKDF (RFC 5869) directly.

extern crate alloc;

pub mod aead;
pub mod message;
pub mod wire;

use alloc::vec;
use alloc::vec::Vec;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Error from an OSCORE operation: AEAD authentication failure, malformed wire
/// data, or a bad key/nonce length. Deliberately opaque (no oracle on which
/// check failed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OscoreError;

/// AEAD algorithm used by an OSCORE Security Context (COSE algorithm registry).
///
/// AES-CCM-16-64-128 (COSE value `10`) is the OSCORE mandatory-to-implement
/// default (RFC 8613 §3.2): 128-bit key, 13-byte nonce, 64-bit (8-byte) tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AeadAlgorithm {
    /// AES-CCM-16-64-128 — the OSCORE default AEAD.
    AesCcm16_64_128,
}

impl AeadAlgorithm {
    /// COSE algorithm identifier (encoded into the HKDF `info` structure).
    #[must_use]
    pub const fn cose_value(self) -> i32 {
        match self {
            Self::AesCcm16_64_128 => 10,
        }
    }

    /// Key length in bytes.
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            Self::AesCcm16_64_128 => 16,
        }
    }

    /// Nonce (Common IV) length in bytes.
    #[must_use]
    pub const fn nonce_len(self) -> usize {
        match self {
            Self::AesCcm16_64_128 => 13,
        }
    }

    /// Authentication tag length in bytes.
    #[must_use]
    pub const fn tag_len(self) -> usize {
        match self {
            Self::AesCcm16_64_128 => 8,
        }
    }
}

/// A derived OSCORE Security Context (RFC 8613 §3.1): the Sender/Recipient keys
/// and the Common IV, produced from the shared Master Secret + Master Salt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityContext {
    /// Key used to protect outgoing messages.
    pub sender_key: Vec<u8>,
    /// Key used to verify/decrypt incoming messages.
    pub recipient_key: Vec<u8>,
    /// Common IV; the per-message nonce is derived from it + the Partial IV.
    pub common_iv: Vec<u8>,
    /// The negotiated AEAD algorithm.
    pub alg: AeadAlgorithm,
}

impl SecurityContext {
    /// Derive a Security Context per RFC 8613 §3.2.1.
    ///
    /// `master_secret` and `master_salt` are the pre-shared inputs; `sender_id`
    /// and `recipient_id` are the OSCORE party identifiers; `id_context` is the
    /// optional ID Context (`None` → CBOR `nil`). The Common IV is derived with
    /// an empty `id`.
    #[must_use]
    pub fn derive(
        master_secret: &[u8],
        master_salt: &[u8],
        sender_id: &[u8],
        recipient_id: &[u8],
        id_context: Option<&[u8]>,
        alg: AeadAlgorithm,
    ) -> Self {
        // PRK = HKDF-Extract(salt = Master Salt, IKM = Master Secret).
        let prk = hkdf_extract(master_salt, master_secret);
        let key_len = alg.key_len();
        let iv_len = alg.nonce_len();
        let cose = alg.cose_value();

        let mut sender_key = vec![0u8; key_len];
        hkdf_expand(
            &prk,
            &encode_info(sender_id, id_context, cose, "Key", key_len),
            &mut sender_key,
        );

        let mut recipient_key = vec![0u8; key_len];
        hkdf_expand(
            &prk,
            &encode_info(recipient_id, id_context, cose, "Key", key_len),
            &mut recipient_key,
        );

        let mut common_iv = vec![0u8; iv_len];
        hkdf_expand(
            &prk,
            &encode_info(&[], id_context, cose, "IV", iv_len),
            &mut common_iv,
        );

        Self {
            sender_key,
            recipient_key,
            common_iv,
            alg,
        }
    }
}

/// HKDF-Extract (RFC 5869 §2.2): `PRK = HMAC-SHA-256(salt, IKM)`.
///
/// An empty `salt` is HMAC-key-padded to a block of zeros, which is equivalent
/// to the RFC-mandated `HashLen` zero bytes — so it needs no special-casing.
fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt).expect("HMAC accepts any key length");
    mac.update(ikm);
    let out = mac.finalize().into_bytes();
    let mut prk = [0u8; 32];
    prk.copy_from_slice(&out);
    prk
}

/// HKDF-Expand (RFC 5869 §2.3): expand `prk` + `info` into `out` (any length up
/// to 255·HashLen).
fn hkdf_expand(prk: &[u8; 32], info: &[u8], out: &mut [u8]) {
    let mut prev: Vec<u8> = Vec::new();
    let mut pos = 0usize;
    let mut counter: u8 = 1;
    while pos < out.len() {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(prk).expect("PRK is 32 bytes");
        mac.update(&prev);
        mac.update(info);
        mac.update(&[counter]);
        let block = mac.finalize().into_bytes();
        let take = core::cmp::min(block.len(), out.len() - pos);
        out[pos..pos + take].copy_from_slice(&block[..take]);
        pos += take;
        prev = block.to_vec();
        counter = counter.wrapping_add(1);
    }
}

/// Serialize the OSCORE HKDF `info` CBOR array (RFC 8613 §3.2.1):
/// `[ id : bstr, id_context : bstr / nil, alg_aead : int, type : tstr, L : uint ]`.
///
/// Canonical/deterministic CBOR — hand-encoded (the structure is tiny and fixed,
/// so this avoids a serde-CBOR dependency and is trivially auditable).
fn encode_info(
    id: &[u8],
    id_context: Option<&[u8]>,
    alg: i32,
    type_str: &str,
    l: usize,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x85); // array(5)
    cbor_bstr(&mut out, id);
    match id_context {
        Some(ctx) => cbor_bstr(&mut out, ctx),
        None => out.push(0xf6), // nil
    }
    cbor_int(&mut out, alg);
    cbor_tstr(&mut out, type_str);
    cbor_uint(&mut out, l as u64);
    out
}

/// CBOR major-type header (`major << 5 | argument`), with the argument encoded
/// in the minimal width (RFC 8949 §3 canonical form).
fn cbor_head(out: &mut Vec<u8>, major: u8, arg: u64) {
    let mt = major << 5;
    if arg < 24 {
        out.push(mt | (arg as u8));
    } else if arg <= u64::from(u8::MAX) {
        out.push(mt | 24);
        out.push(arg as u8);
    } else if arg <= u64::from(u16::MAX) {
        out.push(mt | 25);
        out.extend_from_slice(&(arg as u16).to_be_bytes());
    } else if arg <= u64::from(u32::MAX) {
        out.push(mt | 26);
        out.extend_from_slice(&(arg as u32).to_be_bytes());
    } else {
        out.push(mt | 27);
        out.extend_from_slice(&arg.to_be_bytes());
    }
}

/// CBOR byte string (major type 2).
fn cbor_bstr(out: &mut Vec<u8>, b: &[u8]) {
    cbor_head(out, 2, b.len() as u64);
    out.extend_from_slice(b);
}

/// CBOR text string (major type 3).
fn cbor_tstr(out: &mut Vec<u8>, s: &str) {
    cbor_head(out, 3, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// CBOR unsigned integer (major type 0).
fn cbor_uint(out: &mut Vec<u8>, n: u64) {
    cbor_head(out, 0, n);
}

/// CBOR integer (major type 0 for `>= 0`, major type 1 for negatives).
fn cbor_int(out: &mut Vec<u8>, n: i32) {
    if n >= 0 {
        cbor_head(out, 0, n as u64);
    } else {
        cbor_head(out, 1, (-1 - i64::from(n)) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // RFC 8613 Appendix C.1.1 — Test Vector 1, client. The CLIENT's Sender ID is
    // the EMPTY byte string; Recipient ID = 0x01. Byte-exact ground truth.
    #[test]
    fn rfc8613_c1_1_client() {
        let ctx = SecurityContext::derive(
            &hex("0102030405060708090a0b0c0d0e0f10"), // Master Secret
            &hex("9e7ca92223786340"),                 // Master Salt
            &[],                                      // Sender ID (empty)
            &hex("01"),                               // Recipient ID
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        assert_eq!(ctx.sender_key, hex("f0910ed7295e6ad4b54fc793154302ff"));
        assert_eq!(ctx.recipient_key, hex("ffb14e093c94c9cac9471648b4f98710"));
        assert_eq!(ctx.common_iv, hex("4622d4dd6d944168eefb54987c"));
    }

    // RFC 8613 Appendix C.1.2 — Test Vector 1, server. Mirror of the client:
    // Sender ID = 0x01, Recipient ID = empty; the keys swap.
    #[test]
    fn rfc8613_c1_2_server_mirrors_client() {
        let server = SecurityContext::derive(
            &hex("0102030405060708090a0b0c0d0e0f10"),
            &hex("9e7ca92223786340"),
            &hex("01"), // server is Sender
            &[],        // client is Recipient (empty)
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        assert_eq!(server.sender_key, hex("ffb14e093c94c9cac9471648b4f98710"));
        assert_eq!(
            server.recipient_key,
            hex("f0910ed7295e6ad4b54fc793154302ff")
        );
        assert_eq!(server.common_iv, hex("4622d4dd6d944168eefb54987c"));
    }

    // RFC 8613 Appendix C.2.1 — Test Vector 2, client. Here the client's Sender
    // ID is 0x00 (non-empty) — a second independent vector through the same code.
    // RFC 8613 Appendix C.2.1 — Test Vector 2, client, WITH an ID Context. This
    // exercises the `id_context` field of the HKDF info (CBOR bstr instead of
    // nil). The Recipient Key and Common IV below are the documented C.2.1 values
    // — matching them proves the ID-Context derivation path byte-exact.
    #[test]
    fn rfc8613_c2_1_client_with_id_context() {
        let ctx = SecurityContext::derive(
            &hex("0102030405060708090a0b0c0d0e0f10"),
            &hex("9e7ca92223786340"),
            &hex("00"),                     // Sender ID
            &hex("01"),                     // Recipient ID
            Some(&hex("37cbf3210017a2d3")), // ID Context
            AeadAlgorithm::AesCcm16_64_128,
        );
        assert_eq!(ctx.recipient_key, hex("e39a0c7c77b43f03b4b39ab9a268699f"));
        assert_eq!(ctx.common_iv, hex("2ca58fb85ff1b81c0b7181b85e"));
        // Sender Key for Sender ID 0x00 under this ID Context (same verified
        // derivation; locked as a regression alongside the documented anchors).
        assert_eq!(ctx.sender_key, hex("fcf9e255693e8d1f87dcbd42ab8cae30"));
    }

    // RFC 5869 Appendix A.1 — independent HKDF-SHA-256 test vector, isolating the
    // HKDF primitive from the OSCORE info structure.
    #[test]
    fn rfc5869_a1_hkdf() {
        let ikm = hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let salt = hex("000102030405060708090a0b0c");
        let info = hex("f0f1f2f3f4f5f6f7f8f9");
        let prk = hkdf_extract(&salt, &ikm);
        assert_eq!(
            &prk[..],
            &hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5")[..],
            "PRK"
        );
        let mut okm = [0u8; 42];
        hkdf_expand(&prk, &info, &mut okm);
        assert_eq!(
            &okm[..],
            &hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )[..],
            "OKM"
        );
    }

    #[test]
    fn aead_default_params() {
        let a = AeadAlgorithm::AesCcm16_64_128;
        assert_eq!(a.cose_value(), 10);
        assert_eq!(a.key_len(), 16);
        assert_eq!(a.nonce_len(), 13);
        assert_eq!(a.tag_len(), 8);
    }

    #[test]
    fn cbor_info_shapes() {
        // [ h'00', nil, 10, "Key", 16 ] — canonical CBOR.
        let info = encode_info(&[0x00], None, 10, "Key", 16);
        assert_eq!(
            info,
            hex("8541 00 f6 0a 634b6579 10".replace(' ', "").as_str())
        );
        // [ h'', nil, 10, "IV", 13 ] — empty id (Common IV derivation).
        let iv = encode_info(&[], None, 10, "IV", 13);
        assert_eq!(iv, hex("8540 f6 0a 624956 0d".replace(' ', "").as_str()));
    }
}
