// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP wire codec errors.

use alloc::string::String;

use zerodds_wire::WireError;

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
    /// The CDR layer refused.
    ///
    /// One variant where there used to be two, `Decode` and `Encode`.
    /// `zerodds-wire` has a single [`WireError`] because the two halves
    /// answer the same questions — a length that cannot be true and a
    /// value a type cannot hold are the same refusal read either way —
    /// and a caller that had to match on both wrote the arm twice.
    Wire(WireError),
    /// General wire-format error with a diagnostic string.
    Malformed(String),
}

impl From<WireError> for GiopError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}
