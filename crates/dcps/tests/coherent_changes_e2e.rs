//! DDS 1.4 §2.2.2.4.1.8/.9 + §2.2.3.6 — a coherent set is atomic.
//!
//! The clause under test: with `PRESENTATION.coherent_access = true` and an
//! `access_scope` above INSTANCE, the subscriber presents the samples a
//! publisher wrote between `begin_coherent_changes` and
//! `end_coherent_changes` either **all** or **not at all**. Before the
//! `end_coherent_changes` the reader must see none of them; after it, all of
//! them.
//!
//! The path exercised here is the same-runtime writer→reader route, which is
//! a direct channel handoff: it needs no UDP loopback and therefore no
//! platform gate. The wire form of the marker — `PID_COHERENT_SET`, DDSI-RTPS
//! 2.5 §8.7.6 — is asserted on the octets in
//! `zerodds-rtps`'s `reliable_writer::tests` and `inline_qos::tests`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, PresentationAccessScope, PresentationQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

use common::{isolated_cfg, unique_domain};

/// Presentation policy with `coherent_access` and the given scope.
fn presentation(scope: PresentationAccessScope, coherent: bool) -> PresentationQosPolicy {
    PresentationQosPolicy {
        access_scope: scope,
        coherent_access: coherent,
        ordered_access: false,
    }
}

/// KEEP_ALL: every test in this file writes 2-3 samples to this keyless
/// (single-instance) topic and asserts on all of them released together —
/// the default KEEP_LAST(1) would (correctly) evict all but the newest,
/// since `take()` routes through the same HISTORY-enforcing cache as
/// `take_with_info()`.
fn unbounded_history() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn payload(n: u8) -> RawBytes {
    RawBytes { data: alloc_vec(n) }
}

fn alloc_vec(n: u8) -> Vec<u8> {
    vec![n; 4]
}

/// §2.2.2.4.1.8/.9 + §2.2.3.6: a GROUP-scope, `coherent_access` reader sees
/// nothing of an open coherent set and every sample of it once
/// `end_coherent_changes` closes the set.
#[test]
fn group_coherent_reader_sees_the_set_only_after_end_coherent_changes() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(PresentationAccessScope::Group, true),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(PresentationAccessScope::Group, true),
        ..SubscriberQos::default()
    });

    let topic = participant
        .create_topic::<RawBytes>("CoherentSquare", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, unbounded_history())
        .expect("reader");

    publisher
        .begin_coherent_changes()
        .expect("begin_coherent_changes");
    assert!(publisher.coherent_changes_open());
    for n in 1..=3u8 {
        writer.write(&payload(n)).expect("write");
    }

    // The whole set is in flight, and none of it may be visible. Under GROUP
    // access scope a take is inside begin_access/end_access (DDS 1.4
    // §2.2.2.5.2.8).
    subscriber.begin_access().expect("begin_access");
    let early = reader.take().expect("take before end");
    subscriber.end_access().expect("end_access");
    assert!(
        early.is_empty(),
        "an open coherent set must be invisible, got {} sample(s)",
        early.len()
    );
    assert_eq!(
        reader.coherent_held_len(),
        3,
        "all three members are held back, not dropped"
    );

    publisher
        .end_coherent_changes()
        .expect("end_coherent_changes");
    assert!(!publisher.coherent_changes_open());

    subscriber.begin_access().expect("begin_access");
    let released = reader.take().expect("take after end");
    subscriber.end_access().expect("end_access");
    assert_eq!(
        released.len(),
        3,
        "end_coherent_changes releases the whole set at once"
    );
    assert_eq!(released[0].data, alloc_vec(1));
    assert_eq!(released[1].data, alloc_vec(2));
    assert_eq!(released[2].data, alloc_vec(3));
    assert_eq!(reader.coherent_held_len(), 0);
}

/// §2.2.3.6: two coherent sets in a row are two separate cuts — the first is
/// released by its own `end_coherent_changes` while the second is still open
/// and still invisible.
#[test]
fn two_coherent_sets_are_released_one_at_a_time() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(PresentationAccessScope::Topic, true),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(PresentationAccessScope::Topic, true),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<RawBytes>("CoherentTwoSets", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, unbounded_history())
        .expect("reader");

    publisher.begin_coherent_changes().expect("begin 1");
    writer.write(&payload(1)).expect("write 1");
    writer.write(&payload(2)).expect("write 2");
    publisher.end_coherent_changes().expect("end 1");

    publisher.begin_coherent_changes().expect("begin 2");
    writer.write(&payload(3)).expect("write 3");

    let first = reader.take().expect("take");
    assert_eq!(first.len(), 2, "only the closed set is visible");
    assert_eq!(first[0].data, alloc_vec(1));
    assert_eq!(first[1].data, alloc_vec(2));
    assert_eq!(reader.coherent_held_len(), 1, "the open set is still held");

    publisher.end_coherent_changes().expect("end 2");
    let second = reader.take().expect("take 2");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].data, alloc_vec(3));
}

/// §2.2.2.4.1.9: `end_coherent_changes` without a preceding
/// `begin_coherent_changes` is a precondition violation, not a silent no-op.
#[test]
fn end_coherent_changes_without_begin_is_precondition_not_met() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let publisher = participant.create_publisher(PublisherQos::default());
    let err = publisher
        .end_coherent_changes()
        .expect_err("must not silently succeed");
    assert!(matches!(
        err,
        zerodds_dcps::DdsError::PreconditionNotMet { .. }
    ));
}

/// §2.2.3.6: without `coherent_access` the set is not a unit — the samples
/// stay individually visible while the transaction is open. This is the
/// control that shows the gating above comes from the QoS and not from the
/// writer withholding the samples.
#[test]
fn without_coherent_access_the_samples_stay_visible_during_the_transaction() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(PresentationAccessScope::Group, true),
        ..PublisherQos::default()
    });
    // Subscriber side deliberately WITHOUT coherent_access.
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(PresentationAccessScope::Group, false),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<RawBytes>("CoherentControl", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, unbounded_history())
        .expect("reader");

    publisher.begin_coherent_changes().expect("begin");
    writer.write(&payload(1)).expect("write 1");
    writer.write(&payload(2)).expect("write 2");

    // Under GROUP access scope a take is inside begin_access/end_access
    // (DDS 1.4 §2.2.2.5.2.8).
    subscriber.begin_access().expect("begin_access");
    let seen = reader.take().expect("take");
    subscriber.end_access().expect("end_access");
    assert_eq!(
        seen.len(),
        2,
        "without coherent_access the reader sees the samples as they arrive"
    );
    publisher.end_coherent_changes().expect("end");
    // The end marker carries no user data — it must not surface as a sample.
    subscriber.begin_access().expect("begin_access");
    let after = reader.take().expect("take after end");
    subscriber.end_access().expect("end_access");
    assert!(after.is_empty(), "the ECS marker is not a sample");
    let _ = Duration::from_millis(0);
}

/// §2.2.3.6, INSTANCE scope: "changes to one instance need not be coherent nor
/// ordered with respect to changes to any other instance. In other words,
/// order and coherent changes apply to each instance separately." The scope
/// bounds how far the coherency reaches — it does not switch it off. An
/// INSTANCE-scope subscriber with `coherent_access` therefore still sees the
/// members of a set only once the set is closed; the guarantee simply does not
/// span two instances. The OMG `dds-rtps` `Test_CoherentSets_10` and `_19` run
/// exactly this configuration and count the samples per released set.
#[test]
fn instance_scope_coherent_reader_still_waits_for_end_coherent_changes() {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(PresentationAccessScope::Instance, true),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(PresentationAccessScope::Instance, true),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<RawBytes>("CoherentInstanceScope", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, unbounded_history())
        .expect("reader");

    publisher.begin_coherent_changes().expect("begin");
    for n in 1..=3u8 {
        writer.write(&payload(n)).expect("write");
    }
    let early = reader.take().expect("take before end");
    assert!(
        early.is_empty(),
        "INSTANCE scope bounds the coherency, it does not disable it — got {} sample(s)",
        early.len()
    );
    assert_eq!(reader.coherent_held_len(), 3);

    publisher.end_coherent_changes().expect("end");
    let released = reader.take().expect("take after end");
    assert_eq!(released.len(), 3, "the whole set arrives at once");
    assert_eq!(reader.coherent_held_len(), 0);
}

/// `dcps.presentation_instance_coherence = none`: the reading of DDS 1.4
/// §2.2.3.6 (p. 115) by which begin_coherent_change and end_coherent_change
/// "have no effect on how the subscriber can access the data" with INSTANCE
/// scope. The samples are presented as they arrive, before the set is
/// closed. The default `per_instance` is the test above.
#[test]
fn instance_scope_with_no_coherence_presents_the_samples_as_they_arrive() {
    use zerodds_dcps::runtime::PresentationInstanceCoherence;

    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let mut cfg = isolated_cfg();
    cfg.presentation_instance_coherence = PresentationInstanceCoherence::None;
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(PresentationAccessScope::Instance, true),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(PresentationAccessScope::Instance, true),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<RawBytes>("CoherentInstanceScopeNone", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, unbounded_history())
        .expect("reader");

    publisher.begin_coherent_changes().expect("begin");
    for n in 1..=3u8 {
        writer.write(&payload(n)).expect("write");
    }
    let early = reader.take().expect("take before end");
    assert_eq!(
        early.len(),
        3,
        "with no coherence under INSTANCE the set is not held back"
    );
    assert_eq!(reader.coherent_held_len(), 0);
    publisher.end_coherent_changes().expect("end");
}
