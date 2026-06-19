// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicTypeBuilder + DynamicTypeBuilderFactory (XTypes 1.3 §7.5.4, §7.5.5).
//!
//! Spec behavior:
//! - `add_member` validates name and id immediately against existing
//!   members (spec §7.5.4.1.2 preconditions).
//! - `build()` validates the final structure (inheritance cycle,
//!   mandatory discriminator, etc.) and returns an immutable
//!   `DynamicType`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use super::descriptor::{MemberDescriptor, MemberId, TypeDescriptor, TypeKind};
use super::error::DynamicError;
use super::type_::{DynamicType, DynamicTypeInner, DynamicTypeMember, primitive_name};

/// XTypes §7.5.4 DynamicTypeBuilder.
#[derive(Debug)]
pub struct DynamicTypeBuilder {
    descriptor: TypeDescriptor,
    members: Vec<DynamicTypeMember>,
    sealed: AtomicBool,
}

impl DynamicTypeBuilder {
    /// Internal — the factory creates builders.
    pub(super) fn new(descriptor: TypeDescriptor) -> Self {
        Self {
            descriptor,
            members: Vec::new(),
            sealed: AtomicBool::new(false),
        }
    }

    /// Current descriptor (read-only view).
    #[must_use]
    pub fn descriptor(&self) -> &TypeDescriptor {
        &self.descriptor
    }

    /// Sets the descriptor anew (spec §7.5.4.1 SetDescriptor) — only
    /// allowed before `build()`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if `build()` was already called.
    pub fn set_descriptor(&mut self, descriptor: TypeDescriptor) -> Result<(), DynamicError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DynamicError::PreconditionNotMet(String::from(
                "set_descriptor after build()",
            )));
        }
        descriptor
            .is_consistent()
            .map_err(DynamicError::inconsistent)?;
        self.descriptor = descriptor;
        Ok(())
    }

    /// Adds a member (spec §7.5.4.1.2 AddMember).
    ///
    /// Validates immediately:
    /// - Unique name among the existing members.
    /// - Unique id (only when composite XCDR2-capable).
    /// - The member type is consistent.
    /// - The kind allows members.
    ///
    /// `index` is set automatically if the caller leaves it at 0,
    /// otherwise respected.
    ///
    /// # Errors
    /// `BuilderConflict` on a dup name/id, `IllegalOperation` if the
    /// kind carries no members.
    pub fn add_member(&mut self, mut descriptor: MemberDescriptor) -> Result<(), DynamicError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DynamicError::PreconditionNotMet(String::from(
                "add_member after build()",
            )));
        }
        if !self.descriptor.kind.is_aggregable() {
            return Err(DynamicError::IllegalOperation(alloc::format!(
                "add_member on non-composite kind {:?}",
                self.descriptor.kind
            )));
        }
        descriptor
            .is_consistent()
            .map_err(DynamicError::inconsistent)?;

        // Dup-Name-Check.
        if self
            .members
            .iter()
            .any(|m| m.descriptor.name == descriptor.name)
        {
            return Err(DynamicError::builder(alloc::format!(
                "duplicate member name {}",
                descriptor.name
            )));
        }
        // Dup-Id-Check.
        if self
            .members
            .iter()
            .any(|m| m.descriptor.id == descriptor.id)
        {
            return Err(DynamicError::builder(alloc::format!(
                "duplicate member id {}",
                descriptor.id
            )));
        }
        // Auto-index if the caller leaves index=0 for all (the default pattern).
        let auto_index = u32::try_from(self.members.len()).unwrap_or(u32::MAX);
        if descriptor.index == 0 && auto_index != 0 {
            descriptor.index = auto_index;
        } else if descriptor.index == 0 {
            descriptor.index = 0; // erster Member bleibt 0
        }
        // Member-Type bauen.
        let member_type =
            DynamicType::from_inner(descriptor_to_dynamic_type_inner(&descriptor.member_type)?);
        self.members.push(DynamicTypeMember {
            descriptor,
            member_type,
        });
        Ok(())
    }

    /// Convenience wrapper for structs.
    ///
    /// # Errors
    /// See [`add_member`].
    pub fn add_struct_member(
        &mut self,
        name: impl Into<String>,
        id: MemberId,
        ty: TypeDescriptor,
    ) -> Result<(), DynamicError> {
        let mut d = MemberDescriptor::new(name, id, ty);
        d.index = u32::try_from(self.members.len()).unwrap_or(u32::MAX);
        self.add_member(d)
    }

    /// Spec §7.5.4.1.1 Build — finalizes the builder.
    ///
    /// Validations:
    /// - all member descriptors consistent
    /// - inheritance cycle via names
    /// - union: discriminator + at least 1 case
    /// - unique labels in a union
    ///
    /// # Errors
    /// `BuilderConflict` / `Inconsistent`.
    pub fn build(&self) -> Result<DynamicType, DynamicError> {
        if self.sealed.swap(true, Ordering::AcqRel) {
            return Err(DynamicError::PreconditionNotMet(String::from(
                "build() called twice",
            )));
        }
        self.descriptor
            .is_consistent()
            .map_err(DynamicError::inconsistent)?;
        // Cycle check via names — robust for the common case.
        if let Some(b) = &self.descriptor.base_type {
            check_inheritance_chain(&self.descriptor.name, b)?;
        }
        // Union-specific checks.
        if self.descriptor.kind == TypeKind::Union {
            if self.members.is_empty() {
                return Err(DynamicError::builder(
                    "union without case members".to_string(),
                ));
            }
            let mut seen_labels: BTreeMap<i64, &str> = BTreeMap::new();
            let mut default_count = 0_u32;
            for m in &self.members {
                if m.descriptor.is_default_label {
                    default_count += 1;
                }
                for label in &m.descriptor.label {
                    if let Some(prev) = seen_labels.insert(*label, &m.descriptor.name) {
                        return Err(DynamicError::builder(alloc::format!(
                            "duplicate union label {label} (prev member: {prev})"
                        )));
                    }
                }
            }
            if default_count > 1 {
                return Err(DynamicError::builder(
                    "union with multiple default-label members",
                ));
            }
        }
        let inner = DynamicTypeInner {
            descriptor: self.descriptor.clone(),
            members: self.members.clone(),
        };
        Ok(DynamicType {
            inner: Arc::new(inner),
        })
    }
}

/// Walk through the base_type chain — if a name appears twice, it is a
/// cycle. Depth is capped at 64 (DoS cap).
fn check_inheritance_chain(self_name: &str, base: &TypeDescriptor) -> Result<(), DynamicError> {
    let mut seen: alloc::vec::Vec<&str> = alloc::vec![self_name];
    let mut cur = base;
    let mut depth = 0_usize;
    loop {
        if depth >= 64 {
            return Err(DynamicError::builder("inheritance chain exceeds 64 levels"));
        }
        if seen.iter().any(|n| *n == cur.name) {
            return Err(DynamicError::builder(alloc::format!(
                "inheritance cycle through '{}'",
                cur.name
            )));
        }
        seen.push(&cur.name);
        depth += 1;
        if let Some(b) = &cur.base_type {
            cur = b;
        } else {
            return Ok(());
        }
    }
}

/// Constructs a `DynamicTypeInner` from a `TypeDescriptor` (no
/// add_member cycle — members are derived recursively from
/// `descriptor.bound`/`element_type`/`key_element_type`, but are not in
/// the `members` vec, because that only applies to composite types with
/// named members).
pub(super) fn descriptor_to_dynamic_type_inner(
    desc: &TypeDescriptor,
) -> Result<DynamicTypeInner, DynamicError> {
    desc.is_consistent().map_err(DynamicError::inconsistent)?;
    Ok(DynamicTypeInner {
        descriptor: desc.clone(),
        members: Vec::new(),
    })
}

/// XTypes §7.5.5 DynamicTypeBuilderFactory — Singleton im Spec-Sinne.
///
/// Stateless: no global caches except the primitive singleton pool,
/// which is lazily initialized via `OnceLock`.
pub struct DynamicTypeBuilderFactory;

impl DynamicTypeBuilderFactory {
    /// Spec §7.5.5.1.1 `create_type(descriptor)`.
    ///
    /// # Errors
    /// `Inconsistent` if the descriptor is invalid.
    pub fn create_type(descriptor: TypeDescriptor) -> Result<DynamicTypeBuilder, DynamicError> {
        descriptor
            .is_consistent()
            .map_err(DynamicError::inconsistent)?;
        Ok(DynamicTypeBuilder::new(descriptor))
    }

    /// Convenience variant: creates a struct builder directly.
    #[must_use]
    pub fn create_struct(name: impl Into<String>) -> DynamicTypeBuilder {
        DynamicTypeBuilder::new(TypeDescriptor::structure(name))
    }

    /// Convenience variant: creates a union builder directly with the
    /// given discriminator type.
    ///
    /// # Errors
    /// `Inconsistent` if the discriminator is not permitted.
    pub fn create_union(
        name: impl Into<String>,
        discriminator: TypeDescriptor,
    ) -> Result<DynamicTypeBuilder, DynamicError> {
        let desc = TypeDescriptor::union(name, discriminator);
        Self::create_type(desc)
    }

    /// Spec §7.5.5.1.2 `get_primitive_type(kind)` — Singleton-Cache.
    ///
    /// Repeated calls with the same `kind` return the same
    /// `DynamicType` instance (same `Arc` pointer).
    ///
    /// # Errors
    /// `IllegalOperation` if `kind` is not a primitive.
    pub fn get_primitive_type(kind: TypeKind) -> Result<DynamicType, DynamicError> {
        if !kind.is_primitive() {
            return Err(DynamicError::IllegalOperation(alloc::format!(
                "get_primitive_type called with non-primitive {kind:?}"
            )));
        }
        Ok(primitive_singleton(kind))
    }

    /// Spec §7.5.5.1.3 `create_string_type(bound)` — bounded `string<N>`.
    #[must_use]
    pub fn create_string_type(bound: u32) -> DynamicType {
        DynamicType::from_inner(DynamicTypeInner {
            descriptor: TypeDescriptor::string8(bound),
            members: Vec::new(),
        })
    }

    /// Spec §7.5.5.1.4 `create_wstring_type(bound)`.
    #[must_use]
    pub fn create_wstring_type(bound: u32) -> DynamicType {
        DynamicType::from_inner(DynamicTypeInner {
            descriptor: TypeDescriptor::string16(bound),
            members: Vec::new(),
        })
    }
}

// ----------------------------------------------------------------------
// Primitive-Singleton-Cache
// ----------------------------------------------------------------------

#[cfg(feature = "std")]
fn primitive_singleton(kind: TypeKind) -> DynamicType {
    use std::sync::OnceLock;
    type Cell = OnceLock<DynamicType>;
    macro_rules! cell {
        () => {{
            static C: Cell = OnceLock::new();
            &C
        }};
    }
    let cell: &Cell = match kind {
        TypeKind::Boolean => cell!(),
        TypeKind::Byte => cell!(),
        TypeKind::Int8 => cell!(),
        TypeKind::UInt8 => cell!(),
        TypeKind::Int16 => cell!(),
        TypeKind::UInt16 => cell!(),
        TypeKind::Int32 => cell!(),
        TypeKind::UInt32 => cell!(),
        TypeKind::Int64 => cell!(),
        TypeKind::UInt64 => cell!(),
        TypeKind::Float32 => cell!(),
        TypeKind::Float64 => cell!(),
        TypeKind::Float128 => cell!(),
        TypeKind::Char8 => cell!(),
        TypeKind::Char16 => cell!(),
        // Defensive fallback for non-primitive kinds: build anew on each
        // call — the singleton property only holds for primitives, as
        // the spec caller in `get_primitive_type` validates.
        _ => {
            return DynamicType::from_inner(DynamicTypeInner {
                descriptor: TypeDescriptor::primitive(
                    kind,
                    alloc::string::String::from(primitive_name(kind)),
                ),
                members: Vec::new(),
            });
        }
    };
    cell.get_or_init(|| {
        DynamicType::from_inner(DynamicTypeInner {
            descriptor: TypeDescriptor::primitive(
                kind,
                alloc::string::String::from(primitive_name(kind)),
            ),
            members: Vec::new(),
        })
    })
    .clone()
}

#[cfg(not(feature = "std"))]
fn primitive_singleton(kind: TypeKind) -> DynamicType {
    // no_std path: no OnceLock — we build anew each time. The singleton
    // property is thus structural (same content) instead of
    // identity-based.
    DynamicType::from_inner(DynamicTypeInner {
        descriptor: TypeDescriptor::primitive(
            kind,
            alloc::string::String::from(primitive_name(kind)),
        ),
        members: Vec::new(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn create_type_rejects_invalid_descriptor() {
        let mut bad = TypeDescriptor::structure("");
        bad.kind = TypeKind::Structure;
        let err = DynamicTypeBuilderFactory::create_type(bad).unwrap_err();
        assert!(matches!(err, DynamicError::Inconsistent(_)));
    }

    #[test]
    fn add_member_rejects_duplicate_name() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("a", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        let err = b
            .add_struct_member("a", 2, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap_err();
        assert!(matches!(err, DynamicError::BuilderConflict(_)));
    }

    #[test]
    fn add_member_rejects_duplicate_id() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("a", 5, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        let err = b
            .add_struct_member("b", 5, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap_err();
        assert!(matches!(err, DynamicError::BuilderConflict(_)));
    }

    #[test]
    fn add_member_on_primitive_is_illegal() {
        let mut b = DynamicTypeBuilder::new(TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        let err = b
            .add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap_err();
        assert!(matches!(err, DynamicError::IllegalOperation(_)));
    }

    #[test]
    fn build_twice_rejected() {
        let b = DynamicTypeBuilderFactory::create_struct("::S");
        let _ = b.build().unwrap();
        let err = b.build().unwrap_err();
        assert!(matches!(err, DynamicError::PreconditionNotMet(_)));
    }

    #[test]
    fn primitive_singleton_returns_same_arc() {
        let a = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
        let b = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
        // same Arc pointer (singleton).
        assert!(Arc::ptr_eq(&a.inner, &b.inner));
    }

    #[test]
    fn primitive_singleton_rejects_non_primitive() {
        assert!(matches!(
            DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Structure),
            Err(DynamicError::IllegalOperation(_))
        ));
    }

    #[test]
    fn union_build_requires_at_least_one_member() {
        let disc = TypeDescriptor::primitive(TypeKind::Int32, "int32");
        let b = DynamicTypeBuilderFactory::create_union("::U", disc).unwrap();
        let err = b.build().unwrap_err();
        assert!(matches!(err, DynamicError::BuilderConflict(_)));
    }

    #[test]
    fn union_duplicate_label_rejected() {
        let disc = TypeDescriptor::primitive(TypeKind::Int32, "int32");
        let mut b = DynamicTypeBuilderFactory::create_union("::U", disc).unwrap();
        let mut a =
            MemberDescriptor::new("a", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        a.label = alloc::vec![1, 2];
        b.add_member(a).unwrap();
        let mut c =
            MemberDescriptor::new("c", 2, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        c.label = alloc::vec![2, 3];
        b.add_member(c).unwrap();
        let err = b.build().unwrap_err();
        assert!(matches!(err, DynamicError::BuilderConflict(_)));
    }

    #[test]
    fn build_simple_struct_with_three_members() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("a", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        b.add_struct_member("b", 2, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
            .unwrap();
        b.add_struct_member("c", 3, TypeDescriptor::string8(64))
            .unwrap();
        let t = b.build().unwrap();
        assert_eq!(t.member_count(), 3);
        assert_eq!(t.member_by_name("b").unwrap().id(), 2);
        assert_eq!(t.member_by_id(3).unwrap().name(), "c");
        assert_eq!(t.member_by_index(0).unwrap().name(), "a");
    }
}
