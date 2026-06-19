// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! 2-phase commit protocol (OMG OTS §10.3.2 `Resource`).
//!
//! A [`Resource`] is a transactional participant (e.g. a database connection).
//! The coordinator drives the 2PC:
//!
//! 1. **Prepare** (phase 1): each resource votes [`Vote::Commit`],
//!    [`Vote::Rollback`] or [`Vote::ReadOnly`].
//! 2. **Commit/Rollback** (phase 2): if *every* resource votes
//!    `Commit`/`ReadOnly`, all `Commit` resources are committed (ReadOnly are
//!    done); if *one* votes `Rollback`, all already-prepared ones are rolled
//!    back.
//!
//! Optimizations: **read-only** (a resource with no changes is skipped in phase
//! 2) and **one-phase commit** (with exactly one resource, prepare is skipped).

use alloc::rc::Rc;
use alloc::vec::Vec;

/// Vote of a [`Resource`] in the prepare phase (OMG OTS §10.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    /// Ready to commit (phase 2 decides).
    Commit,
    /// Demands rollback of the entire transaction.
    Rollback,
    /// No changes — skipped in phase 2.
    ReadOnly,
}

/// Heuristic outcome when a resource decided on its own
/// (OMG OTS §10.3.2 `HeuristicMixed`/`HeuristicHazard`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeuristicOutcome {
    /// Partly committed, partly rolled back — data inconsistent.
    Mixed,
    /// Heuristically rolled back.
    Rollback,
    /// Heuristically committed.
    Commit,
    /// Outcome possibly inconsistent (unknown).
    Hazard,
}

/// A transactional participant in the 2-phase commit (OMG OTS §10.3.2).
///
/// `commit`/`rollback`/`commit_one_phase` default to success without heuristics;
/// `prepare` is mandatory.
pub trait Resource {
    /// Phase 1: checks whether the resource can commit and, if so, persists a
    /// prepare record.
    fn prepare(&self) -> Vote;

    /// Phase 2: makes the prepared changes durable.
    ///
    /// # Errors
    /// Heuristic deviation from the coordinator decision.
    fn commit(&self) -> Result<(), HeuristicOutcome> {
        Ok(())
    }

    /// Phase 2: discards the prepared changes.
    ///
    /// # Errors
    /// Heuristic deviation.
    fn rollback(&self) -> Result<(), HeuristicOutcome> {
        Ok(())
    }

    /// One-phase commit optimization (single resource): commits without prepare.
    ///
    /// # Errors
    /// Heuristic deviation / rollback forced.
    fn commit_one_phase(&self) -> Result<(), HeuristicOutcome> {
        Ok(())
    }

    /// Allows the resource to forget a heuristic record (after `commit`/
    /// `rollback` with a heuristic).
    fn forget(&self) {}
}

/// Outcome of the 2-phase commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// All resources committed (or ReadOnly).
    Committed,
    /// Transaction rolled back (veto in phase 1, or empty/forced).
    RolledBack,
    /// Heuristically inconsistent — at least one resource deviated.
    HeuristicMixed,
}

/// Drives the 2-phase commit over `resources` and returns the outcome.
///
/// * empty set → [`Completion::Committed`] (nothing to do),
/// * one resource → one-phase commit,
/// * otherwise prepare phase, then commit or rollback phase depending on the votes.
#[must_use]
pub fn coordinate_commit(resources: &[Rc<dyn Resource>]) -> Completion {
    match resources.len() {
        0 => Completion::Committed,
        1 => match resources[0].commit_one_phase() {
            Ok(()) => Completion::Committed,
            Err(HeuristicOutcome::Rollback) => Completion::RolledBack,
            Err(_) => Completion::HeuristicMixed,
        },
        _ => two_phase(resources),
    }
}

/// Drives a forced rollback over all resources (e.g. after `rollback_only`
/// or a timeout) — no prepare phase.
pub fn coordinate_rollback(resources: &[Rc<dyn Resource>]) {
    for r in resources {
        let _ = r.rollback();
    }
}

fn two_phase(resources: &[Rc<dyn Resource>]) -> Completion {
    // Phase 1: prepare. Remember the votes so phase 2 only touches Commit voters.
    let votes: Vec<Vote> = resources.iter().map(|r| r.prepare()).collect();

    if votes.contains(&Vote::Rollback) {
        // Veto: roll back all resources already prepared with Commit.
        for (r, v) in resources.iter().zip(&votes) {
            if *v == Vote::Commit {
                let _ = r.rollback();
            }
        }
        return Completion::RolledBack;
    }

    // All Commit or ReadOnly → phase 2: commit only the Commit voters.
    let mut heuristic = false;
    for (r, v) in resources.iter().zip(&votes) {
        if *v == Vote::Commit {
            match r.commit() {
                Ok(()) => {}
                Err(HeuristicOutcome::Commit) => {} // committed as expected
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use core::cell::Cell;

    /// Test resource that dictates its vote and counts prepare/commit/rollback.
    struct FakeResource {
        vote: Vote,
        prepared: Cell<u32>,
        committed: Cell<u32>,
        rolled_back: Cell<u32>,
        one_phase: Cell<u32>,
        heuristic: Option<HeuristicOutcome>,
    }
    impl FakeResource {
        fn new(vote: Vote) -> Rc<Self> {
            Rc::new(Self {
                vote,
                prepared: Cell::new(0),
                committed: Cell::new(0),
                rolled_back: Cell::new(0),
                one_phase: Cell::new(0),
                heuristic: None,
            })
        }
    }
    impl Resource for FakeResource {
        fn prepare(&self) -> Vote {
            self.prepared.set(self.prepared.get() + 1);
            self.vote
        }
        fn commit(&self) -> Result<(), HeuristicOutcome> {
            self.committed.set(self.committed.get() + 1);
            self.heuristic.map_or(Ok(()), Err)
        }
        fn rollback(&self) -> Result<(), HeuristicOutcome> {
            self.rolled_back.set(self.rolled_back.get() + 1);
            Ok(())
        }
        fn commit_one_phase(&self) -> Result<(), HeuristicOutcome> {
            self.one_phase.set(self.one_phase.get() + 1);
            match self.vote {
                Vote::Rollback => Err(HeuristicOutcome::Rollback),
                _ => Ok(()),
            }
        }
    }

    #[test]
    fn all_commit_commits_everyone() {
        let a = FakeResource::new(Vote::Commit);
        let b = FakeResource::new(Vote::Commit);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone(), b.clone()];
        assert_eq!(coordinate_commit(&res), Completion::Committed);
        assert_eq!((a.prepared.get(), a.committed.get()), (1, 1));
        assert_eq!((b.prepared.get(), b.committed.get()), (1, 1));
    }

    #[test]
    fn one_rollback_vote_rolls_back_prepared() {
        let a = FakeResource::new(Vote::Commit);
        let b = FakeResource::new(Vote::Rollback);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone(), b.clone()];
        assert_eq!(coordinate_commit(&res), Completion::RolledBack);
        // a had voted Commit → must be rolled back; nobody committed.
        assert_eq!(a.rolled_back.get(), 1);
        assert_eq!(a.committed.get(), 0);
        // b voted Rollback → no commit, no rollback needed (nothing prepared).
        assert_eq!(b.committed.get(), 0);
    }

    #[test]
    fn read_only_skips_phase_two() {
        let a = FakeResource::new(Vote::Commit);
        let b = FakeResource::new(Vote::ReadOnly);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone(), b.clone()];
        assert_eq!(coordinate_commit(&res), Completion::Committed);
        assert_eq!(a.committed.get(), 1);
        // ReadOnly is skipped in phase 2.
        assert_eq!(b.committed.get(), 0);
        assert_eq!(b.rolled_back.get(), 0);
    }

    #[test]
    fn single_resource_uses_one_phase() {
        let a = FakeResource::new(Vote::Commit);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone()];
        assert_eq!(coordinate_commit(&res), Completion::Committed);
        assert_eq!(a.one_phase.get(), 1);
        assert_eq!(a.prepared.get(), 0); // no prepare in one-phase
        assert_eq!(a.committed.get(), 0);
    }

    #[test]
    fn single_resource_one_phase_rollback() {
        let a = FakeResource::new(Vote::Rollback);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone()];
        assert_eq!(coordinate_commit(&res), Completion::RolledBack);
    }

    #[test]
    fn empty_set_commits_trivially() {
        let res: Vec<Rc<dyn Resource>> = Vec::new();
        assert_eq!(coordinate_commit(&res), Completion::Committed);
    }

    #[test]
    fn heuristic_commit_failure_reports_mixed() {
        let a = Rc::new(FakeResource {
            vote: Vote::Commit,
            prepared: Cell::new(0),
            committed: Cell::new(0),
            rolled_back: Cell::new(0),
            one_phase: Cell::new(0),
            heuristic: Some(HeuristicOutcome::Mixed),
        });
        let b = FakeResource::new(Vote::Commit);
        let res: Vec<Rc<dyn Resource>> = alloc::vec![a.clone(), b.clone()];
        assert_eq!(coordinate_commit(&res), Completion::HeuristicMixed);
    }
}
