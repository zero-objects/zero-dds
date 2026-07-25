// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![cfg_attr(not(feature = "std"), no_std)]

//! Crate `zerodds-proto`. Safety classification: **STANDARD**.
//!
//! Protocol Buffers **type reuse** for DDS (rc.7). A `FileDescriptorSet`
//! (produced by `protoc -o out.pb --include_imports x.proto`) is mapped to an
//! XTypes [`DynamicType`], so a `.proto` data model becomes a DDS topic type
//! and rides the **DDS-native XCDR wire** through the reflective `DynamicData`
//! codec — not a protobuf payload. proto **field numbers map directly to
//! XTypes member IDs**, so the type is stable across additions.
//!
//! This is the scalar type mapping (plan checkpoint CP-A1). The
//! `FileDescriptorSet` reader and the message / enum / repeated / map / oneof
//! mapping (CP-A2..A5) build on it.
//!
//! [`DynamicType`]: zerodds_types::dynamic::DynamicType

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod reader;

#[cfg(feature = "alloc")]
pub mod descriptor;

#[cfg(feature = "alloc")]
pub mod mapper;

#[cfg(feature = "alloc")]
pub mod registry;

use zerodds_types::dynamic::TypeKind;

/// A Protocol Buffers field type, i.e. `FieldDescriptorProto.Type` in
/// `descriptor.proto`. The discriminants are the on-the-wire enum values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtoFieldType {
    /// `double` -> `f64`.
    Double = 1,
    /// `float` -> `f32`.
    Float = 2,
    /// `int64` (varint) -> `i64`.
    Int64 = 3,
    /// `uint64` (varint) -> `u64`.
    Uint64 = 4,
    /// `int32` (varint) -> `i32`.
    Int32 = 5,
    /// `fixed64` (8 bytes) -> `u64`.
    Fixed64 = 6,
    /// `fixed32` (4 bytes) -> `u32`.
    Fixed32 = 7,
    /// `bool` -> boolean.
    Bool = 8,
    /// `string` (UTF-8) -> XTypes string.
    String = 9,
    /// Deprecated proto2 group, mapped like a nested message.
    Group = 10,
    /// Nested `message` -> nested struct (resolved from `type_name`).
    Message = 11,
    /// `bytes` -> `sequence<byte>`.
    Bytes = 12,
    /// `uint32` (varint) -> `u32`.
    Uint32 = 13,
    /// `enum` -> XTypes enum (resolved from `type_name`).
    Enum = 14,
    /// `sfixed32` (4 bytes, signed) -> `i32`.
    Sfixed32 = 15,
    /// `sfixed64` (8 bytes, signed) -> `i64`.
    Sfixed64 = 16,
    /// `sint32` (zigzag varint) -> `i32`.
    Sint32 = 17,
    /// `sint64` (zigzag varint) -> `i64`.
    Sint64 = 18,
}

impl ProtoFieldType {
    /// Decode the `FieldDescriptorProto.Type` enum value. Returns `None` for
    /// out-of-range values (a malformed or newer descriptor).
    #[must_use]
    pub fn from_wire(v: i32) -> Option<Self> {
        Some(match v {
            1 => Self::Double,
            2 => Self::Float,
            3 => Self::Int64,
            4 => Self::Uint64,
            5 => Self::Int32,
            6 => Self::Fixed64,
            7 => Self::Fixed32,
            8 => Self::Bool,
            9 => Self::String,
            10 => Self::Group,
            11 => Self::Message,
            12 => Self::Bytes,
            13 => Self::Uint32,
            14 => Self::Enum,
            15 => Self::Sfixed32,
            16 => Self::Sfixed64,
            17 => Self::Sint32,
            18 => Self::Sint64,
            _ => return None,
        })
    }

    /// The XTypes [`TypeKind`] for a **directly-representable** field type.
    ///
    /// `None` for the field types that do not map to a single primitive:
    /// - [`ProtoFieldType::Bytes`] -> `sequence<byte>` (a collection),
    /// - [`ProtoFieldType::Message`] / [`ProtoFieldType::Group`] -> a nested
    ///   struct (resolved from the descriptor's `type_name`),
    /// - [`ProtoFieldType::Enum`] -> an XTypes enum (resolved from `type_name`).
    ///
    /// These are handled by the descriptor mapper (CP-A3), not here.
    #[must_use]
    pub fn scalar_type_kind(self) -> Option<TypeKind> {
        Some(match self {
            Self::Double => TypeKind::Float64,
            Self::Float => TypeKind::Float32,
            Self::Int64 | Self::Sfixed64 | Self::Sint64 => TypeKind::Int64,
            Self::Uint64 | Self::Fixed64 => TypeKind::UInt64,
            Self::Int32 | Self::Sfixed32 | Self::Sint32 => TypeKind::Int32,
            Self::Uint32 | Self::Fixed32 => TypeKind::UInt32,
            Self::Bool => TypeKind::Boolean,
            Self::String => TypeKind::String8,
            Self::Bytes | Self::Message | Self::Group | Self::Enum => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_values_round_trip_the_descriptor_enum() {
        assert_eq!(ProtoFieldType::from_wire(1), Some(ProtoFieldType::Double));
        assert_eq!(ProtoFieldType::from_wire(11), Some(ProtoFieldType::Message));
        assert_eq!(ProtoFieldType::from_wire(18), Some(ProtoFieldType::Sint64));
        assert_eq!(ProtoFieldType::from_wire(0), None);
        assert_eq!(ProtoFieldType::from_wire(19), None);
    }

    #[test]
    fn scalars_map_to_the_expected_type_kind() {
        assert_eq!(
            ProtoFieldType::Double.scalar_type_kind(),
            Some(TypeKind::Float64)
        );
        assert_eq!(
            ProtoFieldType::Float.scalar_type_kind(),
            Some(TypeKind::Float32)
        );
        // signed/zigzag/fixed 32- and 64-bit all fold onto Int32/Int64.
        for t in [
            ProtoFieldType::Int64,
            ProtoFieldType::Sfixed64,
            ProtoFieldType::Sint64,
        ] {
            assert_eq!(t.scalar_type_kind(), Some(TypeKind::Int64));
        }
        for t in [
            ProtoFieldType::Int32,
            ProtoFieldType::Sfixed32,
            ProtoFieldType::Sint32,
        ] {
            assert_eq!(t.scalar_type_kind(), Some(TypeKind::Int32));
        }
        assert_eq!(
            ProtoFieldType::Uint64.scalar_type_kind(),
            Some(TypeKind::UInt64)
        );
        assert_eq!(
            ProtoFieldType::Fixed32.scalar_type_kind(),
            Some(TypeKind::UInt32)
        );
        assert_eq!(
            ProtoFieldType::Bool.scalar_type_kind(),
            Some(TypeKind::Boolean)
        );
        assert_eq!(
            ProtoFieldType::String.scalar_type_kind(),
            Some(TypeKind::String8)
        );
    }

    #[test]
    fn non_scalar_field_types_have_no_direct_kind() {
        for t in [
            ProtoFieldType::Bytes,
            ProtoFieldType::Message,
            ProtoFieldType::Group,
            ProtoFieldType::Enum,
        ] {
            assert_eq!(t.scalar_type_kind(), None);
        }
    }
}
