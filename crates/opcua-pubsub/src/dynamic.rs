// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Dynamic, metadata-driven decoding of RawData DataSet fields whose DataType
//! is a *custom* (non-built-in) structured, enumerated or simple type
//! (OPC-UA Part 6 §5.4 reversible structured encoding, RawData variant of
//! Part 14 §7.2.2.3.4).
//!
//! The RawData encoding carries no per-field type tags, so a custom struct is
//! written inline field-by-field per its
//! [`StructureDefinition`](crate::config::StructureDefinition). The
//! `zerodds-opcua-gateway` `Variant` only models the 25 built-in types, so
//! such values are decoded here into a [`DynamicValue`] tree driven by the
//! [`DataSetMetaData`] DataType descriptions — built-in leaves become
//! [`VariantValue`]s, custom structs become nested [`DynamicValue::Structure`]s.
//!
//! This is the decoding counterpart to the metadata round-trip in
//! [`crate::uadp::discovery`]; together they make custom-typed PubSub DataSets
//! fully usable without a dynamic type system in the gateway.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
use zerodds_opcua_gateway::node_id::{NodeId, NodeIdentifier};

use crate::binary::{UaReader, builtin_type_from_value, check_array_len, decode_builtin_value};
use crate::config::{
    DataSetMetaData, EnumDescription, SimpleTypeDescription, StructureDescription, StructureType,
};
use crate::error::DecodeError;

/// Guards against pathological / cyclic DataType definitions.
const MAX_DEPTH: u32 = 32;

/// A decoded DataSet field value of arbitrary (built-in or custom) DataType.
#[derive(Debug, Clone, PartialEq)]
pub enum DynamicValue {
    /// A built-in scalar value.
    Scalar(VariantValue),
    /// An array of values (any element kind).
    Array(Vec<DynamicValue>),
    /// A custom structure as ordered named members. A `Union` carries its one
    /// selected member; a null union or an absent optional field is
    /// [`DynamicValue::Null`].
    Structure(Vec<DynamicField>),
    /// A null value (null array, null union, or absent optional field).
    Null,
}

/// One named member of a [`DynamicValue::Structure`].
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicField {
    /// Member name (from the [`StructureField`](crate::config::StructureField)).
    pub name: String,
    /// Member value.
    pub value: DynamicValue,
}

/// Converts a self-describing [`Variant`] (from the Variant/DataValue
/// encodings) into the uniform [`DynamicValue`] view.
#[must_use]
pub fn variant_to_dynamic(v: &Variant) -> DynamicValue {
    if v.value.is_empty() {
        return DynamicValue::Null;
    }
    if v.array_dimensions.is_empty() && v.value.len() == 1 {
        DynamicValue::Scalar(v.value[0].clone())
    } else {
        DynamicValue::Array(v.value.iter().cloned().map(DynamicValue::Scalar).collect())
    }
}

/// Decodes a complete RawData payload against `meta`, producing one
/// [`DynamicField`] per metadata field.
///
/// # Errors
/// [`DecodeError`] if a DataType is not described in `meta`, the payload is
/// truncated, the nesting exceeds `MAX_DEPTH`, or trailing bytes remain.
pub fn decode_raw_dataset(
    bytes: &[u8],
    meta: &DataSetMetaData,
) -> Result<Vec<DynamicField>, DecodeError> {
    let mut r = UaReader::new(bytes);
    let mut out = Vec::with_capacity(meta.fields.len());
    for f in &meta.fields {
        let value = decode_value(&mut r, &f.data_type, f.value_rank, meta, 0)?;
        out.push(DynamicField {
            name: f.name.clone(),
            value,
        });
    }
    if !r.is_empty() {
        return Err(DecodeError::MalformedMessage {
            message: "RawData payload longer than the DataSetMetaData fields describe",
        });
    }
    Ok(out)
}

/// A DataType resolved against the metadata descriptions.
enum Resolved<'a> {
    Builtin(zerodds_opcua_gateway::types::BuiltinTypeKind),
    Structure(&'a StructureDescription),
    Enum(&'a EnumDescription),
    Simple(&'a SimpleTypeDescription),
}

/// Resolves a DataType NodeId to a built-in type or a custom description.
fn resolve_kind<'a>(dt: &NodeId, meta: &'a DataSetMetaData) -> Result<Resolved<'a>, DecodeError> {
    if dt.namespace_index == 0 {
        if let NodeIdentifier::Numeric(id) = dt.identifier_type {
            if (1..=25).contains(&id) {
                return Ok(Resolved::Builtin(builtin_type_from_value(id as u8)?));
            }
        }
    }
    if let Some(s) = meta
        .structure_data_types
        .iter()
        .find(|d| d.data_type_id == *dt)
    {
        return Ok(Resolved::Structure(s));
    }
    if let Some(e) = meta.enum_data_types.iter().find(|d| d.data_type_id == *dt) {
        return Ok(Resolved::Enum(e));
    }
    if let Some(s) = meta
        .simple_data_types
        .iter()
        .find(|d| d.data_type_id == *dt)
    {
        return Ok(Resolved::Simple(s));
    }
    Err(DecodeError::MalformedMessage {
        message: "RawData field references a DataType not described in the DataSetMetaData",
    })
}

/// Decodes a value of `dt` honouring its value rank (scalar vs.
/// length-prefixed array).
fn decode_value(
    r: &mut UaReader<'_>,
    dt: &NodeId,
    value_rank: i32,
    meta: &DataSetMetaData,
    depth: u32,
) -> Result<DynamicValue, DecodeError> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::MalformedMessage {
            message: "custom DataType nesting exceeds the supported depth",
        });
    }
    let resolved = resolve_kind(dt, meta)?;
    if value_rank < 0 {
        decode_one(r, &resolved, meta, depth)
    } else {
        let len = r.read_i32()?;
        if len < 0 {
            return Ok(DynamicValue::Null);
        }
        let len = len as usize;
        // Conservative bound: every value needs >= 1 wire byte, so a
        // count above the remaining bytes cannot be genuine. Reject
        // before `Vec::with_capacity` (mirrors
        // `crates/cdr/src/composite.rs`'s `len > reader.remaining()`
        // guard).
        check_array_len(r, len, 1, "array length exceeds remaining bytes")?;
        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(decode_one(r, &resolved, meta, depth)?);
        }
        Ok(DynamicValue::Array(items))
    }
}

/// Decodes a single (scalar) value of an already-resolved DataType.
fn decode_one(
    r: &mut UaReader<'_>,
    resolved: &Resolved<'_>,
    meta: &DataSetMetaData,
    depth: u32,
) -> Result<DynamicValue, DecodeError> {
    match resolved {
        Resolved::Builtin(bt) => Ok(DynamicValue::Scalar(decode_builtin_value(r, *bt)?)),
        Resolved::Simple(s) => Ok(DynamicValue::Scalar(decode_builtin_value(
            r,
            builtin_type_from_value(s.builtin_type)?,
        )?)),
        // An enumeration value is encoded as its underlying built-in integer.
        Resolved::Enum(e) => Ok(DynamicValue::Scalar(decode_builtin_value(
            r,
            builtin_type_from_value(e.builtin_type)?,
        )?)),
        Resolved::Structure(sd) => decode_struct(r, sd, meta, depth + 1),
    }
}

/// Decodes a structured value per its
/// [`StructureDefinition`](crate::config::StructureDefinition) kind.
fn decode_struct(
    r: &mut UaReader<'_>,
    sd: &StructureDescription,
    meta: &DataSetMetaData,
    depth: u32,
) -> Result<DynamicValue, DecodeError> {
    let def = &sd.structure_definition;
    match def.structure_type {
        StructureType::Union | StructureType::UnionWithSubtypedValues => {
            // UInt32 switch (1-based selector; 0 = null union).
            let switch = r.read_u32()?;
            if switch == 0 {
                return Ok(DynamicValue::Null);
            }
            let field =
                def.fields
                    .get((switch - 1) as usize)
                    .ok_or(DecodeError::MalformedMessage {
                        message: "union switch value out of range",
                    })?;
            let value = decode_value(r, &field.data_type, field.value_rank, meta, depth)?;
            Ok(DynamicValue::Structure(alloc::vec![DynamicField {
                name: field.name.clone(),
                value,
            }]))
        }
        StructureType::StructureWithOptionalFields => {
            // UInt32 encoding mask: one bit per optional field, in order.
            let mask = r.read_u32()?;
            let mut out = Vec::with_capacity(def.fields.len());
            let mut optional_index = 0u32;
            for field in &def.fields {
                let present = if field.is_optional {
                    let p = (mask >> optional_index) & 1 == 1;
                    optional_index += 1;
                    p
                } else {
                    true
                };
                let value = if present {
                    decode_value(r, &field.data_type, field.value_rank, meta, depth)?
                } else {
                    DynamicValue::Null
                };
                out.push(DynamicField {
                    name: field.name.clone(),
                    value,
                });
            }
            Ok(DynamicValue::Structure(out))
        }
        StructureType::Structure | StructureType::StructureWithSubtypedValues => {
            let mut out = Vec::with_capacity(def.fields.len());
            for field in &def.fields {
                let value = decode_value(r, &field.data_type, field.value_rank, meta, depth)?;
                out.push(DynamicField {
                    name: field.name.clone(),
                    value,
                });
            }
            Ok(DynamicValue::Structure(out))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{UaEncode, UaWriter};
    use crate::config::{
        EnumDefinition, EnumDescription, EnumField, FieldMetaData, StructureDefinition,
        StructureField,
    };
    use zerodds_opcua_gateway::types::{BuiltinTypeKind, LocalizedText};

    fn loc() -> LocalizedText {
        LocalizedText {
            locale: None,
            text: None,
        }
    }

    fn field(name: &str, data_type: NodeId, value_rank: i32, is_optional: bool) -> StructureField {
        StructureField {
            name: String::from(name),
            description: loc(),
            data_type,
            value_rank,
            array_dimensions: Vec::new(),
            max_string_length: 0,
            is_optional,
        }
    }

    fn struct_desc(
        id: u32,
        ty: StructureType,
        fields: Vec<StructureField>,
    ) -> StructureDescription {
        StructureDescription {
            data_type_id: NodeId::numeric(2, id),
            name: zerodds_opcua_gateway::types::QualifiedName {
                namespace_index: 2,
                name: String::from("T"),
            },
            structure_definition: StructureDefinition {
                default_encoding_id: NodeId::numeric(2, id + 1),
                base_data_type: NodeId::numeric(0, 22),
                structure_type: ty,
                fields,
            },
        }
    }

    /// metadata: one field `p` of custom struct Point{x:Double, y:Double}.
    fn point_meta() -> DataSetMetaData {
        let mut m = DataSetMetaData::new(
            "ds",
            alloc::vec![FieldMetaData {
                name: String::from("p"),
                description: loc(),
                field_flags: 0,
                builtin_type: BuiltinTypeKind::ExtensionObject,
                data_type: NodeId::numeric(2, 4000),
                value_rank: -1,
                array_dimensions: Vec::new(),
                max_string_length: 0,
                data_set_field_id: zerodds_opcua_gateway::types::Guid::from_bytes([0; 16]),
                properties: Vec::new(),
            }],
        );
        m.structure_data_types = alloc::vec![struct_desc(
            4000,
            StructureType::Structure,
            alloc::vec![
                field("x", NodeId::numeric(0, 11), -1, false),
                field("y", NodeId::numeric(0, 11), -1, false),
            ],
        )];
        m
    }

    #[test]
    fn decodes_plain_custom_struct() {
        // RawData: x=1.5, y=-2.0 back to back (no tags).
        let mut w = UaWriter::new();
        VariantValue::Double(1.5).encode(&mut w).unwrap();
        VariantValue::Double(-2.0).encode(&mut w).unwrap();
        let bytes = w.into_vec();

        let fields = decode_raw_dataset(&bytes, &point_meta()).expect("decode");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "p");
        assert_eq!(
            fields[0].value,
            DynamicValue::Structure(alloc::vec![
                DynamicField {
                    name: String::from("x"),
                    value: DynamicValue::Scalar(VariantValue::Double(1.5)),
                },
                DynamicField {
                    name: String::from("y"),
                    value: DynamicValue::Scalar(VariantValue::Double(-2.0)),
                },
            ])
        );
    }

    #[test]
    fn decodes_optional_fields_via_mask() {
        // Struct Opt{ a:Int32 (mandatory), b:Int32 (optional), c:Int32 (optional) }
        let mut m = DataSetMetaData::new(
            "ds",
            alloc::vec![FieldMetaData {
                name: String::from("o"),
                description: loc(),
                field_flags: 0,
                builtin_type: BuiltinTypeKind::ExtensionObject,
                data_type: NodeId::numeric(2, 4000),
                value_rank: -1,
                array_dimensions: Vec::new(),
                max_string_length: 0,
                data_set_field_id: zerodds_opcua_gateway::types::Guid::from_bytes([0; 16]),
                properties: Vec::new(),
            }],
        );
        m.structure_data_types = alloc::vec![struct_desc(
            4000,
            StructureType::StructureWithOptionalFields,
            alloc::vec![
                field("a", NodeId::numeric(0, 6), -1, false),
                field("b", NodeId::numeric(0, 6), -1, true),
                field("c", NodeId::numeric(0, 6), -1, true),
            ],
        )];

        // mask = 0b01 → b present, c absent. a=10, b=20.
        let mut w = UaWriter::new();
        w.write_u32(0b01);
        VariantValue::Int32(10).encode(&mut w).unwrap();
        VariantValue::Int32(20).encode(&mut w).unwrap();
        let bytes = w.into_vec();

        let fields = decode_raw_dataset(&bytes, &m).expect("decode");
        let DynamicValue::Structure(members) = &fields[0].value else {
            panic!("expected struct");
        };
        assert_eq!(
            members[0].value,
            DynamicValue::Scalar(VariantValue::Int32(10))
        );
        assert_eq!(
            members[1].value,
            DynamicValue::Scalar(VariantValue::Int32(20))
        );
        assert_eq!(members[2].value, DynamicValue::Null);
    }

    #[test]
    fn decodes_union_switch() {
        // Union U{ i:Int32 | d:Double }; switch=2 → d selected.
        let mut m = DataSetMetaData::new(
            "ds",
            alloc::vec![FieldMetaData {
                name: String::from("u"),
                description: loc(),
                field_flags: 0,
                builtin_type: BuiltinTypeKind::ExtensionObject,
                data_type: NodeId::numeric(2, 4000),
                value_rank: -1,
                array_dimensions: Vec::new(),
                max_string_length: 0,
                data_set_field_id: zerodds_opcua_gateway::types::Guid::from_bytes([0; 16]),
                properties: Vec::new(),
            }],
        );
        m.structure_data_types = alloc::vec![struct_desc(
            4000,
            StructureType::Union,
            alloc::vec![
                field("i", NodeId::numeric(0, 6), -1, false),
                field("d", NodeId::numeric(0, 11), -1, false),
            ],
        )];

        let mut w = UaWriter::new();
        w.write_u32(2); // select field 2 (d)
        VariantValue::Double(3.25).encode(&mut w).unwrap();
        let bytes = w.into_vec();

        let fields = decode_raw_dataset(&bytes, &m).expect("decode");
        assert_eq!(
            fields[0].value,
            DynamicValue::Structure(alloc::vec![DynamicField {
                name: String::from("d"),
                value: DynamicValue::Scalar(VariantValue::Double(3.25)),
            }])
        );
    }

    #[test]
    fn decodes_nested_struct_and_array() {
        // Outer{ name:String, points:Point[] }, Point{x:Int32,y:Int32}.
        let mut m = DataSetMetaData::new(
            "ds",
            alloc::vec![FieldMetaData {
                name: String::from("outer"),
                description: loc(),
                field_flags: 0,
                builtin_type: BuiltinTypeKind::ExtensionObject,
                data_type: NodeId::numeric(2, 5000),
                value_rank: -1,
                array_dimensions: Vec::new(),
                max_string_length: 0,
                data_set_field_id: zerodds_opcua_gateway::types::Guid::from_bytes([0; 16]),
                properties: Vec::new(),
            }],
        );
        m.structure_data_types = alloc::vec![
            struct_desc(
                4000,
                StructureType::Structure,
                alloc::vec![
                    field("x", NodeId::numeric(0, 6), -1, false),
                    field("y", NodeId::numeric(0, 6), -1, false),
                ],
            ),
            struct_desc(
                5000,
                StructureType::Structure,
                alloc::vec![
                    field("name", NodeId::numeric(0, 12), -1, false),
                    field("points", NodeId::numeric(2, 4000), 1, false),
                ],
            ),
        ];

        let mut w = UaWriter::new();
        VariantValue::String(String::from("hi"))
            .encode(&mut w)
            .unwrap();
        w.write_i32(2); // points array length
        VariantValue::Int32(1).encode(&mut w).unwrap();
        VariantValue::Int32(2).encode(&mut w).unwrap();
        VariantValue::Int32(3).encode(&mut w).unwrap();
        VariantValue::Int32(4).encode(&mut w).unwrap();
        let bytes = w.into_vec();

        let fields = decode_raw_dataset(&bytes, &m).expect("decode");
        let DynamicValue::Structure(outer) = &fields[0].value else {
            panic!("struct");
        };
        assert_eq!(
            outer[0].value,
            DynamicValue::Scalar(VariantValue::String(String::from("hi")))
        );
        let DynamicValue::Array(points) = &outer[1].value else {
            panic!("array");
        };
        assert_eq!(points.len(), 2);
        assert_eq!(
            points[0],
            DynamicValue::Structure(alloc::vec![
                DynamicField {
                    name: String::from("x"),
                    value: DynamicValue::Scalar(VariantValue::Int32(1)),
                },
                DynamicField {
                    name: String::from("y"),
                    value: DynamicValue::Scalar(VariantValue::Int32(2)),
                },
            ])
        );
    }

    #[test]
    fn decodes_enum_as_underlying_integer() {
        let mut m = DataSetMetaData::new(
            "ds",
            alloc::vec![FieldMetaData {
                name: String::from("color"),
                description: loc(),
                field_flags: 0,
                builtin_type: BuiltinTypeKind::Int32,
                data_type: NodeId::numeric(2, 7000),
                value_rank: -1,
                array_dimensions: Vec::new(),
                max_string_length: 0,
                data_set_field_id: zerodds_opcua_gateway::types::Guid::from_bytes([0; 16]),
                properties: Vec::new(),
            }],
        );
        m.enum_data_types = alloc::vec![EnumDescription {
            data_type_id: NodeId::numeric(2, 7000),
            name: zerodds_opcua_gateway::types::QualifiedName {
                namespace_index: 2,
                name: String::from("Color"),
            },
            enum_definition: EnumDefinition {
                fields: alloc::vec![EnumField {
                    value: 2,
                    display_name: loc(),
                    description: loc(),
                    name: String::from("Green"),
                }],
            },
            builtin_type: 6, // Int32
        }];

        let mut w = UaWriter::new();
        VariantValue::Int32(2).encode(&mut w).unwrap();
        let fields = decode_raw_dataset(&w.into_vec(), &m).expect("decode");
        assert_eq!(
            fields[0].value,
            DynamicValue::Scalar(VariantValue::Int32(2))
        );
    }

    #[test]
    fn unknown_datatype_is_rejected() {
        let mut m = point_meta();
        // Point references no metadata → strip the description.
        m.structure_data_types.clear();
        let bytes = alloc::vec![0u8; 16];
        assert!(matches!(
            decode_raw_dataset(&bytes, &m),
            Err(DecodeError::MalformedMessage { .. })
        ));
    }
}
