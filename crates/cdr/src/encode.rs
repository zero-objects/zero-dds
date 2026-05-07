// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `CdrEncode` / `CdrDecode` Traits + Primitive-Implementierungen
//! (W1.3).
//!
//! Trait-basiert statt freier Funktionen, damit Composite-Typen in W2/W3
//! ihre Children einheitlich rekursiv-encodieren koennen ohne grosse
//! Type-Switches.
//!
//! # Alignment-Konvention
//!
//! - `u8`/`i8`/`bool`: 1
//! - `u16`/`i16`: 2
//! - `u32`/`i32`/`f32`/`char`: 4
//! - `u64`/`i64`/`f64`: 8
//!
//! Char wird als XCDR2-`wchar32` (4 Byte) kodiert (OMG-XTypes §7.4.7);
//! das deckt voll Unicode ab. Die XCDR1-`char`-Variante (1 Byte ASCII)
//! lebt im separaten `xcdr1`-Modul (siehe Crate-Header).

use crate::buffer::BufferReader;
use crate::error::DecodeError;

#[cfg(feature = "alloc")]
use crate::buffer::BufferWriter;
#[cfg(feature = "alloc")]
use crate::error::EncodeError;

// ============================================================================
// Traits
// ============================================================================

/// Wert kann in einen [`BufferWriter`] enkodiert werden.
#[cfg(feature = "alloc")]
pub trait CdrEncode {
    /// Schreibt diesen Wert in den Writer (alignment-bewusst).
    ///
    /// # Errors
    /// [`EncodeError`].
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError>;
}

/// Stub-Trait fuer no-alloc Builds: nicht nutzbar, da [`BufferWriter`]
/// nur mit `alloc`-Feature existiert. Wird mit Slice-basiertem Writer
/// in Phase 1 nachgereicht.
#[cfg(not(feature = "alloc"))]
pub trait CdrEncode {}

/// Wert kann aus einem [`BufferReader`] dekodiert werden.
pub trait CdrDecode: Sized {
    /// Liest diesen Wert aus dem Reader (alignment-bewusst).
    ///
    /// # Errors
    /// [`DecodeError`].
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError>;
}

// ============================================================================
// Primitive Encoder/Decoder
// ============================================================================

#[cfg(feature = "alloc")]
impl CdrEncode for u8 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u8(*self)
    }
}

impl CdrDecode for u8 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        reader.read_u8()
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for i8 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u8(*self as u8)
    }
}

impl CdrDecode for i8 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(reader.read_u8()? as i8)
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for u16 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u16(*self)
    }
}

impl CdrDecode for u16 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        reader.read_u16()
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for i16 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u16(*self as u16)
    }
}

impl CdrDecode for i16 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(reader.read_u16()? as i16)
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for u32 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u32(*self)
    }
}

impl CdrDecode for u32 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        reader.read_u32()
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for i32 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u32(*self as u32)
    }
}

impl CdrDecode for i32 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(reader.read_u32()? as i32)
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for u64 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u64(*self)
    }
}

impl CdrDecode for u64 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        reader.read_u64()
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for i64 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u64(*self as u64)
    }
}

impl CdrDecode for i64 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(reader.read_u64()? as i64)
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for f32 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u32(self.to_bits())
    }
}

impl CdrDecode for f32 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self::from_bits(reader.read_u32()?))
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for f64 {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u64(self.to_bits())
    }
}

impl CdrDecode for f64 {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self::from_bits(reader.read_u64()?))
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for bool {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        writer.write_u8(u8::from(*self))
    }
}

impl CdrDecode for bool {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let offset = reader.position();
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(DecodeError::InvalidBool {
                value: other,
                offset,
            }),
        }
    }
}

#[cfg(feature = "alloc")]
impl CdrEncode for char {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        // XCDR2 wchar32 (UTF-32 Codepoint).
        writer.write_u32(*self as u32)
    }
}

impl CdrDecode for char {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let offset = reader.position();
        let value = reader.read_u32()?;
        char::from_u32(value).ok_or(DecodeError::InvalidChar { value, offset })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::Endianness;

    #[cfg(feature = "alloc")]
    fn roundtrip_alloc<T>(value: T)
    where
        T: CdrEncode + CdrDecode + PartialEq + core::fmt::Debug,
    {
        let mut w = BufferWriter::new(Endianness::Little);
        value.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = T::decode(&mut r).expect("decode");
        assert_eq!(decoded, value);
        assert_eq!(r.remaining(), 0);
    }

    #[cfg(feature = "alloc")]
    fn roundtrip_be_alloc<T>(value: T)
    where
        T: CdrEncode + CdrDecode + PartialEq + core::fmt::Debug,
    {
        let mut w = BufferWriter::new(Endianness::Big);
        value.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = T::decode(&mut r).expect("decode");
        assert_eq!(decoded, value);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn u8_roundtrip() {
        roundtrip_alloc(0u8);
        roundtrip_alloc(0xABu8);
        roundtrip_alloc(0xFFu8);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn i8_roundtrip() {
        roundtrip_alloc(0i8);
        roundtrip_alloc(-1i8);
        roundtrip_alloc(127i8);
        roundtrip_alloc(-128i8);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn u16_roundtrip_le_and_be() {
        roundtrip_alloc(0x1234u16);
        roundtrip_be_alloc(0x1234u16);
        roundtrip_alloc(u16::MAX);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn i16_roundtrip() {
        roundtrip_alloc(-1i16);
        roundtrip_alloc(i16::MIN);
        roundtrip_alloc(i16::MAX);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn u32_roundtrip() {
        roundtrip_alloc(0xDEAD_BEEFu32);
        roundtrip_be_alloc(0xCAFE_BABEu32);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn i32_roundtrip() {
        roundtrip_alloc(-1i32);
        roundtrip_alloc(i32::MIN);
        roundtrip_alloc(i32::MAX);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn u64_roundtrip() {
        roundtrip_alloc(0x0102_0304_0506_0708u64);
        roundtrip_be_alloc(0x0102_0304_0506_0708u64);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn i64_roundtrip() {
        roundtrip_alloc(-1i64);
        roundtrip_alloc(i64::MIN);
        roundtrip_alloc(i64::MAX);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn f32_roundtrip_normal_values() {
        roundtrip_alloc(0.0f32);
        roundtrip_alloc(1.5f32);
        roundtrip_alloc(-2.5f32);
        roundtrip_alloc(f32::MIN_POSITIVE);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn f32_roundtrip_special() {
        // NaN ist nicht selbst-equal — nutze to_bits-Vergleich.
        let mut w = BufferWriter::new(Endianness::Little);
        f32::NAN.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = f32::decode(&mut r).unwrap();
        assert!(decoded.is_nan());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn f64_roundtrip() {
        roundtrip_alloc(0.0f64);
        roundtrip_alloc(core::f64::consts::PI);
        roundtrip_alloc(-1.0e-308f64);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn bool_roundtrip() {
        roundtrip_alloc(true);
        roundtrip_alloc(false);
    }

    #[test]
    fn bool_decode_rejects_other_values() {
        let bytes = [0xFFu8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = bool::decode(&mut r);
        assert!(matches!(
            res,
            Err(DecodeError::InvalidBool {
                value: 0xFF,
                offset: 0
            })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn char_roundtrip_ascii_and_unicode() {
        roundtrip_alloc('A');
        roundtrip_alloc('z');
        roundtrip_alloc('0');
        roundtrip_alloc('🦀'); // Unicode-Code-Point ueber BMP
        roundtrip_alloc('ä');
    }

    #[test]
    fn char_decode_rejects_surrogate() {
        // 0xD800 ist Surrogate-Pair-Marker, kein gueltiger Codepoint.
        let bytes = [0x00, 0xD8, 0x00, 0x00];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = char::decode(&mut r);
        assert!(matches!(
            res,
            Err(DecodeError::InvalidChar { value: 0xD800, .. })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn primitives_align_correctly_when_mixed() {
        // u8 + u16 + u32 + u64 → erwartet 16 Byte (1 + 1pad + 2 + 4 + 8)
        let mut w = BufferWriter::new(Endianness::Little);
        1u8.encode(&mut w).unwrap();
        2u16.encode(&mut w).unwrap();
        3u32.encode(&mut w).unwrap();
        4u64.encode(&mut w).unwrap();
        assert_eq!(w.position(), 16);

        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(u8::decode(&mut r).unwrap(), 1);
        assert_eq!(u16::decode(&mut r).unwrap(), 2);
        assert_eq!(u32::decode(&mut r).unwrap(), 3);
        assert_eq!(u64::decode(&mut r).unwrap(), 4);
    }
}
