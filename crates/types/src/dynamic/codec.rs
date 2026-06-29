// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reflective XCDR codec: CDR bytes ⟷ [`DynamicData`], driven by a runtime
//! [`DynamicType`].
//!
//! The DDS CDR codec is mature + interop-hardened (the 4-vendor ×
//! 3-extensibility matrix) but **monomorphic** — `DdsType::decode` per generated
//! Rust type. This module is the missing **reflective** path: it walks a runtime
//! `DynamicType` and reuses the SAME hardened wire framing from
//! `zerodds_cdr::struct_enc` (`decode_appendable` / `read_all_mutable_members` +
//! `encode_appendable` / `encode_mutable_member`) and the `BufferReader` /
//! `BufferWriter` primitives. No DHEADER/EMHEADER/alignment logic is
//! reimplemented — only the type-driven member walk lives here.
//!
//! Supported (both directions, byte-exact round-trip tested):
//! - primitives, enum, `string`, `wstring` (via the hardened `zerodds_cdr::WString`);
//! - struct FINAL / APPENDABLE / MUTABLE; nested struct;
//! - union FINAL / APPENDABLE / MUTABLE (PL_CDR2: discriminator = EMHEADER member
//!   id 0, selected branch = its own EMHEADER member), with the discriminator
//!   encoded at its REAL kind (bool/char/int8/16/32/64/enum — not int32);
//! - sequences + arrays of scalar AND composite (struct/union) elements, incl.
//!   the XCDR2 collection DHEADER for non-primitive elements. Composite element
//!   types are carried via [`crate::dynamic::collection`] (a resolved element
//!   `DynamicType`); scalar elements are rebuilt from the shallow descriptor.
//! - alias (typedef) members whose target is a scalar (composite typedefs are
//!   resolved to their underlying type at IDL-lowering time).
//!
//! Residual (honest `NotSupported`, never silent-wrong):
//! - `long double` / `Float128` — blocked on stable Rust `f128`;
//! - `bitmask` / `bitset` are lowered to their underlying wire integer at
//!   IDL-lowering time (so they decode correctly as integers); the codec keeps no
//!   dedicated kind for them.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::endianness::Endianness;
use zerodds_cdr::error::{DecodeError, EncodeError};
use zerodds_cdr::struct_enc::{
    decode_appendable, encode_appendable, encode_mutable_member, read_all_mutable_members,
};
use zerodds_cdr::xcdr1::{encode_pl_cdr1_member, read_all_pl_cdr1_members, write_pl_cdr1_sentinel};
use zerodds_cdr::{CdrDecode, CdrEncode, WString};

use crate::dynamic::data::{DynamicData, DynamicValue};
use crate::dynamic::descriptor::{ExtensibilityKind, TypeKind};
use crate::dynamic::type_::DynamicType;

/// Reflective-codec error.
#[derive(Debug)]
pub enum CodecError {
    /// A wire-level read/write failure from the underlying CDR layer.
    Wire(String),
    /// A `DynamicData` API failure (type mismatch, unknown member …).
    Dynamic(String),
    /// A construct the reflective path does not yet support.
    NotSupported(String),
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wire(m) => write!(f, "wire: {m}"),
            Self::Dynamic(m) => write!(f, "dynamic: {m}"),
            Self::NotSupported(m) => write!(f, "not supported: {m}"),
        }
    }
}

type R<T> = Result<T, CodecError>;

const fn endian(big: bool) -> Endianness {
    if big {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

/// Sentinel errors used to bail out of the cdr closure helpers; the real
/// [`CodecError`] is captured out-of-band and re-surfaced by the caller.
fn dec_sentinel() -> DecodeError {
    DecodeError::UnexpectedEof {
        needed: 0,
        offset: 0,
    }
}
fn enc_sentinel() -> EncodeError {
    EncodeError::ValueOutOfRange {
        message: "dynamic codec inner error",
    }
}

/// `true` for fixed-width scalar kinds (incl. enum) — collections of these get
/// NO XCDR2 collection DHEADER (mirrors `cdr::composite::needs_collection_dheader`).
const fn is_primitive_kind(kind: TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Boolean
            | TypeKind::Byte
            | TypeKind::Int8
            | TypeKind::UInt8
            | TypeKind::Int16
            | TypeKind::UInt16
            | TypeKind::Int32
            | TypeKind::UInt32
            | TypeKind::Int64
            | TypeKind::UInt64
            | TypeKind::Float32
            | TypeKind::Float64
            | TypeKind::Char8
            | TypeKind::Char16
            | TypeKind::Enumeration
    )
}

/// Build a `DynamicType` for a scalar (primitive or string) element/target
/// described by `desc`. Returns `None` for composite kinds (the documented
/// collection-of-composite / alias-to-composite residual).
fn scalar_dynamic_type(desc: &crate::dynamic::descriptor::TypeDescriptor) -> Option<DynamicType> {
    use crate::dynamic::builder::DynamicTypeBuilderFactory as F;
    match desc.kind {
        TypeKind::String8 => Some(F::create_string_type(
            desc.bound.first().copied().unwrap_or(0),
        )),
        TypeKind::String16 => Some(F::create_wstring_type(
            desc.bound.first().copied().unwrap_or(0),
        )),
        k if is_primitive_kind(k) => Some(DynamicType::new_primitive(k)),
        _ => None,
    }
}

/// The reserved EMHEADER member id of a union's discriminator in PL_CDR(2)
/// framing (DDS-XTypes 1.3 §7.2.2.4.4.4.6: `DISCRIMINATOR_ID`).
const DISCRIMINATOR_MEMBER_ID: u32 = 0;

/// Read an integral union discriminator of `kind`, returned as `i64` (signed
/// kinds are sign-extended). Mirrors the typed codec's per-kind discriminator
/// encoding — NOT hard-coded to int32.
fn read_discriminator(r: &mut BufferReader<'_>, kind: TypeKind) -> R<i64> {
    let w = |e: DecodeError| CodecError::Wire(e.to_string());
    Ok(match kind {
        TypeKind::Boolean | TypeKind::Byte | TypeKind::UInt8 | TypeKind::Char8 => {
            i64::from(r.read_u8().map_err(w)?)
        }
        TypeKind::Int8 => i64::from(r.read_u8().map_err(w)? as i8),
        TypeKind::UInt16 | TypeKind::Char16 => i64::from(r.read_u16().map_err(w)?),
        TypeKind::Int16 => i64::from(r.read_u16().map_err(w)? as i16),
        TypeKind::UInt32 | TypeKind::Enumeration => i64::from(r.read_u32().map_err(w)?),
        TypeKind::Int32 => i64::from(r.read_u32().map_err(w)? as i32),
        TypeKind::UInt64 => r.read_u64().map_err(w)? as i64,
        TypeKind::Int64 => r.read_u64().map_err(w)? as i64,
        other => {
            return Err(CodecError::NotSupported(alloc::format!(
                "union discriminator kind {other:?}"
            )));
        }
    })
}

/// Write an integral union discriminator `val` as `kind`.
fn write_discriminator(w: &mut BufferWriter, kind: TypeKind, val: i64) -> R<()> {
    let e = |x: EncodeError| CodecError::Wire(x.to_string());
    match kind {
        TypeKind::Boolean | TypeKind::Byte | TypeKind::UInt8 | TypeKind::Char8 | TypeKind::Int8 => {
            w.write_u8(val as u8).map_err(e)
        }
        TypeKind::UInt16 | TypeKind::Char16 | TypeKind::Int16 => w.write_u16(val as u16).map_err(e),
        TypeKind::UInt32 | TypeKind::Enumeration | TypeKind::Int32 => {
            w.write_u32(val as u32).map_err(e)
        }
        TypeKind::UInt64 | TypeKind::Int64 => w.write_u64(val as u64).map_err(e),
        other => Err(CodecError::NotSupported(alloc::format!(
            "union discriminator kind {other:?}"
        ))),
    }
}

/// The discriminator kind of a union type (from its descriptor), defaulting to
/// `Int32` when absent.
fn union_discriminator_kind(ty: &DynamicType) -> TypeKind {
    ty.descriptor()
        .discriminator_type
        .as_ref()
        .map_or(TypeKind::Int32, |d| d.kind)
}

// ----------------------------------------------------------------------------
// Decode
// ----------------------------------------------------------------------------

/// Decode `bytes` (a CDR payload WITHOUT the encapsulation header) into a
/// [`DynamicData`] of `ty`. `xcdr2` selects XCDR2 vs XCDR1 framing; `big_endian`
/// the byte order — both come from the sample's encapsulation id on the wire.
///
/// # Errors
/// [`CodecError`] on a wire failure, a `DynamicData` failure, or an unsupported
/// construct.
pub fn decode_dynamic(
    ty: &DynamicType,
    bytes: &[u8],
    xcdr2: bool,
    big_endian: bool,
) -> R<DynamicData> {
    let mut r = BufferReader::new(bytes, endian(big_endian));
    if xcdr2 {
        r = r.xcdr2();
    }
    decode_aggregate(ty, &mut r, xcdr2)
}

fn decode_aggregate(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicData> {
    match ty.kind() {
        TypeKind::Structure => decode_struct(ty, r, xcdr2),
        TypeKind::Union => decode_union(ty, r, xcdr2),
        other => Err(CodecError::NotSupported(alloc::format!(
            "top-level type kind {other:?} is not an aggregate"
        ))),
    }
}

fn decode_struct(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicData> {
    match ty.descriptor().extensibility_kind {
        ExtensibilityKind::Final => {
            // FINAL never carries a DHEADER — read members in declared order.
            let mut data = DynamicData::new(ty.clone());
            read_members_in_order(ty, r, xcdr2, &mut data)?;
            Ok(data)
        }
        ExtensibilityKind::Appendable => {
            // XCDR1: appendable is plain (no DHEADER), exactly like final.
            if !xcdr2 {
                let mut data = DynamicData::new(ty.clone());
                read_members_in_order(ty, r, xcdr2, &mut data)?;
                return Ok(data);
            }
            let mut inner: Option<CodecError> = None;
            let res = decode_appendable(r, |sub| {
                let mut data = DynamicData::new(ty.clone());
                match read_members_in_order(ty, sub, xcdr2, &mut data) {
                    Ok(()) => Ok(data),
                    Err(e) => {
                        inner = Some(e);
                        Err(dec_sentinel())
                    }
                }
            });
            match res {
                Ok(d) => Ok(d),
                Err(e) => Err(inner.unwrap_or_else(|| CodecError::Wire(e.to_string()))),
            }
        }
        ExtensibilityKind::Mutable => {
            if xcdr2 {
                decode_struct_mutable(ty, r, xcdr2)
            } else {
                decode_struct_pl_cdr1(ty, r)
            }
        }
    }
}

fn read_members_in_order(
    ty: &DynamicType,
    r: &mut BufferReader<'_>,
    xcdr2: bool,
    data: &mut DynamicData,
) -> R<()> {
    for i in 0..ty.member_count() {
        let m = ty
            .member_by_index(i)
            .ok_or_else(|| CodecError::Dynamic(alloc::format!("missing member index {i}")))?;
        let mt = m.dynamic_type().clone();
        let id = m.id();
        let val = read_value(&mt, r, xcdr2)?;
        data.set_value_raw(id, val);
    }
    Ok(())
}

fn decode_struct_mutable(
    ty: &DynamicType,
    r: &mut BufferReader<'_>,
    xcdr2: bool,
) -> R<DynamicData> {
    // PL_CDR2: a DHEADER frames the EMHEADER parameter list (XTypes 1.3
    // §7.4.3.4.2). `decode_appendable` strips that DHEADER and bounds the
    // sub-reader so a nested mutable member stops at its own frame — exactly
    // what the typed codec does (`decode_appendable` around
    // `read_all_mutable_members`).
    let mut err: Option<CodecError> = None;
    let res = decode_appendable(r, |sub| {
        let members = read_all_mutable_members(sub)?;
        let mut data = DynamicData::new(ty.clone());
        for mm in members {
            let Some(m) = ty.member_by_id(mm.member_id) else {
                continue; // unknown member id → skip (forward-compat)
            };
            let mt = m.dynamic_type().clone();
            let mut bsub = BufferReader::new(mm.body, sub.endianness());
            if xcdr2 {
                bsub = bsub.xcdr2();
            }
            match read_value(&mt, &mut bsub, xcdr2) {
                Ok(val) => data.set_value_raw(m.id(), val),
                Err(e) => {
                    err = Some(e);
                    return Err(dec_sentinel());
                }
            }
        }
        Ok(data)
    });
    res.map_err(|e| err.unwrap_or_else(|| CodecError::Wire(e.to_string())))
}

/// XCDR1 (`PL_CDR`) decode of a `@mutable` struct: the 16-bit-PID parameter list
/// terminated by `PID_LIST_END`. XCDR1 alignment (max-align 8) is the default of
/// the per-member sub-reader.
fn decode_struct_pl_cdr1(ty: &DynamicType, r: &mut BufferReader<'_>) -> R<DynamicData> {
    let members = read_all_pl_cdr1_members(r).map_err(|e| CodecError::Wire(e.to_string()))?;
    let mut data = DynamicData::new(ty.clone());
    for mm in members {
        let Some(m) = ty.member_by_id(mm.member_id) else {
            continue; // unknown member id → skip
        };
        let mt = m.dynamic_type().clone();
        let mut bsub = BufferReader::new(&mm.body, r.endianness());
        let val = read_value(&mt, &mut bsub, false)?;
        data.set_value_raw(m.id(), val);
    }
    Ok(data)
}

/// XCDR1 (`PL_CDR`) decode of a `@mutable` union: discriminator = PID 0, the
/// single other PID = the selected branch (matched by position, vendor-agnostic).
fn decode_union_pl_cdr1(ty: &DynamicType, r: &mut BufferReader<'_>) -> R<DynamicData> {
    let members = read_all_pl_cdr1_members(r).map_err(|e| CodecError::Wire(e.to_string()))?;
    let mut data = DynamicData::new(ty.clone());
    let disc_idx = members
        .iter()
        .position(|m| m.member_id == DISCRIMINATOR_MEMBER_ID)
        .ok_or_else(|| {
            CodecError::Wire("mutable union (PL_CDR1): no discriminator member id 0".to_string())
        })?;
    let mut dsub = BufferReader::new(&members[disc_idx].body, r.endianness());
    let disc = read_discriminator(&mut dsub, union_discriminator_kind(ty))?;

    let mut chosen = None;
    let mut default = None;
    for m in ty.members() {
        let d = m.descriptor();
        if d.is_default_label {
            default = Some(m);
        }
        if d.label.contains(&disc) {
            chosen = Some(m);
            break;
        }
    }
    if let Some(m) = chosen.or(default) {
        if let Some(bw) = members
            .iter()
            .enumerate()
            .find(|(i, _)| *i != disc_idx)
            .map(|(_, w)| w)
        {
            let mt = m.dynamic_type().clone();
            let mut bsub = BufferReader::new(&bw.body, r.endianness());
            let val = read_value(&mt, &mut bsub, false)?;
            data.set_value_raw(m.id(), val);
        }
    }
    Ok(data)
}

fn decode_union(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicData> {
    match ty.descriptor().extensibility_kind {
        ExtensibilityKind::Final => decode_union_body(ty, r, xcdr2),
        ExtensibilityKind::Appendable => {
            // XCDR1: appendable union = plain disc+branch (no DHEADER).
            if !xcdr2 {
                return decode_union_body(ty, r, xcdr2);
            }
            let mut inner: Option<CodecError> = None;
            let res = decode_appendable(r, |sub| match decode_union_body(ty, sub, xcdr2) {
                Ok(d) => Ok(d),
                Err(e) => {
                    inner = Some(e);
                    Err(dec_sentinel())
                }
            });
            match res {
                Ok(d) => Ok(d),
                Err(e) => Err(inner.unwrap_or_else(|| CodecError::Wire(e.to_string()))),
            }
        }
        ExtensibilityKind::Mutable => {
            if xcdr2 {
                decode_union_mutable(ty, r, xcdr2)
            } else {
                decode_union_pl_cdr1(ty, r)
            }
        }
    }
}

/// XTypes 1.3 PL_CDR(2) union (§7.4.3.4 / §7.2.2.4.4.4.6): a parameter list of
/// EMHEADER-framed members, exactly like a mutable struct. The discriminator is
/// the member with the reserved id [`DISCRIMINATOR_MEMBER_ID`] (0); the selected
/// branch is the member whose id matches the chosen case member. Fully
/// data-driven — branch ids are read from the wire and matched against the
/// `DynamicType` (both derived from the same TypeObject), so no id convention is
/// assumed beyond the spec-fixed discriminator id 0.
fn decode_union_mutable(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicData> {
    // PL_CDR2 frames the parameter list with a DHEADER (like a mutable struct) —
    // strip + bound it via `decode_appendable` so a nested union member stops at
    // its own frame.
    let mut err: Option<CodecError> = None;
    let res = decode_appendable(r, |sub| match decode_union_mutable_body(ty, sub, xcdr2) {
        Ok(d) => Ok(d),
        Err(e) => {
            err = Some(e);
            Err(dec_sentinel())
        }
    });
    res.map_err(|e| err.unwrap_or_else(|| CodecError::Wire(e.to_string())))
}

/// The PL_CDR2 union member walk, on a reader already scoped to the DHEADER body.
fn decode_union_mutable_body(
    ty: &DynamicType,
    r: &mut BufferReader<'_>,
    xcdr2: bool,
) -> R<DynamicData> {
    let members = read_all_mutable_members(r).map_err(|e| CodecError::Wire(e.to_string()))?;
    let mut data = DynamicData::new(ty.clone());

    // The discriminator is the member with the reserved id 0. Track its wire slot
    // so a branch that happens to also carry id 0 (an out-of-band IDL may number a
    // case 0; vendors reserve 0) is matched from the REMAINING slots.
    let disc_idx = members
        .iter()
        .position(|m| m.member_id == DISCRIMINATOR_MEMBER_ID)
        .ok_or_else(|| {
            CodecError::Wire(
                "mutable union: no discriminator member (id 0) on the wire".to_string(),
            )
        })?;
    let mut dsub = BufferReader::new(members[disc_idx].body, r.endianness());
    if xcdr2 {
        dsub = dsub.xcdr2();
    }
    let disc = read_discriminator(&mut dsub, union_discriminator_kind(ty))?;

    let mut chosen = None;
    let mut default = None;
    for m in ty.members() {
        let d = m.descriptor();
        if d.is_default_label {
            default = Some(m);
        }
        if d.label.contains(&disc) {
            chosen = Some(m);
            break;
        }
    }
    if let Some(m) = chosen.or(default) {
        // A union serializes the discriminator + at most ONE selected branch, so
        // the single non-discriminator wire member IS that branch — regardless of
        // the member id the writer assigned it. (Vendors number union cases from 1
        // after the id-0 discriminator, e.g. FastDDS emits case id 2 for the 2nd
        // case; a DynamicType built from an out-of-band IDL numbers them from 0.
        // Matching by position, not id, makes the decode vendor-agnostic.) A
        // valueless branch is simply absent.
        if let Some(bw) = members
            .iter()
            .enumerate()
            .find(|(i, _)| *i != disc_idx)
            .map(|(_, w)| w)
        {
            let mt = m.dynamic_type().clone();
            let mut bsub = BufferReader::new(bw.body, r.endianness());
            if xcdr2 {
                bsub = bsub.xcdr2();
            }
            let val = read_value(&mt, &mut bsub, xcdr2)?;
            data.set_value_raw(m.id(), val);
        }
    }
    Ok(data)
}

fn decode_union_body(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicData> {
    let disc = read_discriminator(r, union_discriminator_kind(ty))?;
    let mut data = DynamicData::new(ty.clone());
    let mut chosen = None;
    let mut default = None;
    for m in ty.members() {
        let d = m.descriptor();
        if d.is_default_label {
            default = Some(m);
        }
        if d.label.contains(&disc) {
            chosen = Some(m);
            break;
        }
    }
    if let Some(m) = chosen.or(default) {
        let mt = m.dynamic_type().clone();
        let val = read_value(&mt, r, xcdr2)?;
        data.set_value_raw(m.id(), val);
    }
    Ok(data)
}

/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn read_value(ty: &DynamicType, r: &mut BufferReader<'_>, xcdr2: bool) -> R<DynamicValue> {
    let w = |e: DecodeError| CodecError::Wire(e.to_string());
    Ok(match ty.kind() {
        TypeKind::Boolean => DynamicValue::Bool(r.read_u8().map_err(w)? != 0),
        TypeKind::Byte => DynamicValue::Byte(r.read_u8().map_err(w)?),
        TypeKind::UInt8 => DynamicValue::UInt8(r.read_u8().map_err(w)?),
        TypeKind::Int8 => DynamicValue::Int8(r.read_u8().map_err(w)? as i8),
        TypeKind::Int16 => DynamicValue::Int16(r.read_u16().map_err(w)? as i16),
        TypeKind::UInt16 => DynamicValue::UInt16(r.read_u16().map_err(w)?),
        TypeKind::Int32 => DynamicValue::Int32(r.read_u32().map_err(w)? as i32),
        TypeKind::UInt32 => DynamicValue::UInt32(r.read_u32().map_err(w)?),
        TypeKind::Enumeration => DynamicValue::Int32(r.read_u32().map_err(w)? as i32),
        TypeKind::Int64 => DynamicValue::Int64(r.read_u64().map_err(w)? as i64),
        TypeKind::UInt64 => DynamicValue::UInt64(r.read_u64().map_err(w)?),
        TypeKind::Float32 => DynamicValue::Float32(f32::from_bits(r.read_u32().map_err(w)?)),
        TypeKind::Float64 => DynamicValue::Float64(f64::from_bits(r.read_u64().map_err(w)?)),
        TypeKind::Char8 => DynamicValue::Char8(r.read_u8().map_err(w)?),
        TypeKind::Char16 => DynamicValue::Char16(r.read_u16().map_err(w)?),
        TypeKind::String8 => DynamicValue::String(r.read_string().map_err(w)?),
        TypeKind::String16 => {
            // wstring: reuse the hardened WString codec (reads BOM + no-BOM).
            let ws = WString::decode(r).map_err(w)?;
            DynamicValue::WString(ws.0.encode_utf16().collect())
        }
        TypeKind::Structure | TypeKind::Union => {
            DynamicValue::Complex(Box::new(decode_aggregate(ty, r, xcdr2)?))
        }
        TypeKind::Sequence => read_collection(ty, r, xcdr2, None)?,
        TypeKind::Array => {
            let n: usize = ty.descriptor().bound.iter().copied().product::<u32>() as usize;
            read_collection(ty, r, xcdr2, Some(n))?
        }
        TypeKind::Alias => {
            // The type model does not auto-deref typedefs; follow base_type to a
            // scalar target. Composite alias targets are the documented residual.
            let target = ty
                .descriptor()
                .base_type
                .as_ref()
                .and_then(|b| scalar_dynamic_type(b))
                .ok_or_else(|| CodecError::NotSupported("alias-to-composite target".to_string()))?;
            read_value(&target, r, xcdr2)?
        }
        other => {
            return Err(CodecError::NotSupported(alloc::format!(
                "member kind {other:?}"
            )));
        }
    })
}

/// Decode a sequence (`fixed_len == None`) or array (`Some(n)`) element-by-element.
///
/// Scalar elements (primitive/enum/string/wstring) are stored as a scalar
/// `DynamicData` with the value under member id 0; COMPOSITE elements
/// (struct/union) are stored as the decoded aggregate `DynamicData` directly —
/// both matching what `data_to_json` / the SQLite sink expect. The resolved
/// element type comes from [`crate::dynamic::collection::resolved_element`] (set
/// by the IDL lowering for composite elements), falling back to rebuilding a
/// scalar element from the shallow descriptor.
/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn read_collection(
    ty: &DynamicType,
    r: &mut BufferReader<'_>,
    xcdr2: bool,
    fixed_len: Option<usize>,
) -> R<DynamicValue> {
    let desc = ty.descriptor();
    let elem_desc = desc
        .element_type
        .as_ref()
        .ok_or_else(|| CodecError::Dynamic("collection has no element_type".to_string()))?;
    let elem_kind = elem_desc.kind;
    let elem_ty = element_type(ty, elem_desc)?;
    let w = |e: DecodeError| CodecError::Wire(e.to_string());
    if xcdr2 && !is_primitive_kind(elem_kind) {
        let _dheader = r.read_u32().map_err(w)?; // covers count+elements / elements
    }
    let count = match fixed_len {
        Some(n) => n,
        None => r.read_u32().map_err(w)? as usize,
    };
    let mut out = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let v = read_value(&elem_ty, r, xcdr2)?;
        let elem_data = match v {
            // Composite element: read_value already built the aggregate.
            DynamicValue::Complex(d) => *d,
            // Scalar element: wrap the value under member id 0.
            scalar => {
                let mut d = DynamicData::new(elem_ty.clone());
                d.set_value_raw(0, scalar);
                d
            }
        };
        out.push(elem_data);
    }
    Ok(DynamicValue::Sequence(out))
}

/// Resolve the element `DynamicType` of a collection: the fully-resolved element
/// (composite-capable) if attached, else a scalar element rebuilt from the
/// shallow descriptor.
fn element_type(
    ty: &DynamicType,
    elem_desc: &crate::dynamic::descriptor::TypeDescriptor,
) -> R<DynamicType> {
    crate::dynamic::collection::resolved_element(ty)
        .cloned()
        .or_else(|| scalar_dynamic_type(elem_desc))
        .ok_or_else(|| {
            CodecError::NotSupported(alloc::format!(
                "collection of {:?} (no resolved element type)",
                elem_desc.kind
            ))
        })
}

/// Encode a sequence/array, mirroring [`read_collection`] (count for sequences;
/// XCDR2 collection DHEADER for non-primitive elements; composite elements
/// written as aggregates, scalars from member id 0).
/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn write_collection(
    items: &[DynamicData],
    ty: &DynamicType,
    w: &mut BufferWriter,
    xcdr2: bool,
) -> R<()> {
    let desc = ty.descriptor();
    let elem_desc = desc
        .element_type
        .as_ref()
        .ok_or_else(|| CodecError::Dynamic("collection has no element_type".to_string()))?;
    let elem_kind = elem_desc.kind;
    let elem_ty = element_type(ty, elem_desc)?;
    let composite = matches!(elem_ty.kind(), TypeKind::Structure | TypeKind::Union);
    let is_array = ty.kind() == TypeKind::Array;
    let e = |x: EncodeError| CodecError::Wire(x.to_string());

    let write_body = |w: &mut BufferWriter| -> R<()> {
        if !is_array {
            w.write_u32(items.len() as u32).map_err(e)?;
        }
        for d in items {
            if composite {
                write_aggregate(d, w, xcdr2)?;
            } else {
                let v = d
                    .get_value(0)
                    .ok_or_else(|| CodecError::Dynamic("scalar element unset".to_string()))?;
                write_value(&v.clone(), &elem_ty, w, xcdr2)?;
            }
        }
        Ok(())
    };

    if xcdr2 && !is_primitive_kind(elem_kind) {
        // DHEADER = byte length of the body; encode to a sub-writer, prepend.
        let mut sub = BufferWriter::new(w.endianness());
        if xcdr2 {
            sub = sub.xcdr2();
        }
        write_body(&mut sub)?;
        let body = sub.into_bytes();
        w.write_u32(body.len() as u32).map_err(e)?;
        w.write_bytes(&body).map_err(e)?;
        Ok(())
    } else {
        write_body(w)
    }
}

// ----------------------------------------------------------------------------
// Encode (mirror)
// ----------------------------------------------------------------------------

/// Encode `data` to a CDR payload (WITHOUT the encapsulation header), the mirror
/// of [`decode_dynamic`]: `encode_dynamic(decode_dynamic(ty, b, x, e)?, x, e)? == b`.
///
/// # Errors
/// [`CodecError`].
pub fn encode_dynamic(data: &DynamicData, xcdr2: bool, big_endian: bool) -> R<Vec<u8>> {
    let mut w = BufferWriter::new(endian(big_endian));
    if xcdr2 {
        w = w.xcdr2();
    }
    write_aggregate(data, &mut w, xcdr2)?;
    Ok(w.into_bytes())
}

fn write_aggregate(data: &DynamicData, w: &mut BufferWriter, xcdr2: bool) -> R<()> {
    let ty = data.dynamic_type().clone();
    match ty.kind() {
        TypeKind::Structure => write_struct(data, &ty, w, xcdr2),
        TypeKind::Union => write_union(data, &ty, w, xcdr2),
        other => Err(CodecError::NotSupported(alloc::format!(
            "top-level type kind {other:?} is not an aggregate"
        ))),
    }
}

fn write_struct(data: &DynamicData, ty: &DynamicType, w: &mut BufferWriter, xcdr2: bool) -> R<()> {
    match ty.descriptor().extensibility_kind {
        ExtensibilityKind::Final => write_members_in_order(data, ty, w, xcdr2),
        ExtensibilityKind::Appendable => {
            // XCDR1 (classic CDR): an @appendable struct is serialized exactly
            // like @final — plain, no DHEADER (the DHEADER frame is XCDR2-only).
            if !xcdr2 {
                return write_members_in_order(data, ty, w, xcdr2);
            }
            let mut inner: Option<CodecError> = None;
            let res = encode_appendable(w, |sub| {
                match write_members_in_order(data, ty, sub, xcdr2) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        inner = Some(e);
                        Err(enc_sentinel())
                    }
                }
            });
            match res {
                Ok(()) => Ok(()),
                Err(e) => Err(inner.unwrap_or_else(|| CodecError::Wire(e.to_string()))),
            }
        }
        ExtensibilityKind::Mutable => {
            // XCDR1: a @mutable struct is PL_CDR1 — a 16-bit-PID/length parameter
            // list terminated by PID_LIST_END (not the XCDR2 EMHEADER/DHEADER).
            if !xcdr2 {
                return write_struct_pl_cdr1(data, ty, w);
            }
            // PL_CDR2: the EMHEADER parameter list is framed by a DHEADER (so a
            // nested mutable member is self-delimiting). Mirror the typed codec,
            // which wraps `encode_mutable_member`s in `encode_appendable`.
            let mut outer: Option<CodecError> = None;
            let res = encode_appendable(w, |fw| {
                for i in 0..ty.member_count() {
                    let m = match ty.member_by_index(i).ok_or_else(|| {
                        CodecError::Dynamic(alloc::format!("missing member index {i}"))
                    }) {
                        Ok(m) => m,
                        Err(e) => {
                            outer = Some(e);
                            return Err(enc_sentinel());
                        }
                    };
                    let id = m.id();
                    let Some(val) = data.get_value(id).cloned() else {
                        continue;
                    };
                    let mt = m.dynamic_type().clone();
                    let mu = m.descriptor().is_must_understand;
                    let res = encode_mutable_member(fw, id, mu, |sub| {
                        match write_value(&val, &mt, sub, xcdr2) {
                            Ok(()) => Ok(()),
                            Err(e) => {
                                outer = Some(e);
                                Err(enc_sentinel())
                            }
                        }
                    });
                    if let Err(e) = res {
                        if outer.is_none() {
                            outer = Some(CodecError::Wire(e.to_string()));
                        }
                        return Err(enc_sentinel());
                    }
                }
                Ok(())
            });
            res.map_err(|e| outer.unwrap_or_else(|| CodecError::Wire(e.to_string())))
        }
    }
}

fn write_members_in_order(
    data: &DynamicData,
    ty: &DynamicType,
    w: &mut BufferWriter,
    xcdr2: bool,
) -> R<()> {
    for i in 0..ty.member_count() {
        let m = ty
            .member_by_index(i)
            .ok_or_else(|| CodecError::Dynamic(alloc::format!("missing member index {i}")))?;
        let id = m.id();
        let val = data
            .get_value(id)
            .ok_or_else(|| CodecError::Dynamic(alloc::format!("member {id} unset on encode")))?
            .clone();
        let mt = m.dynamic_type().clone();
        write_value(&val, &mt, w, xcdr2)?;
    }
    Ok(())
}

/// XCDR1 (`PL_CDR`) encode of a `@mutable` struct: each set member as a
/// 16-bit-PID/length parameter, terminated by `PID_LIST_END` (XTypes 1.3
/// §7.4.1.2). Mirrors the typed codec (`encode_pl_cdr1_member` + sentinel).
fn write_struct_pl_cdr1(data: &DynamicData, ty: &DynamicType, w: &mut BufferWriter) -> R<()> {
    for i in 0..ty.member_count() {
        let m = ty
            .member_by_index(i)
            .ok_or_else(|| CodecError::Dynamic(alloc::format!("missing member index {i}")))?;
        let id = m.id();
        let Some(val) = data.get_value(id).cloned() else {
            continue;
        };
        let mt = m.dynamic_type().clone();
        let mut err: Option<CodecError> = None;
        let res = encode_pl_cdr1_member(w, id, |sub| match write_value(&val, &mt, sub, false) {
            Ok(()) => Ok(()),
            Err(e) => {
                err = Some(e);
                Err(enc_sentinel())
            }
        });
        if let Err(e) = res {
            return Err(err.unwrap_or_else(|| CodecError::Wire(e.to_string())));
        }
    }
    write_pl_cdr1_sentinel(w).map_err(|e| CodecError::Wire(e.to_string()))
}

/// XCDR1 (`PL_CDR`) encode of a `@mutable` union: discriminator as PID 0, then
/// the selected branch as its own PID, terminated by `PID_LIST_END`.
fn write_union_pl_cdr1(data: &DynamicData, ty: &DynamicType, w: &mut BufferWriter) -> R<()> {
    let disc_kind = union_discriminator_kind(ty);
    let m = ty
        .members()
        .find(|m| data.get_value(m.id()).is_some())
        .ok_or_else(|| CodecError::Dynamic("union has no branch set".to_string()))?;
    let disc = m.descriptor().label.first().copied().unwrap_or(0);
    let val = data
        .get_value(m.id())
        .cloned()
        .ok_or_else(|| CodecError::Dynamic("union branch value vanished".to_string()))?;
    let mt = m.dynamic_type().clone();

    let mut err: Option<CodecError> = None;
    let r1 = encode_pl_cdr1_member(w, DISCRIMINATOR_MEMBER_ID, |sub| match write_discriminator(
        sub, disc_kind, disc,
    ) {
        Ok(()) => Ok(()),
        Err(e) => {
            err = Some(e);
            Err(enc_sentinel())
        }
    });
    if let Err(e) = r1 {
        return Err(err.unwrap_or_else(|| CodecError::Wire(e.to_string())));
    }
    let r2 = encode_pl_cdr1_member(w, m.id(), |sub| match write_value(&val, &mt, sub, false) {
        Ok(()) => Ok(()),
        Err(e) => {
            err = Some(e);
            Err(enc_sentinel())
        }
    });
    if let Err(e) = r2 {
        return Err(err.unwrap_or_else(|| CodecError::Wire(e.to_string())));
    }
    write_pl_cdr1_sentinel(w).map_err(|e| CodecError::Wire(e.to_string()))
}

fn write_union(data: &DynamicData, ty: &DynamicType, w: &mut BufferWriter, xcdr2: bool) -> R<()> {
    match ty.descriptor().extensibility_kind {
        ExtensibilityKind::Final => write_union_body(data, ty, w, xcdr2),
        ExtensibilityKind::Appendable => {
            // XCDR1: appendable union = plain disc+branch (no DHEADER), like final.
            if !xcdr2 {
                return write_union_body(data, ty, w, xcdr2);
            }
            let mut inner: Option<CodecError> = None;
            let res = encode_appendable(w, |sub| match write_union_body(data, ty, sub, xcdr2) {
                Ok(()) => Ok(()),
                Err(e) => {
                    inner = Some(e);
                    Err(enc_sentinel())
                }
            });
            match res {
                Ok(()) => Ok(()),
                Err(e) => Err(inner.unwrap_or_else(|| CodecError::Wire(e.to_string()))),
            }
        }
        ExtensibilityKind::Mutable => {
            if xcdr2 {
                write_union_mutable(data, ty, w, xcdr2)
            } else {
                write_union_pl_cdr1(data, ty, w)
            }
        }
    }
}

fn write_union_body(
    data: &DynamicData,
    ty: &DynamicType,
    w: &mut BufferWriter,
    xcdr2: bool,
) -> R<()> {
    let disc_kind = union_discriminator_kind(ty);
    for m in ty.members() {
        let id = m.id();
        if let Some(val) = data.get_value(id).cloned() {
            let disc = m.descriptor().label.first().copied().unwrap_or(0);
            write_discriminator(w, disc_kind, disc)?;
            let mt = m.dynamic_type().clone();
            return write_value(&val, &mt, w, xcdr2);
        }
    }
    Err(CodecError::Dynamic("union has no branch set".to_string()))
}

/// PL_CDR(2) union encode (inverse of [`decode_union_mutable`]): the
/// discriminator as EMHEADER member [`DISCRIMINATOR_MEMBER_ID`] (always
/// must-understand), then the selected branch as its own EMHEADER member. The
/// parameter list is bounded by the enclosing frame, exactly like a mutable
/// struct (`encode_mutable_member` writes no outer DHEADER itself).
fn write_union_mutable(
    data: &DynamicData,
    ty: &DynamicType,
    w: &mut BufferWriter,
    xcdr2: bool,
) -> R<()> {
    let disc_kind = union_discriminator_kind(ty);
    let m = ty
        .members()
        .find(|m| data.get_value(m.id()).is_some())
        .ok_or_else(|| CodecError::Dynamic("union has no branch set".to_string()))?;
    let disc = m.descriptor().label.first().copied().unwrap_or(0);
    let val = data
        .get_value(m.id())
        .cloned()
        .ok_or_else(|| CodecError::Dynamic("union branch value vanished".to_string()))?;
    let mt = m.dynamic_type().clone();

    let mu = m.descriptor().is_must_understand;
    // PL_CDR2: DHEADER-framed parameter list — (1) discriminator as reserved
    // EMHEADER member 0 (must-understand), (2) the selected branch as its own
    // EMHEADER member. The DHEADER comes from `encode_appendable`.
    let mut err: Option<CodecError> = None;
    let res =
        encode_appendable(w, |fw| {
            let r1 = encode_mutable_member(fw, DISCRIMINATOR_MEMBER_ID, true, |sub| {
                match write_discriminator(sub, disc_kind, disc) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        err = Some(e);
                        Err(enc_sentinel())
                    }
                }
            });
            if let Err(e) = r1 {
                if err.is_none() {
                    err = Some(CodecError::Wire(e.to_string()));
                }
                return Err(enc_sentinel());
            }
            let r2 = encode_mutable_member(fw, m.id(), mu, |sub| {
                match write_value(&val, &mt, sub, xcdr2) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        err = Some(e);
                        Err(enc_sentinel())
                    }
                }
            });
            if let Err(e) = r2 {
                if err.is_none() {
                    err = Some(CodecError::Wire(e.to_string()));
                }
                return Err(enc_sentinel());
            }
            Ok(())
        });
    res.map_err(|e| err.unwrap_or_else(|| CodecError::Wire(e.to_string())))
}

/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn write_value(v: &DynamicValue, ty: &DynamicType, w: &mut BufferWriter, xcdr2: bool) -> R<()> {
    let e = |x: EncodeError| CodecError::Wire(x.to_string());
    match v {
        DynamicValue::Bool(b) => w.write_u8(u8::from(*b)).map_err(e),
        DynamicValue::Byte(x) | DynamicValue::UInt8(x) | DynamicValue::Char8(x) => {
            w.write_u8(*x).map_err(e)
        }
        DynamicValue::Int8(x) => w.write_u8(*x as u8).map_err(e),
        DynamicValue::Int16(x) => w.write_u16(*x as u16).map_err(e),
        DynamicValue::UInt16(x) | DynamicValue::Char16(x) => w.write_u16(*x).map_err(e),
        DynamicValue::Int32(x) => w.write_u32(*x as u32).map_err(e),
        DynamicValue::UInt32(x) => w.write_u32(*x).map_err(e),
        DynamicValue::Int64(x) => w.write_u64(*x as u64).map_err(e),
        DynamicValue::UInt64(x) => w.write_u64(*x).map_err(e),
        DynamicValue::Float32(x) => w.write_u32(x.to_bits()).map_err(e),
        DynamicValue::Float64(x) => w.write_u64(x.to_bits()).map_err(e),
        DynamicValue::String(s) => w.write_string(s).map_err(e),
        DynamicValue::WString(units) => {
            // wstring: reuse the hardened WString codec (global BOM policy —
            // matches what zerodds typed writers emit).
            WString(String::from_utf16_lossy(units))
                .encode(w)
                .map_err(e)
        }
        DynamicValue::Complex(d) => write_aggregate(d, w, xcdr2),
        DynamicValue::Sequence(items) => write_collection(items, ty, w, xcdr2),
        DynamicValue::None => Ok(()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dynamic::builder::DynamicTypeBuilderFactory;
    use crate::dynamic::descriptor::{MemberDescriptor, TypeDescriptor};

    /// Build a struct type `{ a: int32, b: uint64, s: string, f: float64 }` with
    /// the given extensibility.
    fn sample_type(ext: ExtensibilityKind) -> DynamicType {
        let mut desc = TypeDescriptor::structure("Sample");
        desc.extensibility_kind = ext;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).expect("builder");
        b.add_struct_member("a", 0, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        b.add_struct_member(
            "b",
            1,
            TypeDescriptor::primitive(TypeKind::UInt64, "uint64"),
        )
        .unwrap();
        let string_desc = DynamicTypeBuilderFactory::create_string_type(255)
            .descriptor()
            .clone();
        b.add_struct_member("s", 2, string_desc).unwrap();
        b.add_struct_member(
            "f",
            3,
            TypeDescriptor::primitive(TypeKind::Float64, "float64"),
        )
        .unwrap();
        b.build().expect("build")
    }

    fn populate(ty: &DynamicType) -> DynamicData {
        let mut d = DynamicData::new(ty.clone());
        d.set_int32_value(0, -7).unwrap();
        d.set_uint64_value(1, 0xDEAD_BEEF_0000_0001).unwrap();
        d.set_string_value(2, "hello-dynamic").unwrap();
        d.set_float64_value(3, 3.5).unwrap();
        d
    }

    fn roundtrip(ext: ExtensibilityKind, xcdr2: bool) {
        let ty = sample_type(ext);
        let src = populate(&ty);
        let bytes = encode_dynamic(&src, xcdr2, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, xcdr2, false).expect("decode");
        // Field values survive.
        assert_eq!(back.get_int32_value(0).unwrap(), -7);
        assert_eq!(back.get_uint64_value(1).unwrap(), 0xDEAD_BEEF_0000_0001);
        assert_eq!(back.get_string_value(2).unwrap(), "hello-dynamic");
        assert!((back.get_float64_value(3).unwrap() - 3.5).abs() < f64::EPSILON);
        // Byte-exact round-trip: re-encoding the decoded value reproduces the wire.
        let bytes2 = encode_dynamic(&back, xcdr2, false).expect("re-encode");
        assert_eq!(
            bytes, bytes2,
            "byte-exact round-trip failed for {ext:?} xcdr2={xcdr2}"
        );
    }

    #[test]
    fn roundtrip_final_xcdr2() {
        roundtrip(ExtensibilityKind::Final, true);
    }
    #[test]
    fn roundtrip_final_xcdr1() {
        roundtrip(ExtensibilityKind::Final, false);
    }
    #[test]
    fn roundtrip_appendable_xcdr2() {
        roundtrip(ExtensibilityKind::Appendable, true);
    }
    #[test]
    fn roundtrip_mutable_xcdr2() {
        roundtrip(ExtensibilityKind::Mutable, true);
    }

    /// Union round-trips: discriminator selects the branch, the branch value
    /// survives, and re-encode is byte-exact.
    #[test]
    fn roundtrip_union() {
        let disc = TypeDescriptor::primitive(TypeKind::Int32, "int32");
        let mut b = DynamicTypeBuilderFactory::create_union("U", disc).expect("union builder");
        // case 1 → int32 a
        let mut m1 =
            MemberDescriptor::new("a", 0, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        m1.index = 0;
        m1.label = alloc::vec![1];
        b.add_member_resolved(m1, DynamicType::new_primitive(TypeKind::Int32))
            .unwrap();
        // case 2 → uint32 u
        let mut m2 = MemberDescriptor::new(
            "u",
            1,
            TypeDescriptor::primitive(TypeKind::UInt32, "uint32"),
        );
        m2.index = 1;
        m2.label = alloc::vec![2];
        b.add_member_resolved(m2, DynamicType::new_primitive(TypeKind::UInt32))
            .unwrap();
        let u = b.build().expect("build union");

        // Select case 2 by setting member id 1.
        let mut d = DynamicData::new(u.clone());
        d.set_uint32_value(1, 0xABCD).unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&u, &bytes, true, false).expect("decode");
        assert_eq!(back.get_uint32_value(1).unwrap(), 0xABCD);
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }

    /// MUTABLE (PL_CDR2) union: the discriminator is EMHEADER member id 0, the
    /// selected branch is its own EMHEADER member. Round-trips both a branch with
    /// id 0 (exercises the discriminator/branch id-0 collision handling) and a
    /// branch with id 1, byte-exact re-encode each time.
    #[test]
    fn roundtrip_union_mutable() {
        let build = || {
            let mut ud =
                TypeDescriptor::union("UM", TypeDescriptor::primitive(TypeKind::Int32, "int32"));
            ud.extensibility_kind = ExtensibilityKind::Mutable;
            let mut b = DynamicTypeBuilderFactory::create_type(ud).expect("union");
            let mut m1 =
                MemberDescriptor::new("a", 0, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
            m1.index = 0;
            m1.label = alloc::vec![1];
            b.add_member_resolved(m1, DynamicType::new_primitive(TypeKind::Int32))
                .unwrap();
            let mut m2 = MemberDescriptor::new(
                "u",
                1,
                TypeDescriptor::primitive(TypeKind::UInt32, "uint32"),
            );
            m2.index = 1;
            m2.label = alloc::vec![2];
            b.add_member_resolved(m2, DynamicType::new_primitive(TypeKind::UInt32))
                .unwrap();
            b.build().expect("build union")
        };

        // case "a" (member id 0) — collides with the discriminator's reserved id.
        let u = build();
        let mut d = DynamicData::new(u.clone());
        d.set_int32_value(0, -42).unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode a");
        let back = decode_dynamic(&u, &bytes, true, false).expect("decode a");
        assert_eq!(back.get_int32_value(0).unwrap(), -42);
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);

        // case "u" (member id 1).
        let mut d2 = DynamicData::new(u.clone());
        d2.set_uint32_value(1, 0xDEAD_BEEF).unwrap();
        let bytes2 = encode_dynamic(&d2, true, false).expect("encode u");
        let back2 = decode_dynamic(&u, &bytes2, true, false).expect("decode u");
        assert_eq!(back2.get_uint32_value(1).unwrap(), 0xDEAD_BEEF);
        assert_eq!(encode_dynamic(&back2, true, false).unwrap(), bytes2);
    }

    /// (B) Non-int32 union discriminator: an int16-discriminated union with an
    /// int8 branch must encode the discriminator as 2 bytes (not 4) — guards the
    /// latent hard-coded-int32 bug the symmetric round-trip alone would mask.
    #[test]
    fn union_int16_discriminator_is_two_bytes() {
        let mut ud =
            TypeDescriptor::union("U16", TypeDescriptor::primitive(TypeKind::Int16, "int16"));
        ud.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(ud).expect("union");
        let mut m =
            MemberDescriptor::new("a", 0, TypeDescriptor::primitive(TypeKind::Int8, "int8"));
        m.index = 0;
        m.label = alloc::vec![10];
        b.add_member_resolved(m, DynamicType::new_primitive(TypeKind::Int8))
            .unwrap();
        let u = b.build().expect("build");

        let mut d = DynamicData::new(u.clone());
        d.set_int8_value(0, 7).unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode");
        // [disc i16 LE = 0x000a][int8 = 0x07] => exactly 3 bytes.
        assert_eq!(
            bytes,
            alloc::vec![0x0a, 0x00, 0x07],
            "discriminator must be 2 bytes"
        );
        let back = decode_dynamic(&u, &bytes, true, false).expect("decode");
        assert_eq!(back.get_int8_value(0).unwrap(), 7);
    }

    // Primitives have no member id 0, so the validating typed setters reject them;
    // scalar elements store their value raw (the codec does the same internally).
    fn scalar_i32(v: i32) -> DynamicData {
        let mut d = DynamicData::new(DynamicType::new_primitive(TypeKind::Int32));
        d.set_value_raw(0, DynamicValue::Int32(v));
        d
    }

    /// (A) sequence<int32> round-trips.
    #[test]
    fn roundtrip_seq_int32() {
        let mut desc = TypeDescriptor::structure("S");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let seq = TypeDescriptor::sequence(
            "seq",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            0,
        );
        b.add_struct_member("xs", 0, seq).unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let items = alloc::vec![scalar_i32(11), scalar_i32(-22), scalar_i32(33),];
        d.set_sequence_value(0, items).unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, true, false).expect("decode");
        let DynamicValue::Sequence(v) = back.get_value(0).unwrap() else {
            panic!("seq")
        };
        let got: alloc::vec::Vec<i32> = v.iter().map(|e| e.get_int32_value(0).unwrap()).collect();
        assert_eq!(got, alloc::vec![11, -22, 33]);
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }

    /// (A) sequence<string> round-trips (exercises the XCDR2 collection DHEADER).
    #[test]
    fn roundtrip_seq_string() {
        let mut desc = TypeDescriptor::structure("S");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let strd = DynamicTypeBuilderFactory::create_string_type(255)
            .descriptor()
            .clone();
        let seq = TypeDescriptor::sequence("seq", strd, 0);
        b.add_struct_member("ss", 0, seq).unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let mk = |s: &str| {
            let mut e = DynamicData::new(DynamicTypeBuilderFactory::create_string_type(255));
            e.set_value_raw(0, DynamicValue::String(s.into()));
            e
        };
        d.set_sequence_value(0, alloc::vec![mk("a"), mk("bb"), mk("ccc")])
            .unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, true, false).expect("decode");
        let DynamicValue::Sequence(v) = back.get_value(0).unwrap() else {
            panic!("seq")
        };
        let got: alloc::vec::Vec<alloc::string::String> =
            v.iter().map(|e| e.get_string_value(0).unwrap()).collect();
        assert_eq!(
            got,
            alloc::vec!["a".to_string(), "bb".to_string(), "ccc".to_string()]
        );
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }

    /// (A) fixed array int32[3] round-trips (no count prefix).
    #[test]
    fn roundtrip_array_int32() {
        let mut desc = TypeDescriptor::structure("S");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let arr = TypeDescriptor::array(
            "arr",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            alloc::vec![3],
        );
        b.add_struct_member("a", 0, arr).unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let items = alloc::vec![scalar_i32(7), scalar_i32(8), scalar_i32(9),];
        d.set_sequence_value(0, items).unwrap();
        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, true, false).expect("decode");
        let DynamicValue::Sequence(v) = back.get_value(0).unwrap() else {
            panic!("arr")
        };
        let got: alloc::vec::Vec<i32> = v.iter().map(|e| e.get_int32_value(0).unwrap()).collect();
        assert_eq!(got, alloc::vec![7, 8, 9]);
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }

    /// Composite-element collection: `sequence<Inner>` round-trips, each element
    /// is the decoded aggregate, and re-encode is byte-exact (XCDR2 collection
    /// DHEADER + per-element appendable DHEADERs).
    #[test]
    fn roundtrip_seq_of_struct() {
        use crate::dynamic::collection;
        let inner = sample_type(ExtensibilityKind::Final);
        let seq_ty = collection::sequence_of(inner.clone(), 0);
        let mut desc = TypeDescriptor::structure("S");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let mut md = MemberDescriptor::new("xs", 0, seq_ty.descriptor().clone());
        md.index = 0;
        b.add_member_resolved(md, seq_ty).unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let e0 = populate(&inner);
        let mut e1 = DynamicData::new(inner.clone());
        e1.set_int32_value(0, 123).unwrap();
        e1.set_uint64_value(1, 7).unwrap();
        e1.set_string_value(2, "second").unwrap();
        e1.set_float64_value(3, -1.25).unwrap();
        d.set_sequence_value(0, alloc::vec![e0, e1]).unwrap();

        for xcdr2 in [true, false] {
            let bytes = encode_dynamic(&d, xcdr2, false).expect("encode");
            let back = decode_dynamic(&ty, &bytes, xcdr2, false).expect("decode");
            let DynamicValue::Sequence(v) = back.get_value(0).unwrap() else {
                panic!("seq")
            };
            assert_eq!(v.len(), 2);
            // Composite element = the aggregate itself (kind Structure).
            assert_eq!(v[0].dynamic_type().kind(), TypeKind::Structure);
            assert_eq!(v[0].get_string_value(2).unwrap(), "hello-dynamic");
            assert_eq!(v[1].get_int32_value(0).unwrap(), 123);
            assert_eq!(v[1].get_string_value(2).unwrap(), "second");
            assert_eq!(
                encode_dynamic(&back, xcdr2, false).unwrap(),
                bytes,
                "byte-exact seq<struct> xcdr2={xcdr2}"
            );
        }
    }

    /// Composite-element fixed array `Inner[2]` round-trips (no count prefix).
    #[test]
    fn roundtrip_array_of_struct() {
        use crate::dynamic::collection;
        let inner = sample_type(ExtensibilityKind::Final);
        let arr_ty = collection::array_of(inner.clone(), alloc::vec![2], "arr");
        let mut desc = TypeDescriptor::structure("S");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let mut md = MemberDescriptor::new("a", 0, arr_ty.descriptor().clone());
        md.index = 0;
        b.add_member_resolved(md, arr_ty).unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let mut e0 = DynamicData::new(inner.clone());
        e0.set_int32_value(0, 1).unwrap();
        e0.set_uint64_value(1, 1).unwrap();
        e0.set_string_value(2, "x").unwrap();
        e0.set_float64_value(3, 1.0).unwrap();
        let mut e1 = DynamicData::new(inner.clone());
        e1.set_int32_value(0, 2).unwrap();
        e1.set_uint64_value(1, 2).unwrap();
        e1.set_string_value(2, "y").unwrap();
        e1.set_float64_value(3, 2.0).unwrap();
        d.set_sequence_value(0, alloc::vec![e0, e1]).unwrap();

        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, true, false).expect("decode");
        let DynamicValue::Sequence(v) = back.get_value(0).unwrap() else {
            panic!("arr")
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].get_string_value(2).unwrap(), "x");
        assert_eq!(v[1].get_int32_value(0).unwrap(), 2);
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }

    /// `wstring` member round-trips (via the hardened `WString` codec).
    #[test]
    fn roundtrip_wstring() {
        let mut desc = TypeDescriptor::structure("W");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let wdesc = DynamicTypeBuilderFactory::create_wstring_type(0)
            .descriptor()
            .clone();
        let mut md = MemberDescriptor::new("w", 0, wdesc);
        md.index = 0;
        b.add_member_resolved(md, DynamicTypeBuilderFactory::create_wstring_type(0))
            .unwrap();
        let ty = b.build().unwrap();

        let mut d = DynamicData::new(ty.clone());
        let units: alloc::vec::Vec<u16> = "wïde→".encode_utf16().collect();
        d.set_value_raw(0, DynamicValue::WString(units.clone()));
        for xcdr2 in [true, false] {
            let bytes = encode_dynamic(&d, xcdr2, false).expect("encode");
            let back = decode_dynamic(&ty, &bytes, xcdr2, false).expect("decode");
            let DynamicValue::WString(got) = back.get_value(0).unwrap() else {
                panic!("wstring")
            };
            assert_eq!(*got, units);
            assert_eq!(encode_dynamic(&back, xcdr2, false).unwrap(), bytes);
        }
    }

    /// Nested struct member round-trips (the member is a full DynamicType).
    #[test]
    fn roundtrip_nested_struct() {
        let inner = sample_type(ExtensibilityKind::Final);
        let mut desc = TypeDescriptor::structure("Outer");
        desc.extensibility_kind = ExtensibilityKind::Final;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).expect("builder");
        b.add_struct_member(
            "id",
            0,
            TypeDescriptor::primitive(TypeKind::UInt32, "uint32"),
        )
        .unwrap();
        let mut md = MemberDescriptor::new("inner", 1, inner.descriptor().clone());
        md.index = 1;
        b.add_member_resolved(md, inner.clone()).unwrap();
        let outer = b.build().expect("build outer");

        let mut d = DynamicData::new(outer.clone());
        d.set_uint32_value(0, 99).unwrap();
        d.set_complex_value(1, populate(&inner)).unwrap();

        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&outer, &bytes, true, false).expect("decode");
        assert_eq!(back.get_uint32_value(0).unwrap(), 99);
        let inner_back = back.get_complex_value(1).unwrap();
        assert_eq!(inner_back.get_string_value(2).unwrap(), "hello-dynamic");
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }
}
