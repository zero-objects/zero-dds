// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicData (XTypes 1.3 §7.5.6).
//!
//! `DynamicData` is a data instance of a `DynamicType`. The API
//! is spec-frozen (method names + signatures). Relative to the spec,
//! two simplifications are made:
//!
//! 1. All 12 typed getters/setters go through a `DynamicValue`
//!    discriminator enum, by which the `set_<T>_value`/`get_<T>_value`
//!    methods do their type check. Type mismatch → `BadParameter`.
//! 2. Loans are modeled via a reference-counted `DataLoan`
//!    (Spec §7.5.6.1 — the full loan API follows with C4.7).

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use super::descriptor::{MemberId, TypeKind};
use super::error::DynamicError;
use super::type_::DynamicType;

// ----------------------------------------------------------------------
// DynamicValue — internal storage enum
// ----------------------------------------------------------------------

/// Sum type for all values storable in DynamicData. Spec §7.5.6
/// speaks of 12 primitive types + composite + sequence — this enum
/// encapsulates the storage.
#[derive(Debug, Clone)]
pub enum DynamicValue {
    /// `bool`.
    Bool(bool),
    /// 8-bit unsigned (`octet`).
    Byte(u8),
    /// `int8`.
    Int8(i8),
    /// `uint8`.
    UInt8(u8),
    /// `int16`.
    Int16(i16),
    /// `uint16`.
    UInt16(u16),
    /// `int32`.
    Int32(i32),
    /// `uint32`.
    UInt32(u32),
    /// `int64`.
    Int64(i64),
    /// `uint64`.
    UInt64(u64),
    /// `float`.
    Float32(f32),
    /// `double`.
    Float64(f64),
    /// `char`.
    Char8(u8),
    /// `wchar`.
    Char16(u16),
    /// `string`.
    String(String),
    /// `wstring` (UTF-16 as Vec<u16>).
    WString(Vec<u16>),
    /// Composite or sequence — embedded `DynamicData`.
    Complex(Box<DynamicData>),
    /// Sequence of DynamicData (spec: the element type is known from the
    /// containerizing TypeDescriptor).
    Sequence(Vec<DynamicData>),
    /// Discard / not set (the default value is returned).
    None,
}

impl PartialEq for DynamicValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Int8(a), Self::Int8(b)) => a == b,
            (Self::UInt8(a), Self::UInt8(b)) => a == b,
            (Self::Int16(a), Self::Int16(b)) => a == b,
            (Self::UInt16(a), Self::UInt16(b)) => a == b,
            (Self::Int32(a), Self::Int32(b)) => a == b,
            (Self::UInt32(a), Self::UInt32(b)) => a == b,
            (Self::Int64(a), Self::Int64(b)) => a == b,
            (Self::UInt64(a), Self::UInt64(b)) => a == b,
            (Self::Float32(a), Self::Float32(b)) => a.to_bits() == b.to_bits(),
            (Self::Float64(a), Self::Float64(b)) => a.to_bits() == b.to_bits(),
            (Self::Char8(a), Self::Char8(b)) => a == b,
            (Self::Char16(a), Self::Char16(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::WString(a), Self::WString(b)) => a == b,
            (Self::Complex(a), Self::Complex(b)) => a.equals(b),
            (Self::Sequence(a), Self::Sequence(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

impl DynamicValue {
    fn kind_str(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Byte(_) => "byte",
            Self::Int8(_) => "int8",
            Self::UInt8(_) => "uint8",
            Self::Int16(_) => "int16",
            Self::UInt16(_) => "uint16",
            Self::Int32(_) => "int32",
            Self::UInt32(_) => "uint32",
            Self::Int64(_) => "int64",
            Self::UInt64(_) => "uint64",
            Self::Float32(_) => "float32",
            Self::Float64(_) => "float64",
            Self::Char8(_) => "char8",
            Self::Char16(_) => "char16",
            Self::String(_) => "string",
            Self::WString(_) => "wstring",
            Self::Complex(_) => "complex",
            Self::Sequence(_) => "sequence",
            Self::None => "none",
        }
    }
}

// ----------------------------------------------------------------------
// DataLoan — simple RAII wrapper
// ----------------------------------------------------------------------

/// Returns a loaned `DynamicData` view onto a member.
/// Lifecycle: `loan_value` → `return_loaned_value` (Spec §7.5.6.1).
///
/// Minimal in phase 4 — the full loan refcount comes with C4.7.
#[derive(Debug)]
pub struct DataLoan {
    member_id: MemberId,
    /// Index into the internal loans array — invalidated on `return`.
    handle: u32,
}

impl DataLoan {
    /// Member id for which the loan was created.
    #[must_use]
    pub fn member_id(&self) -> MemberId {
        self.member_id
    }
}

// ----------------------------------------------------------------------
// DynamicData
// ----------------------------------------------------------------------

/// XTypes 1.3 §7.5.6 DynamicData.
///
/// Data instance of a `DynamicType`. The type must be passed at
/// construction (`DynamicDataFactory::create_data`); afterwards
/// members can be read/written via `set_<T>_value(member_id, value)` /
/// `get_<T>_value`.
#[derive(Debug, Clone)]
pub struct DynamicData {
    type_: DynamicType,
    values: BTreeMap<MemberId, DynamicValue>,
    /// Active loans — for lifecycle validation.
    active_loans: Vec<u32>,
    /// Monotonically growing loan counter.
    next_loan_handle: u32,
}

impl DynamicData {
    pub(super) fn new(type_: DynamicType) -> Self {
        Self {
            type_,
            values: BTreeMap::new(),
            active_loans: Vec::new(),
            next_loan_handle: 1,
        }
    }

    /// Spec §7.5.6.2.1 `type()`.
    #[must_use]
    pub fn dynamic_type(&self) -> &DynamicType {
        &self.type_
    }

    /// Spec §7.5.6.2.6 `equals(other)` — Deep.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self.type_.equals(&other.type_) && self.values == other.values
    }

    /// Spec §7.5.6.2.4 `get_item_count()`.
    #[must_use]
    pub fn item_count(&self) -> u32 {
        u32::try_from(self.values.len()).unwrap_or(u32::MAX)
    }

    /// Checks whether `member_id` is defined in the type.
    fn validate_member(&self, member_id: MemberId) -> Result<(), DynamicError> {
        if self.type_.member_by_id(member_id).is_none() {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "no member with id {member_id} in type {}",
                self.type_.name()
            )));
        }
        Ok(())
    }

    /// General setter — type-safe via `DynamicValue` with
    /// Spec §7.5.4.1.2 TryConstruct apply semantics (C4.7): if the
    /// member type defines a `bound` and the value violates the bound
    /// (too long a string, too long a sequence, mismatched
    /// array length), the `try_construct` strategy decides:
    /// `Discard` leaves the member unset, `UseDefault` sets the
    /// `default_value`, `Trim` truncates to the bound.
    fn set(&mut self, member_id: MemberId, value: DynamicValue) -> Result<(), DynamicError> {
        self.validate_member(member_id)?;
        let member = self.type_.member_by_id(member_id).cloned();
        if let Some(ref m) = member {
            check_value_kind_matches_type(&value, m.dynamic_type().kind())?;
        }
        // TryConstruct apply (Spec §7.5.4.1.2). Members without bounds:
        // `apply_try_construct` returns `Accept(value)` unchanged.
        if let Some(m) = member {
            match crate::dynamic::try_construct::apply_try_construct(&m, value) {
                crate::dynamic::TryConstructOutcome::Accept(v)
                | crate::dynamic::TryConstructOutcome::UseDefault(v)
                | crate::dynamic::TryConstructOutcome::Trim(v) => {
                    self.values.insert(member_id, v);
                }
                crate::dynamic::TryConstructOutcome::Discard => {
                    // Member stays unset (Spec §7.5.4.1.2 Discard).
                    self.values.remove(&member_id);
                }
            }
        } else {
            self.values.insert(member_id, value);
        }
        Ok(())
    }

    /// Lese-Helper.
    fn get(&self, member_id: MemberId) -> Result<&DynamicValue, DynamicError> {
        self.values.get(&member_id).ok_or_else(|| {
            DynamicError::PreconditionNotMet(alloc::format!(
                "member {member_id} not set in {}",
                self.type_.name()
            ))
        })
    }

    /// Generic read of a member's raw [`DynamicValue`], or `None` if unset.
    ///
    /// The reflective codec ([`crate::dynamic::codec`]) uses this to walk a
    /// fully-populated value without the per-kind typed getters.
    #[must_use]
    pub fn get_value(&self, member_id: MemberId) -> Option<&DynamicValue> {
        self.values.get(&member_id)
    }

    /// Insert a raw [`DynamicValue`] for `member_id` WITHOUT TryConstruct
    /// bound-application — a faithful set for the reflective decoder, which
    /// reconstructs exactly what was on the wire. Unknown ids are accepted
    /// (the decoder only ever passes ids from the type's own members).
    pub fn set_value_raw(&mut self, member_id: MemberId, value: DynamicValue) {
        self.values.insert(member_id, value);
    }

    /// The member ids that currently carry a value, in member (index) order of
    /// the type. Used by the reflective encoder to emit members deterministically.
    #[must_use]
    pub fn set_member_ids(&self) -> alloc::vec::Vec<MemberId> {
        let mut ids: alloc::vec::Vec<MemberId> = self.values.keys().copied().collect();
        // Order by the type's declared member index so encode order is stable.
        ids.sort_by_key(|id| self.type_.member_by_id(*id).map_or(u32::MAX, |m| m.index()));
        ids
    }
}

// ----------------------------------------------------------------------
// 12 Primitive Getters/Setters (Spec §7.5.6.3)
// ----------------------------------------------------------------------

macro_rules! primitive_accessor {
    ($get_name:ident, $set_name:ident, $rust_ty:ty, $variant:ident) => {
        impl DynamicData {
            #[doc = concat!("Spec §7.5.6.3 `get_", stringify!($variant), "_value`.")]
            ///
            /// # Errors
            /// `BadParameter` on an unknown id, `Inconsistent` on a
            /// type mismatch.
            pub fn $get_name(&self, member_id: MemberId) -> Result<$rust_ty, DynamicError> {
                let v = self.get(member_id)?;
                if let DynamicValue::$variant(x) = v {
                    Ok(x.clone())
                } else {
                    Err(DynamicError::bad_parameter(alloc::format!(
                        "type mismatch: expected {} got {}",
                        stringify!($variant),
                        v.kind_str()
                    )))
                }
            }

            #[doc = concat!("Spec §7.5.6.3 `set_", stringify!($variant), "_value`.")]
            ///
            /// # Errors
            /// Type mismatch / unknown id.
            pub fn $set_name(
                &mut self,
                member_id: MemberId,
                value: $rust_ty,
            ) -> Result<(), DynamicError> {
                self.set(member_id, DynamicValue::$variant(value))
            }
        }
    };
}

primitive_accessor!(get_boolean_value, set_boolean_value, bool, Bool);
primitive_accessor!(get_byte_value, set_byte_value, u8, Byte);
primitive_accessor!(get_int8_value, set_int8_value, i8, Int8);
primitive_accessor!(get_uint8_value, set_uint8_value, u8, UInt8);
primitive_accessor!(get_int16_value, set_int16_value, i16, Int16);
primitive_accessor!(get_uint16_value, set_uint16_value, u16, UInt16);
primitive_accessor!(get_int32_value, set_int32_value, i32, Int32);
primitive_accessor!(get_uint32_value, set_uint32_value, u32, UInt32);
primitive_accessor!(get_int64_value, set_int64_value, i64, Int64);
primitive_accessor!(get_uint64_value, set_uint64_value, u64, UInt64);
primitive_accessor!(get_float32_value, set_float32_value, f32, Float32);
primitive_accessor!(get_float64_value, set_float64_value, f64, Float64);
primitive_accessor!(get_char8_value, set_char8_value, u8, Char8);
primitive_accessor!(get_char16_value, set_char16_value, u16, Char16);

// String + WString separat — Clone-Semantik vs. Move.
impl DynamicData {
    /// Spec §7.5.6.3 `get_string_value`.
    ///
    /// # Errors
    /// Type-Mismatch.
    pub fn get_string_value(&self, member_id: MemberId) -> Result<String, DynamicError> {
        let v = self.get(member_id)?;
        if let DynamicValue::String(s) = v {
            Ok(s.clone())
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "type mismatch: expected string got {}",
                v.kind_str()
            )))
        }
    }

    /// Spec §7.5.6.3 `set_string_value`.
    ///
    /// # Errors
    /// Type mismatch / unknown id.
    pub fn set_string_value(
        &mut self,
        member_id: MemberId,
        value: impl Into<String>,
    ) -> Result<(), DynamicError> {
        self.set(member_id, DynamicValue::String(value.into()))
    }

    /// Spec §7.5.6.3 `get_wstring_value`.
    ///
    /// # Errors
    /// Type-Mismatch.
    pub fn get_wstring_value(&self, member_id: MemberId) -> Result<Vec<u16>, DynamicError> {
        let v = self.get(member_id)?;
        if let DynamicValue::WString(s) = v {
            Ok(s.clone())
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "type mismatch: expected wstring got {}",
                v.kind_str()
            )))
        }
    }

    /// Spec §7.5.6.3 `set_wstring_value`.
    ///
    /// # Errors
    /// Type mismatch / unknown id.
    pub fn set_wstring_value(
        &mut self,
        member_id: MemberId,
        value: Vec<u16>,
    ) -> Result<(), DynamicError> {
        self.set(member_id, DynamicValue::WString(value))
    }
}

// ----------------------------------------------------------------------
// Composite + Sequence (Spec §7.5.6.5)
// ----------------------------------------------------------------------

impl DynamicData {
    /// Spec §7.5.6.5.1 `get_complex_value(member_id)`.
    ///
    /// Returns a read view onto the composite member.
    ///
    /// # Errors
    /// Type mismatch or member not set.
    pub fn get_complex_value(&self, member_id: MemberId) -> Result<&DynamicData, DynamicError> {
        let v = self.get(member_id)?;
        if let DynamicValue::Complex(d) = v {
            Ok(d)
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "type mismatch: expected complex got {}",
                v.kind_str()
            )))
        }
    }

    /// Spec §7.5.6.5.2 `set_complex_value(member_id, data)`.
    ///
    /// # Errors
    /// Type mismatch or member does not exist.
    pub fn set_complex_value(
        &mut self,
        member_id: MemberId,
        value: DynamicData,
    ) -> Result<(), DynamicError> {
        self.set(member_id, DynamicValue::Complex(Box::new(value)))
    }

    /// Returns the number of elements of a sequence member.
    ///
    /// # Errors
    /// Type mismatch or member not set.
    pub fn get_sequence_length(&self, member_id: MemberId) -> Result<u32, DynamicError> {
        let v = self.get(member_id)?;
        if let DynamicValue::Sequence(seq) = v {
            Ok(u32::try_from(seq.len()).unwrap_or(u32::MAX))
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "type mismatch: expected sequence got {}",
                v.kind_str()
            )))
        }
    }

    /// Sets a sequence completely.
    ///
    /// # Errors
    /// Type mismatch.
    pub fn set_sequence_value(
        &mut self,
        member_id: MemberId,
        value: Vec<DynamicData>,
    ) -> Result<(), DynamicError> {
        self.set(member_id, DynamicValue::Sequence(value))
    }

    /// Returns an element of a sequence member.
    ///
    /// # Errors
    /// Index out of bounds / type mismatch.
    pub fn get_sequence_element(
        &self,
        member_id: MemberId,
        index: u32,
    ) -> Result<&DynamicData, DynamicError> {
        let v = self.get(member_id)?;
        if let DynamicValue::Sequence(seq) = v {
            seq.get(index as usize).ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "sequence index {index} out of range (len={})",
                    seq.len()
                ))
            })
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "type mismatch: expected sequence got {}",
                v.kind_str()
            )))
        }
    }
}

// ----------------------------------------------------------------------
// Loans (Spec §7.5.6.1) — minimal, full lifecycle in C4.7
// ----------------------------------------------------------------------

impl DynamicData {
    /// Spec §7.5.6.1 `loan_value(member_id)`.
    ///
    /// Creates a `DataLoan` handle. Until `return_loaned_value` is
    /// called, another loan operation on the same
    /// member is not allowed.
    ///
    /// # Errors
    /// Member not found, or member already on loan.
    pub fn loan_value(&mut self, member_id: MemberId) -> Result<DataLoan, DynamicError> {
        self.validate_member(member_id)?;
        if self.active_loans.contains(&member_id) {
            return Err(DynamicError::loan(alloc::format!(
                "member {member_id} is already on loan"
            )));
        }
        self.active_loans.push(member_id);
        let handle = self.next_loan_handle;
        self.next_loan_handle = self.next_loan_handle.saturating_add(1);
        Ok(DataLoan { member_id, handle })
    }

    /// Spec §7.5.6.1 `return_loaned_value(loan)`.
    ///
    /// # Errors
    /// `LoanError` if the loan is unknown or already returned.
    pub fn return_loaned_value(&mut self, loan: DataLoan) -> Result<(), DynamicError> {
        let pos = self
            .active_loans
            .iter()
            .position(|m| *m == loan.member_id)
            .ok_or_else(|| {
                DynamicError::loan(alloc::format!(
                    "no active loan for member {}",
                    loan.member_id
                ))
            })?;
        self.active_loans.swap_remove(pos);
        // The handle value is not used further — only kept as a sanity
        // indicator that `return_loaned_value` receives a productive loan.
        let _ = loan.handle;
        Ok(())
    }
}

/// Validates: does the value discriminator match the member type kind?
fn check_value_kind_matches_type(value: &DynamicValue, kind: TypeKind) -> Result<(), DynamicError> {
    let ok = match (value, kind) {
        (DynamicValue::Bool(_), TypeKind::Boolean) => true,
        (DynamicValue::Byte(_), TypeKind::Byte) => true,
        (DynamicValue::Int8(_), TypeKind::Int8) => true,
        (DynamicValue::UInt8(_), TypeKind::UInt8) => true,
        (DynamicValue::Int16(_), TypeKind::Int16) => true,
        (DynamicValue::UInt16(_), TypeKind::UInt16) => true,
        (DynamicValue::Int32(_), TypeKind::Int32 | TypeKind::Enumeration) => true,
        (DynamicValue::UInt32(_), TypeKind::UInt32) => true,
        (DynamicValue::Int64(_), TypeKind::Int64) => true,
        (DynamicValue::UInt64(_), TypeKind::UInt64) => true,
        (DynamicValue::Float32(_), TypeKind::Float32) => true,
        (DynamicValue::Float64(_), TypeKind::Float64) => true,
        (DynamicValue::Char8(_), TypeKind::Char8) => true,
        (DynamicValue::Char16(_), TypeKind::Char16) => true,
        (DynamicValue::String(_), TypeKind::String8) => true,
        (DynamicValue::WString(_), TypeKind::String16) => true,
        (DynamicValue::Complex(_), k) if k.is_aggregable() => true,
        (DynamicValue::Sequence(_), TypeKind::Sequence | TypeKind::Array) => true,
        (DynamicValue::None, _) => true,
        _ => false,
    };
    if !ok {
        return Err(DynamicError::bad_parameter(alloc::format!(
            "value kind {} cannot be assigned to member kind {kind:?}",
            value.kind_str()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dynamic::DynamicTypeBuilderFactory;
    use crate::dynamic::descriptor::{MemberDescriptor, TypeDescriptor};

    fn struct_with_int32_member() -> DynamicType {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        b.build().unwrap()
    }

    #[test]
    fn set_get_int32_roundtrips() {
        let t = struct_with_int32_member();
        let mut d = DynamicData::new(t);
        d.set_int32_value(1, 42).unwrap();
        assert_eq!(d.get_int32_value(1).unwrap(), 42);
    }

    #[test]
    fn set_int32_into_int64_member_rejected() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
            .unwrap();
        let t = b.build().unwrap();
        let mut d = DynamicData::new(t);
        let err = d.set_int32_value(1, 5).unwrap_err();
        assert!(matches!(err, DynamicError::BadParameter(_)));
    }

    #[test]
    fn unknown_member_id_rejected() {
        let t = struct_with_int32_member();
        let mut d = DynamicData::new(t);
        let err = d.set_int32_value(999, 0).unwrap_err();
        assert!(matches!(err, DynamicError::BadParameter(_)));
    }

    #[test]
    fn loan_double_rejected() {
        let t = struct_with_int32_member();
        let mut d = DynamicData::new(t);
        let _l = d.loan_value(1).unwrap();
        let err = d.loan_value(1).unwrap_err();
        assert!(matches!(err, DynamicError::LoanError(_)));
    }

    #[test]
    fn loan_return_then_loan_again_works() {
        let t = struct_with_int32_member();
        let mut d = DynamicData::new(t);
        let l = d.loan_value(1).unwrap();
        d.return_loaned_value(l).unwrap();
        let _l2 = d.loan_value(1).unwrap();
    }

    #[test]
    fn return_unknown_loan_rejected() {
        let t = struct_with_int32_member();
        let mut d = DynamicData::new(t);
        let fake = DataLoan {
            member_id: 1,
            handle: 99,
        };
        let err = d.return_loaned_value(fake).unwrap_err();
        assert!(matches!(err, DynamicError::LoanError(_)));
    }

    #[test]
    fn item_count_grows_with_sets() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("a", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        b.add_struct_member("b", 2, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
            .unwrap();
        let t = b.build().unwrap();
        let mut d = DynamicData::new(t);
        assert_eq!(d.item_count(), 0);
        d.set_int32_value(1, 10).unwrap();
        assert_eq!(d.item_count(), 1);
        d.set_int64_value(2, 20).unwrap();
        assert_eq!(d.item_count(), 2);
    }

    #[test]
    fn equals_distinguishes_value_diffs() {
        let t = struct_with_int32_member();
        let mut a = DynamicData::new(t.clone());
        let mut b = DynamicData::new(t);
        a.set_int32_value(1, 5).unwrap();
        b.set_int32_value(1, 6).unwrap();
        assert!(!a.equals(&b));
        b.set_int32_value(1, 5).unwrap();
        assert!(a.equals(&b));
    }

    #[test]
    fn complex_value_set_get() {
        // outer struct with a nested struct member.
        let mut inner = DynamicTypeBuilderFactory::create_struct("::Inner");
        inner
            .add_struct_member("v", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        let inner_t = inner.build().unwrap();

        let mut outer = DynamicTypeBuilderFactory::create_struct("::Outer");
        outer
            .add_member(MemberDescriptor::new(
                "nested",
                10,
                TypeDescriptor::structure("::Inner"),
            ))
            .unwrap();
        let outer_t = outer.build().unwrap();

        let mut inner_data = DynamicData::new(inner_t);
        inner_data.set_int32_value(1, 99).unwrap();

        let mut outer_data = DynamicData::new(outer_t);
        outer_data.set_complex_value(10, inner_data).unwrap();

        let got = outer_data.get_complex_value(10).unwrap();
        assert_eq!(got.get_int32_value(1).unwrap(), 99);
    }

    #[test]
    fn sequence_length_and_element_access() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member(
            "items",
            1,
            TypeDescriptor::sequence(
                "items",
                TypeDescriptor::primitive(TypeKind::Int32, "int32"),
                100,
            ),
        )
        .unwrap();
        let t = b.build().unwrap();

        let elem_t = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
        let mut e0 = DynamicData::new(elem_t.clone());
        e0.values.insert(0, DynamicValue::Int32(7));
        let mut e1 = DynamicData::new(elem_t);
        e1.values.insert(0, DynamicValue::Int32(8));
        let mut d = DynamicData::new(t);
        d.set_sequence_value(1, alloc::vec![e0, e1]).unwrap();
        assert_eq!(d.get_sequence_length(1).unwrap(), 2);
        let _ = d.get_sequence_element(1, 0).unwrap();
        assert!(d.get_sequence_element(1, 9).is_err());
    }

    #[test]
    fn all_twelve_primitive_setters_work() {
        // This list is mandatory per Spec §7.5.6.3.
        let cases: alloc::vec::Vec<(&str, MemberId, TypeKind)> = alloc::vec![
            ("b", 1, TypeKind::Boolean),
            ("by", 2, TypeKind::Byte),
            ("i8", 3, TypeKind::Int8),
            ("u8", 4, TypeKind::UInt8),
            ("i16", 5, TypeKind::Int16),
            ("u16", 6, TypeKind::UInt16),
            ("i32", 7, TypeKind::Int32),
            ("u32", 8, TypeKind::UInt32),
            ("i64", 9, TypeKind::Int64),
            ("u64", 10, TypeKind::UInt64),
            ("f32", 11, TypeKind::Float32),
            ("f64", 12, TypeKind::Float64),
            ("c8", 13, TypeKind::Char8),
            ("c16", 14, TypeKind::Char16),
        ];
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        for (n, id, k) in &cases {
            b.add_struct_member(
                *n,
                *id,
                TypeDescriptor::primitive(*k, super::super::type_::primitive_name(*k)),
            )
            .unwrap();
        }
        // Plus string and WString.
        b.add_struct_member("s", 100, TypeDescriptor::string8(64))
            .unwrap();
        b.add_struct_member("w", 101, TypeDescriptor::string16(64))
            .unwrap();
        let t = b.build().unwrap();
        let mut d = DynamicData::new(t);
        d.set_boolean_value(1, true).unwrap();
        d.set_byte_value(2, 0xFF).unwrap();
        d.set_int8_value(3, -1).unwrap();
        d.set_uint8_value(4, 255).unwrap();
        d.set_int16_value(5, -2).unwrap();
        d.set_uint16_value(6, 2).unwrap();
        d.set_int32_value(7, -3).unwrap();
        d.set_uint32_value(8, 3).unwrap();
        d.set_int64_value(9, -4).unwrap();
        d.set_uint64_value(10, 4).unwrap();
        d.set_float32_value(11, 1.5).unwrap();
        d.set_float64_value(12, 2.5).unwrap();
        d.set_char8_value(13, b'A').unwrap();
        d.set_char16_value(14, 0x42).unwrap();
        d.set_string_value(100, "abc").unwrap();
        d.set_wstring_value(101, alloc::vec![0x41, 0x42]).unwrap();

        assert!(d.get_boolean_value(1).unwrap());
        assert_eq!(d.get_byte_value(2).unwrap(), 0xFF);
        assert_eq!(d.get_int8_value(3).unwrap(), -1);
        assert_eq!(d.get_uint8_value(4).unwrap(), 255);
        assert_eq!(d.get_int16_value(5).unwrap(), -2);
        assert_eq!(d.get_uint16_value(6).unwrap(), 2);
        assert_eq!(d.get_int32_value(7).unwrap(), -3);
        assert_eq!(d.get_uint32_value(8).unwrap(), 3);
        assert_eq!(d.get_int64_value(9).unwrap(), -4);
        assert_eq!(d.get_uint64_value(10).unwrap(), 4);
        assert!((d.get_float32_value(11).unwrap() - 1.5).abs() < 1e-6);
        assert!((d.get_float64_value(12).unwrap() - 2.5).abs() < 1e-9);
        assert_eq!(d.get_char8_value(13).unwrap(), b'A');
        assert_eq!(d.get_char16_value(14).unwrap(), 0x42);
        assert_eq!(d.get_string_value(100).unwrap(), "abc");
        assert_eq!(d.get_wstring_value(101).unwrap(), alloc::vec![0x41, 0x42]);
    }
}
