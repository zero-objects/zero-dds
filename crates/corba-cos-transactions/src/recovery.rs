// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Durability + Recovery (OMG OTS §10.3.7 `RecoveryCoordinator`).
//!
//! Crash recoverability requires the coordinator to **durably commit its commit
//! decision before** executing phase 2. If it crashes between the decision and
//! phase 2, the [`RecoveryCoordinator`] finds the "in doubt" transactions left
//! in the [`TransactionLog`] after restart and drives their resources to the
//! logged decision (resync).
//!
//! Persistence itself is abstracted behind the [`TransactionLog`] trait (the
//! application supplies a file/DB implementation); [`InMemoryLog`] is the
//! bundled RAM variant. Unknown/incomplete transactions are treated as
//! **presumed-abort** (OMG default).

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::otid::Otid;
use crate::two_phase::{Completion, HeuristicOutcome, Resource, Vote};

/// The durably logged decision of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxOutcome {
    /// The transaction shall commit.
    Commit,
    /// The transaction shall roll back.
    Rollback,
}

/// Persisted state of a transaction (OMG OTS recovery log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogState {
    /// Prepare in progress — no coordinator decision yet. A crash here →
    /// presumed-abort (rollback).
    Preparing,
    /// Decision made + durably written, phase 2 pending. This is the
    /// force-write point: from here on the transaction is recoverable.
    Decided(TxOutcome),
    /// Phase 2 completed.
    Completed(TxOutcome),
}

/// Persistence abstraction for the transaction/heuristic log (OMG OTS §10.3.7).
pub trait TransactionLog {
    /// Force-writes the state of a transaction.
    fn record(&mut self, otid: &Otid, state: LogState);
    /// Reads the logged state.
    fn get(&self, otid: &Otid) -> Option<LogState>;
    /// Forgets the log entry (after completion + resource `forget`).
    fn forget(&mut self, otid: &Otid);
    /// All "in doubt" transactions: decision written, but phase 2
    /// not yet logged as completed.
    fn in_doubt(&self) -> Vec<(Otid, TxOutcome)>;
}

/// Key of a log entry: `(formatID, tid)` identifies the transaction.
fn key(otid: &Otid) -> (i32, Vec<u8>) {
    (otid.format_id, otid.tid.clone())
}

/// In-memory [`TransactionLog`] (default; for tests + non-persistent use).
#[derive(Debug, Default)]
pub struct InMemoryLog {
    records: BTreeMap<(i32, Vec<u8>), (Otid, LogState)>,
}

impl InMemoryLog {
    /// Empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of log entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl TransactionLog for InMemoryLog {
    fn record(&mut self, otid: &Otid, state: LogState) {
        self.records.insert(key(otid), (otid.clone(), state));
    }

    fn get(&self, otid: &Otid) -> Option<LogState> {
        self.records.get(&key(otid)).map(|(_, s)| *s)
    }

    fn forget(&mut self, otid: &Otid) {
        self.records.remove(&key(otid));
    }

    fn in_doubt(&self) -> Vec<(Otid, TxOutcome)> {
        self.records
            .values()
            .filter_map(|(o, s)| match s {
                LogState::Decided(out) => Some((o.clone(), *out)),
                _ => None,
            })
            .collect()
    }
}

/// Drives a 2-phase commit **with durability**: logs `Preparing` →
/// `Decided` (force-write of the decision *before* phase 2) → `Completed`. If
/// the process crashes between `Decided` and `Completed`, the
/// [`RecoveryCoordinator`] resolves the transaction after restart.
///
/// Semantics identical to [`coordinate_commit`](crate::two_phase::coordinate_commit),
/// only with additional log writes.
pub fn coordinate_commit_durable(
    otid: &Otid,
    resources: &[Rc<dyn Resource>],
    log: &mut dyn TransactionLog,
) -> Completion {
    log.record(otid, LogState::Preparing);

    match resources.len() {
        0 => {
            log.record(otid, LogState::Completed(TxOutcome::Commit));
            Completion::Committed
        }
        1 => {
            log.record(otid, LogState::Decided(TxOutcome::Commit));
            let (completion, outcome) = match resources[0].commit_one_phase() {
                Ok(()) => (Completion::Committed, TxOutcome::Commit),
                Err(HeuristicOutcome::Rollback) => (Completion::RolledBack, TxOutcome::Rollback),
                Err(_) => (Completion::HeuristicMixed, TxOutcome::Commit),
            };
            log.record(otid, LogState::Completed(outcome));
            completion
        }
        _ => {
            let votes: Vec<Vote> = resources.iter().map(|r| r.prepare()).collect();
            let outcome = if votes.contains(&Vote::Rollback) {
                TxOutcome::Rollback
            } else {
                TxOutcome::Commit
            };
            // Force-write the decision BEFORE phase 2 — the durability point.
            log.record(otid, LogState::Decided(outcome));

            let completion = execute_phase_two(resources, &votes, outcome);
            log.record(otid, LogState::Completed(outcome));
            completion
        }
    }
}

/// Phase 2 for the durable multi-resource 2PC.
fn execute_phase_two(
    resources: &[Rc<dyn Resource>],
    votes: &[Vote],
    outcome: TxOutcome,
) -> Completion {
    match outcome {
        TxOutcome::Rollback => {
            for (r, v) in resources.iter().zip(votes) {
                if *v == Vote::Commit {
                    let _ = r.rollback();
                }
            }
            Completion::RolledBack
        }
        TxOutcome::Commit => {
            let mut heuristic = false;
            for (r, v) in resources.iter().zip(votes) {
                if *v == Vote::Commit {
                    match r.commit() {
                        Ok(()) | Err(HeuristicOutcome::Commit) => {}
                        Err(_) => {
                            heuristic = true;
                            r.forget();
                        }
                    }
                }
            }
            if heuristic {
                Completion::HeuristicMixed
            } else {
                Completion::Committed
            }
        }
    }
}

/// Supplies the resources of a transaction re-instantiated after a restart
/// (OMG OTS: the recovery mechanism reconstructs the in-doubt resources).
pub trait ResourceResolver {
    /// Resources belonging to transaction `otid`.
    fn resources_for(&self, otid: &Otid) -> Vec<Rc<dyn Resource>>;
}

/// Recovery coordinator (OMG OTS §10.3.7): after a restart, resolves the
/// transactions left "in doubt" in the log.
pub struct RecoveryCoordinator<'a> {
    log: &'a mut dyn TransactionLog,
}

impl<'a> RecoveryCoordinator<'a> {
    /// New recovery coordinator over a (persistent) log.
    pub fn new(log: &'a mut dyn TransactionLog) -> Self {
        Self { log }
    }

    /// OMG `replay_completion`: the logged decision for an in-doubt resource so
    /// it can resynchronize. If nothing (or only `Preparing`) is logged,
    /// **presumed-abort** applies → [`TxOutcome::Rollback`].
    #[must_use]
    pub fn replay_completion(&self, otid: &Otid) -> TxOutcome {
        match self.log.get(otid) {
            Some(LogState::Decided(o) | LogState::Completed(o)) => o,
            _ => TxOutcome::Rollback,
        }
    }

    /// Recovery run: drives each in-doubt transaction, via its resources
    /// (supplied by `resolver`), to the logged decision and marks it as
    /// `Completed`. Returns the resolved transactions.
    pub fn recover(&mut self, resolver: &dyn ResourceResolver) -> Vec<(Otid, TxOutcome)> {
        let pending = self.log.in_doubt();
        for (otid, outcome) in &pending {
            let resources = resolver.resources_for(otid);
            for r in &resources {
                let _ = match outcome {
                    TxOutcome::Commit => r.commit(),
                    TxOutcome::Rollback => r.rollback(),
                };
            }
            self.log.record(otid, LogState::Completed(*outcome));
        }
        pending
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct Res {
        vote: Vote,
        committed: Cell<u32>,
        rolled_back: Cell<u32>,
    }
    impl Res {
        fn new(vote: Vote) -> Rc<Self> {
            Rc::new(Self {
                vote,
                committed: Cell::new(0),
                rolled_back: Cell::new(0),
            })
        }
    }
    impl Resource for Res {
        fn prepare(&self) -> Vote {
            self.vote
        }
        fn commit(&self) -> Result<(), HeuristicOutcome> {
            self.committed.set(self.committed.get() + 1);
            Ok(())
        }
        fn rollback(&self) -> Result<(), HeuristicOutcome> {
            self.rolled_back.set(self.rolled_back.get() + 1);
            Ok(())
        }
        fn commit_one_phase(&self) -> Result<(), HeuristicOutcome> {
            self.committed.set(self.committed.get() + 1);
            Ok(())
        }
    }

    fn otid(n: u8) -> Otid {
        Otid::new(0, alloc::vec![n])
    }

    #[test]
    fn durable_commit_logs_decision_then_completion() {
        let mut log = InMemoryLog::new();
        let o = otid(1);
        let res: Vec<Rc<dyn Resource>> =
            alloc::vec![Res::new(Vote::Commit), Res::new(Vote::Commit)];
        assert_eq!(
            coordinate_commit_durable(&o, &res, &mut log),
            Completion::Committed
        );
        assert_eq!(log.get(&o), Some(LogState::Completed(TxOutcome::Commit)));
    }

    #[test]
    fn durable_rollback_decision_logged() {
        let mut log = InMemoryLog::new();
        let o = otid(2);
        let res: Vec<Rc<dyn Resource>> =
            alloc::vec![Res::new(Vote::Commit), Res::new(Vote::Rollback)];
        assert_eq!(
            coordinate_commit_durable(&o, &res, &mut log),
            Completion::RolledBack
        );
        assert_eq!(log.get(&o), Some(LogState::Completed(TxOutcome::Rollback)));
    }

    /// Resolver that supplies fixed resources for a known otid.
    struct OneTx {
        otid: Otid,
        resources: Vec<Rc<dyn Resource>>,
    }
    impl ResourceResolver for OneTx {
        fn resources_for(&self, otid: &Otid) -> Vec<Rc<dyn Resource>> {
            if otid == &self.otid {
                self.resources.clone()
            } else {
                Vec::new()
            }
        }
    }

    #[test]
    fn recovery_completes_in_doubt_commit() {
        // Crash simulation: log ends at `Decided(Commit)` (phase 2 never ran).
        let mut log = InMemoryLog::new();
        let o = otid(7);
        log.record(&o, LogState::Decided(TxOutcome::Commit));

        let r1 = Res::new(Vote::Commit);
        let r2 = Res::new(Vote::Commit);
        let resolver = OneTx {
            otid: o.clone(),
            resources: alloc::vec![r1.clone(), r2.clone()],
        };

        let recovered = {
            let mut rc = RecoveryCoordinator::new(&mut log);
            rc.recover(&resolver)
        };
        assert_eq!(recovered, alloc::vec![(o.clone(), TxOutcome::Commit)]);
        // Resources were committed after the fact.
        assert_eq!(r1.committed.get(), 1);
        assert_eq!(r2.committed.get(), 1);
        // Log now completed → no more in-doubt.
        assert_eq!(log.get(&o), Some(LogState::Completed(TxOutcome::Commit)));
        assert!(log.in_doubt().is_empty());
    }

    #[test]
    fn recovery_completes_in_doubt_rollback() {
        let mut log = InMemoryLog::new();
        let o = otid(8);
        log.record(&o, LogState::Decided(TxOutcome::Rollback));
        let r = Res::new(Vote::Commit);
        let resolver = OneTx {
            otid: o.clone(),
            resources: alloc::vec![r.clone()],
        };
        let mut rc = RecoveryCoordinator::new(&mut log);
        rc.recover(&resolver);
        assert_eq!(r.rolled_back.get(), 1);
        assert_eq!(r.committed.get(), 0);
    }

    #[test]
    fn replay_completion_returns_logged_decision() {
        let mut log = InMemoryLog::new();
        let o = otid(3);
        log.record(&o, LogState::Decided(TxOutcome::Commit));
        let rc = RecoveryCoordinator::new(&mut log);
        assert_eq!(rc.replay_completion(&o), TxOutcome::Commit);
    }

    #[test]
    fn replay_completion_presumed_abort_for_unknown() {
        let mut log = InMemoryLog::new();
        let rc = RecoveryCoordinator::new(&mut log);
        // Unknown transaction → presumed-abort.
        assert_eq!(rc.replay_completion(&otid(99)), TxOutcome::Rollback);
    }

    #[test]
    fn preparing_state_is_presumed_abort() {
        let mut log = InMemoryLog::new();
        let o = otid(4);
        log.record(&o, LogState::Preparing); // crash before decision
        // Not "in doubt" (no decision) → recovery leaves it untouched.
        assert!(log.in_doubt().is_empty());
        let rc = RecoveryCoordinator::new(&mut log);
        assert_eq!(rc.replay_completion(&o), TxOutcome::Rollback);
    }

    #[test]
    fn single_resource_durable_one_phase() {
        let mut log = InMemoryLog::new();
        let o = otid(5);
        let r = Res::new(Vote::Commit);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![r.clone()];
        assert_eq!(
            coordinate_commit_durable(&o, &res, &mut log),
            Completion::Committed
        );
        assert_eq!(r.committed.get(), 1);
        assert_eq!(log.get(&o), Some(LogState::Completed(TxOutcome::Commit)));
    }
}
