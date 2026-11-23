// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data access, subscribing side (DDS4CCM 1.1 §7.2.2.1.2 to §7.2.2.1.5):
//! interface `Reader`, interface `Getter`, interface `Listener`, interface
//! `StateListener`, the listener controls, `ContentFilterSetting` and the
//! status listeners behind the ports `DDS_Read`, `DDS_Get`, `DDS_Listen`
//! and `DDS_StateListen`, between two participants.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;
use std::time::Instant;

use common::{Heard, StateHeard, Statuses, Track, World, eventually, reliable_keep_all, track};
use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::error::return_code;
use zerodds_dcps::psm_constants::status as status_kind;
use zerodds_dcps::{
    DataReaderQos, DataWriter, DataWriterQos, DdsType, Duration, HANDLE_NIL, InstanceHandle,
    TopicQos,
};
use zerodds_dds4ccm::{
    AccessStatus, ConnectorStatusListener, ContentFilterSetting, DataListenerControl, DdsGet,
    DdsListen, DdsRead, DdsStateListen, Getter, InstanceStatus, InternalError, ListenerMode,
    NonExistent, PortStatusListener, QueryFilter, ReadInfo, Reader, ReaderError,
    StateListenerControl, UNLIMITED,
};
use zerodds_qos::{
    DeadlineQosPolicy, HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};

/// A writer that keeps every sample until the readers have it.
fn keep_all_writer(w: &World) -> DataWriter<Track> {
    w.writer_with(DataWriterQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataWriterQos::default()
    })
}

/// Completes the configuration of a subscribing port of `w` with a reader
/// that keeps every sample, and returns the writer it matched.
macro_rules! configured {
    ($w:expr, $port:expr) => {{
        $port
            .configuration_complete(
                &$w.subscribing,
                &$w.subscriber,
                &$w.sub_topic,
                reliable_keep_all(),
                Arc::clone(&$w.sub_errors),
            )
            .expect("configuration_complete");
        let writer = keep_all_writer(&$w);
        $w.matched(&writer, 1);
        writer
    }};
}

/// Writes `samples` and waits until the reader of the port has them.
fn publish(w: &World, writer: &DataWriter<Track>, samples: &[(u32, u32)]) {
    for (id, value) in samples {
        writer.write(&track(*id, *value)).unwrap();
    }
    w.delivered(writer);
}

fn pairs(data: &[Track]) -> Vec<(u32, u32)> {
    data.iter().map(|t| (t.id, t.value)).collect()
}

fn millis(ms: u64) -> Duration {
    Duration::from_core(std::time::Duration::from_millis(ms))
}

/// `read_all` once the port holds `count` samples that are alive.
fn read_all_of(reader: &dyn Reader<Track>, count: usize) -> (Vec<Track>, Vec<ReadInfo>) {
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port holds the samples", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == count
    });
    assert_eq!(data.len(), infos.len());
    (data, infos)
}

/// `read_last`: the last sample of every instance; `read_all`: all
/// samples of all instances, ordered by instance; without data both are
/// an empty sequence, and what the sequences held before is replaced.
#[test]
fn dds4ccm_7_2_2_1_2_read_last_returns_one_sample_per_instance_and_read_all_every_sample() {
    let w = World::new("Dds4CcmRead");
    let port = DdsRead::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().expect("data of a configured port");

    let (mut data, mut infos) = (vec![track(99, 99)], vec![ReadInfo::default()]);
    reader.read_last(&mut data, &mut infos).unwrap();
    assert!(
        data.is_empty() && infos.is_empty(),
        "no data is a void sequence"
    );
    let (mut data, mut infos) = (vec![track(99, 99)], vec![ReadInfo::default()]);
    reader.read_all(&mut data, &mut infos).unwrap();
    assert!(data.is_empty() && infos.is_empty());

    publish(
        &w,
        &writer,
        &[(1, 10), (2, 20), (1, 11), (3, 30), (2, 21), (1, 12)],
    );
    let (data, infos) = read_all_of(&*reader, 6);
    // Ordered by instance: the samples of one instance are consecutive, in
    // the order they were written.
    let got = pairs(&data);
    for id in 1..=3 {
        let first = got.iter().position(|(i, _)| *i == id).unwrap();
        let last = got.iter().rposition(|(i, _)| *i == id).unwrap();
        assert!(
            got[first..=last].iter().all(|(i, _)| *i == id),
            "instance {id} is not consecutive: {got:?}"
        );
    }
    let of = |id: u32| -> Vec<u32> {
        got.iter()
            .filter(|(i, _)| *i == id)
            .map(|(_, v)| *v)
            .collect()
    };
    assert_eq!(of(1), [10, 11, 12]);
    assert_eq!(of(2), [20, 21]);
    assert_eq!(of(3), [30]);
    // The handle of each info is the one of its datum.
    let entity = port.dds_entity().unwrap();
    for (datum, info) in data.iter().zip(&infos) {
        assert_eq!(info.instance_handle, entity.lookup_instance(datum));
        assert!(!info.instance_handle.is_nil());
    }

    let (mut data, mut infos) = (Vec::new(), Vec::new());
    reader.read_last(&mut data, &mut infos).unwrap();
    let mut last = pairs(&data);
    last.sort_unstable();
    assert_eq!(last, [(1, 12), (2, 21), (3, 30)]);
    assert_eq!(infos.len(), 3);
}

/// The reads of a Reader select READ or NOT_READ and ALIVE: a sample is
/// returned again (ALREADY_SEEN after FRESH_INFO), with the source
/// timestamp of DDS; an instance that was disposed is not returned.
#[test]
fn dds4ccm_7_2_2_1_2_a_read_returns_seen_samples_again_and_alive_instances_only() {
    let w = World::new("Dds4CcmReadMasks");
    let port = DdsRead::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().unwrap();

    let before = zerodds_dcps::get_current_time();
    publish(&w, &writer, &[(1, 10)]);
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the first read that returns the sample", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        !data.is_empty()
    });
    assert_eq!(pairs(&data), [(1, 10)]);
    assert_eq!(infos[0].access_status, AccessStatus::FreshInfo);
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceCreated);
    assert!(infos[0].source_timestamp >= before);
    assert!(infos[0].source_timestamp <= zerodds_dcps::get_current_time());
    let first = infos[0];

    reader.read_all(&mut data, &mut infos).unwrap();
    assert_eq!(pairs(&data), [(1, 10)]);
    assert_eq!(infos[0].access_status, AccessStatus::AlreadySeen);
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceCreated);
    assert_eq!(infos[0].source_timestamp, first.source_timestamp);
    assert_eq!(infos[0].instance_handle, first.instance_handle);

    publish(&w, &writer, &[(2, 20)]);
    read_all_of(&*reader, 2);
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    w.delivered(&writer);
    eventually("the disposed instance is not read any more", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        pairs(&data) == [(2, 20)]
    });
    reader.read_last(&mut data, &mut infos).unwrap();
    assert_eq!(pairs(&data), [(2, 20)]);
}

/// `read_one_last` and `read_one_all`: the instance is the one of the
/// handle if it is not HANDLE_NIL, else the one of the key of the datum; a
/// handle that is not in accordance with the key is an InternalError with
/// the DDS code; an instance DDS holds no data of is NonExistent.
#[test]
fn dds4ccm_7_2_2_1_2_read_one_last_of_an_unknown_instance_is_non_existent() {
    let w = World::new("Dds4CcmReadOne");
    let port = DdsRead::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().unwrap();
    let non_existent = ReaderError::NonExistent(NonExistent { indexes: vec![0] });

    let mut datum = track(1, 0);
    let mut info = ReadInfo::default();
    assert_eq!(
        reader.read_one_last(&mut datum, &mut info, HANDLE_NIL),
        Err(non_existent.clone())
    );
    assert_eq!(datum, track(1, 0), "the datum is untouched");
    let (mut data, mut infos) = (vec![track(99, 99)], vec![ReadInfo::default()]);
    assert_eq!(
        reader.read_one_all(&track(1, 0), &mut data, &mut infos, HANDLE_NIL),
        Err(non_existent.clone())
    );
    assert!(data.is_empty() && infos.is_empty());

    publish(&w, &writer, &[(1, 10), (2, 20), (1, 11), (2, 21), (1, 12)]);
    read_all_of(&*reader, 5);
    let entity = port.dds_entity().unwrap();
    let one = entity.lookup_instance(&track(1, 0));
    let two = entity.lookup_instance(&track(2, 0));

    // By the key.
    let mut datum = track(1, 0);
    reader
        .read_one_last(&mut datum, &mut info, HANDLE_NIL)
        .unwrap();
    assert_eq!(datum, track(1, 12));
    assert_eq!(info.instance_handle, one);
    assert_eq!(info.instance_status, InstanceStatus::InstanceUpdated);
    assert_eq!(info.access_status, AccessStatus::AlreadySeen);
    // By the handle.
    let mut datum = track(2, 0);
    reader.read_one_last(&mut datum, &mut info, two).unwrap();
    assert_eq!(datum, track(2, 21));
    assert_eq!(info.instance_handle, two);

    reader
        .read_one_all(&track(1, 0), &mut data, &mut infos, HANDLE_NIL)
        .unwrap();
    assert_eq!(pairs(&data), [(1, 10), (1, 11), (1, 12)]);
    assert!(infos.iter().all(|i| i.instance_handle == one));
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceCreated);
    reader
        .read_one_all(&track(2, 0), &mut data, &mut infos, two)
        .unwrap();
    assert_eq!(pairs(&data), [(2, 20), (2, 21)]);

    // A handle that is not in accordance with the key.
    let mismatch = ReaderError::InternalError(InternalError {
        error_code: return_code::PRECONDITION_NOT_MET,
        index: 0,
    });
    let mut datum = track(1, 0);
    assert_eq!(
        reader.read_one_last(&mut datum, &mut info, two),
        Err(mismatch.clone())
    );
    assert_eq!(
        reader.read_one_all(&track(1, 0), &mut data, &mut infos, two),
        Err(mismatch)
    );
    let unknown = ReaderError::InternalError(InternalError {
        error_code: return_code::BAD_PARAMETER,
        index: 0,
    });
    assert_eq!(
        reader.read_one_last(&mut datum, &mut info, InstanceHandle::from_raw(0x00DE_AD00)),
        Err(unknown)
    );
    // An instance of another key.
    let mut datum = track(3, 0);
    assert_eq!(
        reader.read_one_last(&mut datum, &mut info, HANDLE_NIL),
        Err(non_existent.clone())
    );

    // A disposed instance does not exist.
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    w.delivered(&writer);
    let mut datum = track(1, 0);
    eventually("the disposed instance is NonExistent", || {
        reader.read_one_last(&mut datum, &mut info, HANDLE_NIL) == Err(non_existent.clone())
    });
    assert_eq!(
        reader.read_one_all(&track(1, 0), &mut data, &mut infos, one),
        Err(non_existent)
    );
}

/// A type without a key: DDS has one instance of it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Level {
    value: u32,
}

impl DdsType for Level {
    const TYPE_NAME: &'static str = "test::Dds4CcmLevel";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let value: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "value" })?;
        Ok(Self {
            value: u32::from_be_bytes(value),
        })
    }
}

/// "In case of a keyless topic, the last value in the topic will be
/// returned" by `read_one_last` and "all values" by `read_one_all`,
/// whatever the datum.
#[test]
fn dds4ccm_7_2_2_1_2_read_one_of_a_keyless_topic_reads_the_one_instance() {
    let w = World::new("Dds4CcmUnusedKeyed");
    let pub_topic = w
        .publishing
        .create_topic::<Level>("Dds4CcmKeyless", TopicQos::default())
        .unwrap();
    let sub_topic = w
        .subscribing
        .create_topic::<Level>("Dds4CcmKeyless", TopicQos::default())
        .unwrap();
    let port = DdsRead::<Level>::new();
    port.configuration_complete(
        &w.subscribing,
        &w.subscriber,
        &sub_topic,
        reliable_keep_all(),
        Arc::clone(&w.sub_errors),
    )
    .unwrap();
    let writer = w
        .publisher
        .create_datawriter::<Level>(
            &pub_topic,
            DataWriterQos {
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataWriterQos::default()
            },
        )
        .unwrap();
    writer
        .wait_for_matched_subscription(1, std::time::Duration::from_secs(15))
        .unwrap();
    let reader = port.data().unwrap();

    for value in [5, 6, 7] {
        writer.write(&Level { value }).unwrap();
    }
    writer
        .wait_for_acknowledgments(std::time::Duration::from_secs(10))
        .unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port holds the three values", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == 3
    });
    let mut datum = Level { value: 1000 };
    let mut info = ReadInfo::default();
    reader
        .read_one_last(&mut datum, &mut info, HANDLE_NIL)
        .unwrap();
    assert_eq!(datum.value, 7);
    reader
        .read_one_all(&Level { value: 1000 }, &mut data, &mut infos, HANDLE_NIL)
        .unwrap();
    let values: Vec<u32> = data.iter().map(|l| l.value).collect();
    assert_eq!(values, [5, 6, 7]);
    reader.read_last(&mut data, &mut infos).unwrap();
    assert_eq!(data, [Level { value: 7 }]);
}

/// Attribute `query`: "acts as a filter for all the read operations made
/// through a port", an empty expression means no query, and a value DDS
/// does not accept is an InternalError that leaves the attribute.
#[test]
fn dds4ccm_7_2_2_1_2_a_query_filters_every_read() {
    let w = World::new("Dds4CcmQuery");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().unwrap();
    let getter = port.fresh_data().unwrap();
    assert_eq!(
        reader.query(),
        QueryFilter::default(),
        "no query by default"
    );

    publish(&w, &writer, &[(1, 5), (1, 20), (2, 6), (2, 30), (3, 7)]);
    read_all_of(&*reader, 5);

    let query = QueryFilter::new("value > %0", vec!["15".into()]);
    reader.set_query(query.clone()).unwrap();
    assert_eq!(reader.query(), query);
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    reader.read_all(&mut data, &mut infos).unwrap();
    assert_eq!(pairs(&data), [(1, 20), (2, 30)]);
    reader.read_last(&mut data, &mut infos).unwrap();
    assert_eq!(pairs(&data), [(1, 20), (2, 30)]);
    reader
        .read_one_all(&track(2, 0), &mut data, &mut infos, HANDLE_NIL)
        .unwrap();
    assert_eq!(pairs(&data), [(2, 30)]);
    let mut datum = track(1, 0);
    let mut info = ReadInfo::default();
    reader
        .read_one_last(&mut datum, &mut info, HANDLE_NIL)
        .unwrap();
    assert_eq!(datum, track(1, 20));
    // The instance lives and the query lets none of its samples through:
    // DDS has no data, which is not NonExistent.
    let mut datum = track(3, 0);
    assert_eq!(
        reader.read_one_last(&mut datum, &mut info, HANDLE_NIL),
        Err(ReaderError::InternalError(InternalError {
            error_code: return_code::NO_DATA,
            index: 0
        }))
    );
    reader
        .read_one_all(&track(3, 0), &mut data, &mut infos, HANDLE_NIL)
        .unwrap();
    assert!(data.is_empty());

    // The Getter of the port gets through the query of the Reader.
    publish(&w, &writer, &[(4, 8), (4, 40), (5, 9)]);
    getter.set_time_out(millis(5000));
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    assert_eq!(pairs(&data), [(4, 40)]);

    // A value DDS refuses: InternalError, the attribute keeps its value.
    let refused = reader
        .set_query(QueryFilter::new("value >", Vec::new()))
        .unwrap_err();
    assert_ne!(refused.error_code, return_code::OK);
    assert_eq!(refused.index, 0);
    assert_eq!(reader.query(), query);
    reader.read_all(&mut data, &mut infos).unwrap();
    assert_eq!(pairs(&data), [(1, 20), (2, 30), (4, 40)]);

    // "" means no query.
    reader.set_query(QueryFilter::default()).unwrap();
    reader.read_all(&mut data, &mut infos).unwrap();
    assert_eq!(data.len(), 8);
}

/// `get_one`: the next sample that was not communicated before, FALSE when
/// `time_out` passes without one; it waits for a sample that arrives
/// within `time_out`.
#[test]
fn dds4ccm_7_2_2_1_2_get_one_returns_false_after_time_out() {
    let w = World::new("Dds4CcmGetOne");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let getter = port.fresh_data().unwrap();
    assert!(getter.time_out().is_zero());

    let mut datum = track(99, 99);
    let mut info = ReadInfo::default();
    // No wait with the zero duration.
    let started = Instant::now();
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
    assert!(started.elapsed() < std::time::Duration::from_millis(250));
    assert_eq!(datum, track(99, 99), "the datum is untouched");

    getter.set_time_out(millis(300));
    assert_eq!(getter.time_out(), millis(300));
    let started = Instant::now();
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
    let waited = started.elapsed();
    assert!(
        waited >= std::time::Duration::from_millis(300),
        "returned after {waited:?}"
    );
    assert!(waited < std::time::Duration::from_secs(5), "{waited:?}");

    // A sample that arrives while the get waits ends the wait.
    getter.set_time_out(millis(20_000));
    let started = Instant::now();
    let got = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| {
            let mut datum = track(0, 0);
            let mut info = ReadInfo::default();
            let got = getter.get_one(&mut datum, &mut info).unwrap();
            (got, datum, info)
        });
        std::thread::sleep(std::time::Duration::from_millis(200));
        writer.write(&track(1, 10)).unwrap();
        waiting.join().unwrap()
    });
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
    assert!(got.0);
    assert_eq!(got.1, track(1, 10));
    assert_eq!(got.2.access_status, AccessStatus::FreshInfo);
    assert_eq!(got.2.instance_status, InstanceStatus::InstanceCreated);

    // One sample per call, in the order they came; a sample is gotten once.
    publish(&w, &writer, &[(1, 11), (1, 12)]);
    getter.set_time_out(millis(5000));
    assert!(getter.get_one(&mut datum, &mut info).unwrap());
    assert_eq!(datum, track(1, 11));
    assert_eq!(info.instance_status, InstanceStatus::InstanceUpdated);
    assert!(getter.get_one(&mut datum, &mut info).unwrap());
    assert_eq!(datum, track(1, 12));
    getter.set_time_out(Duration::ZERO);
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());

    // What the Reader of the port read is not fresh any more.
    publish(&w, &writer, &[(1, 13)]);
    read_all_of(&*port.data().unwrap(), 4);
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
}

/// `get_many`: all available samples within `max_delivered_data`
/// (UNLIMITED by default), the others stay for the next call; ALIVE or
/// NOT_ALIVE: a deletion is gotten, with the key only.
#[test]
fn dds4ccm_7_2_2_1_2_get_many_respects_max_delivered_data() {
    let w = World::new("Dds4CcmGetMany");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let getter = port.fresh_data().unwrap();
    assert_eq!(getter.max_delivered_data(), UNLIMITED);

    let (mut data, mut infos) = (vec![track(99, 99)], vec![ReadInfo::default()]);
    assert!(!getter.get_many(&mut data, &mut infos).unwrap());
    assert!(data.is_empty() && infos.is_empty());

    publish(&w, &writer, &[(1, 10), (2, 20), (1, 11), (2, 21), (1, 12)]);
    getter.set_max_delivered_data(2);
    assert_eq!(getter.max_delivered_data(), 2);
    let mut all = Vec::new();
    for expected in [2, 2, 1] {
        assert!(getter.get_many(&mut data, &mut infos).unwrap());
        assert_eq!(data.len(), expected);
        assert_eq!(infos.len(), expected);
        assert!(
            infos
                .iter()
                .all(|i| i.access_status == AccessStatus::FreshInfo)
        );
        all.extend(pairs(&data));
    }
    assert!(!getter.get_many(&mut data, &mut infos).unwrap());
    all.sort_unstable();
    assert_eq!(all, [(1, 10), (1, 11), (1, 12), (2, 20), (2, 21)]);

    // UNLIMITED: everything at once, ordered by instance.
    getter.set_max_delivered_data(UNLIMITED);
    publish(&w, &writer, &[(3, 30), (4, 40), (3, 31), (4, 41)]);
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    let got = pairs(&data);
    assert_eq!(got.len(), 4);
    assert_eq!(got[0].0, got[1].0, "ordered by instance: {got:?}");
    assert_eq!(got[2].0, got[3].0, "ordered by instance: {got:?}");
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceCreated);
    assert_eq!(infos[1].instance_status, InstanceStatus::InstanceUpdated);

    // A deletion is fresh information too.
    writer.dispose(&track(3, 0), HANDLE_NIL).unwrap();
    w.delivered(&writer);
    getter.set_time_out(millis(5000));
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].id, 3, "the key of the deleted instance");
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceDeleted);
    // Written again, the instance is created again.
    publish(&w, &writer, &[(3, 32)]);
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    assert_eq!(pairs(&data), [(3, 32)]);
    assert_eq!(infos[0].instance_status, InstanceStatus::InstanceCreated);
}

/// `DataListenerControl` and `StateListenerControl`: mode NOT_ENABLED,
/// max_delivered_data UNLIMITED and is_filter_interpreted FALSE by default;
/// the attributes keep what is set.
#[test]
fn dds4ccm_7_2_2_1_3_listener_control_defaults() {
    let listen = DdsListen::<Track>::new();
    let control = listen.data_control();
    assert_eq!(control.mode(), ListenerMode::NotEnabled);
    assert_eq!(control.max_delivered_data(), UNLIMITED);
    control.set_mode(ListenerMode::ManyByMany);
    control.set_max_delivered_data(7);
    assert_eq!(control.mode(), ListenerMode::ManyByMany);
    assert_eq!(control.max_delivered_data(), 7);

    let state = DdsStateListen::<Track>::new();
    let control = state.data_control();
    assert_eq!(control.mode(), ListenerMode::NotEnabled);
    assert_eq!(control.max_delivered_data(), UNLIMITED);
    assert!(!control.is_filter_interpreted());
    control.set_is_filter_interpreted(true);
    control.set_mode(ListenerMode::OneByOne);
    control.set_max_delivered_data(3);
    assert!(control.is_filter_interpreted());
    assert_eq!(control.mode(), ListenerMode::OneByOne);
    assert_eq!(control.max_delivered_data(), 3);
    // A StateListenerControl is a DataListenerControl.
    let base: &dyn DataListenerControl = &*control;
    assert_eq!(base.mode(), ListenerMode::OneByOne);
    let derived: &dyn StateListenerControl = &*control;
    assert!(derived.is_filter_interpreted());
}

/// `Listener`: NOT_ENABLED calls nothing and the data stay; ONE_BY_ONE
/// delivers one sample per `on_one_data`; MANY_BY_MANY delivers groups of
/// at most `max_delivered_data` through `on_many_data`; a sample is
/// delivered once.
#[test]
fn dds4ccm_7_2_2_1_2_listener_modes() {
    let w = World::new("Dds4CcmListen");
    let port = DdsListen::<Track>::new();
    let heard = Arc::new(Heard::default());
    assert!(port.connect_data_listener(heard.clone()).is_none());
    let writer = configured!(w, port);
    let control = port.data_control();

    // NOT_ENABLED: the default. Nothing is called.
    publish(&w, &writer, &[(1, 10), (2, 20), (1, 11), (2, 21), (1, 12)]);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(heard.one.lock().unwrap().is_empty());
    assert!(heard.many.lock().unwrap().is_empty());

    // MANY_BY_MANY with a limit: what the port holds is delivered when the
    // mode is set, in groups of at most two.
    control.set_max_delivered_data(2);
    control.set_mode(ListenerMode::ManyByMany);
    {
        let many = heard.many.lock().unwrap();
        let sizes: Vec<usize> = many.iter().map(Vec::len).collect();
        assert_eq!(sizes, [2, 2, 1]);
        let mut all: Vec<(u32, u32)> = many
            .iter()
            .flatten()
            .map(|(t, _)| (t.id, t.value))
            .collect();
        all.sort_unstable();
        assert_eq!(all, [(1, 10), (1, 11), (1, 12), (2, 20), (2, 21)]);
        assert!(
            many.iter()
                .flatten()
                .all(|(_, i)| i.access_status == AccessStatus::FreshInfo)
        );
    }
    assert!(heard.one.lock().unwrap().is_empty());

    // ONE_BY_ONE: each sample that arrives is one call.
    control.set_mode(ListenerMode::OneByOne);
    for (id, value) in [(3, 30), (3, 31), (4, 40)] {
        writer.write(&track(id, value)).unwrap();
    }
    eventually("three calls of on_one_data", || {
        heard.one.lock().unwrap().len() == 3
    });
    {
        let one = heard.one.lock().unwrap();
        let mut got: Vec<(u32, u32)> = one.iter().map(|(t, _)| (t.id, t.value)).collect();
        got.sort_unstable();
        assert_eq!(got, [(3, 30), (3, 31), (4, 40)]);
        let created: Vec<u32> = one
            .iter()
            .filter(|(_, i)| i.instance_status == InstanceStatus::InstanceCreated)
            .map(|(t, _)| t.value)
            .collect();
        assert_eq!(created, [30, 40], "the first sample of each instance");
    }
    assert_eq!(heard.many.lock().unwrap().len(), 3, "no further group");

    // MANY_BY_MANY without a limit: one group of everything available.
    control.set_mode(ListenerMode::NotEnabled);
    publish(&w, &writer, &[(5, 50), (5, 51), (6, 60)]);
    control.set_max_delivered_data(UNLIMITED);
    control.set_mode(ListenerMode::ManyByMany);
    {
        let many = heard.many.lock().unwrap();
        assert_eq!(many.len(), 4);
        assert_eq!(many[3].len(), 3);
    }
    assert_eq!(heard.one.lock().unwrap().len(), 3);
    assert!(control.delivery_error().is_none());

    // Without a listener the data stay fresh for the next one.
    control.set_mode(ListenerMode::OneByOne);
    let first = port.disconnect_data_listener().expect("the listener");
    drop(first);
    publish(&w, &writer, &[(7, 70)]);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let second = Arc::new(Heard::default());
    port.connect_data_listener(second.clone());
    assert_eq!(
        second
            .one
            .lock()
            .unwrap()
            .iter()
            .map(|(t, _)| (t.id, t.value))
            .collect::<Vec<_>>(),
        [(7, 70)]
    );
    assert_eq!(heard.one.lock().unwrap().len(), 3);
}

/// `StateListener`: on_creation for an instance that is new for the
/// component, on_deletion when it stopped existing (the key only), else
/// on_one_update in mode ONE_BY_ONE and on_many_updates in mode
/// MANY_BY_MANY within max_delivered_data; nothing in mode NOT_ENABLED.
#[test]
fn dds4ccm_7_2_2_1_2_state_listener_calls_on_creation_on_one_update_on_deletion() {
    let w = World::new("Dds4CcmStateListen");
    let port = DdsStateListen::<Track>::new();
    let heard = Arc::new(StateHeard::default());
    port.connect_data_listener(heard.clone());
    let writer = configured!(w, port);
    let control = port.data_control();

    publish(&w, &writer, &[(1, 10)]);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(heard.calls().is_empty(), "NOT_ENABLED calls nothing");

    control.set_mode(ListenerMode::OneByOne);
    assert_eq!(heard.calls(), [("creation", vec![track(1, 10)])]);
    writer.write(&track(1, 11)).unwrap();
    eventually("on_one_update", || heard.calls().len() == 2);
    writer.write(&track(1, 12)).unwrap();
    eventually("on_one_update", || heard.calls().len() == 3);
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    eventually("on_deletion", || heard.calls().len() == 4);
    writer.write(&track(1, 13)).unwrap();
    eventually("on_creation of the instance written again", || {
        heard.calls().len() == 5
    });
    let calls = heard.calls();
    assert_eq!(
        calls[1..3],
        [
            ("one_update", vec![track(1, 11)]),
            ("one_update", vec![track(1, 12)])
        ]
    );
    assert_eq!(calls[3].0, "deletion");
    assert_eq!(calls[3].1[0].id, 1, "the key of the deleted instance");
    assert_eq!(calls[4], ("creation", vec![track(1, 13)]));

    // MANY_BY_MANY: the updates the port holds come as groups of at most
    // max_delivered_data; a creation is its own call.
    control.set_mode(ListenerMode::NotEnabled);
    publish(&w, &writer, &[(1, 14), (1, 15), (1, 16), (2, 20), (2, 21)]);
    control.set_max_delivered_data(2);
    control.set_mode(ListenerMode::ManyByMany);
    let calls = heard.calls()[5..].to_vec();
    let updates: Vec<(u32, u32)> = calls
        .iter()
        .filter(|(operation, _)| *operation == "many_updates")
        .flat_map(|(_, data)| pairs(data))
        .collect();
    let mut sorted = updates.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [(1, 14), (1, 15), (1, 16), (2, 21)], "{calls:?}");
    assert!(
        calls
            .iter()
            .filter(|(operation, _)| *operation == "many_updates")
            .all(|(_, data)| data.len() <= 2),
        "{calls:?}"
    );
    let creations: Vec<&(&'static str, Vec<Track>)> = calls
        .iter()
        .filter(|(operation, _)| *operation == "creation")
        .collect();
    assert_eq!(creations, [&("creation", vec![track(2, 20)])]);
    assert!(
        calls
            .iter()
            .all(|(operation, _)| *operation == "many_updates" || *operation == "creation"),
        "{calls:?}"
    );
    assert!(control.delivery_error().is_none());
}

/// Figure 7.2 over DDS: INSTANCE_CREATED for the first sample of an
/// instance, INSTANCE_UPDATED for the next, INSTANCE_DELETED for its
/// disposal, INSTANCE_CREATED when it is written again; a disposal of an
/// instance the component never saw is not passed.
#[test]
fn dds4ccm_7_2_2_1_2_instance_status_follows_figure_7_2() {
    let w = World::new("Dds4CcmFigure72");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let getter = port.fresh_data().unwrap();
    getter.set_time_out(millis(5000));
    let next = || {
        let mut datum = track(0, 0);
        let mut info = ReadInfo::default();
        assert!(getter.get_one(&mut datum, &mut info).unwrap());
        (datum, info.instance_status)
    };

    writer.write(&track(1, 10)).unwrap();
    assert_eq!(next(), (track(1, 10), InstanceStatus::InstanceCreated));
    writer.write(&track(1, 11)).unwrap();
    assert_eq!(next(), (track(1, 11), InstanceStatus::InstanceUpdated));
    writer.write(&track(2, 20)).unwrap();
    assert_eq!(next(), (track(2, 20), InstanceStatus::InstanceCreated));
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    let (deleted, status) = next();
    assert_eq!(status, InstanceStatus::InstanceDeleted);
    assert_eq!(deleted.id, 1);
    writer.write(&track(1, 12)).unwrap();
    assert_eq!(next(), (track(1, 12), InstanceStatus::InstanceCreated));
    writer.write(&track(1, 13)).unwrap();
    assert_eq!(next(), (track(1, 13), InstanceStatus::InstanceUpdated));

    // An instance that is registered and disposed without a sample: the
    // component never saw it, nothing is passed. The next sample is.
    let three = writer.register_instance(&track(3, 0)).unwrap();
    writer.dispose(&track(3, 0), three).unwrap();
    writer.write(&track(2, 21)).unwrap();
    assert_eq!(next(), (track(2, 21), InstanceStatus::InstanceUpdated));
    getter.set_time_out(Duration::ZERO);
    let mut datum = track(0, 0);
    let mut info = ReadInfo::default();
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
}

/// `INSTANCE_FILTERED_OUT` "if an existing instance just stopped passing
/// the filter", `INSTANCE_FILTERED_IN` "if an existing instance reenters
/// the filter after having been filtered out": the query of the Reader of
/// a `DDS_Get` port, over DDS. The sample that leaves comes with its data;
/// further rejected samples and those of an instance the component does
/// not know come not at all; the disposal of an instance outside the query
/// is its deletion.
#[test]
fn dds4ccm_7_2_2_1_2_an_instance_leaving_and_reentering_the_query_is_filtered_out_and_in() {
    let w = World::new("Dds4CcmFilteredGet");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().unwrap();
    let getter = port.fresh_data().unwrap();
    reader
        .set_query(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    getter.set_time_out(millis(5000));
    let next = || {
        let mut datum = track(0, 0);
        let mut info = ReadInfo::default();
        assert!(getter.get_one(&mut datum, &mut info).unwrap());
        assert_eq!(info.access_status, AccessStatus::FreshInfo);
        (datum, info.instance_status)
    };

    // A rejected sample of an instance the component does not know is
    // nothing; the first one that passes creates the instance.
    publish(&w, &writer, &[(1, 3), (1, 20)]);
    assert_eq!(next(), (track(1, 20), InstanceStatus::InstanceCreated));
    writer.write(&track(1, 21)).unwrap();
    assert_eq!(next(), (track(1, 21), InstanceStatus::InstanceUpdated));
    // It stops passing: once, with the data that left.
    publish(&w, &writer, &[(1, 5), (1, 6), (1, 30)]);
    assert_eq!(next(), (track(1, 5), InstanceStatus::InstanceFilteredOut));
    // (1, 6) is outside as well and is not gotten; (1, 30) reenters.
    assert_eq!(next(), (track(1, 30), InstanceStatus::InstanceFilteredIn));
    writer.write(&track(1, 31)).unwrap();
    assert_eq!(next(), (track(1, 31), InstanceStatus::InstanceUpdated));

    // A read of the Reader returns what passes the query, each sample with
    // the status of its time.
    let (data, infos) = read_all_of(&*reader, 4);
    assert_eq!(pairs(&data), [(1, 20), (1, 21), (1, 30), (1, 31)]);
    let statuses: Vec<InstanceStatus> = infos.iter().map(|i| i.instance_status).collect();
    assert_eq!(
        statuses,
        [
            InstanceStatus::InstanceCreated,
            InstanceStatus::InstanceUpdated,
            InstanceStatus::InstanceFilteredIn,
            InstanceStatus::InstanceUpdated
        ]
    );
    assert!(
        infos
            .iter()
            .all(|i| i.access_status == AccessStatus::AlreadySeen)
    );

    // Another instance is not touched by the first one's state.
    publish(&w, &writer, &[(2, 4), (2, 40)]);
    assert_eq!(next(), (track(2, 40), InstanceStatus::InstanceCreated));

    // Outside the query the instance exists: its disposal is its deletion,
    // and it is created when it is written again.
    writer.write(&track(1, 7)).unwrap();
    assert_eq!(next(), (track(1, 7), InstanceStatus::InstanceFilteredOut));
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    let (deleted, status) = next();
    assert_eq!(status, InstanceStatus::InstanceDeleted);
    assert_eq!(deleted.id, 1);
    publish(&w, &writer, &[(1, 8), (1, 50)]);
    assert_eq!(next(), (track(1, 50), InstanceStatus::InstanceCreated));

    // Without a query nothing is outside: the next sample of an instance
    // that was filtered out reenters.
    writer.write(&track(2, 9)).unwrap();
    assert_eq!(next(), (track(2, 9), InstanceStatus::InstanceFilteredOut));
    reader.set_query(QueryFilter::default()).unwrap();
    writer.write(&track(2, 1)).unwrap();
    assert_eq!(next(), (track(2, 1), InstanceStatus::InstanceFilteredIn));
    getter.set_time_out(Duration::ZERO);
    let mut datum = track(0, 0);
    let mut info = ReadInfo::default();
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
}

/// A get judges every fresh sample once, by the query the Reader has at
/// that time: a sample that query rejected is not gotten later under a
/// query that accepts it. The Reader still reads it, as ALREADY_SEEN.
#[test]
fn dds4ccm_7_2_2_1_2_a_fresh_sample_the_query_rejected_is_not_gotten_under_a_later_query() {
    let w = World::new("Dds4CcmJudgedOnce");
    let port = DdsGet::<Track>::new();
    let writer = configured!(w, port);
    let reader = port.data().unwrap();
    let getter = port.fresh_data().unwrap();
    reader
        .set_query(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    getter.set_time_out(millis(5000));

    publish(&w, &writer, &[(1, 5), (2, 6), (1, 20)]);
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    assert_eq!(pairs(&data), [(1, 20)]);

    // The query now accepts what the get passed over.
    reader
        .set_query(QueryFilter::new("value < %0", vec!["15".into()]))
        .unwrap();
    getter.set_time_out(Duration::ZERO);
    assert!(
        !getter.get_many(&mut data, &mut infos).unwrap(),
        "judged by the query of its get: {:?}",
        pairs(&data)
    );
    reader.read_all(&mut data, &mut infos).unwrap();
    let mut read = pairs(&data);
    read.sort_unstable();
    assert_eq!(read, [(1, 5), (2, 6)]);
    assert!(
        infos
            .iter()
            .all(|i| i.access_status == AccessStatus::AlreadySeen),
        "{infos:?}"
    );
    // What arrives from now on is judged by the new query.
    getter.set_time_out(millis(5000));
    publish(&w, &writer, &[(3, 30), (3, 7)]);
    assert!(getter.get_many(&mut data, &mut infos).unwrap());
    assert_eq!(pairs(&data), [(3, 7)]);
}

/// A `Listener` gets fresh values "whatever the instance status": the
/// exit from the query of the Reader of its port and the reentry come
/// through `on_one_data` with their status.
#[test]
fn dds4ccm_7_2_2_1_2_a_listener_gets_the_exit_from_the_query_and_the_reentry() {
    let w = World::new("Dds4CcmFilteredListen");
    let port = DdsListen::<Track>::new();
    let heard = Arc::new(Heard::default());
    port.connect_data_listener(heard.clone());
    let writer = configured!(w, port);
    port.data()
        .unwrap()
        .set_query(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    port.data_control().set_mode(ListenerMode::OneByOne);

    for (id, value) in [(1, 20), (1, 5), (1, 6), (2, 7), (1, 30)] {
        writer.write(&track(id, value)).unwrap();
    }
    eventually("three calls of on_one_data", || {
        heard.one.lock().unwrap().len() >= 3
    });
    let got: Vec<((u32, u32), InstanceStatus)> = heard
        .one
        .lock()
        .unwrap()
        .iter()
        .map(|(t, i)| ((t.id, t.value), i.instance_status))
        .collect();
    assert_eq!(
        got,
        [
            ((1, 20), InstanceStatus::InstanceCreated),
            ((1, 5), InstanceStatus::InstanceFilteredOut),
            ((1, 30), InstanceStatus::InstanceFilteredIn),
        ]
    );
    assert!(port.data_control().delivery_error().is_none());
}

/// What a `StateListener` of a `DDS_StateListen` port is called with while
/// an instance leaves the query of the Reader, stays outside, reenters,
/// leaves again, is disposed outside and is written again. `at_disposal`
/// is the number of calls the disposal adds.
fn state_listener_calls_around_the_query(
    topic: &str,
    is_filter_interpreted: bool,
    at_disposal: usize,
) -> Vec<(&'static str, (u32, u32))> {
    let w = World::new(topic);
    let port = DdsStateListen::<Track>::new();
    let heard = Arc::new(StateHeard::default());
    port.connect_data_listener(heard.clone());
    let writer = configured!(w, port);
    port.data()
        .unwrap()
        .set_query(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    let control = port.data_control();
    control.set_is_filter_interpreted(is_filter_interpreted);
    control.set_mode(ListenerMode::OneByOne);

    publish(&w, &writer, &[(1, 20), (1, 5), (1, 6), (1, 30), (1, 7)]);
    // The second exit is a call with either value of the attribute.
    eventually("the call for (1, 7)", || {
        heard
            .calls()
            .last()
            .is_some_and(|(_, data)| data == &[track(1, 7)])
    });
    let before = heard.calls().len();
    // The disposal by itself: a DataReader that reads it together with the
    // next sample shows one sample with a changed disposed_generation_count.
    writer.dispose(&track(1, 0), HANDLE_NIL).unwrap();
    w.delivered(&writer);
    if at_disposal == 0 {
        std::thread::sleep(std::time::Duration::from_millis(300));
    } else {
        eventually("the call for the disposal", || {
            heard.calls().len() == before + at_disposal
        });
    }
    assert_eq!(heard.calls().len(), before + at_disposal);
    publish(&w, &writer, &[(1, 40)]);
    eventually("the creation of the instance written again", || {
        heard.calls().last() == Some(&("creation", vec![track(1, 40)]))
    });
    assert!(control.delivery_error().is_none());
    heard
        .calls()
        .into_iter()
        .map(|(operation, data)| {
            assert_eq!(data.len(), 1, "{operation}");
            // Of a deletion "the only fields valid in the provided datum
            // parameter are the ones that make the key".
            let value = if operation == "deletion" {
                0
            } else {
                data[0].value
            };
            (operation, (data[0].id, value))
        })
        .collect()
}

/// `is_filter_interpreted` FALSE, the default: an instance entering or
/// leaving the query of the Reader is "considered as normal instance
/// updates"; the instance exists outside the query and its disposal there
/// is `on_deletion`.
#[test]
fn dds4ccm_7_2_2_1_3_is_filter_interpreted_false_makes_exit_and_reentry_updates() {
    assert_eq!(
        state_listener_calls_around_the_query("Dds4CcmNotInterpreted", false, 1),
        [
            ("creation", (1, 20)),
            ("one_update", (1, 5)),
            ("one_update", (1, 30)),
            ("one_update", (1, 7)),
            ("deletion", (1, 0)),
            ("creation", (1, 40)),
        ]
    );
}

/// `is_filter_interpreted` TRUE: "the associated listener should consider
/// an instance entering in (resp. going out) the filter (if any) of the
/// related Reader, as an instance creation (resp. deletion) and thus
/// trigger the operation on_creation (resp. on_deletion)". The disposal of
/// an instance the listener was told deleted is no second `on_deletion`.
#[test]
fn dds4ccm_7_2_2_1_3_is_filter_interpreted_true_makes_exit_a_deletion_and_reentry_a_creation() {
    assert_eq!(
        state_listener_calls_around_the_query("Dds4CcmInterpreted", true, 0),
        [
            ("creation", (1, 20)),
            ("deletion", (1, 0)),
            ("creation", (1, 30)),
            ("deletion", (1, 0)),
            ("creation", (1, 40)),
        ]
    );
}

/// `ContentFilterSetting::set_filter_parameters` changes the parameters of
/// the content filter of the port; DDS refusing them, or a port without a
/// filter, is an InternalError.
#[test]
fn dds4ccm_7_2_2_1_4_set_filter_parameters() {
    let w = World::new("Dds4CcmFilterParams");
    let port = DdsGet::<Track>::new();
    port.set_filter(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    let config: Arc<dyn ContentFilterSetting> = port.filter_config();
    // Not configured yet: no filter to set the parameters of.
    assert_eq!(
        config.set_filter_parameters(vec!["1".into()]),
        Err(InternalError {
            error_code: return_code::PRECONDITION_NOT_MET,
            index: 0
        })
    );
    let writer = configured!(w, port);
    let getter = port.fresh_data().unwrap();
    getter.set_time_out(millis(5000));
    let (mut data, mut infos) = (Vec::new(), Vec::new());

    publish(&w, &writer, &[(1, 5), (2, 20), (3, 30)]);
    let mut got = Vec::new();
    while got.len() < 2 {
        assert!(getter.get_many(&mut data, &mut infos).unwrap());
        got.extend(pairs(&data));
    }
    got.sort_unstable();
    assert_eq!(got, [(2, 20), (3, 30)]);

    config.set_filter_parameters(vec!["25".into()]).unwrap();
    assert_eq!(
        port.filter(),
        QueryFilter::new("value > %0", vec!["25".into()]),
        "attribute filter shows the parameters set last"
    );
    publish(&w, &writer, &[(4, 22), (5, 40), (6, 26)]);
    let mut got = Vec::new();
    while got.len() < 2 {
        assert!(getter.get_many(&mut data, &mut infos).unwrap());
        got.extend(pairs(&data));
    }
    got.sort_unstable();
    assert_eq!(got, [(5, 40), (6, 26)]);

    // Parameters DDS does not accept: the expression needs %0.
    let refused = config.set_filter_parameters(Vec::new()).unwrap_err();
    assert_ne!(refused.error_code, return_code::OK);
    assert_eq!(port.filter().parameters, ["25"]);

    // A port whose filter has no expression has no content filter.
    let plain = DdsRead::<Track>::new();
    let _writer = configured!(w, plain);
    assert_eq!(
        plain
            .filter_config()
            .set_filter_parameters(vec!["1".into()]),
        Err(InternalError {
            error_code: return_code::PRECONDITION_NOT_MET,
            index: 0
        })
    );
}

/// `PortStatusListener` gets the statuses that are relevant to the
/// component, `ConnectorStatusListener` the ones that are relevant
/// system-wide, with the operations of the DDS listeners; a status no
/// operation names is `on_unexpected_status` with the entity and the kind.
#[test]
fn dds4ccm_7_2_2_1_5_status_listeners() {
    let w = World::new("Dds4CcmStatus");
    let port_statuses = Arc::new(Statuses::default());
    let sub_connector = Arc::new(Statuses::default());
    let pub_connector = Arc::new(Statuses::default());
    w.sub_errors
        .connect(sub_connector.clone() as Arc<dyn ConnectorStatusListener>);
    w.pub_errors
        .connect(pub_connector.clone() as Arc<dyn ConnectorStatusListener>);

    // A reader that requests a deadline of 200 ms, and a writer that
    // offers it.
    let port = DdsRead::<Track>::new();
    assert!(
        port.connect_status(port_statuses.clone() as Arc<dyn PortStatusListener>)
            .is_none()
    );
    let deadline = DeadlineQosPolicy {
        period: zerodds_qos::Duration::from_millis(200),
    };
    port.configuration_complete(
        &w.subscribing,
        &w.subscriber,
        &w.sub_topic,
        DataReaderQos {
            deadline,
            ..reliable_keep_all()
        },
        Arc::clone(&w.sub_errors),
    )
    .unwrap();
    let write = zerodds_dds4ccm::DdsWrite::new(
        w.writer_with(DataWriterQos {
            deadline,
            ..DataWriterQos::default()
        }),
        Arc::clone(&w.pub_errors),
    );
    w.matched(write.dds_entity(), 1);
    // The match is a status no operation of the listeners names.
    let matched_reader = format!("unexpected:reader:{:#x}", status_kind::SUBSCRIPTION_MATCHED);
    let matched_writer = format!("unexpected:writer:{:#x}", status_kind::PUBLICATION_MATCHED);
    eventually("on_unexpected_status of both sides", || {
        sub_connector.has(&matched_reader) && pub_connector.has(&matched_writer)
    });

    // One sample, then silence: both deadlines are missed.
    write.dds_entity().write(&track(1, 10)).unwrap();
    eventually("on_requested_deadline_missed of the port", || {
        port_statuses.has("requested_deadline_missed")
    });
    eventually("on_offered_deadline_missed of the connector", || {
        pub_connector.has("offered_deadline_missed")
    });
    assert!(
        !sub_connector.has("requested_deadline_missed"),
        "a status of the component does not go to the connector"
    );

    // A reader that requests RELIABLE from a BEST_EFFORT writer.
    let best_effort = zerodds_dds4ccm::DdsWrite::new(
        w.writer_with(DataWriterQos {
            reliability: ReliabilityQosPolicy {
                kind: ReliabilityKind::BestEffort,
                ..ReliabilityQosPolicy::default()
            },
            ..DataWriterQos::default()
        }),
        Arc::clone(&w.pub_errors),
    );
    eventually("the incompatible QoS reaches both connectors", || {
        sub_connector.has("requested_incompatible_qos")
            && pub_connector.has("offered_incompatible_qos")
    });
    drop(best_effort);
    let port_events = port_statuses.events();
    assert!(
        port_events
            .iter()
            .all(|e| e == "requested_deadline_missed" || e == "sample_lost"),
        "the port listener has two operations: {port_events:?}"
    );

    // Disconnected, a listener is not called any more.
    assert!(port.disconnect_status().is_some());
    let before = port_statuses.events().len();
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert_eq!(port_statuses.events().len(), before);
}

/// The entities `on_unexpected_status` was called with.
#[derive(Default)]
struct Entities {
    seen: std::sync::Mutex<Vec<zerodds_dds4ccm::DdsEntity>>,
}

impl ConnectorStatusListener for Entities {
    fn on_inconsistent_topic(
        &self,
        _the_topic: &zerodds_dcps::TopicDescriptionHandle,
        _status: zerodds_dcps::status::InconsistentTopicStatus,
    ) {
    }
    fn on_requested_incompatible_qos(
        &self,
        _the_reader: &zerodds_dcps::AnyDataReader,
        _status: zerodds_dcps::status::RequestedIncompatibleQosStatus,
    ) {
    }
    fn on_sample_rejected(
        &self,
        _the_reader: &zerodds_dcps::AnyDataReader,
        _status: zerodds_dcps::status::SampleRejectedStatus,
    ) {
    }
    fn on_offered_deadline_missed(
        &self,
        _the_writer: &zerodds_dcps::publisher::AnyDataWriter,
        _status: zerodds_dcps::status::OfferedDeadlineMissedStatus,
    ) {
    }
    fn on_offered_incompatible_qos(
        &self,
        _the_writer: &zerodds_dcps::publisher::AnyDataWriter,
        _status: zerodds_dcps::status::OfferedIncompatibleQosStatus,
    ) {
    }
    fn on_unexpected_status(
        &self,
        the_entity: &zerodds_dds4ccm::DdsEntity,
        _status_kind: zerodds_dds4ccm::ccm_dds::StatusKind,
    ) {
        self.seen.lock().unwrap().push(the_entity.clone());
    }
}

/// Every status of the DDS listeners of a port and of the topic goes to
/// the operation §7.2.2.1.5 names for it: two to the PortStatusListener,
/// five to the ConnectorStatusListener, the remaining statuses of a reader
/// or a writer to `on_unexpected_status`; the data status stays in the
/// fragment. The DDS listeners are called here as DDS calls them, with the
/// DataReader and the DataWriter of two ports.
#[test]
fn dds4ccm_7_2_2_1_5_each_dds_status_goes_to_the_operation_of_its_listener() {
    use zerodds_dcps::listener::{DataReaderListener, DataWriterListener, TopicListener};
    use zerodds_dcps::status::{
        InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
        OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
        RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
        SampleRejectedStatus, SubscriptionMatchedStatus,
    };
    use zerodds_dds4ccm::{
        DdsEntity, ErrorListener, ReaderStatusListener, Receptacle, TopicStatusListener,
        WriterStatusListener,
    };

    // The DataReader and the DataWriter of two matched ports, as DDS hands
    // them to a listener.
    let w = World::new("Dds4CcmStatusRoutes");
    let entities = Arc::new(Entities::default());
    w.sub_errors
        .connect(entities.clone() as Arc<dyn ConnectorStatusListener>);
    w.pub_errors
        .connect(entities.clone() as Arc<dyn ConnectorStatusListener>);
    let port = DdsRead::<Track>::new();
    port.configuration_complete(
        &w.subscribing,
        &w.subscriber,
        &w.sub_topic,
        reliable_keep_all(),
        Arc::clone(&w.sub_errors),
    )
    .unwrap();
    let write = zerodds_dds4ccm::DdsWrite::new(w.writer(), Arc::clone(&w.pub_errors));
    w.matched(write.dds_entity(), 1);
    let find = |reader: bool| {
        entities.seen.lock().unwrap().iter().find_map(|e| match e {
            DdsEntity::DataReader(r) if reader => Some(DdsEntity::DataReader(r.clone())),
            DdsEntity::DataWriter(w) if !reader => Some(DdsEntity::DataWriter(w.clone())),
            _ => None,
        })
    };
    eventually("both ports reported their match", || {
        find(true).is_some() && find(false).is_some()
    });
    let (Some(DdsEntity::DataReader(the_reader)), Some(DdsEntity::DataWriter(the_writer))) =
        (find(true), find(false))
    else {
        panic!("a reader and a writer");
    };
    assert_eq!(
        the_reader.instance_handle(),
        port.dds_entity().unwrap().instance_handle(),
        "the entity of on_unexpected_status is the DataReader of the port"
    );
    assert_eq!(
        the_writer.instance_handle(),
        write.dds_entity().instance_handle()
    );
    let the_topic = port.dds_entity().unwrap().get_topicdescription().unwrap();

    let component = Arc::new(Statuses::default());
    let connector = Arc::new(Statuses::default());
    let status: Arc<Receptacle<dyn PortStatusListener>> = Arc::new(Receptacle::new());
    status.connect(component.clone() as Arc<dyn PortStatusListener>);
    let error_listener: Arc<ErrorListener> = Arc::new(Receptacle::new());
    error_listener.connect(connector.clone() as Arc<dyn ConnectorStatusListener>);

    let of_reader =
        ReaderStatusListener::<Track>::new(Arc::clone(&status), Arc::clone(&error_listener));
    assert_eq!(
        of_reader.mask(),
        status_kind::REQUESTED_DEADLINE_MISSED
            | status_kind::SAMPLE_LOST
            | status_kind::REQUESTED_INCOMPATIBLE_QOS
            | status_kind::SAMPLE_REJECTED
            | status_kind::LIVELINESS_CHANGED
            | status_kind::SUBSCRIPTION_MATCHED,
        "a port without a data listener leaves DATA_AVAILABLE to DDS"
    );
    of_reader
        .on_requested_deadline_missed_entity(&the_reader, RequestedDeadlineMissedStatus::default());
    of_reader.on_sample_lost_entity(&the_reader, SampleLostStatus::default());
    of_reader.on_requested_incompatible_qos_entity(
        &the_reader,
        RequestedIncompatibleQosStatus::default(),
    );
    of_reader.on_sample_rejected_entity(&the_reader, SampleRejectedStatus::default());
    of_reader.on_liveliness_changed_entity(&the_reader, LivelinessChangedStatus::default());
    of_reader.on_subscription_matched_entity(&the_reader, SubscriptionMatchedStatus::default());
    // The data status is not reported to either listener.
    of_reader.on_data_available_entity(&the_reader);
    assert_eq!(
        component.events(),
        ["requested_deadline_missed", "sample_lost"]
    );
    assert_eq!(
        connector.events(),
        [
            "requested_incompatible_qos".to_string(),
            "sample_rejected".to_string(),
            format!("unexpected:reader:{:#x}", status_kind::LIVELINESS_CHANGED),
            format!("unexpected:reader:{:#x}", status_kind::SUBSCRIPTION_MATCHED),
        ]
    );

    let of_writer = WriterStatusListener::new(Arc::clone(&error_listener));
    assert_eq!(
        of_writer.mask(),
        status_kind::OFFERED_DEADLINE_MISSED
            | status_kind::OFFERED_INCOMPATIBLE_QOS
            | status_kind::LIVELINESS_LOST
            | status_kind::PUBLICATION_MATCHED
    );
    of_writer
        .on_offered_deadline_missed_entity(&the_writer, OfferedDeadlineMissedStatus::default());
    of_writer
        .on_offered_incompatible_qos_entity(&the_writer, OfferedIncompatibleQosStatus::default());
    of_writer.on_liveliness_lost_entity(&the_writer, LivelinessLostStatus::default());
    of_writer.on_publication_matched_entity(&the_writer, PublicationMatchedStatus::default());
    let of_topic = TopicStatusListener::new(Arc::clone(&error_listener));
    assert_eq!(of_topic.mask(), status_kind::INCONSISTENT_TOPIC);
    of_topic.on_inconsistent_topic_entity(&the_topic, InconsistentTopicStatus::default());
    assert_eq!(
        connector.events()[4..],
        [
            "offered_deadline_missed".to_string(),
            "offered_incompatible_qos".to_string(),
            format!("unexpected:writer:{:#x}", status_kind::LIVELINESS_LOST),
            format!("unexpected:writer:{:#x}", status_kind::PUBLICATION_MATCHED),
            "inconsistent_topic".to_string(),
        ]
    );
    assert_eq!(
        component.events().len(),
        2,
        "nothing more for the component"
    );

    // Without a connection a status has nobody to go to.
    status.disconnect();
    error_listener.disconnect();
    of_reader.on_sample_lost_entity(&the_reader, SampleLostStatus::default());
    of_reader.on_sample_rejected_entity(&the_reader, SampleRejectedStatus::default());
    assert_eq!(component.events().len(), 2);
    assert_eq!(connector.events().len(), 9);

    // The DataReader a `DDS_Update` port with `is_global_scope` keeps for
    // itself belongs to a port without a `PortStatusListener`: every
    // status goes to the connector, the two of `PortStatusListener` as
    // unexpected ones.
    let of_connector = Arc::new(Statuses::default());
    let error_listener: Arc<ErrorListener> = Arc::new(Receptacle::new());
    error_listener.connect(of_connector.clone() as Arc<dyn ConnectorStatusListener>);
    let of_observer = ReaderStatusListener::<Track>::of_connector(Arc::clone(&error_listener));
    assert_eq!(of_observer.mask(), of_reader.mask());
    of_observer
        .on_requested_deadline_missed_entity(&the_reader, RequestedDeadlineMissedStatus::default());
    of_observer.on_sample_lost_entity(&the_reader, SampleLostStatus::default());
    of_observer.on_requested_incompatible_qos_entity(
        &the_reader,
        RequestedIncompatibleQosStatus::default(),
    );
    of_observer.on_sample_rejected_entity(&the_reader, SampleRejectedStatus::default());
    of_observer.on_liveliness_changed_entity(&the_reader, LivelinessChangedStatus::default());
    of_observer.on_subscription_matched_entity(&the_reader, SubscriptionMatchedStatus::default());
    of_observer.on_data_available_entity(&the_reader);
    assert_eq!(
        of_connector.events(),
        [
            format!(
                "unexpected:reader:{:#x}",
                status_kind::REQUESTED_DEADLINE_MISSED
            ),
            format!("unexpected:reader:{:#x}", status_kind::SAMPLE_LOST),
            "requested_incompatible_qos".to_string(),
            "sample_rejected".to_string(),
            format!("unexpected:reader:{:#x}", status_kind::LIVELINESS_CHANGED),
            format!("unexpected:reader:{:#x}", status_kind::SUBSCRIPTION_MATCHED),
        ]
    );
}
