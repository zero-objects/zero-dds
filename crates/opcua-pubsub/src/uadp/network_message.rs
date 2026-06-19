// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UADP `NetworkMessage` framing (OPC Foundation Part 14 §7.2.2.2) — the
//! outer message that carries one or more [`DataSetMessage`]s on the wire.
//!
//! The layout is a cascade of optional header blocks gated by three flag
//! bytes:
//!
//! 1. `UADPFlags` (always present) — version + which top-level blocks
//!    follow (PublisherId / GroupHeader / PayloadHeader / ExtendedFlags1).
//! 2. `ExtendedFlags1` (optional) — PublisherId type, DataSetClassId,
//!    Security, Timestamp, PicoSeconds, ExtendedFlags2.
//! 3. `ExtendedFlags2` (optional) — Chunk, PromotedFields, NetworkMessage
//!    type.
//!
//! followed, in spec order, by the NetworkMessage header (PublisherId,
//! DataSetClassId), the [`GroupHeader`], the PayloadHeader (writer-id
//! list), the extended header (Timestamp, PicoSeconds, PromotedFields) and
//! finally the payload (an optional size array plus the DataSetMessages).
//!
//! # Scope (incremental layering)
//!
//! This stage implements the DataSetMessage payload type without security.
//! A NetworkMessage whose Security flag (`ExtendedFlags1` bit 4) or whose
//! non-DataSetMessage type (`ExtendedFlags2` bits 2-4) is set is rejected
//! on decode with [`DecodeError::MalformedMessage`] — PubSub security is
//! stage 5 and UADP discovery is stage 4. Chunked transport
//! (`ExtendedFlags2` bit 0) is likewise rejected.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::Variant;
use zerodds_opcua_gateway::types::Guid;

use crate::binary::{UaDecode, UaEncode, UaReader, UaWriter, len_u16, read_string, write_string};
use crate::error::{DecodeError, EncodeError};
use crate::uadp::dataset_message::DataSetMessage;

// UADPFlags (Part 14 §7.2.2.2.2).
const UADP_VERSION: u8 = 1;
const UF_VERSION_MASK: u8 = 0x0F;
const UF_PUBLISHER_ID: u8 = 0x10;
const UF_GROUP_HEADER: u8 = 0x20;
const UF_PAYLOAD_HEADER: u8 = 0x40;
const UF_EXTENDED_FLAGS1: u8 = 0x80;

// ExtendedFlags1 (Part 14 §7.2.2.2.3).
const EF1_PUBLISHER_ID_TYPE_MASK: u8 = 0x07; // bits 0-2
const EF1_DATASET_CLASS_ID: u8 = 0x08;
const EF1_SECURITY: u8 = 0x10;
const EF1_TIMESTAMP: u8 = 0x20;
const EF1_PICOSECONDS: u8 = 0x40;
const EF1_EXTENDED_FLAGS2: u8 = 0x80;

// ExtendedFlags2 (Part 14 §7.2.2.2.4).
const EF2_CHUNK: u8 = 0x01;
const EF2_PROMOTED_FIELDS: u8 = 0x02;
const EF2_NM_TYPE_MASK: u8 = 0x1C; // bits 2-4 (0 = DataSetMessage)
const EF2_NM_TYPE_SHIFT: u8 = 2;

// GroupFlags (Part 14 §7.2.2.2.5).
const GF_WRITER_GROUP_ID: u8 = 0x01;
const GF_GROUP_VERSION: u8 = 0x02;
const GF_NETWORK_MESSAGE_NUMBER: u8 = 0x04;
const GF_SEQUENCE_NUMBER: u8 = 0x08;

/// PublisherId of a UADP NetworkMessage (Part 14 §6.2.7). The wire type is
/// carried in `ExtendedFlags1` bits 0-2; absent ExtendedFlags1 implies the
/// `Byte` form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublisherId {
    /// `Byte` PublisherId (type code 0).
    Byte(u8),
    /// `UInt16` PublisherId (type code 1).
    UInt16(u16),
    /// `UInt32` PublisherId (type code 2).
    UInt32(u32),
    /// `UInt64` PublisherId (type code 3).
    UInt64(u64),
    /// `String` PublisherId (type code 4).
    String(String),
}

impl PublisherId {
    const fn type_bits(&self) -> u8 {
        match self {
            Self::Byte(_) => 0,
            Self::UInt16(_) => 1,
            Self::UInt32(_) => 2,
            Self::UInt64(_) => 3,
            Self::String(_) => 4,
        }
    }

    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        match self {
            Self::Byte(v) => w.write_u8(*v),
            Self::UInt16(v) => w.write_u16(*v),
            Self::UInt32(v) => w.write_u32(*v),
            Self::UInt64(v) => w.write_u64(*v),
            Self::String(s) => write_string(w, s)?,
        }
        Ok(())
    }

    fn decode(r: &mut UaReader<'_>, type_bits: u8) -> Result<Self, DecodeError> {
        Ok(match type_bits {
            0 => Self::Byte(r.read_u8()?),
            1 => Self::UInt16(r.read_u16()?),
            2 => Self::UInt32(r.read_u32()?),
            3 => Self::UInt64(r.read_u64()?),
            4 => Self::String(read_string(r)?),
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "PublisherId type",
                    value: other as u32,
                });
            }
        })
    }
}

/// Writes the shared header of a discovery NetworkMessage (Part 14
/// §7.2.2.4): the UADPFlags, ExtendedFlags1/2 carrying the NetworkMessage
/// type `nm_type`, and the optional PublisherId. Discovery messages carry no
/// GroupHeader or PayloadHeader. Used by [`crate::uadp::discovery`].
pub(crate) fn write_discovery_header(
    w: &mut UaWriter,
    nm_type: u8,
    publisher_id: Option<&PublisherId>,
) -> Result<(), EncodeError> {
    // ExtendedFlags1 is always present because ExtendedFlags2 (the
    // NetworkMessage type) is always needed for a discovery message.
    let mut ext1 = EF1_EXTENDED_FLAGS2;
    if let Some(p) = publisher_id {
        ext1 |= p.type_bits() & EF1_PUBLISHER_ID_TYPE_MASK;
    }
    let mut uadp = (UADP_VERSION & UF_VERSION_MASK) | UF_EXTENDED_FLAGS1;
    if publisher_id.is_some() {
        uadp |= UF_PUBLISHER_ID;
    }
    w.write_u8(uadp);
    w.write_u8(ext1);
    w.write_u8((nm_type << EF2_NM_TYPE_SHIFT) & EF2_NM_TYPE_MASK);
    if let Some(p) = publisher_id {
        p.encode(w)?;
    }
    Ok(())
}

/// Reads the shared discovery NetworkMessage header, returning the
/// NetworkMessage type and optional PublisherId (Part 14 §7.2.2.4).
pub(crate) fn read_discovery_header(
    r: &mut UaReader<'_>,
) -> Result<(u8, Option<PublisherId>), DecodeError> {
    let uadp = r.read_u8()?;
    let version = uadp & UF_VERSION_MASK;
    if version != UADP_VERSION {
        return Err(DecodeError::InvalidDiscriminant {
            field: "UADP version",
            value: version as u32,
        });
    }
    let has_publisher = uadp & UF_PUBLISHER_ID != 0;
    let ext1 = if uadp & UF_EXTENDED_FLAGS1 != 0 {
        r.read_u8()?
    } else {
        0
    };
    let publisher_type = ext1 & EF1_PUBLISHER_ID_TYPE_MASK;
    let ext2 = if ext1 & EF1_EXTENDED_FLAGS2 != 0 {
        r.read_u8()?
    } else {
        0
    };
    let nm_type = (ext2 & EF2_NM_TYPE_MASK) >> EF2_NM_TYPE_SHIFT;
    let publisher_id = if has_publisher {
        Some(PublisherId::decode(r, publisher_type)?)
    } else {
        None
    };
    Ok((nm_type, publisher_id))
}

/// The UADP `GroupHeader` (Part 14 §7.2.2.2.5) — per-WriterGroup framing
/// shared by every DataSetMessage in the NetworkMessage. Every field is
/// independently optional, gated by its `GroupFlags` bit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupHeader {
    /// Identifier of the WriterGroup that produced the message.
    pub writer_group_id: Option<u16>,
    /// Configuration version of the WriterGroup (`VersionTime`).
    pub group_version: Option<u32>,
    /// Index of this NetworkMessage within the publishing cycle.
    pub network_message_number: Option<u16>,
    /// Per-group rolling sequence number.
    pub sequence_number: Option<u16>,
}

impl GroupHeader {
    /// `true` if no field is set (the header carries no information and the
    /// encoder may still emit it as a single zero `GroupFlags` byte).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.writer_group_id.is_none()
            && self.group_version.is_none()
            && self.network_message_number.is_none()
            && self.sequence_number.is_none()
    }

    fn encode(&self, w: &mut UaWriter) {
        let mut flags = 0u8;
        if self.writer_group_id.is_some() {
            flags |= GF_WRITER_GROUP_ID;
        }
        if self.group_version.is_some() {
            flags |= GF_GROUP_VERSION;
        }
        if self.network_message_number.is_some() {
            flags |= GF_NETWORK_MESSAGE_NUMBER;
        }
        if self.sequence_number.is_some() {
            flags |= GF_SEQUENCE_NUMBER;
        }
        w.write_u8(flags);
        if let Some(v) = self.writer_group_id {
            w.write_u16(v);
        }
        if let Some(v) = self.group_version {
            w.write_u32(v);
        }
        if let Some(v) = self.network_message_number {
            w.write_u16(v);
        }
        if let Some(v) = self.sequence_number {
            w.write_u16(v);
        }
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let flags = r.read_u8()?;
        Ok(Self {
            writer_group_id: (flags & GF_WRITER_GROUP_ID != 0)
                .then(|| r.read_u16())
                .transpose()?,
            group_version: (flags & GF_GROUP_VERSION != 0)
                .then(|| r.read_u32())
                .transpose()?,
            network_message_number: (flags & GF_NETWORK_MESSAGE_NUMBER != 0)
                .then(|| r.read_u16())
                .transpose()?,
            sequence_number: (flags & GF_SEQUENCE_NUMBER != 0)
                .then(|| r.read_u16())
                .transpose()?,
        })
    }
}

/// A UADP `NetworkMessage` (Part 14 §7.2.2.2) carrying one or more
/// [`DataSetMessage`]s.
///
/// The PublisherId, DataSetClassId, GroupHeader, Timestamp, PicoSeconds and
/// PromotedFields are all optional header blocks. When [`payload_header`] is
/// `true` the message emits a PayloadHeader (the DataSetWriter-id list taken
/// from each contained message); this is mandatory for more than one
/// DataSetMessage and is the standard configuration for real publishers.
///
/// [`payload_header`]: Self::payload_header
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkMessage {
    /// Identifier of the Publisher that produced the message.
    pub publisher_id: Option<PublisherId>,
    /// DataSetClass shared by every contained DataSetMessage.
    pub data_set_class_id: Option<Guid>,
    /// Per-WriterGroup header block.
    pub group_header: Option<GroupHeader>,
    /// NetworkMessage-level source timestamp (DateTime ticks).
    pub timestamp: Option<i64>,
    /// Picoseconds refinement of [`timestamp`](Self::timestamp).
    pub pico_seconds: Option<u16>,
    /// Promoted fields lifted out of the DataSets for cheap filtering by
    /// subscribers without decoding the payload.
    pub promoted_fields: Vec<Variant>,
    /// Whether to emit a PayloadHeader (DataSetWriter-id list). Required
    /// when [`messages`](Self::messages) holds more than one entry.
    pub payload_header: bool,
    /// The carried DataSetMessages.
    pub messages: Vec<DataSetMessage>,
}

impl NetworkMessage {
    /// Builds a NetworkMessage carrying `messages` with a PayloadHeader and
    /// no other optional header blocks.
    #[must_use]
    pub fn with_messages(messages: Vec<DataSetMessage>) -> Self {
        Self {
            publisher_id: None,
            data_set_class_id: None,
            group_header: None,
            timestamp: None,
            pico_seconds: None,
            promoted_fields: Vec::new(),
            payload_header: true,
            messages,
        }
    }
}

impl NetworkMessage {
    /// Encodes everything up to (but not including) the payload: the flag
    /// bytes, PublisherId, DataSetClassId, GroupHeader, PayloadHeader and the
    /// extended header. `with_security` sets the `ExtendedFlags1` Security bit
    /// so [`crate::security`] can splice a SecurityHeader between this and the
    /// payload.
    pub(crate) fn encode_header(
        &self,
        w: &mut UaWriter,
        with_security: bool,
    ) -> Result<(), EncodeError> {
        if self.messages.len() > 1 && !self.payload_header {
            return Err(EncodeError::ValueOutOfRange {
                message: "more than one DataSetMessage requires a PayloadHeader",
            });
        }

        let promoted = !self.promoted_fields.is_empty();

        let mut ext2 = 0u8;
        if promoted {
            ext2 |= EF2_PROMOTED_FIELDS;
        }
        let ext2_needed = ext2 != 0;

        // ExtendedFlags1 — also set when ExtendedFlags2 is needed (bit 7).
        let mut ext1 = 0u8;
        if let Some(p) = &self.publisher_id {
            ext1 |= p.type_bits() & EF1_PUBLISHER_ID_TYPE_MASK;
        }
        if self.data_set_class_id.is_some() {
            ext1 |= EF1_DATASET_CLASS_ID;
        }
        if with_security {
            ext1 |= EF1_SECURITY;
        }
        if self.timestamp.is_some() {
            ext1 |= EF1_TIMESTAMP;
        }
        if self.pico_seconds.is_some() {
            ext1 |= EF1_PICOSECONDS;
        }
        if ext2_needed {
            ext1 |= EF1_EXTENDED_FLAGS2;
        }
        let ext1_needed = ext1 != 0;

        let mut uadp = UADP_VERSION & UF_VERSION_MASK;
        if self.publisher_id.is_some() {
            uadp |= UF_PUBLISHER_ID;
        }
        if self.group_header.is_some() {
            uadp |= UF_GROUP_HEADER;
        }
        if self.payload_header {
            uadp |= UF_PAYLOAD_HEADER;
        }
        if ext1_needed {
            uadp |= UF_EXTENDED_FLAGS1;
        }

        w.write_u8(uadp);
        if ext1_needed {
            w.write_u8(ext1);
        }
        if ext2_needed {
            w.write_u8(ext2);
        }

        if let Some(p) = &self.publisher_id {
            p.encode(w)?;
        }
        if let Some(g) = &self.data_set_class_id {
            g.encode(w)?;
        }
        if let Some(gh) = &self.group_header {
            gh.encode(w);
        }

        // PayloadHeader (DataSetMessage payload type): count + writer ids.
        if self.payload_header {
            let count =
                u8::try_from(self.messages.len()).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "NetworkMessage carries more than 255 DataSetMessages",
                })?;
            w.write_u8(count);
            for m in &self.messages {
                w.write_u16(m.writer_id);
            }
        }

        // Extended NetworkMessage header.
        if let Some(ts) = self.timestamp {
            w.write_i64(ts);
        }
        if let Some(pico) = self.pico_seconds {
            w.write_u16(pico);
        }
        if promoted {
            let mut tmp = UaWriter::new();
            for v in &self.promoted_fields {
                v.encode(&mut tmp)?;
            }
            w.write_u16(len_u16("PromotedFields size", tmp.len())?);
            w.write_bytes(tmp.as_slice());
        }
        Ok(())
    }

    /// Encodes the payload: the optional per-message size array (present only
    /// when the PayloadHeader carries more than one DataSetMessage) followed
    /// by the DataSetMessages.
    pub(crate) fn encode_payload(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        if self.payload_header && self.messages.len() > 1 {
            for m in &self.messages {
                w.write_u16(len_u16("DataSetMessage size", m.encoded_len()?)?);
            }
        }
        for m in &self.messages {
            m.encode(w)?;
        }
        Ok(())
    }
}

impl UaEncode for NetworkMessage {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.encode_header(w, false)?;
        self.encode_payload(w)
    }
}

/// The parsed NetworkMessage header, with the reader left at the start of the
/// SecurityHeader (if `security`) or the payload. Used by both the public
/// decode and [`crate::security`].
pub(crate) struct ParsedHeader {
    pub(crate) publisher_id: Option<PublisherId>,
    pub(crate) data_set_class_id: Option<Guid>,
    pub(crate) group_header: Option<GroupHeader>,
    pub(crate) timestamp: Option<i64>,
    pub(crate) pico_seconds: Option<u16>,
    pub(crate) promoted_fields: Vec<Variant>,
    pub(crate) payload_header: bool,
    pub(crate) message_count: u8,
    pub(crate) writer_ids: Vec<u16>,
    pub(crate) security: bool,
}

/// Decodes the NetworkMessage header up to the SecurityHeader/payload
/// boundary, recording (rather than rejecting) the Security flag.
pub(crate) fn decode_header(r: &mut UaReader<'_>) -> Result<ParsedHeader, DecodeError> {
    let uadp = r.read_u8()?;
    let version = uadp & UF_VERSION_MASK;
    if version != UADP_VERSION {
        return Err(DecodeError::InvalidDiscriminant {
            field: "UADP version",
            value: version as u32,
        });
    }
    let has_publisher = uadp & UF_PUBLISHER_ID != 0;
    let has_group = uadp & UF_GROUP_HEADER != 0;
    let payload_header = uadp & UF_PAYLOAD_HEADER != 0;

    let ext1 = if uadp & UF_EXTENDED_FLAGS1 != 0 {
        r.read_u8()?
    } else {
        0
    };
    let publisher_type = ext1 & EF1_PUBLISHER_ID_TYPE_MASK;
    let has_dataset_class_id = ext1 & EF1_DATASET_CLASS_ID != 0;
    let has_timestamp = ext1 & EF1_TIMESTAMP != 0;
    let has_pico = ext1 & EF1_PICOSECONDS != 0;
    let security = ext1 & EF1_SECURITY != 0;

    let ext2 = if ext1 & EF1_EXTENDED_FLAGS2 != 0 {
        r.read_u8()?
    } else {
        0
    };
    if ext2 & EF2_CHUNK != 0 {
        return Err(DecodeError::MalformedMessage {
            message: "chunked UADP NetworkMessages are not supported",
        });
    }
    let nm_type = (ext2 & EF2_NM_TYPE_MASK) >> EF2_NM_TYPE_SHIFT;
    if nm_type != 0 {
        return Err(DecodeError::MalformedMessage {
            message: "only the DataSetMessage NetworkMessage type is supported (discovery is stage 4)",
        });
    }
    let has_promoted = ext2 & EF2_PROMOTED_FIELDS != 0;

    let publisher_id = if has_publisher {
        Some(PublisherId::decode(r, publisher_type)?)
    } else {
        None
    };
    let data_set_class_id = if has_dataset_class_id {
        Some(Guid::decode(r)?)
    } else {
        None
    };
    let group_header = if has_group {
        Some(GroupHeader::decode(r)?)
    } else {
        None
    };

    let (message_count, writer_ids) = if payload_header {
        let count = r.read_u8()?;
        let mut ids = Vec::with_capacity(count as usize);
        for _ in 0..count {
            ids.push(r.read_u16()?);
        }
        (count, ids)
    } else {
        (0, Vec::new())
    };

    let timestamp = if has_timestamp {
        Some(r.read_i64()?)
    } else {
        None
    };
    let pico_seconds = if has_pico { Some(r.read_u16()?) } else { None };
    let promoted_fields = if has_promoted {
        let size = r.read_u16()? as usize;
        let bytes = r.read_bytes(size)?;
        let mut sub = UaReader::new(bytes);
        let mut fields = Vec::new();
        while !sub.is_empty() {
            fields.push(Variant::decode(&mut sub)?);
        }
        fields
    } else {
        Vec::new()
    };

    Ok(ParsedHeader {
        publisher_id,
        data_set_class_id,
        group_header,
        timestamp,
        pico_seconds,
        promoted_fields,
        payload_header,
        message_count,
        writer_ids,
        security,
    })
}

/// Assembles a NetworkMessage from a parsed header and the (already
/// decrypted) payload bytes. Used by [`crate::security`] after decryption.
#[cfg(feature = "security")]
pub(crate) fn finish_decode(
    h: ParsedHeader,
    payload: &[u8],
) -> Result<NetworkMessage, DecodeError> {
    let mut r = UaReader::new(payload);
    let messages = decode_payload(&mut r, h.payload_header, h.message_count, &h.writer_ids)?;
    Ok(NetworkMessage {
        publisher_id: h.publisher_id,
        data_set_class_id: h.data_set_class_id,
        group_header: h.group_header,
        timestamp: h.timestamp,
        pico_seconds: h.pico_seconds,
        promoted_fields: h.promoted_fields,
        payload_header: h.payload_header,
        messages,
    })
}

impl UaDecode for NetworkMessage {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let h = decode_header(r)?;
        if h.security {
            return Err(DecodeError::MalformedMessage {
                message: "secured NetworkMessage must be processed via security::unprotect",
            });
        }
        let messages = decode_payload(r, h.payload_header, h.message_count, &h.writer_ids)?;
        Ok(Self {
            publisher_id: h.publisher_id,
            data_set_class_id: h.data_set_class_id,
            group_header: h.group_header,
            timestamp: h.timestamp,
            pico_seconds: h.pico_seconds,
            promoted_fields: h.promoted_fields,
            payload_header: h.payload_header,
            messages,
        })
    }
}

/// Decodes the DataSetMessage payload, applying the size array (when more
/// than one message is present) so that a trailing RawData message cannot
/// overrun its neighbours, and stamping each message's `writer_id` from the
/// PayloadHeader.
fn decode_payload(
    r: &mut UaReader<'_>,
    payload_header: bool,
    count: u8,
    writer_ids: &[u16],
) -> Result<Vec<DataSetMessage>, DecodeError> {
    if !payload_header {
        // No PayloadHeader: at most a single message spanning the payload.
        return if r.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(alloc::vec![DataSetMessage::decode(r)?])
        };
    }

    let count = count as usize;
    if count <= 1 {
        let mut messages = Vec::with_capacity(count);
        if count == 1 {
            let mut m = DataSetMessage::decode(r)?;
            m.writer_id = writer_ids[0];
            messages.push(m);
        }
        return Ok(messages);
    }

    // count > 1: a UInt16 size array delimits each message.
    let mut sizes = Vec::with_capacity(count);
    for _ in 0..count {
        sizes.push(r.read_u16()? as usize);
    }
    let mut messages = Vec::with_capacity(count);
    for (i, size) in sizes.iter().enumerate() {
        let bytes = r.read_bytes(*size)?;
        let mut sub = UaReader::new(bytes);
        let mut m = DataSetMessage::decode(&mut sub)?;
        if !sub.is_empty() {
            return Err(DecodeError::MalformedMessage {
                message: "DataSetMessage body shorter than its declared payload size",
            });
        }
        m.writer_id = writer_ids[i];
        messages.push(m);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{from_binary, to_binary};
    use crate::uadp::dataset_message::{DataSetData, DataSetMessageKind};
    use zerodds_opcua_gateway::data_value::VariantValue;

    fn key_frame(writer_id: u16, value: i32) -> DataSetMessage {
        DataSetMessage::key_frame_variant(
            writer_id,
            alloc::vec![Variant::scalar(VariantValue::Int32(value))],
        )
    }

    #[test]
    fn minimal_single_message_no_payload_header() {
        let mut nm = NetworkMessage::with_messages(alloc::vec![key_frame(0, 1)]);
        nm.payload_header = false;
        let bytes = to_binary(&nm).expect("encode");
        // UADPFlags only: version 1, no optional blocks.
        assert_eq!(bytes[0] & UF_VERSION_MASK, UADP_VERSION);
        assert_eq!(bytes[0] & UF_EXTENDED_FLAGS1, 0);
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back, nm);
    }

    #[test]
    fn payload_header_single_message_roundtrip() {
        let nm = NetworkMessage::with_messages(alloc::vec![key_frame(7, 42)]);
        let bytes = to_binary(&nm).expect("encode");
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back, nm);
        assert_eq!(back.messages[0].writer_id, 7);
    }

    #[test]
    fn multiple_messages_use_size_array() {
        let nm = NetworkMessage::with_messages(alloc::vec![
            key_frame(1, 10),
            key_frame(2, 20),
            key_frame(3, 30),
        ]);
        let bytes = to_binary(&nm).expect("encode");
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back, nm);
        let ids: Vec<u16> = back.messages.iter().map(|m| m.writer_id).collect();
        assert_eq!(ids, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn full_header_roundtrip() {
        let nm = NetworkMessage {
            publisher_id: Some(PublisherId::UInt32(0xDEAD_BEEF)),
            data_set_class_id: Some(Guid::from_bytes([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF,
            ])),
            group_header: Some(GroupHeader {
                writer_group_id: Some(0x0102),
                group_version: Some(0x0A0B_0C0D),
                network_message_number: Some(5),
                sequence_number: Some(99),
            }),
            timestamp: Some(132_000_000_000_000_000),
            pico_seconds: Some(250),
            promoted_fields: alloc::vec![Variant::scalar(VariantValue::Int32(7))],
            payload_header: true,
            messages: alloc::vec![key_frame(11, 1), key_frame(22, 2)],
        };
        let bytes = to_binary(&nm).expect("encode");
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back, nm);
    }

    #[test]
    fn publisher_id_all_types_roundtrip() {
        for pid in [
            PublisherId::Byte(0x7F),
            PublisherId::UInt16(0x1234),
            PublisherId::UInt32(0x1234_5678),
            PublisherId::UInt64(0x1234_5678_9ABC_DEF0),
            PublisherId::String(String::from("publisher-A")),
        ] {
            let mut nm = NetworkMessage::with_messages(alloc::vec![key_frame(1, 0)]);
            nm.publisher_id = Some(pid.clone());
            let bytes = to_binary(&nm).expect("encode");
            let back: NetworkMessage = from_binary(&bytes).expect("decode");
            assert_eq!(back.publisher_id, Some(pid));
        }
    }

    #[test]
    fn byte_publisher_id_needs_no_extended_flags() {
        let mut nm = NetworkMessage::with_messages(alloc::vec![key_frame(1, 0)]);
        nm.publisher_id = Some(PublisherId::Byte(9));
        let bytes = to_binary(&nm).expect("encode");
        // Byte PublisherId has type code 0, so no ExtendedFlags1 is emitted.
        assert_eq!(bytes[0] & UF_EXTENDED_FLAGS1, 0);
        assert_eq!(bytes[0] & UF_PUBLISHER_ID, UF_PUBLISHER_ID);
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back.publisher_id, Some(PublisherId::Byte(9)));
    }

    #[test]
    fn raw_data_message_bounded_by_size_array() {
        let raw = DataSetMessage {
            writer_id: 1,
            valid: true,
            kind: DataSetMessageKind::KeyFrame,
            sequence_number: None,
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::Raw(alloc::vec![0xAA, 0xBB, 0xCC]),
        };
        let nm = NetworkMessage::with_messages(alloc::vec![raw, key_frame(2, 5)]);
        let bytes = to_binary(&nm).expect("encode");
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        // The size array must stop the RawData message from swallowing the
        // following KeyFrame.
        assert_eq!(back.messages.len(), 2);
        assert_eq!(
            back.messages[0].data,
            DataSetData::Raw(alloc::vec![0xAA, 0xBB, 0xCC])
        );
        assert_eq!(back.messages[1].writer_id, 2);
    }

    #[test]
    fn multiple_messages_without_payload_header_rejected() {
        let mut nm = NetworkMessage::with_messages(alloc::vec![key_frame(1, 0), key_frame(2, 0)]);
        nm.payload_header = false;
        let err = to_binary(&nm).expect_err("must reject");
        assert_eq!(
            err,
            EncodeError::ValueOutOfRange {
                message: "more than one DataSetMessage requires a PayloadHeader",
            }
        );
    }

    #[test]
    fn security_flag_rejected_on_decode() {
        // UADPFlags: version 1 + ExtendedFlags1; ExtendedFlags1: Security bit.
        let bytes = [UADP_VERSION | UF_EXTENDED_FLAGS1, EF1_SECURITY];
        let err = from_binary::<NetworkMessage>(&bytes).expect_err("must reject");
        assert!(matches!(err, DecodeError::MalformedMessage { .. }));
    }

    #[test]
    fn non_dataset_message_type_rejected_on_decode() {
        // ExtendedFlags2 NetworkMessage type 1 (DiscoveryRequest).
        let bytes = [
            UADP_VERSION | UF_EXTENDED_FLAGS1,
            EF1_EXTENDED_FLAGS2,
            1 << EF2_NM_TYPE_SHIFT,
        ];
        let err = from_binary::<NetworkMessage>(&bytes).expect_err("must reject");
        assert!(matches!(err, DecodeError::MalformedMessage { .. }));
    }

    #[test]
    fn bad_version_rejected_on_decode() {
        let bytes = [0x02]; // version 2
        let err = from_binary::<NetworkMessage>(&bytes).expect_err("must reject");
        assert_eq!(
            err,
            DecodeError::InvalidDiscriminant {
                field: "UADP version",
                value: 2,
            }
        );
    }

    #[test]
    fn empty_payload_header_decodes_to_no_messages() {
        let nm = NetworkMessage::with_messages(Vec::new());
        let bytes = to_binary(&nm).expect("encode");
        let back: NetworkMessage = from_binary(&bytes).expect("decode");
        assert!(back.messages.is_empty());
        assert!(back.payload_header);
    }
}
