// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message-level protection: `SRTPS_PREFIX` + `SRTPS_POSTFIX`.
//!
//! Spec §7.3.7: wraps a **whole** RTPS message (with embedded
//! submessages) into an outer protection layer. Used for
//! `rtps_protection_kind=ENCRYPT` in the governance XML.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Like `codec.rs`, the wrapper takes `&dyn CryptographicPlugin`.)
//!
//! ```text
//! [ RTPS header (20 bytes, plaintext) ]
//! [ SRTPS_PREFIX (4+16 bytes) ]
//! [ <encrypted body> ... ]
//! [ SRTPS_POSTFIX (4+0 bytes, empty MAC list in single-receiver mode) ]
//! ```
//!
//! The RTPS header (first 20 bytes) stays **plaintext**, so that
//! receivers can see the magic "RTPS" + version + VendorId + prefix
//! without decrypting first. Everything after it is encrypted +
//! authenticated via AES-GCM.

use alloc::vec::Vec;

use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};

use crate::codec::{SRTPS_POSTFIX, SRTPS_PREFIX, SecurityRtpsError};

/// RTPS header size (spec §8.3.3.1).
pub const RTPS_HEADER_LEN: usize = 20;

const FLAG_LE: u8 = 0x01;

/// `PreSharedKeyFlag` im SRTPS_PREFIX-Submessage-Header — Spec
/// DDS-Security 1.2 §10.9.1.
///
/// When set, the sender signals that the master keys are derived from
/// a **pre-shared key** (DDS:Crypto:PSK:AES-GCM-
/// GMAC:1.2) instead of from an X.509 DH handshake. Decoders may observe the
/// bit to select the correct crypto plugin — on
/// the wire the body layout (TransformIdentifier, body, MACs)
/// stays identical, only the key origin differs.
///
/// Bit position 0x02 (bit 1) — next to `FLAG_LE` (bit 0). Other
/// submessage flags (bits 2..7) are reserved.
pub const PRE_SHARED_KEY_FLAG: u8 = 0x02;

fn push_header(out: &mut Vec<u8>, id: u8, length: u16) {
    out.push(id);
    out.push(FLAG_LE);
    out.extend_from_slice(&length.to_le_bytes());
}

fn push_header_with_flags(out: &mut Vec<u8>, id: u8, flags: u8, length: u16) {
    out.push(id);
    out.push(flags);
    out.extend_from_slice(&length.to_le_bytes());
}

/// Like [`encode_secured_rtps_message`], but additionally sets the
/// `PreSharedKeyFlag` in the SRTPS_PREFIX (spec §10.9.1) — for the
/// PSK crypto path.
///
/// # Errors
/// Like [`encode_secured_rtps_message`].
pub fn encode_secured_rtps_message_psk(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    message: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    if message.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    let (header, body) = message.split_at(RTPS_HEADER_LEN);

    // AAD extension per DDS-Security 1.2 §7.4.6.6 (RTPS message
    // protection): reserved-4 || RTPS-Header[0..20]. Protects the
    // header against tampering — the plugin's `mat.aad(extension)`
    // additionally prepends `transformation_kind || key_id || session_id`.
    let mut aad_extension = Vec::with_capacity(4 + RTPS_HEADER_LEN);
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(header);

    let ciphertext = plugin
        .encrypt_submessage(local, remote_list, body, &aad_extension)
        .map_err(SecurityRtpsError::Crypto)?;

    let body_len = u16::try_from(ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("SRTPS body > u16"))?;

    let mut out = Vec::with_capacity(RTPS_HEADER_LEN + 4 + 16 + 4 + ciphertext.len() + 4);
    out.extend_from_slice(header);

    // SRTPS_PREFIX with PSK flag.
    push_header_with_flags(&mut out, SRTPS_PREFIX, FLAG_LE | PRE_SHARED_KEY_FLAG, 16);
    out.extend_from_slice(&[0u8; 16]);

    push_header(&mut out, crate::codec::SEC_BODY, body_len);
    out.extend_from_slice(&ciphertext);

    push_header(&mut out, SRTPS_POSTFIX, 0);

    Ok(out)
}

/// Reads the `PreSharedKeyFlag` bit from the SRTPS_PREFIX of a
/// secured RTPS message. Returns `None` if the wire is not a valid
/// SRTPS wrapping.
#[must_use]
pub fn srtps_psk_flag(wire: &[u8]) -> Option<bool> {
    if wire.len() < RTPS_HEADER_LEN + 4 {
        return None;
    }
    if wire[RTPS_HEADER_LEN] != SRTPS_PREFIX {
        return None;
    }
    Some(wire[RTPS_HEADER_LEN + 1] & PRE_SHARED_KEY_FLAG != 0)
}

/// Protects a **whole** RTPS message. The first 20 bytes (header)
/// stay plaintext; everything after it (the submessage stream) is
/// encrypted + authenticated. Output:
///
/// ```text
/// [ header (20) | SRTPS_PREFIX | encrypted body | SRTPS_POSTFIX ]
/// ```
///
/// # Errors
/// Input too short for the header, or a crypto-plugin error.
pub fn encode_secured_rtps_message(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    message: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    if message.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    let (header, body) = message.split_at(RTPS_HEADER_LEN);

    // AAD extension per DDS-Security 1.2 §7.4.6.6 (RTPS message
    // protection): reserved-4 || RTPS-Header[0..20]. Protects the
    // header against tampering — the plugin's `mat.aad(extension)`
    // additionally prepends `transformation_kind || key_id || session_id`.
    let mut aad_extension = Vec::with_capacity(4 + RTPS_HEADER_LEN);
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(header);

    let ciphertext = plugin
        .encrypt_submessage(local, remote_list, body, &aad_extension)
        .map_err(SecurityRtpsError::Crypto)?;

    let body_len = u16::try_from(ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("SRTPS body > u16"))?;

    let mut out = Vec::with_capacity(
        RTPS_HEADER_LEN    // plaintext header
            + 4 + 16           // SRTPS_PREFIX
            + 4 + ciphertext.len() // crypted body framed
            + 4, // SRTPS_POSTFIX
    );
    out.extend_from_slice(header);

    // SRTPS_PREFIX: 16-byte TransformIdentifier (0x00..0x00 = single-plugin path; multi-plugin identifiers are hand-allocated in the DCPS runtime).
    push_header(&mut out, SRTPS_PREFIX, 16);
    out.extend_from_slice(&[0u8; 16]);

    // Hook the cipher body in as a single submessage-like structure.
    // Here we set the CT bytes directly — the receiver does not learn the body
    // extent from the submessage length header of the POSTFIX,
    // so we need our own length marker. We use
    // a synthetic SEC_BODY shape: [0x30 0x01 len_lo len_hi ...].
    push_header(&mut out, crate::codec::SEC_BODY, body_len);
    out.extend_from_slice(&ciphertext);

    // SRTPS_POSTFIX empty — single-receiver mode.
    push_header(&mut out, SRTPS_POSTFIX, 0);

    Ok(out)
}

/// Unwraps a whole RTPS message. Expects the same format as
/// [`encode_secured_rtps_message`]. Returns the reconstructed
/// plaintext message (`[header | body]`).
///
/// # Errors
/// Tampered header, wire inconsistency, crypto verify fail.
pub fn decode_secured_rtps_message(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote: CryptoHandle,
    wire: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    if wire.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    let header = &wire[..RTPS_HEADER_LEN];
    let rest = &wire[RTPS_HEADER_LEN..];

    // SRTPS_PREFIX: 4 byte header + 16 byte body.
    if rest.len() < 4 + 16 {
        return Err(SecurityRtpsError::Truncated("SRTPS_PREFIX"));
    }
    if rest[0] != SRTPS_PREFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 0,
            expected: SRTPS_PREFIX,
            got: rest[0],
        });
    }
    if rest[1] & FLAG_LE == 0 {
        return Err(SecurityRtpsError::BigEndianNotSupported);
    }
    // prefix-body length (must be 16, but we blindly follow the
    // value from the header so future extensions are tolerated).
    let mut plen_b = [0u8; 2];
    plen_b.copy_from_slice(&rest[2..4]);
    let plen = u16::from_le_bytes(plen_b) as usize;
    let after_prefix = 4 + plen;
    if rest.len() < after_prefix {
        return Err(SecurityRtpsError::Truncated("SRTPS_PREFIX body"));
    }
    let rest = &rest[after_prefix..];

    // Body-Submessage (SEC_BODY-Kind).
    if rest.len() < 4 {
        return Err(SecurityRtpsError::Truncated("SRTPS body header"));
    }
    if rest[0] != crate::codec::SEC_BODY {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 1,
            expected: crate::codec::SEC_BODY,
            got: rest[0],
        });
    }
    if rest[1] & FLAG_LE == 0 {
        return Err(SecurityRtpsError::BigEndianNotSupported);
    }
    let mut blen_b = [0u8; 2];
    blen_b.copy_from_slice(&rest[2..4]);
    let blen = u16::from_le_bytes(blen_b) as usize;
    let after_body = 4 + blen;
    if rest.len() < after_body {
        return Err(SecurityRtpsError::Truncated("SRTPS body payload"));
    }
    let ciphertext = &rest[4..after_body];
    let after_body_rest = &rest[after_body..];

    // SRTPS_POSTFIX.
    if after_body_rest.len() < 4 {
        return Err(SecurityRtpsError::Truncated("SRTPS_POSTFIX"));
    }
    if after_body_rest[0] != SRTPS_POSTFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 2,
            expected: SRTPS_POSTFIX,
            got: after_body_rest[0],
        });
    }

    // AAD extension symmetric to the encoder.
    let mut aad_extension = Vec::with_capacity(4 + RTPS_HEADER_LEN);
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(header);

    let plain_body = plugin
        .decrypt_submessage(local, remote, ciphertext, &aad_extension)
        .map_err(SecurityRtpsError::Crypto)?;

    let mut out = Vec::with_capacity(RTPS_HEADER_LEN + plain_body.len());
    out.extend_from_slice(header);
    out.extend_from_slice(&plain_body);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
    use zerodds_security::error::SecurityErrorKind;
    use zerodds_security_crypto::AesGcmCryptoPlugin;

    fn make_plugin() -> (AesGcmCryptoPlugin, CryptoHandle, CryptoHandle) {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        (p, local, remote)
    }

    fn fake_rtps_message(body: &[u8]) -> Vec<u8> {
        // 20-byte header: "RTPS" + version(2) + vendor(2) + prefix(12)
        let mut m = Vec::with_capacity(RTPS_HEADER_LEN + body.len());
        m.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        m.extend_from_slice(&[0u8; 12]); // guid prefix
        m.extend_from_slice(body);
        m
    }

    #[test]
    fn encode_keeps_header_in_plaintext() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"[DATA submessage plaintext]");
        let wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        // First 4 bytes = "RTPS" magic.
        assert_eq!(&wire[..4], b"RTPS");
        // The rest of the 20 bytes is identical to msg[..20].
        assert_eq!(&wire[..RTPS_HEADER_LEN], &msg[..RTPS_HEADER_LEN]);
        // SRTPS_PREFIX follows directly.
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    }

    #[test]
    fn encode_body_is_not_in_wire_plain() {
        let (p, local, remote) = make_plugin();
        let secret_body = b"TOP-SECRET submessage body";
        let msg = fake_rtps_message(secret_body);
        let wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        assert!(
            !wire.windows(secret_body.len()).any(|w| w == secret_body),
            "plaintext body must be encrypted"
        );
    }

    #[test]
    fn cross_instance_srtps_roundtrip_via_token() {
        // E3b reproduction: A encodes message-level SRTPS with ITS local key;
        // B decodes with the key exchanged via ParticipantCryptoToken
        // (two plugin instances = the real cross-instance/cross-vendor case).
        // The existing #16 tests use ONLY the same handle (encode+decode with
        // the same `local`) and never hit the slot/key asymmetry.
        let mut pa = AesGcmCryptoPlugin::new();
        let local_a = pa
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let token_a = pa
            .create_local_participant_crypto_tokens(local_a, CryptoHandle(0))
            .unwrap();

        let mut pb = AesGcmCryptoPlugin::new();
        let local_b = pb
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote_a_at_b = pb
            .register_matched_remote_participant(local_b, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        pb.set_remote_participant_crypto_tokens(local_b, remote_a_at_b, &token_a)
            .unwrap();

        let msg = fake_rtps_message(b"[SEDP DATA cross-instance srtps]");
        let wire = encode_secured_rtps_message(&pa, local_a, &[], &msg).unwrap();
        let back = decode_secured_rtps_message(&pb, remote_a_at_b, remote_a_at_b, &wire)
            .expect("cross-instance decode (E3b)");
        assert_eq!(back, msg, "cross-instance SRTPS body must recover");
    }

    #[test]
    fn message_roundtrip_recovers_body() {
        let (p, local, remote) = make_plugin();
        let body = b"[HEARTBEAT][DATA][GAP]";
        let msg = fake_rtps_message(body);
        let wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        let back = decode_secured_rtps_message(&p, local, remote, &wire).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn message_too_short_rejected() {
        let (p, local, remote) = make_plugin();
        let err = encode_secured_rtps_message(&p, local, &[remote], &[0u8; 10]).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::Truncated(_)));
    }

    #[test]
    fn tampered_ciphertext_fails_verify() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"secure submessage stream");
        let mut wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        // Flip a byte in the ciphertext region (after header + prefix + body-hdr).
        let flip_idx = RTPS_HEADER_LEN + 4 + 16 + 4 + 12;
        wire[flip_idx] ^= 0x10;
        let err = decode_secured_rtps_message(&p, local, remote, &wire).unwrap_err();
        match err {
            SecurityRtpsError::Crypto(e) => {
                assert_eq!(e.kind, SecurityErrorKind::CryptoFailed);
            }
            other => panic!("expected Crypto error, got {other:?}"),
        }
    }

    #[test]
    fn missing_srtps_prefix_rejected() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"x");
        let mut wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        wire[RTPS_HEADER_LEN] = 0x15; // fake andere submessage
        let err = decode_secured_rtps_message(&p, local, remote, &wire).unwrap_err();
        assert!(matches!(
            err,
            SecurityRtpsError::UnexpectedSubmessageId {
                pos: 0,
                expected: SRTPS_PREFIX,
                ..
            }
        ));
    }

    #[test]
    fn psk_encode_sets_pre_shared_key_flag() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"psk-protected body");
        let wire = encode_secured_rtps_message_psk(&p, local, &[remote], &msg).unwrap();
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
        // The flags byte carries both LE and PRE_SHARED_KEY_FLAG.
        let flags = wire[RTPS_HEADER_LEN + 1];
        assert!(flags & FLAG_LE != 0);
        assert!(flags & PRE_SHARED_KEY_FLAG != 0);
        assert_eq!(srtps_psk_flag(&wire), Some(true));
    }

    #[test]
    fn non_psk_encode_does_not_set_pre_shared_key_flag() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"non-psk body");
        let wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        assert_eq!(srtps_psk_flag(&wire), Some(false));
    }

    #[test]
    fn psk_encoded_message_decodes_with_classic_decoder() {
        // Spec §10.9: the wire layout is identical — the PSK flag in the
        // SRTPS_PREFIX is informative. The classic decoder may still
        // unpack the body (the LE bit is set).
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"interop-test");
        let wire = encode_secured_rtps_message_psk(&p, local, &[remote], &msg).unwrap();
        let back = decode_secured_rtps_message(&p, local, remote, &wire).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn srtps_psk_flag_returns_none_for_non_srtps() {
        assert_eq!(srtps_psk_flag(&[]), None);
        assert_eq!(srtps_psk_flag(&[0u8; 30]), None);
    }

    #[test]
    fn big_endian_srtps_rejected() {
        let (p, local, remote) = make_plugin();
        let msg = fake_rtps_message(b"x");
        let mut wire = encode_secured_rtps_message(&p, local, &[remote], &msg).unwrap();
        // Set the flags byte at SRTPS_PREFIX to BE.
        wire[RTPS_HEADER_LEN + 1] = 0x00;
        let err = decode_secured_rtps_message(&p, local, remote, &wire).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::BigEndianNotSupported));
    }
}
