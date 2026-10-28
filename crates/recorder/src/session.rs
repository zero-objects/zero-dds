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
    ///
    /// # Errors
    /// See [`SessionError`].
    pub fn record_sample(
        &self,
        now_unix_ns: i64,
        participant_guid: [u8; 16],
        topic: &TopicKey,
        sample_kind: SampleKind,
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
    use super::*;

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

    #[test]
    fn session_writes_header_lazy_on_first_sample() {
        let opts = SessionOptions::new(1_700_000_000_000_000_000)
            .with_participant(p("talker", 1))
            .with_topic(t("/x", "T"));
        let s: RecordingSession<Vec<u8>> = RecordingSession::new(Vec::new(), opts);
        assert_eq!(s.stats().samples_total, 0);
        s.record_sample(
            1_700_000_000_000_001_000,
            [1u8; 16],
            &t("/x", "T"),
            SampleKind::Alive,
            vec![1, 2, 3],
        )
        .unwrap();
        assert_eq!(s.stats().samples_total, 1);
    }

    /// `bytes_total` is the size of the capture, not a sum of sizes.
    ///
    /// It used to add the writer's running total once per sample, which
    /// makes the counter grow with the square of the sample count: after
    /// n samples it held the sum of every intermediate total. A capture
    /// of 1.2 MB was reported as 28 GB. The number is printed at the end
    /// of every run and reads as a throughput figure, so it invites a
    /// wrong conclusion rather than merely being wrong.
    #[test]
    fn bytes_total_is_the_size_written_and_not_a_running_sum() {
        let opts = SessionOptions::new(0)
            .with_participant(p("talker", 1))
            .with_topic(t("/x", "T"));
        let sink: Vec<u8> = Vec::new();
        let s: RecordingSession<Vec<u8>> = RecordingSession::new(sink, opts);

        let mut previous = 0u64;
        for index in 0..20u8 {
            s.record_sample(
                i64::from(index) + 1,
                [1u8; 16],
                &t("/x", "T"),
                SampleKind::Alive,
                vec![index; 64],
            )
            .unwrap();
            let now = s.stats().bytes_total;
            assert!(
                now > previous,
                "sample {index}: the count did not grow ({previous} -> {now})"
            );
            // Each sample carries 64 octets plus a frame header. A growth
            // of more than a kilobyte per sample means the counter is
            // summing totals again.
            assert!(
                now - previous < 1024,
                "sample {index} added {} octets for a 64-octet payload — the \
                 counter is summing running totals",
                now - previous
            );
            previous = now;
        }
    }

    /// A topic that appears mid-run is recorded, and reads back.
    ///
    /// This is the case a recorder meets whenever a writer starts after
    /// it does: the topic is known by name from the command line, its
    /// type only from the writer. The whole round trip is asserted —
    /// added after the header went out, written to, and read back with
    /// the right index and the right name — because each half alone
    /// would pass while the file stayed unreadable.
    #[test]
    fn a_topic_added_after_the_header_records_and_reads_back() {
        use crate::reader::RecordReader;

        // A shared sink, so the bytes are readable while the session
        // still owns its writer.
        #[derive(Clone)]
        struct SharedSink(std::sync::Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for SharedSink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let sink = std::sync::Arc::new(Mutex::new(Vec::new()));
        let opts = SessionOptions::new(0)
            .with_participant(p("talker", 1))
            .with_topic(t("/early", "Early"));
        let s = RecordingSession::new(SharedSink(std::sync::Arc::clone(&sink)), opts);

        // One sample first, so the header is out and the addition has to
        // travel as a frame rather than joining the pending list.
        s.record_sample(
            1,
            [1u8; 16],
            &t("/early", "Early"),
            SampleKind::Alive,
            vec![7],
        )
        .unwrap();

        let late = t("/late", "Late");
        let index = s.add_topic(&late).unwrap();
        assert_eq!(index, 1, "the addition continues the header's list");
        assert_eq!(
            s.add_topic(&late).unwrap(),
            1,
            "adding it twice answers the same index and writes nothing"
        );

        s.record_sample(2, [1u8; 16], &late, SampleKind::Alive, vec![8, 9])
            .unwrap();
        assert_eq!(s.stats().samples_dropped, 0, "the late topic is known now");
        assert_eq!(s.stats().samples_total, 2);

        let bytes = sink.lock().unwrap().clone();
        let mut r = RecordReader::new(&bytes);
        let header = r.parse_header().unwrap();
        assert_eq!(header.topics.len(), 1, "the header holds only what it knew");

        let Ok(Some(first)) = r.next_frame() else {
            panic!("the first frame is missing")
        };
        assert_eq!(first.topic_idx, 0);
        assert_eq!(first.payload, vec![7]);

        let Ok(Some(second)) = r.next_frame() else {
            panic!("the second frame is missing")
        };
        assert_eq!(second.topic_idx, 1);
        assert_eq!(second.payload, vec![8, 9]);
        assert_eq!(
            r.added_topics().len(),
            1,
            "the reader picked the addition out of the frame stream"
        );
        assert_eq!(r.added_topics()[0].name, "/late");
        assert_eq!(r.added_topics()[0].type_name, "Late");
    }

    #[test]
    fn session_drops_unknown_topic() {
        let opts = SessionOptions::new(0)
            .with_participant(p("p", 1))
            .with_topic(t("/known", "T"));
        let s: RecordingSession<Vec<u8>> = RecordingSession::new(Vec::new(), opts);
        s.record_sample(1, [1u8; 16], &t("/unknown", "U"), SampleKind::Alive, vec![])
            .unwrap();
        assert_eq!(s.stats().samples_total, 0);
        assert_eq!(s.stats().samples_dropped, 1);
    }

    #[test]
    fn session_thread_safe_record() {
        use std::sync::Arc;
        use std::thread;
        let opts = SessionOptions::new(0)
            .with_participant(p("p0", 1))
            .with_participant(p("p1", 2))
            .with_topic(t("/a", "T"))
            .with_topic(t("/b", "T"));
        let s: Arc<RecordingSession<Vec<u8>>> = Arc::new(RecordingSession::new(Vec::new(), opts));
        let mut handles = Vec::new();
        for thread_id in 0..4 {
            let s = Arc::clone(&s);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let topic = if i % 2 == 0 {
                        t("/a", "T")
                    } else {
                        t("/b", "T")
                    };
                    let guid_byte = if thread_id < 2 { 1 } else { 2 };
                    s.record_sample(
                        i as i64,
                        [guid_byte; 16],
                        &topic,
                        SampleKind::Alive,
                        vec![i as u8],
                    )
                    .unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(s.stats().samples_total, 400);
        assert_eq!(s.stats().samples_dropped, 0);
    }

    #[test]
    fn session_unknown_participant_falls_back_to_idx_zero() {
        let opts = SessionOptions::new(0)
            .with_participant(p("p", 1))
            .with_topic(t("/a", "T"));
        let s: RecordingSession<Vec<u8>> = RecordingSession::new(Vec::new(), opts);
        // GUID not in the list → fallback idx=0.
        s.record_sample(
            1,
            [99u8; 16], // unknown
            &t("/a", "T"),
            SampleKind::Alive,
            vec![],
        )
        .unwrap();
        assert_eq!(s.stats().samples_total, 1);
    }

    /// Full record → read-back round-trip with the REAL (typed) topic and all
    /// three sample kinds — the recorder side of the type-following capture path
    /// (`zerodds-record`): the header carries the writer's true type (not
    /// RawBytes) and every frame's kind / topic / payload / timestamp-delta
    /// survives a parse.
    #[test]
    fn session_roundtrip_typed_topic_all_kinds() {
        use crate::reader::RecordReader;
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct SharedSink(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedSink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let opts = SessionOptions::new(1_000)
            .with_participant(p("rec", 7))
            .with_topic(t("Track", "cuas::Track"));
        let s = RecordingSession::new(SharedSink(Arc::clone(&buf)), opts);
        let guid = [7u8; 16];
        let key = t("Track", "cuas::Track");
        s.record_sample(
            1_500,
            guid,
            &key,
            SampleKind::Alive,
            vec![0xde, 0xad, 0xbe, 0xef],
        )
        .unwrap();
        s.record_sample(1_600, guid, &key, SampleKind::NotAliveDisposed, vec![])
            .unwrap();
        s.record_sample(1_700, guid, &key, SampleKind::NotAliveUnregistered, vec![])
            .unwrap();
        assert_eq!(s.stats().samples_total, 3);

        let bytes = buf.lock().unwrap().clone();
        let mut rdr = RecordReader::new(&bytes);
        let header = rdr.parse_header().unwrap();
        assert_eq!(header.time_base_unix_ns, 1_000);
        assert_eq!(header.topics.len(), 1);
        assert_eq!(header.topics[0].name, "Track");
        // The REAL writer type is recorded, not the generic RawBytes.
        assert_eq!(header.topics[0].type_name, "cuas::Track");

        let f1 = rdr.next_frame().unwrap().unwrap();
        assert_eq!(f1.sample_kind, SampleKind::Alive);
        assert_eq!(f1.topic_idx, 0);
        assert_eq!(f1.payload, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(f1.timestamp_delta_ns, 500); // 1500 - 1000

        let f2 = rdr.next_frame().unwrap().unwrap();
        assert_eq!(f2.sample_kind, SampleKind::NotAliveDisposed);
        assert!(f2.payload.is_empty());

        let f3 = rdr.next_frame().unwrap().unwrap();
        assert_eq!(f3.sample_kind, SampleKind::NotAliveUnregistered);

        assert!(rdr.next_frame().unwrap().is_none());
    }
}
