// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehlertypen fuer DynamicType / DynamicData (Spec §7.5.6 ReturnCode).

use alloc::string::String;
use core::fmt;

/// Spec-genannter Return-Code (§7.5.6.4) — nicht alle Codes sind in
/// Phase 4 belegt; die wichtigsten sechs sind hier abgebildet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DynamicError {
    /// `BadParameter` — illegales Argument (Type-Mismatch, falsche ID).
    BadParameter(String),
    /// `PreconditionNotMet` — Operation aktuell nicht erlaubt.
    PreconditionNotMet(String),
    /// `IllegalOperation` — auf diesen Kind nicht definiert.
    IllegalOperation(String),
    /// `Unsupported` — Spec-Konstrukt noch nicht implementiert.
    Unsupported(String),
    /// Type/Member-Inkonsistenz (gefunden via `is_consistent`).
    Inconsistent(String),
    /// Builder-Konflikt: dup name/id, fehlende Required-Felder etc.
    BuilderConflict(String),
    /// Loan-Lifecycle-Verletzung.
    LoanError(String),
}

impl DynamicError {
    /// Helper.
    pub(crate) fn bad_parameter(msg: impl Into<String>) -> Self {
        Self::BadParameter(msg.into())
    }

    /// Helper.
    pub(crate) fn inconsistent(msg: impl Into<String>) -> Self {
        Self::Inconsistent(msg.into())
    }

    /// Helper.
    pub(crate) fn builder(msg: impl Into<String>) -> Self {
        Self::BuilderConflict(msg.into())
    }

    /// Helper.
    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(msg.into())
    }

    /// Helper.
    pub(crate) fn loan(msg: impl Into<String>) -> Self {
        Self::LoanError(msg.into())
    }
}

impl fmt::Display for DynamicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadParameter(m) => write!(f, "bad parameter: {m}"),
            Self::PreconditionNotMet(m) => write!(f, "precondition not met: {m}"),
            Self::IllegalOperation(m) => write!(f, "illegal operation: {m}"),
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
            Self::Inconsistent(m) => write!(f, "inconsistent: {m}"),
            Self::BuilderConflict(m) => write!(f, "builder conflict: {m}"),
            Self::LoanError(m) => write!(f, "loan error: {m}"),
        }
    }
}

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
impl std::error::Error for DynamicError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn dynamic_error_display_emits_helpful_text() {
        let e = DynamicError::bad_parameter("x");
        assert!(e.to_string().contains("bad parameter"));
        assert!(e.to_string().contains('x'));
    }
}
