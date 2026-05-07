// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder- und Decoder-Fehler.
//!
//! Bewusst getrennt: ein Wert kann nur enkodiert oder dekodiert werden,
//! aber die Fehler-Kategorien sind unterschiedlich. Encoder-Fehler sind
//! Buffer-/Format-bezogen; Decoder-Fehler sind zusaetzlich Validierungs-
//! Fehler (UnexpectedEof, InvalidUtf8, etc.).

use core::fmt;

/// Fehler beim Encoden eines Werts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// Schreib-Buffer zu klein. Bei `Vec<u8>`-basiertem Writer kann das
    /// nicht passieren; bei fixierten Buffern (no_std + statisches
    /// Array) sehr wohl.
    BufferTooSmall {
        /// Wieviele zusaetzliche Bytes gebraucht worden waeren.
        needed: usize,
        /// Wieviele tatsaechlich verfuegbar waren.
        available: usize,
    },
    /// Wert kann nicht encodiert werden — z.B. `String`-Laenge sprengt
    /// `u32::MAX` (XCDR-Limit).
    ValueOutOfRange {
        /// Beschreibung der Verletzung.
        message: &'static str,
    },
    /// Mutable-Encode hat einen non-optional Member ausgelassen
    /// (XTypes 1.3 §7.4.1.2.3 — "the serialized representation MUST
    /// contain at least the values of all the non-optional members").
    MissingNonOptionalMember {
        /// Member-ID, die fehlte.
        member_id: u32,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { needed, available } => write!(
                f,
                "encoder buffer too small: needed {needed} bytes, available {available}"
            ),
            Self::ValueOutOfRange { message } => write!(f, "value out of range: {message}"),
            Self::MissingNonOptionalMember { member_id } => write!(
                f,
                "mutable struct missing non-optional member: id={member_id}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

/// Fehler beim Decoden eines Werts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Eingabe endete vor dem erwarteten Ende.
    UnexpectedEof {
        /// Wieviele Bytes noch erwartet wurden.
        needed: usize,
        /// Position im Stream, wo das EOF auftrat.
        offset: usize,
    },
    /// UTF-8-Validierung fuer einen String-Wert ist fehlgeschlagen.
    InvalidUtf8 {
        /// Position, an der der String begann.
        offset: usize,
    },
    /// Boolean-Byte war weder 0 noch 1 — XCDR-Spec verbietet das.
    InvalidBool {
        /// Tatsaechlich gelesenes Byte.
        value: u8,
        /// Position des Bytes.
        offset: usize,
    },
    /// Char-Wert ist kein gueltiger Unicode-Codepoint.
    InvalidChar {
        /// Tatsaechlich gelesener u32-Wert.
        value: u32,
        /// Position.
        offset: usize,
    },
    /// Sequence-/Array-Laenge ueberschreitet Bound oder die noch
    /// vorhandenen Bytes.
    LengthExceeded {
        /// Wieviele Elemente die Laenge ankuendigt.
        announced: usize,
        /// Wieviele tatsaechlich noch lesbar sind (best-effort).
        remaining: usize,
        /// Position.
        offset: usize,
    },
    /// String-Format-Verletzung (z.B. Null-Terminator fehlt, Laenge 0).
    InvalidString {
        /// Position, an der der String begann.
        offset: usize,
        /// Kurzbeschreibung (statisch).
        reason: &'static str,
    },
    /// Unbekannter/ungueltiger Enum-Discriminator — wird von Policy-
    /// Decodern genutzt, die strict-mode operieren (z.B. QoS-Enums).
    InvalidEnum {
        /// Enum-Name fuer Debugging (z.B. "DurabilityKind").
        kind: &'static str,
        /// Gelesener Discriminator-Wert.
        value: u32,
    },
    /// Mutable-Decode hat einen `must_understand`-Member mit unbekannter
    /// Member-ID gelesen (XTypes 1.3 §7.4.1.2.3 — Receiver MUSS in dem
    /// Fall die Message verwerfen).
    UnknownMustUnderstandMember {
        /// Member-ID die nicht erkannt wurde.
        member_id: u32,
    },
    /// Mutable-Decode hat einen non-optional Member nicht im Wire
    /// gefunden (XTypes 1.3 §7.4.1.2.3).
    MissingNonOptionalMember {
        /// Member-ID die fehlt.
        member_id: u32,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, offset } => {
                write!(
                    f,
                    "unexpected EOF: needed {needed} more bytes at offset {offset}"
                )
            }
            Self::InvalidUtf8 { offset } => write!(f, "invalid UTF-8 at offset {offset}"),
            Self::InvalidBool { value, offset } => {
                write!(f, "invalid bool byte 0x{value:02x} at offset {offset}")
            }
            Self::InvalidChar { value, offset } => {
                write!(f, "invalid char codepoint U+{value:04X} at offset {offset}")
            }
            Self::LengthExceeded {
                announced,
                remaining,
                offset,
            } => write!(
                f,
                "length {announced} at offset {offset} exceeds remaining {remaining} bytes"
            ),
            Self::InvalidString { offset, reason } => {
                write!(f, "invalid CDR string at offset {offset}: {reason}")
            }
            Self::InvalidEnum { kind, value } => {
                write!(f, "invalid {kind} discriminator: {value}")
            }
            Self::UnknownMustUnderstandMember { member_id } => {
                write!(f, "unknown must_understand member id: {member_id}")
            }
            Self::MissingNonOptionalMember { member_id } => {
                write!(f, "missing non-optional member id: {member_id}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[test]
    fn encode_error_display_buffer_too_small() {
        let e = EncodeError::BufferTooSmall {
            needed: 8,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("8"));
        assert!(s.contains("4"));
    }

    #[test]
    fn encode_error_display_value_out_of_range() {
        let e = EncodeError::ValueOutOfRange {
            message: "string too long",
        };
        assert!(format!("{e}").contains("string too long"));
    }

    #[test]
    fn decode_error_display_unexpected_eof() {
        let e = DecodeError::UnexpectedEof {
            needed: 4,
            offset: 12,
        };
        let s = format!("{e}");
        assert!(s.contains("4"));
        assert!(s.contains("12"));
    }

    #[test]
    fn decode_error_display_invalid_utf8() {
        let e = DecodeError::InvalidUtf8 { offset: 5 };
        assert!(format!("{e}").contains("5"));
    }

    #[test]
    fn decode_error_display_invalid_bool() {
        let e = DecodeError::InvalidBool {
            value: 0xff,
            offset: 0,
        };
        assert!(format!("{e}").contains("ff"));
    }

    #[test]
    fn decode_error_display_invalid_char() {
        let e = DecodeError::InvalidChar {
            value: 0xD800,
            offset: 0,
        };
        assert!(format!("{e}").contains("D800"));
    }

    #[test]
    fn decode_error_display_length_exceeded() {
        let e = DecodeError::LengthExceeded {
            announced: 100,
            remaining: 4,
            offset: 0,
        };
        let s = format!("{e}");
        assert!(s.contains("100"));
        assert!(s.contains("4"));
    }

    #[test]
    fn errors_are_clone_eq() {
        let e1 = EncodeError::ValueOutOfRange { message: "x" };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        let d1 = DecodeError::UnexpectedEof {
            needed: 1,
            offset: 0,
        };
        assert_eq!(d1.clone(), d1);
    }
}
