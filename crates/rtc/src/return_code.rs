// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ReturnCode_t` — Spec §5.2.1.

use core::fmt;

/// `ReturnCode_t` (spec §5.2.1.1-§5.2.1.6, p. 9-10) — status codes
/// for all RTC operations that do not by nature return another
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReturnCode {
    /// `OK` (§5.2.1.1) — Operation completed successfully.
    Ok,
    /// `ERROR` (§5.2.1.2) — Generic, unspecified failure.
    Error,
    /// `BAD_PARAMETER` (§5.2.1.3) — Illegal argument.
    BadParameter,
    /// `UNSUPPORTED` (§5.2.1.4) — Compliance-Point not implemented.
    Unsupported,
    /// `OUT_OF_RESOURCES` (§5.2.1.5) — Resource exhausted.
    OutOfResources,
    /// `PRECONDITION_NOT_MET` (§5.2.1.6) — State-Machine-Violation
    /// (e.g. `initialize` while not in the `Created` state).
    PreconditionNotMet,
}

impl ReturnCode {
    /// Spec §5.2.1.1 — `true` if `Ok`.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Converts to `Result<(), ReturnCode>` for ergonomic
    /// use of the `?` operator in the caller.
    ///
    /// # Errors
    /// Returns `Err(self)` if not `Ok`.
    pub const fn into_result(self) -> Result<(), Self> {
        match self {
            Self::Ok => Ok(()),
            other => Err(other),
        }
    }
}

impl fmt::Display for ReturnCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ok => "OK",
            Self::Error => "ERROR",
            Self::BadParameter => "BAD_PARAMETER",
            Self::Unsupported => "UNSUPPORTED",
            Self::OutOfResources => "OUT_OF_RESOURCES",
            Self::PreconditionNotMet => "PRECONDITION_NOT_MET",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReturnCode {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_is_ok_and_others_are_not() {
        // Spec §5.2.1.1.
        assert!(ReturnCode::Ok.is_ok());
        for rc in [
            ReturnCode::Error,
            ReturnCode::BadParameter,
            ReturnCode::Unsupported,
            ReturnCode::OutOfResources,
            ReturnCode::PreconditionNotMet,
        ] {
            assert!(!rc.is_ok());
        }
    }

    #[test]
    fn into_result_maps_ok_to_unit_and_others_to_err() {
        assert_eq!(ReturnCode::Ok.into_result(), Ok(()));
        assert_eq!(
            ReturnCode::PreconditionNotMet.into_result(),
            Err(ReturnCode::PreconditionNotMet)
        );
    }

    #[test]
    fn display_reports_spec_token_names() {
        assert_eq!(alloc::format!("{}", ReturnCode::Ok), "OK");
        assert_eq!(
            alloc::format!("{}", ReturnCode::PreconditionNotMet),
            "PRECONDITION_NOT_MET"
        );
        assert_eq!(
            alloc::format!("{}", ReturnCode::BadParameter),
            "BAD_PARAMETER"
        );
    }
}
