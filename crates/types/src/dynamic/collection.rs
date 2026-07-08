// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Collection (`sequence`/`array`) `DynamicType`s that retain a **fully
//! resolved** element [`DynamicType`].
//!
//! The public `TypeDescriptor` keeps only a shallow element descriptor
//! (`element_type: Box<TypeDescriptor>`) — enough for scalar elements, but it
//! drops the members of a *composite* element (`sequence<Struct>` etc.), which
//! the reflective codec needs to recurse. Rather than widen the core type model
//! (and the TypeObject bridge that builds on it), this module constructs the
//! collection `DynamicType` with the resolved element kept as its single
//! synthetic member (index 0). [`resolved_element`] reads it back; the codec
//! prefers it and falls back to rebuilding a scalar element from the descriptor
//! when absent (so collections built the plain way still decode).

use alloc::vec;

use super::descriptor::{MemberDescriptor, TypeDescriptor, TypeKind};
use super::type_::{DynamicType, DynamicTypeInner, DynamicTypeMember};

/// The synthetic member name under which the resolved element type is stored.
const ELEMENT_MEMBER: &str = "@element";

/// Build a `sequence<Element>` type (bound `0` = unbounded) that retains the
/// fully-resolved `element` type for the reflective codec.
#[must_use]
pub fn sequence_of(element: DynamicType, bound: u32) -> DynamicType {
    sequence_named("seq", element, bound)
}

/// Like [`sequence_of`] but keeps a caller-supplied type name (e.g. a named
/// `typedef sequence<T> Foo` from a TypeObject) instead of the anonymous `"seq"`.
#[must_use]
pub fn sequence_named(name: &str, element: DynamicType, bound: u32) -> DynamicType {
    let desc = TypeDescriptor::sequence(name, element.descriptor().clone(), bound);
    with_element(desc, element)
}

/// Build an `Element[dims...]` array type that retains the fully-resolved
/// `element` type.
#[must_use]
pub fn array_of(element: DynamicType, dims: alloc::vec::Vec<u32>, name: &str) -> DynamicType {
    let desc = TypeDescriptor::array(name, element.descriptor().clone(), dims);
    with_element(desc, element)
}

fn with_element(desc: TypeDescriptor, element: DynamicType) -> DynamicType {
    let mut md = MemberDescriptor::new(ELEMENT_MEMBER, 0, element.descriptor().clone());
    md.index = 0;
    DynamicType::from_inner(DynamicTypeInner {
        descriptor: desc,
        members: vec![DynamicTypeMember {
            descriptor: md,
            member_type: element,
        }],
    })
}

/// The synthetic member names under which a map's resolved key + value types are
/// stored (index 0 = key, index 1 = value).
const KEY_MEMBER: &str = "@key";
const VALUE_MEMBER: &str = "@value";

/// Build a `map<Key, Value>` type retaining BOTH fully-resolved key and value
/// [`DynamicType`]s (key at index 0, value at index 1) for the reflective codec.
#[must_use]
pub fn map_of(key: DynamicType, value: DynamicType, bound: u32, name: &str) -> DynamicType {
    let desc = TypeDescriptor::map(
        name,
        key.descriptor().clone(),
        value.descriptor().clone(),
        bound,
    );
    let mut km = MemberDescriptor::new(KEY_MEMBER, 0, key.descriptor().clone());
    km.index = 0;
    let mut vm = MemberDescriptor::new(VALUE_MEMBER, 1, value.descriptor().clone());
    vm.index = 1;
    DynamicType::from_inner(DynamicTypeInner {
        descriptor: desc,
        members: vec![
            DynamicTypeMember {
                descriptor: km,
                member_type: key,
            },
            DynamicTypeMember {
                descriptor: vm,
                member_type: value,
            },
        ],
    })
}

/// The resolved element [`DynamicType`] of a collection, if one was attached via
/// [`sequence_of`] / [`array_of`]. Returns `None` for collections that carry only
/// a shallow scalar element descriptor (the codec rebuilds those itself).
#[must_use]
pub fn resolved_element(ty: &DynamicType) -> Option<&DynamicType> {
    if matches!(ty.kind(), TypeKind::Sequence | TypeKind::Array) {
        ty.member_by_index(0).map(|m| m.dynamic_type())
    } else {
        None
    }
}

/// The resolved key type of a `map<K,V>` built via [`map_of`], if present.
#[must_use]
pub fn resolved_map_key(ty: &DynamicType) -> Option<&DynamicType> {
    if ty.kind() == TypeKind::Map {
        ty.member_by_index(0).map(|m| m.dynamic_type())
    } else {
        None
    }
}

/// The resolved value type of a `map<K,V>` built via [`map_of`], if present.
#[must_use]
pub fn resolved_map_value(ty: &DynamicType) -> Option<&DynamicType> {
    if ty.kind() == TypeKind::Map {
        ty.member_by_index(1).map(|m| m.dynamic_type())
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dynamic::builder::DynamicTypeBuilderFactory as F;

    #[test]
    fn sequence_of_struct_retains_element() {
        let mut b = F::create_struct("E");
        b.add_struct_member("a", 0, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        let elem = b.build().unwrap();
        let seq = sequence_of(elem, 0);
        assert_eq!(seq.kind(), TypeKind::Sequence);
        let re = resolved_element(&seq).expect("resolved element");
        assert_eq!(re.kind(), TypeKind::Structure);
        assert_eq!(re.member_count(), 1);
    }

    #[test]
    fn scalar_sequence_has_no_resolved_element() {
        // A plain sequence descriptor (no synthetic member) → None.
        let desc = TypeDescriptor::sequence(
            "seq",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            0,
        );
        let ty = F::create_type(desc).unwrap().build().unwrap();
        assert!(resolved_element(&ty).is_none());
    }
}
