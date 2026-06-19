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

    #[test]
    fn header_round_trips_and_sizes_match() {
        let hello = HelloMessage {
            protocol_version: 0,
            receive_buffer_size: 65_536,
            send_buffer_size: 65_536,
            max_message_size: 0,
            max_chunk_count: 0,
            endpoint_url: String::from("opc.tcp://host:4840"),
        };
        let bytes = hello.encode().expect("encode");
        // Header MessageSize equals the actual byte length.
        let mut r = UaReader::new(&bytes);
        let h = MessageHeader::decode(&mut r).expect("header");
        assert_eq!(h.message_type, MessageType::Hello);
        assert_eq!(h.chunk_type, ChunkType::Final);
        assert_eq!(h.message_size as usize, bytes.len());
        let back = HelloMessage::decode_body(&mut r).expect("body");
        assert_eq!(back, hello);
        assert!(r.is_empty());
    }

    #[test]
    fn acknowledge_round_trips() {
        let ack = AcknowledgeMessage {
            protocol_version: 0,
            receive_buffer_size: 8192,
            send_buffer_size: 8192,
            max_message_size: 16_777_216,
            max_chunk_count: 5000,
        };
        let bytes = ack.encode().expect("encode");
        let mut r = UaReader::new(&bytes);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::Acknowledge
        );
        assert_eq!(AcknowledgeMessage::decode_body(&mut r).expect("b"), ack);
    }

    #[test]
    fn error_round_trips() {
        let err = ErrorMessage {
            error: 0x8086_0000, // BadTcpEndpointUrlInvalid-ish
            reason: String::from("endpoint url rejected"),
        };
        let bytes = err.encode().expect("encode");
        let mut r = UaReader::new(&bytes);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::Error
        );
        assert_eq!(ErrorMessage::decode_body(&mut r).expect("b"), err);
    }

    #[test]
    fn reverse_hello_round_trips() {
        let rhe = ReverseHelloMessage {
            server_uri: String::from("urn:host:server"),
            endpoint_url: String::from("opc.tcp://host:4840"),
        };
        let bytes = rhe.encode().expect("encode");
        let mut r = UaReader::new(&bytes);
        assert_eq!(
            MessageHeader::decode(&mut r).expect("h").message_type,
            MessageType::ReverseHello
        );
        assert_eq!(ReverseHelloMessage::decode_body(&mut r).expect("b"), rhe);
    }

    #[test]
    fn unknown_message_type_rejected() {
        let bytes = *b"XXXF\x08\x00\x00\x00";
        let mut r = UaReader::new(&bytes);
        assert!(matches!(
            MessageHeader::decode(&mut r),
            Err(DecodeError::InvalidDiscriminant { .. })
        ));
    }

    #[test]
    fn oversized_string_rejected() {
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
