// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP wire codec errors.

use alloc::string::String;

use zerodds_cdr::{DecodeError, EncodeError};

/// Result alias.
pub type GiopResult<T> = Result<T, GiopError>;

/// Error in the GIOP wire codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiopError {
    /// Header magic is not `"GIOP"`. Spec §15.4.1.
    InvalidMagic([u8; 4]),
    /// GIOP major/minor is not supported by the codec.
    UnsupportedVersion {
        /// Major.
        major: u8,
        /// Minor.
        minor: u8,
    },
    /// `message_type` octet outside 0..=7 (spec §15.4.1 Tab 15-1).
    UnknownMessageType(u8),
    /// `reply_status` value outside the 6 spec values
    /// (spec §15.4.3.1).
    UnknownReplyStatus(u32),
    /// `locate_status` value outside the 5 spec values
    /// (spec §15.4.6.1).
    UnknownLocateStatus(u32),
    /// `addressing_disposition` value outside 0..=2 (spec §15.4.2.2).
    UnknownAddressingDisposition(u16),
    /// Fragment bit set but the GIOP version does not support
    /// fragments (spec §15.4.9: GIOP 1.0 has no fragment support).
    FragmentNotSupported {
        /// Major.
        major: u8,
        /// Minor.
        minor: u8,
    },
    /// Body size exceeds the `message_size` field.
    BodyTooLarge {
        /// Body size.
        body_size: usize,
        /// Header limit.
        message_size_field: u32,
    },
    /// CDR decode error.
    Decode(DecodeError),
    /// CDR encode error.
    Encode(EncodeError),
    /// General wire-format error with a diagnostic string.
    Malformed(String),
}

impl From<DecodeError> for GiopError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<EncodeError> for GiopError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}
