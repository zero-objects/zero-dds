// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UACP connection-setup messages (OPC-UA Part 6 §7.1.2): the message header
//! plus Hello / Acknowledge / Error / ReverseHello.
//!
//! Every UACP message is `[MessageType:3][ChunkType:1][MessageSize:u32][body]`
//! where `MessageSize` is the total length including the 8-byte header. All
//! primitives are little-endian (Part 6 §5.2); strings are an `Int32` byte
//! length (`-1` = null) followed by UTF-8.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaReader, UaWriter};

const HEADER_LEN: usize = 8;
/// Maximum length of an EndpointUrl / ServerUri string (Part 6 §7.1.2.3).
const MAX_STRING_LEN: usize = 4096;

/// UACP message type (the 3 leading ASCII bytes, Part 6 §7.1.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// `HEL` — Hello (client → server, connection request).
    Hello,
    /// `ACK` — Acknowledge (server → client).
    Acknowledge,
    /// `ERR` — Error.
    Error,
    /// `RHE` — ReverseHello (server → client, reverse connect).
    ReverseHello,
    /// `MSG` — a SecureChannel message.
    Message,
    /// `OPN` — OpenSecureChannel.
    OpenSecureChannel,
    /// `CLO` — CloseSecureChannel.
    CloseSecureChannel,
}

impl MessageType {
    const fn to_bytes(self) -> [u8; 3] {
        match self {
            Self::Hello => *b"HEL",
            Self::Acknowledge => *b"ACK",
            Self::Error => *b"ERR",
            Self::ReverseHello => *b"RHE",
            Self::Message => *b"MSG",
            Self::OpenSecureChannel => *b"OPN",
            Self::CloseSecureChannel => *b"CLO",
        }
    }

    const fn from_bytes(b: [u8; 3]) -> Result<Self, DecodeError> {
        Ok(match &b {
            b"HEL" => Self::Hello,
            b"ACK" => Self::Acknowledge,
            b"ERR" => Self::Error,
            b"RHE" => Self::ReverseHello,
            b"MSG" => Self::Message,
            b"OPN" => Self::OpenSecureChannel,
            b"CLO" => Self::CloseSecureChannel,
            _ => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "UACP MessageType",
                    // Pack the 3 bytes into the diagnostic value.
                    value: u32::from_be_bytes([0, b[0], b[1], b[2]]),
                });
            }
        })
    }
}

/// UACP chunk type (Part 6 §7.1.2.2): the 4th header byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkType {
    /// `F` — final chunk (connection-setup messages are always final).
    Final,
    /// `C` — intermediate chunk.
    Intermediate,
    /// `A` — abort.
    Abort,
}

impl ChunkType {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Final => b'F',
            Self::Intermediate => b'C',
            Self::Abort => b'A',
        }
    }

    const fn from_byte(b: u8) -> Result<Self, DecodeError> {
        Ok(match b {
            b'F' => Self::Final,
            b'C' => Self::Intermediate,
            b'A' => Self::Abort,
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "UACP ChunkType",
                    value: other as u32,
                });
            }
        })
    }
}

/// The 8-byte UACP message header (Part 6 §7.1.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    /// Message type.
    pub message_type: MessageType,
    /// Chunk type.
    pub chunk_type: ChunkType,
    /// Total message size in bytes (including this 8-byte header).
    pub message_size: u32,
}

impl MessageHeader {
    /// Encodes the 8-byte header.
    pub fn encode(&self, w: &mut UaWriter) {
        w.write_bytes(&self.message_type.to_bytes());
        w.write_u8(self.chunk_type.to_byte());
        w.write_u32(self.message_size);
    }

    /// Decodes the 8-byte header.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or an unknown type/chunk byte.
    pub fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let mut t = [0u8; 3];
        t.copy_from_slice(r.read_bytes(3)?);
        let message_type = MessageType::from_bytes(t)?;
        let chunk_type = ChunkType::from_byte(r.read_u8()?)?;
        let message_size = r.read_u32()?;
        Ok(Self {
            message_type,
            chunk_type,
            message_size,
        })
    }
}

fn write_string(w: &mut UaWriter, s: &str) -> Result<(), EncodeError> {
    let len = i32::try_from(s.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "UACP String",
        len: s.len(),
    })?;
    w.write_i32(len);
    w.write_bytes(s.as_bytes());
    Ok(())
}

fn read_string(r: &mut UaReader<'_>) -> Result<String, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(String::new());
    }
    let len = len as usize;
    if len > MAX_STRING_LEN {
        return Err(DecodeError::MalformedMessage {
            message: "UACP String exceeds the 4096-byte limit",
        });
    }
    let bytes = r.read_bytes(len)?;
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| DecodeError::InvalidUtf8)
}

/// Wraps `message_type` + `body` in a full UACP message (prepends the header
/// with the computed `MessageSize`). Connection-setup messages are `Final`.
fn frame(message_type: MessageType, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let total = HEADER_LEN + body.len();
    let message_size = u32::try_from(total).map_err(|_| EncodeError::LengthOverflow {
        what: "UACP MessageSize",
        len: total,
    })?;
    let mut w = UaWriter::with_capacity(total);
    MessageHeader {
        message_type,
        chunk_type: ChunkType::Final,
        message_size,
    }
    .encode(&mut w);
    w.write_bytes(body);
    Ok(w.into_vec())
}

/// UACP `Hello` message (Part 6 §7.1.2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloMessage {
    /// Protocol version the client supports.
    pub protocol_version: u32,
    /// Largest receive buffer the client will accept.
    pub receive_buffer_size: u32,
    /// Largest send buffer the client will use.
    pub send_buffer_size: u32,
    /// Maximum message size (0 = no limit).
    pub max_message_size: u32,
    /// Maximum chunk count (0 = no limit).
    pub max_chunk_count: u32,
    /// Endpoint URL the client wants to connect to.
    pub endpoint_url: String,
}

impl HelloMessage {
    /// Encodes the full `HEL` message (header + body).
    ///
    /// # Errors
    /// [`EncodeError`] if the EndpointUrl exceeds the `Int32` length ceiling.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut b = UaWriter::new();
        b.write_u32(self.protocol_version);
        b.write_u32(self.receive_buffer_size);
        b.write_u32(self.send_buffer_size);
        b.write_u32(self.max_message_size);
        b.write_u32(self.max_chunk_count);
        write_string(&mut b, &self.endpoint_url)?;
        frame(MessageType::Hello, b.as_slice())
    }

    /// Decodes a `HEL` body (the reader is positioned after the header).
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or a bad EndpointUrl.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            protocol_version: r.read_u32()?,
            receive_buffer_size: r.read_u32()?,
            send_buffer_size: r.read_u32()?,
            max_message_size: r.read_u32()?,
            max_chunk_count: r.read_u32()?,
            endpoint_url: read_string(r)?,
        })
    }
}

/// UACP `Acknowledge` message (Part 6 §7.1.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcknowledgeMessage {
    /// Protocol version the server supports.
    pub protocol_version: u32,
    /// Largest receive buffer the server will accept.
    pub receive_buffer_size: u32,
    /// Largest send buffer the server will use.
    pub send_buffer_size: u32,
    /// Maximum message size (0 = no limit).
    pub max_message_size: u32,
    /// Maximum chunk count (0 = no limit).
    pub max_chunk_count: u32,
}

impl AcknowledgeMessage {
    /// Encodes the full `ACK` message.
    ///
    /// # Errors
    /// [`EncodeError`] is not returned in practice (fixed-size body).
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut b = UaWriter::new();
        b.write_u32(self.protocol_version);
        b.write_u32(self.receive_buffer_size);
        b.write_u32(self.send_buffer_size);
        b.write_u32(self.max_message_size);
        b.write_u32(self.max_chunk_count);
        frame(MessageType::Acknowledge, b.as_slice())
    }

    /// Decodes an `ACK` body.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            protocol_version: r.read_u32()?,
            receive_buffer_size: r.read_u32()?,
            send_buffer_size: r.read_u32()?,
            max_message_size: r.read_u32()?,
            max_chunk_count: r.read_u32()?,
        })
    }
}

/// UACP `Error` message (Part 6 §7.1.2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorMessage {
    /// Numeric OPC-UA StatusCode.
    pub error: u32,
    /// Human-readable reason (max 4096 bytes).
    pub reason: String,
}

impl ErrorMessage {
    /// Encodes the full `ERR` message.
    ///
    /// # Errors
    /// [`EncodeError`] if the reason exceeds the `Int32` length ceiling.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut b = UaWriter::new();
        b.write_u32(self.error);
        write_string(&mut b, &self.reason)?;
        frame(MessageType::Error, b.as_slice())
    }

    /// Decodes an `ERR` body.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or a bad reason string.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            error: r.read_u32()?,
            reason: read_string(r)?,
        })
    }
}

/// UACP `ReverseHello` message (Part 6 §7.1.2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseHelloMessage {
    /// URI of the server initiating the reverse connection.
    pub server_uri: String,
    /// Endpoint URL the client should use.
    pub endpoint_url: String,
}

impl ReverseHelloMessage {
    /// Encodes the full `RHE` message.
    ///
    /// # Errors
    /// [`EncodeError`] if a string exceeds the `Int32` length ceiling.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut b = UaWriter::new();
        write_string(&mut b, &self.server_uri)?;
        write_string(&mut b, &self.endpoint_url)?;
        frame(MessageType::ReverseHello, b.as_slice())
    }

    /// Decodes an `RHE` body.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or a bad string.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            server_uri: read_string(r)?,
            endpoint_url: read_string(r)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The octets of OPC 10000-6 §7.1.2: the message header (Table 73: MessageType,
    /// 'F', MessageSize as a little-endian UInt32 over the whole
    /// message), then the body fields in the order of the clause's table,
    /// UInt32 little endian, a String as an Int32 length and its UTF-8
    /// octets (§5.2.2.4).
    fn message(kind: &[u8; 3], body: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.push(b'F');
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn ua_string(s: &str) -> Vec<u8> {
        let mut out = i32::try_from(s.len()).unwrap().to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out
    }

    /// §7.1.2.3, Table 74: Hello is ProtocolVersion, ReceiveBufferSize,
    /// SendBufferSize, MaxMessageSize, MaxChunkCount, EndpointUrl.
    #[test]
    fn uacs_7_1_2_3_a_hello_is_the_header_and_the_fields_of_table_74() {
        let hello = HelloMessage {
            protocol_version: 0,
            receive_buffer_size: 65_536,
            send_buffer_size: 65_536,
            max_message_size: 0,
            max_chunk_count: 0,
            endpoint_url: String::from("opc.tcp://host:4840"),
        };
        let mut body = Vec::new();
        for v in [0_u32, 65_536, 65_536, 0, 0] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&ua_string("opc.tcp://host:4840"));
        let expected = message(b"HEL", &body);
        assert_eq!(hello.encode().expect("encode"), expected);
        let mut r = UaReader::new(&expected);
        let h = MessageHeader::decode(&mut r).expect("header");
        assert_eq!(h.message_type, MessageType::Hello);
        assert_eq!(h.chunk_type, ChunkType::Final);
        assert_eq!(h.message_size as usize, expected.len());
        assert_eq!(HelloMessage::decode_body(&mut r).expect("body"), hello);
        assert!(r.is_empty());
    }

    /// §7.1.2.4, Table 75: Acknowledge is ProtocolVersion,
    /// ReceiveBufferSize, SendBufferSize, MaxMessageSize, MaxChunkCount.
    #[test]
    fn uacs_7_1_2_4_an_acknowledge_is_the_header_and_the_fields_of_table_75() {
        let ack = AcknowledgeMessage {
            protocol_version: 0,
            receive_buffer_size: 8192,
            send_buffer_size: 8192,
            max_message_size: 16_777_216,
            max_chunk_count: 5000,
        };
        let mut body = Vec::new();
        for v in [0_u32, 8192, 8192, 16_777_216, 5000] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let expected = message(b"ACK", &body);
        assert_eq!(ack.encode().expect("encode"), expected);
        let mut r = UaReader::new(&expected);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::Acknowledge
        );
        assert_eq!(AcknowledgeMessage::decode_body(&mut r).expect("b"), ack);
    }

    /// §7.1.2.5, Table 76: Error is the StatusCode and a Reason String
    /// (Bad_TcpEndpointUrlInvalid 0x80830000 of StatusCode.csv, the
    /// UA-Nodeset).
    #[test]
    fn uacs_7_1_2_5_an_error_is_the_header_a_status_code_and_a_reason() {
        let err = ErrorMessage {
            error: 0x8083_0000,
            reason: String::from("endpoint url rejected"),
        };
        let mut body = 0x8083_0000_u32.to_le_bytes().to_vec();
        body.extend_from_slice(&ua_string("endpoint url rejected"));
        let expected = message(b"ERR", &body);
        assert_eq!(err.encode().expect("encode"), expected);
        let mut r = UaReader::new(&expected);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::Error
        );
        assert_eq!(ErrorMessage::decode_body(&mut r).expect("b"), err);
    }

    /// §7.1.2.6, Table 77: ReverseHello is ServerUri and EndpointUrl.
    #[test]
    fn uacs_7_1_2_6_a_reverse_hello_is_the_header_server_uri_and_endpoint_url() {
        let rhe = ReverseHelloMessage {
            server_uri: String::from("urn:host:server"),
            endpoint_url: String::from("opc.tcp://host:4840"),
        };
        let mut body = ua_string("urn:host:server");
        body.extend_from_slice(&ua_string("opc.tcp://host:4840"));
        let expected = message(b"RHE", &body);
        assert_eq!(rhe.encode().expect("encode"), expected);
        let mut r = UaReader::new(&expected);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::ReverseHello
        );
        assert_eq!(ReverseHelloMessage::decode_body(&mut r).expect("b"), rhe);
    }

    #[test]
    fn uacs_7_1_unknown_message_type_rejected() {
        let bytes = *b"XXXF\x08\x00\x00\x00";
        let mut r = UaReader::new(&bytes);
        assert!(matches!(
            MessageHeader::decode(&mut r),
            Err(DecodeError::InvalidDiscriminant { .. })
        ));
    }

    #[test]
    fn uacs_7_1_oversized_string_rejected() {
        // MessageSize is honest but the string length claims > 4096.
        let mut b = UaWriter::new();
        b.write_i32(5000);
        let mut r = UaReader::new(b.as_slice());
        assert!(matches!(
            read_string(&mut r),
            Err(DecodeError::MalformedMessage { .. })
        ));
    }
}
