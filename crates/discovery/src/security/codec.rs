// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire codec for `ParticipantGenericMessage` (Spec §7.5.5): a 4-byte
//! CDR-LE encapsulation header followed by the XCDR1 body.
//!
//! The codec is `zerodds_security::generic_message`'s; this module names
//! it where discovery's callers look for it. It used to be a second copy,
//! kept so that `zerodds-discovery` would not depend on
//! `zerodds-security-runtime`; both depend on `zerodds-security`.

/// The encapsulation and the two functions live with the message, in
/// `zerodds-security`; `zerodds-security-runtime` re-exports the same.
pub use zerodds_security::generic_message::{
    ENCAPSULATION_CDR_BE, ENCAPSULATION_CDR_LE, ENCAPSULATION_HEADER_LEN, decode_generic_message,
    encode_generic_message,
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_security::error::SecurityErrorKind;
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
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

    #[test]
    fn decode_reads_a_big_endian_encapsulation() {
        // CDR_BE (0x0000) used to be rejected: the body decoder read
        // little-endian only. A message encoded big-endian behind the
        // CDR_BE kind now reads back whole, its inline DataHolders
        // included. Not measured against a foreign big-endian sender;
        // the layout is the one the little-endian form is measured in,
        // with every multi-byte field's octets reversed.
        let msg = ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0x11; 16],
                sequence_number: 0x0102_0304_0506_0708,
            },
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![
                zerodds_security::token::DataHolder::new("dds.sec.auth").with_property("a", "b")
            ],
            ..Default::default()
        };
        let mut bytes = alloc::vec![0x00, 0x00, 0x00, 0x00];
        bytes.extend_from_slice(&msg.to_cdr(false));
        // The sequence number follows the 16-octet GUID, most significant
        // octet first.
        assert_eq!(&bytes[20..28], &[1, 2, 3, 4, 5, 6, 7, 8]);
        let back = decode_generic_message(&bytes).unwrap();
        assert_eq!(back, msg);
        // And the same bytes under the little-endian kind do not read as
        // the same message.
        bytes[1] = 0x01;
        assert_ne!(decode_generic_message(&bytes).ok(), Some(msg));
    }
}
