// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The standard CORBA system exceptions the Time Service raises.
//!
//! OMG Time Service 1.1 names four of them: `CORBA::BAD_PARAM`
//! (§2.1.2.1 `new_universal_time`, §2.1.2.3 `new_interval`, §2.3.1.2
//! `set_timer`), `CORBA::DATA_CONVERSION` (§1.3.4.5 `absolute_time`),
//! `CORBA::NO_RESOURCES` (§2.4.1 `register`) and `CORBA::INV_OBJREF`
//! (§2.4.2, use of an unregistered timer event handler). This module has
//! the first two; the Timer Event Service reports the others in
//! `timer_event::TimerEventError`. The service has one user exception of
//! its own, [`crate::TimeUnavailable`] (§1.3.3.1); the Timer Event Service
//! has none (§2.2.4).

use core::fmt;

/// `CORBA::BAD_PARAM` — a parameter is out of range: an inaccuracy above
/// 48 bits (§1.3.2.2, §2.1.2.1), a lower bound above the upper bound
/// (§2.1.2.3), a relative time of zero with no earlier relative time or a
/// trigger time that does not fit `TimeT` (§2.3.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadParam;

impl fmt::Display for BadParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CORBA::BAD_PARAM: parameter out of range")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BadParam {}

/// `CORBA::DATA_CONVERSION` — a result cannot be represented: the sum of
/// current time and relative time overflows `TimeT` (§1.3.4.5), or half
/// the width of an interval exceeds the 48 bits of `InaccuracyT`
/// (§1.3.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataConversion;

impl fmt::Display for DataConversion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CORBA::DATA_CONVERSION: result not representable")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DataConversion {}

/// Why `absolute_time` (§1.3.4.5) gave no UTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsoluteTimeError {
    /// `CORBA::DATA_CONVERSION`: current time plus the relative time
    /// overflows `TimeT`.
    DataConversion,
    /// The clock of the service gave no current time (the condition of
    /// [`crate::TimeUnavailable`], §1.3.3.1). The IDL of `absolute_time`
    /// has no `raises` clause; a CORBA server reports it as a system
    /// exception.
    TimeUnavailable,
}

impl fmt::Display for AbsoluteTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataConversion => fmt::Display::fmt(&DataConversion, f),
            Self::TimeUnavailable => write!(f, "underlying time service unavailable"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AbsoluteTimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_1_3_3_system_exceptions_name_the_corba_exception() {
        assert!(alloc::format!("{BadParam}").starts_with("CORBA::BAD_PARAM"));
        assert!(alloc::format!("{DataConversion}").starts_with("CORBA::DATA_CONVERSION"));
        assert!(
            alloc::format!("{}", AbsoluteTimeError::DataConversion)
                .starts_with("CORBA::DATA_CONVERSION")
        );
        assert!(alloc::format!("{}", AbsoluteTimeError::TimeUnavailable).contains("unavailable"));
    }
}
