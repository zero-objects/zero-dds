// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds_types::dynamic::DynamicData` ↔ a flat, path-named list of
//! [`Variant`]s.
//!
//! Neither the OPC-UA AddressSpace's `HasComponent` children nor a Part 14
//! `PublishedDataSet`/`ReceivedDataSet` carry a nested value — both are a
//! flat list of named leaves (an AddressSpace's leaves are its instance
//! nodes' own Variable values; a DataSet's are its fields). [`flatten`]
//! walks a `(DynamicType, DynamicData)` pair once and produces exactly the
//! list both consumers need, path-naming a nested struct member
//! `outer.inner` the way [`address_space_build`](crate::address_space_build)
//! turns into a dotted NodeId and
//! [`pubsub_glue`](crate::pubsub_glue) turns into a DataSet field name.
//! [`unflatten`] is the inverse, for the OPC-UA-write and PubSub-subscribe
//! directions.

use alloc::collections::BTreeMap;
use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_ast::PrimitiveKind;
use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
use zerodds_types::dynamic::{DynamicData, DynamicType, TypeKind};

/// A `(DynamicType, DynamicData)` pair this module cannot map onto/from
/// `Variant` — named, never approximated. `map`, `wchar`/`wstring` and a
/// map-of-anything all get a live value (see [`flatten`]'s
/// `TypeKind::Map` case and `scalar_value`'s `Char16`/`String16` arms);
/// what remains genuinely unmapped inherits the same design boundary
/// `zerodds_opcua_gateway::address_space::build_sample_instance` already
/// draws for an array/sequence of an aggregate (Gateway 1.0 §9.2.5's own
/// "array of struct is one array Variable, not decomposed" rule) — a
/// dynamic value cannot be squeezed into one `Variant` there either, since
/// `VariantValue` has no aggregate case, and this daemon does not encode it
/// as an `ExtensionObject` blob — plus `bitset`/`fixed<P,S>`/`float128`,
/// which have no OPC-UA builtin `VariantValue` case at all to hold them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// The type/data pair disagree in shape (data says N members, type
    /// says fewer/more; a union's active case is not one of the type's).
    ShapeMismatch(String),
    /// A kind this module does not flatten a live value for: an array or
    /// sequence of a struct/union, a `bitset`, `fixed<P,S>`, or `float128`
    /// (Gateway 1.0 §9.2 gives these a type mapping in `dynamic_type`, but
    /// not a value one here — `map` and `wchar`/`wstring` are no longer in
    /// this list, see [`flatten`]).
    Unsupported {
        /// The field path, dot-joined from the topic's own root.
        path: String,
        /// Why.
        reason: &'static str,
    },
    /// An enumerator/bitflag name in the data has no member of that name in
    /// the type (a peer's model and ours disagree about the same
    /// `TypeIdentifier`, or the value came from an untyped upstream).
    UnknownMember {
        /// The field path.
        path: String,
        /// The name that was not found.
        name: String,
    },
    /// The value nests deeper than `MAX_DEPTH` — possible for a
    /// recursive type (a struct holding a sequence of itself), whose data
    /// arrives from the wire.
    TooDeep {
        /// The field path at the level past the bound.
        path: String,
    },
}

/// Recursion bound of [`flatten`]/[`unflatten`], the one
/// `crate::dynamic_type` applies to the type itself.
const MAX_DEPTH: usize = 64;

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ShapeMismatch(m) => write!(f, "type/data shape mismatch: {m}"),
            Self::Unsupported { path, reason } => {
                write!(f, "{path}: not mapped to a Variant ({reason})")
            }
            Self::UnknownMember { path, name } => {
                write!(f, "{path}: `{name}` is not a member of the type")
            }
            Self::TooDeep { path } => {
                write!(f, "{path}: value nesting exceeds {MAX_DEPTH} levels")
            }
        }
    }
}

impl std::error::Error for ConvertError {}

/// One flattened leaf: its dot-joined path from the sample root, and the
/// value at it.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatField {
    /// e.g. `"position.x"`.
    pub path: String,
    /// The value.
    pub value: Variant,
}

/// Walks `(ty, data)` and returns every leaf `Variant`, path-named, plus
/// every path this module could not map (see [`ConvertError::Unsupported`])
/// — never silently omitted.
///
/// # Errors
/// [`ConvertError::ShapeMismatch`] / [`ConvertError::UnknownMember`] — a
/// structural disagreement between `ty` and `data`, which
/// [`ConvertError::Unsupported`] (returned in `skipped`, not here) is not.
pub fn flatten(
    ty: &DynamicType,
    data: &DynamicData,
) -> Result<(Vec<FlatField>, Vec<ConvertError>), ConvertError> {
    flatten_in(ty, data, crate::daemon::server::NS)
}

/// As [`flatten`], with the DataTypes of the bitmasks (whose leaves are
/// `OptionSet` ExtensionObjects, Gateway 1.0 Table 9.12) in namespace `ns`.
///
/// # Errors
/// As [`flatten`].
pub fn flatten_in(
    ty: &DynamicType,
    data: &DynamicData,
    ns: u16,
) -> Result<(Vec<FlatField>, Vec<ConvertError>), ConvertError> {
    let mut fields = Vec::new();
    let mut skipped = Vec::new();
    walk_value(ty, data, "", ns, &mut fields, &mut skipped, 0)?;
    Ok((fields, skipped))
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, element
/// or alias; refused past `MAX_DEPTH`)
#[allow(clippy::too_many_lines)]
fn walk_value(
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
    match ty.kind() {
        // The members by their names, in the order of the type (a
        // `@mutable` type's data has them in the order the writer chose).
        // An `@optional` member the sample does not have has no leaf in it
        // (`address_space_build::apply_sample` marks the leaves it had).
        TypeKind::Structure => {
            let Some(fields_data) = data.members() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected an aggregate"
                )));
            };
            for (name, _) in fields_data {
                if ty.member_by_name(name).is_none() {
                    return Err(ConvertError::UnknownMember {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                }
            }
            for member in ty.members() {
                let name = member.name();
                let Some(value) = data.member(name) else {
                    if is_optional(&member) {
                        continue;
                    }
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: the sample has no member `{name}`"
                    )));
                };
                let member_type = member
                    .dynamic_type()
                    .map_err(|e| ConvertError::ShapeMismatch(format!("{path}.{name}: {e:?}")))?;
                let child_path = join(path, name);
                walk_value(
                    &member_type,
                    value,
                    &child_path,
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        // The discriminator is the leaf `SwitchField` (the component
        // `zerodds_opcua_gateway::address_space::build_sample_instance`
        // gives a union), the active case the leaves under its name.
        TypeKind::Union => {
            let Some(members) = data.members() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected a union value"
                )));
            };
            let discriminator = data.member("_d").ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: a union value with no discriminator"))
            })?;
            let discriminator_type = ty.discriminator_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: a union with no discriminator type"))
            })?;
            let discriminator_type = resolved_alias(&discriminator_type, path)?;
            let switch_path = join(path, "SwitchField");
            out.push(FlatField {
                value: Variant::scalar(scalar_value(
                    &discriminator_type,
                    discriminator,
                    &switch_path,
                    ns,
                    skipped,
                )?),
                path: switch_path,
            });
            for (name, value) in members {
                if name == "_d" {
                    continue;
                }
                let Some(member) = ty.member_by_name(name) else {
                    return Err(ConvertError::UnknownMember {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                };
                let member_type = member
                    .dynamic_type()
                    .map_err(|e| ConvertError::ShapeMismatch(format!("{path}.{name}: {e:?}")))?;
                let child_path = join(path, name);
                walk_value(
                    &member_type,
                    value,
                    &child_path,
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        TypeKind::Alias => {
            let target = ty.base_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: alias with no target"))
            })?;
            walk_value(&target, data, path, ns, out, skipped, depth + 1)
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element_ty = ty.element_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: collection with no element type"))
            })?;
            let dims = array_dims(ty, path)?;
            // An array of several dimensions is one collection per
            // dimension in the data; its elements are counted in the order
            // they are serialized, the last dimension fastest.
            let items = flat_items(data, dims.len().max(1), path)?;
            // Gateway 1.0 §9.2.5.1.2.3 and §9.2.5.2.2.3: each element of an
            // array or a sequence of structures (or unions) is a Variable
            // of its own, named `<TypeName>_<index>`, with the element's
            // members under it.
            let aggregate = resolved_alias(&element_ty, path)?;
            if matches!(aggregate.kind(), TypeKind::Structure | TypeKind::Union)
                || is_collection(&aggregate)
            {
                // Gateway 1.0 §9.2.5.2.2.5: an element that is itself a
                // collection is a Variable `<SequenceName>_<index>`.
                let type_name = element_type_name(&aggregate, path);
                for (index, item) in items.iter().enumerate() {
                    let child_path = join(path, &element_name(&type_name, index, &dims));
                    walk_value(&aggregate, item, &child_path, ns, out, skipped, depth + 1)?;
                }
                return Ok(());
            }
            let dims = if dims.is_empty() {
                vec![u32::try_from(items.len()).unwrap_or(u32::MAX)]
            } else {
                dims
            };
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(scalar_value(&aggregate, item, path, ns, skipped)?);
            }
            out.push(FlatField {
                path: path.to_string(),
                value: Variant {
                    array_dimensions: dims,
                    value: values,
                },
            });
            Ok(())
        }
        // Gateway 1.0 Tab 9.40/9.41: a `map<K,V>` gets one child node per
        // entry, keyed by the (stringified) map key. `DynamicData`'s own
        // shape for a map (`zerodds_types::dynamic::data::DynamicData::
        // Collection` docs: "a map entry is a two-element collection,
        // [key, value]") is walked the same way a struct's members are —
        // recursively, so a map of structs still decomposes down to
        // scalar leaves. A key that has no node yet gets its nodes when
        // the sample is applied (`address_space_build::apply_sample`).
        TypeKind::Map => {
            let Some(key_ty) = ty.key_element_type() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: map with no key type"
                )));
            };
            let value_ty = ty.element_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: map with no value type"))
            })?;
            let Some(entries) = data.items() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected a map value"
                )));
            };
            for entry in entries {
                let Some(entry_items) = entry.items() else {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: a map entry must be a 2-element [key, value] collection"
                    )));
                };
                let [key_data, value_data] = entry_items else {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: a map entry must have exactly 2 elements, got {}",
                        entry_items.len()
                    )));
                };
                let key_str = map_key_string(&key_ty, key_data, path)?;
                let child_path = join(path, &key_str);
                walk_value(
                    &value_ty,
                    value_data,
                    &child_path,
                    ns,
                    out,
                    skipped,
                    depth + 1,
                )?;
            }
            Ok(())
        }
        // A bitset is its named bitfields, one leaf each, as a struct is
        // its members (spec §5.3; Gateway 1.0 has no clause for a bitset).
        // The value of a bitfield is a number of its holder type.
        TypeKind::Bitset => {
            let DynamicData::Bitfields(values) = data else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected the bitfields of a bitset"
                )));
            };
            let kinds = crate::dynamic_type::bitfields(ty);
            for (name, value) in values {
                let Some((_, kind)) = kinds.iter().find(|(known, _)| known == name) else {
                    return Err(ConvertError::UnknownMember {
                        path: path.to_string(),
                        name: name.clone(),
                    });
                };
                let child_path = join(path, name);
                let DynamicData::Integer { value: number, .. } = value else {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{child_path}: the value of a bitfield is a number"
                    )));
                };
                out.push(FlatField {
                    path: child_path,
                    value: Variant::scalar(bitfield_value(*kind, *number)),
                });
            }
            Ok(())
        }
        // Scalar (primitive/string/enum/bitmask) — one leaf.
        _ => {
            let value = scalar_value(ty, data, path, ns, skipped)?;
            out.push(FlatField {
                path: path.to_string(),
                value: Variant::scalar(value),
            });
            Ok(())
        }
    }
}

/// The value of a bitfield whose holder is of `kind`.
pub(crate) fn bitfield_value(kind: TypeKind, number: i128) -> VariantValue {
    match kind {
        TypeKind::Boolean => VariantValue::Boolean(number != 0),
        TypeKind::Int8 => VariantValue::SByte(number as i8),
        TypeKind::Int16 => VariantValue::Int16(number as i16),
        TypeKind::UInt16 => VariantValue::UInt16(number as u16),
        TypeKind::Int32 => VariantValue::Int32(number as i32),
        TypeKind::UInt32 => VariantValue::UInt32(number as u32),
        TypeKind::Int64 => VariantValue::Int64(number as i64),
        TypeKind::UInt64 => VariantValue::UInt64(number as u64),
        _ => VariantValue::Byte(number as u8),
    }
}

/// Whether `member` is an `@optional` member.
pub(crate) fn is_optional(member: &zerodds_types::dynamic::DynamicTypeMember) -> bool {
    member
        .descriptor()
        .is_ok_and(|descriptor| descriptor.is_optional)
}

/// Whether `ty` is a collection: an array, a sequence or a `map`.
pub(crate) fn is_collection(ty: &DynamicType) -> bool {
    matches!(
        ty.kind(),
        TypeKind::Array | TypeKind::Sequence | TypeKind::Map
    )
}

/// The name the elements of the collection at `path` are named after
/// ([`element_name`]): the type of a structure or a union element
/// (Gateway 1.0 §9.2.5.1.2.3), the collection's own name for an element
/// that is itself a collection (§9.2.5.2.2.5, `<SequenceName>_<index>`).
pub(crate) fn element_type_name(element: &DynamicType, path: &str) -> String {
    if is_collection(element) {
        path.rsplit('.').next().unwrap_or(path).to_string()
    } else {
        element.qualified_name()
    }
}

/// The dimensions of the array `ty`; none for a sequence.
pub(crate) fn array_dims(ty: &DynamicType, path: &str) -> Result<Vec<u32>, ConvertError> {
    if matches!(ty.kind(), TypeKind::Array) {
        Ok(ty
            .descriptor()
            .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e:?}")))?
            .bound)
    } else {
        Ok(Vec::new())
    }
}

/// The elements of a collection of `depth` dimensions (one collection per
/// dimension in `data`), in the order they are serialized.
///
/// zerodds-lint: recursion-depth 8 (one level per array dimension)
pub(crate) fn flat_items<'a>(
    data: &'a DynamicData,
    depth: usize,
    path: &str,
) -> Result<Vec<&'a DynamicData>, ConvertError> {
    let Some(items) = data.items() else {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: expected a collection value"
        )));
    };
    if depth <= 1 {
        return Ok(items.iter().collect());
    }
    let mut out = Vec::new();
    for item in items {
        out.extend(flat_items(item, depth - 1, path)?);
    }
    Ok(out)
}

/// The inverse of [`flat_items`]: the elements as one collection per
/// dimension of `dims`.
///
/// zerodds-lint: recursion-depth 8 (one level per array dimension)
pub(crate) fn nest(items: Vec<DynamicData>, dims: &[u32]) -> DynamicData {
    if dims.len() <= 1 {
        return DynamicData::Collection(items);
    }
    let inner = dims[1..]
        .iter()
        .map(|dim| *dim as usize)
        .product::<usize>()
        .max(1);
    let mut out = Vec::new();
    let mut rest = items.into_iter();
    loop {
        let chunk: Vec<DynamicData> = rest.by_ref().take(inner).collect();
        if chunk.is_empty() {
            break;
        }
        out.push(nest(chunk, &dims[1..]));
    }
    DynamicData::Collection(out)
}

/// The type an alias chain ends at; `ty` itself when it is no alias.
pub(crate) fn resolved_alias(ty: &DynamicType, path: &str) -> Result<DynamicType, ConvertError> {
    let mut current = ty.clone();
    for _ in 0..MAX_DEPTH {
        if !matches!(current.kind(), TypeKind::Alias) {
            return Ok(current);
        }
        current = current
            .base_type()
            .ok_or_else(|| ConvertError::ShapeMismatch(format!("{path}: alias with no target")))?;
    }
    Err(ConvertError::TooDeep {
        path: path.to_string(),
    })
}

/// The name of element `index` of an array or a sequence of the structure
/// or union `type_name` (Gateway 1.0 §9.2.5.1.2.3): `<TypeName>_<index>`,
/// for an array of several dimensions the position in each, as in
/// `<TypeName>_1_2_3`. `dims` is empty for a sequence; the index counts
/// the elements in the order they are serialized, the last dimension
/// fastest. The type's simple name stands for a scoped one.
#[must_use]
pub fn element_name(type_name: &str, index: usize, dims: &[u32]) -> String {
    let simple = type_name.rsplit("::").next().unwrap_or(type_name);
    if dims.len() < 2 {
        return std::format!("{simple}_{index}");
    }
    let mut positions = std::vec![0usize; dims.len()];
    let mut rest = index;
    for (slot, dim) in positions.iter_mut().zip(dims).rev() {
        let dim = (*dim as usize).max(1);
        *slot = rest % dim;
        rest /= dim;
    }
    let mut name = String::from(simple);
    for position in positions {
        name.push('_');
        name.push_str(&position.to_string());
    }
    name
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        std::format!("{path}.{name}")
    }
}

/// Renders a map key as a path segment — the inverse,
/// [`parse_map_key`], parses it back for [`unflatten`]. Only the key
/// kinds a map key can actually be (an integer, a string, a boolean, or
/// an enumerator name — never an aggregate/collection/floating-point
/// key, which IDL itself does not allow as a map key type) are handled;
/// anything else is a type/data disagreement, not a value this module
/// declines to map.
pub(crate) fn map_key_string(
    key_ty: &DynamicType,
    key_data: &DynamicData,
    path: &str,
) -> Result<String, ConvertError> {
    match (key_ty.kind(), key_data) {
        (
            TypeKind::Int8
            | TypeKind::UInt8
            | TypeKind::Byte
            | TypeKind::Int16
            | TypeKind::UInt16
            | TypeKind::Int32
            | TypeKind::UInt32
            | TypeKind::Int64
            | TypeKind::UInt64,
            DynamicData::Integer { value, .. },
        ) => Ok(value.to_string()),
        (TypeKind::String8 | TypeKind::String16, DynamicData::Text(v)) => Ok(v.clone()),
        (TypeKind::Boolean, DynamicData::Boolean(v)) => Ok(v.to_string()),
        (TypeKind::Enumeration, DynamicData::Enumerator(name)) => Ok(name.clone()),
        (kind, data) => Err(ConvertError::ShapeMismatch(format!(
            "{path}: map key kind {kind:?} does not match key data {data:?}"
        ))),
    }
}

/// The inverse of [`map_key_string`] — parses a path segment back into
/// the `DynamicData` a map key of `key_ty` must be, for [`build_value`]'s
/// `TypeKind::Map` case.
pub(crate) fn parse_map_key(
    key_ty: &DynamicType,
    segment: &str,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let bad = || {
        ConvertError::ShapeMismatch(format!(
            "{path}: `{segment}` does not parse as a {:?} map key",
            key_ty.kind()
        ))
    };
    match key_ty.kind() {
        TypeKind::Int8 => Ok(int_data(
            PrimitiveKind::Int8,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::UInt8 | TypeKind::Byte => Ok(int_data(
            PrimitiveKind::UInt8,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::Int16 => Ok(int_data(
            PrimitiveKind::Int16,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::UInt16 => Ok(int_data(
            PrimitiveKind::UInt16,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::Int32 => Ok(int_data(
            PrimitiveKind::Int32,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::UInt32 => Ok(int_data(
            PrimitiveKind::UInt32,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::Int64 => Ok(int_data(
            PrimitiveKind::Int64,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::UInt64 => Ok(int_data(
            PrimitiveKind::UInt64,
            segment.parse().map_err(|_| bad())?,
        )),
        TypeKind::String8 | TypeKind::String16 => Ok(DynamicData::Text(segment.to_string())),
        TypeKind::Boolean => Ok(DynamicData::Boolean(segment.parse().map_err(|_| bad())?)),
        TypeKind::Enumeration => {
            if key_ty.member_by_name(segment).is_none() {
                return Err(bad());
            }
            Ok(DynamicData::Enumerator(segment.to_string()))
        }
        _ => Err(bad()),
    }
}

/// Converts one atomic (non-aggregate, non-collection) `(type, data)` pair
/// to a single [`VariantValue`].
pub(crate) fn scalar_value(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
    ns: u16,
    skipped: &mut Vec<ConvertError>,
) -> Result<VariantValue, ConvertError> {
    match (ty.kind(), data) {
        (TypeKind::Boolean, DynamicData::Boolean(v)) => Ok(VariantValue::Boolean(*v)),
        (TypeKind::Int8, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::SByte(*value as i8))
        }
        (TypeKind::UInt8 | TypeKind::Byte, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::Byte(*value as u8))
        }
        (TypeKind::Int16, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::Int16(*value as i16))
        }
        (TypeKind::UInt16, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::UInt16(*value as u16))
        }
        (TypeKind::Int32, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::Int32(*value as i32))
        }
        (TypeKind::UInt32, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::UInt32(*value as u32))
        }
        (TypeKind::Int64, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::Int64(*value as i64))
        }
        (TypeKind::UInt64, DynamicData::Integer { value, .. }) => {
            Ok(VariantValue::UInt64(*value as u64))
        }
        (TypeKind::Float32, DynamicData::Float { value, .. }) => {
            Ok(VariantValue::Float(*value as f32))
        }
        (TypeKind::Float64, DynamicData::Float { value, .. }) => Ok(VariantValue::Double(*value)),
        (TypeKind::Char8, DynamicData::Character { value, .. }) => Ok(VariantValue::Byte(
            value.chars().next().map_or(0, |c| c as u8),
        )),
        (TypeKind::String8, DynamicData::Text(v)) => Ok(VariantValue::String(v.clone())),
        // `zerodds_opcua_gateway::dds_to_ua::walker::DdsType::opc_ua_data_type`
        // already maps Char16 -> UInt16 and String16 -> String at the type
        // level; this mirrors it at the value level rather than refusing a
        // value the type-level walker already promised a home for.
        (TypeKind::Char16, DynamicData::Character { value, .. }) => Ok(VariantValue::UInt16(
            value.chars().next().map_or(0, |c| c as u16),
        )),
        (TypeKind::String16, DynamicData::Text(v)) => Ok(VariantValue::String(v.clone())),
        (TypeKind::Enumeration, DynamicData::Enumerator(name)) => {
            let ordinal = ty
                .member_by_name(name)
                .ok_or_else(|| ConvertError::UnknownMember {
                    path: path.to_string(),
                    name: name.clone(),
                })?;
            Ok(VariantValue::Int32(
                i32::try_from(ordinal.literal_value().unwrap_or(0)).unwrap_or(0),
            ))
        }
        // Gateway 1.0 Table 9.12: the value of a bitmask is its `OptionSet`
        // DataType, `Value` and `ValidBits` (OPC 10000-3 §8.40).
        (TypeKind::Bitmask, DynamicData::Bits(_)) => {
            let (value, valid) = option_set_bits(ty, data, path, skipped)?;
            let mut w = zerodds_opcua_pubsub::UaWriter::with_capacity(value.len() * 2 + 8);
            crate::ua_structure::write_byte_string(&mut w, &value);
            crate::ua_structure::write_byte_string(&mut w, &valid);
            Ok(VariantValue::ExtensionObject(
                zerodds_opcua_gateway::data_value::ExtensionObject {
                    type_id: crate::ua_types::encoding_id(ns, &ty.qualified_name()),
                    body: zerodds_opcua_gateway::data_value::ExtensionObjectBody::ByteString(
                        w.into_vec(),
                    ),
                },
            ))
        }
        // A scalar leaf that cannot be mapped (unlike a collection element,
        // which the caller already skips as a unit) leaves its whole
        // `FlatField` unrepresentable, so this returns the error directly;
        // the call site propagates it with `?` — named, not silently
        // dropped, one level up.
        // Gateway 1.0 §9.2.1: `long double` is a `ByteString`, and "to
        // store the Float128 value, the length of the equivalent
        // ByteString shall be 16". The octets are the binary128 value
        // least significant first, the order of every number in the OPC UA
        // binary encoding (Part 6 §5.2.2); the Gateway names no order.
        (TypeKind::Float128, DynamicData::LongDouble(v)) => {
            Ok(VariantValue::ByteString(v.to_le_bytes().to_vec()))
        }
        // OPC 10000-6 §5.1.10: a `fixed<P,S>` is a `Decimal`, an
        // ExtensionObject of the Decimal DataType (`i=50`) whose body is the
        // Int16 scale and the unscaled value, two's complement, least
        // significant byte first.
        (TypeKind::NoType, DynamicData::Fixed(text)) => {
            let (_, scale) = crate::dynamic_type::fixed_of(ty).ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: a fixed value of no fixed type"))
            })?;
            let unscaled = fixed_unscaled(text, scale).ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: `{text}` is no decimal"))
            })?;
            Ok(VariantValue::ExtensionObject(decimal(unscaled, scale)))
        }
        (kind, data) => Err(ConvertError::ShapeMismatch(std::format!(
            "{path}: type kind {kind:?} does not match data variant {data:?}"
        ))),
    }
}

/// The `Value` and the `ValidBits` of the `OptionSet` of a bitmask value:
/// bit `n` is bit `n % 8` (counted from the least significant) of octet
/// `n / 8`, in as many octets as the bound of the bitmask needs. A bit is
/// valid when the bitmask names a flag at its position.
pub(crate) fn option_set_bits(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
    skipped: &mut Vec<ConvertError>,
) -> Result<(Vec<u8>, Vec<u8>), ConvertError> {
    let DynamicData::Bits(flags) = data else {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: expected the flags of a bitmask"
        )));
    };
    let bound = usize::from(ty.bit_bound().unwrap_or(32)).clamp(1, 64);
    let octets = bound.div_ceil(8);
    let mut value = vec![0u8; octets];
    let mut valid = vec![0u8; octets];
    let set = |bits: &mut [u8], position: u32| {
        if let Some(octet) = bits.get_mut(position as usize / 8) {
            *octet |= 1 << (position % 8);
        }
    };
    for member in ty.members() {
        if let Some(position) = member.literal_value() {
            set(&mut valid, position);
        }
    }
    for flag in flags {
        let DynamicData::Text(name) = flag else {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: a bitmask flag must be a name"
            )));
        };
        match ty
            .member_by_name(name)
            .and_then(|member| member.literal_value())
        {
            Some(position) => set(&mut value, position),
            None => skipped.push(ConvertError::Unsupported {
                path: path.to_string(),
                reason: "a bitmask flag name the type does not declare",
            }),
        }
    }
    Ok((value, valid))
}

/// The flags of a bitmask from the `Value` and `ValidBits` of its
/// `OptionSet`: a valid bit takes the value written, every other keeps
/// `current` (OPC 10000-3 §8.40: "new value = (value & validBits) |
/// (current value & ~validBits)"). A valid bit at a position the bitmask
/// names no flag at is refused, as §8.40 refuses it.
pub(crate) fn bits_from_option_set(
    ty: &DynamicType,
    value: &[u8],
    valid: &[u8],
    current: Option<&DynamicData>,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    if value.len() != valid.len() {
        return Err(ConvertError::ShapeMismatch(format!(
            "{path}: an OptionSet whose Value and ValidBits differ in length"
        )));
    }
    let bit = |bits: &[u8], position: u32| {
        bits.get(position as usize / 8)
            .is_some_and(|octet| octet & (1 << (position % 8)) != 0)
    };
    let members = ty.members();
    for position in 0..u32::try_from(valid.len() * 8).unwrap_or(u32::MAX) {
        if bit(valid, position)
            && !members
                .iter()
                .any(|member| member.literal_value() == Some(position))
        {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: bit {position} names no flag of the bitmask"
            )));
        }
    }
    let was_set = |name: &str| match current {
        Some(DynamicData::Bits(flags)) => flags
            .iter()
            .any(|flag| matches!(flag, DynamicData::Text(set) if set == name)),
        _ => false,
    };
    let mut flags = Vec::new();
    for member in &members {
        let Some(position) = member.literal_value() else {
            continue;
        };
        let on = if bit(valid, position) {
            bit(value, position)
        } else {
            was_set(member.name())
        };
        if on {
            flags.push(DynamicData::Text(member.name().to_string()));
        }
    }
    Ok(DynamicData::Bits(flags))
}

/// The unscaled integer of the decimal text of a `fixed<P,S>` value (an
/// optional `-`, digits, an optional point and at most `scale` digits
/// after it).
fn fixed_unscaled(text: &str, scale: u8) -> Option<i128> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    if fraction.len() > usize::from(scale)
        || !whole
            .chars()
            .chain(fraction.chars())
            .all(|c| c.is_ascii_digit())
        || (whole.is_empty() && fraction.is_empty())
    {
        return None;
    }
    let mut value: i128 = 0;
    for c in whole
        .chars()
        .chain(fraction.chars())
        .chain(core::iter::repeat_n(
            '0',
            usize::from(scale) - fraction.len(),
        ))
    {
        value = value
            .checked_mul(10)?
            .checked_add(i128::from(c as u8 - b'0'))?;
    }
    Some(if negative { -value } else { value })
}

/// A `Decimal` (OPC 10000-6 §5.1.10) of `unscaled` × 10^-`scale`.
fn decimal(unscaled: i128, scale: u8) -> zerodds_opcua_gateway::data_value::ExtensionObject {
    let mut body = i16::from(scale).to_le_bytes().to_vec();
    let bytes = unscaled.to_le_bytes();
    // The shortest two's complement form: no byte that only repeats the
    // sign of the one before it.
    let mut len = bytes.len();
    while len > 1 {
        let top = bytes[len - 1];
        let next_sign = bytes[len - 2] & 0x80;
        if (top == 0 && next_sign == 0) || (top == 0xFF && next_sign != 0) {
            len -= 1;
        } else {
            break;
        }
    }
    body.extend_from_slice(&bytes[..len]);
    zerodds_opcua_gateway::data_value::ExtensionObject {
        type_id: zerodds_opcua_gateway::node_id::NodeId::numeric(0, 50),
        body: zerodds_opcua_gateway::data_value::ExtensionObjectBody::ByteString(body),
    }
}

/// The canonical text of a `fixed<digits, scale>` from a `Decimal`: `None`
/// for an ExtensionObject that is no Decimal, one whose value has more
/// fraction digits than `scale` holds without loss, or more than `digits`
/// digits.
fn fixed_from_decimal(
    object: &zerodds_opcua_gateway::data_value::ExtensionObject,
    digits: u8,
    scale: u8,
) -> Option<String> {
    use zerodds_opcua_gateway::data_value::ExtensionObjectBody;
    if object.type_id != zerodds_opcua_gateway::node_id::NodeId::numeric(0, 50) {
        return None;
    }
    let ExtensionObjectBody::ByteString(body) = &object.body else {
        return None;
    };
    // "If the length is less than or equal to 2 then the Decimal is an
    // invalid value".
    if body.len() <= 2 || body.len() > 2 + 16 {
        return None;
    }
    let decimal_scale = i16::from_le_bytes([body[0], body[1]]);
    let value_bytes = &body[2..];
    let fill = if value_bytes[value_bytes.len() - 1] & 0x80 != 0 {
        0xFF
    } else {
        0
    };
    let mut full = [fill; 16];
    full[..value_bytes.len()].copy_from_slice(value_bytes);
    let mut value = i128::from_le_bytes(full);
    let target = i16::from(scale);
    if decimal_scale > target {
        let divisor = 10i128.checked_pow(u32::try_from(decimal_scale - target).ok()?)?;
        if value % divisor != 0 {
            return None;
        }
        value /= divisor;
    } else if decimal_scale < target {
        value =
            value.checked_mul(10i128.checked_pow(u32::try_from(target - decimal_scale).ok()?)?)?;
    }
    let magnitude = value.unsigned_abs().to_string();
    if magnitude.len() > usize::from(digits) {
        return None;
    }
    let padded = format!("{magnitude:0>width$}", width = usize::from(scale) + 1);
    let (whole, fraction) = padded.split_at(padded.len() - usize::from(scale));
    let mut out = String::new();
    if value < 0 {
        out.push('-');
    }
    out.push_str(whole);
    if scale > 0 {
        out.push('.');
        out.push_str(fraction);
    }
    Some(out)
}

/// Table 9's zero value for `ty` — the `PublishedDataSet`'s initial fields
/// and [`unflatten`]'s `default` before any real sample has arrived.
///
/// # Errors
/// [`zerodds_wire_codec::ProbeError`] for a type kind Table 9 has no
/// default for.
pub fn default_data(ty: &DynamicType) -> Result<DynamicData, zerodds_wire_codec::ProbeError> {
    zerodds_wire_codec::default::of_type(ty.ast(), ty.id(), &ty.qualified_name())
}

/// Rebuilds a `DynamicData` value from a flat, path-named field list — the
/// inverse of [`flatten`], for the OPC-UA-write and PubSub-subscribe
/// directions. Fields the type declares but `fields` does not name keep
/// `default`'s value; `default` is normally the topic's last known sample
/// (so a partial write only changes the fields it names), never a value
/// this module invents.
///
/// # Errors
/// [`ConvertError::ShapeMismatch`] / [`ConvertError::UnknownMember`] if a
/// path or value does not fit the type.
pub fn unflatten(
    ty: &DynamicType,
    fields: &[FlatField],
    default: &DynamicData,
) -> Result<DynamicData, ConvertError> {
    let by_path: BTreeMap<&str, &Variant> =
        fields.iter().map(|f| (f.path.as_str(), &f.value)).collect();
    build_value(ty, "", &by_path, default, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, element
/// or alias; refused past `MAX_DEPTH`)
fn build_value(
    ty: &DynamicType,
    path: &str,
    fields: &BTreeMap<&str, &Variant>,
    default: &DynamicData,
    depth: usize,
) -> Result<DynamicData, ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep {
            path: path.to_string(),
        });
    }
    match ty.kind() {
        // The members by their names. An `@optional` member `default` does
        // not have stays absent unless `fields` writes under it; then it
        // starts from its type's zero value.
        TypeKind::Structure => {
            let mut out = Vec::with_capacity(ty.member_count() as usize);
            for member in ty.members() {
                let member_type = member.dynamic_type().map_err(|e| {
                    ConvertError::ShapeMismatch(format!("{path}.{}: {e:?}", member.name()))
                })?;
                let child_path = join(path, member.name());
                let member_default = match default.member(member.name()) {
                    Some(value) => value.clone(),
                    None if !writes_under(fields, &child_path) => continue,
                    None => default_data(&member_type)
                        .map_err(|e| ConvertError::ShapeMismatch(format!("{child_path}: {e}")))?,
                };
                out.push((
                    member.name().to_string(),
                    build_value(
                        &member_type,
                        &child_path,
                        fields,
                        &member_default,
                        depth + 1,
                    )?,
                ));
            }
            Ok(DynamicData::Aggregate(out))
        }
        TypeKind::Alias => {
            let target = ty.base_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: alias with no target"))
            })?;
            build_value(&target, path, fields, default, depth + 1)
        }
        // The inverse of `walk_value`'s bitset case: a bitfield `fields`
        // names takes the number written, the others keep `default`'s.
        TypeKind::Bitset => {
            let DynamicData::Bitfields(defaults) = default else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected the bitfields of a bitset"
                )));
            };
            let mut out = Vec::with_capacity(defaults.len());
            for (name, before) in defaults {
                let child_path = join(path, name);
                let value = match fields.get(child_path.as_str()) {
                    None => before.clone(),
                    Some(variant) => {
                        let written = variant.value.first().ok_or_else(|| {
                            ConvertError::ShapeMismatch(format!(
                                "{child_path}: an empty Variant has no scalar value"
                            ))
                        })?;
                        let number = match written {
                            VariantValue::Boolean(v) => i128::from(*v),
                            other => as_i128(other, &child_path)?,
                        };
                        match before {
                            DynamicData::Integer { kind, .. } => DynamicData::Integer {
                                kind: *kind,
                                value: number,
                            },
                            _ => DynamicData::Integer {
                                kind: None,
                                value: number,
                            },
                        }
                    }
                };
                out.push((name.clone(), value));
            }
            Ok(DynamicData::Bitfields(out))
        }
        // The inverse of `walk_value`'s collection case for elements that
        // are structures or unions: every element the collection has is
        // rebuilt from the leaves under its `<TypeName>_<index>`. A write
        // changes the elements that are there; it adds none.
        TypeKind::Array | TypeKind::Sequence => {
            let Some(element_ty) = ty.element_type() else {
                return Ok(default.clone());
            };
            let aggregate = resolved_alias(&element_ty, path)?;
            let dims = array_dims(ty, path)?;
            if !matches!(aggregate.kind(), TypeKind::Structure | TypeKind::Union)
                && !is_collection(&aggregate)
            {
                // A collection of scalars is one leaf (the caller reads it
                // whole).
                return match fields.get(path) {
                    Some(variant) => collection_from_variant(ty, &aggregate, variant, path),
                    None => Ok(default.clone()),
                };
            }
            let items = flat_items(default, dims.len().max(1), path)?;
            let type_name = element_type_name(&aggregate, path);
            let mut out = Vec::with_capacity(items.len());
            for (index, item) in items.into_iter().enumerate() {
                let child_path = join(path, &element_name(&type_name, index, &dims));
                out.push(build_value(
                    &aggregate,
                    &child_path,
                    fields,
                    item,
                    depth + 1,
                )?);
            }
            Ok(nest(out, &dims))
        }
        // The inverse of `walk_value`'s union case: the discriminator from
        // `SwitchField`, or, when only a case is written, one that selects
        // that case; the active case from the leaves under its name.
        TypeKind::Union => build_union(ty, path, fields, default, depth),
        // The inverse of `walk_value`'s `TypeKind::Map` case: rebuilds
        // every key `fields` touches under this path, plus every key
        // `default` already has that `fields` leaves alone (so a partial
        // write to one map entry never drops another).
        TypeKind::Map => {
            let Some(key_ty) = ty.key_element_type() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: map with no key type"
                )));
            };
            let value_ty = ty.element_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: map with no value type"))
            })?;
            let default_entries = data_map_entries(default, &key_ty, path)?;
            let prefix = std::format!("{path}.");
            let mut keys: Vec<String> = Vec::new();
            for field_path in fields.keys() {
                if let Some(rest) = field_path.strip_prefix(prefix.as_str()) {
                    let key_segment = rest.split('.').next().unwrap_or(rest);
                    if !keys.iter().any(|k| k == key_segment) {
                        keys.push(key_segment.to_string());
                    }
                }
            }
            for (existing_key, _) in &default_entries {
                if !keys.iter().any(|k| k == existing_key) {
                    keys.push(existing_key.clone());
                }
            }
            // Only reached for a map key this map's own `default` never
            // saw and `fields` names with a non-scalar value type this
            // function recurses into further rather than reading
            // directly — the same "should not normally be reached for a
            // well-formed pair" placeholder the Structure case above
            // uses.
            let fallback = DynamicData::Boolean(false);
            let mut out = Vec::with_capacity(keys.len());
            for key_segment in keys {
                let key_data = parse_map_key(&key_ty, &key_segment, path)?;
                let child_path = join(path, &key_segment);
                let entry_default = default_entries
                    .iter()
                    .find(|(k, _)| k == &key_segment)
                    .map_or(&fallback, |(_, v)| v);
                let value_data =
                    build_value(&value_ty, &child_path, fields, entry_default, depth + 1)?;
                out.push(DynamicData::Collection(std::vec![key_data, value_data]));
            }
            Ok(DynamicData::Collection(out))
        }
        TypeKind::Bitmask => match fields.get(path) {
            Some(variant) => match variant.value.first() {
                Some(VariantValue::ExtensionObject(object)) => {
                    let (value, valid) = crate::ua_structure::option_set_of(object, path)?;
                    bits_from_option_set(ty, &value, &valid, Some(default), path)
                }
                _ => scalar_from_variant(ty, variant, path),
            },
            None => Ok(default.clone()),
        },
        _ => match fields.get(path) {
            Some(variant) => scalar_from_variant(ty, variant, path),
            None => Ok(default.clone()),
        },
    }
}

/// Whether `fields` writes `path` or a path under it.
fn writes_under(fields: &BTreeMap<&str, &Variant>, path: &str) -> bool {
    fields.keys().any(|written| {
        *written == path
            || written
                .strip_prefix(path)
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

/// zerodds-lint: recursion-depth 64 (through `build_value`, one level per
/// nested member, element or alias; refused past `MAX_DEPTH`)
fn build_union(
    ty: &DynamicType,
    path: &str,
    fields: &BTreeMap<&str, &Variant>,
    default: &DynamicData,
    depth: usize,
) -> Result<DynamicData, ConvertError> {
    let switch_path = join(path, "SwitchField");
    let written_switch = fields.get(switch_path.as_str());
    let written_case = ty
        .members()
        .into_iter()
        .find(|member| writes_under(fields, &join(path, member.name())));
    if written_switch.is_none() && written_case.is_none() {
        return Ok(default.clone());
    }
    let discriminator_type = ty.discriminator_type().ok_or_else(|| {
        ConvertError::ShapeMismatch(format!("{path}: a union with no discriminator type"))
    })?;
    let discriminator_type = resolved_alias(&discriminator_type, path)?;
    let discriminator = match (written_switch, &written_case) {
        (Some(variant), _) => scalar_from_variant(&discriminator_type, variant, &switch_path)?,
        (None, Some(case)) => {
            let value = crate::ua_structure::label_of(ty, case, path)?;
            crate::ua_structure::discriminator_data(&discriminator_type, value, path)?
        }
        (None, None) => return Ok(default.clone()),
    };
    let label = crate::ua_structure::label_value(&discriminator_type, &discriminator, path)?;
    let mut out = vec![("_d".to_string(), discriminator)];
    if let Some(case) = crate::ua_structure::case_of(ty, label) {
        let case_type = case
            .dynamic_type()
            .map_err(|e| ConvertError::ShapeMismatch(format!("{path}.{}: {e:?}", case.name())))?;
        let case_path = join(path, case.name());
        let case_default = match default.member(case.name()) {
            Some(value) => value.clone(),
            None => default_data(&case_type)
                .map_err(|e| ConvertError::ShapeMismatch(format!("{case_path}: {e}")))?,
        };
        out.push((
            case.name().to_string(),
            build_value(&case_type, &case_path, fields, &case_default, depth + 1)?,
        ));
    }
    Ok(DynamicData::Aggregate(out))
}

/// The collection a Variant written to an array or a sequence of scalars
/// holds: each of its values as an element of `element_ty`. An array takes
/// as many values as it has elements.
pub(crate) fn collection_from_variant(
    ty: &DynamicType,
    element_ty: &DynamicType,
    variant: &Variant,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    if matches!(ty.kind(), TypeKind::Array) {
        let elements: u64 = ty
            .descriptor()
            .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e:?}")))?
            .bound
            .iter()
            .map(|dim| u64::from(*dim))
            .product();
        if variant.value.len() as u64 != elements {
            return Err(ConvertError::ShapeMismatch(format!(
                "{path}: the array has {elements} elements, the value written {}",
                variant.value.len()
            )));
        }
    }
    let items = variant
        .value
        .iter()
        .map(|value| scalar_from_variant(element_ty, &Variant::scalar(value.clone()), path))
        .collect::<Result<Vec<_>, ConvertError>>()?;
    Ok(nest(items, &array_dims(ty, path)?))
}

/// Extracts `default`'s own map entries as `(key string, value data)`
/// pairs — `default.items()` empty/absent (Table 9's zero value for a map
/// is the empty map) yields none, not an error.
fn data_map_entries(
    default: &DynamicData,
    key_ty: &DynamicType,
    path: &str,
) -> Result<Vec<(String, DynamicData)>, ConvertError> {
    let Some(items) = default.items() else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(items.len());
    for entry in items {
        let Some(entry_items) = entry.items() else {
            continue;
        };
        let [key_data, value_data] = entry_items else {
            continue;
        };
        out.push((map_key_string(key_ty, key_data, path)?, value_data.clone()));
    }
    Ok(out)
}

pub(crate) fn scalar_from_variant(
    ty: &DynamicType,
    variant: &Variant,
    path: &str,
) -> Result<DynamicData, ConvertError> {
    let value = variant.value.first().ok_or_else(|| {
        ConvertError::ShapeMismatch(format!("{path}: an empty Variant has no scalar value"))
    })?;
    match (ty.kind(), value) {
        (TypeKind::Boolean, VariantValue::Boolean(v)) => Ok(DynamicData::Boolean(*v)),
        (TypeKind::Int8, v) => Ok(int_data(PrimitiveKind::Int8, as_i128(v, path)?)),
        (TypeKind::UInt8 | TypeKind::Byte, v) => {
            Ok(int_data(PrimitiveKind::UInt8, as_i128(v, path)?))
        }
        (TypeKind::Int16, v) => Ok(int_data(PrimitiveKind::Int16, as_i128(v, path)?)),
        (TypeKind::UInt16, v) => Ok(int_data(PrimitiveKind::UInt16, as_i128(v, path)?)),
        (TypeKind::Int32, v) => Ok(int_data(PrimitiveKind::Int32, as_i128(v, path)?)),
        (TypeKind::UInt32, v) => Ok(int_data(PrimitiveKind::UInt32, as_i128(v, path)?)),
        (TypeKind::Int64, v) => Ok(int_data(PrimitiveKind::Int64, as_i128(v, path)?)),
        (TypeKind::UInt64, v) => Ok(int_data(PrimitiveKind::UInt64, as_i128(v, path)?)),
        (TypeKind::Float32, VariantValue::Float(v)) => Ok(DynamicData::Float {
            kind: Some(PrimitiveKind::Float32),
            value: f64::from(*v),
        }),
        (TypeKind::Float64, VariantValue::Double(v)) => Ok(DynamicData::Float {
            kind: Some(PrimitiveKind::Float64),
            value: *v,
        }),
        (TypeKind::NoType, VariantValue::ExtensionObject(object)) => {
            let (digits, scale) = crate::dynamic_type::fixed_of(ty).ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: an ExtensionObject for no fixed type"))
            })?;
            let text = fixed_from_decimal(object, digits, scale).ok_or_else(|| {
                ConvertError::ShapeMismatch(format!(
                    "{path}: not a Decimal that fits fixed<{digits},{scale}>"
                ))
            })?;
            Ok(DynamicData::Fixed(text))
        }
        (TypeKind::Float128, VariantValue::ByteString(octets)) => {
            let octets: [u8; 16] = octets.as_slice().try_into().map_err(|_| {
                ConvertError::ShapeMismatch(format!(
                    "{path}: a long double is a ByteString of 16 octets, got {}",
                    octets.len()
                ))
            })?;
            Ok(DynamicData::LongDouble(
                zerodds_types::dynamic::Float128::from_le_bytes(octets),
            ))
        }
        (TypeKind::Char8, VariantValue::Byte(v)) => {
            Ok(DynamicData::character(PrimitiveKind::Char, *v as char))
        }
        (TypeKind::String8, VariantValue::String(v)) => Ok(DynamicData::Text(v.clone())),
        (TypeKind::Char16, VariantValue::UInt16(v)) => Ok(DynamicData::character(
            PrimitiveKind::WChar,
            char::from_u32(u32::from(*v)).unwrap_or('\u{FFFD}'),
        )),
        (TypeKind::String16, VariantValue::String(v)) => Ok(DynamicData::Text(v.clone())),
        (TypeKind::Enumeration, VariantValue::Int32(ordinal)) => {
            let wanted = u32::try_from(*ordinal).ok();
            let member = ty
                .members()
                .into_iter()
                .find(|member| wanted.is_some() && member.literal_value() == wanted)
                .ok_or_else(|| {
                    ConvertError::ShapeMismatch(format!(
                        "{path}: ordinal {ordinal} names no enumerator"
                    ))
                })?;
            Ok(DynamicData::Enumerator(member.name().to_string()))
        }
        (TypeKind::Bitmask, VariantValue::ExtensionObject(object)) => {
            let (value, valid) = crate::ua_structure::option_set_of(object, path)?;
            bits_from_option_set(ty, &value, &valid, None, path)
        }
        (TypeKind::Bitmask, VariantValue::UInt32(bits)) => {
            let mut flags = Vec::new();
            for member in ty.members() {
                if bits & (1u32 << member.literal_value().unwrap_or(0)) != 0 {
                    flags.push(DynamicData::Text(member.name().to_string()));
                }
            }
            Ok(DynamicData::Bits(flags))
        }
        (kind, value) => Err(ConvertError::ShapeMismatch(std::format!(
            "{path}: type kind {kind:?} does not match Variant value {value:?}"
        ))),
    }
}

fn int_data(kind: PrimitiveKind, value: i128) -> DynamicData {
    DynamicData::Integer {
        kind: Some(kind),
        value,
    }
}

fn as_i128(value: &VariantValue, path: &str) -> Result<i128, ConvertError> {
    Ok(match value {
        VariantValue::SByte(v) => i128::from(*v),
        VariantValue::Byte(v) => i128::from(*v),
        VariantValue::Int16(v) => i128::from(*v),
        VariantValue::UInt16(v) => i128::from(*v),
        VariantValue::Int32(v) => i128::from(*v),
        VariantValue::UInt32(v) => i128::from(*v),
        VariantValue::Int64(v) => i128::from(*v),
        VariantValue::UInt64(v) => i128::from(*v),
        other => {
            return Err(ConvertError::ShapeMismatch(std::format!(
                "{path}: expected an integer Variant, got {other:?}"
            )));
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use zerodds_ast::Ast;

    fn ast_of(idl: &str) -> Arc<Ast> {
        Arc::new(
            zerodds_wire_probe::encode::model_from_idl(idl, "dynamic_value_test.idl")
                .expect("idl parses"),
        )
    }

    #[test]
    fn opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back() {
        let ast = ast_of("struct P { long x; long y; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![
            (
                "x".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 3,
                },
            ),
            (
                "y".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 4,
                },
            ),
        ]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].path, "x");
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Int32(3)));
        assert_eq!(fields[1].path, "y");
        assert_eq!(fields[1].value, Variant::scalar(VariantValue::Int32(4)));

        // The other way: the two variables written back become the struct.
        let rebuilt = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(rebuilt, data);
    }

    #[test]
    fn opcuabridge_5_3_a_value_nested_past_the_bound_is_refused_not_walked() {
        // S0 { long v; }, S1 { S0 m; }, …: one struct level per step.
        let levels = MAX_DEPTH + 2;
        let mut idl = "struct S0 { long v; };".to_string();
        for i in 1..=levels {
            idl.push_str(&format!(" struct S{i} {{ S{} m; }};", i - 1));
        }
        let ast = ast_of(&idl);
        let ty = DynamicType::find(&ast, &format!("S{levels}")).expect("type");
        let mut data = DynamicData::Aggregate(vec![(
            "v".to_string(),
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 1,
            },
        )]);
        for _ in 1..=levels {
            data = DynamicData::Aggregate(vec![("m".to_string(), data)]);
        }
        assert!(matches!(
            flatten(&ty, &data),
            Err(ConvertError::TooDeep { .. })
        ));
        assert!(matches!(
            unflatten(&ty, &[], &data),
            Err(ConvertError::TooDeep { .. })
        ));
    }

    #[test]
    fn opcuabridge_5_3_nested_struct_flattens_with_dotted_paths() {
        let ast = ast_of("struct Inner { long v; }; struct Outer { Inner i; };");
        let ty = DynamicType::find(&ast, "Outer").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "i".to_string(),
            DynamicData::Aggregate(vec![(
                "v".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 9,
                },
            )]),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields[0].path, "i.v");
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Int32(9)));
    }

    #[test]
    fn opcuabridge_5_3_partial_unflatten_keeps_the_rest_of_the_default() {
        let ast = ast_of("struct P { long x; long y; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let default = DynamicData::Aggregate(vec![
            (
                "x".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 1,
                },
            ),
            (
                "y".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 2,
                },
            ),
        ]);
        let fields = vec![FlatField {
            path: "x".to_string(),
            value: Variant::scalar(VariantValue::Int32(100)),
        }];
        let rebuilt = unflatten(&ty, &fields, &default).expect("unflatten");
        let DynamicData::Aggregate(members) = rebuilt else {
            panic!("expected aggregate");
        };
        assert_eq!(
            members[0].1,
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 100
            }
        );
        assert_eq!(
            members[1].1,
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 2
            }
        );
    }

    #[test]
    fn opcuabridge_5_3_enum_flattens_to_its_ordinal_and_back() {
        let ast = ast_of("enum Color { RED, GREEN, BLUE }; struct P { Color c; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "c".to_string(),
            DynamicData::Enumerator("GREEN".to_string()),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Int32(1)));
        let rebuilt = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(rebuilt, data);
    }

    #[test]
    fn opcuabridge_5_3_sequence_of_primitives_is_one_variant() {
        let ast = ast_of("struct P { sequence<long, 4> items; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "items".to_string(),
            DynamicData::Collection(vec![
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 1,
                },
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value: 2,
                },
            ]),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields[0].path, "items");
        assert!(fields[0].value.is_1d_array());
        assert_eq!(fields[0].value.value.len(), 2);
    }

    /// Gateway 1.0 §9.2.5.2.2.3: each element of a sequence of structures
    /// is a Variable `<TypeName>_<index>` with the members under it; the
    /// leaves give the sequence back, and a write to one member of one
    /// element changes that member.
    #[test]
    fn opcuabridge_5_3_a_sequence_of_structs_is_one_variable_per_element_and_back() {
        let ast = ast_of("struct E { long v; }; struct P { sequence<E, 4> items; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let item = |value: i128| {
            DynamicData::Aggregate(vec![(
                "v".to_string(),
                DynamicData::Integer {
                    kind: Some(PrimitiveKind::Int32),
                    value,
                },
            )])
        };
        let sample = |items: Vec<DynamicData>| {
            DynamicData::Aggregate(vec![("items".to_string(), DynamicData::Collection(items))])
        };
        let data = sample(vec![item(7), item(8)]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let seen: Vec<(&str, &Variant)> =
            fields.iter().map(|f| (f.path.as_str(), &f.value)).collect();
        assert_eq!(
            seen,
            [
                ("items.E_0.v", &Variant::scalar(VariantValue::Int32(7))),
                ("items.E_1.v", &Variant::scalar(VariantValue::Int32(8))),
            ]
        );
        assert_eq!(unflatten(&ty, &fields, &data).expect("unflatten"), data);

        let written = FlatField {
            path: "items.E_1.v".to_string(),
            value: Variant::scalar(VariantValue::Int32(80)),
        };
        assert_eq!(
            unflatten(&ty, &[written], &data).expect("unflatten"),
            sample(vec![item(7), item(80)])
        );
    }

    /// Gateway 1.0 §9.2.5.1.2.3: `<TypeName>_<index>`, and for several
    /// dimensions the position in each (`<TypeName>_1_2_3`).
    #[test]
    fn opcuabridge_5_3_an_element_is_named_by_its_position_in_each_dimension() {
        assert_eq!(element_name("Shape", 3, &[]), "Shape_3");
        assert_eq!(element_name("Shape", 3, &[4]), "Shape_3");
        assert_eq!(element_name("m::Shape", 5, &[2, 3]), "Shape_1_2");
        assert_eq!(element_name("Shape", 23, &[2, 3, 4]), "Shape_1_2_3");
        assert_eq!(element_name("Shape", 0, &[2, 3, 4]), "Shape_0_0_0");
    }

    /// A Variant written to a sequence of numbers replaces the sequence; an
    /// array takes as many values as it has elements.
    #[test]
    fn opcuabridge_5_3_a_write_to_a_collection_of_numbers_replaces_it() {
        let ast = ast_of("struct P { sequence<long, 4> items; long pair[2]; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let numbers = |values: &[i128]| {
            DynamicData::Collection(
                values
                    .iter()
                    .map(|value| DynamicData::Integer {
                        kind: Some(PrimitiveKind::Int32),
                        value: *value,
                    })
                    .collect(),
            )
        };
        let sample = |items: &[i128], pair: &[i128]| {
            DynamicData::Aggregate(vec![
                ("items".to_string(), numbers(items)),
                ("pair".to_string(), numbers(pair)),
            ])
        };
        let data = sample(&[1, 2], &[0, 0]);
        let variant = |values: &[i32]| Variant {
            array_dimensions: vec![values.len() as u32],
            value: values.iter().map(|v| VariantValue::Int32(*v)).collect(),
        };
        let write = |path: &str, values: &[i32]| FlatField {
            path: path.to_string(),
            value: variant(values),
        };
        assert_eq!(
            unflatten(&ty, &[write("items", &[5, 6, 7])], &data).expect("unflatten"),
            sample(&[5, 6, 7], &[0, 0])
        );
        assert_eq!(
            unflatten(&ty, &[write("pair", &[9, 8])], &data).expect("unflatten"),
            sample(&[1, 2], &[9, 8])
        );
        assert!(matches!(
            unflatten(&ty, &[write("pair", &[9, 8, 7])], &data),
            Err(ConvertError::ShapeMismatch(_))
        ));
    }

    #[test]
    fn opcuabridge_5_3_wide_char_maps_to_uint16_and_back() {
        let ast = ast_of("struct P { wchar c; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        // A `DynamicData::Character` with kind `WChar` — built the same way
        // the codec's `Sink::character` would.
        let data = DynamicData::Aggregate(vec![(
            "c".to_string(),
            DynamicData::character(PrimitiveKind::WChar, 'x'),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(
            fields[0].value,
            Variant::scalar(VariantValue::UInt16('x' as u16))
        );

        let back = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(back, data);
    }

    #[test]
    fn opcuabridge_5_3_wide_string_maps_to_string_and_back() {
        let ast = ast_of("struct P { wstring s; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "s".to_string(),
            DynamicData::Text("héllo".to_string()),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(
            fields[0].value,
            Variant::scalar(VariantValue::String("héllo".to_string()))
        );
        let back = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(back, data);
    }

    /// Gateway 1.0 §9.2.1: a `long double` is a `ByteString` of length 16,
    /// here the binary128 value least significant octet first; it comes
    /// back as the same value, and a `ByteString` of another length is
    /// refused.
    #[test]
    fn opcuabridge_5_3_long_double_is_a_byte_string_of_sixteen_and_back() {
        use zerodds_types::dynamic::Float128;
        let ast = ast_of("struct P { long double v; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        // 1.0 as binary128: sign 0, exponent 0x3FFF, mantissa 0.
        let one = Float128::from_bits(0x3FFF_u128 << 112);
        let data = DynamicData::Aggregate(vec![("v".to_string(), DynamicData::LongDouble(one))]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let mut octets = vec![0u8; 16];
        octets[14] = 0xFF;
        octets[15] = 0x3F;
        assert_eq!(
            fields[0].value,
            Variant::scalar(VariantValue::ByteString(octets))
        );
        let back = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(back, data);

        let short = FlatField {
            path: "v".to_string(),
            value: Variant::scalar(VariantValue::ByteString(vec![0; 8])),
        };
        assert!(matches!(
            unflatten(&ty, &[short], &data),
            Err(ConvertError::ShapeMismatch(_))
        ));
    }

    /// OPC 10000-6 §5.1.10: a `fixed<P,S>` is a `Decimal` (ExtensionObject
    /// of `i=50`: Int16 scale, then the unscaled value in the fewest
    /// two's complement bytes); it comes back as the same value, a Decimal
    /// of another scale is rescaled, one that does not fit is refused.
    #[test]
    fn opcuabridge_5_3_a_fixed_is_a_decimal_and_back() {
        use zerodds_opcua_gateway::data_value::{ExtensionObject, ExtensionObjectBody};
        use zerodds_opcua_gateway::node_id::NodeId;
        let ast = ast_of("struct P { fixed<7,3> price; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let sample = |text: &str| {
            DynamicData::Aggregate(vec![(
                "price".to_string(),
                DynamicData::Fixed(text.to_string()),
            )])
        };
        let data = sample("-1234.567");
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let VariantValue::ExtensionObject(object) = &fields[0].value.value[0] else {
            panic!("{:?}", fields[0].value);
        };
        assert_eq!(object.type_id, NodeId::numeric(0, 50));
        // Scale 3, then -1234567 = 0xED2979 negative: 0x79 0x29 0xED.
        assert_eq!(
            object.body,
            ExtensionObjectBody::ByteString(vec![3, 0, 0x79, 0x29, 0xED])
        );
        assert_eq!(unflatten(&ty, &fields, &data).expect("unflatten"), data);

        let decimal = |scale: i16, value: &[u8]| FlatField {
            path: "price".to_string(),
            value: Variant::scalar(VariantValue::ExtensionObject(ExtensionObject {
                type_id: NodeId::numeric(0, 50),
                body: ExtensionObjectBody::ByteString(
                    scale.to_le_bytes().iter().chain(value).copied().collect(),
                ),
            })),
        };
        // 125 × 10^-1 = 12.5 → 12.500.
        assert_eq!(
            unflatten(&ty, &[decimal(1, &[125])], &data).expect("unflatten"),
            sample("12.500")
        );
        // 1 × 10^-4 does not fit three fraction digits; 10^8 not seven.
        assert!(unflatten(&ty, &[decimal(4, &[1])], &data).is_err());
        assert!(unflatten(&ty, &[decimal(0, &[0x00, 0xE1, 0xF5, 0x05])], &data).is_err());
        // Two octets are no Decimal.
        assert!(unflatten(&ty, &[decimal(0, &[])], &data).is_err());
    }

    /// Spec §5.3: a bitset is one leaf per named bitfield, a number of the
    /// bitfield's holder type; a write to one bitfield keeps the others.
    #[test]
    fn opcuabridge_5_3_a_bitset_is_one_leaf_per_bitfield_and_back() {
        let ast =
            ast_of("bitset B { bitfield<3> a; bitfield<2>; bitfield<12> b; }; struct P { B v; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let number = |value: i128| DynamicData::Integer { kind: None, value };
        let bits = |a: i128, b: i128| {
            DynamicData::Aggregate(vec![(
                "v".to_string(),
                DynamicData::Bitfields(vec![
                    ("a".to_string(), number(a)),
                    ("b".to_string(), number(b)),
                ]),
            )])
        };
        let data = bits(5, 1000);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let seen: Vec<(&str, i128)> = fields
            .iter()
            .map(|f| {
                (
                    f.path.as_str(),
                    as_i128(&f.value.value[0], &f.path).expect("a number"),
                )
            })
            .collect();
        assert_eq!(seen, [("v.a", 5), ("v.b", 1000)]);
        assert_eq!(unflatten(&ty, &fields, &data).expect("unflatten"), data);

        // A write to `a` alone keeps `b`.
        let mut written = fields[0].clone();
        written.value = match &fields[0].value.value[0] {
            VariantValue::Byte(_) => Variant::scalar(VariantValue::Byte(2)),
            VariantValue::SByte(_) => Variant::scalar(VariantValue::SByte(2)),
            other => panic!("the holder of a three-bit bitfield is an octet, got {other:?}"),
        };
        assert_eq!(
            unflatten(&ty, &[written], &data).expect("unflatten"),
            bits(2, 1000)
        );
    }

    #[test]
    fn opcuabridge_5_3_map_of_primitives_decomposes_to_one_leaf_per_key_and_back() {
        let ast = ast_of("struct P { map<string, long> m; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "m".to_string(),
            DynamicData::Collection(vec![
                DynamicData::Collection(vec![
                    DynamicData::Text("a".to_string()),
                    DynamicData::integer(PrimitiveKind::Int32, 1),
                ]),
                DynamicData::Collection(vec![
                    DynamicData::Text("b".to_string()),
                    DynamicData::integer(PrimitiveKind::Int32, 2),
                ]),
            ]),
        )]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields.len(), 2);
        let by_path: std::collections::BTreeMap<&str, &Variant> =
            fields.iter().map(|f| (f.path.as_str(), &f.value)).collect();
        assert_eq!(*by_path["m.a"], Variant::scalar(VariantValue::Int32(1)));
        assert_eq!(*by_path["m.b"], Variant::scalar(VariantValue::Int32(2)));

        let back = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(back, data);
    }

    #[test]
    fn opcuabridge_5_3_map_partial_write_keeps_the_untouched_key() {
        let ast = ast_of("struct P { map<string, long> m; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "m".to_string(),
            DynamicData::Collection(vec![
                DynamicData::Collection(vec![
                    DynamicData::Text("a".to_string()),
                    DynamicData::integer(PrimitiveKind::Int32, 1),
                ]),
                DynamicData::Collection(vec![
                    DynamicData::Text("b".to_string()),
                    DynamicData::integer(PrimitiveKind::Int32, 2),
                ]),
            ]),
        )]);
        // Only "m.a" is written; "m.b" must survive unchanged.
        let fields = vec![FlatField {
            path: "m.a".to_string(),
            value: Variant::scalar(VariantValue::Int32(99)),
        }];
        let back = unflatten(&ty, &fields, &data).expect("unflatten");
        let DynamicData::Aggregate(members) = &back else {
            panic!("expected aggregate");
        };
        let (_, m) = &members[0];
        let DynamicData::Collection(entries) = m else {
            panic!("expected map collection");
        };
        assert_eq!(entries.len(), 2);
        let mut seen_a = false;
        let mut seen_b = false;
        for entry in entries {
            let DynamicData::Collection(kv) = entry else {
                panic!("expected entry collection");
            };
            let [DynamicData::Text(k), DynamicData::Integer { value, .. }] = kv.as_slice() else {
                panic!("expected [Text, Integer]");
            };
            if k == "a" {
                assert_eq!(*value, 99);
                seen_a = true;
            } else if k == "b" {
                assert_eq!(*value, 2);
                seen_b = true;
            }
        }
        assert!(seen_a && seen_b);
    }

    #[test]
    fn opcuabridge_5_3_a_union_is_its_switch_field_and_its_active_case_and_back() {
        let ast = ast_of(
            "union U switch (long) { case 1: long a; case 2: string b; }; \
             struct S { U pick; long n; };",
        );
        let ty = DynamicType::find(&ast, "S").expect("type");
        let data = default_data(&ty).expect("default");
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let paths: Vec<&str> = fields.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["pick.SwitchField", "n"]);
        // A write to a case that is not active makes it the active one.
        let written = unflatten(
            &ty,
            &[FlatField {
                path: "pick.b".into(),
                value: Variant::scalar(VariantValue::String("x".into())),
            }],
            &data,
        )
        .expect("unflatten");
        let (fields, _) = flatten(&ty, &written).expect("flatten");
        assert_eq!(fields[0].path, "pick.SwitchField");
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Int32(2)));
        assert_eq!(fields[1].path, "pick.b");
        assert_eq!(
            fields[1].value,
            Variant::scalar(VariantValue::String("x".into()))
        );
        // The discriminator written selects the case.
        let switched = unflatten(
            &ty,
            &[FlatField {
                path: "pick.SwitchField".into(),
                value: Variant::scalar(VariantValue::Int32(1)),
            }],
            &written,
        )
        .expect("unflatten");
        let (fields, _) = flatten(&ty, &switched).expect("flatten");
        assert_eq!(fields[1].path, "pick.a");
    }

    #[test]
    fn opcuabridge_5_3_an_absent_optional_member_has_no_leaf_until_it_is_written() {
        let ast = ast_of("struct O { long a; @optional long b; };");
        let ty = DynamicType::find(&ast, "O").expect("type");
        let data = default_data(&ty).expect("default");
        let (fields, _) = flatten(&ty, &data).expect("flatten");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].path, "a");
        let written = unflatten(
            &ty,
            &[FlatField {
                path: "b".into(),
                value: Variant::scalar(VariantValue::Int32(5)),
            }],
            &data,
        )
        .expect("unflatten");
        let (fields, _) = flatten(&ty, &written).expect("flatten");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].value, Variant::scalar(VariantValue::Int32(5)));
        // A member the type does not have is refused.
        let foreign = DynamicData::Aggregate(vec![(
            "z".into(),
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 1,
            },
        )]);
        assert!(matches!(
            flatten(&ty, &foreign),
            Err(ConvertError::UnknownMember { .. })
        ));
    }

    #[test]
    fn opcuabridge_5_3_an_array_of_two_dimensions_is_one_leaf_and_back() {
        let ast = ast_of("struct G { long grid[2][3]; };");
        let ty = DynamicType::find(&ast, "G").expect("type");
        let data = default_data(&ty).expect("default");
        let (fields, _) = flatten(&ty, &data).expect("flatten");
        assert_eq!(fields[0].value.array_dimensions, vec![2, 3]);
        assert_eq!(fields[0].value.value.len(), 6);
        let values: Vec<VariantValue> = (0..6).map(VariantValue::Int32).collect();
        let written = unflatten(
            &ty,
            &[FlatField {
                path: "grid".into(),
                value: Variant {
                    array_dimensions: vec![2, 3],
                    value: values.clone(),
                },
            }],
            &data,
        )
        .expect("unflatten");
        // One collection per dimension, the last dimension fastest.
        let grid = written.member("grid").expect("grid");
        let rows = grid.items().expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1].items().expect("row")[0],
            DynamicData::Integer {
                kind: Some(PrimitiveKind::Int32),
                value: 3
            }
        );
        let (fields, _) = flatten(&ty, &written).expect("flatten");
        assert_eq!(fields[0].value.value, values);
    }

    #[test]
    fn opcuabridge_5_3_a_bitmask_is_an_option_set_and_a_write_keeps_the_bits_not_valid() {
        // Gateway 1.0 Table 9.12, OPC 10000-3 §8.40.
        let ast =
            ast_of("@bit_bound(12) bitmask Flags { A, B, C }; struct FlagHolder { Flags f; };");
        let ty = DynamicType::find(&ast, "FlagHolder").expect("type");
        let data = DynamicData::Aggregate(vec![(
            "f".into(),
            DynamicData::Bits(vec![
                DynamicData::Text("A".into()),
                DynamicData::Text("C".into()),
            ]),
        )]);
        let (fields, _) = flatten_in(&ty, &data, 2).expect("flatten");
        let VariantValue::ExtensionObject(object) = &fields[0].value.value[0] else {
            panic!("not an ExtensionObject: {:?}", fields[0].value);
        };
        assert_eq!(object.type_id, crate::ua_types::encoding_id(2, "Flags"));
        let (value, valid) = crate::ua_structure::option_set_of(object, "f").expect("option set");
        assert_eq!(value, vec![0b101, 0]);
        assert_eq!(valid, vec![0b111, 0]);
        // B on, A off; C is not valid in the write and keeps its value.
        let mut w = zerodds_opcua_pubsub::UaWriter::with_capacity(16);
        crate::ua_structure::write_byte_string(&mut w, &[0b010, 0]);
        crate::ua_structure::write_byte_string(&mut w, &[0b011, 0]);
        let written = unflatten(
            &ty,
            &[FlatField {
                path: "f".into(),
                value: Variant::scalar(VariantValue::ExtensionObject(
                    zerodds_opcua_gateway::data_value::ExtensionObject {
                        type_id: object.type_id.clone(),
                        body: zerodds_opcua_gateway::data_value::ExtensionObjectBody::ByteString(
                            w.into_vec(),
                        ),
                    },
                )),
            }],
            &data,
        )
        .expect("unflatten");
        assert_eq!(
            written.member("f"),
            Some(&DynamicData::Bits(vec![
                DynamicData::Text("B".into()),
                DynamicData::Text("C".into())
            ]))
        );
        // A valid bit at a position no flag has is refused (§8.40).
        let mut w = zerodds_opcua_pubsub::UaWriter::with_capacity(16);
        crate::ua_structure::write_byte_string(&mut w, &[0, 0]);
        crate::ua_structure::write_byte_string(&mut w, &[0b1000, 0]);
        assert!(
            unflatten(
                &ty,
                &[FlatField {
                    path: "f".into(),
                    value: Variant::scalar(VariantValue::ExtensionObject(
                        zerodds_opcua_gateway::data_value::ExtensionObject {
                            type_id: object.type_id.clone(),
                            body:
                                zerodds_opcua_gateway::data_value::ExtensionObjectBody::ByteString(
                                    w.into_vec()
                                ),
                        }
                    )),
                }],
                &data,
            )
            .is_err()
        );
    }

    #[test]
    fn opcuabridge_5_3_a_sequence_of_sequences_is_a_variable_per_inner_sequence_and_back() {
        // Gateway 1.0 §9.2.5.2.2.5: `<SequenceName>_<index>`.
        let ast = ast_of(
            "struct S { sequence<sequence<long>> rows; sequence<map<string, long>> tables; };",
        );
        let ty = DynamicType::find(&ast, "S").expect("type");
        let int = |value| DynamicData::Integer {
            kind: Some(PrimitiveKind::Int32),
            value,
        };
        let data = DynamicData::Aggregate(vec![
            (
                "rows".into(),
                DynamicData::Collection(vec![
                    DynamicData::Collection(vec![int(1), int(2)]),
                    DynamicData::Collection(vec![int(3)]),
                ]),
            ),
            (
                "tables".into(),
                DynamicData::Collection(vec![DynamicData::Collection(vec![
                    DynamicData::Collection(vec![DynamicData::Text("k".into()), int(9)]),
                ])]),
            ),
        ]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(skipped.is_empty());
        let paths: Vec<&str> = fields.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["rows.rows_0", "rows.rows_1", "tables.tables_0.k"]);
        assert_eq!(
            fields[0].value,
            Variant {
                array_dimensions: vec![2],
                value: vec![VariantValue::Int32(1), VariantValue::Int32(2)],
            }
        );
        let written = unflatten(
            &ty,
            &[FlatField {
                path: "rows.rows_1".into(),
                value: Variant {
                    array_dimensions: vec![2],
                    value: vec![VariantValue::Int32(7), VariantValue::Int32(8)],
                },
            }],
            &data,
        )
        .expect("unflatten");
        let rows = written
            .member("rows")
            .and_then(DynamicData::items)
            .expect("rows");
        assert_eq!(rows[1], DynamicData::Collection(vec![int(7), int(8)]));
        assert_eq!(rows[0], DynamicData::Collection(vec![int(1), int(2)]));
    }
}
