// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Security-Error-Typen. OMG DDS-Security 1.1 §8.1.2 `SecurityException`.
//!
//! Ein gemeinsamer Error-Typ ueber alle Plugins — damit Call-Sites nicht
//! fuenf verschiedene Result-Variants handhaben muessen. Die `Kind`-
//! Varianten matchen die Spec-Fehlercodes.

extern crate alloc;

use alloc::borrow::Cow;
use core::fmt;

/// Security-Operation-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SecurityError {
    /// Fehler-Kategorie.
    pub kind: SecurityErrorKind,
    /// Menschenlesbarer Hinweis (nicht fuer Endnutzer — intern).
    pub detail: Cow<'static, str>,
}

/// Kategorie des Security-Fehlers.
///
/// Spec-Bezug OMG DDS-Security 1.1 §8.1.2. Die Codes sind offen
/// gehalten (`#[non_exhaustive]`), damit v1.4 zusaetzliche Varianten
/// ohne Breaking-Change einfuegen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecurityErrorKind {
    /// Identity-Handshake fehlgeschlagen (Zertifikat ungueltig,
    /// Signatur falsch, Peer nicht im Trust-Store).
    AuthenticationFailed,
    /// Peer hat keine Permission fuer diese Operation.
    AccessDenied,
    /// Cryptographic-Operation fehlgeschlagen (AES-GCM-Tag-Verify,
    /// HMAC-Mismatch, Key-unbekannt).
    CryptoFailed,
    /// Fehlende oder unplausible Konfiguration (z.B. Cert fehlt,
    /// Permissions-XML parst nicht).
    InvalidConfiguration,
    /// Ungueltiges Argument (None wo Some erwartet, leerer Key usw.).
    BadArgument,
    /// Feature nicht implementiert. v1.3-Plugin-SPI signalisiert so,
    /// wenn eine Methode in v1.4 vorgesehen ist.
    NotImplemented,
    /// Interne unerwartete Error — Plugin-Bug.
    Internal,
}

impl SecurityError {
    /// Konstruktor.
    #[must_use]
    pub fn new(kind: SecurityErrorKind, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Shortcut fuer `NotImplemented`.
    #[must_use]
    pub fn not_implemented(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::new(SecurityErrorKind::NotImplemented, detail)
    }

    /// Shortcut fuer `BadArgument`.
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

/// Plugin-Operation-Result.
pub type SecurityResult<T> = core::result::Result<T, SecurityError>;
