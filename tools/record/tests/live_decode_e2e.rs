// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Live end-to-end of `record --decode` against a real typed writer.
//!
//! A real DCPS writer publishes XCDR2-LE-encoded `cuas::Track` samples on a
//! topic; a separate `zerodds-record --decode` process discovers it
//! (type-following), captures + decodes, and writes typed SQLite + NDJSON. We
//! then assert the decoded rows/values and that the retained `raw_cdr` lets
//! `zerodds-replay` read the NDJSON back.
//!
//! Linux-only: macOS multicast loopback is unreliable for cross-process DDS
//! discovery (see project memory). Run on codepit.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_recorder_decode::type_source::dynamic_type_from_idl;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::dynamic::DynamicDataFactory;
use zerodds_types::dynamic::codec::encode_dynamic;
use zerodds_types::dynamic::data::DynamicValue as DV;
use zerodds_types::dynamic::type_::DynamicType;

const IDL: &str = r#"
    module cuas {
        @appendable struct Track {
            unsigned long id;
            string name;
            sequence<long> xs;
        };
    };
"#;
const TOPIC: &str = "TrackTopic";
const TYPE_NAME: &str = "cuas::Track";
const DOMAIN: u32 = 47;

fn record_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-record"))
}

fn make_wire() -> Vec<u8> {
    let ty = dynamic_type_from_idl(IDL, "Track").unwrap();
    let mut d = DynamicDataFactory::create_data(&ty).unwrap();
    d.set_uint32_value(0, 42).unwrap();
    d.set_string_value(1, "bandit").unwrap();
    let mut xs = Vec::new();
    for v in [7_i32, -3, 1000] {
        let mut e = DynamicDataFactory::create_data(&DynamicType::new_primitive(
            zerodds_types::dynamic::descriptor::TypeKind::Int32,
        ))
        .unwrap();
        e.set_value_raw(0, DV::Int32(v));
        xs.push(e);
    }
    d.set_sequence_value(2, xs).unwrap();
    // XCDR2 little-endian — matches the writer's default encapsulation header.
    // DEFAULT_OFFER is `[XCDR2]` (feat/xcdr2 01df55e9), so `write_user_sample`
    // stamps a CDR2/D_CDR2_LE header; the body bytes must be XCDR2 to match,
    // else the recorder decodes an XCDR1 body as XCDR2 and misreads `id` as a
    // length prefix.
    encode_dynamic(&d, true, false).unwrap()
}

fn writer_config() -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.to_string(),
        type_name: TYPE_NAME.to_string(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

#[test]
fn live_record_decode_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let idl_path = dir.path().join("tracks.idl");
    std::fs::write(&idl_path, IDL).unwrap();
    let db_path = dir.path().join("tracks.db");
    let json_path = dir.path().join("tracks.ndjson");
    let zddsrec_path = dir.path().join("cap.zddsrec");

    // --- Publisher: a real typed writer pumping the canonical sample. ---
    let mut prefix = [0u8; 12];
    prefix[0..4].copy_from_slice(&std::process::id().to_le_bytes());
    prefix[11] = 0xAA;
    let runtime = DcpsRuntime::start(
        i32::try_from(DOMAIN).unwrap(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .expect("publisher runtime");
    let eid = runtime
        .register_user_writer(writer_config())
        .expect("register writer");
    let wire = make_wire();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_w = Arc::clone(&stop);
    let rt_w = Arc::clone(&runtime);
    let wire_w = wire.clone();
    let pump = std::thread::spawn(move || {
        while !stop_w.load(Ordering::Relaxed) {
            let _ = rt_w.write_user_sample(eid, wire_w.clone());
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    // Let the writer get announced before the recorder's settle window.
    std::thread::sleep(Duration::from_millis(800));

    // --- Recorder process: discover, capture, decode → SQLite + NDJSON. ---
    let status = Command::new(record_bin())
        .args([
            "record",
            "-t",
            TOPIC,
            "-d",
            &DOMAIN.to_string(),
            "--duration",
            "5s",
            "-o",
            zddsrec_path.to_str().unwrap(),
            "--decode",
            "--type-file",
            idl_path.to_str().unwrap(),
            "--map",
            &format!("{TOPIC}={TYPE_NAME}"),
            "--out-sqlite",
            db_path.to_str().unwrap(),
            "--out-json",
            json_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn record");
    assert!(status.success(), "record exited non-zero: {status:?}");

    stop.store(true, Ordering::Relaxed);
    pump.join().ok();
    drop(runtime);

    // --- Assert SQLite: typed row + joined sequence child. ---
    let conn = rusqlite::Connection::open(&db_path).expect("open db");
    let (id, name): (i64, String) = conn
        .query_row(
            "SELECT id, name FROM t_TrackTopic ORDER BY sample_id LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("first decoded row");
    assert_eq!(id, 42);
    assert_eq!(name, "bandit");
    let xs_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM t_TrackTopic__xs \
             WHERE sample_id = (SELECT MIN(sample_id) FROM t_TrackTopic)",
            [],
            |r| r.get(0),
        )
        .expect("xs child count");
    assert_eq!(xs_count, 3, "xs sequence joined by sample_id");

    // --- Assert NDJSON: decoded value + raw_cdr present and byte-exact. ---
    let nd = std::fs::read_to_string(&json_path).expect("read ndjson");
    let line = nd
        .lines()
        .find(|l| !l.trim().is_empty())
        .expect("at least one ndjson line");
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["topic"], serde_json::json!(TOPIC));
    assert_eq!(v["type"], serde_json::json!(TYPE_NAME));
    assert_eq!(v["value"]["id"], serde_json::json!(42));
    assert_eq!(v["value"]["name"], serde_json::json!("bandit"));
    assert_eq!(v["value"]["xs"], serde_json::json!([7, -3, 1000]));
    let raw_hex = v["raw_cdr"].as_str().unwrap();
    assert_eq!(
        zerodds_recorder_decode::json_sink::from_hex(raw_hex).unwrap(),
        wire,
        "stored raw_cdr is byte-exact with the published wire body"
    );

    // --- Replay reads the NDJSON back. Verified IN-PROCESS via the library
    // (`from_ndjson`) rather than spawning the sibling `zerodds-replay` binary:
    // that binary lives in the shared CI target cache, which can lag the source
    // across runners (a stale copy predating `.ndjson` format detection read the
    // file as a `.zddsrec` and failed with "bad file magic"). The CLI is a thin
    // wrapper around exactly this function, so the in-process check has the same
    // coverage without the cache-version coupling. ---
    let parsed = zerodds_recorder_decode::replay_source::from_ndjson(std::io::BufReader::new(
        std::fs::File::open(&json_path).expect("open ndjson"),
    ))
    .expect("replay-source parses the recorded NDJSON");
    assert!(
        parsed
            .iter()
            .any(|s| s.topic == TOPIC && s.type_name == TYPE_NAME && s.raw_cdr == wire),
        "replay-source should yield the recorded sample (topic/type/raw_cdr byte-exact); got {} sample(s)",
        parsed.len(),
    );
}
