// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE-Wire-Format-Fehler.
//!
//! Analog zu `zerodds_rtps::WireError`, aber mit XRCE-spezifischen
//! Varianten (z.B. fuer fehlende ClientKey-Praesenz).

use core::fmt;

/// Fehler beim Encodieren oder Decodieren von XRCE-Wire-Daten.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XrceError {
    /// Ausgabe-Buffer zu klein.
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
        /// Position im Stream zum Zeitpunkt des Fehlers.
        offset: usize,
    },
    /// Submessage-ID ist nicht in 0..=15 (Spec §8.3.5).
    UnknownSubmessageId {
        /// Roher Submessage-ID-Byte.
        id: u8,
    },
    /// SubmessageHeader-Length deutet auf Body, der ueber das Datagram-
    /// Ende hinaus reichen wuerde (Truncation).
    TruncatedSubmessageBody {
        /// Header-Wert `submessage_length`.
        declared: u16,
        /// Tatsaechlich verfuegbare Bytes ab Body-Start.
        available: usize,
    },
    /// Anzahl Submessages in einer Message ueberschreitet
    /// `DOSC_MAX_SUBMESSAGES` — DoS-Schutz.
    TooManySubmessages {
        /// Festes Limit.
        limit: usize,
    },
    /// Payload-Groesse ueberschreitet `DOSC_MAX_PAYLOAD_SIZE` —
    /// DoS-Schutz.
    PayloadTooLarge {
        /// Limit.
        limit: usize,
        /// Tatsaechliche Groesse.
        actual: usize,
    },
    /// SubmessageLength ist nicht zum 4-Byte-Alignment passend
    /// (§8.3.3: Offset jeder Submessage muss multiple von 4 sein).
    UnalignedSubmessage {
        /// Offset, an dem das Problem auftrat.
        offset: usize,
    },
    /// Numerischer Wert ueberschreitet das Wire-Feld.
    ValueOutOfRange {
        /// Beschreibung.
        message: &'static str,
    },
}

impl fmt::Display for XrceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteOverflow { needed, available } => write!(
                f,
                "xrce write overflow: needed {needed}, available {available}"
            ),
            Self::UnexpectedEof { needed, offset } => {
                write!(f, "xrce unexpected EOF: needed {needed} at offset {offset}")
            }
            Self::UnknownSubmessageId { id } => {
                write!(f, "xrce unknown submessage id 0x{id:02x}")
            }
            Self::TruncatedSubmessageBody {
                declared,
                available,
            } => write!(
                f,
                "xrce submessage body truncated: declared {declared}, available {available}"
            ),
            Self::TooManySubmessages { limit } => {
                write!(f, "xrce too many submessages: limit {limit}")
            }
            Self::PayloadTooLarge { limit, actual } => {
                write!(f, "xrce payload too large: limit {limit}, actual {actual}")
            }
            Self::UnalignedSubmessage { offset } => {
                write!(f, "xrce submessage not 4-byte aligned at offset {offset}")
            }
            Self::ValueOutOfRange { message } => {
                write!(f, "xrce value out of range: {message}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for XrceError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[cfg(feature = "alloc")]
    #[test]
    fn write_overflow_display_includes_numbers() {
        let e = XrceError::WriteOverflow {
            needed: 16,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("16") && s.contains("4"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unknown_submessage_id_display_uses_hex() {
        let e = XrceError::UnknownSubmessageId { id: 0xAB };
        let s = format!("{e}");
        assert!(s.contains("ab"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn truncated_body_display_includes_values() {
        let e = XrceError::TruncatedSubmessageBody {
            declared: 100,
            available: 50,
        };
        let s = format!("{e}");
        assert!(s.contains("100") && s.contains("50"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn too_many_submessages_display_mentions_limit() {
        let e = XrceError::TooManySubmessages { limit: 64 };
        assert!(format!("{e}").contains("64"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn payload_too_large_display_includes_both() {
        let e = XrceError::PayloadTooLarge {
            limit: 1024,
            actual: 2048,
        };
        let s = format!("{e}");
        assert!(s.contains("1024") && s.contains("2048"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unaligned_display_mentions_offset() {
        let e = XrceError::UnalignedSubmessage { offset: 17 };
        assert!(format!("{e}").contains("17"));
    }
}
