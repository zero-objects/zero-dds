// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message-Level-Schutz: `SRTPS_PREFIX` + `SRTPS_POSTFIX`.
//!
//! Spec §7.3.7: wickelt eine **ganze** RTPS-Message (mit eingebetteten
//! Submessages) in einen äußeren Schutz-Layer. Genutzt für
//! `rtps_protection_kind=ENCRYPT` im Governance-XML.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Wie `codec.rs` nimmt der Wrapper `&dyn CryptographicPlugin`.)
//!
//! ```text
//! [ RTPS-Header (20 byte, plaintext) ]
//! [ SRTPS_PREFIX (4+16 byte) ]
//! [ <encrypted body> ... ]
//! [ SRTPS_POSTFIX (4+0 byte, leere MAC-Liste im Single-Receiver-Modus) ]
//! ```
//!
//! Der RTPS-Header (ersten 20 byte) bleibt **plaintext**, damit
//! Receiver die Magic "RTPS" + Version + VendorId + Prefix sehen
//! können, ohne erst zu entschluesseln. Alles dahinter wird via
//! AES-GCM verschluesselt + authentifiziert.

use alloc::vec::Vec;

use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};

use crate::codec::{SRTPS_POSTFIX, SRTPS_PREFIX, SecurityRtpsError};

/// RTPS-Header-Groesse (Spec §8.3.3.1).
pub const RTPS_HEADER_LEN: usize = 20;

const FLAG_LE: u8 = 0x01;

/// `PreSharedKeyFlag` im SRTPS_PREFIX-Submessage-Header — Spec
/// DDS-Security 1.2 §10.9.1.
///
/// Wenn gesetzt, signalisiert der Sender, dass die Master-Keys aus
/// einem **Pre-Shared-Key** abgeleitet sind (DDS:Crypto:PSK:AES-GCM-
/// GMAC:1.2) statt aus einem X.509-DH-Handshake. Decoder duerfen das
/// Bit beobachten, um den korrekten Crypto-Plugin auszuwaehlen — auf
/// dem Wire bleibt der Body-Layout (TransformIdentifier, Body, MACs)
/// identisch, nur die Key-Herkunft unterscheidet sich.
///
/// Bit-Position 0x02 (Bit 1) — neben `FLAG_LE` (Bit 0). Andere
/// Submessage-Flags (Bits 2..7) sind reserviert.
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

/// Wie [`encode_secured_rtps_message`], aber setzt zusaetzlich den
/// `PreSharedKeyFlag` im SRTPS_PREFIX (Spec §10.9.1) — fuer den
/// PSK-Crypto-Pfad.
///
/// # Errors
/// Wie [`encode_secured_rtps_message`].
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

    // AAD-Extension per DDS-Security 1.2 §7.4.6.6 (RTPS-Message-
    // Protection): reserved-4 || RTPS-Header[0..20]. Schützt den
    // Header gegen Tampering — der Plugin's `mat.aad(extension)`
    // prependet zusätzlich `transformation_kind || key_id || session_id`.
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

    // SRTPS_PREFIX mit PSK-Flag.
    push_header_with_flags(&mut out, SRTPS_PREFIX, FLAG_LE | PRE_SHARED_KEY_FLAG, 16);
    out.extend_from_slice(&[0u8; 16]);

    push_header(&mut out, crate::codec::SEC_BODY, body_len);
    out.extend_from_slice(&ciphertext);

    push_header(&mut out, SRTPS_POSTFIX, 0);

    Ok(out)
}

/// Liest den `PreSharedKeyFlag`-Bit aus dem SRTPS_PREFIX einer
/// secured RTPS-Message. Liefert `None` wenn die Wire kein gueltiges
/// SRTPS-Wrapping ist.
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

/// Schuetzt eine **ganze** RTPS-Message. Die ersten 20 byte (Header)
/// bleiben plaintext; alles dahinter (Submessage-Stream) wird
/// verschluesselt + authentifiziert. Output:
///
/// ```text
/// [ header (20) | SRTPS_PREFIX | encrypted body | SRTPS_POSTFIX ]
/// ```
///
/// # Errors
/// Input zu kurz fuer den Header oder Crypto-Plugin-Fehler.
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

    // AAD-Extension per DDS-Security 1.2 §7.4.6.6 (RTPS-Message-
    // Protection): reserved-4 || RTPS-Header[0..20]. Schützt den
    // Header gegen Tampering — der Plugin's `mat.aad(extension)`
    // prependet zusätzlich `transformation_kind || key_id || session_id`.
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

    // SRTPS_PREFIX: 16-byte TransformIdentifier (0x00..0x00 = Single-Plugin-Pfad; Multi-Plugin-Identifier sind im DCPS-Runtime hand-allokiert).
    push_header(&mut out, SRTPS_PREFIX, 16);
    out.extend_from_slice(&[0u8; 16]);

    // Cipherbody als einzelne Submessage-artige Struktur einhaengen.
    // Hier setzen wir die CT-Bytes direkt — der Empfaenger weiss ueber
    // den Submessage-Laengen-Header vom POSTFIX den Body-Umfang nicht,
    // deshalb brauchen wir einen eigenen Length-Marker. Wir nutzen
    // einen synthetischen SEC_BODY-Shape: [0x30 0x01 len_lo len_hi ...].
    push_header(&mut out, crate::codec::SEC_BODY, body_len);
    out.extend_from_slice(&ciphertext);

    // SRTPS_POSTFIX leer — Single-Receiver-Modus.
    push_header(&mut out, SRTPS_POSTFIX, 0);

    Ok(out)
}

/// Unwrap eine ganze RTPS-Message. Erwartet das gleiche Format wie
/// [`encode_secured_rtps_message`]. Liefert die rekonstruierte
/// plaintext-Message (`[header | body]`).
///
/// # Errors
/// Tampered Header, Wire-Inkonsistenz, Crypto-Verify-Fail.
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
    // prefix-body length (muss 16 sein, aber wir folgen blind dem
    // Wert aus dem Header, damit zukuenftige Extensions stoeren).
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

    // AAD-Extension symmetrisch zum Encoder.
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
        // Erste 4 byte = "RTPS" magic.
        assert_eq!(&wire[..4], b"RTPS");
        // Rest der 20 byte ist identisch zu msg[..20].
        assert_eq!(&wire[..RTPS_HEADER_LEN], &msg[..RTPS_HEADER_LEN]);
        // SRTPS_PREFIX folgt direkt.
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
            "plaintext body muss verschluesselt sein"
        );
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
        // Flip byte im ciphertext-Bereich (nach header + prefix + body-hdr).
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
        // Flags-Byte traegt sowohl LE als auch PRE_SHARED_KEY_FLAG.
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
        // Spec §10.9: das Wire-Layout ist identisch — der PSK-Flag im
        // SRTPS_PREFIX ist informativ. Der klassische Decoder darf den
        // body trotzdem auspacken (das LE-Bit ist gesetzt).
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
        // Flags-Byte beim SRTPS_PREFIX auf BE setzen.
        wire[RTPS_HEADER_LEN + 1] = 0x00;
        let err = decode_secured_rtps_message(&p, local, remote, &wire).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::BigEndianNotSupported));
    }
}
