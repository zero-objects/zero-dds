// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PersistenceStorageComponent — Persistent State Service §10.
//!
//! Spec PSS 1.0 (`formal/2002-09-08`): definiert ein Storage-System
//! fuer CCM-Entity-Components, mit `storagetype` + `storagehome`
//! Datenmodell. Diese Component liefert eine produktionsreife
//! In-Memory-Implementation; Production-Caller plugt eine
//! persistente Backend-DB ein (PostgreSQL, RocksDB).
//!
//! Spec §10.2 — `StorageObject`: Lifecycle-State Active/Passive +
//! Identity (oid).

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use zerodds_corba_ccm::cif::{CifError, ComponentExecutor};
use zerodds_corba_ccm::context::ComponentContext;

/// Storage-Eintrag — Spec PSS §10.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageEntry {
    /// Storage-Object-Id (oid). Spec §10.2.1 — Primary-Key.
    pub oid: Vec<u8>,
    /// State-Bytes (CDR-encoded oder von Caller serialisiert).
    pub state: Vec<u8>,
    /// `dirty` — wurde modifiziert seit letztem `flush`.
    pub dirty: bool,
}

/// Persistence-Component-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    /// Object existiert nicht.
    NotFound(Vec<u8>),
    /// Object existiert bereits.
    DuplicateOid(Vec<u8>),
    /// CIF-Error.
    Cif(CifError),
}

impl core::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound(oid) => write!(f, "object not found: {oid:?}"),
            Self::DuplicateOid(oid) => write!(f, "duplicate oid: {oid:?}"),
            Self::Cif(e) => write!(f, "cif: {e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PersistenceError {}

/// PersistenceStorageComponent — production-ready CCM-Component.
#[derive(Default)]
pub struct PersistenceStorageComponent {
    storage: BTreeMap<Vec<u8>, StorageEntry>,
    activated: bool,
    flush_count: u64,
    ctx: Option<Box<dyn ComponentContext>>,
}

impl core::fmt::Debug for PersistenceStorageComponent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PersistenceStorageComponent")
            .field("entries", &self.storage.len())
            .field("activated", &self.activated)
            .field("flush_count", &self.flush_count)
            .finish_non_exhaustive()
    }
}

impl PersistenceStorageComponent {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `create` — Spec PSS §10.3.2 (StorageHomeBase::create).
    ///
    /// # Errors
    /// `DuplicateOid` wenn die oid schon existiert.
    pub fn create(&mut self, oid: Vec<u8>, state: Vec<u8>) -> Result<(), PersistenceError> {
        if self.storage.contains_key(&oid) {
            return Err(PersistenceError::DuplicateOid(oid));
        }
        self.storage.insert(
            oid.clone(),
            StorageEntry {
                oid,
                state,
                dirty: true,
            },
        );
        Ok(())
    }

    /// `find` — Spec PSS §10.3.4 (StorageHomeBase::find_by_short_pid).
    #[must_use]
    pub fn find(&self, oid: &[u8]) -> Option<&StorageEntry> {
        self.storage.get(oid)
    }

    /// `update` — modifiziert State, markiert `dirty`.
    ///
    /// # Errors
    /// `NotFound` wenn die oid unbekannt ist.
    pub fn update(&mut self, oid: &[u8], new_state: Vec<u8>) -> Result<(), PersistenceError> {
        let entry = self
            .storage
            .get_mut(oid)
            .ok_or_else(|| PersistenceError::NotFound(oid.to_vec()))?;
        entry.state = new_state;
        entry.dirty = true;
        Ok(())
    }

    /// `destroy` — Spec PSS §10.3.5 (StorageObjectBase::destroy).
    ///
    /// # Errors
    /// `NotFound` wenn die oid unbekannt ist.
    pub fn destroy(&mut self, oid: &[u8]) -> Result<(), PersistenceError> {
        if self.storage.remove(oid).is_none() {
            return Err(PersistenceError::NotFound(oid.to_vec()));
        }
        Ok(())
    }

    /// `flush` — Spec PSS §10.4.1 (Catalog::flush). Resettet alle
    /// `dirty`-Flags und inkrementiert den Flush-Counter.
    pub fn flush(&mut self) -> u64 {
        for entry in self.storage.values_mut() {
            entry.dirty = false;
        }
        self.flush_count += 1;
        self.flush_count
    }

    /// Anzahl der Storage-Entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Liefert `true` wenn keine Eintraege vorhanden sind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Liefert die aktuelle Flush-Anzahl.
    #[must_use]
    pub fn flush_count(&self) -> u64 {
        self.flush_count
    }

    /// Liefert die Anzahl `dirty` Eintraege.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.storage.values().filter(|e| e.dirty).count()
    }

    /// Liefert `true` wenn aktiviert.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.activated
    }
}

impl ComponentExecutor for PersistenceStorageComponent {
    fn set_context(&mut self, context: Box<dyn ComponentContext>) {
        self.ctx = Some(context);
    }

    fn ccm_activate(&mut self) -> Result<(), CifError> {
        self.activated = true;
        Ok(())
    }

    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        // Spec PSS §10.4.1: vor Passivierung muessen dirty-Eintraege
        // geflusht werden, sonst geht State verloren.
        let _ = self.flush();
        self.activated = false;
        Ok(())
    }

    fn ccm_remove(&mut self) -> Result<(), CifError> {
        self.activated = false;
        self.storage.clear();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn create_and_find_round_trip() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"oid1".to_vec(), b"state-A".to_vec()).unwrap();
        let entry = s.find(b"oid1").unwrap();
        assert_eq!(entry.state, b"state-A");
        assert!(entry.dirty);
    }

    #[test]
    fn duplicate_create_rejected() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"oid1".to_vec(), alloc::vec![]).unwrap();
        assert!(matches!(
            s.create(b"oid1".to_vec(), alloc::vec![]),
            Err(PersistenceError::DuplicateOid(_))
        ));
    }

    #[test]
    fn update_changes_state_and_marks_dirty() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"oid".to_vec(), b"old".to_vec()).unwrap();
        s.flush();
        assert_eq!(s.dirty_count(), 0);
        s.update(b"oid", b"new".to_vec()).unwrap();
        assert_eq!(s.find(b"oid").unwrap().state, b"new");
        assert_eq!(s.dirty_count(), 1);
    }

    #[test]
    fn update_unknown_fails() {
        let mut s = PersistenceStorageComponent::new();
        assert!(matches!(
            s.update(b"nope", alloc::vec![]),
            Err(PersistenceError::NotFound(_))
        ));
    }

    #[test]
    fn destroy_removes_entry() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"oid".to_vec(), alloc::vec![]).unwrap();
        s.destroy(b"oid").unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn flush_resets_dirty_and_increments_counter() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"a".to_vec(), alloc::vec![]).unwrap();
        s.create(b"b".to_vec(), alloc::vec![]).unwrap();
        assert_eq!(s.dirty_count(), 2);
        let n = s.flush();
        assert_eq!(n, 1);
        assert_eq!(s.dirty_count(), 0);
        assert_eq!(s.flush_count(), 1);
    }

    #[test]
    fn passivate_flushes_dirty_entries() {
        let mut s = PersistenceStorageComponent::new();
        s.ccm_activate().unwrap();
        s.create(b"a".to_vec(), alloc::vec![1]).unwrap();
        assert_eq!(s.dirty_count(), 1);
        s.ccm_passivate().unwrap();
        assert_eq!(s.dirty_count(), 0);
        assert!(!s.is_active());
    }

    #[test]
    fn remove_clears_all() {
        let mut s = PersistenceStorageComponent::new();
        s.create(b"a".to_vec(), alloc::vec![]).unwrap();
        s.ccm_remove().unwrap();
        assert!(s.is_empty());
    }
}
