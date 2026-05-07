// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IR-Fehler.

use alloc::string::String;

/// Result-Alias.
pub type IrResult<T> = Result<T, IrError>;

/// IR-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrError {
    /// `RepositoryId` ist nicht im Format `IDL:<scoped>:<version>`.
    InvalidRepositoryId(String),
    /// `RepositoryId` schon vergeben.
    DuplicateRepositoryId(String),
    /// Lookup-Name nicht im Repository.
    LookupFailed(String),
    /// `TypeCode` ist von Wire-Daten korrupt.
    InvalidTypeCode(String),
}
