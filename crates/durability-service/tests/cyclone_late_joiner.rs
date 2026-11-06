// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The service between two Cyclone DDS processes: a Cyclone writer of an
//! `@appendable` type publishes five samples under XCDR2 and goes away; a
//! Cyclone reader that joins later gets them from the service, decoded.
//!
//! The oracle is Cyclone DDS 0.10 on both sides, through its Python binding:
//! the writer frames each body as `D_CDR2` (DDS-XTypes 1.3 §7.4.2: an
//! `@appendable` struct under XCDR2 is a DHEADER, then the members), and the
//! reader decodes a body by the header it comes with (DDSI-RTPS 2.5 §10.5).
//! Replayed under `CDR2`, the header of an `@final` type, the DHEADER would
//! be read as the first member (OPEN-ITEMS 60). Both Cyclone processes
//! accept XCDR2 alone, so the replay writer has to announce XCDR2 first
//! (DDS-XTypes 1.3 §7.6.3.1.1); it announced XCDR first before, and the late
//! joiner never matched it. The topic is named with
//! `--topic` semantics (`serve_discovered`): its type name and keyedness come
//! from the Cyclone writer.
//!
//! Live against another vendor, so `#[ignore]`: it needs python3 with the
//! cyclonedds binding, which the interop image has; the weekly job
//! `vendor-live-dispose-services` runs it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::DurabilityStore;
use zerodds_durability_store_sqlite::SqliteStore;

const TOPIC: &str = "CycloneReadings";
const SAMPLES: u32 = 5;

/// The type, the QoS and a participant on `sys.argv[1]`, shared by the two
/// scripts.
const PRELUDE: &str = r#"
import sys, time
from dataclasses import dataclass
from cyclonedds.core import Policy, Qos
from cyclonedds.domain import DomainParticipant
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import appendable
from cyclonedds.idl.types import uint32
from cyclonedds.pub import DataWriter
from cyclonedds.sub import DataReader
from cyclonedds.topic import Topic
from cyclonedds.util import duration

@appendable
@dataclass
class Reading(IdlStruct, typename="Reading"):
    id: uint32 = 0
    text: str = ""

qos = Qos(
    Policy.Reliability.Reliable(duration(seconds=1)),
    Policy.History.KeepAll,
    Policy.Durability.TransientLocal,
    Policy.DataRepresentation(use_cdrv0_representation=False, use_xcdrv2_representation=True),
)
dp = DomainParticipant(int(sys.argv[1]))
tp = Topic(dp, sys.argv[2], Reading)
"#;

/// Writes `SAMPLES` readings once the service's reader matched, then waits
/// for its standard input to close.
const WRITER: &str = r#"
w = DataWriter(dp, tp, qos=qos)
end = time.time() + 15
while w.get_publication_matched_status().current_count < 1:
    if time.time() > end:
        sys.exit("no reader matched")
    time.sleep(0.05)
print("MATCHED", flush=True)
for i in range(int(sys.argv[3])):
    w.write(Reading(id=i, text="reading-%d" % i))
print("WRITTEN", flush=True)
sys.stdin.read()
"#;

/// Prints every reading it takes until it has `sys.argv[3]` or 15 s passed.
const READER: &str = r#"
r = DataReader(dp, tp, qos=qos)
print("READY", flush=True)
got = 0
end = time.time() + 15
while got < int(sys.argv[3]) and time.time() < end:
    for s in r.take(N=100):
        if s.sample_info.valid_data:
            print("SAMPLE %d %s" % (s.id, s.text), flush=True)
            got += 1
    time.sleep(0.02)
print("DONE", flush=True)
"#;

/// `python3 -c <PRELUDE + script> <domain> <topic> <samples>`, its standard
/// output as a channel of lines.
fn python(script: &str, domain: i32) -> (Child, mpsc::Receiver<String>) {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(format!("{PRELUDE}{script}"))
        .args([domain.to_string(), TOPIC.to_string(), SAMPLES.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("python3 with the cyclonedds binding");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    (child, rx)
}

fn wait_line(rx: &mpsc::Receiver<String>, what: &str, seen: &mut Vec<String>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                let found = line == what;
                seen.push(line);
                if found {
                    return;
                }
            }
            Err(_) => panic!("no {what} line from the Cyclone process: {seen:?}"),
        }
    }
}

#[test]
#[ignore = "interop pillar, job vendor-live-dispose-services: live against Eclipse Cyclone DDS, needs python3 with the cyclonedds binding"]
fn xtypes_7_4_2_an_appendable_sample_of_cyclone_reaches_a_cyclone_late_joiner_through_the_service()
{
    let domain = zerodds_test_domains::unique_domain();
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(common::keep_all()).unwrap());
    let service = Arc::new(DurabilityService::start(domain, Arc::clone(&store)).unwrap());
    service.serve_discovered(TOPIC, common::keep_all()).unwrap();

    let (mut writer, lines) = python(WRITER, domain);
    let mut seen = Vec::new();
    wait_line(&lines, "WRITTEN", &mut seen);
    let deadline = Instant::now() + Duration::from_secs(15);
    while store.stats(TOPIC).map(|s| s.samples).unwrap_or(0) < SAMPLES as usize {
        assert!(
            Instant::now() < deadline,
            "the service did not store the samples"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(writer.stdin.take());
    assert!(
        writer.wait().unwrap().success(),
        "the Cyclone writer failed"
    );

    let (mut reader, lines) = python(READER, domain);
    let mut seen = Vec::new();
    wait_line(&lines, "DONE", &mut seen);
    assert!(
        reader.wait().unwrap().success(),
        "the Cyclone reader failed"
    );
    let samples: Vec<&str> = seen
        .iter()
        .filter_map(|l| l.strip_prefix("SAMPLE "))
        .collect();
    let want: Vec<String> = (0..SAMPLES).map(|i| format!("{i} reading-{i}")).collect();
    assert_eq!(samples, want, "every reading, decoded, in order");
    service.shutdown();
}
