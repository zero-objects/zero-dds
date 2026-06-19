// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Flow control — RFC 9113 §5.2 + §6.9.
//!
//! Spec §5.2: there is a flow window per stream AND per connection.
//! A sender may not send more than `min(stream_window, conn_window)`
//! of `DATA` bytes before receiving a `WINDOW_UPDATE`.

use crate::error::Http2Error;

/// Initial window size per Spec §6.5.2 (`SETTINGS_INITIAL_WINDOW_SIZE`).
pub const INITIAL_WINDOW_SIZE: i64 = 65_535;

/// Flow-control window with signed i64 (Spec §6.9: can transiently
/// become negative when settings shrink the window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowControl {
    window: i64,
    max: i64,
}

impl Default for FlowControl {
    fn default() -> Self {
        Self {
            window: INITIAL_WINDOW_SIZE,
            max: 0x7fff_ffff,
        }
    }
}

impl FlowControl {
    /// Constructor with initial window size.
    #[must_use]
    pub fn new(initial: i64) -> Self {
        Self {
            window: initial,
            max: 0x7fff_ffff,
        }
    }

    /// Current window.
    #[must_use]
    pub fn window(&self) -> i64 {
        self.window
    }

    /// Consumes `n` bytes from the window. Spec §6.9.1.
    ///
    /// # Errors
    /// `FlowControlExceeded` if the window would become negative.
    pub fn consume(&mut self, n: u32) -> Result<(), Http2Error> {
        let n = i64::from(n);
        if n > self.window {
            return Err(Http2Error::FlowControlExceeded);
        }
        self.window -= n;
        Ok(())
    }

    /// Applies a `WINDOW_UPDATE`. Spec §6.9.1.
    ///
    /// # Errors
    /// `Protocol(FlowControlError)` if the window
    /// 2^31-1 ueberschreitet.
    pub fn apply_window_update(&mut self, increment: u32) -> Result<(), Http2Error> {
        use crate::error::ErrorCode;
        if increment == 0 {
            // Spec §6.9: increment of 0 must be treated as PROTOCOL_ERROR
            // for streams (or connection-level for stream id 0).
            return Err(Http2Error::Protocol(ErrorCode::ProtocolError));
        }
        let new_window = self.window.saturating_add(i64::from(increment));
        if new_window > self.max {
            return Err(Http2Error::Protocol(ErrorCode::FlowControlError));
        }
        self.window = new_window;
        Ok(())
    }

    /// Apply a new `INITIAL_WINDOW_SIZE` from a SETTINGS update
    /// (spec §6.9.2): the window is adjusted by the difference.
    pub fn apply_initial_window_size_change(&mut self, old: i64, new: i64) {
        let delta = new - old;
        self.window += delta;
    }
}

/// Encodes a WINDOW_UPDATE frame payload (4 bytes, R bit + 31-bit
/// increment). Spec §6.9.
#[must_use]
pub fn encode_window_update(increment: u32) -> [u8; 4] {
    let v = increment & 0x7fff_ffff;
    [
        ((v >> 24) & 0xff) as u8,
        ((v >> 16) & 0xff) as u8,
        ((v >> 8) & 0xff) as u8,
        (v & 0xff) as u8,
    ]
}

/// Decodes a WINDOW_UPDATE frame payload (4 bytes).
///
/// # Errors
/// `Protocol(FrameSizeError)` if the payload is not 4 bytes.
pub fn decode_window_update(payload: &[u8]) -> Result<u32, Http2Error> {
    use crate::error::ErrorCode;
    if payload.len() != 4 {
        return Err(Http2Error::Protocol(ErrorCode::FrameSizeError));
    }
    let v = (u32::from(payload[0]) << 24)
        | (u32::from(payload[1]) << 16)
        | (u32::from(payload[2]) << 8)
        | u32::from(payload[3]);
    Ok(v & 0x7fff_ffff)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_initial() {
        let fc = FlowControl::default();
        assert_eq!(fc.window(), INITIAL_WINDOW_SIZE);
    }

    #[test]
    fn consume_reduces_window() {
        let mut fc = FlowControl::new(1000);
        fc.consume(400).unwrap();
        assert_eq!(fc.window(), 600);
    }

    #[test]
    fn consume_more_than_window_rejected() {
        let mut fc = FlowControl::new(100);
        assert_eq!(fc.consume(101), Err(Http2Error::FlowControlExceeded));
        assert_eq!(fc.window(), 100, "window unchanged on error");
    }

    #[test]
    fn window_update_raises_window() {
        let mut fc = FlowControl::new(0);
        fc.apply_window_update(500).unwrap();
        assert_eq!(fc.window(), 500);
    }

    #[test]
    fn window_update_zero_rejected() {
        let mut fc = FlowControl::default();
        assert!(fc.apply_window_update(0).is_err());
    }

    #[test]
    fn window_update_overflow_rejected() {
        let mut fc = FlowControl::new(0x7fff_fff0);
        assert!(fc.apply_window_update(0x10000).is_err());
    }

    #[test]
    fn initial_window_size_change_adjusts_window() {
        let mut fc = FlowControl::new(1000);
        fc.apply_initial_window_size_change(65_535, 131_070);
        assert_eq!(fc.window(), 1000 + 65_535);
    }

    #[test]
    fn round_trip_window_update_codec() {
        let bytes = encode_window_update(0x12_34_56);
        assert_eq!(decode_window_update(&bytes).unwrap(), 0x12_34_56);
    }

    #[test]
    fn r_bit_stripped_on_decode() {
        let bytes = [0x80, 0x00, 0x00, 0x01]; // R-bit set
        assert_eq!(decode_window_update(&bytes).unwrap(), 1);
    }

    #[test]
    fn wrong_payload_size_rejected() {
        assert!(decode_window_update(&[0; 3]).is_err());
        assert!(decode_window_update(&[0; 5]).is_err());
    }
}
