// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Builtin-Subscriber — vorinstallierter Subscriber mit den 4
//! Builtin-Topic-Readern (DDS 1.4 §2.2.2.2.1.7
//! `get_builtin_subscriber`).
//!
//! Pro `DomainParticipant` existiert genau **ein** Builtin-Subscriber.
//! Er ist der Application-API-Sicht-Wrapper auf den
//! Discovery-Cache der Runtime: jedes empfangene SPDP-Beacon
//! oder SEDP-Pub/Sub-Event laesst ein Sample in den jeweiligen
//! Builtin-Reader fallen, und User-Code kann es per
//! `take()/read()` abholen — wie bei jedem anderen DataReader.
//!
//! # Spec-Pfade
//!
//! - DDS-DCPS 1.4 §2.2.5: Built-in Topics (alle 4)
//! - DDS-DCPS 1.4 §2.2.2.2.1.7: `get_builtin_subscriber()`
//! - DDSI-RTPS 2.5 §8.5.4: SEDP Built-in Endpoints
//!
//! # Built-in-DataReader-QoS (Spec §2.2.5 Table 12)
//!
//! Reliability=RELIABLE, Durability=TRANSIENT_LOCAL, History=KEEP_LAST(1).
//! Wir setzen diese Defaults an `BuiltinSubscriber::new`. User koennen
//! sie (analog Spec) **nicht** modifizieren — `lookup_datareader` ist
//! read-only.

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

/// Spec-konforme DataReader-QoS fuer Builtin-Topics (DDS 1.4 §2.2.5
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

/// Builtin-Subscriber. Haelt 4 vor-erzeugte DataReader fuer die
/// Builtin-Topics. Die `inbox` jedes Readers ist als
/// `Arc<Mutex<Vec<crate::runtime::UserSample>>>` herausgegeben, damit der Runtime-
/// Discovery-Hook neue Samples ohne Lock-Cycles einspeisen kann.
#[derive(Debug)]
pub struct BuiltinSubscriber {
    /// Der "transparente" Subscriber-Handle, den `get_builtin_subscriber`
    /// zurueckgibt — fuer API-Symmetrie zu User-Subscribers.
    subscriber: Subscriber,
    /// Reader fuer `DCPSParticipant` (entdeckte Participants).
    participant_reader: DataReader<ParticipantBuiltinTopicData>,
    /// Reader fuer `DCPSTopic` (entdeckte Topics).
    topic_reader: DataReader<TopicBuiltinTopicData>,
    /// Reader fuer `DCPSPublication` (entdeckte Writer).
    publication_reader: DataReader<PublicationBuiltinTopicData>,
    /// Reader fuer `DCPSSubscription` (entdeckte Reader).
    subscription_reader: DataReader<SubscriptionBuiltinTopicData>,
    /// Sink-Inboxes (gemeinsam mit den Readers oben). Werden vom
    /// Runtime-Discovery-Hook als Push-Targets verwendet.
    sinks: BuiltinSinks,
}

/// Bundle der 4 shared inboxes — wird vom `DcpsRuntime` an den
/// SPDP/SEDP-Hot-Path weitergereicht. Cloning ist billig (Arc-Bumps).
#[derive(Debug, Clone)]
pub struct BuiltinSinks {
    /// Inbox des `DCPSParticipant`-Readers.
    pub participant: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox des `DCPSTopic`-Readers.
    pub topic: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox des `DCPSPublication`-Readers.
    pub publication: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox des `DCPSSubscription`-Readers.
    pub subscription: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
}

impl BuiltinSinks {
    /// Bequemer Helfer: encoded ein Builtin-Sample und schiebt es in
    /// den passenden Reader-Inbox.
    ///
    /// # Errors
    /// `WireError` wenn das Encoding fehlschlaegt; `PreconditionNotMet`
    /// wenn der Mutex vergiftet ist.
    pub fn push_participant(&self, sample: &ParticipantBuiltinTopicData) -> Result<()> {
        push_into(&self.participant, sample)
    }

    /// Push fuer `DCPSTopic`.
    ///
    /// # Errors
    /// Wie [`Self::push_participant`].
    pub fn push_topic(&self, sample: &TopicBuiltinTopicData) -> Result<()> {
        push_into(&self.topic, sample)
    }

    /// Push fuer `DCPSPublication`.
    ///
    /// # Errors
    /// Wie [`Self::push_participant`].
    pub fn push_publication(&self, sample: &PublicationBuiltinTopicData) -> Result<()> {
        push_into(&self.publication, sample)
    }

    /// Push fuer `DCPSSubscription`.
    ///
    /// # Errors
    /// Wie [`Self::push_participant`].
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
    // Builtin-Samples kommen aus dem lokalen Discovery-Pfad (kein
    // Remote-Writer); writer_guid + writer_strength sind Default-Init.
    // Builtin-Topics nutzen Shared-Ownership, also keine Filter-
    // Aktivierung im Reader.
    guard.push(crate::runtime::UserSample::Alive {
        payload: buf,
        writer_guid: [0u8; 16],
        writer_strength: 0,
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
    /// Konstruiert einen Builtin-Subscriber **mit** vor-erzeugten
    /// Readern fuer alle 4 Builtin-Topics. Wird genau einmal pro
    /// `DomainParticipant` (vom Konstruktor) gerufen.
    #[must_use]
    pub fn new() -> Self {
        // Subscriber mit Default-QoS — der Subscriber selbst ist
        // schlicht ein API-Wrapper, kein Runtime-Endpoint.
        let subscriber = Subscriber::new(SubscriberQos::default(), None);
        let inner = subscriber.inner.clone();

        let qos = builtin_reader_qos();

        // 4 Inboxes anlegen (shared zwischen DataReader und Runtime-
        // Discovery-Hook).
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

    /// Underlying Subscriber-Handle (API-Spiegel zu User-Subscribers).
    #[must_use]
    pub fn subscriber(&self) -> &Subscriber {
        &self.subscriber
    }

    /// Sinks fuer den Runtime-Discovery-Hook. Sollte nur intern
    /// (im DcpsRuntime) genutzt werden.
    #[must_use]
    pub fn sinks(&self) -> BuiltinSinks {
        self.sinks.clone()
    }

    /// Liefert eine Kopie des typed DataReaders fuer ein Builtin-Topic.
    /// Spec: `Subscriber::lookup_datareader(topic_name)` (DDS 1.4
    /// §2.2.2.5.1.5).
    ///
    /// **Topic-Name + Type-Parameter MUESSEN konsistent sein** (z.B.
    /// `lookup_datareader::<ParticipantBuiltinTopicData>("DCPSParticipant")`),
    /// sonst gibt es `BadParameter`.
    ///
    /// # Errors
    /// `BadParameter` wenn `topic_name` keinem Builtin-Topic entspricht
    /// oder Type-Parameter und Topic-Name auseinanderlaufen.
    pub fn lookup_datareader<T: BuiltinTopic>(&self, topic_name: &str) -> Result<DataReader<T>> {
        T::lookup(self, topic_name)
    }

    /// Direkter Zugriff auf den `DCPSParticipant`-Reader (Convenience-
    /// API, vermeidet generische Lookup-Pfade fuer Builtin-Topics).
    #[must_use]
    pub fn participant_reader(&self) -> DataReader<ParticipantBuiltinTopicData> {
        clone_reader(&self.participant_reader)
    }

    /// Direkter Zugriff auf den `DCPSTopic`-Reader.
    #[must_use]
    pub fn topic_reader(&self) -> DataReader<TopicBuiltinTopicData> {
        clone_reader(&self.topic_reader)
    }

    /// Direkter Zugriff auf den `DCPSPublication`-Reader.
    #[must_use]
    pub fn publication_reader(&self) -> DataReader<PublicationBuiltinTopicData> {
        clone_reader(&self.publication_reader)
    }

    /// Direkter Zugriff auf den `DCPSSubscription`-Reader.
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

/// Marker-Trait + Lookup-Routing fuer die 4 Builtin-Topic-Typen.
///
/// Damit `lookup_datareader::<T>(topic_name)` per Type-Parameter den
/// richtigen Reader trifft. Externe Typen koennen das Trait nicht
/// implementieren — `BuiltinTopic` ist sealed.
pub trait BuiltinTopic: DdsType + private::Sealed + Sized {
    /// Topic-Name aus DDS 1.4 §2.2.5.
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

/// Klont einen Reader, indem ein neuer Reader-Handle mit derselben
/// shared inbox + identischer Topic-/QoS-Snapshot konstruiert wird.
/// Wir koennen `DataReader<T>` nicht direkt `Clone` machen, weil die
/// `runtime`/`rx`-Felder (mpsc + Mutex) nicht `Clone` sind. Fuer
/// Builtin-Reader sind beide aber `None`, daher ist der Spezial-Klon
/// hier sicher.
fn clone_reader<T: DdsType + Send + Sync + 'static>(r: &DataReader<T>) -> DataReader<T> {
    DataReader::<T>::new_builtin(
        r.topic().clone(),
        r.qos().clone(),
        builtin_clone_subscriber_inner(r),
        r.__inbox_handle(),
    )
}

fn builtin_clone_subscriber_inner<T: DdsType>(_r: &DataReader<T>) -> Arc<SubscriberInner> {
    // Builtin-Subscriber ist statisch: wir konstruieren bei jedem
    // Lookup einen neuen Inner — er wird ohnehin nicht zur
    // Runtime-Routing-Lookup verwendet.
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
        // Smoke: der innere Subscriber-Handle ist zugreifbar (API-
        // Symmetrie zu User-Subscribers).
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
