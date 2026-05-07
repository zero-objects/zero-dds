// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-qos`. Safety classification: **SAFE**.
//!
//! DDS QoS-Policies (DDS 1.4 §2.2.3) + Request/Offered-Compatibility-Matrix
//! + PL_CDR_LE PID-Wire-Codec (DDSI-RTPS §9.6.3.2). Pure-Rust no_std + alloc.
//!
//! ## Spec
//!
//! - **DDS 1.4** §2.2.3 — alle 22 Standard-Policies (Durability, Reliability,
//!   Liveliness, Ownership, Partition, …) inkl. §2.2.3.23
//!   Exclusive-Ownership-Resolver-Logik (§2.2.2.5.5).
//! - **DDS 1.4** §2.2.3 Table "QoS compatibility" — Request/Offered-Matrix
//!   ([`check_compatibility`]).
//! - **DDSI-RTPS 2.5** §9.6.3.2 — Wire-PIDs fuer ParameterList-Encoding
//!   ([`Pid`]).
//!
//! ## Schichten-Position
//!
//! Layer 1 — Primitives. Direkte Abhaengige: `zerodds-rtps`, `zerodds-discovery`,
//! `zerodds-dcps`, `zerodds-dcps-async`, `zerodds-c-api`, `zerodds-rpc`, `zerodds-security-runtime`,
//! `zerodds-xml`, `zerodds-zenoh-bridge`.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! **Top-Level:**
//! - [`Duration`] — DDS-Duration (sec + nanosec) fuer alle zeitbasierten Policies.
//! - [`Pid`] — DDSI-RTPS §9.6.3.2 PID-Konstanten fuer QoS-Policy-Wire-Encoding.
//! - [`CompatibilityResult`] / [`IncompatibleReason`] / [`check_compatibility`]
//!   — Request/Offered-Matrix.
//!
//! **Policies-Modul** ([`policies`]):
//! - 22 Standard-Policies: [`DurabilityQosPolicy`], [`DurabilityServiceQosPolicy`],
//!   [`DeadlineQosPolicy`], [`LatencyBudgetQosPolicy`], [`LivelinessQosPolicy`],
//!   [`ReliabilityQosPolicy`], [`DestinationOrderQosPolicy`], [`HistoryQosPolicy`],
//!   [`ResourceLimitsQosPolicy`], [`TransportPriorityQosPolicy`], [`LifespanQosPolicy`],
//!   [`UserDataQosPolicy`], [`OwnershipQosPolicy`], [`OwnershipStrengthQosPolicy`],
//!   [`PresentationQosPolicy`], [`PartitionQosPolicy`], [`TopicDataQosPolicy`],
//!   [`GroupDataQosPolicy`], [`TimeBasedFilterQosPolicy`],
//!   [`ReaderDataLifecycleQosPolicy`], [`WriterDataLifecycleQosPolicy`],
//!   [`EntityFactoryQosPolicy`].
//! - QoS-Aggregate: [`ReaderQos`], [`WriterQos`].
//! - Kind-Enums: [`DurabilityKind`], [`ReliabilityKind`], [`LivelinessKind`],
//!   [`OwnershipKind`], [`HistoryKind`], [`DestinationOrderKind`],
//!   [`PresentationAccessScope`].
//!
//! **Exclusive-Ownership-Resolver** ([`exclusive_ownership`]):
//! - [`exclusive_ownership::OwnershipResolver`] /
//!   [`exclusive_ownership::OwnershipCandidate`] /
//!   [`exclusive_ownership::resolve_strongest`] —
//!   DDS 1.4 §2.2.3.23 Strongest-Writer-Selection fuer DataReader.take()
//!   bei Topics mit `Exclusive`-Ownership-QoS.
//!
//! ## Beispiel
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
//! // Reliable writer + BestEffort reader (default) sind kompatibel.
//! assert!(matches!(check_compatibility(&writer_qos, &reader_qos), CompatibilityResult::Compatible));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// `zerodds-qos` setzt `alloc` zwingend voraus (Partition-Strings,
// GenericData-Buffers, ResourceLimits-Vec, …). `zerodds-cdr` mit
// `alloc`-Feature ist mandatory dep, also ist `extern crate alloc`
// immer verfuegbar.
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
