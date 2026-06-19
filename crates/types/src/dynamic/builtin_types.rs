// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 §7.6.5 + Annex E — Built-in Types Set (C4.4).
//!
//! The spec defines four pre-registered common types that every spec-
//! conformant DDS implementation MUST be able to use as a topic type:
//!
//! ```idl
//! @nested
//! struct DDS::String {
//!     string value;          // unbounded
//! };
//!
//! @nested
//! struct DDS::KeyedString {
//!     @key string key;       // topic key
//!     string value;
//! };
//!
//! @nested
//! struct DDS::Bytes {
//!     sequence<octet> value; // unbounded
//! };
//!
//! @nested
//! struct DDS::KeyedBytes {
//!     @key string key;
//!     sequence<octet> value;
//! };
//! ```
//!
//! Use case: cross-vendor demos and tutorials (Cyclone DDS,
//! FastDDS, RTI Connext) register these types by default. Without
//! them, `IDLPub <topic-name> "Hello"` cannot be started without a custom
//! type definition.
//!
//! The types are exposed here as singleton functions — the caller
//! `register_type` calls them once a DDS participant is enabled
//! (Spec §7.6.5).

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;

use super::builder::DynamicTypeBuilder;
use super::descriptor::{MemberDescriptor, TypeDescriptor, TypeKind};
use super::error::DynamicError;
use super::type_::DynamicType;

/// Spec-Type-Name `"DDS::String"` (Spec §7.6.5).
pub const NAME_DDS_STRING: &str = "DDS::String";

/// Spec-Type-Name `"DDS::KeyedString"`.
pub const NAME_DDS_KEYED_STRING: &str = "DDS::KeyedString";

/// Spec-Type-Name `"DDS::Bytes"`.
pub const NAME_DDS_BYTES: &str = "DDS::Bytes";

/// Spec-Type-Name `"DDS::KeyedBytes"`.
pub const NAME_DDS_KEYED_BYTES: &str = "DDS::KeyedBytes";

/// `DDS::String` — Spec §7.6.5 (Annex E).
///
/// `@nested struct DDS::String { string value; };`
///
/// # Errors
/// Build error if the DynamicTypeBuilder API balks — should never
/// happen for this statically defined type.
pub fn dds_string() -> Result<DynamicType, DynamicError> {
    let mut desc = TypeDescriptor::structure(NAME_DDS_STRING);
    desc.is_nested = true;
    let mut builder = DynamicTypeBuilder::new(desc);
    builder.add_member(MemberDescriptor::new("value", 1, unbounded_string()))?;
    builder.build()
}

fn unbounded_string() -> TypeDescriptor {
    let mut t = TypeDescriptor::primitive(TypeKind::String8, "string");
    // Spec §7.5.1.2.4: bound = 0 = unbounded.
    t.bound = alloc::vec![0];
    t
}

fn unbounded_octet_sequence() -> TypeDescriptor {
    let mut t = TypeDescriptor::primitive(TypeKind::Sequence, "sequence");
    t.bound = alloc::vec![0];
    t.element_type = Some(Box::new(TypeDescriptor::primitive(TypeKind::Byte, "octet")));
    t
}

/// `DDS::KeyedString` — Spec §7.6.5 (Annex E).
///
/// `@nested struct DDS::KeyedString { @key string key; string value; };`
///
/// # Errors
/// siehe [`dds_string`].
pub fn dds_keyed_string() -> Result<DynamicType, DynamicError> {
    let mut desc = TypeDescriptor::structure(NAME_DDS_KEYED_STRING);
    desc.is_nested = true;
    let mut builder = DynamicTypeBuilder::new(desc);
    let mut key = MemberDescriptor::new("key", 1, unbounded_string());
    key.is_key = true;
    builder.add_member(key)?;
    builder.add_member(MemberDescriptor::new("value", 2, unbounded_string()))?;
    builder.build()
}

/// `DDS::Bytes` — Spec §7.6.5 (Annex E).
///
/// `@nested struct DDS::Bytes { sequence<octet> value; };`
///
/// # Errors
/// siehe [`dds_string`].
pub fn dds_bytes() -> Result<DynamicType, DynamicError> {
    let mut desc = TypeDescriptor::structure(NAME_DDS_BYTES);
    desc.is_nested = true;
    let mut builder = DynamicTypeBuilder::new(desc);
    builder.add_member(MemberDescriptor::new(
        "value",
        1,
        unbounded_octet_sequence(),
    ))?;
    builder.build()
}

/// `DDS::KeyedBytes` — Spec §7.6.5 (Annex E).
///
/// `@nested struct DDS::KeyedBytes { @key string key; sequence<octet> value; };`
///
/// # Errors
/// siehe [`dds_string`].
pub fn dds_keyed_bytes() -> Result<DynamicType, DynamicError> {
    let mut desc = TypeDescriptor::structure(NAME_DDS_KEYED_BYTES);
    desc.is_nested = true;
    let mut builder = DynamicTypeBuilder::new(desc);
    let mut key = MemberDescriptor::new("key", 1, unbounded_string());
    key.is_key = true;
    builder.add_member(key)?;
    builder.add_member(MemberDescriptor::new(
        "value",
        2,
        unbounded_octet_sequence(),
    ))?;
    builder.build()
}

/// Returns all 4 builtin types in spec order. Convenience helper
/// for `Participant::enable()` paths that register all builtin types
/// at once.
///
/// # Errors
/// See [`dds_string`].
pub fn all_builtin_types() -> Result<[(String, DynamicType); 4], DynamicError> {
    Ok([
        (String::from(NAME_DDS_STRING), dds_string()?),
        (String::from(NAME_DDS_KEYED_STRING), dds_keyed_string()?),
        (String::from(NAME_DDS_BYTES), dds_bytes()?),
        (String::from(NAME_DDS_KEYED_BYTES), dds_keyed_bytes()?),
    ])
}

/// Discriminator: is `name` one of the 4 builtin-type spec names?
#[must_use]
pub fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        NAME_DDS_STRING | NAME_DDS_KEYED_STRING | NAME_DDS_BYTES | NAME_DDS_KEYED_BYTES
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn spec_names_match_annex_e() {
        // Spec §7.6.5 — these strings may NEVER drift, otherwise
        // Cyclone/FastDDS topic matching is impossible.
        assert_eq!(NAME_DDS_STRING, "DDS::String");
        assert_eq!(NAME_DDS_KEYED_STRING, "DDS::KeyedString");
        assert_eq!(NAME_DDS_BYTES, "DDS::Bytes");
        assert_eq!(NAME_DDS_KEYED_BYTES, "DDS::KeyedBytes");
    }

    #[test]
    fn dds_string_has_one_value_member() {
        let t = dds_string().unwrap();
        assert_eq!(t.name(), NAME_DDS_STRING);
        assert_eq!(t.kind(), TypeKind::Structure);
        assert_eq!(t.member_count(), 1);
        let member = t.member_by_name("value").unwrap();
        assert_eq!(member.dynamic_type().kind(), TypeKind::String8);
        assert!(!member.descriptor().is_key);
        assert!(t.descriptor().is_nested);
    }

    #[test]
    fn dds_keyed_string_has_key_and_value() {
        let t = dds_keyed_string().unwrap();
        assert_eq!(t.name(), NAME_DDS_KEYED_STRING);
        assert_eq!(t.member_count(), 2);
        let key = t.member_by_name("key").unwrap();
        assert!(key.descriptor().is_key);
        assert_eq!(key.dynamic_type().kind(), TypeKind::String8);
        let value = t.member_by_name("value").unwrap();
        assert!(!value.descriptor().is_key);
        assert_eq!(value.dynamic_type().kind(), TypeKind::String8);
    }

    #[test]
    fn dds_bytes_has_unbounded_octet_sequence() {
        let t = dds_bytes().unwrap();
        assert_eq!(t.name(), NAME_DDS_BYTES);
        assert_eq!(t.member_count(), 1);
        let value = t.member_by_name("value").unwrap();
        assert_eq!(value.dynamic_type().kind(), TypeKind::Sequence);
        // Spec §7.5.1.2.4: bound = 0 = unbounded.
        assert_eq!(value.dynamic_type().descriptor().bound, alloc::vec![0]);
    }

    #[test]
    fn dds_keyed_bytes_has_key_and_octet_sequence() {
        let t = dds_keyed_bytes().unwrap();
        assert_eq!(t.name(), NAME_DDS_KEYED_BYTES);
        assert_eq!(t.member_count(), 2);
        assert!(t.member_by_name("key").unwrap().descriptor().is_key);
        assert_eq!(
            t.member_by_name("value").unwrap().dynamic_type().kind(),
            TypeKind::Sequence
        );
    }

    #[test]
    fn all_builtin_types_returns_four_with_correct_names() {
        let types = all_builtin_types().unwrap();
        let names: alloc::vec::Vec<_> = types.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "DDS::String",
                "DDS::KeyedString",
                "DDS::Bytes",
                "DDS::KeyedBytes"
            ]
        );
    }

    #[test]
    fn is_builtin_type_name_recognises_all_four() {
        assert!(is_builtin_type_name("DDS::String"));
        assert!(is_builtin_type_name("DDS::KeyedString"));
        assert!(is_builtin_type_name("DDS::Bytes"));
        assert!(is_builtin_type_name("DDS::KeyedBytes"));
        assert!(!is_builtin_type_name("dds::string"));
        assert!(!is_builtin_type_name("MyApp::Custom"));
    }

    #[test]
    fn all_builtin_types_have_nested_annotation() {
        // Spec §7.6.5: all 4 are `@nested` (not intended as a top-level topic,
        // but usable as a member type or topic-via-wrap).
        for (name, t) in all_builtin_types().unwrap() {
            assert!(
                t.descriptor().is_nested,
                "{name} should have is_nested=true"
            );
        }
    }

    #[test]
    fn dds_string_singleton_calls_produce_equal_types() {
        // Zwei separate Aufrufe liefern logisch equivalente Types.
        let a = dds_string().unwrap();
        let b = dds_string().unwrap();
        assert!(a.equals(&b));
    }

    #[test]
    fn keyed_types_have_exactly_one_key_member() {
        for (name, t) in [
            (NAME_DDS_KEYED_STRING, dds_keyed_string().unwrap()),
            (NAME_DDS_KEYED_BYTES, dds_keyed_bytes().unwrap()),
        ] {
            let key_count = (0..t.member_count())
                .filter_map(|i| t.member_by_index(i))
                .filter(|m| m.descriptor().is_key)
                .count();
            assert_eq!(key_count, 1, "{name}: expected exactly 1 key member");
        }
    }

    #[test]
    fn unkeyed_types_have_no_key_member() {
        for (name, t) in [
            (NAME_DDS_STRING, dds_string().unwrap()),
            (NAME_DDS_BYTES, dds_bytes().unwrap()),
        ] {
            let key_count = (0..t.member_count())
                .filter_map(|i| t.member_by_index(i))
                .filter(|m| m.descriptor().is_key)
                .count();
            assert_eq!(key_count, 0, "{name}: expected no key members");
        }
    }
}
