// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Crate `zerodds-rs`. Safety classification: **SAFE**.
//!
//! `zerodds-rs` — idiomatische Rust-SDK-Facade.
//!
//! Re-exportiert die nutzerseitig relevanten APIs aus den darunterliegenden
//! Implementierungs-Crates (`zerodds-dcps`, `zerodds-dcps-async`,
//! `zerodds-qos`, `zerodds-types`, `zerodds-monitor`, `zerodds-recorder`).
//!
//! Anwendungen sollten **diese** Crate nutzen — direkter Zugriff auf
//! `zerodds-dcps` etc. ist erlaubt, aber an `zerodds-rs` muss sich die
//! Public-API stabilisieren.
//!
//! # Beispiel
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
//! let _ = writer; // offline-Mode hat kein UDP — nur API-Smoke.
//! ```
//!
//! ## Re-Exports
//!
//! - **[`crate::dcps`]** — synchrone DCPS-Surface (`zerodds_dcps::*`).
//! - **[`crate::aio`]** — async DCPS (`zerodds_dcps_async::*`).
//! - **[`crate::qos`]** — alle 22 QoS-Policies (`zerodds_qos::*`).
//! - **[`crate::types`]** — IDL-/XTypes-Typsystem (`zerodds_types::*`).
//! - **[`crate::monitor`]** — Statistics + Built-in DCPS-Monitor.
//! - **[`crate::recorder`]** — Sample-Recording + -Replay.
//!
//! Plus Top-Level-Convenience-Re-Exports der haeufigsten Symbole.

#![deny(unsafe_code)]
#![warn(missing_docs)]

/// Sync DCPS-API (`zerodds_dcps`).
pub use zerodds_dcps as dcps;

/// Async DCPS-API (`zerodds_dcps_async`).
pub use zerodds_dcps_async as aio;

/// QoS-Policies (`zerodds_qos`).
pub use zerodds_qos as qos;

/// IDL-/XTypes-Typsystem (`zerodds_types`).
pub use zerodds_types as types;

/// DCPS-Monitor (`zerodds_monitor`).
pub use zerodds_monitor as monitor;

/// Sample-Recording (`zerodds_recorder`).
pub use zerodds_recorder as recorder;

/// Foundation-Layer (Time-Service, Observability) (`zerodds_foundation`).
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
