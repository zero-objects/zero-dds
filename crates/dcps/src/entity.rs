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

/// Work an entity does once, at its first `enable`, before it counts as
/// enabled: a DataWriter's or DataReader's registration with the runtime,
/// which announces it. A failure leaves the entity disabled and the work in
/// place for the next `enable`.
#[cfg(feature = "std")]
pub(crate) struct EnableWork(Box<dyn FnMut() -> Result<()> + Send>);

#[cfg(feature = "std")]
impl core::fmt::Debug for EnableWork {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("EnableWork")
    }
}

/// The statuses an entity tells by looking rather than by an event: a
/// DataReader's DATA_AVAILABLE while it holds data not read yet, a
/// Subscriber's DATA_ON_READERS while one of its readers does (DDS 1.4
/// §2.2.4.1). Read with every [`EntityState::status_changes`].
#[cfg(feature = "std")]
pub(crate) struct StatusProbe(Arc<dyn Fn() -> StatusMask + Send + Sync>);

#[cfg(feature = "std")]
impl core::fmt::Debug for StatusProbe {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StatusProbe")
    }
}

/// Atomic container for the entity lifecycle.
#[derive(Debug)]
pub struct EntityState {
    enabled: AtomicBool,
    /// The entity that created this one (the participant of a publisher,
    /// the publisher of a writer, ...); `enable` is refused while it is
    /// disabled (DDS 1.4 §2.2.2.1.1.7).
    #[cfg(feature = "std")]
    factory: std::sync::Mutex<Option<alloc::sync::Weak<EntityState>>>,
    /// The entities this one created, which its `enable` reaches when
    /// `autoenable_children` is set.
    #[cfg(feature = "std")]
    children: std::sync::Mutex<alloc::vec::Vec<alloc::sync::Weak<EntityState>>>,
    /// ENTITY_FACTORY.autoenable_created_entities of this entity as a
    /// factory (DDS 1.4 §2.2.3.20).
    autoenable_children: AtomicBool,
    /// See [`EnableWork`].
    #[cfg(feature = "std")]
    enable_work: std::sync::Mutex<Option<EnableWork>>,
    /// `true` after a successful `delete_*()` — Spec §2.2.1.1.5
    /// (RC ALREADY_DELETED). Public ops MUST call
    /// [`Self::check_not_deleted`] before any effect.
    deleted: AtomicBool,
    instance_handle: InstanceHandle,
    /// Bitmask of the status bits changed **since the last
    /// `get_status_changes()` read**.
    status_changes: AtomicU32,
    /// See [`StatusProbe`].
    #[cfg(feature = "std")]
    status_probe: std::sync::Mutex<Option<StatusProbe>>,
    /// Bitmask of the status bits covered by the listener (for the
    /// bubble-up logic).
    listener_mask: AtomicU32,
    /// Status bits whose listener look is running; see
    /// [`crate::listener_dispatch::poll_ledger`].
    listener_looks_running: AtomicU32,
    /// Status bits a caller asked a look for that has not run yet.
    listener_looks_requested: AtomicU32,
    /// The entity's one StatusCondition (DDS 1.4 §2.2.2.1.9: "A
    /// StatusCondition object is a specific Condition that is associated
    /// with each Entity"). It holds this state weakly.
    #[cfg(feature = "std")]
    status_condition: std::sync::OnceLock<Arc<StatusCondition>>,
    /// How to get the entity back from its state; see [`EntityRef`].
    #[cfg(feature = "std")]
    entity_ref: std::sync::OnceLock<EntityRefMaker>,
}

/// What gives the entity a state belongs to, while it exists.
#[cfg(feature = "std")]
struct EntityRefMaker(Box<dyn Fn() -> Option<EntityRef> + Send + Sync>);

#[cfg(feature = "std")]
impl core::fmt::Debug for EntityRefMaker {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("EntityRefMaker")
    }
}

/// The entities of the process by their handle, held weakly: how a listener
/// call finds the entity the status is of (DDS 1.4 §2.3.3: the listener
/// operations take the entity).
#[cfg(feature = "std")]
static ENTITY_REGISTRY: std::sync::Mutex<
    alloc::collections::BTreeMap<InstanceHandle, alloc::sync::Weak<EntityState>>,
> = std::sync::Mutex::new(alloc::collections::BTreeMap::new());

#[cfg(feature = "std")]
impl Drop for EntityState {
    fn drop(&mut self) {
        if let Ok(mut registry) = ENTITY_REGISTRY.lock() {
            registry.remove(&self.instance_handle);
        }
    }
}

/// The entity with `handle`, while it exists and has said how to be reached.
#[cfg(feature = "std")]
pub(crate) fn resolve_entity(handle: InstanceHandle) -> Option<EntityRef> {
    let state = ENTITY_REGISTRY
        .lock()
        .ok()?
        .get(&handle)
        .and_then(alloc::sync::Weak::upgrade)?;
    state.entity_ref()
}

/// An Entity whatever its kind: what `StatusCondition::get_entity` returns
/// (DDS 1.4 §2.2.2.1.9.3). A DataWriter, a DataReader and a Topic are
/// returned type-erased; `typed` on the first two gives the handle of their
/// type.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum EntityRef {
    /// A DomainParticipant.
    DomainParticipant(crate::participant::DomainParticipant),
    /// A Publisher.
    Publisher(crate::publisher::Publisher),
    /// A Subscriber.
    Subscriber(crate::subscriber::Subscriber),
    /// A Topic, as an untyped description.
    Topic(crate::topic::TopicDescriptionHandle),
    /// A DataWriter.
    DataWriter(crate::publisher::AnyDataWriter),
    /// A DataReader.
    DataReader(crate::subscriber::AnyDataReader),
}

impl EntityState {
    /// New state, initially **disabled** (spec default for all
    /// entities except DomainParticipantFactory).
    #[must_use]
    pub fn new() -> Arc<Self> {
        Self::with_enabled(false)
    }

    /// New state, **already enabled** — for DomainParticipantFactory
    /// (Spec §2.2.2.1.4: the factory is always enabled).
    #[must_use]
    pub fn new_enabled() -> Arc<Self> {
        Self::with_enabled(true)
    }

    fn with_enabled(enabled: bool) -> Arc<Self> {
        let state = Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            #[cfg(feature = "std")]
            factory: std::sync::Mutex::new(None),
            #[cfg(feature = "std")]
            children: std::sync::Mutex::new(alloc::vec::Vec::new()),
            autoenable_children: AtomicBool::new(true),
            #[cfg(feature = "std")]
            enable_work: std::sync::Mutex::new(None),
            deleted: AtomicBool::new(false),
            instance_handle: ENTITY_HANDLE_ALLOCATOR.allocate(),
            status_changes: AtomicU32::new(0),
            #[cfg(feature = "std")]
            status_probe: std::sync::Mutex::new(None),
            listener_mask: AtomicU32::new(0),
            listener_looks_running: AtomicU32::new(0),
            listener_looks_requested: AtomicU32::new(0),
            #[cfg(feature = "std")]
            status_condition: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            entity_ref: std::sync::OnceLock::new(),
        });
        #[cfg(feature = "std")]
        if let Ok(mut registry) = ENTITY_REGISTRY.lock() {
            registry.insert(state.instance_handle, Arc::downgrade(&state));
        }
        state
    }

    /// True if the entity is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Sets enabled=true (idempotent). Returns `true` if the call
    /// performed the false→true transition (for cascade logic).
    ///
    /// The flag alone: no factory check, no `EnableWork`, no cascade.
    /// An entity's `enable` is [`Self::enable_entity`].
    pub fn enable(&self) -> bool {
        !self.enabled.swap(true, Ordering::AcqRel)
    }

    /// Records `factory` as the entity that created this one: this
    /// entity's [`Self::enable_entity`] is refused while `factory` is
    /// disabled, and `factory`'s reaches this one.
    #[cfg(feature = "std")]
    pub(crate) fn set_factory(self: &Arc<Self>, factory: &Arc<EntityState>) {
        if let Ok(mut f) = self.factory.lock() {
            *f = Some(Arc::downgrade(factory));
        }
        if let Ok(mut children) = factory.children.lock() {
            children.retain(|c| c.strong_count() > 0);
            children.push(Arc::downgrade(self));
        }
    }

    /// ENTITY_FACTORY.autoenable_created_entities of this entity as a
    /// factory: whether [`Self::enable_entity`] enables what it created.
    pub(crate) fn set_autoenable_children(&self, on: bool) {
        self.autoenable_children.store(on, Ordering::Release);
    }

    /// Sets the [`EnableWork`] the first [`Self::enable_entity`] runs.
    #[cfg(feature = "std")]
    pub(crate) fn set_enable_work(&self, work: impl FnMut() -> Result<()> + Send + 'static) {
        if let Ok(mut w) = self.enable_work.lock() {
            *w = Some(EnableWork(Box::new(work)));
        }
    }

    /// DDS 1.4 §2.2.2.1.1.7 `enable`. Refused with `PreconditionNotMet`
    /// while the entity's factory is disabled ("Calling enable on an
    /// Entity whose factory is not enabled will fail"). The first call
    /// runs the entity's `EnableWork` and only then marks it enabled;
    /// a failing work leaves it disabled. When the entity's ENTITY_FACTORY
    /// says so, every entity it created is enabled after it ("the enable
    /// operation on the factory will automatically enable all entities
    /// created from the factory"). Idempotent.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a disabled factory; the work's error; the
    /// first error of an entity the cascade enables, after the cascade
    /// reached all of them.
    #[cfg(feature = "std")]
    pub fn enable_entity(&self) -> Result<()> {
        let factory = self
            .factory
            .lock()
            .ok()
            .and_then(|f| f.as_ref().and_then(alloc::sync::Weak::upgrade));
        if factory.is_some_and(|f| !f.is_enabled()) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the factory of this entity is not enabled",
            });
        }
        {
            let mut work = self
                .enable_work
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "entity enable work poisoned",
                })?;
            if self.is_enabled() {
                return Ok(());
            }
            if let Some(w) = work.as_mut() {
                (w.0)()?;
            }
            *work = None;
            self.enabled.store(true, Ordering::Release);
        }
        if !self.autoenable_children.load(Ordering::Acquire) {
            return Ok(());
        }
        let children: alloc::vec::Vec<Arc<EntityState>> = self
            .children
            .lock()
            .map(|c| c.iter().filter_map(alloc::sync::Weak::upgrade).collect())
            .unwrap_or_default();
        let mut first_error = None;
        for child in children {
            if let Err(error) = child.enable_entity() {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Local 64-bit identifier of this entity.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.instance_handle
    }

    /// Current status-changes mask: the flags the status events set, and
    /// what the `StatusProbe` sees. Reading does NOT clear — the caller
    /// takes the relevant bits back itself via
    /// [`Self::clear_status_changes`].
    #[must_use]
    pub fn status_changes(&self) -> StatusMask {
        // DDS 1.4 §2.2.2.1.1.6: "if the entity is not enabled, all
        // communication statuses are in the 'untriggered' state".
        if !self.is_enabled() {
            return 0;
        }
        let set = self.status_changes.load(Ordering::Acquire);
        #[cfg(feature = "std")]
        {
            let probe = self
                .status_probe
                .lock()
                .ok()
                .and_then(|p| p.as_ref().map(|p| Arc::clone(&p.0)));
            if let Some(probe) = probe {
                return set | probe();
            }
        }
        set
    }

    /// Installs `probe` (see [`StatusProbe`]).
    #[cfg(feature = "std")]
    pub(crate) fn set_status_probe(&self, probe: Arc<dyn Fn() -> StatusMask + Send + Sync>) {
        if let Ok(mut slot) = self.status_probe.lock() {
            *slot = Some(StatusProbe(probe));
        }
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

    /// Asks for a listener look at the status `bit`.
    pub(crate) fn request_listener_look(&self, bit: StatusMask) {
        self.listener_looks_requested
            .fetch_or(bit, Ordering::AcqRel);
    }

    /// Whether a look at `bit` was asked for and has not run.
    pub(crate) fn listener_look_requested(&self, bit: StatusMask) -> bool {
        self.listener_looks_requested.load(Ordering::Acquire) & bit != 0
    }

    /// Takes the request for a look at `bit`; `true` if there was one.
    pub(crate) fn take_listener_look_request(&self, bit: StatusMask) -> bool {
        self.listener_looks_requested
            .fetch_and(!bit, Ordering::AcqRel)
            & bit
            != 0
    }

    /// Starts the look at `bit`; `false` while another one runs.
    pub(crate) fn begin_listener_look(&self, bit: StatusMask) -> bool {
        self.listener_looks_running.fetch_or(bit, Ordering::AcqRel) & bit == 0
    }

    /// Ends the look at `bit`.
    pub(crate) fn end_listener_look(&self, bit: StatusMask) {
        self.listener_looks_running
            .fetch_and(!bit, Ordering::AcqRel);
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
    /// enabled (Spec §2.2.2.1.1.7 RC NOT_ENABLED), and `Err(AlreadyDeleted)`
    /// first if it is deleted: an operation on a deleted entity returns
    /// ALREADY_DELETED (§2.2.1.1), whether or not it was enabled.
    ///
    /// # Errors
    /// `DdsError::AlreadyDeleted` if `is_deleted()`, else
    /// `DdsError::NotEnabled` if `is_enabled() == false`.
    pub fn check_enabled(&self) -> crate::error::Result<()> {
        self.check_not_deleted()?;
        if !self.is_enabled() {
            Err(crate::error::DdsError::NotEnabled)
        } else {
            Ok(())
        }
    }
}

/// `StatusCondition` — DDS 1.4 §2.2.2.1.9, the primary WaitSet hook.
///
/// Each Entity has one: [`Entity::get_status_condition`] returns the same
/// object on every call, so the `enabled_statuses` set on it (§2.2.2.1.9.1)
/// is the entity's, and a WaitSet it is attached to detaches it again by the
/// same `Arc`. It holds the entity's state weakly; its trigger value is
/// [`EntityState::status_changes`] masked by the enabled statuses.
#[derive(Debug)]
pub struct StatusCondition {
    state: alloc::sync::Weak<EntityState>,
    handle: InstanceHandle,
    enabled_statuses: AtomicU32,
}

impl StatusCondition {
    /// Constructor (internal; the entity makes its one StatusCondition).
    #[must_use]
    pub fn new(state: Arc<EntityState>) -> Self {
        Self {
            handle: state.instance_handle(),
            state: Arc::downgrade(&state),
            enabled_statuses: AtomicU32::new(crate::psm_constants::status::ANY),
        }
    }

    /// Sets the `enabled_statuses` mask. DDS 1.4 §2.2.2.1.9.1.
    pub fn set_enabled_statuses(&self, mask: StatusMask) {
        self.enabled_statuses.store(mask, Ordering::Release);
    }

    /// `get_enabled_statuses` of DDS 1.4 Entity::get_enabled_statuses (the spelling of the specification).
    #[must_use]
    pub fn get_enabled_statuses(&self) -> StatusMask {
        self.enabled_statuses()
    }

    /// Returns the current `enabled_statuses` mask.
    #[must_use]
    pub fn enabled_statuses(&self) -> StatusMask {
        self.enabled_statuses.load(Ordering::Acquire)
    }

    /// True if (status_changes & enabled_statuses) != 0.
    /// DDS 1.4 §2.2.2.1.9 trigger_value. False once the entity is gone.
    #[must_use]
    pub fn trigger_value(&self) -> bool {
        let enabled = self.enabled_statuses.load(Ordering::Acquire);
        self.state
            .upgrade()
            .is_some_and(|state| (enabled & state.status_changes()) != 0)
    }

    /// The `InstanceHandle` of the entity this StatusCondition is bound to;
    /// it outlives the entity. [`Self::get_entity`] is the Entity itself.
    #[must_use]
    pub fn get_entity_handle(&self) -> InstanceHandle {
        self.handle
    }

    /// DDS 1.4 §2.2.2.1.9.3 `get_entity`: the Entity this StatusCondition
    /// is associated with. `None` once the entity's last handle is gone, and
    /// for an entity the library creates and does not hand out (the
    /// built-in subscriber's readers).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_entity(&self) -> Option<EntityRef> {
        self.state.upgrade()?.entity_ref()
    }

    /// The state of the entity, while it exists (DDS 1.4 §2.2.2.1.9 — direct
    /// path): lets caller code inspect the entity's status mask and
    /// lifecycle flags without going through the entity wrapper.
    #[must_use]
    pub fn entity_state(&self) -> Option<Arc<EntityState>> {
        self.state.upgrade()
    }
}

#[cfg(feature = "std")]
impl EntityState {
    /// The entity's one StatusCondition, made on the first call.
    pub fn status_condition(this: &Arc<Self>) -> Arc<StatusCondition> {
        Arc::clone(
            this.status_condition
                .get_or_init(|| Arc::new(StatusCondition::new(Arc::clone(this)))),
        )
    }

    /// Records how to get the entity back from its state (once); `make`
    /// holds the entity's inner part weakly.
    pub(crate) fn set_entity_ref(
        &self,
        make: impl Fn() -> Option<EntityRef> + Send + Sync + 'static,
    ) {
        let _ = self.entity_ref.set(EntityRefMaker(Box::new(make)));
    }

    /// The entity this is the state of, while it exists.
    pub(crate) fn entity_ref(&self) -> Option<EntityRef> {
        (self.entity_ref.get()?.0)()
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
    /// [`QosArg::Default`](crate::qos::QosArg::Default) (`*_QOS_DEFAULT`)
    /// is "the current default QoS set in the Entity's factory" (DDS 1.4
    /// §2.2.2.1.1.1), under the same rules as that QoS given as a value.
    ///
    /// # Errors
    /// * [`DdsError::ImmutablePolicy`] if an immutable field is to be
    ///   changed after `enable()`.
    /// * [`DdsError::InconsistentPolicy`] if the new QoS combination is
    ///   inconsistent.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()>;

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

    /// The entity's `StatusCondition`: the same object on every call (DDS
    /// 1.4 §2.2.2.1.9, §2.2.2.1.1.8 `get_statuscondition`).
    fn get_status_condition(&self) -> Arc<StatusCondition> {
        EntityState::status_condition(&self.entity_state())
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
        // §2.2.2.1.1.7: a disabled entity reports no status changes.
        let s = EntityState::new_enabled();
        s.set_status_bits(0b0011);
        s.set_status_bits(0b1100);
        assert_eq!(s.status_changes(), 0b1111);
        s.clear_status_changes(0b0101);
        assert_eq!(s.status_changes(), 0b1010);
    }

    #[test]
    fn status_condition_trigger_value() {
        // §2.2.2.1.1.7: the conditions of a disabled entity are inactive.
        let s = EntityState::new_enabled();
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
        assert!(Arc::ptr_eq(
            &state,
            &cond.entity_state().expect("the entity")
        ));
    }

    #[test]
    fn status_condition_entity_state_reflects_lifecycle_changes() {
        // The get_entity path must make lifecycle changes visible
        // (e.g. enable, mark_deleted) so callers can inspect the state
        // directly.
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        assert!(!cond.entity_state().expect("the entity").is_enabled());
        let _ = state.enable();
        assert!(cond.entity_state().expect("the entity").is_enabled());
        let _ = state.mark_deleted();
        assert!(cond.entity_state().expect("the entity").is_deleted());
    }
}
