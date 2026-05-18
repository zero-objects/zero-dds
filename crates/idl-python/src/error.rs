// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error-Familie fuer `zerodds-idl-python`-Codegen.

use core::fmt;

/// Codegen-Fehler beim Emittieren von Python-Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlPythonError {
    /// IDL-Konstrukt ausserhalb des unterstuetzten Python-Mapping-Scopes
    /// (z.B. Bitmask, Map, Fixed, Any, Valuetype, Interface). Phase-2-
    /// Material.
    Unsupported(String),
    /// Sprach-Map-Konflikt: ein IDL-Identifier kollidiert mit einem
    /// Python-Reserved-Word und der konfigurierte Escape-Mode wuerde
    /// dazu fuehren, dass zwei verschiedene IDL-Namen denselben Python-
    /// Namen ergeben.
    NameConflict(String),
}

impl fmt::Display for IdlPythonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(msg) => write!(f, "unsupported IDL construct: {msg}"),
            Self::NameConflict(msg) => write!(f, "python name conflict: {msg}"),
        }
    }
}

impl std::error::Error for IdlPythonError {}

/// Spezialisiertes `Result` fuer Codegen-Aufrufe.
pub type Result<T> = core::result::Result<T, IdlPythonError>;
