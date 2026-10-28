// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-flatdata-1.0 §10.4 live: a Cyclone reader takes the samples of
//! a ZeroDDS writer whose SEDP announcement carries `PID_SHM_LOCATOR`
//! (vendor PID 0x8001). The foreign reader has to skip the vendor PID and
//! match the writer as any other.
//!
//! The reader is a Cyclone reader through the `cyclonedds` Python binding
//! (in the interop CI image) that prints how many samples it took. The type
//! has no key, so the writer is registered NoKey (entity kind 0x03): Cyclone
//! matches a reader only with a writer of the same keyedness
//! (`topickind_qos_match_p_lock`, ddsi_endpoint_match.c).
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

use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

const DOMAIN: u32 = 42;
const TOPIC: &str = "ZeroddsShmLocatorProbe";

/// A reliable KEEP_ALL Cyclone reader of `struct ZeroddsShmLocatorProbe {
/// uint32 seq; }`: prints `matched` once it has matched a writer, then
/// `received <n> missing <first missing seqs>` every 100 ms until its
/// deadline (with Cyclone's default KEEP_LAST 1 it would see only every
/// fifth sample).
const READER: &str = r#"
import sys, time
from dataclasses import dataclass
from cyclonedds.core import Qos, Policy
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.types import uint32
from cyclonedds.sub import DataReader
from cyclonedds.topic import Topic
from cyclonedds.util import duration

@dataclass
class ZeroddsShmLocatorProbe(IdlStruct, typename="ZeroddsShmLocatorProbe"):
    seq: uint32

dp = DomainParticipant(int(sys.argv[1]))
tp = Topic(dp, "ZeroddsShmLocatorProbe", ZeroddsShmLocatorProbe)
rd = DataReader(dp, tp, qos=Qos(Policy.Reliability.Reliable(duration(seconds=1)), Policy.History.KeepAll))
print("ready", flush=True)
deadline = time.time() + float(sys.argv[2])
seen = set()
matched = False
while time.time() < deadline:
    if not matched and rd.get_matched_publications():
        matched = True
        print("matched", flush=True)
    for sample in rd.take(N=256):
        seen.add(sample.seq)
    missing = sorted(set(range(50)) - seen)[:8]
    print(f"received {len(seen)} missing {missing}", flush=True)
    time.sleep(0.1)
"#;

/// The last `received N` the reader printed.
fn samples_received(output: &str) -> u64 {
    output
        .lines()
        .rev()
        .find_map(|l| {
            l.strip_prefix("received ")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires the cyclonedds Python binding"]
fn a_cyclone_reader_takes_the_samples_of_a_writer_announcing_the_shm_locator() {
    if !cross_vendor::require_or_skip("cyclone_live_shm_locator") {
        return;
    }
    let mut cyclone = cross_vendor::start_cyclone_python(
        "cyclone-shm-locator-reader",
        READER,
        &[DOMAIN.to_string(), "20".to_string()],
    )
    .expect("start the Cyclone Python reader");
    let rt = DcpsRuntime::start(
        DOMAIN as i32,
        GuidPrefix::from_bytes([0xAC; 12]),
        RuntimeConfig {
            spdp_period: Duration::from_millis(500),
            ..RuntimeConfig::default()
        },
    )
    .expect("start");
    let eid = rt
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: TOPIC.into(),
                type_name: TOPIC.into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .expect("writer");
    // hostname_hash | uid | slot_count | slot_size | CDR string path: the
    // shape ShmLocator::to_bytes_le writes.
    rt.set_shm_locator(
        eid,
        vec![
            0xEF, 0xBE, 0xAD, 0xDE, 0x64, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x80, 0x00,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x00, b'/', b'a', 0x00, 0x00,
        ],
    );
    // Both sides matched: the reader is VOLATILE, so what the writer sends
    // before Cyclone has matched it is, by the spec, not the reader's.
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_writer_matched_count(eid) == 0
        || !cyclone
            .output()
            .unwrap_or_default()
            .lines()
            .any(|l| l == "matched")
    {
        cyclone
            .ensure_running()
            .expect("the Cyclone reader exited early");
        assert!(
            Instant::now() < deadline,
            "the writer and Cyclone's reader did not match within 10 s ({} Cyclone participants discovered):\n{}",
            rt.discovered_participants()
                .iter()
                .filter(|p| p.sender_vendor == VendorId::CYCLONE)
                .count(),
            cyclone.output().unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(100));
    }
    // One unsigned long, XCDR1 little-endian behind the encapsulation
    // header write_user_sample puts in front.
    for seq in 0u32..50 {
        rt.write_user_sample(eid, seq.to_le_bytes().to_vec())
            .expect("write");
        thread::sleep(Duration::from_millis(20));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received = 0;
    while Instant::now() < deadline {
        cyclone
            .ensure_running()
            .expect("the Cyclone reader exited early");
        received = samples_received(&cyclone.output().unwrap_or_default());
        if received >= 50 {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    eprintln!("the Cyclone reader took {received} samples");
    assert!(
        received >= 50,
        "Cyclone took {received} of 50 samples from the writer with PID 0x8001:\n{}",
        cyclone.output().unwrap_or_default()
    );
    rt.shutdown();
}
