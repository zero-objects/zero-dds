// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-ffi-loader-1.0 §5 and §8.3: each binding of the specification
//! takes, in a process of its own, the samples a Rust publisher wrote in
//! another process.
//!
//! The publisher is this test binary again in the role `publisher`: a
//! participant made through the public Rust API (`DomainParticipantFactory`)
//! with a RELIABLE, KEEP_ALL writer of XCDR2. The subscriber is a program in
//! the binding's language, through the binding's own entry points. Over the C
//! ABI of `zerodds.h`: C (`zerodds_reader_*`), C++ (`zerodds::Reader` of
//! `crates/cpp/include/zerodds/dds.hpp`, §3.4), Python (the ctypes loader
//! `crates/py/python/zerodds/loader.py`, §3.1), TypeScript (the koffi
//! binding `crates/ts-node`, §3.5), C# (`DllImport` in `crates/cs`, §3.3)
//! and Dart (the dart:ffi loader of the Flutter app of the dds-chat
//! tutorial, §3.6). Without it: the Java PSM (`crates/java-omgdds`, §3.2,
//! pure Java with its own RTPS stack) and the WASM build (`crates/ts-wasm`,
//! §3.5: its DCPS client through the daemon `zerodds-ws-bridged`, its
//! WebAssembly codec decoding the bodies). The sources are under
//! `tests/integration/peers/ffi/`. The jobs build what a subscriber needs
//! before the tests and name it: `tests/integration/ffi_peers.sh`
//! `libzerodds`, the C, C++ and C# programs and the Node packages
//! (`ZERODDS_LIB`, `ZERODDS_FFI_PEERS`, job `integration-bindings-ffi`);
//! `tests/integration/flutter_chat.sh` `libzerodds` and the app's packages
//! (`ZERODDS_LIB`, job `integration-flutter`); `tests/integration/java_psm_peer.sh`
//! the Java subscriber against the PSM's classes (`ZERODDS_JAVA_PSM_CLASSPATH`,
//! job `integration-java-psm`); `tests/integration/ts_wasm.sh` the
//! WebAssembly glue and the daemon's build step (`ZERODDS_WS_BRIDGED`, job
//! `integration-ts-wasm`).
//!
//! The oracle is stated before the run and is no ZeroDDS code: the
//! publisher writes `SAMPLES` fixed samples of
//! `@final struct FfiSample { unsigned long seq; string text; }`, whose
//! XCDR2 little-endian body DDS-XTypes 1.3 §7.4.3.5 gives as the members in
//! order — `seq` in four octets, then the string as its length with the
//! terminating NUL in four octets and the characters with the NUL
//! (§7.4.3.4.4, rule (6) of §7.4.3.5.3) — and the subscriber prints every
//! body it takes in hex. RELIABLE and KEEP_ALL on both ends (DDS 1.4
//! §2.2.3.14, §2.2.3.18) deliver each sample once, and a writer's samples
//! arrive in the order written (DDSI-RTPS 2.5 §8.4.2.2, §8.4.12.2): the
//! subscriber's bodies are exactly the `SAMPLES` stated ones, in order. Every
//! `text` is seven characters, so every body is 16 octets and no padding of
//! the serialized payload (DDSI-RTPS 2.5 §10.5) is part of the comparison.
//! The WASM subscriber also prints the members its WebAssembly codec reads
//! from each body; they are held to the stated `seq` and `text`.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::{DdsType, PublisherQos, SubscriberQos, TopicQos};
use zerodds_integration::dds::{participant, reliable_reader, reliable_writer};
use zerodds_integration::{
    EVENT, Line, Peer, Scratch, Stream, daemon_command, event, free_port, lease_domain, role,
    role_command,
};

/// Samples the publisher writes.
const SAMPLES: u32 = 16;
/// What one wait of a scenario may take. Discovery over loopback matches in
/// well under a second.
const STEP: Duration = Duration::from_secs(10);
/// The type name both ends register.
const TYPE_NAME: &str = "integration::FfiSample";

/// `@final struct FfiSample { unsigned long seq; string text; }`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct FfiSample {
    seq: u32,
    text: String,
}

impl FfiSample {
    /// The sample the publisher writes as number `seq`.
    fn numbered(seq: u32) -> Self {
        Self {
            seq,
            text: format!("ffi {seq:03}"),
        }
    }
}

/// The XCDR2 little-endian body of [`FfiSample::numbered`] as DDS-XTypes 1.3
/// §7.4.3.5 lays it out, in hex: `seq`, the string's length with its NUL,
/// the characters, the NUL. Written out here, apart from the publisher's
/// encoder, as the value every subscriber's body is held to.
fn stated_body_hex(seq: u32) -> String {
    let text = format!("ffi {seq:03}");
    let mut body = Vec::new();
    body.extend_from_slice(&seq.to_le_bytes());
    body.extend_from_slice(&(u32::try_from(text.len()).unwrap() + 1).to_le_bytes());
    body.extend_from_slice(text.as_bytes());
    body.push(0);
    assert_eq!(
        body.len(),
        16,
        "seven characters keep every body at 16 octets"
    );
    body.iter().map(|b| format!("{b:02x}")).collect()
}

impl DdsType for FfiSample {
    const TYPE_NAME: &'static str = TYPE_NAME;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        let length = u32::try_from(self.text.len() + 1).map_err(|_| EncodeError::Invalid {
            what: "text longer than a CDR string",
        })?;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(self.text.as_bytes());
        out.push(0);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<u32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or(DecodeError::Invalid {
                    what: "truncated FfiSample",
                })
        };
        let seq = word(0)?;
        let length = word(4)? as usize;
        let text = bytes
            .get(8..8 + length.saturating_sub(1))
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or(DecodeError::Invalid {
                what: "FfiSample text",
            })?;
        Ok(Self {
            seq,
            text: text.to_owned(),
        })
    }
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set for this role"))
}

/// The publisher role: matches the subscriber, writes the `SAMPLES` samples,
/// waits until the subscriber has acknowledged every one (DDS 1.4
/// §2.2.2.4.2.23), and holds its participant until the scenario releases it.
fn publisher_role() {
    let domain: i32 = env("IT_DOMAIN").parse().unwrap();
    let participant = participant(domain);
    let topic = participant
        .create_topic::<FfiSample>(&env("IT_TOPIC"), TopicQos::default())
        .expect("topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<FfiSample>(&topic, reliable_writer())
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, STEP)
        .expect("the writer matches the subscriber of the binding");
    event("matched");
    for seq in 0..SAMPLES {
        writer.write(&FfiSample::numbered(seq)).expect("write");
    }
    writer
        .wait_for_acknowledgments(STEP)
        .expect("the subscriber acknowledges every sample");
    event("acknowledged");
    zerodds_integration::wait_for_release();
}

/// The subscriber role, the other end of the Java publisher: matches it,
/// takes the `SAMPLES` samples with RELIABLE and KEEP_ALL and prints each as
/// `EVENT taken <seq> <text>`, decoded by the layout this file states, then
/// holds its participant until the scenario releases it.
fn subscriber_role() {
    let domain: i32 = env("IT_DOMAIN").parse().unwrap();
    let participant = participant(domain);
    let topic = participant
        .create_topic::<FfiSample>(&env("IT_TOPIC"), TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<FfiSample>(&topic, reliable_reader())
        .expect("reader");
    reader
        .wait_for_matched_publication(1, STEP)
        .expect("the reader matches the Java publisher");
    event("matched");
    let deadline = std::time::Instant::now() + STEP;
    let mut taken = 0;
    while taken < SAMPLES && std::time::Instant::now() < deadline {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if reader.wait_for_data(left).is_err() {
            break;
        }
        for sample in reader.take().expect("take") {
            event(format!("taken {} {}", sample.seq, sample.text));
            taken += 1;
        }
    }
    event(format!("received n={taken}"));
    zerodds_integration::wait_for_release();
}

/// A file or directory the job's build step named, or the failed scenario
/// that says so.
fn built(variable: &str) -> PathBuf {
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "{variable} is not set: the job's build step builds what the subscriber needs \
                 and names it (see the head of this file); this scenario does not skip"
            )
        });
    assert!(
        path.exists(),
        "{variable}={} does not exist",
        path.display()
    );
    path
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// `LD_LIBRARY_PATH` with the directory of `library` in front.
fn library_path(library: &Path) -> OsString {
    let directory = library.parent().expect("the library's directory");
    std::env::var_os("LD_LIBRARY_PATH").map_or_else(
        || directory.as_os_str().to_owned(),
        |rest| {
            let mut joined = directory.as_os_str().to_owned();
            joined.push(":");
            joined.push(rest);
            joined
        },
    )
}

/// The subscriber of one binding: its program, with the domain, the topic,
/// the type name and the number of samples as its arguments (the WASM one
/// the bridge's URL before them), and the library a binding over the C ABI
/// loads.
fn subscriber(language: &str, domain: u32, topic: &str, bridge: Option<&str>) -> Command {
    let sources = repository().join("tests/integration/peers/ffi");
    let mut command = match language {
        "c" | "cpp" => Command::new(built("ZERODDS_FFI_PEERS").join(format!("sub_{language}"))),
        "python" => {
            let mut command = Command::new("python3");
            command
                .arg(sources.join("sub.py"))
                .env("PYTHONPATH", repository().join("crates/py/python"));
            command
        }
        "typescript" => {
            // `tsx` and `koffi` resolve from the binding's own packages,
            // which the build step installed.
            let mut command = Command::new("node");
            command
                .args(["--import", "tsx"])
                .arg(sources.join("sub.mts"))
                .current_dir(repository().join("crates/ts-node"));
            command
        }
        "csharp" => {
            let mut command = Command::new("dotnet");
            command.arg(built("ZERODDS_FFI_PEERS").join("csharp/FfiSub.dll"));
            command
        }
        "dart" => {
            // `package:ffi` of the loader resolves from the app's packages,
            // which flutter_chat.sh fetched.
            let app = repository().join("examples/tutorials/dds-chat/apps/flutter-mobile");
            let packages = app.join(".dart_tool/package_config.json");
            assert!(
                packages.is_file(),
                "{} is missing: tests/integration/flutter_chat.sh runs `flutter pub get` for the app",
                packages.display()
            );
            let mut command = Command::new("dart");
            command
                .arg(format!("--packages={}", packages.display()))
                .arg(sources.join("sub.dart"))
                .current_dir(app);
            command
        }
        "java" => {
            let classpath = std::env::var("ZERODDS_JAVA_PSM_CLASSPATH").unwrap_or_else(|_| {
                panic!(
                    "ZERODDS_JAVA_PSM_CLASSPATH is not set: tests/integration/java_psm_peer.sh \
                     compiles the Java subscriber against the PSM's classes; this scenario does \
                     not skip"
                )
            });
            let mut command = Command::new("java");
            command
                .arg("-Dzerodds.rtps.enable=true")
                .arg("-cp")
                .arg(classpath)
                .arg("FfiSub");
            command
        }
        "wasm" => {
            // `tsx`, `ws` and the WebAssembly glue resolve from the crate's
            // own packages and its `pkg-node/`, which ts_wasm.sh made.
            let crate_dir = repository().join("crates/ts-wasm");
            let glue = crate_dir.join("pkg-node/zerodds_ts_wasm.js");
            assert!(
                glue.is_file(),
                "{} is missing: tests/integration/ts_wasm.sh generates it",
                glue.display()
            );
            let mut command = Command::new("node");
            command
                .args(["--import", "tsx"])
                .arg(sources.join("sub_wasm.mts"))
                .arg(bridge.expect("the WASM subscriber needs the bridge"))
                .current_dir(crate_dir);
            command
        }
        other => panic!("no subscriber for {other}"),
    };
    if matches!(
        language,
        "c" | "cpp" | "python" | "typescript" | "csharp" | "dart"
    ) {
        let library = built("ZERODDS_LIB");
        command
            .env("LD_LIBRARY_PATH", library_path(&library))
            .env("ZERODDS_LIB", &library);
    }
    command.args([
        domain.to_string(),
        topic.to_owned(),
        TYPE_NAME.to_owned(),
        SAMPLES.to_string(),
    ]);
    command
}

/// The daemon `zerodds-ws-bridged` (zerodds-ws-bridge-1.0) on `domain`,
/// bridging `topic` from DDS to WebSocket on its default path (§4.2, §5.1),
/// RELIABLE with a history that holds every sample of the scenario. Returns
/// the daemon, its URL and the address of its metrics (§8.2).
fn bridge(scratch: &Scratch, domain: u32, topic: &str) -> (Peer, String, String) {
    let port = free_port();
    let admin = format!("127.0.0.1:{}", free_port());
    let config = scratch.path().join("ws-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"listen: "127.0.0.1:{port}"
domain: {domain}
log_level: info
auth:
  mode: "none"
topics:
  - name: "{topic}"
    type: "{TYPE_NAME}"
    direction: "out"
    qos:
      reliability: "reliable"
      history:
        depth: {depth}
metrics:
  enabled: true
  listen: "{admin}"
"#,
            depth = 2 * SAMPLES
        ),
    )
    .expect("write the daemon's configuration");
    let mut command = daemon_command("ZERODDS_WS_BRIDGED");
    command.arg("--config").arg(&config);
    let mut daemon = Peer::spawn("ws-bridged", command);
    daemon.wait_for("the line `bound on`", STEP, |l| l.text.contains("bound on"));
    (daemon, format!("ws://127.0.0.1:{port}"), admin)
}

/// The daemon's Prometheus counters (zerodds-ws-bridge-1.0 §8.2), for the
/// failure message: what it took from DDS and what it sent to clients.
fn metrics(admin: &str) -> String {
    use std::io::{Read, Write};
    let Ok(mut stream) = std::net::TcpStream::connect(admin) else {
        return format!("no metrics at {admin}");
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.write_all(
        format!("GET /metrics HTTP/1.1\r\nHost: {admin}\r\nConnection: close\r\n\r\n").as_bytes(),
    );
    let mut body = String::new();
    let _ = stream.read_to_string(&mut body);
    body.lines()
        .filter(|line| !line.starts_with('#') && line.contains("_total"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The scenario: the binding's subscriber and the Rust publisher on a leased
/// domain; the subscriber's bodies against the stated ones.
fn run(test: &str, language: &str) -> Vec<Line> {
    let lease = lease_domain();
    let domain = u32::try_from(lease.id()).expect("a leased domain is not negative");
    let topic = format!("FfiLive_{language}");
    let scratch = Scratch::new(&format!("ffi-{language}"));
    let daemon = (language == "wasm").then(|| bridge(&scratch, domain, &topic));
    let mut sub = Peer::spawn(
        &format!("{language} subscriber"),
        subscriber(
            language,
            domain,
            &topic,
            daemon.as_ref().map(|(_, url, _)| url.as_str()),
        ),
    );
    // Through the bridge the publisher matches the daemon's DataReader, which
    // is there from the daemon's start, and the daemon forwards to the
    // connections open at the time a sample arrives (VOLATILE): the
    // publisher starts once the WASM subscriber's connection is open. A
    // subscriber over DDS matches the publisher itself, so both start
    // together.
    if daemon.is_some() {
        sub.wait_for_event("matched", STEP);
    }
    let mut publisher = {
        let mut command = role_command(test, "publisher");
        command
            .env("IT_DOMAIN", domain.to_string())
            .env("IT_TOPIC", &topic);
        Peer::spawn("rust publisher", command)
    };
    // The binding's side first: a failure to match names its output.
    if daemon.is_none() {
        sub.wait_for_event("matched", STEP);
    }
    publisher.wait_for_event("matched", STEP);
    publisher.wait_for_event("acknowledged", STEP);
    sub.wait_for_event("received", STEP);
    // The publisher has every sample acknowledged; what the subscriber takes
    // after this is a sample delivered twice.
    sub.close_stdin();
    let (status, lines) = sub.wait_exit_lines(STEP);
    let _ = publisher.release(STEP);
    let daemon_transcript = daemon.map_or_else(String::new, |(daemon, _, admin)| {
        let counters = metrics(&admin);
        let (_, lines) = daemon.stop_lines(STEP);
        let text: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        format!("ws-bridged:\n{}\n{counters}", text.join("\n"))
    });
    let mut transcript: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
    transcript.push(&daemon_transcript);
    assert!(
        status.success(),
        "{language}: the subscriber exited with {status}\n{}",
        transcript.join("\n")
    );
    let taken: Vec<String> = lines
        .iter()
        .filter(|line| line.stream == Stream::Out)
        .filter_map(|line| line.text.strip_prefix(&format!("{EVENT}sample ")))
        .map(str::to_owned)
        .collect();
    let stated: Vec<String> = (0..SAMPLES).map(stated_body_hex).collect();
    assert_eq!(
        taken,
        stated,
        "{language}: RELIABLE and KEEP_ALL deliver every sample once and in the order written \
         (DDS 1.4 §2.2.3.14, §2.2.3.18; DDSI-RTPS 2.5 §8.4.2.2), each body as DDS-XTypes 1.3 \
         §7.4.3.5 lays it out\n{}",
        transcript.join("\n")
    );
    if language == "wasm" {
        let decoded: Vec<String> = lines
            .iter()
            .filter(|line| line.stream == Stream::Out)
            .filter_map(|line| line.text.strip_prefix(&format!("{EVENT}decoded ")))
            .map(str::to_owned)
            .collect();
        let stated: Vec<String> = (0..SAMPLES)
            .map(|seq| {
                let sample = FfiSample::numbered(seq);
                format!("{} {} rest=0", sample.seq, sample.text)
            })
            .collect();
        assert_eq!(
            decoded,
            stated,
            "wasm: the WebAssembly codec reads `seq` and `text` from each body and nothing after \
             them\n{}",
            transcript.join("\n")
        );
    }
    drop(scratch);
    lines
}

/// The lines of `lines` on standard output that follow `EVENT <prefix>`.
fn events(lines: &[Line], prefix: &str) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.stream == Stream::Out)
        .filter_map(|line| line.text.strip_prefix(&format!("{EVENT}{prefix}")))
        .map(str::to_owned)
        .collect()
}

/// DDS 1.4 §2.2.4.1 as a Java endpoint reports a match with an endpoint of
/// the Rust process and its end: the listener told of the match with both
/// counts and both changes at 1, of its end with `current_count` 0 and
/// `current_count_change` -1, `total_count` staying 1; the status read after
/// each says `total_count` 1 and `current_count` 1, then 0.
fn assert_matched_status(side: &str, lines: &[Line]) {
    let transcript: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(
        events(lines, "listener "),
        [
            "total=1 total_change=1 current=1 current_change=1",
            "total=1 total_change=0 current=0 current_change=-1",
        ],
        "{side}: the listener is told of the match with the Rust endpoint and of its end \
         (DDS 1.4 §2.2.4.1)\n{}",
        transcript.join("\n")
    );
    assert_eq!(
        events(lines, "status "),
        ["matched total=1 current=1", "unmatched total=1 current=0"],
        "{side}: the status counts the Rust endpoint while it matches (DDS 1.4 §2.2.4.1)\n{}",
        transcript.join("\n")
    );
}

/// The Java publisher (`FfiPub`) and the Rust subscriber on a leased domain:
/// the subscriber's samples against the stated ones, the publisher's
/// PUBLICATION_MATCHED against DDS 1.4 §2.2.4.1.
fn run_java_publisher(test: &str) {
    let lease = lease_domain();
    let domain = u32::try_from(lease.id()).expect("a leased domain is not negative");
    let topic = "FfiLive_java_publisher";
    let mut reader = {
        let mut command = role_command(test, "subscriber");
        command
            .env("IT_DOMAIN", domain.to_string())
            .env("IT_TOPIC", topic);
        Peer::spawn("rust subscriber", command)
    };
    let classpath = std::env::var("ZERODDS_JAVA_PSM_CLASSPATH").unwrap_or_else(|_| {
        panic!(
            "ZERODDS_JAVA_PSM_CLASSPATH is not set: tests/integration/java_psm_peer.sh compiles \
             the Java publisher against the PSM's classes; this scenario does not skip"
        )
    });
    let mut writer = {
        let mut command = Command::new("java");
        command
            .arg("-Dzerodds.rtps.enable=true")
            .arg("-cp")
            .arg(classpath)
            .arg("FfiPub")
            .args([
                domain.to_string(),
                topic.to_owned(),
                TYPE_NAME.to_owned(),
                SAMPLES.to_string(),
            ]);
        Peer::spawn("java publisher", command)
    };
    writer.wait_for_event("matched", STEP);
    reader.wait_for_event("matched", STEP);
    writer.wait_for_event("acknowledged", STEP);
    reader.wait_for_event("received", STEP);
    writer.close_stdin();
    let (status, lines) = writer.wait_exit_lines(STEP);
    let transcript: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
    assert!(
        status.success(),
        "java: the publisher exited with {status}\n{}",
        transcript.join("\n")
    );
    let taken = events(reader.lines(), "taken ");
    let _ = reader.release(STEP);
    let stated: Vec<String> = (0..SAMPLES)
        .map(|seq| {
            let sample = FfiSample::numbered(seq);
            format!("{} {}", sample.seq, sample.text)
        })
        .collect();
    assert_eq!(
        taken, stated,
        "java: the Rust reader takes every sample of the Java writer once and in order"
    );
    assert_matched_status("java publisher", &lines);
}

fn dispatch(test: &str, language: &str) -> Vec<Line> {
    match role().as_deref() {
        Some("publisher") => {
            publisher_role();
            Vec::new()
        }
        Some(other) => panic!("unknown role {other}"),
        None => run(test, language),
    }
}

#[test]
fn ffiloader_5_a_c_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_c_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "c",
    );
}

#[test]
fn ffiloader_5_a_cpp_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_cpp_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "cpp",
    );
}

#[test]
fn ffiloader_5_a_python_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_python_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "python",
    );
}

#[test]
fn ffiloader_5_a_typescript_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_typescript_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "typescript",
    );
}

#[test]
fn ffiloader_5_a_csharp_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_csharp_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "csharp",
    );
}

#[test]
fn ffiloader_5_a_dart_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_dart_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "dart",
    );
}

#[test]
fn ffiloader_5_a_java_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_java_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "java",
    );
}

#[test]
fn ffiloader_5_a_wasm_subscriber_takes_the_samples_of_a_rust_publisher_in_order() {
    dispatch(
        "ffiloader_5_a_wasm_subscriber_takes_the_samples_of_a_rust_publisher_in_order",
        "wasm",
    );
}

#[test]
fn dcps_2_2_4_1_subscription_matched_counts_a_writer_of_another_process() {
    let lines = dispatch(
        "dcps_2_2_4_1_subscription_matched_counts_a_writer_of_another_process",
        "java",
    );
    if role().is_none() {
        assert_matched_status("java subscriber", &lines);
    }
}

#[test]
fn dcps_2_2_4_1_publication_matched_counts_a_reader_of_another_process() {
    let test = "dcps_2_2_4_1_publication_matched_counts_a_reader_of_another_process";
    match role().as_deref() {
        Some("subscriber") => subscriber_role(),
        Some(other) => panic!("unknown role {other}"),
        None => run_java_publisher(test),
    }
}
