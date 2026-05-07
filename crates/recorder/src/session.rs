// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Live-Recording-Session — high-level API fuer laufende Capture.
//!
//! Kapselt einen [`RecordWriter`] hinter einer Topic-Indexing-Schicht:
//! Konsument ruft `record_sample(topic, type, payload)` und der
//! Indexer kuemmert sich um:
//!
//! * Initialer Header beim ersten Sample geschrieben (lazy).
//! * Topic-/Participant-Eintraege werden bei Bedarf nachgetragen.
//!   Da das Format Header-Once ist, wird ein neuer Header geschrieben
//!   wenn die ersten Topics noch unbekannt sind — d.h. der Caller
//!   sollte die Set vorab via [`SessionOptions`] anmelden.
//! * Atomare Counter (Frames, Bytes) fuer das Dashboard.
//!
//! Die Anbindung an die DcpsRuntime (Hook auf Built-in-Topics
//! `DCPSPublication`/`DCPSSubscription`) ist Aufgabe des dcps-Crate
//! und des `tools/recorder-bridge` — `RecordingSession` bietet dem
//! hier den thread-safen Schreib-Ingress.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::format::{Frame, Header, ParticipantEntry, SampleKind, TopicEntry};
use crate::writer::{RecordWriter, WriteError};

/// Bequemer Topic-Schluessel: Tuple aus Topic-Name und Type-Name.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TopicKey {
    /// DDS-Topic-Name (z.B. mit `rt/`-Prefix).
    pub topic: String,
    /// Type-Name (z.B. `"std_msgs::msg::String"`).
    pub type_name: String,
}

/// Setup-Optionen fuer eine Session.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// UNIX-Epoch-Anchor in Nanosekunden — Frame-Timestamps sind
    /// Deltas dazu.
    pub time_base_unix_ns: i64,
    /// Vorab-bekannte Participants (GUID + Name).
    pub participants: Vec<ParticipantEntry>,
    /// Vorab-bekannte Topics. Falls ein Topic kommt das hier nicht
    /// drin ist, ignoriert die Session den Sample (Counter
    /// `samples_dropped_unknown_topic` wird inkrementiert).
    pub topics: Vec<TopicKey>,
}

impl SessionOptions {
    /// Konstruktor mit time_base_unix_ns und leeren Listen.
    #[must_use]
    pub fn new(time_base_unix_ns: i64) -> Self {
        Self {
            time_base_unix_ns,
            participants: Vec::new(),
            topics: Vec::new(),
        }
    }

    /// Fuegt einen Participant hinzu (Builder-Form).
    #[must_use]
    pub fn with_participant(mut self, p: ParticipantEntry) -> Self {
        self.participants.push(p);
        self
    }

    /// Fuegt ein Topic hinzu (Builder-Form).
    #[must_use]
    pub fn with_topic(mut self, t: TopicKey) -> Self {
        self.topics.push(t);
        self
    }
}

/// Session-Fehler.
#[derive(Debug)]
pub enum SessionError {
    /// Underlying writer error.
    Writer(WriteError),
    /// Session-Mutex vergiftet.
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

/// Live-Recording-Session.
///
/// Thread-safe: mehrere Threads koennen gleichzeitig
/// `record_sample` callen.
pub struct RecordingSession<W: std::io::Write + Send> {
    inner: Mutex<Inner<W>>,
    samples_total: AtomicU64,
    samples_dropped: AtomicU64,
    bytes_total: AtomicU64,
}

struct Inner<W: std::io::Write> {
    writer: RecordWriter<W>,
    /// (topic, type) → topic_idx im Header.
    topic_index: Vec<(TopicKey, u32)>,
    participant_index: Vec<([u8; 16], u32)>,
    time_base_unix_ns: i64,
    header_written: bool,
    /// Vor-Allokierte Header-Daten — werden beim ersten Sample
    /// geflushed.
    pending_header: Header,
}

impl<W: std::io::Write + Send> RecordingSession<W> {
    /// Erzeugt eine neue Session ueber `sink`. Der Header wird beim
    /// ersten `record_sample` geschrieben.
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

    /// Schreibt einen Sample. `now_unix_ns` muss die aktuelle
    /// Wallclock-Zeit in Nanosekunden seit Epoch sein.
    ///
    /// # Errors
    /// Siehe [`SessionError`].
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
        self.bytes_total
            .fetch_add(g.writer.bytes_written(), Ordering::Relaxed);
        Ok(())
    }

    /// Liefert die aktuellen Counter (Snapshot).
    #[must_use]
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            samples_total: self.samples_total.load(Ordering::Relaxed),
            samples_dropped: self.samples_dropped.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
        }
    }
}

/// Counter-Snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Anzahl erfolgreich geschriebener Samples.
    pub samples_total: u64,
    /// Anzahl droppter Samples (Topic nicht im Header).
    pub samples_dropped: u64,
    /// Gesamte File-Bytes (inkl. Header).
    pub bytes_total: u64,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
        // GUID nicht in der Liste → fallback idx=0.
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
}
