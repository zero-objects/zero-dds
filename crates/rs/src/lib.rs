// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Crate `zerodds-rs`. Safety classification: **SAFE**.
//!
//! `zerodds-rs` — idiomatic Rust SDK facade.
//!
//! Re-exports the user-facing APIs from the underlying
//! implementation crates (`zerodds-dcps`, `zerodds-dcps-async`,
//! `zerodds-qos`, `zerodds-types`, `zerodds-monitor`, `zerodds-recorder`).
//!
//! Applications should use **this** crate — direct access to
//! `zerodds-dcps` etc. is allowed, but the public API must stabilize
//! against `zerodds-rs`.
//!
//! # Example
//!
//! ```no_run
//! use zerodds_rs::{
//!     DomainParticipantFactory, DomainParticipantQos, RawBytes, TopicQos,
//!     PublisherQos, SubscriberQos, DataWriterQos, DataReaderQos,
//! };
//!
//! let f = DomainParticipantFactory::instance();
//! let dp = f.create_participant_offline(0, DomainParticipantQos::default());
//! let topic = dp.create_topic::<RawBytes>("Chatter", TopicQos::default()).unwrap();
//! let publisher = dp.create_publisher(PublisherQos::default());
//! let writer = publisher
//!     .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
//!     .unwrap();
//! let _ = writer; // offline mode has no UDP — API smoke only.
//! ```
//!
//! ## Re-Exports
//!
//! - **[`crate::dcps`]** — synchronous DCPS surface (`zerodds_dcps::*`).
//! - **[`crate::aio`]** — async DCPS (`zerodds_dcps_async::*`).
//! - **[`crate::qos`]** — all 22 QoS policies (`zerodds_qos::*`).
//! - **[`crate::types`]** — IDL/XTypes type system (`zerodds_types::*`).
//! - **[`crate::monitor`]** — statistics + built-in DCPS monitor.
//! - **[`crate::recorder`]** — sample recording + replay.
//!
//! Plus top-level convenience re-exports of the most common symbols.

#![deny(unsafe_code)]
#![warn(missing_docs)]

/// Sync DCPS-API (`zerodds_dcps`).
pub use zerodds_dcps as dcps;

/// Async DCPS-API (`zerodds_dcps_async`).
pub use zerodds_dcps_async as aio;

/// QoS-Policies (`zerodds_qos`).
pub use zerodds_qos as qos;

/// IDL/XTypes type system (`zerodds_types`).
pub use zerodds_types as types;

/// DCPS-Monitor (`zerodds_monitor`).
pub use zerodds_monitor as monitor;

/// Sample recording (`zerodds_recorder`).
pub use zerodds_recorder as recorder;

/// Foundation layer (time service, observability) (`zerodds_foundation`).
pub use zerodds_foundation as foundation;

// ---- Top-Level Convenience Re-Exports -------------------------------------

pub use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriter, DataWriterQos, DdsError, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, Publisher, PublisherQos, RawBytes, Result,
    Subscriber, SubscriberQos, Topic, TopicQos,
};

pub use zerodds_dcps::dds_type::DdsType;
pub use zerodds_dcps::instance_handle::InstanceHandle;

pub use zerodds_qos::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_singleton_returns_stable_ref() {
        let f1 = DomainParticipantFactory::instance() as *const _;
        let f2 = DomainParticipantFactory::instance() as *const _;
        assert_eq!(f1, f2);
    }

    #[test]
    fn participant_offline_create_topic() {
        let f = DomainParticipantFactory::instance();
        let dp = f.create_participant_offline(7, DomainParticipantQos::default());
        let t = dp.create_topic::<RawBytes>("X", TopicQos::default());
        assert!(t.is_ok());
    }

    #[test]
    fn re_exports_resolve() {
        // QoS-Re-Export.
        let _ = Duration::from_millis(500);
        // Async-Re-Export ist erreichbar (Modulpfad).
        let _: &str = stringify!(aio::AsyncDomainParticipantFactory);
    }
}
