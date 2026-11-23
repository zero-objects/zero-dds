// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.4.5 Mapping when the DCPS model is fixed: a DLRL Cache and
//! a participant that acts at DCPS level share a topic whose key is
//! `SITE`, `NUM` and that has no class and no oid field. The DCPS side
//! holds the type of the IDL text below, built without the mapping.

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::sync::Arc;

use dds::{Bus, delete, enabled, mono, station_mapping, wait_until};
use zerodds_dcps::qos::{ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter, SerializedTypeSupport};
use zerodds_dcps::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos, TopicQos};
use zerodds_dlrl::cache::{CacheBase, CacheUsage, ObjectState};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::Value;
use zerodds_types::dynamic::{DynamicSample, DynamicType};

const IDL: &str = "module Legacy { @final struct StationType { @key string SITE; @key long NUM; double LEVEL; }; };";

fn dcps_type() -> (Arc<SerializedTypeSupport>, DynamicType) {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(IDL, "legacy.idl").unwrap());
    let ty = DynamicType::find(&ast, "Legacy::StationType").unwrap();
    let support = SerializedTypeSupport::create_type_support(&ty).unwrap();
    let ty = support.get_type().unwrap();
    (support, ty)
}

fn station(ty: &DynamicType, site: &str, num: i32, level: f64) -> DynamicSample {
    let mut s = DynamicSample::new(ty).unwrap();
    let id = s.get_member_id_by_name("SITE").unwrap();
    s.set_string_value(id, site).unwrap();
    let id = s.get_member_id_by_name("NUM").unwrap();
    s.set_int32_value(id, num).unwrap();
    let id = s.get_member_id_by_name("LEVEL").unwrap();
    s.set_float64_value(id, level).unwrap();
    s
}

#[test]
fn dlrl_8_1_4_5_a_fixed_dcps_model_is_read_and_written_as_objects() {
    let bus = Bus::new();
    let m = station_mapping();
    let (support, ty) = dcps_type();
    // The DCPS side: a plain DataWriter and DataReader of the fixed type.
    let plain = bus.participant();
    let topic = plain
        .create_dynamic_topic("STATION", TopicQos::default(), support)
        .unwrap();
    let writer: DynamicDataWriter = plain
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    let reader: DynamicDataReader = plain
        .create_subscriber(SubscriberQos::default())
        .create_datareader(
            &topic,
            DataReaderQos {
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::Reliable,
                    ..ReliabilityQosPolicy::default()
                },
                ..DataReaderQos::default()
            },
        )
        .unwrap();

    // The DLRL side.
    let side = enabled(&bus, CacheUsage::ReadWrite, &m, &["Station"]);
    let home = side.home("Station");
    let base = CacheBase::Cache(side.cache.clone());
    wait_until("the DCPS writer matched", || {
        !writer.get_matched_subscriptions().unwrap().is_empty()
    });

    // DCPS -> objects: one key value, one object with a local oid.
    writer.write(&station(&ty, "north", 7, 1.5)).unwrap();
    wait_until("the station as object", || {
        home.get_objects(&base).unwrap().len() == 1
    });
    let north = home.get_objects(&base).unwrap().pop().unwrap();
    assert_eq!(
        north.get("site").unwrap(),
        mono(Value::String("north".into()))
    );
    assert_eq!(north.get("num").unwrap(), mono(Value::Long(7)));
    assert_eq!(dds::dbl(&north, "level"), 1.5);
    let oid = north.oid();
    assert!(!oid.is_nil());
    // An update of the same key is the same object.
    writer.write(&station(&ty, "north", 7, 2.5)).unwrap();
    wait_until("the update", || dds::dbl(&north, "level") == 2.5);
    assert_eq!(home.get_objects(&base).unwrap().len(), 1);
    assert_eq!(north.oid(), oid);

    // Objects -> DCPS: the identity is the key fields; create_object has
    // no key to give, register_object derives the oid from them.
    let access = side.cache.create_access(CacheUsage::ReadWrite).unwrap();
    assert!(matches!(
        home.create_object(&access),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    let south = home.create_unregistered_object(&access).unwrap();
    assert!(south.oid().is_nil());
    south
        .set("site", mono(Value::String("south".into())))
        .unwrap();
    south.set("num", mono(Value::Long(3))).unwrap();
    south.set("level", mono(Value::Double(0.25))).unwrap();
    assert!(matches!(
        access.write(),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    home.register_object(&south).unwrap();
    assert!(!south.oid().is_nil());
    assert!(matches!(
        south.set("num", mono(Value::Long(4))),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    south.set("level", mono(Value::Double(0.5))).unwrap();
    access.write().unwrap();
    wait_until("the DCPS sample", || {
        reader.take().unwrap().iter().any(|s| {
            let mut s = s.clone();
            let site = s.get_member_id_by_name("SITE").unwrap();
            let num = s.get_member_id_by_name("NUM").unwrap();
            let level = s.get_member_id_by_name("LEVEL").unwrap();
            s.get_string_value(site).unwrap() == "south"
                && s.get_int32_value(num).unwrap() == 3
                && s.get_float64_value(level).unwrap() == 0.5
        })
    });
    // The key of an existing object cannot be registered again.
    let again = home.create_unregistered_object(&access).unwrap();
    again
        .set("site", mono(Value::String("north".into())))
        .unwrap();
    again.set("num", mono(Value::Long(7))).unwrap();
    again.set("level", mono(Value::Double(0.0))).unwrap();
    assert!(matches!(
        home.register_object(&again),
        Err(DlrlException::AlreadyExisting { .. })
    ));

    // A dispose at DCPS level deletes the object.
    writer
        .dispose(&station(&ty, "north", 7, 0.0), zerodds_dcps::HANDLE_NIL)
        .unwrap();
    wait_until("the deletion", || home.find_object(oid, &base).is_err());
    assert_eq!(north.read_state(), ObjectState::Deleted);
    delete(side);
}

/// An object of `create_unregistered_object` with the key values `site`,
/// `num`, registered.
fn register(
    home: &zerodds_dlrl::cache::ObjectHome,
    access: &zerodds_dlrl::cache::CacheAccess,
    site: &str,
    num: i32,
) -> Result<zerodds_dlrl::cache::ObjectRoot, DlrlException> {
    let o = home.create_unregistered_object(access).unwrap();
    o.set("site", mono(Value::String(site.into()))).unwrap();
    o.set("num", mono(Value::Long(num))).unwrap();
    o.set("level", mono(Value::Double(1.0))).unwrap();
    home.register_object(&o).map(|()| o)
}

/// §8.1.4.5: the implementation manages the association of key values
/// and oids; a key whose object no longer exists is free again. In a
/// WRITE_ONLY Cache (no DataReader, no incoming dispose): an object
/// destroyed and written; an object registered and rolled back by
/// `refresh`; an object registered and dropped by `purge` and by
/// `delete_access`. A key whose object was written and not destroyed
/// stays taken.
#[test]
fn dlrl_8_1_4_5_a_key_is_free_again_once_its_object_is_gone() {
    let bus = Bus::new();
    let m = station_mapping();
    let side = enabled(&bus, CacheUsage::WriteOnly, &m, &["Station"]);
    let home = side.home("Station");
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();

    // Written: the key is taken, also after the access dropped the
    // object (a refresh of a WRITE_ONLY access holds no clone).
    register(home, &access, "east", 1).unwrap();
    access.write().unwrap();
    assert!(matches!(
        register(home, &access, "east", 1),
        Err(DlrlException::AlreadyExisting { .. })
    ));
    access.refresh().unwrap();
    assert!(access.objects().is_empty());
    assert!(matches!(
        register(home, &access, "east", 1),
        Err(DlrlException::AlreadyExisting { .. })
    ));

    // Written, then destroyed and written: free again.
    let held = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let second = register(home, &held, "west", 2).unwrap();
    held.write().unwrap();
    second.destroy().unwrap();
    held.write().unwrap();
    let again = register(home, &held, "west", 2).unwrap();
    assert_ne!(again.oid(), second.oid());
    assert!(!again.oid().is_nil());

    // Registered, never written: rolled back by refresh.
    let rolled = register(home, &held, "north", 3).unwrap();
    held.refresh().unwrap();
    assert!(!held.objects().contains(&rolled));
    register(home, &held, "north", 3).unwrap();

    // Registered, destroyed, not written: rolled back by refresh.
    held.refresh().unwrap();
    assert!(held.objects().is_empty());
    let destroyed = register(home, &held, "south", 4).unwrap();
    destroyed.destroy().unwrap();
    held.refresh().unwrap();
    register(home, &held, "south", 4).unwrap();

    // Dropped by purge and by delete_access.
    held.purge().unwrap();
    let purged = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    register(home, &purged, "south", 4).unwrap();
    register(home, &purged, "north", 3).unwrap();
    side.cache.delete_access(&purged).unwrap();
    let last = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    register(home, &last, "south", 4).unwrap();
    register(home, &last, "north", 3).unwrap();
    // The written ones stay taken.
    assert!(matches!(
        register(home, &last, "east", 1),
        Err(DlrlException::AlreadyExisting { .. })
    ));
    delete(side);
}
