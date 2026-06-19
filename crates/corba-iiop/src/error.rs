// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP transport errors.

use alloc::string::String;

use zerodds_corba_giop::GiopError;

use crate::profile_body::CdrError;

/// IIOP transport error.
#[derive(Debug)]
pub enum IiopError {
    /// TCP bind or TCP connect error.
    Io(std::io::Error),
    /// GIOP wire-codec error.
    Giop(GiopError),
    /// CDR marshalling error in ProfileBody / BiDir.
    Cdr(CdrError),
    /// Connection was closed by the peer.
    Closed,
    /// Pool is exhausted (`max_connections` reached).
    PoolExhausted,
    /// Catch-all diagnostic message.
    Other(String),
}

impl From<std::io::Error> for IiopError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<GiopError> for IiopError {
    fn from(e: GiopError) -> Self {
        Self::Giop(e)
    }
}

impl From<CdrError> for IiopError {
    fn from(e: CdrError) -> Self {
        Self::Cdr(e)
    }
}

impl core::fmt::Display for IiopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Giop(e) => write!(f, "giop error: {e:?}"),
            Self::Cdr(e) => write!(f, "cdr error: {e:?}"),
            Self::Closed => write!(f, "connection closed by peer"),
            Self::PoolExhausted => write!(f, "connection pool exhausted"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for IiopError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
