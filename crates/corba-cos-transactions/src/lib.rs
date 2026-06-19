// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-transactions`. Safety classification: **STANDARD**.
//!
//! OMG **Object Transaction Service** (OTS / `CosTransactions`) 1.4
//! (`formal/2003-09-02`) — pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
//! The OTS brings distributed transactions (ACID) into the CORBA model and is the
//! core lever for the "drop-in for legacy finance systems" migration: 2-phase
//! commit across multiple servers, with transaction-context propagation via the
//! GIOP service context `TransactionService` (id = 0).
//!
//! Implements:
//! - **`otid_t`** (§ OTS Appendix A) — the cross-ORB transaction
//!   identifier; byte-exact CDR codec ([`otid`]).
//! - **`PropagationContext`** + `TransactionService` service context (id = 0) —
//!   transaction-context propagation over GIOP ([`propagation`]).
//! - **2-phase commit protocol** (`prepare`/`commit`/`rollback`/
//!   `commit_one_phase`/`forget`) with a vote-driven state machine, incl.
//!   read-only optimization + one-phase commit ([`two_phase`]).
//! - **`Current` / `Coordinator` / `Terminator` / `Control`** — the OTS
//!   orchestration interfaces as Rust types ([`transaction`]).
//! - **`TransactionLog` / `RecoveryCoordinator`** (§10.3.7) — durability: the
//!   commit decision is force-written before phase 2; a restart resolves
//!   in-doubt transactions as presumed-abort ([`recovery`]).
//!
//! Spec: OMG Transaction Service 1.4. Real-Time CORBA + CosTrading are separate
//! (Trading subordinate, RT-CORBA covered via DDS-QoS).

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

pub mod otid;
pub mod propagation;
pub mod recovery;
pub mod transaction;
pub mod two_phase;

pub use otid::Otid;
pub use propagation::{PropagationContext, TRANSACTION_SERVICE_CONTEXT_ID, TransIdentity};
pub use recovery::{
    InMemoryLog, LogState, RecoveryCoordinator, ResourceResolver, TransactionLog, TxOutcome,
    coordinate_commit_durable,
};
pub use transaction::{Control, Coordinator, Current, Status, Terminator, TransactionError};
pub use two_phase::{Completion, HeuristicOutcome, Resource, Vote, coordinate_commit};
