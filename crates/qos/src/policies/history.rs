// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! HistoryQosPolicy (DDS 1.4 §2.2.3.17).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): u32 kind + i32 depth = 8 byte.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// History-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum HistoryKind {
    /// Keep last N (§2.2.3.17.1). Default.
    #[default]
    KeepLast = 0,
    /// Keep all.
    KeepAll = 1,
}

impl HistoryKind {
    /// Strikter Mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::KeepLast),
            1 => Some(Self::KeepAll),
            _ => None,
        }
    }

    /// Forward-kompatibler Mapper (unbekannt → KeepLast).
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::KeepAll,
            _ => Self::KeepLast,
        }
    }
}

/// HistoryQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryQosPolicy {
    /// Kind.
    pub kind: HistoryKind,
    /// Depth (nur fuer `KeepLast`). Default 1 (§2.2.3.17.3).
    pub depth: i32,
}

impl Default for HistoryQosPolicy {
    fn default() -> Self {
        Self {
            kind: HistoryKind::KeepLast,
            depth: 1,
        }
    }
}

impl HistoryQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)?;
        w.write_u32(self.depth as u32)
    }

    /// Wire-Decoding (strict).
    ///
    /// # Errors
    /// Buffer-Underflow oder unbekannter Kind-Wert.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = HistoryKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "HistoryKind",
            value: v,
        })?;
        let depth = r.read_u32()? as i32;
        Ok(Self { kind, depth })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_keep_last_1() {
        let d = HistoryQosPolicy::default();
        assert_eq!(d.kind, HistoryKind::KeepLast);
        assert_eq!(d.depth, 1);
    }

    #[test]
    fn try_from_u32_strict() {
        assert_eq!(HistoryKind::try_from_u32(0), Some(HistoryKind::KeepLast));
        assert_eq!(HistoryKind::try_from_u32(1), Some(HistoryKind::KeepAll));
        assert_eq!(HistoryKind::try_from_u32(2), None);
    }

    fn roundtrip(p: HistoryQosPolicy) {
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(HistoryQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn encode_decode_roundtrip() {
        roundtrip(HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 42,
        });
    }

    #[test]
    fn encode_decode_keep_all() {
        roundtrip(HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: -1,
        });
    }

    /// Spec §9.6.3.2: `from_u32` ist forward-kompatibel — unbekannte
    /// Werte mappen auf Default-Variante `KeepLast` statt zu erroren.
    #[test]
    fn from_u32_forward_compatible() {
        assert_eq!(HistoryKind::from_u32(0), HistoryKind::KeepLast);
        assert_eq!(HistoryKind::from_u32(1), HistoryKind::KeepAll);
        // Unknown discriminators collapse to KeepLast.
        assert_eq!(HistoryKind::from_u32(2), HistoryKind::KeepLast);
        assert_eq!(HistoryKind::from_u32(0xDEAD_BEEF), HistoryKind::KeepLast);
    }

    /// Decode mit unbekanntem `kind`-Discriminator → `InvalidEnum`-Fehler.
    /// Verhindert Regressions gegen "forward-compat decoder" (würde hier
    /// absichtlich `try_from_u32` verwenden, strikt).
    #[test]
    fn decode_unknown_kind_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(99).unwrap(); // unknown kind
        w.write_u32(10).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = HistoryQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(
            matches!(
                err,
                zerodds_cdr::DecodeError::InvalidEnum {
                    kind: "HistoryKind",
                    value: 99
                }
            ),
            "expected InvalidEnum{{HistoryKind,99}}, got {err:?}"
        );
    }

    /// Decode mit kurzem Buffer (nur kind, kein depth) → short-read error
    /// fuer den `read_u32` am depth-Offset.
    #[test]
    fn decode_short_buffer_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(0).unwrap(); // kind=KeepLast, but no depth follows
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(HistoryQosPolicy::decode_from(&mut r).is_err());
    }

    /// Default ist `KeepLast/1`; Debug-Format enthaelt die Variante.
    #[test]
    fn debug_format_contains_kind() {
        let d = HistoryQosPolicy::default();
        let dbg = alloc::format!("{d:?}");
        assert!(dbg.contains("KeepLast"), "debug: {dbg}");
    }

    /// Copy-Semantik (`HistoryKind` ist `Copy`) und PartialEq/Clone-Pfade.
    #[test]
    fn kind_copy_clone_eq() {
        let k = HistoryKind::KeepAll;
        let k2 = k;
        #[allow(clippy::clone_on_copy)]
        let k3 = k.clone();
        assert_eq!(k, k2);
        assert_eq!(k, k3);
    }
}
