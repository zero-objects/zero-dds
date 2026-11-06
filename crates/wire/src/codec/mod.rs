// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The codec API for a type defined in Rust: [`DdsSerialize`],
//! [`DdsDeserialize`], and the description they are checked against,
//! [`DdsTypeInfo`].
//!
//! A type implements all three — `derive(DdsType)` of `zerodds-derive`
//! writes them, a hand-written impl writes the first two by hand and the
//! description beside them. The description is the IDL declaration the
//! type stands for ([`TypeDesc`]) and, as a type, the list of its members'
//! Rust types ([`DdsTypeInfo::Members`], `(u32, (String, ()))`). The
//! serializer is typed by that list:
//!
//! ```
//! use zerodds_wire::codec::{
//!     DdsSerialize, DdsSerializer, DdsTypeInfo, Final, MemberDesc, Serialized, TypeDesc,
//! };
//! use zerodds_wire::WireError;
//!
//! struct Pose {
//!     id: u32,
//!     label: String,
//! }
//!
//! impl DdsTypeInfo for Pose {
//!     type Members = (u32, (String, ()));
//!     type Extensibility = Final;
//!     const LAYOUT: TypeDesc = TypeDesc::structure::<Self::Members, Final>(
//!         "app::Pose",
//!         &[MemberDesc::new("id").key(), MemberDesc::new("label").bound(16)],
//!     );
//! }
//!
//! impl DdsSerialize for Pose {
//!     fn serialize(&self, out: DdsSerializer<'_>) -> Result<Serialized, WireError> {
//!         out.serialize_struct::<Self>()?
//!             .member(&self.id)?
//!             .member(&self.label)?
//!             .end()
//!     }
//! }
//! ```
//!
//! `member` takes a value of the head of the remaining list and `end`
//! exists only once the list is empty, so a member left out, one too many
//! or one of another type does not compile, and only [`StructSerializer::end`]
//! makes the [`Serialized`] the impl has to return. What is checked at run
//! time is what depends on the value: a bound, a `NonZero` that arrives as
//! zero, a text the type does not parse.
//!
//! Decoding mirrors it: [`DdsDeserializer::deserialize_struct`] for a
//! `@final` or `@appendable` type hands the members out in order, and
//! [`DdsDeserializer::deserialize_mutable`] for a `@mutable` one fills one
//! typed slot per member id from the headers the stream carries.
//!
//! A union ([`DdsUnionInfo`]) is typed the same way: `Members` lists the
//! branches' Rust types, and [`UnionSerializer::branch`] takes the branch
//! as a type-level index ([`B0`], [`B1`], …) and a value of exactly that
//! branch's type; the discriminator is checked against the branch's labels
//! at run time, because it is a value.
//!
//! A member read with [`DdsMemberRead<'de>`] may borrow from the received
//! bytes: `&'de str`, `&'de [u8]`, `Cow<'de, str>` and `Cow<'de, [u8]>` are
//! lent out of the sample instead of copied.
//!
//! The framing — alignment (XTypes 1.3 §7.4.1), DHEADER and EMHEADER with
//! member id and must-understand flag, `@optional` presence, a sequence's
//! DHEADER, the bounds, the key holder (§7.6.8) — is the layout plan's
//! ([`PlanSet`]), which `zerodds-codec` builds from the description with
//! the planner `zerodds-build`'s generated code is built with. One plan per
//! encapsulation: XCDR1 or XCDR2, each in either byte order.

mod de;
pub mod desc;
mod key;
mod members;
pub mod plan;
mod ser;
mod time;

pub use de::{
    DdsDeserializer, Decoded, Finish, MutableSlots, StructDeserializer, UnionDeserializer,
    ValueDeserializer,
};
pub use desc::{
    AutoId, CaseDesc, EnumeratorDesc, Extensibility, HashId, MemberDesc, Primitive, TypeArg,
    TypeArgs, TypeDesc, TypeKind, ValueDesc,
};
pub use key::key_holder_from_body;
pub use members::{
    IdlType, With, default_repr, deserialize_repr, repr_field_value, serialize_repr,
    with_field_value,
};
pub use plan::{
    BranchPlan, Header, HolderPlan, KeyPlan, MemberFrame, PlanSet, Root, StructPlan, UnionPlan,
    ValuePlan,
};
pub use ser::{DdsSerializer, Serialized, StructSerializer, UnionSerializer, ValueSerializer};

use core::marker::PhantomData;

use crate::WireError;

/// One member position's Rust type and the IDL type it stands for.
///
/// Implemented here for the primitives, `String`, `core::num::NonZero*`,
/// `Vec<T>`, `[T; N]`, `Option<T>` and `core::time::Duration` (and, with
/// the `std` feature, `std::time::SystemTime`); `derive(WireRepr)` and
/// `derive(DdsType)` of `zerodds-derive` implement it for theirs.
pub trait DdsMember: Sized {
    /// The IDL type this Rust type stands for.
    const DESC: ValueDesc;

    /// Write the value at its position.
    ///
    /// # Errors
    /// What the value or the writer refuses.
    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError>;

    /// The value a member takes that the sample does not carry: the IDL
    /// default of the type (XTypes 1.3 Table 9), as this Rust type holds
    /// it.
    ///
    /// # Errors
    /// A Rust type that does not accept its IDL default — a `NonZeroU32`
    /// has no zero — has none, and a sample that leaves the member out is
    /// refused.
    fn default_member() -> Result<Self, WireError>;

    /// Whether an `@optional` member is there. `true` for every type but
    /// `Option`.
    fn is_present(&self) -> bool {
        true
    }

    /// The value of an `@optional` member the stream marks absent.
    ///
    /// # Errors
    /// Every type but `Option`: an absence it cannot hold.
    fn absent() -> Result<Self, WireError> {
        Err(WireError::refused(
            0,
            "a value: the member is not @optional and the stream marks it absent",
        ))
    }

    /// The value a content filter compares for this member (DDS 1.4
    /// §B.2.1): `path` is what follows the member's name in a dotted
    /// field reference (`pose.x` reaches `x` of the member `pose`).
    ///
    /// `None` where the filter has no value for the type — a collection,
    /// an enumeration, an `unsigned long long` — so the filter denies the
    /// sample rather than compare a substitute. A [`crate::WireRepr`]
    /// type answers with the IDL value it writes, as a peer receives it.
    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        let _ = path;
        None
    }
}

/// A member's Rust type read from a sample whose bytes live for `'de`.
///
/// Every owned type reads for any `'de`; a borrowed one (`&'de str`,
/// `&'de [u8]`, `Cow<'de, str>`, `Cow<'de, [u8]>`, a derived type with a
/// lifetime) only from bytes that outlive it, and lends from them.
pub trait DdsMemberRead<'de>: DdsMember {
    /// Read the value at its position.
    ///
    /// # Errors
    /// What the stream or the type refuses.
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError>;
}

/// A union's discriminator type: an integer, `bool`, `char` or a field-less
/// enumeration (IDL 4.2 §7.4.1.4.4.4), with the integer value a case label
/// of it stands for.
pub trait Discriminator: DdsMember {
    /// The label value: the integer, `0`/`1` for `bool`, the code point for
    /// `char`, the enumerator's wire value for an enumeration.
    fn label(&self) -> i128;

    /// The value a label stands for, `None` for one this type does not hold.
    fn from_label(label: i128) -> Option<Self>;
}

/// The first branch of a union, as a type-level index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Here;

/// The branch after `I`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct There<I>(PhantomData<I>);

/// Branch 0.
pub type B0 = Here;
/// Branch 1.
pub type B1 = There<B0>;
/// Branch 2.
pub type B2 = There<B1>;
/// Branch 3.
pub type B3 = There<B2>;
/// Branch 4.
pub type B4 = There<B3>;
/// Branch 5.
pub type B5 = There<B4>;
/// Branch 6.
pub type B6 = There<B5>;
/// Branch 7.
pub type B7 = There<B6>;

/// The member at the type-level index `I` of a member list.
pub trait Select<I>: MemberList {
    /// Its Rust type.
    type Item: DdsMember;
    /// Its position.
    const INDEX: usize;
}

impl<H: DdsMember, T: MemberList> Select<Here> for (H, T) {
    type Item = H;
    const INDEX: usize = 0;
}

impl<H: DdsMember, T: Select<I>, I> Select<There<I>> for (H, T) {
    type Item = T::Item;
    const INDEX: usize = 1 + T::INDEX;
}

/// A member value as a content filter compares it (DDS 1.4 Annex B, and
/// XTypes 1.3 §7.6.7.2 for an absent `@optional` member).
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// `boolean`.
    Bool(bool),
    /// An integer up to `long long`.
    Int(i64),
    /// `float` or `double`.
    Float(f64),
    /// A `string` or a `char`.
    Text(alloc::string::String),
    /// An absent `@optional` member.
    Null,
    /// A member of a union whose discriminator selects another member
    /// (DDS-XTypes 1.4 Beta 1 §7.6.7.1): no value, and the filter that
    /// names it is false.
    Unselected,
}

/// [`DdsMember::DESC`] as a function, for a description that names a type
/// it reaches.
#[must_use]
pub fn desc_of<T: DdsMember>() -> ValueDesc {
    T::DESC
}

/// The members of a described type, as a list of their Rust types:
/// `(u32, (String, ()))`.
pub trait MemberList {
    /// How many members.
    const LEN: usize;
    /// One slot per member, filled as a `@mutable` stream names them.
    type Slots;
    /// One value per member.
    type Values;

    /// The value description of the member at `index`.
    fn value_desc(index: usize) -> Option<ValueDesc>;

    /// No member read yet.
    #[doc(hidden)]
    fn empty_slots() -> Self::Slots;

    /// The values, a member the stream did not name taking its default.
    ///
    /// # Errors
    /// A member whose default its Rust type refuses.
    #[doc(hidden)]
    fn into_values(slots: Self::Slots) -> Result<Self::Values, WireError>;
}

impl MemberList for () {
    const LEN: usize = 0;
    type Slots = ();
    type Values = ();

    fn value_desc(_index: usize) -> Option<ValueDesc> {
        None
    }

    fn empty_slots() -> Self::Slots {}

    fn into_values(_slots: Self::Slots) -> Result<Self::Values, WireError> {
        Ok(())
    }
}

/// A member list read from bytes that live for `'de`.
pub trait MemberListRead<'de>: MemberList {
    /// Read the member at `index` into its slot.
    ///
    /// # Errors
    /// A second header for one member, and what the value refuses.
    #[doc(hidden)]
    fn read_slot(
        slots: &mut Self::Slots,
        index: usize,
        input: ValueDeserializer<'_, 'de>,
    ) -> Result<(), WireError>;

    /// Read the member at `index` behind the header its frame gives it
    /// into its slot: a key holder's member, in the holder's order.
    ///
    /// # Errors
    /// A member read twice, and what the frame or the value refuses.
    #[doc(hidden)]
    fn read_frame_slot(
        slots: &mut Self::Slots,
        index: usize,
        reader: &mut crate::Reader<'de>,
        plans: &PlanSet,
        frame: &MemberFrame,
    ) -> Result<(), WireError>;
}

impl MemberListRead<'_> for () {
    fn read_slot(
        _slots: &mut Self::Slots,
        _index: usize,
        _input: ValueDeserializer<'_, '_>,
    ) -> Result<(), WireError> {
        Err(WireError::refused(0, "a member the type declares"))
    }

    fn read_frame_slot(
        _slots: &mut Self::Slots,
        _index: usize,
        _reader: &mut crate::Reader<'_>,
        _plans: &PlanSet,
        _frame: &MemberFrame,
    ) -> Result<(), WireError> {
        Err(WireError::refused(0, "a member the type declares"))
    }
}

impl<'de, H: DdsMemberRead<'de>, T: MemberListRead<'de>> MemberListRead<'de> for (H, T) {
    fn read_slot(
        slots: &mut Self::Slots,
        index: usize,
        input: ValueDeserializer<'_, 'de>,
    ) -> Result<(), WireError> {
        if index > 0 {
            return T::read_slot(&mut slots.1, index - 1, input);
        }
        if slots.0.is_some() {
            return Err(WireError::refused(
                input.position(),
                "one header per member, as XTypes 1.3 §7.4.3.4.2 gives it",
            ));
        }
        slots.0 = Some(H::deserialize_member(input)?);
        Ok(())
    }

    fn read_frame_slot(
        slots: &mut Self::Slots,
        index: usize,
        reader: &mut crate::Reader<'de>,
        plans: &PlanSet,
        frame: &MemberFrame,
    ) -> Result<(), WireError> {
        if index > 0 {
            return T::read_frame_slot(&mut slots.1, index - 1, reader, plans, frame);
        }
        if slots.0.is_some() {
            return Err(WireError::refused(
                reader.position(),
                "one value per member in a key holder",
            ));
        }
        slots.0 = Some(de::read_frame::<H>(reader, plans, frame)?);
        Ok(())
    }
}

impl<H: DdsMember, T: MemberList> MemberList for (H, T) {
    const LEN: usize = 1 + T::LEN;
    type Slots = (Option<H>, T::Slots);
    type Values = (H, T::Values);

    fn value_desc(index: usize) -> Option<ValueDesc> {
        if index == 0 {
            Some(H::DESC)
        } else {
            T::value_desc(index - 1)
        }
    }

    fn empty_slots() -> Self::Slots {
        (None, T::empty_slots())
    }

    fn into_values(slots: Self::Slots) -> Result<Self::Values, WireError> {
        let head = match slots.0 {
            Some(value) => value,
            None => H::default_member()?,
        };
        Ok((head, T::into_values(slots.1)?))
    }
}

/// An extensibility kind as a type, so that the decoder a type takes is
/// chosen at compile time.
pub trait ExtensibilityKind {
    /// The kind.
    const KIND: Extensibility;
}

/// `@final`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Final;

/// `@appendable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appendable;

/// `@mutable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mutable;

impl ExtensibilityKind for Final {
    const KIND: Extensibility = Extensibility::Final;
}

impl ExtensibilityKind for Appendable {
    const KIND: Extensibility = Extensibility::Appendable;
}

impl ExtensibilityKind for Mutable {
    const KIND: Extensibility = Extensibility::Mutable;
}

/// The kinds whose members are read in declaration order.
pub trait InOrder: ExtensibilityKind {}

impl InOrder for Final {}

impl InOrder for Appendable {}

/// A type's IDL shape: its declaration and its members' Rust types.
///
/// For a union, `Members` lists the branches' Rust types in declaration
/// order and the type also implements [`DdsUnionInfo`].
pub trait DdsTypeInfo {
    /// The members' Rust types in declaration order, `(u32, (String, ()))`.
    type Members: MemberList;
    /// The extensibility, as a type.
    type Extensibility: ExtensibilityKind;
    /// The declaration (`TypeDesc::structure::<Self::Members, Self::Extensibility>`).
    const LAYOUT: TypeDesc;
    /// [`Self::LAYOUT`] with a static address.
    const LAYOUT_REF: &'static TypeDesc = &Self::LAYOUT;
    /// Holds [`Self::LAYOUT`] to [`Self::Members`] and
    /// [`Self::Extensibility`]: evaluated when the type is serialized or
    /// deserialized, so a description that names another number of
    /// members or another extensibility does not compile.
    #[doc(hidden)]
    const CHECK: () = check_layout(
        &Self::LAYOUT,
        <Self::Members as MemberList>::LEN,
        <Self::Extensibility as ExtensibilityKind>::KIND,
    );
}

/// [`DdsTypeInfo::CHECK`].
#[doc(hidden)]
pub const fn check_layout(desc: &TypeDesc, len: usize, kind: Extensibility) {
    // An enumeration has no members and no extensibility: it fails both.
    let (extensibility, members, count) = match desc.kind {
        TypeKind::Struct {
            extensibility,
            members,
            count,
            ..
        } => (Some(extensibility), members.len(), count),
        TypeKind::Union {
            extensibility,
            cases,
            count,
            ..
        } => (Some(extensibility), cases.len(), count),
        TypeKind::Enum { .. } => (None, usize::MAX, usize::MAX),
    };
    assert!(
        extensibility.is_some(),
        "DdsTypeInfo describes a struct or a union; an enumeration is a DdsMember"
    );
    assert!(
        members == len && count == len,
        "DdsTypeInfo::LAYOUT names another number of members than DdsTypeInfo::Members"
    );
    assert!(
        matches!(extensibility, Some(declared) if declared as u8 == kind as u8),
        "DdsTypeInfo::LAYOUT names another extensibility than DdsTypeInfo::Extensibility"
    );
}

/// A union's discriminator type, beside its [`DdsTypeInfo`].
pub trait DdsUnionInfo: DdsTypeInfo {
    /// The discriminator's Rust type.
    type Discriminator: Discriminator;
    /// Holds [`DdsTypeInfo::LAYOUT`] to being a union: evaluated where the
    /// type is serialized or deserialized as one.
    #[doc(hidden)]
    const IS_UNION: () = assert!(
        matches!(Self::LAYOUT.kind, TypeKind::Union { .. }),
        "DdsUnionInfo is for a type whose LAYOUT is a union"
    );
}

/// Write a value through the serializer its layout plan drives.
pub trait DdsSerialize: DdsTypeInfo {
    /// Hand the members to [`DdsSerializer::serialize_struct`] in
    /// declaration order and return what [`StructSerializer::end`] gives.
    ///
    /// # Errors
    /// What a member's value or the writer refuses.
    fn serialize(&self, out: DdsSerializer<'_>) -> Result<Serialized, WireError>;
}

/// Read a value through the deserializer its layout plan drives.
///
/// `'de` is the lifetime of the received bytes: a `string` member reaches
/// a `zerodds_wire::WireRepr` type as a `&'de str` lent out of them.
pub trait DdsDeserialize<'de>: DdsTypeInfo + Sized {
    /// Take the members from [`DdsDeserializer::deserialize_struct`] or
    /// [`DdsDeserializer::deserialize_mutable`] and return the value
    /// through [`Finish::with`].
    ///
    /// # Errors
    /// What the stream or a member's type refuses.
    fn deserialize(input: DdsDeserializer<'_, 'de>) -> Result<Decoded<Self>, WireError>;
}

/// The value of a described struct a sample does not carry: every member
/// at its default.
///
/// # Errors
/// A member whose Rust type refuses its IDL default.
pub fn default_struct<T: DdsDeserialize<'static>>() -> Result<T, WireError> {
    T::deserialize(DdsDeserializer::defaults()).map(Decoded::into_inner)
}
