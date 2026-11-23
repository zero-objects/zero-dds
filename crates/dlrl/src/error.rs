// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The exceptions of the DLRL API (DDS 1.2 §8.1.6.2, §8.2.1.2.1).
//!
//! §8.1.6.2 lists eight exceptions: `DCPSError`, `BadHomeDefinition`,
//! `NotFound`, `AlreadyExisting`, `AlreadyDeleted`, `PreconditionNotMet`,
//! `NoSuchElement`, `SQLError`. Three more are raised by the text but are
//! neither in that list nor declared in the generic IDL of §8.2.1.2.1:
//! `ExpiredTimeOut` (`Cache::lock`, §8.1.6.3.4), `ReadOnlyMode`
//! (`CacheAccess::write` in the IDL of §8.2.1.2.1) and `DDS::BadParameter`
//! (`FooHome::create_selection` in the implied IDL of §8.2.1.2.2). All
//! eleven are built; each carries the string attribute `message` that
//! §8.1.6.2 requires.
//!
//! The generic IDL declares the exceptions in `module DDS` without a
//! `#pragma prefix`, so their repository ids are `IDL:DDS/<name>:1.0`.

use alloc::format;
use alloc::string::String;

/// An exception of the DLRL API. "Each exception contains a string
/// attribute named 'message', that gives a more precise explanation of
/// the reason for the exception" (§8.1.6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlException {
    /// `DCPSError` — an unexpected error in the DCPS.
    DcpsError {
        /// The message.
        message: String,
    },
    /// `BadHomeDefinition` — a registered ObjectHome depends on an
    /// ObjectHome that is not registered.
    BadHomeDefinition {
        /// The message.
        message: String,
    },
    /// `NotFound` — a reference to an object that has not (yet) been
    /// received, or to a class without subscription (§8.1.6.1.2.1).
    NotFound {
        /// The message.
        message: String,
    },
    /// `AlreadyExisting` — an identity already in use.
    AlreadyExisting {
        /// The message.
        message: String,
    },
    /// `AlreadyDeleted` — an operation on an object already deleted.
    AlreadyDeleted {
        /// The message.
        message: String,
    },
    /// `PreconditionNotMet` — a precondition of the operation is not met.
    PreconditionNotMet {
        /// The message.
        message: String,
    },
    /// `NoSuchElement` — a non-existing element of a collection.
    NoSuchElement {
        /// The message.
        message: String,
    },
    /// `SQLError` — an SQL expression with bad syntax, non-existing
    /// fields, or not consistent with its parameters (Annex B).
    SqlError {
        /// The message.
        message: String,
    },
    /// `ExpiredTimeOut` — the time-out of `Cache::lock` expired
    /// (§8.1.6.3.4).
    ExpiredTimeOut {
        /// The message.
        message: String,
    },
    /// `ReadOnlyMode` — `write` on a CacheAccess that does not allow
    /// writing (IDL of §8.2.1.2.1, `CacheAccess::write`).
    ReadOnlyMode {
        /// The message.
        message: String,
    },
    /// `DDS::BadParameter` — a parameter the operation does not accept
    /// (implied IDL of §8.2.1.2.2, `FooHome::create_selection`).
    BadParameter {
        /// The message.
        message: String,
    },
}

impl DlrlException {
    /// The name of the exception as the specification spells it.
    #[must_use]
    pub const fn spec_name(&self) -> &'static str {
        match self {
            Self::DcpsError { .. } => "DCPSError",
            Self::BadHomeDefinition { .. } => "BadHomeDefinition",
            Self::NotFound { .. } => "NotFound",
            Self::AlreadyExisting { .. } => "AlreadyExisting",
            Self::AlreadyDeleted { .. } => "AlreadyDeleted",
            Self::PreconditionNotMet { .. } => "PreconditionNotMet",
            Self::NoSuchElement { .. } => "NoSuchElement",
            Self::SqlError { .. } => "SQLError",
            Self::ExpiredTimeOut { .. } => "ExpiredTimeOut",
            Self::ReadOnlyMode { .. } => "ReadOnlyMode",
            Self::BadParameter { .. } => "BadParameter",
        }
    }

    /// The repository id, `IDL:DDS/<name>:1.0` (§8.2.1.2.1 declares the
    /// exceptions in `module DDS`, without a `#pragma prefix`).
    #[must_use]
    pub fn repository_id(&self) -> String {
        format!("IDL:DDS/{}:1.0", self.spec_name())
    }

    /// The `message` attribute.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::DcpsError { message }
            | Self::BadHomeDefinition { message }
            | Self::NotFound { message }
            | Self::AlreadyExisting { message }
            | Self::AlreadyDeleted { message }
            | Self::PreconditionNotMet { message }
            | Self::NoSuchElement { message }
            | Self::SqlError { message }
            | Self::ExpiredTimeOut { message }
            | Self::ReadOnlyMode { message }
            | Self::BadParameter { message } => message,
        }
    }

    /// `PreconditionNotMet` with `message`.
    #[must_use]
    pub fn precondition(message: impl Into<String>) -> Self {
        Self::PreconditionNotMet {
            message: message.into(),
        }
    }

    /// `NotFound` with `message`.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    /// `DCPSError` with `message`.
    #[must_use]
    pub fn dcps(message: impl Into<String>) -> Self {
        Self::DcpsError {
            message: message.into(),
        }
    }

    /// `SQLError` with `message`.
    #[must_use]
    pub fn sql(message: impl Into<String>) -> Self {
        Self::SqlError {
            message: message.into(),
        }
    }

    /// `AlreadyDeleted` with `message`.
    #[must_use]
    pub fn already_deleted(message: impl Into<String>) -> Self {
        Self::AlreadyDeleted {
            message: message.into(),
        }
    }
}

impl core::fmt::Display for DlrlException {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.spec_name(), self.message())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DlrlException {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    fn all(message: &str) -> Vec<DlrlException> {
        let m = || message.to_string();
        alloc::vec![
            DlrlException::DcpsError { message: m() },
            DlrlException::BadHomeDefinition { message: m() },
            DlrlException::NotFound { message: m() },
            DlrlException::AlreadyExisting { message: m() },
            DlrlException::AlreadyDeleted { message: m() },
            DlrlException::PreconditionNotMet { message: m() },
            DlrlException::NoSuchElement { message: m() },
            DlrlException::SqlError { message: m() },
            DlrlException::ExpiredTimeOut { message: m() },
            DlrlException::ReadOnlyMode { message: m() },
            DlrlException::BadParameter { message: m() },
        ]
    }

    /// §8.1.6.2: every exception carries the attribute `message`, and
    /// its text is what `message()` and `Display` give back.
    #[test]
    fn dlrl_8_1_6_2_every_exception_carries_a_message() {
        for e in all("the reason") {
            assert_eq!(e.message(), "the reason");
            assert_eq!(
                e.to_string(),
                alloc::format!("{}: the reason", e.spec_name())
            );
        }
    }

    /// §8.1.6.2 and §8.2.1.2.1: the eight listed exceptions and the
    /// three raised outside the list, with the names of the IDL and
    /// repository ids in `module DDS`.
    #[test]
    fn dlrl_8_1_6_2_repository_ids_are_in_module_dds() {
        let ids: Vec<String> = all("x").iter().map(DlrlException::repository_id).collect();
        assert_eq!(
            ids,
            [
                "IDL:DDS/DCPSError:1.0",
                "IDL:DDS/BadHomeDefinition:1.0",
                "IDL:DDS/NotFound:1.0",
                "IDL:DDS/AlreadyExisting:1.0",
                "IDL:DDS/AlreadyDeleted:1.0",
                "IDL:DDS/PreconditionNotMet:1.0",
                "IDL:DDS/NoSuchElement:1.0",
                "IDL:DDS/SQLError:1.0",
                "IDL:DDS/ExpiredTimeOut:1.0",
                "IDL:DDS/ReadOnlyMode:1.0",
                "IDL:DDS/BadParameter:1.0",
            ]
        );
    }
}
