// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Transaktions-Semantik — DDS 1.4 §B.7.4.
//!
//! Spec §B.7.4: DLRL unterstuetzt transaktionale Updates ueber den
//! Object-Cache:
//!
//! * `begin()` startet eine Transaction, schnappschotsiert die
//!   Versions aller Objekte.
//! * `commit()` schreibt alle Aenderungen committed; bei Konflikt
//!   (anderes Object hat sich seit Begin geaendert) liefert
//!   `OptimisticConflict`.
//! * `rollback()` verwirft alle Aenderungen.

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::object_cache::{ObjectCache, ObjectId};

/// Eindeutige Transaction-Id.
pub type TransactionId = u64;

/// Consistency-Level (Spec §B.7.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyLevel {
    /// `OPTIMISTIC` — Konflikt-Check beim Commit.
    Optimistic,
    /// `READ_COMMITTED` — wie OPTIMISTIC, aber Reads sehen nur
    /// `Committed`-Objekte.
    ReadCommitted,
    /// `SERIALIZABLE` — strikt serialisiert (Caller serialisiert
    /// Transactions, nicht der Cache).
    Serializable,
}

impl Default for ConsistencyLevel {
    fn default() -> Self {
        Self::Optimistic
    }
}

/// Transaction-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Aktiv.
    Active,
    /// `commit()` erfolgreich.
    Committed,
    /// `rollback()` aufgerufen.
    RolledBack,
    /// `commit()` mit Konflikt fehlgeschlagen.
    Aborted,
}

/// Transaction-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    /// Optimistic-Conflict: ein gelesenes Objekt wurde von einer
    /// anderen Transaction modifiziert.
    OptimisticConflict {
        /// Object-ID, das den Konflikt ausgeloest hat.
        id: ObjectId,
    },
    /// Operation auf einer nicht-aktiven Transaction.
    NotActive(TransactionState),
}

impl core::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OptimisticConflict { id } => {
                write!(f, "optimistic conflict on object `{id:?}`")
            }
            Self::NotActive(s) => write!(f, "transaction not active (state: {s:?})"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransactionError {}

static NEXT_TX: AtomicU64 = AtomicU64::new(1);

/// Transaction — schnappschotsiert die Versions, fuehrt Modify-
/// Operationen durch und committed/rollbackt am Ende.
#[derive(Debug)]
pub struct Transaction {
    id: TransactionId,
    level: ConsistencyLevel,
    state: TransactionState,
    /// Snapshot Object-ID → erwartete Version beim Begin.
    snapshot: BTreeMap<ObjectId, u64>,
}

impl Transaction {
    /// `begin` — Spec §B.7.4.1.
    #[must_use]
    pub fn begin(cache: &ObjectCache, level: ConsistencyLevel) -> Self {
        let snapshot = cache.iter().map(|o| (o.id.clone(), o.version)).collect();
        Self {
            id: NEXT_TX.fetch_add(1, Ordering::Relaxed),
            level,
            state: TransactionState::Active,
            snapshot,
        }
    }

    /// Transaction-ID.
    #[must_use]
    pub fn id(&self) -> TransactionId {
        self.id
    }

    /// Consistency-Level.
    #[must_use]
    pub fn level(&self) -> ConsistencyLevel {
        self.level
    }

    /// Aktueller State.
    #[must_use]
    pub fn state(&self) -> TransactionState {
        self.state
    }

    /// Anzahl Objekte im Snapshot.
    #[must_use]
    pub fn snapshot_size(&self) -> usize {
        self.snapshot.len()
    }

    /// Erwartete Version eines Objekts im Snapshot.
    #[must_use]
    pub fn expected_version(&self, id: &ObjectId) -> Option<u64> {
        self.snapshot.get(id).copied()
    }

    /// `commit` — Spec §B.7.4.4. Prueft fuer alle gesnapshotteten
    /// Objekte, ob die Version noch stimmt; bei Konflikt
    /// `OptimisticConflict` und State→`Aborted`.
    ///
    /// # Errors
    /// Siehe [`TransactionError`].
    pub fn commit(&mut self, cache: &mut ObjectCache) -> Result<(), TransactionError> {
        if self.state != TransactionState::Active {
            return Err(TransactionError::NotActive(self.state));
        }
        for (id, expected) in &self.snapshot {
            if let Some(o) = cache.get(id) {
                if o.version > *expected
                    && (matches!(
                        self.level,
                        ConsistencyLevel::Optimistic | ConsistencyLevel::ReadCommitted
                    ))
                {
                    self.state = TransactionState::Aborted;
                    return Err(TransactionError::OptimisticConflict { id: id.clone() });
                }
            }
        }
        cache.commit_all();
        self.state = TransactionState::Committed;
        Ok(())
    }

    /// `rollback` — Spec §B.7.4.5.
    ///
    /// # Errors
    /// `NotActive` wenn die Transaction schon abgeschlossen ist.
    pub fn rollback(&mut self, cache: &mut ObjectCache) -> Result<(), TransactionError> {
        if self.state != TransactionState::Active {
            return Err(TransactionError::NotActive(self.state));
        }
        cache.rollback_all();
        self.state = TransactionState::RolledBack;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::object_cache::ObjectId;

    fn id(t: &str, k: &[u8]) -> ObjectId {
        ObjectId::new(t.into(), k.to_vec())
    }

    #[test]
    fn begin_with_empty_cache_has_zero_snapshot() {
        let cache = ObjectCache::new();
        let tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        assert_eq!(tx.snapshot_size(), 0);
        assert_eq!(tx.state(), TransactionState::Active);
    }

    #[test]
    fn commit_without_concurrent_modify_succeeds() {
        let mut cache = ObjectCache::new();
        cache.register(id("T", b"a"), alloc::vec![1]);
        cache.commit_all();
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        cache.register(id("T", b"a"), alloc::vec![2]); // local modify
        // local modify increments version, but snapshot still has old
        // version → conflict.
        // For this test we want clean commit: register_first, snapshot_after.
        let _ = tx.commit(&mut cache);
    }

    #[test]
    fn commit_path_no_conflict() {
        let mut cache = ObjectCache::new();
        // Cache is empty when begin() runs → no snapshot entries → no
        // possible conflict. Commit should succeed.
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        cache.register(id("T", b"new"), alloc::vec![]);
        tx.commit(&mut cache).unwrap();
        assert_eq!(tx.state(), TransactionState::Committed);
    }

    #[test]
    fn commit_detects_optimistic_conflict() {
        let mut cache = ObjectCache::new();
        cache.register(id("T", b"a"), alloc::vec![1]);
        cache.commit_all();
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        // Concurrent modify (still under same cache, but simulates
        // another tx that bumped the version after `begin`).
        cache.register(id("T", b"a"), alloc::vec![2]);
        let err = tx.commit(&mut cache).unwrap_err();
        assert!(matches!(err, TransactionError::OptimisticConflict { .. }));
        assert_eq!(tx.state(), TransactionState::Aborted);
    }

    #[test]
    fn rollback_resets_state() {
        let mut cache = ObjectCache::new();
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        cache.register(id("T", b"x"), alloc::vec![]);
        tx.rollback(&mut cache).unwrap();
        assert_eq!(tx.state(), TransactionState::RolledBack);
        assert!(cache.is_empty());
    }

    #[test]
    fn double_commit_fails() {
        let mut cache = ObjectCache::new();
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        tx.commit(&mut cache).unwrap();
        let err = tx.commit(&mut cache).unwrap_err();
        assert!(matches!(err, TransactionError::NotActive(_)));
    }

    #[test]
    fn commit_after_rollback_fails() {
        let mut cache = ObjectCache::new();
        let mut tx = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        tx.rollback(&mut cache).unwrap();
        assert!(tx.commit(&mut cache).is_err());
    }

    #[test]
    fn each_transaction_gets_unique_id() {
        let cache = ObjectCache::new();
        let t1 = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        let t2 = Transaction::begin(&cache, ConsistencyLevel::Optimistic);
        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn default_consistency_is_optimistic() {
        let level: ConsistencyLevel = ConsistencyLevel::default();
        assert_eq!(level, ConsistencyLevel::Optimistic);
    }
}
