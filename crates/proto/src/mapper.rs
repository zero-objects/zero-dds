// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Map a parsed protobuf [`Message`] to an XTypes [`DynamicType`].
//!
//! proto field numbers become XTypes member ids, so with
//! [`ExtensibilityKind::Mutable`] (member-id-tagged EMHEADERs) the mapped type
//! carries the same evolution guarantees as the source `.proto`. The caller
//! picks the extensibility; `Mutable` is the proto-faithful choice, `Appendable`
//! the more compact wire.
//!
//! This step covers scalars, `string`, `bytes` (-> `sequence<byte>`) and
//! `repeated` (-> `sequence<T>`). Nested-message, enum and `map<K,V>` fields are
//! resolved by the type registry in the follow-up step.

use alloc::format;
use alloc::string::String;

use zerodds_types::dynamic::{
    DynamicType, DynamicTypeBuilderFactory, ExtensibilityKind, MemberDescriptor, TypeDescriptor,
    TypeKind,
};

use crate::ProtoFieldType;
use crate::descriptor::{Field, Label, Message};

/// Why a message could not be mapped to a `DynamicType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    /// A field type is not yet handled by this step (nested message, enum or
    /// group — pending the type registry).
    UnsupportedField {
        /// The offending field's name.
        field: String,
        /// Human-readable reason.
        reason: &'static str,
    },
    /// The XTypes builder rejected the type (e.g. a duplicate member id).
    Build(String),
    /// A field referenced a message/enum name not present in the descriptor set.
    UnknownType(String),
    /// The message graph is recursive; XTypes DynamicType is a finite tree.
    Cyclic(String),
}

fn canonical_name(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Boolean => "boolean",
        TypeKind::Float32 => "float",
        TypeKind::Float64 => "double",
        TypeKind::Int32 => "int32",
        TypeKind::Int64 => "int64",
        TypeKind::UInt32 => "uint32",
        TypeKind::UInt64 => "uint64",
        _ => "prim",
    }
}

/// The element `TypeDescriptor` for a field (before any `repeated` wrapping).
/// Only scalar / `string` / `bytes` map here; message / enum / group elements
/// are resolved by the [`crate::registry`] (they need the full type graph).
pub(crate) fn element_descriptor(field: &Field) -> Result<TypeDescriptor, MapError> {
    let ty = field.ty.ok_or_else(|| MapError::UnsupportedField {
        field: field.name.clone(),
        reason: "unknown field type code",
    })?;
    Ok(match ty {
        ProtoFieldType::String => TypeDescriptor::string8(0),
        ProtoFieldType::Bytes => TypeDescriptor::sequence(
            "sequence<octet>",
            TypeDescriptor::primitive(TypeKind::Byte, "byte"),
            0,
        ),
        ProtoFieldType::Message | ProtoFieldType::Group | ProtoFieldType::Enum => {
            return Err(MapError::UnsupportedField {
                field: field.name.clone(),
                reason: "nested message / enum needs the type registry (next step)",
            });
        }
        scalar => {
            let kind = scalar
                .scalar_type_kind()
                .ok_or_else(|| MapError::UnsupportedField {
                    field: field.name.clone(),
                    reason: "no primitive kind",
                })?;
            TypeDescriptor::primitive(kind, canonical_name(kind))
        }
    })
}

/// The full member `TypeDescriptor` for a field, wrapping the element in an
/// unbounded `sequence<T>` when the field is `repeated`.
pub(crate) fn field_descriptor(field: &Field) -> Result<TypeDescriptor, MapError> {
    let element = element_descriptor(field)?;
    Ok(match field.label {
        Label::Repeated => {
            let name = format!("sequence<{}>", element.name);
            TypeDescriptor::sequence(name, element, 0)
        }
        Label::Optional | Label::Required => element,
    })
}

/// Map a protobuf message to an XTypes `DynamicType` with the given
/// extensibility. Field numbers are preserved as member ids.
///
/// # Errors
/// [`MapError`] if a field type is not yet supported or the builder rejects a
/// member.
pub fn message_to_type(
    message: &Message,
    extensibility: ExtensibilityKind,
) -> Result<DynamicType, MapError> {
    let mut td = TypeDescriptor::structure(message.name.clone());
    td.extensibility_kind = extensibility;
    let mut builder = DynamicTypeBuilderFactory::create_type(td)
        .map_err(|e| MapError::Build(format!("{e:?}")))?;

    for field in &message.fields {
        let id = u32::try_from(field.number).map_err(|_| MapError::UnsupportedField {
            field: field.name.clone(),
            reason: "field number out of range",
        })?;
        let member = MemberDescriptor::new(field.name.clone(), id, field_descriptor(field)?);
        builder
            .add_member(member)
            .map_err(|e| MapError::Build(format!("{e:?}")))?;
    }

    builder
        .build()
        .map_err(|e| MapError::Build(format!("{e:?}")))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn field(name: &str, number: i32, ty: ProtoFieldType, label: Label) -> Field {
        Field {
            name: name.to_string(),
            number,
            label,
            ty: Some(ty),
            type_name: String::new(),
            oneof_index: None,
        }
    }

    #[test]
    fn maps_scalar_string_and_repeated_members() {
        let msg = Message {
            name: "Sensor".to_string(),
            fields: vec![
                field("label", 1, ProtoFieldType::String, Label::Optional),
                field("value", 2, ProtoFieldType::Int32, Label::Optional),
                field("samples", 5, ProtoFieldType::Double, Label::Repeated),
                field("blob", 7, ProtoFieldType::Bytes, Label::Optional),
            ],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };

        let dt = message_to_type(&msg, ExtensibilityKind::Mutable).unwrap();
        assert_eq!(dt.member_count(), 4);

        // field numbers preserved as member ids
        assert_eq!(dt.member_by_name("label").unwrap().id(), 1);
        assert_eq!(dt.member_by_name("value").unwrap().id(), 2);
        assert_eq!(dt.member_by_name("samples").unwrap().id(), 5);
        assert_eq!(dt.member_by_name("blob").unwrap().id(), 7);
        // id-indexed lookup finds the right member (proto -> XTypes @id)
        assert_eq!(dt.member_by_id(5).unwrap().name(), "samples");
    }

    #[test]
    fn nested_message_field_is_unsupported_for_now() {
        let msg = Message {
            name: "Outer".to_string(),
            fields: vec![Field {
                name: "inner".to_string(),
                number: 1,
                label: Label::Optional,
                ty: Some(ProtoFieldType::Message),
                type_name: ".pkg.Inner".to_string(),
                oneof_index: None,
            }],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        assert!(matches!(
            message_to_type(&msg, ExtensibilityKind::Appendable),
            Err(MapError::UnsupportedField { .. })
        ));
    }
}
