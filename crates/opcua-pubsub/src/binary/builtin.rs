// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA binary encoders/decoders for the built-in structured types
//! (Part 6 §5.2.2.9 – §5.2.2.17), operating on the `zerodds-opcua-gateway`
//! type model.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{
    DataValue, ExtensionObject, ExtensionObjectBody, Variant, VariantValue,
};
use zerodds_opcua_gateway::node_id::{ExpandedNodeId, NodeId, NodeIdentifier};
use zerodds_opcua_gateway::types::{BuiltinTypeKind, Guid, LocalizedText, QualifiedName};

use super::{
    UaDecode, UaEncode, UaReader, UaWriter, check_array_len, len_i32, read_byte_string,
    read_string, write_byte_string, write_string,
};
use crate::error::{DecodeError, EncodeError};

// ---------------------------------------------------------------------------
// Guid (Part 6 §5.1.5) — Data1 UInt32 LE, Data2/Data3 UInt16 LE, Data4 raw.
// ---------------------------------------------------------------------------

fn encode_guid(w: &mut UaWriter, g: &Guid) {
    w.write_u32(g.data1);
    w.write_u16(g.data2);
    w.write_u16(g.data3);
    w.write_bytes(&g.data4);
}

fn decode_guid(r: &mut UaReader<'_>) -> Result<Guid, DecodeError> {
    let data1 = r.read_u32()?;
    let data2 = r.read_u16()?;
    let data3 = r.read_u16()?;
    let mut data4 = [0u8; 8];
    data4.copy_from_slice(r.read_bytes(8)?);
    Ok(Guid {
        data1,
        data2,
        data3,
        data4,
    })
}

impl UaEncode for Guid {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        encode_guid(w, self);
        Ok(())
    }
}

impl UaDecode for Guid {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        decode_guid(r)
    }
}

// ---------------------------------------------------------------------------
// NodeId (Part 6 §5.2.2.9) — encoding byte selects the most compact form.
// ---------------------------------------------------------------------------

const NODEID_TWO_BYTE: u8 = 0x00;
const NODEID_FOUR_BYTE: u8 = 0x01;
const NODEID_NUMERIC: u8 = 0x02;
const NODEID_STRING: u8 = 0x03;
const NODEID_GUID: u8 = 0x04;
const NODEID_BYTESTRING: u8 = 0x05;

impl UaEncode for NodeId {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        match &self.identifier_type {
            NodeIdentifier::Numeric(id) => {
                if self.namespace_index == 0 && *id <= u32::from(u8::MAX) {
                    w.write_u8(NODEID_TWO_BYTE);
                    w.write_u8(*id as u8);
                } else if self.namespace_index <= u16::from(u8::MAX) && *id <= u32::from(u16::MAX) {
                    w.write_u8(NODEID_FOUR_BYTE);
                    w.write_u8(self.namespace_index as u8);
                    w.write_u16(*id as u16);
                } else {
                    w.write_u8(NODEID_NUMERIC);
                    w.write_u16(self.namespace_index);
                    w.write_u32(*id);
                }
            }
            NodeIdentifier::String(s) => {
                w.write_u8(NODEID_STRING);
                w.write_u16(self.namespace_index);
                write_string(w, s)?;
            }
            NodeIdentifier::Guid(g) => {
                w.write_u8(NODEID_GUID);
                w.write_u16(self.namespace_index);
                encode_guid(w, g);
            }
            NodeIdentifier::Opaque(b) => {
                w.write_u8(NODEID_BYTESTRING);
                w.write_u16(self.namespace_index);
                write_byte_string(w, b)?;
            }
        }
        Ok(())
    }
}

/// Decodes a NodeId body given an already-read encoding byte — shared by
/// [`NodeId`] and [`ExpandedNodeId`] (which carry the type in the same
/// byte alongside their own flag bits).
fn decode_node_id_with_enc(r: &mut UaReader<'_>, enc: u8) -> Result<NodeId, DecodeError> {
    let node = match enc & 0x0F {
        NODEID_TWO_BYTE => NodeId::numeric(0, u32::from(r.read_u8()?)),
        NODEID_FOUR_BYTE => {
            let ns = u16::from(r.read_u8()?);
            let id = u32::from(r.read_u16()?);
            NodeId::numeric(ns, id)
        }
        NODEID_NUMERIC => {
            let ns = r.read_u16()?;
            let id = r.read_u32()?;
            NodeId::numeric(ns, id)
        }
        NODEID_STRING => {
            let ns = r.read_u16()?;
            NodeId {
                namespace_index: ns,
                identifier_type: NodeIdentifier::String(read_string(r)?),
            }
        }
        NODEID_GUID => {
            let ns = r.read_u16()?;
            NodeId {
                namespace_index: ns,
                identifier_type: NodeIdentifier::Guid(decode_guid(r)?),
            }
        }
        NODEID_BYTESTRING => {
            let ns = r.read_u16()?;
            NodeId {
                namespace_index: ns,
                identifier_type: NodeIdentifier::Opaque(read_byte_string(r)?),
            }
        }
        other => {
            return Err(DecodeError::InvalidDiscriminant {
                field: "NodeId encoding",
                value: u32::from(other),
            });
        }
    };
    Ok(node)
}

impl UaDecode for NodeId {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let enc = r.read_u8()?;
        decode_node_id_with_enc(r, enc)
    }
}

// ---------------------------------------------------------------------------
// ExpandedNodeId (Part 6 §5.2.2.10) — NodeId + optional URI + ServerIndex.
// ---------------------------------------------------------------------------

const EXPNODEID_URI_FLAG: u8 = 0x80;
const EXPNODEID_SERVER_FLAG: u8 = 0x40;

impl UaEncode for ExpandedNodeId {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        let uri_present = !self.namespace_uri.is_empty();
        let server_present = self.server_index != 0;

        // Encode the inner NodeId, then OR the ExpandedNodeId flag bits
        // into its leading encoding byte (Part 6 §5.2.2.10).
        let mut inner = UaWriter::new();
        self.node_id.encode(&mut inner)?;
        let mut bytes = inner.into_vec();
        if let Some(first) = bytes.first_mut() {
            if uri_present {
                *first |= EXPNODEID_URI_FLAG;
            }
            if server_present {
                *first |= EXPNODEID_SERVER_FLAG;
            }
        }
        w.write_bytes(&bytes);

        if uri_present {
            write_string(w, &self.namespace_uri)?;
        }
        if server_present {
            w.write_u32(self.server_index);
        }
        Ok(())
    }
}

impl UaDecode for ExpandedNodeId {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let enc = r.read_u8()?;
        let node_id = decode_node_id_with_enc(r, enc)?;
        let namespace_uri = if enc & EXPNODEID_URI_FLAG != 0 {
            read_string(r)?
        } else {
            String::new()
        };
        let server_index = if enc & EXPNODEID_SERVER_FLAG != 0 {
            r.read_u32()?
        } else {
            0
        };
        Ok(ExpandedNodeId {
            node_id,
            namespace_uri,
            server_index,
        })
    }
}

// ---------------------------------------------------------------------------
// QualifiedName (Part 6 §5.2.2.13) + LocalizedText (Part 6 §5.2.2.14).
// ---------------------------------------------------------------------------

impl UaEncode for QualifiedName {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u16(self.namespace_index);
        write_string(w, &self.name)
    }
}

impl UaDecode for QualifiedName {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let namespace_index = r.read_u16()?;
        let name = read_string(r)?;
        Ok(QualifiedName {
            namespace_index,
            name,
        })
    }
}

const LOCALIZEDTEXT_LOCALE_FLAG: u8 = 0x01;
const LOCALIZEDTEXT_TEXT_FLAG: u8 = 0x02;

impl UaEncode for LocalizedText {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        let mut mask = 0u8;
        if self.locale.is_some() {
            mask |= LOCALIZEDTEXT_LOCALE_FLAG;
        }
        if self.text.is_some() {
            mask |= LOCALIZEDTEXT_TEXT_FLAG;
        }
        w.write_u8(mask);
        if let Some(locale) = &self.locale {
            write_string(w, locale)?;
        }
        if let Some(text) = &self.text {
            write_string(w, text)?;
        }
        Ok(())
    }
}

impl UaDecode for LocalizedText {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let mask = r.read_u8()?;
        let locale = if mask & LOCALIZEDTEXT_LOCALE_FLAG != 0 {
            Some(read_string(r)?)
        } else {
            None
        };
        let text = if mask & LOCALIZEDTEXT_TEXT_FLAG != 0 {
            Some(read_string(r)?)
        } else {
            None
        };
        Ok(LocalizedText { locale, text })
    }
}

// ---------------------------------------------------------------------------
// ExtensionObject (Part 6 §5.2.2.15).
// ---------------------------------------------------------------------------

const EXTOBJ_BODY_NONE: u8 = 0x00;
const EXTOBJ_BODY_BYTESTRING: u8 = 0x01;
const EXTOBJ_BODY_XML: u8 = 0x02;

impl UaEncode for ExtensionObject {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.type_id.encode(w)?;
        match &self.body {
            ExtensionObjectBody::None => w.write_u8(EXTOBJ_BODY_NONE),
            ExtensionObjectBody::ByteString(b) => {
                w.write_u8(EXTOBJ_BODY_BYTESTRING);
                write_byte_string(w, b)?;
            }
            ExtensionObjectBody::XmlElement(s) => {
                w.write_u8(EXTOBJ_BODY_XML);
                write_byte_string(w, s.as_bytes())?;
            }
        }
        Ok(())
    }
}

impl UaDecode for ExtensionObject {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let type_id = NodeId::decode(r)?;
        let enc = r.read_u8()?;
        let body = match enc {
            EXTOBJ_BODY_NONE => ExtensionObjectBody::None,
            EXTOBJ_BODY_BYTESTRING => ExtensionObjectBody::ByteString(read_byte_string(r)?),
            EXTOBJ_BODY_XML => {
                let bytes = read_byte_string(r)?;
                let s = String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                ExtensionObjectBody::XmlElement(s)
            }
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "ExtensionObject encoding",
                    value: u32::from(other),
                });
            }
        };
        Ok(ExtensionObject { type_id, body })
    }
}

// ---------------------------------------------------------------------------
// VariantValue — a single scalar element of a Variant. The discriminant
// (built-in type id) lives in the Variant encoding byte, so VariantValue
// itself only encodes the bare value; decoding requires the type id.
// ---------------------------------------------------------------------------

impl UaEncode for VariantValue {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        match self {
            Self::Boolean(b) => w.write_u8(u8::from(*b)),
            Self::SByte(v) => w.write_u8(*v as u8),
            Self::Byte(v) => w.write_u8(*v),
            Self::Int16(v) => w.write_i16(*v),
            Self::UInt16(v) => w.write_u16(*v),
            Self::Int32(v) => w.write_i32(*v),
            Self::UInt32(v) => w.write_u32(*v),
            Self::Int64(v) => w.write_i64(*v),
            Self::UInt64(v) => w.write_u64(*v),
            Self::Float(v) => w.write_f32(*v),
            Self::Double(v) => w.write_f64(*v),
            Self::String(s) => write_string(w, s)?,
            Self::DateTime(t) => w.write_i64(*t),
            Self::Guid(g) => encode_guid(w, g),
            Self::ByteString(b) => write_byte_string(w, b)?,
            Self::XmlElement(s) => write_byte_string(w, s.as_bytes())?,
            Self::NodeId(n) => n.encode(w)?,
            Self::StatusCode(c) => w.write_u32(*c),
            Self::QualifiedName(q) => q.encode(w)?,
            Self::LocalizedText(l) => l.encode(w)?,
            Self::ExtensionObject(e) => e.encode(w)?,
        }
        Ok(())
    }
}

/// Decodes a single Variant element given its built-in type id (1–22).
///
/// Type ids 18 (ExpandedNodeId), 23 (DataValue), 24 (Variant) and 25
/// (DiagnosticInfo) are not representable in the gateway `VariantValue`
/// model and yield [`DecodeError::InvalidDiscriminant`].
fn decode_variant_value(r: &mut UaReader<'_>, type_id: u8) -> Result<VariantValue, DecodeError> {
    let value = match type_id {
        1 => VariantValue::Boolean(r.read_u8()? != 0),
        2 => VariantValue::SByte(r.read_u8()? as i8),
        3 => VariantValue::Byte(r.read_u8()?),
        4 => VariantValue::Int16(r.read_i16()?),
        5 => VariantValue::UInt16(r.read_u16()?),
        6 => VariantValue::Int32(r.read_i32()?),
        7 => VariantValue::UInt32(r.read_u32()?),
        8 => VariantValue::Int64(r.read_i64()?),
        9 => VariantValue::UInt64(r.read_u64()?),
        10 => VariantValue::Float(r.read_f32()?),
        11 => VariantValue::Double(r.read_f64()?),
        12 => VariantValue::String(read_string(r)?),
        13 => VariantValue::DateTime(r.read_i64()?),
        14 => VariantValue::Guid(decode_guid(r)?),
        15 => VariantValue::ByteString(read_byte_string(r)?),
        16 => {
            let bytes = read_byte_string(r)?;
            VariantValue::XmlElement(
                String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?,
            )
        }
        17 => VariantValue::NodeId(NodeId::decode(r)?),
        19 => VariantValue::StatusCode(r.read_u32()?),
        20 => VariantValue::QualifiedName(QualifiedName::decode(r)?),
        21 => VariantValue::LocalizedText(LocalizedText::decode(r)?),
        22 => VariantValue::ExtensionObject(ExtensionObject::decode(r)?),
        other => {
            return Err(DecodeError::InvalidDiscriminant {
                field: "Variant element type",
                value: u32::from(other),
            });
        }
    };
    Ok(value)
}

/// Decodes one bare value of the given built-in type. Used by the RawData
/// DataSetMessage decoder ([`crate::reader`]), which carries no per-field
/// type tags and instead relies on the DataSetMetaData built-in types.
pub(crate) fn decode_builtin_value(
    r: &mut UaReader<'_>,
    kind: BuiltinTypeKind,
) -> Result<VariantValue, DecodeError> {
    decode_variant_value(r, kind.value())
}

/// Maps a `BuiltInType` byte (Part 6 §5.1.2 enum, 1–25) back to a
/// [`BuiltinTypeKind`].
pub(crate) fn builtin_type_from_value(v: u8) -> Result<BuiltinTypeKind, DecodeError> {
    use BuiltinTypeKind::{
        Boolean, Byte, ByteString, DataValue as DataValueKind, DateTime, DiagnosticInfo, Double,
        ExpandedNodeId, ExtensionObject, Float, Guid as GuidKind, Int16, Int32, Int64,
        LocalizedText as LocalizedTextKind, NodeId as NodeIdKind,
        QualifiedName as QualifiedNameKind, SByte, StatusCode as StatusCodeKind,
        String as StringKind, UInt16, UInt32, UInt64, Variant as VariantKind, XmlElement,
    };
    Ok(match v {
        1 => Boolean,
        2 => SByte,
        3 => Byte,
        4 => Int16,
        5 => UInt16,
        6 => Int32,
        7 => UInt32,
        8 => Int64,
        9 => UInt64,
        10 => Float,
        11 => Double,
        12 => StringKind,
        13 => DateTime,
        14 => GuidKind,
        15 => ByteString,
        16 => XmlElement,
        17 => NodeIdKind,
        18 => ExpandedNodeId,
        19 => StatusCodeKind,
        20 => QualifiedNameKind,
        21 => LocalizedTextKind,
        22 => ExtensionObject,
        23 => DataValueKind,
        24 => VariantKind,
        25 => DiagnosticInfo,
        other => {
            return Err(DecodeError::InvalidDiscriminant {
                field: "BuiltInType",
                value: other as u32,
            });
        }
    })
}

// ---------------------------------------------------------------------------
// Variant (Part 6 §5.2.2.16).
// ---------------------------------------------------------------------------

const VARIANT_TYPE_MASK: u8 = 0x3F;
const VARIANT_DIMENSIONS_FLAG: u8 = 0x40;
const VARIANT_ARRAY_FLAG: u8 = 0x80;

impl UaEncode for Variant {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        if self.value.is_empty() {
            // Null Variant — encoding byte 0 (Part 6 §5.2.2.16).
            w.write_u8(0);
            return Ok(());
        }
        let kind = self.type_kind().ok_or(EncodeError::ValueOutOfRange {
            message: "Variant has mixed element types",
        })?;
        let type_id = kind.value();
        let is_scalar = self.array_dimensions.is_empty() && self.value.len() == 1;
        let has_dims = self.array_dimensions.len() > 1;

        let mut enc = type_id & VARIANT_TYPE_MASK;
        if !is_scalar {
            enc |= VARIANT_ARRAY_FLAG;
        }
        if has_dims {
            enc |= VARIANT_DIMENSIONS_FLAG;
        }
        w.write_u8(enc);

        if is_scalar {
            if let Some(v) = self.value.first() {
                v.encode(w)?;
            }
        } else {
            w.write_i32(len_i32("Variant array", self.value.len())?);
            for v in &self.value {
                v.encode(w)?;
            }
        }

        if has_dims {
            w.write_i32(len_i32("ArrayDimensions", self.array_dimensions.len())?);
            for d in &self.array_dimensions {
                w.write_i32(len_i32("ArrayDimension", *d as usize)?);
            }
        }
        Ok(())
    }
}

impl UaDecode for Variant {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let enc = r.read_u8()?;
        let type_id = enc & VARIANT_TYPE_MASK;
        if type_id == 0 {
            return Ok(Variant {
                array_dimensions: Vec::new(),
                value: Vec::new(),
            });
        }
        let is_array = enc & VARIANT_ARRAY_FLAG != 0;
        let has_dims = enc & VARIANT_DIMENSIONS_FLAG != 0;

        let mut value = Vec::new();
        if is_array {
            let len = r.read_i32()?;
            if len < 0 {
                return Err(DecodeError::NegativeLength {
                    field: "Variant array",
                });
            }
            let len = len as usize;
            // Conservative bound: every value needs >= 1 wire byte, so a
            // count above the remaining bytes cannot be genuine. Reject
            // before `reserve` (see `check_array_len`).
            check_array_len(r, len, 1, "Variant array length exceeds remaining bytes")?;
            value.reserve(len);
            for _ in 0..len {
                value.push(decode_variant_value(r, type_id)?);
            }
        } else {
            value.push(decode_variant_value(r, type_id)?);
        }

        let array_dimensions = if has_dims {
            let n = r.read_i32()?;
            if n < 0 {
                return Err(DecodeError::NegativeLength {
                    field: "ArrayDimensions",
                });
            }
            let n = n as usize;
            check_array_len(r, n, 4, "ArrayDimensions length exceeds remaining bytes")?;
            let mut dims = Vec::with_capacity(n);
            for _ in 0..n {
                dims.push(r.read_i32()? as u32);
            }
            dims
        } else if is_array {
            // 1-D arrays carry no explicit dimensions field on the wire;
            // record the length so a re-encode is byte-symmetric.
            alloc::vec![value.len() as u32]
        } else {
            Vec::new()
        };

        Ok(Variant {
            array_dimensions,
            value,
        })
    }
}

// ---------------------------------------------------------------------------
// DataValue (Part 6 §5.2.2.17).
// ---------------------------------------------------------------------------

const DV_VALUE_FLAG: u8 = 0x01;
const DV_STATUS_FLAG: u8 = 0x02;
const DV_SOURCE_TS_FLAG: u8 = 0x04;
const DV_SERVER_TS_FLAG: u8 = 0x08;
const DV_SOURCE_PICO_FLAG: u8 = 0x10;
const DV_SERVER_PICO_FLAG: u8 = 0x20;

impl UaEncode for DataValue {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        let mut mask = 0u8;
        if self.value.is_some() {
            mask |= DV_VALUE_FLAG;
        }
        if self.status.is_some() {
            mask |= DV_STATUS_FLAG;
        }
        if self.source_timestamp.is_some() {
            mask |= DV_SOURCE_TS_FLAG;
        }
        if self.server_timestamp.is_some() {
            mask |= DV_SERVER_TS_FLAG;
        }
        if self.source_pico_sec.is_some() {
            mask |= DV_SOURCE_PICO_FLAG;
        }
        if self.server_pico_sec.is_some() {
            mask |= DV_SERVER_PICO_FLAG;
        }
        w.write_u8(mask);

        if let Some(value) = &self.value {
            value.encode(w)?;
        }
        if let Some(status) = self.status {
            w.write_u32(status);
        }
        if let Some(ts) = self.source_timestamp {
            w.write_i64(ts);
        }
        if let Some(ts) = self.server_timestamp {
            w.write_i64(ts);
        }
        if let Some(pico) = self.source_pico_sec {
            w.write_u16(pico);
        }
        if let Some(pico) = self.server_pico_sec {
            w.write_u16(pico);
        }
        Ok(())
    }
}

impl UaDecode for DataValue {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let mask = r.read_u8()?;
        let value = if mask & DV_VALUE_FLAG != 0 {
            Some(Variant::decode(r)?)
        } else {
            None
        };
        let status = if mask & DV_STATUS_FLAG != 0 {
            Some(r.read_u32()?)
        } else {
            None
        };
        let source_timestamp = if mask & DV_SOURCE_TS_FLAG != 0 {
            Some(r.read_i64()?)
        } else {
            None
        };
        let server_timestamp = if mask & DV_SERVER_TS_FLAG != 0 {
            Some(r.read_i64()?)
        } else {
            None
        };
        let source_pico_sec = if mask & DV_SOURCE_PICO_FLAG != 0 {
            Some(r.read_u16()?)
        } else {
            None
        };
        let server_pico_sec = if mask & DV_SERVER_PICO_FLAG != 0 {
            Some(r.read_u16()?)
        } else {
            None
        };
        Ok(DataValue {
            value,
            status,
            source_timestamp,
            server_timestamp,
            source_pico_sec,
            server_pico_sec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{from_binary, to_binary};
    use zerodds_opcua_gateway::types::BuiltinTypeKind;

    fn roundtrip<T: UaEncode + UaDecode + PartialEq + core::fmt::Debug>(v: &T) -> Vec<u8> {
        let bytes = to_binary(v).expect("encode");
        let back: T = from_binary(&bytes).expect("decode");
        assert_eq!(&back, v, "roundtrip mismatch");
        bytes
    }

    #[test]
    fn nodeid_two_byte_compact_form() {
        let n = NodeId::numeric(0, 42);
        let bytes = roundtrip(&n);
        // Part 6 §5.2.2.9 — TwoByte form: [0x00][id].
        assert_eq!(bytes, alloc::vec![NODEID_TWO_BYTE, 42]);
    }

    #[test]
    fn nodeid_four_byte_form() {
        let n = NodeId::numeric(5, 1000);
        let bytes = roundtrip(&n);
        assert_eq!(bytes.first().copied(), Some(NODEID_FOUR_BYTE));
        assert_eq!(bytes.len(), 4); // enc + ns(u8) + id(u16)
    }

    #[test]
    fn nodeid_numeric_full_form() {
        let n = NodeId::numeric(300, 70_000);
        let bytes = roundtrip(&n);
        assert_eq!(bytes.first().copied(), Some(NODEID_NUMERIC));
        assert_eq!(bytes.len(), 7); // enc + ns(u16) + id(u32)
    }

    #[test]
    fn nodeid_string_form() {
        let n = NodeId {
            namespace_index: 2,
            identifier_type: NodeIdentifier::String(String::from("Temperature")),
        };
        let bytes = roundtrip(&n);
        assert_eq!(bytes.first().copied(), Some(NODEID_STRING));
    }

    #[test]
    fn guid_encodes_data1_little_endian() {
        let g = Guid {
            data1: 0x0102_0304,
            data2: 0x0506,
            data3: 0x0708,
            data4: [9, 10, 11, 12, 13, 14, 15, 16],
        };
        let bytes = roundtrip(&g);
        // Data1 little-endian per Part 6 §5.1.5.
        assert_eq!(&bytes[0..4], &[0x04, 0x03, 0x02, 0x01]);
        assert_eq!(bytes.len(), 16);
    }

    #[test]
    fn expanded_node_id_with_uri_and_server() {
        let e = ExpandedNodeId {
            node_id: NodeId::numeric(0, 7),
            namespace_uri: String::from("urn:example"),
            server_index: 3,
        };
        let bytes = roundtrip(&e);
        // First byte carries both flags.
        assert_ne!(bytes.first().copied().unwrap_or(0) & EXPNODEID_URI_FLAG, 0);
        assert_ne!(
            bytes.first().copied().unwrap_or(0) & EXPNODEID_SERVER_FLAG,
            0
        );
    }

    #[test]
    fn expanded_node_id_bare() {
        let e = ExpandedNodeId {
            node_id: NodeId::numeric(0, 7),
            namespace_uri: String::new(),
            server_index: 0,
        };
        let bytes = roundtrip(&e);
        assert_eq!(bytes.first().copied().unwrap_or(0xFF) & 0xC0, 0);
    }

    #[test]
    fn qualified_name_roundtrip() {
        let q = QualifiedName {
            namespace_index: 1,
            name: String::from("Sensor"),
        };
        roundtrip(&q);
    }

    #[test]
    fn localized_text_partial_masks() {
        roundtrip(&LocalizedText {
            locale: Some(String::from("en")),
            text: Some(String::from("Hello")),
        });
        roundtrip(&LocalizedText {
            locale: None,
            text: Some(String::from("only text")),
        });
        let bytes = roundtrip(&LocalizedText {
            locale: None,
            text: None,
        });
        assert_eq!(bytes, alloc::vec![0u8]);
    }

    #[test]
    fn extension_object_bodies() {
        roundtrip(&ExtensionObject {
            type_id: NodeId::numeric(0, 1),
            body: ExtensionObjectBody::None,
        });
        roundtrip(&ExtensionObject {
            type_id: NodeId::numeric(2, 99),
            body: ExtensionObjectBody::ByteString(alloc::vec![1, 2, 3, 4]),
        });
        roundtrip(&ExtensionObject {
            type_id: NodeId::numeric(0, 5),
            body: ExtensionObjectBody::XmlElement(String::from("<x/>")),
        });
    }

    #[test]
    fn variant_scalar_int32() {
        let v = Variant::scalar(VariantValue::Int32(-12345));
        let bytes = roundtrip(&v);
        // enc byte = Int32 kind value (6), no array/dim bits.
        assert_eq!(bytes.first().copied(), Some(BuiltinTypeKind::Int32.value()));
    }

    #[test]
    fn variant_scalar_string() {
        roundtrip(&Variant::scalar(VariantValue::String(String::from("hi"))));
    }

    #[test]
    fn variant_null() {
        let v = Variant {
            array_dimensions: Vec::new(),
            value: Vec::new(),
        };
        let bytes = roundtrip(&v);
        assert_eq!(bytes, alloc::vec![0u8]);
    }

    #[test]
    fn variant_1d_array() {
        let v = Variant {
            array_dimensions: alloc::vec![3],
            value: alloc::vec![
                VariantValue::Int32(1),
                VariantValue::Int32(2),
                VariantValue::Int32(3),
            ],
        };
        let bytes = roundtrip(&v);
        assert_ne!(bytes.first().copied().unwrap_or(0) & VARIANT_ARRAY_FLAG, 0);
        assert_eq!(
            bytes.first().copied().unwrap_or(0) & VARIANT_DIMENSIONS_FLAG,
            0
        );
    }

    #[test]
    fn variant_multidim_array() {
        let v = Variant {
            array_dimensions: alloc::vec![2, 2],
            value: alloc::vec![
                VariantValue::Byte(1),
                VariantValue::Byte(2),
                VariantValue::Byte(3),
                VariantValue::Byte(4),
            ],
        };
        let bytes = roundtrip(&v);
        let enc = bytes.first().copied().unwrap_or(0);
        assert_ne!(enc & VARIANT_ARRAY_FLAG, 0);
        assert_ne!(enc & VARIANT_DIMENSIONS_FLAG, 0);
    }

    #[test]
    fn variant_array_oversized_length_rejected_cleanly() {
        // Buffer-cap hardening regression (builtin.rs:538): a
        // wire-supplied array length must be rejected before
        // `Vec::reserve`, not turned into a multi-GB allocation
        // attempt from a 5-byte packet.
        let mut bytes = alloc::vec![VARIANT_ARRAY_FLAG | BuiltinTypeKind::Int32.value()];
        bytes.extend_from_slice(&1_000_000_000i32.to_le_bytes());
        // No element bytes follow.
        let err = from_binary::<Variant>(&bytes).expect_err("should reject cleanly");
        assert!(matches!(err, DecodeError::MalformedMessage { .. }));
    }

    #[test]
    fn array_dimensions_oversized_length_rejected_cleanly() {
        // Buffer-cap hardening regression (builtin.rs:553): the
        // ArrayDimensions count must be bounds-checked before
        // `Vec::with_capacity`.
        let mut bytes = alloc::vec![
            VARIANT_ARRAY_FLAG | VARIANT_DIMENSIONS_FLAG | BuiltinTypeKind::Byte.value()
        ];
        // One-element array (so the array-length + element decode
        // succeeds) …
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.push(0); // the single Byte element
        // … followed by a huge, unsupported ArrayDimensions count.
        bytes.extend_from_slice(&1_000_000_000i32.to_le_bytes());
        let err = from_binary::<Variant>(&bytes).expect_err("should reject cleanly");
        assert!(matches!(err, DecodeError::MalformedMessage { .. }));
    }

    #[test]
    fn variant_unsupported_element_type_rejected() {
        // type id 24 = Variant-of-Variant, not representable.
        let bytes = alloc::vec![24u8, 0x00];
        let err = from_binary::<Variant>(&bytes).expect_err("should reject");
        assert!(matches!(err, DecodeError::InvalidDiscriminant { .. }));
    }

    #[test]
    fn data_value_full() {
        let dv = DataValue {
            value: Some(Variant::scalar(VariantValue::Double(3.5))),
            status: Some(0x8000_0000),
            source_timestamp: Some(132_000_000_000_000_000),
            server_timestamp: Some(132_000_000_000_000_001),
            source_pico_sec: Some(10),
            server_pico_sec: Some(20),
        };
        roundtrip(&dv);
    }

    #[test]
    fn data_value_empty_mask() {
        let dv = DataValue {
            value: None,
            status: None,
            source_timestamp: None,
            server_timestamp: None,
            source_pico_sec: None,
            server_pico_sec: None,
        };
        let bytes = roundtrip(&dv);
        assert_eq!(bytes, alloc::vec![0u8]);
    }
}
