// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-dcps`. Safety classification: **STANDARD**.
//!
//! DCPS Public API (OMG DDS 1.4 §2.2.2): `DomainParticipant`,
//! `Publisher`, `Subscriber`, `Topic`, `DataWriter`, `DataReader`.
//!
//! Spec: OMG DDS 1.4 §2.2 (Data-Centric Publish-Subscribe Module) +
//! DDSI-RTPS 2.5 §8.5 (Discovery + WLP) + XTypes 1.3 §7.6.3
//! (TypeLookup-Service Wiring).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Bauend auf Layer 1
//! (foundation/cdr/qos/types/time-service), Layer 2
//! (rtps/discovery/transport-*), Layer 3 (idl/idl-rust/xml).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`DomainParticipantFactory`] — Singleton-Factory;
//!   `create_participant` spawnt eine Live-Runtime mit
//!   UDP/SPDP/SEDP/WLP, `create_participant_offline` baut einen
//!   in-process-Skeleton ohne Netzwerk fuer Unit-Tests.
//! - [`DomainParticipant`] — Top-Level-Entity; erzeugt
//!   Publishers/Subscribers/Topics, fuehrt Built-in-Type-Registry,
//!   exponiert TypeLookup-Hooks und Ignore-Filter.
//! - [`Publisher`] / [`DataWriter`] — typed `Writer<T>` mit `DdsType`-
//!   Bound; integriert RTPS-ReliableWriter (Live) bzw. In-Memory-Queue
//!   (Offline) plus Durability-Backend (DDS 1.4 §2.2.3.5).
//! - [`Subscriber`] / [`DataReader`] — typed `Reader<T>` mit
//!   `take`/`read`/Conditions, Sample-Cache und InstanceState-Tracker
//!   (DDS 1.4 §2.2.2.5).
//! - [`Topic`] / [`ContentFilteredTopic`] / [`MultiTopic`] — Topic-
//!   Hierarchie inkl. SQL-Filter (DDS 1.4 §2.2.2.3).
//! - Builtin-Topics: [`BuiltinSubscriber`] +
//!   [`DcpsParticipantBuiltinTopicData`] /
//!   [`DcpsPublicationBuiltinTopicData`] /
//!   [`DcpsSubscriptionBuiltinTopicData`] /
//!   [`DcpsTopicBuiltinTopicData`] (DDS 1.4 §2.2.5).
//! - Conditions/WaitSet: [`Condition`] / [`ReadCondition`] /
//!   [`QueryCondition`] / [`GuardCondition`] / [`WaitSet`].
//! - QoS-Familien: [`DomainParticipantQos`], [`PublisherQos`],
//!   [`SubscriberQos`], [`TopicQos`], [`DataWriterQos`],
//!   [`DataReaderQos`].
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_dcps::*;
//! let factory = DomainParticipantFactory::instance();
//! // Offline-Mode fuer Doctest (kein UDP-Multicast noetig).
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
pub mod dds_type;
#[cfg(feature = "std")]
pub mod durability_service;
pub mod entity;
pub mod error;
pub mod factory;
/// ADR-0005: opt-in Flatdata-Integration.
#[cfg(all(feature = "std", feature = "flatdata-integration"))]
pub mod flatdata_integration;
pub mod instance_handle;
#[cfg(feature = "std")]
pub mod instance_tracker;
#[cfg(feature = "alloc")]
pub mod interop;
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
pub mod sample;
pub mod sample_info;
pub mod status;
pub mod subscriber;
pub mod time;
pub mod topic;
#[cfg(feature = "std")]
pub mod wlp;

// Flat-Re-Exports fuer die typische Import-Zeile.
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
        // Loopback smoke: Factory → Offline-Participant → Topic → Writer/Reader.
        // Schleifen wir einen Sample per __push_raw von DataWriter
        // in den DataReader. Live-Transport wird in den Runtime-Tests
        // in crates/dcps/src/runtime.rs abgedeckt.
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
        // Manuell queue drainen und in den Reader pushen, simuliert
        // den Live-Transport-Pfad.
        for bytes in w.__drain_pending() {
            r.__push_raw(bytes).unwrap();
        }
        let samples = r.take().unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].data, vec![1, 2, 3]);
        assert_eq!(samples[1].data, vec![4, 5]);
    }
}
