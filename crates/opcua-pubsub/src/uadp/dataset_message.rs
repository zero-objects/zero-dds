// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UADP `DataSetMessage` framing (OPC Foundation Part 14 §7.2.2.3).
//!
//! A `DataSetMessage` carries one published DataSet sample. Its header is
//! a pair of flag bytes (`DataSetFlags1` / `DataSetFlags2`) followed by
//! the optional sequence number, timestamp, status and configuration
//! version, then the field data. The field data layout depends on the
//! *field encoding* (Variant / RawData / DataValue) and the *message kind*
//! (KeyFrame / DeltaFrame / Event / KeepAlive).

use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant};

use crate::binary::{UaDecode, UaEncode, UaReader, UaWriter, len_u16};
use crate::error::{DecodeError, EncodeError};

// DataSetFlags1 (Part 14 §7.2.2.3.2).
const DSF1_VALID: u8 = 0x01;
const DSF1_FIELD_ENCODING_MASK: u8 = 0x06; // bits 1-2
const DSF1_FIELD_ENCODING_SHIFT: u8 = 1;
const DSF1_SEQUENCE_NUMBER: u8 = 0x08;
const DSF1_STATUS: u8 = 0x10;
const DSF1_CONFIG_MAJOR: u8 = 0x20;
const DSF1_CONFIG_MINOR: u8 = 0x40;
const DSF1_FLAGS2: u8 = 0x80;

// DataSetFlags2 (Part 14 §7.2.2.3.3).
const DSF2_TYPE_MASK: u8 = 0x0F; // bits 0-3
const DSF2_TIMESTAMP: u8 = 0x10;
const DSF2_PICOSECONDS: u8 = 0x20;

/// Field encoding of a `DataSetMessage` (Part 14 §7.2.2.3.2,
/// `DataSetFlags1` bits 1-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldEncoding {
    /// Each field is a full `Variant` (self-describing).
    Variant,
    /// Fields are concatenated without type tags — requires the
    /// `DataSetMetaData` to interpret.
    RawData,
    /// Each field is a `DataValue` (value + status + timestamps).
    DataValue,
}

impl FieldEncoding {
    const fn to_bits(self) -> u8 {
        match self {
            Self::Variant => 0,
            Self::RawData => 1,
            Self::DataValue => 2,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, DecodeError> {
        match bits {
            0 => Ok(Self::Variant),
            1 => Ok(Self::RawData),
            2 => Ok(Self::DataValue),
            other => Err(DecodeError::InvalidDiscriminant {
                field: "DataSetMessage field encoding",
                value: other as u32,
            }),
        }
    }
}

/// Message kind of a `DataSetMessage` (Part 14 §7.2.2.3.3,
/// `DataSetFlags2` bits 0-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSetMessageKind {
    /// Full snapshot of all DataSet fields.
    KeyFrame,
    /// Only the fields that changed since the last KeyFrame.
    DeltaFrame,
    /// An Event notification.
    Event,
    /// A liveness heartbeat with no field data.
    KeepAlive,
}

impl DataSetMessageKind {
    const fn to_bits(self) -> u8 {
        match self {
            Self::KeyFrame => 0,
            Self::DeltaFrame => 1,
            Self::Event => 2,
            Self::KeepAlive => 3,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, DecodeError> {
        match bits {
            0 => Ok(Self::KeyFrame),
            1 => Ok(Self::DeltaFrame),
            2 => Ok(Self::Event),
            3 => Ok(Self::KeepAlive),
            other => Err(DecodeError::InvalidDiscriminant {
                field: "DataSetMessage kind",
                value: other as u32,
            }),
        }
    }
}

/// The field payload of a `DataSetMessage`. The variant determines both
/// the on-wire field encoding and (for delta vs key) the frame layout.
#[derive(Debug, Clone, PartialEq)]
pub enum DataSetData {
    /// KeyFrame/Event fields, Variant-encoded.
    Variant(Vec<Variant>),
    /// KeyFrame/Event fields, DataValue-encoded.
    DataValue(Vec<DataValue>),
    /// DeltaFrame fields (field index + value), Variant-encoded.
    DeltaVariant(Vec<(u16, Variant)>),
    /// DeltaFrame fields (field index + value), DataValue-encoded.
    DeltaDataValue(Vec<(u16, DataValue)>),
    /// RawData field encoding — opaque bytes (needs DataSetMetaData to
    /// interpret). The reader consumes to the message boundary.
    Raw(Vec<u8>),
    /// No field data (KeepAlive).
    None,
}

impl DataSetData {
    const fn field_encoding(&self) -> FieldEncoding {
        match self {
            Self::Variant(_) | Self::DeltaVariant(_) | Self::None => FieldEncoding::Variant,
            Self::Raw(_) => FieldEncoding::RawData,
            Self::DataValue(_) | Self::DeltaDataValue(_) => FieldEncoding::DataValue,
        }
    }
}

/// A single UADP `DataSetMessage` (Part 14 §7.2.2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetMessage {
    /// DataSetWriter that produced this message (referenced from the
    /// NetworkMessage payload header).
    pub writer_id: u16,
    /// `DataSetFlags1` bit 0 — whether the message content is valid.
    pub valid: bool,
    /// Message kind (KeyFrame / DeltaFrame / Event / KeepAlive).
    pub kind: DataSetMessageKind,
    /// Optional per-message sequence number.
    pub sequence_number: Option<u16>,
    /// Optional source timestamp (DateTime ticks).
    pub timestamp: Option<i64>,
    /// Optional picoseconds refinement of the timestamp.
    pub pico_seconds: Option<u16>,
    /// Optional 16-bit status (high word of an OPC-UA StatusCode).
    pub status: Option<u16>,
    /// Optional configuration version (major).
    pub config_major_version: Option<u32>,
    /// Optional configuration version (minor).
    pub config_minor_version: Option<u32>,
    /// Field payload.
    pub data: DataSetData,
}

impl DataSetMessage {
    /// Creates a valid KeyFrame carrying Variant-encoded fields.
    #[must_use]
    pub fn key_frame_variant(writer_id: u16, fields: Vec<Variant>) -> Self {
        Self {
            writer_id,
            valid: true,
            kind: DataSetMessageKind::KeyFrame,
            sequence_number: None,
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::Variant(fields),
        }
    }

    /// Encoded byte length of this message (used to fill the
    /// NetworkMessage payload size array without a second allocation
    /// pass at the call site).
    pub(crate) fn encoded_len(&self) -> Result<usize, EncodeError> {
        let mut w = UaWriter::new();
        self.encode(&mut w)?;
        Ok(w.len())
    }
}

impl UaEncode for DataSetMessage {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        let field_encoding = self.data.field_encoding();
        let flags2_needed = self.kind != DataSetMessageKind::KeyFrame
            || self.timestamp.is_some()
            || self.pico_seconds.is_some();

        let mut flags1 = 0u8;
        if self.valid {
            flags1 |= DSF1_VALID;
        }
        flags1 |=
            (field_encoding.to_bits() << DSF1_FIELD_ENCODING_SHIFT) & DSF1_FIELD_ENCODING_MASK;
        if self.sequence_number.is_some() {
            flags1 |= DSF1_SEQUENCE_NUMBER;
        }
        if self.status.is_some() {
            flags1 |= DSF1_STATUS;
        }
        if self.config_major_version.is_some() {
            flags1 |= DSF1_CONFIG_MAJOR;
        }
        if self.config_minor_version.is_some() {
            flags1 |= DSF1_CONFIG_MINOR;
        }
        if flags2_needed {
            flags1 |= DSF1_FLAGS2;
        }
        w.write_u8(flags1);

        if flags2_needed {
            let mut flags2 = self.kind.to_bits() & DSF2_TYPE_MASK;
            if self.timestamp.is_some() {
                flags2 |= DSF2_TIMESTAMP;
            }
            if self.pico_seconds.is_some() {
                flags2 |= DSF2_PICOSECONDS;
            }
            w.write_u8(flags2);
        }

        // Header fields, in spec order (§7.2.2.3.4).
        if let Some(sn) = self.sequence_number {
            w.write_u16(sn);
        }
        if let Some(ts) = self.timestamp {
            w.write_i64(ts);
        }
        if let Some(pico) = self.pico_seconds {
            w.write_u16(pico);
        }
        if let Some(status) = self.status {
            w.write_u16(status);
        }
        if let Some(major) = self.config_major_version {
            w.write_u32(major);
        }
        if let Some(minor) = self.config_minor_version {
            w.write_u32(minor);
        }

        // Field data.
        match &self.data {
            DataSetData::None => {}
            DataSetData::Variant(fields) => {
                w.write_u16(len_u16("DataSet field count", fields.len())?);
                for f in fields {
                    f.encode(w)?;
                }
            }
            DataSetData::DataValue(fields) => {
                w.write_u16(len_u16("DataSet field count", fields.len())?);
                for f in fields {
                    f.encode(w)?;
                }
            }
            DataSetData::DeltaVariant(fields) => {
                w.write_u16(len_u16("DataSet delta count", fields.len())?);
                for (idx, f) in fields {
                    w.write_u16(*idx);
                    f.encode(w)?;
                }
            }
            DataSetData::DeltaDataValue(fields) => {
                w.write_u16(len_u16("DataSet delta count", fields.len())?);
                for (idx, f) in fields {
                    w.write_u16(*idx);
                    f.encode(w)?;
                }
            }
            DataSetData::Raw(bytes) => {
                w.write_bytes(bytes);
            }
        }
        Ok(())
    }
}

impl UaDecode for DataSetMessage {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let flags1 = r.read_u8()?;
        let valid = flags1 & DSF1_VALID != 0;
        let field_encoding = FieldEncoding::from_bits(
            (flags1 & DSF1_FIELD_ENCODING_MASK) >> DSF1_FIELD_ENCODING_SHIFT,
        )?;

        let (kind, has_timestamp, has_pico) = if flags1 & DSF1_FLAGS2 != 0 {
            let flags2 = r.read_u8()?;
            (
                DataSetMessageKind::from_bits(flags2 & DSF2_TYPE_MASK)?,
                flags2 & DSF2_TIMESTAMP != 0,
                flags2 & DSF2_PICOSECONDS != 0,
            )
        } else {
            (DataSetMessageKind::KeyFrame, false, false)
        };

        let sequence_number = if flags1 & DSF1_SEQUENCE_NUMBER != 0 {
            Some(r.read_u16()?)
        } else {
            None
        };
        let timestamp = if has_timestamp {
            Some(r.read_i64()?)
        } else {
            None
        };
        let pico_seconds = if has_pico { Some(r.read_u16()?) } else { None };
        let status = if flags1 & DSF1_STATUS != 0 {
            Some(r.read_u16()?)
        } else {
            None
        };
        let config_major_version = if flags1 & DSF1_CONFIG_MAJOR != 0 {
            Some(r.read_u32()?)
        } else {
            None
        };
        let config_minor_version = if flags1 & DSF1_CONFIG_MINOR != 0 {
            Some(r.read_u32()?)
        } else {
            None
        };

        let data = decode_field_data(r, kind, field_encoding)?;

        Ok(DataSetMessage {
            writer_id: 0, // assigned by the NetworkMessage payload header
            valid,
            kind,
            sequence_number,
            timestamp,
            pico_seconds,
            status,
            config_major_version,
            config_minor_version,
            data,
        })
    }
}

fn decode_field_data(
    r: &mut UaReader<'_>,
    kind: DataSetMessageKind,
    field_encoding: FieldEncoding,
) -> Result<DataSetData, DecodeError> {
    if kind == DataSetMessageKind::KeepAlive {
        return Ok(DataSetData::None);
    }
    match field_encoding {
        FieldEncoding::RawData => {
            // RawData has no field count; it spans the remainder of the
            // (already size-bounded) message.
            let rest = r.read_bytes(r.remaining())?;
            Ok(DataSetData::Raw(rest.to_vec()))
        }
        FieldEncoding::Variant => {
            let count = r.read_u16()?;
            if kind == DataSetMessageKind::DeltaFrame {
                let mut fields = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let idx = r.read_u16()?;
                    fields.push((idx, Variant::decode(r)?));
                }
                Ok(DataSetData::DeltaVariant(fields))
            } else {
                let mut fields = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    fields.push(Variant::decode(r)?);
                }
                Ok(DataSetData::Variant(fields))
            }
        }
        FieldEncoding::DataValue => {
            let count = r.read_u16()?;
            if kind == DataSetMessageKind::DeltaFrame {
                let mut fields = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let idx = r.read_u16()?;
                    fields.push((idx, DataValue::decode(r)?));
                }
                Ok(DataSetData::DeltaDataValue(fields))
            } else {
                let mut fields = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    fields.push(DataValue::decode(r)?);
                }
                Ok(DataSetData::DataValue(fields))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{from_binary, to_binary};
    use zerodds_opcua_gateway::data_value::VariantValue;

    #[test]
    fn key_frame_variant_roundtrip() {
        let msg = DataSetMessage::key_frame_variant(
            7,
            alloc::vec![
                Variant::scalar(VariantValue::Int32(42)),
                Variant::scalar(VariantValue::Boolean(true)),
            ],
        );
        let bytes = to_binary(&msg).expect("encode");
        let back: DataSetMessage = from_binary(&bytes).expect("decode");
        // writer_id is assigned by the NetworkMessage layer, not the
        // DataSetMessage body — compare the rest.
        assert_eq!(back.data, msg.data);
        assert!(back.valid);
        assert_eq!(back.kind, DataSetMessageKind::KeyFrame);
    }

    #[test]
    fn key_frame_with_header_fields() {
        let msg = DataSetMessage {
            writer_id: 1,
            valid: true,
            kind: DataSetMessageKind::KeyFrame,
            sequence_number: Some(99),
            timestamp: Some(132_000_000_000_000_000),
            pico_seconds: Some(5),
            status: Some(0x8000),
            config_major_version: Some(2),
            config_minor_version: Some(7),
            data: DataSetData::DataValue(alloc::vec![DataValue::new_value(
                Variant::scalar(VariantValue::Double(1.5)),
                0,
                0,
            )]),
        };
        let bytes = to_binary(&msg).expect("encode");
        let mut back: DataSetMessage = from_binary(&bytes).expect("decode");
        back.writer_id = 1;
        assert_eq!(back, msg);
    }

    #[test]
    fn delta_frame_variant_roundtrip() {
        let msg = DataSetMessage {
            writer_id: 0,
            valid: true,
            kind: DataSetMessageKind::DeltaFrame,
            sequence_number: None,
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::DeltaVariant(alloc::vec![
                (2, Variant::scalar(VariantValue::Int32(10))),
                (5, Variant::scalar(VariantValue::Int32(20))),
            ]),
        };
        let bytes = to_binary(&msg).expect("encode");
        let back: DataSetMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back.data, msg.data);
        assert_eq!(back.kind, DataSetMessageKind::DeltaFrame);
    }

    #[test]
    fn keep_alive_has_no_data() {
        let msg = DataSetMessage {
            writer_id: 0,
            valid: true,
            kind: DataSetMessageKind::KeepAlive,
            sequence_number: Some(3),
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::None,
        };
        let bytes = to_binary(&msg).expect("encode");
        let back: DataSetMessage = from_binary(&bytes).expect("decode");
        assert_eq!(back.kind, DataSetMessageKind::KeepAlive);
        assert_eq!(back.data, DataSetData::None);
        assert_eq!(back.sequence_number, Some(3));
    }

    #[test]
    fn raw_data_consumes_remainder() {
        let msg = DataSetMessage {
            writer_id: 0,
            valid: true,
            kind: DataSetMessageKind::KeyFrame,
            sequence_number: None,
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::Raw(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let bytes = to_binary(&msg).expect("encode");
        let back: DataSetMessage = from_binary(&bytes).expect("decode");
        assert_eq!(
            back.data,
            DataSetData::Raw(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF])
        );
    }
}
