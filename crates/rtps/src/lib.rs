// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer/Reader state machines, RTPS submessages, fragmentation.
//!
//! Crate `zerodds-rtps`. Safety classification: **SAFE**.
//! See `docs/architecture/02_architecture.md §3` and
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! DDSI-RTPS 2.5: header and submessages (`submessages`, `header`,
//! `header_extension`), parameter lists and the discovery data built on them
//! (`participant_data`, `publication_data`, `subscription_data`), the
//! best-effort and reliable writer and reader state machines with their
//! proxies, the history cache, fragmentation and reassembly
//! (`fragment_assembler`), and the vendor behaviour table (`dialect`).
//! Transports are in the `transport-*` crates.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
pub mod datagram;
#[cfg(feature = "alloc")]
pub mod dialect;
pub mod endpoint_security_info;
pub mod error;
#[cfg(feature = "alloc")]
pub mod fragment_assembler;
#[cfg(feature = "alloc")]
pub mod group_digest;
pub mod header;
#[cfg(feature = "alloc")]
pub mod header_extension;
#[cfg(feature = "alloc")]
pub mod history_cache;
#[cfg(feature = "alloc")]
pub mod inline_qos;
#[cfg(feature = "alloc")]
pub mod message_builder;
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "alloc")]
pub mod parameter_list;
#[cfg(feature = "alloc")]
pub mod participant_data;
#[cfg(feature = "alloc")]
pub mod participant_message_data;
pub mod participant_security_info;
#[cfg(feature = "alloc")]
pub mod payload;
#[cfg(feature = "alloc")]
pub mod property_list;
#[cfg(feature = "alloc")]
pub mod publication_data;
#[cfg(feature = "alloc")]
pub mod qos_bridge;
#[cfg(feature = "alloc")]
pub mod reader;
#[cfg(feature = "alloc")]
pub mod reader_proxy;
#[cfg(feature = "alloc")]
pub mod receiver_state;
#[cfg(feature = "alloc")]
pub mod reliable_reader;
#[cfg(feature = "alloc")]
pub mod reliable_stateless_writer;
#[cfg(feature = "alloc")]
pub mod reliable_writer;
pub mod security_algo_info;
pub mod submessage_header;
#[cfg(feature = "alloc")]
pub mod submessages;
#[cfg(feature = "alloc")]
pub mod subscription_data;
/// `DiscoveredTopicData`, the SEDP topics writer's sample (DDSI-RTPS 2.5 §8.5.4.2).
#[cfg(feature = "alloc")]
pub mod topic_data;
/// `TypeIdV1` and `TypeObjectV1` in the built-in topic data (XTypes 1.3 Annex D).
#[cfg(feature = "alloc")]
pub mod type_v1;
pub mod wire_types;
#[cfg(feature = "alloc")]
pub mod writer;
#[cfg(feature = "alloc")]
pub mod writer_group;
#[cfg(feature = "alloc")]
pub mod writer_proxy;

pub use error::WireError;
pub use header::{RTPS_MAGIC, RtpsHeader};
pub use submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
pub use wire_types::{
    EntityId, EntityKind, Guid, GuidPrefix, Locator, LocatorKind, ProtocolVersion, SequenceNumber,
    VendorId,
};
