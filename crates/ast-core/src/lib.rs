// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-ast-core`. Safety classification: **SAFE**.
//!
//! Generic infrastructure for the common AST: generational arenas with
//! typed handles, stable node identity, three-valued references,
//! undo-logged transactions with changeset emission, observers,
//! cycle-safe traversal, incremental indices and a deterministic
//! snapshot codec.
//!
//! This crate knows nothing about DDS, IDL or wire formats. The model
//! itself lives in `zerodds-ast`, wire layout in `zerodds-ast-layout`.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-ast-1.0.md` §1 (storage and identity),
//!   §2 (references), §5 (transactions and events), §6 (traversal),
//!   §9 (snapshot), §10 (error model).
//!
//! ## Layer position
//!
//! Layer 0 — no dependencies. Direct dependents: `zerodds-ast`,
//! `zerodds-ast-layout`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

// The unit-test harness itself needs std; the crate does not.
#[cfg(test)]
extern crate std;

pub mod arena;
pub mod change;
pub mod error;
pub mod id;
pub mod index;
pub mod observer;
pub mod reference;
pub mod snapshot;
pub mod txn;
pub mod walk;

pub use arena::{Arena, Id};
pub use change::{Change, Changeset, FieldTag, FieldValue, NodeKindTag, RefState};
pub use error::AstCoreError;
pub use id::{IdAllocator, IdSpace, NodeId};
pub use index::{Index, IndexLookup};
pub use observer::{AstObserver, ObserverRegistry, ObserverToken};
pub use reference::{
    BreakCause, PendingRef, Ref, RepositoryId, RosInterfaceKind, RosTypeName, ScopedName,
    TypeIdentifierKey, XmlRefKey,
};
pub use snapshot::{SNAPSHOT_MAGIC, SNAPSHOT_VERSION, SnapshotReader, SnapshotWriter};
pub use txn::{TxnLog, UndoPayload, UndoRecord, Undoable};
pub use walk::{VisitSet, WalkOrder, reachable, walk_graph};
