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
        // OPC 10000-6 Table 27: each timestamp is followed by its
        // picoseconds.
        if let Some(ts) = self.source_timestamp {
            w.write_i64(ts);
        }
        if let Some(pico) = self.source_pico_sec {
            w.write_u16(pico);
        }
        if let Some(ts) = self.server_timestamp {
            w.write_i64(ts);
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
        let source_pico_sec = if mask & DV_SOURCE_PICO_FLAG != 0 {
            Some(r.read_u16()?)
        } else {
            None
        };
        let server_timestamp = if mask & DV_SERVER_TS_FLAG != 0 {
            Some(r.read_i64()?)
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

    /// `v` encodes to `expected`, the octets OPC 10000-6 §5.2.2 gives for
    /// it, and `expected` decodes to `v`.
    fn octets<T: UaEncode + UaDecode + PartialEq + core::fmt::Debug>(v: &T, expected: &[u8]) {
        assert_eq!(to_binary(v).expect("encode"), expected, "{v:?}");
        let back: T = from_binary(expected).expect("decode");
        assert_eq!(&back, v);
    }

    fn ua_string(s: &str) -> Vec<u8> {
        let mut out = i32::try_from(s.len()).expect("len").to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out
    }

    /// §5.2.2.9, Tables 17 to 20: the Two Byte form (0x00, the identifier
    /// as a Byte), the Four Byte form (0x01, the namespace as a Byte, the
    /// identifier as a UInt16), the Numeric form (0x02, UInt16 and UInt32)
    /// and the String form (0x03, UInt16 and a String; the example of
    /// Figure 7, namespace 1 and "Hot水", whose first identifier octet the
    /// figure prints as 0x72 for the 'H' of the text).
    #[test]
    fn uacs_5_2_2_9_the_four_nodeid_forms_of_tables_17_to_20() {
        octets(&NodeId::numeric(0, 42), &[NODEID_TWO_BYTE, 42]);
        octets(
            &NodeId::numeric(5, 1025),
            &[NODEID_FOUR_BYTE, 0x05, 0x01, 0x04],
        );
        octets(
            &NodeId::numeric(300, 70_000),
            &[NODEID_NUMERIC, 0x2C, 0x01, 0x70, 0x11, 0x01, 0x00],
        );
        octets(
            &NodeId {
                namespace_index: 1,
                identifier_type: NodeIdentifier::String(String::from("Hot水")),
            },
            &[
                NODEID_STRING,
                0x01,
                0x00,
                0x06,
                0x00,
                0x00,
                0x00,
                0x48,
                0x6F,
                0x74,
                0xE6,
                0xB0,
                0xB4,
            ],
        );
    }

    /// §5.2.2.6, Figure 5: the Guid 72962B91-FA75-4AE6-8D28-B404DC7DAF63.
    #[test]
    fn uacs_5_2_2_6_the_guid_of_figure_5() {
        octets(
            &Guid {
                data1: 0x7296_2B91,
                data2: 0xFA75,
                data3: 0x4AE6,
                data4: [0x8D, 0x28, 0xB4, 0x04, 0xDC, 0x7D, 0xAF, 0x63],
            },
            &[
                0x91, 0x2B, 0x96, 0x72, 0x75, 0xFA, 0xE6, 0x4A, 0x8D, 0x28, 0xB4, 0x04, 0xDC, 0x7D,
                0xAF, 0x63,
            ],
        );
    }

    /// §5.2.2.10, Table 21: the NodeId with the NamespaceUri flag 0x80 and
    /// the ServerIndex flag 0x40, then the NamespaceUri and the ServerIndex;
    /// without them the NodeId alone.
    #[test]
    fn uacs_5_2_2_10_an_expanded_nodeid_flags_and_appends_uri_and_server_index() {
        let mut expected = alloc::vec![
            NODEID_TWO_BYTE | EXPNODEID_URI_FLAG | EXPNODEID_SERVER_FLAG,
            7
        ];
        expected.extend_from_slice(&ua_string("urn:example"));
        expected.extend_from_slice(&3_u32.to_le_bytes());
        octets(
            &ExpandedNodeId {
                node_id: NodeId::numeric(0, 7),
                namespace_uri: String::from("urn:example"),
                server_index: 3,
            },
            &expected,
        );
        octets(
            &ExpandedNodeId {
                node_id: NodeId::numeric(0, 7),
                namespace_uri: String::new(),
                server_index: 0,
            },
            &[NODEID_TWO_BYTE, 7],
        );
    }

    /// §5.2.2.13, Table 23: NamespaceIndex (UInt16) and Name (String).
    #[test]
    fn uacs_5_2_2_13_a_qualified_name_is_namespace_index_and_name() {
        let mut expected = alloc::vec![0x01, 0x00];
        expected.extend_from_slice(&ua_string("Sensor"));
        octets(
            &QualifiedName {
                namespace_index: 1,
                name: String::from("Sensor"),
            },
            &expected,
        );
    }

    /// §5.2.2.14, Table 24: the EncodingMask (0x01 Locale, 0x02 Text), then
    /// the fields it names.
    #[test]
    fn uacs_5_2_2_14_localized_text_carries_the_fields_its_mask_names() {
        let mut both = alloc::vec![0x03];
        both.extend_from_slice(&ua_string("en"));
        both.extend_from_slice(&ua_string("Hello"));
        octets(
            &LocalizedText {
                locale: Some(String::from("en")),
                text: Some(String::from("Hello")),
            },
            &both,
        );
        let mut text = alloc::vec![0x02];
        text.extend_from_slice(&ua_string("only text"));
        octets(
            &LocalizedText {
                locale: None,
                text: Some(String::from("only text")),
            },
            &text,
        );
        octets(
            &LocalizedText {
                locale: None,
                text: None,
            },
            &[0x00],
        );
    }

    /// §5.2.2.15, Table 25: TypeId, Encoding (0x00 no body, 0x01
    /// ByteString, 0x02 XmlElement), Length and Body.
    #[test]
    fn uacs_5_2_2_15_an_extension_object_is_type_id_encoding_length_and_body() {
        octets(
            &ExtensionObject {
                type_id: NodeId::numeric(0, 1),
                body: ExtensionObjectBody::None,
            },
            &[NODEID_TWO_BYTE, 0x01, 0x00],
        );
        octets(
            &ExtensionObject {
                type_id: NodeId::numeric(2, 99),
                body: ExtensionObjectBody::ByteString(alloc::vec![1, 2, 3, 4]),
            },
            &[
                NODEID_FOUR_BYTE,
                0x02,
                0x63,
                0x00,
                0x01,
                0x04,
                0x00,
                0x00,
                0x00,
                1,
                2,
                3,
                4,
            ],
        );
        let mut xml = alloc::vec![NODEID_TWO_BYTE, 0x05, 0x02];
        xml.extend_from_slice(&ua_string("<x/>"));
        octets(
            &ExtensionObject {
                type_id: NodeId::numeric(0, 5),
                body: ExtensionObjectBody::XmlElement(String::from("<x/>")),
            },
            &xml,
        );
    }

    /// §5.2.2.16, Table 26: the EncodingMask carries the built-in type id
    /// (Table 1: Byte 3, Int32 6, String 12) in bits 0 to 5; a scalar
    /// follows it directly, a NULL Variant is the mask 0 alone.
    #[test]
    fn uacs_5_2_2_16_a_scalar_variant_is_its_type_id_and_its_value() {
        octets(
            &Variant::scalar(VariantValue::Int32(-12345)),
            &[BuiltinTypeKind::Int32.value(), 0xC7, 0xCF, 0xFF, 0xFF],
        );
        let mut s = alloc::vec![BuiltinTypeKind::String.value()];
        s.extend_from_slice(&ua_string("hi"));
        octets(
            &Variant::scalar(VariantValue::String(String::from("hi"))),
            &s,
        );
        octets(
            &Variant {
                array_dimensions: Vec::new(),
                value: Vec::new(),
            },
            &[0x00],
        );
    }

    /// §5.2.2.16, Table 26: bit 7 marks an array, the ArrayLength (Int32)
    /// and the elements follow; bit 6 marks the ArrayDimensions, which
    /// follow the values for two or more dimensions.
    #[test]
    fn uacs_5_2_2_16_an_array_variant_has_its_length_and_its_dimensions_after_the_values() {
        let mut one = alloc::vec![
            VARIANT_ARRAY_FLAG | BuiltinTypeKind::Int32.value(),
            3,
            0,
            0,
            0
        ];
        for v in [1_i32, 2, 3] {
            one.extend_from_slice(&v.to_le_bytes());
        }
        octets(
            &Variant {
                array_dimensions: alloc::vec![3],
                value: alloc::vec![
                    VariantValue::Int32(1),
                    VariantValue::Int32(2),
                    VariantValue::Int32(3),
                ],
            },
            &one,
        );
        octets(
            &Variant {
                array_dimensions: alloc::vec![2, 2],
                value: alloc::vec![
                    VariantValue::Byte(1),
                    VariantValue::Byte(2),
                    VariantValue::Byte(3),
                    VariantValue::Byte(4),
                ],
            },
            &[
                VARIANT_ARRAY_FLAG | VARIANT_DIMENSIONS_FLAG | BuiltinTypeKind::Byte.value(),
                4,
                0,
                0,
                0,
                1,
                2,
                3,
                4,
                2,
                0,
                0,
                0,
                2,
                0,
                0,
                0,
                2,
                0,
                0,
                0,
            ],
        );
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

    /// §5.2.2.17, Table 27: the EncodingMask (0x01 Value, 0x02 Status,
    /// 0x04 SourceTimestamp, 0x08 ServerTimestamp, 0x10 SourcePicoseconds,
    /// 0x20 ServerPicoseconds), then Value, Status, SourceTimestamp,
    /// SourcePicoseconds, ServerTimestamp, ServerPicoseconds in that order;
    /// an empty DataValue is the mask 0 alone.
    #[test]
    fn uacs_5_2_2_17_a_data_value_is_its_mask_and_the_fields_in_the_order_of_table_27() {
        let mut expected = alloc::vec![0x3F, BuiltinTypeKind::Double.value()];
        expected.extend_from_slice(&3.5_f64.to_le_bytes());
        expected.extend_from_slice(&0x8000_0000_u32.to_le_bytes());
        expected.extend_from_slice(&132_000_000_000_000_000_i64.to_le_bytes());
        expected.extend_from_slice(&10_u16.to_le_bytes());
        expected.extend_from_slice(&132_000_000_000_000_001_i64.to_le_bytes());
        expected.extend_from_slice(&20_u16.to_le_bytes());
        octets(
            &DataValue {
                value: Some(Variant::scalar(VariantValue::Double(3.5))),
                status: Some(0x8000_0000),
                source_timestamp: Some(132_000_000_000_000_000),
                server_timestamp: Some(132_000_000_000_000_001),
                source_pico_sec: Some(10),
                server_pico_sec: Some(20),
            },
            &expected,
        );
        octets(
            &DataValue {
                value: None,
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
            &[0x00],
        );
    }
}
