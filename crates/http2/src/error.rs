// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HTTP/2 error codes — RFC 9113 §7.

use core::fmt;

/// Error code (RFC 9113 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    /// `NO_ERROR` (0x0).
    NoError = 0x0,
    /// `PROTOCOL_ERROR` (0x1).
    ProtocolError = 0x1,
    /// `INTERNAL_ERROR` (0x2).
    InternalError = 0x2,
    /// `FLOW_CONTROL_ERROR` (0x3).
    FlowControlError = 0x3,
    /// `SETTINGS_TIMEOUT` (0x4).
    SettingsTimeout = 0x4,
    /// `STREAM_CLOSED` (0x5).
    StreamClosed = 0x5,
    /// `FRAME_SIZE_ERROR` (0x6).
    FrameSizeError = 0x6,
    /// `REFUSED_STREAM` (0x7).
    RefusedStream = 0x7,
    /// `CANCEL` (0x8).
    Cancel = 0x8,
    /// `COMPRESSION_ERROR` (0x9).
    CompressionError = 0x9,
    /// `CONNECT_ERROR` (0xa).
    ConnectError = 0xa,
    /// `ENHANCE_YOUR_CALM` (0xb).
    EnhanceYourCalm = 0xb,
    /// `INADEQUATE_SECURITY` (0xc).
    InadequateSecurity = 0xc,
    /// `HTTP_1_1_REQUIRED` (0xd).
    Http11Required = 0xd,
}

impl ErrorCode {
    /// Mapping `u32 -> ErrorCode`. Unknown codes return
    /// `InternalError` as a fallback (Spec §7: receivers may treat
    /// unknown codes as `INTERNAL_ERROR`).
    #[must_use]
    pub fn from_u32(v: u32) -> Self {
        match v {
            0x0 => Self::NoError,
            0x1 => Self::ProtocolError,
            0x2 => Self::InternalError,
            0x3 => Self::FlowControlError,
            0x4 => Self::SettingsTimeout,
            0x5 => Self::StreamClosed,
            0x6 => Self::FrameSizeError,
            0x7 => Self::RefusedStream,
            0x8 => Self::Cancel,
            0x9 => Self::CompressionError,
            0xa => Self::ConnectError,
            0xb => Self::EnhanceYourCalm,
            0xc => Self::InadequateSecurity,
            0xd => Self::Http11Required,
            _ => Self::InternalError,
        }
    }
}

/// HTTP/2-layer error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Http2Error {
    /// Frame header too short (< 9 bytes).
    ShortFrameHeader,
    /// Payload too short for the frame type.
    ShortPayload,
    /// Frame length exceeds `MAX_FRAME_SIZE`.
    FrameTooLarge {
        /// Received length.
        got: u32,
        /// Current max frame size.
        max: u32,
    },
    /// Unknown/reserved frame type (Spec §4.1: SHOULD ignore).
    UnknownFrameType(u8),
    /// Wrong connection preface.
    BadPreface,
    /// Stream state does not allow this frame type.
    InvalidState,
    /// Stream id 0 where stream != 0 is required.
    StreamIdZero,
    /// Stream id != 0 where stream == 0 is required (e.g. SETTINGS).
    StreamIdNonZero,
    /// Flow-control window underrun.
    FlowControlExceeded,
    /// Generic protocol error with code.
    Protocol(ErrorCode),
}

impl fmt::Display for Http2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortFrameHeader => f.write_str("short frame header (< 9 bytes)"),
            Self::ShortPayload => f.write_str("short frame payload"),
            Self::FrameTooLarge { got, max } => write!(f, "frame too large: {got} > {max}"),
            Self::UnknownFrameType(t) => write!(f, "unknown frame type 0x{t:x}"),
            Self::BadPreface => f.write_str("bad connection preface"),
            Self::InvalidState => f.write_str("invalid stream state for frame"),
            Self::StreamIdZero => f.write_str("stream id 0 not allowed"),
            Self::StreamIdNonZero => f.write_str("stream id must be 0"),
            Self::FlowControlExceeded => f.write_str("flow control window exceeded"),
            Self::Protocol(e) => write!(f, "protocol error: {e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Http2Error {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn from_u32_round_trip() {
        for code in [
            ErrorCode::NoError,
            ErrorCode::ProtocolError,
            ErrorCode::InternalError,
            ErrorCode::FlowControlError,
            ErrorCode::SettingsTimeout,
            ErrorCode::StreamClosed,
            ErrorCode::FrameSizeError,
            ErrorCode::RefusedStream,
            ErrorCode::Cancel,
            ErrorCode::CompressionError,
            ErrorCode::ConnectError,
            ErrorCode::EnhanceYourCalm,
            ErrorCode::InadequateSecurity,
            ErrorCode::Http11Required,
        ] {
            assert_eq!(ErrorCode::from_u32(code as u32), code);
        }
    }

    #[test]
    fn unknown_code_maps_to_internal_error() {
        assert_eq!(ErrorCode::from_u32(0xffff), ErrorCode::InternalError);
    }

    #[test]
    fn error_display_does_not_panic() {
        let _ = alloc::format!("{}", Http2Error::ShortFrameHeader);
        let _ = alloc::format!("{}", Http2Error::FrameTooLarge { got: 1, max: 0 });
    }
}
