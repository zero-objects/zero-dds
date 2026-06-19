// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C3.4-b — API bridge for the DDS-Security 1.2 §7.5.3/§7.5.4 builtin
//! topics (`DCPSParticipantStatelessMessage` + `DCPSParticipantVolatileMessage-
//! Secure`). Wraps the spec data model from `zerodds_security::generic_message`
//! into a DCPS-ready form:
//!
//! - 4-byte PL_CDR encapsulation header (spec RTPS 2.5 §10) before the
//!   bytes — the same envelope as `ParticipantBuiltinTopicData` DATA
//!   submessages.
//! - QoS defaults per topic (spec §7.5.3 BestEffort, §7.5.4 Reliable +
//!   VOLATILE + KEEP_ALL).
//!
//! **What does not happen here (C3.4-c):** the actual DataWriter/
//! DataReader creation in the DCPS runtime. The caller uses these
//! helpers to push the wire bytes via a standard RawBytes DataWriter
//! with the matching EntityIds (see `zerodds_rtps::wire_types::EntityId::
//! BUILTIN_PARTICIPANT_STATELESS_MESSAGE_*` from C3.8).

use alloc::vec::Vec;

use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// Layer-neutral QoS trio for the two builtin topics. The caller in the
/// DCPS layer maps these onto its `DataWriterQos`/`DataReaderQos`
/// aggregate types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTopicQos {
    /// `RELIABILITY`-Policy.
    pub reliability: ReliabilityQosPolicy,
    /// `DURABILITY`-Policy.
    pub durability: DurabilityQosPolicy,
    /// `HISTORY`-Policy.
    pub history: HistoryQosPolicy,
}
use zerodds_security::generic_message::ParticipantGenericMessage;

/// CDR-LE encapsulation kind (spec RTPS 2.5 §10.2). The same 4-byte envelope
/// as ParticipantBuiltinTopicData (CDR_LE instead of PL_CDR_LE — the
/// ParticipantGenericMessage is a **structured CDR**, not a
/// ParameterList).
pub const ENCAPSULATION_CDR_LE: [u8; 2] = [0x00, 0x01];

/// Encapsulation header length (spec §10.1: 2 byte kind + 2 byte options).
pub const ENCAPSULATION_HEADER_LEN: usize = 4;

/// Encodes a `ParticipantGenericMessage` as
/// `serialized_payload` bytes for a DATA submessage (with a 4-byte
/// CDR-LE encapsulation header + XCDR1 body).
#[must_use]
pub fn encode_generic_message(msg: &ParticipantGenericMessage) -> Vec<u8> {
    let body = msg.to_cdr_le();
    let mut out = Vec::with_capacity(ENCAPSULATION_HEADER_LEN + body.len());
    out.extend_from_slice(&ENCAPSULATION_CDR_LE);
    out.extend_from_slice(&[0, 0]); // options (Spec: must be 0)
    out.extend_from_slice(&body);
    out
}

/// Decodes a `ParticipantGenericMessage` from
/// `serialized_payload` bytes (with a 4-byte encapsulation header).
///
/// # Errors
/// `BadArgument` if the encapsulation header is missing or carries a
/// kind other than CDR_LE / CDR_BE; CDR decode errors are
/// passed through.
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
    // Skip 2 byte options.
    ParticipantGenericMessage::from_cdr_le(&bytes[ENCAPSULATION_HEADER_LEN..])
}

/// Spec §7.5.3 — BestEffort reliability for the DCPSParticipantStateless-
/// Message topic. Stateless = no sequence tracking, every DATA
/// submessage is standalone.
#[must_use]
pub fn stateless_message_qos() -> BuiltinTopicQos {
    BuiltinTopicQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..ReliabilityQosPolicy::default()
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 0,
        },
    }
}

/// Spec §7.5.4 Tab.19/20 — Reliable + VOLATILE + KEEP_ALL for the
/// DCPSParticipantVolatileMessageSecure topic.
#[must_use]
pub fn volatile_secure_qos() -> BuiltinTopicQos {
    BuiltinTopicQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..ReliabilityQosPolicy::default()
        },
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Volatile,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 0,
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_security::generic_message::{MessageIdentity, class_id};
    use zerodds_security::token::DataHolder;

    fn sample_msg() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.to_string(),
            message_data: vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        }
    }

    #[test]
    fn encode_starts_with_cdr_le_encapsulation() {
        let msg = sample_msg();
        let bytes = encode_generic_message(&msg);
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let msg = sample_msg();
        let bytes = encode_generic_message(&msg);
        let back = decode_generic_message(&bytes).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn decode_rejects_unknown_encapsulation() {
        let bytes = vec![0x00, 0x99, 0, 0, 0, 0, 0, 0];
        let err = decode_generic_message(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn decode_rejects_truncated() {
        let bytes = vec![0x00, 0x01];
        let err = decode_generic_message(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn stateless_qos_is_best_effort_volatile_keep_all() {
        let q = stateless_message_qos();
        assert_eq!(q.reliability.kind, ReliabilityKind::BestEffort);
        assert_eq!(q.durability.kind, DurabilityKind::Volatile);
        assert_eq!(q.history.kind, HistoryKind::KeepAll);
    }

    #[test]
    fn volatile_secure_qos_is_reliable_volatile_keep_all() {
        let q = volatile_secure_qos();
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.durability.kind, DurabilityKind::Volatile);
        assert_eq!(q.history.kind, HistoryKind::KeepAll);
    }

    #[test]
    fn stateless_and_volatile_qos_differ() {
        // Spec §7.5.3 vs §7.5.4 — the two topics MUST have different
        // reliability, otherwise someone misread it.
        assert_ne!(
            stateless_message_qos().reliability.kind,
            volatile_secure_qos().reliability.kind
        );
    }

    #[test]
    fn full_handshake_token_through_bridge() {
        // E2E: the auth plugin builds a HandshakeRequest → ParticipantGeneric-
        // Message → encapsulated bytes; the receiver decodes back
        // to the DataHolder.
        let token = DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")
            .with_property("c.dsign_algo", "ECDSA-SHA256")
            .with_binary_property("c.id", vec![0x30, 0x82, 0x01, 0x23]);
        let msg = ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 1,
            },
            destination_participant_key: [0xBB; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.to_string(),
            message_data: vec![token.clone()],
            ..ParticipantGenericMessage::default()
        };
        let wire = encode_generic_message(&msg);
        let decoded = decode_generic_message(&wire).unwrap();
        assert_eq!(decoded.message_data.len(), 1);
        assert_eq!(decoded.message_data[0], token);
    }
}
