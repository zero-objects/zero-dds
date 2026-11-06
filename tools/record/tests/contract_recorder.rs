// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-record` with `zerodds-recorder`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the tool uses:
//! a `RecordingSession` over its output file with the participants and
//! topics it knows (`SessionOptions`, `ParticipantEntry`, `TopicKey`),
//! `add_topic` for a topic whose writer appears later, `record_sample` with
//! the `SampleKind` and the representation identifier of each sample,
//! `finish` at the end, and `RecordReader` for `info` and `list`. What it
//! relies on, from `docs/specs/zddsrec-1.0.md`: the capture is the header,
//! then a frame per sample with its kind (0, 1, 2) and identifier; a topic
//! added after the header takes the next index; a run without samples
//! leaves the header.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::{Arc, Mutex};

use zerodds_recorder::format::{ParticipantEntry, SampleKind};
use zerodds_recorder::reader::RecordReader;
use zerodds_recorder::session::{RecordingSession, SessionOptions, TopicKey};

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
fn zddsrec_a_session_writes_the_header_then_a_frame_per_sample() {
    let sink = SharedSink::default();
    let opts = SessionOptions::new(1_000)
        .with_participant(ParticipantEntry {
            guid: [7; 16],
            name: "zerodds-record".into(),
        })
        .with_topic(topic("Early", "E"));
    let session = RecordingSession::new(sink.clone(), opts);
    session
        .record_sample(
            1_500,
            [7; 16],
            &topic("Early", "E"),
            SampleKind::Alive,
            Some([0x00, 0x0b]),
            vec![1],
        )
        .unwrap();
    assert_eq!(session.add_topic(&topic("Late", "L")).unwrap(), 1);
    session
        .record_sample(
            1_600,
            [7; 16],
            &topic("Late", "L"),
            SampleKind::NotAliveDisposed,
            None,
            vec![],
        )
        .unwrap();
    session.finish().unwrap();

    let bytes = sink.0.lock().unwrap().clone();
    assert_eq!(&bytes[..4], b"ZDDS");
    let mut r = RecordReader::new(&bytes);
    assert_eq!(r.parse_header().unwrap().topics.len(), 1);
    let first = r.next_frame().unwrap().unwrap();
    assert_eq!(
        (
            first.timestamp_delta_ns,
            first.topic_idx,
            first.sample_kind,
            first.representation_identifier
        ),
        (500, 0, SampleKind::Alive, Some([0x00, 0x0b]))
    );
    let second = r.next_frame().unwrap().unwrap();
    assert_eq!(
        (
            second.topic_idx,
            second.sample_kind,
            second.representation_identifier
        ),
        (1, SampleKind::NotAliveDisposed, None)
    );
    assert_eq!(r.added_topics()[0].name, "Late");
    assert_eq!(SampleKind::NotAliveUnregistered.to_u8(), 2);
}

#[test]
fn zddsrec_a_run_without_samples_leaves_the_header() {
    let sink = SharedSink::default();
    let session = RecordingSession::new(
        sink.clone(),
        SessionOptions::new(9).with_topic(topic("T", "X")),
    );
    session.finish().unwrap();
    let bytes = sink.0.lock().unwrap().clone();
    let mut r = RecordReader::new(&bytes);
    assert_eq!(r.parse_header().unwrap().topics[0].type_name, "X");
    assert_eq!(r.next_frame().unwrap(), None);
}
