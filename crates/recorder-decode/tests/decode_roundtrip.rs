// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end of the decoded-output feature, network-free.
//!
//! Simulates the recorder pipeline on a known typed sample:
//!
//! 1. IDL → `Ast` → `DynamicType` (the `--type-file` path);
//! 2. `encode` → CDR octets (the wire payload a real writer would emit, and
//!    what the opaque `.zddsrec` stores);
//! 3. fan out through `JsonSink` + `SqliteSink` (what `record --decode` does);
//! 4. read both back via `replay_source` (what `zerodds-replay` does);
//! 5. assert the retained `raw_cdr` is byte-exact (replay fidelity) AND that
//!    re-decoding it reproduces the original field values (decode fidelity).
//!
//! Run across XCDR1/XCDR2 × little/big-endian so the stored octets and the
//! representation flags round-trip for every wire framing.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::Cursor;

use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_recorder_decode::json_sink::{JsonSink, data_to_json};
use zerodds_recorder_decode::replay_source::{from_ndjson, from_sqlite};
use zerodds_recorder_decode::sqlite_sink::SqliteSink;
use zerodds_recorder_decode::type_source::TypeBook;
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::{ProbeRequest, decode, encode};

const IDL: &str = r"
    module cuas {
        @appendable struct Track {
            unsigned long id;
            string name;
            sequence<long> xs;
        };
    };
";

const TOPIC: &str = "TrackTopic";
const TYPE_NAME: &str = "cuas::Track";

fn int(value: i128) -> DynamicData {
    DynamicData::Integer { kind: None, value }
}

/// The canonical sample: id=42, name="bandit", xs=[7,-3,1000].
fn make_sample() -> DynamicData {
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

fn run_one(encoding: Encoding, endianness: Endianness) {
    let book = TypeBook::from_idl(IDL).expect("idl");
    // The tools take a simple name off the command line.
    let ty = book.resolve("Track").expect("idl→type");
    let request = ProbeRequest::new(TYPE_NAME, encoding, endianness);
    let label = format!("{encoding:?}/{endianness:?}");

    // (2) Encode the wire payload (what a writer emits / the capture stores).
    let wire = encode(book.ast(), &request, &make_sample()).expect("encode");

    // (3) Fan out to both sinks, exactly as record --decode does. recv_ts marks
    // capture order; writer GUID is hex-encoded.
    let recv_ts = 1_700_000_000_000i64;
    let writer_hex = "00112233445566778899aabbccddeeff";

    let mut json_buf: Vec<u8> = Vec::new();
    {
        let mut js = JsonSink::new(&mut json_buf);
        let back: DynamicData = decode(book.ast(), &request, &wire).expect("decode for json");
        js.write_sample(TOPIC, TYPE_NAME, recv_ts, writer_hex, &back, &wire)
            .expect("json write");
        assert_eq!(js.count(), 1);
    }

    let db_path = format!(
        "file:decode_e2e_{}_{}?mode=memory&cache=shared",
        std::process::id(),
        label.replace('/', "_")
    );
    let keep = rusqlite::Connection::open(&db_path).unwrap(); // keep shared DB alive
    {
        let mut sq = SqliteSink::open(&db_path).expect("sqlite open");
        sq.ensure_topic(TOPIC, TYPE_NAME, IDL, &ty).expect("ensure");
        let back: DynamicData = decode(book.ast(), &request, &wire).expect("decode for sqlite");
        let sid = sq
            .insert_sample(TOPIC, &ty, recv_ts, writer_hex, &wire, &back)
            .expect("insert");
        assert_eq!(sid, 1, "first record marker is 1");
        // The child table holds the scalar sequence joined by sample_id.
        let n: i64 = sq
            .conn()
            .query_row(
                &format!("SELECT COUNT(*) FROM t_{TOPIC}__xs WHERE sample_id = 1"),
                [],
                |r| r.get(0),
            )
            .expect("count xs");
        assert_eq!(n, 3, "xs has 3 joined rows");
    }

    // (4) Read both formats back via the replay source.
    let from_json = from_ndjson(Cursor::new(json_buf)).expect("from_ndjson");
    let from_db = from_sqlite(&db_path).expect("from_sqlite");
    drop(keep);

    assert_eq!(from_json.len(), 1);
    assert_eq!(from_db.len(), 1);

    for (src, s) in [("json", &from_json[0]), ("sqlite", &from_db[0])] {
        // (5a) byte-exact replay fidelity.
        assert_eq!(
            s.raw_cdr, wire,
            "{src}: raw_cdr must be byte-exact ({label})"
        );
        assert_eq!(s.topic, TOPIC, "{src}: topic");
        assert_eq!(s.type_name, TYPE_NAME, "{src}: type");
        assert_eq!(s.recv_ts_ns, recv_ts, "{src}: recv_ts");

        // (5b) decode fidelity — the retained octets re-decode to the originals.
        let redecoded: DynamicData = decode(book.ast(), &request, &s.raw_cdr)
            .unwrap_or_else(|e| panic!("{src}: re-decode {e:?}"));
        let j = data_to_json(&redecoded);
        assert_eq!(j["id"], serde_json::json!(42), "{src}: id");
        assert_eq!(j["name"], serde_json::json!("bandit"), "{src}: name");
        assert_eq!(j["xs"], serde_json::json!([7, -3, 1000]), "{src}: xs");
    }
}

/// `Track` is `@appendable`, so its XCDR2 form carries a delimiter and its
/// XCDR1 form does not — the two encodings the recorder meets on a wire.
#[test]
fn roundtrip_xcdr2_le() {
    run_one(Encoding::DelimitedCdr, Endianness::Little);
}

#[test]
fn roundtrip_xcdr2_be() {
    run_one(Encoding::DelimitedCdr, Endianness::Big);
}

#[test]
fn roundtrip_xcdr1_le() {
    run_one(Encoding::PlainCdr1, Endianness::Little);
}

#[test]
fn roundtrip_xcdr1_be() {
    run_one(Encoding::PlainCdr1, Endianness::Big);
}

/// Composite-element collection (`sequence<Inner>`) decodes end-to-end into a
/// SQLite child table with one column per flattened element field, joined by
/// `sample_id` + `idx`.
#[test]
fn composite_sequence_sqlite_child_table() {
    const SRC: &str = r"
        module m {
            @final struct Inner { long a; string s; };
            @final struct Track { unsigned long id; sequence<Inner> items; };
        };
    ";
    let book = TypeBook::from_idl(SRC).expect("idl");
    let ty = book.resolve("m::Track").unwrap();
    let request = ProbeRequest::new("m::Track", Encoding::PlainCdr2, Endianness::Little);

    let element = |a: i128, s: &str| {
        DynamicData::Aggregate(vec![
            (String::from("a"), int(a)),
            (String::from("s"), DynamicData::Text(String::from(s))),
        ])
    };
    let value = DynamicData::Aggregate(vec![
        (String::from("id"), int(9)),
        (
            String::from("items"),
            DynamicData::Collection(vec![element(11, "aa"), element(22, "bb")]),
        ),
    ]);
    let wire = encode(book.ast(), &request, &value).unwrap();
    let back: DynamicData = decode(book.ast(), &request, &wire).unwrap();

    let path = format!(
        "file:decode_e2e_compseq_{}?mode=memory&cache=shared",
        std::process::id()
    );
    let keep = rusqlite::Connection::open(&path).unwrap();
    {
        let mut sq = SqliteSink::open(&path).unwrap();
        sq.ensure_topic("Track", "m::Track", SRC, &ty).unwrap();
        let sid = sq
            .insert_sample("Track", &ty, 1, "aa", &wire, &back)
            .unwrap();
        assert_eq!(sid, 1);
        assert!(sq.skipped_members().is_empty(), "no member was dropped");
    }
    // The child table carries the element's flattened columns (a, s).
    let rows: Vec<(i64, i64, String)> = {
        let mut stmt = keep
            .prepare("SELECT idx, a, s FROM t_Track__items WHERE sample_id = 1 ORDER BY idx")
            .expect("child table exists with a/s columns");
        let mapped = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        mapped.collect::<Result<_, _>>().unwrap()
    };
    drop(keep);
    assert_eq!(
        rows,
        vec![(0, 11, "aa".to_string()), (1, 22, "bb".to_string())]
    );
}

/// Two samples across two topics in SQLite come back globally ts-ordered (the
/// `from_sqlite` cross-topic merge), and the second record marker is 2.
#[test]
fn sqlite_multi_topic_ordering() {
    let book = TypeBook::from_idl(IDL).expect("idl");
    let ty = book.resolve(TYPE_NAME).unwrap();
    let request = ProbeRequest::new(TYPE_NAME, Encoding::DelimitedCdr, Endianness::Little);
    let db_path = format!(
        "file:decode_e2e_multi_{}?mode=memory&cache=shared",
        std::process::id()
    );
    let keep = rusqlite::Connection::open(&db_path).unwrap();
    {
        let mut sq = SqliteSink::open(&db_path).unwrap();
        sq.ensure_topic("A", TYPE_NAME, IDL, &ty).unwrap();
        sq.ensure_topic("B", TYPE_NAME, IDL, &ty).unwrap();
        let wire = encode(book.ast(), &request, &make_sample()).unwrap();
        let back: DynamicData = decode(book.ast(), &request, &wire).unwrap();
        // Insert B first (later ts), then A (earlier ts) → from_sqlite re-sorts.
        sq.insert_sample("B", &ty, 200, "bb", &wire, &back).unwrap();
        sq.insert_sample("A", &ty, 100, "aa", &wire, &back).unwrap();
    }
    let got = from_sqlite(&db_path).unwrap();
    drop(keep);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].topic, "A", "earliest ts first");
    assert_eq!(got[1].topic, "B");
}
