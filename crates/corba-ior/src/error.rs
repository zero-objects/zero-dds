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
    /// The text does not begin with the scheme `IOR:` (in any case) of a
    /// stringified IOR.
    MissingIorPrefix,
    /// Stringified IOR has an odd hex count.
    OddHexLength,
    /// Stringified IOR contains non-hex characters.
    InvalidHexChar(char),
    /// `corbaloc:` URL has the wrong scheme.
    InvalidUrlScheme(String),
    /// `corbaloc:` address has an invalid host:port.
    InvalidCorbalocAddress(String),
    /// `rir` beside another address in one URL (Part 2 §7.6.10.2: "The
    /// rir protocol cannot be used with any other protocol in a URL").
    RirWithOtherProtocol(String),
    /// A bad `%XX` escape in a key string or a stringified name, or a
    /// stringified name that is not UTF-8.
    InvalidKeyString(String),
    /// Generic wire error.
    Malformed(String),
}

impl IorError {
    /// The standard minor code of the `BAD_PARAM` exception
    /// `string_to_object` raises for this error (Part 2 §7.6.10): 7 bad
    /// scheme name (also text without the `IOR:` scheme where a
    /// stringified IOR is read), 8 bad address, 9 bad schema specific
    /// part (an encoding of the IOR or a key string that does not
    /// decode), 10 any other reason.
    #[must_use]
    pub fn bad_param_minor(&self) -> u32 {
        match self {
            Self::InvalidUrlScheme(_) | Self::MissingIorPrefix => 7,
            Self::InvalidCorbalocAddress(_) | Self::RirWithOtherProtocol(_) => 8,
            Self::InvalidKeyString(_)
            | Self::OddHexLength
            | Self::InvalidHexChar(_)
            | Self::Cdr(_)
            | Self::Wire(_) => 9,
            Self::Malformed(_) => 10,
        }
    }
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
