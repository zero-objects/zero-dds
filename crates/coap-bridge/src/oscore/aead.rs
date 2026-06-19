//! OSCORE AEAD layer (RFC 8613 §5): AES-CCM-16-64-128, the per-message nonce
//! construction (§5.2), the integrity-protection AAD / COSE Encrypt0
//! `Enc_structure` (§5.4), and the message protect/unprotect.
//!
//! Builds on the Security Context key derivation in [`super`]. AES-CCM is the
//! OSCORE mandatory AEAD (COSE algorithm 10): 128-bit key, 13-byte nonce, 8-byte
//! tag. Correctness is anchored to RFC 3610 Packet Vector #1 (the canonical
//! AES-CCM vector) for the primitive, plus spec-deterministic nonce/AAD vectors
//! and a protect->unprotect round-trip.
//!
//! `no_std + alloc`.

extern crate alloc;

use alloc::vec::Vec;

use aes::Aes128;
use ccm::Ccm;
use ccm::aead::{Aead, KeyInit, Payload};
use ccm::consts::{U8, U13};

use super::{AeadAlgorithm, OscoreError, SecurityContext};

/// AES-CCM with an 8-byte tag and a 13-byte nonce = AES-CCM-16-64-128.
type AesCcm16_64_128 = Ccm<Aes128, U8, U13>;

/// AES-CCM-16-64-128 encrypt: returns `ciphertext || tag` (RFC 8613 default AEAD).
///
/// # Errors
/// `Err` if the key length is wrong or the AEAD backend fails.
pub fn aes_ccm_encrypt(
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, OscoreError> {
    let cipher = AesCcm16_64_128::new_from_slice(key).map_err(|_| OscoreError)?;
    let nonce = ccm::aead::Nonce::<AesCcm16_64_128>::from_slice(nonce);
    cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| OscoreError)
}

/// AES-CCM-16-64-128 decrypt of `ciphertext || tag`; `Err` on auth failure.
///
/// # Errors
/// `Err` if the tag does not verify or inputs are malformed.
pub fn aes_ccm_decrypt(
    key: &[u8],
    nonce: &[u8],
    ct_and_tag: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, OscoreError> {
    let cipher = AesCcm16_64_128::new_from_slice(key).map_err(|_| OscoreError)?;
    let nonce = ccm::aead::Nonce::<AesCcm16_64_128>::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ct_and_tag,
                aad,
            },
        )
        .map_err(|_| OscoreError)
}

/// Build the OSCORE AEAD nonce (RFC 8613 §5.2): `nonce_len` bytes.
///
/// Layout (before the XOR): `[ S (1 byte) | ID_PIV left-padded to nonce_len-6 |
/// Partial IV left-padded to 5 ]`, then XOR with the Common IV. `S` is the byte
/// length of `id_piv` (the Sender ID of the party that generated the Partial IV).
#[must_use]
pub fn oscore_nonce(common_iv: &[u8], id_piv: &[u8], partial_iv: &[u8]) -> Vec<u8> {
    let nlen = common_iv.len();
    let mut buf = alloc::vec![0u8; nlen];
    // S = size of ID_PIV.
    buf[0] = id_piv.len() as u8;
    // ID_PIV, right-aligned into bytes [1 .. nlen-5).
    let id_region_end = nlen - 5;
    if id_piv.len() < id_region_end {
        let start = id_region_end - id_piv.len();
        buf[start..id_region_end].copy_from_slice(id_piv);
    }
    // Partial IV, right-aligned into the last 5 bytes.
    if partial_iv.len() <= 5 {
        let start = nlen - partial_iv.len();
        buf[start..nlen].copy_from_slice(partial_iv);
    }
    for i in 0..nlen {
        buf[i] ^= common_iv[i];
    }
    buf
}

/// Build the OSCORE integrity-protection AAD (RFC 8613 §5.4): the COSE Encrypt0
/// `Enc_structure` `[ "Encrypt0", h'', external_aad ]`, where `external_aad` is
/// the CBOR array `[ oscore_version(1), [alg_aead], request_kid, request_piv,
/// class_I_options ]`. Hand-encoded canonical CBOR.
#[must_use]
pub fn oscore_aad(
    alg: AeadAlgorithm,
    request_kid: &[u8],
    request_piv: &[u8],
    class_i_options: &[u8],
) -> Vec<u8> {
    // external_aad = [ 1, [alg], kid, piv, options ]
    let mut ext = Vec::new();
    ext.push(0x85); // array(5)
    super_cbor_uint(&mut ext, 1); // oscore_version
    ext.push(0x81); // array(1) of algorithms
    super_cbor_int(&mut ext, alg.cose_value());
    super_cbor_bstr(&mut ext, request_kid);
    super_cbor_bstr(&mut ext, request_piv);
    super_cbor_bstr(&mut ext, class_i_options);

    // Enc_structure = [ "Encrypt0", h'' (empty protected), bstr(external_aad) ]
    let mut aad = Vec::new();
    aad.push(0x83); // array(3)
    super_cbor_tstr(&mut aad, "Encrypt0");
    super_cbor_bstr(&mut aad, &[]); // empty protected header
    super_cbor_bstr(&mut aad, &ext); // external_aad as a byte string
    aad
}

/// Protect a plaintext for a request (RFC 8613 §8.1): produce the COSE Encrypt0
/// ciphertext (`ciphertext || tag`) under the Sender Key, with the nonce + AAD
/// derived from the Sender ID + Partial IV. For a request, `request_kid` =
/// Sender ID and `request_piv` = Partial IV.
///
/// # Errors
/// `Err` on AEAD failure.
pub fn protect_request(
    ctx: &SecurityContext,
    sender_id: &[u8],
    partial_iv: &[u8],
    class_i_options: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, OscoreError> {
    let nonce = oscore_nonce(&ctx.common_iv, sender_id, partial_iv);
    let aad = oscore_aad(ctx.alg, sender_id, partial_iv, class_i_options);
    aes_ccm_encrypt(&ctx.sender_key, &nonce, plaintext, &aad)
}

/// Unprotect a request ciphertext (RFC 8613 §8.2): verify + decrypt under the
/// Recipient Key, with the nonce + AAD reconstructed from the request's Sender
/// ID (= the peer's `kid`) + Partial IV.
///
/// # Errors
/// `Err` if the tag does not verify (replay/tamper) or inputs are malformed.
pub fn unprotect_request(
    ctx: &SecurityContext,
    request_kid: &[u8],
    partial_iv: &[u8],
    class_i_options: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, OscoreError> {
    let nonce = oscore_nonce(&ctx.common_iv, request_kid, partial_iv);
    let aad = oscore_aad(ctx.alg, request_kid, partial_iv, class_i_options);
    aes_ccm_decrypt(&ctx.recipient_key, &nonce, ciphertext, &aad)
}

// --- minimal canonical-CBOR helpers (mirror super::encode_info's encoder) ---
fn super_cbor_head(out: &mut Vec<u8>, major: u8, arg: u64) {
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
fn super_cbor_uint(out: &mut Vec<u8>, n: u64) {
    super_cbor_head(out, 0, n);
}
fn super_cbor_int(out: &mut Vec<u8>, n: i32) {
    if n >= 0 {
        super_cbor_head(out, 0, n as u64);
    } else {
        super_cbor_head(out, 1, (-1 - i64::from(n)) as u64);
    }
}
fn super_cbor_bstr(out: &mut Vec<u8>, b: &[u8]) {
    super_cbor_head(out, 2, b.len() as u64);
    out.extend_from_slice(b);
}
fn super_cbor_tstr(out: &mut Vec<u8>, s: &str) {
    super_cbor_head(out, 3, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: alloc::string::String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // RFC 3610 Packet Vector #1 — the canonical AES-CCM (M=8, L=2 = AES-CCM-
    // 16-64-128) test vector. Anchors the AEAD primitive independent of OSCORE.
    #[test]
    fn rfc3610_packet_vector_1() {
        let key = hex("C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF");
        let nonce = hex("00000003020100A0A1A2A3A4A5");
        let aad = hex("0001020304050607");
        let pt = hex("08090A0B0C0D0E0F101112131415161718191A1B1C1D1E");
        let expected = hex("588C979A61C663D2F066D0C2C0F989806D5F6B61DAC38417E8D12CFDF926E0");
        let ct = aes_ccm_encrypt(&key, &nonce, &pt, &aad).unwrap();
        assert_eq!(ct, expected, "RFC 3610 PV#1 ciphertext+tag");
        let back = aes_ccm_decrypt(&key, &nonce, &ct, &aad).unwrap();
        assert_eq!(back, pt, "decrypt round-trips");
        // tamper -> auth failure
        let mut bad = ct.clone();
        bad[0] ^= 0x01;
        assert!(aes_ccm_decrypt(&key, &nonce, &bad, &aad).is_err());
    }

    // RFC 8613 §5.2 nonce — spec-deterministic from the C.1.1 Common IV
    // (4622d4dd6d944168eefb54987c), an empty Sender ID, and Partial IV 0x14.
    // nonce_input = 00 | 00..00(7) | 00 00 00 00 14, XOR Common IV.
    #[test]
    fn oscore_nonce_c1_piv20() {
        let common_iv = hex("4622d4dd6d944168eefb54987c");
        let nonce = oscore_nonce(&common_iv, &[], &[0x14]);
        assert_eq!(nonce, hex("4622d4dd6d944168eefb549868"));
    }

    // RFC 8613 §5.4 AAD — external_aad [1,[10],h'',h'14',h''] inside the COSE
    // Encrypt0 Enc_structure. Canonical CBOR is deterministic.
    #[test]
    fn oscore_aad_structure() {
        let aad = oscore_aad(AeadAlgorithm::AesCcm16_64_128, &[], &[0x14], &[]);
        // external_aad = 85 01 81 0a 40 41 14 40  (8 bytes)
        // Enc_structure = 83 | 68 "Encrypt0" | 40 | 48 <external_aad>
        let expected = hex("83 68 456e63727970743 0 40 48 85 01 81 0a 40 41 14 40"
            .replace(' ', "")
            .as_str());
        assert_eq!(aad, expected);
    }

    // Full protect -> unprotect round-trip over a derived C.1.1-style context.
    #[test]
    fn protect_unprotect_roundtrip() {
        // Client context C.1.1 (sender id empty) and the mirrored server context
        // C.1.2 (the server's recipient_key == client's sender_key).
        let client = SecurityContext::derive(
            &hex("0102030405060708090a0b0c0d0e0f10"),
            &hex("9e7ca92223786340"),
            &[],
            &hex("01"),
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        let server = SecurityContext::derive(
            &hex("0102030405060708090a0b0c0d0e0f10"),
            &hex("9e7ca92223786340"),
            &hex("01"),
            &[],
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        let plaintext = b"\x01gETcoap"; // arbitrary inner-message bytes
        let piv = [0x05u8];
        let ct = protect_request(&client, &[], &piv, &[], plaintext).unwrap();
        // Server unprotects with its Recipient Key (== client Sender Key) + the
        // request kid (the client Sender ID = empty) + the same Partial IV.
        let pt = unprotect_request(&server, &[], &piv, &[], &ct).unwrap();
        assert_eq!(pt, plaintext);
        // Wrong Partial IV (replay/tamper) -> auth failure.
        assert!(unprotect_request(&server, &[], &[0x06], &[], &ct).is_err());
    }
}
