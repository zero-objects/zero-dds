// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR-Codec-Fehler.

use alloc::string::String;

use zerodds_corba_iiop::profile_body::CdrError;

/// Result-Alias.
pub type IorResult<T> = Result<T, IorError>;

/// IOR-Codec-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IorError {
    /// CDR-Marshalling-Fehler.
    Cdr(CdrError),
    /// `zerodds-cdr`-Decode-Fehler.
    CdrDecode(zerodds_cdr::DecodeError),
    /// `zerodds-cdr`-Encode-Fehler.
    CdrEncode(zerodds_cdr::EncodeError),
    /// stringified-IOR hat das `IOR:`-Prefix nicht.
    MissingIorPrefix,
    /// stringified-IOR hat ungerade Hex-Anzahl.
    OddHexLength,
    /// stringified-IOR enthaelt nicht-Hex-Zeichen.
    InvalidHexChar(char),
    /// `corbaloc:` URL hat falsches Schema.
    InvalidUrlScheme(String),
    /// `corbaloc:`-Address hat ungueltiges Host:Port.
    InvalidCorbalocAddress(String),
    /// Generischer Wire-Fehler.
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
