// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Shape-driven CDR codec for flat structs (XCDR1 + XCDR2, `@final` /
//! `@appendable`).
//!
//! Operates on the CDR **body** (without the encapsulation header) exactly as
//! the runtime user byte-path delivers it, mirroring the `zerodds-wire` body
//! layout (align origin 0; max alignment 8 for XCDR1, 4 for XCDR2;
//! Cyclone/FastDDS byte-compatible). The byte order is the one the
//! encapsulation header of the sample named (DDSI-RTPS 2.5 §10.5, DDS-XTypes
//! 1.3 §7.4.1, "the endianness of the serialized data"); a body is written
//! back in the order it was read in.
//!
//! The XCDR2 alignment cap is the reader's and the writer's own
//! `max_align`, set at construction; `zerodds-wire` has no `.xcdr2()`
//! toggle because the cap is the only thing the two encodings differ in
//! here. The writer cannot fail — every `write_*` returns `()` — so the
//! encode path carries error returns only for the value/kind mismatch it
//! detects itself.

use zerodds_wire::{Endianness, Reader, Writer};

/// XCDR1 caps alignment at 8, XCDR2 at 4 — XTypes 1.3 §7.4.2.
const XCDR1_MAX_ALIGN: usize = 8;
/// See [`XCDR1_MAX_ALIGN`].
const XCDR2_MAX_ALIGN: usize = 4;

use super::shape::{ScalarKind, TypeShape};
use crate::error::RoutingError;

/// A decoded scalar value.
#[derive(Debug, Clone, PartialEq)]
pub enum DynValue {
    /// boolean
    Bool(bool),
    /// uint8 / octet
    U8(u8),
    /// int8
    I8(i8),
    /// uint16
    U16(u16),
    /// int16
    I16(i16),
    /// uint32
    U32(u32),
    /// int32
    I32(i32),
    /// uint64
    U64(u64),
    /// int64
    I64(i64),
    /// float
    F32(f32),
    /// double
    F64(f64),
    /// string
    Str(String),
}

fn dec_err(what: &str) -> RoutingError {
    RoutingError::Transform {
        route: "<decode>".into(),
        reason: format!("cdr decode: {what}"),
    }
}

fn enc_err(what: &str) -> RoutingError {
    RoutingError::Transform {
        route: "<encode>".into(),
        reason: format!("cdr encode: {what}"),
    }
}

fn align_for(kind: ScalarKind, xcdr2: bool) -> usize {
    if xcdr2 {
        kind.xcdr2_align()
    } else {
        kind.natural_align()
    }
}

/// Decodes a body of `xcdr2` (else XCDR1) in `endianness` into the shape's
/// ordered values.
///
/// # Errors
/// [`RoutingError::Transform`] on truncated/invalid input.
pub fn decode(
    shape: &TypeShape,
    body: &[u8],
    xcdr2: bool,
    endianness: Endianness,
) -> crate::error::Result<Vec<DynValue>> {
    let max_align = if xcdr2 {
        XCDR2_MAX_ALIGN
    } else {
        XCDR1_MAX_ALIGN
    };
    let mut r = Reader::with_max_align(body, endianness, max_align);
    if xcdr2 && shape.appendable {
        // DHEADER (object size) — read and discard; members follow.
        r.read_u32().map_err(|_| dec_err("dheader"))?;
    }
    let mut out = Vec::with_capacity(shape.members.len());
    for m in &shape.members {
        let a = align_for(m.kind, xcdr2);
        r.align_to(a).map_err(|_| dec_err("align"))?;
        let v = match m.kind {
            ScalarKind::Bool => DynValue::Bool(r.read_u8().map_err(|_| dec_err("bool"))? != 0),
            ScalarKind::U8 => DynValue::U8(r.read_u8().map_err(|_| dec_err("u8"))?),
            ScalarKind::I8 => DynValue::I8(r.read_i8().map_err(|_| dec_err("i8"))?),
            ScalarKind::U16 => DynValue::U16(r.read_u16().map_err(|_| dec_err("u16"))?),
            ScalarKind::I16 => DynValue::I16(r.read_i16().map_err(|_| dec_err("i16"))?),
            ScalarKind::U32 => DynValue::U32(r.read_u32().map_err(|_| dec_err("u32"))?),
            ScalarKind::I32 => DynValue::I32(r.read_i32().map_err(|_| dec_err("i32"))?),
            ScalarKind::U64 => DynValue::U64(r.read_u64().map_err(|_| dec_err("u64"))?),
            ScalarKind::I64 => DynValue::I64(r.read_i64().map_err(|_| dec_err("i64"))?),
            ScalarKind::F32 => DynValue::F32(r.read_f32().map_err(|_| dec_err("f32"))?),
            ScalarKind::F64 => DynValue::F64(r.read_f64().map_err(|_| dec_err("f64"))?),
            ScalarKind::String => DynValue::Str(r.read_string().map_err(|_| dec_err("string"))?),
        };
        out.push(v);
    }
    Ok(out)
}

/// Encodes the shape's values back into a CDR body of `xcdr2` (else
/// XCDR1) in `endianness`.
///
/// # Errors
/// [`RoutingError::Transform`] on a value/kind mismatch.
pub fn encode(
    shape: &TypeShape,
    values: &[DynValue],
    xcdr2: bool,
    endianness: Endianness,
) -> crate::error::Result<Vec<u8>> {
    if values.len() != shape.members.len() {
        return Err(enc_err("value/member count mismatch"));
    }
    let max_align = if xcdr2 {
        XCDR2_MAX_ALIGN
    } else {
        XCDR1_MAX_ALIGN
    };
    let mut w = Writer::with_max_align(endianness, max_align);
    // Appendable XCDR2: reserve a DHEADER, fill members, then patch the
    // size. `write_dheader` aligns to 4, reserves the four bytes and hands
    // back the handle `finish_dheader` patches — XTypes 1.3 §7.4.3.4.2 —
    // so the offset arithmetic is the writer's, not this file's.
    let dheader = (xcdr2 && shape.appendable).then(|| w.write_dheader());

    for (m, v) in shape.members.iter().zip(values) {
        let a = align_for(m.kind, xcdr2);
        w.align_to(a);
        match (m.kind, v) {
            (ScalarKind::Bool, DynValue::Bool(b)) => w.write_bool(*b),
            (ScalarKind::U8, DynValue::U8(x)) => w.write_u8(*x),
            (ScalarKind::I8, DynValue::I8(x)) => w.write_i8(*x),
            (ScalarKind::U16, DynValue::U16(x)) => w.write_u16(*x),
            (ScalarKind::I16, DynValue::I16(x)) => w.write_i16(*x),
            (ScalarKind::U32, DynValue::U32(x)) => w.write_u32(*x),
            (ScalarKind::I32, DynValue::I32(x)) => w.write_i32(*x),
            (ScalarKind::U64, DynValue::U64(x)) => w.write_u64(*x),
            (ScalarKind::I64, DynValue::I64(x)) => w.write_i64(*x),
            (ScalarKind::F32, DynValue::F32(x)) => w.write_f32(*x),
            (ScalarKind::F64, DynValue::F64(x)) => w.write_f64(*x),
            (ScalarKind::String, DynValue::Str(s)) => w.write_string(s),
            _ => return Err(enc_err("value/kind mismatch")),
        }
    }

    if let Some(header) = dheader {
        w.finish_dheader(header);
    }
    Ok(w.finish())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    //! The oracle is the vendors' octets of the interop corpus case
    //! `struct-final` / `struct-appendable` (`@final`/`@appendable struct
    //! Sample { long a; double b; short c; }` with a = -1234567, b = pi,
    //! c = -2), from `tests/interop-corpus/expected-bytes.tsv`: the octets
    //! Cyclone, RTI, Fast DDS and OpenDDS write in each cell below.
    use super::*;
    use crate::transform::shape::Member;

    fn shape(appendable: bool) -> TypeShape {
        let member = |name: &str, kind| Member {
            name: name.into(),
            kind,
        };
        TypeShape {
            name: "Sample".into(),
            members: vec![
                member("a", ScalarKind::I32),
                member("b", ScalarKind::F64),
                member("c", ScalarKind::I16),
            ],
            appendable,
        }
    }

    fn values() -> Vec<DynValue> {
        vec![
            DynValue::I32(-1_234_567),
            DynValue::F64(core::f64::consts::PI),
            DynValue::I16(-2),
        ]
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// (appendable, xcdr2, endianness, the vendors' octets)
    fn cells() -> [(bool, bool, Endianness, Vec<u8>); 8] {
        use Endianness::{Big, Little};
        [
            (
                false,
                false,
                Big,
                hex("ffed297900000000400921fb54442d18fffe"),
            ),
            (
                false,
                false,
                Little,
                hex("7929edff00000000182d4454fb210940feff"),
            ),
            (false, true, Big, hex("ffed2979400921fb54442d18fffe")),
            (false, true, Little, hex("7929edff182d4454fb210940feff")),
            (
                true,
                false,
                Big,
                hex("ffed297900000000400921fb54442d18fffe"),
            ),
            (
                true,
                false,
                Little,
                hex("7929edff00000000182d4454fb210940feff"),
            ),
            (true, true, Big, hex("0000000effed2979400921fb54442d18fffe")),
            (
                true,
                true,
                Little,
                hex("0e0000007929edff182d4454fb210940feff"),
            ),
        ]
    }

    #[test]
    fn xtypes_7_4_1_every_vendor_cell_reads_as_its_values_in_its_byte_order() {
        for (appendable, xcdr2, endianness, octets) in cells() {
            let read = decode(&shape(appendable), &octets, xcdr2, endianness).unwrap();
            assert_eq!(read, values(), "{appendable} {xcdr2} {endianness:?}");
        }
    }

    #[test]
    fn xtypes_7_4_1_the_values_are_written_as_every_vendor_cell() {
        for (appendable, xcdr2, endianness, octets) in cells() {
            let written = encode(&shape(appendable), &values(), xcdr2, endianness).unwrap();
            assert_eq!(written, octets, "{appendable} {xcdr2} {endianness:?}");
        }
    }
}
