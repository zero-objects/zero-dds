// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Security error types. OMG DDS-Security 1.2 §9.2.1 `SecurityException`.
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
    /// Human-readable hint (not for end users — internal): the `message` of
    /// the `SecurityException` (DDS-Security 1.2 §9.2.1 Table 28).
    pub detail: Cow<'static, str>,
    /// `code` of the `SecurityException` (Table 28): the number of the
    /// category, [`SecurityErrorKind::code`].
    pub code: i32,
    /// `minor_code` of the `SecurityException` (Table 28): a number the plugin
    /// chooses for the cause within the category; 0 unless it sets one.
    pub minor_code: i32,
}

/// Category of the security error.
///
/// Spec reference OMG DDS-Security 1.2 §9.2.1. The codes are kept
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

impl SecurityErrorKind {
    /// The `code` of a `SecurityException` for this category (Table 28 names
    /// the member, the specification assigns no values): 1 to 7 in the order
    /// of the variants; stable.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::AuthenticationFailed => 1,
            Self::AccessDenied => 2,
            Self::CryptoFailed => 3,
            Self::InvalidConfiguration => 4,
            Self::BadArgument => 5,
            Self::NotImplemented => 6,
            Self::Internal => 7,
        }
    }
}

impl SecurityError {
    /// Constructor: `code` is that of the category, `minor_code` 0.
    #[must_use]
    pub fn new(kind: SecurityErrorKind, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            code: kind.code(),
            minor_code: 0,
        }
    }

    /// The same error with `minor_code` set.
    #[must_use]
    pub fn with_minor_code(mut self, minor_code: i32) -> Self {
        self.minor_code = minor_code;
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    /// DDS-Security 1.2 §9.2.1 Table 28: message, code and minor_code.
    #[test]
    fn a_security_exception_has_message_code_and_minor_code() {
        let e = SecurityError::new(SecurityErrorKind::CryptoFailed, "tag mismatch");
        assert_eq!(
            (e.detail.as_ref(), e.code, e.minor_code),
            ("tag mismatch", 3, 0)
        );
        let e = e.with_minor_code(42);
        assert_eq!((e.code, e.minor_code), (3, 42));
        let codes = [
            SecurityErrorKind::AuthenticationFailed,
            SecurityErrorKind::AccessDenied,
            SecurityErrorKind::CryptoFailed,
            SecurityErrorKind::InvalidConfiguration,
            SecurityErrorKind::BadArgument,
            SecurityErrorKind::NotImplemented,
            SecurityErrorKind::Internal,
        ]
        .map(SecurityErrorKind::code);
        assert_eq!(codes, [1, 2, 3, 4, 5, 6, 7]);
    }
}
