// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS Data-Local-Reconstruction-Layer (DLRL) — DDS 1.4 §2.2 + §B.
//!
//! Crate `zerodds-dlrl`. Safety classification: **STANDARD**.
//!
//! DLRL (Data Local Reconstruction Layer) is an optional DDS profile
//! that provides an object-centric cache on top of DCPS with identity
//! tracking, relationship resolution, and transactional updates
//! (Spec §2.1.3).
//!
//! # Modules
//!
//! * [`object_cache`] — object cache with identity tracking + WeakRef
//!   (Spec §B.2 + §B.6).
//! * [`transaction`] — transaction semantics with optimistic concurrency
//!   (Spec §B.7.4).
//! * [`relationship`] — relationship resolver (mono/bidirectional,
//!   compositional/referential, cascaded update/delete) (Spec §B.5).
//! * [`subscription`] — HomeFactory/HomeListener/ObjectListener
//!   (Spec §B.3 + §B.6).
//! * [`query`] — query engine with filter/order/limit (Spec §B.7).
//! * [`pragma`] — DLRL pragma parser (`#pragma DCPS_DATA_TYPE` etc.).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod metamodel;
pub mod object_cache;
pub mod pragma;
pub mod query;
pub mod relationship;
pub mod subscription;
pub mod transaction;

pub use object_cache::{ObjectCache, ObjectId, ObjectRef, WeakObjectRef};
pub use pragma::{DlrlPragma, ParsePragmaError, parse_pragma};
pub use query::{Query, QueryError, QueryResult, SortOrder};
pub use relationship::{
    CascadeMode, Direction, Relationship, RelationshipKind, RelationshipResolver,
};
pub use subscription::{
    HomeFactory, HomeListener, ObjectChangeKind, ObjectListener, SubscriptionRegistry,
};
pub use transaction::{
    ConsistencyLevel, Transaction, TransactionError, TransactionId, TransactionState,
};
