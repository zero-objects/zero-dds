// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data access, publishing side (DDS4CCM 1.1 §7.2.2.1.1): interface
//! `InstanceHandleManager`, interface `Writer` and interface `Updater`
//! behind the ports `DDS_Write` and `DDS_Update`, between two participants.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{
    POISON, PROBED, Track, World, eventually, open_at_encode, open_at_key, probe_coherent_changes,
    reliable_keep_all, track,
};
use zerodds_dcps::error::return_code;
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReader, DataWriter, DataWriterQos, DdsError, HANDLE_NIL, InstanceHandle, PublisherQos,
    ReadSelection,
};
use zerodds_dds4ccm::{
    AlreadyCreated, DdsUpdate, DdsWrite, InstanceHandleManager, InternalError, NonExistent,
    Updater, UpdaterError, Writer,
};
use zerodds_qos::{HistoryKind, HistoryQosPolicy};

/// A writer that keeps every sample until the reader has it.
fn keep_all_writer(w: &World) -> DataWriter<Track> {
    w.writer_with(DataWriterQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataWriterQos::default()
    })
}

/// A plain DataReader of the subscribing participant, matched with
/// `writer`: what the data space sees of the port under test.
fn subscribed(w: &World, writer: &DataWriter<Track>) -> DataReader<Track> {
    let reader = w
        .subscriber
        .create_datareader::<Track>(&w.sub_topic, reliable_keep_all())
        .expect("reader");
    w.matched(writer, 1);
    reader
}

/// Every sample with data `reader` holds, as `(id, value)`, once `writer`
/// has no sample left to deliver.
fn received(w: &World, writer: &DataWriter<Track>, reader: &DataReader<Track>) -> Vec<(u32, u32)> {
    w.delivered(writer);
    match reader.read_selected(&ReadSelection::ANY) {
        Ok(samples) => samples
            .iter()
            .filter(|s| s.info.valid_data)
            .map(|s| (s.data.id, s.data.value))
            .collect(),
        Err(DdsError::NoData) => Vec::new(),
        Err(error) => panic!("read: {error:?}"),
    }
}

/// The state `reader` holds of the instance of `id`.
fn instance_state(reader: &DataReader<Track>, id: u32) -> Option<InstanceStateKind> {
    match reader.read_selected(&ReadSelection::limited(0)) {
        Ok(_) | Err(DdsError::NoData) => {}
        Err(error) => panic!("read: {error:?}"),
    }
    let handle = reader.lookup_instance(&track(id, 0));
    reader
        .instance_tracker()
        .get_by_handle(handle)
        .map(|state| state.kind)
}

/// What the probe noted since the last call: whether the publisher had a
/// coherent set open at every DDS order, at none, or at some.
fn orders_in_a_coherent_set() -> Option<bool> {
    let noted = open_at_encode();
    assert!(!noted.is_empty(), "the batch reached DDS");
    if noted.iter().all(|open| *open) {
        Some(true)
    } else if noted.iter().all(|open| !*open) {
        Some(false)
    } else {
        None
    }
}

fn internal(code: i32, index: u32) -> InternalError {
    InternalError {
        error_code: code,
        index,
    }
}

/// `InstanceHandleManager`: register_instance returns the handle of the
/// key, the same for the same key; unregister_instance releases it, by the
/// handle or by the key; a DDS error is an InternalError with its code.
#[test]
fn dds4ccm_7_2_2_1_1_instance_handle_manager_registers_and_unregisters_through_dds() {
    let w = World::new("Dds4CcmHandles");
    let port = DdsWrite::new(w.writer(), Arc::clone(&w.pub_errors));
    let data = port.data();

    let one = data.register_instance(&track(1, 0)).unwrap();
    let two = data.register_instance(&track(2, 0)).unwrap();
    assert!(!one.is_nil() && !two.is_nil());
    assert_ne!(one, two);
    assert_eq!(data.register_instance(&track(1, 99)).unwrap(), one);
    assert_eq!(port.dds_entity().lookup_instance(&track(1, 0)), one);

    data.unregister_instance(&track(1, 0), one).unwrap();
    data.unregister_instance(&track(2, 0), HANDLE_NIL).unwrap();
    // The handle of another instance than the key: the error of DDS.
    let three = data.register_instance(&track(3, 0)).unwrap();
    let four = data.register_instance(&track(4, 0)).unwrap();
    let expected = port
        .dds_entity()
        .unregister_instance(&track(3, 0), four)
        .unwrap_err();
    assert_eq!(
        data.unregister_instance(&track(3, 0), four),
        Err(InternalError::from_dds(&expected, 0))
    );
    data.unregister_instance(&track(3, 0), three).unwrap();
}

/// `write_one`: the instance is the one of the handle if it is not
/// HANDLE_NIL, else the one of the key; a handle that is not in accordance
/// with the key is an InternalError with the DDS code, and nothing is
/// written.
#[test]
fn dds4ccm_7_2_2_1_1_write_one_with_a_handle_that_contradicts_the_key_is_internal_error() {
    let w = World::new("Dds4CcmWriteOne");
    let port = DdsWrite::new(keep_all_writer(&w), Arc::clone(&w.pub_errors));
    let reader = subscribed(&w, port.dds_entity());
    let data = port.data();

    let one = data.register_instance(&track(1, 0)).unwrap();
    let two = data.register_instance(&track(2, 0)).unwrap();
    data.write_one(&track(1, 10), HANDLE_NIL).unwrap();
    data.write_one(&track(1, 11), one).unwrap();
    assert_eq!(
        data.write_one(&track(1, 12), two),
        Err(internal(return_code::PRECONDITION_NOT_MET, 0))
    );
    assert_eq!(
        data.write_one(&track(1, 13), InstanceHandle::from_raw(0x00DE_AD00)),
        Err(internal(return_code::BAD_PARAMETER, 0))
    );
    // Any other DDS error of the write.
    let expected = port.dds_entity().write(&track(1, POISON)).unwrap_err();
    assert_eq!(
        data.write_one(&track(1, POISON), HANDLE_NIL),
        Err(InternalError::from_dds(&expected, 0))
    );
    assert_eq!(received(&w, port.dds_entity(), &reader), [(1, 10), (1, 11)]);
}

/// `write_many`: the DDS orders stop at the first error, whose index the
/// InternalError carries; what was written before it stays written.
#[test]
fn dds4ccm_7_2_2_1_1_write_many_stops_at_the_first_error_and_reports_its_index() {
    let w = World::new("Dds4CcmWriteMany");
    let port = DdsWrite::new(keep_all_writer(&w), Arc::clone(&w.pub_errors));
    let reader = subscribed(&w, port.dds_entity());
    let data = port.data();

    data.write_many(&[track(1, 1), track(2, 2), track(1, 3)])
        .unwrap();
    data.write_many(&[]).unwrap();
    let expected = port.dds_entity().write(&track(9, POISON)).unwrap_err();
    assert_eq!(
        data.write_many(&[
            track(3, 4),
            track(4, 5),
            track(5, POISON),
            track(6, 6),
            track(7, POISON)
        ]),
        Err(InternalError::from_dds(&expected, 2))
    );
    let mut got = received(&w, port.dds_entity(), &reader);
    got.sort_unstable();
    assert_eq!(got, [(1, 1), (1, 3), (2, 2), (3, 4), (4, 5)]);
}

/// `is_coherent_write`, FALSE by default: with TRUE the DDS orders of a
/// `_many` operation are between begin_coherent_changes and
/// end_coherent_changes of the publisher, and the set is closed also after
/// an error. Writer and Updater.
#[test]
fn dds4ccm_7_2_2_1_1_coherent_write_brackets_the_batch() {
    let w = World::new("Dds4CcmCoherent");
    // A publisher of its own per port: the probe looks at one.
    let publisher = w.publishing.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<Track>(&w.pub_topic, DataWriterQos::default())
        .unwrap();
    let port = DdsWrite::new(writer, Arc::clone(&w.pub_errors));
    let data = port.data();
    probe_coherent_changes(port.dds_entity().get_publisher());
    let batch = [track(PROBED, 1), track(PROBED + 1, 2), track(PROBED + 2, 3)];

    assert!(!data.is_coherent_write());
    data.write_many(&batch).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(false));

    data.set_is_coherent_write(true);
    assert!(data.is_coherent_write());
    data.write_many(&batch).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(true));
    assert!(!publisher.coherent_changes_open(), "the set is closed");

    // write_one is one order, not a batch.
    data.write_one(&track(PROBED, 4), HANDLE_NIL).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(false));

    assert!(
        data.write_many(&[track(PROBED, 5), track(PROBED + 1, POISON)])
            .is_err()
    );
    assert_eq!(orders_in_a_coherent_set(), Some(true));
    assert!(
        !publisher.coherent_changes_open(),
        "the set is closed after an error"
    );

    // The Updater, over a writer of the same publisher.
    let writer = publisher
        .create_datawriter::<Track>(&w.pub_topic, DataWriterQos::default())
        .unwrap();
    let port = DdsUpdate::new(writer, Arc::clone(&w.pub_errors));
    let data = port.data();
    let batch = [track(PROBED + 10, 1), track(PROBED + 11, 2)];
    assert!(!Updater::is_coherent_write(&*data));
    data.create_many(&batch).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(false));
    Updater::set_is_coherent_write(&*data, true);
    assert!(Updater::is_coherent_write(&*data));
    data.update_many(&batch).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(true));
    assert!(!publisher.coherent_changes_open());
    // update_one is one order, not a batch.
    data.update_one(&batch[0], HANDLE_NIL).unwrap();
    assert_eq!(orders_in_a_coherent_set(), Some(false));

    // "successful write or dispose DDS orders": a dispose order of a keyed
    // type encodes no sample, so the probe at the sample does not see it.
    // The DataWriter computes the key of the datum of every order; the
    // probe at the key notes each of the two orders of a deletion.
    let entity = port.dds_entity();
    let lone = track(PROBED + 20, 1);
    data.create_one(&lone, HANDLE_NIL).unwrap();
    open_at_key();
    let handle = entity.register_instance(&lone).unwrap();
    let at_register = open_at_key().len();
    entity.dispose(&lone, handle).unwrap();
    let at_dispose = open_at_key();
    assert!(
        !at_dispose.is_empty(),
        "a dispose order computes the key of its datum"
    );
    assert!(
        at_dispose.iter().all(|open| !*open),
        "no set is open around an order of the test"
    );
    let per_deletion = at_register + at_dispose.len();

    // The lifecycle check of the fragment looks the instances up before the
    // first order; the orders are the last key computations of the call.
    let orders = batch.len() * per_deletion;
    let of_orders = |noted: &[bool]| -> Vec<bool> {
        assert!(
            noted.len() >= orders,
            "register_instance and dispose of every datum: {noted:?}"
        );
        noted[noted.len() - orders..].to_vec()
    };
    data.delete_many(&batch).unwrap();
    let noted = of_orders(&open_at_key());
    assert!(
        noted.iter().all(|open| *open),
        "every dispose order is in the coherent set: {noted:?}"
    );
    assert!(!publisher.coherent_changes_open(), "the set is closed");

    // Without is_coherent_write the same orders are in no set.
    Updater::set_is_coherent_write(&*data, false);
    data.create_many(&batch).unwrap();
    open_at_key();
    data.delete_many(&batch).unwrap();
    let noted = of_orders(&open_at_key());
    assert!(noted.iter().all(|open| !*open), "{noted:?}");
    // delete_one is one order, not a batch.
    Updater::set_is_coherent_write(&*data, true);
    data.create_many(&batch).unwrap();
    open_at_key();
    data.delete_one(&batch[0], HANDLE_NIL).unwrap();
    let noted = open_at_key();
    assert!(noted.len() >= per_deletion, "{noted:?}");
    assert!(noted.iter().all(|open| !*open), "{noted:?}");
}

/// `create_one`: an instance that exists is AlreadyCreated with index 0,
/// and DDS is not asked to write.
#[test]
fn dds4ccm_7_2_2_1_1_create_one_of_an_existing_instance_is_already_created() {
    let w = World::new("Dds4CcmCreateOne");
    let port = DdsUpdate::new(keep_all_writer(&w), Arc::clone(&w.pub_errors));
    let reader = subscribed(&w, port.dds_entity());
    let data = port.data();
    assert!(!data.is_global_scope(), "FALSE by default");

    data.create_one(&track(1, 10), HANDLE_NIL).unwrap();
    assert_eq!(
        data.create_one(&track(1, 11), HANDLE_NIL),
        Err(UpdaterError::AlreadyCreated(AlreadyCreated {
            indexes: vec![0]
        }))
    );
    // The handle of another instance than the key.
    let one = port.dds_entity().lookup_instance(&track(1, 0));
    assert_eq!(
        data.create_one(&track(2, 20), one),
        Err(UpdaterError::InternalError(internal(
            return_code::PRECONDITION_NOT_MET,
            0
        )))
    );
    assert_eq!(received(&w, port.dds_entity(), &reader), [(1, 10)]);
}

/// `update_one` and `delete_one`: an instance that does not exist is
/// NonExistent with index 0; the instance is the one of the handle or of
/// the key; `delete_one` disposes the instance, after which it does not
/// exist and can be created again.
#[test]
fn dds4ccm_7_2_2_1_1_delete_one_disposes_and_update_one_needs_an_existing_instance() {
    let w = World::new("Dds4CcmUpdateDelete");
    let port = DdsUpdate::new(keep_all_writer(&w), Arc::clone(&w.pub_errors));
    let reader = subscribed(&w, port.dds_entity());
    let data = port.data();
    let missing = || Err(UpdaterError::NonExistent(NonExistent { indexes: vec![0] }));

    assert_eq!(data.update_one(&track(1, 1), HANDLE_NIL), missing());
    assert_eq!(data.delete_one(&track(1, 1), HANDLE_NIL), missing());
    // Registered is not created.
    let one = data.register_instance(&track(1, 0)).unwrap();
    assert_eq!(data.update_one(&track(1, 1), one), missing());

    data.create_one(&track(1, 10), HANDLE_NIL).unwrap();
    data.update_one(&track(1, 11), HANDLE_NIL).unwrap();
    data.update_one(&track(1, 12), one).unwrap();
    data.create_one(&track(2, 20), HANDLE_NIL).unwrap();
    let two = port.dds_entity().lookup_instance(&track(2, 0));
    assert_eq!(
        data.update_one(&track(1, 13), two),
        Err(UpdaterError::InternalError(internal(
            return_code::PRECONDITION_NOT_MET,
            0
        )))
    );
    assert_eq!(
        data.delete_one(&track(1, 0), InstanceHandle::from_raw(0x00DE_AD00)),
        Err(UpdaterError::InternalError(internal(
            return_code::BAD_PARAMETER,
            0
        )))
    );
    assert_eq!(
        received(&w, port.dds_entity(), &reader),
        [(1, 10), (1, 11), (1, 12), (2, 20)]
    );
    assert_eq!(instance_state(&reader, 1), Some(InstanceStateKind::Alive));

    data.delete_one(&track(1, 0), one).unwrap();
    w.delivered(port.dds_entity());
    eventually("the reader sees the instance disposed", || {
        instance_state(&reader, 1) == Some(InstanceStateKind::NotAliveDisposed)
    });
    assert_eq!(instance_state(&reader, 2), Some(InstanceStateKind::Alive));
    assert_eq!(data.update_one(&track(1, 14), HANDLE_NIL), missing());
    assert_eq!(data.delete_one(&track(1, 0), HANDLE_NIL), missing());

    data.create_one(&track(1, 15), HANDLE_NIL).unwrap();
    w.delivered(port.dds_entity());
    eventually("the instance lives again", || {
        instance_state(&reader, 1) == Some(InstanceStateKind::Alive)
    });
}

/// `create_many`, `update_many`, `delete_many`: the lifecycle check is
/// applied to all submitted instances before any DDS order; every
/// erroneous instance is reported by its index and nothing is written.
#[test]
fn dds4ccm_7_2_2_1_1_update_many_reports_every_missing_index() {
    let w = World::new("Dds4CcmMany");
    let port = DdsUpdate::new(keep_all_writer(&w), Arc::clone(&w.pub_errors));
    let reader = subscribed(&w, port.dds_entity());
    let data = port.data();

    data.create_many(&[track(1, 10), track(2, 20), track(3, 30)])
        .unwrap();
    assert_eq!(
        data.create_many(&[track(4, 40), track(2, 21), track(5, 50), track(3, 31)]),
        Err(UpdaterError::AlreadyCreated(AlreadyCreated {
            indexes: vec![1, 3]
        }))
    );
    // One instance twice in a create: the second finds it created.
    assert_eq!(
        data.create_many(&[track(6, 60), track(6, 61)]),
        Err(UpdaterError::AlreadyCreated(AlreadyCreated {
            indexes: vec![1]
        }))
    );
    assert_eq!(
        data.update_many(&[track(7, 70), track(1, 11), track(8, 80), track(9, 90)]),
        Err(UpdaterError::NonExistent(NonExistent {
            indexes: vec![0, 2, 3]
        }))
    );
    assert_eq!(
        data.delete_many(&[track(1, 0), track(7, 0)]),
        Err(UpdaterError::NonExistent(NonExistent { indexes: vec![1] }))
    );
    // Nothing of the refused calls reached DDS.
    let mut got = received(&w, port.dds_entity(), &reader);
    got.sort_unstable();
    assert_eq!(got, [(1, 10), (2, 20), (3, 30)]);
    assert!(instance_state(&reader, 4).is_none());
    assert!(instance_state(&reader, 6).is_none());

    // Updates of one instance twice are two updates.
    data.update_many(&[track(1, 12), track(2, 22), track(1, 13)])
        .unwrap();
    let mut got = received(&w, port.dds_entity(), &reader);
    got.sort_unstable();
    assert_eq!(got, [(1, 10), (1, 12), (1, 13), (2, 20), (2, 22), (3, 30)]);

    // A DDS error after the check: the orders stop there, with the index.
    let expected = port.dds_entity().write(&track(1, POISON)).unwrap_err();
    assert_eq!(
        data.update_many(&[track(3, 32), track(2, POISON), track(1, 14)]),
        Err(UpdaterError::InternalError(InternalError::from_dds(
            &expected, 1
        )))
    );
    let got = received(&w, port.dds_entity(), &reader);
    assert!(got.contains(&(3, 32)) && !got.contains(&(1, 14)), "{got:?}");

    data.delete_many(&[track(1, 0), track(3, 0)]).unwrap();
    w.delivered(port.dds_entity());
    eventually("the reader sees both instances disposed", || {
        instance_state(&reader, 1) == Some(InstanceStateKind::NotAliveDisposed)
            && instance_state(&reader, 3) == Some(InstanceStateKind::NotAliveDisposed)
    });
    assert_eq!(instance_state(&reader, 2), Some(InstanceStateKind::Alive));
    assert_eq!(
        data.update_many(&[track(1, 15), track(2, 23), track(3, 33)]),
        Err(UpdaterError::NonExistent(NonExistent {
            indexes: vec![0, 2]
        }))
    );
}

/// `is_global_scope`: with TRUE the lifecycle check is global to the data
/// space. An instance another participant created exists, one it disposed
/// does not; with FALSE the Updater knows its own instances only.
#[test]
fn dds4ccm_7_2_2_1_1_is_global_scope_checks_the_lifecycle_in_the_data_space() {
    let w = World::new("Dds4CcmGlobal");
    // The other participant of the data space writes too.
    let other = w
        .subscribing
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Track>(&w.sub_topic, DataWriterQos::default())
        .unwrap();
    let observer = w
        .publishing
        .create_subscriber(zerodds_dcps::SubscriberQos::default())
        .create_datareader::<Track>(&w.pub_topic, reliable_keep_all())
        .unwrap();
    other
        .wait_for_matched_subscription(1, std::time::Duration::from_secs(15))
        .unwrap();
    let global = DdsUpdate::with_global_scope(w.writer(), observer, Arc::clone(&w.pub_errors));
    let local = DdsUpdate::new(w.writer(), Arc::clone(&w.pub_errors));
    let (global, local) = (global.data(), local.data());
    assert!(global.is_global_scope());
    assert!(!local.is_global_scope());
    let observer = global.observer().expect("the observer of the global scope");

    other.write(&track(7, 70)).unwrap();
    other
        .wait_for_acknowledgments(std::time::Duration::from_secs(10))
        .unwrap();
    eventually("the observer sees the instance of the other", || {
        instance_state(observer, 7) == Some(InstanceStateKind::Alive)
    });
    assert_eq!(
        global.create_one(&track(7, 71), HANDLE_NIL),
        Err(UpdaterError::AlreadyCreated(AlreadyCreated {
            indexes: vec![0]
        }))
    );
    assert_eq!(
        local.update_one(&track(7, 72), HANDLE_NIL),
        Err(UpdaterError::NonExistent(NonExistent { indexes: vec![0] }))
    );
    global.update_one(&track(7, 73), HANDLE_NIL).unwrap();
    // Its own instances exist for it before the observer has them.
    global.create_one(&track(8, 80), HANDLE_NIL).unwrap();
    global.update_one(&track(8, 81), HANDLE_NIL).unwrap();
    assert_eq!(
        global.create_many(&[track(9, 90), track(7, 74), track(8, 82)]),
        Err(UpdaterError::AlreadyCreated(AlreadyCreated {
            indexes: vec![1, 2]
        }))
    );

    // The other participant disposes the instance: it does not exist.
    other.dispose(&track(7, 0), HANDLE_NIL).unwrap();
    other
        .wait_for_acknowledgments(std::time::Duration::from_secs(10))
        .unwrap();
    eventually("the observer sees the disposal of the other", || {
        instance_state(observer, 7) == Some(InstanceStateKind::NotAliveDisposed)
    });
    assert_eq!(
        global.update_one(&track(7, 75), HANDLE_NIL),
        Err(UpdaterError::NonExistent(NonExistent { indexes: vec![0] }))
    );
    global.create_one(&track(7, 76), HANDLE_NIL).unwrap();
    // A deletion of this Updater counts before the observer has it.
    global.delete_one(&track(8, 0), HANDLE_NIL).unwrap();
    assert_eq!(
        global.update_one(&track(8, 83), HANDLE_NIL),
        Err(UpdaterError::NonExistent(NonExistent { indexes: vec![0] }))
    );
}

/// The interfaces are object safe and a port hands them out as the
/// specification names them: `Writer : InstanceHandleManager`,
/// `Updater : InstanceHandleManager`.
#[test]
fn dds4ccm_7_2_2_1_1_writer_and_updater_inherit_the_instance_handle_manager() {
    let w = World::new("Dds4CcmInherit");
    let write = DdsWrite::new(w.writer(), Arc::clone(&w.pub_errors));
    let update = DdsUpdate::new(w.writer(), Arc::clone(&w.pub_errors));
    let writer: Arc<dyn Writer<Track>> = write.data();
    let updater: Arc<dyn Updater<Track>> = update.data();
    fn registers(manager: &dyn InstanceHandleManager<Track>) -> InstanceHandle {
        manager.register_instance(&track(1, 0)).unwrap()
    }
    let as_manager: &dyn InstanceHandleManager<Track> = &*write.data();
    assert!(!registers(as_manager).is_nil());
    let as_manager: &dyn InstanceHandleManager<Track> = &*update.data();
    assert!(!registers(as_manager).is_nil());
    writer.write_one(&track(1, 1), HANDLE_NIL).unwrap();
    updater.create_one(&track(1, 1), HANDLE_NIL).unwrap();
}
