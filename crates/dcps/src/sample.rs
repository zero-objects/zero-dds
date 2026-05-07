// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Sample<T>` — Daten + [`SampleInfo`] zusammen.
//!
//! Spec-Referenz: OMG DDS-DCPS 1.4 §2.2.2.5.3 `read`/`take`. Die Spec
//! arbeitet mit zwei parallelen Sequenzen `data_values` und
//! `sample_infos`; in unserer Rust-API buendeln wir sie zu einem
//! `Sample<T> { data, info }`.
//!
//! ## valid_data
//!
//! Wenn `info.valid_data == false`, traegt das Sample reine
//! Lifecycle-Information (Dispose-/Unregister-Marker). `data` enthaelt
//! in diesem Fall einen Default-konstruierten `T` mit nur den Key-
//! Feldern befuellt (Spec §2.2.2.5.1.13).

extern crate alloc;

use crate::sample_info::SampleInfo;

/// Einzelner Sample-Datensatz aus `read`/`take`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample<T> {
    /// Application-Daten. Bei `info.valid_data == false` ist nur der
    /// Key-Anteil bedeutungsvoll.
    pub data: T,
    /// Metadaten (Spec §2.2.2.5.1).
    pub info: SampleInfo,
}

impl<T> Sample<T> {
    /// Konstruktor.
    #[must_use]
    pub fn new(data: T, info: SampleInfo) -> Self {
        Self { data, info }
    }

    /// `true` wenn das Sample Nutzdaten enthaelt (nicht nur einen
    /// Lifecycle-Marker).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.info.valid_data
    }

    /// Komfort-Akzessor: Lifecycle-Sicht ohne Daten.
    #[must_use]
    pub fn info(&self) -> &SampleInfo {
        &self.info
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::instance_handle::InstanceHandle;
    use crate::sample_info::{InstanceStateKind, SampleStateKind, ViewStateKind};
    use crate::time::Time;

    #[test]
    fn sample_new_holds_data_and_info() {
        let info = SampleInfo::new_alive(
            InstanceHandle::from_raw(1),
            InstanceHandle::from_raw(2),
            Time::new(10, 0),
        );
        let s = Sample::new(42u32, info);
        assert_eq!(s.data, 42);
        assert_eq!(s.info().instance_handle, InstanceHandle::from_raw(1));
        assert!(s.is_valid());
    }

    #[test]
    fn sample_with_invalid_data_marker() {
        let info = SampleInfo {
            valid_data: false,
            instance_state: InstanceStateKind::NotAliveDisposed,
            ..SampleInfo::default()
        };
        let s = Sample::new(0u32, info);
        assert!(!s.is_valid());
        assert_eq!(s.info.instance_state, InstanceStateKind::NotAliveDisposed);
    }

    #[test]
    fn sample_states_in_info_default() {
        let s = Sample::new((), SampleInfo::default());
        assert_eq!(s.info.sample_state, SampleStateKind::NotRead);
        assert_eq!(s.info.view_state, ViewStateKind::New);
    }
}
