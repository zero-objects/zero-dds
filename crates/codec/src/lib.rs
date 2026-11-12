// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-codec`. Safety classification: **SAFE**.
//!
//! A DDS topic type defined in Rust instead of IDL.
//!
//! ```ignore
//! use zerodds_codec::{DdsType, WireRepr};
//!
//! #[derive(DdsType)]
//! #[dds(name = "app::Track", extensibility = "appendable")]
//! struct Track {
//!     #[dds(key)]
//!     id: core::num::NonZeroU32, // unsigned long
//!     at: Dtg,                   // string, through the Dtg's WireRepr
//!     #[dds(bound = 64)]
//!     label: String,             // string<64>
//!     note: Option<String>,      // @optional string
//! }
//!
//! println!("{}", Track::idl()?); // the IDL a C or C++ peer is built from
//! ```
//!
//! `derive(DdsType)` (crate `zerodds-derive`, re-exported here) writes
//! three things for a struct: its description ([`DdsTypeInfo`]), its
//! codec ([`DdsSerialize`], [`DdsDeserialize`]) and the
//! `zerodds_dcps::DdsType` impl a `DataWriter` and a `DataReader` take.
//! A hand-written codec writes the first two itself, with the derive's
//! `#[dds(codec = "manual")]` or wholly by hand; the serializer is typed
//! by the description, so a member left out, one too many or one of
//! another type does not compile (see [`codec`]).
//!
//! ## One model
//!
//! The description becomes the IDL it stands for ([`idl::idl_of`]), the
//! IDL goes through `zerodds-idl-compose` as a `.idl` file would, and the
//! model it composes to goes to the same places a `.idl` file's goes:
//! `zerodds-types` computes the `TypeObject` and the `TypeIdentifier`,
//! `zerodds-ast-layout` plans the layout under XCDR2 and XCDR1. The plans
//! drive the serializer and the deserializer of `zerodds-wire`. Nothing
//! here decides an octet, an id or a hash a second way, so a derived type
//! and the type `zerodds-build` generates from the equivalent IDL are the
//! same type on the wire and in discovery.
//!
//! The model is built once per type, on first use, and kept for the
//! process ([`model::model`]).
//!
//! ## Unions, generic types, borrowed fields
//!
//! An enum whose variants carry data is an IDL `union`: its discriminator
//! type is `#[dds(discriminator = "…")]` (`i32` when not given), a variant's
//! labels are `#[dds(case = …)]` or counted, `#[dds(default)]` marks the
//! `default:` case and `#[dds(no_member)]` a variant that holds a
//! discriminator no case takes. A generic type is one IDL type per
//! instantiation, named from its arguments (`Pair<u32>` is
//! `Pair_unsigned_long`). A field may borrow from the received sample:
//! [`decode`] over `&'de [u8]` lends `&'de str`, `&'de [u8]` and
//! `Cow<'de, …>` out of the bytes. A type with a borrowed field has no
//! `zerodds_dcps::DdsType` impl: a DataReader takes an owned `T`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod idl;
mod key_holder_be;
pub mod model;

pub use model::{Model, ModelError};
pub use zerodds_derive::{DdsType, WireRepr};
pub use zerodds_wire::codec;
pub use zerodds_wire::codec::{
    Appendable, B0, B1, B2, B3, B4, B5, B6, B7, DdsDeserialize, DdsDeserializer, DdsMember,
    DdsMemberRead, DdsSerialize, DdsSerializer, DdsTypeInfo, DdsUnionInfo, Decoded, Discriminator,
    FieldValue, Final, Mutable, Serialized, With,
};
pub use zerodds_wire::{DisplayFromStr, IdlOut, WireError, WireRepr, WireReprWith, Written};

use zerodds_wire::codec::PlanSet;
use zerodds_wire::{Endianness, Reader, Writer};

/// The four encapsulations a sample of a described type travels in
/// (XTypes 1.3 §7.6.3.1.2): XCDR2 or XCDR1, in either byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encapsulation {
    /// XCDR2, little-endian.
    Xcdr2Le,
    /// XCDR2, big-endian.
    Xcdr2Be,
    /// XCDR1, little-endian.
    Xcdr1Le,
    /// XCDR1, big-endian.
    Xcdr1Be,
}

impl Encapsulation {
    /// All four.
    pub const ALL: [Self; 4] = [Self::Xcdr2Le, Self::Xcdr2Be, Self::Xcdr1Le, Self::Xcdr1Be];

    /// The byte order.
    #[must_use]
    pub const fn endianness(self) -> Endianness {
        match self {
            Self::Xcdr2Le | Self::Xcdr1Le => Endianness::Little,
            Self::Xcdr2Be | Self::Xcdr1Be => Endianness::Big,
        }
    }

    /// Whether this is XCDR1.
    #[must_use]
    pub const fn is_xcdr1(self) -> bool {
        matches!(self, Self::Xcdr1Le | Self::Xcdr1Be)
    }

    /// The encapsulation identifier a sample of a type with
    /// `extensibility` carries (XTypes 1.3 Table 60, DDSI-RTPS 2.5
    /// §10.5): `CDR`/`PL_CDR` under XCDR1, `CDR2`/`D_CDR2`/`PL_CDR2`
    /// under XCDR2, the odd value for little-endian.
    #[must_use]
    pub const fn identifier(self, extensibility: codec::Extensibility) -> [u8; 2] {
        use codec::Extensibility as E;
        let big = match (self.is_xcdr1(), extensibility) {
            (true, E::Final | E::Appendable) => 0x0000,
            (true, E::Mutable) => 0x0002,
            (false, E::Final) => 0x0006,
            (false, E::Appendable) => 0x0008,
            (false, E::Mutable) => 0x000a,
        };
        let id: u16 = match self.endianness() {
            Endianness::Little => big | 1,
            Endianness::Big => big,
        };
        id.to_be_bytes()
    }

    /// The encapsulation an identifier names, for a type with
    /// `extensibility`; `None` for one that type does not travel in.
    #[must_use]
    pub fn from_identifier(
        identifier: [u8; 2],
        extensibility: codec::Extensibility,
    ) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|encapsulation| encapsulation.identifier(extensibility) == identifier)
    }
}

/// Why a value was not encoded or decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The type has no model: its description does not compose or plan.
    Model(ModelError),
    /// The type has no XCDR1 plan.
    NoXcdr1 {
        /// The type.
        type_name: &'static str,
        /// What the planner said.
        detail: String,
    },
    /// The value or the stream was refused, with the reason and the
    /// offset.
    Wire(WireError),
    /// A lent sample without data: a dispose or an unregister
    /// (`SampleInfo::valid_data` is `false`).
    NoData,
    /// A lent sample read as a type other than its DataReader's.
    OtherType {
        /// The DataReader's type, which the loan holds samples of.
        lent: &'static str,
        /// The type it was read as.
        asked: String,
    },
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Model(error) => write!(f, "no model: {error}"),
            Self::NoXcdr1 { type_name, detail } => {
                write!(f, "{type_name} has no XCDR1 layout: {detail}")
            }
            Self::Wire(error) => write!(f, "{error}"),
            Self::NoData => write!(f, "the lent sample has no data"),
            Self::OtherType { lent, asked } => {
                write!(f, "a sample of {lent} read as {asked}")
            }
        }
    }
}

/// A value written by a [`zerodds_dcps::DataWriter`] of another Rust type
/// of the same IDL type ([`zerodds_dcps::WriteAs`]): a derived type whose
/// fields borrow for less than the program's life, written through the
/// writer of its `'static` form. [`write()`] wraps it.
#[derive(Debug, Clone, Copy)]
pub struct Borrowed<'v, V>(pub &'v V);

impl<T: zerodds_dcps::DdsType, V: DdsSerialize> zerodds_dcps::WriteAs<T> for Borrowed<'_, V> {
    fn encode_as(
        &self,
        out: &mut Vec<u8>,
        xcdr1: bool,
        big_endian: bool,
    ) -> Result<(), zerodds_dcps::EncodeError> {
        let encapsulation = match (xcdr1, big_endian) {
            (false, false) => Encapsulation::Xcdr2Le,
            (false, true) => Encapsulation::Xcdr2Be,
            (true, false) => Encapsulation::Xcdr1Le,
            (true, true) => Encapsulation::Xcdr1Be,
        };
        encode(self.0, encapsulation, out).map_err(|error| zerodds_dcps::EncodeError::Invalid {
            what: match error {
                CodecError::Wire(_) => "the value does not encode",
                _ => "the type has no model",
            },
        })
    }

    fn key_material_as(
        &self,
    ) -> Result<Option<zerodds_dcps::dds_type::KeyMaterial>, zerodds_dcps::EncodeError> {
        let refused = |error: CodecError| zerodds_dcps::EncodeError::Invalid {
            what: match error {
                CodecError::Wire(_) => "the key does not encode",
                _ => "the type has no model",
            },
        };
        let Some(hash) = key_hash(self.0).map_err(refused)? else {
            return Ok(None);
        };
        Ok(Some((hash, key_holder(self.0).map_err(refused)?)))
    }
}

/// Writes `value` with `writer`, whose type is `value`'s IDL type with
/// `'static` borrows: a derived type whose fields borrow, written without
/// owning them (GitHub #33).
///
/// # Errors
/// `BadParameter` when `value`'s IDL name is not the writer's type,
/// otherwise as `DataWriter::write`.
pub fn write<T, V>(writer: &zerodds_dcps::DataWriter<T>, value: &V) -> zerodds_dcps::Result<()>
where
    T: zerodds_dcps::DdsType + Send + 'static,
    V: DdsSerialize,
{
    let name = V::LAYOUT_REF.scoped_name();
    if name.trim_start_matches("::") != T::TYPE_NAME.trim_start_matches("::") {
        return Err(zerodds_dcps::DdsError::BadParameter {
            what: "a value of another IDL type than the writer's",
        });
    }
    writer.write_as(&Borrowed(value))
}

/// Reading what a DataReader lent ([`zerodds_dcps::DataReader::take_loan`])
/// as a type that may borrow from it: a field `&'a str`, `&'a [u8]`, a
/// `Cow<'a, …>` or a derived type with a lifetime is lent out of the octets
/// the reader received, nothing is copied (GitHub #33).
pub trait LoanExt {
    /// The sample at `index` read as `V`; `None` past the last one.
    ///
    /// # Errors
    /// [`CodecError::NoData`] for a sample without data,
    /// [`CodecError::OtherType`] for a `V` whose IDL name is not the
    /// DataReader's type, otherwise what [`decode`] refuses.
    fn read_as<'a, V: DdsDeserialize<'a>>(&'a self, index: usize) -> Option<Result<V, CodecError>>;
}

impl LoanExt for zerodds_dcps::loan::Loan {
    fn read_as<'a, V: DdsDeserialize<'a>>(&'a self, index: usize) -> Option<Result<V, CodecError>> {
        let sample = self.get(index)?;
        let asked = V::LAYOUT_REF.scoped_name();
        let unscoped = |name: &str| name.trim_start_matches("::").to_string();
        if unscoped(&asked) != unscoped(self.type_name()) {
            return Some(Err(CodecError::OtherType {
                lent: self.type_name(),
                asked: asked.into_owned(),
            }));
        }
        let Some(body) = sample.body() else {
            return Some(Err(CodecError::NoData));
        };
        let encapsulation = match (sample.is_xcdr1(), sample.is_big_endian()) {
            (false, false) => Encapsulation::Xcdr2Le,
            (false, true) => Encapsulation::Xcdr2Be,
            (true, false) => Encapsulation::Xcdr1Le,
            (true, true) => Encapsulation::Xcdr1Be,
        };
        Some(decode(body, encapsulation))
    }
}

impl std::error::Error for CodecError {}

impl From<ModelError> for CodecError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl From<WireError> for CodecError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// The plans of `T` under `encapsulation`.
fn plans_of<T: DdsTypeInfo>(encapsulation: Encapsulation) -> Result<&'static PlanSet, CodecError> {
    let model = model::model::<T>()?;
    if encapsulation.is_xcdr1() {
        model.xcdr1.as_ref().map_err(|detail| CodecError::NoXcdr1 {
            type_name: model.name,
            detail: detail.clone(),
        })
    } else {
        Ok(&model.xcdr2)
    }
}

/// Append `value`'s body under `encapsulation` to `out`: the octets after
/// the four of the encapsulation header. A refused value leaves `out` as
/// it was.
///
/// # Errors
/// [`CodecError`]: no model or plan, or a value the plan refuses — a
/// bound exceeded, a `NonZero` at zero, a text its type does not write.
pub fn encode<T: DdsSerialize>(
    value: &T,
    encapsulation: Encapsulation,
    out: &mut Vec<u8>,
) -> Result<(), CodecError> {
    let plans = plans_of::<T>(encapsulation)?;
    let start = out.len();
    let mut w = Writer::appending(
        encapsulation.endianness(),
        plans.max_align,
        core::mem::take(out),
    );
    let written = value
        .serialize(DdsSerializer::new(&mut w, plans))
        .and_then(|_| w.check_deferred());
    *out = w.finish();
    if written.is_err() {
        out.truncate(start);
    }
    written?;
    Ok(())
}

/// Read a value of `T` from a body under `encapsulation`. A `string` a
/// [`WireRepr`] member reads reaches it as a `&str` lent out of `bytes`.
///
/// # Errors
/// [`CodecError`]: no model or plan, a stream the plan refuses, or a
/// value a member's type refuses, with the reason and the offset.
pub fn decode<'de, T: DdsDeserialize<'de>>(
    bytes: &'de [u8],
    encapsulation: Encapsulation,
) -> Result<T, CodecError> {
    let plans = plans_of::<T>(encapsulation)?;
    let mut reader = Reader::with_max_align(bytes, encapsulation.endianness(), plans.max_align);
    Ok(T::deserialize(DdsDeserializer::new(&mut reader, plans))?.into_inner())
}

/// The key holder of `value` (XTypes 1.3 §7.6.8): its key members in
/// PLAIN_CDR2, big-endian. Empty for a type with no key.
///
/// # Errors
/// [`CodecError`]: no model, or a key member the plan refuses.
pub fn key_holder<T: DdsSerialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    let model = model::model::<T>()?;
    let Some(key) = &model.xcdr2.key else {
        return Ok(Vec::new());
    };
    let mut w = Writer::with_max_align(Endianness::Big, key.max_align);
    value.serialize(DdsSerializer::key_holder(&mut w, &model.xcdr2))?;
    Ok(w.finish_checked()?)
}

/// The KeyHash of `value` (XTypes 1.3 §7.6.8 step 5, DDSI-RTPS 2.5
/// §9.6.3.8): the holder zero-padded when the type's holder never
/// exceeds sixteen octets, its MD5 otherwise. `None` for a type with no
/// key.
///
/// # Errors
/// As [`key_holder`].
pub fn key_hash<T: DdsSerialize>(value: &T) -> Result<Option<[u8; 16]>, CodecError> {
    let model = model::model::<T>()?;
    let Some(key) = &model.xcdr2.key else {
        return Ok(None);
    };
    let holder = key_holder(value)?;
    hash_of_holder(key, &holder).map(Some)
}

/// XTypes 1.3 §7.6.8 step 5: the KeyHash of a key holder, by the branch
/// the plan chose for the type.
fn hash_of_holder(
    key: &zerodds_wire::codec::KeyPlan,
    holder: &[u8],
) -> Result<[u8; 16], CodecError> {
    if !key.zero_padded {
        return Ok(zerodds_wire::md5(holder));
    }
    // The plan chose the zero-padded branch because no holder of the type
    // exceeds sixteen octets; one that did would be cut, so it is refused.
    if holder.len() > 16 {
        return Err(CodecError::Wire(WireError::refused(
            16,
            "a key holder of at most sixteen octets, as the plan's zero-padded KeyHash has",
        )));
    }
    let mut hash = [0u8; 16];
    hash[..holder.len()].copy_from_slice(holder);
    Ok(hash)
}

/// The KeyHash and the key holder of the sample in `bytes` (a body under
/// `encapsulation`), read from the body by the plans: the key members are
/// read, every other member is stepped over, no value of `T` is built
/// ([`zerodds_wire::codec::key_holder_from_body`]). `Ok(None)` for a type
/// without a key, and for a shape that read does not take; the whole
/// sample is read then.
///
/// # Errors
/// [`CodecError`]: no model or plan, or what the stream refuses.
pub fn key_material_by_plan<T: DdsTypeInfo>(
    bytes: &[u8],
    encapsulation: Encapsulation,
) -> Result<Option<zerodds_dcps::dds_type::KeyMaterial>, CodecError> {
    let model = model::model::<T>()?;
    let Some(key) = &model.xcdr2.key else {
        return Ok(None);
    };
    let data = plans_of::<T>(encapsulation)?;
    let (
        zerodds_wire::codec::Root::Struct(data_root),
        zerodds_wire::codec::Root::Struct(holder_root),
    ) = (data.root, key.root)
    else {
        return Ok(None);
    };
    let mut reader = Reader::with_max_align(bytes, encapsulation.endianness(), data.max_align);
    let mut holder = Writer::with_max_align(Endianness::Big, key.max_align);
    let taken = zerodds_wire::codec::key_holder_from_body(
        &mut reader,
        data,
        data_root,
        &model.xcdr2,
        holder_root,
        &mut holder,
    )?;
    if !taken {
        return Ok(None);
    }
    let holder = holder.finish_checked()?;
    Ok(Some((hash_of_holder(key, &holder)?, holder)))
}

/// Read a value of `T` back from its key holder: the key members from the
/// holder, every other member at its IDL default.
///
/// # Errors
/// [`CodecError`]: no model, a type with no key, a holder the plan
/// refuses, or a member whose Rust type refuses its default.
pub fn from_key_holder<T: for<'de> DdsDeserialize<'de>>(bytes: &[u8]) -> Result<T, CodecError> {
    let model = model::model::<T>()?;
    let key = model.xcdr2.key.as_ref().ok_or_else(|| {
        CodecError::Wire(WireError::refused(
            0,
            "a type with @key members, for its key holder",
        ))
    })?;
    let mut reader = Reader::with_max_align(bytes, Endianness::Big, key.max_align);
    Ok(T::deserialize(DdsDeserializer::key_holder(&mut reader, &model.xcdr2))?.into_inner())
}

/// The IDL `T` stands for, with every type it reaches: what a C, C++ or
/// other vendor's peer is generated from.
///
/// # Errors
/// [`ModelError`]: a description that does not compose.
pub fn idl<T: DdsTypeInfo>() -> Result<String, ModelError> {
    model::model::<T>().map(|model| model.idl.clone())
}

/// What `derive(DdsType)` expands to names, kept out of the documented
/// surface.
#[doc(hidden)]
pub mod __private {
    pub use zerodds_dcps as dcps;
    pub use zerodds_types as types;
    pub use zerodds_wire as wire;

    use super::{CodecError, DdsDeserialize, DdsSerialize, DdsTypeInfo, Encapsulation, FieldValue};
    use zerodds_dcps::{DecodeError, EncodeError, FilterValue};

    /// How many distinct reasons [`reason`] keeps before it answers with
    /// the kind alone.
    const MAX_REASONS: usize = 256;

    /// The reason of a refusal, as the `&'static str` the DCPS error
    /// carries (`DecodeError::Invalid { what }`).
    ///
    /// The text is kept once per distinct reason, without the offset, so
    /// the refusals of one type map onto a small set. A `from_wire` that
    /// puts the received value into its reason would make every sample a
    /// new text; past [`MAX_REASONS`] the kind alone is reported, so a
    /// peer cannot grow the set without bound.
    fn reason(error: &CodecError) -> &'static str {
        use std::collections::HashSet;
        use std::sync::{Mutex, OnceLock};
        use zerodds_wire::WireError as W;
        static REASONS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
        let (text, kind) = match error {
            CodecError::Wire(W::Truncated { .. }) => {
                return "zerodds-codec: the stream ended inside a value";
            }
            CodecError::Wire(W::ImpossibleLength { .. }) => {
                return "zerodds-codec: a declared length the stream cannot hold";
            }
            CodecError::Wire(W::NotUtf8 { .. }) => {
                return "zerodds-codec: a string holding bytes that are not UTF-8";
            }
            CodecError::Wire(W::OutOfRange { expected, .. }) => (
                format!("zerodds-codec: expected {expected}"),
                "zerodds-codec: a value outside what the type admits",
            ),
            CodecError::Wire(W::Discarded { expected, .. }) => (
                format!("zerodds-codec: discarded, the type holds {expected}"),
                "zerodds-codec: a value the receiving type discards",
            ),
            CodecError::Wire(W::WrongFraming { expected, .. }) => (
                format!("zerodds-codec: framing, expected {expected}"),
                "zerodds-codec: the framing is not what this encoding asks for",
            ),
            other => (
                format!("zerodds-codec: {other}"),
                "zerodds-codec: the type has no model or no plan",
            ),
        };
        let reasons = REASONS.get_or_init(|| Mutex::new(HashSet::new()));
        let Ok(mut reasons) = reasons.lock() else {
            return kind;
        };
        if let Some(found) = reasons.get(text.as_str()) {
            return found;
        }
        if reasons.len() >= MAX_REASONS {
            return kind;
        }
        let kept: &'static str = Box::leak(text.into_boxed_str());
        reasons.insert(kept);
        kept
    }

    fn encode_error(error: &CodecError) -> EncodeError {
        EncodeError::Invalid {
            what: reason(error),
        }
    }

    fn decode_error(error: &CodecError) -> DecodeError {
        match error {
            CodecError::Wire(wire @ zerodds_wire::WireError::Discarded { .. }) => {
                DecodeError::from(wire.clone())
            }
            other => DecodeError::Invalid {
                what: reason(other),
            },
        }
    }

    /// `DdsType::encode` and its three siblings.
    ///
    /// # Errors
    /// [`EncodeError::Invalid`] with the codec's reason.
    pub fn encode<T: DdsSerialize>(
        value: &T,
        encapsulation: Encapsulation,
        out: &mut Vec<u8>,
    ) -> Result<(), EncodeError> {
        super::encode(value, encapsulation, out).map_err(|error| encode_error(&error))
    }

    /// `DdsType::decode` and its three siblings.
    ///
    /// # Errors
    /// [`DecodeError`] with the codec's reason.
    pub fn decode<T: for<'de> DdsDeserialize<'de>>(
        bytes: &[u8],
        encapsulation: Encapsulation,
    ) -> Result<T, DecodeError> {
        super::decode(bytes, encapsulation).map_err(|error| decode_error(&error))
    }

    /// `DdsType::encode_continuing`: the value behind a prefix someone
    /// else wrote, alignment counted from the start of the sample.
    ///
    /// # Errors
    /// [`EncodeError::Invalid`] with the codec's reason.
    pub fn encode_continuing<T: DdsSerialize>(
        value: &T,
        prefix: &mut Vec<u8>,
        xcdr1: bool,
    ) -> Result<(), EncodeError> {
        let encapsulation = if xcdr1 {
            Encapsulation::Xcdr1Le
        } else {
            Encapsulation::Xcdr2Le
        };
        let plans = super::plans_of::<T>(encapsulation).map_err(|error| encode_error(&error))?;
        let start = prefix.len();
        let mut w = zerodds_wire::Writer::continuing(
            zerodds_wire::Endianness::Little,
            plans.max_align,
            core::mem::take(prefix),
        );
        let written = value
            .serialize(super::DdsSerializer::new(&mut w, plans))
            .and_then(|_| w.check_deferred());
        *prefix = w.finish();
        if let Err(error) = written {
            prefix.truncate(start);
            return Err(encode_error(&CodecError::Wire(error)));
        }
        Ok(())
    }

    /// `DdsType::decode_continuing`.
    ///
    /// # Errors
    /// [`DecodeError`] with the codec's reason.
    pub fn decode_continuing<T: for<'de> DdsDeserialize<'de>>(
        bytes: &[u8],
        offset: usize,
        xcdr1: bool,
    ) -> Result<T, DecodeError> {
        let encapsulation = if xcdr1 {
            Encapsulation::Xcdr1Le
        } else {
            Encapsulation::Xcdr2Le
        };
        let plans = super::plans_of::<T>(encapsulation).map_err(|error| decode_error(&error))?;
        let mut reader = zerodds_wire::Reader::with_max_align(
            bytes,
            zerodds_wire::Endianness::Little,
            plans.max_align,
        );
        reader
            .skip_to(offset)
            .map_err(|error| decode_error(&CodecError::Wire(error)))?;
        T::deserialize(super::DdsDeserializer::new(&mut reader, plans))
            .map(super::Decoded::into_inner)
            .map_err(|error| decode_error(&CodecError::Wire(error)))
    }

    pub use crate::key_holder_be::encode_key_holder_be;

    /// `DdsType::key_material_from_body` of a type that borrows: the value
    /// read lent out of `bytes`, and its KeyHash and key holder (XTypes 1.3
    /// §7.6.8). A DataReader of the type's `'static` form learns the
    /// sample's instance from it without owning the value.
    ///
    /// # Errors
    /// [`DecodeError`] with the codec's reason; `Invalid` for a type
    /// without a key.
    pub fn key_material_from_body<'de, V: DdsDeserialize<'de> + DdsSerialize>(
        bytes: &'de [u8],
        xcdr1: bool,
        big_endian: bool,
    ) -> Result<([u8; 16], Vec<u8>), DecodeError> {
        let encapsulation = match (xcdr1, big_endian) {
            (false, false) => Encapsulation::Xcdr2Le,
            (false, true) => Encapsulation::Xcdr2Be,
            (true, false) => Encapsulation::Xcdr1Le,
            (true, true) => Encapsulation::Xcdr1Be,
        };
        let value: V = super::decode(bytes, encapsulation).map_err(|error| decode_error(&error))?;
        let hash = super::key_hash(&value)
            .map_err(|error| decode_error(&error))?
            .ok_or(DecodeError::Invalid {
                what: "a type without a key has no KeyHash",
            })?;
        let holder = super::key_holder(&value).map_err(|error| decode_error(&error))?;
        Ok((hash, holder))
    }

    /// `DdsType::key_material_from_body` of a type that owns its fields:
    /// [`super::key_material_by_plan`], `None` where it reads no key, so
    /// that the reader reads the key members or the whole sample.
    pub fn key_material_by_plan<T: DdsTypeInfo>(
        bytes: &[u8],
        xcdr1: bool,
        big_endian: bool,
    ) -> zerodds_dcps::dds_type::KeyMaterialRead {
        let encapsulation = match (xcdr1, big_endian) {
            (false, false) => Encapsulation::Xcdr2Le,
            (false, true) => Encapsulation::Xcdr2Be,
            (true, false) => Encapsulation::Xcdr1Le,
            (true, true) => Encapsulation::Xcdr1Be,
        };
        super::key_material_by_plan::<T>(bytes, encapsulation)
            .map_err(|error| decode_error(&error))
            .transpose()
    }

    /// `DdsType::decode_key_holder_be`.
    ///
    /// # Errors
    /// [`DecodeError`] with the codec's reason.
    pub fn decode_key_holder_be<T: for<'de> DdsDeserialize<'de>>(
        bytes: &[u8],
    ) -> Result<T, DecodeError> {
        super::from_key_holder(bytes).map_err(|error| decode_error(&error))
    }

    /// The model of `T` for the `DdsType` methods that cannot report:
    /// `None` for a description that does not compose or plan. Such a type
    /// states no identity, and every encode and decode of it fails with
    /// the model's reason (`CodecError::Model`), so it is refused at its
    /// first sample rather than matched by name and sent.
    fn model_of<T: DdsTypeInfo>() -> Option<&'static super::Model> {
        super::model::model::<T>().ok()
    }

    /// `DdsType::type_name`: the scoped name, a generic type's with its
    /// arguments spelled in; the template where the type has no model.
    #[must_use]
    pub fn type_name<T: DdsTypeInfo>() -> &'static str {
        model_of::<T>().map_or(T::LAYOUT.name, |model| model.name)
    }

    /// `DdsType::type_identifier`.
    #[must_use]
    pub fn type_identifier<T: DdsTypeInfo>() -> zerodds_types::TypeIdentifier {
        model_of::<T>().map_or(zerodds_types::TypeIdentifier::None, |model| {
            model.type_identifier.clone()
        })
    }

    /// `DdsType::type_object`.
    #[must_use]
    pub fn type_object<T: DdsTypeInfo>() -> Option<zerodds_types::type_object::TypeObject> {
        model_of::<T>().and_then(|model| model.type_object.clone())
    }

    /// `DdsType::type_object_dependencies`.
    #[must_use]
    pub fn type_object_dependencies<T: DdsTypeInfo>() -> Vec<zerodds_types::type_object::TypeObject>
    {
        model_of::<T>().map_or_else(Vec::new, |model| model.dependencies.clone())
    }

    /// `DdsType::type_component`.
    #[must_use]
    pub fn type_component<T: DdsTypeInfo>() -> Option<zerodds_types::resolve::TypeComponent> {
        model_of::<T>().and_then(|model| model.component.clone())
    }

    /// `DdsType::key_holder_max_size`.
    #[must_use]
    pub fn key_holder_max_size<T: DdsTypeInfo>() -> Option<usize> {
        model_of::<T>().and_then(|model| model.key_holder_max_size)
    }

    /// `DdsType::field_value`.
    #[must_use]
    pub fn field_value(value: Option<FieldValue>) -> Option<FilterValue> {
        value.and_then(|value| match value {
            FieldValue::Bool(value) => Some(FilterValue::Bool(value)),
            FieldValue::Int(value) => Some(FilterValue::Int(value)),
            FieldValue::Float(value) => Some(FilterValue::Float(value)),
            FieldValue::Text(value) => Some(FilterValue::String(value)),
            FieldValue::Null => Some(FilterValue::Null),
            FieldValue::Unselected => None,
        })
    }

    /// `DdsType::field_unselected`.
    #[must_use]
    pub fn field_unselected(value: Option<FieldValue>) -> bool {
        matches!(value, Some(FieldValue::Unselected))
    }

    /// `DdsType::EXTENSIBILITY` of a description's extensibility.
    #[must_use]
    pub const fn extensibility(
        extensibility: super::codec::Extensibility,
    ) -> zerodds_dcps::Extensibility {
        match extensibility {
            super::codec::Extensibility::Final => zerodds_dcps::Extensibility::Final,
            super::codec::Extensibility::Appendable => zerodds_dcps::Extensibility::Appendable,
            super::codec::Extensibility::Mutable => zerodds_dcps::Extensibility::Mutable,
        }
    }

    /// Whether a description has a `@key` member, or is a union keyed by
    /// its discriminator.
    #[must_use]
    pub const fn has_key(desc: &super::codec::TypeDesc) -> bool {
        match desc.kind {
            super::codec::TypeKind::Struct { members, .. } => {
                let mut index = 0;
                while index < members.len() {
                    if members[index].key {
                        return true;
                    }
                    index += 1;
                }
                false
            }
            // `switch (@key D)` (DDS-XTypes 1.3 §7.2.2.4.4.4.8).
            super::codec::TypeKind::Union { key, .. } => key,
            super::codec::TypeKind::Enum { .. } => false,
        }
    }
}
