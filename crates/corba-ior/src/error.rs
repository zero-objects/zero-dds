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
    /// What the wire runtime refused. One variant, because
    /// `zerodds-wire` has one error type where `zerodds-cdr` had two;
    /// two variants over one type is a `From` the compiler cannot
    /// choose between.
    Wire(zerodds_wire::WireError),
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

impl From<zerodds_wire::WireError> for IorError {
    fn from(error: zerodds_wire::WireError) -> Self {
        Self::Wire(error)
    }
}
