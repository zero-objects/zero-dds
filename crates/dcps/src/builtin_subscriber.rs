// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Built-in subscriber — preinstalled subscriber with the 4
//! built-in-topic readers (DDS 1.4 §2.2.2.2.1.7
//! `get_builtin_subscriber`).
//!
//! Per `DomainParticipant` there is exactly **one** built-in
//! subscriber. It is the application-API-view wrapper over the
//! runtime's discovery cache: every received SPDP beacon or SEDP
//! pub/sub event drops a sample into the respective built-in reader,
//! and user code can pick it up via `take()/read()` — just like any
//! other DataReader.
//!
//! # Spec paths
//!
//! - DDS-DCPS 1.4 §2.2.5: Built-in Topics (all 4)
//! - DDS-DCPS 1.4 §2.2.2.2.1.7: `get_builtin_subscriber()`
//! - DDSI-RTPS 2.5 §8.5.4: SEDP Built-in Endpoints
//!
//! # Built-in DataReader QoS (spec §2.2.5 Table 12)
//!
//! Reliability=RELIABLE, Durability=TRANSIENT_LOCAL, History=KEEP_LAST(1).
//! We set these defaults in `BuiltinSubscriber::new`. Users (per spec)
//! **cannot** modify them — `lookup_datareader` is read-only.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::Mutex;

use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};

use crate::builtin_topics::{
    ParticipantBuiltinTopicData, PublicationBuiltinTopicData, SubscriptionBuiltinTopicData,
    TOPIC_NAME_DCPS_PARTICIPANT, TOPIC_NAME_DCPS_PUBLICATION, TOPIC_NAME_DCPS_SUBSCRIPTION,
    TOPIC_NAME_DCPS_TOPIC, TopicBuiltinTopicData,
};
use crate::dds_type::DdsType;
use crate::error::{DdsError, Result};
use crate::qos::{DataReaderQos, SubscriberQos, TopicQos};
use crate::subscriber::{DataReader, Subscriber, SubscriberInner};
use crate::topic::Topic;

/// Spec-conformant DataReader QoS for built-in topics (DDS 1.4 §2.2.5
/// Table 12): RELIABLE + TRANSIENT_LOCAL + KEEP_LAST(1).
#[must_use]
pub fn builtin_reader_qos() -> DataReaderQos {
    let mut qos = DataReaderQos::default();
    qos.reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        max_blocking_time: qos.reliability.max_blocking_time,
    };
    qos.durability = DurabilityQosPolicy {
        kind: DurabilityKind::TransientLocal,
    };
    qos.history = HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 1,
    };
    qos
}

/// Built-in subscriber. Holds 4 pre-created DataReaders for the
/// built-in topics. Each reader's `inbox` is exposed as
/// `Arc<Mutex<Vec<crate::runtime::UserSample>>>` so that the runtime
/// discovery hook can feed in new samples without lock cycles.
#[derive(Debug)]
pub struct BuiltinSubscriber {
    /// The "transparent" subscriber handle that `get_builtin_subscriber`
    /// returns — for API symmetry with user subscribers.
    subscriber: Subscriber,
    /// Reader for `DCPSParticipant` (discovered participants).
    participant_reader: DataReader<ParticipantBuiltinTopicData>,
    /// Reader for `DCPSTopic` (discovered topics).
    topic_reader: DataReader<TopicBuiltinTopicData>,
    /// Reader for `DCPSPublication` (discovered writers).
    publication_reader: DataReader<PublicationBuiltinTopicData>,
    /// Reader for `DCPSSubscription` (discovered readers).
    subscription_reader: DataReader<SubscriptionBuiltinTopicData>,
    /// Sink inboxes (shared with the readers above). Used by the
    /// runtime discovery hook as push targets.
    sinks: BuiltinSinks,
}

/// Bundle of the 4 shared inboxes — handed by `DcpsRuntime` to the
/// SPDP/SEDP hot path. Cloning is cheap (Arc bumps).
#[derive(Debug, Clone)]
pub struct BuiltinSinks {
    /// Inbox of the `DCPSParticipant` reader.
    pub participant: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox of the `DCPSTopic` reader.
    pub topic: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox of the `DCPSPublication` reader.
    pub publication: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox of the `DCPSSubscription` reader.
    pub subscription: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
}

impl BuiltinSinks {
    /// Convenience helper: encodes a built-in sample and pushes it into
    /// the matching reader inbox.
    ///
    /// # Errors
    /// `WireError` if the encoding fails; `PreconditionNotMet` if the
    /// mutex is poisoned.
    pub fn push_participant(&self, sample: &ParticipantBuiltinTopicData) -> Result<()> {
        push_into(&self.participant, sample)
    }

    /// Push for `DCPSTopic`.
    ///
    /// # Errors
    /// As [`Self::push_participant`].
    pub fn push_topic(&self, sample: &TopicBuiltinTopicData) -> Result<()> {
        push_into(&self.topic, sample)
    }

    /// Push for `DCPSPublication`.
    ///
    /// # Errors
    /// As [`Self::push_participant`].
    pub fn push_publication(&self, sample: &PublicationBuiltinTopicData) -> Result<()> {
        push_into(&self.publication, sample)
    }

    /// Push for `DCPSSubscription`.
    ///
    /// # Errors
    /// As [`Self::push_participant`].
    pub fn push_subscription(&self, sample: &SubscriptionBuiltinTopicData) -> Result<()> {
        push_into(&self.subscription, sample)
    }
}

fn push_into<T: DdsType>(
    sink: &Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    sample: &T,
) -> Result<()> {
    let mut buf = Vec::new();
    sample.encode(&mut buf).map_err(|e| DdsError::WireError {
        message: format_err(&e),
    })?;
    let mut guard = sink.lock().map_err(|_| DdsError::PreconditionNotMet {
        reason: "builtin sink mutex poisoned",
    })?;
    // Built-in samples come from the local discovery path (no remote
    // writer); writer_guid + writer_strength are default-initialized.
    // Built-in topics use shared ownership, so no filter activation in
    // the reader.
    guard.push(crate::runtime::UserSample::Alive {
        payload: crate::sample_bytes::SampleBytes::from_vec(buf),
        writer_guid: [0u8; 16],
        writer_strength: 0,
        // Built-in-topic samples are encoded ZeroDDS-internally —
        // XCDR1 baseline.
        representation: 0,
    });
    Ok(())
}

fn format_err(e: &crate::dds_type::EncodeError) -> String {
    use core::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, "{e}");
    s
}

impl BuiltinSubscriber {
    /// Constructs a built-in subscriber **with** pre-created readers for
    /// all 4 built-in topics. Called exactly once per
    /// `DomainParticipant` (by the constructor).
    #[must_use]
    pub fn new() -> Self {
        // Subscriber with default QoS — the subscriber itself is simply
        // an API wrapper, not a runtime endpoint.
        let subscriber = Subscriber::new(SubscriberQos::default(), None);
        let inner = subscriber.inner.clone();

        let qos = builtin_reader_qos();

        // Create 4 inboxes (shared between the DataReader and the
        // runtime discovery hook).
        let part_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));
        let topic_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));
        let pub_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));
        let sub_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));

        let participant_reader = DataReader::new_builtin(
            Topic::<ParticipantBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_PARTICIPANT.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            part_inbox.clone(),
        );
        let topic_reader = DataReader::new_builtin(
            Topic::<TopicBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_TOPIC.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            topic_inbox.clone(),
        );
        let publication_reader = DataReader::new_builtin(
            Topic::<PublicationBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_PUBLICATION.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            pub_inbox.clone(),
        );
        let subscription_reader = DataReader::new_builtin(
            Topic::<SubscriptionBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_SUBSCRIPTION.to_string(),
                TopicQos::default(),
            ),
            qos,
            inner,
            sub_inbox.clone(),
        );

        Self {
            subscriber,
            participant_reader,
            topic_reader,
            publication_reader,
            subscription_reader,
            sinks: BuiltinSinks {
                participant: part_inbox,
                topic: topic_inbox,
                publication: pub_inbox,
                subscription: sub_inbox,
            },
        }
    }

    /// Underlying subscriber handle (API mirror of user subscribers).
    #[must_use]
    pub fn subscriber(&self) -> &Subscriber {
        &self.subscriber
    }

    /// Sinks for the runtime discovery hook. Should only be used
    /// internally (in `DcpsRuntime`).
    #[must_use]
    pub fn sinks(&self) -> BuiltinSinks {
        self.sinks.clone()
    }

    /// Returns a copy of the typed DataReader for a built-in topic.
    /// Spec: `Subscriber::lookup_datareader(topic_name)` (DDS 1.4
    /// §2.2.2.5.1.5).
    ///
    /// **Topic name + type parameter MUST be consistent** (e.g.
    /// `lookup_datareader::<ParticipantBuiltinTopicData>("DCPSParticipant")`),
    /// otherwise it returns `BadParameter`.
    ///
    /// # Errors
    /// `BadParameter` if `topic_name` matches no built-in topic, or if
    /// the type parameter and topic name diverge.
    pub fn lookup_datareader<T: BuiltinTopic>(&self, topic_name: &str) -> Result<DataReader<T>> {
        T::lookup(self, topic_name)
    }

    /// Direct access to the `DCPSParticipant` reader (convenience API,
    /// avoids generic lookup paths for built-in topics).
    #[must_use]
    pub fn participant_reader(&self) -> DataReader<ParticipantBuiltinTopicData> {
        clone_reader(&self.participant_reader)
    }

    /// Direct access to the `DCPSTopic` reader.
    #[must_use]
    pub fn topic_reader(&self) -> DataReader<TopicBuiltinTopicData> {
        clone_reader(&self.topic_reader)
    }

    /// Direct access to the `DCPSPublication` reader.
    #[must_use]
    pub fn publication_reader(&self) -> DataReader<PublicationBuiltinTopicData> {
        clone_reader(&self.publication_reader)
    }

    /// Direct access to the `DCPSSubscription` reader.
    #[must_use]
    pub fn subscription_reader(&self) -> DataReader<SubscriptionBuiltinTopicData> {
        clone_reader(&self.subscription_reader)
    }
}

impl Default for BuiltinSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker trait + lookup routing for the 4 built-in-topic types.
///
/// So that `lookup_datareader::<T>(topic_name)` hits the right reader
/// via the type parameter. External types cannot implement the trait —
/// `BuiltinTopic` is sealed.
pub trait BuiltinTopic: DdsType + private::Sealed + Sized {
    /// Topic name from DDS 1.4 §2.2.5.
    const TOPIC_NAME: &'static str;
    #[doc(hidden)]
    fn lookup(sub: &BuiltinSubscriber, topic_name: &str) -> Result<DataReader<Self>>;
}

mod private {
    pub trait Sealed {}
    impl Sealed for crate::builtin_topics::ParticipantBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::TopicBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::PublicationBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::SubscriptionBuiltinTopicData {}
}

impl BuiltinTopic for ParticipantBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PARTICIPANT;
    fn lookup(sub: &BuiltinSubscriber, topic_name: &str) -> Result<DataReader<Self>> {
        if topic_name != Self::TOPIC_NAME {
            return Err(DdsError::BadParameter {
                what: "builtin topic_name does not match type parameter",
            });
        }
        Ok(sub.participant_reader())
    }
}

impl BuiltinTopic for TopicBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_TOPIC;
    fn lookup(sub: &BuiltinSubscriber, topic_name: &str) -> Result<DataReader<Self>> {
        if topic_name != Self::TOPIC_NAME {
            return Err(DdsError::BadParameter {
                what: "builtin topic_name does not match type parameter",
            });
        }
        Ok(sub.topic_reader())
    }
}

impl BuiltinTopic for PublicationBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PUBLICATION;
    fn lookup(sub: &BuiltinSubscriber, topic_name: &str) -> Result<DataReader<Self>> {
        if topic_name != Self::TOPIC_NAME {
            return Err(DdsError::BadParameter {
                what: "builtin topic_name does not match type parameter",
            });
        }
        Ok(sub.publication_reader())
    }
}

impl BuiltinTopic for SubscriptionBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_SUBSCRIPTION;
    fn lookup(sub: &BuiltinSubscriber, topic_name: &str) -> Result<DataReader<Self>> {
        if topic_name != Self::TOPIC_NAME {
            return Err(DdsError::BadParameter {
                what: "builtin topic_name does not match type parameter",
            });
        }
        Ok(sub.subscription_reader())
    }
}

/// Clones a reader by constructing a new reader handle with the same
/// shared inbox + identical topic/QoS snapshot. We cannot make
/// `DataReader<T>` directly `Clone`, because the `runtime`/`rx` fields
/// (mpsc + Mutex) are not `Clone`. For built-in readers, however, both
/// are `None`, so the special-case clone here is safe.
fn clone_reader<T: DdsType + Send + Sync + 'static>(r: &DataReader<T>) -> DataReader<T> {
    DataReader::<T>::new_builtin(
        r.topic().clone(),
        r.qos().clone(),
        builtin_clone_subscriber_inner(r),
        r.__inbox_handle(),
    )
}

fn builtin_clone_subscriber_inner<T: DdsType>(_r: &DataReader<T>) -> Arc<SubscriberInner> {
    // The built-in subscriber is static: we construct a new inner on
    // every lookup — it is not used for runtime routing lookup anyway.
    Arc::new(SubscriberInner {
        qos: std::sync::Mutex::new(SubscriberQos::default()),
        entity_state: crate::entity::EntityState::new(),
        runtime: None,
        listener: std::sync::Mutex::new(None),
        participant: std::sync::Mutex::new(None),
        access_scope: crate::coherent_set::GroupAccessScope::new(),
        datareaders: std::sync::Mutex::new(alloc::vec::Vec::new()),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::builtin_topics::{
        ParticipantBuiltinTopicData, PublicationBuiltinTopicData, SubscriptionBuiltinTopicData,
        TopicBuiltinTopicData,
    };
    use zerodds_rtps::wire_types::Guid;

    fn mk_guid(seed: u8) -> Guid {
        let mut b = [0u8; 16];
        for (i, slot) in b.iter_mut().enumerate() {
            *slot = seed.wrapping_add(i as u8);
        }
        Guid::from_bytes(b)
    }

    #[test]
    fn builtin_subscriber_has_four_readers() {
        let bs = BuiltinSubscriber::new();
        assert_eq!(bs.participant_reader().topic().name(), "DCPSParticipant");
        assert_eq!(bs.topic_reader().topic().name(), "DCPSTopic");
        assert_eq!(bs.publication_reader().topic().name(), "DCPSPublication");
        assert_eq!(bs.subscription_reader().topic().name(), "DCPSSubscription");
    }

    #[test]
    fn builtin_reader_qos_is_spec_default() {
        let q = builtin_reader_qos();
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.durability.kind, DurabilityKind::TransientLocal);
        assert_eq!(q.history.kind, HistoryKind::KeepLast);
        assert_eq!(q.history.depth, 1);
    }

    #[test]
    fn lookup_datareader_routes_by_type() {
        let bs = BuiltinSubscriber::new();
        let r = bs
            .lookup_datareader::<ParticipantBuiltinTopicData>("DCPSParticipant")
            .unwrap();
        assert_eq!(r.topic().name(), "DCPSParticipant");

        let r = bs
            .lookup_datareader::<TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        assert_eq!(r.topic().name(), "DCPSTopic");

        let r = bs
            .lookup_datareader::<PublicationBuiltinTopicData>("DCPSPublication")
            .unwrap();
        assert_eq!(r.topic().name(), "DCPSPublication");

        let r = bs
            .lookup_datareader::<SubscriptionBuiltinTopicData>("DCPSSubscription")
            .unwrap();
        assert_eq!(r.topic().name(), "DCPSSubscription");
    }

    #[test]
    fn lookup_datareader_rejects_wrong_topic_name() {
        let bs = BuiltinSubscriber::new();
        let err = bs
            .lookup_datareader::<ParticipantBuiltinTopicData>("DCPSPublication")
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn sinks_push_participant_lands_in_reader() {
        let bs = BuiltinSubscriber::new();
        let sample = ParticipantBuiltinTopicData {
            key: mk_guid(0xA0),
            user_data: alloc::vec![],
        };
        bs.sinks().push_participant(&sample).unwrap();
        let reader = bs
            .lookup_datareader::<ParticipantBuiltinTopicData>("DCPSParticipant")
            .unwrap();
        let samples = reader.take().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].key, sample.key);
    }

    #[test]
    fn sinks_push_topic_lands_in_reader() {
        let bs = BuiltinSubscriber::new();
        let sample = TopicBuiltinTopicData {
            key: TopicBuiltinTopicData::synthesize_key("MyT", "MyType"),
            name: "MyT".to_string(),
            type_name: "MyType".to_string(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityKind::Reliable,
        };
        bs.sinks().push_topic(&sample).unwrap();
        let reader = bs
            .lookup_datareader::<TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        let samples = reader.take().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].name, "MyT");
    }

    #[test]
    fn sinks_push_publication_lands_in_reader() {
        let bs = BuiltinSubscriber::new();
        let sample = PublicationBuiltinTopicData {
            key: mk_guid(0xB0),
            participant_key: mk_guid(0xC0),
            topic_name: "T".to_string(),
            type_name: "T".to_string(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityKind::BestEffort,
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness_lease_seconds: 0,
            deadline_seconds: 0,
            lifespan_seconds: 0,
            partition: alloc::vec![],
        };
        bs.sinks().push_publication(&sample).unwrap();
        let reader = bs
            .lookup_datareader::<PublicationBuiltinTopicData>("DCPSPublication")
            .unwrap();
        let samples = reader.take().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].key, sample.key);
    }

    #[test]
    fn sinks_push_subscription_lands_in_reader() {
        let bs = BuiltinSubscriber::new();
        let sample = SubscriptionBuiltinTopicData {
            key: mk_guid(0xD0),
            participant_key: mk_guid(0xE0),
            topic_name: "T".to_string(),
            type_name: "T".to_string(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityKind::Reliable,
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness_lease_seconds: 0,
            deadline_seconds: 0,
            partition: alloc::vec![],
        };
        bs.sinks().push_subscription(&sample).unwrap();
        let reader = bs
            .lookup_datareader::<SubscriptionBuiltinTopicData>("DCPSSubscription")
            .unwrap();
        let samples = reader.take().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].topic_name, "T");
    }

    #[test]
    fn subscriber_handle_is_accessible() {
        let bs = BuiltinSubscriber::new();
        // Smoke: the inner subscriber handle is accessible (API
        // symmetry with user subscribers).
        let _: &Subscriber = bs.subscriber();
    }

    #[test]
    fn default_constructs_via_new() {
        let bs: BuiltinSubscriber = Default::default();
        assert_eq!(bs.participant_reader().topic().name(), "DCPSParticipant");
    }

    #[test]
    fn lookup_topic_with_wrong_topic_name_for_topic_type() {
        let bs = BuiltinSubscriber::new();
        let err = bs
            .lookup_datareader::<TopicBuiltinTopicData>("DCPSParticipant")
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn lookup_publication_with_wrong_topic_name() {
        let bs = BuiltinSubscriber::new();
        let err = bs
            .lookup_datareader::<PublicationBuiltinTopicData>("DCPSTopic")
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn lookup_subscription_with_wrong_topic_name() {
        let bs = BuiltinSubscriber::new();
        let err = bs
            .lookup_datareader::<SubscriptionBuiltinTopicData>("DCPSTopic")
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn read_does_not_remove_samples() {
        let bs = BuiltinSubscriber::new();
        let sample = ParticipantBuiltinTopicData {
            key: mk_guid(0x33),
            user_data: alloc::vec![],
        };
        bs.sinks().push_participant(&sample).unwrap();
        let reader = bs.participant_reader();
        let s1 = reader.read().unwrap();
        let s2 = reader.read().unwrap();
        assert_eq!(s1.len(), 1);
        assert_eq!(s2.len(), 1);
    }
}
