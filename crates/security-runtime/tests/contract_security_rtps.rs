// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with `zerodds-security-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-rtps`, from its non-test
//! sources:
//! - `gate.rs` (`SecurityGate`): `encode_secured_submessage(crypto, local,
//!   &[], plaintext)` and `decode_secured_submessage(crypto, local, local,
//!   wire)` per topic, `encode_secured_rtps_message(crypto, local, &[],
//!   message)` and `decode_secured_rtps_message(crypto, slot, slot, wire)`
//!   per domain; `SecurityRtpsError`, kept in `SecurityGateError::Wrapper`;
//!   `SEC_PREFIX` (a submessage is secured when its first octet is
//!   `SEC_PREFIX`), `SRTPS_PREFIX` and `RTPS_HEADER_LEN` (a message is
//!   secured when the octet behind the 20-octet header is `SRTPS_PREFIX`).
//! - `shared.rs` (`SharedSecurityGate`): `RTPS_HEADER_LEN` and
//!   `SRTPS_PREFIX` for the same test on inbound and outbound messages.
//!
//! What it relies on: the wrapped forms start where the gate looks
//! (`SEC_PREFIX` first; the RTPS Header kept in the clear and
//! `SRTPS_PREFIX` behind it), every part is a well-formed RTPS submessage,
//! the unwrap gives back what was wrapped, and wire that is not a wrapped
//! form, or was changed, is an error.
//!
//! Oracles: DDS-Security 1.2 §7.4.7.5.2 (SEC_BODY = 0x30), §7.4.7.6.2
//! (SEC_PREFIX = 0x31), §7.4.7.7.2 (SEC_POSTFIX = 0x32), §7.4.7.8.2
//! (SRTPS_PREFIX = 0x33), §7.4.7.9.2 (SRTPS_POSTFIX = 0x34), §7.4.7.8.1 (the
//! EndiannessFlag is bit 0 of the flags), §10.5.3.3.4.6.4 (the RTPS Header
//! goes into the protected body as an INFO_SOURCE); DDSI-RTPS 2.5 §8.3.3
//! and §9.4.4 (the RTPS Header: "RTPS", ProtocolVersion, VendorId,
//! GuidPrefix, 20 octets) and §9.4.5.1 (SubmessageHeader: id, flags,
//! octetsToNextHeader in the byte order of the E flag). The cipher is
//! `AesGcmCryptoPlugin` of `zerodds-security-crypto`, as in the gate.
#![allow(clippy::unwrap_used)]

use zerodds_security::authentication::IdentityHandle;
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_rtps::{
    RTPS_HEADER_LEN, SEC_PREFIX, SRTPS_PREFIX, SecurityRtpsError, decode_secured_rtps_message,
    decode_secured_submessage, encode_secured_rtps_message, encode_secured_submessage,
};

/// DDSI-RTPS 2.5 §8.3.3: "RTPS", version 2.5, a vendor id and a GUID
/// prefix.
const HEADER: [u8; 20] = [
    b'R', b'T', b'P', b'S', 0x02, 0x05, 0x01, 0x0f, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0a, 0x0b, 0x0c,
];

/// A HEARTBEAT submessage (id 0x07, E set, 28 octets) as a plain body.
const HEARTBEAT: [u8; 32] = [
    0x07, 0x01, 0x1c, 0x00, // header
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, // reader, writer id
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // firstSN 1
    0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, // lastSN 5
    0x01, 0x00, 0x00, 0x00, // count 1
];

/// A plugin with its local participant, as `SecurityGate::ensure_local`
/// makes it.
fn plugin() -> (AesGcmCryptoPlugin, CryptoHandle) {
    let mut crypto = AesGcmCryptoPlugin::new();
    let local = crypto
        .register_local_participant(IdentityHandle(1), &[])
        .unwrap();
    (crypto, local)
}

/// The submessage ids of `wire`, read with DDSI-RTPS 2.5 §9.4.5.1; every
/// header has to have the E flag and its length has to fit.
fn submessage_ids(wire: &[u8]) -> Vec<u8> {
    let mut ids = Vec::new();
    let mut at = 0;
    while at < wire.len() {
        assert!(at + 4 <= wire.len(), "a submessage header is cut at {at}");
        assert_eq!(wire[at + 1] & 0x01, 0x01, "E flag at {at}");
        let len = usize::from(u16::from_le_bytes([wire[at + 2], wire[at + 3]]));
        ids.push(wire[at]);
        at += 4 + len;
    }
    assert_eq!(at, wire.len(), "the last submessage ends with the wire");
    ids
}

#[test]
fn security_7_4_7_the_ids_the_gate_detects_secured_wire_by() {
    assert_eq!(SEC_PREFIX, 0x31);
    assert_eq!(SRTPS_PREFIX, 0x33);
    assert_eq!(RTPS_HEADER_LEN, 20);
    assert_eq!(RTPS_HEADER_LEN, HEADER.len());
}

#[test]
fn security_7_4_7_6_a_wrapped_submessage_is_prefix_body_and_postfix() {
    let (crypto, local) = plugin();
    let wire = encode_secured_submessage(&crypto, local, &[], &HEARTBEAT).unwrap();
    // `SecurityGate::decode_inbound` looks at the first octet.
    assert_eq!(wire[0], SEC_PREFIX);
    assert_eq!(submessage_ids(&wire), [0x31, 0x30, 0x32]);
    assert!(!wire.windows(HEARTBEAT.len()).any(|w| w == HEARTBEAT));
    let back = decode_secured_submessage(&crypto, local, local, &wire).unwrap();
    assert_eq!(back, HEARTBEAT);
}

#[test]
fn security_7_4_7_8_a_wrapped_message_keeps_its_header_and_starts_its_body_with_srtps_prefix() {
    let (crypto, local) = plugin();
    let mut message = HEADER.to_vec();
    message.extend_from_slice(&HEARTBEAT);
    let wire = encode_secured_rtps_message(&crypto, local, &[], &message).unwrap();
    // `SharedSecurityGate` and `SecurityGate` look behind the header.
    assert_eq!(wire[..RTPS_HEADER_LEN], HEADER);
    assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    assert_eq!(submessage_ids(&wire[RTPS_HEADER_LEN..]), [0x33, 0x30, 0x34]);
    let back = decode_secured_rtps_message(&crypto, local, local, &wire).unwrap();
    assert_eq!(back, message);
}

#[test]
fn wire_that_is_no_wrapped_form_or_was_changed_is_an_error() {
    let (crypto, local) = plugin();
    let sub = encode_secured_submessage(&crypto, local, &[], &HEARTBEAT).unwrap();

    // A plain submessage where a SEC_PREFIX is expected.
    assert_eq!(
        decode_secured_submessage(&crypto, local, local, &HEARTBEAT).unwrap_err(),
        SecurityRtpsError::UnexpectedSubmessageId {
            pos: 0,
            expected: SEC_PREFIX,
            got: 0x07,
        }
    );
    // A changed octet of the ciphertext: the cipher refuses it.
    let mut changed = sub.clone();
    let last_of_body = changed.len() - 5;
    changed[last_of_body] ^= 0x01;
    assert!(matches!(
        decode_secured_submessage(&crypto, local, local, &changed),
        Err(SecurityRtpsError::Crypto(_))
    ));
    // Big-endian headers are not read.
    let mut big_endian = sub.clone();
    big_endian[1] &= !0x01;
    assert_eq!(
        decode_secured_submessage(&crypto, local, local, &big_endian).unwrap_err(),
        SecurityRtpsError::BigEndianNotSupported
    );
    // Cut short.
    assert!(matches!(
        decode_secured_submessage(&crypto, local, local, &sub[..10]),
        Err(SecurityRtpsError::Truncated(_))
    ));

    let mut message = HEADER.to_vec();
    message.extend_from_slice(&HEARTBEAT);
    let wire = encode_secured_rtps_message(&crypto, local, &[], &message).unwrap();
    // §10.5.3.3.4.6.4: the header is not in the AAD; the protected body
    // carries it as an INFO_SOURCE. A header that names another GUID prefix
    // does not make the message that sender's: the INFO_SOURCE with the
    // original prefix stays in front of the body.
    let mut other_sender = wire.clone();
    other_sender[8] ^= 0x01;
    let read = decode_secured_rtps_message(&crypto, local, local, &other_sender).unwrap();
    let mut forged = message.clone();
    forged[8] ^= 0x01;
    assert_ne!(read, forged);
    assert!(read.windows(12).any(|w| w == &HEADER[8..20]));
    // Less than a header.
    assert!(matches!(
        encode_secured_rtps_message(&crypto, local, &[], &HEADER[..19]),
        Err(SecurityRtpsError::Truncated(_))
    ));
}
