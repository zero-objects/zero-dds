// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OPC UA DataTypes of a topic's DDS type (Gateway 1.0 §9.2).
//!
//! Every structure, union, enumeration and bitmask a topic's type uses is a
//! DataType node of its own: `<Name>DataType`, a subtype of `Structure`,
//! `Union`, `Enumeration` or `OptionSet`, with its `DataTypeDefinition`
//! (OPC 10000-3 §5.8.3) and, for a structured one, its `Default Binary`
//! encoding node, whose NodeId is the TypeId of the type's
//! ExtensionObjects (OPC 10000-6 §5.2.2.15). A structure also has its
//! `<Name>VariableType` (§9.2.4.1.2, Table 9.16). An enumeration has the
//! `EnumValues` Property (Table 9.7), a bitmask the `OptionSetValues`
//! Property (Table 9.11).
//!
//! The NodeIds follow from the type's scoped name, in the daemon's
//! namespace: `DataType:<name>`, `DataType:<name>/DefaultBinary`,
//! `VariableType:<name>`. Two different types of one name are refused
//! ([`TypeConflict`]): their values would carry one TypeId.

use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_opcua_gateway::data_value::{
    DataValue, ExtensionObject, ExtensionObjectBody, Variant, VariantValue,
};
use zerodds_opcua_gateway::dds_to_ua::naming::{data_type_name, variable_type_name};
use zerodds_opcua_gateway::dds_to_ua::walker::{BitmaskDef, DdsType, EnumDef, StructDef, UnionDef};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_pubsub::UaEncode;
use zerodds_opcua_pubsub::config::{
    EnumDefinition, EnumField, StructureDefinition, StructureField, StructureType,
};
use zerodds_opcua_server::address_space::{AddressSpace, NodeClass, NodeMeta, reference_types};
use zerodds_opcua_server::attributes::{
    ClassAttributes, DataTypeAttributes, VariableAttributes, VariableTypeAttributes, access_level,
};

/// NodeIds of namespace 0 (OPC 10000-5, `NodeIds.csv`).
pub mod ns0 {
    /// `Boolean`.
    pub const BOOLEAN: u32 = 1;
    /// `SByte`.
    pub const SBYTE: u32 = 2;
    /// `Byte`.
    pub const BYTE: u32 = 3;
    /// `Int16`.
    pub const INT16: u32 = 4;
    /// `UInt16`.
    pub const UINT16: u32 = 5;
    /// `Int32`.
    pub const INT32: u32 = 6;
    /// `UInt32`.
    pub const UINT32: u32 = 7;
    /// `Int64`.
    pub const INT64: u32 = 8;
    /// `UInt64`.
    pub const UINT64: u32 = 9;
    /// `Float`.
    pub const FLOAT: u32 = 10;
    /// `Double`.
    pub const DOUBLE: u32 = 11;
    /// `String`.
    pub const STRING: u32 = 12;
    /// `ByteString`.
    pub const BYTE_STRING: u32 = 15;
    /// `LocalizedText`.
    pub const LOCALIZED_TEXT: u32 = 21;
    /// `Structure`.
    pub const STRUCTURE: u32 = 22;
    /// `BaseDataType`.
    pub const BASE_DATA_TYPE: u32 = 24;
    /// `Enumeration`.
    pub const ENUMERATION: u32 = 29;
    /// `HasModellingRule`.
    pub const HAS_MODELLING_RULE: u32 = 37;
    /// `HasEncoding`.
    pub const HAS_ENCODING: u32 = 38;
    /// `Decimal`.
    pub const DECIMAL: u32 = 50;
    /// `BaseVariableType`.
    pub const BASE_VARIABLE_TYPE: u32 = 62;
    /// `BaseDataVariableType`.
    pub const BASE_DATA_VARIABLE_TYPE: u32 = 63;
    /// `PropertyType`.
    pub const PROPERTY_TYPE: u32 = 68;
    /// `DataTypeEncodingType`.
    pub const DATA_TYPE_ENCODING_TYPE: u32 = 76;
    /// `ModellingRule_Mandatory`.
    pub const MODELLING_RULE_MANDATORY: u32 = 78;
    /// `ModellingRule_Optional`.
    pub const MODELLING_RULE_OPTIONAL: u32 = 80;
    /// `StructureDefinition_Encoding_DefaultBinary`.
    pub const STRUCTURE_DEFINITION_BINARY: u32 = 122;
    /// `EnumDefinition_Encoding_DefaultBinary`.
    pub const ENUM_DEFINITION_BINARY: u32 = 123;
    /// `EnumValueType`.
    pub const ENUM_VALUE_TYPE: u32 = 7594;
    /// `EnumValueType_Encoding_DefaultBinary`.
    pub const ENUM_VALUE_TYPE_BINARY: u32 = 8251;
    /// `OptionSet`.
    pub const OPTION_SET: u32 = 12755;
    /// `Union`.
    pub const UNION: u32 = 12756;
    /// `KeyValuePair`.
    pub const KEY_VALUE_PAIR: u32 = 14533;
    /// `KeyValuePair_Encoding_DefaultBinary`.
    pub const KEY_VALUE_PAIR_BINARY: u32 = 14846;
}

/// The URI of the daemon's namespace, the one its NodeIds and generated
/// DataTypes are in (`crate::daemon::server::NS`, entry 2 of the server's
/// `NamespaceArray`).
pub const NAMESPACE_URI: &str = "urn:zerodds:opcua-bridge";

/// The recursion bound of the type walk, the one
/// [`crate::dynamic_type`] applies when it builds the type.
const MAX_DEPTH: usize = 64;

/// Two different types of one scoped name: the second's DataType would
/// replace the first's, and the values of both would carry one TypeId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConflict {
    /// The scoped name of the types.
    pub type_name: String,
}

impl core::fmt::Display for TypeConflict {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "two different types are named `{}`; their OPC UA DataTypes would share one NodeId",
            self.type_name
        )
    }
}

impl std::error::Error for TypeConflict {}

/// The DataType of the DDS type `type_name`.
#[must_use]
pub fn data_type_id(ns: u16, type_name: &str) -> NodeId {
    string_id(ns, std::format!("DataType:{type_name}"))
}

/// The `Default Binary` encoding of the DDS type `type_name`: the TypeId
/// of its ExtensionObjects.
#[must_use]
pub fn encoding_id(ns: u16, type_name: &str) -> NodeId {
    string_id(ns, std::format!("DataType:{type_name}/DefaultBinary"))
}

/// The VariableType of the structure `type_name`.
#[must_use]
pub fn variable_type_id(ns: u16, type_name: &str) -> NodeId {
    string_id(ns, std::format!("VariableType:{type_name}"))
}

/// A string NodeId; a scoped name past the 4096 octets of a string
/// identifier is cut there (no DDS type name comes near it, and the cut
/// name still names one type).
fn string_id(ns: u16, identifier: String) -> NodeId {
    let mut identifier = identifier;
    if identifier.len() > 4096 {
        let mut end = 4096;
        while !identifier.is_char_boundary(end) {
            end -= 1;
        }
        identifier.truncate(end);
    }
    NodeId::string(ns, identifier).unwrap_or(NodeId::numeric(ns, 0))
}

/// The name of a type without its scope, for the BrowseNames.
fn simple(type_name: &str) -> &str {
    type_name.rsplit("::").next().unwrap_or(type_name)
}

/// The DataType, ValueRank and ArrayDimensions of a Variable or a structure
/// field of a DDS type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    /// `DataType`.
    pub data_type: NodeId,
    /// `ValueRank`: `-1` a scalar, the number of dimensions of an array.
    pub value_rank: i32,
    /// `ArrayDimensions`; `0` for a dimension of no fixed length.
    pub array_dimensions: Vec<u32>,
}

/// The [`Shape`] of `ty` (Gateway 1.0 Table 9.2 for the primitive types,
/// §9.2.5 for the collections). A `map` is an array of `KeyValuePair`
/// (OPC 10000-5 §12.21), its key the text the Gateway names the entry by
/// (Table 9.41).
#[must_use]
pub fn shape_of(ns: u16, ty: &DdsType) -> Shape {
    match ty.resolve_alias() {
        // A collection of collections (Gateway 1.0 §9.2.5.2.2.5) has no
        // array type in OPC UA: its elements are Variants, each holding
        // one inner collection.
        DdsType::Array {
            element,
            dimensions,
        } if is_collection(element) => Shape {
            data_type: NodeId::numeric(0, ns0::BASE_DATA_TYPE),
            value_rank: i32::try_from(dimensions.len()).unwrap_or(1).max(1),
            array_dimensions: dimensions.clone(),
        },
        DdsType::Sequence { element, bound } if is_collection(element) => Shape {
            data_type: NodeId::numeric(0, ns0::BASE_DATA_TYPE),
            value_rank: 1,
            array_dimensions: std::vec![bound.unwrap_or(0)],
        },
        DdsType::Array {
            element,
            dimensions,
        } => Shape {
            data_type: data_type_of(ns, element),
            value_rank: i32::try_from(dimensions.len()).unwrap_or(1).max(1),
            array_dimensions: dimensions.clone(),
        },
        DdsType::Sequence { element, bound } => Shape {
            data_type: data_type_of(ns, element),
            value_rank: 1,
            array_dimensions: std::vec![bound.unwrap_or(0)],
        },
        DdsType::Map { bound, .. } => Shape {
            data_type: NodeId::numeric(0, ns0::KEY_VALUE_PAIR),
            value_rank: 1,
            array_dimensions: std::vec![bound.unwrap_or(0)],
        },
        other => Shape {
            data_type: data_type_of(ns, other),
            value_rank: -1,
            array_dimensions: Vec::new(),
        },
    }
}

/// The DataType of a value of `ty`; for a collection, that of its
/// elements.
///
/// zerodds-lint: recursion-depth 64 (one level per collection around an
/// element; the type's nesting is bounded where it is built)
#[must_use]
pub fn data_type_of(ns: u16, ty: &DdsType) -> NodeId {
    let n = |id: u32| NodeId::numeric(0, id);
    match ty.resolve_alias() {
        DdsType::Boolean => n(ns0::BOOLEAN),
        DdsType::Int8 => n(ns0::SBYTE),
        DdsType::UInt8 | DdsType::Char8 => n(ns0::BYTE),
        DdsType::Int16 => n(ns0::INT16),
        DdsType::UInt16 | DdsType::Char16 => n(ns0::UINT16),
        DdsType::Int32 => n(ns0::INT32),
        DdsType::UInt32 => n(ns0::UINT32),
        DdsType::Int64 => n(ns0::INT64),
        DdsType::UInt64 => n(ns0::UINT64),
        DdsType::Float32 => n(ns0::FLOAT),
        DdsType::Float64 => n(ns0::DOUBLE),
        DdsType::Float128 => n(ns0::BYTE_STRING),
        DdsType::Fixed { .. } => n(ns0::DECIMAL),
        DdsType::String8 | DdsType::String16 => n(ns0::STRING),
        DdsType::Enum(def) => data_type_id(ns, &def.name),
        DdsType::Bitmask(def) => data_type_id(ns, &def.name),
        DdsType::Struct(def) => data_type_id(ns, &def.name),
        DdsType::Union(def) => data_type_id(ns, &def.name),
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. }
            if is_collection(element) =>
        {
            n(ns0::BASE_DATA_TYPE)
        }
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => {
            data_type_of(ns, element)
        }
        DdsType::Map { .. } => n(ns0::KEY_VALUE_PAIR),
        DdsType::Alias { .. } => n(ns0::BASE_DATA_TYPE),
    }
}

/// Whether `ty` is an array, a sequence or a `map`.
fn is_collection(ty: &DdsType) -> bool {
    matches!(
        ty.resolve_alias(),
        DdsType::Array { .. } | DdsType::Sequence { .. } | DdsType::Map { .. }
    )
}

/// The DDS type of the part of a sample of `ty` at `path`, as
/// [`crate::dynamic_value::flatten`] names the paths: a member or a case by
/// its name, a union's discriminator as `SwitchField`, an element of a
/// collection by its element name, an entry of a `map` by its key.
#[must_use]
pub fn type_at_path<'a>(ty: &'a DdsType, path: &str) -> Option<&'a DdsType> {
    let mut current = ty;
    let mut rest = path;
    for _ in 0..=MAX_DEPTH {
        if rest.is_empty() {
            return Some(current);
        }
        let (head, tail) = rest.split_once('.').unwrap_or((rest, ""));
        current = match current.resolve_alias() {
            DdsType::Struct(def) => {
                &def.members
                    .iter()
                    .find(|member| member.name == head)?
                    .member_type
            }
            DdsType::Union(def) if head == "SwitchField" => &def.discriminator,
            DdsType::Union(def) => &def.cases.iter().find(|case| case.name == head)?.member_type,
            DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => element,
            DdsType::Map { value, .. } => {
                // A key may hold dots: the whole rest is the key of an entry
                // whose value has nothing under it.
                if matches!(
                    value.resolve_alias(),
                    DdsType::Struct(_)
                        | DdsType::Union(_)
                        | DdsType::Map { .. }
                        | DdsType::Array { .. }
                        | DdsType::Sequence { .. }
                ) {
                    value
                } else {
                    return Some(value);
                }
            }
            _ => return None,
        };
        rest = tail;
    }
    None
}

/// The built-in type a value of `ty` is transferred as (OPC 10000-14
/// §6.2.3.2.3, `BuiltInType`): an enumeration as Int32, a structure, a
/// union, a bitmask, a `map` and a `Decimal` as ExtensionObject, a
/// collection of collections as Variant, a collection as its elements.
///
/// zerodds-lint: recursion-depth 2 (it calls itself only for the element of
/// a collection whose element is no collection)
#[must_use]
pub fn builtin_of(ty: &DdsType) -> zerodds_opcua_gateway::types::BuiltinTypeKind {
    use zerodds_opcua_gateway::types::BuiltinTypeKind as B;
    match ty.resolve_alias() {
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. }
            if is_collection(element) =>
        {
            B::Variant
        }
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => builtin_of(element),
        DdsType::Boolean => B::Boolean,
        DdsType::Int8 => B::SByte,
        DdsType::UInt8 | DdsType::Char8 => B::Byte,
        DdsType::Int16 => B::Int16,
        DdsType::UInt16 | DdsType::Char16 => B::UInt16,
        DdsType::Int32 | DdsType::Enum(_) => B::Int32,
        DdsType::UInt32 => B::UInt32,
        DdsType::Int64 => B::Int64,
        DdsType::UInt64 => B::UInt64,
        DdsType::Float32 => B::Float,
        DdsType::Float64 => B::Double,
        DdsType::Float128 => B::ByteString,
        DdsType::String8 | DdsType::String16 => B::String,
        DdsType::Fixed { .. }
        | DdsType::Bitmask(_)
        | DdsType::Struct(_)
        | DdsType::Union(_)
        | DdsType::Map { .. } => B::ExtensionObject,
        DdsType::Alias { .. } => B::Variant,
    }
}

/// The descriptions of the DataTypes a value of `ty` uses, for the
/// `DataTypeSchemaHeader` of a `DataSetMetaData` (OPC 10000-14
/// §6.2.3.2.2): a `StructureDescription` per structure and union (and
/// `KeyValuePair`, which is no built-in type, for a `map`), an
/// `EnumDescription` per enumeration (built-in type Int32) and bitmask
/// (an `OptionSet` in an ExtensionObject).
#[must_use]
pub fn type_descriptions(
    ns: u16,
    ty: &DdsType,
) -> (
    Vec<zerodds_opcua_pubsub::config::StructureDescription>,
    Vec<zerodds_opcua_pubsub::config::EnumDescription>,
) {
    let mut structures = Vec::new();
    let mut enums = Vec::new();
    describe(ns, ty, &mut structures, &mut enums, 0);
    (structures, enums)
}

/// zerodds-lint: recursion-depth 64 (one level per nested type; refused
/// past `MAX_DEPTH`)
fn describe(
    ns: u16,
    ty: &DdsType,
    structures: &mut Vec<zerodds_opcua_pubsub::config::StructureDescription>,
    enums: &mut Vec<zerodds_opcua_pubsub::config::EnumDescription>,
    depth: usize,
) {
    use zerodds_opcua_pubsub::config::{EnumDescription, StructureDescription};
    if depth > MAX_DEPTH {
        return;
    }
    let name = |type_name: &str| QualifiedName {
        namespace_index: ns,
        name: data_type_name(simple(type_name)),
    };
    let mut structure = |id: NodeId, qname: QualifiedName, definition: StructureDefinition| {
        if !structures.iter().any(|known| known.data_type_id == id) {
            structures.push(StructureDescription {
                data_type_id: id,
                name: qname,
                structure_definition: definition,
            });
        }
    };
    match ty {
        DdsType::Alias { target, .. } => describe(ns, target, structures, enums, depth + 1),
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => {
            describe(ns, element, structures, enums, depth + 1);
        }
        DdsType::Map { key, value, .. } => {
            structure(
                NodeId::numeric(0, ns0::KEY_VALUE_PAIR),
                ns0_name("KeyValuePair"),
                StructureDefinition {
                    default_encoding_id: NodeId::numeric(0, ns0::KEY_VALUE_PAIR_BINARY),
                    base_data_type: NodeId::numeric(0, ns0::STRUCTURE),
                    structure_type: StructureType::Structure,
                    fields: std::vec![
                        StructureField {
                            name: "Key".into(),
                            description: no_text(),
                            data_type: NodeId::numeric(0, 20),
                            value_rank: -1,
                            array_dimensions: Vec::new(),
                            max_string_length: 0,
                            is_optional: false,
                        },
                        StructureField {
                            name: "Value".into(),
                            description: no_text(),
                            data_type: NodeId::numeric(0, ns0::BASE_DATA_TYPE),
                            value_rank: -1,
                            array_dimensions: Vec::new(),
                            max_string_length: 0,
                            is_optional: false,
                        },
                    ],
                },
            );
            describe(ns, key, structures, enums, depth + 1);
            describe(ns, value, structures, enums, depth + 1);
        }
        DdsType::Struct(def) => {
            structure(
                data_type_id(ns, &def.name),
                name(&def.name),
                structure_definition(ns, def),
            );
            for member in &def.members {
                describe(ns, &member.member_type, structures, enums, depth + 1);
            }
        }
        DdsType::Union(def) => {
            structure(
                data_type_id(ns, &def.name),
                name(&def.name),
                union_definition(ns, def),
            );
            describe(ns, &def.discriminator, structures, enums, depth + 1);
            for case in &def.cases {
                describe(ns, &case.member_type, structures, enums, depth + 1);
            }
        }
        DdsType::Enum(def) => {
            let id = data_type_id(ns, &def.name);
            if !enums.iter().any(|known| known.data_type_id == id) {
                enums.push(EnumDescription {
                    data_type_id: id,
                    name: name(&def.name),
                    enum_definition: enum_definition(def),
                    builtin_type: 6,
                });
            }
        }
        DdsType::Bitmask(def) => {
            let id = data_type_id(ns, &def.name);
            if !enums.iter().any(|known| known.data_type_id == id) {
                enums.push(EnumDescription {
                    data_type_id: id,
                    name: name(&def.name),
                    enum_definition: option_set_definition(def),
                    builtin_type: 22,
                });
            }
        }
        _ => {}
    }
}

/// The TypeDefinition of a Variable of `ty`: the `<Name>VariableType` of a
/// structure (Table 9.16), `BaseDataVariableType` for everything else
/// (Tables 9.8, 9.12, 9.21).
#[must_use]
pub fn type_definition_of(ns: u16, ty: &DdsType) -> NodeId {
    match ty.resolve_alias() {
        DdsType::Struct(def) => variable_type_id(ns, &def.name),
        _ => NodeId::numeric(0, ns0::BASE_DATA_VARIABLE_TYPE),
    }
}

/// Adds the DataType nodes of every structure, union, enumeration and
/// bitmask `ty` uses, with their encodings, Properties and the
/// VariableTypes of the structures, and the namespace-0 types they are
/// subtypes of. A type whose nodes are there already is left as it is.
///
/// # Errors
/// [`TypeConflict`] when a DataType of the same NodeId is there with
/// another definition. Nodes added before the conflict was found stay; they
/// describe types that are what they say.
pub fn ensure_types(space: &mut AddressSpace, ns: u16, ty: &DdsType) -> Result<(), TypeConflict> {
    ensure_base_types(space);
    ensure(space, ns, ty, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per nested type; refused
/// past `MAX_DEPTH`, which the type's own construction already bounds)
fn ensure(
    space: &mut AddressSpace,
    ns: u16,
    ty: &DdsType,
    depth: usize,
) -> Result<(), TypeConflict> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    match ty {
        DdsType::Alias { target, .. } => ensure(space, ns, target, depth + 1),
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => {
            ensure(space, ns, element, depth + 1)
        }
        DdsType::Map { key, value, .. } => {
            ensure(space, ns, key, depth + 1)?;
            ensure(space, ns, value, depth + 1)
        }
        DdsType::Enum(def) => ensure_enum(space, ns, def),
        DdsType::Bitmask(def) => ensure_bitmask(space, ns, def),
        DdsType::Struct(def) => {
            for member in &def.members {
                ensure(space, ns, &member.member_type, depth + 1)?;
            }
            ensure_struct(space, ns, def)
        }
        DdsType::Union(def) => {
            ensure(space, ns, &def.discriminator, depth + 1)?;
            for case in &def.cases {
                ensure(space, ns, &case.member_type, depth + 1)?;
            }
            ensure_union(space, ns, def)
        }
        _ => Ok(()),
    }
}

/// The namespace-0 types the generated ones are subtypes of, so that a
/// client finds the generated DataTypes by browsing `HasSubtype` from
/// `BaseDataType`, `Structure` or `Enumeration` (as a client does that
/// loads the DataTypeDefinitions of a server).
fn ensure_base_types(space: &mut AddressSpace) {
    let data_types: [(u32, &str, Option<u32>); 5] = [
        (ns0::BASE_DATA_TYPE, "BaseDataType", None),
        (ns0::STRUCTURE, "Structure", Some(ns0::BASE_DATA_TYPE)),
        (ns0::ENUMERATION, "Enumeration", Some(ns0::BASE_DATA_TYPE)),
        (ns0::OPTION_SET, "OptionSet", Some(ns0::STRUCTURE)),
        (ns0::UNION, "Union", Some(ns0::STRUCTURE)),
    ];
    for (id, name, parent) in data_types {
        let node = NodeId::numeric(0, id);
        if space.node_meta(&node).is_some() {
            continue;
        }
        space.add_node(
            NodeMeta::new(node.clone(), NodeClass::DataType, ns0_name(name)).with_class_attributes(
                ClassAttributes::DataType(DataTypeAttributes {
                    is_abstract: true,
                    data_type_definition: None,
                }),
            ),
        );
        if let Some(parent) = parent {
            space.add_reference(
                NodeId::numeric(0, parent),
                reference_types::HAS_SUBTYPE,
                node,
            );
        }
    }
    let variable_types: [(u32, &str, Option<u32>); 2] = [
        (ns0::BASE_VARIABLE_TYPE, "BaseVariableType", None),
        (
            ns0::BASE_DATA_VARIABLE_TYPE,
            "BaseDataVariableType",
            Some(ns0::BASE_VARIABLE_TYPE),
        ),
    ];
    for (id, name, parent) in variable_types {
        let node = NodeId::numeric(0, id);
        if space.node_meta(&node).is_some() {
            continue;
        }
        space.add_node(
            NodeMeta::new(node.clone(), NodeClass::VariableType, ns0_name(name))
                .with_class_attributes(ClassAttributes::VariableType(VariableTypeAttributes {
                    is_abstract: id == ns0::BASE_VARIABLE_TYPE,
                    ..VariableTypeAttributes::default()
                })),
        );
        if let Some(parent) = parent {
            space.add_reference(
                NodeId::numeric(0, parent),
                reference_types::HAS_SUBTYPE,
                node,
            );
        }
    }
}

fn ns0_name(name: &str) -> QualifiedName {
    QualifiedName {
        namespace_index: 0,
        name: name.to_string(),
    }
}

fn text(value: &str) -> LocalizedText {
    LocalizedText {
        locale: None,
        text: Some(value.to_string()),
    }
}

fn no_text() -> LocalizedText {
    LocalizedText {
        locale: None,
        text: None,
    }
}

/// The ExtensionObject of a value in its binary encoding.
fn extension_object<T: UaEncode>(type_id: u32, value: &T) -> ExtensionObject {
    ExtensionObject {
        type_id: NodeId::numeric(0, type_id),
        body: ExtensionObjectBody::ByteString(
            zerodds_opcua_pubsub::to_binary(value).unwrap_or_default(),
        ),
    }
}

/// The field of a structure (or the case of a union) `name` of `ty`.
fn field(ns: u16, name: &str, ty: &DdsType, is_optional: bool) -> StructureField {
    let shape = shape_of(ns, ty);
    StructureField {
        name: name.to_string(),
        description: no_text(),
        data_type: shape.data_type,
        value_rank: shape.value_rank,
        array_dimensions: shape.array_dimensions,
        max_string_length: 0,
        is_optional,
    }
}

/// The `StructureDefinition` of a structure (Table 9.15): a field per
/// member, in the order of the members; a structure with an `@optional`
/// member is a `StructureWithOptionalFields` (OPC 10000-6 §5.2.7).
#[must_use]
pub fn structure_definition(ns: u16, def: &StructDef) -> StructureDefinition {
    let optional = def.members.iter().any(|member| member.is_optional);
    StructureDefinition {
        default_encoding_id: encoding_id(ns, &def.name),
        base_data_type: NodeId::numeric(0, ns0::STRUCTURE),
        structure_type: if optional {
            StructureType::StructureWithOptionalFields
        } else {
            StructureType::Structure
        },
        fields: def
            .members
            .iter()
            .map(|member| field(ns, &member.name, &member.member_type, member.is_optional))
            .collect(),
    }
}

/// The `StructureDefinition` of a union (Table 9.20): a field per case, in
/// the order of the cases; the switch value of a case is its position,
/// counted from 1.
#[must_use]
pub fn union_definition(ns: u16, def: &UnionDef) -> StructureDefinition {
    StructureDefinition {
        default_encoding_id: encoding_id(ns, &def.name),
        base_data_type: NodeId::numeric(0, ns0::UNION),
        structure_type: StructureType::Union,
        fields: def
            .cases
            .iter()
            .map(|case| field(ns, &case.name, &case.member_type, false))
            .collect(),
    }
}

/// The `DataTypeDefinition` of a bitmask (Table 9.11): an
/// `EnumDefinition`, a field per flag whose `Value` is the number of its
/// bit (OPC 10000-3 §5.8.3, §8.52). The value of the bitmask is the
/// `OptionSet` structure, `Value` and `ValidBits` (§8.40).
#[must_use]
pub fn option_set_definition(def: &BitmaskDef) -> EnumDefinition {
    EnumDefinition {
        fields: def
            .bits
            .iter()
            .map(|(name, position)| EnumField {
                value: i64::from(*position),
                display_name: text(name),
                description: no_text(),
                name: name.clone(),
            })
            .collect(),
    }
}

/// The `EnumDefinition` of an enumeration (Table 9.7): a field per
/// enumerator with its value and its name.
#[must_use]
pub fn enum_definition(def: &EnumDef) -> EnumDefinition {
    EnumDefinition {
        fields: def
            .literals
            .iter()
            .map(|(name, value)| EnumField {
                value: *value,
                display_name: text(name),
                description: no_text(),
                name: name.clone(),
            })
            .collect(),
    }
}

/// Adds the DataType `type_name`, a subtype of `base`, with `definition`,
/// and its `Default Binary` encoding when `encoded`. `Ok(false)` when the
/// same DataType is there already.
fn ensure_data_type(
    space: &mut AddressSpace,
    ns: u16,
    type_name: &str,
    base: u32,
    definition: ExtensionObject,
    encoded: bool,
) -> Result<bool, TypeConflict> {
    let id = data_type_id(ns, type_name);
    if let Some(meta) = space.node_meta(&id) {
        return match &meta.class_attributes {
            ClassAttributes::DataType(attributes)
                if attributes.data_type_definition.as_ref() == Some(&definition) =>
            {
                Ok(false)
            }
            _ => Err(TypeConflict {
                type_name: type_name.to_string(),
            }),
        };
    }
    space.add_node(
        NodeMeta::new(
            id.clone(),
            NodeClass::DataType,
            QualifiedName {
                namespace_index: ns,
                name: data_type_name(simple(type_name)),
            },
        )
        .with_class_attributes(ClassAttributes::DataType(DataTypeAttributes {
            is_abstract: false,
            data_type_definition: Some(definition),
        })),
    );
    space.add_reference(
        NodeId::numeric(0, base),
        reference_types::HAS_SUBTYPE,
        id.clone(),
    );
    if encoded {
        let encoding = encoding_id(ns, type_name);
        let type_definition = NodeId::numeric(0, ns0::DATA_TYPE_ENCODING_TYPE);
        space.add_node(
            NodeMeta::new(
                encoding.clone(),
                NodeClass::Object,
                ns0_name("Default Binary"),
            )
            .with_type_definition(type_definition.clone()),
        );
        space.add_reference(
            encoding.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            type_definition,
        );
        space.add_reference(id, NodeId::numeric(0, ns0::HAS_ENCODING), encoding);
    }
    Ok(true)
}

/// Adds the Property `name` of the DataType `type_name`.
fn add_property(
    space: &mut AddressSpace,
    ns: u16,
    type_name: &str,
    name: &str,
    data_type: u32,
    value: Variant,
) {
    let owner = data_type_id(ns, type_name);
    let id = string_id(ns, std::format!("DataType:{type_name}/{name}"));
    let type_definition = NodeId::numeric(0, ns0::PROPERTY_TYPE);
    space.add_node(
        NodeMeta::new(id.clone(), NodeClass::Variable, ns0_name(name))
            .with_type_definition(type_definition.clone())
            .with_class_attributes(ClassAttributes::Variable(VariableAttributes {
                data_type: NodeId::numeric(0, data_type),
                value_rank: 1,
                array_dimensions: std::vec![0],
                access_level: access_level::CURRENT_READ,
                user_access_level: access_level::CURRENT_READ,
                access_level_ex: u32::from(access_level::CURRENT_READ),
                ..VariableAttributes::default()
            })),
    );
    space.set_value(id.clone(), DataValue::new_value(value, 0, 0));
    space.add_reference(
        id.clone(),
        reference_types::HAS_TYPE_DEFINITION,
        type_definition,
    );
    space.add_reference(owner, reference_types::HAS_PROPERTY, id);
}

fn ensure_enum(space: &mut AddressSpace, ns: u16, def: &EnumDef) -> Result<(), TypeConflict> {
    let definition = extension_object(ns0::ENUM_DEFINITION_BINARY, &enum_definition(def));
    if !ensure_data_type(space, ns, &def.name, ns0::ENUMERATION, definition, false)? {
        return Ok(());
    }
    // Table 9.7: the `EnumValues` Property, an `EnumValueType` per
    // enumerator (`Value`, `DisplayName`, `Description`).
    let values = def
        .literals
        .iter()
        .map(|(name, value)| {
            let mut w = zerodds_opcua_pubsub::UaWriter::with_capacity(32);
            w.write_i64(*value);
            let _ = text(name).encode(&mut w);
            let _ = no_text().encode(&mut w);
            VariantValue::ExtensionObject(ExtensionObject {
                type_id: NodeId::numeric(0, ns0::ENUM_VALUE_TYPE_BINARY),
                body: ExtensionObjectBody::ByteString(w.into_vec()),
            })
        })
        .collect::<Vec<_>>();
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    add_property(
        space,
        ns,
        &def.name,
        "EnumValues",
        ns0::ENUM_VALUE_TYPE,
        Variant {
            array_dimensions: std::vec![count],
            value: values,
        },
    );
    Ok(())
}

fn ensure_bitmask(space: &mut AddressSpace, ns: u16, def: &BitmaskDef) -> Result<(), TypeConflict> {
    let definition = extension_object(ns0::ENUM_DEFINITION_BINARY, &option_set_definition(def));
    if !ensure_data_type(space, ns, &def.name, ns0::OPTION_SET, definition, true)? {
        return Ok(());
    }
    // Table 9.11: an entry per position of the bound, the name of the flag
    // at it or `UndefinedPosition_<position>`.
    let values = (0..def.bound)
        .map(|position| {
            let name = def.bits.iter().find(|(_, at)| *at == position).map_or_else(
                || std::format!("UndefinedPosition_{position}"),
                |(name, _)| name.clone(),
            );
            VariantValue::LocalizedText(text(&name))
        })
        .collect::<Vec<_>>();
    add_property(
        space,
        ns,
        &def.name,
        "OptionSetValues",
        ns0::LOCALIZED_TEXT,
        Variant {
            array_dimensions: std::vec![def.bound],
            value: values,
        },
    );
    Ok(())
}

fn ensure_union(space: &mut AddressSpace, ns: u16, def: &UnionDef) -> Result<(), TypeConflict> {
    let definition = extension_object(ns0::STRUCTURE_DEFINITION_BINARY, &union_definition(ns, def));
    ensure_data_type(space, ns, &def.name, ns0::UNION, definition, true).map(|_| ())
}

fn ensure_struct(space: &mut AddressSpace, ns: u16, def: &StructDef) -> Result<(), TypeConflict> {
    let definition = extension_object(
        ns0::STRUCTURE_DEFINITION_BINARY,
        &structure_definition(ns, def),
    );
    if !ensure_data_type(space, ns, &def.name, ns0::STRUCTURE, definition, true)? {
        return Ok(());
    }
    // Table 9.16: `<Name>VariableType`, a subtype of BaseDataVariableType of
    // the structure's DataType, with a component Variable per member.
    let variable_type = variable_type_id(ns, &def.name);
    space.add_node(
        NodeMeta::new(
            variable_type.clone(),
            NodeClass::VariableType,
            QualifiedName {
                namespace_index: ns,
                name: variable_type_name(simple(&def.name)),
            },
        )
        .with_class_attributes(ClassAttributes::VariableType(VariableTypeAttributes {
            is_abstract: false,
            data_type: data_type_id(ns, &def.name),
            value_rank: -1,
            array_dimensions: Vec::new(),
        })),
    );
    space.add_reference(
        NodeId::numeric(0, ns0::BASE_DATA_VARIABLE_TYPE),
        reference_types::HAS_SUBTYPE,
        variable_type.clone(),
    );
    for member in &def.members {
        let id = string_id(
            ns,
            std::format!("VariableType:{}/{}", def.name, member.name),
        );
        let shape = shape_of(ns, &member.member_type);
        let type_definition = NodeId::numeric(0, ns0::BASE_DATA_VARIABLE_TYPE);
        space.add_node(
            NodeMeta::new(
                id.clone(),
                NodeClass::Variable,
                QualifiedName {
                    namespace_index: ns,
                    name: member.name.clone(),
                },
            )
            .with_type_definition(type_definition.clone())
            .with_class_attributes(ClassAttributes::Variable(VariableAttributes {
                data_type: shape.data_type,
                value_rank: shape.value_rank,
                array_dimensions: shape.array_dimensions,
                access_level: access_level::CURRENT_READ,
                user_access_level: access_level::CURRENT_READ,
                access_level_ex: u32::from(access_level::CURRENT_READ),
                ..VariableAttributes::default()
            })),
        );
        space.add_reference(
            variable_type.clone(),
            reference_types::HAS_COMPONENT,
            id.clone(),
        );
        space.add_reference(
            id.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            type_definition,
        );
        // Table 9.16: "Optional" for an optional member, "Mandatory" for
        // every other.
        space.add_reference(
            id,
            NodeId::numeric(0, ns0::HAS_MODELLING_RULE),
            NodeId::numeric(
                0,
                if member.is_optional {
                    ns0::MODELLING_RULE_OPTIONAL
                } else {
                    ns0::MODELLING_RULE_MANDATORY
                },
            ),
        );
    }
    Ok(())
}
