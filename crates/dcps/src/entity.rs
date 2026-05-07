// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Entity-Lifecycle (DDS DCPS 1.4 §2.2.2.1) — gemeinsame Basis fuer
//! `DomainParticipant`, `Publisher`, `Subscriber`, `Topic`,
//! `DataWriter`, `DataReader`.
//!
//! Spec-Verhalten (§2.2.2.1.1 Entity-Base):
//! 1. **Lifecycle:** `create_*` → `enable()` → operational → `delete_*`.
//!    Pre-`enable()` ist die Entity inert (kein Discovery, keine Wire-
//!    Aktivitaet); set_qos auf alle Felder erlaubt.
//! 2. **set_qos** post-`enable()`: nur Felder mit "Changeable=YES"
//!    duerfen geaendert werden — sonst [`DdsError::ImmutablePolicy`]
//!    (§2.2.3 Tab. 2.13 Spalte "Changeable").
//! 3. **enable()** ist idempotent. Wenn das Parent-Entity (Participant)
//!    `entity_factory.autoenable_created_entities=TRUE` hat, werden
//!    Children bei Erzeugung automatisch enabled.
//! 4. **StatusCondition** ist der Hook fuer den `WaitSet` —
//!    `trigger_value()` liefert true, wenn ein Status mit Bit in der
//!    `enabled_statuses`-Mask aktiv ist.
//! 5. **InstanceHandle** ist eindeutig pro Entity (lokaler 64-Bit-Counter,
//!    nicht auf der Wire — siehe [`crate::instance_handle`]).
//!
//! .1 liefert die low-level [`Entity`]-Trait + [`EntityState`]
//! als Building-Block. Die Implementierungen (Publisher, DataWriter, ...)
//! halten ein `Arc<EntityState>` und delegieren die Trait-Methoden.

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::error::{DdsError, Result};
use crate::instance_handle::{InstanceHandle, InstanceHandleAllocator};

/// Globaler Allocator fuer Entity-InstanceHandles. Eine Instanz
/// pro Process — Handles sind innerhalb des Process eindeutig.
static ENTITY_HANDLE_ALLOCATOR: InstanceHandleAllocator = InstanceHandleAllocator::new();

/// `StatusMask` — 32-bit Bitmask der Status-Kinds (DCPS §2.2.4.1).
/// Werte aus [`crate::psm_constants::status`].
pub type StatusMask = u32;

/// Atomic-Container fuer den Entity-Lifecycle.
#[derive(Debug)]
pub struct EntityState {
    enabled: AtomicBool,
    /// `true` nach erfolgreichem `delete_*()` — Spec §2.2.1.1.5
    /// (RC ALREADY_DELETED). Public-Ops MUESSEN
    /// [`Self::check_not_deleted`] vor jedem Effekt aufrufen.
    deleted: AtomicBool,
    instance_handle: InstanceHandle,
    /// Bitmask der **seit letztem `get_status_changes()` Read**
    /// geaenderten Status-Bits.
    status_changes: AtomicU32,
    /// Bitmask der vom Listener abgedeckten Status-Bits (zur
    /// Bubble-Up-Logik in ).
    listener_mask: AtomicU32,
}

impl EntityState {
    /// Neuer State, initial **disabled** (Spec-Default fuer alle
    /// Entities ausser DomainParticipantFactory).
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

    /// Neuer State, **bereits enabled** — fuer DomainParticipantFactory
    /// (Spec §2.2.2.1.4: Factory ist immer enabled).
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

    /// True wenn die Entity enabled ist.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Setzt enabled=true (idempotent). Liefert `true` wenn der Aufruf
    /// die Transition false→true vollzogen hat (fuer Cascade-Logik).
    pub fn enable(&self) -> bool {
        !self.enabled.swap(true, Ordering::AcqRel)
    }

    /// Lokaler 64-Bit-Identifier dieser Entity.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.instance_handle
    }

    /// Aktuelle Status-Changes-Mask. Lesen leert NICHT — der Caller
    /// nimmt entscheidende Bits selbst zurueck via
    /// [`Self::clear_status_changes`].
    #[must_use]
    pub fn status_changes(&self) -> StatusMask {
        self.status_changes.load(Ordering::Acquire)
    }

    /// Setzt zusaetzliche Status-Bits (vom Discovery/Runtime-Layer
    /// gerufen, wenn ein Status-Event eintrifft).
    pub fn set_status_bits(&self, bits: StatusMask) {
        self.status_changes.fetch_or(bits, Ordering::AcqRel);
    }

    /// Loescht die uebergebenen Bits aus der Status-Changes-Mask
    /// (nach Caller's Read).
    pub fn clear_status_changes(&self, bits: StatusMask) {
        self.status_changes.fetch_and(!bits, Ordering::AcqRel);
    }

    /// Listener-Maske setzen — beeinflusst Bubble-Up.
    pub fn set_listener_mask(&self, mask: StatusMask) {
        self.listener_mask.store(mask, Ordering::Release);
    }

    /// Listener-Maske lesen.
    #[must_use]
    pub fn listener_mask(&self) -> StatusMask {
        self.listener_mask.load(Ordering::Acquire)
    }

    /// `true` wenn die Entity bereits `delete_*` durchlaufen hat.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted.load(Ordering::Acquire)
    }

    /// Markiert die Entity als geloescht (idempotent). Liefert `true`
    /// beim ersten Aufruf (Transition false→true), `false` bei
    /// nachfolgenden Aufrufen.
    pub fn mark_deleted(&self) -> bool {
        !self.deleted.swap(true, Ordering::AcqRel)
    }

    /// Guard-Helper fuer Public-Ops: liefert `Err(AlreadyDeleted)`
    /// wenn die Entity bereits geloescht wurde, sonst `Ok(())`.
    /// Nutzungs-Pattern:
    /// ```ignore
    /// pub fn write(&self, sample: T) -> Result<()> {
    ///     self.entity_state().check_not_deleted()?;
    ///     // ... eigentliche Logik ...
    /// }
    /// ```
    ///
    /// # Errors
    /// `DdsError::AlreadyDeleted` wenn `is_deleted() == true`.
    pub fn check_not_deleted(&self) -> crate::error::Result<()> {
        if self.is_deleted() {
            Err(crate::error::DdsError::AlreadyDeleted)
        } else {
            Ok(())
        }
    }

    /// Guard-Helper: liefert `Err(NotEnabled)` wenn die Entity nicht
    /// enabled ist (Spec §2.2.2.1.1.7 RC NOT_ENABLED).
    ///
    /// # Errors
    /// `DdsError::NotEnabled` wenn `is_enabled() == false`.
    pub fn check_enabled(&self) -> crate::error::Result<()> {
        if !self.is_enabled() {
            Err(crate::error::DdsError::NotEnabled)
        } else {
            Ok(())
        }
    }
}

/// `StatusCondition` — Spec §2.2.2.1.6, der primaere WaitSet-Hook.
///
/// In minimal: traegt eine `enabled_statuses`-Mask + delegiert
/// `trigger_value()` an [`EntityState::status_changes`]. In wird
/// das Object voll integriert (set_enabled_statuses, attach to WaitSet).
#[derive(Debug, Clone)]
pub struct StatusCondition {
    state: Arc<EntityState>,
    enabled_statuses: Arc<AtomicU32>,
}

impl StatusCondition {
    /// Konstruktor (intern; vom Entity erzeugt).
    #[must_use]
    pub fn new(state: Arc<EntityState>) -> Self {
        Self {
            state,
            enabled_statuses: Arc::new(AtomicU32::new(crate::psm_constants::status::ANY)),
        }
    }

    /// Setzt die `enabled_statuses`-Mask. Spec §2.2.2.1.6.
    pub fn set_enabled_statuses(&self, mask: StatusMask) {
        self.enabled_statuses.store(mask, Ordering::Release);
    }

    /// Liefert die aktuelle `enabled_statuses`-Mask.
    #[must_use]
    pub fn enabled_statuses(&self) -> StatusMask {
        self.enabled_statuses.load(Ordering::Acquire)
    }

    /// True wenn (status_changes & enabled_statuses) != 0.
    /// Spec §2.2.2.1.6 trigger_value.
    #[must_use]
    pub fn trigger_value(&self) -> bool {
        let enabled = self.enabled_statuses.load(Ordering::Acquire);
        let changes = self.state.status_changes();
        (enabled & changes) != 0
    }

    /// Liefert das `InstanceHandle` der Entity, an die diese
    /// StatusCondition gebunden ist. Spec DCPS 1.4 §2.2.2.1.9
    /// `get_entity()` — die Rust-API liefert den Handle anstelle eines
    /// `&dyn Entity`-Pointers, weil dieselbe `Arc<EntityState>` von
    /// mehreren Entity-Wrappern (DataReader/DataWriter/...) gehalten
    /// werden kann; der Handle ist die einzige Identitaet, die ueber
    /// die Wrapper-Granularitaet hinaus stabil ist.
    #[must_use]
    pub fn get_entity_handle(&self) -> InstanceHandle {
        self.state.instance_handle()
    }

    /// Liefert eine geteilte Referenz auf den zugrunde liegenden
    /// `EntityState` (Spec §2.2.2.1.9 — direkter Pfad). Erlaubt
    /// Caller-Code, Status-Mask und Lifecycle-Flags der Entity zu
    /// inspizieren, ohne durch den Entity-Wrapper gehen zu muessen.
    #[must_use]
    pub fn entity_state(&self) -> &Arc<EntityState> {
        &self.state
    }
}

/// Entity-Trait — gemeinsame Lifecycle-API der 6 Entity-Typen
/// (DCPS §2.2.2.1).
///
/// Nicht-blocking, Send+Sync — alle Methoden delegieren auf
/// `Arc<EntityState>`.
pub trait Entity {
    /// QoS-Typ fuer diese Entity (z.B. `DomainParticipantQos`,
    /// `DataWriterQos`, ...).
    type Qos: Clone;

    /// Liefert die aktuelle QoS (clone).
    /// Spec §2.2.2.1.2 `get_qos`.
    fn get_qos(&self) -> Self::Qos;

    /// Aendert QoS. Pre-enable: alles erlaubt. Post-enable: nur
    /// Felder mit "Changeable=YES" — sonst `ImmutablePolicy`-Error.
    /// Spec §2.2.2.1.2 `set_qos`.
    ///
    /// # Errors
    /// * [`DdsError::ImmutablePolicy`] wenn ein immutables Feld nach
    ///   `enable()` geaendert werden soll.
    /// * [`DdsError::InconsistentPolicy`] wenn die neue QoS-Kombination
    ///   inkonsistent ist.
    fn set_qos(&self, qos: Self::Qos) -> Result<()>;

    /// Enabled die Entity (idempotent). Spec §2.2.2.1.4 `enable`.
    ///
    /// # Errors
    /// [`DdsError::PreconditionNotMet`] wenn das Parent-Entity nicht
    /// enabled ist (Spec: Children koennen nicht vor Parent enabled
    /// werden — ausser Factory selbst).
    fn enable(&self) -> Result<()>;

    /// True wenn die Entity bereits enabled ist.
    fn is_enabled(&self) -> bool {
        self.entity_state().is_enabled()
    }

    /// `StatusCondition` dieser Entity.
    /// Spec §2.2.2.1.6 `get_status_condition`.
    fn get_status_condition(&self) -> StatusCondition {
        StatusCondition::new(self.entity_state())
    }

    /// Bitmask der Status-Kinds, die seit letztem Read geaendert haben.
    /// Spec §2.2.2.1.5 `get_status_changes`.
    fn get_status_changes(&self) -> StatusMask {
        self.entity_state().status_changes()
    }

    /// Lokaler 64-Bit-Identifier. Spec §2.2.2.1.7 `get_instance_handle`.
    fn get_instance_handle(&self) -> InstanceHandle {
        self.entity_state().instance_handle()
    }

    /// Interner Accessor — jede Impl liefert ihren `Arc<EntityState>`.
    fn entity_state(&self) -> Arc<EntityState>;
}

/// Hilfsfunktion: validiert dass ein QoS-Feld `policy_name` post-enable
/// nicht geaendert wurde. Verwendung in `set_qos`-Impls:
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

        // Keine Status-Aenderung → kein Trigger.
        assert!(!cond.trigger_value());

        // Status mit nicht-enabled Bit → kein Trigger.
        s.set_status_bits(0b0001);
        assert!(!cond.trigger_value());

        // Status mit enabled Bit → Trigger.
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
        // DomainParticipantFactory ist immer enabled (Spec §2.2.2.1.4).
        let s = EntityState::new_enabled();
        assert!(s.check_enabled().is_ok());
    }

    // ---- §2.2.2.1.9 StatusCondition.get_entity ----

    #[test]
    fn status_condition_get_entity_handle_matches_owner_state() {
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        // Handle der Condition == Handle der Entity, an die sie gebunden ist.
        assert_eq!(cond.get_entity_handle(), state.instance_handle());
    }

    #[test]
    fn status_condition_get_entity_handle_unique_per_entity() {
        // Zwei verschiedene Entities → zwei verschiedene Handles ueber
        // ihre StatusConditions.
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
        // Identitaet via Arc::ptr_eq — die Condition haelt genau diesen Arc,
        // keinen Clone der Inner.
        assert!(Arc::ptr_eq(&state, cond.entity_state()));
    }

    #[test]
    fn status_condition_entity_state_reflects_lifecycle_changes() {
        // get_entity-Pfad muss Lifecycle-Aenderungen sichtbar machen
        // (z.B. enable, mark_deleted), damit Caller den State direkt
        // inspizieren koennen.
        let state = EntityState::new();
        let cond = StatusCondition::new(state.clone());
        assert!(!cond.entity_state().is_enabled());
        let _ = state.enable();
        assert!(cond.entity_state().is_enabled());
        let _ = state.mark_deleted();
        assert!(cond.entity_state().is_deleted());
    }
}
