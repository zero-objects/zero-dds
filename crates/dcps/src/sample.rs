// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Sample<T>` — data + [`SampleInfo`] together.
//!
//! Spec reference: OMG DDS-DCPS 1.4 §2.2.2.5.3 `read`/`take`. The spec
//! works with two parallel sequences `data_values` and `sample_infos`;
//! in our Rust API we bundle them into a single `Sample<T> { data, info }`.
//!
//! ## valid_data
//!
//! When `info.valid_data == false`, the sample carries pure lifecycle
//! information (dispose/unregister marker). In that case `data` holds a
//! default-constructed `T` with only the key fields filled in
//! (spec §2.2.2.5.1.13).

extern crate alloc;

use crate::sample_info::SampleInfo;

/// A single sample record from `read`/`take`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample<T> {
    /// Application data. When `info.valid_data == false`, only the key
    /// portion is meaningful.
    pub data: T,
    /// Metadata (spec §2.2.2.5.1).
    pub info: SampleInfo,
}

impl<T> Sample<T> {
    /// Constructor.
    #[must_use]
    pub fn new(data: T, info: SampleInfo) -> Self {
        Self { data, info }
    }

    /// `true` if the sample carries payload (not just a lifecycle
    /// marker).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.info.valid_data
    }

    /// Convenience accessor: lifecycle view without data.
    #[must_use]
    pub fn info(&self) -> &SampleInfo {
        &self.info
    }
}
