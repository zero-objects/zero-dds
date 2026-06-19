// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! OTS e2e: a distributed transaction over two transactional resources
//! (bank accounts with real state), driven by `Current`/Coordinator/
//! Terminator + 2-phase commit. Demonstrates:
//! * atomic commit (both accounts change only if both commit),
//! * atomic rollback on veto (insufficient funds → no change),
//! * transaction context propagation over the `TransactionService`
//!   ServiceContext bytes (coordinator → wire → remote server, same `otid`).

use std::cell::Cell;
use std::rc::Rc;

use zerodds_corba_cos_transactions::{
    Current, HeuristicOutcome, PropagationContext, Resource, Status, TransactionError, Vote,
};

/// An account as a transactional resource: `prepare` validates (no negative
/// balance), `commit` makes the staged posting durable, `rollback` discards it.
struct Account {
    balance: Cell<i64>,
    pending: Cell<i64>, // staged posting (set during begin..prepare)
}
impl Account {
    fn new(balance: i64) -> Rc<Self> {
        Rc::new(Self {
            balance: Cell::new(balance),
            pending: Cell::new(0),
        })
    }
    fn stage(&self, delta: i64) {
        self.pending.set(delta);
    }
    fn balance(&self) -> i64 {
        self.balance.get()
    }
}
impl Resource for Account {
    fn prepare(&self) -> Vote {
        let delta = self.pending.get();
        if delta == 0 {
            return Vote::ReadOnly; // no posting → no phase 2
        }
        if self.balance.get() + delta < 0 {
            return Vote::Rollback; // would overdraw → veto
        }
        Vote::Commit
    }
    fn commit(&self) -> Result<(), HeuristicOutcome> {
        self.balance.set(self.balance.get() + self.pending.get());
        self.pending.set(0);
        Ok(())
    }
    fn rollback(&self) -> Result<(), HeuristicOutcome> {
        self.pending.set(0); // discard the posting
        Ok(())
    }
    fn commit_one_phase(&self) -> Result<(), HeuristicOutcome> {
        self.commit()
    }
}

/// Atomic transfer: 100 from A to B — both commit, both balances change.
#[test]
fn distributed_transfer_commits_atomically() {
    let a = Account::new(500);
    let b = Account::new(200);
    let mut current = Current::new(30);
    current.begin().unwrap();
    let coord = current.get_control().unwrap().get_coordinator();

    a.stage(-100); // debit
    b.stage(100); // credit
    coord.register_resource(a.clone()).unwrap();
    coord.register_resource(b.clone()).unwrap();

    current.commit().unwrap();
    assert_eq!(current.get_status(), Status::NoTransaction);
    assert_eq!(a.balance(), 400);
    assert_eq!(b.balance(), 300);
    assert_eq!(a.balance() + b.balance(), 700, "sum preserved (atomic)");
}

/// Insufficient funds: A has too little → A votes Rollback → the ENTIRE
/// transaction rolls back, NO account changes.
#[test]
fn distributed_transfer_rolls_back_on_veto() {
    let a = Account::new(50);
    let b = Account::new(200);
    let mut current = Current::new(30);
    current.begin().unwrap();
    let coord = current.get_control().unwrap().get_coordinator();

    a.stage(-100); // would pull A to -50 → veto
    b.stage(100);
    coord.register_resource(a.clone()).unwrap();
    coord.register_resource(b.clone()).unwrap();

    assert_eq!(current.commit().unwrap_err(), TransactionError::RolledBack);
    assert_eq!(a.balance(), 50, "A unchanged");
    assert_eq!(b.balance(), 200, "B unchanged (atomic rollback)");
}

/// Transaction context propagation: the coordinator creates the
/// `PropagationContext`, serializes it as `TransactionService` ServiceContext
/// bytes (id = 0); a "remote server" parses them and sees the same `otid` —
/// so it can join the same transaction.
#[test]
fn transaction_context_propagates_over_wire() {
    let mut current = Current::new(45);
    current.begin().unwrap();
    let coord = current.get_control().unwrap().get_coordinator();
    let local_otid = coord.otid().clone();

    // Client side: serialize the context into the ServiceContext bytes.
    let ctx = coord.get_txcontext();
    let wire = ctx
        .to_service_context_data(zerodds_cdr::Endianness::Big)
        .unwrap();

    // Server side: parse the bytes, identify the transaction.
    let remote = PropagationContext::from_service_context_data(&wire).unwrap();
    assert_eq!(remote.timeout, 45);
    assert_eq!(remote.current.otid, local_otid);
    assert!(coord.is_same_transaction(&remote.current.otid));
}
