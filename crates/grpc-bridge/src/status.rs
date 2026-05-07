// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC `grpc-status` Codes — Spec §"Status".

/// gRPC Status-Codes (kanonische 0..=16).
///
/// Spec: `https://github.com/grpc/grpc/blob/master/doc/statuscodes.md`
/// + `protocol-http2.md` §"Status".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// `0` Ok.
    Ok,
    /// `1` Cancelled.
    Cancelled,
    /// `2` Unknown.
    Unknown,
    /// `3` InvalidArgument.
    InvalidArgument,
    /// `4` DeadlineExceeded.
    DeadlineExceeded,
    /// `5` NotFound.
    NotFound,
    /// `6` AlreadyExists.
    AlreadyExists,
    /// `7` PermissionDenied.
    PermissionDenied,
    /// `8` ResourceExhausted.
    ResourceExhausted,
    /// `9` FailedPrecondition.
    FailedPrecondition,
    /// `10` Aborted.
    Aborted,
    /// `11` OutOfRange.
    OutOfRange,
    /// `12` Unimplemented.
    Unimplemented,
    /// `13` Internal.
    Internal,
    /// `14` Unavailable.
    Unavailable,
    /// `15` DataLoss.
    DataLoss,
    /// `16` Unauthenticated.
    Unauthenticated,
}

impl Status {
    /// Numerischer Wert (0..=16).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Cancelled => 1,
            Self::Unknown => 2,
            Self::InvalidArgument => 3,
            Self::DeadlineExceeded => 4,
            Self::NotFound => 5,
            Self::AlreadyExists => 6,
            Self::PermissionDenied => 7,
            Self::ResourceExhausted => 8,
            Self::FailedPrecondition => 9,
            Self::Aborted => 10,
            Self::OutOfRange => 11,
            Self::Unimplemented => 12,
            Self::Internal => 13,
            Self::Unavailable => 14,
            Self::DataLoss => 15,
            Self::Unauthenticated => 16,
        }
    }

    /// Konstruiert aus numerischem Wert. Liefert `None` fuer
    /// Unbekanntes.
    #[must_use]
    pub const fn from_code(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Ok),
            1 => Some(Self::Cancelled),
            2 => Some(Self::Unknown),
            3 => Some(Self::InvalidArgument),
            4 => Some(Self::DeadlineExceeded),
            5 => Some(Self::NotFound),
            6 => Some(Self::AlreadyExists),
            7 => Some(Self::PermissionDenied),
            8 => Some(Self::ResourceExhausted),
            9 => Some(Self::FailedPrecondition),
            10 => Some(Self::Aborted),
            11 => Some(Self::OutOfRange),
            12 => Some(Self::Unimplemented),
            13 => Some(Self::Internal),
            14 => Some(Self::Unavailable),
            15 => Some(Self::DataLoss),
            16 => Some(Self::Unauthenticated),
            _ => None,
        }
    }

    /// `true` fuer `Status::Ok`.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Kanonischer Name (Caller verwendet typisch `grpc-message`-
    /// Header zusaetzlich).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Cancelled => "CANCELLED",
            Self::Unknown => "UNKNOWN",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::DeadlineExceeded => "DEADLINE_EXCEEDED",
            Self::NotFound => "NOT_FOUND",
            Self::AlreadyExists => "ALREADY_EXISTS",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::ResourceExhausted => "RESOURCE_EXHAUSTED",
            Self::FailedPrecondition => "FAILED_PRECONDITION",
            Self::Aborted => "ABORTED",
            Self::OutOfRange => "OUT_OF_RANGE",
            Self::Unimplemented => "UNIMPLEMENTED",
            Self::Internal => "INTERNAL",
            Self::Unavailable => "UNAVAILABLE",
            Self::DataLoss => "DATA_LOSS",
            Self::Unauthenticated => "UNAUTHENTICATED",
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn all_17_codes_round_trip_via_numeric() {
        for n in 0..=16u8 {
            let s = Status::from_code(n).expect("ok");
            assert_eq!(s.code(), n);
        }
    }

    #[test]
    fn unknown_codes_yield_none() {
        assert_eq!(Status::from_code(17), None);
        assert_eq!(Status::from_code(255), None);
    }

    #[test]
    fn ok_is_only_status_with_is_ok_true() {
        assert!(Status::Ok.is_ok());
        for n in 1..=16u8 {
            let s = Status::from_code(n).unwrap_or(Status::Ok);
            assert!(!s.is_ok() || s.code() == 0);
        }
    }

    #[test]
    fn well_known_codes_match_spec_values() {
        // Spec — kanonische Tabelle.
        assert_eq!(Status::Ok.code(), 0);
        assert_eq!(Status::Cancelled.code(), 1);
        assert_eq!(Status::DeadlineExceeded.code(), 4);
        assert_eq!(Status::NotFound.code(), 5);
        assert_eq!(Status::Unimplemented.code(), 12);
        assert_eq!(Status::Unavailable.code(), 14);
        assert_eq!(Status::Unauthenticated.code(), 16);
    }

    #[test]
    fn names_use_screaming_snake_case() {
        // Konventional aus statuscodes.md.
        assert_eq!(Status::Ok.name(), "OK");
        assert_eq!(Status::DeadlineExceeded.name(), "DEADLINE_EXCEEDED");
        assert_eq!(Status::FailedPrecondition.name(), "FAILED_PRECONDITION");
    }
}
