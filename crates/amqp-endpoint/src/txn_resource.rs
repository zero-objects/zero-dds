// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The transactions a coordinator holds: the transactional resource of
//! OASIS AMQP 1.0 Part 4, without the work itself.
//!
//! [`TxnTable`] allocates the identifier of a declare (§4.2), keeps the
//! work a container records against a transaction (§4.4) and hands it
//! back with the discharge (§4.3): to apply it, or to undo it. What the
//! work is, is the container's: `W`. Each transaction belongs to the
//! control link that declared it (`O`); when that link goes, its
//! transactions are rolled back (§4.1). A transaction that stays
//! undischarged past the time-out is rolled back; its discharge is then
//! refused with `amqp:transaction:timeout` (§4.5.8).
//!
//! `no_std` + `alloc`.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::limits::TxnLimits;
use crate::terminus::AmqpError;
use crate::transactions::{TransactionError, TxnId};

struct Txn<O, W> {
    owner: O,
    work: Vec<W>,
    /// Why the transaction can only be rolled back.
    doom: Option<(TransactionError, String)>,
    /// When it times out; `None` without a time-out or once it did.
    deadline_ms: Option<u64>,
}

/// What a discharge does (§4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discharged<W> {
    /// `fail` was not set and nothing stands against it: the work is to be
    /// applied.
    Commit(Vec<W>),
    /// `fail` was set: the work is to be undone. Always possible (§4.3:
    /// "the coordinator MUST always be able to complete a discharge where
    /// the fail flag is set to true").
    Rollback(Vec<W>),
    /// The discharge cannot be completed: the error goes to the
    /// controller as the transaction error; the work, if the transaction
    /// existed, is to be undone.
    Refused {
        /// The transaction error.
        error: AmqpError,
        /// The work of the transaction.
        work: Vec<W>,
    },
}

/// The transactions of one coordinator.
pub struct TxnTable<O, W> {
    limits: TxnLimits,
    next: u64,
    txns: BTreeMap<TxnId, Txn<O, W>>,
}

impl<O, W> core::fmt::Debug for TxnTable<O, W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TxnTable")
            .field("limits", &self.limits)
            .field("transactions", &self.txns.len())
            .finish_non_exhaustive()
    }
}

impl<O: PartialEq + Clone, W> TxnTable<O, W> {
    /// No transactions, bounded by `limits`.
    #[must_use]
    pub fn new(limits: TxnLimits) -> Self {
        Self {
            limits,
            next: 0,
            txns: BTreeMap::new(),
        }
    }

    /// Transactions declared and not discharged.
    #[must_use]
    pub fn len(&self) -> usize {
        self.txns.len()
    }

    /// Whether no transaction is declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.txns.is_empty()
    }

    /// Declares a transaction for the control link `owner` at `now_ms`
    /// and returns its identifier (§4.2).
    ///
    /// # Errors
    /// `amqp:resource-limit-exceeded` when the table holds as many
    /// transactions as [`TxnLimits::max_transactions`] admits.
    pub fn declare(&mut self, owner: O, now_ms: u64) -> Result<TxnId, AmqpError> {
        if self.txns.len() >= self.limits.max_transactions as usize {
            return Err(AmqpError::new(
                "amqp:resource-limit-exceeded",
                format!(
                    "{} transactions are declared and not discharged, the bound",
                    self.limits.max_transactions
                ),
            ));
        }
        // Eight octets of a counter: unique for the life of the table, and
        // within the 32 octets of §4.5.4.
        let id = loop {
            self.next = self.next.wrapping_add(1);
            let Some(id) = TxnId::new(&self.next.to_be_bytes()) else {
                continue;
            };
            if !self.txns.contains_key(&id) {
                break id;
            }
        };
        let deadline_ms =
            (self.limits.timeout_ms != 0).then(|| now_ms.saturating_add(self.limits.timeout_ms));
        self.txns.insert(
            id.clone(),
            Txn {
                owner,
                work: Vec::new(),
                doom: None,
                deadline_ms,
            },
        );
        Ok(id)
    }

    /// Whether work can be recorded against `txn`.
    ///
    /// # Errors
    /// `amqp:transaction:unknown-id` for an identifier that names no
    /// transaction; `amqp:transaction:timeout` for one that timed out.
    pub fn check(&self, txn: &TxnId) -> Result<(), AmqpError> {
        match self.txns.get(txn) {
            None => Err(TransactionError::UnknownId.error(format!("no transaction {txn}"))),
            Some(Txn {
                doom: Some((TransactionError::Timeout, reason)),
                ..
            }) => Err(TransactionError::Timeout.error(reason.clone())),
            Some(_) => Ok(()),
        }
    }

    /// The control link that declared `txn`.
    #[must_use]
    pub fn owner(&self, txn: &TxnId) -> Option<&O> {
        self.txns.get(txn).map(|t| &t.owner)
    }

    /// Records work against `txn`.
    ///
    /// # Errors
    /// As [`TxnTable::check`].
    pub fn record(&mut self, txn: &TxnId, work: W) -> Result<(), AmqpError> {
        self.check(txn)?;
        if let Some(t) = self.txns.get_mut(txn) {
            t.work.push(work);
        }
        Ok(())
    }

    /// The work recorded against `txn`; empty for an unknown one.
    #[must_use]
    pub fn work(&self, txn: &TxnId) -> &[W] {
        self.txns.get(txn).map_or(&[], |t| t.work.as_slice())
    }

    /// The work recorded against `txn`, to change it.
    pub fn work_mut(&mut self, txn: &TxnId) -> Option<&mut Vec<W>> {
        self.txns.get_mut(txn).map(|t| &mut t.work)
    }

    /// The transaction can no longer commit (§4.4.4: "it MUST NOT allow
    /// the successful discharge of the associated transaction"): a
    /// discharge without `fail` is refused with
    /// `amqp:transaction:rollback` and `reason`. The first reason stays.
    pub fn doom(&mut self, txn: &TxnId, reason: impl Into<String>) {
        if let Some(t) = self.txns.get_mut(txn)
            && t.doom.is_none()
        {
            t.doom = Some((TransactionError::Rollback, reason.into()));
        }
    }

    /// Discharges `txn` over the control link `owner` (§4.3). The
    /// transaction is gone after it, whatever the result.
    pub fn discharge(&mut self, owner: &O, txn: &TxnId, fail: bool) -> Discharged<W> {
        // A transaction another control link declared is not this link's
        // to discharge: for this link the identifier does not exist.
        if self.txns.get(txn).is_none_or(|t| t.owner != *owner) {
            return Discharged::Refused {
                error: TransactionError::UnknownId.error(format!("no transaction {txn}")),
                work: Vec::new(),
            };
        }
        let Some(t) = self.txns.remove(txn) else {
            return Discharged::Refused {
                error: TransactionError::UnknownId.error(format!("no transaction {txn}")),
                work: Vec::new(),
            };
        };
        if fail {
            return Discharged::Rollback(t.work);
        }
        match t.doom {
            Some((error, reason)) => Discharged::Refused {
                error: error.error(reason),
                work: t.work,
            },
            None => Discharged::Commit(t.work),
        }
    }

    /// The control link `owner` is gone: its transactions are rolled back
    /// (§4.1). Returns each with its work, to undo.
    pub fn close(&mut self, owner: &O) -> Vec<(TxnId, Vec<W>)> {
        let ids: Vec<TxnId> = self
            .txns
            .iter()
            .filter(|(_, t)| t.owner == *owner)
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| self.txns.remove(&id).map(|t| (id, t.work)))
            .collect()
    }

    /// Rolls back the transactions whose time is up at `now_ms`. Returns
    /// each with its work, to undo. The identifier stays known until its
    /// discharge, which is refused with `amqp:transaction:timeout`, as is
    /// further work for it.
    pub fn expire(&mut self, now_ms: u64) -> Vec<(TxnId, Vec<W>)> {
        let timeout_ms = self.limits.timeout_ms;
        let mut expired = Vec::new();
        for (id, t) in &mut self.txns {
            if t.deadline_ms.is_some_and(|deadline| now_ms >= deadline) {
                t.deadline_ms = None;
                t.doom = Some((
                    TransactionError::Timeout,
                    format!("transaction {id} was not discharged within {timeout_ms} ms"),
                ));
                expired.push((id.clone(), core::mem::take(&mut t.work)));
            }
        }
        expired
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    fn table(max: u32, timeout_ms: u64) -> TxnTable<u8, &'static str> {
        TxnTable::new(TxnLimits {
            max_transactions: max,
            timeout_ms,
        })
    }

    /// §4.2, §4.5.4: each declare gets an identifier of its own, of at
    /// most 32 octets; the table declares no more than its bound and says
    /// so with amqp:resource-limit-exceeded; a discharge makes room.
    #[test]
    fn amqp10_4_2_a_declare_allocates_an_identifier_of_its_own() {
        let mut t = table(3, 0);
        assert!(t.is_empty());
        let a = t.declare(1, 0).expect("first");
        let b = t.declare(1, 0).expect("second");
        let c = t.declare(2, 0).expect("third");
        assert!(a != b && b != c && a != c);
        for id in [&a, &b, &c] {
            assert!(id.as_bytes().len() <= TxnId::MAX_LEN);
            assert_eq!(t.check(id), Ok(()));
        }
        assert_eq!(t.len(), 3);
        assert_eq!(t.owner(&c), Some(&2));
        let full = t.declare(1, 0).expect_err("the bound");
        assert_eq!(full.condition, "amqp:resource-limit-exceeded");
        assert_eq!(t.discharge(&1, &a, true), Discharged::Rollback(vec![]));
        let d = t.declare(1, 0).expect("room again");
        assert!(d != a, "an identifier is not handed out twice");
    }

    /// §4.3: a discharge hands the recorded work back, to apply it
    /// (`fail` not set) or to undo it (`fail` set); the transaction is
    /// gone after it. An identifier that names none, or one of another
    /// control link, is refused with amqp:transaction:unknown-id.
    #[test]
    fn amqp10_4_3_a_discharge_hands_back_the_work_to_apply_or_to_undo() {
        let mut t = table(8, 0);
        let commit = t.declare(1, 0).unwrap();
        let rollback = t.declare(1, 0).unwrap();
        t.record(&commit, "post a").unwrap();
        t.record(&commit, "retire b").unwrap();
        t.record(&rollback, "post c").unwrap();
        assert_eq!(t.work(&commit), ["post a", "retire b"]);
        t.work_mut(&commit)
            .expect("work")
            .retain(|w| *w != "retire b");
        t.record(&commit, "retire d").unwrap();
        assert_eq!(
            t.discharge(&1, &commit, false),
            Discharged::Commit(vec!["post a", "retire d"])
        );
        assert_eq!(
            t.discharge(&1, &rollback, true),
            Discharged::Rollback(vec!["post c"])
        );
        assert!(t.is_empty());
        // Discharged once.
        for fail in [false, true] {
            let Discharged::Refused { error, work } = t.discharge(&1, &commit, fail) else {
                panic!("a discharged transaction is unknown");
            };
            assert_eq!(error.condition, "amqp:transaction:unknown-id");
            assert!(work.is_empty());
        }
        let unknown = TxnId::new(b"never declared").unwrap();
        assert_eq!(
            t.check(&unknown).expect_err("unknown").condition,
            "amqp:transaction:unknown-id"
        );
        assert_eq!(
            t.record(&unknown, "x").expect_err("unknown").condition,
            "amqp:transaction:unknown-id"
        );
        assert!(t.work(&unknown).is_empty());
        assert!(t.work_mut(&unknown).is_none());
        // Another control link.
        let own = t.declare(1, 0).unwrap();
        t.record(&own, "post e").unwrap();
        let Discharged::Refused { error, work } = t.discharge(&2, &own, false) else {
            panic!("not the link that declared it");
        };
        assert_eq!(error.condition, "amqp:transaction:unknown-id");
        assert!(work.is_empty());
        assert_eq!(t.work(&own), ["post e"], "untouched");
    }

    /// §4.1: "If the control link is closed while there exist
    /// non-discharged transactions it created, then all such transactions
    /// are immediately rolled back, and attempts to perform further
    /// transactional work on them will lead to failure."
    #[test]
    fn amqp10_4_1_closing_the_control_link_rolls_back_its_transactions() {
        let mut t = table(8, 0);
        let a = t.declare(1, 0).unwrap();
        let b = t.declare(1, 0).unwrap();
        let other = t.declare(2, 0).unwrap();
        t.record(&a, "post a").unwrap();
        t.record(&b, "post b").unwrap();
        t.record(&other, "post o").unwrap();
        let mut rolled_back = t.close(&1);
        rolled_back.sort();
        let mut expected = vec![(a.clone(), vec!["post a"]), (b.clone(), vec!["post b"])];
        expected.sort();
        assert_eq!(rolled_back, expected);
        for id in [&a, &b] {
            assert_eq!(
                t.record(id, "more").expect_err("gone").condition,
                "amqp:transaction:unknown-id"
            );
        }
        assert_eq!(t.work(&other), ["post o"]);
        assert!(t.close(&1).is_empty());
    }

    /// §4.4.4: a transaction whose work the resource could not take "MUST
    /// NOT allow the successful discharge": a discharge without `fail` is
    /// refused with amqp:transaction:rollback and the work is undone; one
    /// with `fail` is completed.
    #[test]
    fn amqp10_4_4_4_a_doomed_transaction_refuses_the_commit_and_takes_the_rollback() {
        let mut t = table(8, 0);
        let a = t.declare(1, 0).unwrap();
        let b = t.declare(1, 0).unwrap();
        t.record(&a, "post a").unwrap();
        t.record(&b, "post b").unwrap();
        t.doom(&a, "node q is full");
        t.doom(&a, "a later reason");
        t.doom(&b, "node q is full");
        assert_eq!(t.check(&a), Ok(()), "work is still recorded");
        let Discharged::Refused { error, work } = t.discharge(&1, &a, false) else {
            panic!("a doomed transaction does not commit");
        };
        assert_eq!(error.condition, "amqp:transaction:rollback");
        assert_eq!(error.description.as_deref(), Some("node q is full"));
        assert_eq!(work, ["post a"]);
        assert_eq!(
            t.discharge(&1, &b, true),
            Discharged::Rollback(vec!["post b"])
        );
    }

    /// §4.5.8 transaction-timeout: a transaction not discharged within
    /// the time-out is rolled back; work for it and its commit are refused
    /// with amqp:transaction:timeout, its rollback is completed. Without a
    /// time-out none expires.
    #[test]
    fn amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout() {
        let mut t = table(8, 1_000);
        let early = t.declare(1, 0).unwrap();
        let late = t.declare(1, 600).unwrap();
        let rolled = t.declare(1, 0).unwrap();
        t.record(&early, "post a").unwrap();
        t.record(&late, "post b").unwrap();
        assert!(t.expire(999).is_empty());
        let mut expired = t.expire(1_000);
        expired.sort();
        let mut expected = vec![(early.clone(), vec!["post a"]), (rolled.clone(), vec![])];
        expected.sort();
        assert_eq!(expired, expected);
        assert!(t.expire(1_001).is_empty(), "expired once");
        assert_eq!(
            t.record(&early, "more").expect_err("timed out").condition,
            "amqp:transaction:timeout"
        );
        assert_eq!(t.record(&late, "more"), Ok(()));
        let Discharged::Refused { error, work } = t.discharge(&1, &early, false) else {
            panic!("a timed-out transaction does not commit");
        };
        assert_eq!(error.condition, "amqp:transaction:timeout");
        assert!(work.is_empty(), "undone when it expired");
        assert_eq!(t.discharge(&1, &rolled, true), Discharged::Rollback(vec![]));
        assert_eq!(t.expire(1_600).len(), 1, "the later one at its own time");
        // No time-out.
        let mut forever = table(8, 0);
        let id = forever.declare(1, 0).unwrap();
        assert!(forever.expire(u64::MAX).is_empty());
        assert_eq!(forever.check(&id), Ok(()));
    }
}
