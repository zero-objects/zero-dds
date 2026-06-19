// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Shape-driven CDR codec for flat structs (XCDR1 + XCDR2, `@final` /
//! `@appendable`).
//!
//! Operates on the CDR **body** (without the encapsulation header) exactly as
//! the runtime user byte-path delivers it, mirroring the `zerodds-cdr` body
//! layout (align origin 0; max alignment 8 for XCDR1, 4 for XCDR2;
//! Cyclone/FastDDS byte-compatible). The encapsulation header is stripped on
//! receive, so the source endianness is not recoverable from the body — the
//! codec assumes **little-endian** (the universal DDS default on LE hosts) and
//! re-encodes little-endian. Byte pass-through routes (no filter/transform)
//! never decode and so are endianness-agnostic.

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

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

/// Decodes a body into the shape's ordered values.
///
/// # Errors
/// [`RoutingError::Transform`] on truncated/invalid input.
pub fn decode(
    shape: &TypeShape,
    body: &[u8],
    representation: u8,
) -> crate::error::Result<Vec<DynValue>> {
    let xcdr2 = representation != 0;
    let mut r = BufferReader::new(body, Endianness::Little);
    if xcdr2 {
        r = r.xcdr2();
        if shape.appendable {
            // DHEADER (object size) — read and discard; members follow.
            r.read_u32().map_err(|_| dec_err("dheader"))?;
        }
    }
    let mut out = Vec::with_capacity(shape.members.len());
    for m in &shape.members {
        let a = align_for(m.kind, xcdr2);
        r.align(a).map_err(|_| dec_err("align"))?;
        let v = match m.kind {
            ScalarKind::Bool => DynValue::Bool(r.read_u8().map_err(|_| dec_err("bool"))? != 0),
            ScalarKind::U8 => DynValue::U8(r.read_u8().map_err(|_| dec_err("u8"))?),
            ScalarKind::I8 => DynValue::I8(r.read_u8().map_err(|_| dec_err("i8"))? as i8),
            ScalarKind::U16 => DynValue::U16(r.read_u16().map_err(|_| dec_err("u16"))?),
            ScalarKind::I16 => DynValue::I16(r.read_u16().map_err(|_| dec_err("i16"))? as i16),
            ScalarKind::U32 => DynValue::U32(r.read_u32().map_err(|_| dec_err("u32"))?),
            ScalarKind::I32 => DynValue::I32(r.read_u32().map_err(|_| dec_err("i32"))? as i32),
            ScalarKind::U64 => DynValue::U64(r.read_u64().map_err(|_| dec_err("u64"))?),
            ScalarKind::I64 => DynValue::I64(r.read_u64().map_err(|_| dec_err("i64"))? as i64),
            ScalarKind::F32 => {
                DynValue::F32(f32::from_bits(r.read_u32().map_err(|_| dec_err("f32"))?))
            }
            ScalarKind::F64 => {
                DynValue::F64(f64::from_bits(r.read_u64().map_err(|_| dec_err("f64"))?))
            }
            ScalarKind::String => DynValue::Str(r.read_string().map_err(|_| dec_err("string"))?),
        };
        out.push(v);
    }
    Ok(out)
}

/// Encodes the shape's values back into a CDR body.
///
/// # Errors
/// [`RoutingError::Transform`] on a value/kind mismatch.
pub fn encode(
    shape: &TypeShape,
    values: &[DynValue],
    representation: u8,
) -> crate::error::Result<Vec<u8>> {
    if values.len() != shape.members.len() {
        return Err(enc_err("value/member count mismatch"));
    }
    let xcdr2 = representation != 0;
    let mut w = if xcdr2 {
        BufferWriter::new(Endianness::Little).xcdr2()
    } else {
        BufferWriter::new(Endianness::Little)
    };
    // Appendable XCDR2: reserve a DHEADER, fill members, then patch the size.
    let dheader_at = if xcdr2 && shape.appendable {
        w.align(4);
        let at = w.position();
        w.write_u32(0).map_err(|_| enc_err("dheader placeholder"))?;
        Some(at)
    } else {
        None
    };

    for (m, v) in shape.members.iter().zip(values) {
        let a = align_for(m.kind, xcdr2);
        w.align(a);
        match (m.kind, v) {
            (ScalarKind::Bool, DynValue::Bool(b)) => {
                w.write_u8(u8::from(*b)).map_err(|_| enc_err("bool"))?;
            }
            (ScalarKind::U8, DynValue::U8(x)) => w.write_u8(*x).map_err(|_| enc_err("u8"))?,
            (ScalarKind::I8, DynValue::I8(x)) => {
                w.write_u8(*x as u8).map_err(|_| enc_err("i8"))?;
            }
            (ScalarKind::U16, DynValue::U16(x)) => w.write_u16(*x).map_err(|_| enc_err("u16"))?,
            (ScalarKind::I16, DynValue::I16(x)) => {
                w.write_u16(*x as u16).map_err(|_| enc_err("i16"))?;
            }
            (ScalarKind::U32, DynValue::U32(x)) => w.write_u32(*x).map_err(|_| enc_err("u32"))?,
            (ScalarKind::I32, DynValue::I32(x)) => {
                w.write_u32(*x as u32).map_err(|_| enc_err("i32"))?;
            }
            (ScalarKind::U64, DynValue::U64(x)) => w.write_u64(*x).map_err(|_| enc_err("u64"))?,
            (ScalarKind::I64, DynValue::I64(x)) => {
                w.write_u64(*x as u64).map_err(|_| enc_err("i64"))?;
            }
            (ScalarKind::F32, DynValue::F32(x)) => {
                w.write_u32(x.to_bits()).map_err(|_| enc_err("f32"))?;
            }
            (ScalarKind::F64, DynValue::F64(x)) => {
                w.write_u64(x.to_bits()).map_err(|_| enc_err("f64"))?;
            }
            (ScalarKind::String, DynValue::Str(s)) => {
                w.write_string(s).map_err(|_| enc_err("string"))?;
            }
            _ => return Err(enc_err("value/kind mismatch")),
        }
    }

    let mut bytes = w.into_bytes();
    if let Some(at) = dheader_at {
        // DHEADER value = bytes after the DHEADER (object size), XTypes §7.4.3.4.2.
        let size = (bytes.len() - at - 4) as u32;
        bytes[at..at + 4].copy_from_slice(&size.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::transform::shape::Member;

    fn shape_final() -> TypeShape {
        TypeShape {
            name: "T".into(),
            members: vec![
                Member {
                    name: "a".into(),
                    kind: ScalarKind::U32,
                },
                Member {
                    name: "b".into(),
                    kind: ScalarKind::String,
                },
                Member {
                    name: "c".into(),
                    kind: ScalarKind::F64,
                },
            ],
            appendable: false,
        }
    }

    #[test]
    fn roundtrip_xcdr1() {
        let sh = shape_final();
        let vals = vec![
            DynValue::U32(0x1122_3344),
            DynValue::Str("hi".into()),
            DynValue::F64(3.5),
        ];
        let body = encode(&sh, &vals, 0).unwrap();
        let back = decode(&sh, &body, 0).unwrap();
        assert_eq!(vals, back);
    }

    #[test]
    fn roundtrip_xcdr2() {
        let sh = shape_final();
        let vals = vec![
            DynValue::U32(7),
            DynValue::Str("world".into()),
            DynValue::F64(-1.25),
        ];
        let body = encode(&sh, &vals, 2).unwrap();
        let back = decode(&sh, &body, 2).unwrap();
        assert_eq!(vals, back);
    }

    #[test]
    fn roundtrip_xcdr2_appendable_dheader() {
        let mut sh = shape_final();
        sh.appendable = true;
        let vals = vec![
            DynValue::U32(42),
            DynValue::Str("x".into()),
            DynValue::F64(2.0),
        ];
        let body = encode(&sh, &vals, 2).unwrap();
        // DHEADER (first 4 bytes LE) must equal the remaining body length.
        let dh = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        assert_eq!(dh, body.len() - 4);
        let back = decode(&sh, &body, 2).unwrap();
        assert_eq!(vals, back);
    }
}
