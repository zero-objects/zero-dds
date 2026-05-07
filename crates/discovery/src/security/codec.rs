// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire-Codec fuer `ParticipantGenericMessage` (Spec §7.5.5).
//!
//! Identische Semantik wie `zerodds_security_runtime::builtin_topics`: 4-Byte
//! CDR-LE-Encapsulation-Header gefolgt vom XCDR1-Body. Wir reimplementieren
//! es hier inline, damit `zerodds-discovery` nicht von `zerodds-security-runtime`
//! abhaengt (Cargo-Zyklus mit den Dev-Dep-Tests in security-runtime).

extern crate alloc;
use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::generic_message::ParticipantGenericMessage;

/// CDR-LE-Encapsulation-Kind (Spec RTPS 2.5 §10.2).
pub const ENCAPSULATION_CDR_LE: [u8; 2] = [0x00, 0x01];

/// Encapsulation-Header-Laenge (Spec §10.1: 2 byte kind + 2 byte options).
pub const ENCAPSULATION_HEADER_LEN: usize = 4;

/// Encoded eine `ParticipantGenericMessage` als `serialized_payload`-
/// Bytes fuer eine DATA-Submessage.
#[must_use]
pub fn encode_generic_message(msg: &ParticipantGenericMessage) -> Vec<u8> {
    let body = msg.to_cdr_le();
    let mut out = Vec::with_capacity(ENCAPSULATION_HEADER_LEN + body.len());
    out.extend_from_slice(&ENCAPSULATION_CDR_LE);
    out.extend_from_slice(&[0, 0]); // options (Spec: must be 0)
    out.extend_from_slice(&body);
    out
}

/// Decoded eine `ParticipantGenericMessage` aus `serialized_payload`-
/// Bytes (mit 4-byte Encapsulation-Header).
///
/// # Errors
/// `BadArgument` wenn der Encapsulation-Header fehlt oder ein anderes
/// Kind als CDR_LE / CDR_BE traegt; CDR-Decode-Fehler werden
/// durchgereicht.
pub fn decode_generic_message(bytes: &[u8]) -> SecurityResult<ParticipantGenericMessage> {
    if bytes.len() < ENCAPSULATION_HEADER_LEN {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: encapsulation-header truncated",
        ));
    }
    let kind = [bytes[0], bytes[1]];
    if kind != ENCAPSULATION_CDR_LE && kind != [0x00, 0x00] {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: only CDR_LE encapsulation supported",
        ));
    }
    ParticipantGenericMessage::from_cdr_le(&bytes[ENCAPSULATION_HEADER_LEN..])
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_security::generic_message::{MessageIdentity, class_id};
    use zerodds_security::token::DataHolder;

    fn sample() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 7,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        }
    }

    #[test]
    fn roundtrip_preserves_message() {
        let msg = sample();
        let bytes = encode_generic_message(&msg);
        let back = decode_generic_message(&bytes).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn encode_starts_with_cdr_le_encapsulation() {
        let bytes = encode_generic_message(&sample());
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn decode_rejects_truncated_header() {
        let err = decode_generic_message(&[0x00, 0x01]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn decode_rejects_unknown_encapsulation() {
        let err = decode_generic_message(&[0x00, 0x99, 0, 0, 0, 0, 0, 0]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
