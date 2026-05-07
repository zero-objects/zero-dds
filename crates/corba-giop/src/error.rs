// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP Wire-Codec-Fehler.

use alloc::string::String;

use zerodds_cdr::{DecodeError, EncodeError};

/// Result-Alias.
pub type GiopResult<T> = Result<T, GiopError>;

/// Fehler im GIOP-Wire-Codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiopError {
    /// Header-Magic ist nicht `"GIOP"`. Spec §15.4.1.
    InvalidMagic([u8; 4]),
    /// GIOP-Major/Minor wird vom Codec nicht unterstuetzt.
    UnsupportedVersion {
        /// Major.
        major: u8,
        /// Minor.
        minor: u8,
    },
    /// `message_type`-Octet ausserhalb 0..=7 (Spec §15.4.1 Tab 15-1).
    UnknownMessageType(u8),
    /// `reply_status`-Wert ausserhalb der 6 Spec-Werte
    /// (Spec §15.4.3.1).
    UnknownReplyStatus(u32),
    /// `locate_status`-Wert ausserhalb der 5 Spec-Werte
    /// (Spec §15.4.6.1).
    UnknownLocateStatus(u32),
    /// `addressing_disposition`-Wert ausserhalb 0..=2 (Spec §15.4.2.2).
    UnknownAddressingDisposition(u16),
    /// Fragment-Bit gesetzt aber GIOP-Version unterstuetzt keine
    /// Fragments (Spec §15.4.9: GIOP 1.0 hat kein Fragment-Support).
    FragmentNotSupported {
        /// Major.
        major: u8,
        /// Minor.
        minor: u8,
    },
    /// Body-Groesse ueberschreitet das `message_size`-Feld.
    BodyTooLarge {
        /// Body-Groesse.
        body_size: usize,
        /// Header-Limit.
        message_size_field: u32,
    },
    /// CDR-Decode-Fehler.
    Decode(DecodeError),
    /// CDR-Encode-Fehler.
    Encode(EncodeError),
    /// Allgemeiner Wire-Format-Fehler mit Diagnose-String.
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
