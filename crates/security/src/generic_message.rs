// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §7.5.5 — `ParticipantGenericMessage` (C3.4).
//!
//! Wire data type for the two builtin topics from §7.5.3 + §7.5.4:
//!
//! | Topic                                   | Reliability | Endpoints (bits, §7.4.7.1)            | Content                                     |
//! |----------------------------------------|-------------|----------------------------------------|---------------------------------------------|
//! | `DCPSParticipantStatelessMessage`      | BestEffort  | 22/23 (`STATELESS_*_{WRITER,READER}`)  | HandshakeRequest/Reply/FinalMessageToken    |
//! | `DCPSParticipantVolatileMessageSecure` | Reliable    | 24/25 (`VOLATILE_*_{WRITER,READER}`)   | CryptoToken exchange messages               |
//!
//! Spec §7.5.5 Tab.10:
//!
//! ```text
//! struct MessageIdentity {
//!   GUID_t            source_guid;          // 16 byte
//!   long long         sequence_number;      // 8 byte (CDR i64)
//! };
//! struct ParticipantGenericMessage {
//!   MessageIdentity   message_identity;
//!   MessageIdentity   related_message_identity;
//!   GUID_t            destination_participant_key;
//!   GUID_t            destination_endpoint_key;
//!   GUID_t            source_endpoint_key;
//!   string<256>       message_class_id;
//!   sequence<DataHolder> message_data;
//! };
//! ```
//!
//! The encoding is XCDR1 (PL_CDR_LE) — the ParticipantGenericMessage
//! is transported as the `serialized_payload` of a DATA submessage.
//!
//! `message_class_id`-Konstanten (Spec §7.5.5):
//!
//! | class_id                                  | Meaning                                         |
//! |-------------------------------------------|-------------------------------------------------|
//! | `"dds.sec.auth_request"`                  | Initiator → Replier: HandshakeRequestMessage    |
//! | `"dds.sec.auth"`                          | Replier → Initiator: HandshakeReplyMessage      |
//! | `"dds.sec.auth"` (related ≠ NIL)          | Initiator → Replier: HandshakeFinalMessage      |
//! | `"dds.sec.participant_crypto_tokens"`     | Crypto-Token-Exchange (Volatile-Topic)          |
//! | `"dds.sec.datawriter_crypto_tokens"`      | DataWriter-Slot Crypto-Tokens                   |
//! | `"dds.sec.datareader_crypto_tokens"`      | DataReader-Slot Crypto-Tokens                   |

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{SecurityError, SecurityErrorKind, SecurityResult};
use crate::token::DataHolder;

/// Topic name for the stateless auth handshake (spec §7.5.3).
pub const TOPIC_STATELESS_MESSAGE: &str = "DCPSParticipantStatelessMessage";

/// Topic name for the crypto-token exchange (spec §7.5.4).
pub const TOPIC_VOLATILE_MESSAGE_SECURE: &str = "DCPSParticipantVolatileMessageSecure";

/// Type name of both topics (spec §7.5.3 + §7.5.4): identical.
pub const TYPE_NAME_GENERIC_MESSAGE: &str = "ParticipantGenericMessage";

/// `message_class_id` constants (spec §7.5.5).
pub mod class_id {
    /// `HandshakeRequestMessage` (initiator → replier).
    pub const AUTH_REQUEST: &str = "dds.sec.auth_request";
    /// `HandshakeReplyMessage` (replier → initiator) **and**
    /// `HandshakeFinalMessage` (initiator → replier with
    /// `related_message_identity != NIL`).
    pub const AUTH: &str = "dds.sec.auth";
    /// Crypto-token exchange at the participant level.
    pub const PARTICIPANT_CRYPTO_TOKENS: &str = "dds.sec.participant_crypto_tokens";
    /// Crypto-token exchange for a DataWriter slot.
    pub const DATAWRITER_CRYPTO_TOKENS: &str = "dds.sec.datawriter_crypto_tokens";
    /// Crypto-token exchange for a DataReader slot.
    pub const DATAREADER_CRYPTO_TOKENS: &str = "dds.sec.datareader_crypto_tokens";
}

/// `MessageIdentity` (spec §7.5.5 Tab.10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageIdentity {
    /// 16-byte GUID of the sender.
    pub source_guid: [u8; 16],
    /// 8-byte sequence number (i64). 0 = NIL/unset.
    pub sequence_number: i64,
}

impl MessageIdentity {
    /// True if both fields have default values (NIL indicator).
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.source_guid == [0; 16] && self.sequence_number == 0
    }
}

/// `ParticipantGenericMessage` (Spec §7.5.5 Tab.10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantGenericMessage {
    /// Unique sender identity per message.
    pub message_identity: MessageIdentity,
    /// `MessageIdentity` of the predecessor message — set for replies + finals,
    /// NIL (all bytes 0) for initial requests.
    pub related_message_identity: MessageIdentity,
    /// Destination participant GUID (16 bytes). 0 = broadcast to all
    /// matched receivers.
    pub destination_participant_key: [u8; 16],
    /// Destination endpoint GUID (or 0 for participant-wide).
    pub destination_endpoint_key: [u8; 16],
    /// Source-Endpoint-GUID.
    pub source_endpoint_key: [u8; 16],
    /// `message_class_id` string (see [`class_id`]).
    pub message_class_id: String,
    /// Sequence of `DataHolder` — typically ONE (e.g. a
    /// HandshakeMessageToken or a CryptoToken bundle).
    pub message_data: Vec<DataHolder>,
}

/// Maximum wire body of a `ParticipantGenericMessage` (DoS cap).
const MAX_GENERIC_MESSAGE_BYTES: usize = 256 * 1024;

/// Maximum `message_data` sequence length (DoS cap).
const MAX_MESSAGE_DATA_LEN: u32 = 64;

/// Maximum `message_class_id` length (spec: string<256>).
const MAX_CLASS_ID_LEN: u32 = 256;

impl ParticipantGenericMessage {
    /// Encode → XCDR1-LE bytes (without the PL_CDR encapsulation header — the
    /// wire layer appends that separately, because ParticipantGenericMessage
    /// is not PL_CDR (ParameterList) but structured CDR).
    #[must_use]
    pub fn to_cdr_le(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(128);
        encode_message_identity(&mut out, &self.message_identity, true);
        encode_message_identity(&mut out, &self.related_message_identity, true);
        out.extend_from_slice(&self.destination_participant_key);
        out.extend_from_slice(&self.destination_endpoint_key);
        out.extend_from_slice(&self.source_endpoint_key);
        encode_string(&mut out, &self.message_class_id, true);
        encode_u32(&mut out, self.message_data.len() as u32, true);
        for dh in &self.message_data {
            // Spec `sequence<DataHolder>` (GenericMessageData): each
            // DataHolder INLINE as a CDR struct (4-aligned), NOT
            // length-prefixed. Cross-vendor critical: cyclone/FastDDS
            // deserialize sequence<DataHolder> inline — an
            // octet-seq length prefix would be misinterpreted as the
            // class_id string length ("deserialization failed").
            align(&mut out, 4);
            out.extend_from_slice(&dh.to_cdr_le());
        }
        out
    }

    /// Decode from XCDR1-LE bytes.
    ///
    /// # Errors
    /// `BadArgument` on truncation, exceeded DoS caps, or
    /// non-UTF-8 in `message_class_id`.
    pub fn from_cdr_le(bytes: &[u8]) -> SecurityResult<Self> {
        if bytes.len() > MAX_GENERIC_MESSAGE_BYTES {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "generic_message: payload exceeds DoS cap",
            ));
        }
        let mut cur = Cursor::new(bytes, true);
        let message_identity = decode_message_identity(&mut cur)?;
        let related_message_identity = decode_message_identity(&mut cur)?;
        let destination_participant_key = cur.read_array16()?;
        let destination_endpoint_key = cur.read_array16()?;
        let source_endpoint_key = cur.read_array16()?;
        let message_class_id = decode_string(&mut cur)?;
        if message_class_id.len() > MAX_CLASS_ID_LEN as usize {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "generic_message: message_class_id exceeds 256 bytes",
            ));
        }
        let count = cur.read_u32()?;
        if count > MAX_MESSAGE_DATA_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "generic_message: message_data sequence too long",
            ));
        }
        let mut message_data = Vec::with_capacity(count as usize);
        for _ in 0..count {
            // Inline DataHolder (4-aligned), the length is determined by the
            // decoder itself — no length prefix (see to_cdr_le).
            cur.align(4)?;
            let (dh, consumed) = DataHolder::from_cdr_le_consumed(&cur.buf[cur.pos..])?;
            cur.advance(consumed)?;
            message_data.push(dh);
        }
        Ok(Self {
            message_identity,
            related_message_identity,
            destination_participant_key,
            destination_endpoint_key,
            source_endpoint_key,
            message_class_id,
            message_data,
        })
    }
}

// ----------------------------------------------------------------------
// XCDR1-LE primitives
// ----------------------------------------------------------------------

fn align(buf: &mut Vec<u8>, n: usize) {
    let pad = (n - buf.len() % n) % n;
    for _ in 0..pad {
        buf.push(0);
    }
}

fn encode_u32(buf: &mut Vec<u8>, v: u32, le: bool) {
    align(buf, 4);
    if le {
        buf.extend_from_slice(&v.to_le_bytes());
    } else {
        buf.extend_from_slice(&v.to_be_bytes());
    }
}

fn encode_i64(buf: &mut Vec<u8>, v: i64, le: bool) {
    align(buf, 8);
    if le {
        buf.extend_from_slice(&v.to_le_bytes());
    } else {
        buf.extend_from_slice(&v.to_be_bytes());
    }
}

fn encode_string(buf: &mut Vec<u8>, s: &str, le: bool) {
    let bytes = s.as_bytes();
    let len = (bytes.len() + 1) as u32;
    encode_u32(buf, len, le);
    buf.extend_from_slice(bytes);
    buf.push(0);
}

fn encode_message_identity(buf: &mut Vec<u8>, mi: &MessageIdentity, le: bool) {
    buf.extend_from_slice(&mi.source_guid);
    encode_i64(buf, mi.sequence_number, le);
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    le: bool,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8], le: bool) -> Self {
        Self { buf, pos: 0, le }
    }

    fn align(&mut self, n: usize) -> SecurityResult<()> {
        let pad = (n - self.pos % n) % n;
        self.advance(pad)
    }

    fn advance(&mut self, n: usize) -> SecurityResult<()> {
        if self.pos.saturating_add(n) > self.buf.len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "generic_message: truncated",
            ));
        }
        self.pos += n;
        Ok(())
    }

    fn read_u32(&mut self) -> SecurityResult<u32> {
        self.align(4)?;
        let start = self.pos;
        self.advance(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(&self.buf[start..start + 4]);
        Ok(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }

    fn read_i64(&mut self) -> SecurityResult<i64> {
        self.align(8)?;
        let start = self.pos;
        self.advance(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&self.buf[start..start + 8]);
        Ok(if self.le {
            i64::from_le_bytes(a)
        } else {
            i64::from_be_bytes(a)
        })
    }

    fn read_array16(&mut self) -> SecurityResult<[u8; 16]> {
        let start = self.pos;
        self.advance(16)?;
        let mut a = [0u8; 16];
        a.copy_from_slice(&self.buf[start..start + 16]);
        Ok(a)
    }

    fn read_slice(&mut self, n: usize) -> SecurityResult<&'a [u8]> {
        let start = self.pos;
        self.advance(n)?;
        Ok(&self.buf[start..start + n])
    }
}

fn decode_message_identity(cur: &mut Cursor<'_>) -> SecurityResult<MessageIdentity> {
    let source_guid = cur.read_array16()?;
    let sequence_number = cur.read_i64()?;
    Ok(MessageIdentity {
        source_guid,
        sequence_number,
    })
}

fn decode_string(cur: &mut Cursor<'_>) -> SecurityResult<String> {
    let len = cur.read_u32()? as usize;
    if len == 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: zero-length string (no NUL)",
        ));
    }
    if len > MAX_CLASS_ID_LEN as usize + 1 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: string > cap",
        ));
    }
    let raw = cur.read_slice(len)?;
    if raw[len - 1] != 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: missing terminating NUL",
        ));
    }
    let s = core::str::from_utf8(&raw[..len - 1]).map_err(|_| {
        SecurityError::new(SecurityErrorKind::BadArgument, "generic_message: non-utf8")
    })?;
    Ok(s.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_msg() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 42,
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
    fn roundtrip_le() {
        let msg = sample_msg();
        let bytes = msg.to_cdr_le();
        let back = ParticipantGenericMessage::from_cdr_le(&bytes).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn message_data_dataholder_is_inline_not_length_prefixed() {
        // Spec `sequence<DataHolder>`: cyclone/FastDDS serialize the
        // DataHolders INLINE (no octet-seq length prefix). Earlier ZeroDDS
        // wrapped each DataHolder in a length-prefixed sequence<octet> —
        // cyclone read the prefix as the class_id string length → "deserialization
        // failed". Here: the (single) DataHolder must stand as a contiguous
        // inline block at the end, and the 4 bytes before it are the
        // sequence COUNT (=1), NOT its length.
        let msg = sample_msg();
        let bytes = msg.to_cdr_le();
        let dh_inline = msg.message_data[0].to_cdr_le();
        assert!(
            bytes.ends_with(&dh_inline),
            "the DataHolder must stand INLINE at the end"
        );
        let pos = bytes.len() - dh_inline.len();
        let prefix = u32::from_le_bytes([
            bytes[pos - 4],
            bytes[pos - 3],
            bytes[pos - 2],
            bytes[pos - 1],
        ]);
        assert_eq!(
            prefix, 1,
            "before the DataHolder stands the sequence count (=1)"
        );
        assert_ne!(
            prefix as usize,
            dh_inline.len(),
            "NO octet-seq length prefix before the DataHolder"
        );
    }

    #[test]
    fn nil_message_identity() {
        let mi = MessageIdentity::default();
        assert!(mi.is_nil());
        let mi2 = MessageIdentity {
            source_guid: [0xAA; 16],
            sequence_number: 0,
        };
        assert!(!mi2.is_nil());
    }

    #[test]
    fn class_id_constants_match_spec() {
        // Spec §7.5.5 — these strings must NEVER have drifted, otherwise
        // Cyclone/FastDDS won't match our auth messages.
        assert_eq!(class_id::AUTH_REQUEST, "dds.sec.auth_request");
        assert_eq!(class_id::AUTH, "dds.sec.auth");
        assert_eq!(
            class_id::PARTICIPANT_CRYPTO_TOKENS,
            "dds.sec.participant_crypto_tokens"
        );
        assert_eq!(
            class_id::DATAWRITER_CRYPTO_TOKENS,
            "dds.sec.datawriter_crypto_tokens"
        );
        assert_eq!(
            class_id::DATAREADER_CRYPTO_TOKENS,
            "dds.sec.datareader_crypto_tokens"
        );
    }

    #[test]
    fn topic_name_constants_match_spec() {
        assert_eq!(TOPIC_STATELESS_MESSAGE, "DCPSParticipantStatelessMessage");
        assert_eq!(
            TOPIC_VOLATILE_MESSAGE_SECURE,
            "DCPSParticipantVolatileMessageSecure"
        );
        assert_eq!(TYPE_NAME_GENERIC_MESSAGE, "ParticipantGenericMessage");
    }

    #[test]
    fn empty_message_data_roundtrip() {
        let msg = ParticipantGenericMessage {
            message_class_id: class_id::AUTH.to_string(),
            ..ParticipantGenericMessage::default()
        };
        let bytes = msg.to_cdr_le();
        let back = ParticipantGenericMessage::from_cdr_le(&bytes).unwrap();
        assert_eq!(msg, back);
        assert!(back.message_data.is_empty());
    }

    #[test]
    fn handshake_request_token_in_message_data() {
        // Realistic scenario: the initiator sends its
        // HandshakeRequestMessageToken via DCPSParticipantStateless.
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
            message_data: vec![token],
            ..ParticipantGenericMessage::default()
        };
        let bytes = msg.to_cdr_le();
        let back = ParticipantGenericMessage::from_cdr_le(&bytes).unwrap();
        assert_eq!(back.message_data.len(), 1);
        assert_eq!(back.message_data[0].class_id, "DDS:Auth:PKI-DH:1.2+AuthReq");
        assert_eq!(
            back.message_data[0].property("c.dsign_algo"),
            Some("ECDSA-SHA256")
        );
        assert_eq!(
            back.message_data[0].binary_property("c.id"),
            Some(&[0x30, 0x82, 0x01, 0x23][..])
        );
    }

    #[test]
    fn related_message_identity_links_reply_to_request() {
        // The replier sets related_message_identity = the sender_identity of the
        // request, so the initiator can map the reply to its request.
        let request_id = MessageIdentity {
            source_guid: [0xAA; 16],
            sequence_number: 1,
        };
        let reply = ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xDD; 16],
                sequence_number: 1,
            },
            related_message_identity: request_id.clone(),
            destination_participant_key: [0xAA; 16],
            source_endpoint_key: [0xDD; 16],
            message_class_id: class_id::AUTH.to_string(),
            ..ParticipantGenericMessage::default()
        };
        let bytes = reply.to_cdr_le();
        let back = ParticipantGenericMessage::from_cdr_le(&bytes).unwrap();
        assert_eq!(back.related_message_identity, request_id);
    }

    #[test]
    fn truncated_buffer_rejected() {
        let msg = sample_msg();
        let bytes = msg.to_cdr_le();
        let truncated = &bytes[..bytes.len() / 2];
        assert!(ParticipantGenericMessage::from_cdr_le(truncated).is_err());
    }

    #[test]
    fn invalid_class_id_utf8_rejected() {
        // Encode with forged non-UTF-8 class-id bytes.
        let mut buf = Vec::new();
        // message_identity (24 byte: 16 + i64 padded)
        buf.extend_from_slice(&[0u8; 16]);
        buf.extend_from_slice(&0i64.to_le_bytes());
        // related_message_identity
        buf.extend_from_slice(&[0u8; 16]);
        buf.extend_from_slice(&0i64.to_le_bytes());
        // 3x GUID
        buf.extend_from_slice(&[0u8; 48]);
        // class_id len = 5, then 4 bytes invalid utf-8 + NUL
        buf.extend_from_slice(&5u32.to_le_bytes());
        buf.extend_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC, 0x00]);
        // align + message_data count = 0
        align(&mut buf, 4);
        buf.extend_from_slice(&0u32.to_le_bytes());
        let err = ParticipantGenericMessage::from_cdr_le(&buf).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn dos_cap_total_payload_rejected() {
        let big = vec![0u8; MAX_GENERIC_MESSAGE_BYTES + 1];
        let err = ParticipantGenericMessage::from_cdr_le(&big).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn message_data_cap_rejected() {
        // Forge: count = 1_000_000.
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0u8; 24]); // mi
        buf.extend_from_slice(&[0u8; 24]); // related
        buf.extend_from_slice(&[0u8; 48]); // 3 GUIDs
        // class_id len = 1 + NUL
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.push(0);
        align(&mut buf, 4);
        buf.extend_from_slice(&1_000_000u32.to_le_bytes());
        let err = ParticipantGenericMessage::from_cdr_le(&buf).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
