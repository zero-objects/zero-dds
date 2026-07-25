// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-dcps`. Safety classification: **STANDARD**.
//!
//! DCPS Public API (OMG DDS 1.4 §2.2.2): `DomainParticipant`,
//! `Publisher`, `Subscriber`, `Topic`, `DataWriter`, `DataReader`.
//!
//! Spec: OMG DDS 1.4 §2.2 (Data-Centric Publish-Subscribe Module) +
//! DDSI-RTPS 2.5 §8.5 (Discovery + WLP) + XTypes 1.3 §7.6.3
//! (TypeLookup service wiring).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Built on Layer 1
//! (foundation/cdr/qos/types/time-service), Layer 2
//! (rtps/discovery/transport-*), Layer 3 (idl/idl-rust/xml).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`DomainParticipantFactory`] — singleton factory;
//!   `create_participant` spawns a live runtime with UDP/SPDP/SEDP/WLP,
//!   `create_participant_offline` builds an in-process skeleton without
//!   networking for unit tests.
//! - [`DomainParticipant`] — top-level entity; creates
//!   publishers/subscribers/topics, maintains the built-in type
//!   registry, exposes TypeLookup hooks and ignore filters.
//! - [`Publisher`] / [`DataWriter`] — typed `Writer<T>` with a `DdsType`
//!   bound; integrates the RTPS ReliableWriter (live) or an in-memory
//!   queue (offline) plus the durability backend (DDS 1.4 §2.2.3.5).
//! - [`Subscriber`] / [`DataReader`] — typed `Reader<T>` with
//!   `take`/`read`/conditions, sample cache and InstanceState tracker
//!   (DDS 1.4 §2.2.2.5).
//! - [`Topic`] / [`ContentFilteredTopic`] / [`MultiTopic`] — the topic
//!   hierarchy incl. SQL filters (DDS 1.4 §2.2.2.3).
//! - Builtin topics: [`BuiltinSubscriber`] +
//!   [`DcpsParticipantBuiltinTopicData`] /
//!   [`DcpsPublicationBuiltinTopicData`] /
//!   [`DcpsSubscriptionBuiltinTopicData`] /
//!   [`DcpsTopicBuiltinTopicData`] (DDS 1.4 §2.2.5).
//! - Conditions/WaitSet: [`Condition`] / [`ReadCondition`] /
//!   [`QueryCondition`] / [`GuardCondition`] / [`WaitSet`].
//! - QoS families: [`DomainParticipantQos`], [`PublisherQos`],
//!   [`SubscriberQos`], [`TopicQos`], [`DataWriterQos`],
//!   [`DataReaderQos`].
//!
//! ## Example
//!
//! ```
//! use zerodds_dcps::*;
//! let factory = DomainParticipantFactory::instance();
//! // Offline mode for the doctest (no UDP multicast needed).
//! let participant = factory.create_participant_offline(0, DomainParticipantQos::default());
//! let topic = participant
//!     .create_topic::<RawBytes>("Chatter", TopicQos::default())
//!     .expect("create_topic");
//! let publisher = participant.create_publisher(PublisherQos::default());
//! let writer = publisher
//!     .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
//!     .expect("create_datawriter");
//! writer.write(&RawBytes::new(vec![1, 2, 3])).expect("write");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod builtin_subscriber;
pub mod builtin_topics;
pub mod coherent_set;
#[cfg(feature = "std")]
pub mod condition;
/// Cross-vendor same-host zero-copy with Cyclone over iceoryx C++ (POSH).
#[cfg(all(feature = "std", feature = "cyclone-iox", target_os = "linux"))]
pub mod cyclone_iox_integration;
pub mod dds_type;
#[cfg(feature = "std")]
pub mod durability_service;
pub mod entity;
pub mod error;
pub mod factory;
/// ADR-0005: opt-in flatdata integration.
#[cfg(all(feature = "std", feature = "flatdata-integration"))]
pub mod flatdata_integration;
/// In-process discovery fastpath (same-process, same-domain).
#[cfg(feature = "std")]
mod inproc;
pub mod instance_handle;
#[cfg(feature = "std")]
pub mod instance_tracker;
#[cfg(feature = "alloc")]
pub mod interop;
/// Preference-ordered multi-transport for user traffic (SHM + UDP fallback).
#[cfg(feature = "std")]
pub mod layered_transport;
pub mod listener;
#[cfg(feature = "std")]
pub mod listener_dispatch;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod participant;
pub mod psm_constants;
pub mod publisher;
pub mod qos;
#[cfg(feature = "std")]
pub mod runtime;
pub mod same_host;
/// Wave 4b.3: feature-gated bridge between the `same_host` tracker and
/// `zerodds-transport-shm::PosixShmTransport`. Only compiled when the
/// `same-host-shm` feature is active.
#[cfg(all(feature = "std", feature = "same-host-shm"))]
pub mod same_host_shm;
/// 4b.5: alternative UDS datagram backend for same-host paths. Not true
/// zero-copy but container-friendly. Only compiled when the
/// `same-host-uds` feature is active.
#[cfg(all(feature = "std", feature = "same-host-uds"))]
pub mod same_host_uds;
pub mod sample;
pub mod sample_bytes;
pub mod sample_info;
/// D.5e Phase 3 — deadline-heap scheduler (event-driven replacement for the
/// fixed-period tick poll). Std-only (mpsc channel + Instant park).
#[cfg(feature = "std")]
pub mod scheduler;
/// Multi-peer SHM adapter for `user_unicast` transport selection. Wraps
/// multiple `PosixShmTransport` pairs behind the transport trait.
/// Feature-gated via `same-host-shm` because zerodds-transport-shm is
/// optional.
#[cfg(all(feature = "std", feature = "same-host-shm"))]
pub mod shm_user;
pub mod status;
pub mod subscriber;
pub mod time;
pub mod topic;
#[cfg(feature = "std")]
pub mod wlp;

// Flat re-exports for the typical import line.
pub use builtin_subscriber::{BuiltinSinks, BuiltinSubscriber, BuiltinTopic, builtin_reader_qos};
pub use builtin_topics::{
    ParticipantBuiltinTopicData as DcpsParticipantBuiltinTopicData,
    PublicationBuiltinTopicData as DcpsPublicationBuiltinTopicData,
    SubscriptionBuiltinTopicData as DcpsSubscriptionBuiltinTopicData, TOPIC_NAME_DCPS_PARTICIPANT,
    TOPIC_NAME_DCPS_PUBLICATION, TOPIC_NAME_DCPS_SUBSCRIPTION, TOPIC_NAME_DCPS_TOPIC,
    TopicBuiltinTopicData as DcpsTopicBuiltinTopicData,
};
pub use dds_type::{
    DdsType, DdsTypeRow, DecodeError, EncodeError, Extensibility, ExtensibilityKind, RawBytes,
};
pub use entity::{Entity, EntityState, StatusCondition, StatusMask, immutable_if_enabled};
/// The SQL-filter field value type ([`DdsType::field_value`] / content-filtered
/// topics). Re-exported so `#[derive(DdsType)]` can emit `field_value` without
/// the user crate depending on `zerodds-sql-filter` directly.
pub use zerodds_sql_filter::Value as FilterValue;

pub use coherent_set::{CoherentScope, CoherentSetMarker, GroupAccessScope};
#[cfg(feature = "std")]
pub use condition::{Condition, GuardCondition, QueryCondition, ReadCondition, WaitSet};
pub use error::{DdsError, Result};
pub use factory::DomainParticipantFactory;
pub use instance_handle::{HANDLE_NIL, InstanceHandle, InstanceHandleAllocator};
#[cfg(feature = "std")]
pub use instance_tracker::{InstanceState, InstanceTracker, KeyHash};
#[cfg(feature = "std")]
pub use participant::IgnoreFilter;
pub use participant::{DomainId, DomainParticipant};
pub use publisher::{DataWriter, Publisher};
pub use qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
pub use sample::Sample;
pub use sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind, instance_state_mask,
    sample_state_mask, view_state_mask,
};
pub use subscriber::{DataReader, Subscriber};
pub use time::{Duration, Time, get_current_time};
#[cfg(feature = "std")]
pub use topic::hash_join_two;
pub use topic::{
    ContentFilteredTopic, JoinedRow, MultiTopic, Topic, TopicDescription, TopicDescriptionHandle,
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_in_process_loopback() {
        // Loopback smoke: Factory → offline participant → Topic → Writer/Reader.
        // We loop a sample via __push_raw from the DataWriter into the
        // DataReader. Live transport is covered in the runtime tests in
        // crates/dcps/src/runtime.rs.
        let factory = DomainParticipantFactory::instance();
        let p = factory.create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();

        let pub_ = p.create_publisher(PublisherQos::default());
        let w = pub_
            .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
            .unwrap();

        let sub = p.create_subscriber(SubscriberQos::default());
        let r = sub
            .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
            .unwrap();

        w.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
        w.write(&RawBytes::new(vec![4, 5])).unwrap();
        // Manually drain the queue and push into the reader, simulating
        // the live transport path.
        for bytes in w.__drain_pending() {
            r.__push_raw(bytes).unwrap();
        }
        let samples = r.take().unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].data, vec![1, 2, 3]);
        assert_eq!(samples[1].data, vec![4, 5]);
    }
}
