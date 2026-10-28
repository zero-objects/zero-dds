// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicData` with its type, and `DynamicDataFactory`. XTypes 1.3
//! §7.5.2.10, §7.5.2.11, Table 57, Table 58 and Annex C.
//!
//! [`super::DynamicData`] is a value without a type: the second
//! implementation of the codec's `Source` and `Sink`. §7.5.2.11's
//! `DynamicData` is a value **of a type** — "Each object of the DynamicData
//! class represents a corresponding object of the type represented by the
//! DynamicData object's DynamicType" — with the typed getters and setters
//! of Annex C. [`DynamicSample`] is that: a [`DynamicType`] and a
//! [`super::DynamicData`] that belongs to it.
//!
//! ## Which member id is which value
//!
//! §7.5.2.11.1 leaves most of the mapping unspecified, and this fixes it:
//!
//! | type | a value is | its member id |
//! |---|---|---|
//! | structure, annotation | a member | the member's id |
//! | union | a case member | the member's id |
//! | sequence, string, array | an element | its index (an array flattened) |
//! | map | a value | its index; a key is looked up by `get_member_id_by_name` |
//! | bitmask | a flag | its bit position |
//! | enumeration, primitive | the sample itself | `MEMBER_ID_INVALID` |
//!
//! A bit that no flag names cannot be held, because a bitmask value is
//! the set of flag names; reading it is `RETCODE_BAD_PARAMETER`.
//!
//! ## Reference semantics
//!
//! "DynamicData objects have reference semantics; however, there is an
//! equals operation to allow them to be deeply compared" (§7.5.2.11). A
//! [`DynamicSample`] is a handle on a value in a store it shares with the
//! handles it hands out: the value `get_complex_value` or `loan_value`
//! returns is **the value inside the sample**, and a setter on it is seen
//! through the sample, and the other way round. `clone` (§7.5.2.11.4) is
//! the deep copy, a sample of its own; [`DynamicSample::share`] is
//! another handle on the same sample.
//!
//! A handle names its value by the path from the root of the store. A
//! member that has no value yet (a non-optional one at its default) is
//! stored when the handle is taken, so that the handle has something to
//! refer to. What has no storage of its own is returned as a copy and
//! says so: a character of a string and a flag of a bitmask are
//! computed from the string and the set of names, not held, and a member
//! of a union that is not the selected one is `RETCODE_NO_DATA`. A handle
//! whose value is gone (the member was cleared, the element removed) reads
//! and writes `RETCODE_NO_DATA`.
//!
//! `loan_value` is the same handle with the clause's bookkeeping: at
//! most one loan outstanding, and `return_loaned_value` refuses what is
//! not that loan. Nothing is written back on return, because nothing was
//! taken out.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use zerodds_ast::PrimitiveKind as P;
use zerodds_float128::Float128;

use super::data::DynamicData;
use super::descriptor::{
    MEMBER_ID_INVALID, MemberDescriptor, MemberId, TryConstructKind, TypeKind,
};
use super::error::DynamicError;
use super::type_::{DynamicType, DynamicTypeMember};

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// Annex C `typedef sequence<long> Int32Seq`.
pub type Int32Seq = Vec<i32>;
/// Annex C `typedef sequence<unsigned long> UInt32Seq`.
pub type UInt32Seq = Vec<u32>;
/// Annex C `typedef sequence<int8> Int8Seq`.
pub type Int8Seq = Vec<i8>;
/// Annex C `typedef sequence<uint8> UInt8Seq`.
pub type UInt8Seq = Vec<u8>;
/// Annex C `typedef sequence<short> Int16Seq`.
pub type Int16Seq = Vec<i16>;
/// Annex C `typedef sequence<unsigned short> UInt16Seq`.
pub type UInt16Seq = Vec<u16>;
/// Annex C `typedef sequence<long long> Int64Seq`.
pub type Int64Seq = Vec<i64>;
/// Annex C `typedef sequence<unsigned long long> UInt64Seq`.
pub type UInt64Seq = Vec<u64>;
/// Annex C `typedef sequence<float> Float32Seq`.
pub type Float32Seq = Vec<f32>;
/// Annex C `typedef sequence<double> Float64Seq`.
pub type Float64Seq = Vec<f64>;
/// Annex C `typedef sequence<long double> Float128Seq`.
pub type Float128Seq = Vec<Float128>;
/// Annex C `typedef sequence<char> CharSeq`.
pub type CharSeq = Vec<char>;
/// Annex C `typedef sequence<wchar> WcharSeq`.
pub type WcharSeq = Vec<char>;
/// Annex C `typedef sequence<boolean> BooleanSeq`.
pub type BooleanSeq = Vec<bool>;
/// Annex C `typedef sequence<octet> ByteSeq`.
pub type ByteSeq = Vec<u8>;
/// `typedef sequence<string> StringSeq` (commented out in Annex C because
/// the DDS module declares it).
pub type StringSeq = Vec<String>;
/// Annex C `typedef sequence<wstring> WstringSeq`.
pub type WstringSeq = Vec<String>;

/// The factory of §7.5.2.10, "logically a singleton". It holds no state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DynamicDataFactory;

impl DynamicDataFactory {
    /// Table 57 `get_instance`.
    #[must_use]
    pub const fn get_instance() -> Self {
        Self
    }

    /// §7.5.2.10.3 `delete_instance`; nothing is held, so it succeeds.
    ///
    /// # Errors
    /// None.
    pub const fn delete_instance() -> Result<(), DynamicError> {
        Ok(())
    }

    /// §7.5.2.10.1 `create_data`: a new sample of `type_`, every
    /// non-optional member at its default (Table 9) and every optional
    /// one absent.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a type that has no kind;
    /// [`DynamicError::Inconsistent`] where a default cannot be named.
    pub fn create_data(&self, type_: &DynamicType) -> Result<DynamicSample, DynamicError> {
        DynamicSample::new(type_)
    }

    /// §7.5.2.10.2 `delete_data`: the sample is dropped. A sample that
    /// still has a value on loan is a sample with a loan outstanding.
    ///
    /// # Errors
    /// [`DynamicError::PreconditionNotMet`] when the sample has a loan
    /// outstanding.
    pub fn delete_data(&self, data: DynamicSample) -> Result<(), DynamicError> {
        if data.loan.is_some() {
            return Err(DynamicError::precondition_not_met(
                "the sample has a loaned value that was not returned",
            ));
        }
        drop(data);
        Ok(())
    }
}

/// Where in a sample one value sits.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Place {
    /// The sample itself.
    Whole,
    /// A member of an aggregate, by name.
    Member(String),
    /// An element of a sequence, a string or an array, flattened.
    Element(usize),
    /// A value of a map, by the index of its entry.
    Entry(usize),
    /// A flag of a bitmask, by name.
    Flag(String),
}

/// What a member id resolves to.
struct Target {
    place: Place,
    /// The type of the value there, aliases followed.
    ty: DynamicType,
    /// Whether it is an optional member.
    optional: bool,
    /// The member of an aggregate, where it is one.
    member: Option<DynamicTypeMember>,
}

/// A value on loan: where it was taken from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Loan {
    place: Place,
}

/// One step from a value to the value inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// A member of an aggregate.
    Member(String),
    /// An item of a collection, or the key (0) and the value (1) of a map
    /// entry.
    Index(usize),
}

/// The value at the end of `path`, from `root`.
fn locate<'a>(root: &'a DynamicData, path: &[Step]) -> Option<&'a DynamicData> {
    let mut at = root;
    for step in path {
        at = match step {
            Step::Member(name) => at.member(name)?,
            Step::Index(index) => at.items()?.get(*index)?,
        };
    }
    Some(at)
}

fn locate_mut<'a>(root: &'a mut DynamicData, path: &[Step]) -> Option<&'a mut DynamicData> {
    let mut at = root;
    for step in path {
        at = match (step, at) {
            (Step::Member(name), DynamicData::Aggregate(entries)) => {
                &mut entries.iter_mut().find(|(known, _)| known == name)?.1
            }
            (Step::Index(index), DynamicData::Collection(items)) => items.get_mut(*index)?,
            _ => return None,
        };
    }
    Some(at)
}

/// The steps from an array to its element `index`, flattened.
fn flat_steps(dims: &[usize], index: usize) -> Option<Vec<Step>> {
    let mut out = Vec::new();
    let mut rest = dims;
    let mut index = index;
    loop {
        let Some((_, tail)) = rest.split_first() else {
            out.push(Step::Index(index));
            return Some(out);
        };
        let stride: usize = tail.iter().product::<usize>().max(1);
        out.push(Step::Index(index / stride));
        if tail.is_empty() {
            return Some(out);
        }
        index %= stride;
        rest = tail;
    }
}

/// The value every handle on one sample shares.
type Store = Arc<Mutex<DynamicData>>;

fn gone() -> DynamicError {
    DynamicError::no_data("the value this handle refers to is no longer in the sample")
}

fn lock(store: &Store) -> Result<std::sync::MutexGuard<'_, DynamicData>, DynamicError> {
    store
        .lock()
        .map_err(|_| DynamicError::precondition_not_met("the sample's store is poisoned"))
}

/// A data sample and its type. §7.5.2.11.
///
/// A handle with reference semantics: see the module notes. Two handles
/// on the same sample are the same object, [`Self::clone`] is the copy.
pub struct DynamicSample {
    ty: DynamicType,
    store: Store,
    path: Vec<Step>,
    serial: u64,
    loan: Option<Loan>,
    loaned_from: Option<(u64, Place)>,
    pending_key: Option<String>,
}

impl core::fmt::Debug for DynamicSample {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "DynamicSample({})", self.ty.name())
    }
}

impl Clone for DynamicSample {
    /// §7.5.2.11.4 `clone`: "a new data sample with the same contents as
    /// this one". A clone is a sample of its own: it has no loan.
    fn clone(&self) -> Self {
        let value = self.snapshot().unwrap_or_else(|_| {
            // A handle whose value is gone has nothing to copy; the copy
            // is a sample of its type at its defaults, which is what a
            // sample with no value reads as.
            zerodds_wire_codec::default_of_type::<DynamicData>(self.ty.ast(), self.ty.id(), "")
                .unwrap_or(DynamicData::Aggregate(Vec::new()))
        });
        Self::with_value(self.ty.clone(), value)
    }
}

impl DynamicSample {
    /// A sample of `type_` at its defaults. See
    /// [`DynamicDataFactory::create_data`].
    ///
    /// # Errors
    /// As [`DynamicDataFactory::create_data`].
    pub fn new(type_: &DynamicType) -> Result<Self, DynamicError> {
        if type_.kind() == TypeKind::NoType {
            return Err(DynamicError::bad_parameter(
                "a type of no kind has no sample",
            ));
        }
        let value = zerodds_wire_codec::default_of_type::<DynamicData>(type_.ast(), type_.id(), "")
            .map_err(|error| DynamicError::Inconsistent {
                detail: error.to_string(),
            })?;
        Ok(Self::with_value(type_.clone(), value))
    }

    /// A sample of `type_` holding `value`, which the caller says belongs
    /// to it (it comes from `zerodds_wire_codec::decode` over this type,
    /// for instance). The value is not checked against the type.
    #[must_use]
    pub fn with_value(type_: DynamicType, value: DynamicData) -> Self {
        Self {
            ty: type_,
            store: Arc::new(Mutex::new(value)),
            path: Vec::new(),
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
            loan: None,
            loaned_from: None,
            pending_key: None,
        }
    }

    /// Another handle on this sample: the same object, and a change
    /// through either is seen through both (§7.5.2.11, "reference
    /// semantics"). It has no loan of its own.
    #[must_use]
    pub fn share(&self) -> Self {
        Self {
            ty: self.ty.clone(),
            store: Arc::clone(&self.store),
            path: self.path.clone(),
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
            loan: None,
            loaned_from: None,
            pending_key: None,
        }
    }

    /// Whether `other` is a handle on the same value of the same sample.
    #[must_use]
    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store) && self.path == other.path
    }

    /// The value, as it is now: a copy, for the codec.
    ///
    /// # Errors
    /// [`DynamicError::NoData`] where the value is gone from the sample.
    pub fn snapshot(&self) -> Result<DynamicData, DynamicError> {
        self.view(|value| Ok(value.clone()))
    }

    /// Runs `f` on the value, under the sample's lock. `f` calls nothing
    /// that takes the lock again.
    fn view<R>(
        &self,
        f: impl FnOnce(&DynamicData) -> Result<R, DynamicError>,
    ) -> Result<R, DynamicError> {
        let guard = lock(&self.store)?;
        let value = locate(&guard, &self.path).ok_or_else(gone)?;
        f(value)
    }

    fn view_mut<R>(
        &self,
        f: impl FnOnce(&mut DynamicData) -> Result<R, DynamicError>,
    ) -> Result<R, DynamicError> {
        let mut guard = lock(&self.store)?;
        let value = locate_mut(&mut guard, &self.path).ok_or_else(gone)?;
        f(value)
    }

    /// §7.5.2.11.8 `type`; "its value shall not be nil".
    #[must_use]
    pub const fn type_(&self) -> &DynamicType {
        &self.ty
    }

    /// The value, for the codec: a copy of what the sample holds now.
    ///
    /// # Errors
    /// [`DynamicError::NoData`] where the value is gone from the sample.
    pub fn value(&self) -> Result<DynamicData, DynamicError> {
        self.snapshot()
    }

    /// The value, taken out of the sample (a copy; the handle may be on a
    /// store others share).
    ///
    /// # Errors
    /// As [`Self::value`].
    pub fn into_value(self) -> Result<DynamicData, DynamicError> {
        self.snapshot()
    }

    // -- ids -----------------------------------------------------------

    fn base(&self) -> Result<DynamicType, DynamicError> {
        self.ty.resolve_alias()
    }

    /// Every member of an aggregate a sample of it can hold: a base type's
    /// before its own (§7.2.4.4.8).
    fn aggregate_members(ty: &DynamicType) -> Vec<DynamicTypeMember> {
        let mut out = ty
            .base_type()
            .and_then(|base| base.resolve_alias().ok())
            .map_or_else(Vec::new, |base| Self::aggregate_members(&base));
        out.extend(ty.members());
        out
    }

    fn dimensions(ty: &DynamicType) -> Vec<usize> {
        ty.descriptor().map_or_else(
            |_| Vec::new(),
            |d| d.bound.iter().map(|b| *b as usize).collect(),
        )
    }

    fn bound(ty: &DynamicType) -> Option<usize> {
        ty.descriptor()
            .ok()
            .and_then(|d| d.bound.first().copied())
            .map(|b| b as usize)
    }

    /// §7.5.2.11.1 `get_member_id_by_name`.
    ///
    /// For a map, a key the sample does not hold is answered with the id
    /// one past the last entry and remembered, so that the next setter
    /// appends a value under that key ("use get_member_id_by_name to
    /// obtain an ID for the new map key").
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] where the name selects no value.
    pub fn get_member_id_by_name(&mut self, name: &str) -> Result<MemberId, DynamicError> {
        let base = self.base()?;
        match base.kind() {
            TypeKind::Structure | TypeKind::Annotation | TypeKind::Union => {
                let members = if base.kind() == TypeKind::Union {
                    base.members()
                } else {
                    Self::aggregate_members(&base)
                };
                members
                    .into_iter()
                    .find(|member| member.name() == name)
                    .map(|member| member.id())
                    .ok_or_else(|| {
                        DynamicError::bad_parameter(alloc::format!(
                            "`{}` has no member `{name}`",
                            base.name()
                        ))
                    })
            }
            TypeKind::Map => {
                let (found, len) = self.view(|value| {
                    let entries = value.items().unwrap_or(&[]);
                    let found = entries.iter().position(|entry| {
                        entry
                            .items()
                            .and_then(|pair| pair.first())
                            .is_some_and(|key| key_text(key) == name)
                    });
                    Ok((found, entries.len()))
                })?;
                if let Some(index) = found {
                    return Ok(u32::try_from(index).unwrap_or(u32::MAX));
                }
                self.pending_key = Some(name.to_string());
                Ok(u32::try_from(len).unwrap_or(u32::MAX))
            }
            TypeKind::Bitmask => base
                .members()
                .into_iter()
                .find(|member| member.name() == name)
                .and_then(|member| member.literal_value())
                .ok_or_else(|| {
                    DynamicError::bad_parameter(alloc::format!("no flag named `{name}`"))
                }),
            TypeKind::Sequence | TypeKind::Array | TypeKind::String8 | TypeKind::String16 => {
                Err(DynamicError::bad_parameter(
                    "the elements of this type are reached by index, not by name",
                ))
            }
            _ if name.is_empty() => Ok(MEMBER_ID_INVALID),
            _ => Err(DynamicError::bad_parameter(
                "a value with no contained values is reached by a nil name",
            )),
        }
    }

    /// §7.5.2.11.1 `get_member_id_at_index`: indices are "relative to
    /// other values in a particular DynamicData object", so for an
    /// aggregate it is the position among the members **present**.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] past the end; one past the last
    /// element of a string, a sequence or a map is allowed, as the
    /// clause's way to append.
    pub fn get_member_id_at_index(&self, index: u32) -> Result<MemberId, DynamicError> {
        let base = self.base()?;
        let at = index as usize;
        match base.kind() {
            TypeKind::Structure | TypeKind::Annotation | TypeKind::Union => {
                let (name, held) = self.view(|value| {
                    let present = value.members().unwrap_or(&[]);
                    Ok((present.get(at).map(|(name, _)| name.clone()), present.len()))
                })?;
                let name = name.ok_or_else(|| {
                    DynamicError::bad_parameter(alloc::format!(
                        "the sample holds {held} members, none at index {index}"
                    ))
                })?;
                let name = &name;
                let members = if base.kind() == TypeKind::Union {
                    base.members()
                } else {
                    Self::aggregate_members(&base)
                };
                members
                    .into_iter()
                    .find(|member| member.name() == name)
                    .map(|member| member.id())
                    .ok_or_else(|| {
                        DynamicError::bad_parameter(alloc::format!(
                            "`{name}` is no member of `{}`",
                            base.name()
                        ))
                    })
            }
            TypeKind::Sequence | TypeKind::String8 | TypeKind::String16 | TypeKind::Map => {
                let len = self.view(|value| {
                    Ok(value.items().map_or_else(
                        || match value {
                            DynamicData::Text(text) => text.chars().count(),
                            _ => 0,
                        },
                        <[DynamicData]>::len,
                    ))
                })?;
                if at > len {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "index {index} is more than one past the last of {len}"
                    )));
                }
                Ok(index)
            }
            TypeKind::Array => {
                let total: usize = Self::dimensions(&base).iter().product();
                if at >= total {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "index {index} is outside an array of {total}"
                    )));
                }
                Ok(index)
            }
            TypeKind::Bitmask => {
                let bound = base.bit_bound().map_or(32, usize::from);
                if at >= bound {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "bit {index} is outside a bitmask of {bound}"
                    )));
                }
                Ok(index)
            }
            _ if at == 0 => Ok(MEMBER_ID_INVALID),
            _ => Err(DynamicError::bad_parameter(
                "a value with no contained values has only index 0",
            )),
        }
    }

    fn resolve(&self, id: MemberId) -> Result<Target, DynamicError> {
        let base = self.base()?;
        let kind = base.kind();
        match kind {
            TypeKind::Structure | TypeKind::Annotation | TypeKind::Union => {
                let members = if kind == TypeKind::Union {
                    base.members()
                } else {
                    Self::aggregate_members(&base)
                };
                let member = members
                    .into_iter()
                    .find(|member| member.id() == id)
                    .ok_or_else(|| {
                        DynamicError::bad_parameter(alloc::format!(
                            "`{}` has no member with the id {id}",
                            base.name()
                        ))
                    })?;
                let ty = member.dynamic_type()?.resolve_alias()?;
                let optional = member.descriptor()?.is_optional;
                Ok(Target {
                    place: Place::Member(member.name().to_string()),
                    ty,
                    optional,
                    member: Some(member),
                })
            }
            TypeKind::Sequence | TypeKind::String8 | TypeKind::String16 | TypeKind::Array => {
                let element = base
                    .element_type()
                    .ok_or_else(|| DynamicError::bad_parameter("a collection with no element"))?
                    .resolve_alias()?;
                if kind == TypeKind::Array {
                    let total: usize = Self::dimensions(&base).iter().product();
                    if id as usize >= total {
                        return Err(DynamicError::bad_parameter(alloc::format!(
                            "index {id} is outside an array of {total}"
                        )));
                    }
                }
                Ok(Target {
                    place: Place::Element(id as usize),
                    ty: element,
                    optional: false,
                    member: None,
                })
            }
            TypeKind::Map => {
                let element = base
                    .element_type()
                    .ok_or_else(|| DynamicError::bad_parameter("a map with no value type"))?
                    .resolve_alias()?;
                Ok(Target {
                    place: Place::Entry(id as usize),
                    ty: element,
                    optional: false,
                    member: None,
                })
            }
            TypeKind::Bitmask => {
                let flag = base
                    .members()
                    .into_iter()
                    .find(|member| member.literal_value() == Some(id))
                    .ok_or_else(|| {
                        DynamicError::bad_parameter(alloc::format!(
                            "no flag at bit {id}: a bitmask value holds named flags only"
                        ))
                    })?;
                Ok(Target {
                    place: Place::Flag(flag.name().to_string()),
                    ty: flag.dynamic_type()?,
                    optional: false,
                    member: Some(flag),
                })
            }
            _ if id == MEMBER_ID_INVALID => Ok(Target {
                place: Place::Whole,
                ty: base,
                optional: false,
                member: None,
            }),
            _ => Err(DynamicError::bad_parameter(
                "a value with no contained values is reached by MEMBER_ID_INVALID",
            )),
        }
    }

    /// [`Self::resolve`] for the operations on a whole sequence or array,
    /// where a sample that is itself the collection names it by
    /// `MEMBER_ID_INVALID` (the plural accessors have no other id for it).
    fn resolve_values(&self, id: MemberId) -> Result<Target, DynamicError> {
        let base = self.base()?;
        if id == MEMBER_ID_INVALID && matches!(base.kind(), TypeKind::Sequence | TypeKind::Array) {
            return Ok(Target {
                place: Place::Whole,
                ty: base,
                optional: false,
                member: None,
            });
        }
        self.resolve(id)
    }

    // -- reading and writing a place -------------------------------------

    fn read(&self, target: &Target) -> Result<DynamicData, DynamicError> {
        self.view(|value| self.read_in(value, target))
    }

    fn read_in(&self, value: &DynamicData, target: &Target) -> Result<DynamicData, DynamicError> {
        match &target.place {
            Place::Whole => Ok(value.clone()),
            Place::Member(name) => match value.member(name) {
                Some(value) => Ok(value.clone()),
                None if target.optional => Err(DynamicError::no_data(alloc::format!(
                    "the optional member `{name}` has no value"
                ))),
                None => match &target.member {
                    Some(member) => member.default_value(),
                    None => Err(DynamicError::no_data(alloc::format!(
                        "no value for `{name}`"
                    ))),
                },
            },
            Place::Element(index) => self.element(value, *index),
            Place::Entry(index) => {
                let entry = value
                    .items()
                    .and_then(|entries| entries.get(*index))
                    .and_then(DynamicData::items)
                    .ok_or_else(|| DynamicError::bad_parameter("no entry at that index"))?;
                entry
                    .get(1)
                    .cloned()
                    .ok_or_else(|| DynamicError::bad_parameter("an entry without a value"))
            }
            Place::Flag(name) => Ok(DynamicData::Boolean(value.items().is_some_and(|flags| {
                flags
                    .iter()
                    .any(|f| matches!(f, DynamicData::Text(t) if t == name))
            }))),
        }
    }

    fn element(&self, value: &DynamicData, index: usize) -> Result<DynamicData, DynamicError> {
        let base = self.base()?;
        match (value, base.kind()) {
            (DynamicData::Text(text), TypeKind::String8 | TypeKind::String16) => {
                let wide = base.kind() == TypeKind::String16;
                text.chars()
                    .nth(index)
                    .map(|c| DynamicData::character(if wide { P::WChar } else { P::Char }, c))
                    .ok_or_else(|| {
                        DynamicError::bad_parameter(alloc::format!("no character at {index}"))
                    })
            }
            (DynamicData::Collection(items), TypeKind::Array) => {
                flat_get(items, &Self::dimensions(&base), index)
                    .cloned()
                    .ok_or_else(|| DynamicError::bad_parameter("no element at that index"))
            }
            (DynamicData::Collection(items), _) => items
                .get(index)
                .cloned()
                .ok_or_else(|| DynamicError::bad_parameter("no element at that index")),
            _ => Err(DynamicError::bad_parameter("the sample holds no elements")),
        }
    }

    fn write(&mut self, target: &Target, new: DynamicData) -> Result<(), DynamicError> {
        let store = Arc::clone(&self.store);
        let mut guard = lock(&store)?;
        let value = locate_mut(&mut guard, &self.path).ok_or_else(gone)?;
        self.write_in(value, target, new)
    }

    fn write_in(
        &mut self,
        value: &mut DynamicData,
        target: &Target,
        new: DynamicData,
    ) -> Result<(), DynamicError> {
        match &target.place {
            Place::Whole => {
                *value = new;
                Ok(())
            }
            Place::Member(name) => {
                let union_member = self.base()?.kind() == TypeKind::Union;
                if union_member {
                    self.select_union_member(value, target, name)?;
                }
                let DynamicData::Aggregate(entries) = value else {
                    return Err(DynamicError::bad_parameter("the sample is no aggregate"));
                };
                match entries.iter_mut().find(|(known, _)| known == name) {
                    Some((_, slot)) => *slot = new,
                    None => entries.push((name.clone(), new)),
                }
                Ok(())
            }
            Place::Element(index) => self.write_element(value, *index, new),
            Place::Entry(index) => self.write_entry(value, *index, new),
            Place::Flag(name) => {
                let set = matches!(new, DynamicData::Boolean(true));
                let order: Vec<String> = self
                    .base()?
                    .members()
                    .iter()
                    .map(|m| m.name().to_string())
                    .collect();
                let DynamicData::Bits(flags) = value else {
                    return Err(DynamicError::bad_parameter("the sample is no bitmask"));
                };
                flags.retain(|f| !matches!(f, DynamicData::Text(t) if t == name));
                if set {
                    flags.push(DynamicData::Text(name.clone()));
                    flags.sort_by_key(|f| match f {
                        DynamicData::Text(t) => {
                            order.iter().position(|o| o == t).unwrap_or(usize::MAX)
                        }
                        _ => usize::MAX,
                    });
                }
                Ok(())
            }
        }
    }

    fn write_element(
        &mut self,
        value: &mut DynamicData,
        index: usize,
        new: DynamicData,
    ) -> Result<(), DynamicError> {
        let base = self.base()?;
        let kind = base.kind();
        let bound = Self::bound(&base);
        match (value, kind) {
            (DynamicData::Text(text), TypeKind::String8 | TypeKind::String16) => {
                let character = new.as_char().ok_or_else(|| {
                    DynamicError::bad_parameter("a string element is a character")
                })?;
                let mut chars: Vec<char> = text.chars().collect();
                if index < chars.len() {
                    chars[index] = character;
                } else if index == chars.len() {
                    if bound.is_some_and(|b| index >= b) {
                        return Err(DynamicError::bad_parameter("the string is at its bound"));
                    }
                    chars.push(character);
                } else {
                    return Err(DynamicError::bad_parameter("an index past the end"));
                }
                *text = chars.into_iter().collect();
                Ok(())
            }
            (DynamicData::Collection(items), TypeKind::Array) => {
                let dims = Self::dimensions(&base);
                flat_set(items, &dims, index, new)
                    .ok_or_else(|| DynamicError::bad_parameter("no element at that index"))
            }
            (DynamicData::Collection(items), _) => {
                if index < items.len() {
                    items[index] = new;
                } else if index == items.len() {
                    if bound.is_some_and(|b| index >= b) {
                        return Err(DynamicError::bad_parameter(
                            "the collection is at its bound",
                        ));
                    }
                    items.push(new);
                } else {
                    return Err(DynamicError::bad_parameter("an index past the end"));
                }
                Ok(())
            }
            _ => Err(DynamicError::bad_parameter("the sample holds no elements")),
        }
    }

    fn write_entry(
        &mut self,
        value: &mut DynamicData,
        index: usize,
        new: DynamicData,
    ) -> Result<(), DynamicError> {
        let base = self.base()?;
        let bound = Self::bound(&base);
        let key = self.pending_key.take();
        let DynamicData::Collection(entries) = value else {
            return Err(DynamicError::bad_parameter("the sample is no map"));
        };
        if index < entries.len() {
            if let DynamicData::Collection(pair) = &mut entries[index] {
                if pair.len() == 2 {
                    pair[1] = new;
                    return Ok(());
                }
            }
            return Err(DynamicError::bad_parameter("a malformed map entry"));
        }
        if index == entries.len() {
            let key = key.ok_or_else(|| {
                DynamicError::bad_parameter(
                    "a new map entry needs its key: ask get_member_id_by_name for the id",
                )
            })?;
            if bound.is_some_and(|b| index >= b) {
                return Err(DynamicError::bad_parameter("the map is at its bound"));
            }
            let key_type = base
                .key_element_type()
                .ok_or_else(|| DynamicError::bad_parameter("a map with no key type"))?
                .resolve_alias()?;
            let key_value = key_from_text(&key_type, &key)?;
            entries.push(DynamicData::Collection(alloc::vec![key_value, new]));
            return Ok(());
        }
        Err(DynamicError::bad_parameter("an index past the end"))
    }

    /// Writing a union member selects it: the discriminator is set to the
    /// member's first label, or, for the default member, to a value no
    /// other member's label selects.
    fn select_union_member(
        &mut self,
        value_of_union: &mut DynamicData,
        target: &Target,
        name: &str,
    ) -> Result<(), DynamicError> {
        let base = self.base()?;
        let Some(member) = target.member.as_ref() else {
            return Ok(());
        };
        let descriptor = member.descriptor()?;
        let discriminator_type = base
            .discriminator_type()
            .ok_or_else(|| DynamicError::bad_parameter("a union with no discriminator"))?
            .resolve_alias()?;
        let value = if let Some(label) = descriptor.label.first() {
            discriminator_value(&discriminator_type, *label)?
        } else if descriptor.is_default_label {
            let used: Vec<i64> = base
                .members()
                .iter()
                .filter_map(|m| m.descriptor().ok())
                .flat_map(|d| d.label)
                .collect();
            let free = (0_i64..)
                .find(|candidate| !used.contains(candidate))
                .unwrap_or(0);
            discriminator_value(&discriminator_type, free)?
        } else {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "`{name}` is selected by no label"
            )));
        };
        let DynamicData::Aggregate(entries) = value_of_union else {
            return Err(DynamicError::bad_parameter("the sample is no union"));
        };
        // A union holds the discriminator and the selected member only.
        entries.retain(|(known, _)| known == "_d" || known == name);
        match entries.iter_mut().find(|(known, _)| known == "_d") {
            Some((_, slot)) => *slot = value,
            None => entries.insert(0, (String::from("_d"), value)),
        }
        Ok(())
    }

    // -- descriptor, count, equality, clone --------------------------------

    /// §7.5.2.11.2 `get_descriptor`: the descriptor of the value `id`
    /// names.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an id that names no value.
    pub fn get_descriptor(&self, id: MemberId) -> Result<MemberDescriptor, DynamicError> {
        let target = self.resolve(id)?;
        if let Some(member) = &target.member {
            return member.descriptor();
        }
        let index = match target.place {
            Place::Element(i) | Place::Entry(i) => u32::try_from(i).unwrap_or(u32::MAX),
            _ => 0,
        };
        Ok(MemberDescriptor {
            name: String::new(),
            id,
            type_: target.ty,
            index,
            label: Vec::new(),
            is_default_label: false,
            try_construct_kind: TryConstructKind::Discard,
            is_key: false,
            is_optional: false,
            is_must_understand: false,
            is_shared: false,
            has_default_value: false,
            default_value: String::new(),
        })
    }

    /// Annex C `set_descriptor`. The descriptor of a value is the type's
    /// (`DynamicType` is frozen), so this succeeds when `descriptor`
    /// equals the one the value has and is refused otherwise.
    ///
    /// # Errors
    /// [`DynamicError::PreconditionNotMet`] for a descriptor that differs.
    pub fn set_descriptor(
        &self,
        id: MemberId,
        descriptor: &MemberDescriptor,
    ) -> Result<(), DynamicError> {
        if self.get_descriptor(id)?.equals(descriptor) {
            Ok(())
        } else {
            Err(DynamicError::precondition_not_met(
                "the descriptor of a value is its type's, and the type is not changed",
            ))
        }
    }

    /// §7.5.2.11.6 `get_item_count`.
    #[must_use]
    pub fn get_item_count(&self) -> u32 {
        let Ok(value) = self.snapshot() else {
            return 0;
        };
        let count = match &value {
            DynamicData::Bits(flags) => flags.len(),
            DynamicData::Aggregate(entries) | DynamicData::Bitfields(entries) => entries.len(),
            DynamicData::Collection(items) => self.base().map_or(items.len(), |base| {
                if base.kind() == TypeKind::Array {
                    Self::dimensions(&base).iter().product()
                } else {
                    items.len()
                }
            }),
            DynamicData::Text(text) => text.chars().count(),
            _ => 1,
        };
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// §7.5.2.11.5 `equals`: equal types, and values equal as the clause
    /// has it (members without regard to their order, a non-optional
    /// member left out equal to its default).
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        // Each value is copied out before the other is looked at: the two
        // handles may be on one sample, and a lock is not taken twice.
        let (Ok(mine), Ok(theirs)) = (self.snapshot(), other.snapshot()) else {
            return false;
        };
        self.ty.equals(&other.ty) && self.ty.values_equal(&mine, &theirs).unwrap_or(false)
    }

    // -- clearing -----------------------------------------------------------

    /// §7.5.2.11.3 `clear_all_values`.
    ///
    /// # Errors
    /// [`DynamicError::Inconsistent`] where a default cannot be named.
    pub fn clear_all_values(&mut self) -> Result<(), DynamicError> {
        let fresh =
            zerodds_wire_codec::default_of_type::<DynamicData>(self.ty.ast(), self.ty.id(), "")
                .map_err(|error| DynamicError::Inconsistent {
                    detail: error.to_string(),
                })?;
        self.view_mut(|value| {
            *value = fresh;
            Ok(())
        })
    }

    /// §7.5.2.11.3 `clear_nonkey_values`: "exactly the same effect as
    /// clear_all_values with one exception: the values of key fields of
    /// aggregated types retain their values."
    ///
    /// # Errors
    /// As [`Self::clear_all_values`].
    pub fn clear_nonkey_values(&mut self) -> Result<(), DynamicError> {
        let current = self.snapshot()?;
        let kept: Vec<(String, DynamicData)> = match (&current, self.base()?.kind()) {
            (DynamicData::Aggregate(entries), TypeKind::Structure | TypeKind::Annotation) => {
                let base = self.base()?;
                Self::aggregate_members(&base)
                    .iter()
                    .filter(|member| member.descriptor().is_ok_and(|d| d.is_key))
                    .filter_map(|member| {
                        entries
                            .iter()
                            .find(|(name, _)| name == member.name())
                            .cloned()
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        self.clear_all_values()?;
        self.view_mut(|current| {
            if let DynamicData::Aggregate(entries) = current {
                for (name, value) in kept {
                    match entries.iter_mut().find(|(known, _)| *known == name) {
                        Some((_, slot)) => *slot = value,
                        None => entries.push((name, value)),
                    }
                }
            }
            Ok(())
        })
    }

    /// §7.5.2.11.3 `clear_value`:
    ///
    /// - an aggregate: an optional member is removed, any other member
    ///   takes its default;
    /// - a variable-length collection (string, sequence, map): the
    ///   element is removed and the rest move down;
    /// - an array: the element takes its default;
    /// - a bitmask: the bit is cleared;
    /// - an enumeration: the first value;
    /// - a primitive: its default.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for an id that names no value.
    pub fn clear_value(&mut self, id: MemberId) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        let store = Arc::clone(&self.store);
        let mut guard = lock(&store)?;
        let value = locate_mut(&mut guard, &self.path).ok_or_else(gone)?;
        match &target.place {
            Place::Whole => {
                *value = if target.ty.kind() == TypeKind::Enumeration {
                    let first = target.ty.members().into_iter().next().ok_or_else(|| {
                        DynamicError::bad_parameter("an enumeration with no literal")
                    })?;
                    DynamicData::Enumerator(first.name().to_string())
                } else {
                    zerodds_wire_codec::default_of_type::<DynamicData>(
                        target.ty.ast(),
                        target.ty.id(),
                        "",
                    )
                    .map_err(|error| DynamicError::Inconsistent {
                        detail: error.to_string(),
                    })?
                };
                Ok(())
            }
            Place::Member(name) => {
                if target.optional {
                    if let DynamicData::Aggregate(entries) = value {
                        entries.retain(|(known, _)| known != name);
                    }
                    return Ok(());
                }
                let default = match &target.member {
                    Some(member) => member.default_value::<DynamicData>()?,
                    None => return Err(DynamicError::bad_parameter("no such member")),
                };
                self.write_in(value, &target, default)
            }
            Place::Element(index) => {
                let base = self.base()?;
                match (&mut *value, base.kind()) {
                    (DynamicData::Text(text), _) => {
                        let mut chars: Vec<char> = text.chars().collect();
                        if *index >= chars.len() {
                            return Err(DynamicError::bad_parameter("an index past the end"));
                        }
                        chars.remove(*index);
                        *text = chars.into_iter().collect();
                        Ok(())
                    }
                    (DynamicData::Collection(items), TypeKind::Sequence) => {
                        if *index >= items.len() {
                            return Err(DynamicError::bad_parameter("an index past the end"));
                        }
                        items.remove(*index);
                        Ok(())
                    }
                    _ => {
                        let default = zerodds_wire_codec::default_of_type::<DynamicData>(
                            target.ty.ast(),
                            target.ty.id(),
                            "",
                        )
                        .map_err(|error| DynamicError::Inconsistent {
                            detail: error.to_string(),
                        })?;
                        self.write_in(value, &target, default)
                    }
                }
            }
            Place::Entry(index) => {
                let DynamicData::Collection(entries) = value else {
                    return Err(DynamicError::bad_parameter("the sample is no map"));
                };
                if *index >= entries.len() {
                    return Err(DynamicError::bad_parameter("an index past the end"));
                }
                entries.remove(*index);
                Ok(())
            }
            Place::Flag(_) => self.write_in(value, &target, DynamicData::Boolean(false)),
        }
    }

    // -- loans --------------------------------------------------------------

    /// §7.5.2.11.7 `loan_value`: the value `id` names, as a handle on the
    /// value inside the sample. At most one loan is outstanding.
    ///
    /// # Errors
    /// [`DynamicError::PreconditionNotMet`] when a loan is outstanding
    /// ("the loan operation shall return a nil value");
    /// [`DynamicError::NoData`] when the value does not exist or has no
    /// storage of its own to refer to.
    pub fn loan_value(&mut self, id: MemberId) -> Result<Self, DynamicError> {
        if self.loan.is_some() {
            return Err(DynamicError::precondition_not_met(
                "a loan is outstanding; return it before loaning again",
            ));
        }
        let target = self.resolve(id)?;
        let mut child = self.reference_to(&target)?;
        child.loaned_from = Some((self.serial, target.place.clone()));
        self.loan = Some(Loan {
            place: target.place,
        });
        Ok(child)
    }

    /// §7.5.2.11.7 `return_loaned_value`: the loan is over. The value was
    /// never taken out, so nothing goes back; what the borrower did to it
    /// is in the sample.
    ///
    /// # Errors
    /// [`DynamicError::PreconditionNotMet`] when `value` is not the
    /// outstanding loan of this sample.
    pub fn return_loaned_value(&mut self, value: Self) -> Result<(), DynamicError> {
        let from = value.loaned_from.as_ref();
        let outstanding = self.loan.as_ref();
        let matches = match (from, outstanding) {
            (Some((serial, place)), Some(loan)) => *serial == self.serial && *place == loan.place,
            _ => false,
        };
        if !matches {
            return Err(DynamicError::precondition_not_met(
                "the value is not an outstanding loan from this sample",
            ));
        }
        self.loan = None;
        Ok(())
    }

    // -- the typed values -----------------------------------------------------

    /// A handle on the value `target` names, inside this sample.
    ///
    /// A non-optional member that has no value yet is stored at its
    /// default first. A character of a string, a flag of a bitmask and an
    /// unselected member of a union have no storage to refer to.
    fn reference_to(&self, target: &Target) -> Result<Self, DynamicError> {
        let mut path = self.path.clone();
        match &target.place {
            Place::Whole => return Ok(self.share()),
            Place::Member(name) => {
                let base = self.base()?;
                let name = name.clone();
                let present = self.view(|value| Ok(value.member(&name).is_some()))?;
                if !present {
                    if base.kind() == TypeKind::Union {
                        return Err(DynamicError::no_data(alloc::format!(
                            "`{name}` is not the selected member of the union"
                        )));
                    }
                    if target.optional {
                        return Err(DynamicError::no_data(alloc::format!(
                            "the optional member `{name}` has no value"
                        )));
                    }
                    let default = match &target.member {
                        Some(member) => member.default_value::<DynamicData>()?,
                        None => return Err(DynamicError::bad_parameter("no such member")),
                    };
                    self.view_mut(|value| {
                        let DynamicData::Aggregate(entries) = value else {
                            return Err(DynamicError::bad_parameter("the sample is no aggregate"));
                        };
                        entries.push((name.clone(), default));
                        Ok(())
                    })?;
                }
                path.push(Step::Member(name));
            }
            Place::Element(index) => {
                let base = self.base()?;
                match base.kind() {
                    TypeKind::String8 | TypeKind::String16 => {
                        // A character is computed from the string.
                        let value = self.read(target)?;
                        return Ok(Self::with_value(target.ty.clone(), value));
                    }
                    TypeKind::Array => {
                        let steps =
                            flat_steps(&Self::dimensions(&base), *index).ok_or_else(|| {
                                DynamicError::bad_parameter("no element at that index")
                            })?;
                        path.extend(steps);
                    }
                    _ => path.push(Step::Index(*index)),
                }
            }
            Place::Entry(index) => {
                path.push(Step::Index(*index));
                path.push(Step::Index(1));
            }
            Place::Flag(_) => {
                // A flag is a name in the set, not a value held.
                let value = self.read(target)?;
                return Ok(Self::with_value(target.ty.clone(), value));
            }
        }
        // The value has to be there for the handle to refer to it.
        let exists = {
            let guard = lock(&self.store)?;
            locate(&guard, &path).is_some()
        };
        if !exists {
            return Err(DynamicError::no_data(
                "the sample holds no value there to refer to",
            ));
        }
        Ok(Self {
            ty: target.ty.clone(),
            store: Arc::clone(&self.store),
            path,
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
            loan: None,
            loaned_from: None,
            pending_key: None,
        })
    }

    /// Annex C `get_complex_value`: the value `id` names, as a handle on
    /// the value inside this sample (§7.5.2.11, reference semantics).
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
    pub fn get_complex_value(&self, id: MemberId) -> Result<Self, DynamicError> {
        let target = self.resolve(id)?;
        self.reference_to(&target)
    }

    /// Annex C `set_complex_value`: the contents of `value` become the
    /// value `id` names (the sample keeps its own storage; nothing of
    /// `value` is shared). Its type has to equal the type there.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a value of another type.
    pub fn set_complex_value(&mut self, id: MemberId, value: &Self) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        if !target.ty.equals(&value.ty) {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "a value of `{}` cannot be set where `{}` is",
                value.ty.name(),
                target.ty.name()
            )));
        }
        // Copied out first: `value` may be a handle on this very sample.
        let copy = value.snapshot()?;
        self.write(&target, copy)
    }

    // numeric reading and writing
    fn effective_kind(ty: &DynamicType) -> Option<TypeKind> {
        match ty.kind() {
            kind if kind.is_primitive() => Some(kind),
            TypeKind::Enumeration => Some(match ty.bit_bound().unwrap_or(32) {
                0..=8 => TypeKind::Int8,
                9..=16 => TypeKind::Int16,
                _ => TypeKind::Int32,
            }),
            TypeKind::Bitmask => Some(match ty.bit_bound().unwrap_or(32) {
                0..=8 => TypeKind::UInt8,
                9..=16 => TypeKind::UInt16,
                17..=32 => TypeKind::UInt32,
                _ => TypeKind::UInt64,
            }),
            _ => None,
        }
    }

    fn number(&self, target: &Target) -> Result<Num, DynamicError> {
        let value = self.read(target)?;
        match value {
            DynamicData::Boolean(b) => Ok(Num::Int(i128::from(b))),
            DynamicData::Integer { value, .. } => Ok(Num::Int(value)),
            DynamicData::Float { value, .. } => Ok(Num::Float(value)),
            DynamicData::LongDouble(value) => Ok(Num::Wide(value)),
            DynamicData::Character { value, .. } => value
                .chars()
                .next()
                .map(Num::Char)
                .ok_or_else(|| DynamicError::bad_parameter("an empty character")),
            DynamicData::Enumerator(name) => target
                .ty
                .member_by_name(&name)
                .and_then(|member| member.literal_value())
                .map(|v| Num::Int(i128::from(v)))
                .ok_or_else(|| {
                    DynamicError::bad_parameter(alloc::format!("`{name}` has no value"))
                }),
            DynamicData::Bits(flags) => {
                let mut mask: i128 = 0;
                for flag in flags {
                    let DynamicData::Text(name) = flag else {
                        continue;
                    };
                    let position = target
                        .ty
                        .member_by_name(&name)
                        .and_then(|member| member.literal_value())
                        .ok_or_else(|| {
                            DynamicError::bad_parameter(alloc::format!("`{name}` is no flag"))
                        })?;
                    mask |= 1_i128 << position;
                }
                Ok(Num::Int(mask))
            }
            _ => Err(DynamicError::bad_parameter(
                "the value is not a number, a character or a Boolean",
            )),
        }
    }

    /// Whether the value at `target` may be read as `accessor`, and the
    /// kind it has: "a smaller type promotes to a large type but not vice
    /// versa".
    fn check_get(target: &Target, accessor: TypeKind) -> Result<(), DynamicError> {
        let kind = Self::effective_kind(&target.ty)
            .ok_or_else(|| DynamicError::bad_parameter("the value is no primitive"))?;
        if kind == accessor || promotes(kind, accessor) {
            Ok(())
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "a value of {} cannot be read as {}",
                kind.as_str(),
                accessor.as_str()
            )))
        }
    }

    fn check_set(target: &Target, accessor: TypeKind) -> Result<TypeKind, DynamicError> {
        let kind = Self::effective_kind(&target.ty)
            .ok_or_else(|| DynamicError::bad_parameter("the value is no primitive"))?;
        if kind == accessor || promotes(accessor, kind) {
            Ok(kind)
        } else {
            Err(DynamicError::bad_parameter(alloc::format!(
                "a {} cannot be set where {} is",
                accessor.as_str(),
                kind.as_str()
            )))
        }
    }

    fn get_integer(&self, id: MemberId, accessor: TypeKind) -> Result<i128, DynamicError> {
        let target = self.resolve(id)?;
        Self::check_get(&target, accessor)?;
        match self.number(&target)? {
            Num::Int(v) => Ok(v),
            Num::Char(c) => Ok(i128::from(u32::from(c))),
            _ => Err(DynamicError::bad_parameter("the value is no integer")),
        }
    }

    fn set_integer(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        value: i128,
    ) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        let kind = Self::check_set(&target, accessor)?;
        let stored = self.store_integer(&target, kind, value)?;
        self.write(&target, stored)
    }

    fn store_integer(
        &self,
        target: &Target,
        kind: TypeKind,
        value: i128,
    ) -> Result<DynamicData, DynamicError> {
        match target.ty.kind() {
            TypeKind::Enumeration => target
                .ty
                .members()
                .into_iter()
                .find(|m| m.literal_value().map(i128::from) == Some(value))
                .map(|m| DynamicData::Enumerator(m.name().to_string()))
                .ok_or_else(|| {
                    DynamicError::bad_parameter(alloc::format!(
                        "{value} is no value of `{}`",
                        target.ty.name()
                    ))
                }),
            TypeKind::Bitmask => {
                let mut flags = Vec::new();
                let mut covered: i128 = 0;
                for flag in target.ty.members() {
                    let Some(position) = flag.literal_value() else {
                        continue;
                    };
                    covered |= 1_i128 << position;
                    if value & (1_i128 << position) != 0 {
                        flags.push(DynamicData::Text(flag.name().to_string()));
                    }
                }
                if value & !covered != 0 {
                    return Err(DynamicError::bad_parameter(
                        "the value sets a bit that no flag names",
                    ));
                }
                Ok(DynamicData::Bits(flags))
            }
            _ => match kind {
                TypeKind::Boolean => Ok(DynamicData::Boolean(value != 0)),
                TypeKind::Float32 => Ok(DynamicData::float(P::Float32, value as f64)),
                TypeKind::Float64 => Ok(DynamicData::float(P::Float64, value as f64)),
                TypeKind::Float128 => {
                    Ok(DynamicData::LongDouble(Float128::from_i128_rounded(value)))
                }
                TypeKind::Char8 | TypeKind::Char16 => {
                    let c = u32::try_from(value)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(|| DynamicError::bad_parameter("not a character"))?;
                    Ok(DynamicData::character(
                        if kind == TypeKind::Char8 {
                            P::Char
                        } else {
                            P::WChar
                        },
                        c,
                    ))
                }
                other => {
                    let primitive = other
                        .as_primitive()
                        .ok_or_else(|| DynamicError::bad_parameter("the value is no primitive"))?;
                    Ok(DynamicData::integer(primitive, value))
                }
            },
        }
    }

    fn get_double(&self, id: MemberId, accessor: TypeKind) -> Result<f64, DynamicError> {
        let target = self.resolve(id)?;
        Self::check_get(&target, accessor)?;
        match self.number(&target)? {
            Num::Int(v) => Ok(v as f64),
            Num::Float(v) => Ok(v),
            Num::Char(c) => Ok(f64::from(u32::from(c))),
            Num::Wide(_) => Err(DynamicError::bad_parameter("a long double is no double")),
        }
    }

    fn set_double(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        value: f64,
    ) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        let kind = Self::check_set(&target, accessor)?;
        let stored = match kind {
            TypeKind::Float32 => DynamicData::float(P::Float32, value),
            TypeKind::Float64 => DynamicData::float(P::Float64, value),
            _ => DynamicData::LongDouble(Float128::from_f64(value)),
        };
        self.write(&target, stored)
    }

    /// Annex C `get_float128_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
    pub fn get_float128_value(&self, id: MemberId) -> Result<Float128, DynamicError> {
        let target = self.resolve(id)?;
        Self::check_get(&target, TypeKind::Float128)?;
        match self.number(&target)? {
            Num::Int(v) => Ok(Float128::from_i128_rounded(v)),
            Num::Float(v) => Ok(Float128::from_f64(v)),
            Num::Wide(v) => Ok(v),
            Num::Char(c) => Ok(Float128::from_u128_rounded(u128::from(u32::from(c)))),
        }
    }

    /// Annex C `set_float128_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_float128_value(
        &mut self,
        id: MemberId,
        value: Float128,
    ) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        Self::check_set(&target, TypeKind::Float128)?;
        self.write(&target, DynamicData::LongDouble(value))
    }

    fn get_character(&self, id: MemberId, accessor: TypeKind) -> Result<char, DynamicError> {
        let target = self.resolve(id)?;
        Self::check_get(&target, accessor)?;
        match self.number(&target)? {
            Num::Char(c) => Ok(c),
            // §7.5.2.11.9: "Byte → (any)": a Byte reads as the character
            // with its code.
            Num::Int(v) if target.ty.kind() == TypeKind::Byte => u32::try_from(v)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| DynamicError::bad_parameter("not a character")),
            _ => Err(DynamicError::bad_parameter("the value is no character")),
        }
    }

    fn set_character(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        value: char,
    ) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        let kind = Self::check_set(&target, accessor)?;
        let stored = match kind {
            TypeKind::Char8 => DynamicData::character(P::Char, value),
            TypeKind::Char16 => DynamicData::character(P::WChar, value),
            _ => self.store_integer(&target, kind, i128::from(u32::from(value)))?,
        };
        self.write(&target, stored)
    }

    /// Annex C `get_boolean_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
    pub fn get_boolean_value(&self, id: MemberId) -> Result<bool, DynamicError> {
        let target = self.resolve(id)?;
        Self::check_get(&target, TypeKind::Boolean)?;
        match self.read(&target)? {
            DynamicData::Boolean(b) => Ok(b),
            // §7.5.2.11.9: "Byte → (any)", so a Byte reads as a Boolean:
            // true where it is not zero.
            DynamicData::Integer { value, .. } if target.ty.kind() == TypeKind::Byte => {
                Ok(value != 0)
            }
            _ => Err(DynamicError::bad_parameter("the value is no Boolean")),
        }
    }

    /// Annex C `set_boolean_value`; on a bitmask, `id` is the bit and the
    /// flag is set or cleared.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_boolean_value(&mut self, id: MemberId, value: bool) -> Result<(), DynamicError> {
        self.set_integer(id, TypeKind::Boolean, i128::from(value))
    }

    /// Annex C `get_string_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] where the value is no string.
    pub fn get_string_value(&self, id: MemberId) -> Result<String, DynamicError> {
        self.text(id, TypeKind::String8)
    }

    /// Annex C `get_wstring_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] where the value is no wstring.
    pub fn get_wstring_value(&self, id: MemberId) -> Result<String, DynamicError> {
        self.text(id, TypeKind::String16)
    }

    fn text(&self, id: MemberId, kind: TypeKind) -> Result<String, DynamicError> {
        let target = self.resolve(id)?;
        if target.ty.kind() != kind {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "the value is {}, not {}",
                target.ty.kind().as_str(),
                kind.as_str()
            )));
        }
        match self.read(&target)? {
            DynamicData::Text(text) => Ok(text),
            _ => Err(DynamicError::bad_parameter("the value is no string")),
        }
    }

    /// Annex C `set_string_value`; the string may also be appended to a
    /// resizable collection (§7.5.2.11.9).
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] for a value past the string's bound
    /// or a value that is no string.
    pub fn set_string_value(&mut self, id: MemberId, value: &str) -> Result<(), DynamicError> {
        self.set_text(id, TypeKind::String8, value)
    }

    /// Annex C `set_wstring_value`.
    ///
    /// # Errors
    /// As [`Self::set_string_value`].
    pub fn set_wstring_value(&mut self, id: MemberId, value: &str) -> Result<(), DynamicError> {
        self.set_text(id, TypeKind::String16, value)
    }

    fn set_text(&mut self, id: MemberId, kind: TypeKind, value: &str) -> Result<(), DynamicError> {
        let target = self.resolve(id)?;
        if target.ty.kind() != kind {
            return Err(DynamicError::bad_parameter(alloc::format!(
                "the value is {}, not {}",
                target.ty.kind().as_str(),
                kind.as_str()
            )));
        }
        if let Some(bound) = Self::bound(&target.ty) {
            if value.chars().count() > bound {
                return Err(DynamicError::bad_parameter(alloc::format!(
                    "{} characters exceed the bound of {bound}",
                    value.chars().count()
                )));
            }
        }
        self.write(&target, DynamicData::Text(value.to_string()))
    }

    // -- sequences of values ------------------------------------------------

    fn collection_target(&self, id: MemberId) -> Result<(Target, Vec<DynamicData>), DynamicError> {
        let target = self.resolve_values(id)?;
        let kind = target.ty.kind();
        if !matches!(kind, TypeKind::Sequence | TypeKind::Array) {
            return Err(DynamicError::bad_parameter(
                "the value is no sequence or array",
            ));
        }
        let value = self.read(&target)?;
        let mut flat = Vec::new();
        flatten(&value, Self::dimensions(&target.ty).len().max(1), &mut flat);
        Ok((target, flat))
    }

    fn element_target(target: &Target) -> Result<Target, DynamicError> {
        let element = target
            .ty
            .element_type()
            .ok_or_else(|| DynamicError::bad_parameter("a collection with no element"))?
            .resolve_alias()?;
        Ok(Target {
            place: Place::Whole,
            ty: element,
            optional: false,
            member: None,
        })
    }

    fn get_integers(&self, id: MemberId, accessor: TypeKind) -> Result<Vec<i128>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        Self::check_get(&element, accessor)?;
        items
            .into_iter()
            .map(|item| {
                let one = Self::with_value(element.ty.clone(), item);
                match one.number(&element)? {
                    Num::Int(v) => Ok(v),
                    Num::Char(c) => Ok(i128::from(u32::from(c))),
                    _ => Err(DynamicError::bad_parameter("an element is no integer")),
                }
            })
            .collect()
    }

    fn get_doubles(&self, id: MemberId, accessor: TypeKind) -> Result<Vec<f64>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        Self::check_get(&element, accessor)?;
        items
            .into_iter()
            .map(|item| {
                let one = Self::with_value(element.ty.clone(), item);
                match one.number(&element)? {
                    Num::Int(v) => Ok(v as f64),
                    Num::Float(v) => Ok(v),
                    Num::Char(c) => Ok(f64::from(u32::from(c))),
                    Num::Wide(_) => Err(DynamicError::bad_parameter("an element is a long double")),
                }
            })
            .collect()
    }

    fn set_values(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        values: Vec<DynamicData>,
    ) -> Result<(), DynamicError> {
        let target = self.resolve_values(id)?;
        let kind = target.ty.kind();
        let element = Self::element_target(&target)?;
        if !matches!(accessor, TypeKind::String8 | TypeKind::String16) {
            Self::check_set(&element, accessor)?;
        }
        let stored = match kind {
            TypeKind::Sequence => {
                if let Some(bound) = Self::bound(&target.ty) {
                    if values.len() > bound {
                        return Err(DynamicError::bad_parameter(alloc::format!(
                            "{} elements exceed the bound of {bound}",
                            values.len()
                        )));
                    }
                }
                DynamicData::Collection(values)
            }
            TypeKind::Array => {
                let dims = Self::dimensions(&target.ty);
                let total: usize = dims.iter().product();
                if values.len() != total {
                    return Err(DynamicError::bad_parameter(alloc::format!(
                        "an array of {total} is set from {} values",
                        values.len()
                    )));
                }
                nest(values, &dims)
            }
            _ => {
                return Err(DynamicError::bad_parameter(
                    "the value is no sequence or array",
                ));
            }
        };
        self.write(&target, stored)
    }

    fn set_integers(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        values: &[i128],
    ) -> Result<(), DynamicError> {
        let target = self.resolve_values(id)?;
        let element = Self::element_target(&target)?;
        let kind = Self::check_set(&element, accessor)?;
        let stored = values
            .iter()
            .map(|v| self.store_integer(&element, kind, *v))
            .collect::<Result<Vec<_>, _>>()?;
        self.set_values(id, accessor, stored)
    }

    fn set_doubles(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        values: &[f64],
    ) -> Result<(), DynamicError> {
        let target = self.resolve_values(id)?;
        let element = Self::element_target(&target)?;
        let kind = Self::check_set(&element, accessor)?;
        let stored = values
            .iter()
            .map(|v| match kind {
                TypeKind::Float32 => DynamicData::float(P::Float32, *v),
                TypeKind::Float64 => DynamicData::float(P::Float64, *v),
                _ => DynamicData::LongDouble(Float128::from_f64(*v)),
            })
            .collect();
        self.set_values(id, accessor, stored)
    }

    /// Annex C `get_float128_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_float128_values(&self, id: MemberId) -> Result<Vec<Float128>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        Self::check_get(&element, TypeKind::Float128)?;
        items
            .into_iter()
            .map(|item| {
                let one = Self::with_value(element.ty.clone(), item);
                match one.number(&element)? {
                    Num::Int(v) => Ok(Float128::from_i128_rounded(v)),
                    Num::Float(v) => Ok(Float128::from_f64(v)),
                    Num::Wide(v) => Ok(v),
                    Num::Char(c) => Ok(Float128::from_u128_rounded(u128::from(u32::from(c)))),
                }
            })
            .collect()
    }

    /// Annex C `set_float128_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_float128_values(
        &mut self,
        id: MemberId,
        values: &[Float128],
    ) -> Result<(), DynamicError> {
        let stored = values.iter().map(|v| DynamicData::LongDouble(*v)).collect();
        self.set_values(id, TypeKind::Float128, stored)
    }

    /// Annex C `get_boolean_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_boolean_values(&self, id: MemberId) -> Result<Vec<bool>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        Self::check_get(&element, TypeKind::Boolean)?;
        items
            .into_iter()
            .map(|item| match item {
                DynamicData::Boolean(b) => Ok(b),
                DynamicData::Integer { value, .. } if element.ty.kind() == TypeKind::Byte => {
                    Ok(value != 0)
                }
                _ => Err(DynamicError::bad_parameter("an element is no Boolean")),
            })
            .collect()
    }

    /// Annex C `set_boolean_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_boolean_values(
        &mut self,
        id: MemberId,
        values: &[bool],
    ) -> Result<(), DynamicError> {
        let numbers: Vec<i128> = values.iter().map(|v| i128::from(*v)).collect();
        self.set_integers(id, TypeKind::Boolean, &numbers)
    }

    /// Annex C `get_string_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_string_values(&self, id: MemberId) -> Result<Vec<String>, DynamicError> {
        self.texts(id, TypeKind::String8)
    }

    /// Annex C `get_wstring_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_wstring_values(&self, id: MemberId) -> Result<Vec<String>, DynamicError> {
        self.texts(id, TypeKind::String16)
    }

    fn texts(&self, id: MemberId, kind: TypeKind) -> Result<Vec<String>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        if element.ty.kind() != kind {
            return Err(DynamicError::bad_parameter(
                "the elements are not those strings",
            ));
        }
        items
            .into_iter()
            .map(|item| match item {
                DynamicData::Text(text) => Ok(text),
                _ => Err(DynamicError::bad_parameter("an element is no string")),
            })
            .collect()
    }

    /// Annex C `set_string_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_string_values(
        &mut self,
        id: MemberId,
        values: &[String],
    ) -> Result<(), DynamicError> {
        self.set_texts(id, TypeKind::String8, values)
    }

    /// Annex C `set_wstring_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_wstring_values(
        &mut self,
        id: MemberId,
        values: &[String],
    ) -> Result<(), DynamicError> {
        self.set_texts(id, TypeKind::String16, values)
    }

    fn set_texts(
        &mut self,
        id: MemberId,
        kind: TypeKind,
        values: &[String],
    ) -> Result<(), DynamicError> {
        let target = self.resolve_values(id)?;
        let element = Self::element_target(&target)?;
        if element.ty.kind() != kind {
            return Err(DynamicError::bad_parameter(
                "the elements are not those strings",
            ));
        }
        if let Some(bound) = Self::bound(&element.ty) {
            if values.iter().any(|v| v.chars().count() > bound) {
                return Err(DynamicError::bad_parameter("a string exceeds its bound"));
            }
        }
        let stored = values
            .iter()
            .map(|v| DynamicData::Text(v.clone()))
            .collect();
        self.set_values(id, kind, stored)
    }
}

/// A number read out of a value.
#[derive(Debug, Clone)]
enum Num {
    Int(i128),
    Float(f64),
    Wide(Float128),
    Char(char),
}

/// §7.5.2.11.9: "the following promotions shall be supported". A smaller
/// type promotes to a larger one and not the other way.
fn promotes(from: TypeKind, to: TypeKind) -> bool {
    use TypeKind::{
        Boolean, Byte, Char8, Char16, Float32, Float64, Float128, Int8, Int16, Int32, Int64, UInt8,
        UInt16, UInt32, UInt64,
    };
    let all = [
        Boolean, Byte, Int8, UInt8, Int16, UInt16, Int32, UInt32, Int64, UInt64, Float32, Float64,
        Float128, Char8, Char16,
    ];
    if !all.contains(&to) {
        return false;
    }
    match from {
        Int8 => matches!(to, Int16 | Int32 | Int64 | Float32 | Float64 | Float128),
        Int16 => matches!(to, Int32 | Int64 | Float32 | Float64 | Float128),
        Int32 => matches!(to, Int64 | Float64 | Float128),
        Int64 => matches!(to, Float128),
        UInt8 => matches!(
            to,
            Int16 | Int32 | Int64 | UInt16 | UInt32 | UInt64 | Float32 | Float64 | Float128
        ),
        UInt16 => matches!(
            to,
            Int32 | Int64 | UInt32 | UInt64 | Float32 | Float64 | Float128
        ),
        UInt32 => matches!(to, Int64 | UInt64 | Float64 | Float128),
        UInt64 => matches!(to, Float128),
        Float32 => matches!(to, Float64 | Float128),
        Float64 => matches!(to, Float128),
        Char8 => matches!(
            to,
            Char16 | Int16 | Int32 | Int64 | Float32 | Float64 | Float128
        ),
        Char16 => matches!(to, Int32 | Int64 | Float32 | Float64 | Float128),
        Byte => true,
        Boolean => matches!(
            to,
            Int8 | Int16
                | Int32
                | Int64
                | UInt8
                | UInt16
                | UInt32
                | UInt64
                | Float32
                | Float64
                | Float128
        ),
        _ => false,
    }
}

/// The text a map key is looked up by.
fn key_text(key: &DynamicData) -> String {
    match key {
        DynamicData::Text(text) | DynamicData::Enumerator(text) | DynamicData::Fixed(text) => {
            text.clone()
        }
        DynamicData::Integer { value, .. } => value.to_string(),
        DynamicData::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
        DynamicData::Character { value, .. } => value.clone(),
        _ => String::new(),
    }
}

/// A map key of `ty` written as `text`.
fn key_from_text(ty: &DynamicType, text: &str) -> Result<DynamicData, DynamicError> {
    let bad =
        || DynamicError::bad_parameter(alloc::format!("`{text}` is no key of `{}`", ty.name()));
    match ty.kind() {
        TypeKind::String8 | TypeKind::String16 => Ok(DynamicData::Text(text.to_string())),
        TypeKind::Enumeration => ty
            .member_by_name(text)
            .map(|m| DynamicData::Enumerator(m.name().to_string()))
            .ok_or_else(bad),
        TypeKind::Boolean => match text {
            "true" | "TRUE" => Ok(DynamicData::Boolean(true)),
            "false" | "FALSE" => Ok(DynamicData::Boolean(false)),
            _ => Err(bad()),
        },
        TypeKind::Char8 | TypeKind::Char16 => text
            .chars()
            .next()
            .map(|c| {
                DynamicData::character(
                    if ty.kind() == TypeKind::Char8 {
                        P::Char
                    } else {
                        P::WChar
                    },
                    c,
                )
            })
            .ok_or_else(bad),
        kind => {
            let primitive = kind.as_primitive().ok_or_else(bad)?;
            let value: i128 = text.parse().map_err(|_| bad())?;
            Ok(DynamicData::integer(primitive, value))
        }
    }
}

/// The discriminator value for `label` in the discriminator type.
fn discriminator_value(ty: &DynamicType, label: i64) -> Result<DynamicData, DynamicError> {
    match ty.kind() {
        TypeKind::Boolean => Ok(DynamicData::Boolean(label != 0)),
        TypeKind::Enumeration => ty
            .members()
            .into_iter()
            .find(|m| m.literal_value().map(i64::from) == Some(label))
            .map(|m| DynamicData::Enumerator(m.name().to_string()))
            .ok_or_else(|| DynamicError::bad_parameter("a label no enumerator has")),
        TypeKind::Char8 | TypeKind::Char16 => u32::try_from(label)
            .ok()
            .and_then(char::from_u32)
            .map(|c| {
                DynamicData::character(
                    if ty.kind() == TypeKind::Char8 {
                        P::Char
                    } else {
                        P::WChar
                    },
                    c,
                )
            })
            .ok_or_else(|| DynamicError::bad_parameter("a label that is no character")),
        kind => kind
            .as_primitive()
            .map(|p| DynamicData::integer(p, i128::from(label)))
            .ok_or_else(|| DynamicError::bad_parameter("a discriminator that is no integer")),
    }
}

/// The element at `index` of an array flattened, "in the order specified
/// by the IDL specification".
///
/// zerodds-lint: recursion-depth 8 (one frame per array dimension; `dims`
/// loses one at each call, and a type has the dimensions its descriptor
/// names)
fn flat_get<'a>(items: &'a [DynamicData], dims: &[usize], index: usize) -> Option<&'a DynamicData> {
    let Some((_, rest)) = dims.split_first() else {
        return items.get(index);
    };
    let stride: usize = rest.iter().product::<usize>().max(1);
    let outer = items.get(index / stride)?;
    if rest.is_empty() {
        return Some(outer);
    }
    flat_get(outer.items()?, rest, index % stride)
}

/// zerodds-lint: recursion-depth 8 (one frame per array dimension; `dims`
/// loses one at each call)
fn flat_set(
    items: &mut [DynamicData],
    dims: &[usize],
    index: usize,
    new: DynamicData,
) -> Option<()> {
    let Some((_, rest)) = dims.split_first() else {
        *items.get_mut(index)? = new;
        return Some(());
    };
    let stride: usize = rest.iter().product::<usize>().max(1);
    let outer = items.get_mut(index / stride)?;
    if rest.is_empty() {
        *outer = new;
        return Some(());
    }
    match outer {
        DynamicData::Collection(inner) => flat_set(inner, rest, index % stride, new),
        _ => None,
    }
}

/// zerodds-lint: recursion-depth 8 (one frame per array dimension; `depth`
/// loses one at each call)
fn flatten(value: &DynamicData, depth: usize, out: &mut Vec<DynamicData>) {
    match (value, depth) {
        (DynamicData::Collection(items), d) if d > 1 => {
            for item in items {
                flatten(item, d - 1, out);
            }
        }
        (DynamicData::Collection(items), _) => out.extend(items.iter().cloned()),
        _ => {}
    }
}

/// zerodds-lint: recursion-depth 8 (one frame per array dimension; `dims`
/// loses one at each call)
fn nest(values: Vec<DynamicData>, dims: &[usize]) -> DynamicData {
    let Some((_, rest)) = dims.split_first() else {
        return DynamicData::Collection(values);
    };
    if rest.is_empty() {
        return DynamicData::Collection(values);
    }
    let stride: usize = rest.iter().product::<usize>().max(1);
    let mut out = Vec::new();
    let mut iter = values.into_iter();
    loop {
        let chunk: Vec<DynamicData> = iter.by_ref().take(stride).collect();
        if chunk.is_empty() {
            break;
        }
        out.push(nest(chunk, rest));
    }
    DynamicData::Collection(out)
}

macro_rules! integer_accessors {
    ($(($get:ident, $set:ident, $gets:ident, $sets:ident, $ty:ty, $kind:expr)),* $(,)?) => {
        impl DynamicSample {
            $(
                #[doc = concat!("Annex C `", stringify!($get), "`; a smaller type promotes, a larger does not (§7.5.2.11.9).")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
                pub fn $get(&self, id: MemberId) -> Result<$ty, DynamicError> {
                    let v = self.get_integer(id, $kind)?;
                    <$ty>::try_from(v).map_err(|_| DynamicError::bad_parameter("the value does not fit"))
                }

                #[doc = concat!("Annex C `", stringify!($set), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $set(&mut self, id: MemberId, value: $ty) -> Result<(), DynamicError> {
                    self.set_integer(id, $kind, i128::from(value))
                }

                #[doc = concat!("Annex C `", stringify!($gets), "`; an array is read too.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $gets(&self, id: MemberId) -> Result<Vec<$ty>, DynamicError> {
                    self.get_integers(id, $kind)?
                        .into_iter()
                        .map(|v| <$ty>::try_from(v).map_err(|_| DynamicError::bad_parameter("a value does not fit")))
                        .collect()
                }

                #[doc = concat!("Annex C `", stringify!($sets), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $sets(&mut self, id: MemberId, values: &[$ty]) -> Result<(), DynamicError> {
                    let wide: Vec<i128> = values.iter().map(|v| i128::from(*v)).collect();
                    self.set_integers(id, $kind, &wide)
                }
            )*
        }
    };
}

integer_accessors!(
    (
        get_int8_value,
        set_int8_value,
        get_int8_values,
        set_int8_values,
        i8,
        TypeKind::Int8
    ),
    (
        get_uint8_value,
        set_uint8_value,
        get_uint8_values,
        set_uint8_values,
        u8,
        TypeKind::UInt8
    ),
    (
        get_int16_value,
        set_int16_value,
        get_int16_values,
        set_int16_values,
        i16,
        TypeKind::Int16
    ),
    (
        get_uint16_value,
        set_uint16_value,
        get_uint16_values,
        set_uint16_values,
        u16,
        TypeKind::UInt16
    ),
    (
        get_int32_value,
        set_int32_value,
        get_int32_values,
        set_int32_values,
        i32,
        TypeKind::Int32
    ),
    (
        get_uint32_value,
        set_uint32_value,
        get_uint32_values,
        set_uint32_values,
        u32,
        TypeKind::UInt32
    ),
    (
        get_int64_value,
        set_int64_value,
        get_int64_values,
        set_int64_values,
        i64,
        TypeKind::Int64
    ),
    (
        get_uint64_value,
        set_uint64_value,
        get_uint64_values,
        set_uint64_values,
        u64,
        TypeKind::UInt64
    ),
    (
        get_byte_value,
        set_byte_value,
        get_byte_values,
        set_byte_values,
        u8,
        TypeKind::Byte
    ),
);

macro_rules! float_accessors {
    ($(($get:ident, $set:ident, $gets:ident, $sets:ident, $ty:ty, $kind:expr)),* $(,)?) => {
        impl DynamicSample {
            $(
                #[doc = concat!("Annex C `", stringify!($get), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
                pub fn $get(&self, id: MemberId) -> Result<$ty, DynamicError> {
                    self.get_double(id, $kind).map(|v| v as $ty)
                }

                #[doc = concat!("Annex C `", stringify!($set), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $set(&mut self, id: MemberId, value: $ty) -> Result<(), DynamicError> {
                    self.set_double(id, $kind, f64::from(value))
                }

                #[doc = concat!("Annex C `", stringify!($gets), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $gets(&self, id: MemberId) -> Result<Vec<$ty>, DynamicError> {
                    Ok(self.get_doubles(id, $kind)?.into_iter().map(|v| v as $ty).collect())
                }

                #[doc = concat!("Annex C `", stringify!($sets), "`.")]
                ///
                /// # Errors
                /// [`DynamicError::BadParameter`].
                pub fn $sets(&mut self, id: MemberId, values: &[$ty]) -> Result<(), DynamicError> {
                    let wide: Vec<f64> = values.iter().map(|v| f64::from(*v)).collect();
                    self.set_doubles(id, $kind, &wide)
                }
            )*
        }
    };
}

float_accessors!(
    (
        get_float32_value,
        set_float32_value,
        get_float32_values,
        set_float32_values,
        f32,
        TypeKind::Float32
    ),
    (
        get_float64_value,
        set_float64_value,
        get_float64_values,
        set_float64_values,
        f64,
        TypeKind::Float64
    ),
);

impl DynamicSample {
    /// Annex C `get_char8_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
    pub fn get_char8_value(&self, id: MemberId) -> Result<char, DynamicError> {
        self.get_character(id, TypeKind::Char8)
    }

    /// Annex C `set_char8_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_char8_value(&mut self, id: MemberId, value: char) -> Result<(), DynamicError> {
        self.set_character(id, TypeKind::Char8, value)
    }

    /// Annex C `get_char16_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`], [`DynamicError::NoData`].
    pub fn get_char16_value(&self, id: MemberId) -> Result<char, DynamicError> {
        self.get_character(id, TypeKind::Char16)
    }

    /// Annex C `set_char16_value`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_char16_value(&mut self, id: MemberId, value: char) -> Result<(), DynamicError> {
        self.set_character(id, TypeKind::Char16, value)
    }

    /// Annex C `get_char8_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_char8_values(&self, id: MemberId) -> Result<Vec<char>, DynamicError> {
        self.characters(id, TypeKind::Char8)
    }

    /// Annex C `get_char16_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn get_char16_values(&self, id: MemberId) -> Result<Vec<char>, DynamicError> {
        self.characters(id, TypeKind::Char16)
    }

    fn characters(&self, id: MemberId, accessor: TypeKind) -> Result<Vec<char>, DynamicError> {
        let (target, items) = self.collection_target(id)?;
        let element = Self::element_target(&target)?;
        Self::check_get(&element, accessor)?;
        let byte = element.ty.kind() == TypeKind::Byte;
        items
            .into_iter()
            .map(|item| match item {
                // "Byte → (any)", as for the single value.
                DynamicData::Integer { value, .. } if byte => u32::try_from(value)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| DynamicError::bad_parameter("not a character")),
                other => other
                    .as_char()
                    .ok_or_else(|| DynamicError::bad_parameter("an element is no character")),
            })
            .collect()
    }

    /// The characters, stored as the kind of the elements: a character where
    /// they are characters, its code where they are numbers (the promotions
    /// of §7.5.2.11.9).
    fn set_characters(
        &mut self,
        id: MemberId,
        accessor: TypeKind,
        values: &[char],
    ) -> Result<(), DynamicError> {
        let target = self.resolve_values(id)?;
        let element = Self::element_target(&target)?;
        let kind = Self::check_set(&element, accessor)?;
        let stored = values
            .iter()
            .map(|c| match kind {
                TypeKind::Char8 => Ok(DynamicData::character(P::Char, *c)),
                TypeKind::Char16 => Ok(DynamicData::character(P::WChar, *c)),
                other => self.store_integer(&element, other, i128::from(u32::from(*c))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.set_values(id, accessor, stored)
    }

    /// Annex C `set_char8_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_char8_values(&mut self, id: MemberId, values: &[char]) -> Result<(), DynamicError> {
        self.set_characters(id, TypeKind::Char8, values)
    }

    /// Annex C `set_char16_values`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`].
    pub fn set_char16_values(&mut self, id: MemberId, values: &[char]) -> Result<(), DynamicError> {
        self.set_characters(id, TypeKind::Char16, values)
    }
}
