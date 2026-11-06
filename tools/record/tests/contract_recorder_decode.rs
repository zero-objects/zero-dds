// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-record` with `zerodds-recorder-decode`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the tool uses
//! for `--decode`: `TypeBook` for `--type-file`, `parse_topic_type_map` for
//! `--map`, `encoding_of` for the encoding a sample is in, `JsonSink` for
//! `--out-json` with `to_hex` for the writer GUID, `SqliteSink` for
//! `--out-sqlite`, each fed a `RawSample` (body and representation
//! identifier). What it relies on, from zerodds-record(1) and
//! zerodds-replay(1): an NDJSON line per sample with the topic, the type,
//! the value and the body in `raw_cdr`; a SQLite table per topic that keeps
//! the body; `--map TOPIC=Type` split at the first `=`; and DDS-XTypes 1.3
//! §7.4.2 for the encoding of an appendable struct under XCDR2.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_ast_layout::key::Encoding;
use zerodds_recorder_decode::RawSample;
use zerodds_recorder_decode::json_sink::{JsonSink, to_hex};
use zerodds_recorder_decode::sqlite_sink::SqliteSink;
use zerodds_recorder_decode::type_source::{TypeBook, parse_topic_type_map};
use zerodds_recorder_decode::wire::encoding_of;
use zerodds_types::dynamic::DynamicData;

const IDL: &str = "module m { @appendable struct T { long a; }; };";

#[test]
fn a_decoded_sample_is_an_ndjson_line_and_a_row_that_keep_the_body() {
    let book = TypeBook::from_idl(IDL).unwrap();
    let ty = book.resolve("m::T").unwrap();
    assert_eq!(encoding_of(&ty, true), Encoding::DelimitedCdr);
    assert_eq!(
        parse_topic_type_map(&[String::from("Topic=m::T=x")]).unwrap(),
        vec![(String::from("Topic"), String::from("m::T=x"))]
    );
    // DELIMITED_CDR little endian of { a = 5 }: DHEADER 4, then the long.
    let body = [4, 0, 0, 0, 5, 0, 0, 0];
    let raw = RawSample {
        body: &body,
        representation_identifier: Some([0x00, 0x09]),
    };
    let value = DynamicData::Aggregate(vec![(
        String::from("a"),
        DynamicData::Integer {
            kind: None,
            value: 5,
        },
    )]);
    let writer = to_hex(&[0xaa, 0x01]);
    assert_eq!(writer, "aa01");

    let mut out = Vec::new();
    {
        let mut json = JsonSink::new(&mut out);
        json.write_sample("Topic", "m::T", 7, &writer, &value, raw)
            .unwrap();
        assert_eq!(json.count(), 1);
    }
    let line = String::from_utf8(out).unwrap();
    for part in [
        r#""topic":"Topic""#,
        r#""type":"m::T""#,
        r#""value":{"a":5}"#,
        r#""raw_cdr":"0400000005000000""#,
    ] {
        assert!(line.contains(part), "{part} in {line}");
    }

    let mut sqlite = SqliteSink::open_in_memory().unwrap();
    sqlite.ensure_topic("Topic", "m::T", IDL, &ty).unwrap();
    let id = sqlite
        .insert_sample("Topic", &ty, 7, &writer, raw, &value)
        .unwrap();
    let (a, kept): (i64, Vec<u8>) = sqlite
        .conn()
        .query_row(
            "SELECT a, raw_cdr FROM t_Topic WHERE sample_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((a, kept), (5, body.to_vec()));
}
