// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-replay` with `zerodds-recorder-decode`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the tool uses:
//! `from_ndjson` and `from_sqlite`, each giving `RecordedSample`s with the
//! topic, the type, the reception time, the body (`raw_cdr`) and the
//! representation identifier of its header. What it relies on, from
//! zerodds-replay(1): a decoded capture carries the original body so that it
//! replays byte for byte, and the samples come in recorded order. The NDJSON
//! line is written out here as the text the record tool writes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::io::Cursor;

use zerodds_recorder_decode::replay_source::{RecordedSample, from_ndjson, from_sqlite};

#[test]
fn a_decoded_line_gives_back_the_body_and_its_identifier() {
    let ndjson = concat!(
        r#"{"topic":"B","type":"m::T","recv_ts_ns":200,"writer":"aa","value":{},"raw_cdr":"0a0b","representation_identifier":"0009"}"#,
        "\n",
        r#"{"topic":"A","type":"m::T","recv_ts_ns":100,"writer":"aa","value":{},"raw_cdr":"ff"}"#,
        "\n"
    );
    let got: Vec<RecordedSample> = from_ndjson(Cursor::new(ndjson)).unwrap();
    assert_eq!(got.len(), 2);
    let b = got.iter().find(|s| s.topic == "B").unwrap();
    assert_eq!(
        (
            b.type_name.as_str(),
            b.recv_ts_ns,
            b.raw_cdr.as_slice(),
            b.representation_identifier
        ),
        ("m::T", 200, &[0x0a, 0x0b][..], Some([0x00, 0x09]))
    );
    let a = got.iter().find(|s| s.topic == "A").unwrap();
    assert_eq!(
        (a.raw_cdr.as_slice(), a.representation_identifier),
        (&[0xff][..], None)
    );
}

#[test]
fn a_file_that_is_no_sqlite_capture_is_an_error() {
    let path = std::env::temp_dir().join(format!("zds-replay-contract-{}.db", std::process::id()));
    std::fs::write(&path, b"not a database").unwrap();
    assert!(from_sqlite(path.to_str().unwrap()).is_err());
    let _ = std::fs::remove_file(&path);
}
