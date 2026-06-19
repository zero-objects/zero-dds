// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG Persistent State Service (PSS) stub layer.
//!
//! Spec: OMG PSS 1.2 (formal/2002-09-06). We provide the
//! object-mapping data structures + storage trait as a stub layer
//! for the CCM Extended Level Java path (omg-ccm-4.0 §2 item 6).
//!
//! The actual persistent-storage binding is provided by the caller (e.g.
//! a SQLite / RDBMS / NoSQL backend).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// PSS storage errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PssError {
    /// Object with the given PID not found.
    NotFound,
    /// Storage-backend error.
    StorageError(String),
    /// Invalid state (e.g. object already deleted).
    InvalidState(String),
}

/// Spec PSS §3 — `Pid` (persistent identifier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pid {
    /// Storage-home identifier.
    pub home_id: String,
    /// Object-specific key.
    pub key: Vec<u8>,
}

/// Spec PSS §6 — `StorageObject` trait.
pub trait StorageObject: Send + Sync {
    /// Persistent ID.
    fn pid(&self) -> &Pid;
    /// Marshal operation: serialize the object state.
    fn marshal(&self) -> Vec<u8>;
}

/// Spec PSS §7 — `StorageHome` trait for persistent-object management.
pub trait StorageHome: Send + Sync {
    /// Spec PSS §7.2 — `create(pid, value)`.
    ///
    /// # Errors
    /// `PssError::StorageError` on a backend error.
    fn create(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError>;

    /// Spec PSS §7.3 — `find_by_pid(pid)`.
    ///
    /// # Errors
    /// `PssError::NotFound` if not present.
    fn find_by_pid(&self, pid: &Pid) -> Result<Vec<u8>, PssError>;

    /// Spec PSS §7.4 — `delete(pid)`.
    ///
    /// # Errors
    /// `PssError::NotFound` if not present.
    fn delete(&self, pid: &Pid) -> Result<(), PssError>;
}

/// In-memory implementation of the `StorageHome` trait for tests +
/// a default stub.
#[derive(Default)]
pub struct InMemoryStorageHome {
    storage: Mutex<BTreeMap<Pid, Vec<u8>>>,
}

impl core::fmt::Debug for InMemoryStorageHome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.storage.lock().map_or(0, |g| g.len());
        f.debug_struct("InMemoryStorageHome")
            .field("count", &n)
            .finish()
    }
}

impl InMemoryStorageHome {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of persisted objects.
    pub fn len(&self) -> usize {
        self.storage.lock().map_or(0, |g| g.len())
    }

    /// `true` if there are no objects.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Ord for Pid {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.home_id
            .cmp(&other.home_id)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for Pid {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl StorageHome for InMemoryStorageHome {
    fn create(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError> {
        if let Ok(mut g) = self.storage.lock() {
            g.insert(pid, value);
            Ok(())
        } else {
            Err(PssError::StorageError("lock-poisoned".into()))
        }
    }

    fn find_by_pid(&self, pid: &Pid) -> Result<Vec<u8>, PssError> {
        let g = self
            .storage
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        g.get(pid).cloned().ok_or(PssError::NotFound)
    }

    fn delete(&self, pid: &Pid) -> Result<(), PssError> {
        let mut g = self
            .storage
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        g.remove(pid).ok_or(PssError::NotFound).map(|_| ())
    }
}

/// Spec PSS §10 — transaction status (subset of
/// `CosTransactions::Status`). Cross-ref `corba-ccm-ejb::tx::TxStatus`
/// — we copy the subset here to avoid the layer cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PssTxStatus {
    /// No active transaction.
    NoTransaction,
    /// A transaction is running (`begin_transaction` without `commit`/`rollback`).
    Active,
    /// `commit_transaction` completed.
    Committed,
    /// `rollback` executed — pending buffer discarded.
    RolledBack,
}

/// Tx handle — returned by `begin_transaction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxHandle(u64);

/// PSS session — wraps StorageHome + transaction state + pending buffer.
///
/// Spec PSS §10 — transactions are tx-aware: `store(pid, value)` and
/// `remove(pid)` write into the pending buffer; `commit` applies it
/// to the `StorageHome`, `rollback` discards it.
pub struct PssSession {
    home: Arc<dyn StorageHome>,
    in_transaction: Mutex<bool>,
    /// Pending buffer (Pid → Some(value)=write, None=delete) during
    /// a transaction.
    pending: Mutex<BTreeMap<Pid, Option<Vec<u8>>>>,
    /// Current tx status.
    tx_status: Mutex<PssTxStatus>,
    /// Monotonically increasing tx counter.
    next_tx_id: Mutex<u64>,
}

impl core::fmt::Debug for PssSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let tx = self.in_transaction.lock().map(|g| *g).unwrap_or(false);
        f.debug_struct("PssSession")
            .field("in_transaction", &tx)
            .finish()
    }
}

impl PssSession {
    /// Constructor.
    #[must_use]
    pub fn new(home: Arc<dyn StorageHome>) -> Self {
        Self {
            home,
            in_transaction: Mutex::new(false),
            pending: Mutex::new(BTreeMap::new()),
            tx_status: Mutex::new(PssTxStatus::NoTransaction),
            next_tx_id: Mutex::new(1),
        }
    }

    /// Spec PSS §10 — `begin_transaction`. Returns a tx handle.
    ///
    /// # Errors
    /// `PssError::InvalidState` if already in a transaction.
    pub fn begin_transaction(&self) -> Result<TxHandle, PssError> {
        let mut g = self
            .in_transaction
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        if *g {
            return Err(PssError::InvalidState("already in transaction".into()));
        }
        *g = true;
        let mut status = self
            .tx_status
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        *status = PssTxStatus::Active;
        let mut counter = self
            .next_tx_id
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        let id = *counter;
        *counter = counter.wrapping_add(1);
        Ok(TxHandle(id))
    }

    /// Spec PSS §10 — `commit(tx)`. Applies the pending buffer to the
    /// `StorageHome`.
    ///
    /// # Errors
    /// `PssError::InvalidState` if there is no active transaction.
    pub fn commit(&self, _tx: TxHandle) -> Result<(), PssError> {
        let mut g = self
            .in_transaction
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        if !*g {
            return Err(PssError::InvalidState("no active transaction".into()));
        }
        // Apply the pending buffer.
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        for (pid, op) in pending.iter() {
            match op {
                Some(value) => {
                    self.home.create(pid.clone(), value.clone())?;
                }
                None => {
                    // Best-effort: delete can return NotFound if no
                    // prior create(pid) ever existed. PSS spec
                    // §10.4 requires `silent` for the commit path.
                    let _ = self.home.delete(pid);
                }
            }
        }
        pending.clear();
        *g = false;
        let mut status = self
            .tx_status
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        *status = PssTxStatus::Committed;
        Ok(())
    }

    /// Spec PSS §10 — `rollback(tx)`. Discards the pending buffer.
    ///
    /// # Errors
    /// `PssError::InvalidState` if there is no active transaction.
    pub fn rollback(&self, _tx: TxHandle) -> Result<(), PssError> {
        let mut g = self
            .in_transaction
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        if !*g {
            return Err(PssError::InvalidState("no active transaction".into()));
        }
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        pending.clear();
        *g = false;
        let mut status = self
            .tx_status
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        *status = PssTxStatus::RolledBack;
        Ok(())
    }

    /// Spec PSS §10 — returns the current tx status.
    #[must_use]
    pub fn tx_status(&self) -> PssTxStatus {
        self.tx_status
            .lock()
            .map(|g| *g)
            .unwrap_or(PssTxStatus::NoTransaction)
    }

    /// Spec PSS §10.5 — legacy begin (without a tx handle, kept for
    /// backwards compatibility).
    ///
    /// # Errors
    /// `PssError::InvalidState` if already in a transaction.
    pub fn begin_transaction_legacy(&self) -> Result<(), PssError> {
        self.begin_transaction().map(|_| ())
    }

    /// Spec PSS §10.5 — legacy commit (without a tx handle).
    ///
    /// # Errors
    /// `PssError::InvalidState` if not in a transaction.
    pub fn commit_transaction(&self) -> Result<(), PssError> {
        self.commit(TxHandle(0))
    }

    /// Spec PSS §6 — `store(pid, value)`. Tx-aware: in tx mode
    /// writes into the pending buffer, otherwise straight through.
    ///
    /// # Errors
    /// Siehe [`PssError`].
    pub fn store(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError> {
        if self.is_in_transaction() {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
            pending.insert(pid, Some(value));
            Ok(())
        } else {
            self.home.create(pid, value)
        }
    }

    /// Spec PSS §6 — `remove(pid)`. Tx-aware, analogous to `store`.
    ///
    /// # Errors
    /// Siehe [`PssError`].
    pub fn remove(&self, pid: &Pid) -> Result<(), PssError> {
        if self.is_in_transaction() {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
            pending.insert(pid.clone(), None);
            Ok(())
        } else {
            self.home.delete(pid)
        }
    }

    /// Spec PSS §6 — `flush(pid, value)`. Writes straight through to the
    /// `StorageHome` (without the tx pending buffer).
    ///
    /// # Errors
    /// Siehe [`PssError`].
    pub fn flush(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError> {
        self.home.create(pid, value)
    }

    /// Spec PSS §6 — `load(pid)`. Tx-aware: reads from the pending
    /// buffer if the Pid is marked there as `Some(value)`; on
    /// `None` (delete pending) it returns `NotFound`.
    ///
    /// # Errors
    /// Siehe [`PssError`].
    pub fn load(&self, pid: &Pid) -> Result<Vec<u8>, PssError> {
        if self.is_in_transaction() {
            let pending = self
                .pending
                .lock()
                .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
            if let Some(op) = pending.get(pid) {
                return match op {
                    Some(v) => Ok(v.clone()),
                    None => Err(PssError::NotFound),
                };
            }
        }
        self.home.find_by_pid(pid)
    }

    fn is_in_transaction(&self) -> bool {
        self.in_transaction.lock().map(|g| *g).unwrap_or(false)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pid(home: &str, key: &[u8]) -> Pid {
        Pid {
            home_id: home.into(),
            key: key.to_vec(),
        }
    }

    #[test]
    fn in_memory_storage_create_and_find() {
        let h = InMemoryStorageHome::new();
        h.create(pid("Home", b"k1"), alloc::vec![1, 2, 3])
            .expect("ok");
        let v = h.find_by_pid(&pid("Home", b"k1")).expect("found");
        assert_eq!(v, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn find_unknown_pid_returns_not_found() {
        let h = InMemoryStorageHome::new();
        assert_eq!(
            h.find_by_pid(&pid("Home", b"missing")),
            Err(PssError::NotFound)
        );
    }

    #[test]
    fn delete_existing_pid() {
        let h = InMemoryStorageHome::new();
        h.create(pid("Home", b"k1"), alloc::vec![1]).expect("ok");
        h.delete(&pid("Home", b"k1")).expect("ok");
        assert_eq!(h.find_by_pid(&pid("Home", b"k1")), Err(PssError::NotFound));
    }

    #[test]
    fn delete_unknown_pid_returns_not_found() {
        let h = InMemoryStorageHome::new();
        assert_eq!(h.delete(&pid("Home", b"missing")), Err(PssError::NotFound));
    }

    #[test]
    fn len_tracks_count() {
        let h = InMemoryStorageHome::new();
        assert!(h.is_empty());
        h.create(pid("Home", b"a"), alloc::vec![]).expect("ok");
        h.create(pid("Home", b"b"), alloc::vec![]).expect("ok");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn pss_session_transaction_lifecycle() {
        let h: Arc<dyn StorageHome> = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(h);
        s.begin_transaction().expect("ok");
        assert_eq!(
            s.begin_transaction(),
            Err(PssError::InvalidState("already in transaction".into()))
        );
        s.commit_transaction().expect("ok");
        assert_eq!(
            s.commit_transaction(),
            Err(PssError::InvalidState("no active transaction".into()))
        );
    }

    #[test]
    fn pss_session_flush_and_load() {
        let h: Arc<dyn StorageHome> = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(h);
        s.flush(pid("H", b"x"), alloc::vec![42]).expect("ok");
        assert_eq!(s.load(&pid("H", b"x")).expect("ok"), alloc::vec![42]);
    }

    #[test]
    fn pid_ordering_stable() {
        let p1 = pid("A", b"1");
        let p2 = pid("A", b"2");
        let p3 = pid("B", b"1");
        assert!(p1 < p2);
        assert!(p2 < p3);
    }

    #[test]
    fn pss_error_variants_distinct() {
        assert_ne!(PssError::NotFound, PssError::StorageError("x".into()));
        assert_ne!(
            PssError::StorageError("a".into()),
            PssError::InvalidState("a".into())
        );
    }

    // §2 CP3 — tx-aware lifecycle wire-up.

    #[test]
    fn pss_begin_commit_roundtrip_persists_pending_writes() {
        let home = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(home.clone() as Arc<dyn StorageHome>);
        let tx = s.begin_transaction().expect("begin");
        s.store(pid("H", b"k1"), alloc::vec![0xAA]).expect("store");
        // Before commit the value is NOT in the StorageHome.
        assert_eq!(home.find_by_pid(&pid("H", b"k1")), Err(PssError::NotFound));
        s.commit(tx).expect("commit");
        // After commit it is in the StorageHome.
        assert_eq!(home.find_by_pid(&pid("H", b"k1")), Ok(alloc::vec![0xAA]));
        assert_eq!(s.tx_status(), PssTxStatus::Committed);
    }

    #[test]
    fn pss_rollback_restores_prev_state() {
        let home = Arc::new(InMemoryStorageHome::new());
        // Initial state: a value already exists.
        home.create(pid("H", b"k1"), alloc::vec![0x11]).expect("ok");
        let s = PssSession::new(home.clone() as Arc<dyn StorageHome>);
        let tx = s.begin_transaction().expect("begin");
        // Pending update + pending delete for a different key.
        s.store(pid("H", b"k1"), alloc::vec![0x22]).expect("store");
        s.store(pid("H", b"k2"), alloc::vec![0x33]).expect("store");
        // Within the tx, load() reads the pending value.
        assert_eq!(s.load(&pid("H", b"k1")).expect("load"), alloc::vec![0x22]);
        // Rollback discards the pending buffer.
        s.rollback(tx).expect("rollback");
        // The StorageHome stays in its original state.
        assert_eq!(home.find_by_pid(&pid("H", b"k1")), Ok(alloc::vec![0x11]));
        assert_eq!(home.find_by_pid(&pid("H", b"k2")), Err(PssError::NotFound));
        assert_eq!(s.tx_status(), PssTxStatus::RolledBack);
    }

    #[test]
    fn pss_load_after_store_in_tx_returns_pending_value() {
        let home = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(home as Arc<dyn StorageHome>);
        let _tx = s.begin_transaction().expect("begin");
        s.store(pid("H", b"k1"), alloc::vec![0x55]).expect("store");
        assert_eq!(s.load(&pid("H", b"k1")).expect("load"), alloc::vec![0x55]);
        // Pending delete of a different key: load returns NotFound.
        s.store(pid("H", b"k2"), alloc::vec![0x66]).expect("store");
        s.remove(&pid("H", b"k2")).expect("remove");
        assert_eq!(s.load(&pid("H", b"k2")), Err(PssError::NotFound));
    }

    #[test]
    fn pss_tx_status_transitions_active_committed_rolledback() {
        let home = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(home as Arc<dyn StorageHome>);
        assert_eq!(s.tx_status(), PssTxStatus::NoTransaction);
        let tx = s.begin_transaction().expect("begin");
        assert_eq!(s.tx_status(), PssTxStatus::Active);
        s.commit(tx).expect("commit");
        assert_eq!(s.tx_status(), PssTxStatus::Committed);
        // New tx cycle.
        let tx2 = s.begin_transaction().expect("begin2");
        assert_eq!(s.tx_status(), PssTxStatus::Active);
        s.rollback(tx2).expect("rollback");
        assert_eq!(s.tx_status(), PssTxStatus::RolledBack);
    }
}
