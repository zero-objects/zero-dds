// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message-level protection: `SRTPS_PREFIX` + `SRTPS_POSTFIX`.
//!
//! DDS-Security 1.2 §7.4.7.8, §7.4.7.9 and §10.5.3.3.4.6: wraps a **whole**
//! RTPS message into an outer protection layer. Used for
//! `rtps_protection_kind` other than NONE in the governance XML.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Like `codec.rs`, the wrapper takes `&dyn CryptographicPlugin`.)
//!
//! ```text
//! [ RTPS header (20 octets, unchanged) ]
//! [ SRTPS_PREFIX: CryptoHeader (20 octets) ]
//! [ SEC_BODY: CryptoContent, Encrypt(RTPSMessage{RTPSHdr -> InfoSourceSubMsg}) ]
//! [ SRTPS_POSTFIX: CryptoFooter ]
//! ```
//!
//! This is the form of §10.5.3.3.4.6.4 (authenticated encryption, no
//! AdditionalAuthenticatedData): the header goes into the protected body as
//! an INFO_SOURCE, so the AdditionalAuthenticatedDataFlag stays clear. The
//! CryptoHeader names the key, so the plugin that holds it writes the frame
//! ([`CryptographicPlugin::encode_rtps_message_cyclone`]); before, the
//! SRTPS_PREFIX carried 16 zero octets and the SEC_BODY no length field.

use alloc::vec::Vec;

use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};

use crate::codec::{SRTPS_PREFIX, SecurityRtpsError};

/// RTPS header size (spec §8.3.3.1).
pub const RTPS_HEADER_LEN: usize = 20;

/// `PreSharedKeyFlag` in the SRTPS_PREFIX-Submessage-Header — Spec
/// DDS-Security 1.2 §7.4.7.8.3, Table 7.
///
/// When set, the sender signals that the master keys are derived from
/// a **pre-shared key** (DDS:Crypto:PSK:AES-GCM-
/// GMAC:1.2) instead of from an X.509 DH handshake. Decoders may observe the
/// bit to select the correct crypto plugin — on
/// the wire the body layout (TransformIdentifier, body, MACs)
/// stays identical, only the key origin differs.
///
/// Bit position 0x04 (bit 2): the wire representation of §7.4.7.8.1 is
/// `|X|X|X|X|X|P|A|E|`, and Table 7 has `P = SubmessageHeader.flags & 0x04`.
/// Bit 1 (0x02) is the AdditionalAuthenticatedDataFlag; the constant was
/// 0x02 and marked a PSK message as one with AAD.
pub const PRE_SHARED_KEY_FLAG: u8 = 0x04;

/// Like [`encode_secured_rtps_message`], protected with the pre-shared key
/// of the participant `local` (DDS-Security 1.2 §10.9.3.3.3 Table 89): the
/// PreSharedKeyFlag is set in the SRTPS_PREFIX.
///
/// # Errors
/// Like [`encode_secured_rtps_message`].
pub fn encode_secured_rtps_message_psk(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    message: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let _ = remote_list;
    if message.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    Ok(plugin.encode_rtps_message_psk(local, message)?)
}

/// The submessage id of the HeaderExtension (DDSI-RTPS 2.5 §9.4.5.1.1).
const HEADER_EXTENSION: u8 = 0x00;

/// Where the SRTPS_PREFIX of a protected RTPS message begins: directly
/// behind the RTPS Header or, if the message has one, behind its
/// HeaderExtension. DDS-Security 1.2 §10.5.3.3.1 Table 80: the output of
/// `encode_rtps_message` is "the original RTPS Header and, if present, the
/// original HeaderExtension, followed by the SecureRTPSPrefixSubMsg"; a
/// message sent over a stream carries its length in a HeaderExtension
/// (DDSI-RTPS 2.5 §8.3.3), which stands directly behind the header
/// (§8.3.7.3). `None` for a message whose first submessage, the
/// HeaderExtension aside, is no SRTPS_PREFIX with its four header octets:
/// a plain message, or one cut short.
#[must_use]
pub fn srtps_prefix_offset(wire: &[u8]) -> Option<usize> {
    let mut at = RTPS_HEADER_LEN;
    if wire.get(at) == Some(&HEADER_EXTENSION) {
        // id, flags (E is bit 0), octetsToNextHeader.
        let flags = *wire.get(at + 1)?;
        let len = [*wire.get(at + 2)?, *wire.get(at + 3)?];
        let len = if flags & 0x01 != 0 {
            u16::from_le_bytes(len)
        } else {
            u16::from_be_bytes(len)
        };
        at += 4 + usize::from(len);
    }
    (wire.get(at) == Some(&SRTPS_PREFIX) && wire.len() >= at + 4).then_some(at)
}

/// Whether `wire` is an RTPS message protected as a whole: it has an
/// SRTPS_PREFIX where [`srtps_prefix_offset`] finds one.
#[must_use]
pub fn is_secured_rtps_message(wire: &[u8]) -> bool {
    srtps_prefix_offset(wire).is_some()
}

/// The flags octet of the SRTPS_PREFIX of `wire` (DDS-Security 1.2
/// §7.4.7.8.1: `|X|X|X|X|X|P|A|E|`); `None` for a message without one.
#[must_use]
pub fn srtps_prefix_flags(wire: &[u8]) -> Option<u8> {
    srtps_prefix_offset(wire).and_then(|at| wire.get(at + 1).copied())
}

/// Reads the `PreSharedKeyFlag` bit from the SRTPS_PREFIX of a
/// secured RTPS message, behind the RTPS Header and the HeaderExtension,
/// if there is one. Returns `None` if the wire is not a valid SRTPS
/// wrapping.
#[must_use]
pub fn srtps_psk_flag(wire: &[u8]) -> Option<bool> {
    srtps_prefix_flags(wire).map(|flags| flags & PRE_SHARED_KEY_FLAG != 0)
}

/// Protects a **whole** RTPS message with the key material the plugin
/// holds under `local` (DDS-Security 1.2 §10.5.3.3.4.6.4). The RTPS header
/// stays as it was; the rest is the SRTPS_PREFIX, the protected body and
/// the SRTPS_POSTFIX. The receiver-specific MACs are those of the key
/// material; `remote_list` is not read.
///
/// # Errors
/// Input too short for the header, or a crypto-plugin error.
pub fn encode_secured_rtps_message(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    message: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let _ = remote_list;
    if message.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    Ok(plugin.encode_rtps_message_cyclone(local, message)?)
}

/// Unwraps a whole RTPS message with the key material the plugin holds
/// under `remote`, the sender's; with `remote` equal to `local`, a message
/// this participant sent, the plugin finds the key by the
/// `transformation_key_id` among all it holds, its own included. Returns
/// the plaintext message (`[header | body]`).
///
/// # Errors
/// No SRTPS_PREFIX with a CryptoHeader behind the header and the
/// HeaderExtension, if there is one; big-endian, cut short, or a crypto
/// verify fail.
pub fn decode_secured_rtps_message(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote: CryptoHandle,
    wire: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    if wire.len() < RTPS_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("rtps-message header"));
    }
    // Without an SRTPS_PREFIX behind a HeaderExtension the check runs on
    // what follows the header and names what it found there.
    let prefix = srtps_prefix_offset(wire).unwrap_or(RTPS_HEADER_LEN);
    crate::codec::check_prefix(&wire[prefix..], SRTPS_PREFIX, "SRTPS_PREFIX")?;
    if local == remote {
        Ok(plugin.decode_rtps_message_cyclone(wire)?)
    } else {
        Ok(plugin.decode_rtps_message_cyclone_from(remote, wire)?)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codec::{CRYPTO_HEADER_LEN, SEC_BODY, SRTPS_POSTFIX};
    use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
    use zerodds_security_crypto::AesGcmCryptoPlugin;

    fn fake_rtps_message(body: &[u8]) -> Vec<u8> {
        // 20-byte header: "RTPS" + version(2) + vendor(2) + prefix(12)
        let mut m = Vec::with_capacity(RTPS_HEADER_LEN + body.len());
        m.extend_from_slice(b"RTPS\x02\x05\x01\x0f");
        m.extend_from_slice(&[
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ]);
        m.extend_from_slice(body);
        m
    }

    /// A DATA-like body: an INFO_TS submessage (0x09, E, 8 octets).
    const BODY: [u8; 12] = [0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];

    /// Sender `a`, its token, and receiver `b` that knows `a` only from the
    /// token (Table 72).
    fn pair() -> (
        AesGcmCryptoPlugin,
        CryptoHandle,
        Vec<u8>,
        AesGcmCryptoPlugin,
        CryptoHandle,
    ) {
        let mut a = AesGcmCryptoPlugin::new();
        let local_a = a
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let token_a = a
            .create_local_participant_crypto_tokens(local_a, CryptoHandle(0))
            .unwrap();
        let mut b = AesGcmCryptoPlugin::new();
        let local_b = b
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let a_at_b = b
            .register_matched_remote_participant(local_b, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        b.set_remote_participant_crypto_tokens(local_b, a_at_b, &token_a)
            .unwrap();
        (a, local_a, token_a, b, a_at_b)
    }

    /// §10.5.3.3.4.6.4 (authenticated encryption, no AAD): the RTPS header
    /// unchanged; SRTPS_PREFIX with E set, A and P clear and a CryptoHeader of
    /// 20 octets whose transformation_kind and transformation_key_id are
    /// those of the sender's KeyMaterial_AES_GCM_GMAC (§10.5.2.1.1, §10.5.2.3);
    /// then SEC_BODY and SRTPS_POSTFIX. The receiver that has only the token
    /// reads the message back. Before, the SRTPS_PREFIX carried 16 zero octets.
    #[test]
    fn security_7_4_7_8_the_prefix_carries_the_crypto_header_of_the_senders_key() {
        let (a, local_a, token_a, b, a_at_b) = pair();
        let msg = fake_rtps_message(&BODY);
        let wire = encode_secured_rtps_message(&a, local_a, &[], &msg).unwrap();
        assert_eq!(wire[..RTPS_HEADER_LEN], msg[..RTPS_HEADER_LEN]);
        let rest = &wire[RTPS_HEADER_LEN..];
        assert_eq!(rest[0], SRTPS_PREFIX);
        assert_eq!(rest[1] & 0x07, 0x01, "E set, A and P clear");
        assert_eq!(
            usize::from(u16::from_le_bytes([rest[2], rest[3]])),
            CRYPTO_HEADER_LEN
        );
        let salt = u32::from_be_bytes(token_a[4..8].try_into().unwrap()) as usize;
        assert_eq!(rest[4..8], token_a[..4], "transformation_kind");
        assert_ne!(rest[4..8], [0; 4], "CRYPTO_TRANSFORMATION_KIND_NONE");
        assert_eq!(
            rest[8..12],
            token_a[8 + salt..12 + salt],
            "transformation_key_id"
        );
        let body_at = 4 + CRYPTO_HEADER_LEN;
        assert_eq!(rest[body_at], SEC_BODY);
        let body_len = usize::from(u16::from_le_bytes([rest[body_at + 2], rest[body_at + 3]]));
        assert_eq!(rest[body_at + 4 + body_len], SRTPS_POSTFIX);
        assert!(!wire.windows(BODY.len()).any(|w| w == BODY));
        let back = decode_secured_rtps_message(&b, a_at_b, a_at_b, &wire).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn the_wire_is_checked_before_the_plugin_reads_it() {
        let (a, local_a, _, b, a_at_b) = pair();
        assert!(matches!(
            encode_secured_rtps_message(&a, local_a, &[], &[0u8; 19]),
            Err(SecurityRtpsError::Truncated(_))
        ));
        let msg = fake_rtps_message(&BODY);
        assert!(matches!(
            decode_secured_rtps_message(&b, a_at_b, a_at_b, &msg),
            Err(SecurityRtpsError::UnexpectedSubmessageId { .. })
        ));
        let wire = encode_secured_rtps_message(&a, local_a, &[], &msg).unwrap();
        let mut big_endian = wire.clone();
        big_endian[RTPS_HEADER_LEN + 1] &= !0x01;
        assert_eq!(
            decode_secured_rtps_message(&b, a_at_b, a_at_b, &big_endian).unwrap_err(),
            SecurityRtpsError::BigEndianNotSupported
        );
        let mut changed = wire.clone();
        let last = changed.len() - 30;
        changed[last] ^= 0x01;
        assert!(decode_secured_rtps_message(&b, a_at_b, a_at_b, &changed).is_err());
    }

    /// §7.4.6.6 Table 7, §7.4.7.8.3: a message the plugin of DDS:Crypto:PSK
    /// protects (§10.9.3.1, Table 89) carries E, A and P (0x04) in its
    /// SRTPS_PREFIX, and `srtps_psk_flag` reads P; a message of the
    /// AES-GCM-GMAC plugin has P clear.
    #[test]
    fn security_7_4_6_6_psk_encode_sets_pre_shared_key_flag() {
        use zerodds_security_crypto::PskCryptoPlugin;
        use zerodds_security_crypto::psk_rtps::{Passphrase, PskConfig, PskProtection};
        let mut psk = PskCryptoPlugin::new(PskConfig {
            domain_id: 0,
            domain_tag: alloc::string::String::new(),
            passphrase: Passphrase::parse("7:the passphrase of this test").unwrap(),
            alternatives: Vec::new(),
            key_bits: 256,
            protection: PskProtection::Encrypt,
        })
        .unwrap();
        let msg = fake_rtps_message(&BODY);
        let local = CryptoHandle(
            psk.register_local_participant_for(
                [msg[4], msg[5]],
                [msg[6], msg[7]],
                msg[8..RTPS_HEADER_LEN].try_into().unwrap(),
            )
            .unwrap(),
        );
        let wire = encode_secured_rtps_message_psk(&psk, local, &[], &msg).unwrap();
        assert_eq!(wire[..RTPS_HEADER_LEN], msg[..RTPS_HEADER_LEN]);
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
        assert_eq!(wire[RTPS_HEADER_LEN + 1] & 0x07, 0x07, "E, A and P");
        assert_eq!(srtps_psk_flag(&wire), Some(true));
        let (a, local_a, _, _, _) = pair();
        let classic = encode_secured_rtps_message(&a, local_a, &[], &msg).unwrap();
        assert_eq!(srtps_psk_flag(&classic), Some(false));
    }

    /// A HeaderExtension that carries only the messageLength `len`
    /// (DDSI-RTPS 2.5 §9.4.5.2: flags E and L, four octets).
    fn length_extension(len: u32) -> [u8; 8] {
        let l = len.to_le_bytes();
        [0x00, 0x03, 0x04, 0x00, l[0], l[1], l[2], l[3]]
    }

    /// Table 80: the SRTPS_PREFIX follows the RTPS Header or the
    /// HeaderExtension behind it. A message a stream leg gave a
    /// HeaderExtension with its length is found protected, its flags are
    /// read behind the extension, in both byte orders of the extension's
    /// length; a plain message, one with a HeaderExtension and a plain
    /// submessage, and one cut inside the extension or the prefix are not.
    #[test]
    fn security_10_5_3_3_1_the_srtps_prefix_is_found_behind_a_header_extension() {
        let (a, local_a, _, _, _) = pair();
        let msg = fake_rtps_message(&BODY);
        let wire = encode_secured_rtps_message(&a, local_a, &[], &msg).unwrap();
        assert_eq!(srtps_prefix_offset(&wire), Some(RTPS_HEADER_LEN));
        assert!(is_secured_rtps_message(&wire));

        let mut stated = wire[..RTPS_HEADER_LEN].to_vec();
        stated.extend_from_slice(&length_extension(u32::try_from(wire.len() + 8).unwrap()));
        stated.extend_from_slice(&wire[RTPS_HEADER_LEN..]);
        assert_eq!(srtps_prefix_offset(&stated), Some(RTPS_HEADER_LEN + 8));
        assert!(is_secured_rtps_message(&stated));
        assert_eq!(srtps_prefix_flags(&stated), Some(wire[RTPS_HEADER_LEN + 1]));
        assert_eq!(srtps_psk_flag(&stated), Some(false));
        stated[RTPS_HEADER_LEN + 9] |= PRE_SHARED_KEY_FLAG;
        assert_eq!(srtps_psk_flag(&stated), Some(true));

        // A big-endian HeaderExtension with a timestamp behind the length:
        // twelve octets of body.
        let mut big = wire[..RTPS_HEADER_LEN].to_vec();
        big.extend_from_slice(&[0x00, 0x06, 0x00, 0x0C]);
        big.extend_from_slice(&[0; 12]);
        big.extend_from_slice(&wire[RTPS_HEADER_LEN..]);
        assert_eq!(srtps_prefix_offset(&big), Some(RTPS_HEADER_LEN + 16));

        // Plain: no prefix behind the header, none behind the extension.
        assert_eq!(srtps_prefix_offset(&msg), None);
        let mut plain = msg[..RTPS_HEADER_LEN].to_vec();
        plain.extend_from_slice(&length_extension(40));
        plain.extend_from_slice(&BODY);
        assert_eq!(srtps_prefix_offset(&plain), None);
        assert!(!is_secured_rtps_message(&plain));
        assert_eq!(srtps_psk_flag(&plain), None);
        // Cut short: inside the extension, behind it, inside the prefix.
        for cut in [21, 23, 28, 29, 31] {
            assert_eq!(srtps_prefix_offset(&stated[..cut]), None, "{cut}");
        }
        assert_eq!(srtps_prefix_offset(&stated[..32]), Some(28));
        assert_eq!(srtps_prefix_offset(&msg[..RTPS_HEADER_LEN]), None);
        assert_eq!(srtps_prefix_offset(&[]), None);
    }

    /// §7.4.7.8.3: P is bit 2 of the SRTPS_PREFIX flags.
    #[test]
    fn security_7_4_7_8_3_the_pre_shared_key_flag_is_read_from_bit_2() {
        let mut wire = fake_rtps_message(&[]);
        assert_eq!(srtps_psk_flag(&wire), None);
        wire.extend_from_slice(&[SRTPS_PREFIX, 0x01 | PRE_SHARED_KEY_FLAG, 20, 0]);
        assert_eq!(srtps_psk_flag(&wire), Some(true));
        wire[RTPS_HEADER_LEN + 1] = 0x01;
        assert_eq!(srtps_psk_flag(&wire), Some(false));
    }
}
