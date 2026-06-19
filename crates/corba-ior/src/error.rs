// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR codec errors.

use alloc::string::String;

use zerodds_corba_iiop::profile_body::CdrError;

/// Result alias.
pub type IorResult<T> = Result<T, IorError>;

/// IOR codec error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IorError {
    /// CDR marshalling error.
    Cdr(CdrError),
    /// `zerodds-cdr` decode error.
    CdrDecode(zerodds_cdr::DecodeError),
    /// `zerodds-cdr` encode error.
    CdrEncode(zerodds_cdr::EncodeError),
    /// Stringified IOR is missing the `IOR:` prefix.
    MissingIorPrefix,
    /// Stringified IOR has an odd hex count.
    OddHexLength,
    /// Stringified IOR contains non-hex characters.
    InvalidHexChar(char),
    /// `corbaloc:` URL has the wrong scheme.
    InvalidUrlScheme(String),
    /// `corbaloc:` address has an invalid host:port.
    InvalidCorbalocAddress(String),
    /// Generic wire error.
    Malformed(String),
}

impl From<CdrError> for IorError {
    fn from(e: CdrError) -> Self {
        Self::Cdr(e)
    }
}

impl From<zerodds_cdr::DecodeError> for IorError {
    fn from(e: zerodds_cdr::DecodeError) -> Self {
        Self::CdrDecode(e)
    }
}

impl From<zerodds_cdr::EncodeError> for IorError {
    fn from(e: zerodds_cdr::EncodeError) -> Self {
        Self::CdrEncode(e)
    }
}
