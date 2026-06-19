// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Security error types. OMG DDS-Security 1.1 §8.1.2 `SecurityException`.
//!
//! A common error type across all plugins — so that call sites do not
//! have to handle five different result variants. The `Kind`
//! variants match the spec error codes.

extern crate alloc;

use alloc::borrow::Cow;
use core::fmt;

/// Security operation error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SecurityError {
    /// Error category.
    pub kind: SecurityErrorKind,
    /// Human-readable hint (not for end users — internal).
    pub detail: Cow<'static, str>,
}

/// Category of the security error.
///
/// Spec reference OMG DDS-Security 1.1 §8.1.2. The codes are kept
/// open (`#[non_exhaustive]`) so that v1.4 can add additional variants
/// without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecurityErrorKind {
    /// Identity handshake failed (certificate invalid,
    /// signature wrong, peer not in the trust store).
    AuthenticationFailed,
    /// The peer has no permission for this operation.
    AccessDenied,
    /// Cryptographic operation failed (AES-GCM tag verify,
    /// HMAC mismatch, key unknown).
    CryptoFailed,
    /// Missing or implausible configuration (e.g. cert missing,
    /// permissions XML does not parse).
    InvalidConfiguration,
    /// Invalid argument (None where Some expected, empty key, etc.).
    BadArgument,
    /// Feature not implemented. The v1.3 plugin SPI signals this way
    /// when a method is planned for v1.4.
    NotImplemented,
    /// Internal unexpected error — plugin bug.
    Internal,
}

impl SecurityError {
    /// Constructor.
    #[must_use]
    pub fn new(kind: SecurityErrorKind, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Shortcut for `NotImplemented`.
    #[must_use]
    pub fn not_implemented(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::new(SecurityErrorKind::NotImplemented, detail)
    }

    /// Shortcut for `BadArgument`.
    #[must_use]
    pub fn bad_argument(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::new(SecurityErrorKind::BadArgument, detail)
    }
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "security error [{:?}]: {}", self.kind, self.detail)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SecurityError {}

/// Plugin operation result.
pub type SecurityResult<T> = core::result::Result<T, SecurityError>;
