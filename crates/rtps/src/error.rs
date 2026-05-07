// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS-Wire-Format-Fehler.

use core::fmt;

/// Fehler beim Encodieren oder Decodieren von RTPS-Wire-Daten.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    /// Buffer ist zu klein fuer den Schreib-Vorgang.
    WriteOverflow {
        /// Wieviele Bytes gebraucht worden waeren.
        needed: usize,
        /// Wieviele tatsaechlich verfuegbar waren.
        available: usize,
    },
    /// Eingabe endete vor dem erwarteten Ende.
    UnexpectedEof {
        /// Wieviele Bytes noch erwartet wurden.
        needed: usize,
        /// Position im Stream.
        offset: usize,
    },
    /// Magic-Bytes des RTPS-Headers stimmen nicht ("RTPS").
    InvalidMagic {
        /// Tatsaechlich gelesene 4 Bytes.
        found: [u8; 4],
    },
    /// Submessage-ID ist nicht im supported-Set.
    UnknownSubmessageId {
        /// Roher Submessage-ID-Byte.
        id: u8,
    },
    /// Locator-Kind hat unerwarteten Wert.
    InvalidLocatorKind {
        /// Roher i32-Wert.
        kind: i32,
    },
    /// Numerischer Wert ueberschreitet das Wire-Feld.
    ValueOutOfRange {
        /// Beschreibung.
        message: &'static str,
    },
    /// Encapsulation-Kind im CDR-Header wird nicht unterstuetzt
    /// (z.B. PL_CDR2, CDR_LE bei erwartetem PL_CDR).
    UnsupportedEncapsulation {
        /// Die beiden Encapsulation-Bytes `[kind_hi, kind_lo]`.
        kind: [u8; 2],
    },
    /// Spec-konformes Wire-Feature, das in dieser Phase (noch) nicht
    /// unterstuetzt wird (z.B. Inline-QoS in DATA_FRAG).
    UnsupportedFeature {
        /// Name/Kurzbeschreibung des Features.
        what: &'static str,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteOverflow { needed, available } => {
                write!(
                    f,
                    "RTPS write overflow: needed {needed}, available {available}"
                )
            }
            Self::UnexpectedEof { needed, offset } => {
                write!(f, "RTPS unexpected EOF: needed {needed} at offset {offset}")
            }
            Self::InvalidMagic { found } => {
                write!(f, "RTPS invalid magic: expected b\"RTPS\", got {found:?}")
            }
            Self::UnknownSubmessageId { id } => {
                write!(f, "RTPS unknown submessage id 0x{id:02x}")
            }
            Self::InvalidLocatorKind { kind } => {
                write!(f, "RTPS invalid locator kind {kind}")
            }
            Self::ValueOutOfRange { message } => write!(f, "RTPS value out of range: {message}"),
            Self::UnsupportedEncapsulation { kind } => {
                write!(
                    f,
                    "RTPS unsupported encapsulation kind 0x{:02x}{:02x}",
                    kind[0], kind[1]
                )
            }
            Self::UnsupportedFeature { what } => {
                write!(f, "RTPS unsupported feature: {what}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WireError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[cfg(feature = "alloc")]
    #[test]
    fn write_overflow_display() {
        let e = WireError::WriteOverflow {
            needed: 16,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("16"));
        assert!(s.contains("4"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn invalid_magic_display() {
        let e = WireError::InvalidMagic {
            found: [b'X', b'X', b'X', b'X'],
        };
        let s = format!("{e}");
        assert!(s.contains("RTPS"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unknown_submessage_id_display_uses_hex() {
        let e = WireError::UnknownSubmessageId { id: 0x42 };
        let s = format!("{e}");
        assert!(s.contains("42"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn invalid_locator_kind_display() {
        let e = WireError::InvalidLocatorKind { kind: 99 };
        assert!(format!("{e}").contains("99"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn value_out_of_range_display() {
        let e = WireError::ValueOutOfRange { message: "too big" };
        assert!(format!("{e}").contains("too big"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unexpected_eof_display() {
        let e = WireError::UnexpectedEof {
            needed: 8,
            offset: 12,
        };
        let s = format!("{e}");
        assert!(s.contains("8"));
        assert!(s.contains("12"));
    }
}
