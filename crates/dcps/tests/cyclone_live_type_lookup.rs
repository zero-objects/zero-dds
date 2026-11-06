// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.6.3 against Cyclone DDS: the TypeLookup service and
//! the assignability verdict it feeds, with Cyclone's own type objects on
//! the other side.
//!
//! The `Robot` of `interop/cyclone-xtypes-27`, `@appendable { unsigned
//! long id; unsigned long label; }`, on topic `robot`. ZeroDDS holds it
//! through the dynamic type support built from its IDL; the Cyclone side
//! is a writer or reader of the `cyclonedds` Python binding (in the
//! interop image) whose type hashes differ from it, so the side that has
//! to decide fetches the other's type with a `getTypes` request:
//!
//! - a member appended (`extra`): assignable (§7.2.4.4.8), the samples
//!   arrive, in both directions — ZeroDDS asks Cyclone, and Cyclone asks
//!   ZeroDDS;
//! - `label` declared a string: not assignable, the pair does not
//!   communicate and the reader's topic reports INCONSISTENT_TOPIC
//!   (§7.6.3.4.2);
//! - a `@final` writer, under XCDR1 and under XCDR2: a `@final` type is
//!   not assignable to an `@appendable` one (§7.2.4.4.8), refused in the
//!   same way.
//!
//! The verdict is taken on the object that came over the wire: each test
//! also finds Cyclone's object in the participant's TypeLookup registry
//! (a member ZeroDDS's own type does not have, or `label` as a string).
//! They were the cells of `tests/interop/ci/run_vendor.sh cyclone`, run
//! by the old suite's job `interop-cyclone`; here on a leased domain and
//! without a fixed window, in the job `vendor-live-dcps`.
//!
//! Opt-in like the other Cyclone live tests: `--ignored`,
//! `INTEROP_REQUIRE=1`.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{AbsentTypeConsistency, RuntimeConfig};
use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter, SerializedTypeSupport};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, SubscriberQos, Topic, TopicQos,
};
use zerodds_types::dynamic::{DynamicSample, DynamicType};
use zerodds_types::type_object::flags::StructTypeFlag;
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier};

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

const TOPIC: &str = "robot";
const ROBOT_IDL: &str = "@appendable struct Robot { unsigned long id; unsigned long label; };";
/// How long a verdict may take: discovery, the `getTypes` round trip and
/// the match, measured at 1 to 3 s on the Linux test host.
const VERDICT: Duration = Duration::from_secs(15);

/// A Cyclone writer of `Robot` in the variant `argv[1]` and the
/// representation `argv[2]` on domain `argv[3]`: prints `ready`, then
/// `matched` once it has matched a reader, and writes `id = 1` every
/// 100 ms until `argv[4]` seconds are up.
const WRITER: &str = r#"
import sys, time
from dataclasses import dataclass
from cyclonedds.core import Policy, Qos
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.pub import DataWriter, Publisher
from cyclonedds.topic import Topic

variant, representation = sys.argv[1], sys.argv[2]
domain, seconds = int(sys.argv[3]), float(sys.argv[4])
if variant == "appendable-extended":
    @appendable
    @dataclass
    class Robot(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
        extra: uint32 = 0
elif variant == "appendable-retyped":
    @appendable
    @dataclass
    class Robot(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: str = ""
elif variant == "final":
    @dataclass
    class Robot(IdlStruct, typename="Robot"):
        id: uint32 = 0
        label: uint32 = 0
else:
    sys.exit("unknown variant " + variant)

qos = Qos(Policy.DataRepresentation(
    use_cdrv0_representation=(representation == "xcdr1"),
    use_xcdrv2_representation=(representation == "xcdr2"),
))
dp = DomainParticipant(domain)
w = DataWriter(Publisher(dp), Topic(dp, "robot", Robot), qos=qos)
print("ready", flush=True)
deadline = time.time() + seconds
n = 0
matched = False
while time.time() < deadline:
    if not matched and w.get_matched_subscriptions():
        matched = True
        print("matched", flush=True)
    w.write(Robot(id=1, label=str(n)) if variant == "appendable-retyped" else Robot(id=1, label=n))
    n += 1
    time.sleep(0.1)
"#;

/// A Cyclone reader of `Robot` with the member `extra` appended, on
/// domain `argv[1]`: prints `matched` once it has matched a writer and
/// `received <n> <id>` for every sample it takes, until `argv[2]` seconds
/// are up.
const READER: &str = r#"
import sys, time
from dataclasses import dataclass
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.sub import DataReader, Subscriber
from cyclonedds.topic import Topic

@appendable
@dataclass
class Robot(IdlStruct, typename="Robot"):
    id: uint32 = 0
    label: uint32 = 0
    extra: uint32 = 0

domain, seconds = int(sys.argv[1]), float(sys.argv[2])
dp = DomainParticipant(domain)
r = DataReader(Subscriber(dp), Topic(dp, "robot", Robot))
print("ready", flush=True)
deadline = time.time() + seconds
n = 0
matched = False
while time.time() < deadline:
    if not matched and r.get_matched_publications():
        matched = True
        print("matched", flush=True)
    for sample in r.take(N=20):
        if sample.sample_info.valid_data:
            n += 1
            print(f"received {n} {sample.id} {sample.extra}", flush=True)
    time.sleep(0.05)
"#;

fn support() -> (Arc<SerializedTypeSupport>, DynamicType) {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(ROBOT_IDL, "robot.idl").expect("the IDL"),
    );
    let type_ = DynamicType::find(&ast, "Robot").expect("Robot");
    let complete =
        zerodds_types::from_ast::complete_type_object(&ast, type_.id()).expect("the object");
    let support = SerializedTypeSupport::from_complete_type_objects("Robot", complete, Vec::new())
        .expect("the support");
    let type_ = support.get_type().expect("the type");
    (support, type_)
}

/// A participant on `domain` with the default discovery of the
/// specification, which Cyclone's participant speaks.
fn participant(domain: i32, config: RuntimeConfig) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), config)
        .expect("participant")
}

fn robot_topic(p: &DomainParticipant, support: Arc<SerializedTypeSupport>) -> Topic<DynamicSample> {
    p.create_dynamic_topic(TOPIC, TopicQos::default(), support)
        .expect("topic")
}

/// The structures in the participant's TypeLookup registry, as their
/// flags and their members' types: the minimal form, which a fetched
/// complete object is filed with as well.
fn registry_structs(p: &DomainParticipant) -> Vec<(u16, Vec<TypeIdentifier>)> {
    let runtime = p.runtime().expect("an online participant");
    let Ok(server) = runtime.type_lookup_server.lock() else {
        return Vec::new();
    };
    server
        .registry
        .iter_minimals()
        .filter_map(|(_, object)| match object {
            MinimalTypeObject::Struct(s) => Some((
                s.struct_flags.0,
                s.member_seq
                    .iter()
                    .map(|m| m.common.member_type_id.clone())
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// Whether the registry holds a structure `wanted` picks. ZeroDDS's own
/// `Robot` is `@appendable` with two `unsigned long` members; a structure
/// that is not came from Cyclone in a `getTypes` reply.
fn fetched(p: &DomainParticipant, wanted: fn(u16, &[TypeIdentifier]) -> bool) -> bool {
    registry_structs(p)
        .iter()
        .any(|(flags, members)| wanted(*flags, members))
}

fn uint32(member: &TypeIdentifier) -> bool {
    matches!(member, TypeIdentifier::Primitive(PrimitiveKind::UInt32))
}

/// Cyclone's `Robot` with `extra` appended.
fn appended_member(flags: u16, members: &[TypeIdentifier]) -> bool {
    flags & StructTypeFlag::IS_APPENDABLE != 0 && members.len() == 3 && members.iter().all(uint32)
}

/// Cyclone's `Robot` whose `label` is a string.
fn label_as_a_string(flags: u16, members: &[TypeIdentifier]) -> bool {
    flags & StructTypeFlag::IS_APPENDABLE != 0
        && members.len() == 2
        && uint32(&members[0])
        && !uint32(&members[1])
}

/// Cyclone's `@final` `Robot`.
fn final_robot(flags: u16, members: &[TypeIdentifier]) -> bool {
    flags & StructTypeFlag::IS_FINAL != 0 && members.len() == 2 && members.iter().all(uint32)
}

/// Polls `done` with what the Cyclone program printed so far, every
/// 50 ms, until it holds or `VERDICT` is up; the program must stay up
/// meanwhile.
fn wait(
    cyclone: &mut cross_vendor::VendorChild,
    what: &str,
    state: impl Fn() -> String,
    mut done: impl FnMut(&str) -> bool,
) {
    let deadline = Instant::now() + VERDICT;
    loop {
        let output = cyclone.output().unwrap_or_default();
        if done(&output) {
            return;
        }
        cyclone
            .ensure_running()
            .unwrap_or_else(|e| panic!("the Cyclone program exited early: {e}"));
        assert!(
            Instant::now() < deadline,
            "{what} not within {VERDICT:?}; {}; Cyclone:\n{output}",
            state()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn start_writer(variant: &str, representation: &str, domain: i32) -> cross_vendor::VendorChild {
    cross_vendor::start_cyclone_python(
        &format!("cyclone-robot-writer-{variant}-{representation}"),
        WRITER,
        &[
            variant.to_string(),
            representation.to_string(),
            domain.to_string(),
            "30".to_string(),
        ],
    )
    .expect("start the Cyclone writer")
}

/// The ZeroDDS reader of `Robot` against a Cyclone writer that it must
/// refuse once it holds the writer's type: the topic reports
/// INCONSISTENT_TOPIC, the reader matched nothing and took nothing, and
/// Cyclone's writer did not match it either.
fn refuses(variant: &str, representation: &str, received: fn(u16, &[TypeIdentifier]) -> bool) {
    let lease = zerodds_test_domains::lease_domain();
    let mut cyclone = start_writer(variant, representation, lease.id());
    let (support, _) = support();
    let p = participant(lease.id(), RuntimeConfig::default());
    let topic = robot_topic(&p, support);
    let reader: DynamicDataReader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic, DataReaderQos::default())
        .expect("reader");
    let mut taken = 0;
    let state = || {
        format!(
            "matched {}, INCONSISTENT_TOPIC {:?}, the registry's structures {:?}",
            reader.matched_publication_count(),
            topic.get_inconsistent_topic_status().map(|s| s.total_count),
            registry_structs(&p)
        )
    };
    wait(
        &mut cyclone,
        "the type of the Cyclone writer and INCONSISTENT_TOPIC",
        state,
        |_| {
            taken += reader.take().map(|s| s.len()).unwrap_or(0);
            fetched(&p, received)
                && topic
                    .get_inconsistent_topic_status()
                    .map(|s| s.total_count >= 1)
                    .unwrap_or(false)
        },
    );
    taken += reader.take().map(|s| s.len()).unwrap_or(0);
    let output = cyclone.output().unwrap_or_default();
    assert_eq!(
        reader.matched_publication_count(),
        0,
        "the reader matched the writer it refused"
    );
    assert_eq!(taken, 0, "the reader took samples of a writer it refused");
    assert!(
        !output.lines().any(|l| l == "matched"),
        "Cyclone's writer matched the reader:\n{output}"
    );
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding (vendor-live-dcps)"]
fn xtypes_7_6_3_3_4_a_reader_fetches_the_type_of_a_cyclone_writer_with_a_member_appended_and_takes_its_samples()
 {
    if !cross_vendor::require_or_skip("cyclone_live_type_lookup") {
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let mut cyclone = start_writer("appendable-extended", "xcdr2", lease.id());
    let (support, _) = support();
    let p = participant(lease.id(), RuntimeConfig::default());
    let topic = robot_topic(&p, support);
    let reader: DynamicDataReader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic, DataReaderQos::default())
        .expect("reader");
    let mut ids = Vec::new();
    let state = || {
        format!(
            "matched {}, the registry's structures {:?}",
            reader.matched_publication_count(),
            registry_structs(&p)
        )
    };
    wait(
        &mut cyclone,
        "three samples of the Cyclone writer",
        state,
        |_| {
            for mut sample in reader.take().unwrap_or_default() {
                let id = sample.get_member_id_by_name("id").expect("id");
                ids.push(sample.get_uint32_value(id).expect("the value of id"));
            }
            ids.len() >= 3
        },
    );
    assert!(ids.iter().all(|id| *id == 1), "the ids taken: {ids:?}");
    assert!(
        fetched(&p, appended_member),
        "the reader matched without holding the type of the Cyclone writer: {}",
        state()
    );
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding (vendor-live-dcps)"]
fn xtypes_7_6_3_4_2_a_reader_refuses_a_cyclone_writer_whose_label_is_a_string_after_fetching_its_type()
 {
    if !cross_vendor::require_or_skip("cyclone_live_type_lookup") {
        return;
    }
    refuses("appendable-retyped", "xcdr2", label_as_a_string);
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding (vendor-live-dcps)"]
fn xtypes_7_2_4_4_8_a_final_cyclone_writer_under_xcdr1_is_refused_by_an_appendable_reader() {
    if !cross_vendor::require_or_skip("cyclone_live_type_lookup") {
        return;
    }
    refuses("final", "xcdr1", final_robot);
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding (vendor-live-dcps)"]
fn xtypes_7_2_4_4_8_a_final_cyclone_writer_under_xcdr2_is_refused_by_an_appendable_reader() {
    if !cross_vendor::require_or_skip("cyclone_live_type_lookup") {
        return;
    }
    refuses("final", "xcdr2", final_robot);
}

/// Cyclone asks: its reader has the member `extra` appended and fetches
/// the type of the ZeroDDS writer with `getTypes`. Cyclone DDS 11.0.1
/// announces no TypeConsistencyEnforcement (PID 0x0074) for its reader and
/// coerces; DDS-XTypes 1.3 §7.6.3.4.1 has the writer assume
/// DISALLOW_TYPE_COERCION for an absent policy, so the writer's participant
/// reads the absent policy as ALLOW_TYPE_COERCION
/// (`xtypes.type_consistency_absent = allow`), as the old runner set it for
/// the Cyclone pairs. The writer offers XCDR2: under XCDR1 an `@appendable`
/// sample has no DHEADER, and Cyclone's Python binding fails to read a
/// sample shorter than its type.
#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding (vendor-live-dcps)"]
fn xtypes_7_6_3_3_4_a_cyclone_reader_with_a_member_appended_fetches_the_type_of_a_writer_and_takes_its_samples()
 {
    if !cross_vendor::require_or_skip("cyclone_live_type_lookup") {
        return;
    }
    use zerodds_rtps::publication_data::data_representation as dr;
    let lease = zerodds_test_domains::lease_domain();
    let mut cyclone = cross_vendor::start_cyclone_python(
        "cyclone-robot-reader-extended",
        READER,
        &[lease.id().to_string(), "30".to_string()],
    )
    .expect("start the Cyclone reader");
    let (support, type_) = support();
    let p = participant(
        lease.id(),
        RuntimeConfig {
            absent_type_consistency: AbsentTypeConsistency::Allow,
            ..RuntimeConfig::default()
        },
    );
    let topic = robot_topic(&p, support);
    let writer: DynamicDataWriter = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(
            &topic,
            DataWriterQos {
                data_representation: Some(vec![dr::XCDR2]),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let mut sample = DynamicSample::new(&type_).expect("a sample");
    let id = sample.get_member_id_by_name("id").expect("id");
    sample.set_uint32_value(id, 7).expect("id = 7");
    let state = || format!("matched {}", writer.matched_subscription_count());
    // The reader is VOLATILE: what the writer sends before Cyclone has
    // matched it is, by the specification, not the reader's.
    wait(
        &mut cyclone,
        "the writer and Cyclone's reader matching",
        state,
        |output| writer.matched_subscription_count() > 0 && output.lines().any(|l| l == "matched"),
    );
    let mut written = 0;
    wait(
        &mut cyclone,
        "three samples at Cyclone's reader",
        state,
        |output| {
            if written < 50 {
                writer.write(&sample).expect("write");
                written += 1;
            }
            output
                .lines()
                .filter(|l| l.starts_with("received "))
                .count()
                >= 3
        },
    );
    let output = cyclone.output().unwrap_or_default();
    // `received <n> <id> <extra>`: the id written, the appended member at
    // its default (§7.2.4.4.8).
    let first = output
        .lines()
        .find(|l| l.starts_with("received "))
        .expect("a received line");
    assert!(first.ends_with(" 7 0"), "Cyclone read: {first}\n{output}");
}
