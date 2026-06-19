// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP Message-Type Enum — Spec §15.4.1 Table 15-1.

use crate::error::{GiopError, GiopResult};

/// GIOP message type — `octet` value in the 11th byte of the header.
///
/// Spec §15.4.1 Table 15-1: 8 standardized values (0..7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// `Request` — client method invocation (spec §15.4.2).
    Request = 0,
    /// `Reply` — server response (spec §15.4.3).
    Reply = 1,
    /// `CancelRequest` — client cancels a pending request (spec §15.4.4).
    CancelRequest = 2,
    /// `LocateRequest` — object location probe (spec §15.4.5).
    LocateRequest = 3,
    /// `LocateReply` — reply to a LocateRequest (spec §15.4.6).
    LocateReply = 4,
    /// `CloseConnection` — server closes the connection (spec §15.4.7).
    CloseConnection = 5,
    /// `MessageError` — wire error (spec §15.4.8).
    MessageError = 6,
    /// `Fragment` — multi-frame message body (spec §15.4.9, as of GIOP 1.1).
    Fragment = 7,
}

impl MessageType {
    /// Discriminant value (octet representation).
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Parsing from an octet (spec §15.4.1 Table 15-1).
    ///
    /// # Errors
    /// `GiopError::UnknownMessageType` if the octet matches none of the
    /// 8 standard values.
    pub const fn from_u8(value: u8) -> GiopResult<Self> {
        match value {
            0 => Ok(Self::Request),
            1 => Ok(Self::Reply),
            2 => Ok(Self::CancelRequest),
            3 => Ok(Self::LocateRequest),
            4 => Ok(Self::LocateReply),
            5 => Ok(Self::CloseConnection),
            6 => Ok(Self::MessageError),
            7 => Ok(Self::Fragment),
            other => Err(GiopError::UnknownMessageType(other)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn message_type_values_match_spec_table_15_1() {
        // Spec §15.4.1 Table 15-1.
        assert_eq!(MessageType::Request.as_u8(), 0);
        assert_eq!(MessageType::Reply.as_u8(), 1);
        assert_eq!(MessageType::CancelRequest.as_u8(), 2);
        assert_eq!(MessageType::LocateRequest.as_u8(), 3);
        assert_eq!(MessageType::LocateReply.as_u8(), 4);
        assert_eq!(MessageType::CloseConnection.as_u8(), 5);
        assert_eq!(MessageType::MessageError.as_u8(), 6);
        assert_eq!(MessageType::Fragment.as_u8(), 7);
    }

    #[test]
    fn from_u8_round_trip() {
        for v in 0u8..=7 {
            let mt = MessageType::from_u8(v).unwrap();
            assert_eq!(mt.as_u8(), v);
        }
    }

    #[test]
    fn unknown_message_type_is_diagnostic() {
        let err = MessageType::from_u8(8).unwrap_err();
        assert!(matches!(err, GiopError::UnknownMessageType(8)));
    }
}
