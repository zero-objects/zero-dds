// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OTS orchestration: `Current` / `Coordinator` / `Terminator` / `Control`
//! (OMG OTS §10.3) as Rust types over the 2PC engine.
//!
//! * [`Coordinator`] — manages the [`Resource`] participants + the transaction
//!   status, supplies the [`PropagationContext`] for GIOP propagation.
//! * [`Terminator`] — ends the transaction (`commit`/`rollback`), drives the
//!   2-phase commit.
//! * [`Control`] — handle on a transaction (Coordinator + Terminator).
//! * [`Current`] — context-bound transaction management
//!   (`begin`/`commit`/`rollback`).

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use crate::otid::Otid;
use crate::propagation::PropagationContext;
use crate::two_phase::{Completion, Resource, coordinate_commit, coordinate_rollback};

/// Transaction status (OMG OTS §10.3.2 `Status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Active — resources may be registered.
    Active,
    /// Marked for rollback (`rollback_only`) — `commit` will fail.
    MarkedRollback,
    /// Prepare phase completed.
    Prepared,
    /// Successfully committed.
    Committed,
    /// Rolled back.
    RolledBack,
    /// Commit in progress (phase 2).
    Committing,
    /// Rollback in progress.
    RollingBack,
    /// No transaction in this context.
    NoTransaction,
}

/// OTS errors (subset of the OMG OTS exceptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionError {
    /// No active transaction (`NoTransaction`).
    NoTransaction,
    /// Operation in an inactive state (`Inactive`).
    Inactive,
    /// Transaction was rolled back (`TRANSACTION_ROLLEDBACK`).
    RolledBack,
    /// Heuristically inconsistent outcome (`HeuristicMixed`).
    HeuristicMixed,
}

/// Coordinator (OMG OTS §10.3.4): holds the resources + status of a transaction.
pub struct Coordinator {
    otid: Otid,
    timeout: u32,
    status: Cell<Status>,
    resources: RefCell<Vec<Rc<dyn Resource>>>,
}

impl Coordinator {
    /// New coordinator for an active transaction with `otid` + timeout (sec).
    #[must_use]
    pub fn new(otid: Otid, timeout: u32) -> Self {
        Self {
            otid,
            timeout,
            status: Cell::new(Status::Active),
            resources: RefCell::new(Vec::new()),
        }
    }

    /// Registers a [`Resource`] as a transaction participant.
    ///
    /// # Errors
    /// [`TransactionError::Inactive`] if the transaction is no longer active;
    /// [`TransactionError::RolledBack`] if it is marked for rollback.
    pub fn register_resource(&self, resource: Rc<dyn Resource>) -> Result<(), TransactionError> {
        match self.status.get() {
            Status::Active => {
                self.resources.borrow_mut().push(resource);
                Ok(())
            }
            Status::MarkedRollback => Err(TransactionError::RolledBack),
            _ => Err(TransactionError::Inactive),
        }
    }

    /// Marks the transaction for rollback (any later `commit` fails).
    ///
    /// # Errors
    /// [`TransactionError::Inactive`] if not active.
    pub fn rollback_only(&self) -> Result<(), TransactionError> {
        match self.status.get() {
            Status::Active | Status::MarkedRollback => {
                self.status.set(Status::MarkedRollback);
                Ok(())
            }
            _ => Err(TransactionError::Inactive),
        }
    }

    /// Current status.
    #[must_use]
    pub fn get_status(&self) -> Status {
        self.status.get()
    }

    /// Number of registered resources.
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.resources.borrow().len()
    }

    /// `otid_t` of this transaction.
    #[must_use]
    pub fn otid(&self) -> &Otid {
        &self.otid
    }

    /// Whether `other` denotes the same transaction (same `otid`).
    #[must_use]
    pub fn is_same_transaction(&self, other: &Otid) -> bool {
        &self.otid == other
    }

    /// This transaction's [`PropagationContext`] to be propagated over GIOP.
    #[must_use]
    pub fn get_txcontext(&self) -> PropagationContext {
        PropagationContext::flat(self.timeout, self.otid.clone())
    }
}

/// Terminator (OMG OTS §10.3.5): ends a transaction.
pub struct Terminator {
    coord: Rc<Coordinator>,
}

impl Terminator {
    /// New terminator for `coord`.
    #[must_use]
    pub fn new(coord: Rc<Coordinator>) -> Self {
        Self { coord }
    }

    /// Commits the transaction: drives the 2-phase commit over all resources.
    /// A transaction marked for rollback is rolled back instead.
    ///
    /// # Errors
    /// [`TransactionError::RolledBack`] (veto/marked) or
    /// [`TransactionError::HeuristicMixed`].
    pub fn commit(&self) -> Result<(), TransactionError> {
        if self.coord.status.get() == Status::MarkedRollback {
            self.do_rollback();
            return Err(TransactionError::RolledBack);
        }
        self.coord.status.set(Status::Committing);
        let resources = self.coord.resources.borrow().clone();
        match coordinate_commit(&resources) {
            Completion::Committed => {
                self.coord.status.set(Status::Committed);
                Ok(())
            }
            Completion::RolledBack => {
                self.coord.status.set(Status::RolledBack);
                Err(TransactionError::RolledBack)
            }
            Completion::HeuristicMixed => {
                self.coord.status.set(Status::Committed);
                Err(TransactionError::HeuristicMixed)
            }
        }
    }

    /// Rolls the transaction back (phase 2 rollback over all resources).
    pub fn rollback(&self) {
        self.do_rollback();
    }

    fn do_rollback(&self) {
        self.coord.status.set(Status::RollingBack);
        let resources = self.coord.resources.borrow().clone();
        coordinate_rollback(&resources);
        self.coord.status.set(Status::RolledBack);
    }
}

/// Control (OMG OTS §10.3.3): handle on a transaction — access to the
/// [`Coordinator`] and [`Terminator`].
#[derive(Clone)]
pub struct Control {
    coord: Rc<Coordinator>,
}

impl Control {
    /// New Control over a fresh coordinator (`otid` + timeout).
    #[must_use]
    pub fn new(otid: Otid, timeout: u32) -> Self {
        Self {
            coord: Rc::new(Coordinator::new(otid, timeout)),
        }
    }

    /// The coordinator of this transaction.
    #[must_use]
    pub fn get_coordinator(&self) -> Rc<Coordinator> {
        Rc::clone(&self.coord)
    }

    /// A terminator for this transaction.
    #[must_use]
    pub fn get_terminator(&self) -> Terminator {
        Terminator::new(Rc::clone(&self.coord))
    }
}

/// Current (OMG OTS §10.3.6): context-bound transaction management. Holds the
/// transaction active in the current context and issues deterministic `otid`s
/// (counter-based; a real ORB uses UUID/time).
#[derive(Default)]
pub struct Current {
    active: Option<Control>,
    counter: u32,
    default_timeout: u32,
}

impl Current {
    /// New `Current` with a default timeout (sec; 0 = no timeout).
    #[must_use]
    pub fn new(default_timeout: u32) -> Self {
        Self {
            active: None,
            counter: 0,
            default_timeout,
        }
    }

    /// Begins a new transaction and makes it active in the context.
    ///
    /// # Errors
    /// [`TransactionError::Inactive`] if a transaction is already active
    /// (nested transactions are not supported here).
    pub fn begin(&mut self) -> Result<(), TransactionError> {
        if self.active.is_some() {
            return Err(TransactionError::Inactive);
        }
        self.counter = self.counter.wrapping_add(1);
        let otid = Otid::new(0, self.counter.to_be_bytes().to_vec());
        self.active = Some(Control::new(otid, self.default_timeout));
        Ok(())
    }

    /// Commits the active transaction and leaves the context.
    ///
    /// # Errors
    /// [`TransactionError::NoTransaction`] without an active transaction;
    /// otherwise the `Terminator::commit` errors.
    pub fn commit(&mut self) -> Result<(), TransactionError> {
        let control = self.active.take().ok_or(TransactionError::NoTransaction)?;
        control.get_terminator().commit()
    }

    /// Rolls the active transaction back and leaves the context.
    ///
    /// # Errors
    /// [`TransactionError::NoTransaction`] without an active transaction.
    pub fn rollback(&mut self) -> Result<(), TransactionError> {
        let control = self.active.take().ok_or(TransactionError::NoTransaction)?;
        control.get_terminator().rollback();
        Ok(())
    }

    /// Marks the active transaction for rollback.
    ///
    /// # Errors
    /// [`TransactionError::NoTransaction`] without an active transaction.
    pub fn rollback_only(&self) -> Result<(), TransactionError> {
        let control = self
            .active
            .as_ref()
            .ok_or(TransactionError::NoTransaction)?;
        control.get_coordinator().rollback_only()
    }

    /// Status of the active transaction (or [`Status::NoTransaction`]).
    #[must_use]
    pub fn get_status(&self) -> Status {
        self.active
            .as_ref()
            .map_or(Status::NoTransaction, |c| c.get_coordinator().get_status())
    }

    /// The Control of the active transaction (for `register_resource` etc.).
    #[must_use]
    pub fn get_control(&self) -> Option<Control> {
        self.active.clone()
    }

    /// Suspends the active transaction (returns its Control, clears the context).
    pub fn suspend(&mut self) -> Option<Control> {
        self.active.take()
    }

    /// Restores a suspended transaction into the context.
    pub fn resume(&mut self, control: Control) {
        self.active = Some(control);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::two_phase::{HeuristicOutcome, Vote};
    use core::cell::Cell;

    struct Acct {
        vote: Vote,
        committed: Cell<bool>,
        rolled_back: Cell<bool>,
    }
    impl Acct {
        fn new(vote: Vote) -> Rc<Self> {
            Rc::new(Self {
                vote,
                committed: Cell::new(false),
                rolled_back: Cell::new(false),
            })
        }
    }
    impl Resource for Acct {
        fn prepare(&self) -> Vote {
            self.vote
        }
        fn commit(&self) -> Result<(), HeuristicOutcome> {
            self.committed.set(true);
            Ok(())
        }
        fn rollback(&self) -> Result<(), HeuristicOutcome> {
            self.rolled_back.set(true);
            Ok(())
        }
        fn commit_one_phase(&self) -> Result<(), HeuristicOutcome> {
            self.committed.set(true);
            Ok(())
        }
    }

    #[test]
    fn current_begin_commit_two_resources() {
        let mut cur = Current::new(30);
        cur.begin().unwrap();
        assert_eq!(cur.get_status(), Status::Active);
        let coord = cur.get_control().unwrap().get_coordinator();
        let debit = Acct::new(Vote::Commit);
        let credit = Acct::new(Vote::Commit);
        coord.register_resource(debit.clone()).unwrap();
        coord.register_resource(credit.clone()).unwrap();
        cur.commit().unwrap();
        assert!(debit.committed.get() && credit.committed.get());
        assert_eq!(cur.get_status(), Status::NoTransaction); // context left
    }

    #[test]
    fn rollback_only_forces_rollback_on_commit() {
        let mut cur = Current::new(0);
        cur.begin().unwrap();
        let coord = cur.get_control().unwrap().get_coordinator();
        let r = Acct::new(Vote::Commit);
        coord.register_resource(r.clone()).unwrap();
        cur.rollback_only().unwrap();
        assert_eq!(cur.get_status(), Status::MarkedRollback);
        assert_eq!(cur.commit().unwrap_err(), TransactionError::RolledBack);
        assert!(r.rolled_back.get() && !r.committed.get());
    }

    #[test]
    fn explicit_rollback() {
        let mut cur = Current::new(0);
        cur.begin().unwrap();
        let coord = cur.get_control().unwrap().get_coordinator();
        let r = Acct::new(Vote::Commit);
        coord.register_resource(r.clone()).unwrap();
        cur.rollback().unwrap();
        assert!(r.rolled_back.get());
    }

    #[test]
    fn commit_without_transaction_errors() {
        let mut cur = Current::new(0);
        assert_eq!(cur.commit().unwrap_err(), TransactionError::NoTransaction);
    }

    #[test]
    fn double_begin_rejected() {
        let mut cur = Current::new(0);
        cur.begin().unwrap();
        assert_eq!(cur.begin().unwrap_err(), TransactionError::Inactive);
    }

    #[test]
    fn suspend_resume_keeps_resources() {
        let mut cur = Current::new(0);
        cur.begin().unwrap();
        let coord = cur.get_control().unwrap().get_coordinator();
        coord.register_resource(Acct::new(Vote::Commit)).unwrap();
        let suspended = cur.suspend().unwrap();
        assert_eq!(cur.get_status(), Status::NoTransaction);
        cur.resume(suspended);
        assert_eq!(
            cur.get_control()
                .unwrap()
                .get_coordinator()
                .resource_count(),
            1
        );
    }

    #[test]
    fn propagation_context_carries_otid() {
        let mut cur = Current::new(45);
        cur.begin().unwrap();
        let pc = cur.get_control().unwrap().get_coordinator().get_txcontext();
        assert_eq!(pc.timeout, 45);
        assert_eq!(pc.current.otid.format_id, 0);
        assert!(!pc.current.otid.tid.is_empty());
    }
}
