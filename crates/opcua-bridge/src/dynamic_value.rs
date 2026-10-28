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
    let mut fields = Vec::new();
    let mut skipped = Vec::new();
    walk_value(ty, data, "", &mut fields, &mut skipped, 0)?;
    Ok((fields, skipped))
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, element
/// or alias; refused past `MAX_DEPTH`)
fn walk_value(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
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
        TypeKind::Structure => {
            let members = ty.members();
            let Some(fields_data) = data.members() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected an aggregate"
                )));
            };
            if members.len() != fields_data.len() {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: type has {} members, data has {}",
                    members.len(),
                    fields_data.len()
                )));
            }
            for (member, (name, value)) in members.iter().zip(fields_data.iter()) {
                if member.name() != name {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: member order disagrees (`{}` vs `{name}`)",
                        member.name()
                    )));
                }
                let member_type = member
                    .dynamic_type()
                    .map_err(|e| ConvertError::ShapeMismatch(format!("{path}.{name}: {e:?}")))?;
                let child_path = join(path, name);
                walk_value(&member_type, value, &child_path, out, skipped, depth + 1)?;
            }
            Ok(())
        }
        TypeKind::Union => {
            let Some(members) = data.members() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected a union value"
                )));
            };
            for (name, value) in members {
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
                walk_value(&member_type, value, &child_path, out, skipped, depth + 1)?;
            }
            Ok(())
        }
        TypeKind::Alias => {
            let target = ty.base_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: alias with no target"))
            })?;
            walk_value(&target, data, path, out, skipped, depth + 1)
        }
        TypeKind::Array | TypeKind::Sequence => {
            let element_ty = ty.element_type().ok_or_else(|| {
                ConvertError::ShapeMismatch(format!("{path}: collection with no element type"))
            })?;
            let Some(items) = data.items() else {
                return Err(ConvertError::ShapeMismatch(format!(
                    "{path}: expected a collection value"
                )));
            };
            if matches!(element_ty.kind(), TypeKind::Structure | TypeKind::Union) {
                skipped.push(ConvertError::Unsupported {
                    path: path.to_string(),
                    reason: "an array/sequence of a struct or union is one array Variable per Gateway 1.0 §9.2.5, not decomposed into per-element leaves",
                });
                return Ok(());
            }
            let dims = if matches!(ty.kind(), TypeKind::Array) {
                ty.descriptor()
                    .map_err(|e| ConvertError::ShapeMismatch(format!("{path}: {e:?}")))?
                    .bound
            } else {
                vec![u32::try_from(items.len()).unwrap_or(u32::MAX)]
            };
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(scalar_value(&element_ty, item, path, skipped)?);
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
        // entry, keyed by the (stringified) map key — unlike an
        // array/sequence of an aggregate (Tab 9's own "one Variable, not
        // decomposed" rule for THAT construct), the spec's own tables give
        // a map's entries individual identity. `DynamicData`'s own shape
        // for a map (`zerodds_types::dynamic::data::DynamicData::
        // Collection` docs: "a map entry is a two-element collection,
        // [key, value]") is walked the same way a struct's members are —
        // recursively, so a map of structs still decomposes down to
        // scalar leaves.
        //
        // Not yet done: `AddressSpace`/`PublishedDataSet` build their node/
        // field set once, from the topic's Table-9 default (an empty
        // map), so a key that was not already present is reported as an
        // unmatched path by `address_space_build::apply_sample` /
        // silently kept at its `pubsub_glue` PDS field's last value
        // rather than a new node/field materializing live — the honest,
        // already-existing "not silently dropped" mechanism every other
        // shape-drift uses, not a new one, until dynamic node/field
        // creation is added.
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
                walk_value(&value_ty, value_data, &child_path, out, skipped, depth + 1)?;
            }
            Ok(())
        }
        TypeKind::Bitset => {
            skipped.push(ConvertError::Unsupported {
                path: path.to_string(),
                // No live OPC-UA `VariantValue` case fits an IDL `bitset`
                // (unlike `bitmask`, which packs into `UInt32`, `bitset`
                // is a byte-packed named-field record with no OPC-UA
                // builtin counterpart in Part 6 §5.1.2's type table).
                // Inferred from the absence of a `bitset` row there, not
                // a specific Gateway 1.0 §9.2 clause this crate has the
                // text of.
                reason: "no OPC-UA builtin VariantValue case fits a bitset (inferred from its absence in Part 6 §5.1.2's type table, not a specific Gateway 1.0 §9.2 clause)",
            });
            Ok(())
        }
        // Scalar (primitive/string/enum/bitmask) — one leaf.
        _ => {
            let value = scalar_value(ty, data, path, skipped)?;
            out.push(FlatField {
                path: path.to_string(),
                value: Variant::scalar(value),
            });
            Ok(())
        }
    }
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
fn map_key_string(
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
fn parse_map_key(
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
fn scalar_value(
    ty: &DynamicType,
    data: &DynamicData,
    path: &str,
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
        (TypeKind::Bitmask, DynamicData::Bits(flags)) => {
            let mut acc: u64 = 0;
            for flag in flags {
                let DynamicData::Text(name) = flag else {
                    return Err(ConvertError::ShapeMismatch(format!(
                        "{path}: a bitmask flag must be a name"
                    )));
                };
                match ty.member_by_name(name) {
                    Some(member) => acc |= 1u64 << member.literal_value().unwrap_or(0),
                    None => skipped.push(ConvertError::Unsupported {
                        path: path.to_string(),
                        reason: "a bitmask flag name the type does not declare",
                    }),
                }
            }
            Ok(VariantValue::UInt32(u32::try_from(acc).unwrap_or(u32::MAX)))
        }
        // A scalar leaf that cannot be mapped (unlike a collection element,
        // which the caller already skips as a unit) leaves its whole
        // `FlatField` unrepresentable, so this returns the error directly;
        // the call site propagates it with `?` — named, not silently
        // dropped, one level up.
        (TypeKind::Float128, _) => Err(ConvertError::Unsupported {
            path: path.to_string(),
            // Part 6 §5.1.2's type table stops at `Double` (IEEE 754
            // binary64) — no builtin `VariantValue` case holds IEEE
            // binary128/extended precision. Inferred from that absence,
            // not a specific Gateway 1.0 §9.2 clause this crate has the
            // text of (this daemon has no `fixed<P,S>` fallback either,
            // for the same reason).
            reason: "no OPC-UA builtin VariantValue case fits float128/long double (Part 6 §5.1.2's type table stops at Double)",
        }),
        (kind, data) => Err(ConvertError::ShapeMismatch(std::format!(
            "{path}: type kind {kind:?} does not match data variant {data:?}"
        ))),
    }
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
        TypeKind::Structure => {
            let default_members = default.members().unwrap_or(&[]);
            // A fallback for a member index `default` (structurally) lacks —
            // only reachable if `default` disagrees with `ty`, which is a
            // caller invariant violation, not a value this function invents
            // for a well-formed default/type pair.
            let fallback = DynamicData::Boolean(false);
            let mut out = Vec::with_capacity(ty.member_count() as usize);
            for (index, member) in ty.members().iter().enumerate() {
                let member_type = member.dynamic_type().map_err(|e| {
                    ConvertError::ShapeMismatch(format!("{path}.{}: {e:?}", member.name()))
                })?;
                let child_path = join(path, member.name());
                let member_default = default_members.get(index).map_or(&fallback, |(_, v)| v);
                out.push((
                    member.name().to_string(),
                    build_value(&member_type, &child_path, fields, member_default, depth + 1)?,
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
        TypeKind::Array | TypeKind::Sequence | TypeKind::Union | TypeKind::Bitset => {
            // Not decomposed on the way out either (see `flatten`'s design
            // boundary) — a write that does not touch this path keeps its
            // prior value verbatim, which is the correct behaviour for a
            // field this daemon never split into leaves.
            let _ = fields;
            Ok(default.clone())
        }
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
        _ => match fields.get(path) {
            Some(variant) => scalar_from_variant(ty, variant, path),
            None => Ok(default.clone()),
        },
    }
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

fn scalar_from_variant(
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
    fn flatten_and_unflatten_a_flat_struct_round_trips() {
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

        let rebuilt = unflatten(&ty, &fields, &data).expect("unflatten");
        assert_eq!(rebuilt, data);
    }

    #[test]
    fn a_value_nested_past_the_bound_is_refused_not_walked() {
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
    fn nested_struct_flattens_with_dotted_paths() {
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
    fn partial_unflatten_keeps_the_rest_of_the_default() {
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
    fn enum_flattens_to_its_ordinal_and_back() {
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
    fn sequence_of_primitives_is_one_variant() {
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

    #[test]
    fn sequence_of_struct_is_reported_skipped_not_dropped_silently() {
        let ast = ast_of("struct E { long v; }; struct P { sequence<E, 2> items; };");
        let ty = DynamicType::find(&ast, "P").expect("type");
        let data =
            DynamicData::Aggregate(vec![("items".to_string(), DynamicData::Collection(vec![]))]);
        let (fields, skipped) = flatten(&ty, &data).expect("flatten");
        assert!(fields.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(matches!(&skipped[0], ConvertError::Unsupported { path, .. } if path == "items"));
    }

    #[test]
    fn wide_char_maps_to_uint16_and_back() {
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
    fn wide_string_maps_to_string_and_back() {
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

    #[test]
    fn map_of_primitives_decomposes_to_one_leaf_per_key_and_back() {
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
    fn map_partial_write_keeps_the_untouched_key() {
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
}
