// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The value of an aggregate Variable (Gateway 1.0 §9.2.4, §9.2.5): a
//! structure, a union, a bitset or a bitmask as an ExtensionObject in the
//! OPC UA binary encoding (OPC 10000-6 §5.2.6 to §5.2.8), laid out as the
//! `DataTypeDefinition`s of [`crate::ua_types`] describe it; a collection of
//! structures as an array of them; a `map` as an array of `KeyValuePair`.
//!
//! [`aggregate_values`] gives the value of every aggregate Variable of a
//! sample, path-named as [`crate::dynamic_value::flatten`] names the
//! leaves. [`replace_at`] puts a value a client wrote to an aggregate
//! Variable into the sample.
//!
//! The layout, field by field:
//! * a structure: its members in the order of the type; with an
//!   `@optional` member, first a UInt32 mask, a bit per optional member,
//!   set when it is present (§5.2.7);
//! * a union: the UInt32 switch, the position of the active case counted
//!   from 1 (0 when no case is active), then the case (§5.2.8, Gateway
//!   Table 9.20);
//! * a bitset: its named bitfields, each as the number of its holder type;
//! * a bitmask: the `OptionSet` structure, `Value` and `ValidBits`;
//! * an enumeration: its value as an Int32 (§5.2.4);
//! * `fixed<P,S>`: a `Decimal` ExtensionObject (§5.1.10);
//! * an array or a sequence: the Int32 count and the elements; an array of
//!   several dimensions as the inline matrix (§5.2.5);
//! * a `map`: an array of `KeyValuePair`, the key as a QualifiedName of its
//!   text, the value as a Variant;
//! * the other types: their built-in type.

use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_ast::PrimitiveKind;
use zerodds_opcua_gateway::data_value::{
    ExtensionObject, ExtensionObjectBody, Variant, VariantValue,
};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::QualifiedName;
use zerodds_opcua_pubsub::{UaDecode, UaEncode, UaReader, UaWriter};
use zerodds_types::dynamic::{DynamicData, DynamicType, DynamicTypeMember, TypeKind};

use crate::dynamic_value::{
    ConvertError, FlatField, array_dims, bits_from_option_set, collection_from_variant,
    default_data, element_name, element_type_name, flat_items, is_collection, is_optional,
    map_key_string, nest, option_set_bits, parse_map_key, resolved_alias, scalar_from_variant,
    scalar_value,
};
use crate::ua_types::{encoding_id, ns0};

/// The recursion bound, the one the type is built with.
const MAX_DEPTH: usize = 64;

/// Writes a ByteString (Int32 length, then the octets).
pub(crate) fn write_byte_string(w: &mut UaWriter, bytes: &[u8]) {
    w.write_i32(i32::try_from(bytes.len()).unwrap_or(i32::MAX));
    w.write_bytes(bytes);
}

fn read_byte_string(r: &mut UaReader<'_>, path: &str) -> Result<Vec<u8>, ConvertError> {
    let len = r.read_i32().map_err(|e| decode_error(path, &e))?;
    if len < 0 {
        return Ok(Vec::new());
    }
    Ok(r.read_bytes(len as usize)
        .map_err(|e| decode_error(path, &e))?
        .to_vec())
}

fn decode_error(path: &str, error: &dyn core::fmt::Debug) -> ConvertError {
    ConvertError::ShapeMismatch(format!("{path}: {error:?}"))
}

/// The `Value` and the `ValidBits` of an `OptionSet` ExtensionObject.
///
/// # Errors
/// [`ConvertError::ShapeMismatch`] for an ExtensionObject with no binary
/// body or one that is no `OptionSet`.
pub(crate) fn option_set_of(
    object: &ExtensionObject,
    path: &str,
) -> Result<(Vec<u8>, Vec<u8>), ConvertError> {
    let ExtensionObjectBody::ByteString(body) = &object.body else {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: an OptionSet with no binary body"
        )));
    };
    let mut r = UaReader::new(body);
    let value = read_byte_string(&mut r, path)?;
    let valid = read_byte_string(&mut r, path)?;
    Ok((value, valid))
}

/// The label value of a discriminator value: its number, the value of an
/// enumerator, `0`/`1` for a boolean, the code of a character.
pub(crate) fn label_value(
    discriminator_type: &DynamicType,
    data: &DynamicData,
    path: &str,
) -> Result<i64, ConvertError> {
    match data {
        DynamicData::Integer { value, .. } => i64::try_from(*value).map_err(|_| {
            ConvertError::ShapeMismatch(format!("{path}: a discriminator past 64 bits"))
        }),
        DynamicData::Boolean(value) => Ok(i64::from(*value)),
        DynamicData::Character { value, .. } => {
            Ok(value.chars().next().map_or(0, |c| i64::from(u32::from(c))))
        }
        DynamicData::Enumerator(name) => discriminator_type
            .member_by_name(name)
            .and_then(|member| member.literal_value())
            .map(i64::from)
            .ok_or_else(|| ConvertError::UnknownMember {
                path: path.to_string(),
                name: name.clone(),
            }),
        other => Err(ConvertError::ShapeMismatch(format!(
            "{path}: {other:?} is no discriminator value"
        ))),
    }
}

/// The discriminator value of the label `value`, as `discriminator_type`
/// holds it.
pub(crate) fn discriminator_data(
    discriminator_type: &DynamicType,
    value: i64,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let integer = |kind: PrimitiveKind| {
        Ok(DynamicData::Integer {
            kind: Some(kind),
            value: i128::from(value),
        })
    };
    match discriminator_type.kind() {
        TypeKind::Boolean => Ok(DynamicData::Boolean(value != 0)),
        TypeKind::Int8 => integer(PrimitiveKind::Int8),
        TypeKind::UInt8 | TypeKind::Byte => integer(PrimitiveKind::UInt8),
        TypeKind::Int16 => integer(PrimitiveKind::Int16),
        TypeKind::UInt16 => integer(PrimitiveKind::UInt16),
        TypeKind::Int32 => integer(PrimitiveKind::Int32),
        TypeKind::UInt32 => integer(PrimitiveKind::UInt32),
        TypeKind::Int64 => integer(PrimitiveKind::Int64),
        TypeKind::UInt64 => integer(PrimitiveKind::UInt64),
        TypeKind::Char8 | TypeKind::Char16 => {
            let character = u32::try_from(value)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| {
                    ConvertError::ShapeMismatch(format!("{path}: {value} is no character"))
                })?;
            Ok(DynamicData::character(
                if matches!(discriminator_type.kind(), TypeKind::Char8) {
                    PrimitiveKind::Char
                } else {
                    PrimitiveKind::WChar
                },
                character,
            ))
        }
        TypeKind::Enumeration => discriminator_type
            .members()
            .into_iter()
            .find(|member| member.literal_value().map(i64::from) == Some(value))
            .map(|member| DynamicData::Enumerator(member.name().to_string()))
            .ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: {value} names no enumerator"))
            }),
        other => Err(ConvertError::ShapeMismatch(format!(
            "{path}: a discriminator of kind {other:?}"
        ))),
    }
}

/// The case of the union `ty` the label `value` selects: the case whose
/// labels hold it, else the default case.
pub(crate) fn case_of(ty: &DynamicType, value: i64) -> Option<DynamicTypeMember> {
    let members = ty.members();
    let labelled = members.iter().find(|member| {
        member
            .descriptor()
            .is_ok_and(|descriptor| descriptor.label.contains(&value))
    });
    labelled
        .or_else(|| {
            members.iter().find(|member| {
                member
                    .descriptor()
                    .is_ok_and(|descriptor| descriptor.is_default_label)
            })
        })
        .cloned()
}

/// A label value that selects `case`: its first label, or, for the default
/// case, the smallest non-negative value no other case names.
pub(crate) fn label_of(
    ty: &DynamicType,
    case: &DynamicTypeMember,
    path: &str,
) -> Result<i64, ConvertError> {
    let descriptor = case
        .descriptor()
        .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e:?}")))?;
    if let Some(first) = descriptor.label.first() {
        return Ok(*first);
    }
    let taken: Vec<i64> = ty
        .members()
        .iter()
        .filter_map(|member| member.descriptor().ok())
        .flat_map(|descriptor| descriptor.label)
        .collect();
    let discriminator = ty
        .discriminator_type()
        .and_then(|d| resolved_alias(&d, path).ok());
    let candidates: Vec<i64> = match discriminator.as_ref().map(DynamicType::kind) {
        Some(TypeKind::Enumeration) => discriminator
            .iter()
            .flat_map(DynamicType::members)
            .filter_map(|member| member.literal_value().map(i64::from))
            .collect(),
        Some(TypeKind::Boolean) => std::vec![0, 1],
        _ => (0..=i64::from(u16::MAX)).collect(),
    };
    candidates
        .into_iter()
        .find(|value| !taken.contains(value))
        .ok_or_else(|| {
            ConvertError::ShapeMismatch(format!(
                "{path}: no discriminator value is left for the default case"
            ))
        })
}

/// The ExtensionObject of a value of the structure, union, bitset or
/// bitmask `ty`, its TypeId the type's `Default Binary` encoding in `ns`.
///
/// # Errors
/// [`ConvertError`] when the value does not fit the type, or holds a
/// collection of collections, which no field of a `StructureDefinition`
/// describes.
pub fn to_extension_object(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
    path: &str,
) -> Result<ExtensionObject, ConvertError> {
    let ty = resolved_alias(ty, path)?;
    let mut w = UaWriter::with_capacity(64);
    encode_field(&ty, data, ns, &mut w, path, 0)?;
    Ok(ExtensionObject {
        type_id: encoding_id(ns, &ty.qualified_name()),
        body: ExtensionObjectBody::ByteString(w.into_vec()),
    })
}

/// The value an ExtensionObject of the structure, union, bitset or bitmask
/// `ty` holds.
///
/// # Errors
/// [`ConvertError::ShapeMismatch`] for an ExtensionObject of another type,
/// one with no binary body, one that ends early or has octets past the
/// value.
pub fn from_extension_object(
    ty: &DynamicType,
    object: &ExtensionObject,
    ns: u16,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let ty = resolved_alias(ty, path)?;
    let expected = encoding_id(ns, &ty.qualified_name());
    if object.type_id != expected {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: an ExtensionObject of {}, not of {}",
            object.type_id.to_canonical_string(),
            expected.to_canonical_string()
        )));
    }
    let ExtensionObjectBody::ByteString(body) = &object.body else {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: an ExtensionObject with no binary body"
        )));
    };
    let mut r = UaReader::new(body);
    let data = decode_field(&ty, &mut r, ns, path, 0)?;
    if !r.is_empty() {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: {} octets past the value",
            r.remaining()
        )));
    }
    Ok(data)
}

/// The Variant of a value of `ty`: a structure, union, bitset or bitmask
/// as its ExtensionObject, a `map` as an array of `KeyValuePair`, a
/// collection as an array of its elements, a scalar as its built-in type.
///
/// # Errors
/// As [`to_extension_object`].
pub fn variant_of(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
    path: &str,
) -> Result<Variant, ConvertError> {
    variant_of_at(ty, data, ns, path, 0)
}

/// [`variant_of`] at a nesting of `depth` collections.
///
/// zerodds-lint: recursion-depth 64 (one level per collection whose elements
/// are collections; refused past `MAX_DEPTH`)
fn variant_of_at(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
    path: &str,
    depth: usize,
) -> Result<Variant, ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    let ty = resolved_alias(ty, path)?;
    match ty.kind() {
        TypeKind::Structure | TypeKind::Union | TypeKind::Bitset | TypeKind::Bitmask => {
            Ok(Variant::scalar(VariantValue::ExtensionObject(
                to_extension_object(&ty, data, ns, path)?,
            )))
        }
        TypeKind::Map => {
            let pairs = map_pairs(&ty, data, ns, path)?;
            Ok(Variant {
                array_dimensions: std::vec![u32::try_from(pairs.len()).unwrap_or(u32::MAX)],
                value: pairs
                    .into_iter()
                    .map(VariantValue::ExtensionObject)
                    .collect(),
            })
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element = collection_element(&ty, path)?;
            let dims = array_dims(&ty, path)?;
            let items = flat_items(data, dims.len().max(1), path)?;
            let mut values = Vec::with_capacity(items.len());
            for item in &items {
                let element_value = variant_of_at(&element, item, ns, path, depth + 1)?;
                values.extend(element_value.value);
            }
            Ok(Variant {
                array_dimensions: if dims.is_empty() {
                    std::vec![u32::try_from(items.len()).unwrap_or(u32::MAX)]
                } else {
                    dims
                },
                value: values,
            })
        }
        _ => {
            let mut skipped = Vec::new();
            Ok(Variant::scalar(scalar_value(
                &ty,
                data,
                path,
                ns,
                &mut skipped,
            )?))
        }
    }
}

/// The value of `ty` a Variant holds; the inverse of [`variant_of`].
///
/// zerodds-lint: recursion-depth 2 (it calls itself only for the element of
/// a collection whose elements are structures, unions, bitsets or maps, and
/// those kinds return without calling it)
///
/// # Errors
/// [`ConvertError`] when the Variant does not hold a value of the type.
pub fn data_of_variant(
    ty: &DynamicType,
    variant: &Variant,
    ns: u16,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let ty = resolved_alias(ty, path)?;
    let object = |value: Option<&VariantValue>| match value {
        Some(VariantValue::ExtensionObject(object)) => Ok(object.clone()),
        other => Err(ConvertError::ShapeMismatch(format!(
            "{path}: expected an ExtensionObject, got {other:?}"
        ))),
    };
    match ty.kind() {
        TypeKind::Structure | TypeKind::Union | TypeKind::Bitset => {
            from_extension_object(&ty, &object(variant.value.first())?, ns, path)
        }
        TypeKind::Map => {
            let objects = variant
                .value
                .iter()
                .map(|value| object(Some(value)))
                .collect::<Result<Vec<_>, _>>()?;
            map_from_pairs(&ty, &objects, ns, path)
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element = collection_element(&ty, path)?;
            if matches!(
                element.kind(),
                TypeKind::Structure | TypeKind::Union | TypeKind::Bitset | TypeKind::Map
            ) {
                let dims = array_dims(&ty, path)?;
                let items = variant
                    .value
                    .iter()
                    .map(|value| {
                        data_of_variant(&element, &Variant::scalar(value.clone()), ns, path)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                check_array_count(&dims, items.len(), path)?;
                return Ok(nest(items, &dims));
            }
            collection_from_variant(&ty, &element, variant, path)
        }
        _ => scalar_from_variant(&ty, variant, path),
    }
}

/// An array takes as many elements as its dimensions hold.
fn check_array_count(dims: &[u32], count: usize, path: &str) -> Result<(), ConvertError> {
    if dims.is_empty() {
        return Ok(());
    }
    let expected: u64 = dims.iter().map(|dim| u64::from(*dim)).product();
    if count as u64 != expected {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: the array has {expected} elements, the value written {count}"
        )));
    }
    Ok(())
}

/// The element type of a collection, refusing a collection of collections:
/// no field of a `StructureDefinition` and no Variant describes one.
fn collection_element(ty: &DynamicType, path: &str) -> Result<DynamicType, ConvertError> {
    let element = ty.element_type().ok_or_else(|| {
        ConvertError::ShapeMismatch(format!("{path}: a collection with no element type"))
    })?;
    let element = resolved_alias(&element, path)?;
    if is_collection(&element) {
        return Err(ConvertError::Unsupported {
            path: path.to_string(),
            reason: "a collection of collections is no Variant (a Variant holds no Variant here)",
        });
    }
    Ok(element)
}

/// The element type of a collection in a structure field: a collection may
/// hold collections there, each element a Variant.
fn field_element(ty: &DynamicType, path: &str) -> Result<DynamicType, ConvertError> {
    let element = ty.element_type().ok_or_else(|| {
        ConvertError::ShapeMismatch(format!("{path}: a collection with no element type"))
    })?;
    resolved_alias(&element, path)
}

/// The entries of a `map` as `KeyValuePair` ExtensionObjects.
fn map_pairs(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
    path: &str,
) -> Result<Vec<ExtensionObject>, ConvertError> {
    let (key_ty, value_ty) = map_types(ty, path)?;
    let Some(entries) = data.items() else {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: expected a map value"
        )));
    };
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some([key, value]) = entry.items() else {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: a map entry must be a [key, value] collection"
            )));
        };
        let mut w = UaWriter::with_capacity(32);
        encode_pair(&key_ty, &value_ty, key, value, ns, &mut w, path)?;
        out.push(ExtensionObject {
            type_id: NodeId::numeric(0, ns0::KEY_VALUE_PAIR_BINARY),
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        });
    }
    Ok(out)
}

fn map_types(ty: &DynamicType, path: &str) -> Result<(DynamicType, DynamicType), ConvertError> {
    let key = ty
        .key_element_type()
        .ok_or_else(|| ConvertError::ShapeMismatch(format!("{path}: a map with no key type")))?;
    let value = ty
        .element_type()
        .ok_or_else(|| ConvertError::ShapeMismatch(format!("{path}: a map with no value type")))?;
    Ok((resolved_alias(&key, path)?, value))
}

/// One `KeyValuePair` (OPC 10000-5 §12.21): the key as a QualifiedName of
/// its text in namespace 0, the value as a Variant.
fn encode_pair(
    key_ty: &DynamicType,
    value_ty: &DynamicType,
    key: &DynamicData,
    value: &DynamicData,
    ns: u16,
    w: &mut UaWriter,
    path: &str,
) -> Result<(), ConvertError> {
    let key_text = map_key_string(key_ty, key, path)?;
    let entry_path = std::format!("{path}.{key_text}");
    QualifiedName {
        namespace_index: 0,
        name: key_text,
    }
    .encode(w)
    .map_err(|e| decode_error(&entry_path, &e))?;
    variant_of(value_ty, value, ns, &entry_path)?
        .encode(w)
        .map_err(|e| decode_error(&entry_path, &e))
}

fn decode_pair(
    key_ty: &DynamicType,
    value_ty: &DynamicType,
    r: &mut UaReader<'_>,
    ns: u16,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let key = QualifiedName::decode(r).map_err(|e| decode_error(path, &e))?;
    let entry_path = std::format!("{path}.{}", key.name);
    let value = Variant::decode(r).map_err(|e| decode_error(&entry_path, &e))?;
    Ok(DynamicData::Collection(std::vec![
        parse_map_key(key_ty, &key.name, path)?,
        data_of_variant(value_ty, &value, ns, &entry_path)?,
    ]))
}

fn map_from_pairs(
    ty: &DynamicType,
    objects: &[ExtensionObject],
    ns: u16,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let (key_ty, value_ty) = map_types(ty, path)?;
    let mut entries = Vec::with_capacity(objects.len());
    for object in objects {
        if object.type_id != NodeId::numeric(0, ns0::KEY_VALUE_PAIR_BINARY) {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: a map entry that is no KeyValuePair"
            )));
        }
        let ExtensionObjectBody::ByteString(body) = &object.body else {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: a KeyValuePair with no binary body"
            )));
        };
        let mut r = UaReader::new(body);
        entries.push(decode_pair(&key_ty, &value_ty, &mut r, ns, path)?);
    }
    Ok(DynamicData::Collection(entries))
}

/// Writes the value of `ty` as a field of a structure.
///
/// zerodds-lint: recursion-depth 64 (one level per nested member, case,
/// element or alias; refused past `MAX_DEPTH`)
#[allow(clippy::too_many_lines)]
fn encode_field(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
    w: &mut UaWriter,
    path: &str,
    depth: usize,
) -> Result<(), ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    let ty = resolved_alias(ty, path)?;
    match ty.kind() {
        TypeKind::Structure => {
            let members = ty.members();
            let optional: Vec<&DynamicTypeMember> = members
                .iter()
                .filter(|member| is_optional(member))
                .collect();
            if !optional.is_empty() {
                let mut mask = 0u32;
                for (bit, member) in optional.iter().enumerate().take(32) {
                    if data.member(member.name()).is_some() {
                        mask |= 1 << bit;
                    }
                }
                w.write_u32(mask);
            }
            for member in &members {
                let member_path = std::format!("{path}.{}", member.name());
                let Some(value) = data.member(member.name()) else {
                    if is_optional(member) {
                        continue;
                    }
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: the value has no member `{}`",
                        member.name()
                    )));
                };
                let member_type = member
                    .dynamic_type()
                    .map_err(|e| decode_error(&member_path, &e))?;
                encode_field(&member_type, value, ns, w, &member_path, depth + 1)?;
            }
            Ok(())
        }
        TypeKind::Union => {
            let active = data
                .members()
                .unwrap_or(&[])
                .iter()
                .find(|(name, _)| name != "_d");
            let Some((name, value)) = active else {
                w.write_u32(0);
                return Ok(());
            };
            let members = ty.members();
            let Some(position) = members.iter().position(|member| member.name() == name) else {
                return Err(ConvertError::UnknownMember {
                    path: path.to_string(),
                    name: name.clone(),
                });
            };
            w.write_u32(u32::try_from(position + 1).unwrap_or(u32::MAX));
            let case_path = std::format!("{path}.{name}");
            let case_type = members[position]
                .dynamic_type()
                .map_err(|e| decode_error(&case_path, &e))?;
            encode_field(&case_type, value, ns, w, &case_path, depth + 1)
        }
        TypeKind::Bitset => {
            let DynamicData::Bitfields(values) = data else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected the bitfields of a bitset"
                )));
            };
            for (name, kind) in crate::dynamic_type::bitfields(&ty) {
                let number = match values.iter().find(|(known, _)| *known == name) {
                    Some((_, DynamicData::Integer { value, .. })) => *value,
                    _ => 0,
                };
                crate::dynamic_value::bitfield_value(kind, number)
                    .encode(w)
                    .map_err(|e| decode_error(path, &e))?;
            }
            Ok(())
        }
        TypeKind::Bitmask => {
            let mut skipped = Vec::new();
            let (value, valid) = option_set_bits(&ty, data, path, &mut skipped)?;
            write_byte_string(w, &value);
            write_byte_string(w, &valid);
            Ok(())
        }
        TypeKind::Map => {
            let (key_ty, value_ty) = map_types(&ty, path)?;
            let entries = data.items().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: expected a map value"))
            })?;
            w.write_i32(i32::try_from(entries.len()).unwrap_or(i32::MAX));
            for entry in entries {
                let Some([key, value]) = entry.items() else {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: a map entry must be a [key, value] collection"
                    )));
                };
                encode_pair(&key_ty, &value_ty, key, value, ns, w, path)?;
            }
            Ok(())
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element = field_element(&ty, path)?;
            let nested = is_collection(&element);
            let dims = array_dims(&ty, path)?;
            let items = flat_items(data, dims.len().max(1), path)?;
            if dims.len() > 1 {
                // §5.2.5 Table 28: the dimensions, then the values.
                w.write_i32(i32::try_from(dims.len()).unwrap_or(i32::MAX));
                for dim in &dims {
                    w.write_i32(i32::try_from(*dim).unwrap_or(i32::MAX));
                }
            } else {
                w.write_i32(i32::try_from(items.len()).unwrap_or(i32::MAX));
            }
            for item in items {
                if nested {
                    // A collection of collections: each element a Variant
                    // of one inner collection (the field's DataType is
                    // BaseDataType, OPC 10000-6 §5.2.6).
                    variant_of(&element, item, ns, path)?
                        .encode(w)
                        .map_err(|e| decode_error(path, &e))?;
                } else {
                    encode_field(&element, item, ns, w, path, depth + 1)?;
                }
            }
            Ok(())
        }
        _ => {
            let mut skipped = Vec::new();
            scalar_value(&ty, data, path, ns, &mut skipped)?
                .encode(w)
                .map_err(|e| decode_error(path, &e))
        }
    }
}

/// Reads a value of `ty` written as a field of a structure.
///
/// zerodds-lint: recursion-depth 64 (one level per nested member, case,
/// element or alias; refused past `MAX_DEPTH`)
#[allow(clippy::too_many_lines)]
fn decode_field(
    ty: &DynamicType,
    r: &mut UaReader<'_>,
    ns: u16,
    path: &str,
    depth: usize,
) -> Result<DynamicData, ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    let ty = resolved_alias(ty, path)?;
    let err = |e: zerodds_opcua_pubsub::DecodeError| decode_error(path, &e);
    match ty.kind() {
        TypeKind::Structure => {
            let members = ty.members();
            let optional_count = members.iter().filter(|member| is_optional(member)).count();
            let mask = if optional_count > 0 {
                let mask = r.read_u32().map_err(err)?;
                let used = if optional_count >= 32 {
                    u32::MAX
                } else {
                    (1u32 << optional_count) - 1
                };
                // §5.2.7: "Decoders shall report an error if unassigned bits
                // are not 0."
                if mask & !used != 0 {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: the encoding mask sets bits no optional field has"
                    )));
                }
                mask
            } else {
                0
            };
            let mut out = Vec::with_capacity(members.len());
            let mut bit = 0;
            for member in &members {
                let member_path = std::format!("{path}.{}", member.name());
                if is_optional(member) {
                    let present = bit < 32 && mask & (1 << bit) != 0;
                    bit += 1;
                    if !present {
                        continue;
                    }
                }
                let member_type = member
                    .dynamic_type()
                    .map_err(|e| decode_error(&member_path, &e))?;
                out.push((
                    member.name().to_string(),
                    decode_field(&member_type, r, ns, &member_path, depth + 1)?,
                ));
            }
            Ok(DynamicData::Aggregate(out))
        }
        TypeKind::Union => {
            let switch = r.read_u32().map_err(err)?;
            let members = ty.members();
            let discriminator_type = ty.discriminator_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: a union with no discriminator type"))
            })?;
            let discriminator_type = resolved_alias(&discriminator_type, path)?;
            if switch == 0 {
                // No case: a discriminator no case and no default selects.
                if members.iter().any(|member| {
                    member
                        .descriptor()
                        .is_ok_and(|descriptor| descriptor.is_default_label)
                }) {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: a union with a default case has always a case"
                    )));
                }
                let taken: Vec<i64> = members
                    .iter()
                    .filter_map(|member| member.descriptor().ok())
                    .flat_map(|descriptor| descriptor.label)
                    .collect();
                let free = (0..=i64::from(u16::MAX))
                    .find(|value| !taken.contains(value))
                    .unwrap_or(0);
                return Ok(DynamicData::Aggregate(std::vec![(
                    "_d".to_string(),
                    discriminator_data(&discriminator_type, free, path)?,
                )]));
            }
            let Some(case) = members.get(switch as usize - 1) else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: switch {switch} past the {} cases",
                    members.len()
                )));
            };
            let case_path = std::format!("{path}.{}", case.name());
            let label = label_of(&ty, case, path)?;
            let case_type = case
                .dynamic_type()
                .map_err(|e| decode_error(&case_path, &e))?;
            Ok(DynamicData::Aggregate(std::vec![
                (
                    "_d".to_string(),
                    discriminator_data(&discriminator_type, label, path)?
                ),
                (
                    case.name().to_string(),
                    decode_field(&case_type, r, ns, &case_path, depth + 1)?,
                ),
            ]))
        }
        TypeKind::Bitset => {
            let mut out = Vec::new();
            for (name, kind) in crate::dynamic_type::bitfields(&ty) {
                let number: i128 = match kind {
                    TypeKind::Boolean => i128::from(r.read_u8().map_err(err)? != 0),
                    TypeKind::Int8 => i128::from(r.read_u8().map_err(err)? as i8),
                    TypeKind::Int16 => i128::from(r.read_i16().map_err(err)?),
                    TypeKind::UInt16 => i128::from(r.read_u16().map_err(err)?),
                    TypeKind::Int32 => i128::from(r.read_i32().map_err(err)?),
                    TypeKind::UInt32 => i128::from(r.read_u32().map_err(err)?),
                    TypeKind::Int64 => i128::from(r.read_i64().map_err(err)?),
                    TypeKind::UInt64 => i128::from(r.read_u64().map_err(err)?),
                    _ => i128::from(r.read_u8().map_err(err)?),
                };
                out.push((
                    name,
                    DynamicData::Integer {
                        kind: None,
                        value: number,
                    },
                ));
            }
            Ok(DynamicData::Bitfields(out))
        }
        TypeKind::Bitmask => {
            let value = read_byte_string(r, path)?;
            let valid = read_byte_string(r, path)?;
            bits_from_option_set(&ty, &value, &valid, None, path)
        }
        TypeKind::Map => {
            let (key_ty, value_ty) = map_types(&ty, path)?;
            let count = r.read_i32().map_err(err)?;
            let mut entries = Vec::new();
            for _ in 0..count.max(0) {
                entries.push(decode_pair(&key_ty, &value_ty, r, ns, path)?);
            }
            Ok(DynamicData::Collection(entries))
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element = field_element(&ty, path)?;
            let nested = is_collection(&element);
            let dims = array_dims(&ty, path)?;
            let count = if dims.len() > 1 {
                let elements = dims.iter().map(|dim| *dim as usize).product::<usize>();
                let first = r.read_i32().map_err(err)?;
                if usize::try_from(first).ok() == Some(elements) && elements != dims.len() {
                    // The one-dimensional form an encoder of OPC 10000-6
                    // before 1.05 writes (§5.2.5: "The inline matrix
                    // representation is not supported by earlier versions
                    // of this specification"): the count of all elements.
                    elements
                } else {
                    let mut written = Vec::new();
                    for _ in 0..first.clamp(0, 8) {
                        written.push(r.read_i32().map_err(err)?);
                    }
                    let expected: Vec<i32> = dims
                        .iter()
                        .map(|dim| i32::try_from(*dim).unwrap_or(i32::MAX))
                        .collect();
                    if usize::try_from(first).ok() != Some(dims.len()) || written != expected {
                        return Err(ConvertError::ShapeMismatch(format!(
                            "{path}: an array of dimensions {written:?}, not {expected:?}"
                        )));
                    }
                    elements
                }
            } else {
                usize::try_from(r.read_i32().map_err(err)?.max(0)).unwrap_or(0)
            };
            check_array_count(&dims, count, path)?;
            // Each element takes at least one octet: a count past what is
            // left is no value of this type.
            if count > r.remaining() {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: {count} elements in {} octets",
                    r.remaining()
                )));
            }
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(if nested {
                    let variant = Variant::decode(r).map_err(err)?;
                    data_of_variant(&element, &variant, ns, path)?
                } else {
                    decode_field(&element, r, ns, path, depth + 1)?
                });
            }
            Ok(nest(items, &dims))
        }
        kind => {
            let value = match kind {
                TypeKind::Boolean => VariantValue::Boolean(r.read_u8().map_err(err)? != 0),
                TypeKind::Int8 => VariantValue::SByte(r.read_u8().map_err(err)? as i8),
                TypeKind::UInt8 | TypeKind::Byte | TypeKind::Char8 => {
                    VariantValue::Byte(r.read_u8().map_err(err)?)
                }
                TypeKind::Int16 => VariantValue::Int16(r.read_i16().map_err(err)?),
                TypeKind::UInt16 | TypeKind::Char16 => {
                    VariantValue::UInt16(r.read_u16().map_err(err)?)
                }
                TypeKind::Int32 | TypeKind::Enumeration => {
                    VariantValue::Int32(r.read_i32().map_err(err)?)
                }
                TypeKind::UInt32 => VariantValue::UInt32(r.read_u32().map_err(err)?),
                TypeKind::Int64 => VariantValue::Int64(r.read_i64().map_err(err)?),
                TypeKind::UInt64 => VariantValue::UInt64(r.read_u64().map_err(err)?),
                TypeKind::Float32 => VariantValue::Float(r.read_f32().map_err(err)?),
                TypeKind::Float64 => VariantValue::Double(r.read_f64().map_err(err)?),
                TypeKind::String8 | TypeKind::String16 => {
                    let octets = read_byte_string(r, path)?;
                    VariantValue::String(String::from_utf8(octets).map_err(|_| {
                        ConvertError::ShapeMismatch(format!("{path}: a String that is no UTF-8"))
                    })?)
                }
                TypeKind::Float128 => VariantValue::ByteString(read_byte_string(r, path)?),
                TypeKind::NoType => {
                    VariantValue::ExtensionObject(ExtensionObject::decode(r).map_err(err)?)
                }
                other => {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: no OPC UA field for kind {other:?}"
                    )));
                }
            };
            scalar_from_variant(&ty, &Variant::scalar(value), path)
        }
    }
}

/// The value of every aggregate Variable of a sample of `ty`, path-named as
/// [`crate::dynamic_value::flatten`] names the leaves: a structure, a
/// union or a bitset as its ExtensionObject, a collection of structures or
/// unions as the array of them (and each element as its own), a `map` as
/// its `KeyValuePair`s. A value that cannot be one (a collection of
/// collections) is returned in the second list, with its path.
///
/// # Errors
/// [`ConvertError::ShapeMismatch`] / [`ConvertError::UnknownMember`] for a
/// value that does not fit the type.
pub fn aggregate_values(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
) -> Result<(Vec<FlatField>, Vec<ConvertError>), ConvertError> {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    aggregates(ty, data, "", ns, &mut out, &mut skipped, 0)?;
    Ok((out, skipped))
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, case,
/// element, entry or alias; refused past `MAX_DEPTH`)
fn aggregates(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
    ns: u16,
    out: &mut Vec<FlatField>,
    skipped: &mut Vec<ConvertError>,
    depth: usize,
) -> Result<(), ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    let ty = resolved_alias(ty, path)?;
    let join = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            std::format!("{path}.{name}")
        }
    };
    let emit = |out: &mut Vec<FlatField>, skipped: &mut Vec<ConvertError>| match variant_of(
        &ty, data, ns, path,
    ) {
        Ok(value) => {
            out.push(FlatField {
                path: path.to_string(),
                value,
            });
            Ok(())
        }
        Err(error @ ConvertError::Unsupported { .. }) => {
            skipped.push(error);
            Ok(())
        }
        Err(error) => Err(error),
    };
    match ty.kind() {
        TypeKind::Structure => {
            emit(out, skipped)?;
            for member in ty.members() {
                let Some(value) = data.member(member.name()) else {
                    continue;
                };
                let member_type = member.dynamic_type().map_err(|e| decode_error(path, &e))?;
                aggregates(
                    &member_type,
                    value,
                    &join(member.name()),
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        TypeKind::Union => {
            emit(out, skipped)?;
            for (name, value) in data.members().unwrap_or(&[]) {
                if name == "_d" {
                    continue;
                }
                let Some(case) = ty.member_by_name(name) else {
                    return Err(ConvertError::UnknownMember {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                };
                let case_type = case.dynamic_type().map_err(|e| decode_error(path, &e))?;
                aggregates(&case_type, value, &join(name), ns, out, skipped, depth + 1)?;
            }
            Ok(())
        }
        TypeKind::Bitset => emit(out, skipped),
        TypeKind::Map => {
            emit(out, skipped)?;
            let (key_ty, value_ty) = map_types(&ty, path)?;
            for entry in data.items().unwrap_or(&[]) {
                let Some([key, value]) = entry.items() else {
                    continue;
                };
                let key_text = map_key_string(&key_ty, key, path)?;
                aggregates(
                    &value_ty,
                    value,
                    &join(&key_text),
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        TypeKind::Array | TypeKind::Sequence => {
            let Some(element) = ty.element_type() else {
                return Ok(());
            };
            let element = resolved_alias(&element, path)?;
            let nested = is_collection(&element);
            if !matches!(element.kind(), TypeKind::Structure | TypeKind::Union) && !nested {
                return Ok(());
            }
            // A collection of collections has no Value of its own (no
            // Variant holds Variants here); its elements have theirs.
            if !nested {
                emit(out, skipped)?;
            }
            let dims = array_dims(&ty, path)?;
            let type_name = element_type_name(&element, path);
            for (index, item) in flat_items(data, dims.len().max(1), path)?
                .into_iter()
                .enumerate()
            {
                aggregates(
                    &element,
                    item,
                    &join(&element_name(&type_name, index, &dims)),
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Puts `new`, a value of the aggregate at `path` (as [`aggregate_values`]
/// names it), into the sample `data` of `ty`. A member that is absent, an
/// element past the collection's end or a map key the sample does not
/// have is refused; a case of a union that is not active becomes the
/// active one.
///
/// # Errors
/// [`ConvertError`] when `path` names no aggregate of the sample.
pub fn replace_at(
    ty: &DynamicType,
    data: &mut DynamicData,
    path: &str,
    new: DynamicData,
) -> Result<(), ConvertError> {
    replace(ty, data, path, path, new, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per path segment; refused
/// past `MAX_DEPTH`)
fn replace(
    ty: &DynamicType,
    data: &mut DynamicData,
    rest: &str,
    path: &str,
    new: DynamicData,
    depth: usize,
) -> Result<(), ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    if rest.is_empty() {
        *data = new;
        return Ok(());
    }
    let ty = resolved_alias(ty, path)?;
    let missing = || ConvertError::ShapeMismatch(format!("{path}: no such part of the sample"));
    let split = |rest: &str| -> (String, String) {
        match rest.split_once('.') {
            Some((head, tail)) => (head.to_string(), tail.to_string()),
            None => (rest.to_string(), String::new()),
        }
    };
    match ty.kind() {
        TypeKind::Structure => {
            let (name, tail) = split(rest);
            let member = ty.member_by_name(&name).ok_or_else(missing)?;
            let member_type = member.dynamic_type().map_err(|e| decode_error(path, &e))?;
            let DynamicData::Aggregate(members) = data else {
                return Err(missing());
            };
            let slot = match members.iter().position(|(known, _)| *known == name) {
                Some(index) => index,
                None if is_optional(&member) => {
                    let start = default_data(&member_type)
                        .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e}")))?;
                    members.push((name.clone(), start));
                    members.len() - 1
                }
                None => return Err(missing()),
            };
            replace(
                &member_type,
                &mut members[slot].1,
                &tail,
                path,
                new,
                depth + 1,
            )
        }
        TypeKind::Union => {
            let (name, tail) = split(rest);
            let case = ty.member_by_name(&name).ok_or_else(missing)?;
            let case_type = case.dynamic_type().map_err(|e| decode_error(path, &e))?;
            let DynamicData::Aggregate(members) = data else {
                return Err(missing());
            };
            if !members.iter().any(|(known, _)| *known == name) {
                let discriminator_type = ty.discriminator_type().ok_or_else(missing)?;
                let discriminator_type = resolved_alias(&discriminator_type, path)?;
                let label = label_of(&ty, &case, path)?;
                let start = default_data(&case_type)
                    .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e}")))?;
                *members = std::vec![
                    (
                        "_d".to_string(),
                        discriminator_data(&discriminator_type, label, path)?
                    ),
                    (name.clone(), start),
                ];
            }
            let slot = members
                .iter()
                .position(|(known, _)| *known == name)
                .ok_or_else(missing)?;
            replace(
                &case_type,
                &mut members[slot].1,
                &tail,
                path,
                new,
                depth + 1,
            )
        }
        TypeKind::Array | TypeKind::Sequence => {
            let (name, tail) = split(rest);
            let element = ty.element_type().ok_or_else(missing)?;
            let element = resolved_alias(&element, path)?;
            let dims = array_dims(&ty, path)?;
            let count = flat_items(data, dims.len().max(1), path)?.len();
            // The path up to this collection names its elements when they
            // are collections themselves.
            let consumed = path
                .strip_suffix(rest)
                .unwrap_or(path)
                .trim_end_matches('.');
            let type_name = element_type_name(&element, consumed);
            let index = (0..count)
                .find(|index| element_name(&type_name, *index, &dims) == name)
                .ok_or_else(missing)?;
            let slot = flat_item_mut(data, dims.len().max(1), index).ok_or_else(missing)?;
            replace(&element, slot, &tail, path, new, depth + 1)
        }
        TypeKind::Map => {
            let (key_ty, value_ty) = map_types(&ty, path)?;
            let DynamicData::Collection(entries) = data else {
                return Err(missing());
            };
            for entry in entries.iter_mut() {
                let DynamicData::Collection(pair) = entry else {
                    continue;
                };
                let [key, value] = pair.as_mut_slice() else {
                    continue;
                };
                let key_text = map_key_string(&key_ty, key, path)?;
                let tail = if rest == key_text {
                    Some(String::new())
                } else {
                    rest.strip_prefix(key_text.as_str())
                        .and_then(|after| after.strip_prefix('.'))
                        .map(ToString::to_string)
                };
                if let Some(tail) = tail {
                    return replace(&value_ty, value, &tail, path, new, depth + 1);
                }
            }
            Err(missing())
        }
        _ => Err(missing()),
    }
}

/// The type of the part of a sample of `ty` at `path` (as
/// [`aggregate_values`] names it).
///
/// # Errors
/// [`ConvertError`] when `path` names no part of the sample.
pub fn type_at(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
) -> Result<DynamicType, ConvertError> {
    type_of(ty, data, path, path, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per path segment; refused
/// past `MAX_DEPTH`)
fn type_of(
    ty: &DynamicType,
    data: &DynamicData,
    rest: &str,
    path: &str,
    depth: usize,
) -> Result<DynamicType, ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    if rest.is_empty() {
        return Ok(ty.clone());
    }
    let ty = resolved_alias(ty, path)?;
    let missing = || ConvertError::ShapeMismatch(format!("{path}: no such part of the sample"));
    let (head, tail) = rest.split_once('.').unwrap_or((rest, ""));
    match ty.kind() {
        TypeKind::Structure | TypeKind::Union => {
            let member = ty.member_by_name(head).ok_or_else(missing)?;
            let member_type = member.dynamic_type().map_err(|e| decode_error(path, &e))?;
            match data.member(head) {
                Some(value) => type_of(&member_type, value, tail, path, depth + 1),
                None if tail.is_empty() => Ok(member_type),
                None => {
                    let start = default_data(&member_type)
                        .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e}")))?;
                    type_of(&member_type, &start, tail, path, depth + 1)
                }
            }
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element = ty.element_type().ok_or_else(missing)?;
            let element = resolved_alias(&element, path)?;
            let dims = array_dims(&ty, path)?;
            let items = flat_items(data, dims.len().max(1), path)?;
            // The path up to this collection names its elements when they
            // are collections themselves.
            let consumed = path
                .strip_suffix(rest)
                .unwrap_or(path)
                .trim_end_matches('.');
            let type_name = element_type_name(&element, consumed);
            let index = (0..items.len())
                .find(|index| element_name(&type_name, *index, &dims) == head)
                .ok_or_else(missing)?;
            type_of(&element, items[index], tail, path, depth + 1)
        }
        TypeKind::Map => {
            let (key_ty, value_ty) = map_types(&ty, path)?;
            for entry in data.items().unwrap_or(&[]) {
                let Some([key, value]) = entry.items() else {
                    continue;
                };
                let key_text = map_key_string(&key_ty, key, path)?;
                if rest == key_text {
                    return Ok(value_ty);
                }
                if let Some(after) = rest
                    .strip_prefix(key_text.as_str())
                    .and_then(|after| after.strip_prefix('.'))
                {
                    return type_of(&value_ty, value, after, path, depth + 1);
                }
            }
            Err(missing())
        }
        _ => Err(missing()),
    }
}

/// The element `index` of a collection of `depth` dimensions, counted as
/// [`flat_items`] counts.
///
/// zerodds-lint: recursion-depth 8 (one level per array dimension)
fn flat_item_mut(data: &mut DynamicData, depth: usize, index: usize) -> Option<&mut DynamicData> {
    let DynamicData::Collection(items) = data else {
        return None;
    };
    if depth <= 1 {
        return items.get_mut(index);
    }
    let mut rest = index;
    for item in items.iter_mut() {
        let size = count_items(item, depth - 1);
        if rest < size {
            return flat_item_mut(item, depth - 1, rest);
        }
        rest -= size;
    }
    None
}

/// zerodds-lint: recursion-depth 8 (one level per array dimension)
fn count_items(data: &DynamicData, depth: usize) -> usize {
    match data {
        DynamicData::Collection(items) if depth > 1 => {
            items.iter().map(|item| count_items(item, depth - 1)).sum()
        }
        DynamicData::Collection(items) => items.len(),
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use zerodds_ast::Ast;

    fn ast_of(idl: &str) -> Arc<Ast> {
        Arc::new(
            zerodds_wire_probe::encode::model_from_idl(idl, "ua_structure_test.idl")
                .expect("idl parses"),
        )
    }

    fn body(object: &ExtensionObject) -> &[u8] {
        match &object.body {
            ExtensionObjectBody::ByteString(bytes) => bytes,
            other => panic!("no binary body: {other:?}"),
        }
    }

    fn with(ty: &DynamicType, data: &DynamicData, fields: &[(&str, VariantValue)]) -> DynamicData {
        let fields: Vec<FlatField> = fields
            .iter()
            .map(|(path, value)| FlatField {
                path: (*path).to_string(),
                value: Variant::scalar(value.clone()),
            })
            .collect();
        crate::dynamic_value::unflatten(ty, &fields, data).expect("unflatten")
    }

    #[test]
    fn opcuabridge_5_3_a_structure_is_its_default_binary_extension_object() {
        // OPC 10000-6 §5.2.6: the fields in the order of the definition,
        // each in its built-in encoding; the TypeId is the encoding node.
        let ast = ast_of("struct P { long x; string s; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = with(
            &ty,
            &default_data(&ty).expect("default"),
            &[
                ("x", VariantValue::Int32(3)),
                ("s", VariantValue::String("ab".into())),
            ],
        );
        let object = to_extension_object(&ty, &data, 2, "").expect("encode");
        assert_eq!(object.type_id, encoding_id(2, "P"));
        assert_eq!(body(&object), &[3, 0, 0, 0, 2, 0, 0, 0, b'a', b'b']);
        assert_eq!(
            from_extension_object(&ty, &object, 2, "").expect("decode"),
            data
        );
    }

    #[test]
    fn opcuabridge_5_3_optional_members_are_an_encoding_mask_first() {
        // §5.2.7: a bit per optional field, the first optional field bit 0.
        let ast = ast_of("struct O { long a; @optional long b; @optional long c; };");
        let ty = DynamicType::find(&ast, "O").expect("type");
        let data = with(
            &ty,
            &default_data(&ty).expect("default"),
            &[("a", VariantValue::Int32(1)), ("c", VariantValue::Int32(7))],
        );
        let object = to_extension_object(&ty, &data, 2, "").expect("encode");
        assert_eq!(body(&object), &[2, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0]);
        let back = from_extension_object(&ty, &object, 2, "").expect("decode");
        assert_eq!(back.member("b"), None);
        assert_eq!(back.member("c"), data.member("c"));
        // An unassigned bit set is refused.
        let mut bad = body(&object).to_vec();
        bad[0] = 0x06;
        let bad = ExtensionObject {
            type_id: object.type_id.clone(),
            body: ExtensionObjectBody::ByteString(bad),
        };
        assert!(from_extension_object(&ty, &bad, 2, "").is_err());
    }

    #[test]
    fn opcuabridge_5_3_a_union_is_its_switch_then_the_case() {
        // §5.2.8 and Gateway Table 9.20: the switch is the position of the
        // case counted from 1, whatever the discriminator's value.
        let ast = ast_of(
            "union U switch (long) { case 7: long a; case 9: case 10: string b; }; \
             struct H { U pick; };",
        );
        let holder = DynamicType::find(&ast, "H").expect("type");
        let data = with(
            &holder,
            &default_data(&holder).expect("default"),
            &[("pick.b", VariantValue::String("z".into()))],
        );
        let union_type = DynamicType::find(&ast, "U").expect("union");
        let pick = data.member("pick").expect("pick");
        let object = to_extension_object(&union_type, pick, 2, "pick").expect("encode");
        assert_eq!(body(&object), &[2, 0, 0, 0, 1, 0, 0, 0, b'z']);
        let back = from_extension_object(&union_type, &object, 2, "pick").expect("decode");
        assert_eq!(&back, pick);
        assert_eq!(
            back.member("_d"),
            Some(&DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 9
            })
        );
    }

    #[test]
    fn opcuabridge_5_3_an_array_of_two_dimensions_is_the_inline_matrix_and_the_older_form_is_read()
    {
        // OPC 10000-6 §5.2.5 Table 28: the dimensions, then the values.
        let ast = ast_of("struct Grid { long cells[2][3]; };");
        let ty = DynamicType::find(&ast, "Grid").expect("type");
        let data = default_data(&ty).expect("default");
        let object = to_extension_object(&ty, &data, 2, "").expect("encode");
        let mut matrix = std::vec![2, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0];
        matrix.extend(std::iter::repeat_n(0u8, 24));
        assert_eq!(body(&object), matrix.as_slice());
        // The one-dimensional form of encoders before 1.05.
        let mut older = std::vec![6, 0, 0, 0];
        for value in 0..6i32 {
            older.extend_from_slice(&value.to_le_bytes());
        }
        let back = from_extension_object(
            &ty,
            &ExtensionObject {
                type_id: object.type_id.clone(),
                body: ExtensionObjectBody::ByteString(older),
            },
            2,
            "",
        )
        .expect("decode");
        let rows = back
            .member("cells")
            .and_then(DynamicData::items)
            .expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1].items().expect("row")[2],
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 5
            }
        );
    }

    #[test]
    fn opcuabridge_5_3_a_collection_of_collections_is_a_field_of_variants() {
        // The field's DataType is BaseDataType (an array of it), so each
        // element is a Variant (OPC 10000-6 §5.2.6), here an Int32 array.
        let ast = ast_of("struct RowSet { sequence<sequence<long>> rows; };");
        let ty = DynamicType::find(&ast, "RowSet").expect("type");
        let int = |value| DynamicData::Integer {
            kind: Some(PrimitiveKind::Int32),
            value,
        };
        let data = DynamicData::Aggregate(std::vec![(
            "rows".into(),
            DynamicData::Collection(std::vec![
                DynamicData::Collection(std::vec![int(1), int(2)]),
                DynamicData::Collection(std::vec![int(3)]),
            ]),
        )]);
        let object = to_extension_object(&ty, &data, 2, "").expect("encode");
        assert_eq!(
            body(&object),
            &[
                2, 0, 0, 0, // two elements
                0x86, 2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, // Variant Int32[2]
                0x86, 1, 0, 0, 0, 3, 0, 0, 0, // Variant Int32[1]
            ]
        );
        assert_eq!(
            from_extension_object(&ty, &object, 2, "").expect("decode"),
            data
        );
        // The values of the aggregates: the inner sequences are leaves, the
        // outer one has no Value of its own.
        let (values, skipped) = aggregate_values(&ty, &data, 2).expect("aggregates");
        assert!(skipped.is_empty(), "{skipped:?}");
        let paths: Vec<&str> = values.iter().map(|field| field.path.as_str()).collect();
        assert_eq!(paths, [""]);
        // A write of the inner sequence of structures reaches it by name.
        let ast = ast_of(
            "struct Point { long x; }; struct LineSet { sequence<sequence<Point>> lines; };",
        );
        let lines = DynamicType::find(&ast, "LineSet").expect("type");
        let point = DynamicType::find(&ast, "Point").expect("point");
        let p = |x| DynamicData::Aggregate(std::vec![("x".into(), int(x))]);
        let mut sample = DynamicData::Aggregate(std::vec![(
            "lines".into(),
            DynamicData::Collection(std::vec![DynamicData::Collection(std::vec![p(1)])]),
        )]);
        let (values, _) = aggregate_values(&lines, &sample, 2).expect("aggregates");
        let paths: Vec<&str> = values.iter().map(|field| field.path.as_str()).collect();
        assert_eq!(paths, ["", "lines.lines_0", "lines.lines_0.Point_0"]);
        replace_at(&lines, &mut sample, "lines.lines_0.Point_0", p(5)).expect("replace");
        assert_eq!(
            type_at(&lines, &sample, "lines.lines_0")
                .expect("type")
                .kind(),
            TypeKind::Sequence
        );
        let _ = point;
        let (fields, _) = crate::dynamic_value::flatten_in(&lines, &sample, 2).expect("flatten");
        assert_eq!(fields[0].path, "lines.lines_0.Point_0.x");
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Int32(5)));
    }

    #[test]
    fn opcuabridge_5_3_every_aggregate_gets_its_value_and_a_write_replaces_it() {
        let ast = ast_of(
            "struct Point { long x; long y; }; \
             @bit_bound(12) bitmask FlagSet { A, B, C }; \
             enum ModeKind { OFF, ON }; \
             union Choice switch (short) { case 1: Point p; case 2: double d; }; \
             struct Track { Point at; sequence<Point> path; map<string, long> tags; \
                            FlagSet flags; ModeKind mode; Choice pick; long grid[2][3]; fixed<6,2> f; };",
        );
        let ty = DynamicType::find(&ast, "Track").expect("type");
        let start = default_data(&ty).expect("default");
        let point = DynamicType::find(&ast, "Point").expect("point");
        let mut data = start.clone();
        replace_at(
            &ty,
            &mut data,
            "path",
            data_of_variant(
                &DynamicType::find(&ast, "Track")
                    .expect("type")
                    .member_by_name("path")
                    .expect("path")
                    .dynamic_type()
                    .expect("type"),
                &Variant {
                    array_dimensions: std::vec![2],
                    value: [(1, 2), (3, 4)]
                        .iter()
                        .map(|(x, y)| {
                            let p = with(
                                &point,
                                &default_data(&point).expect("default"),
                                &[
                                    ("x", VariantValue::Int32(*x)),
                                    ("y", VariantValue::Int32(*y)),
                                ],
                            );
                            VariantValue::ExtensionObject(
                                to_extension_object(&point, &p, 2, "").expect("point"),
                            )
                        })
                        .collect(),
                },
                2,
                "path",
            )
            .expect("path value"),
        )
        .expect("replace");
        let data = with(
            &ty,
            &data,
            &[
                ("at.x", VariantValue::Int32(5)),
                ("pick.p.y", VariantValue::Int32(8)),
                ("mode", VariantValue::Int32(1)),
            ],
        );
        let (values, skipped) = aggregate_values(&ty, &data, 2).expect("aggregates");
        assert!(skipped.is_empty(), "{skipped:?}");
        let paths: Vec<&str> = values.iter().map(|field| field.path.as_str()).collect();
        for path in [
            "",
            "at",
            "path",
            "path.Point_0",
            "path.Point_1",
            "tags",
            "pick",
            "pick.p",
        ] {
            assert!(paths.contains(&path), "{path} missing from {paths:?}");
        }
        // The whole value comes back from its ExtensionObject.
        let root = &values
            .iter()
            .find(|field| field.path.is_empty())
            .expect("root")
            .value;
        let back = data_of_variant(&ty, root, 2, "").expect("decode");
        assert_eq!(back, data);
        // The sequence is the array of its elements.
        let path_value = &values
            .iter()
            .find(|field| field.path == "path")
            .expect("path")
            .value;
        assert_eq!(path_value.value.len(), 2);
        // A write of one element replaces it.
        let new_point = with(
            &point,
            &default_data(&point).expect("default"),
            &[("x", VariantValue::Int32(30))],
        );
        let mut written = data.clone();
        replace_at(&ty, &mut written, "path.Point_1", new_point.clone()).expect("replace");
        let (fields, _) = crate::dynamic_value::flatten_in(&ty, &written, 2).expect("flatten");
        let x = fields
            .iter()
            .find(|field| field.path == "path.Point_1.x")
            .expect("x");
        assert_eq!(x.value, Variant::scalar(VariantValue::Int32(30)));
        assert_eq!(
            type_at(&ty, &written, "path.Point_1")
                .expect("type")
                .qualified_name(),
            "Point"
        );
        // A path the sample does not have is refused.
        assert!(replace_at(&ty, &mut written, "path.Point_5", new_point).is_err());
    }
}
