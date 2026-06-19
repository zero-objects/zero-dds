// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Entity lifecycle (DDS DCPS 1.4 §2.2.2.1) — common base for
//! `DomainParticipant`, `Publisher`, `Subscriber`, `Topic`,
//! `DataWriter`, `DataReader`.
//!
//! Spec behavior (§2.2.2.1.1 entity base):
//! 1. **Lifecycle:** `create_*` → `enable()` → operational → `delete_*`.
//!    Before `enable()` the entity is inert (no discovery, no wire
//!    activity); set_qos on all fields is allowed.
//! 2. **set_qos** after `enable()`: only fields with "Changeable=YES"
//!    may be changed — otherwise [`DdsError::ImmutablePolicy`]
//!    (§2.2.3 Tab. 2.13 column "Changeable").
//! 3. **enable()** is idempotent. If the parent entity (participant)
//!    has `entity_factory.autoenable_created_entities=TRUE`, children
//!    are automatically enabled on creation.
//! 4. **StatusCondition** is the hook for the `WaitSet` —
//!    `trigger_value()` returns true when a status whose bit is in the
//!    `enabled_statuses` mask is active.
//! 5. **InstanceHandle** is unique per entity (a local 64-bit counter,
//!    not on the wire — see [`crate::instance_handle`]).
//!
//! This module provides the low-level [`Entity`] trait + [`EntityState`]
//! as a building block. The implementations (Publisher, DataWriter,
//! ...) hold an `Arc<EntityState>` and delegate the trait methods.

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::error::{DdsError, Result};
use crate::instance_handle::{InstanceHandle, InstanceHandleAllocator};

/// Global allocator for entity InstanceHandles. One instance per
/// process — handles are unique within the process.
static ENTITY_HANDLE_ALLOCATOR: InstanceHandleAllocator = InstanceHandleAllocator::new();

/// `StatusMask` — 32-bit bitmask of the status kinds (DCPS §2.2.4.1).
/// Values from [`crate::psm_constants::status`].
pub type StatusMask = u32;

/// Atomic container for the entity lifecycle.
#[derive(Debug)]
pub struct EntityState {
    enabled: AtomicBool,
    /// `true` after a successful `delete_*()` — Spec §2.2.1.1.5
    /// (RC ALREADY_DELETED). Public ops MUST call
    /// [`Self::check_not_deleted`] before any effect.
    deleted: AtomicBool,
    instance_handle: InstanceHandle,
    /// Bitmask of the status bits changed **since the last
    /// `get_status_changes()` read**.
    status_changes: AtomicU32,
    /// Bitmask of the status bits covered by the listener (for the
    /// bubble-up logic).
    listener_mask: AtomicU32,
}

impl EntityState {
    /// New state, initially **disabled** (spec default for all
    /// entities except DomainParticipantFactory).
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(false),
            deleted: AtomicBool::new(false),
            instance_handle: ENTITY_HANDLE_ALLOCATOR.allocate(),
            status_changes: AtomicU32::new(0),
            listener_mask: AtomicU32::new(0),
        })
    }

    /// New state, **already enabled** — for DomainParticipantFactory
    /// (Spec §2.2.2.1.4: the factory is always enabled).
    #[must_use]
    pub fn new_enabled() -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(true),
            deleted: AtomicBool::new(false),
            instance_handle: ENTITY_HANDLE_ALLOCATOR.allocate(),
            status_changes: AtomicU32::new(0),
            listener_mask: AtomicU32::new(0),
        })
    }

    /// True if the entity is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Sets enabled=true (idempotent). Returns `true` if the call
    /// performed the false→true transition (for cascade logic).
    pub fn enable(&self) -> bool {
        !self.enabled.swap(true, Ordering::AcqRel)
    }

    /// Local 64-bit identifier of this entity.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.instance_handle
    }

    /// Current status-changes mask. Reading does NOT clear — the
    /// caller takes the relevant bits back itself via
    /// [`Self::clear_status_changes`].
    #[must_use]
    pub fn status_changes(&self) -> StatusMask {
        self.status_changes.load(Ordering::Acquire)
    }

    /// Sets additional status bits (called by the discovery/runtime
    /// layer when a status event arrives).
    pub fn set_status_bits(&self, bits: StatusMask) {
        self.status_changes.fetch_or(bits, Ordering::AcqRel);
    }

    /// Clears the given bits from the status-changes mask (after the
    /// caller's read).
    pub fn clear_status_changes(&self, bits: StatusMask) {
        self.status_changes.fetch_and(!bits, Ordering::AcqRel);
    }

    /// Set the listener mask — affects bubble-up.
    pub fn set_listener_mask(&self, mask: StatusMask) {
        self.listener_mask.store(mask, Ordering::Release);
    }

    /// Read the listener mask.
    #[must_use]
    pub fn listener_mask(&self) -> StatusMask {
        self.listener_mask.load(Ordering::Acquire)
    }

    /// `true` if the entity has already gone through `delete_*`.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted.load(Ordering::Acquire)
    }

    /// Marks the entity as deleted (idempotent). Returns `true` on the
    /// first call (false→true transition), `false` on subsequent
    /// calls.
    pub fn mark_deleted(&self) -> bool {
        !self.deleted.swap(true, Ordering::AcqRel)
    }

    /// Guard helper for public ops: returns `Err(AlreadyDeleted)` if
    /// the entity has already been deleted, otherwise `Ok(())`.
    /// Usage pattern:
    /// ```ignore
    /// pub fn write(&self, sample: T) -> Result<()> {
    ///     self.entity_state().check_not_deleted()?;
    ///     // ... the actual logic ...
    /// }
    /// ```
    ///
    /// # Errors
    /// `DdsError::AlreadyDeleted` if `is_deleted() == true`.
    pub fn check_not_deleted(&self) -> crate::error::Result<()> {
        if self.is_deleted() {
            Err(crate::error::DdsError::AlreadyDeleted)
        } else {
            Ok(())
        }
    }

    /// Guard helper: returns `Err(NotEnabled)` if the entity is not
    /// enabled (Spec §2.2.2.1.1.7 RC NOT_ENABLED).
    ///
    /// # Errors
    /// `DdsError::NotEnabled` if `is_enabled() == false`.
    pub fn check_enabled(&self) -> crate::error::Result<()> {
        if !self.is_enabled() {
            Err(crate::error::DdsError::NotEnabled)
        } else {
            Ok(())
        }
    }
}

/// `StatusCondition` — Spec §2.2.2.1.6, the primary WaitSet hook.
///
/// Minimal form: carries an `enabled_statuses` mask + delegates
/// `trigger_value()` to [`EntityState::status_changes`]. The object is
/// fully integrated (set_enabled_statuses, attach to WaitSet).
#[derive(Debug, Clone)]
pub struct StatusCondition {
    state: Arc<EntityState>,
    enabled_statuses: Arc<AtomicU32>,
}

impl StatusCondition {
    /// Constructor (internal; created by the entity).
    #[must_use]
    pub fn new(state: Arc<EntityState>) -> Self {
        Self {
            state,
            enabled_statuses: Arc::new(AtomicU32::new(crate::psm_constants::status::ANY)),
        }
    }

    /// Sets the `enabled_statuses` mask. Spec §2.2.2.1.6.
    pub fn set_enabled_statuses(&self, mask: StatusMask) {
        self.enabled_statuses.store(mask, Ordering::Release);
    }

    /// Returns the current `enabled_statuses` mask.
    #[must_use]
    pub fn enabled_statuses(&self) -> StatusMask {
        self.enabled_statuses.load(Ordering::Acquire)
    }

    /// True if (status_changes & enabled_statuses) != 0.
    /// Spec §2.2.2.1.6 trigger_value.
    #[must_use]
    pub fn trigger_value(&self) -> bool {
        let enabled = self.enabled_statuses.load(Ordering::Acquire);
        let changes = self.state.status_changes();
        (enabled & changes) != 0
    }

    /// Returns the `InstanceHandle` of the entity to which this
    /// StatusCondition is bound. Spec DCPS 1.4 §2.2.2.1.9
    /// `get_entity()` — the Rust API returns the handle instead of a
    /// `&dyn Entity` pointer, because the same `Arc<EntityState>` can
    /// be held by multiple entity wrappers (DataReader/DataWriter/...);
    /// the handle is the only identity that is stable beyond the
    /// wrapper granularity.
    #[must_use]
    pub fn get_entity_handle(&self) -> InstanceHandle {
        self.state.instance_handle()
    }

    /// Returns a shared reference to the underlying `EntityState`
    /// (Spec §2.2.2.1.9 — direct path). Lets caller code inspect the
    /// entity's status mask and lifecycle flags without going through
    /// the entity wrapper.
    #[must_use]
    pub fn entity_state(&self) -> &Arc<EntityState> {
        &self.state
    }
}

/// Entity trait — common lifecycle API of the 6 entity types
/// (DCPS §2.2.2.1).
///
/// Non-blocking, Send+Sync — all methods delegate to
/// `Arc<EntityState>`.
pub trait Entity {
    /// QoS type for this entity (e.g. `DomainParticipantQos`,
    /// `DataWriterQos`, ...).
    type Qos: Clone;

    /// Returns the current QoS (clone).
    /// Spec §2.2.2.1.2 `get_qos`.
    fn get_qos(&self) -> Self::Qos;

    /// Changes the QoS. Before enable: everything allowed. After
    /// enable: only fields with "Changeable=YES" — otherwise an
    /// `ImmutablePolicy` error. Spec §2.2.2.1.2 `set_qos`.
    ///
    /// # Errors
    /// * [`DdsError::ImmutablePolicy`] if an immutable field is to be
    ///   changed after `enable()`.
    /// * [`DdsError::InconsistentPolicy`] if the new QoS combination is
    ///   inconsistent.
    fn set_qos(&self, qos: Self::Qos) -> Result<()>;

    /// Enables the entity (idempotent). Spec §2.2.2.1.4 `enable`.
    ///
    /// # Errors
    /// [`DdsError::PreconditionNotMet`] if the parent entity is not
    /// enabled (per spec, children cannot be enabled before the parent
    /// — except the factory itself).
    fn enable(&self) -> Result<()>;

    /// True if the entity is already enabled.
    fn is_enabled(&self) -> bool {
        self.entity_state().is_enabled()
    }

    /// `StatusCondition` of this entity.
    /// Spec §2.2.2.1.6 `get_status_condition`.
    fn get_status_condition(&self) -> StatusCondition {
        StatusCondition::new(self.entity_state())
    }

    /// Bitmask of the status kinds changed since the last read.
    /// Spec §2.2.2.1.5 `get_status_changes`.
    fn get_status_changes(&self) -> StatusMask {
        self.entity_state().status_changes()
    }

    /// Local 64-bit identifier. Spec §2.2.2.1.7 `get_instance_handle`.
    fn get_instance_handle(&self) -> InstanceHandle {
        self.entity_state().instance_handle()
    }

    /// Internal accessor — each impl returns its `Arc<EntityState>`.
    fn entity_state(&self) -> Arc<EntityState>;
}

/// Helper function: validates that a QoS field `policy_name` was not
/// changed after enable. Used in `set_qos` impls:
///
/// ```ignore
/// if state.is_enabled() && new.durability != old.durability {
///     return Err(immutable_if_enabled("DURABILITY"));
/// }
/// ```
#[must_use]
pub fn immutable_if_enabled(policy_name: &'static str) -> DdsError {
    DdsError::ImmutablePolicy {
        policy: policy_name,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn entity_state_starts_disabled() {
        let s = EntityState::new();
        assert!(!s.is_enabled());
    }

    #[test]
    fn entity_state_factory_starts_enabled() {
        let s = EntityState::new_enabled();
        assert!(s.is_enabled());
    }

    #[test]
    fn enable_is_idempotent_and_reports_first_transition() {
        let s = EntityState::new();
        assert!(s.enable(), "first enable returns true");
        assert!(!s.enable(), "second enable returns false");
        assert!(s.is_enabled());
    }

    #[test]
    fn instance_handles_are_unique_per_entity() {
        let a = EntityState::new();
        let b = EntityState::new();
        assert_ne!(a.instance_handle(), b.instance_handle());
    }

    #[test]
    fn status_bits_or_in_and_clear() {
        let s = EntityState::new();
        s.set_status_bits(0b0011);
        s.set_status_bits(0b1100);
        assert_eq!(s.status_changes(), 0b1111);
        s.clear_status_changes(0b0101);
        assert_eq!(s.status_changes(), 0b1010);
    }

    #[test]
    fn status_condition_trigger_value() {
        let s = EntityState::new();
        let cond = StatusCondition::new(s.clone());
        cond.set_enabled_statuses(0b1010);

        // No status change → no trigger.
        assert!(!cond.trigger_value());

        // Status with a non-enabled bit → no trigger.
        s.set_status_bits(0b0001);
        assert!(!cond.trigger_value());

        // Status with an enabled bit → trigger.
        s.set_status_bits(0b0010);
        assert!(cond.trigger_value());
    }

    #[test]
    fn listener_mask_is_round_tripped() {
        let s = EntityState::new();
        s.set_listener_mask(0xABCD);
        assert_eq!(s.listener_mask(), 0xABCD);
    }

    #[test]
    fn immutable_if_enabled_returns_correct_error() {
        let e = immutable_if_enabled("DURABILITY");
        assert!(matches!(
            e,
            DdsError::ImmutablePolicy {
                policy: "DURABILITY"
            }
        ));
    }

    // ---- §2.2.1.1.5 ALREADY_DELETED ----

    #[test]
    fn check_not_deleted_passes_for_fresh_entity() {
        let s = EntityState::new();
        assert!(s.check_not_deleted().is_ok());
        assert!(!s.is_deleted());
    }

    #[test]
    fn check_not_deleted_returns_already_deleted_after_mark() {
        let s = EntityState::new();
        let first = s.mark_deleted();
        assert!(first, "first mark_deleted should return true");
        assert!(s.is_deleted());
        let res = s.check_not_deleted();
        assert!(matches!(res, Err(DdsError::AlreadyDeleted)));
    }

    #[test]
    fn mark_deleted_is_idempotent() {
        let s = EntityState::new();
        assert!(s.mark_deleted());
        // Second call returns false (already-deleted state).
        assert!(!s.mark_deleted());
        assert!(s.is_deleted());
    }

    // ---- §2.2.1.1.7 NOT_ENABLED ----

    #[test]
    fn check_enabled_returns_not_enabled_for_disabled_entity() {
        let s = EntityState::new();
        assert!(!s.is_enabled());
        let res = s.check_enabled();
        assert!(matches!(res, Err(DdsError::NotEnabled)));
    }

    #[test]
    fn check_enabled_passes_after_enable() {
        let s = EntityState::new();
        let _ = s.enable();
        assert!(s.check_enabled().is_ok());
    }

    #[test]
    fn check_enabled_passes_for_factory_entity() {
        // DomainParticipantFactory is always enabled (Spec §2.2.2.1.4).
        let s = EntityState::new_enabled();
        assert!(s.check_enabled().is_ok());
    }

    // ---- §2.2.2.1.9 StatusCondition.get_entity ----

    #[test]
    fn status_condition_get_entity_handle_matches_owner_state() {
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        // Handle of the condition == handle of the entity it is bound to.
        assert_eq!(cond.get_entity_handle(), state.instance_handle());
    }

    #[test]
    fn status_condition_get_entity_handle_unique_per_entity() {
        // Two different entities → two different handles via their
        // StatusConditions.
        let s1 = EntityState::new();
        let s2 = EntityState::new();
        let c1 = StatusCondition::new(s1);
        let c2 = StatusCondition::new(s2);
        assert_ne!(c1.get_entity_handle(), c2.get_entity_handle());
    }

    #[test]
    fn status_condition_entity_state_returns_same_arc() {
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        // Identity via Arc::ptr_eq — the condition holds exactly this
        // Arc, not a clone of the inner.
        assert!(Arc::ptr_eq(&state, cond.entity_state()));
    }

    #[test]
    fn status_condition_entity_state_reflects_lifecycle_changes() {
        // The get_entity path must make lifecycle changes visible
        // (e.g. enable, mark_deleted) so callers can inspect the state
        // directly.
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        assert!(!cond.entity_state().is_enabled());
        let _ = state.enable();
        assert!(cond.entity_state().is_enabled());
        let _ = state.mark_deleted();
        assert!(cond.entity_state().is_deleted());
    }
}
