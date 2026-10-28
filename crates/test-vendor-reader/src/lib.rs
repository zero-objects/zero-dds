// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-test-vendor-reader`. Safety classification: **TOOLING**.
//!
//! Drives a DDS reader of another vendor from a test and reads back the
//! instance state it saw per instance. See the README for the protocol.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead as _, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a reader listens at most if the test never tells it to stop.
pub const READER_SECONDS: u32 = 25;

/// The environment variable that names a reader program (a command line;
/// the protocol arguments are appended).
pub const READER_ENV: &str = "ZERODDS_DISPOSE_READER";

/// The Cyclone DDS reader, through Cyclone's Python binding. It speaks the
/// protocol of this crate's README: RELIABLE, KEEP_ALL, the last instance
/// state per instance.
pub const CYCLONE_READER_PY: &str = r#"
import os, sys, time
from dataclasses import dataclass
from cyclonedds.domain import DomainParticipant
from cyclonedds.topic import Topic
from cyclonedds.sub import DataReader
from cyclonedds.core import Qos, Policy, InstanceState
from cyclonedds.idl import IdlStruct
from cyclonedds.idl.annotations import keylist, final
from cyclonedds.idl.types import int32
from cyclonedds.util import duration

@final
@keylist(["id"])
@dataclass
class KeyedPointA(IdlStruct, typename="KeyedPointA"):
    id: int32 = 0
    value: int32 = 0

NAMES = {
    InstanceState.Alive: "ALIVE",
    InstanceState.NotAliveDisposed: "NOT_ALIVE_DISPOSED",
    InstanceState.NotAliveNoWriters: "NOT_ALIVE_NO_WRITERS",
}
dp = DomainParticipant(int(sys.argv[1]))
tp = Topic(dp, sys.argv[2], KeyedPointA)
qos = Qos(Policy.Reliability.Reliable(duration(seconds=1)), Policy.History.KeepAll)
rd = DataReader(dp, tp, qos=qos)
print("READY", flush=True)
ids, states = {}, {}
matched = snapped = False
end = time.time() + float(sys.argv[3])
while time.time() < end and not os.path.exists(sys.argv[5]):
    if not matched and rd.get_subscription_matched_status().current_count >= 1:
        print("MATCHED", flush=True)
        matched = True
    for s in rd.take(N=100):
        info = s.sample_info
        if info.valid_data:
            if info.instance_handle not in ids:
                print(f"SEEN id={s.id}", flush=True)
            ids[info.instance_handle] = s.id
        states[info.instance_handle] = NAMES.get(info.instance_state, str(info.instance_state))
    if not snapped and os.path.exists(sys.argv[4]):
        for handle, state in states.items():
            print(f"SNAP id={ids.get(handle, '?')} state={state}", flush=True)
        print("SNAPDONE", flush=True)
        snapped = True
    time.sleep(0.02)
for handle, state in states.items():
    print(f"FINAL id={ids.get(handle, '?')} state={state}", flush=True)
"#;

/// The IDL of the type every reader here reads.
pub const KEYED_POINT_A_IDL: &str = "@final struct KeyedPointA { @key long id; long value; };";

/// The COMPLETE `TypeObject` of [`KEYED_POINT_A_IDL`], serialized — what a
/// ZeroDDS writer registers with and what a bridge's `type_file` holds.
#[must_use]
pub fn keyed_point_a_type_object() -> Vec<u8> {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(KEYED_POINT_A_IDL, "keyed-point-a.idl")
            .expect("the KeyedPointA model"),
    );
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "KeyedPointA")
        .expect("KeyedPointA in the model");
    zerodds_types::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, type_.id())
            .expect("the complete TypeObject"),
    )
    .to_bytes_le()
    .expect("encode the TypeObject")
}

/// `@final` PLAIN_CDR body of `{ id, value }`, little-endian.
#[must_use]
pub fn keyed_point_a(id: i32, value: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
    out
}

/// The KeyHash of instance `id`: its big-endian octets, padded to 16
/// (DDSI-RTPS 2.5 §9.6.4.8, the key fits in 16 octets).
#[must_use]
pub fn keyed_point_a_key_hash(id: i32) -> [u8; 16] {
    let mut h = [0u8; 16];
    h[..4].copy_from_slice(&id.to_be_bytes());
    h
}

/// The highest domain every reader here takes. OpenSplice 6.9 refuses
/// `231` ("config: root/Domain/Id/#text: 231: out of range", measured) and
/// takes `230`, one below `zerodds_test_domains::MAX_DOMAIN`.
pub const HIGHEST_DOMAIN: i32 = 230;

/// A domain lease every reader here accepts.
#[must_use]
pub fn lease_domain() -> zerodds_test_domains::DomainLease {
    zerodds_test_domains::lease_domain_at_most(HIGHEST_DOMAIN)
}

/// A reader program as a command and a label for its output lines: the
/// Cyclone reader when `which` is `cyclone`, otherwise the one
/// [`READER_ENV`] names.
///
/// # Panics
/// `which` is not `cyclone` and [`READER_ENV`] is unset — a missing reader
/// fails the test rather than skipping it.
#[must_use]
pub fn reader_command(which: &str) -> (Command, String) {
    if which == "cyclone" {
        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
        let script = std::env::temp_dir().join(format!(
            "zerodds-vendor-reader-cyclone-{}-{}.py",
            std::process::id(),
            unique()
        ));
        std::fs::write(&script, CYCLONE_READER_PY).expect("write the Cyclone reader");
        let mut command = Command::new(python);
        command.arg(script);
        return (command, "cyclone".to_string());
    }
    let line = std::env::var(READER_ENV).unwrap_or_else(|_| {
        panic!("{READER_ENV} names the reader program; tools/dispose-reader/build.sh builds one")
    });
    let mut parts = line.split_whitespace();
    let program = parts.next().expect("a program in the reader command line");
    let label = std::path::Path::new(program)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("foreign")
        .to_string();
    let mut command = Command::new(program);
    command.args(parts);
    (command, label)
}

fn unique() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A running reader of another vendor.
pub struct VendorReader {
    child: Child,
    lines: Receiver<String>,
    label: String,
    dir: PathBuf,
    seen: BTreeSet<i32>,
}

impl VendorReader {
    /// Starts `command` with the protocol arguments on `domain` and
    /// `topic`, and waits for `READY`.
    ///
    /// # Panics
    /// The reader does not start or does not print `READY`; its stderr is
    /// in the message.
    #[must_use]
    pub fn start(mut command: Command, label: &str, domain: i32, topic: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-vendor-reader-{label}-{}-{}",
            std::process::id(),
            unique()
        ));
        std::fs::create_dir_all(&dir).expect("the reader's work directory");
        let mut child = command
            .arg(domain.to_string())
            .arg(topic)
            .arg(READER_SECONDS.to_string())
            .arg(dir.join("snapshot"))
            .arg(dir.join("stop"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("the {label} reader did not start: {e}"));
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        match lines.recv_timeout(Duration::from_secs(u64::from(READER_SECONDS))) {
            Ok(line) if line == "READY" => {}
            other => {
                let status = child.wait().expect("wait");
                let mut err = String::new();
                std::io::Read::read_to_string(&mut child.stderr.take().expect("stderr"), &mut err)
                    .ok();
                panic!("the {label} reader did not come up ({other:?}, {status}); stderr:\n{err}");
            }
        }
        Self {
            child,
            lines,
            label: label.to_string(),
            dir,
            seen: BTreeSet::new(),
        }
    }

    /// The next line within `timeout`: `Ok(None)` when none came, `Err`
    /// when the reader ended.
    fn next_line(&mut self, timeout: Duration) -> Result<Option<String>, ()> {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => {
                println!("{}| {line}", self.label);
                Ok(Some(line))
            }
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(()),
        }
    }

    /// Waits until the reader reports `MATCHED`.
    ///
    /// # Panics
    /// The reader ends without matching.
    pub fn wait_matched(&mut self) {
        loop {
            match self.next_line(Duration::from_secs(1)) {
                Ok(Some(line)) if line == "MATCHED" => return,
                Ok(_) => {}
                Err(()) => panic!("the {} reader ended without matching a writer", self.label),
            }
        }
    }

    /// Whether the reader reports a valid sample of each of `ids` within
    /// `timeout` (`SEEN id=<id>`, once per instance). Ids it reported
    /// before this call count.
    ///
    /// # Panics
    /// The reader ends.
    pub fn wait_seen(&mut self, ids: &[i32], timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if ids.iter().all(|id| self.seen.contains(id)) {
                return true;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match self.next_line(left) {
                Ok(Some(line)) => {
                    if let Some(id) = line.strip_prefix("SEEN id=").and_then(|v| v.parse().ok()) {
                        self.seen.insert(id);
                    }
                }
                Ok(None) => {}
                Err(()) => panic!("the {} reader ended while waiting for samples", self.label),
            }
        }
    }

    /// The last state the reader saw per instance, by instance id, now.
    ///
    /// # Panics
    /// The reader ends before `SNAPDONE`.
    pub fn snapshot(&mut self) -> BTreeMap<String, String> {
        std::fs::write(self.dir.join("snapshot"), b"").expect("ask for the snapshot");
        let mut states = BTreeMap::new();
        loop {
            match self.next_line(Duration::from_secs(1)) {
                Ok(Some(line)) if line == "SNAPDONE" => return states,
                Ok(Some(line)) => {
                    if let Some(rest) = line.strip_prefix("SNAP id=") {
                        let (id, state) = rest.split_once(" state=").expect("SNAP line");
                        states.insert(id.to_string(), state.to_string());
                    }
                }
                Ok(None) => {}
                Err(()) => panic!("the {} reader ended before SNAPDONE", self.label),
            }
        }
    }

    /// Stops the reader and returns its `FINAL` states.
    ///
    /// # Panics
    /// The reader exits with a failure.
    #[must_use]
    pub fn finish(mut self) -> BTreeMap<String, String> {
        std::fs::write(self.dir.join("stop"), b"").expect("tell the reader to stop");
        let mut states = BTreeMap::new();
        while let Ok(line) = self.next_line(Duration::from_secs(1)) {
            let Some(line) = line else { continue };
            if let Some(rest) = line.strip_prefix("FINAL id=") {
                let (id, state) = rest.split_once(" state=").expect("FINAL line");
                states.insert(id.to_string(), state.to_string());
            }
        }
        let status = self.child.wait().expect("wait");
        let _ = std::fs::remove_dir_all(&self.dir);
        assert!(
            status.success(),
            "the {} reader exited with {status}",
            self.label
        );
        states
    }

    /// The label its output lines carry.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}
