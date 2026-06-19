// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SampleInfo` — per-sample metadata that `DataReader::read`/`take`
//! deliver alongside each sample.
//!
//! Spec reference: OMG DDS-DCPS 1.4 §2.2.2.5.1 `SampleInfo`. The spec
//! defines 11 fields that together describe the **statechart** of a
//! sample:
//!
//! 1. `sample_state`: whether the reader has already read the sample
//!    (`READ`) or not (`NOT_READ`).
//! 2. `view_state`: whether the reader instance is new (`NEW`) or
//!    already known (`NOT_NEW`).
//! 3. `instance_state`: lifecycle state of the instance (`ALIVE`,
//!    `NOT_ALIVE_DISPOSED`, `NOT_ALIVE_NO_WRITERS`).
//! 4. `disposed_generation_count`: number of `NOT_ALIVE_DISPOSED → ALIVE`
//!    transitions since the first sample of this instance.
//! 5. `no_writers_generation_count`: number of `NOT_ALIVE_NO_WRITERS → ALIVE`
//!    transitions since the first sample of this instance.
//! 6. `sample_rank`: number of samples in the same instance that follow
//!    this one in the cache (spec §2.2.2.5.1.5).
//! 7. `generation_rank`: difference in generation counts between this
//!    sample and the last sample of the instance in the same read set.
//! 8. `absolute_generation_rank`: like `generation_rank`, but relative
//!    to the **current** generation count.
//! 9. `source_timestamp`: wall-clock time of the write operation.
//! 10. `instance_handle`: local handle of the instance (key-based).
//! 11. `publication_handle`: local handle of the sending DataWriter.
//! 12. `valid_data`: `false` for dispose/unregister markers without
//!     payload (spec §2.2.2.5.1.13).
//!
//! Note: the spec lists `valid_data` as the 12th field; some texts do
//! not count it, so you will find both "11" and "12" fields in the
//! documentation. We carry all of them.

extern crate alloc;

use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
use crate::time::Time;

/// `SampleStateKind` (DDS 1.4 §2.2.2.5.1.1) — maintained per reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SampleStateKind {
    /// Sample has never been delivered to the application via
    /// `read`/`take`.
    NotRead,
    /// Sample has already been delivered at least once via `read`
    /// (after `take` it is gone, so this only matters for `read`).
    Read,
}

impl SampleStateKind {
    /// `true` if this is an as-yet-unread sample.
    #[must_use]
    pub const fn is_not_read(&self) -> bool {
        matches!(self, Self::NotRead)
    }
}

impl Default for SampleStateKind {
    fn default() -> Self {
        Self::NotRead
    }
}

/// `ViewStateKind` (DDS 1.4 §2.2.2.5.1.2) — maintained per instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViewStateKind {
    /// First sample of this instance (or first after
    /// `NOT_ALIVE_NO_WRITERS → ALIVE`).
    New,
    /// Reader has already delivered samples for this instance.
    NotNew,
}

impl Default for ViewStateKind {
    fn default() -> Self {
        Self::New
    }
}

/// `InstanceStateKind` (DDS 1.4 §2.2.2.5.1.3) — maintained per instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceStateKind {
    /// At least one DataWriter has registered the instance and it has
    /// not been disposed.
    Alive,
    /// At least one DataWriter has disposed the instance.
    NotAliveDisposed,
    /// All DataWriters that had registered the instance have called
    /// `unregister_instance` or are gone.
    NotAliveNoWriters,
}

impl InstanceStateKind {
    /// `true` if the instance is still alive.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        matches!(self, Self::Alive)
    }
    /// `true` if the instance is either disposed or no-writers.
    #[must_use]
    pub const fn is_not_alive(&self) -> bool {
        !self.is_alive()
    }
}

impl Default for InstanceStateKind {
    fn default() -> Self {
        Self::Alive
    }
}

/// Bitmask for sample-state filtering in `read`/`take` calls
/// (DDS 1.4 §2.2.2.5.1.4 `SampleStateMask`).
pub mod sample_state_mask {
    /// `NOT_READ` only.
    pub const NOT_READ: u32 = 1 << 0;
    /// `READ` only.
    pub const READ: u32 = 1 << 1;
    /// `READ | NOT_READ` — both.
    pub const ANY: u32 = NOT_READ | READ;
}

/// Bitmask for view-state filtering (§2.2.2.5.1.4 `ViewStateMask`).
pub mod view_state_mask {
    /// `NEW`.
    pub const NEW: u32 = 1 << 0;
    /// `NOT_NEW`.
    pub const NOT_NEW: u32 = 1 << 1;
    /// Both.
    pub const ANY: u32 = NEW | NOT_NEW;
}

/// Bitmask for instance-state filtering (§2.2.2.5.1.4
/// `InstanceStateMask`).
pub mod instance_state_mask {
    /// `ALIVE`.
    pub const ALIVE: u32 = 1 << 0;
    /// `NOT_ALIVE_DISPOSED`.
    pub const NOT_ALIVE_DISPOSED: u32 = 1 << 1;
    /// `NOT_ALIVE_NO_WRITERS`.
    pub const NOT_ALIVE_NO_WRITERS: u32 = 1 << 2;
    /// `NOT_ALIVE_DISPOSED | NOT_ALIVE_NO_WRITERS`.
    pub const NOT_ALIVE: u32 = NOT_ALIVE_DISPOSED | NOT_ALIVE_NO_WRITERS;
    /// All three.
    pub const ANY: u32 = ALIVE | NOT_ALIVE;
}

/// `SampleInfo` (DDS 1.4 §2.2.2.5.1) — per-sample metadata.
///
/// Filled by the DataReader in the `take`/`read` path. Application
/// code is meant to rely on these fields to:
/// * distinguish new samples from re-read ones (`sample_state`),
/// * detect new instances (`view_state == NEW`),
/// * observe lifecycle events (`instance_state`, `valid_data == false`),
/// * follow reordering / generation changes (`*_generation_count`,
///   `*_rank`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleInfo {
    /// Sample-State (`READ` / `NOT_READ`).
    pub sample_state: SampleStateKind,
    /// View-State (`NEW` / `NOT_NEW`).
    pub view_state: ViewStateKind,
    /// Instance-State (`ALIVE` / `NOT_ALIVE_*`).
    pub instance_state: InstanceStateKind,
    /// How many times the instance has gone through `NOT_ALIVE_DISPOSED → ALIVE`
    /// since its first sample.
    pub disposed_generation_count: i32,
    /// How many times the instance has gone through `NOT_ALIVE_NO_WRITERS → ALIVE`
    /// since its first sample.
    pub no_writers_generation_count: i32,
    /// Number of samples in the same instance that follow this one in
    /// the cache (§2.2.2.5.1.5).
    pub sample_rank: i32,
    /// Difference in `generation_count` between this sample and the last
    /// sample of the instance in the same read set.
    pub generation_rank: i32,
    /// Difference in `generation_count` between this sample and the
    /// **current** generation count.
    pub absolute_generation_rank: i32,
    /// Wall-clock time of the `write`/`dispose`/`unregister`.
    pub source_timestamp: Time,
    /// Local handle of the instance.
    pub instance_handle: InstanceHandle,
    /// Local handle of the sending DataWriter (`HANDLE_NIL` in the
    /// offline / unknown case).
    pub publication_handle: InstanceHandle,
    /// `true` if the sample carries payload; `false` for a plain
    /// dispose/unregister marker (§2.2.2.5.1.13).
    pub valid_data: bool,
}

impl Default for SampleInfo {
    fn default() -> Self {
        Self {
            sample_state: SampleStateKind::NotRead,
            view_state: ViewStateKind::New,
            instance_state: InstanceStateKind::Alive,
            disposed_generation_count: 0,
            no_writers_generation_count: 0,
            sample_rank: 0,
            generation_rank: 0,
            absolute_generation_rank: 0,
            source_timestamp: Time::default(),
            instance_handle: HANDLE_NIL,
            publication_handle: HANDLE_NIL,
            valid_data: true,
        }
    }
}

impl SampleInfo {
    /// Constructs a default info block for a fresh sample of a **new**
    /// instance with `valid_data = true`.
    #[must_use]
    pub fn new_alive(
        instance: InstanceHandle,
        publication: InstanceHandle,
        timestamp: Time,
    ) -> Self {
        Self {
            sample_state: SampleStateKind::NotRead,
            view_state: ViewStateKind::New,
            instance_state: InstanceStateKind::Alive,
            instance_handle: instance,
            publication_handle: publication,
            source_timestamp: timestamp,
            valid_data: true,
            ..Self::default()
        }
    }

    /// Checks whether this `SampleInfo` is accepted by the given state
    /// masks (spec §2.2.2.5.3.1, `read_w_condition`).
    #[must_use]
    pub fn matches_states(&self, sample_mask: u32, view_mask: u32, instance_mask: u32) -> bool {
        let sample_bit = match self.sample_state {
            SampleStateKind::NotRead => sample_state_mask::NOT_READ,
            SampleStateKind::Read => sample_state_mask::READ,
        };
        let view_bit = match self.view_state {
            ViewStateKind::New => view_state_mask::NEW,
            ViewStateKind::NotNew => view_state_mask::NOT_NEW,
        };
        let inst_bit = match self.instance_state {
            InstanceStateKind::Alive => instance_state_mask::ALIVE,
            InstanceStateKind::NotAliveDisposed => instance_state_mask::NOT_ALIVE_DISPOSED,
            InstanceStateKind::NotAliveNoWriters => instance_state_mask::NOT_ALIVE_NO_WRITERS,
        };
        (sample_mask & sample_bit) != 0
            && (view_mask & view_bit) != 0
            && (instance_mask & inst_bit) != 0
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_alive_new_not_read() {
        let info = SampleInfo::default();
        assert_eq!(info.sample_state, SampleStateKind::NotRead);
        assert_eq!(info.view_state, ViewStateKind::New);
        assert_eq!(info.instance_state, InstanceStateKind::Alive);
        assert!(info.valid_data);
        assert_eq!(info.disposed_generation_count, 0);
        assert_eq!(info.no_writers_generation_count, 0);
        assert_eq!(info.instance_handle, HANDLE_NIL);
        assert_eq!(info.publication_handle, HANDLE_NIL);
    }

    #[test]
    fn instance_state_predicates() {
        assert!(InstanceStateKind::Alive.is_alive());
        assert!(!InstanceStateKind::Alive.is_not_alive());
        assert!(InstanceStateKind::NotAliveDisposed.is_not_alive());
        assert!(InstanceStateKind::NotAliveNoWriters.is_not_alive());
    }

    #[test]
    fn sample_state_predicate() {
        assert!(SampleStateKind::NotRead.is_not_read());
        assert!(!SampleStateKind::Read.is_not_read());
    }

    #[test]
    fn matches_states_filter() {
        let info = SampleInfo::default();
        assert!(info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
        assert!(info.matches_states(
            sample_state_mask::NOT_READ,
            view_state_mask::NEW,
            instance_state_mask::ALIVE,
        ));
        assert!(!info.matches_states(
            sample_state_mask::READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
        assert!(!info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::NOT_NEW,
            instance_state_mask::ANY,
        ));
        assert!(!info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::NOT_ALIVE,
        ));
    }

    #[test]
    fn new_alive_constructor_sets_handles_and_timestamp() {
        let h = InstanceHandle::from_raw(7);
        let pub_h = InstanceHandle::from_raw(42);
        let ts = Time::new(1, 2);
        let info = SampleInfo::new_alive(h, pub_h, ts);
        assert_eq!(info.instance_handle, h);
        assert_eq!(info.publication_handle, pub_h);
        assert_eq!(info.source_timestamp, ts);
        assert!(info.valid_data);
        assert_eq!(info.instance_state, InstanceStateKind::Alive);
    }

    // ---- §2.2.2.5.5 all 12 SampleInfo fields available ----

    #[test]
    fn sample_info_all_spec_fields_accessible() {
        // Spec §2.2.2.5.5 lists 12 fields; all must be readable and
        // settable. The test verifies field identity via concrete
        // values.
        let info = SampleInfo {
            sample_state: SampleStateKind::Read,
            view_state: ViewStateKind::NotNew,
            instance_state: InstanceStateKind::NotAliveDisposed,
            disposed_generation_count: 3,
            no_writers_generation_count: 5,
            sample_rank: 7,
            generation_rank: 9,
            absolute_generation_rank: 11,
            source_timestamp: Time::new(1, 2),
            instance_handle: InstanceHandle::from_raw(0xCAFE),
            publication_handle: InstanceHandle::from_raw(0xBEEF),
            valid_data: false,
        };

        assert_eq!(info.sample_state, SampleStateKind::Read);
        assert_eq!(info.view_state, ViewStateKind::NotNew);
        assert_eq!(info.instance_state, InstanceStateKind::NotAliveDisposed);
        assert_eq!(info.disposed_generation_count, 3);
        assert_eq!(info.no_writers_generation_count, 5);
        assert_eq!(info.sample_rank, 7);
        assert_eq!(info.generation_rank, 9);
        assert_eq!(info.absolute_generation_rank, 11);
        assert_eq!(info.source_timestamp, Time::new(1, 2));
        assert_eq!(info.instance_handle, InstanceHandle::from_raw(0xCAFE));
        assert_eq!(info.publication_handle, InstanceHandle::from_raw(0xBEEF));
        assert!(!info.valid_data);
    }

    #[test]
    fn sample_info_dispose_marker_has_invalid_data() {
        // §2.2.2.5.1.13 — dispose/unregister marker has valid_data=false.
        // Negative: a sample with valid_data=false has no payload; the
        // caller MUST NOT evaluate sample.data.
        let info = SampleInfo {
            valid_data: false,
            instance_state: InstanceStateKind::NotAliveDisposed,
            ..SampleInfo::default()
        };
        assert!(!info.valid_data);
        assert!(info.instance_state.is_not_alive());
    }

    #[test]
    fn sample_info_three_state_dimensions_independent() {
        // Spec §2.2.2.5.1 — three orthogonal state dimensions.
        // matches_states must filter on each one individually.
        let info = SampleInfo {
            sample_state: SampleStateKind::Read,
            view_state: ViewStateKind::NotNew,
            instance_state: InstanceStateKind::Alive,
            ..SampleInfo::default()
        };
        // Read+NotNew+Alive matched.
        assert!(info.matches_states(
            sample_state_mask::READ,
            view_state_mask::NOT_NEW,
            instance_state_mask::ALIVE,
        ));
        // sample_state mismatch → reject.
        assert!(!info.matches_states(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
    }

    #[test]
    fn sample_info_generation_rank_starts_zero() {
        // Default sample: all generation counters 0.
        let info = SampleInfo::default();
        assert_eq!(info.disposed_generation_count, 0);
        assert_eq!(info.no_writers_generation_count, 0);
        assert_eq!(info.sample_rank, 0);
        assert_eq!(info.generation_rank, 0);
        assert_eq!(info.absolute_generation_rank, 0);
    }

    #[test]
    fn enum_default_impls() {
        assert_eq!(SampleStateKind::default(), SampleStateKind::NotRead);
        assert_eq!(ViewStateKind::default(), ViewStateKind::New);
        assert_eq!(InstanceStateKind::default(), InstanceStateKind::Alive);
    }
}
