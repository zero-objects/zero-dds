// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-recorder-bridge` with `zerodds-recorder`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the bridge
//! uses: a `RecordingSession` with the participants and topics it was given
//! (`SessionOptions`, `ParticipantEntry`, `TopicKey`), `record_sample` with
//! the `SampleKind` and representation identifier of each sample, and the
//! counters of `stats`. What it relies on, from `docs/specs/zddsrec-1.0.md`
//! and the documentation of `record_sample`: each sample of a known topic is
//! a frame with its kind and identifier, a sample of a topic the session
//! does not know is dropped and counted, `bytes_total` is the size written.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::{Arc, Mutex};

use zerodds_recorder::{
    ParticipantEntry, RecordReader, RecordingSession, SampleKind, SessionOptions, TopicKey,
};

/// A sink the test reads while the session still owns its writer.
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

fn topic(name: &str, type_name: &str) -> TopicKey {
    TopicKey {
        topic: name.into(),
        type_name: type_name.into(),
    }
}

#[test]
fn zddsrec_a_known_topic_is_framed_and_an_unknown_one_counted() {
    let sink = SharedSink::default();
    let opts = SessionOptions::new(0)
        .with_participant(ParticipantEntry {
            guid: [1; 16],
            name: "bridge".into(),
        })
        .with_topic(topic("Known", "K"));
    let session = RecordingSession::new(sink.clone(), opts);
    session
        .record_sample(
            10,
            [1; 16],
            &topic("Known", "K"),
            SampleKind::NotAliveUnregistered,
            Some([0x00, 0x03]),
            vec![2],
        )
        .unwrap();
    session
        .record_sample(
            11,
            [1; 16],
            &topic("Other", "O"),
            SampleKind::Alive,
            None,
            vec![3],
        )
        .unwrap();
    let stats = session.stats();
    let bytes = sink.0.lock().unwrap().clone();
    assert_eq!((stats.samples_total, stats.samples_dropped), (1, 1));
    assert_eq!(stats.bytes_total, bytes.len() as u64);
    let mut r = RecordReader::new(&bytes);
    r.parse_header().unwrap();
    let frame = r.next_frame().unwrap().unwrap();
    assert_eq!(
        (
            frame.sample_kind,
            frame.representation_identifier,
            frame.payload
        ),
        (
            SampleKind::NotAliveUnregistered,
            Some([0x00, 0x03]),
            vec![2]
        )
    );
    assert_eq!(r.next_frame().unwrap(), None);
}
