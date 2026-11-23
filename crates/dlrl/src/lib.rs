// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS Data Local Reconstruction Layer (DLRL) — DDS 1.2 §8 and Annex B.
//!
//! Crate `zerodds-dlrl`. Safety classification: **STANDARD**.
//!
//! DLRL (Data Local Reconstruction Layer) is an optional DDS profile
//! that provides an object-centric cache on top of DCPS (Spec §2.1.3).
//! This crate describes object models with their relations
//! ([`model`]), maps them onto DCPS topics and rows ([`mapping`],
//! [`rows`]), and holds objects in a cache with identity tracking
//! ([`object_cache`]) and transactional updates ([`transaction`]).
//!
//! # Modules
//!
//! * [`object_cache`] — an object cache with identity tracking and weak
//!   references, older than the run-time entities of [`cache`] and not a
//!   type of DDS 1.2 §8.
//! * [`transaction`] — transactions with optimistic concurrency over
//!   [`object_cache`], not a type of DDS 1.2 §8.
//! * [`model`] — the shared part of a DLRL object model: classes with
//!   single inheritance, attributes, relations with inverses and
//!   compositions, the type enforcement of values (DDS 1.2 §8.1.3).
//! * [`mapping`] — the structural mapping onto DCPS topics, with the
//!   default mapping and the topic descriptions (DDS 1.2 §8.1.4).
//! * [`rows`] — the rows of the DCPS tables of one object, and objects
//!   reconstructed from rows (DDS 1.2 §8.1.4.2).
//! * [`types`] — `DLRLOid` and the other value types of the PSM
//!   (DDS 1.2 §8.2.1.2.1).
//! * [`error`] — `DlrlException`, the exceptions of DDS 1.2 §8.1.6.2.
//! * [`model_tags`] (feature `model-tags`) — the XML model tags of
//!   DDS 1.2 §8.2.2.3 under the DTD of §8.2.2.3.1.
//! * [`generation`] (feature `model-tags`) — the generator of §8.2.2.1:
//!   model description and model tags give model and mapping.
//! * [`cache`] (feature `dcps`) — the run-time entities over
//!   `zerodds-dcps`: `CacheFactory`, `Cache`, `CacheAccess`, `Contract`,
//!   `ObjectHome`, `ObjectRoot`, `CacheListener`, `ObjectListener`,
//!   `Selection`, `SelectionCriterion`, `FilterCriterion`,
//!   `QueryCriterion`, `SelectionListener` and the collections
//!   `Collection`, `List`, `Set`, `StrMap`, `IntMap`, with the
//!   operational mapping of §8.1.5 and the update rounds of §8.1.6.
//! * [`dcps_binding`] (feature `dcps`) — the DCPS data type of each
//!   topic of a mapping and the conversion of rows into samples.
//! * [`criterion`] — queries and filters in the syntax of DDS 1.2
//!   Annex B (`QueryExpression`), for `QueryCriterion` and the
//!   `content_filter` of an `ObjectHome`.
//! * [`pragma`] — DLRL pragma parser (`#pragma DCPS_DATA_TYPE` etc.).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "dcps")]
pub mod cache;
pub mod criterion;
#[cfg(feature = "dcps")]
pub mod dcps_binding;
pub mod error;
#[cfg(feature = "model-tags")]
pub mod generation;
pub mod mapping;
pub mod metamodel;
pub mod model;
#[cfg(feature = "model-tags")]
pub mod model_tags;
pub mod object_cache;
pub mod pragma;
pub mod rows;
pub mod transaction;
pub mod types;

pub use criterion::{QueryExpression, QueryGrammar};
pub use mapping::{
    ClassMapping, ClassMappingSpec, KeyContent, KeyDescription, Mapping, MappingError,
    MemberMapping, MemberMappingSpec, MultiTopicSpec, TopicDescription, TopicSpec,
};
pub use model::{
    AttributeType, AttributeValue, BasicType, ClassModel, CollectionBasis, Model, ModelError,
    ObjectRef as DlrlObjectRef, Relation, SimpleType, TypeError, Value,
};
pub use object_cache::{ObjectCache, ObjectId, ObjectRef, WeakObjectRef};
pub use pragma::{DlrlPragma, ParsePragmaError, parse_pragma};
pub use rows::{Cell, ObjectData, Reconstruction, Row, RowError};
pub use transaction::{
    ConsistencyLevel, Transaction, TransactionError, TransactionId, TransactionState,
};
pub use types::{
    CriterionKind, DlrlOid, MembershipState, ReferenceScope, RelationDescription, RelationKind,
};

pub use error::DlrlException;
pub use rows::KeyAssociation;

#[cfg(feature = "dcps")]
pub use cache::{
    Cache, CacheAccess, CacheBase, CacheDescription, CacheFactory, CacheKind, CacheListener,
    CacheUsage, Collection, Contract, DcpsState, FilterCriterion, IntMap, List, ObjectHome,
    ObjectListener, ObjectRoot, ObjectScope, ObjectState, QueryCriterion, Selection,
    SelectionCriterion, SelectionListener, Set, StrMap, UNLIMITED_RELATED_OBJECTS,
};
