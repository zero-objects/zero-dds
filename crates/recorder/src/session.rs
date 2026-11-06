// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Live recording session — high-level API for a running capture.
//!
//! Wraps a [`RecordWriter`] behind a topic-indexing layer:
//! the consumer calls `record_sample(topic, type, payload)` and the
//! indexer takes care of:
//!
//! * Initial header written on the first sample (lazy).
//! * Topic/participant entries are added on demand.
//!   Since the format is header-once, a new header is written
//!   when the first topics are still unknown — i.e. the caller
//!   should register the set up front via [`SessionOptions`].
//! * Atomic counters (frames, bytes) for the dashboard.
//!
//! Wiring to the DcpsRuntime (a hook on the built-in topics
//! `DCPSPublication`/`DCPSSubscription`) is the job of the dcps crate
//! and of `tools/recorder-bridge` — `RecordingSession` provides the
//! thread-safe write ingress here.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::format::{Frame, Header, ParticipantEntry, SampleKind, TopicEntry};
use crate::writer::{RecordWriter, WriteError};

/// Convenient topic key: tuple of topic name and type name.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TopicKey {
    /// DDS topic name (e.g. with an `rt/` prefix).
    pub topic: String,
    /// Type name (e.g. `"std_msgs::msg::String"`).
    pub type_name: String,
}

/// Setup options for a session.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// UNIX epoch anchor in nanoseconds — frame timestamps are
    /// deltas relative to it.
    pub time_base_unix_ns: i64,
    /// Pre-known participants (GUID + name).
    pub participants: Vec<ParticipantEntry>,
    /// Pre-known topics. If a topic arrives that is not
    /// in here, the session ignores the sample (the counter
    /// `samples_dropped_unknown_topic` is incremented).
    pub topics: Vec<TopicKey>,
}

impl SessionOptions {
    /// Constructor with time_base_unix_ns and empty lists.
    #[must_use]
    pub fn new(time_base_unix_ns: i64) -> Self {
        Self {
            time_base_unix_ns,
            participants: Vec::new(),
            topics: Vec::new(),
        }
    }

    /// Adds a participant (builder form).
    #[must_use]
    pub fn with_participant(mut self, p: ParticipantEntry) -> Self {
        self.participants.push(p);
        self
    }

    /// Adds a topic (builder form).
    #[must_use]
    pub fn with_topic(mut self, t: TopicKey) -> Self {
        self.topics.push(t);
        self
    }
}

/// Session error.
#[derive(Debug)]
pub enum SessionError {
    /// Underlying writer error.
    Writer(WriteError),
    /// Session mutex poisoned.
    Poisoned,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(e) => write!(f, "writer: {e}"),
            Self::Poisoned => write!(f, "session mutex poisoned"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<WriteError> for SessionError {
    fn from(e: WriteError) -> Self {
        Self::Writer(e)
    }
}

/// Live recording session.
///
/// Thread-safe: multiple threads can call
/// `record_sample` concurrently.
pub struct RecordingSession<W: std::io::Write + Send> {
    inner: Mutex<Inner<W>>,
    samples_total: AtomicU64,
    samples_dropped: AtomicU64,
    bytes_total: AtomicU64,
}

struct Inner<W: std::io::Write> {
    writer: RecordWriter<W>,
    /// (topic, type) → topic_idx in the header.
    topic_index: Vec<(TopicKey, u32)>,
    participant_index: Vec<([u8; 16], u32)>,
    time_base_unix_ns: i64,
    header_written: bool,
    /// Pre-allocated header data — flushed on the first sample.
    pending_header: Header,
}

impl<W: std::io::Write + Send> RecordingSession<W> {
    /// Creates a new session over `sink`. The header is written on
    /// the first `record_sample`.
    pub fn new(sink: W, opts: SessionOptions) -> Self {
        let mut topic_index = Vec::with_capacity(opts.topics.len());
        for (i, t) in opts.topics.iter().enumerate() {
            topic_index.push((t.clone(), i as u32));
        }
        let participant_index = opts
            .participants
            .iter()
            .enumerate()
            .map(|(i, p)| (p.guid, i as u32))
            .collect();
        let header = Header {
            time_base_unix_ns: opts.time_base_unix_ns,
            participants: opts.participants,
            topics: opts
                .topics
                .into_iter()
                .map(|t| TopicEntry {
                    name: t.topic,
                    type_name: t.type_name,
                })
                .collect(),
        };
        Self {
            inner: Mutex::new(Inner {
                writer: RecordWriter::new(sink),
                topic_index,
                participant_index,
                time_base_unix_ns: opts.time_base_unix_ns,
                header_written: false,
                pending_header: header,
            }),
            samples_total: AtomicU64::new(0),
            samples_dropped: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
        }
    }

    /// Adds a topic the session did not know at construction.
    ///
    /// A recorder cannot always name every topic up front: it learns a
    /// topic's type from the writer that publishes it, and a writer that
    /// starts later is a normal case, not an error. Before this existed
    /// such a topic had two outcomes and both were bad — the recorder
    /// either gave up on it, or announced a placeholder type that no
    /// typed writer would ever match, and then recorded nothing for the
    /// rest of the run without saying so.
    ///
    /// Answers the topic's index. Already-known topics answer their
    /// existing index and write nothing, so calling this on discovery is
    /// safe to repeat.
    ///
    /// # Errors
    /// See [`SessionError`]. Writing the addition can fail like any
    /// other write.
    pub fn add_topic(&self, topic: &TopicKey) -> Result<u32, SessionError> {
        let mut g = self.inner.lock().map_err(|_| SessionError::Poisoned)?;
        if let Some((_, index)) = g.topic_index.iter().find(|(k, _)| k == topic) {
            return Ok(*index);
        }
        if !g.header_written {
            // Still cheap: the header has not gone out, so the topic can
            // join the list it will be written from.
            let index = u32::try_from(g.topic_index.len()).unwrap_or(u32::MAX);
            g.pending_header.topics.push(TopicEntry {
                name: topic.topic.clone(),
                type_name: topic.type_name.clone(),
            });
            g.topic_index.push((topic.clone(), index));
            return Ok(index);
        }
        let entry = TopicEntry {
            name: topic.topic.clone(),
            type_name: topic.type_name.clone(),
        };
        let index = g.writer.add_topic(&entry)?;
        g.topic_index.push((topic.clone(), index));
        Ok(index)
    }

    /// Writes a sample. `now_unix_ns` must be the current
    /// wall-clock time in nanoseconds since the epoch.
    /// `representation_identifier` is the identifier of the sample's
    /// encapsulation header (DDSI-RTPS 2.5 §10.5, e.g.
    /// `UserSample::Alive::representation_identifier`), `None` where there
    /// is none (a dispose or unregister without payload); `payload` is the
    /// body behind that header. A sample of a topic the session does not
    /// know is dropped and counted in `samples_dropped`; a participant GUID
    /// it does not know is recorded under participant index 0.
    ///
    /// # Errors
    /// See [`SessionError`].
    pub fn record_sample(
        &self,
        now_unix_ns: i64,
        participant_guid: [u8; 16],
        topic: &TopicKey,
        sample_kind: SampleKind,
        representation_identifier: Option<[u8; 2]>,
        payload: Vec<u8>,
    ) -> Result<(), SessionError> {
        let mut g = self.inner.lock().map_err(|_| SessionError::Poisoned)?;
        if !g.header_written {
            let header = g.pending_header.clone();
            g.writer.write_header(&header)?;
            g.header_written = true;
        }
        let Some(topic_idx) = g
            .topic_index
            .iter()
            .find(|(k, _)| k == topic)
            .map(|(_, i)| *i)
        else {
            self.samples_dropped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        };
        let participant_idx = g
            .participant_index
            .iter()
            .find(|(g_guid, _)| g_guid == &participant_guid)
            .map(|(_, i)| *i)
            .unwrap_or(0);
        let frame = Frame {
            timestamp_delta_ns: now_unix_ns - g.time_base_unix_ns,
            participant_idx,
            topic_idx,
            sample_kind,
            representation_identifier,
            payload,
        };
        g.writer.write_frame(&frame)?;
        self.samples_total.fetch_add(1, Ordering::Relaxed);
        // `bytes_written` is the writer's running total, not the size of
        // the frame just written, so it is stored and not added. Adding
        // it made the counter the sum of every intermediate total, which
        // grows with the square of the sample count: a 1.2 MB capture
        // was reported as 28 GB, and one of 20 000 samples as 282 GB.
        // The number appears in the closing line of every run and reads
        // as a throughput figure.
        //
        // The store is safe against the concurrent readers of `stats`
        // because `g` is held for the whole of this function: writes to
        // this counter are serialised by the same lock that serialises
        // the frames.
        self.bytes_total
            .store(g.writer.bytes_written(), Ordering::Relaxed);
        Ok(())
    }

    /// Ends the capture: writes the header if no sample did (a capture
    /// without samples is a header without frames, `docs/specs/zddsrec-1.0.md`,
    /// "File structure"), then flushes the sink.
    ///
    /// # Errors
    /// See [`SessionError`].
    pub fn finish(&self) -> Result<(), SessionError> {
        let mut g = self.inner.lock().map_err(|_| SessionError::Poisoned)?;
        if !g.header_written {
            let header = g.pending_header.clone();
            g.writer.write_header(&header)?;
            g.header_written = true;
            self.bytes_total
                .store(g.writer.bytes_written(), Ordering::Relaxed);
        }
        g.writer.flush()?;
        Ok(())
    }

    /// Returns the current counters (snapshot).
    #[must_use]
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            samples_total: self.samples_total.load(Ordering::Relaxed),
            samples_dropped: self.samples_dropped.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
        }
    }
}

/// Counter snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Number of successfully written samples.
    pub samples_total: u64,
    /// Number of dropped samples (topic not in the header).
    pub samples_dropped: u64,
    /// Total file bytes (incl. header).
    pub bytes_total: u64,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)] // tests may use unwrap and panic.
mod tests {
    //! The section numbers are those of `docs/spec-coverage/zddsrec-1.0.md`.
    use super::*;
    use crate::reader::RecordReader;
    use std::sync::Arc;

    fn p(name: &str, guid_byte: u8) -> ParticipantEntry {
        ParticipantEntry {
            guid: [guid_byte; 16],
            name: name.into(),
        }
    }
    fn t(topic: &str, ty: &str) -> TopicKey {
        TopicKey {
            topic: topic.into(),
            type_name: ty.into(),
        }
    }

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
    impl SharedSink {
        fn bytes(&self) -> Vec<u8> {
            self.0.lock().unwrap().clone()
        }
    }

    /// The header goes out with the first sample, with the session's
    /// time base, participants and topics (the writer's type name); a frame
    /// holds its time as the delta to the time base, its kind and payload.
    #[test]
    fn zddsrec_5_the_header_goes_out_with_the_first_sample_and_frames_hold_time_deltas() {
        let sink = SharedSink::default();
        let opts = SessionOptions::new(1_000)
            .with_participant(p("rec", 7))
            .with_topic(t("Track", "cuas::Track"));
        let s = RecordingSession::new(sink.clone(), opts);
        assert!(sink.bytes().is_empty(), "no header before the first sample");
        let key = t("Track", "cuas::Track");
        for (now, kind, payload) in [
            (1_500, SampleKind::Alive, vec![0xde, 0xad]),
            (1_600, SampleKind::NotAliveDisposed, vec![]),
            (1_700, SampleKind::NotAliveUnregistered, vec![]),
        ] {
            let id = (!payload.is_empty()).then_some([0x00, 0x09]);
            s.record_sample(now, [7; 16], &key, kind, id, payload)
                .unwrap();
        }
        let bytes = sink.bytes();
        let mut r = RecordReader::new(&bytes);
        let header = r.parse_header().unwrap();
        assert_eq!(header.time_base_unix_ns, 1_000);
        assert_eq!(header.participants, vec![p("rec", 7)]);
        assert_eq!(
            header.topics,
            vec![TopicEntry {
                name: "Track".into(),
                type_name: "cuas::Track".into()
            }]
        );
        let mut got = Vec::new();
        while let Some(f) = r.next_frame().unwrap() {
            got.push((
                f.timestamp_delta_ns,
                f.sample_kind,
                f.representation_identifier,
                f.payload,
            ));
        }
        assert_eq!(
            got,
            vec![
                (500, SampleKind::Alive, Some([0x00, 0x09]), vec![0xde, 0xad]),
                (600, SampleKind::NotAliveDisposed, None, vec![]),
                (700, SampleKind::NotAliveUnregistered, None, vec![]),
            ]
        );
    }

    /// A capture without samples is its header (`finish`), which a reader
    /// reads as a file without frames.
    #[test]
    fn zddsrec_1_a_capture_without_samples_is_its_header() {
        let sink = SharedSink::default();
        let s = RecordingSession::new(
            sink.clone(),
            SessionOptions::new(5).with_topic(t("/x", "T")),
        );
        s.finish().unwrap();
        let bytes = sink.bytes();
        assert_eq!(s.stats().bytes_total, bytes.len() as u64);
        let mut r = RecordReader::new(&bytes);
        assert_eq!(r.parse_header().unwrap().time_base_unix_ns, 5);
        assert_eq!(r.next_frame().unwrap(), None);
    }

    /// `bytes_total` is the size of the capture: after each sample, the
    /// number of octets the sink holds.
    ///
    /// It used to add the writer's running total once per sample, which
    /// makes the counter grow with the square of the sample count: a
    /// capture of 1.2 MB was reported as 28 GB.
    #[test]
    fn the_byte_count_is_the_size_of_the_capture() {
        let sink = SharedSink::default();
        let opts = SessionOptions::new(0)
            .with_participant(p("talker", 1))
            .with_topic(t("/x", "T"));
        let s = RecordingSession::new(sink.clone(), opts);
        for index in 0..20u8 {
            s.record_sample(
                i64::from(index) + 1,
                [1u8; 16],
                &t("/x", "T"),
                SampleKind::Alive,
                Some([0x00, 0x09]),
                vec![index; 64],
            )
            .unwrap();
            assert_eq!(
                s.stats().bytes_total,
                sink.bytes().len() as u64,
                "sample {index}"
            );
        }
    }

    /// A topic that appears mid-run (its writer started after the
    /// recorder) goes out as a `TopicAdd` with the next index; adding it
    /// twice answers the same index and writes nothing; its samples read
    /// back under that index.
    #[test]
    fn zddsrec_2_a_topic_added_after_the_header_takes_the_next_index() {
        let sink = SharedSink::default();
        let opts = SessionOptions::new(0)
            .with_participant(p("talker", 1))
            .with_topic(t("/early", "Early"));
        let s = RecordingSession::new(sink.clone(), opts);
        // One sample first, so the header is out and the addition has to
        // travel as a frame rather than joining the pending list.
        s.record_sample(
            1,
            [1u8; 16],
            &t("/early", "Early"),
            SampleKind::Alive,
            None,
            vec![7],
        )
        .unwrap();
        let late = t("/late", "Late");
        assert_eq!(s.add_topic(&late).unwrap(), 1);
        let before = sink.bytes().len();
        assert_eq!(s.add_topic(&late).unwrap(), 1);
        assert_eq!(sink.bytes().len(), before);
        s.record_sample(2, [1u8; 16], &late, SampleKind::Alive, None, vec![8, 9])
            .unwrap();
        assert_eq!(s.stats().samples_dropped, 0);

        let bytes = sink.bytes();
        let mut r = RecordReader::new(&bytes);
        assert_eq!(r.parse_header().unwrap().topics.len(), 1);
        let first = r.next_frame().unwrap().unwrap();
        let second = r.next_frame().unwrap().unwrap();
        assert_eq!((first.topic_idx, first.payload), (0, vec![7]));
        assert_eq!((second.topic_idx, second.payload), (1, vec![8, 9]));
        assert_eq!(
            r.added_topics(),
            [TopicEntry {
                name: "/late".into(),
                type_name: "Late".into()
            }]
        );
    }

    /// What the session does not know (the documentation of
    /// `record_sample`): a topic is dropped and counted, a participant GUID
    /// is recorded under index 0.
    #[test]
    fn an_unknown_topic_is_dropped_and_an_unknown_participant_is_index_zero() {
        let sink = SharedSink::default();
        let opts = SessionOptions::new(0)
            .with_participant(p("p", 1))
            .with_topic(t("/known", "T"));
        let s = RecordingSession::new(sink.clone(), opts);
        s.record_sample(
            1,
            [1u8; 16],
            &t("/unknown", "U"),
            SampleKind::Alive,
            None,
            vec![],
        )
        .unwrap();
        s.record_sample(
            2,
            [99u8; 16],
            &t("/known", "T"),
            SampleKind::Alive,
            None,
            vec![5],
        )
        .unwrap();
        assert_eq!((s.stats().samples_total, s.stats().samples_dropped), (1, 1));
        let bytes = sink.bytes();
        let mut r = RecordReader::new(&bytes);
        r.parse_header().unwrap();
        let only = r.next_frame().unwrap().unwrap();
        assert_eq!((only.participant_idx, only.payload), (0, vec![5]));
        assert_eq!(r.next_frame().unwrap(), None);
    }

    /// Four threads recording at once: every sample is a frame of the file.
    #[test]
    fn samples_of_four_threads_are_all_frames_of_the_file() {
        let sink = SharedSink::default();
        let opts = SessionOptions::new(0)
            .with_participant(p("p0", 1))
            .with_participant(p("p1", 2))
            .with_topic(t("/a", "T"))
            .with_topic(t("/b", "T"));
        let s = Arc::new(RecordingSession::new(sink.clone(), opts));
        let handles: Vec<_> = (0..4u8)
            .map(|thread_id| {
                let s = Arc::clone(&s);
                std::thread::spawn(move || {
                    for i in 0..100u8 {
                        let topic = if i % 2 == 0 {
                            t("/a", "T")
                        } else {
                            t("/b", "T")
                        };
                        let guid = [1 + thread_id / 2; 16];
                        s.record_sample(
                            i64::from(i),
                            guid,
                            &topic,
                            SampleKind::Alive,
                            None,
                            vec![i],
                        )
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let bytes = sink.bytes();
        let mut r = RecordReader::new(&bytes);
        r.parse_header().unwrap();
        let mut frames = 0;
        while r.next_frame().unwrap().is_some() {
            frames += 1;
        }
        assert_eq!((frames, s.stats().samples_total), (400, 400));
    }
}
