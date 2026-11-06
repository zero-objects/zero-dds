// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! [`DdsMember`] and [`DdsMemberRead`] for the Rust types this crate
//! knows: the primitives, `String`, `core::num::NonZero*`, `Vec<T>`,
//! `[T; N]`, `Option<T>`, the borrowed `&str`, `&[u8]`, `Cow<str>` and
//! `Cow<[u8]>`, and a [`crate::WireRepr`] type or an adapter's; and
//! [`Discriminator`] for the integers, `bool` and `char`.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZero;

use super::ser::Erased;
use super::{
    DdsMember, DdsMemberRead, Discriminator, FieldValue, Primitive, ValueDesc, ValueDeserializer,
    ValueSerializer, desc_of,
};
use crate::repr::{IdlOut, IdlPrimitive};
use crate::{Endianness, Reader, WireError, WireRepr, WireReprWith, Writer, Written};

/// The IDL type a [`WireRepr::Idl`] names, as the codec writes and reads
/// it: `str` or an [`IdlPrimitive`].
pub trait IdlType {
    /// The description of the IDL type.
    const DESC: ValueDesc;

    /// Write through an [`IdlOut`] of this type.
    ///
    /// # Errors
    /// What the plan or `write` refuses.
    fn write_repr(
        out: ValueSerializer<'_>,
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Result<(), WireError>;

    /// Read a value of this type and hand it to `read`.
    ///
    /// # Errors
    /// What the plan, the stream or `read` refuses.
    fn read_repr<'de, R>(
        input: ValueDeserializer<'_, 'de>,
        read: impl FnOnce(&Self) -> Result<R, WireError>,
    ) -> Result<R, WireError>;

    /// Hand the IDL default (XTypes 1.3 Table 9) to `read`.
    ///
    /// # Errors
    /// What `read` refuses.
    fn default_repr<R>(read: impl FnOnce(&Self) -> Result<R, WireError>) -> Result<R, WireError>;

    /// The value `write` puts out, as a content filter compares it; `None`
    /// when `write` refuses or the filter has no value for the type.
    fn field_value_of(
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Option<FieldValue>;
}

impl IdlType for str {
    const DESC: ValueDesc = ValueDesc::String { bound: None };

    fn write_repr(
        out: ValueSerializer<'_>,
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Result<(), WireError> {
        out.idl_str(write)
    }

    fn read_repr<'de, R>(
        input: ValueDeserializer<'_, 'de>,
        read: impl FnOnce(&Self) -> Result<R, WireError>,
    ) -> Result<R, WireError> {
        input.idl_str(read)
    }

    fn default_repr<R>(read: impl FnOnce(&Self) -> Result<R, WireError>) -> Result<R, WireError> {
        read("")
    }

    fn field_value_of(
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Option<FieldValue> {
        let mut writer = Writer::with_max_align(Endianness::Little, 4);
        write(IdlOut::string(&mut writer)).ok()?;
        let bytes = writer.finish();
        Reader::with_max_align(&bytes, Endianness::Little, 4)
            .read_str()
            .ok()
            .map(|text| FieldValue::Text(String::from(text)))
    }
}

impl<P: IdlPrimitive> IdlType for P {
    const DESC: ValueDesc = ValueDesc::Primitive(P::PRIMITIVE);

    fn write_repr(
        out: ValueSerializer<'_>,
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Result<(), WireError> {
        out.idl_primitive(write)
    }

    fn read_repr<'de, R>(
        input: ValueDeserializer<'_, 'de>,
        read: impl FnOnce(&Self) -> Result<R, WireError>,
    ) -> Result<R, WireError> {
        input.idl_primitive::<P, R>(read)
    }

    fn default_repr<R>(read: impl FnOnce(&Self) -> Result<R, WireError>) -> Result<R, WireError> {
        read(&P::ZERO)
    }

    fn field_value_of(
        write: impl FnOnce(IdlOut<'_, Self>) -> Result<Written, WireError>,
    ) -> Option<FieldValue> {
        let mut writer = Writer::with_max_align(Endianness::Little, 8);
        write(IdlOut::over(&mut writer, None)).ok()?;
        let bytes = writer.finish();
        P::read(&mut Reader::with_max_align(&bytes, Endianness::Little, 8))
            .ok()?
            .field_value()
    }
}

/// [`DdsMember::serialize_member`] of a [`WireRepr`] type: its `to_wire`
/// through the [`IdlOut`] of its IDL type.
///
/// # Errors
/// What the plan or the type refuses.
pub fn serialize_repr<T>(value: &T, out: ValueSerializer<'_>) -> Result<(), WireError>
where
    T: WireRepr,
    T::Idl: IdlType,
{
    <T::Idl as IdlType>::write_repr(out, |out| value.to_wire(out))
}

/// [`DdsMember::deserialize_member`] of a [`WireRepr`] type: its
/// `from_wire` over the IDL value, a `string` lent out of the sample.
///
/// # Errors
/// What the stream or the type refuses.
pub fn deserialize_repr<T>(input: ValueDeserializer<'_, '_>) -> Result<T, WireError>
where
    T: WireRepr,
    T::Idl: IdlType,
{
    let at = input.position();
    <T::Idl as IdlType>::read_repr(input, |wire| crate::repr::from_wire::<T>(wire, at))
}

/// [`DdsMember::default_member`] of a [`WireRepr`] type: its IDL default,
/// converted.
///
/// # Errors
/// A type that does not accept its IDL default.
pub fn default_repr<T>() -> Result<T, WireError>
where
    T: WireRepr,
    T::Idl: IdlType,
{
    <T::Idl as IdlType>::default_repr(|wire| crate::repr::from_default::<T>(wire))
}

/// [`DdsMember::field_value`] of a [`WireRepr`] type: the IDL value its
/// `to_wire` writes. A dotted path past it reaches nothing.
#[must_use]
pub fn repr_field_value<T>(value: &T, path: Option<&str>) -> Option<FieldValue>
where
    T: WireRepr,
    T::Idl: IdlType,
{
    if path.is_some() {
        return None;
    }
    <T::Idl as IdlType>::field_value_of(|out| value.to_wire(out))
}

/// [`DdsMember`] for a [`WireRepr`] type, as `derive(WireRepr)` writes it:
/// `dds_member_via_wire_repr!(Dtg, CallSign);`.
#[macro_export]
macro_rules! dds_member_via_wire_repr {
    ($($ty:ty),* $(,)?) => {$(
        impl $crate::codec::DdsMember for $ty {
            const DESC: $crate::codec::ValueDesc =
                <<$ty as $crate::WireRepr>::Idl as $crate::codec::IdlType>::DESC;

            fn serialize_member(
                &self,
                out: $crate::codec::ValueSerializer<'_>,
            ) -> ::core::result::Result<(), $crate::WireError> {
                $crate::codec::serialize_repr(self, out)
            }

            fn default_member() -> ::core::result::Result<Self, $crate::WireError> {
                $crate::codec::default_repr()
            }

            fn field_value(
                &self,
                path: ::core::option::Option<&str>,
            ) -> ::core::option::Option<$crate::codec::FieldValue> {
                $crate::codec::repr_field_value(self, path)
            }
        }

        impl<'de> $crate::codec::DdsMemberRead<'de> for $ty {
            fn deserialize_member(
                input: $crate::codec::ValueDeserializer<'_, 'de>,
            ) -> ::core::result::Result<Self, $crate::WireError> {
                $crate::codec::deserialize_repr(input)
            }
        }
    )*};
}

dds_member_via_wire_repr!(
    bool, u8, i8, i16, u16, i32, u32, i64, u64, f32, f64, char, String
);
dds_member_via_wire_repr!(
    NonZero<u8>,
    NonZero<i8>,
    NonZero<u16>,
    NonZero<i16>,
    NonZero<u32>,
    NonZero<i32>,
    NonZero<u64>,
    NonZero<i64>,
);

macro_rules! discriminators {
    ($($ty:ty),*) => {$(
        impl Discriminator for $ty {
            fn label(&self) -> i128 {
                i128::from(*self)
            }
            fn from_label(label: i128) -> Option<Self> {
                Self::try_from(label).ok()
            }
        }
    )*};
}

discriminators!(i8, u8, i16, u16, i32, u32, i64, u64);

impl Discriminator for bool {
    fn label(&self) -> i128 {
        i128::from(*self)
    }
    fn from_label(label: i128) -> Option<Self> {
        match label {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

impl Discriminator for char {
    fn label(&self) -> i128 {
        i128::from(u32::from(*self))
    }
    fn from_label(label: i128) -> Option<Self> {
        u32::try_from(label).ok().and_then(char::from_u32)
    }
}

/// `string`, lent out of the received sample.
impl DdsMember for &str {
    const DESC: ValueDesc = ValueDesc::String { bound: None };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.string(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok("")
    }

    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        path.is_none()
            .then(|| FieldValue::Text(String::from(*self)))
    }
}

impl<'de: 'a, 'a> DdsMemberRead<'de> for &'a str {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.borrowed_str()
    }
}

/// `sequence<octet>`, lent out of the received sample.
impl DdsMember for &[u8] {
    const DESC: ValueDesc = ValueDesc::Sequence {
        element: desc_of::<u8>,
        bound: None,
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.octets(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(&[])
    }
}

impl<'de: 'a, 'a> DdsMemberRead<'de> for &'a [u8] {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.borrowed_octets()
    }
}

/// `string`: borrowed from the received sample when read, either form
/// when written.
impl<'a> DdsMember for Cow<'a, str> {
    const DESC: ValueDesc = ValueDesc::String { bound: None };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.string(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(Cow::Borrowed(""))
    }

    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        path.is_none()
            .then(|| FieldValue::Text(String::from(self.as_ref())))
    }
}

impl<'de: 'a, 'a> DdsMemberRead<'de> for Cow<'a, str> {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.borrowed_str().map(Cow::Borrowed)
    }
}

/// `sequence<octet>`: borrowed from the received sample when read.
impl<'a> DdsMember for Cow<'a, [u8]> {
    const DESC: ValueDesc = ValueDesc::Sequence {
        element: desc_of::<u8>,
        bound: None,
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.octets(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(Cow::Borrowed(&[]))
    }
}

impl<'de: 'a, 'a> DdsMemberRead<'de> for Cow<'a, [u8]> {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.borrowed_octets().map(Cow::Borrowed)
    }
}

/// `sequence<T>`.
impl<T: DdsMember> DdsMember for Vec<T> {
    const DESC: ValueDesc = ValueDesc::Sequence {
        element: desc_of::<T>,
        bound: None,
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.sequence(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(Self::new())
    }
}

impl<'de, T: DdsMemberRead<'de>> DdsMemberRead<'de> for Vec<T> {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.sequence()
    }
}

/// An array dimension as IDL declares it, a `u32` (IDL 4.2
/// §7.4.1.4.4.6, a positive integer constant): a longer Rust array does
/// not compile as a member.
const fn array_length(n: usize) -> u32 {
    assert!(
        n <= u32::MAX as usize,
        "an array member longer than 2^32 - 1 elements, which no IDL array is"
    );
    n as u32
}

/// One array dimension; `[[T; 3]; 2]` is `T x[2][3]`.
impl<T: DdsMember, const N: usize> DdsMember for [T; N] {
    const DESC: ValueDesc = ValueDesc::Array {
        element: desc_of::<T>,
        len: array_length(N),
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.array(self)
    }

    fn default_member() -> Result<Self, WireError> {
        let mut values = Vec::with_capacity(N);
        for _ in 0..N {
            values.push(T::default_member()?);
        }
        values
            .try_into()
            .map_err(|_| WireError::refused(0, "an array of its declared length"))
    }
}

impl<'de, T: DdsMemberRead<'de>, const N: usize> DdsMemberRead<'de> for [T; N] {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.array::<T, N>()
    }
}

/// `@optional T`.
impl<T: DdsMember> DdsMember for Option<T> {
    const DESC: ValueDesc = ValueDesc::Optional {
        inner: desc_of::<T>,
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        match self {
            Some(value) => value.serialize_member(out),
            None => Err(WireError::refused(
                0,
                "a present value; the serializer frames an absent one",
            )),
        }
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(None)
    }

    fn is_present(&self) -> bool {
        self.is_some()
    }

    fn absent() -> Result<Self, WireError> {
        Ok(None)
    }

    /// XTypes 1.3 §7.6.7.2: an absent member is `null` where the present
    /// one has a value for the filter, a scalar or a string.
    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        match self {
            Some(value) => value.field_value(path),
            None => match (path, T::DESC) {
                (None, ValueDesc::Primitive(Primitive::UInt64)) => None,
                (None, ValueDesc::Primitive(_) | ValueDesc::String { .. }) => {
                    Some(FieldValue::Null)
                }
                _ => None,
            },
        }
    }
}

impl<'de, T: DdsMemberRead<'de>> DdsMemberRead<'de> for Option<T> {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        T::deserialize_member(input).map(Some)
    }
}

/// A member of a type from another crate, through the adapter `A`
/// (`#[dds(with = "...")]`): the [`WireReprWith`] counterpart of a
/// [`WireRepr`] member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct With<A, T> {
    value: T,
    adapter: PhantomData<fn() -> A>,
}

impl<A, T> With<A, T> {
    /// The value, to be written through `A`.
    pub const fn new(value: T) -> Self {
        Self {
            value,
            adapter: PhantomData,
        }
    }

    /// The value.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<A, T> DdsMember for With<A, T>
where
    A: WireReprWith<T> + Default,
    A::Idl: IdlType,
{
    const DESC: ValueDesc = <A::Idl as IdlType>::DESC;

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        <A::Idl as IdlType>::write_repr(out, |out| A::default().to_wire(&self.value, out))
    }

    fn default_member() -> Result<Self, WireError> {
        <A::Idl as IdlType>::default_repr(|wire| {
            crate::repr::from_default_with(&A::default(), wire).map(Self::new)
        })
    }

    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        if path.is_some() {
            return None;
        }
        <A::Idl as IdlType>::field_value_of(|out| A::default().to_wire(&self.value, out))
    }
}

impl<'de, A, T> DdsMemberRead<'de> for With<A, T>
where
    A: WireReprWith<T> + Default,
    A::Idl: IdlType,
{
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        let at = input.position();
        <A::Idl as IdlType>::read_repr(input, |wire| {
            crate::repr::from_wire_with(&A::default(), wire, at).map(Self::new)
        })
    }
}

/// [`DdsMember::field_value`] of a member written through the adapter `A`
/// (`#[dds(with = "...")]`): the IDL value the adapter writes.
#[must_use]
pub fn with_field_value<A, T>(value: &T, path: Option<&str>) -> Option<FieldValue>
where
    A: WireReprWith<T> + Default,
    A::Idl: IdlType,
{
    if path.is_some() {
        return None;
    }
    <A::Idl as IdlType>::field_value_of(|out| A::default().to_wire(value, out))
}

/// A borrowed member written through an adapter: what
/// [`super::StructSerializer::member_with`] hands the serializer.
pub(crate) struct WithRef<'v, A, T> {
    value: &'v T,
    adapter: PhantomData<fn() -> A>,
}

impl<'v, A, T> WithRef<'v, A, T> {
    pub(crate) const fn new(value: &'v T) -> Self {
        Self {
            value,
            adapter: PhantomData,
        }
    }
}

impl<A, T> Erased for WithRef<'_, A, T>
where
    A: WireReprWith<T> + Default,
    A::Idl: IdlType,
{
    fn erased_serialize(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        <A::Idl as IdlType>::write_repr(out, |out| A::default().to_wire(self.value, out))
    }

    fn erased_present(&self) -> bool {
        true
    }
}
