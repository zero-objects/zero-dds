// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error type for the durability storage layer.

use alloc::string::String;
use core::fmt;

/// Result alias for the durability store.
pub type Result<T> = core::result::Result<T, StoreError>;

/// Errors surfaced by a [`crate::DurabilityStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A QoS contract cap was reached (`max_samples`, `max_instances`,
    /// `max_samples_per_instance` under `KEEP_ALL`). The caller maps this to a
    /// `RESOURCE_LIMITS_EXCEEDED` event. Carries a static reason.
    OutOfResources(&'static str),
    /// Backend/adapter failure (I/O, sqlite, serialization). Carries an
    /// owned message because adapter errors are dynamic.
    Backend(String),
    /// An internal lock was poisoned (a thread panicked while holding it).
    Poisoned(&'static str),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfResources(w) => write!(f, "durability store: out of resources: {w}"),
            Self::Backend(m) => write!(f, "durability store: backend error: {m}"),
            Self::Poisoned(w) => write!(f, "durability store: lock poisoned: {w}"),
        }
    }
}

impl core::error::Error for StoreError {}
