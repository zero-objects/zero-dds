// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interop-Test-Typen fuer Cross-Vendor-Nachweis.
//!
//! Enthaelt Application-Typen, die in der DDS-Welt als de-facto-
//! Interop-Benchmark gelten — allen voran `ShapeType` aus dem
//! RTI/Cyclone/Fast-DDS-ShapesDemo. Diese Typen sind nicht fuer
//! Produktions-Nutzung gedacht, sondern dafuer, gegenueber anderen
//! DDS-Stacks zu beweisen, dass unsere Wire- und Typen-Semantik
//! byte-kompatibel ist.
//!
//! # `ShapeType`
//!
//! Spec-Grundlage (IDL, wie von RTI/Cyclone/Fast-DDS verwendet):
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
//! Encoding: XCDR2 Little-Endian (das ist die Standard-Einstellung
//! saemtlicher ShapesDemo-Implementierungen, und entspricht unserem
//! User-Payload-Encapsulation-Header `0x00 0x07 0x00 0x00`).
//!
//! CDR-Layout:
//! ```text
//! offset 0  : uint32   color.length (inkl. null-terminator)
//! offset 4  : bytes    color.utf8_bytes
//! offset 4+n: uint8    0x00          (null-terminator)
//! padding            (auf naechste 4-Byte-Grenze)
//! offset *  : int32    x
//! offset *+4: int32    y
//! offset *+8: int32    shapesize
//! ```
//!
//! Der `@key` auf `color` wird auf Wire-Level nicht gesondert behandelt
//! — er steuert Instance-Keying, nicht die Serialisierung. Fuer unser
//! Sample-Matching in v1.2 (noch keine Instance-Map im Reader) ist
//! jede (color, x, y, shapesize)-Kombination effektiv ein eigenes
//! Sample.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::endianness::Endianness;

use crate::dds_type::{DdsType, DecodeError, EncodeError};

/// RTI / Cyclone / Fast-DDS ShapesDemo-kompatibler Application-Type.
///
/// Siehe Modul-Doku fuer Spec und Layout. Farbe ist Instance-Key,
/// x/y/shapesize sind die typischen Formen-Koordinaten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeType {
    /// Farbe / Instance-Key. In ShapesDemo-Implementierungen typisch
    /// `"BLUE"`, `"RED"`, `"GREEN"`, `"YELLOW"`, `"MAGENTA"`, `"CYAN"`,
    /// `"ORANGE"`, `"PURPLE"`. Keine inhaltliche Validierung hier —
    /// beliebige UTF-8-Strings sind zulaessig.
    pub color: String,
    /// X-Koordinate in Pixeln (ShapesDemo-Canvas ist ~240×270).
    pub x: i32,
    /// Y-Koordinate.
    pub y: i32,
    /// Formen-Groesse in Pixeln. Typisch 30.
    pub shapesize: i32,
}

impl ShapeType {
    /// Konstruktor.
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
    /// Type-Name **exakt** wie RTI/Cyclone/Fast-DDS ShapesDemo. Aendern
    /// wuerde Matching mit jedem anderen ShapesDemo-Client brechen.
    const TYPE_NAME: &'static str = "ShapeType";
    /// ShapesDemo-IDL: `@key string color`. ShapeType ist keyed —
    /// Per-Instance-QoS (TimeBasedFilter, Ownership, Lifecycle)
    /// haengt davon ab.
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
