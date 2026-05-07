// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP-Message Modell — RFC 7252 §3 + §12.1.

use alloc::vec::Vec;
use core::fmt;

use crate::option::CoapOption;

/// `Type` field (RFC 7252 §3, S. 16).
///
/// 2-bit unsigned integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// `0` — Confirmable.
    Confirmable,
    /// `1` — Non-Confirmable.
    NonConfirmable,
    /// `2` — Acknowledgement.
    Acknowledgement,
    /// `3` — Reset.
    Reset,
}

impl MessageType {
    /// Konvertiert vom 2-Bit-Wire-Wert.
    ///
    /// # Errors
    /// Liefert `None` wenn `v > 3`.
    #[must_use]
    pub const fn from_bits(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Confirmable),
            1 => Some(Self::NonConfirmable),
            2 => Some(Self::Acknowledgement),
            3 => Some(Self::Reset),
            _ => None,
        }
    }

    /// Wire-Wert (2 Bit).
    #[must_use]
    pub const fn to_bits(self) -> u8 {
        match self {
            Self::Confirmable => 0,
            Self::NonConfirmable => 1,
            Self::Acknowledgement => 2,
            Self::Reset => 3,
        }
    }
}

/// CoAP-Code (RFC 7252 §3 + §12.1) — 8-bit, 3-bit class + 5-bit detail.
///
/// Format `c.dd` mit `c` ∈ 0..=7 und `dd` ∈ 0..=31.
///
/// Code-Class:
/// * 0 — Request (0.00 = empty, 0.01-0.04 = method codes).
/// * 2 — Success Response.
/// * 4 — Client Error.
/// * 5 — Server Error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoapCode {
    /// 3-bit class.
    pub class: u8,
    /// 5-bit detail.
    pub detail: u8,
}

impl CoapCode {
    /// Konstruktor; class+detail werden auf gueltige Bit-Bereiche
    /// (3-bit, 5-bit) gemaskt.
    #[must_use]
    pub const fn new(class: u8, detail: u8) -> Self {
        Self {
            class: class & 0b111,
            detail: detail & 0b1_1111,
        }
    }

    /// Decodiert vom Wire-Byte (RFC 7252 §3 Code-Field).
    #[must_use]
    pub const fn from_byte(b: u8) -> Self {
        Self {
            class: (b >> 5) & 0b111,
            detail: b & 0b1_1111,
        }
    }

    /// Encodiert ins Wire-Byte.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        (self.class << 5) | (self.detail & 0b1_1111)
    }

    /// `EMPTY` (0.00, RFC 7252 §3) — Spec-Sonderfall fuer Reset/ACK.
    pub const EMPTY: Self = Self::new(0, 0);
    /// `GET` (0.01).
    pub const GET: Self = Self::new(0, 1);
    /// `POST` (0.02).
    pub const POST: Self = Self::new(0, 2);
    /// `PUT` (0.03).
    pub const PUT: Self = Self::new(0, 3);
    /// `DELETE` (0.04).
    pub const DELETE: Self = Self::new(0, 4);
    /// `2.01 Created`.
    pub const CREATED: Self = Self::new(2, 1);
    /// `2.02 Deleted`.
    pub const DELETED: Self = Self::new(2, 2);
    /// `2.03 Valid`.
    pub const VALID: Self = Self::new(2, 3);
    /// `2.04 Changed`.
    pub const CHANGED: Self = Self::new(2, 4);
    /// `2.05 Content`.
    pub const CONTENT: Self = Self::new(2, 5);
    /// `4.00 Bad Request`.
    pub const BAD_REQUEST: Self = Self::new(4, 0);
    /// `4.04 Not Found`.
    pub const NOT_FOUND: Self = Self::new(4, 4);
    /// `5.00 Internal Server Error`.
    pub const INTERNAL_SERVER_ERROR: Self = Self::new(5, 0);

    /// `true` wenn class == 0 und detail > 0 — Request-Method (Spec
    /// §3 + §12.1.1).
    #[must_use]
    pub const fn is_request(self) -> bool {
        self.class == 0 && self.detail > 0
    }

    /// `true` wenn class == 2 — Success-Response (Spec §12.1.2).
    #[must_use]
    pub const fn is_success(self) -> bool {
        self.class == 2
    }

    /// `true` wenn class == 4 — Client-Error (Spec §12.1.2).
    #[must_use]
    pub const fn is_client_error(self) -> bool {
        self.class == 4
    }

    /// `true` wenn class == 5 — Server-Error (Spec §12.1.2).
    #[must_use]
    pub const fn is_server_error(self) -> bool {
        self.class == 5
    }

    /// `true` wenn Empty-Code (0.00).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.class == 0 && self.detail == 0
    }
}

impl fmt::Display for CoapCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.class, self.detail)
    }
}

/// CoAP-Message — RFC 7252 §3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoapMessage {
    /// Spec §3 — `Ver`. Wir setzen immer 1 beim Encode.
    pub version: u8,
    /// Spec §3 — `Type`.
    pub message_type: MessageType,
    /// Spec §3 — `Code`.
    pub code: CoapCode,
    /// Spec §3 — `Message ID` (16 bit).
    pub message_id: u16,
    /// Spec §3 — `Token`. Length 0..=8 (TKL field).
    pub token: Vec<u8>,
    /// Spec §3.1 — Optionen, sortiert nach Number (Spec verlangt
    /// Delta-Encoding-Reihenfolge).
    pub options: Vec<CoapOption>,
    /// Spec §3 — Payload (nach 0xFF-Marker).
    pub payload: Vec<u8>,
}

impl CoapMessage {
    /// Konstruiert eine neue Message mit Default `version=1` und
    /// leeren Options/Payload.
    #[must_use]
    pub const fn new(message_type: MessageType, code: CoapCode, message_id: u16) -> Self {
        Self {
            version: 1,
            message_type,
            code,
            message_id,
            token: Vec::new(),
            options: Vec::new(),
            payload: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_round_trips_via_bits() {
        // RFC 7252 §3.
        for t in [
            MessageType::Confirmable,
            MessageType::NonConfirmable,
            MessageType::Acknowledgement,
            MessageType::Reset,
        ] {
            assert_eq!(MessageType::from_bits(t.to_bits()), Some(t));
        }
    }

    #[test]
    fn message_type_rejects_out_of_range() {
        for v in 4..=7 {
            assert_eq!(MessageType::from_bits(v), None);
        }
    }

    #[test]
    fn code_byte_round_trip() {
        // RFC 7252 §3 + §12.1.
        for class in 0..8 {
            for detail in 0..32 {
                let c = CoapCode::new(class, detail);
                assert_eq!(CoapCode::from_byte(c.to_byte()), c);
            }
        }
    }

    #[test]
    fn well_known_codes_match_spec_values() {
        // RFC 7252 §12.1.
        assert_eq!(CoapCode::GET.to_byte(), 0b000_00001);
        assert_eq!(CoapCode::POST.to_byte(), 0b000_00010);
        assert_eq!(CoapCode::PUT.to_byte(), 0b000_00011);
        assert_eq!(CoapCode::DELETE.to_byte(), 0b000_00100);
        // 2.05 Content = class 2 detail 5 = 0100_0101 = 0x45.
        assert_eq!(CoapCode::CONTENT.to_byte(), 0x45);
        // 4.04 Not Found.
        assert_eq!(CoapCode::NOT_FOUND.to_byte(), 0x84);
        // 5.00 Internal Server Error.
        assert_eq!(CoapCode::INTERNAL_SERVER_ERROR.to_byte(), 0xA0);
    }

    #[test]
    fn code_display_uses_dotted_format() {
        // Spec §3 — "documented as c.dd".
        assert_eq!(alloc::format!("{}", CoapCode::GET), "0.01");
        assert_eq!(alloc::format!("{}", CoapCode::CONTENT), "2.05");
        assert_eq!(alloc::format!("{}", CoapCode::NOT_FOUND), "4.04");
        assert_eq!(
            alloc::format!("{}", CoapCode::INTERNAL_SERVER_ERROR),
            "5.00"
        );
    }

    #[test]
    fn code_classification_predicates() {
        assert!(CoapCode::GET.is_request());
        assert!(!CoapCode::GET.is_success());
        assert!(CoapCode::CONTENT.is_success());
        assert!(CoapCode::NOT_FOUND.is_client_error());
        assert!(CoapCode::INTERNAL_SERVER_ERROR.is_server_error());
        assert!(CoapCode::EMPTY.is_empty());
        assert!(!CoapCode::GET.is_empty());
    }

    #[test]
    fn new_message_defaults_version_to_1() {
        // Spec §3 — Implementations MUST set version to 1.
        let m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 42);
        assert_eq!(m.version, 1);
        assert_eq!(m.message_id, 42);
        assert!(m.token.is_empty());
        assert!(m.options.is_empty());
        assert!(m.payload.is_empty());
    }
}
