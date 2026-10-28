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
//! Reliability=RELIABLE, Durability=TRANSIENT_LOCAL, History=KEEP_LAST(1);
//! the Subscriber's PRESENTATION has `access_scope = TOPIC`.
//! We set these defaults in `BuiltinSubscriber::new`. Users (per spec)
//! **cannot** modify them — `lookup_datareader` is read-only.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::Mutex;

use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy, PresentationAccessScope,
    PresentationQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};

use crate::builtin_topics::{
    ParticipantBuiltinTopicData, ParticipantBuiltinTopicDataSecure, PublicationBuiltinTopicData,
    PublicationBuiltinTopicDataSecure, SubscriptionBuiltinTopicData,
    SubscriptionBuiltinTopicDataSecure, TOPIC_NAME_DCPS_PARTICIPANT,
    TOPIC_NAME_DCPS_PARTICIPANTS_SECURE, TOPIC_NAME_DCPS_PUBLICATION,
    TOPIC_NAME_DCPS_PUBLICATIONS_SECURE, TOPIC_NAME_DCPS_SUBSCRIPTION,
    TOPIC_NAME_DCPS_SUBSCRIPTIONS_SECURE, TOPIC_NAME_DCPS_TOPIC, TopicBuiltinTopicData,
};
use crate::dds_type::DdsType;
use crate::error::{DdsError, Result};
use crate::instance_tracker::InstanceTracker;
use crate::qos::{DataReaderQos, SubscriberQos, TopicQos};
use crate::subscriber::{DataReader, Subscriber};
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

/// QoS of the built-in Subscriber (DDS 1.4 §2.2.5): PRESENTATION with
/// `access_scope = TOPIC`, `coherent_access` and `ordered_access` FALSE.
#[must_use]
pub fn builtin_subscriber_qos() -> SubscriberQos {
    SubscriberQos {
        presentation: PresentationQosPolicy {
            access_scope: PresentationAccessScope::Topic,
            coherent_access: false,
            ordered_access: false,
        },
        ..SubscriberQos::default()
    }
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
    /// Reader for `DCPSParticipantsSecure` (DDS-Security 1.2 §7.5.1.6):
    /// the participants that announced themselves over the secure SPDP.
    participants_secure_reader: DataReader<ParticipantBuiltinTopicDataSecure>,
    /// Reader for `DCPSPublicationsSecure` (§7.5.1.7): the writers announced
    /// over the secure SEDP.
    publications_secure_reader: DataReader<PublicationBuiltinTopicDataSecure>,
    /// Reader for `DCPSSubscriptionsSecure` (§7.5.1.8).
    subscriptions_secure_reader: DataReader<SubscriptionBuiltinTopicDataSecure>,
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
    /// Inbox of the `DCPSParticipantsSecure` reader.
    pub participants_secure: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox of the `DCPSPublicationsSecure` reader.
    pub publications_secure: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// Inbox of the `DCPSSubscriptionsSecure` reader.
    pub subscriptions_secure: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
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

    /// Marks the `DCPSPublication` instance for `guid` disposed — the remote
    /// deleted its DataWriter (SEDP dispose). Observers of the built-in
    /// publications reader then see the instance go `NOT_ALIVE_DISPOSED`
    /// (DDS-DCPS 1.4 §2.2.5). The instance key of a built-in publication is the
    /// endpoint GUID, which for a ≤16-byte key is its own KeyHash
    /// (DDSI-RTPS §9.6.4.8) — so `guid.to_bytes()` addresses exactly the
    /// instance an earlier ALIVE sample created.
    pub fn dispose_publication(&self, guid: zerodds_rtps::wire_types::Guid) {
        push_lifecycle(&self.publication, guid);
        push_lifecycle(&self.publications_secure, guid);
    }

    /// `DCPSSubscription` counterpart of [`Self::dispose_publication`].
    pub fn dispose_subscription(&self, guid: zerodds_rtps::wire_types::Guid) {
        push_lifecycle(&self.subscription, guid);
        push_lifecycle(&self.subscriptions_secure, guid);
    }

    /// Marks the `DCPSTopic` instance `key` disposed: no peer announces
    /// that topic and type any more, and no discovered endpoint names it.
    pub fn dispose_topic(&self, key: zerodds_rtps::wire_types::Guid) {
        push_lifecycle(&self.topic, key);
    }

    /// Marks a departed `DCPSParticipant` instance disposed.
    pub fn dispose_participant(&self, guid: zerodds_rtps::wire_types::Guid) {
        push_lifecycle(&self.participant, guid);
        push_lifecycle(&self.participants_secure, guid);
    }

    /// Pushes a participant announced over the secure SPDP into the
    /// `DCPSParticipantsSecure` reader.
    ///
    /// # Errors
    /// `WireError` if the encoding fails; `PreconditionNotMet` if the mutex is
    /// poisoned.
    pub fn push_participant_secure(
        &self,
        sample: &ParticipantBuiltinTopicDataSecure,
    ) -> Result<()> {
        push_into(&self.participants_secure, sample)
    }

    /// Pushes a writer announced over the secure SEDP into the
    /// `DCPSPublicationsSecure` reader.
    ///
    /// # Errors
    /// As [`Self::push_participant_secure`].
    pub fn push_publication_secure(
        &self,
        sample: &PublicationBuiltinTopicDataSecure,
    ) -> Result<()> {
        push_into(&self.publications_secure, sample)
    }

    /// Pushes a reader announced over the secure SEDP into the
    /// `DCPSSubscriptionsSecure` reader.
    ///
    /// # Errors
    /// As [`Self::push_participant_secure`].
    pub fn push_subscription_secure(
        &self,
        sample: &SubscriptionBuiltinTopicDataSecure,
    ) -> Result<()> {
        push_into(&self.subscriptions_secure, sample)
    }
}

/// Pushes a `NOT_ALIVE_DISPOSED` lifecycle marker for `guid` into a built-in
/// reader inbox. Silent no-op if the inbox mutex is poisoned (a dropped
/// discovery notification is non-fatal).
fn push_lifecycle(
    sink: &Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    guid: zerodds_rtps::wire_types::Guid,
) {
    if let Ok(mut guard) = sink.lock() {
        guard.push(crate::runtime::UserSample::Lifecycle {
            key_hash: guid.to_bytes(),
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            // Builtin-topic entities have no XTypes-registered
            // CompleteTypeObject and no instance key beyond the entity
            // GUID itself — no key holder to give.
            key_holder: None,
            source_timestamp: None,
            group: None,
            // The built-in topic's announcing writer is not known here.
            writer_guid: [0; 16],
            coherent_set_sn: None,
            source_sequence_number: -1,
        });
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
        key_hash: None,
        payload: crate::sample_bytes::SampleBytes::from_vec(buf),
        writer_guid: [0u8; 16],
        writer_strength: 0,
        // Built-in-topic samples are encoded ZeroDDS-internally —
        // XCDR1 baseline, little-endian.
        representation: 0,
        big_endian: false,
        source_timestamp: None,
        reception_timestamp: None,
        // Built-in-topic samples are not user data — a durability service does
        // not ingest them, so there is no source sequence to carry.
        source_sequence_number: -1,
        coherent_set_sn: None,
        // Built-in-topic samples carry no LIFESPAN — `0` == INFINITE.
        lifespan_nanos: 0,
        related_sample_identity: None,
        group: None,
        crypto_content: false,
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
        // The QoS of §2.2.5 — the subscriber itself is simply an API
        // wrapper, not a runtime endpoint.
        let subscriber = Subscriber::new(builtin_subscriber_qos(), None);
        // The built-in Subscriber exists with its participant, enabled
        // (DDS 1.4 §2.2.5), as its readers do.
        #[cfg(feature = "std")]
        subscriber.inner.entity_state.enable();
        // DDS 1.4 §2.2.5: "The built-in Subscriber and all of its built-in
        // Topics have nil listeners with all statuses appearing in their
        // listener masks. The built-in DataReaders have nil listeners with no
        // statuses in their masks." A status of a built-in reader ends at the
        // subscriber and does not reach a listener of the participant; the
        // readers' nil listeners with an empty mask are no listener.
        #[cfg(feature = "std")]
        subscriber.set_listener(None, crate::psm_constants::status::ANY);
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
        let part_secure_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));
        let pub_secure_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));
        let sub_secure_inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>> =
            Arc::new(Mutex::new(Vec::new()));

        let participant_reader = DataReader::new_builtin(
            Topic::<ParticipantBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_PARTICIPANT.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            part_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        let topic_reader = DataReader::new_builtin(
            Topic::<TopicBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_TOPIC.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            topic_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        let publication_reader = DataReader::new_builtin(
            Topic::<PublicationBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_PUBLICATION.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            pub_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        let subscription_reader = DataReader::new_builtin(
            Topic::<SubscriptionBuiltinTopicData>::new_orphan(
                TOPIC_NAME_DCPS_SUBSCRIPTION.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            sub_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        // DDS-Security 1.2 §7.5.1.6 to §7.5.1.8: "The QoS associated with the
        // [secure topics] shall be the same as for the DCPSPublications and
        // DCPSSubscriptions builtin Topic", RELIABLE among them.
        let participants_secure_reader = DataReader::new_builtin(
            Topic::<ParticipantBuiltinTopicDataSecure>::new_orphan(
                TOPIC_NAME_DCPS_PARTICIPANTS_SECURE.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            part_secure_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        let publications_secure_reader = DataReader::new_builtin(
            Topic::<PublicationBuiltinTopicDataSecure>::new_orphan(
                TOPIC_NAME_DCPS_PUBLICATIONS_SECURE.to_string(),
                TopicQos::default(),
            ),
            qos.clone(),
            inner.clone(),
            pub_secure_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );
        let subscriptions_secure_reader = DataReader::new_builtin(
            Topic::<SubscriptionBuiltinTopicDataSecure>::new_orphan(
                TOPIC_NAME_DCPS_SUBSCRIPTIONS_SECURE.to_string(),
                TopicQos::default(),
            ),
            qos,
            inner,
            sub_secure_inbox.clone(),
            Arc::new(crate::subscriber::SampleCache::new()),
            InstanceTracker::with_key_handles(),
        );

        // The four built-in readers are the readers of the built-in
        // Subscriber (DDS 1.4 §2.2.5): `lookup_datareader` and
        // `get_datareaders` find them like any reader.
        #[cfg(feature = "std")]
        {
            subscriber.track_reader(&participant_reader);
            subscriber.track_reader(&topic_reader);
            subscriber.track_reader(&publication_reader);
            subscriber.track_reader(&subscription_reader);
            subscriber.track_reader(&participants_secure_reader);
            subscriber.track_reader(&publications_secure_reader);
            subscriber.track_reader(&subscriptions_secure_reader);
        }

        Self {
            subscriber,
            participant_reader,
            topic_reader,
            publication_reader,
            subscription_reader,
            participants_secure_reader,
            publications_secure_reader,
            subscriptions_secure_reader,
            sinks: BuiltinSinks {
                participant: part_inbox,
                topic: topic_inbox,
                publication: pub_inbox,
                subscription: sub_inbox,
                participants_secure: part_secure_inbox,
                publications_secure: pub_secure_inbox,
                subscriptions_secure: sub_secure_inbox,
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

    /// Direct access to the `DCPSParticipantsSecure` reader (DDS-Security 1.2
    /// §7.5.1.6).
    #[must_use]
    pub fn participants_secure_reader(&self) -> DataReader<ParticipantBuiltinTopicDataSecure> {
        clone_reader(&self.participants_secure_reader)
    }

    /// Direct access to the `DCPSPublicationsSecure` reader (§7.5.1.7).
    #[must_use]
    pub fn publications_secure_reader(&self) -> DataReader<PublicationBuiltinTopicDataSecure> {
        clone_reader(&self.publications_secure_reader)
    }

    /// Direct access to the `DCPSSubscriptionsSecure` reader (§7.5.1.8).
    #[must_use]
    pub fn subscriptions_secure_reader(&self) -> DataReader<SubscriptionBuiltinTopicDataSecure> {
        clone_reader(&self.subscriptions_secure_reader)
    }
}

/// The built-in Subscriber is a Subscriber (DDS 1.4 §2.2.5): QoS, listener,
/// StatusCondition and the other operations are those of its
/// [`Subscriber`], whose readers are the four built-in ones, so
/// `lookup_datareader` and `get_datareaders` find them.
impl core::ops::Deref for BuiltinSubscriber {
    type Target = Subscriber;
    fn deref(&self) -> &Subscriber {
        &self.subscriber
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
}

mod private {
    pub trait Sealed {}
    impl Sealed for crate::builtin_topics::ParticipantBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::TopicBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::PublicationBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::SubscriptionBuiltinTopicData {}
    impl Sealed for crate::builtin_topics::ParticipantBuiltinTopicDataSecure {}
    impl Sealed for crate::builtin_topics::PublicationBuiltinTopicDataSecure {}
    impl Sealed for crate::builtin_topics::SubscriptionBuiltinTopicDataSecure {}
}

impl BuiltinTopic for ParticipantBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PARTICIPANT;
}

impl BuiltinTopic for TopicBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_TOPIC;
}

impl BuiltinTopic for PublicationBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PUBLICATION;
}

impl BuiltinTopic for SubscriptionBuiltinTopicData {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_SUBSCRIPTION;
}

impl BuiltinTopic for ParticipantBuiltinTopicDataSecure {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PARTICIPANTS_SECURE;
}

impl BuiltinTopic for PublicationBuiltinTopicDataSecure {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_PUBLICATIONS_SECURE;
}

impl BuiltinTopic for SubscriptionBuiltinTopicDataSecure {
    const TOPIC_NAME: &'static str = TOPIC_NAME_DCPS_SUBSCRIPTIONS_SECURE;
}

/// Clones a reader by constructing a new reader handle with the same
/// shared inbox/cache/instance-tracker + identical topic/QoS snapshot. We
/// cannot make `DataReader<T>` directly `Clone`, because the `runtime`/`rx`
/// fields (mpsc + Mutex) are not `Clone`. For built-in readers, however,
/// both are `None`, so the special-case clone here is safe. `cache`
/// and `instances` must travel with `inbox` — see `new_builtin`'s doc
/// comment — or a second clone's `take()`/`read()` would see nothing
/// (already drained into the first clone's private cache) and
/// `DomainParticipant::delete_contained_entities`'s inbox clear would
/// leave stale samples behind in any clone that had already read once.
/// A builtin topic has one reader; a lookup hands out another handle to it.
fn clone_reader<T: DdsType + Send + Sync + 'static>(r: &DataReader<T>) -> DataReader<T> {
    r.clone()
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

    /// DDS-Security 1.2 §7.5.1.6 to §7.5.1.8: the three secure built-in topics
    /// have the names the spec gives, a reader each, and the QoS of the
    /// DCPSPublications and DCPSSubscriptions topics — RELIABLE among them.
    #[test]
    fn the_secure_topics_have_their_names_and_the_reliable_qos() {
        use crate::builtin_topics::{
            ParticipantBuiltinTopicDataSecure, PublicationBuiltinTopicDataSecure,
            SubscriptionBuiltinTopicDataSecure,
        };
        let bs = BuiltinSubscriber::new();
        assert_eq!(
            bs.participants_secure_reader().topic().name(),
            "DCPSParticipantsSecure"
        );
        assert_eq!(
            bs.publications_secure_reader().topic().name(),
            "DCPSPublicationsSecure"
        );
        assert_eq!(
            bs.subscriptions_secure_reader().topic().name(),
            "DCPSSubscriptionsSecure"
        );
        let found = bs
            .lookup_datareader::<ParticipantBuiltinTopicDataSecure>("DCPSParticipantsSecure")
            .unwrap();
        assert_eq!(found.qos().reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(
            bs.publications_secure_reader().qos().reliability.kind,
            bs.publication_reader().qos().reliability.kind
        );
        assert_eq!(
            bs.subscriptions_secure_reader().qos(),
            bs.subscription_reader().qos()
        );
        // A name for another topic is refused.
        assert!(
            bs.lookup_datareader::<PublicationBuiltinTopicDataSecure>("DCPSPublications")
                .is_none()
        );
        assert!(
            bs.lookup_datareader::<SubscriptionBuiltinTopicDataSecure>("DCPSSubscriptionsSecure")
                .is_some()
        );
    }

    /// §2.2.5: the built-in Subscriber has a nil listener with all statuses
    /// in its mask, so what a built-in reader reports does not reach the
    /// listener of the participant.
    #[cfg(feature = "std")]
    #[test]
    fn a_builtin_status_does_not_reach_the_participant_listener() {
        use crate::listener::DomainParticipantListener;
        use core::sync::atomic::{AtomicU32, Ordering};

        #[derive(Default)]
        struct Told(AtomicU32);
        impl DomainParticipantListener for Told {
            fn on_data_available(&self, _reader: crate::instance_handle::InstanceHandle) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            fn on_data_on_readers(&self, _subscriber: crate::instance_handle::InstanceHandle) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let p = crate::participant::DomainParticipant::new(
            0,
            crate::qos::DomainParticipantQos::default(),
        );
        let told = Arc::new(Told::default());
        p.set_listener(
            Some(told.clone()),
            crate::psm_constants::status::DATA_AVAILABLE
                | crate::psm_constants::status::DATA_ON_READERS,
        );
        let builtin = p.get_builtin_subscriber();
        let reader = builtin.participant_reader();
        assert!(reader.get_listener().is_none());
        assert!(
            builtin.subscriber().get_listener().is_none(),
            "the nil listener reads back as none"
        );
        reader.notify_data_arrived();
        assert_eq!(told.0.load(Ordering::SeqCst), 0);
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

    /// DDS 1.4 §2.2.5: "PRESENTATION access_scope = TOPIC coherent_access =
    /// FALSE ordered_access = FALSE" for the built-in Subscriber. It had
    /// the default, INSTANCE.
    #[test]
    fn the_builtin_subscriber_presents_by_topic() {
        use crate::entity::Entity;
        let bs = BuiltinSubscriber::new();
        let p = bs.subscriber().get_qos().presentation;
        assert_eq!(p.access_scope, PresentationAccessScope::Topic);
        assert!(!p.coherent_access);
        assert!(!p.ordered_access);
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
        assert!(
            bs.lookup_datareader::<ParticipantBuiltinTopicData>("DCPSPublication")
                .is_none()
        );
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
            key: TopicBuiltinTopicData::synthesize_key(
                zerodds_rtps::wire_types::GuidPrefix::from_bytes([1; 12]),
                "MyT",
                "MyType",
            ),
            name: "MyT".to_string(),
            type_name: "MyType".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
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
            ..PublicationBuiltinTopicData::default()
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
            ..SubscriptionBuiltinTopicData::default()
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
        assert!(
            bs.lookup_datareader::<TopicBuiltinTopicData>("DCPSParticipant")
                .is_none()
        );
    }

    #[test]
    fn lookup_publication_with_wrong_topic_name() {
        let bs = BuiltinSubscriber::new();
        assert!(
            bs.lookup_datareader::<PublicationBuiltinTopicData>("DCPSTopic")
                .is_none()
        );
    }

    #[test]
    fn lookup_subscription_with_wrong_topic_name() {
        let bs = BuiltinSubscriber::new();
        assert!(
            bs.lookup_datareader::<SubscriptionBuiltinTopicData>("DCPSTopic")
                .is_none()
        );
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
