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
//! Spec basis (IDL, as used by RTI/Cyclone/Fast-DDS):
//!
//! ```idl
//! struct ShapeType {
//!     @key string<128> color;
//!     int32 x;
//!     int32 y;
//!     int32 shapesize;
//! };
//! ```
//!
//! Encoding: XCDR2 little-endian (the default setting of all ShapesDemo
//! implementations, and matching our user-payload encapsulation header
//! `0x00 0x07 0x00 0x00`).
//!
//! CDR layout:
//! ```text
//! offset 0  : uint32   color.length (incl. null-terminator)
//! offset 4  : bytes    color.utf8_bytes
//! offset 4+n: uint8    0x00          (null-terminator)
//! padding            (to the next 4-byte boundary)
//! offset *  : int32    x
//! offset *+4: int32    y
//! offset *+8: int32    shapesize
//! ```
//!
//! The `@key` on `color` is not handled specially at the wire level —
//! it controls instance keying, not serialization. For our sample
//! matching in v1.2 (no instance map in the reader yet), every
//! (color, x, y, shapesize) combination is effectively its own sample.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::endianness::Endianness;

use crate::dds_type::{DdsType, DecodeError, EncodeError};

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
}

impl ShapeType {
    /// Constructor.
    #[must_use]
    pub fn new(color: impl Into<String>, x: i32, y: i32, shapesize: i32) -> Self {
        Self {
            color: color.into(),
            x,
            y,
            shapesize,
        }
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

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_string(&self.color);
    }

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string(&self.color)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeType.color encoding",
            })?;
        w.write_u32(self.x as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeType.x encoding",
            })?;
        w.write_u32(self.y as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeType.y encoding",
            })?;
        w.write_u32(self.shapesize as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeType.shapesize encoding",
            })?;
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        let color = r.read_string().map_err(|_| DecodeError::Invalid {
            what: "ShapeType.color decoding",
        })?;
        let x = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeType.x decoding",
        })? as i32;
        let y = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeType.y decoding",
        })? as i32;
        let shapesize = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeType.shapesize decoding",
        })? as i32;
        Ok(Self {
            color,
            x,
            y,
            shapesize,
        })
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
    /// `@key string color` — keyed, like [`ShapeType`].
    const HAS_KEY: bool = true;

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_string(&self.color);
    }

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string(&self.color)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.color encoding",
            })?;
        w.write_u32(self.x as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.x encoding",
            })?;
        w.write_u32(self.y as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.y encoding",
            })?;
        w.write_u32(self.shapesize as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.shapesize encoding",
            })?;
        w.write_u32(self.fill_kind.to_i32() as u32)
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.fillKind encoding",
            })?;
        // `float angle` — IEEE-754 32-bit, written as its little-endian bit
        // pattern (the CDR float32 wire form).
        w.write_u32(self.angle.to_bits())
            .map_err(|_| EncodeError::Invalid {
                what: "ShapeExtendedType.angle encoding",
            })?;
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        let color = r.read_string().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.color decoding",
        })?;
        let x = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.x decoding",
        })? as i32;
        let y = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.y decoding",
        })? as i32;
        let shapesize = r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.shapesize decoding",
        })? as i32;
        let fill_kind = ShapeFillKind::from_i32(r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.fillKind decoding",
        })? as i32);
        let angle = f32::from_bits(r.read_u32().map_err(|_| DecodeError::Invalid {
            what: "ShapeExtendedType.angle decoding",
        })?);
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
