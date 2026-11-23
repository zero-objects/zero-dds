// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the port tests share: a keyed type, two participants on one bus
//! and recorders for the listeners a component provides.

#![allow(
    dead_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::publisher::AnyDataWriter;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::status::{
    InconsistentTopicStatus, OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus,
};
use zerodds_dcps::{
    AnyDataReader, DataReaderQos, DataWriter, DataWriterQos, DdsType, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, FilterValue, Publisher, PublisherQos,
    Subscriber, SubscriberQos, Topic, TopicDescriptionHandle, TopicQos,
};
use zerodds_dds4ccm::ccm_dds::{DdsEntity, StatusKind};
use zerodds_dds4ccm::{
    Component, ConnectorStatusListener, Container, DdsEvent, DdsState, ErrorListener, Listener,
    PortStatusListener, ReadInfo, Receptacle, StateListener,
};
use zerodds_qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};

/// A `value` that cannot be encoded: every DDS order with it fails.
pub const POISON: u32 = u32::MAX;

/// From this `id` on, `encode` and `encode_key_holder_be` note whether the
/// probed publisher has a coherent set open.
pub const PROBED: u32 = 9000;

static PROBE: Mutex<Option<(Publisher, ThreadId)>> = Mutex::new(None);
static OPEN_AT_ENCODE: Mutex<Vec<bool>> = Mutex::new(Vec::new());
static OPEN_AT_KEY: Mutex<Vec<bool>> = Mutex::new(Vec::new());

/// Notes, for every sample from [`PROBED`] on that is encoded, and for
/// every key of such a sample this thread computes, whether `publisher`
/// has a coherent set open.
pub fn probe_coherent_changes(publisher: Publisher) {
    *PROBE.lock().unwrap() = Some((publisher, std::thread::current().id()));
}

/// What the probe noted at the samples since the last call.
pub fn open_at_encode() -> Vec<bool> {
    std::mem::take(&mut *OPEN_AT_ENCODE.lock().unwrap())
}

/// What the probe noted at the keys since the last call. A DataWriter
/// computes the key of the datum of every order, also of those that do not
/// encode a sample (`register_instance`, `dispose` of a keyed type). Only
/// the thread that set the probe is noted: the DDS orders run on the
/// thread of the caller, a receiving side computes keys on its own.
pub fn open_at_key() -> Vec<bool> {
    std::mem::take(&mut *OPEN_AT_KEY.lock().unwrap())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Track {
    pub id: u32,
    pub value: u32,
}

pub fn track(id: u32, value: u32) -> Track {
    Track { id, value }
}

impl DdsType for Track {
    const TYPE_NAME: &'static str = "test::Dds4CcmTrack";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        if self.value == POISON {
            return Err(EncodeError::Invalid { what: "poison" });
        }
        if self.id >= PROBED
            && let Some((publisher, _)) = PROBE.lock().unwrap().as_ref()
        {
            OPEN_AT_ENCODE
                .lock()
                .unwrap()
                .push(publisher.coherent_changes_open());
        }
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize, what: &'static str| -> Result<u32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_be_bytes)
                .ok_or(DecodeError::Invalid { what })
        };
        Ok(Self {
            id: word(0, "id")?,
            value: word(4, "value")?,
        })
    }

    fn field_value(&self, path: &str) -> Option<FilterValue> {
        match path {
            "id" => Some(FilterValue::Int(i64::from(self.id))),
            "value" => Some(FilterValue::Int(i64::from(self.value))),
            _ => None,
        }
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        if self.id >= PROBED
            && let Some((publisher, thread)) = PROBE.lock().unwrap().as_ref()
            && *thread == std::thread::current().id()
        {
            OPEN_AT_KEY
                .lock()
                .unwrap()
                .push(publisher.coherent_changes_open());
        }
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let id: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "key" })?;
        Ok(Self {
            id: u32::from_be_bytes(id),
            value: 0,
        })
    }
}

/// A runtime configuration with an SPDP multicast group of its own, so the
/// participants of one test find each other and no others.
pub fn isolated_cfg() -> RuntimeConfig {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let raw = std::process::id().wrapping_mul(2_654_435_761) ^ slot.wrapping_mul(0x9E37_79B9);
    let x = 1 + ((raw >> 8) & 0x7F) as u8;
    let y = 2 + (raw & 0x7F) as u8;
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        nack_response_delay: Duration::ZERO,
        reader_heartbeat_response_delay: Duration::ZERO,
        spdp_multicast_group: Ipv4Addr::new(239, 255, x, y),
        ..RuntimeConfig::default()
    }
}

/// RELIABLE and KEEP_ALL: a test sees every sample that was written.
pub fn reliable_keep_all() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..ReliabilityQosPolicy::default()
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

/// Polls `holds` for at most ten seconds.
pub fn eventually(what: &str, mut holds: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if holds() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("not within 10 s: {what}");
}

/// Two participants of one domain: one publishes, one subscribes.
pub struct World {
    pub publishing: DomainParticipant,
    pub subscribing: DomainParticipant,
    pub pub_topic: Topic<Track>,
    pub sub_topic: Topic<Track>,
    pub publisher: Publisher,
    pub subscriber: Subscriber,
    /// `error_listener` of the connector on the publishing side.
    pub pub_errors: Arc<ErrorListener>,
    /// `error_listener` of the connector on the subscribing side.
    pub sub_errors: Arc<ErrorListener>,
}

impl World {
    pub fn new(topic: &str) -> Self {
        let domain = zerodds_test_domains::unique_domain();
        let cfg = isolated_cfg();
        let factory = DomainParticipantFactory::instance();
        let publishing = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("publishing participant");
        let subscribing = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("subscribing participant");
        let pub_topic = publishing
            .create_topic::<Track>(topic, TopicQos::default())
            .expect("topic of the publishing side");
        let sub_topic = subscribing
            .create_topic::<Track>(topic, TopicQos::default())
            .expect("topic of the subscribing side");
        Self {
            publisher: publishing.create_publisher(PublisherQos::default()),
            subscriber: subscribing.create_subscriber(SubscriberQos::default()),
            publishing,
            subscribing,
            pub_topic,
            sub_topic,
            pub_errors: Arc::new(Receptacle::new()),
            sub_errors: Arc::new(Receptacle::new()),
        }
    }

    /// A DataWriter of the publishing side with `qos`.
    pub fn writer_with(&self, qos: DataWriterQos) -> DataWriter<Track> {
        self.publisher
            .create_datawriter::<Track>(&self.pub_topic, qos)
            .expect("writer")
    }

    /// A DataWriter of the publishing side with the default QoS.
    pub fn writer(&self) -> DataWriter<Track> {
        self.writer_with(DataWriterQos::default())
    }

    /// Waits until `writer` matched `readers` readers.
    pub fn matched(&self, writer: &DataWriter<Track>, readers: usize) {
        writer
            .wait_for_matched_subscription(readers, Duration::from_secs(15))
            .expect("the writer matches the readers of the test");
    }

    /// Waits until every reliable reader has what `writer` wrote.
    pub fn delivered(&self, writer: &DataWriter<Track>) {
        writer
            .wait_for_acknowledgments(Duration::from_secs(10))
            .expect("the readers acknowledge what was written");
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let factory = DomainParticipantFactory::instance();
        for participant in [&self.publishing, &self.subscribing] {
            let _ = participant.delete_contained_entities();
            let _ = factory.delete_participant(participant);
        }
    }
}

/// Two containers of one domain on a bus of their own: the components of
/// one publish, those of the other subscribe. Each container has its own
/// DomainParticipants, so the two sides meet through discovery.
pub struct Deployment {
    pub domain: i32,
    pub publishing: Container,
    pub subscribing: Container,
}

impl Deployment {
    pub fn new() -> Self {
        let cfg = isolated_cfg();
        Self {
            domain: zerodds_test_domains::unique_domain(),
            publishing: Container::with_runtime_config("publishing", cfg.clone()),
            subscribing: Container::with_runtime_config("subscribing", cfg),
        }
    }

    /// A component of the publishing container.
    pub fn publisher(&self) -> Component {
        self.publishing.component("source")
    }

    /// A component of the subscribing container.
    pub fn subscriber(&self) -> Component {
        self.subscribing.component("sink")
    }

    /// A configured `DDS_State` connector of the deployment's domain for
    /// the topic `topic` with the QoS profile `qos_profile`.
    pub fn state(&self, topic: &str, qos_profile: &str) -> DdsState<Track> {
        let connector = DdsState::<Track>::new();
        connector.set_domain_id(self.domain).unwrap();
        connector.set_topic_name(topic).unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        connector.set_qos_profile(qos_profile).unwrap();
        connector.configuration_complete().unwrap();
        connector
    }

    /// A configured `DDS_Event` connector; see [`Self::state`].
    pub fn event(&self, topic: &str, qos_profile: &str) -> DdsEvent<Track> {
        let connector = DdsEvent::<Track>::new();
        connector.set_domain_id(self.domain).unwrap();
        connector.set_topic_name(topic).unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        connector.set_qos_profile(qos_profile).unwrap();
        connector.configuration_complete().unwrap();
        connector
    }
}

impl Drop for Deployment {
    fn drop(&mut self) {
        // What a test left in the default entities keeps them; the process
        // of the test ends after it.
        let _ = self.publishing.remove();
        let _ = self.subscribing.remove();
    }
}

/// A profile document with one profile `P` of `containers`.
pub fn profile(containers: &str) -> String {
    format!("<dds><qos_profile name=\"P\">{containers}</qos_profile></dds>")
}

/// RELIABLE and KEEP_ALL on both ends, as a profile: a test sees every
/// sample that was written.
pub const RELIABLE_KEEP_ALL: &str = "\
    <datawriter_qos>\
      <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
      <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>\
    </datawriter_qos>\
    <datareader_qos>\
      <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
      <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>\
    </datareader_qos>";

/// What a `Listener` was called with, and on which thread.
#[derive(Default)]
pub struct Heard {
    pub one: Mutex<Vec<(Track, ReadInfo)>>,
    pub many: Mutex<Vec<Vec<(Track, ReadInfo)>>>,
    /// The thread of each call: its id and its name.
    pub threads: Mutex<Vec<(ThreadId, Option<String>)>>,
}

impl Heard {
    fn note_thread(&self) {
        let thread = std::thread::current();
        self.threads
            .lock()
            .unwrap()
            .push((thread.id(), thread.name().map(str::to_owned)));
    }

    /// The names of the threads that called, without repetition.
    pub fn thread_names(&self) -> Vec<Option<String>> {
        let mut names: Vec<_> = self
            .threads
            .lock()
            .unwrap()
            .iter()
            .map(|(_, name)| name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Listener<Track> for Heard {
    fn on_one_data(&self, datum: &Track, info: &ReadInfo) {
        self.note_thread();
        self.one.lock().unwrap().push((datum.clone(), *info));
    }

    fn on_many_data(&self, data: &[Track], infos: &[ReadInfo]) {
        assert_eq!(data.len(), infos.len());
        self.note_thread();
        self.many
            .lock()
            .unwrap()
            .push(data.iter().cloned().zip(infos.iter().copied()).collect());
    }
}

/// The operations of a `StateListener` in the order they were called, each
/// with its data.
#[derive(Default)]
pub struct StateHeard {
    pub calls: Mutex<Vec<(&'static str, Vec<Track>)>>,
}

impl StateHeard {
    pub fn calls(&self) -> Vec<(&'static str, Vec<Track>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl StateListener<Track> for StateHeard {
    fn on_creation(&self, datum: &Track, _info: &ReadInfo) {
        self.calls
            .lock()
            .unwrap()
            .push(("creation", vec![datum.clone()]));
    }

    fn on_one_update(&self, datum: &Track, _info: &ReadInfo) {
        self.calls
            .lock()
            .unwrap()
            .push(("one_update", vec![datum.clone()]));
    }

    fn on_many_updates(&self, data: &[Track], infos: &[ReadInfo]) {
        assert_eq!(data.len(), infos.len());
        self.calls
            .lock()
            .unwrap()
            .push(("many_updates", data.to_vec()));
    }

    fn on_deletion(&self, datum: &Track, _info: &ReadInfo) {
        self.calls
            .lock()
            .unwrap()
            .push(("deletion", vec![datum.clone()]));
    }
}

/// The operations of the two status listeners in the order they were
/// called.
#[derive(Default)]
pub struct Statuses {
    pub events: Mutex<Vec<String>>,
    /// The name of the thread of each call.
    pub threads: Mutex<Vec<Option<String>>>,
}

impl Statuses {
    pub fn has(&self, event: &str) -> bool {
        self.events.lock().unwrap().iter().any(|e| e == event)
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }

    fn note(&self, event: String) {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().name().map(str::to_owned));
        self.events.lock().unwrap().push(event);
    }
}

impl PortStatusListener for Statuses {
    fn on_requested_deadline_missed(
        &self,
        _the_reader: &AnyDataReader,
        _status: RequestedDeadlineMissedStatus,
    ) {
        self.note("requested_deadline_missed".into());
    }

    fn on_sample_lost(&self, _the_reader: &AnyDataReader, _status: SampleLostStatus) {
        self.note("sample_lost".into());
    }
}

impl ConnectorStatusListener for Statuses {
    fn on_inconsistent_topic(
        &self,
        _the_topic: &TopicDescriptionHandle,
        _status: InconsistentTopicStatus,
    ) {
        self.note("inconsistent_topic".into());
    }

    fn on_requested_incompatible_qos(
        &self,
        _the_reader: &AnyDataReader,
        _status: RequestedIncompatibleQosStatus,
    ) {
        self.note("requested_incompatible_qos".into());
    }

    fn on_sample_rejected(&self, _the_reader: &AnyDataReader, _status: SampleRejectedStatus) {
        self.note("sample_rejected".into());
    }

    fn on_offered_deadline_missed(
        &self,
        _the_writer: &AnyDataWriter,
        _status: OfferedDeadlineMissedStatus,
    ) {
        self.note("offered_deadline_missed".into());
    }

    fn on_offered_incompatible_qos(
        &self,
        _the_writer: &AnyDataWriter,
        _status: OfferedIncompatibleQosStatus,
    ) {
        self.note("offered_incompatible_qos".into());
    }

    fn on_unexpected_status(&self, the_entity: &DdsEntity, status_kind: StatusKind) {
        let entity = match the_entity {
            DdsEntity::Topic(_) => "topic",
            DdsEntity::DataReader(_) => "reader",
            DdsEntity::DataWriter(_) => "writer",
        };
        self.note(format!("unexpected:{entity}:{status_kind:#x}"));
    }
}

/// The `(id, value)` of each sample, sorted.
pub fn pairs(data: &[Track]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<_> = data.iter().map(|t| (t.id, t.value)).collect();
    pairs.sort_unstable();
    pairs
}

/// `get_one` until it returned `count` samples; those that report the
/// deletion of their instance are returned apart, as the ids.
pub fn get_all(
    getter: &dyn zerodds_dds4ccm::Getter<Track>,
    count: usize,
) -> (Vec<(u32, u32)>, Vec<u32>) {
    getter.set_time_out(zerodds_dcps::Duration::from_core(
        std::time::Duration::from_millis(200),
    ));
    let (mut got, mut deleted) = (Vec::new(), Vec::new());
    eventually("the port gets the samples", || {
        let (mut datum, mut info) = (Track::default(), ReadInfo::default());
        while getter.get_one(&mut datum, &mut info).unwrap() {
            if info.instance_status == zerodds_dds4ccm::InstanceStatus::InstanceDeleted {
                deleted.push(datum.id);
            } else {
                got.push(datum.clone());
            }
        }
        got.len() + deleted.len() >= count
    });
    (pairs(&got), deleted)
}
