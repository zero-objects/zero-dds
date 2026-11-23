// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex E (non-normative), "QoS Policies for the DDS Patterns": the two
//! profiles of `zerodds_dds4ccm::profiles` against the four tables of the
//! annex, and connectors configured with them over DDS.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

mod common;

use std::process::Command;

use common::{Deployment, Track, eventually, track};
use zerodds_dcps::qos::{
    DestinationOrderKind, DurabilityKind, HistoryKind, PresentationAccessScope,
    PresentationQosPolicy, ReliabilityKind,
};
use zerodds_dcps::{Entity, HANDLE_NIL};
use zerodds_dds4ccm::{DdsGet, DdsRead, Getter, ReadInfo, Reader, Updater, Writer, profiles};
use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderQosPolicy, DurabilityQosPolicy, DurabilityServiceQosPolicy,
    Duration, EntityFactoryQosPolicy, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy,
    LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy, PartitionQosPolicy,
    ReaderDataLifecycleQosPolicy, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy, TransportPriorityQosPolicy, WriterDataLifecycleQosPolicy,
};
use zerodds_xml::{
    CCM_LIBRARY, DDS4CCM_QOS_PROFILE_XSD, EntityQos, QosProfile, ReadOptions, parse_dds_xml_with,
};

/// The one profile of a pattern document.
fn profile_of(document: &str, name: &str) -> QosProfile {
    let parsed =
        parse_dds_xml_with(document, &ReadOptions::accept_vendor_forms()).expect("the profile");
    let library = parsed
        .value
        .qos_libraries
        .into_iter()
        .find(|l| l.name == CCM_LIBRARY)
        .expect("a profile directly under <dds>");
    assert_eq!(library.profiles.len(), 1);
    let profile = library.profiles.into_iter().next().unwrap();
    assert_eq!(profile.name, name);
    profile
}

/// The one container of a kind, without a topic filter.
fn one(slots: &[zerodds_xml::FilteredEntityQos]) -> EntityQos {
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].topic_filter, None);
    slots[0].qos.clone()
}

/// What the four tables give a Topic, a DataWriter and a DataReader alike:
/// "Deadline infinite", "Latency budget 0", "Liveness AUTOMATIC
/// lease_duration=infinite", "Ownership SHARED", "Resource limits
/// max_*=LENGTH_UNLIMITED", "Destination order BY_SOURCE_TIMESTAMP".
fn shared_rows(durability: DurabilityKind, reliability: ReliabilityKind) -> EntityQos {
    EntityQos {
        durability: Some(DurabilityQosPolicy { kind: durability }),
        deadline: Some(DeadlineQosPolicy {
            period: Duration::INFINITE,
        }),
        latency_budget: Some(LatencyBudgetQosPolicy {
            duration: Duration::ZERO,
        }),
        liveliness: Some(LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: Duration::INFINITE,
        }),
        reliability: Some(ReliabilityQosPolicy {
            kind: reliability,
            max_blocking_time: ReliabilityQosPolicy::default().max_blocking_time,
        }),
        destination_order: Some(DestinationOrderQosPolicy {
            kind: DestinationOrderKind::BySourceTimestamp,
        }),
        resource_limits: Some(ResourceLimitsQosPolicy {
            max_samples: -1,
            max_instances: -1,
            max_samples_per_instance: -1,
        }),
        ownership: Some(OwnershipQosPolicy {
            kind: OwnershipKind::Shared,
        }),
        ..EntityQos::default()
    }
}

/// The rows a Topic and a DataWriter have and a DataReader has not:
/// "Lifespan infinite", "Transport priority 0".
fn writing_rows(mut qos: EntityQos) -> EntityQos {
    qos.lifespan = Some(LifespanQosPolicy {
        duration: Duration::INFINITE,
    });
    qos.transport_priority = Some(TransportPriorityQosPolicy { value: 0 });
    qos
}

/// The rows of a DataReader: "Reader data lifecycle
/// autopurge_nowriter_samples_delay=infinite
/// autopurge_disposed_samples_delay=infinite", "Time based filter
/// minimum_separation=0".
fn reading_rows(mut qos: EntityQos) -> EntityQos {
    qos.time_based_filter = Some(TimeBasedFilterQosPolicy {
        minimum_separation: Duration::ZERO,
    });
    qos.reader_data_lifecycle = Some(ReaderDataLifecycleQosPolicy {
        autopurge_nowriter_samples_delay: Duration::INFINITE,
        autopurge_disposed_samples_delay: Duration::INFINITE,
    });
    qos
}

/// Publisher and Subscriber, the same in the four tables: "Entity factory
/// autoenabled_created_entities=TRUE", "Partition \"\"", "Presentation
/// INSTANCE coherent_access=FALSE ordered_access=TRUE".
fn group_rows() -> EntityQos {
    EntityQos {
        presentation: Some(PresentationQosPolicy {
            access_scope: PresentationAccessScope::Instance,
            coherent_access: false,
            ordered_access: true,
        }),
        partition: Some(PartitionQosPolicy { names: Vec::new() }),
        entity_factory: Some(EntityFactoryQosPolicy {
            autoenable_created_entities: true,
        }),
        ..EntityQos::default()
    }
}

/// Annex E, tables "Observable / State Pattern" and "Observer / State
/// Pattern": DURABILITY TRANSIENT_LOCAL, RELIABILITY RELIABLE, HISTORY
/// KEEP_LAST depth=1, and the durability service of the observable.
#[test]
fn dds4ccm_annex_e_the_state_profile_has_the_values_of_the_observable_and_the_observer_table() {
    let p = profile_of(profiles::STATE_TRANSFER, "StateTransfer");
    let rows = || {
        let mut qos = shared_rows(DurabilityKind::TransientLocal, ReliabilityKind::Reliable);
        qos.history = Some(HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        });
        qos
    };
    // "Durability service service_cleanup_delay=0 history_kind=KEEP_LAST
    // history_depth=1 max_*=LENGTH_UNLIMITED": Topic and DataWriter of the
    // observable.
    let with_service = |mut qos: EntityQos| {
        qos.durability_service = Some(DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::ZERO,
            history_kind: HistoryKind::KeepLast,
            history_depth: 1,
            max_samples: -1,
            max_instances: -1,
            max_samples_per_instance: -1,
        });
        qos
    };
    assert_eq!(
        one(&p.topic_qos),
        with_service(writing_rows(rows())),
        "Topic"
    );
    assert_eq!(
        one(&p.datawriter_qos),
        with_service(writing_rows(rows())),
        "Data Writer"
    );
    assert_eq!(one(&p.datareader_qos), reading_rows(rows()), "Data Reader");
    assert_eq!(one(&p.publisher_qos), group_rows(), "Publisher");
    assert_eq!(one(&p.subscriber_qos), group_rows(), "Subscriber");
    // The tables have no column for the DomainParticipant.
    assert!(p.domainparticipant_qos.is_empty());
}

/// Annex E, tables "Supplier / Event Pattern" and "Consumer / Event
/// Pattern": DURABILITY VOLATILE, RELIABILITY BEST_EFFORT, HISTORY
/// KEEP_ALL, and "Writer data lifecycle
/// autodispose_unregistered_instance=FALSE" of the supplier.
#[test]
fn dds4ccm_annex_e_the_event_profile_has_the_values_of_the_supplier_and_the_consumer_table() {
    let p = profile_of(profiles::EVENT_TRANSFER, "EventTransfer");
    let rows = || {
        let mut qos = shared_rows(DurabilityKind::Volatile, ReliabilityKind::BestEffort);
        // The tables give the kind and no depth.
        qos.history = Some(HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: HistoryQosPolicy::default().depth,
        });
        qos
    };
    assert_eq!(one(&p.topic_qos), writing_rows(rows()), "Topic");
    let mut writer = writing_rows(rows());
    writer.writer_data_lifecycle = Some(WriterDataLifecycleQosPolicy {
        autodispose_unregistered_instances: false,
    });
    assert_eq!(one(&p.datawriter_qos), writer, "Data Writer");
    assert_eq!(one(&p.datareader_qos), reading_rows(rows()), "Data Reader");
    assert_eq!(one(&p.publisher_qos), group_rows(), "Publisher");
    assert_eq!(one(&p.subscriber_qos), group_rows(), "Subscriber");
    assert!(p.domainparticipant_qos.is_empty());
}

/// Annex E — both documents are QoS profiles of §7.4.2: valid against the
/// schema of Annex C, asked of `xmllint`. Without `xmllint` the test prints
/// `[skip]`.
#[test]
fn dds4ccm_annex_e_the_pattern_profiles_are_valid_against_the_schema_of_annex_c() {
    let prog = std::env::var("ZERODDS_XMLLINT").unwrap_or_else(|_| "xmllint".into());
    match Command::new(&prog).arg("--version").output() {
        Ok(out) if out.status.success() => {}
        _ => {
            eprintln!("[skip] xmllint not runnable (set ZERODDS_XMLLINT to override)");
            return;
        }
    }
    let dir = std::env::temp_dir().join("zerodds-dds4ccm-pattern-profiles");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let schema = dir.join("dds4ccm_qos_profile.xsd");
    std::fs::write(&schema, DDS4CCM_QOS_PROFILE_XSD).expect("schema");
    let validate = |name: &str, document: &str| -> bool {
        let path = dir.join(name);
        std::fs::write(&path, document).expect("document");
        let out = Command::new(&prog)
            .arg("--nonet")
            .arg("--noout")
            .arg("--schema")
            .arg(&schema)
            .arg(&path)
            .output()
            .expect("xmllint runs");
        out.status.success()
    };
    // The validator says no to a document that is not a profile.
    assert!(!validate(
        "not_a_profile.xml",
        "<dds xmlns=\"http://www.omg.org/dds/\"><qos_profile/></dds>"
    ));
    assert!(validate("state_transfer.xml", profiles::STATE_TRANSFER));
    assert!(validate("event_transfer.xml", profiles::EVENT_TRANSFER));
}

fn pairs(data: &[Track]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<_> = data.iter().map(|t| (t.id, t.value)).collect();
    pairs.sort_unstable();
    pairs
}

/// Annex E over DDS, pattern State Transfer: the entities of a `DDS_State`
/// connector with the profile have its values, and an observer that comes
/// after the observable wrote gets the last value of every instance
/// (TRANSIENT_LOCAL, RELIABLE, KEEP_LAST depth 1).
#[test]
fn dds4ccm_annex_e_a_state_connector_with_the_state_profile_gives_a_late_observer_the_last_value() {
    let d = Deployment::new();
    let publishing = d.state("Dds4CcmAnnexEState", profiles::STATE_TRANSFER);
    let observable = publishing.observable(&d.publisher(), false).unwrap();

    let writer = observable.dds_entity();
    let qos = writer.qos();
    assert_eq!(qos.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        (qos.history.kind, qos.history.depth),
        (HistoryKind::KeepLast, 1)
    );
    assert_eq!(
        qos.destination_order.kind,
        DestinationOrderKind::BySourceTimestamp
    );
    let ordered = PresentationQosPolicy {
        access_scope: PresentationAccessScope::Instance,
        coherent_access: false,
        ordered_access: true,
    };
    assert_eq!(writer.get_publisher().get_qos().presentation, ordered);
    let topic_qos = writer.topic().qos();
    assert_eq!(topic_qos.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(topic_qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        topic_qos.destination_order.kind,
        DestinationOrderKind::BySourceTimestamp
    );

    // The state, before any observer exists: two values of the first
    // instance, one of the second.
    let updater = observable.data();
    updater.create_one(&track(1, 10), HANDLE_NIL).unwrap();
    updater.update_one(&track(1, 11), HANDLE_NIL).unwrap();
    updater.create_one(&track(2, 20), HANDLE_NIL).unwrap();

    let subscribing = d.state("Dds4CcmAnnexEState", profiles::STATE_TRANSFER);
    let late = DdsRead::<Track>::new();
    subscribing
        .passive_observer(&d.subscriber(), &late)
        .unwrap();
    let reader = late.dds_entity().unwrap();
    let qos = reader.qos();
    assert_eq!(qos.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        (qos.history.kind, qos.history.depth),
        (HistoryKind::KeepLast, 1)
    );
    assert_eq!(
        qos.destination_order.kind,
        DestinationOrderKind::BySourceTimestamp
    );
    assert_eq!(reader.get_subscriber().get_qos().presentation, ordered);

    let port = late.data().unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::<ReadInfo>::new());
    eventually("the late observer holds the state", || {
        port.read_all(&mut data, &mut infos).unwrap();
        data.len() == 2
    });
    // The last value of each instance and not the one before it.
    assert_eq!(pairs(&data), [(1, 11), (2, 20)]);

    drop(observable);
    publishing.remove().unwrap();
    subscribing.remove().unwrap();
}

/// Annex E over DDS, pattern Event Transfer: the entities of a `DDS_Event`
/// connector with the profile have its values (VOLATILE, BEST_EFFORT,
/// KEEP_ALL, no automatic dispose), an event sent while the consumer
/// listens reaches it, and a consumer gets no event that was sent before
/// it came (VOLATILE).
#[test]
fn dds4ccm_annex_e_an_event_connector_with_the_event_profile_delivers_what_is_sent_from_then_on() {
    let d = Deployment::new();
    let supplying = d.event("Dds4CcmAnnexEEvent", profiles::EVENT_TRANSFER);
    let supplier = supplying.supplier(&d.publisher()).unwrap();

    let writer = supplier.dds_entity();
    let qos = writer.qos();
    assert_eq!(qos.durability.kind, DurabilityKind::Volatile);
    assert_eq!(qos.reliability.kind, ReliabilityKind::BestEffort);
    assert_eq!(qos.history.kind, HistoryKind::KeepAll);
    assert_eq!(
        qos.destination_order.kind,
        DestinationOrderKind::BySourceTimestamp
    );
    assert!(!qos.writer_data_lifecycle.autodispose_unregistered_instances);
    assert!(writer.get_publisher().get_qos().presentation.ordered_access);

    // Events nobody listens to.
    for value in 1..=3 {
        supplier
            .data()
            .write_one(&track(1, value), HANDLE_NIL)
            .unwrap();
    }

    let consuming = d.event("Dds4CcmAnnexEEvent", profiles::EVENT_TRANSFER);
    let pull = DdsGet::<Track>::new();
    consuming.pull_consumer(&d.subscriber(), &pull).unwrap();
    let reader = pull.dds_entity().unwrap();
    let qos = reader.qos();
    assert_eq!(qos.durability.kind, DurabilityKind::Volatile);
    assert_eq!(qos.reliability.kind, ReliabilityKind::BestEffort);
    assert_eq!(qos.history.kind, HistoryKind::KeepAll);
    assert_eq!(
        qos.destination_order.kind,
        DestinationOrderKind::BySourceTimestamp
    );
    assert!(
        reader
            .get_subscriber()
            .get_qos()
            .presentation
            .ordered_access
    );
    writer
        .wait_for_matched_subscription(1, std::time::Duration::from_secs(15))
        .expect("the consumer matches the supplier");

    // BEST_EFFORT: an event may be lost, so the supplier sends until the
    // consumer has one. Every event from here on has a value above 100.
    let getter = pull.fresh_data().unwrap();
    getter.set_time_out(zerodds_dcps::Duration::from_core(
        std::time::Duration::from_millis(100),
    ));
    let mut got = Vec::new();
    let mut value = 100;
    eventually(
        "an event sent while the consumer listens reaches it",
        || {
            value += 1;
            supplier
                .data()
                .write_one(&track(1, value), HANDLE_NIL)
                .unwrap();
            let (mut datum, mut info) = (Track::default(), ReadInfo::default());
            while getter.get_one(&mut datum, &mut info).unwrap() {
                got.push(datum.clone());
            }
            !got.is_empty()
        },
    );
    // VOLATILE: none of the three events that were sent before the
    // consumer came.
    assert!(got.iter().all(|t| t.value > 100), "{got:?}");
    // The events of the instance in the order they were sent.
    let values: Vec<u32> = got.iter().map(|t| t.value).collect();
    let mut sorted = values.clone();
    sorted.sort_unstable();
    assert_eq!(values, sorted);

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}
