// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §7.5.5 — `ParticipantGenericMessage` (C3.4).
//!
//! Wire-Datentyp fuer die zwei Builtin-Topics aus §7.5.3 + §7.5.4:
//!
//! | Topic                                   | Reliability | Endpoints (Bits, §7.4.7.1)            | Inhalt                                      |
//! |----------------------------------------|-------------|----------------------------------------|---------------------------------------------|
//! | `DCPSParticipantStatelessMessage`      | BestEffort  | 22/23 (`STATELESS_*_{WRITER,READER}`)  | HandshakeRequest/Reply/FinalMessageToken    |
//! | `DCPSParticipantVolatileMessageSecure` | Reliable    | 24/25 (`VOLATILE_*_{WRITER,READER}`)   | CryptoToken-Exchange-Nachrichten            |
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
//! Encoding ist XCDR1 (PL_CDR_LE) — die ParticipantGenericMessage
//! wird als `serialized_payload` einer DATA-Submessage transportiert.
//!
//! `message_class_id`-Konstanten (Spec §7.5.5):
//!
//! | class_id                                  | Bedeutung                                       |
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

/// Topic-Name fuer den Stateless-Auth-Handshake (Spec §7.5.3).
pub const TOPIC_STATELESS_MESSAGE: &str = "DCPSParticipantStatelessMessage";

/// Topic-Name fuer Crypto-Token-Exchange (Spec §7.5.4).
pub const TOPIC_VOLATILE_MESSAGE_SECURE: &str = "DCPSParticipantVolatileMessageSecure";

/// Type-Name beider Topics (Spec §7.5.3 + §7.5.4): identisch.
pub const TYPE_NAME_GENERIC_MESSAGE: &str = "ParticipantGenericMessage";

/// `message_class_id`-Konstanten (Spec §7.5.5).
pub mod class_id {
    /// `HandshakeRequestMessage` (Initiator → Replier).
    pub const AUTH_REQUEST: &str = "dds.sec.auth_request";
    /// `HandshakeReplyMessage` (Replier → Initiator) **und**
    /// `HandshakeFinalMessage` (Initiator → Replier mit
    /// `related_message_identity != NIL`).
    pub const AUTH: &str = "dds.sec.auth";
    /// Crypto-Token-Exchange auf Participant-Ebene.
    pub const PARTICIPANT_CRYPTO_TOKENS: &str = "dds.sec.participant_crypto_tokens";
    /// Crypto-Token-Exchange fuer einen DataWriter-Slot.
    pub const DATAWRITER_CRYPTO_TOKENS: &str = "dds.sec.datawriter_crypto_tokens";
    /// Crypto-Token-Exchange fuer einen DataReader-Slot.
    pub const DATAREADER_CRYPTO_TOKENS: &str = "dds.sec.datareader_crypto_tokens";
}

/// `MessageIdentity` (Spec §7.5.5 Tab.10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageIdentity {
    /// 16-byte GUID des Senders.
    pub source_guid: [u8; 16],
    /// 8-byte Sequence-Number (i64). 0 = NIL/unset.
    pub sequence_number: i64,
}

impl MessageIdentity {
    /// True wenn beide Felder Default-Werte haben (NIL-Indicator).
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.source_guid == [0; 16] && self.sequence_number == 0
    }
}

/// `ParticipantGenericMessage` (Spec §7.5.5 Tab.10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantGenericMessage {
    /// Eindeutige Sender-Identitaet pro Message.
    pub message_identity: MessageIdentity,
    /// `MessageIdentity` der Vorgaenger-Message — bei Replies + Finals
    /// gesetzt, bei initialen Requests NIL (alle Bytes 0).
    pub related_message_identity: MessageIdentity,
    /// Destination-Participant-GUID (16 byte). 0 = broadcast an alle
    /// matched Receiver.
    pub destination_participant_key: [u8; 16],
    /// Destination-Endpoint-GUID (oder 0 fuer Participant-Wide).
    pub destination_endpoint_key: [u8; 16],
    /// Source-Endpoint-GUID.
    pub source_endpoint_key: [u8; 16],
    /// `message_class_id`-String (siehe [`class_id`]).
    pub message_class_id: String,
    /// Sequenz von `DataHolder` — typischerweise EINER (z.B. ein
    /// HandshakeMessageToken oder ein CryptoToken-Bundle).
    pub message_data: Vec<DataHolder>,
}

/// Maximaler Wire-Body eines `ParticipantGenericMessage` (DoS-Cap).
const MAX_GENERIC_MESSAGE_BYTES: usize = 256 * 1024;

/// Maximale `message_data`-Sequenz-Laenge (DoS-Cap).
const MAX_MESSAGE_DATA_LEN: u32 = 64;

/// Maximale `message_class_id`-Laenge (Spec: string<256>).
const MAX_CLASS_ID_LEN: u32 = 256;

impl ParticipantGenericMessage {
    /// Encode → XCDR1-LE Bytes (ohne PL_CDR-Encapsulation-Header — den
    /// fuegt der Wire-Layer separat an, weil ParticipantGenericMessage
    /// kein PL_CDR (ParameterList), sondern strukturiertes CDR ist).
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
            // DataHolder embedded → CDR-Bytes des DataHolders selbst,
            // length-prefixed damit der Decoder die Begrenzungen kennt.
            let dh_bytes = dh.to_cdr_le();
            encode_octet_seq(&mut out, &dh_bytes, true);
        }
        out
    }

    /// Decode aus XCDR1-LE Bytes.
    ///
    /// # Errors
    /// `BadArgument` bei Truncation, ueberschrittenen DoS-Caps oder
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
            let dh_bytes = decode_octet_seq(&mut cur)?;
            let dh = DataHolder::from_cdr_le(&dh_bytes)?;
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

fn encode_octet_seq(buf: &mut Vec<u8>, v: &[u8], le: bool) {
    encode_u32(buf, v.len() as u32, le);
    buf.extend_from_slice(v);
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

fn decode_octet_seq(cur: &mut Cursor<'_>) -> SecurityResult<Vec<u8>> {
    let len = cur.read_u32()? as usize;
    if len > MAX_GENERIC_MESSAGE_BYTES {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "generic_message: octet_seq > cap",
        ));
    }
    Ok(cur.read_slice(len)?.to_vec())
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
        // Spec §7.5.5 — diese Strings duerfen NIE driftet sein, sonst
        // matched Cyclone/FastDDS unsere Auth-Messages nicht.
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
        // Realistisches Scenario: Initiator schickt seinen
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
        // Replier setzt related_message_identity = sender_identity des
        // Requests, damit Initiator die Reply seinem Request zuordnen
        // kann.
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
        // Encode mit forged non-UTF-8-class-id-Bytes.
        let mut buf = Vec::new();
        // message_identity (24 byte: 16 + i64 padded)
        buf.extend_from_slice(&[0u8; 16]);
        buf.extend_from_slice(&0i64.to_le_bytes());
        // related_message_identity
        buf.extend_from_slice(&[0u8; 16]);
        buf.extend_from_slice(&0i64.to_le_bytes());
        // 3x GUID
        buf.extend_from_slice(&[0u8; 48]);
        // class_id-len = 5, dann 4 byte invalid utf-8 + NUL
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
        // class_id-len = 1 + NUL
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.push(0);
        align(&mut buf, 4);
        buf.extend_from_slice(&1_000_000u32.to_le_bytes());
        let err = ParticipantGenericMessage::from_cdr_le(&buf).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
