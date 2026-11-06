// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A Rust type that stands for an IDL primitive or `string`.
//!
//! An IDL `typedef string UtcTimestamp;` is a `String` in the generated
//! Rust, and a value that is no timestamp is a `String` too. Rust code
//! that wants a type which cannot hold such a value names one in its
//! `build.rs` (`zerodds_build::Config::rust_type`), and the generated
//! struct holds that type instead. What reaches the wire does not change:
//! the IDL stays the one definition of the octets, the `TypeObject`, the
//! `TypeIdentifier`, assignability and the key hash, and a C or C++ peer
//! built from the same IDL reads the same samples.
//!
//! [`WireRepr`] is the conversion. Its `Idl` names the IDL type the Rust
//! type stands for — `str` for a `string`, `u32` for an `unsigned long`
//! — and the two directions are typed by it:
//!
//! - **Writing**, [`WireRepr::to_wire`] gets an [`IdlOut`] for exactly
//!   that IDL type and hands back the [`Written`] that only an `IdlOut`
//!   makes. An impl therefore writes one value of the member's IDL type,
//!   and the compiler checks it. A `string` is written from a borrowed
//!   `&str` ([`IdlOut::str`]) or formatted straight into the sample's
//!   buffer ([`IdlOut::display`]), with no `String` in between.
//! - **Reading**, [`WireRepr::from_wire`] gets `&Self::Idl`: for a
//!   `string` a `&str` lent out of the received sample
//!   ([`crate::Reader::read_str`]), nothing allocated on the way. A value
//!   the type does not accept is refused ([`WireError::invalid_value`]),
//!   and the generated decoder rejects the sample with that reason
//!   instead of handing out an invalid value.
//!
//! A type the orphan rule keeps a crate from implementing [`WireRepr`]
//! for goes through an adapter instead: [`WireReprWith`], with
//! [`DisplayFromStr`] for any `Display + FromStr` type
//! (`zerodds_build::Config::rust_type_with`).
//!
//! This crate carries the impls a user cannot write: the IDL primitives'
//! own Rust types, `core::num::NonZero*` over the integers, `String` and
//! `Cow<'_, str>`.
//!
//! The functions at the end of this module are what the generated code
//! calls; a hand-written codec calls the trait.

use crate::{Endianness, Reader, WireError, Writer};
use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;
use core::fmt::Display;
use core::marker::PhantomData;
use core::num::NonZero;
use core::str::FromStr;

/// Proof that an [`IdlOut`] wrote its one value.
///
/// Only an `IdlOut` makes one, so a [`WireRepr::to_wire`] that returns
/// `Ok` has written exactly one value of its IDL type: none would leave
/// it without a `Written` to return, and an `IdlOut` is consumed by its
/// write.
#[derive(Debug)]
pub struct Written {
    _private: (),
}

impl Written {
    const fn new() -> Self {
        Self { _private: () }
    }
}

/// Where one value of the IDL type `Idl` goes: the sample's writer and
/// what the member's declaration says about the value.
///
/// `IdlOut<'_, str>` writes a `string` ([`Self::str`], [`Self::display`]),
/// `IdlOut<'_, u32>` an `unsigned long` ([`Self::value`]), one constructor
/// per IDL type. Each write consumes the `IdlOut`.
pub struct IdlOut<'w, Idl: ?Sized> {
    writer: &'w mut Writer,
    /// The `string<N>` bound; `None` for every other type.
    bound: Option<usize>,
    idl: PhantomData<fn(&Idl)>,
}

impl<Idl: ?Sized> core::fmt::Debug for IdlOut<'_, Idl> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdlOut")
            .field("at", &self.writer.position())
            .field("bound", &self.bound)
            .finish()
    }
}

impl<'w, Idl: ?Sized> IdlOut<'w, Idl> {
    pub(crate) const fn over(writer: &'w mut Writer, bound: Option<usize>) -> Self {
        Self {
            writer,
            bound,
            idl: PhantomData,
        }
    }
}

impl<'w> IdlOut<'w, str> {
    /// An IDL `string`, unbounded.
    pub const fn string(writer: &'w mut Writer) -> Self {
        Self::over(writer, None)
    }

    /// An IDL `string<bound>`: at most `bound` octets of text.
    pub const fn bounded_string(writer: &'w mut Writer, bound: usize) -> Self {
        Self::over(writer, Some(bound))
    }

    /// The `string`, from a borrowed `&str`.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a text over the bound, and for one
    /// with a NUL inside, which every C peer stops at. Nothing is written
    /// then.
    pub fn str(self, value: &str) -> Result<Written, WireError> {
        if let Some(offset) = value.bytes().position(|byte| byte == 0) {
            return Err(WireError::OutOfRange {
                at: self.writer.position() + offset,
                expected: String::from("no NUL inside a string a C peer would stop at"),
            });
        }
        match self.bound {
            Some(bound) => self.writer.write_string_bounded(value, bound)?,
            None => self.writer.write_string(value),
        }
        Ok(Written::new())
    }

    /// The `string`, formatted by `value`'s `Display` straight into the
    /// sample's buffer; its length is patched in afterwards.
    ///
    /// No `String` is built on the way: the bound and the NUL check run
    /// on the text as the formatter left it in the buffer.
    ///
    /// # Errors
    /// As [`Self::str`], and [`WireError::OutOfRange`] for a `Display`
    /// impl that reports an error. The buffer is left as it was.
    pub fn display<T: Display + ?Sized>(self, value: &T) -> Result<Written, WireError> {
        self.writer.write_string_display(value, self.bound)?;
        Ok(Written::new())
    }
}

mod sealed {
    pub trait Sealed {}
}

/// The Rust type of an IDL primitive, as [`WireRepr::Idl`] names it.
///
/// Sealed: what the wire carries for each is fixed by XTypes 1.3 §7.4.
/// `bool` is `boolean`, `u8` `octet` and `uint8`, `i8` `int8`, `i16`
/// `short`, `u16` `unsigned short`, `i32` `long`, `u32` `unsigned long`,
/// `i64` `long long`, `u64` `unsigned long long`, `f32` `float`, `f64`
/// `double`, `char` `char`.
pub trait IdlPrimitive: sealed::Sealed + Copy {
    /// The IDL primitive, as the codec's descriptions name it (`u8` is
    /// `octet`).
    const PRIMITIVE: crate::codec::Primitive;

    /// The IDL default, XTypes 1.3 Table 9: zero, `false`, NUL.
    const ZERO: Self;

    /// Whether a value of this Rust type is what `kind` carries (`u8` is
    /// both `octet` and `uint8`).
    #[must_use]
    fn stands_for(kind: crate::codec::Primitive) -> bool {
        kind == Self::PRIMITIVE
    }

    /// Write one value.
    ///
    /// # Errors
    /// A `char` above U+00FF, which an IDL `char` cannot carry.
    fn write(writer: &mut Writer, value: Self) -> Result<(), WireError>;

    /// Read one value.
    ///
    /// # Errors
    /// As the reader's own call for the type.
    fn read(reader: &mut Reader<'_>) -> Result<Self, WireError>;

    /// The value a content filter compares (DDS 1.4 Annex B): an integer
    /// up to `long long`, a float, a boolean, a `char` as text; `None`
    /// for an `unsigned long long`, which the filter's `i64` does not
    /// hold.
    fn field_value(self) -> Option<crate::codec::FieldValue>;
}

use crate::codec::FieldValue;

macro_rules! primitives {
    ($($ty:ty => $write:ident, $read:ident, $kind:ident, $zero:expr, |$v:ident| $field:expr;)*) => {$(
        impl sealed::Sealed for $ty {}
        impl IdlPrimitive for $ty {
            const PRIMITIVE: crate::codec::Primitive = crate::codec::Primitive::$kind;
            const ZERO: Self = $zero;
            fn write(writer: &mut Writer, value: Self) -> Result<(), WireError> {
                writer.$write(value);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<Self, WireError> {
                reader.$read()
            }
            fn field_value(self) -> Option<crate::codec::FieldValue> {
                let $v = self;
                $field
            }
        }
    )*};
}

primitives! {
    bool => write_bool, read_bool, Boolean, false, |v| Some(FieldValue::Bool(v));
    i8 => write_i8, read_i8, Int8, 0, |v| Some(FieldValue::Int(i64::from(v)));
    i16 => write_i16, read_i16, Int16, 0, |v| Some(FieldValue::Int(i64::from(v)));
    u16 => write_u16, read_u16, UInt16, 0, |v| Some(FieldValue::Int(i64::from(v)));
    i32 => write_i32, read_i32, Int32, 0, |v| Some(FieldValue::Int(i64::from(v)));
    u32 => write_u32, read_u32, UInt32, 0, |v| Some(FieldValue::Int(i64::from(v)));
    i64 => write_i64, read_i64, Int64, 0, |v| Some(FieldValue::Int(v));
    u64 => write_u64, read_u64, UInt64, 0, |v| { let _ = v; None };
    f32 => write_f32, read_f32, Float32, 0.0, |v| Some(FieldValue::Float(f64::from(v)));
    f64 => write_f64, read_f64, Float64, 0.0, |v| Some(FieldValue::Float(v));
}

impl sealed::Sealed for u8 {}
/// `octet` and `uint8` both: the two IDL types share one Rust type, and a
/// description names `octet`.
impl IdlPrimitive for u8 {
    const PRIMITIVE: crate::codec::Primitive = crate::codec::Primitive::Octet;
    const ZERO: Self = 0;
    fn stands_for(kind: crate::codec::Primitive) -> bool {
        matches!(
            kind,
            crate::codec::Primitive::Octet | crate::codec::Primitive::UInt8
        )
    }
    fn write(writer: &mut Writer, value: Self) -> Result<(), WireError> {
        writer.write_u8(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, WireError> {
        reader.read_u8()
    }
    fn field_value(self) -> Option<FieldValue> {
        Some(FieldValue::Int(i64::from(self)))
    }
}

impl sealed::Sealed for char {}
impl IdlPrimitive for char {
    const PRIMITIVE: crate::codec::Primitive = crate::codec::Primitive::Char;
    const ZERO: Self = '\0';
    fn write(writer: &mut Writer, value: Self) -> Result<(), WireError> {
        writer.write_char(value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, WireError> {
        reader.read_char()
    }
    fn field_value(self) -> Option<FieldValue> {
        Some(FieldValue::Text(alloc::string::ToString::to_string(&self)))
    }
}

impl<P: IdlPrimitive> IdlOut<'_, P> {
    /// The value.
    ///
    /// # Errors
    /// A `char` above U+00FF, which an IDL `char` cannot carry.
    pub fn value(self, value: P) -> Result<Written, WireError> {
        P::write(self.writer, value)?;
        Ok(Written::new())
    }
}

macro_rules! constructors {
    ($($ty:ty => $($name:ident: $idl:literal),+;)*) => {$(
        impl<'w> IdlOut<'w, $ty> {
            $(
                #[doc = concat!("An IDL `", $idl, "`.")]
                pub const fn $name(writer: &'w mut Writer) -> Self {
                    Self::over(writer, None)
                }
            )+
        }
    )*};
}

constructors! {
    bool => boolean: "boolean";
    u8 => octet: "octet", uint8: "uint8";
    i8 => int8: "int8";
    i16 => int16: "short";
    u16 => uint16: "unsigned short";
    i32 => int32: "long";
    u32 => uint32: "unsigned long";
    i64 => int64: "long long";
    u64 => uint64: "unsigned long long";
    f32 => float: "float";
    f64 => double: "double";
    char => char: "char";
}

/// A Rust type that stands for an IDL primitive or `string` on the wire.
///
/// `Idl` is the IDL type's Rust spelling: `str` for a `string`, the
/// [`IdlPrimitive`] for a primitive. The generated code asks for exactly
/// that type and the compiler holds the impl to it; a type mapped onto a
/// `string` member whose `Idl` is `u32` does not compile.
///
/// ```
/// use zerodds_wire::{IdlOut, WireError, WireRepr, Written};
///
/// /// A call sign: two to eight upper-case letters and digits.
/// #[derive(Debug, Clone, PartialEq)]
/// pub struct CallSign(String);
///
/// impl WireRepr for CallSign {
///     type Idl = str;
///     fn to_wire(&self, out: IdlOut<'_, str>) -> Result<Written, WireError> {
///         out.str(&self.0)
///     }
///     fn from_wire(wire: &str) -> Result<Self, WireError> {
///         let valid = (2..=8).contains(&wire.len())
///             && wire.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
///         if valid {
///             Ok(Self(wire.to_owned()))
///         } else {
///             Err(WireError::invalid_value("a call sign of 2 to 8 upper-case letters and digits"))
///         }
///     }
/// }
/// ```
pub trait WireRepr: Sized {
    /// The IDL type this type stands for: `str`, or an [`IdlPrimitive`].
    type Idl: ?Sized;

    /// Write the value as its IDL type.
    ///
    /// # Errors
    /// What the [`IdlOut`] refuses, or a refusal of the impl's own.
    fn to_wire(&self, out: IdlOut<'_, Self::Idl>) -> Result<Written, WireError>;

    /// The value from its IDL form, or a refusal naming what is accepted
    /// ([`WireError::invalid_value`]).
    ///
    /// # Errors
    /// A value of the IDL type this type does not accept; the sample is
    /// not delivered.
    fn from_wire(wire: &Self::Idl) -> Result<Self, WireError>;
}

/// [`WireRepr`] for a type `T` this crate cannot implement it for, through
/// an adapter value.
///
/// The orphan rule keeps a crate from writing `impl WireRepr for
/// chrono::DateTime<Utc>`; it can write `impl WireReprWith<DateTime<Utc>>
/// for MyAdapter` for a unit struct of its own, and name that adapter
/// beside the type (`zerodds_build::Config::rust_type_with`). The
/// generated code calls it through a reference to the adapter value, so an
/// adapter is a unit struct or a constant.
pub trait WireReprWith<T> {
    /// As [`WireRepr::Idl`].
    type Idl: ?Sized;

    /// As [`WireRepr::to_wire`], for `value`.
    ///
    /// # Errors
    /// As [`WireRepr::to_wire`].
    fn to_wire(&self, value: &T, out: IdlOut<'_, Self::Idl>) -> Result<Written, WireError>;

    /// As [`WireRepr::from_wire`].
    ///
    /// # Errors
    /// As [`WireRepr::from_wire`].
    // The adapter value is `self` and the converted value the result: the
    // name is `WireRepr::from_wire`'s, so a generated call reads the same
    // on both routes.
    #[allow(clippy::wrong_self_convention)]
    fn from_wire(&self, wire: &Self::Idl) -> Result<T, WireError>;
}

/// The adapter for a type with a text form: an IDL `string` written with
/// `Display` and read with `FromStr`.
///
/// `Display` writes into the sample's buffer ([`IdlOut::display`]) and
/// `FromStr` parses the `&str` lent out of the received sample; a text
/// `FromStr` refuses rejects the sample with the parser's message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DisplayFromStr;

impl<T> WireReprWith<T> for DisplayFromStr
where
    T: Display + FromStr,
    T::Err: Display,
{
    type Idl = str;

    fn to_wire(&self, value: &T, out: IdlOut<'_, str>) -> Result<Written, WireError> {
        out.display(value)
    }

    fn from_wire(&self, wire: &str) -> Result<T, WireError> {
        wire.parse::<T>()
            .map_err(|error| WireError::invalid_value(format!("text `FromStr` accepts: {error}")))
    }
}

macro_rules! identity {
    ($($ty:ty),*) => {$(
        impl WireRepr for $ty {
            type Idl = $ty;
            fn to_wire(&self, out: IdlOut<'_, $ty>) -> Result<Written, WireError> {
                out.value(*self)
            }
            fn from_wire(wire: &$ty) -> Result<Self, WireError> {
                Ok(*wire)
            }
        }
    )*};
}

identity!(bool, u8, i8, i16, u16, i32, u32, i64, u64, f32, f64, char);

macro_rules! non_zero {
    ($($ty:ty),*) => {$(
        /// The IDL integer, with zero refused on the way in.
        impl WireRepr for NonZero<$ty> {
            type Idl = $ty;
            fn to_wire(&self, out: IdlOut<'_, $ty>) -> Result<Written, WireError> {
                out.value(self.get())
            }
            fn from_wire(wire: &$ty) -> Result<Self, WireError> {
                NonZero::new(*wire).ok_or_else(|| {
                    WireError::invalid_value(concat!(
                        "a value other than zero, which a `NonZero<",
                        stringify!($ty),
                        ">` holds"
                    ))
                })
            }
        }
    )*};
}

non_zero!(u8, i8, u16, i16, u32, i32, u64, i64);

impl WireRepr for String {
    type Idl = str;

    fn to_wire(&self, out: IdlOut<'_, str>) -> Result<Written, WireError> {
        out.str(self)
    }

    fn from_wire(wire: &str) -> Result<Self, WireError> {
        Ok(Self::from(wire))
    }
}

/// An IDL `string` as `String` is; a borrowed value is written without a
/// copy, and a read one is owned.
impl WireRepr for Cow<'_, str> {
    type Idl = str;

    fn to_wire(&self, out: IdlOut<'_, str>) -> Result<Written, WireError> {
        out.str(self)
    }

    fn from_wire(wire: &str) -> Result<Self, WireError> {
        Ok(Cow::Owned(String::from(wire)))
    }
}

// -- what the generated code calls ----------------------------------------

/// Write `value` as its IDL type.
///
/// [`WireRepr::to_wire`] with the [`Written`] dropped, the shape the
/// generated code's element functions take.
///
/// # Errors
/// As [`WireRepr::to_wire`].
pub fn to_wire<T: WireRepr>(value: &T, out: IdlOut<'_, T::Idl>) -> Result<(), WireError> {
    value.to_wire(out).map(drop)
}

/// [`to_wire`] through an adapter.
///
/// # Errors
/// As [`WireReprWith::to_wire`].
pub fn to_wire_with<A: WireReprWith<T>, T>(
    adapter: &A,
    value: &T,
    out: IdlOut<'_, A::Idl>,
) -> Result<(), WireError> {
    adapter.to_wire(value, out).map(drop)
}

/// The value of a member read at `at`, from its IDL form.
///
/// A refusal names `at`, the offset the IDL value was read from, so the
/// rejected sample's error points into its bytes.
///
/// # Errors
/// As [`WireRepr::from_wire`].
pub fn from_wire<T: WireRepr>(wire: &T::Idl, at: usize) -> Result<T, WireError> {
    T::from_wire(wire).map_err(|error| error.located(at))
}

/// [`from_wire`] through an adapter.
///
/// # Errors
/// As [`WireReprWith::from_wire`].
pub fn from_wire_with<A: WireReprWith<T>, T>(
    adapter: &A,
    wire: &A::Idl,
    at: usize,
) -> Result<T, WireError> {
    adapter.from_wire(wire).map_err(|error| error.located(at))
}

/// The value a member takes that the sample did not carry: its IDL
/// default (XTypes 1.3 Table 9, or its `@default`), converted.
///
/// A type that does not accept the IDL default — a `NonZero` for the
/// default zero — has no default value, and a sample that leaves the
/// member out is rejected with this reason.
///
/// # Errors
/// As [`WireRepr::from_wire`], saying the value was the member's default.
pub fn from_default<T: WireRepr>(wire: &T::Idl) -> Result<T, WireError> {
    T::from_wire(wire).map_err(defaulted)
}

/// [`from_default`] through an adapter.
///
/// # Errors
/// As [`from_default`].
pub fn from_default_with<A: WireReprWith<T>, T>(
    adapter: &A,
    wire: &A::Idl,
) -> Result<T, WireError> {
    adapter.from_wire(wire).map_err(defaulted)
}

fn defaulted(error: WireError) -> WireError {
    match error {
        WireError::OutOfRange { at, expected } => WireError::OutOfRange {
            at,
            expected: format!(
                "{expected}, for the IDL default value of a member the sample did not carry"
            ),
        },
        other => other,
    }
}

/// Compiles only where `T`'s [`WireRepr::Idl`] is `I`.
///
/// The generated code states every `rust_type` mapping once with it, so
/// a Rust type whose `Idl` does not match its member's IDL type fails the
/// build at a line that names both.
pub const fn idl_type_is<T, I>()
where
    T: WireRepr<Idl = I>,
    I: ?Sized,
{
}

/// [`idl_type_is`] for an adapter `A` over `T`.
pub const fn idl_type_with_is<A, T, I>()
where
    A: WireReprWith<T, Idl = I>,
    I: ?Sized,
{
}

/// The IDL `string` `value` stands for, as a `String`.
///
/// What a content filter compares: written through
/// [`WireRepr::to_wire`] and read back, so it is the text a peer
/// receives.
///
/// # Errors
/// As [`WireRepr::to_wire`].
pub fn idl_string<T: WireRepr<Idl = str>>(value: &T) -> Result<String, WireError> {
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    value.to_wire(IdlOut::string(&mut writer))?;
    let bytes = writer.finish();
    Reader::with_max_align(&bytes, Endianness::Little, 4)
        .read_str()
        .map(String::from)
}

/// [`idl_string`] through an adapter.
///
/// # Errors
/// As [`WireReprWith::to_wire`].
pub fn idl_string_with<A: WireReprWith<T, Idl = str>, T>(
    adapter: &A,
    value: &T,
) -> Result<String, WireError> {
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    adapter.to_wire(value, IdlOut::string(&mut writer))?;
    let bytes = writer.finish();
    Reader::with_max_align(&bytes, Endianness::Little, 4)
        .read_str()
        .map(String::from)
}

/// The IDL primitive `value` stands for.
///
/// As [`idl_string`], for a primitive.
///
/// # Errors
/// As [`WireRepr::to_wire`].
pub fn idl_primitive<T, P>(value: &T) -> Result<P, WireError>
where
    T: WireRepr<Idl = P>,
    P: IdlPrimitive,
{
    let mut writer = Writer::with_max_align(Endianness::Little, 8);
    value.to_wire(IdlOut::over(&mut writer, None))?;
    let bytes = writer.finish();
    P::read(&mut Reader::with_max_align(&bytes, Endianness::Little, 8))
}

/// [`idl_primitive`] through an adapter.
///
/// # Errors
/// As [`WireReprWith::to_wire`].
pub fn idl_primitive_with<A, T, P>(adapter: &A, value: &T) -> Result<P, WireError>
where
    A: WireReprWith<T, Idl = P>,
    P: IdlPrimitive,
{
    let mut writer = Writer::with_max_align(Endianness::Little, 8);
    adapter.to_wire(value, IdlOut::over(&mut writer, None))?;
    let bytes = writer.finish();
    P::read(&mut Reader::with_max_align(&bytes, Endianness::Little, 8))
}
