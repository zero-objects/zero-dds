// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-qos`. Safety classification: **SAFE**.
//!
//! DDS QoS-Policies (DDS 1.4 §2.2.3) + Request/Offered-Compatibility-Matrix
//! + PL_CDR_LE PID-Wire-Codec (DDSI-RTPS §9.6.3.2). Pure-Rust no_std + alloc.
//!
//! ## Spec
//!
//! - **DDS 1.4** §2.2.3 — all 22 standard policies (Durability, Reliability,
//!   Liveliness, Ownership, Partition, …) incl. §2.2.3.23
//!   exclusive-ownership resolver logic (§2.2.2.5.5).
//! - **DDS 1.4** §2.2.3 Table "QoS compatibility" — request/offered matrix
//!   ([`check_compatibility`]).
//! - **DDSI-RTPS 2.5** §9.6.3.2 — wire PIDs for ParameterList encoding
//!   ([`Pid`]).
//!
//! ## Layer position
//!
//! Layer 1 — primitives. Direct dependents: `zerodds-rtps`, `zerodds-discovery`,
//! `zerodds-dcps`, `zerodds-dcps-async`, `zerodds-c-api`, `zerodds-rpc`, `zerodds-security-runtime`,
//! `zerodds-xml`, `zerodds-zenoh-bridge`.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! **Top level:**
//! - [`Duration`] — DDS duration (sec + nanosec) for all time-based policies.
//! - [`Pid`] — DDSI-RTPS §9.6.3.2 PID constants for QoS-policy wire encoding.
//! - [`CompatibilityResult`] / [`IncompatibleReason`] / [`check_compatibility`]
//!   — request/offered matrix.
//!
//! **Policies module** ([`policies`]):
//! - 22 standard policies: [`DurabilityQosPolicy`], [`DurabilityServiceQosPolicy`],
//!   [`DeadlineQosPolicy`], [`LatencyBudgetQosPolicy`], [`LivelinessQosPolicy`],
//!   [`ReliabilityQosPolicy`], [`DestinationOrderQosPolicy`], [`HistoryQosPolicy`],
//!   [`ResourceLimitsQosPolicy`], [`TransportPriorityQosPolicy`], [`LifespanQosPolicy`],
//!   [`UserDataQosPolicy`], [`OwnershipQosPolicy`], [`OwnershipStrengthQosPolicy`],
//!   [`PresentationQosPolicy`], [`PartitionQosPolicy`], [`TopicDataQosPolicy`],
//!   [`GroupDataQosPolicy`], [`TimeBasedFilterQosPolicy`],
//!   [`ReaderDataLifecycleQosPolicy`], [`WriterDataLifecycleQosPolicy`],
//!   [`EntityFactoryQosPolicy`].
//! - QoS aggregates: [`ReaderQos`], [`WriterQos`].
//! - Kind enums: [`DurabilityKind`], [`ReliabilityKind`], [`LivelinessKind`],
//!   [`OwnershipKind`], [`HistoryKind`], [`DestinationOrderKind`],
//!   [`PresentationAccessScope`].
//!
//! **Exclusive-Ownership-Resolver** ([`exclusive_ownership`]):
//! - [`exclusive_ownership::OwnershipResolver`] /
//!   [`exclusive_ownership::OwnershipCandidate`] /
//!   [`exclusive_ownership::resolve_strongest`] —
//!   DDS 1.4 §2.2.3.23 strongest-writer selection for DataReader.take()
//!   on topics with `Exclusive` ownership QoS.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_qos::{
//!     ReliabilityKind, ReliabilityQosPolicy, ReaderQos, WriterQos,
//!     check_compatibility, CompatibilityResult,
//! };
//!
//! let mut writer_qos = WriterQos::default();
//! writer_qos.reliability = ReliabilityQosPolicy { kind: ReliabilityKind::Reliable, ..Default::default() };
//! let reader_qos = ReaderQos::default();
//! // Reliable writer + BestEffort reader (default) are compatible.
//! assert!(matches!(check_compatibility(&writer_qos, &reader_qos), CompatibilityResult::Compatible));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// `zerodds-qos` strictly requires `alloc` (partition strings,
// GenericData buffers, ResourceLimits vec, …). `zerodds-cdr` with
// the `alloc` feature is a mandatory dep, so `extern crate alloc`
// is always available.
extern crate alloc;

pub mod compatibility;
mod defaults;
pub mod duration;
pub mod exclusive_ownership;
pub mod pid;
pub mod policies;
#[cfg(test)]
mod review_tests;
mod wire_helpers;

pub use compatibility::{CompatibilityResult, IncompatibleReason};
pub use duration::Duration;
pub use pid::Pid;
pub use policies::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, EntityFactoryQosPolicy, GroupDataQosPolicy,
    HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessKind,
    LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    ReaderDataLifecycleQosPolicy, ReaderQos, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy, WriterQos,
    check_compatibility,
};
