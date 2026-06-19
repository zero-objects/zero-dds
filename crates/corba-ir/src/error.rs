// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IR errors.

use alloc::string::String;

/// Result alias.
pub type IrResult<T> = Result<T, IrError>;

/// IR error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrError {
    /// `RepositoryId` is not in the format `IDL:<scoped>:<version>`.
    InvalidRepositoryId(String),
    /// `RepositoryId` already taken.
    DuplicateRepositoryId(String),
    /// Lookup name not in the repository.
    LookupFailed(String),
    /// `TypeCode` is corrupt from wire data.
    InvalidTypeCode(String),
}
