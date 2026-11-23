// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CosTransactions::Current` as a local interface — CORBA 3.3 Part 3
//! §6.13.2.2: "The following CORBA transaction service interface is
//! changed to a local interface: `CosTransactions::Current`".
//!
//! A local object has no object reference: it is not marshalled into a
//! request or a reply, and `ORB::object_to_string` refuses it (`MARSHAL`
//! minor 4, Part 1 §8.2.2). An application reaches it through the ORB, as
//! the initial reference `"TransactionCurrent"`. [`TransactionCurrent`] is
//! that object: a handle any thread may share, behind which each thread
//! has a [`Current`] of its own — the transaction context of that thread
//! (OTS §10.3.6).

use alloc::collections::BTreeMap;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::transaction::Current;

/// The `ORB::resolve_initial_references` identifier of the transaction
/// current (OTS §10.3.6).
pub const TRANSACTION_CURRENT_INITIAL_REFERENCE: &str = "TransactionCurrent";

/// The repository id of `CosTransactions::Current`.
pub const CURRENT_REPOSITORY_ID: &str = "IDL:omg.org/CosTransactions/Current:1.0";

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

std::thread_local! {
    /// The `Current` of each `TransactionCurrent` on this thread.
    static CURRENTS: RefCell<BTreeMap<usize, Current>> = const { RefCell::new(BTreeMap::new()) };
}

/// The local `CosTransactions::Current` object of an ORB (Part 3
/// §6.13.2.2): shared by every thread, a [`Current`] per thread.
#[derive(Debug)]
pub struct TransactionCurrent {
    id: usize,
    default_timeout: AtomicU32,
}

impl TransactionCurrent {
    /// A transaction current whose transactions time out after
    /// `default_timeout` seconds (0: no timeout).
    #[must_use]
    pub fn new(default_timeout: u32) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            default_timeout: AtomicU32::new(default_timeout),
        }
    }

    /// OTS `Current::set_timeout`: the timeout of the transactions the
    /// threads begin from now on, in seconds (0: no timeout).
    pub fn set_timeout(&self, seconds: u32) {
        self.default_timeout.store(seconds, Ordering::Relaxed);
        self.with(|c| c.set_default_timeout(seconds));
    }

    /// Runs `f` on the calling thread's [`Current`]. A thread that calls
    /// this for the first time gets a `Current` without a transaction.
    /// `f` must not call `with` on the same object again: the inner call
    /// would see a fresh `Current` that the outer one then replaces.
    pub fn with<R>(&self, f: impl FnOnce(&mut Current) -> R) -> R {
        let mut current = CURRENTS
            .with(|m| m.borrow_mut().remove(&self.id))
            .unwrap_or_else(|| Current::new(self.default_timeout.load(Ordering::Relaxed)));
        let r = f(&mut current);
        CURRENTS.with(|m| {
            m.borrow_mut().insert(self.id, current);
        });
        r
    }
}

impl Drop for TransactionCurrent {
    fn drop(&mut self) {
        // The other threads' entries go with their threads.
        let _ = CURRENTS.try_with(|m| m.borrow_mut().remove(&self.id));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::transaction::{Status, TransactionError};

    fn assert_shareable<T: Send + Sync + 'static>() {}

    #[test]
    fn corba_p3_6_13_2_2_each_thread_has_its_own_transaction_context() {
        assert_shareable::<TransactionCurrent>();
        let tc = std::sync::Arc::new(TransactionCurrent::new(0));
        tc.with(|c| c.begin()).unwrap();
        assert_eq!(tc.with(|c| c.get_status()), Status::Active);
        let other = std::sync::Arc::clone(&tc);
        let seen = std::thread::spawn(move || {
            let before = other.with(|c| c.get_status());
            other.with(|c| c.begin()).unwrap();
            other.with(|c| c.commit()).unwrap();
            before
        })
        .join()
        .unwrap();
        assert_eq!(seen, Status::NoTransaction);
        // This thread's transaction is untouched by the other thread.
        assert_eq!(tc.with(|c| c.get_status()), Status::Active);
        tc.with(|c| c.commit()).unwrap();
        assert_eq!(tc.with(|c| c.get_status()), Status::NoTransaction);
        assert_eq!(
            tc.with(|c| c.commit()).unwrap_err(),
            TransactionError::NoTransaction
        );
    }

    #[test]
    fn corba_p3_6_13_2_2_two_transaction_currents_do_not_share_a_context() {
        let a = TransactionCurrent::new(0);
        let b = TransactionCurrent::new(0);
        a.with(|c| c.begin()).unwrap();
        assert_eq!(b.with(|c| c.get_status()), Status::NoTransaction);
        a.with(|c| c.rollback()).unwrap();
    }

    #[test]
    fn corba_p3_6_13_2_2_set_timeout_reaches_the_next_transaction() {
        let tc = TransactionCurrent::new(0);
        tc.set_timeout(30);
        tc.with(|c| c.begin()).unwrap();
        let timeout = tc.with(|c| {
            c.get_control()
                .expect("a transaction")
                .get_coordinator()
                .timeout()
        });
        assert_eq!(timeout, 30);
        tc.with(|c| c.rollback()).unwrap();
    }
}
