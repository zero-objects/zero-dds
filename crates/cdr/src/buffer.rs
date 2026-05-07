// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Alignment-tracking Buffer-Reader/-Writer fuer XCDR.
//!
//! XCDR-Daten sind alignment-pflichtig: ein `u32`-Feld muss an einer
//! 4-Byte-Boundary beginnen, ein `u64` an 8-Byte-Boundary etc. Der
//! Encoder fuegt vor jedem Write die noetigen Padding-Bytes (Wert 0)
//! ein; der Decoder skipped sie.
//!
//! Alignment wird relativ zum **Stream-Anfang** (Offset 0) berechnet,
//! nicht relativ zur aktuellen Position innerhalb eines verschachtelten
//! Members. Das entspricht OMG-XTypes §7.4.1: "All elements are aligned
//! to a multiple of their alignment requirement, relative to the
//! beginning of the encapsulation".
//!
//! Bewusste Architektur-Wahl: nur dynamischer Vec-basierter Writer
//! (alloc-Feature). Slice-basierter Writer für no_std-without-alloc
//! ist nicht implementiert — `alloc` ist via `zerodds-foundation`
//! ohnehin transitive Mandatory-Dep.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::endianness::Endianness;
use crate::error::DecodeError;
#[cfg(feature = "alloc")]
use crate::error::EncodeError;

// ============================================================================
// BufferWriter
// ============================================================================

/// Schreib-Buffer mit Alignment-Tracking und konfigurierbarer Endianness.
///
/// Phase 0 nutzt `Vec<u8>` als Backing — wachsendes alloc-Feature.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct BufferWriter {
    bytes: Vec<u8>,
    endianness: Endianness,
}

#[cfg(feature = "alloc")]
impl BufferWriter {
    /// Erstellt einen leeren Writer mit der gegebenen Endianness.
    #[must_use]
    pub fn new(endianness: Endianness) -> Self {
        Self {
            bytes: Vec::new(),
            endianness,
        }
    }

    /// Erstellt einen Writer mit vor-allokierter Kapazitaet.
    #[must_use]
    pub fn with_capacity(endianness: Endianness, cap: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(cap),
            endianness,
        }
    }

    /// Liefert die aktuelle Endianness.
    #[must_use]
    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// Aktuelle Position (== bisher geschriebene Byte-Anzahl).
    #[must_use]
    pub fn position(&self) -> usize {
        self.bytes.len()
    }

    /// Konsumiert den Writer und liefert den geschriebenen Buffer.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Read-only Sicht auf den bisher geschriebenen Buffer.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Fuegt Null-Padding ein, bis die aktuelle Position ein Vielfaches
    /// von `alignment` ist. Alignment muss eine Zweierpotenz sein
    /// (1, 2, 4, 8); andere Werte sind in XCDR nicht definiert.
    pub fn align(&mut self, alignment: usize) {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let pos = self.bytes.len();
        let pad = padding_for(pos, alignment);
        for _ in 0..pad {
            self.bytes.push(0);
        }
    }

    /// Schreibt rohe Bytes ohne Alignment.
    ///
    /// # Errors
    /// Liefert nie einen Fehler bei `Vec`-basiertem Backing — die
    /// Signatur ist symmetrisch zum Slice-basierten Writer (Phase 1).
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), EncodeError> {
        self.bytes.extend_from_slice(data);
        Ok(())
    }

    /// Schreibt ein einzelnes Byte (1-Byte Alignment).
    ///
    /// # Errors
    /// Wie `write_bytes`.
    pub fn write_u8(&mut self, value: u8) -> Result<(), EncodeError> {
        self.bytes.push(value);
        Ok(())
    }

    /// Aligned + schreibt `u16`.
    ///
    /// # Errors
    /// Wie `write_bytes`.
    pub fn write_u16(&mut self, value: u16) -> Result<(), EncodeError> {
        self.align(2);
        self.write_bytes(&self.endianness.write_u16(value))
    }

    /// Aligned + schreibt `u32`.
    ///
    /// # Errors
    /// Wie `write_bytes`.
    pub fn write_u32(&mut self, value: u32) -> Result<(), EncodeError> {
        self.align(4);
        self.write_bytes(&self.endianness.write_u32(value))
    }

    /// Aligned + schreibt `u64`.
    ///
    /// # Errors
    /// Wie `write_bytes`.
    pub fn write_u64(&mut self, value: u64) -> Result<(), EncodeError> {
        self.align(8);
        self.write_bytes(&self.endianness.write_u64(value))
    }

    /// Schreibt einen CDR-String (§9.3.2.7): 4-byte length (inkl.
    /// Null-Terminator) + UTF-8 Bytes + 1 Null-Byte. Kein Trailing-
    /// Padding — das naechste Feld aligned sich selbst.
    ///
    /// # Errors
    /// Wie `write_bytes`.
    pub fn write_string(&mut self, s: &str) -> Result<(), EncodeError> {
        let bytes = s.as_bytes();
        // Laenge = s.len() + 1 fuer null-terminator
        let len = u32::try_from(bytes.len().saturating_add(1)).map_err(|_| {
            EncodeError::ValueOutOfRange {
                message: "CDR string length exceeds u32::MAX",
            }
        })?;
        self.write_u32(len)?;
        self.write_bytes(bytes)?;
        self.write_u8(0)
    }
}

// ============================================================================
// BufferReader
// ============================================================================

/// Lese-Buffer mit Alignment-Tracking.
#[derive(Debug, Clone)]
pub struct BufferReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    endianness: Endianness,
}

impl<'a> BufferReader<'a> {
    /// Konstruiert einen Reader ueber den gegebenen Slice.
    #[must_use]
    pub fn new(bytes: &'a [u8], endianness: Endianness) -> Self {
        Self {
            bytes,
            pos: 0,
            endianness,
        }
    }

    /// Aktuelle Endianness.
    #[must_use]
    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// Aktuelle Position im Stream.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Verbleibende Bytes bis zum Ende.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    /// Skipt Padding bis zur naechsten `alignment`-Boundary.
    ///
    /// # Errors
    /// `UnexpectedEof`, wenn nicht genug Bytes fuer das Padding da sind.
    pub fn align(&mut self, alignment: usize) -> Result<(), DecodeError> {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be power of two"
        );
        let pad = padding_for(self.pos, alignment);
        if self.remaining() < pad {
            return Err(DecodeError::UnexpectedEof {
                needed: pad,
                offset: self.pos,
            });
        }
        self.pos += pad;
        Ok(())
    }

    /// Liest exakt `n` Bytes als Slice (ohne Alignment).
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::UnexpectedEof {
                needed: n,
                offset: self.pos,
            });
        }
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Liest ein einzelnes Byte.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let slice = self.read_bytes(1)?;
        Ok(slice[0])
    }

    /// Aligned + liest `u16`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u16(&mut self) -> Result<u16, DecodeError> {
        self.align(2)?;
        let slice = self.read_bytes(2)?;
        let mut buf = [0u8; 2];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u16(buf))
    }

    /// Aligned + liest `u32`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u32(&mut self) -> Result<u32, DecodeError> {
        self.align(4)?;
        let slice = self.read_bytes(4)?;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u32(buf))
    }

    /// Aligned + liest `u64`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u64(&mut self) -> Result<u64, DecodeError> {
        self.align(8)?;
        let slice = self.read_bytes(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u64(buf))
    }

    /// Liest einen CDR-String (§9.3.2.7): 4-byte length (inkl. Null-
    /// Terminator) + UTF-8 Bytes + 1 Null-Byte. Gibt den String **ohne**
    /// Terminator zurueck.
    ///
    /// # Errors
    /// `UnexpectedEof` bei zu kurzen Daten; `InvalidData` bei nicht-UTF-8
    /// Bytes, fehlendem Null-Terminator oder `length == 0`.
    #[cfg(feature = "alloc")]
    pub fn read_string(&mut self) -> Result<alloc::string::String, DecodeError> {
        use alloc::string::String;
        let start = self.pos;
        let len = self.read_u32()? as usize;
        if len == 0 {
            return Err(DecodeError::InvalidString {
                offset: start,
                reason: "length must be > 0 (null terminator required)",
            });
        }
        let raw = self.read_bytes(len)?;
        // last byte must be null terminator
        if raw[len - 1] != 0 {
            return Err(DecodeError::InvalidString {
                offset: start,
                reason: "missing null terminator",
            });
        }
        String::from_utf8(raw[..len - 1].to_vec())
            .map_err(|_| DecodeError::InvalidUtf8 { offset: start + 4 })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Berechnet die Anzahl Padding-Bytes, um `pos` an die naechste
/// Vielfaches-von-`alignment`-Boundary zu schieben.
#[must_use]
pub fn padding_for(pos: usize, alignment: usize) -> usize {
    let mask = alignment - 1;
    (alignment - (pos & mask)) & mask
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::vec;

    #[test]
    fn padding_for_zero_position_is_zero() {
        assert_eq!(padding_for(0, 4), 0);
        assert_eq!(padding_for(0, 8), 0);
    }

    #[test]
    fn padding_for_already_aligned_is_zero() {
        assert_eq!(padding_for(8, 4), 0);
        assert_eq!(padding_for(16, 8), 0);
    }

    #[test]
    fn padding_for_one_byte_to_4_align_is_three() {
        assert_eq!(padding_for(1, 4), 3);
    }

    #[test]
    fn padding_for_three_bytes_to_8_align_is_five() {
        assert_eq!(padding_for(3, 8), 5);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_writes_u8_without_padding() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(0xAB).unwrap();
        assert_eq!(w.as_bytes(), &[0xAB]);
        assert_eq!(w.position(), 1);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_aligns_u32_after_u8() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(0xAB).unwrap();
        w.write_u32(0xDEAD_BEEF).unwrap();
        // 1 byte 0xAB + 3 bytes padding + 4 bytes LE u32
        assert_eq!(w.as_bytes(), &[0xAB, 0, 0, 0, 0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_aligns_u64_after_u8() {
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u8(0x01).unwrap();
        w.write_u64(0x0203_0405_0607_0809).unwrap();
        // 1 byte + 7 bytes padding + 8 bytes BE u64
        assert_eq!(
            w.as_bytes(),
            &[0x01, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_with_capacity_preserves_endianness() {
        let w = BufferWriter::with_capacity(Endianness::Big, 64);
        assert_eq!(w.endianness(), Endianness::Big);
        assert_eq!(w.position(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_into_bytes_returns_full_buffer() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(0xCAFE_BABE).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes, vec![0xBE, 0xBA, 0xFE, 0xCA]);
    }

    #[test]
    fn reader_reads_u8() {
        let bytes = [0xAB, 0xCD];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 0xAB);
        assert_eq!(r.position(), 1);
    }

    #[test]
    fn reader_aligns_before_u32() {
        let bytes = [0xAB, 0, 0, 0, 0xEF, 0xBE, 0xAD, 0xDE];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 0xAB);
        assert_eq!(r.read_u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn reader_aligns_before_u64_be() {
        let bytes = [0x01, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(r.read_u8().unwrap(), 0x01);
        assert_eq!(r.read_u64().unwrap(), 0x0203_0405_0607_0809);
    }

    #[test]
    fn reader_unexpected_eof_on_short_read() {
        let bytes = [0u8; 2];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = r.read_u32();
        // u32 verlangt 4 Byte ab Position 0; nur 2 verfuegbar.
        match res {
            Err(DecodeError::UnexpectedEof {
                needed: 4,
                offset: 0,
            }) => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn reader_eof_on_align_overflow() {
        let bytes = [0u8; 1];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let _ = r.read_u8().unwrap();
        let res = r.align(8);
        assert!(matches!(res, Err(DecodeError::UnexpectedEof { .. })));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_reader_roundtrip_mixed_primitives() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(1).unwrap();
        w.write_u16(0x1234).unwrap();
        w.write_u32(0x5678_9ABC).unwrap();
        w.write_u64(0x0102_0304_0506_0708).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.read_u16().unwrap(), 0x1234);
        assert_eq!(r.read_u32().unwrap(), 0x5678_9ABC);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.remaining(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_roundtrip_ascii() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("ChatterTopic").unwrap();
        let bytes = w.into_bytes();
        // length=13 (12 chars + null), 12 bytes ascii, 1 null
        assert_eq!(&bytes[0..4], &[13, 0, 0, 0]);
        assert_eq!(&bytes[4..16], b"ChatterTopic");
        assert_eq!(bytes[16], 0);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "ChatterTopic");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_empty() {
        // "": length=1 (just null), 0 chars, 1 null
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("").unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[..], &[1, 0, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_utf8_multibyte() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("Zähler").unwrap(); // ä = 2 bytes UTF-8
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "Zähler");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_length_zero() {
        let bytes = [0u8, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidString { .. })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_missing_null_terminator() {
        // length=4, aber nur ASCII ohne null-terminator
        let bytes = [4u8, 0, 0, 0, b'A', b'B', b'C', b'D'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidString { .. })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_invalid_utf8() {
        // length=3, bytes = 0xFF, 0xFE, null → invalid UTF-8
        let bytes = [3u8, 0, 0, 0, 0xFF, 0xFE, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidUtf8 { .. })
        ));
    }

    // ---- Mutation-Killer fuer BufferReader ----

    /// Faengt Mutation `endianness -> Default::default()`. Reader muss
    /// die Endianness aus dem Konstruktor zurueckgeben, nicht Default.
    /// Default::default() ist Little — wir testen mit Big.
    #[test]
    fn reader_endianness_getter_returns_construction_value() {
        let r_be = BufferReader::new(&[0u8; 4], Endianness::Big);
        assert_eq!(r_be.endianness(), Endianness::Big);
        let r_le = BufferReader::new(&[0u8; 4], Endianness::Little);
        assert_eq!(r_le.endianness(), Endianness::Little);
    }

    /// Faengt Mutation `< -> <=` in `align`. align() darf NICHT erroren
    /// wenn `remaining()` exakt `pad` Bytes hat.
    #[test]
    fn reader_align_succeeds_when_remaining_equals_pad() {
        // pos=1, align=4 → pad=3, remaining muss >= 3 sein.
        // Genau 4 Bytes Puffer: read_u8 ueber 1 Byte, dann align(4)
        // braucht 3 pad-Bytes. Buffer hat nach read_u8 noch 3 Bytes
        // → remaining == pad (3). Original `<`: 3<3 false, kein Error.
        // Mutation `<=`: 3<=3 true, Error.
        let bytes = [0xAA, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        assert!(r.align(4).is_ok());
        assert_eq!(r.position(), 4);
    }

    /// Faengt Mutation `+= -> *=` auf `self.pos += pad` in align().
    /// align() muss pos VORWAERTS bewegen.
    #[test]
    fn reader_align_advances_position_strictly() {
        let bytes = [0xAA, 0, 0, 0, 1, 2, 3, 4];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        assert_eq!(r.position(), 1);
        r.align(4).unwrap();
        // pos *= pad = 1*3 = 3 mit Mutation; pos = 1+3 = 4 ohne Mutation.
        assert_eq!(r.position(), 4);
    }

    /// Faengt Mutation `+= -> *=` auf `self.pos += n` in read_bytes().
    /// read_bytes() muss pos um genau n vorwaerts bewegen.
    #[test]
    fn reader_read_bytes_advances_position_strictly() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let _ = r.read_bytes(3).unwrap();
        // pos *= n = 0*3 = 0 mit Mutation; pos = 0+3 = 3 ohne Mutation.
        assert_eq!(r.position(), 3);
        let _ = r.read_bytes(2).unwrap();
        // pos *= 2 = 3*2 = 6 mit Mutation; pos = 3+2 = 5 ohne Mutation.
        assert_eq!(r.position(), 5);
    }

    /// Faengt Mutation `read_u16 -> Ok(0)`. read_u16 muss tatsaechlich
    /// die Bytes lesen, nicht pauschal 0 zurueckgeben.
    #[test]
    fn reader_read_u16_returns_actual_bytes_not_zero() {
        let bytes = [0x12, 0x34];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u16().unwrap(), 0x3412);
        let mut r_be = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(r_be.read_u16().unwrap(), 0x1234);
    }

    /// Faengt Mutation `+ -> *` auf `start + 4` in der read_string-
    /// Fehler-Offset-Berechnung.
    ///
    /// Spec: offset zeigt auf den Beginn der utf-8-Daten = start + 4
    /// (nach dem 4-Byte-Length-Prefix). `start` wird VOR dem
    /// `read_u32`/Alignment gesetzt; bei pos=1 zu Beginn ist start=1
    /// und offset=5. Mutation `*` wuerde 1*4=4 liefern.
    #[test]
    fn reader_read_string_invalid_utf8_offset_is_start_plus_four() {
        // Vorgaenger-u8 verschiebt start auf 1 (nicht 0), damit `+` vs
        // `*` differenzieren: 1+4=5 vs 1*4=4.
        let mut bytes = vec![0xAB]; // pos=0
        // pad to 4: 3 zero bytes
        bytes.extend_from_slice(&[0, 0, 0]);
        // length=3 LE (inkl. Null-Terminator-Slot)
        bytes.extend_from_slice(&3u32.to_le_bytes());
        // 3 Bytes, letztes ist NUL — die ersten zwei sind invalid utf-8.
        bytes.extend_from_slice(&[0xFF, 0xFE, 0]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        let err = r.read_string().unwrap_err();
        // `start = self.pos` BEVOR read_u32. Nach read_u8 ist pos=1.
        // Also start=1, start+4=5. Mutation `*`: 1*4=4. Differenz reicht.
        match err {
            DecodeError::InvalidUtf8 { offset } => assert_eq!(offset, 5),
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }

    /// Zweite Variante mit start=2: 2+4=6 vs 2*4=8 — beidseitige
    /// Differenzierung (oben start=1: +1, unten start=2: +4).
    #[test]
    fn reader_read_string_invalid_utf8_offset_with_start_two() {
        let mut bytes = vec![0xAB, 0xCD]; // pos=0..2
        // pad to 4: 2 zero bytes
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&[0xFF, 0xFE, 0]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        r.read_u8().unwrap();
        let err = r.read_string().unwrap_err();
        match err {
            DecodeError::InvalidUtf8 { offset } => assert_eq!(offset, 6),
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }
}
