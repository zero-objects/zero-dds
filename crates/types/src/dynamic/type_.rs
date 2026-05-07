// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicType + DynamicTypeMember (XTypes 1.3 §7.5.3).
//!
//! `DynamicType` ist eine Read-Only-Sicht auf einen zur Laufzeit
//! konstruierten Typ. Konstruktion erfolgt via
//! [`crate::dynamic::DynamicTypeBuilder`], danach ist die Instanz
//! immutable.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::descriptor::{MemberDescriptor, MemberId, TypeDescriptor, TypeKind};
use super::error::DynamicError;

/// Member-Sicht auf einen DynamicType (Spec §7.5.3.4 GetMember).
///
/// Jeder Member kapselt seinen Descriptor + den ihm zugeordneten
/// (rekursiven) DynamicType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicTypeMember {
    pub(super) descriptor: MemberDescriptor,
    pub(super) member_type: DynamicType,
}

impl DynamicTypeMember {
    /// Lese-Sicht auf den Descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &MemberDescriptor {
        &self.descriptor
    }

    /// Member-Name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.descriptor.name
    }

    /// Member-Id.
    #[must_use]
    pub fn id(&self) -> MemberId {
        self.descriptor.id
    }

    /// `index` im Composite (Reihenfolge).
    #[must_use]
    pub fn index(&self) -> u32 {
        self.descriptor.index
    }

    /// Vollstaendiger DynamicType dieses Members.
    #[must_use]
    pub fn dynamic_type(&self) -> &DynamicType {
        &self.member_type
    }

    /// `equals` per Spec §7.5.3.5: Deep-Equality.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.member_type.equals(&other.member_type)
    }
}

/// Innerer Zustand eines DynamicType — referenz-gezaehlt fuer billiges
/// Cloning. Gleicher Inhalt → strukturell gleich (PartialEq), aber ggf.
/// anderer Arc-Pointer (Equality wird strukturell verglichen).
#[derive(Debug)]
pub(super) struct DynamicTypeInner {
    pub(super) descriptor: TypeDescriptor,
    pub(super) members: Vec<DynamicTypeMember>,
}

impl PartialEq for DynamicTypeInner {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.members == other.members
    }
}

impl Eq for DynamicTypeInner {}

/// XTypes 1.3 §7.5.3 DynamicType.
///
/// Read-only-API auf einen zur Laufzeit konstruierten Type. Erzeugt
/// werden DynamicTypes ausschliesslich via `DynamicTypeBuilder::build`
/// oder `DynamicTypeBuilderFactory::get_primitive_type`.
#[derive(Debug, Clone)]
pub struct DynamicType {
    pub(super) inner: Arc<DynamicTypeInner>,
}

impl PartialEq for DynamicType {
    fn eq(&self, other: &Self) -> bool {
        // Strukturelle Gleichheit (die `equals`-Spec-Methode); Arc-Identity
        // ist nur eine Optimierung.
        Arc::ptr_eq(&self.inner, &other.inner) || *self.inner == *other.inner
    }
}

impl Eq for DynamicType {}

impl DynamicType {
    pub(super) fn from_inner(inner: DynamicTypeInner) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Spec §7.5.3.1.1 `get_name()`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.descriptor.name
    }

    /// Spec §7.5.3.1.2 `get_kind()`.
    #[must_use]
    pub fn kind(&self) -> TypeKind {
        self.inner.descriptor.kind
    }

    /// Spec §7.5.3.1 `get_descriptor()`.
    #[must_use]
    pub fn descriptor(&self) -> &TypeDescriptor {
        &self.inner.descriptor
    }

    /// Spec §7.5.3.4.1 `get_member_count()`.
    #[must_use]
    pub fn member_count(&self) -> u32 {
        u32::try_from(self.inner.members.len()).unwrap_or(u32::MAX)
    }

    /// Spec §7.5.3.4.4 `get_member_by_index(index)`.
    #[must_use]
    pub fn member_by_index(&self, index: u32) -> Option<&DynamicTypeMember> {
        self.inner.members.get(index as usize)
    }

    /// Spec §7.5.3.4.2 `get_member(MemberId)`.
    #[must_use]
    pub fn member_by_id(&self, id: MemberId) -> Option<&DynamicTypeMember> {
        self.inner.members.iter().find(|m| m.descriptor.id == id)
    }

    /// Spec §7.5.3.4.3 `get_member_by_name(name)`.
    #[must_use]
    pub fn member_by_name(&self, name: &str) -> Option<&DynamicTypeMember> {
        self.inner
            .members
            .iter()
            .find(|m| m.descriptor.name == name)
    }

    /// Iterator ueber alle Members in Index-Reihenfolge.
    pub fn members(&self) -> impl Iterator<Item = &DynamicTypeMember> {
        self.inner.members.iter()
    }

    /// Spec §7.5.3.5 `equals(other)` — Deep-Equality.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.inner, &other.inner) {
            return true;
        }
        if self.inner.descriptor != other.inner.descriptor
            || self.inner.members.len() != other.inner.members.len()
        {
            return false;
        }
        self.inner
            .members
            .iter()
            .zip(other.inner.members.iter())
            .all(|(a, b)| a.equals(b))
    }

    /// True wenn der Typ ein Composite-Type ist (Members traegt).
    #[must_use]
    pub fn is_aggregable(&self) -> bool {
        self.kind().is_aggregable()
    }

    /// Validiert, dass der Typ insgesamt konsistent ist (Spec §7.5.3.5
    /// + Block-A-Constraints + Member-Konsistenz).
    ///
    /// # Errors
    /// `DynamicError::Inconsistent` mit Detail.
    pub fn is_consistent(&self) -> Result<(), DynamicError> {
        self.inner
            .descriptor
            .is_consistent()
            .map_err(DynamicError::inconsistent)?;
        for m in &self.inner.members {
            m.descriptor
                .is_consistent()
                .map_err(DynamicError::inconsistent)?;
        }
        Ok(())
    }

    /// Convenience: erzeugt einen primitiven `DynamicType` fuer Type-Bridges.
    ///
    /// Fuer mehrfach-genutzte Primitive bevorzuge
    /// [`crate::dynamic::DynamicTypeBuilderFactory::get_primitive_type`]
    /// (Singleton-Cache).
    #[must_use]
    pub fn new_primitive(kind: TypeKind) -> Self {
        let name = primitive_name(kind);
        Self::from_inner(DynamicTypeInner {
            descriptor: TypeDescriptor::primitive(kind, String::from(name)),
            members: Vec::new(),
        })
    }
}

/// Kanonisches Spec-Mapping kind → Name (Spec §7.5.1 Table 10 + IDL-Spelling).
pub(super) const fn primitive_name(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Boolean => "boolean",
        TypeKind::Byte => "octet",
        TypeKind::Int8 => "int8",
        TypeKind::UInt8 => "uint8",
        TypeKind::Int16 => "int16",
        TypeKind::UInt16 => "uint16",
        TypeKind::Int32 => "int32",
        TypeKind::UInt32 => "uint32",
        TypeKind::Int64 => "int64",
        TypeKind::UInt64 => "uint64",
        TypeKind::Float32 => "float",
        TypeKind::Float64 => "double",
        TypeKind::Float128 => "long double",
        TypeKind::Char8 => "char",
        TypeKind::Char16 => "wchar",
        _ => "<non-primitive>",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn primitive_dynamic_type_has_correct_kind_and_name() {
        let t = DynamicType::new_primitive(TypeKind::Int32);
        assert_eq!(t.kind(), TypeKind::Int32);
        assert_eq!(t.name(), "int32");
        assert_eq!(t.member_count(), 0);
    }

    #[test]
    fn equals_is_reflexive_and_value_based() {
        let a = DynamicType::new_primitive(TypeKind::Int32);
        let b = DynamicType::new_primitive(TypeKind::Int32);
        assert!(a.equals(&a));
        // verschiedene Arcs, aber strukturell gleich:
        assert!(a.equals(&b));
        assert_eq!(a, b);
    }

    #[test]
    fn equals_distinguishes_different_kinds() {
        let a = DynamicType::new_primitive(TypeKind::Int32);
        let b = DynamicType::new_primitive(TypeKind::Int64);
        assert!(!a.equals(&b));
    }
}
