// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-replay` with `zerodds-recorder`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the tool uses:
//! `RecordReader` over a capture (`parse_header`, `version`, the frames
//! with their `SampleKind` and representation identifier, the topics added
//! in the stream) and `ZDDSREC_VERSION` in its output. What it relies on,
//! from `docs/specs/zddsrec-1.0.md`: the version is 3; a version 3 frame
//! carries the identifier, a version 2 frame none; a `TopicAdd` extends the
//! topic list. The captures are written out here as octets from the spec.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_recorder::{RecordReader, SampleKind, ZDDSREC_VERSION};

fn string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&u32::try_from(s.len()).unwrap().to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// A header of `version` with no participant and topic `/t` of type `T`.
fn header(version: u32) -> Vec<u8> {
    let mut o = b"ZDDS".to_vec();
    o.extend_from_slice(&version.to_le_bytes());
    o.extend_from_slice(&0i64.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&1u32.to_le_bytes());
    string(&mut o, "T");
    string(&mut o, "/t");
    o
}

#[test]
fn zddsrec_the_frames_of_version_3_and_2_and_a_topic_add() {
    assert_eq!(ZDDSREC_VERSION, 3);
    let mut v3 = header(3);
    v3.push(b'T');
    string(&mut v3, "U");
    string(&mut v3, "/u");
    v3.push(b'F');
    v3.extend_from_slice(&5i64.to_le_bytes());
    v3.extend_from_slice(&0u32.to_le_bytes());
    v3.extend_from_slice(&1u32.to_le_bytes());
    v3.extend_from_slice(&[1, 1, 0x00, 0x0a]);
    v3.extend_from_slice(&1u32.to_le_bytes());
    v3.push(9);
    let mut r = RecordReader::new(&v3);
    r.parse_header().unwrap();
    assert_eq!(r.version(), 3);
    let f = r.next_frame_view().unwrap().unwrap();
    assert_eq!(
        (
            f.topic_idx,
            f.sample_kind,
            f.representation_identifier,
            f.payload
        ),
        (
            1,
            SampleKind::NotAliveDisposed,
            Some([0x00, 0x0a]),
            &[9u8][..]
        )
    );
    assert_eq!(r.added_topics()[0].name, "/u");

    let mut v2 = header(2);
    v2.push(b'F');
    v2.extend_from_slice(&5i64.to_le_bytes());
    v2.extend_from_slice(&[0; 8]);
    v2.push(0);
    v2.extend_from_slice(&0u32.to_le_bytes());
    let mut r = RecordReader::new(&v2);
    r.parse_header().unwrap();
    let f = r.next_frame_view().unwrap().unwrap();
    assert_eq!(
        (f.sample_kind, f.representation_identifier),
        (SampleKind::Alive, None)
    );
}
