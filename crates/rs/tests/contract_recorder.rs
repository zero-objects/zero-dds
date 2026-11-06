// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-rs` with `zerodds-recorder`
//! (internal/release/rc8-test-suite.md, "Crate tier"). The facade
//! re-exports the crate as `zerodds_rs::recorder`; what an application
//! reaches through it is the recorder's API. Checked here: the re-export
//! carries the session and the reader, a capture written through it reads
//! back, and a reader takes the layout of `docs/specs/zddsrec-1.0.md`
//! (magic, version 3, time base, two counts).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::{Arc, Mutex};

use zerodds_rs::recorder::{
    ParticipantEntry, RecordReader, RecordingSession, SampleKind, SessionOptions, TopicKey,
};

#[derive(Clone, Default)]
struct SharedSink(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for SharedSink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn zddsrec_a_capture_through_the_facade_reads_back() {
    let key = TopicKey {
        topic: "T".into(),
        type_name: "X".into(),
    };
    let sink = SharedSink::default();
    // zddsrec-1.0 "Indices": the recorder knows its participants before the
    // first frame.
    let options = SessionOptions::new(0)
        .with_participant(ParticipantEntry {
            guid: [0; 16],
            name: "facade".into(),
        })
        .with_topic(key.clone());
    let session = RecordingSession::new(sink.clone(), options);
    session
        .record_sample(
            3,
            [0; 16],
            &key,
            SampleKind::Alive,
            Some([0x00, 0x01]),
            vec![4],
        )
        .unwrap();
    session.finish().unwrap();
    let bytes = sink.0.lock().unwrap().clone();
    let mut r = RecordReader::new(&bytes);
    r.parse_header().unwrap();
    assert_eq!(r.next_frame().unwrap().unwrap().payload, vec![4]);

    let mut file = b"ZDDS".to_vec();
    file.extend_from_slice(&3u32.to_le_bytes());
    file.extend_from_slice(&[0; 16]);
    let mut r = RecordReader::new(&file);
    let header = r.parse_header().unwrap();
    assert!(header.topics.is_empty() && header.participants.is_empty());
    assert_eq!(r.version(), 3);
}
