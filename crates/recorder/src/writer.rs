// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RecordWriter` — schreibt einen `.zddsrec`-Stream in einen
//! `std::io::Write`-Sink.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::format::{Frame, Header, ParticipantEntry, TopicEntry};

/// Fehler beim Schreiben.
#[derive(Debug)]
pub enum WriteError {
    /// I/O-Fehler vom Sink.
    Io(std::io::Error),
    /// Header bereits geschrieben — Frames duerfen folgen.
    HeaderAlreadyWritten,
    /// Frame ohne vorhergehendem Header.
    HeaderMissing,
    /// Ungueltiger Index im Frame (uebersteigt Header-Range).
    OutOfRangeIdx {
        /// Was der Frame referenzierte.
        idx: u32,
        /// Wieviele Eintraege der Header bietet.
        len: u32,
        /// Welches Feld ("participant" oder "topic").
        field: &'static str,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::HeaderAlreadyWritten => write!(f, "header already written"),
            Self::HeaderMissing => write!(f, "frame written before header"),
            Self::OutOfRangeIdx { idx, len, field } => {
                write!(f, "{field}_idx {idx} >= {field}_count {len}")
            }
        }
    }
}

impl std::error::Error for WriteError {}

impl From<std::io::Error> for WriteError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Streamender Writer fuer `.zddsrec`-Files.
pub struct RecordWriter<W: std::io::Write> {
    sink: W,
    header_written: bool,
    participants_count: u32,
    topics_count: u32,
    frames_written: u64,
    bytes_written: u64,
}

impl<W: std::io::Write> RecordWriter<W> {
    /// Erzeugt einen Writer ueber `sink`. Caller muss `write_header`
    /// vor dem ersten Frame aufrufen.
    pub fn new(sink: W) -> Self {
        Self {
            sink,
            header_written: false,
            participants_count: 0,
            topics_count: 0,
            frames_written: 0,
            bytes_written: 0,
        }
    }

    /// Schreibt den Header. Kann nur einmal aufgerufen werden.
    ///
    /// # Errors
    /// Returnt [`WriteError::HeaderAlreadyWritten`] wenn schon
    /// gerufen, sonst beliebige IO-Fehler.
    pub fn write_header(&mut self, header: &Header) -> Result<(), WriteError> {
        if self.header_written {
            return Err(WriteError::HeaderAlreadyWritten);
        }
        let mut buf =
            Vec::with_capacity(64 + header.participants.len() * 32 + header.topics.len() * 32);
        header.write(&mut buf);
        self.sink.write_all(&buf)?;
        self.bytes_written = self.bytes_written.saturating_add(buf.len() as u64);
        self.participants_count = u32::try_from(header.participants.len()).unwrap_or(u32::MAX);
        self.topics_count = u32::try_from(header.topics.len()).unwrap_or(u32::MAX);
        self.header_written = true;
        Ok(())
    }

    /// Schreibt einen Frame. Header muss zuvor gesetzt sein.
    ///
    /// # Errors
    /// Returnt [`WriteError::HeaderMissing`] wenn `write_header`
    /// noch nicht aufgerufen, oder [`WriteError::OutOfRangeIdx`] wenn
    /// `participant_idx`/`topic_idx` aus dem Header-Range fallen.
    pub fn write_frame(&mut self, frame: &Frame) -> Result<(), WriteError> {
        if !self.header_written {
            return Err(WriteError::HeaderMissing);
        }
        if frame.participant_idx >= self.participants_count {
            return Err(WriteError::OutOfRangeIdx {
                idx: frame.participant_idx,
                len: self.participants_count,
                field: "participant",
            });
        }
        if frame.topic_idx >= self.topics_count {
            return Err(WriteError::OutOfRangeIdx {
                idx: frame.topic_idx,
                len: self.topics_count,
                field: "topic",
            });
        }
        let mut buf = Vec::with_capacity(32 + frame.payload.len());
        frame.write(&mut buf);
        self.sink.write_all(&buf)?;
        self.bytes_written = self.bytes_written.saturating_add(buf.len() as u64);
        self.frames_written = self.frames_written.saturating_add(1);
        Ok(())
    }

    /// Anzahl bisher geschriebener Frames.
    #[must_use]
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    /// Anzahl Bytes auf dem Sink.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Konsumiert den Writer und gibt den darunter liegenden Sink
    /// zurueck (relevant fuer Cursor-Tests etc.).
    pub fn into_inner(self) -> W {
        self.sink
    }
}

/// Convenience: schreibt Header + alle Frames in einem Schwung.
///
/// # Errors
/// Wie [`RecordWriter::write_header`] / [`RecordWriter::write_frame`].
pub fn write_all<W: std::io::Write>(
    sink: W,
    header: &Header,
    frames: impl IntoIterator<Item = Frame>,
) -> Result<RecordWriter<W>, WriteError> {
    let mut w = RecordWriter::new(sink);
    w.write_header(header)?;
    for f in frames {
        w.write_frame(&f)?;
    }
    Ok(w)
}

/// Bequemer Header-Builder fuer Tests.
#[must_use]
pub fn header_with(
    time_base_unix_ns: i64,
    participants: Vec<(String, [u8; 16])>,
    topics: Vec<(String, String)>,
) -> Header {
    Header {
        time_base_unix_ns,
        participants: participants
            .into_iter()
            .map(|(name, guid)| ParticipantEntry { guid, name })
            .collect(),
        topics: topics
            .into_iter()
            .map(|(name, type_name)| TopicEntry { name, type_name })
            .collect(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;
    use crate::format::SampleKind;

    #[test]
    fn header_must_come_before_frames() {
        let mut w = RecordWriter::new(Vec::<u8>::new());
        let f = Frame {
            timestamp_delta_ns: 0,
            participant_idx: 0,
            topic_idx: 0,
            sample_kind: SampleKind::Alive,
            payload: Vec::new(),
        };
        let r = w.write_frame(&f);
        assert!(matches!(r, Err(WriteError::HeaderMissing)));
    }

    #[test]
    fn header_only_once() {
        let mut w = RecordWriter::new(Vec::<u8>::new());
        let h = Header {
            time_base_unix_ns: 0,
            participants: Vec::new(),
            topics: Vec::new(),
        };
        w.write_header(&h).unwrap();
        let r = w.write_header(&h);
        assert!(matches!(r, Err(WriteError::HeaderAlreadyWritten)));
    }

    #[test]
    fn frame_idx_must_be_in_range() {
        let mut w = RecordWriter::new(Vec::<u8>::new());
        let h = header_with(
            0,
            vec![(String::from("p0"), [0u8; 16])],
            vec![(String::from("/topic"), String::from("Type"))],
        );
        w.write_header(&h).unwrap();
        let bad = Frame {
            timestamp_delta_ns: 1,
            participant_idx: 1,
            topic_idx: 0,
            sample_kind: SampleKind::Alive,
            payload: Vec::new(),
        };
        let r = w.write_frame(&bad);
        assert!(matches!(
            r,
            Err(WriteError::OutOfRangeIdx {
                field: "participant",
                ..
            })
        ));
    }

    #[test]
    fn write_all_helper() {
        let h = header_with(
            1_700_000_000_000_000_000,
            vec![(String::from("p"), [1u8; 16])],
            vec![(String::from("/t"), String::from("T"))],
        );
        let frames = (0..5).map(|i| Frame {
            timestamp_delta_ns: i as i64 * 1000,
            participant_idx: 0,
            topic_idx: 0,
            sample_kind: SampleKind::Alive,
            payload: vec![i as u8; 4],
        });
        let w = write_all(Vec::<u8>::new(), &h, frames).unwrap();
        assert_eq!(w.frames_written(), 5);
        assert!(w.bytes_written() > 0);
    }
}
