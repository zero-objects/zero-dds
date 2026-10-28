// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interop test types for cross-vendor verification.
//!
//! Contains application types that count as a de-facto interop benchmark
//! in the DDS world — above all `ShapeType` from the RTI/Cyclone/Fast-DDS
//! ShapesDemo. These types are not meant for production use, but to prove
//! against other DDS stacks that our wire and type semantics are
//! byte-compatible.
//!
//! # `ShapeType`
//!
//! Spec basis — the official OMG dds-rtps interoperability `srcCxx/shape.idl`
//! (pinned suite 526350d3), which RTI/Cyclone/Fast-DDS build against:
//!
//! ```idl
//! @appendable
//! struct ShapeType {
//!     @key string<128> color;
//!     int32 x;
//!     int32 y;
//!     int32 shapesize;
//!     sequence<uint8> additional_payload_size;
//! };
//! ```
//!
//! That declaration is not only quoted here, it is a file:
//! `interop/shapes/omg-dds-rtps.idl`. The codec below is written by hand and
//! the file is read by the front end, and
//! `crates/dcps/tests/shapes_type_idl_agreement.rs` encodes the same values
//! through both and compares the octets in all four modes of the interop
//! matrix — so this type is no longer a second source of truth beside the
//! generator.
//!
//! **There are two `ShapeType`s under this one name.** The other is the
//! four-member `@final` form the classic ShapesDemo GUIs speak,
//! `interop/shapes/shapesdemo.idl` (and, in the public example mirror,
//! `zerodds-examples/cross-vendor-shapes/ShapeType.idl`). It is a different
//! type on the wire, not a shorter one; `interop/shapes/README.md` records
//! which peer speaks which.
//!
//! `@appendable` (not `@final`): in XCDR2 the sample carries a leading
//! **DHEADER** (uint32 = byte length of the members) and the encapsulation
//! header is `0x00 0x09 0x00 0x00` (D_CDR2/LE), not `0x00 0x07`. A `@final`
//! encoding (no DHEADER, 4 members) is rejected by Cyclone's appendable
//! reader — it reads `color.length` as the DHEADER and mis-frames the object
//! (measured: `deserialization Square/ShapeType failed`).
//!
//! `additional_payload_size` is a `sequence<uint8>` whose **length** carries
//! the extra payload size (large-data/throughput tests); the bytes are filler.
//! It is always on the wire (length 0 when empty).
//!
//! Encoding: XCDR2 little-endian (the ShapesDemo default). The other three
//! modes the matrix carries are implemented too — XCDR2 big-endian
//! (`D_CDR2_BE`) and XCDR1 in both orders (`CDR_LE`/`CDR_BE`, no DHEADER,
//! XTypes 1.3 Table 60) — because the writer picks the codec from the
//! encapsulation it stamps, and a mode left to the trait default would ship
//! little-endian octets behind a big-endian identifier.
//!
//! CDR layout (little-endian; the big-endian body is the same fields with
//! every multi-octet one reversed, the DHEADER included):
//! ```text
//! offset 0  : uint32   DHEADER (byte length of the members that follow)
//! offset 4  : uint32   color.length (incl. null-terminator)
//! offset 8  : bytes    color.utf8_bytes
//! offset 8+n: uint8    0x00          (null-terminator)
//! padding            (to the next 4-byte boundary)
//! offset *  : int32    x
//! offset *+4: int32    y
//! offset *+8: int32    shapesize
//! offset *+12: uint32  additional_payload_size.length
//! offset *+16: bytes   additional_payload_size elements
//! ```
//!
//! The `@key` on `color` is not handled specially at the wire level —
//! it controls instance keying, not serialization. For our sample
//! matching in v1.2 (no instance map in the reader yet), every
//! (color, x, y, shapesize) combination is effectively its own sample.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

use crate::dds_type::{DdsType, DecodeError, EncodeError, Extensibility};

/// XCDR2's `MAXALIGN`. XTypes 1.3 §7.4.1: `ALIGN(N)` aligns to
/// `MIN(N, MAXALIGN)`, and MAXALIGN is four under XCDR2 and eight under
/// XCDR1. A writer that padded an eight-octet primitive to eight under
/// XCDR2 would put four octets on the wire that no peer expects.
const XCDR2_MAX_ALIGN: usize = 4;

/// XCDR1's `MAXALIGN`, the eight of the same clause; `Writer::new`'s
/// default.
const XCDR1_MAX_ALIGN: usize = 8;

/// RTI / Cyclone / Fast-DDS ShapesDemo-compatible application type.
///
/// See the module docs for spec and layout. Color is the instance key,
/// x/y/shapesize are the typical shape coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeType {
    /// Color / instance key. In ShapesDemo implementations typically
    /// `"BLUE"`, `"RED"`, `"GREEN"`, `"YELLOW"`, `"MAGENTA"`, `"CYAN"`,
    /// `"ORANGE"`, `"PURPLE"`. No content validation here — any UTF-8
    /// string is allowed.
    pub color: String,
    /// X coordinate in pixels (the ShapesDemo canvas is ~240×270).
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Shape size in pixels. Typically 30.
    pub shapesize: i32,
    /// `sequence<uint8> additional_payload_size` (OMG `shape.idl`). The
    /// **length** carries the extra payload size for large-data/throughput
    /// tests; the bytes themselves are filler. Empty for normal shape
    /// samples — still serialized (length 0) so the wire type stays the
    /// 5-member `@appendable` `ShapeType`.
    pub additional_payload_size: Vec<u8>,
}

impl ShapeType {
    /// Constructor for a normal shape sample (no additional payload).
    #[must_use]
    pub fn new(color: impl Into<String>, x: i32, y: i32, shapesize: i32) -> Self {
        Self {
            color: color.into(),
            x,
            y,
            shapesize,
            additional_payload_size: Vec::new(),
        }
    }

    /// Constructor with an explicit `additional_payload_size` sequence
    /// (OMG `--additional-payload-size` large-data tests). `payload` becomes
    /// the sequence content; its length is the announced payload size.
    #[must_use]
    pub fn with_payload(
        color: impl Into<String>,
        x: i32,
        y: i32,
        shapesize: i32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            color: color.into(),
            x,
            y,
            shapesize,
            additional_payload_size: payload,
        }
    }

    /// XCDR2 DELIMITED_CDR body, `endian`-ordered: the DHEADER and the 5
    /// members behind it.
    ///
    /// The byte order is a parameter and not a constant because the
    /// encapsulation identifier carries it: `D_CDR2_LE` is `0x00 0x09` and
    /// `D_CDR2_BE` is `0x00 0x08` (RTPS 2.5 §10.5), and the identifier states
    /// the order of *the whole* serialized payload — the DHEADER (XTypes 1.3
    /// §7.4.3.4.2 makes it a `uint32` of the stream) as much as the members.
    /// A little-endian body under `D_CDR2_BE` is not a smaller mistake than a
    /// wrong member: the peer reads the DHEADER byte-swapped and frames the
    /// object on a length that is not the one written.
    fn encode_xcdr2_endian(
        &self,
        out: &mut Vec<u8>,
        endian: Endianness,
    ) -> core::result::Result<(), EncodeError> {
        let payload_len = self.payload_len()?;
        // The DHEADER goes out as a placeholder and is patched with the body's
        // length once the members are written — the length is not known until
        // then.
        let mut w = Writer::appending(endian, XCDR2_MAX_ALIGN, core::mem::take(out));
        let dheader = w.write_dheader();
        w.write_string(&self.color);
        w.write_u32(self.x as u32);
        w.write_u32(self.y as u32);
        w.write_u32(self.shapesize as u32);
        // sequence<uint8>: uint32 length + raw bytes (primitive element →
        // no per-sequence DHEADER, XTypes 1.3 §7.4.3.5.3).
        w.write_u32(payload_len);
        w.write_bytes(&self.additional_payload_size);
        w.finish_dheader(dheader);
        *out = w.finish();
        Ok(())
    }

    /// Symmetric XCDR2 decode, `endian`-ordered. See
    /// [`Self::encode_xcdr2_endian`] for why the order is a parameter.
    fn decode_xcdr2_endian(
        bytes: &[u8],
        endian: Endianness,
    ) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, endian, XCDR2_MAX_ALIGN);
        let extent = r.read_dheader()?;
        let end = r.position() + extent;
        let color = r.read_string()?;
        let x = r.read_u32()? as i32;
        let y = r.read_u32()? as i32;
        let shapesize = r.read_u32()? as i32;
        // @appendable trailing member: a 4-member peer (or a truncated
        // body) legitimately omits it — an exhausted body is an empty
        // sequence and not an error (XTypes 1.3 §7.4.3.4.5). `end` and
        // not the stream: see `read_trailing_sequence`.
        let additional_payload_size = Self::read_trailing_sequence(&mut r, end)?;
        Ok(Self {
            color,
            x,
            y,
            shapesize,
            additional_payload_size,
        })
    }

    /// XCDR1 (classic CDR) body, `endian`-ordered: the 5 members **without** a
    /// DHEADER. XCDR1 has no dedicated `@appendable` encoding — it is framed
    /// like `@final` (XTypes 1.3 Table 60), so the DHEADER that `encode()` writes
    /// for XCDR2 must be omitted. Alignment cap is XCDR1's 8, which is
    /// `Writer::new`'s default; ShapeType has no 8-byte member, so LE XCDR1 and
    /// XCDR2 bodies differ only by that missing DHEADER.
    fn encode_xcdr1_endian(
        &self,
        out: &mut Vec<u8>,
        endian: Endianness,
    ) -> core::result::Result<(), EncodeError> {
        let payload_len = self.payload_len()?;
        let mut w = Writer::appending(endian, XCDR1_MAX_ALIGN, core::mem::take(out));
        w.write_string(&self.color);
        w.write_u32(self.x as u32);
        w.write_u32(self.y as u32);
        w.write_u32(self.shapesize as u32);
        w.write_u32(payload_len);
        w.write_bytes(&self.additional_payload_size);
        *out = w.finish();
        Ok(())
    }

    /// The `sequence<octet>` length as it goes on the wire.
    ///
    /// Refused and not truncated: a length that does not fit its field
    /// would announce fewer octets than follow, and the peer would read
    /// the remainder as the next sample's members.
    fn payload_len(&self) -> core::result::Result<u32, EncodeError> {
        u32::try_from(self.additional_payload_size.len()).map_err(|_| EncodeError::Invalid {
            what: "ShapeType.additional_payload_size exceeds u32::MAX",
        })
    }

    /// Symmetric XCDR1 decode: the 5 members without a DHEADER. A 4-member peer
    /// (or a truncated body) legitimately omits the trailing sequence.
    fn decode_xcdr1_endian(
        bytes: &[u8],
        endian: Endianness,
    ) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes, endian);
        let color = r.read_string()?;
        let x = r.read_u32()? as i32;
        let y = r.read_u32()? as i32;
        let shapesize = r.read_u32()? as i32;
        // XCDR1 has no DHEADER, so the body runs to the end of the
        // stream and `remaining` is the extent.
        let additional_payload_size = Self::read_trailing_sequence(&mut r, bytes.len())?;
        Ok(Self {
            color,
            x,
            y,
            shapesize,
            additional_payload_size,
        })
    }

    /// The trailing `sequence<octet>`, or nothing when the body ends first.
    ///
    /// `end` is where the *body* ends and not where the stream does. The
    /// two differ under `@appendable`, whose DHEADER states an extent
    /// that a peer's trailing padding or a following member may sit
    /// past — a reader that asked the stream instead would take those
    /// four octets for this sequence's length and then allocate on them.
    fn read_trailing_sequence(
        r: &mut Reader<'_>,
        end: usize,
    ) -> core::result::Result<Vec<u8>, DecodeError> {
        if end.saturating_sub(r.position()) < 4 {
            return Ok(Vec::new());
        }
        let n = r.read_u32()? as usize;
        Ok(r.read_bytes(n)?.to_vec())
    }
}

impl DdsType for ShapeType {
    /// Type name **exactly** as in RTI/Cyclone/Fast-DDS ShapesDemo.
    /// Changing it would break matching with any other ShapesDemo client.
    const TYPE_NAME: &'static str = "ShapeType";
    /// ShapesDemo IDL: `@key string color`. ShapeType is keyed —
    /// per-instance QoS (TimeBasedFilter, Ownership, Lifecycle) depends
    /// on it.
    const HAS_KEY: bool = true;
    /// OMG `shape.idl` declares `@appendable` — in XCDR2 this frames the
    /// sample with a leading DHEADER and stamps the D_CDR2 (0x0009)
    /// encapsulation id. Cyclone/RTI/Fast-DDS reject a `@final` framing.
    const EXTENSIBILITY: Extensibility = Extensibility::Appendable;

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_string(&self.color);
    }

    /// The inverse. DDS 1.4 §2.2.2.5.1.13 declares the key fields valid on a
    /// dispose/unregister sample, and the holder above is where they are: a
    /// PLAIN_CDR2-BE `string` — a four-octet length counting the NUL, the
    /// octets, then the NUL — with no DHEADER in front, so `Self::decode`
    /// would read that length as one. The members outside the holder are not
    /// in the stream and take their defaults; the clause declares them
    /// meaningless on such a sample.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Big, 4);
        Ok(Self {
            color: r.read_string()?,
            x: 0,
            y: 0,
            shapesize: 0,
            additional_payload_size: Vec::new(),
        })
    }

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_xcdr2_endian(out, Endianness::Little)
    }

    /// The same DELIMITED_CDR body big-endian, for the `D_CDR2_BE` (`0x00
    /// 0x08`) encapsulation a writer stamps under `ZERODDS_WIRE_BIG_ENDIAN`.
    ///
    /// Overridden and not left to the trait default, which delegates to
    /// [`Self::encode`] and would therefore put **little-endian** octets
    /// behind a big-endian encapsulation identifier. Nothing on either side
    /// reports that: the header parses, the length is right, and the peer
    /// byte-swaps a DHEADER of 24 into 402 653 184. Measured by
    /// `crates/dcps/tests/shapes_type_idl_agreement.rs`, which is what found
    /// it — the hand-derived reference in `shapes_type_wire.rs` covers the
    /// little-endian mode only.
    fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_xcdr2_endian(out, Endianness::Big)
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_xcdr2_endian(bytes, Endianness::Little)
    }

    /// Symmetric to [`Self::encode_be`]: the subscriber routes a sample whose
    /// encapsulation declared `*_BE` here (`decode_for_encap`), so a default
    /// that read the body little-endian would decode a peer's big-endian
    /// ShapeType into a sample the application cannot tell from a correct one.
    fn decode_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_xcdr2_endian(bytes, Endianness::Big)
    }

    // XCDR1 (classic CDR) overrides: @appendable is framed like @final under
    // XCDR1 — no DHEADER (XTypes 1.3 Table 60). The DataWriter selects these when
    // the writer's effective DataRepresentation is XCDR1 (e.g. `-x 1`), so the
    // body matches the CDR_LE/CDR_BE encapsulation instead of shipping the
    // XCDR2 DHEADER form.
    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_xcdr1_endian(out, Endianness::Little)
    }

    fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_xcdr1_endian(out, Endianness::Big)
    }

    fn decode_xcdr1(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_xcdr1_endian(bytes, Endianness::Little)
    }

    fn decode_xcdr1_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_xcdr1_endian(bytes, Endianness::Big)
    }
}

/// ShapesDemo `ShapeFillKind` enum (RTI 7.x / Fast-DDS ShapeExtended IDL).
///
/// ```idl
/// @final
/// enum ShapeFillKind { SOLID_FILL, TRANSPARENT_FILL, HORIZONTAL_HATCH, VERTICAL_HATCH };
/// ```
///
/// Wire form: a 4-byte little-endian `int32` discriminant (0..=3), per the
/// XTypes enum default underlying type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ShapeFillKind {
    /// `0` — solid fill (the ShapesDemo default).
    #[default]
    SolidFill = 0,
    /// `1` — transparent (outline only).
    TransparentFill = 1,
    /// `2` — horizontal hatch.
    HorizontalHatch = 2,
    /// `3` — vertical hatch.
    VerticalHatch = 3,
}

impl ShapeFillKind {
    /// The wire discriminant.
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self as i32
    }

    /// Maps a wire discriminant to a fill kind, falling back to `SolidFill`
    /// for an out-of-range value (forward-compatible with extended enums).
    #[must_use]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::TransparentFill,
            2 => Self::HorizontalHatch,
            3 => Self::VerticalHatch,
            _ => Self::SolidFill,
        }
    }
}

/// RTI 7.x / Fast-DDS **default** ShapesDemo type — `ShapeExtendedType`.
///
/// ```idl
/// @final
/// struct ShapeExtendedType {
///     @key string color;
///     long x;
///     long y;
///     long shapesize;
///     ShapeFillKind fillKind;
///     float angle;
/// };
/// ```
///
/// This is the modern vendor default: RTI Connext 7.x ShapesDemo publishes
/// `ShapeExtendedType` unless started with `-dataType Shape`. Carrying it
/// natively lets ZeroDDS interop with an unmodified RTI ShapesDemo (no flag).
/// It is a **distinct topic type** from [`ShapeType`] (different `TYPE_NAME`),
/// so SEDP matches only against other `ShapeExtendedType` endpoints.
///
/// That declaration is not only quoted here, it is a file:
/// `interop/shapes/shapesdemo-extended.idl`. The codec below is written by
/// hand and the file is read by the front end, and
/// `crates/dcps/tests/shapes_type_idl_agreement.rs` encodes the same values
/// through both and compares the octets in all four modes the interop matrix
/// carries — so this type is no longer a second source of truth beside the
/// generator. Until that file was written it was one: its member order, its
/// `@final` framing and the four-octet width of the `fillKind` discriminant
/// were checked only against byte sequences derived by the same hand that
/// wrote the codec.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeExtendedType {
    /// Color / instance key (same semantics as [`ShapeType::color`]).
    pub color: String,
    /// X coordinate in pixels.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Shape size in pixels.
    pub shapesize: i32,
    /// Fill style (new in the extended type).
    pub fill_kind: ShapeFillKind,
    /// Rotation angle in degrees (new in the extended type).
    pub angle: f32,
}

impl ShapeExtendedType {
    /// Constructor.
    #[must_use]
    pub fn new(
        color: impl Into<String>,
        x: i32,
        y: i32,
        shapesize: i32,
        fill_kind: ShapeFillKind,
        angle: f32,
    ) -> Self {
        Self {
            color: color.into(),
            x,
            y,
            shapesize,
            fill_kind,
            angle,
        }
    }
}

impl DdsType for ShapeExtendedType {
    /// Type name **exactly** as in RTI/Fast-DDS ShapesDemo. Must not change.
    const TYPE_NAME: &'static str = "ShapeExtendedType";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    /// `@key string color` — keyed, like [`ShapeType`].
    const HAS_KEY: bool = true;

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_string(&self.color);
    }

    /// The inverse. DDS 1.4 §2.2.2.5.1.13 declares the key fields valid on a
    /// dispose/unregister sample, and the holder above is where they are: a
    /// PLAIN_CDR2-BE `string` — a four-octet length counting the NUL, the
    /// octets, then the NUL — with no DHEADER in front, so `Self::decode`
    /// would read that length as one. The members outside the holder are not
    /// in the stream and take their defaults; the clause declares them
    /// meaningless on such a sample.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Big, 4);
        Ok(Self {
            color: r.read_string()?,
            x: 0,
            y: 0,
            shapesize: 0,
            fill_kind: ShapeFillKind::SolidFill,
            angle: 0.0,
        })
    }

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_endian(out, Endianness::Little)
    }

    fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_endian(out, Endianness::Big)
    }

    fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode_endian(out, Endianness::Big)
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_endian(bytes, Endianness::Little)
    }

    fn decode_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_endian(bytes, Endianness::Big)
    }

    fn decode_xcdr1_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_endian(bytes, Endianness::Big)
    }
}

impl ShapeExtendedType {
    /// The six members in `endian` order. `@final` and no eight-octet member,
    /// so this one body serves both XCDR1 and XCDR2 — the encapsulation
    /// identifier is the only thing that separates them, and that is the
    /// runtime's to choose.
    fn encode_endian(
        &self,
        out: &mut Vec<u8>,
        endian: Endianness,
    ) -> core::result::Result<(), EncodeError> {
        let mut w = Writer::new(endian);
        w.write_string(&self.color);
        w.write_u32(self.x as u32);
        w.write_u32(self.y as u32);
        w.write_u32(self.shapesize as u32);
        w.write_u32(self.fill_kind.to_i32() as u32);
        // `float angle` — IEEE-754 32-bit, written as its bit pattern in the
        // stream's byte order (the CDR float32 wire form).
        w.write_u32(self.angle.to_bits());
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    /// Symmetric read of [`Self::encode_endian`].
    fn decode_endian(bytes: &[u8], endian: Endianness) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes, endian);
        let color = r.read_string()?;
        let x = r.read_u32()? as i32;
        let y = r.read_u32()? as i32;
        let shapesize = r.read_u32()? as i32;
        let fill_kind = ShapeFillKind::from_i32(r.read_u32()? as i32);
        let angle = f32::from_bits(r.read_u32()?);
        Ok(Self {
            color,
            x,
            y,
            shapesize,
            fill_kind,
            angle,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn shapetype_appendable_wire_golden() {
        // Deterministic offline golden (no DDS, no net): color="BLUE", x=100,
        // y=100, shapesize=30, empty additional_payload_size. Body only — the
        // 4-byte encap header (0x0009 D_CDR2/@appendable/LE) is prepended by
        // the writer layer (publisher.rs set_user_writer_wire_extensibility).
        assert_eq!(
            <ShapeType as DdsType>::EXTENSIBILITY,
            Extensibility::Appendable
        );
        let s = ShapeType::new("BLUE", 100, 100, 30);
        let mut body = Vec::new();
        s.encode(&mut body).unwrap();
        // Expected @appendable body: DHEADER(28) + color(9) + pad(3) + x + y +
        // shapesize + seq_len(0).
        assert_eq!(
            body,
            alloc::vec![
                0x1c, 0x00, 0x00, 0x00, // DHEADER = 28
                0x05, 0x00, 0x00, 0x00, // color len = 5
                0x42, 0x4c, 0x55, 0x45, 0x00, // "BLUE"\0
                0x00, 0x00, 0x00, // pad to 4
                0x64, 0x00, 0x00, 0x00, // x = 100
                0x64, 0x00, 0x00, 0x00, // y = 100
                0x1e, 0x00, 0x00, 0x00, // shapesize = 30
                0x00, 0x00, 0x00, 0x00, // additional_payload_size.len = 0
            ]
        );
        // Round-trip.
        assert_eq!(ShapeType::decode(&body).unwrap(), s);
    }

    #[test]
    fn shapetype_additional_payload_on_wire() {
        // --additional-payload-size N puts N bytes on the wire (large-data).
        let s = ShapeType::with_payload("RED", 1, 2, 3, alloc::vec![0xAB; 5]);
        let mut body = Vec::new();
        s.encode(&mut body).unwrap();
        // Last 9 bytes: seq_len=5 (LE) + 5 filler bytes.
        assert_eq!(
            &body[body.len() - 9..],
            &[0x05, 0, 0, 0, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB]
        );
        assert_eq!(ShapeType::decode(&body).unwrap(), s);
    }

    #[test]
    fn shapetype_xcdr1_omits_dheader_golden() {
        // XCDR1 @appendable is framed like @final: the 5 members with NO leading
        // DHEADER. Same sample as the XCDR2 golden (color="BLUE",100,100,30).
        let s = ShapeType::new("BLUE", 100, 100, 30);
        let mut x1 = Vec::new();
        s.encode_xcdr1(&mut x1).unwrap();
        assert_eq!(
            x1,
            alloc::vec![
                0x05, 0x00, 0x00, 0x00, // color len = 5 (NO DHEADER before it)
                0x42, 0x4c, 0x55, 0x45, 0x00, // "BLUE"\0
                0x00, 0x00, 0x00, // pad to 4
                0x64, 0x00, 0x00, 0x00, // x = 100
                0x64, 0x00, 0x00, 0x00, // y = 100
                0x1e, 0x00, 0x00, 0x00, // shapesize = 30
                0x00, 0x00, 0x00, 0x00, // additional_payload_size.len = 0
            ]
        );
        // The XCDR2 body is exactly the XCDR1 body prefixed by the DHEADER.
        let mut x2 = Vec::new();
        s.encode(&mut x2).unwrap();
        assert_eq!(x2.len(), x1.len() + 4, "XCDR2 adds a 4-byte DHEADER");
        assert_eq!(&x2[0..4], &[0x1c, 0x00, 0x00, 0x00], "DHEADER = 28");
        assert_eq!(&x2[4..], &x1[..], "members identical after the DHEADER");
    }

    #[test]
    fn shapetype_xcdr1_round_trips_le_and_be() {
        let s = ShapeType::with_payload("RED", -5, 7, 30, alloc::vec![1, 2, 3]);
        // LE
        let mut le = Vec::new();
        s.encode_xcdr1(&mut le).unwrap();
        assert_eq!(ShapeType::decode_xcdr1(&le).unwrap(), s);
        // BE
        let mut be = Vec::new();
        s.encode_xcdr1_be(&mut be).unwrap();
        assert_eq!(ShapeType::decode_xcdr1_be(&be).unwrap(), s);
        // LE and BE differ (multi-byte fields byte-swapped).
        assert_ne!(le, be);
    }

    #[test]
    fn shapetype_decode_tolerates_missing_trailing_member() {
        // A 4-member @appendable peer omits additional_payload_size; the body
        // is DHEADER(24) + the 4 core members. decode() must treat the absent
        // trailing member as an empty sequence (XTypes appendable rule).
        let body = alloc::vec![
            0x14, 0x00, 0x00, 0x00, // DHEADER = 20 (4 core members, no seq)
            0x04, 0x00, 0x00, 0x00, // color len = 4 ("RED"\0)
            0x52, 0x45, 0x44, 0x00, // "RED"\0
            0x01, 0x00, 0x00, 0x00, // x = 1
            0x02, 0x00, 0x00, 0x00, // y = 2
            0x03, 0x00, 0x00, 0x00, // shapesize = 3
        ];
        let decoded = ShapeType::decode(&body).unwrap();
        assert_eq!(decoded, ShapeType::new("RED", 1, 2, 3));
        assert!(decoded.additional_payload_size.is_empty());
    }

    #[test]
    fn shape_extended_round_trip() {
        let s = ShapeExtendedType::new("BLUE", 100, 150, 30, ShapeFillKind::HorizontalHatch, 45.5);
        let mut bytes = Vec::new();
        s.encode(&mut bytes).unwrap();
        let back = ShapeExtendedType::decode(&bytes).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn shape_extended_type_name_distinct_from_shape() {
        assert_eq!(ShapeExtendedType::TYPE_NAME, "ShapeExtendedType");
        assert_ne!(ShapeExtendedType::TYPE_NAME, ShapeType::TYPE_NAME);
        // Regression guard that the codegen sets `HAS_KEY` for a keyed type.
        // It is a `const bool`, so clippy folds it and warns it would be
        // optimised out — that is exactly the intent here (a compile-time fact).
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(ShapeExtendedType::HAS_KEY);
        }
    }

    #[test]
    fn shape_extended_wire_layout() {
        // @final XCDR2/CDR LE: string(len+bytes) + x + y + shapesize + fillKind
        // (int32) + angle (float32 bits). "RED\0" = 4-byte len + 4 bytes.
        let s = ShapeExtendedType::new("RED", 1, 2, 30, ShapeFillKind::SolidFill, 0.0);
        let mut bytes = Vec::new();
        s.encode(&mut bytes).unwrap();
        // 4 (strlen) + 4 ("RED\0") + 4*4 (x,y,shapesize,fillKind) + 4 (angle).
        assert_eq!(bytes.len(), 4 + 4 + 16 + 4);
        // String length prefix = 4 ("RED" + NUL), little-endian.
        assert_eq!(&bytes[0..4], &[4, 0, 0, 0]);
        assert_eq!(&bytes[4..8], b"RED\0");
        // fillKind SolidFill = 0, angle 0.0 = 0x00000000.
        assert_eq!(&bytes[20..24], &[0, 0, 0, 0]); // fillKind
        assert_eq!(&bytes[24..28], &[0, 0, 0, 0]); // angle
    }

    /// Proves the XCDR1 (max-align 8) vs XCDR2 (max-align 4) alignment
    /// difference is real, so the write-path dispatch to `encode_xcdr1` genuinely
    /// changes the wire — not just for the DHEADER but for 8-byte-member padding.
    #[test]
    fn u64_type_xcdr1_and_xcdr2_alignment_differs() {
        struct AlignProbe {
            a: u8,
            b: u64,
        }
        impl DdsType for AlignProbe {
            const TYPE_NAME: &'static str = "AlignProbe";
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                // XCDR2: max-alignment 4 → u64 aligns to 4.
                let mut w = Writer::with_max_align(Endianness::Little, XCDR2_MAX_ALIGN);
                w.write_u8(self.a);
                w.write_u64(self.b);
                out.extend_from_slice(w.as_bytes());
                Ok(())
            }
            fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                // XCDR1: max-alignment 8 → u64 aligns to 8.
                let mut w = Writer::new(Endianness::Little);
                w.write_u8(self.a);
                w.write_u64(self.b);
                out.extend_from_slice(w.as_bytes());
                Ok(())
            }
            fn decode(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                // Encode-only probe; a trivial decode keeps the trait satisfied.
                Ok(Self { a: 0, b: 0 })
            }
        }
        let s = AlignProbe { a: 0x11, b: 0x2222 };
        let mut x2 = Vec::new();
        s.encode(&mut x2).unwrap();
        let mut x1 = Vec::new();
        s.encode_xcdr1(&mut x1).unwrap();
        // XCDR2: a@0, pad 3, u64@4  -> 12 bytes.
        assert_eq!(x2.len(), 12);
        // XCDR1: a@0, pad 7, u64@8  -> 16 bytes.
        assert_eq!(x1.len(), 16);
        assert_ne!(
            x1, x2,
            "XCDR1 and XCDR2 wire forms must differ for a u64 type"
        );
    }

    #[test]
    fn fill_kind_round_trips_and_clamps() {
        for k in [
            ShapeFillKind::SolidFill,
            ShapeFillKind::TransparentFill,
            ShapeFillKind::HorizontalHatch,
            ShapeFillKind::VerticalHatch,
        ] {
            assert_eq!(ShapeFillKind::from_i32(k.to_i32()), k);
        }
        // Unknown discriminant → SolidFill (forward-compatible).
        assert_eq!(ShapeFillKind::from_i32(99), ShapeFillKind::SolidFill);
    }
}
