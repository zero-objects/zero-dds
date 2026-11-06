// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The decoded output, network-free: a captured body goes through the JSON
//! and the SQLite sink and comes back from both (`zerodds-replay`'s
//! sources) byte for byte, with the representation identifier of its
//! header, and decodes to the values it holds.
//!
//! The bodies are the octets DDS-XTypes 1.3 §7.4 gives for
//! `@appendable struct Track { unsigned long id; string name; sequence<long>
//! xs; }` with id = 42, name = "bandit", xs = [7, -3, 1000], written out by
//! hand: under XCDR2 a DHEADER (§7.4.2), under XCDR1 none (§7.4.1.1: PLAIN_CDR
//! for an appendable type); a string is its length with the NUL, then the
//! characters and the NUL (§7.4.1.1.2), and the sequence's length is aligned
//! to four.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::Cursor;

use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_recorder_decode::RawSample;
use zerodds_recorder_decode::json_sink::{JsonSink, data_to_json};
use zerodds_recorder_decode::replay_source::{from_ndjson, from_sqlite};
use zerodds_recorder_decode::sqlite_sink::SqliteSink;
use zerodds_recorder_decode::type_source::TypeBook;
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::{ProbeRequest, decode};

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

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// (encoding, byte order, representation identifier, the body's octets)
fn cells() -> Vec<(Encoding, Endianness, [u8; 2], Vec<u8>)> {
    vec![
        (
            Encoding::DelimitedCdr,
            Endianness::Little,
            [0x00, 0x09],
            hex("20000000 2a000000 07000000 62616e64697400 00 03000000 07000000 fdffffff e8030000"),
        ),
        (
            Encoding::DelimitedCdr,
            Endianness::Big,
            [0x00, 0x08],
            hex("00000020 0000002a 00000007 62616e64697400 00 00000003 00000007 fffffffd 000003e8"),
        ),
        (
            Encoding::PlainCdr1,
            Endianness::Little,
            [0x00, 0x01],
            hex("2a000000 07000000 62616e64697400 00 03000000 07000000 fdffffff e8030000"),
        ),
        (
            Encoding::PlainCdr1,
            Endianness::Big,
            [0x00, 0x00],
            hex("0000002a 00000007 62616e64697400 00 00000003 00000007 fffffffd 000003e8"),
        ),
    ]
}

fn run_one(encoding: Encoding, endianness: Endianness, identifier: [u8; 2], wire: Vec<u8>) {
    let book = TypeBook::from_idl(IDL).expect("idl");
    // The tools take a simple name off the command line.
    let ty = book.resolve("Track").expect("idl→type");
    let request = ProbeRequest::new(TYPE_NAME, encoding, endianness);
    let label = format!("{encoding:?}/{endianness:?}");

    // (3) Fan out to both sinks, exactly as record --decode does. recv_ts marks
    // capture order; writer GUID is hex-encoded.
    let recv_ts = 1_700_000_000_000i64;
    let writer_hex = "00112233445566778899aabbccddeeff";

    let mut json_buf: Vec<u8> = Vec::new();
    {
        let mut js = JsonSink::new(&mut json_buf);
        let back: DynamicData = decode(book.ast(), &request, &wire).expect("decode for json");
        js.write_sample(
            TOPIC,
            TYPE_NAME,
            recv_ts,
            writer_hex,
            &back,
            RawSample {
                body: &wire,
                representation_identifier: Some(identifier),
            },
        )
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
            .insert_sample(
                TOPIC,
                &ty,
                recv_ts,
                writer_hex,
                RawSample {
                    body: &wire,
                    representation_identifier: Some(identifier),
                },
                &back,
            )
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
        assert_eq!(
            s.representation_identifier,
            Some(identifier),
            "{src}: the identifier of the header ({label})"
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

/// The four cells: `Track` is `@appendable`, so its XCDR2 form carries a
/// delimiter and its XCDR1 form does not, each in both byte orders.
#[test]
fn xtypes_7_4_a_captured_body_comes_back_from_both_sinks_byte_exact_and_decoded() {
    for (encoding, endianness, identifier, wire) in cells() {
        run_one(encoding, endianness, identifier, wire);
    }
}

/// `zerodds-record(1)`, `--out-sqlite`: per-topic SQLite. A sequence of
/// structs (`sequence<Inner>`) gets a child table with one column per field
/// of the element, joined by `sample_id` and `idx`.
#[test]
fn a_sequence_of_structs_gets_a_child_table_with_a_column_per_field() {
    const SRC: &str = r"
        module m {
            @final struct Inner { long a; string s; };
            @final struct Track { unsigned long id; sequence<Inner> items; };
        };
    ";
    let book = TypeBook::from_idl(SRC).expect("idl");
    let ty = book.resolve("m::Track").unwrap();

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
    // The child table is made from the decoded value; the retained body
    // is not read here.
    let wire: Vec<u8> = Vec::new();

    let path = format!(
        "file:decode_e2e_compseq_{}?mode=memory&cache=shared",
        std::process::id()
    );
    let keep = rusqlite::Connection::open(&path).unwrap();
    {
        let mut sq = SqliteSink::open(&path).unwrap();
        sq.ensure_topic("Track", "m::Track", SRC, &ty).unwrap();
        let sid = sq
            .insert_sample(
                "Track",
                &ty,
                1,
                "aa",
                RawSample {
                    body: &wire,
                    representation_identifier: Some([0x00, 0x09]),
                },
                &value,
            )
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

/// `zerodds-replay(1)`: the frames come back in recorded order; from a
/// SQLite capture across its per-topic tables, by reception time.
#[test]
fn a_sqlite_capture_of_two_topics_replays_in_recorded_order() {
    let book = TypeBook::from_idl(IDL).expect("idl");
    let ty = book.resolve(TYPE_NAME).unwrap();
    let (encoding, endianness, identifier, wire) = cells().remove(0);
    let request = ProbeRequest::new(TYPE_NAME, encoding, endianness);
    let db_path = format!(
        "file:decode_e2e_multi_{}?mode=memory&cache=shared",
        std::process::id()
    );
    let keep = rusqlite::Connection::open(&db_path).unwrap();
    {
        let mut sq = SqliteSink::open(&db_path).unwrap();
        sq.ensure_topic("A", TYPE_NAME, IDL, &ty).unwrap();
        sq.ensure_topic("B", TYPE_NAME, IDL, &ty).unwrap();
        let back: DynamicData = decode(book.ast(), &request, &wire).unwrap();
        // Insert B first (later ts), then A (earlier ts) → from_sqlite re-sorts.
        sq.insert_sample(
            "B",
            &ty,
            200,
            "bb",
            RawSample {
                body: &wire,
                representation_identifier: Some(identifier),
            },
            &back,
        )
        .unwrap();
        sq.insert_sample(
            "A",
            &ty,
            100,
            "aa",
            RawSample {
                body: &wire,
                representation_identifier: Some(identifier),
            },
            &back,
        )
        .unwrap();
    }
    let got = from_sqlite(&db_path).unwrap();
    drop(keep);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].topic, "A", "earliest ts first");
    assert_eq!(got[1].topic, "B");
}
