// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-DCPS extended ports (DDS4CCM 1.1 §7.2.2.2): the six port types,
//! the basic ports each one is made of, attribute `filter` with
//! `setraises(NonChangeable)`, and `dds_entity`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{Heard, StateHeard, Statuses, Track, World, eventually, reliable_keep_all, track};
use zerodds_dcps::error::return_code;
use zerodds_dcps::sample_info::sample_state_mask;
use zerodds_dcps::{
    DataReader, DataWriter, DataWriterQos, DdsError, HANDLE_NIL, ReadSelection,
    TopicDescriptionKind,
};
use zerodds_dds4ccm::{
    ContentFilterSetting, DataListenerControl, DdsGet, DdsListen, DdsRead, DdsStateListen,
    DdsUpdate, DdsWrite, Getter, InternalError, Listener, ListenerMode, NonChangeable,
    PortStatusListener, QueryFilter, ReadInfo, Reader, StateListener, StateListenerControl,
    Updater, Writer,
};
use zerodds_qos::{HistoryKind, HistoryQosPolicy};

fn keep_all_writer(w: &World) -> DataWriter<Track> {
    w.writer_with(DataWriterQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataWriterQos::default()
    })
}

macro_rules! complete {
    ($w:expr, $port:expr) => {
        $port.configuration_complete(
            &$w.subscribing,
            &$w.subscriber,
            &$w.sub_topic,
            reliable_keep_all(),
            Arc::clone(&$w.sub_errors),
        )
    };
}

fn pairs(data: &[Track]) -> Vec<(u32, u32)> {
    data.iter().map(|t| (t.id, t.value)).collect()
}

/// `read_all` once it returns `count` samples.
fn read_all_of(reader: &dyn Reader<Track>, count: usize) -> Vec<(u32, u32)> {
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port holds the samples", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == count
    });
    pairs(&data)
}

/// Each port type is made of the basic ports of §7.2.2.2, under their
/// names and with their interfaces:
///
/// | port type | basic ports |
/// |---|---|
/// | DDS_Write | data: Writer, dds_entity: DataWriter |
/// | DDS_Update | data: Updater, dds_entity: DataWriter |
/// | DDS_Read | data: Reader, filter, filter_config: ContentFilterSetting, dds_entity: DataReader, status: PortStatusListener |
/// | DDS_Get | those of DDS_Read and fresh_data: Getter |
/// | DDS_Listen | those of DDS_Read, data_control: DataListenerControl, data_listener: Listener |
/// | DDS_StateListen | those of DDS_Read, data_control: StateListenerControl, data_listener: StateListener |
#[test]
fn dds4ccm_7_2_2_2_each_port_type_has_the_basic_ports_of_the_specification() {
    let w = World::new("Dds4CcmPortTypes");
    let statuses = || Arc::new(Statuses::default()) as Arc<dyn PortStatusListener>;

    let write = DdsWrite::new(w.writer(), Arc::clone(&w.pub_errors));
    let data: Arc<dyn Writer<Track>> = write.data();
    let entity: &DataWriter<Track> = write.dds_entity();
    data.write_one(&track(1, 1), HANDLE_NIL).unwrap();
    assert!(!entity.lookup_instance(&track(1, 0)).is_nil());

    let update = DdsUpdate::new(w.writer(), Arc::clone(&w.pub_errors));
    let data: Arc<dyn Updater<Track>> = update.data();
    let entity: &DataWriter<Track> = update.dds_entity();
    data.create_one(&track(1, 1), HANDLE_NIL).unwrap();
    assert!(!entity.lookup_instance(&track(1, 0)).is_nil());

    let read = DdsRead::<Track>::new();
    let _: QueryFilter = read.filter();
    let _: Arc<dyn ContentFilterSetting> = read.filter_config();
    assert!(read.connect_status(statuses()).is_none());
    complete!(w, read).unwrap();
    let _: Arc<dyn Reader<Track>> = read.data().unwrap();
    let _: DataReader<Track> = read.dds_entity().unwrap();

    let get = DdsGet::<Track>::new();
    let _: QueryFilter = get.filter();
    let _: Arc<dyn ContentFilterSetting> = get.filter_config();
    assert!(get.connect_status(statuses()).is_none());
    complete!(w, get).unwrap();
    let _: Arc<dyn Reader<Track>> = get.data().unwrap();
    let _: Arc<dyn Getter<Track>> = get.fresh_data().unwrap();
    let _: DataReader<Track> = get.dds_entity().unwrap();

    let listen = DdsListen::<Track>::new();
    let _: QueryFilter = listen.filter();
    let _: Arc<dyn ContentFilterSetting> = listen.filter_config();
    let control: Arc<dyn DataListenerControl> = listen.data_control();
    assert_eq!(control.mode(), ListenerMode::NotEnabled);
    let listener: Arc<dyn Listener<Track>> = Arc::new(Heard::default());
    assert!(listen.connect_data_listener(listener).is_none());
    assert!(listen.connect_status(statuses()).is_none());
    complete!(w, listen).unwrap();
    let _: Arc<dyn Reader<Track>> = listen.data().unwrap();
    let _: DataReader<Track> = listen.dds_entity().unwrap();

    let state = DdsStateListen::<Track>::new();
    let _: QueryFilter = state.filter();
    let _: Arc<dyn ContentFilterSetting> = state.filter_config();
    let control: Arc<dyn StateListenerControl> = state.data_control();
    assert!(!control.is_filter_interpreted());
    let listener: Arc<dyn StateListener<Track>> = Arc::new(StateHeard::default());
    assert!(state.connect_data_listener(listener).is_none());
    assert!(state.connect_status(statuses()).is_none());
    complete!(w, state).unwrap();
    let _: Arc<dyn Reader<Track>> = state.data().unwrap();
    let _: DataReader<Track> = state.dds_entity().unwrap();

    // Each subscribing port has a DataReader of its own (§7.4.1).
    let handles = [
        read.dds_entity().unwrap().instance_handle(),
        get.dds_entity().unwrap().instance_handle(),
        listen.dds_entity().unwrap().instance_handle(),
        state.dds_entity().unwrap().instance_handle(),
    ];
    for (at, handle) in handles.iter().enumerate() {
        assert!(!handles[..at].contains(handle), "{handles:?}");
    }
}

/// Attribute `filter` with `setraises(NonChangeable)`: it is set while the
/// port is configured and raises NonChangeable afterwards; the uses ports
/// of a subscribing port exist once the configuration is complete.
#[test]
fn dds4ccm_7_2_2_2_filter_is_non_changeable_after_configuration() {
    let w = World::new("Dds4CcmNonChangeable");
    let port = DdsRead::<Track>::new();
    assert_eq!(
        port.filter(),
        QueryFilter::default(),
        "no filter by default"
    );
    assert!(port.data().is_none());
    assert!(port.dds_entity().is_none());

    port.set_filter(QueryFilter::new("value > %0", vec!["1".into()]))
        .unwrap();
    let filter = QueryFilter::new("value > %0", vec!["15".into()]);
    port.set_filter(filter.clone()).unwrap();
    assert_eq!(port.filter(), filter);

    complete!(w, port).unwrap();
    assert_eq!(
        port.set_filter(QueryFilter::new("value < %0", vec!["3".into()])),
        Err(NonChangeable)
    );
    assert_eq!(port.set_filter(QueryFilter::default()), Err(NonChangeable));
    assert_eq!(port.filter(), filter, "the attribute keeps its value");
    // A port is configured once.
    assert_eq!(
        complete!(w, port),
        Err(InternalError {
            error_code: return_code::PRECONDITION_NOT_MET,
            index: 0
        })
    );

    // The same for the other subscribing port types, without a filter.
    let get = DdsGet::<Track>::new();
    assert!(get.fresh_data().is_none());
    complete!(w, get).unwrap();
    assert_eq!(get.set_filter(filter.clone()), Err(NonChangeable));
    let listen = DdsListen::<Track>::new();
    complete!(w, listen).unwrap();
    assert_eq!(listen.set_filter(filter.clone()), Err(NonChangeable));
    let state = DdsStateListen::<Track>::new();
    complete!(w, state).unwrap();
    assert_eq!(state.set_filter(filter), Err(NonChangeable));

    // A filter DDS does not accept fails the configuration with the DDS
    // code; the port is not configured and takes another filter.
    let refused = DdsRead::<Track>::new();
    refused
        .set_filter(QueryFilter::new("value >", Vec::new()))
        .unwrap();
    let error = complete!(w, refused).unwrap_err();
    assert_ne!(error.error_code, return_code::OK);
    assert!(refused.data().is_none());
    refused
        .set_filter(QueryFilter::new("value > 3", Vec::new()))
        .unwrap();
    complete!(w, refused).unwrap();
    assert!(refused.data().is_some());
}

/// A filter with an expression makes the port subscribe through a
/// ContentFilteredTopic: every basic port of the port gets what passes the
/// filter and nothing else; a port without a filter gets everything.
#[test]
fn dds4ccm_7_2_2_2_filter_selects_what_the_reader_receives() {
    let w = World::new("Dds4CcmFilter");
    let filtered = DdsListen::<Track>::new();
    filtered
        .set_filter(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    let heard = Arc::new(Heard::default());
    filtered.connect_data_listener(heard.clone());
    filtered.data_control().set_mode(ListenerMode::OneByOne);
    complete!(w, filtered).unwrap();
    let plain = DdsRead::<Track>::new();
    complete!(w, plain).unwrap();
    let writer = keep_all_writer(&w);
    w.matched(&writer, 2);

    let description = filtered
        .dds_entity()
        .unwrap()
        .get_topicdescription()
        .unwrap();
    assert_eq!(
        description.kind(),
        TopicDescriptionKind::ContentFilteredTopic
    );
    assert_eq!(
        plain
            .dds_entity()
            .unwrap()
            .get_topicdescription()
            .unwrap()
            .kind(),
        TopicDescriptionKind::Topic
    );

    for (id, value) in [(1, 5), (2, 20), (3, 30), (1, 16), (2, 4)] {
        writer.write(&track(id, value)).unwrap();
    }
    w.delivered(&writer);
    assert_eq!(read_all_of(&*plain.data().unwrap(), 5).len(), 5);
    // The Listener of the filtered port.
    eventually("the listener heard what passes the filter", || {
        heard.one.lock().unwrap().len() == 3
    });
    let mut pushed: Vec<(u32, u32)> = heard
        .one
        .lock()
        .unwrap()
        .iter()
        .map(|(t, _)| (t.id, t.value))
        .collect();
    pushed.sort_unstable();
    assert_eq!(pushed, [(1, 16), (2, 20), (3, 30)]);
    // The Reader of the filtered port.
    let mut read = read_all_of(&*filtered.data().unwrap(), 3);
    read.sort_unstable();
    assert_eq!(read, [(1, 16), (2, 20), (3, 30)]);
    // The DataReader of the filtered port.
    let held = filtered
        .dds_entity()
        .unwrap()
        .read_selected(&ReadSelection::ANY)
        .unwrap();
    assert_eq!(held.len(), 3);
    assert!(held.iter().all(|s| s.data.value > 15));
}

/// `dds_entity` is the DataWriter or the DataReader the other basic ports
/// of the port work on.
#[test]
fn dds4ccm_7_2_2_2_dds_entity_is_the_underlying_reader() {
    let w = World::new("Dds4CcmEntity");
    let port = DdsGet::<Track>::new();
    complete!(w, port).unwrap();
    let raw = keep_all_writer(&w);
    let raw_handle = raw.instance_handle();
    let write = DdsWrite::new(raw, Arc::clone(&w.pub_errors));
    assert_eq!(write.dds_entity().instance_handle(), raw_handle);
    w.matched(write.dds_entity(), 1);
    let reader = port.data().unwrap();
    let entity = port.dds_entity().unwrap();
    assert_eq!(
        entity.instance_handle(),
        reader.dds_entity().instance_handle()
    );
    assert_eq!(entity.topic().name(), "Dds4CcmEntity");

    // What the Writer of the port writes leaves through its DataWriter.
    write.data().write_one(&track(1, 10), HANDLE_NIL).unwrap();
    write.data().write_one(&track(2, 20), HANDLE_NIL).unwrap();
    w.delivered(write.dds_entity());
    assert!(!write.dds_entity().lookup_instance(&track(2, 0)).is_nil());

    // What the Reader of the port read is READ on the DataReader.
    assert_eq!(read_all_of(&*reader, 2).len(), 2);
    assert!(matches!(
        entity.read_selected(&ReadSelection {
            sample_states: sample_state_mask::NOT_READ,
            ..ReadSelection::ANY
        }),
        Err(DdsError::NoData)
    ));
    // What is taken from the DataReader is gone for the port.
    let taken = entity.take().unwrap();
    assert_eq!(taken.len(), 2);
    let (mut data, mut infos) = (Vec::new(), Vec::<ReadInfo>::new());
    reader.read_all(&mut data, &mut infos).unwrap();
    assert!(data.is_empty());
    // And what the DataReader read is not fresh for the Getter.
    write.data().write_one(&track(3, 30), HANDLE_NIL).unwrap();
    w.delivered(write.dds_entity());
    eventually("the DataReader reads the sample", || {
        entity
            .read_selected(&ReadSelection::ANY)
            .is_ok_and(|samples| samples.len() == 1)
    });
    let getter = port.fresh_data().unwrap();
    let mut datum = track(0, 0);
    let mut info = ReadInfo::default();
    assert!(!getter.get_one(&mut datum, &mut info).unwrap());
}

/// The DataReader of a subscribing port is "entirely dedicated to" it: the
/// port creates it when its configuration is complete and deletes it, with
/// its ContentFilteredTopic, when the port is removed.
#[test]
fn dds4ccm_7_2_2_2_a_removed_port_deletes_its_reader() {
    let w = World::new("Dds4CcmRemove");
    let port = DdsRead::<Track>::new();
    // Not configured: nothing to remove.
    port.remove().unwrap();
    port.set_filter(QueryFilter::new("value > %0", vec!["15".into()]))
        .unwrap();
    complete!(w, port).unwrap();
    let writer = keep_all_writer(&w);
    w.matched(&writer, 1);
    let reader = port.data().unwrap();
    let entity = port.dds_entity().unwrap();
    reader
        .set_query(QueryFilter::new("value < 100", Vec::new()))
        .unwrap();

    port.remove().unwrap();
    eventually("the writer lost its reader", || {
        writer
            .wait_for_matched_subscription(1, std::time::Duration::from_millis(1))
            .is_err()
    });
    assert!(matches!(entity.take(), Err(DdsError::AlreadyDeleted)));
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    assert_eq!(
        reader.read_all(&mut data, &mut infos),
        Err(InternalError {
            error_code: return_code::ALREADY_DELETED,
            index: 0
        })
    );
    // The subscriber has no reader left and can be deleted.
    w.subscribing.delete_subscriber(&w.subscriber).unwrap();
}
