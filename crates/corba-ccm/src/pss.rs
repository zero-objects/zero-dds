// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG Persistent State Service (PSS) Stub-Layer.
//!
//! Spec: OMG PSS 1.2 (formal/2002-09-06). Wir liefern die
//! Object-Mapping-Datenstrukturen + Storage-Trait als Stub-Layer
//! fuer den CCM-Extended-Level-Java-Pfad (omg-ccm-4.0 §2 Punkt 6).
//!
//! Echte Persistent-Storage-Bindung erfolgt durch Caller (z.B.
//! SQLite / RDBMS / NoSQL-Backend).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// PSS-Storage-Errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PssError {
    /// Object mit gegebener PID nicht gefunden.
    NotFound,
    /// Storage-Backend-Fehler.
    StorageError(String),
    /// Invalid-State (z.B. Object schon geloescht).
    InvalidState(String),
}

/// Spec PSS §3 — `Pid` (Persistent Identifier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pid {
    /// Storage-Home-Identifier.
    pub home_id: String,
    /// Object-spezifischer Key.
    pub key: Vec<u8>,
}

/// Spec PSS §6 — `StorageObject`-Trait.
pub trait StorageObject: Send + Sync {
    /// Persistent ID.
    fn pid(&self) -> &Pid;
    /// Marshal-Operation: serialisiere den Object-State.
    fn marshal(&self) -> Vec<u8>;
}

/// Spec PSS §7 — `StorageHome`-Trait fuer Persistent-Object-Verwaltung.
pub trait StorageHome: Send + Sync {
    /// Spec PSS §7.2 — `create(pid, value)`.
    ///
    /// # Errors
    /// `PssError::StorageError` bei Backend-Fehler.
    fn create(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError>;

    /// Spec PSS §7.3 — `find_by_pid(pid)`.
    ///
    /// # Errors
    /// `PssError::NotFound` wenn nicht vorhanden.
    fn find_by_pid(&self, pid: &Pid) -> Result<Vec<u8>, PssError>;

    /// Spec PSS §7.4 — `delete(pid)`.
    ///
    /// # Errors
    /// `PssError::NotFound` wenn nicht vorhanden.
    fn delete(&self, pid: &Pid) -> Result<(), PssError>;
}

/// In-Memory-Implementation des `StorageHome`-Trait fuer Tests +
/// Default-Stub.
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
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Anzahl persistierter Objects.
    pub fn len(&self) -> usize {
        self.storage.lock().map_or(0, |g| g.len())
    }

    /// `true` wenn keine Objects.
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

/// Spec PSS §10 — Transaction-Status (Subset von
/// `CosTransactions::Status`). Cross-Ref `corba-ccm-ejb::tx::TxStatus`
/// — wir kopieren das Subset hier, um den Layer-Zyklus zu vermeiden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PssTxStatus {
    /// Keine aktive Transaction.
    NoTransaction,
    /// Transaction laeuft (`begin_transaction` ohne `commit`/`rollback`).
    Active,
    /// `commit_transaction` durchgelaufen.
    Committed,
    /// `rollback` ausgefuehrt — Pending-Buffer verworfen.
    RolledBack,
}

/// Tx-Handle — von `begin_transaction` zurueckgegeben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxHandle(u64);

/// PSS-Session — wraps StorageHome + Transaction-State + Pending-Buffer.
///
/// Spec PSS §10 — Transaktionen sind tx-aware: `store(pid, value)` und
/// `remove(pid)` schreiben in den Pending-Buffer; `commit` wendet ihn
/// auf die `StorageHome` an, `rollback` verwirft ihn.
pub struct PssSession {
    home: Arc<dyn StorageHome>,
    in_transaction: Mutex<bool>,
    /// Pending-Buffer (Pid → Some(value)=write, None=delete) waehrend
    /// einer Transaction.
    pending: Mutex<BTreeMap<Pid, Option<Vec<u8>>>>,
    /// Aktueller Tx-Status.
    tx_status: Mutex<PssTxStatus>,
    /// Monoton steigender Tx-Counter.
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
    /// Konstruktor.
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

    /// Spec PSS §10 — `begin_transaction`. Liefert Tx-Handle.
    ///
    /// # Errors
    /// `PssError::InvalidState` wenn schon in Transaction.
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

    /// Spec PSS §10 — `commit(tx)`. Wendet den Pending-Buffer auf die
    /// `StorageHome` an.
    ///
    /// # Errors
    /// `PssError::InvalidState` wenn keine aktive Transaction.
    pub fn commit(&self, _tx: TxHandle) -> Result<(), PssError> {
        let mut g = self
            .in_transaction
            .lock()
            .map_err(|_| PssError::StorageError("lock-poisoned".into()))?;
        if !*g {
            return Err(PssError::InvalidState("no active transaction".into()));
        }
        // Pending-Buffer applien.
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
                    // Best-Effort: Delete kann NotFound geben, wenn nie
                    // ein vorheriger create(pid) existierte. PSS-Spec
                    // §10.4 verlangt `silent` fuer commit-Pfad.
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

    /// Spec PSS §10 — `rollback(tx)`. Verwirft den Pending-Buffer.
    ///
    /// # Errors
    /// `PssError::InvalidState` wenn keine aktive Transaction.
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

    /// Spec PSS §10 — Liefert den aktuellen Tx-Status.
    #[must_use]
    pub fn tx_status(&self) -> PssTxStatus {
        self.tx_status
            .lock()
            .map(|g| *g)
            .unwrap_or(PssTxStatus::NoTransaction)
    }

    /// Spec PSS §10.5 — Legacy-Begin (ohne Tx-Handle, keep fuer
    /// Rueckwaerts-Kompat).
    ///
    /// # Errors
    /// `PssError::InvalidState` wenn schon in Transaction.
    pub fn begin_transaction_legacy(&self) -> Result<(), PssError> {
        self.begin_transaction().map(|_| ())
    }

    /// Spec PSS §10.5 — Legacy-Commit (ohne Tx-Handle).
    ///
    /// # Errors
    /// `PssError::InvalidState` wenn nicht in Transaction.
    pub fn commit_transaction(&self) -> Result<(), PssError> {
        self.commit(TxHandle(0))
    }

    /// Spec PSS §6 — `store(pid, value)`. Tx-aware: schreibt im
    /// Tx-Modus in den Pending-Buffer, sonst direkt durch.
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

    /// Spec PSS §6 — `remove(pid)`. Tx-aware analog zu `store`.
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

    /// Spec PSS §6 — `flush(pid, value)`. Schreibt direkt durch zur
    /// `StorageHome` (ohne Tx-Pending-Buffer).
    ///
    /// # Errors
    /// Siehe [`PssError`].
    pub fn flush(&self, pid: Pid, value: Vec<u8>) -> Result<(), PssError> {
        self.home.create(pid, value)
    }

    /// Spec PSS §6 — `load(pid)`. Tx-aware: liest aus dem Pending-
    /// Buffer wenn die Pid dort als `Some(value)` markiert ist; bei
    /// `None` (Delete-Pending) liefert `NotFound`.
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

    // §2 CP3 — Tx-aware-Lifecycle Wire-up.

    #[test]
    fn pss_begin_commit_roundtrip_persists_pending_writes() {
        let home = Arc::new(InMemoryStorageHome::new());
        let s = PssSession::new(home.clone() as Arc<dyn StorageHome>);
        let tx = s.begin_transaction().expect("begin");
        s.store(pid("H", b"k1"), alloc::vec![0xAA]).expect("store");
        // Vor Commit ist der Wert NICHT in der StorageHome.
        assert_eq!(home.find_by_pid(&pid("H", b"k1")), Err(PssError::NotFound));
        s.commit(tx).expect("commit");
        // Nach Commit liegt er in der StorageHome.
        assert_eq!(home.find_by_pid(&pid("H", b"k1")), Ok(alloc::vec![0xAA]));
        assert_eq!(s.tx_status(), PssTxStatus::Committed);
    }

    #[test]
    fn pss_rollback_restores_prev_state() {
        let home = Arc::new(InMemoryStorageHome::new());
        // Initialer Zustand: ein Wert existiert schon.
        home.create(pid("H", b"k1"), alloc::vec![0x11]).expect("ok");
        let s = PssSession::new(home.clone() as Arc<dyn StorageHome>);
        let tx = s.begin_transaction().expect("begin");
        // Pending-Update + Pending-Delete fuer einen anderen Key.
        s.store(pid("H", b"k1"), alloc::vec![0x22]).expect("store");
        s.store(pid("H", b"k2"), alloc::vec![0x33]).expect("store");
        // Innerhalb der Tx liest load() den Pending-Wert.
        assert_eq!(s.load(&pid("H", b"k1")).expect("load"), alloc::vec![0x22]);
        // Rollback verwirft die Pending-Buffer.
        s.rollback(tx).expect("rollback");
        // StorageHome bleibt im urspruenglichen Zustand.
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
        // Pending-Delete eines anderen Keys: load liefert NotFound.
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
        // Neuer Tx-Cycle.
        let tx2 = s.begin_transaction().expect("begin2");
        assert_eq!(s.tx_status(), PssTxStatus::Active);
        s.rollback(tx2).expect("rollback");
        assert_eq!(s.tx_status(), PssTxStatus::RolledBack);
    }
}
