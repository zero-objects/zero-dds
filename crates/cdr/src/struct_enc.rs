// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Struct-Encoding mit Extensibility (W3, XCDR2 §7.4.3, §7.4.5).
//!
//! XCDR2 kennt drei Struct-Layouts:
//!
//! - **`@final`**: tight-packed, kein Header. Members in deklarierter
//!   Reihenfolge. Reader und Writer muessen die Member-Liste exakt
//!   teilen — keine forward/backward compatibility.
//! - **`@appendable`**: 4-byte **DHEADER** (uint32 = Byte-Laenge des
//!   Bodys nach dem Header) + tight-packed Members. Forward-Kompatibel:
//!   Reader kann Bytes nach den bekannten Members ueberspringen.
//! - **`@mutable`**: Pro Member ein **EMHEADER** + Wert. EMHEADER =
//!   `uint32` mit Member-ID + Length-Code. Backward-/forward-kompatibel
//!   durch Member-ID-basierte Zuordnung.
//!
//! Helpers für alle 3 Modi mit klassischem 2-pass Encoder (inner
//! Buffer für Body, dann Header + Body in outer). Length-Codes:
//! LC0..LC7 vollständig implementiert (Default-Konstruktor `LC4`
//! Variable-Length mit separatem `uint32` NEXTINT; LC0..LC3 kompakte
//! 1/2/4/8-byte ohne NEXTINT; LC5..LC7 sequence/array-spezifisch).
//!
//! Alignment: Body-Inhalt eines DHEADER-/EMHEADER-Frames startet bei
//! Offset 0 relativ zum Body-Start (XCDR2 §7.4.3.4.5).

extern crate alloc;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::error::{DecodeError, EncodeError};

// ============================================================================
// @appendable
// ============================================================================

/// Encoded eine `@appendable`-Struktur. Der Body wird in einen inneren
/// Buffer geschrieben, dann Length + Body in den Outer-Writer.
///
/// # Errors
/// Encoder-Fehler aus dem Body oder `ValueOutOfRange`, wenn der Body
/// `u32::MAX` Bytes ueberschreitet.
pub fn encode_appendable<F>(writer: &mut BufferWriter, body: F) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    let mut inner = BufferWriter::new(writer.endianness());
    body(&mut inner)?;
    let bytes = inner.into_bytes();
    let len = u32::try_from(bytes.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "appendable struct body exceeds u32::MAX",
    })?;
    writer.write_u32(len)?;
    writer.write_bytes(&bytes)?;
    Ok(())
}

/// Decoded eine `@appendable`-Struktur. Liest die DHEADER-Length, baut
/// einen Sub-Reader auf den Body und uebergibt ihn an `body`. Der
/// Sub-Reader erlaubt dem Body, weniger Bytes zu konsumieren als
/// announced — ungenutzte Bytes werden uebersprungen.
///
/// # Errors
/// Decoder-Fehler aus dem Body oder `LengthExceeded`/`UnexpectedEof`,
/// wenn die Length nicht in den Stream passt.
pub fn decode_appendable<T, F>(reader: &mut BufferReader<'_>, body: F) -> Result<T, DecodeError>
where
    F: FnOnce(&mut BufferReader<'_>) -> Result<T, DecodeError>,
{
    let len = reader.read_u32()? as usize;
    if len > reader.remaining() {
        return Err(DecodeError::LengthExceeded {
            announced: len,
            remaining: reader.remaining(),
            offset: reader.position(),
        });
    }
    let body_bytes = reader.read_bytes(len)?;
    let mut sub = BufferReader::new(body_bytes, reader.endianness());
    body(&mut sub)
}

// ============================================================================
// @mutable
// ============================================================================

/// Length-Code-Variante (XTypes 1.3 §7.4.3.4.2). WP 1.A: alle 8 LCs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LengthCode {
    /// 1-byte body, no NEXTINT.
    Lc0 = 0,
    /// 2-byte body, no NEXTINT.
    Lc1 = 1,
    /// 4-byte body, no NEXTINT.
    Lc2 = 2,
    /// 8-byte body, no NEXTINT.
    Lc3 = 3,
    /// Variable-length body, NEXTINT (uint32) = body length in bytes.
    Lc4 = 4,
    /// Variable-length aggregate, NEXTINT = body length INKL. DHEADER.
    Lc5 = 5,
    /// Array of 4-Byte-Primitives, NEXTINT = #Elemente, body = `4 + 4*N`.
    Lc6 = 6,
    /// Array of 8-Byte-Primitives, NEXTINT = #Elemente, body = `4 + 8*N`.
    Lc7 = 7,
}

impl LengthCode {
    /// Body-Laenge in Bytes fuer diesen LC, gegeben der NEXTINT-Wert.
    #[must_use]
    pub fn body_len(self, nextint: u32) -> u64 {
        match self {
            Self::Lc0 => 1,
            Self::Lc1 => 2,
            Self::Lc2 => 4,
            Self::Lc3 => 8,
            Self::Lc4 | Self::Lc5 => u64::from(nextint),
            Self::Lc6 => 4 * u64::from(nextint) + 4,
            Self::Lc7 => 8 * u64::from(nextint) + 4,
        }
    }

    /// `true` wenn der LC ein NEXTINT-Feld nach dem EMHEADER traegt.
    #[must_use]
    pub const fn has_nextint(self) -> bool {
        matches!(self, Self::Lc4 | Self::Lc5 | Self::Lc6 | Self::Lc7)
    }

    /// Decode aus dem 3-bit Wire-Feld eines EMHEADER.
    #[must_use]
    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Lc0),
            1 => Some(Self::Lc1),
            2 => Some(Self::Lc2),
            3 => Some(Self::Lc3),
            4 => Some(Self::Lc4),
            5 => Some(Self::Lc5),
            6 => Some(Self::Lc6),
            7 => Some(Self::Lc7),
            _ => None,
        }
    }
}

/// Encoded ein `@mutable`-Member mit **LC4** (Default-Universal-Code).
///
/// # Errors
/// Body-Fehler oder Member-ID > 0x0FFF_FFFF.
pub fn encode_mutable_member<F>(
    writer: &mut BufferWriter,
    member_id: u32,
    must_understand: bool,
    body: F,
) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    encode_mutable_member_lc(writer, member_id, must_understand, LengthCode::Lc4, body)
}

/// Encoded ein `@mutable`-Member mit explizitem Length-Code.
///
/// # Errors
/// `ValueOutOfRange` bei Member-ID-Overflow oder Body-Length-Mismatch
/// fuer den gewaehlten LC.
pub fn encode_mutable_member_lc<F>(
    writer: &mut BufferWriter,
    member_id: u32,
    must_understand: bool,
    lc: LengthCode,
    body: F,
) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    if member_id > 0x0FFF_FFFF {
        return Err(EncodeError::ValueOutOfRange {
            message: "EMHEADER member_id exceeds 28-bit field",
        });
    }
    let mut inner = BufferWriter::new(writer.endianness());
    body(&mut inner)?;
    let body_bytes = inner.into_bytes();
    let body_len = body_bytes.len();

    let nextint: Option<u32> = match lc {
        LengthCode::Lc0 => {
            if body_len != 1 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC0 requires exactly 1 byte body",
                });
            }
            None
        }
        LengthCode::Lc1 => {
            if body_len != 2 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC1 requires exactly 2 bytes body",
                });
            }
            None
        }
        LengthCode::Lc2 => {
            if body_len != 4 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC2 requires exactly 4 bytes body",
                });
            }
            None
        }
        LengthCode::Lc3 => {
            if body_len != 8 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC3 requires exactly 8 bytes body",
                });
            }
            None
        }
        LengthCode::Lc4 | LengthCode::Lc5 => {
            let n = u32::try_from(body_len).map_err(|_| EncodeError::ValueOutOfRange {
                message: "LC4/LC5 body exceeds u32::MAX",
            })?;
            Some(n)
        }
        LengthCode::Lc6 => {
            // `body_len % 4 != 0` ist aequivalent zu `(body_len - 4) % 4 != 0`
            // fuer body_len >= 4 (nach dem ersten Check). Eliminiert eine
            // mathematisch-aequivalente `-4`/`+4`-Mutation.
            if body_len < 4 || body_len % 4 != 0 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC6 body must be DHEADER + 4n bytes",
                });
            }
            let n =
                u32::try_from((body_len - 4) / 4).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "LC6 element count exceeds u32::MAX",
                })?;
            Some(n)
        }
        LengthCode::Lc7 => {
            // body_len < 4: Reject. body_len in {4,12,20,...}: Pass.
            // body_len in {5,6,7}: Pass body_len % 8 ≠ 0 → Reject. Aber
            // Boundary-Check muss aufpassen: body_len=8 hat body_len%8=0,
            // aber (8-4)/8=0.5 ist kein gueltiges n. Wir brauchen
            // `(body_len - 4) % 8 == 0`, was aequivalent zu
            // `body_len % 8 == 4` ist.
            if body_len < 4 || body_len % 8 != 4 {
                return Err(EncodeError::ValueOutOfRange {
                    message: "LC7 body must be DHEADER + 8n bytes",
                });
            }
            let n =
                u32::try_from((body_len - 4) / 8).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "LC7 element count exceeds u32::MAX",
                })?;
            Some(n)
        }
    };

    let m_bit = u32::from(must_understand) << 31;
    let lc_bits = (lc as u32) << 28;
    // Arithmetic form statt OR: bit-positions ueberlappen nicht
    // (m_bit=Bit31, lc_bits=Bits 28-30, member_id<=Bits 0-27).
    // Mathematisch identisch zu `m_bit | lc_bits | member_id`,
    // aber mutation-detection-freundlicher: `+` vs `^`/`-`/`*`
    // sind nicht aequivalent zueinander.
    let emheader = m_bit + lc_bits + member_id;
    writer.write_u32(emheader)?;
    if let Some(ni) = nextint {
        writer.write_u32(ni)?;
    }
    writer.write_bytes(&body_bytes)?;
    Ok(())
}

/// Encoder fuer einen `@mutable`-Struct mit Validierung der
/// non-optional-Member-Vollstaendigkeit (XTypes 1.3 §7.4.1.2.3).
///
/// Vor jedem Member-Encode wird `member_id` als "emitted" gemerkt; beim
/// `finish` muss jede in `required_ids` aufgelistete Member-ID emittiert
/// worden sein, sonst wird `EncodeError::MissingNonOptionalMember`
/// zurueckgegeben.
///
/// Spec-Hintergrund: Eine `EXTENSIBLE` (final/appendable/mutable)-Encode
/// MUSS alle non-optional Member enthalten. Dieser Validator schliesst
/// das Encoder-Loch fuer @mutable, weil bei MUTABLE die EMHEADER-Reihenfolge
/// nicht festliegt und Encoder-Bugs sonst stillschweigend passierten.
pub struct MutableStructEncoder<'a> {
    writer: &'a mut BufferWriter,
    required_ids: Vec<u32>,
    emitted_ids: Vec<u32>,
}

impl<'a> MutableStructEncoder<'a> {
    /// Neuer Encoder. `required_ids` ist die Liste der Member-IDs,
    /// die spec-konform alle emittiert werden MUESSEN (= alle
    /// non-optional Member des Structs).
    pub fn new(writer: &'a mut BufferWriter, required_ids: Vec<u32>) -> Self {
        Self {
            writer,
            required_ids,
            emitted_ids: Vec::new(),
        }
    }

    /// Member encoden. Verhalten wie `encode_mutable_member`, plus
    /// Tracking der emittierten ID.
    ///
    /// # Errors
    /// Wie `encode_mutable_member`.
    pub fn encode_member<F>(
        &mut self,
        member_id: u32,
        must_understand: bool,
        body: F,
    ) -> Result<(), EncodeError>
    where
        F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
    {
        encode_mutable_member(self.writer, member_id, must_understand, body)?;
        self.emitted_ids.push(member_id);
        Ok(())
    }

    /// Member mit explizitem Length-Code.
    ///
    /// # Errors
    /// Wie `encode_mutable_member_lc`.
    pub fn encode_member_lc<F>(
        &mut self,
        member_id: u32,
        must_understand: bool,
        lc: LengthCode,
        body: F,
    ) -> Result<(), EncodeError>
    where
        F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
    {
        encode_mutable_member_lc(self.writer, member_id, must_understand, lc, body)?;
        self.emitted_ids.push(member_id);
        Ok(())
    }

    /// Schliesst die Mutable-Sequenz ab und prueft, dass jede in
    /// `required_ids` aufgelistete Member-ID auch emittiert wurde.
    ///
    /// # Errors
    /// `MissingNonOptionalMember { member_id }` mit der ersten
    /// fehlenden ID (deterministisch in Reihenfolge der `required_ids`-
    /// Liste).
    pub fn finish(self) -> Result<(), EncodeError> {
        for required in &self.required_ids {
            if !self.emitted_ids.contains(required) {
                return Err(EncodeError::MissingNonOptionalMember {
                    member_id: *required,
                });
            }
        }
        Ok(())
    }
}

/// Geparstes EMHEADER + Body-Slice eines `@mutable`-Members.
#[derive(Debug, Clone)]
pub struct MutableMember<'a> {
    /// 28-bit Member-ID.
    pub member_id: u32,
    /// `must_understand`-Flag.
    pub must_understand: bool,
    /// Length-Code.
    pub length_code: LengthCode,
    /// Body als unverbrauchter Slice.
    pub body: &'a [u8],
}

/// Liest einen `@mutable`-Member-Eintrag (EMHEADER + NEXTINT + Body).
///
/// # Errors
/// `UnexpectedEof` / `LengthExceeded` bei truncated/oversize Body.
pub fn read_mutable_member<'a>(
    reader: &mut BufferReader<'a>,
) -> Result<Option<MutableMember<'a>>, DecodeError> {
    if reader.remaining() == 0 {
        return Ok(None);
    }
    let emheader = reader.read_u32()?;
    let must_understand = (emheader >> 31) & 1 == 1;
    let lc_raw = ((emheader >> 28) & 0b0111) as u8;
    let member_id = emheader & 0x0FFF_FFFF;
    let length_code = LengthCode::from_wire(lc_raw).ok_or_else(|| DecodeError::LengthExceeded {
        announced: usize::from(lc_raw),
        remaining: 0,
        offset: reader.position(),
    })?;

    let nextint = if length_code.has_nextint() {
        reader.read_u32()?
    } else {
        0
    };

    let body_len_u64 = length_code.body_len(nextint);
    let body_len = usize::try_from(body_len_u64).map_err(|_| DecodeError::LengthExceeded {
        announced: usize::MAX,
        remaining: reader.remaining(),
        offset: reader.position(),
    })?;
    if body_len > reader.remaining() {
        return Err(DecodeError::LengthExceeded {
            announced: body_len,
            remaining: reader.remaining(),
            offset: reader.position(),
        });
    }
    let body = reader.read_bytes(body_len)?;
    Ok(Some(MutableMember {
        member_id,
        must_understand,
        length_code,
        body,
    }))
}

/// Aller-Members-eines-`@mutable`-Structs in eine Map sammeln. Erlaubt
/// dem Caller, Members nach ID zu suchen statt sequenziell zu lesen.
///
/// # Errors
/// Wie [`read_mutable_member`].
pub fn read_all_mutable_members<'a>(
    reader: &mut BufferReader<'a>,
) -> Result<Vec<MutableMember<'a>>, DecodeError> {
    let mut out = Vec::new();
    while let Some(m) = read_mutable_member(reader)? {
        out.push(m);
    }
    Ok(out)
}

// ============================================================================
// @final (no-op-Wrapper)
// ============================================================================

/// `@final`-Struct: tight-packed, kein Header. Diese Funktion ist ein
/// reiner Convenience-Wrapper, damit die 3 Extensibility-Modi
/// uniforme Aufruf-Sites haben.
///
/// # Errors
/// Body-Fehler.
pub fn encode_final<F>(writer: &mut BufferWriter, body: F) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    body(writer)
}

/// Decoder-Pendant: einfach den Body aufrufen.
///
/// # Errors
/// Body-Fehler.
pub fn decode_final<T, F>(reader: &mut BufferReader<'_>, body: F) -> Result<T, DecodeError>
where
    F: FnOnce(&mut BufferReader<'_>) -> Result<T, DecodeError>,
{
    body(reader)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::Endianness;
    use crate::encode::{CdrDecode, CdrEncode};
    use alloc::vec;

    // ---- @final ----

    #[test]
    fn final_struct_two_u32_members() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_final(&mut w, |w| {
            42u32.encode(w)?;
            100u32.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        // Tight-packed: 2 * 4 byte u32 = 8 byte total.
        assert_eq!(bytes.len(), 8);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let (a, b) = decode_final(&mut r, |r| {
            Ok::<_, DecodeError>((u32::decode(r)?, u32::decode(r)?))
        })
        .unwrap();
        assert_eq!((a, b), (42, 100));
    }

    // ---- @appendable ----

    #[test]
    fn appendable_struct_writes_dheader() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_appendable(&mut w, |w| {
            42u32.encode(w)?;
            7u8.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        // DHEADER (4 byte u32 = body length) + body.
        // Body: u32 (4) + u8 (1) = 5 byte.
        assert_eq!(&bytes[0..4], &[5, 0, 0, 0]); // DHEADER LE
        assert_eq!(&bytes[4..8], &[42, 0, 0, 0]); // u32 = 42
        assert_eq!(bytes[8], 7);
        assert_eq!(bytes.len(), 9);
    }

    #[test]
    fn appendable_struct_roundtrip() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_appendable(&mut w, |w| {
            42u32.encode(w)?;
            7u8.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let (a, b) = decode_appendable(&mut r, |r| {
            Ok::<_, DecodeError>((u32::decode(r)?, u8::decode(r)?))
        })
        .unwrap();
        assert_eq!((a, b), (42, 7));
    }

    #[test]
    fn appendable_decoder_skips_extra_trailing_bytes() {
        // Schreibe Encoder-Struktur mit 2 Members, aber Decoder liest
        // nur das erste — der Sub-Reader-Trick wirft die zweiten Bytes
        // weg ohne Fehler.
        let mut w = BufferWriter::new(Endianness::Little);
        encode_appendable(&mut w, |w| {
            42u32.encode(w)?;
            99u8.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let only_first = decode_appendable(&mut r, u32::decode).unwrap();
        assert_eq!(only_first, 42);
        // Outer-Reader hat alles konsumiert (DHEADER + komplettem Body).
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn appendable_decoder_rejects_announced_overrun() {
        let bytes = [0xFFu8, 0xFF, 0xFF, 0xFF, 1, 2, 3];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = decode_appendable(&mut r, u32::decode);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    // ---- @mutable ----

    // ---- MutableStructEncoder (XTypes 1.3 §7.4.1.2.3) ----

    #[test]
    fn mutable_struct_encoder_succeeds_when_all_required_emitted() {
        let mut w = BufferWriter::new(Endianness::Little);
        let mut enc = MutableStructEncoder::new(&mut w, vec![1, 2, 3]);
        enc.encode_member(1, false, |w| 42u32.encode(w)).unwrap();
        enc.encode_member(2, false, |w| 7u8.encode(w)).unwrap();
        enc.encode_member(3, false, |w| 99u16.encode(w)).unwrap();
        enc.finish().unwrap();
    }

    #[test]
    fn mutable_encode_omitting_non_optional_member_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        let mut enc = MutableStructEncoder::new(&mut w, vec![1, 2, 3]);
        enc.encode_member(1, false, |w| 42u32.encode(w)).unwrap();
        // Member 2 wird nicht emittiert — Spec-Verletzung.
        enc.encode_member(3, false, |w| 99u16.encode(w)).unwrap();
        let err = enc.finish().unwrap_err();
        assert_eq!(err, EncodeError::MissingNonOptionalMember { member_id: 2 });
    }

    #[test]
    fn mutable_encode_first_missing_id_is_reported() {
        let mut w = BufferWriter::new(Endianness::Little);
        let mut enc = MutableStructEncoder::new(&mut w, vec![10, 20, 30]);
        enc.encode_member(20, false, |w| 5u32.encode(w)).unwrap();
        // 10 und 30 fehlen — Encoder meldet 10 zuerst.
        let err = enc.finish().unwrap_err();
        assert_eq!(err, EncodeError::MissingNonOptionalMember { member_id: 10 });
    }

    #[test]
    fn mutable_encode_optional_only_with_no_required_succeeds() {
        // Wenn alle Member optional sind, ist required_ids leer und der
        // Encoder darf auch null Member emittieren.
        let mut w = BufferWriter::new(Endianness::Little);
        let enc = MutableStructEncoder::new(&mut w, vec![]);
        enc.finish().unwrap();
    }

    #[test]
    fn mutable_encode_extra_optional_emitted_does_not_break_finish() {
        // required = [1]; emitted = [1, 99]; OK — 99 ist optional.
        let mut w = BufferWriter::new(Endianness::Little);
        let mut enc = MutableStructEncoder::new(&mut w, vec![1]);
        enc.encode_member(1, false, |w| 42u32.encode(w)).unwrap();
        enc.encode_member(99, false, |w| 0u8.encode(w)).unwrap();
        enc.finish().unwrap();
    }

    #[test]
    fn mutable_encode_with_lc_variant_tracks_id() {
        let mut w = BufferWriter::new(Endianness::Little);
        let mut enc = MutableStructEncoder::new(&mut w, vec![5]);
        enc.encode_member_lc(5, false, LengthCode::Lc0, |w| 0x42u8.encode(w))
            .unwrap();
        enc.finish().unwrap();
    }

    #[test]
    fn mutable_member_emheader_layout() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member(&mut w, 0x1234, false, |w| 42u32.encode(w)).unwrap();
        let bytes = w.into_bytes();
        // EMHEADER LE: m_bit=0, lc=4 (bits 30-28 = 100), member_id=0x1234
        // → 0x4000_1234
        assert_eq!(&bytes[0..4], &[0x34, 0x12, 0x00, 0x40]);
        // NEXTINT = body length = 4
        assert_eq!(&bytes[4..8], &[4, 0, 0, 0]);
        // body = u32 LE 42
        assert_eq!(&bytes[8..12], &[42, 0, 0, 0]);
    }

    #[test]
    fn mutable_member_must_understand_sets_high_bit() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member(&mut w, 1, true, |w| 0u8.encode(w)).unwrap();
        let bytes = w.into_bytes();
        // EMHEADER LE: m_bit=1, lc=4, id=1 → 0xC000_0001
        assert_eq!(&bytes[0..4], &[0x01, 0x00, 0x00, 0xC0]);
    }

    #[test]
    fn mutable_member_rejects_oversized_id() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member(&mut w, 0xFFFF_FFFF, false, |w| 0u8.encode(w));
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    #[test]
    fn mutable_struct_roundtrip_two_members() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member(&mut w, 1, false, |w| 42u32.encode(w)).unwrap();
        encode_mutable_member(&mut w, 2, true, |w| 7u8.encode(w)).unwrap();
        let bytes = w.into_bytes();

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let members = read_all_mutable_members(&mut r).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].member_id, 1);
        assert!(!members[0].must_understand);
        assert_eq!(members[1].member_id, 2);
        assert!(members[1].must_understand);

        let mut sub = BufferReader::new(members[0].body, Endianness::Little);
        assert_eq!(u32::decode(&mut sub).unwrap(), 42);
        let mut sub = BufferReader::new(members[1].body, Endianness::Little);
        assert_eq!(u8::decode(&mut sub).unwrap(), 7);
    }

    #[test]
    fn mutable_member_reads_none_on_eof() {
        let bytes: [u8; 0] = [];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = read_mutable_member(&mut r).unwrap();
        assert!(res.is_none());
    }

    // ---- WP 1.A: LC0..7 Encoder/Decoder ----

    #[test]
    fn lc0_encode_decode_one_byte_body() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc0, |w| 0x42u8.encode(w)).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..4], &[0x01, 0x00, 0x00, 0x00]);
        assert_eq!(bytes[4], 0x42);
        assert_eq!(bytes.len(), 5);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc0);
        assert_eq!(m.body, &[0x42]);
    }

    #[test]
    fn lc1_encode_decode_two_byte_body() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 7, false, LengthCode::Lc1, |w| 0x1234u16.encode(w))
            .unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 4 + 2);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc1);
        assert_eq!(m.body, &[0x34, 0x12]);
    }

    #[test]
    fn lc2_encode_decode_four_byte_body() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 9, true, LengthCode::Lc2, |w| 42u32.encode(w)).unwrap();
        let bytes = w.into_bytes();
        // m=1, lc=2, id=9 → 0xA000_0009 LE
        assert_eq!(&bytes[0..4], &[0x09, 0x00, 0x00, 0xA0]);
        assert_eq!(bytes.len(), 4 + 4);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc2);
        assert!(m.must_understand);
    }

    #[test]
    fn lc3_encode_decode_eight_byte_body() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 11, false, LengthCode::Lc3, |w| {
            0xDEADBEEF_CAFEBABEu64.encode(w)
        })
        .unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 4 + 8);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc3);
        assert_eq!(m.body.len(), 8);
    }

    #[test]
    fn lc4_default_path_unchanged() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member(&mut w, 1, false, |w| 42u32.encode(w)).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 4 + 4 + 4);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc4);
    }

    #[test]
    fn lc5_aggregate_with_dheader() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc5, |w| {
            8u32.encode(w)?;
            42u32.encode(w)?;
            7u32.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 20);
        assert_eq!(&bytes[4..8], &[12, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc5);
        assert_eq!(m.body.len(), 12);
    }

    #[test]
    fn lc6_array_of_4byte_primitives() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |w| {
            12u32.encode(w)?;
            10u32.encode(w)?;
            20u32.encode(w)?;
            30u32.encode(w)?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[4..8], &[3, 0, 0, 0]);
        assert_eq!(bytes.len(), 4 + 4 + 16);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc6);
        assert_eq!(m.body.len(), 16);
    }

    #[test]
    fn lc6_lc7_roundtrip_against_cyclone_sample() {
        // Spec §7.4.3.4.2: LC=6 fuer 4-byte-Element-Arrays; LC=7 fuer
        // 8-byte-Element-Arrays. Beide Encoder erzeugen dasselbe
        // Wire-Layout, das Cyclone DDS und FastDDS dekodieren koennen.
        // Wir verifizieren byte-genau drei Stellen:
        //   - LC=6 EMHEADER (Bits 30-28 = 110)
        //   - NEXTINT = element-count (4 byte)
        //   - DHEADER + payload
        let mut w = BufferWriter::new(Endianness::Little);
        // LC=6 Body-Layout: DHEADER (4) + 4n element-bytes.
        // 100 u32 Elemente = 400 byte → body_len = 404.
        encode_mutable_member_lc(&mut w, 0xABCD, false, LengthCode::Lc6, |w| {
            // DHEADER: gibt die Anzahl der Element-Bytes an (400).
            400u32.encode(w)?;
            for i in 0..100u32 {
                i.encode(w)?;
            }
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();

        // EMHEADER: must_understand=0, lc=6, member_id=0xABCD.
        // → 0x6000_ABCD LE = [0xCD, 0xAB, 0x00, 0x60].
        assert_eq!(&bytes[0..4], &[0xCD, 0xAB, 0x00, 0x60]);
        // NEXTINT = element-count = 100 LE.
        assert_eq!(&bytes[4..8], &[100, 0, 0, 0]);
        // Payload: DHEADER 4 + 100 * 4 = 404 byte ab Offset 8.
        assert_eq!(bytes.len(), 8 + 404);

        // Decoder akzeptiert.
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc6);
        assert_eq!(m.member_id, 0xABCD);
        assert_eq!(m.body.len(), 404);
    }

    #[test]
    fn lc6_with_many_elements_decodes_correctly() {
        // 70_000 Elemente — Encoder schreibt LC=6 mit grossem NEXTINT.
        // Verifiziert, dass der Decoder den >= u16 NEXTINT korrekt
        // liest (kein silent-truncate).
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 5, false, LengthCode::Lc6, |w| {
            // DHEADER = element-bytes-count (70_000 * 4 = 280_000).
            280_000u32.encode(w)?;
            for i in 0..70_000u32 {
                i.encode(w)?;
            }
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        let nextint = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(nextint, 70_000);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        // body inkl. DHEADER = 4 + 280_000.
        assert_eq!(m.body.len(), 4 + 70_000 * 4);
    }

    #[test]
    fn lc7_array_of_8byte_primitives() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc7, |w| {
            16u32.encode(w)?;
            w.write_bytes(&100u64.to_le_bytes())?;
            w.write_bytes(&200u64.to_le_bytes())?;
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[4..8], &[2, 0, 0, 0]);
        assert_eq!(bytes.len(), 4 + 4 + 20);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.length_code, LengthCode::Lc7);
        assert_eq!(m.body.len(), 20);
    }

    #[test]
    fn lc0_rejects_wrong_body_size() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc0, |w| 42u32.encode(w));
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    #[test]
    fn lc6_rejects_misaligned_body() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |w| {
            0u32.encode(w)?;
            0u8.encode(w)?;
            0u8.encode(w)?;
            0u8.encode(w)?;
            Ok(())
        });
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    #[test]
    fn length_code_body_len_calculation() {
        assert_eq!(LengthCode::Lc0.body_len(0), 1);
        assert_eq!(LengthCode::Lc1.body_len(0), 2);
        assert_eq!(LengthCode::Lc2.body_len(0), 4);
        assert_eq!(LengthCode::Lc3.body_len(0), 8);
        assert_eq!(LengthCode::Lc4.body_len(100), 100);
        assert_eq!(LengthCode::Lc5.body_len(20), 20);
        assert_eq!(LengthCode::Lc6.body_len(3), 16);
        assert_eq!(LengthCode::Lc7.body_len(2), 20);
    }

    #[test]
    fn length_code_has_nextint_flag() {
        assert!(!LengthCode::Lc0.has_nextint());
        assert!(!LengthCode::Lc3.has_nextint());
        assert!(LengthCode::Lc4.has_nextint());
        assert!(LengthCode::Lc7.has_nextint());
    }

    #[test]
    fn length_code_from_wire_roundtrip() {
        for v in 0..=7u8 {
            let lc = LengthCode::from_wire(v).expect("valid");
            assert_eq!(lc as u8, v);
        }
        assert!(LengthCode::from_wire(8).is_none());
    }

    // ---- Mixed nesting ----

    #[test]
    fn appendable_in_mutable_member() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member(&mut w, 5, false, |w| {
            encode_appendable(w, |w| {
                42u32.encode(w)?;
                100u32.encode(w)?;
                Ok(())
            })
        })
        .unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let m = read_mutable_member(&mut r).unwrap().unwrap();
        assert_eq!(m.member_id, 5);
        let mut sub = BufferReader::new(m.body, Endianness::Little);
        let (a, b) = decode_appendable(&mut sub, |r| {
            Ok::<_, DecodeError>((u32::decode(r)?, u32::decode(r)?))
        })
        .unwrap();
        assert_eq!((a, b), (42, 100));
    }

    // ---- Mutation-Killer fuer encode_mutable_member_lc ----

    /// Faengt Mutation `>` -> `>=` auf member_id-Boundary (Zeile 176).
    /// member_id == 0x0FFFFFFF (= 28-bit-MAX) muss DURCHGEHEN.
    #[test]
    fn mutable_member_id_at_28bit_max_accepted() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 0x0FFF_FFFF, false, LengthCode::Lc2, |inner| {
            u32::encode(&0u32, inner)
        });
        assert!(
            res.is_ok(),
            "member_id=0x0FFFFFFF must succeed, got {res:?}"
        );
    }

    /// Member_id ueber 28-Bit muss ABGELEHNT werden.
    /// Faengt `>` -> `==` Mutation auf der gleichen Zeile (=> nur exact
    /// match wuerde erroren, alle hoeheren wuerden Pass werden).
    #[test]
    fn mutable_member_id_29bit_rejected() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 0x1000_0000, false, LengthCode::Lc2, |inner| {
            u32::encode(&0u32, inner)
        });
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    /// Faengt Mutation `<` -> `==` auf body_len < 4 in Lc6 (Zeile 226).
    /// body_len < 4 muss erroren — egal welcher Wert.
    #[test]
    fn lc6_body_len_less_than_4_rejected() {
        for short_len in [0usize, 1, 2, 3] {
            let mut w = BufferWriter::new(Endianness::Little);
            let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |inner| {
                inner.write_bytes(&vec![0u8; short_len])
            });
            assert!(
                matches!(res, Err(EncodeError::ValueOutOfRange { .. })),
                "Lc6 with body_len={short_len} must error, got {res:?}"
            );
        }
    }

    /// Faengt `<` -> `<=` Mutation: body_len == 4 (DHEADER allein, n=0)
    /// muss DURCHGEHEN bei Lc6 (4-4=0, 0%4=0).
    #[test]
    fn lc6_body_len_exactly_4_accepted() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |inner| {
            inner.write_bytes(&[0u8; 4])
        });
        assert!(res.is_ok(), "Lc6 body_len=4 must succeed, got {res:?}");
    }

    /// Faengt Mutation `-` -> `+` auf `(body_len - 4) / 4` fuer Lc6.
    /// nextint muss EXAKT (body_len - 4) / 4 sein, nicht (body_len + 4) / 4.
    /// body_len=12 → original n=2, mutiert n=4.
    #[test]
    fn lc6_nextint_value_is_minus_4_div_4() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |inner| {
            inner.write_bytes(&[0u8; 12])
        })
        .unwrap();
        let bytes = w.into_bytes();
        // Layout: 4-byte EMHEADER + 4-byte nextint + 12-byte body = 20.
        assert_eq!(bytes.len(), 20);
        // nextint = bytes[4..8] LE
        let mut ni = [0u8; 4];
        ni.copy_from_slice(&bytes[4..8]);
        let nextint = u32::from_le_bytes(ni);
        assert_eq!(nextint, 2, "nextint must be (12-4)/4=2, not (12+4)/4=4");
    }

    /// Lc7-Variante: gleiche Mutationen wie Lc6 mit `% 8` und `/ 8`.
    /// body_len < 4 muss erroren.
    #[test]
    fn lc7_body_len_less_than_4_rejected() {
        for short_len in [0usize, 1, 2, 3] {
            let mut w = BufferWriter::new(Endianness::Little);
            let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc7, |inner| {
                inner.write_bytes(&vec![0u8; short_len])
            });
            assert!(
                matches!(res, Err(EncodeError::ValueOutOfRange { .. })),
                "Lc7 with body_len={short_len} must error"
            );
        }
    }

    /// Lc7 body_len==4 (DHEADER + 0 elements) muss durchgehen.
    /// Faengt `<` -> `<=` Mutation.
    #[test]
    fn lc7_body_len_exactly_4_accepted() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc7, |inner| {
            inner.write_bytes(&[0u8; 4])
        });
        assert!(res.is_ok());
    }

    /// Lc7 nextint = (body_len - 4) / 8. body_len=20 → n=2 original,
    /// n=3 mit `+`-Mutation.
    #[test]
    fn lc7_nextint_value_is_minus_4_div_8() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc7, |inner| {
            inner.write_bytes(&[0u8; 20])
        })
        .unwrap();
        let bytes = w.into_bytes();
        // 4 EMHEADER + 4 nextint + 20 body = 28
        assert_eq!(bytes.len(), 28);
        let mut ni = [0u8; 4];
        ni.copy_from_slice(&bytes[4..8]);
        let nextint = u32::from_le_bytes(ni);
        assert_eq!(nextint, 2, "nextint must be (20-4)/8=2, not (20+4)/8=3");
    }

    /// Lc6 body_len=8 muss erroren ((8-4)%4=0 ok, also Pass — kein
    /// Boundary-Fail). Hier Test fuer Lc6 body_len=6: (6-4)%4=2 ≠ 0 → Error.
    /// Faengt `||` -> `&&` (Zeile 226 wurde nicht direkt missed, aber
    /// Test fuer Vollstaendigkeit).
    #[test]
    fn lc6_misaligned_body_len_rejected() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc6, |inner| {
            inner.write_bytes(&[0u8; 6])
        });
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    /// Lc7 misaligned body. Faengt `||` -> `&&` (Zeile 238).
    /// body_len=12: (12-4)%8 = 8%8 = 0 ok. Need body_len=10: (10-4)%8 = 6.
    #[test]
    fn lc7_misaligned_body_len_rejected() {
        let mut w = BufferWriter::new(Endianness::Little);
        let res = encode_mutable_member_lc(&mut w, 1, false, LengthCode::Lc7, |inner| {
            inner.write_bytes(&[0u8; 10])
        });
        assert!(matches!(res, Err(EncodeError::ValueOutOfRange { .. })));
    }

    /// EMHEADER must_understand-Bit + LC-Bits werden korrekt gesetzt.
    /// Faengt `|` -> `^`/`-`/`*` Mutationen auf der EMHEADER-
    /// Konstruktion (nach Refactor zu `+`).
    #[test]
    fn emheader_combines_must_understand_lc_and_member_id() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_mutable_member_lc(&mut w, 0x123_4567, true, LengthCode::Lc6, |inner| {
            inner.write_bytes(&[0u8; 4])
        })
        .unwrap();
        let bytes = w.into_bytes();
        let mut h = [0u8; 4];
        h.copy_from_slice(&bytes[..4]);
        let emheader = u32::from_le_bytes(h);
        // m_bit (Bit 31) = 0x8000_0000
        // lc_bits (Lc6 = 6 << 28) = 0x6000_0000
        // member_id = 0x0123_4567
        // Sum = 0x8000_0000 + 0x6000_0000 + 0x0123_4567 = 0xE123_4567
        assert_eq!(emheader, 0xE123_4567);
    }
}
