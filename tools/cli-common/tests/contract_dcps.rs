// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-cli-common` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps` (`src/lib.rs`):
//! - `runtime::UserReaderConfig` and `runtime::UserWriterConfig`, built over
//!   `Default` with only `topic_name` and `type_name` set, in
//!   `raw_reader_config`, `raw_reader_config_typed` and
//!   `reliable_raw_writer_config`.
//! - `participant::host_id_bytes`, octets 2..6 of `stable_prefix`.
//! - `runtime::DcpsRuntime::discovered_publication_topics` in
//!   `TypeFollower::poll`.
//!
//! What it relies on and is checked here:
//! - The writer config is RELIABLE and VOLATILE with every other policy at
//!   its default and no TypeObject announced ("RELIABLE and VOLATILE, every
//!   other policy at its default, no TypeObject announced", its own rustdoc).
//!   Oracles: DDS 1.4 §2.2.3, the QoS table (RELIABILITY RELIABLE is the
//!   default for DataWriters; DURABILITY VOLATILE, OWNERSHIP SHARED,
//!   TRANSPORT_PRIORITY 0, PARTITION the empty sequence, LIFESPAN and
//!   DEADLINE infinite, LIVELINESS AUTOMATIC with an infinite lease;
//!   `DURATION_INFINITE` of DDSI-RTPS 2.5 §9.3.2), and the rustdoc of
//!   `UserWriterConfig`'s `Default` (`TypeIdentifier::None`).
//! - The reader config announces the given type name and no TypeIdentifier,
//!   so that matching falls back to the type name, and requests VOLATILE,
//!   the lowest durability, so that it excludes no writer (DDS 1.4 §2.2.3
//!   DURABILITY, RxO "VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT").
//! - `RAW_BYTES_TYPE_NAME` is the type name a ZeroDDS `RawBytes` writer
//!   announces (`zerodds_dcps::RawBytes::TYPE_NAME`, its rustdoc).
//! - The host identifier is the one every participant of the process puts
//!   in octets 2..6 of its prefix (rustdoc of `participant::host_id_bytes`:
//!   cached per process; of the generated participant prefix: "bytes[2..6]:
//!   host id"), so a tool and a participant of its host are same-host peers.
//!
//! `TypeFollower::poll` needs a bound runtime; the signature of
//! `discovered_publication_topics` is checked at compile time. No socket is
//! opened.

use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserWriterConfig};
use zerodds_dcps::{DdsType, RawBytes};
use zerodds_qos::{DurabilityKind, Duration, LivelinessKind, OwnershipKind};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::TypeIdentifier;

#[test]
fn dcps_2_2_3_the_raw_writer_config_is_reliable_and_volatile_with_default_policies() {
    let cfg: UserWriterConfig =
        zerodds_cli_common::reliable_raw_writer_config("bench/topic", "zerodds::RawBytes");
    assert_eq!(cfg.topic_name, "bench/topic");
    assert_eq!(cfg.type_name, "zerodds::RawBytes");
    assert!(cfg.reliable, "RELIABLE, the DataWriter default");
    assert_eq!(cfg.durability, DurabilityKind::Volatile);
    assert_eq!(cfg.ownership, OwnershipKind::Shared);
    assert_eq!(cfg.ownership_strength, 0);
    assert_eq!(cfg.transport_priority, 0);
    assert!(cfg.partition.is_empty(), "the default partition");
    assert_eq!(cfg.deadline.period, Duration::INFINITE);
    assert_eq!(cfg.lifespan.duration, Duration::INFINITE);
    assert_eq!(cfg.liveliness.kind, LivelinessKind::Automatic);
    assert_eq!(cfg.liveliness.lease_duration, Duration::INFINITE);
    assert!(cfg.user_data.is_empty());
    assert_eq!(cfg.type_identifier, TypeIdentifier::None);
    assert_eq!(cfg.data_representation_offer, None);
}

#[test]
fn dcps_2_2_3_the_raw_reader_config_follows_the_type_name_and_excludes_no_writer() {
    let cfg: UserReaderConfig =
        zerodds_cli_common::raw_reader_config_typed("tracks", "cuas::Track");
    assert_eq!(cfg.topic_name, "tracks");
    assert_eq!(cfg.type_name, "cuas::Track");
    assert_eq!(cfg.type_identifier, TypeIdentifier::None);
    assert_eq!(cfg.durability, DurabilityKind::Volatile);
    assert!(cfg.partition.is_empty());
    assert!(cfg.content_filter.is_none());
    assert!(!cfg.relay);

    let raw = zerodds_cli_common::raw_reader_config("tracks");
    assert_eq!(raw.type_name, <RawBytes as DdsType>::TYPE_NAME);
}

#[test]
fn the_raw_type_name_is_the_one_a_raw_bytes_writer_announces() {
    assert_eq!(
        zerodds_cli_common::RAW_BYTES_TYPE_NAME,
        <RawBytes as DdsType>::TYPE_NAME
    );
    assert_eq!(<RawBytes as DdsType>::type_name(), "zerodds::RawBytes");
}

#[test]
fn a_tool_and_a_participant_of_its_host_share_the_host_id() {
    let host = zerodds_dcps::participant::host_id_bytes();
    assert_eq!(zerodds_dcps::participant::host_id_bytes(), host);
    let tool = zerodds_cli_common::stable_prefix(0xFE);
    assert_eq!(tool.host_id(), host);
    // The prefix a participant of this process takes.
    let participant = GuidPrefix::zerodds(host, zerodds_dcps::participant::next_process_unique());
    assert_eq!(participant.0[2..6], host);
    assert!(tool.is_same_host(participant));
}

#[test]
fn the_type_follower_reads_topic_and_type_name_pairs() {
    let _: fn(&DcpsRuntime) -> Vec<(String, String)> = DcpsRuntime::discovered_publication_topics;
}
