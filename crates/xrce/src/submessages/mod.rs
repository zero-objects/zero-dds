// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE-Submessage-Wire-Format (Spec §8.3.4 + §8.3.5).
//!
//! Each submessage consists of a 4-byte header and an opaque
//! body. The body is only structurally validated in C6.2.A (length,
//! alignment, some scalar fields for the state-machine submessages
//! ACKNACK/HEARTBEAT/TIMESTAMP). The inner structure (XCDR2 encodings
//! of the object variants etc.) is the task of C6.2.B.
//!
//! ```text
//! +---------------+---------------+---------------+---------------+
//! | submessage_id |     flags     |       submessage_length       |
//! +---------------+---------------+---------------+---------------+
//! |                                                               |
//! ~                            payload                            ~
//! |                                                               |
//! +---------------+---------------+---------------+---------------+
//! ```
//!
//! `submessage_length` is, per §8.3.4, **always little-endian**, even
//! when the E-flag (bit 0 of `flags`) indicates that the body is BE.
//! Submessage offsets within a message are 4-byte aligned
//! (§8.3.3).

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::header::MessageHeader;

pub mod acknack;
pub mod create;
pub mod create_client;
pub mod data;
pub mod delete;
pub mod fragment;
pub mod get_info;
pub mod heartbeat;
pub mod info;
pub mod read_data;
pub mod reset;
pub mod status;
pub mod status_agent;
pub mod timestamp;
pub mod timestamp_reply;
pub mod write_data;

pub use acknack::AckNackPayload;
pub use create::CreatePayload;
pub use create_client::CreateClientPayload;
pub use data::DataPayload;
pub use delete::DeletePayload;
pub use fragment::{FRAGMENT_FLAG_LAST, FragmentPayload};
pub use get_info::GetInfoPayload;
pub use heartbeat::HeartbeatPayload;
pub use info::InfoPayload;
pub use read_data::{
    DataDeliveryControl, READ_SPEC_MAX_FILTER_BYTES, ReadDataPayload, ReadSpecification,
};
pub use reset::ResetPayload;
pub use status::StatusPayload;
pub use status_agent::StatusAgentPayload;
pub use timestamp::{TIME_T_WIRE_SIZE, TimePoint, TimestampPayload};
pub use timestamp_reply::TimestampReplyPayload;
pub use write_data::{DataFormat, WriteDataPayload};

/// E-flag bit (endianness of the body; 0 = BE, 1 = LE).
pub const FLAG_E_LITTLE_ENDIAN: u8 = 0x01;

/// DoS cap: maximum number of submessages in a message. 64 suffices
/// for all realistic use cases (Annex B examples have max ~6
/// submessages); protects against decoder bombs with thousands of 4-byte
/// headers.
pub const DOSC_MAX_SUBMESSAGES: usize = 64;

/// DoS cap: maximum payload size of a message in bytes. 64 KiB
/// corresponds to the `submessage_length` u16 limit per submessage; we
/// use this for the entire message as an outer cap too, since Spec
/// §11.2.3 says "UDP payload = exactly one XRCE message" and UDP
/// datagrams cannot become larger than 65,507 bytes.
pub const DOSC_MAX_PAYLOAD_SIZE: usize = 65_535;

/// Submessage IDs per §8.3.5 (values 0..15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum SubmessageId {
    CreateClient = 0,
    Create = 1,
    GetInfo = 2,
    Delete = 3,
    StatusAgent = 4,
    Status = 5,
    Info = 6,
    WriteData = 7,
    ReadData = 8,
    Data = 9,
    AckNack = 10,
    Heartbeat = 11,
    Reset = 12,
    Fragment = 13,
    Timestamp = 14,
    TimestampReply = 15,
}

impl SubmessageId {
    /// Raw wire value.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Converts a byte. IDs > 15 are not in the spec.
    ///
    /// # Errors
    /// `UnknownSubmessageId`.
    pub fn from_u8(byte: u8) -> Result<Self, XrceError> {
        match byte {
            0 => Ok(Self::CreateClient),
            1 => Ok(Self::Create),
            2 => Ok(Self::GetInfo),
            3 => Ok(Self::Delete),
            4 => Ok(Self::StatusAgent),
            5 => Ok(Self::Status),
            6 => Ok(Self::Info),
            7 => Ok(Self::WriteData),
            8 => Ok(Self::ReadData),
            9 => Ok(Self::Data),
            10 => Ok(Self::AckNack),
            11 => Ok(Self::Heartbeat),
            12 => Ok(Self::Reset),
            13 => Ok(Self::Fragment),
            14 => Ok(Self::Timestamp),
            15 => Ok(Self::TimestampReply),
            other => Err(XrceError::UnknownSubmessageId { id: other }),
        }
    }
}

/// SubmessageHeader (§8.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmessageHeader {
    /// Submessage class.
    pub submessage_id: SubmessageId,
    /// Flag byte. Bit 0 = E-flag (LE); further bits submessage-specific.
    pub flags: u8,
    /// Body length in bytes (always LE on the wire, §8.3.4).
    pub submessage_length: u16,
}

impl SubmessageHeader {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// `true` when the E-flag (LE body) is set.
    #[must_use]
    pub fn is_little_endian(self) -> bool {
        (self.flags & FLAG_E_LITTLE_ENDIAN) != 0
    }

    /// Endianness of the body.
    #[must_use]
    pub fn body_endianness(self) -> Endianness {
        if self.is_little_endian() {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// Encodes into a 4-byte array. `submessage_length` is always LE
    /// (Spec §8.3.4: "submessageLength little-endian regardless").
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0] = self.submessage_id.as_u8();
        out[1] = self.flags;
        let len_bytes = self.submessage_length.to_le_bytes();
        out[2..].copy_from_slice(&len_bytes);
        out
    }

    /// Decodes a 4-byte slice.
    ///
    /// # Errors
    /// `UnexpectedEof`, `UnknownSubmessageId`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, XrceError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let id = SubmessageId::from_u8(bytes[0])?;
        let flags = bytes[1];
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&bytes[2..4]);
        let submessage_length = u16::from_le_bytes(len_bytes);
        Ok(Self {
            submessage_id: id,
            flags,
            submessage_length,
        })
    }
}

/// A submessage: header + body. The body is `Vec<u8>` because the inner
/// structure is kept opaque in C6.2.A.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submessage {
    /// 4-byte header.
    pub header: SubmessageHeader,
    /// Body. Its length must match `header.submessage_length`
    /// (verified during encode).
    pub body: Vec<u8>,
}

#[cfg(feature = "alloc")]
impl Submessage {
    /// Constructs a submessage with the given ID + flags + body.
    /// Sets `submessage_length` automatically to `body.len()`.
    ///
    /// # Errors
    /// `PayloadTooLarge` if `body.len() > u16::MAX`.
    pub fn new(id: SubmessageId, flags: u8, body: Vec<u8>) -> Result<Self, XrceError> {
        if body.len() > usize::from(u16::MAX) {
            return Err(XrceError::PayloadTooLarge {
                limit: usize::from(u16::MAX),
                actual: body.len(),
            });
        }
        let len = u16::try_from(body.len()).map_err(|_| XrceError::ValueOutOfRange {
            message: "submessage body exceeds u16",
        })?;
        Ok(Self {
            header: SubmessageHeader {
                submessage_id: id,
                flags,
                submessage_length: len,
            },
            body,
        })
    }

    /// Wire size: header + body + padding to 4-byte alignment, except
    /// for the last submessage in a message.
    #[must_use]
    pub fn wire_size_unpadded(&self) -> usize {
        SubmessageHeader::WIRE_SIZE + self.body.len()
    }

    /// Padding in bytes to the next 4-byte multiple.
    #[must_use]
    pub fn padding_bytes(&self) -> usize {
        let unpadded = self.wire_size_unpadded();
        let modu = unpadded % 4;
        if modu == 0 { 0 } else { 4 - modu }
    }
}

/// XRCE message: header + 1..n submessages.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// MessageHeader.
    pub header: MessageHeader,
    /// 1..N Submessages.
    pub submessages: Vec<Submessage>,
}

#[cfg(feature = "alloc")]
impl Message {
    /// Constructor with a DoS cap check.
    ///
    /// # Errors
    /// `TooManySubmessages` if more than `DOSC_MAX_SUBMESSAGES`
    /// are given.
    pub fn new(header: MessageHeader, submessages: Vec<Submessage>) -> Result<Self, XrceError> {
        if submessages.len() > DOSC_MAX_SUBMESSAGES {
            return Err(XrceError::TooManySubmessages {
                limit: DOSC_MAX_SUBMESSAGES,
            });
        }
        Ok(Self {
            header,
            submessages,
        })
    }

    /// Encodes the entire message into a freshly allocated
    /// `Vec<u8>`.
    ///
    /// Submessage bodies are padded to 4-byte multiples
    /// (Spec §8.3.3), EXCEPT the last submessage —
    /// the spec lets the last submessage end without a trailing
    /// padding tail, because the next datagram follows.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the final size exceeds
    /// `DOSC_MAX_PAYLOAD_SIZE`.
    pub fn encode(&self) -> Result<Vec<u8>, XrceError> {
        if self.submessages.len() > DOSC_MAX_SUBMESSAGES {
            return Err(XrceError::TooManySubmessages {
                limit: DOSC_MAX_SUBMESSAGES,
            });
        }

        // Precompute the size for a single allocation.
        let header_size = self.header.wire_size();
        let mut total = header_size;
        for (i, sm) in self.submessages.iter().enumerate() {
            total += sm.wire_size_unpadded();
            // Padding after every submessage EXCEPT the last.
            if i + 1 < self.submessages.len() {
                total += sm.padding_bytes();
            }
        }
        if total > DOSC_MAX_PAYLOAD_SIZE {
            return Err(XrceError::PayloadTooLarge {
                limit: DOSC_MAX_PAYLOAD_SIZE,
                actual: total,
            });
        }

        let mut out = Vec::with_capacity(total);
        // Header
        let mut hdr_buf = [0u8; MessageHeader::WIRE_SIZE_WITH_KEY];
        let n = self.header.write_to(&mut hdr_buf)?;
        out.extend_from_slice(&hdr_buf[..n]);
        // Submessages
        for (i, sm) in self.submessages.iter().enumerate() {
            // The submessage body length must be consistent.
            if usize::from(sm.header.submessage_length) != sm.body.len() {
                return Err(XrceError::ValueOutOfRange {
                    message: "submessage_length mismatches body length",
                });
            }
            out.extend_from_slice(&sm.header.to_bytes());
            out.extend_from_slice(&sm.body);
            if i + 1 < self.submessages.len() {
                let pad = sm.padding_bytes();
                if pad > 0 {
                    out.resize(out.len() + pad, 0);
                }
            }
        }
        Ok(out)
    }

    /// Decodes a complete message from a datagram.
    ///
    /// Validates submessage alignment (4-byte) and consumes bytes
    /// up to the end of the buffer or to a truncation error.
    ///
    /// # Errors
    /// `UnexpectedEof`, `UnknownSubmessageId`,
    /// `TruncatedSubmessageBody`, `TooManySubmessages`,
    /// `PayloadTooLarge`, `UnalignedSubmessage`.
    pub fn decode(bytes: &[u8]) -> Result<Self, XrceError> {
        if bytes.len() > DOSC_MAX_PAYLOAD_SIZE {
            return Err(XrceError::PayloadTooLarge {
                limit: DOSC_MAX_PAYLOAD_SIZE,
                actual: bytes.len(),
            });
        }

        let (header, hdr_len) = MessageHeader::read_from(bytes)?;
        let mut offset = hdr_len;
        let mut submessages: Vec<Submessage> = Vec::new();

        while offset < bytes.len() {
            // §8.3.3: offsets are 4-byte-aligned. Header starts
            // at offset 0, header size is 4 or 8 → already aligned.
            // We check before every submessage read.
            if offset % 4 != 0 {
                return Err(XrceError::UnalignedSubmessage { offset });
            }

            // Submessage-Header
            if bytes.len() - offset < SubmessageHeader::WIRE_SIZE {
                return Err(XrceError::UnexpectedEof {
                    needed: SubmessageHeader::WIRE_SIZE,
                    offset,
                });
            }
            let sh = SubmessageHeader::from_bytes(&bytes[offset..])?;
            offset += SubmessageHeader::WIRE_SIZE;

            let body_len = usize::from(sh.submessage_length);
            let available = bytes.len() - offset;
            if body_len > available {
                return Err(XrceError::TruncatedSubmessageBody {
                    declared: sh.submessage_length,
                    available,
                });
            }
            let body = bytes[offset..offset + body_len].to_vec();
            offset += body_len;

            submessages.push(Submessage { header: sh, body });

            if submessages.len() > DOSC_MAX_SUBMESSAGES {
                return Err(XrceError::TooManySubmessages {
                    limit: DOSC_MAX_SUBMESSAGES,
                });
            }

            // Consume padding to the 4-byte boundary — only if bytes
            // still follow after this submessage.
            if offset < bytes.len() {
                let pad = (4 - (offset % 4)) % 4;
                let pad_take = pad.min(bytes.len() - offset);
                offset += pad_take;
            }
        }
        Ok(Self {
            header,
            submessages,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::header::{ClientKey, SessionId, StreamId};
    use crate::serial_number::SerialNumber16;

    #[test]
    fn submessage_id_roundtrip_all_16_values() {
        for byte in 0u8..=15 {
            let id = SubmessageId::from_u8(byte).unwrap();
            assert_eq!(id.as_u8(), byte);
        }
    }

    #[test]
    fn submessage_id_rejects_unknown_byte() {
        let res = SubmessageId::from_u8(16);
        assert!(matches!(
            res,
            Err(XrceError::UnknownSubmessageId { id: 16 })
        ));
        let res = SubmessageId::from_u8(0xFF);
        assert!(matches!(
            res,
            Err(XrceError::UnknownSubmessageId { id: 0xFF })
        ));
    }

    #[test]
    fn submessage_header_roundtrip() {
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Heartbeat,
            flags: FLAG_E_LITTLE_ENDIAN,
            submessage_length: 5,
        };
        let bytes = sh.to_bytes();
        assert_eq!(bytes[0], 11); // Heartbeat
        assert_eq!(bytes[1], 1);
        assert_eq!(bytes[2], 5);
        assert_eq!(bytes[3], 0);
        let decoded = SubmessageHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, sh);
    }

    #[test]
    fn submessage_header_length_is_always_le_even_with_be_body() {
        // E-flag = 0 (BE body), but length is LE nonetheless
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: 0,
            submessage_length: 0x0102,
        };
        let bytes = sh.to_bytes();
        assert_eq!(bytes[2], 0x02); // LE: low byte first
        assert_eq!(bytes[3], 0x01);
    }

    #[test]
    fn message_encode_decode_roundtrip_no_key_single_sm() {
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::BUILTIN_BEST_EFFORT,
            SerialNumber16::new(7),
        )
        .unwrap();
        let sm = Submessage::new(
            SubmessageId::WriteData,
            FLAG_E_LITTLE_ENDIAN,
            alloc::vec![0xAA, 0xBB, 0xCC, 0xDD],
        )
        .unwrap();
        let msg = Message::new(header, alloc::vec![sm]).unwrap();
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn message_encode_decode_roundtrip_with_key() {
        let header = MessageHeader::with_client_key(
            SessionId(0x10),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(0x1234),
            ClientKey([1, 2, 3, 4]),
        )
        .unwrap();
        let sm = Submessage::new(
            SubmessageId::Heartbeat,
            FLAG_E_LITTLE_ENDIAN,
            alloc::vec![0; 5],
        )
        .unwrap();
        let msg = Message::new(header, alloc::vec![sm]).unwrap();
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn message_two_submessages_padded_correctly() {
        let header =
            MessageHeader::without_client_key(SessionId(0x80), StreamId(2), SerialNumber16::new(0))
                .unwrap();
        // Body of 5 bytes → next submessage must start at +(5+3)=8
        let sm1 = Submessage::new(
            SubmessageId::WriteData,
            FLAG_E_LITTLE_ENDIAN,
            alloc::vec![1, 2, 3, 4, 5],
        )
        .unwrap();
        let sm2 = Submessage::new(
            SubmessageId::AckNack,
            FLAG_E_LITTLE_ENDIAN,
            alloc::vec![10, 20, 30, 40, 50],
        )
        .unwrap();
        let msg = Message::new(header, alloc::vec![sm1, sm2]).unwrap();
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes).unwrap();
        assert_eq!(decoded.submessages.len(), 2);
        assert_eq!(decoded.submessages[0].body, alloc::vec![1, 2, 3, 4, 5]);
        assert_eq!(decoded.submessages[1].body, alloc::vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn message_decode_rejects_too_many_submessages_via_too_many_concat() {
        // Manually assembled datagram with DOSC_MAX_SUBMESSAGES+1
        // Reset-Submessages (Body=0).
        let header = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let mut hdr_buf = [0u8; 4];
        header.write_to(&mut hdr_buf).unwrap();
        let mut bytes: Vec<u8> = hdr_buf.to_vec();
        for _ in 0..(DOSC_MAX_SUBMESSAGES + 1) {
            bytes.extend_from_slice(&[SubmessageId::Reset.as_u8(), 0, 0, 0]);
        }
        let res = Message::decode(&bytes);
        assert!(matches!(
            res,
            Err(XrceError::TooManySubmessages { limit }) if limit == DOSC_MAX_SUBMESSAGES
        ));
    }

    #[test]
    fn message_decode_rejects_truncated_body() {
        let header = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let mut hdr_buf = [0u8; 4];
        header.write_to(&mut hdr_buf).unwrap();
        let mut bytes: Vec<u8> = hdr_buf.to_vec();
        // Submessage with length=10 but 0 bytes of body
        bytes.extend_from_slice(&[SubmessageId::WriteData.as_u8(), FLAG_E_LITTLE_ENDIAN, 10, 0]);
        let res = Message::decode(&bytes);
        assert!(matches!(
            res,
            Err(XrceError::TruncatedSubmessageBody {
                declared: 10,
                available: 0
            })
        ));
    }

    #[test]
    fn message_decode_rejects_unknown_submessage_id() {
        let header = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let mut hdr_buf = [0u8; 4];
        header.write_to(&mut hdr_buf).unwrap();
        let mut bytes: Vec<u8> = hdr_buf.to_vec();
        bytes.extend_from_slice(&[42, 0, 0, 0]);
        let res = Message::decode(&bytes);
        assert!(matches!(
            res,
            Err(XrceError::UnknownSubmessageId { id: 42 })
        ));
    }

    #[test]
    fn message_encode_rejects_too_many_submessages_via_constructor() {
        let header = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let sms: Vec<Submessage> = (0..(DOSC_MAX_SUBMESSAGES + 1))
            .map(|_| Submessage::new(SubmessageId::Reset, 0, alloc::vec![]).unwrap())
            .collect();
        let res = Message::new(header, sms);
        assert!(matches!(
            res,
            Err(XrceError::TooManySubmessages { limit }) if limit == DOSC_MAX_SUBMESSAGES
        ));
    }

    #[test]
    fn message_decode_rejects_oversized_payload_input() {
        let huge: Vec<u8> = alloc::vec![0u8; DOSC_MAX_PAYLOAD_SIZE + 1];
        let res = Message::decode(&huge);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }
}
