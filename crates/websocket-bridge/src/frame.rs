// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket Frame Modell — RFC 6455 §5.2.

use alloc::vec::Vec;

/// `Opcode` (RFC 6455 §5.2, S. 28-29) — 4-bit, definiert Interpretation
/// der Payload.
///
/// Spec-Werte:
/// * 0x0 — Continuation
/// * 0x1 — Text Frame (UTF-8)
/// * 0x2 — Binary Frame
/// * 0x3-0x7 — Reserved Non-Control
/// * 0x8 — Connection Close
/// * 0x9 — Ping
/// * 0xA — Pong
/// * 0xB-0xF — Reserved Control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Opcode {
    /// `0x0` — Continuation frame.
    Continuation,
    /// `0x1` — Text frame (UTF-8).
    Text,
    /// `0x2` — Binary frame.
    Binary,
    /// `0x8` — Connection-Close frame.
    Close,
    /// `0x9` — Ping frame.
    Ping,
    /// `0xA` — Pong frame.
    Pong,
    /// `0x3-0x7` reserved non-control / `0xB-0xF` reserved control.
    Reserved(u8),
}

impl Opcode {
    /// Konvertiert vom Wire-Wert (4-bit).
    #[must_use]
    pub const fn from_bits(v: u8) -> Self {
        match v & 0x0F {
            0x0 => Self::Continuation,
            0x1 => Self::Text,
            0x2 => Self::Binary,
            0x8 => Self::Close,
            0x9 => Self::Ping,
            0xA => Self::Pong,
            other => Self::Reserved(other),
        }
    }

    /// Wire-Wert (4-bit).
    #[must_use]
    pub const fn to_bits(self) -> u8 {
        match self {
            Self::Continuation => 0x0,
            Self::Text => 0x1,
            Self::Binary => 0x2,
            Self::Close => 0x8,
            Self::Ping => 0x9,
            Self::Pong => 0xA,
            Self::Reserved(v) => v & 0x0F,
        }
    }

    /// Spec §5.5 — Control Frames sind opcodes 0x8-0xF. Sie haben
    /// folgende Eigenschaften: payload <= 125 bytes (Spec §5.5),
    /// duerfen nicht fragmentiert werden (FIN=1).
    #[must_use]
    pub const fn is_control(self) -> bool {
        match self {
            Self::Close | Self::Ping | Self::Pong => true,
            Self::Reserved(v) => (v & 0x08) != 0,
            _ => false,
        }
    }
}

/// WebSocket-Frame — RFC 6455 §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Spec §5.2 — `FIN` bit. Final fragment indicator.
    pub fin: bool,
    /// Spec §5.2 — `RSV1` bit. MUST be 0 unless extension negotiated.
    pub rsv1: bool,
    /// Spec §5.2 — `RSV2` bit.
    pub rsv2: bool,
    /// Spec §5.2 — `RSV3` bit.
    pub rsv3: bool,
    /// Spec §5.2 — Opcode.
    pub opcode: Opcode,
    /// Spec §5.2 — Masking-Key (`Some` wenn MASK=1; immer `Some` von
    /// client→server).
    pub masking_key: Option<[u8; 4]>,
    /// Spec §5.2 — Payload (already unmasked beim Decode; wird beim
    /// Encode automatisch maskiert wenn `masking_key` gesetzt).
    pub payload: Vec<u8>,
}

impl Frame {
    /// Konstruiert einen unmaskierten Text-Frame mit FIN=1.
    #[must_use]
    pub fn text(s: impl Into<alloc::string::String>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            masking_key: None,
            payload: s.into().into_bytes(),
        }
    }

    /// Konstruiert einen unmaskierten Binary-Frame mit FIN=1.
    #[must_use]
    pub const fn binary(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Binary,
            masking_key: None,
            payload,
        }
    }

    /// Konstruiert einen Ping-Frame (FIN=1, max. 125 Bytes Payload —
    /// Spec §5.5).
    #[must_use]
    pub const fn ping(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Ping,
            masking_key: None,
            payload,
        }
    }

    /// Konstruiert einen Pong-Frame (Spec §5.5.3 — als Reply-zu-Ping
    /// MUST denselben Payload haben).
    #[must_use]
    pub const fn pong(payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Pong,
            masking_key: None,
            payload,
        }
    }

    /// Konstruiert einen Close-Frame mit Status-Code + optionaler
    /// Reason. Spec §5.5.1 + §7.4.
    #[must_use]
    pub fn close(status: u16, reason: &str) -> Self {
        let mut payload = Vec::with_capacity(2 + reason.len());
        payload.extend_from_slice(&status.to_be_bytes());
        payload.extend_from_slice(reason.as_bytes());
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Close,
            masking_key: None,
            payload,
        }
    }

    /// Aktiviert Client→Server-Masking. Spec §5.3 — "All frames sent
    /// from client to server have this bit set to 1".
    #[must_use]
    pub const fn with_mask(mut self, key: [u8; 4]) -> Self {
        self.masking_key = Some(key);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_round_trip_via_bits() {
        for v in 0..16u8 {
            let op = Opcode::from_bits(v);
            assert_eq!(op.to_bits(), v);
        }
    }

    #[test]
    fn opcode_well_known_values_match_spec() {
        // RFC 6455 §5.2.
        assert_eq!(Opcode::Continuation.to_bits(), 0x0);
        assert_eq!(Opcode::Text.to_bits(), 0x1);
        assert_eq!(Opcode::Binary.to_bits(), 0x2);
        assert_eq!(Opcode::Close.to_bits(), 0x8);
        assert_eq!(Opcode::Ping.to_bits(), 0x9);
        assert_eq!(Opcode::Pong.to_bits(), 0xA);
    }

    #[test]
    fn opcode_is_control_predicate() {
        // Spec §5.5 — control opcodes 0x8-0xF.
        assert!(Opcode::Close.is_control());
        assert!(Opcode::Ping.is_control());
        assert!(Opcode::Pong.is_control());
        assert!(!Opcode::Text.is_control());
        assert!(!Opcode::Binary.is_control());
        assert!(!Opcode::Continuation.is_control());
        // Reserved control range 0xB-0xF.
        assert!(Opcode::Reserved(0xB).is_control());
        // Reserved non-control 0x3-0x7.
        assert!(!Opcode::Reserved(0x3).is_control());
    }

    #[test]
    fn text_frame_constructor_sets_fin_and_opcode() {
        let f = Frame::text("hello");
        assert!(f.fin);
        assert_eq!(f.opcode, Opcode::Text);
        assert!(f.masking_key.is_none());
        assert_eq!(f.payload, alloc::vec![b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn close_frame_includes_status_code_in_be_payload() {
        // Spec §7.4 — Status-Code als 16-bit BE.
        let f = Frame::close(1000, "");
        assert_eq!(&f.payload[..2], &1000u16.to_be_bytes());
    }

    #[test]
    fn close_frame_with_reason_carries_utf8_bytes() {
        let f = Frame::close(1001, "Going Away");
        assert_eq!(&f.payload[..2], &1001u16.to_be_bytes());
        assert_eq!(&f.payload[2..], b"Going Away");
    }

    #[test]
    fn with_mask_sets_masking_key() {
        // Spec §5.3 — Client masks all frames.
        let f = Frame::text("hi").with_mask([1, 2, 3, 4]);
        assert_eq!(f.masking_key, Some([1, 2, 3, 4]));
    }
}
