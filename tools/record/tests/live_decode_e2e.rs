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
//! discovery. Run on the Linux test host.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_recorder_decode::type_source::TypeBook;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::{ProbeRequest, encode};

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
/// The domain is leased and not written down. A fixed number here is a
/// live socket on `7400 + 250*n` shared with every other test binary
/// cargo runs beside this one, and what that looks like is not a bind
/// error but an empty table: the recorder attaches, no sample of ours
/// arrives, and the first query returns no rows.
fn lease_domain() -> u32 {
    u32::try_from(zerodds_test_domains::unique_domain()).unwrap_or(47)
}

fn record_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-record"))
}

/// The `cuas::Track` every sample of the test carries.
fn canonical_value() -> DynamicData {
    let int = |value: i128| DynamicData::Integer { kind: None, value };
    DynamicData::Aggregate(vec![
        (String::from("id"), int(42)),
        (
            String::from("name"),
            DynamicData::Text(String::from("bandit")),
        ),
        (
            String::from("xs"),
            DynamicData::Collection(vec![int(7), int(-3), int(1000)]),
        ),
    ])
}

fn make_wire() -> Vec<u8> {
    let book = TypeBook::from_idl(IDL).unwrap();
    let value = canonical_value();
    // The body has to be the encoding the writer *declares*, and the writer
    // declares `data_representation_offer.first()`. That is
    // `DEFAULT_OFFER = [XCDR, XCDR2]` — XCDR**1** first — so
    // `user_payload_encap` stamps `CDR_LE` (0x01) and a reader derives
    // `representation = 0` from it.
    //
    // The comment this replaces said `DEFAULT_OFFER` was `[XCDR2]`, citing a
    // commit from before the list gained its XCDR1 head. Writing an XCDR2
    // body under an XCDR1 header does not fail where the mismatch is: the
    // reader honours the header, reads the XCDR2 delimiter as `id`, and then
    // reads `id` as the length of `name` —
    // `at 4: a length of 42 with 28 bytes left`, with 42 being the `id` that
    // was written.
    let request = ProbeRequest::new(TYPE_NAME, Encoding::PlainCdr1, Endianness::Little);
    encode(book.ast(), &request, &value).unwrap()
}

fn writer_config() -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.to_string(),
        type_name: TYPE_NAME.to_string(),
        ..Default::default()
    }
}

/// The receive time of the one sample [`seed_an_earlier_recording`] writes.
const STALE_RECV_TS_NS: i64 = 1;

/// Leaves at `db` what an earlier `--out-sqlite` to the same path left: the
/// topic's table with one sample in it.
///
/// `-o` and `--out-json` replace their file. `--out-sqlite` appended to it,
/// so the database of a second recording held both, and `zerodds-replay`
/// paced the second one's samples behind the first's by the time between
/// the two recordings — hours, on a host that runs the example twice.
fn seed_an_earlier_recording(db: &std::path::Path) {
    let book = TypeBook::from_idl(IDL).unwrap();
    let ty = book.resolve(TYPE_NAME).unwrap();
    let mut sink =
        zerodds_recorder_decode::sqlite_sink::SqliteSink::open(db.to_str().unwrap()).unwrap();
    sink.ensure_topic(TOPIC, TYPE_NAME, IDL, &ty).unwrap();
    sink.insert_sample(
        TOPIC,
        &ty,
        STALE_RECV_TS_NS,
        "00",
        &make_wire(),
        &canonical_value(),
    )
    .unwrap();
}

#[test]
fn live_record_decode_roundtrip() {
    // Held for the life of this process, so the recorder subprocess can be
    // pointed at the same number.
    let domain = lease_domain();
    let dir = tempfile::tempdir().unwrap();
    let idl_path = dir.path().join("tracks.idl");
    std::fs::write(&idl_path, IDL).unwrap();
    let db_path = dir.path().join("tracks.db");
    let json_path = dir.path().join("tracks.ndjson");
    let zddsrec_path = dir.path().join("cap.zddsrec");
    seed_an_earlier_recording(&db_path);

    // --- Publisher: a real typed writer pumping the canonical sample. ---
    let mut prefix = [0u8; 12];
    prefix[0..4].copy_from_slice(&std::process::id().to_le_bytes());
    prefix[11] = 0xAA;
    let runtime = DcpsRuntime::start(
        i32::try_from(domain).unwrap(),
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
            &domain.to_string(),
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
    let stale: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM t_TrackTopic WHERE recv_ts_ns = ?1",
            [STALE_RECV_TS_NS],
            |r| r.get(0),
        )
        .expect("count stale rows");
    assert_eq!(
        stale, 0,
        "the database holds a row of the recording before this one"
    );
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
