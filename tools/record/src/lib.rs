// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-record` library — CLI parsing + dispatch logic.
//!
//! Crate `zerodds-record`. Safety classification: **COMFORT**.
//! User-facing CLI-Tool; das Recording-Backend ist `zerodds-recorder`.
//!
//! Das eigentliche Recording-Backend liegt in `zerodds-recorder`
//! (`crates/recorder/`). Diese Crate liefert nur das CLI-Frontend
//! plus Inspect-Helpers für `.zddsrec`-Files.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Sub-command des Record-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `record` — startet eine Capture-Session.
    Record(RecordArgs),
    /// `info <FILE>` — liest Header eines `.zddsrec` und druckt Metadata.
    Info(InfoArgs),
    /// `list <FILE>` — zählt Frames pro Topic und druckt Sample-Count.
    List(InfoArgs),
}

/// Argumente für `zerodds-record record`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordArgs {
    /// Output-Pfad für `.zddsrec`. Default: `capture-<timestamp>.zddsrec`.
    pub output: Option<String>,
    /// DDS-Domain-ID. Default: 0.
    pub domain: u32,
    /// Topic-Filter (Prefix oder Glob); leer = alle Topics.
    pub topics: Vec<String>,
    /// Recording-Duration; `None` = bis SIGINT.
    pub duration: Option<Duration>,
    /// Maximale Sample-Größe (DoS-Cap). Default 1 MiB.
    pub max_sample_bytes: usize,
}

impl Default for RecordArgs {
    fn default() -> Self {
        Self {
            output: None,
            domain: 0,
            topics: Vec::new(),
            duration: None,
            max_sample_bytes: 1 << 20,
        }
    }
}

/// Argumente für `info` und `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoArgs {
    /// Pfad zur `.zddsrec`-Datei.
    pub file: String,
}

/// Parse-Fehler beim CLI-args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Kein subcommand angegeben.
    Missing,
    /// Unbekanntes subcommand.
    Unknown(String),
    /// Required-arg fehlt.
    MissingArg(&'static str),
    /// Wert ist nicht parse-bar.
    BadValue {
        /// Welche flag.
        flag: &'static str,
        /// Was eingegeben war.
        got: String,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "no sub-command given"),
            Self::Unknown(s) => write!(f, "unknown sub-command: {s}"),
            Self::MissingArg(a) => write!(f, "missing required arg: {a}"),
            Self::BadValue { flag, got } => write!(f, "bad value for --{flag}: {got}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse `args` (typisch `env::args().skip(1)`) zu einem Command.
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let sub = args.first().ok_or(ParseError::Missing)?;
    match sub.as_str() {
        "record" => parse_record(&args[1..]).map(Command::Record),
        "info" => parse_info(&args[1..]).map(Command::Info),
        "list" => parse_info(&args[1..]).map(Command::List),
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

fn parse_record(rest: &[String]) -> Result<RecordArgs, ParseError> {
    let mut out = RecordArgs::default();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                out.output = Some(rest.get(i).ok_or(ParseError::MissingArg("output"))?.clone());
            }
            "--domain" | "-d" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("domain"))?;
                out.domain = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "domain",
                    got: v.clone(),
                })?;
            }
            "--topic" | "-t" => {
                i += 1;
                out.topics
                    .push(rest.get(i).ok_or(ParseError::MissingArg("topic"))?.clone());
            }
            "--duration" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("duration"))?;
                out.duration = Some(parse_duration(v)?);
            }
            "--max-sample-bytes" => {
                i += 1;
                let v = rest
                    .get(i)
                    .ok_or(ParseError::MissingArg("max-sample-bytes"))?;
                out.max_sample_bytes = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "max-sample-bytes",
                    got: v.clone(),
                })?;
            }
            other => return Err(ParseError::Unknown(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

fn parse_info(rest: &[String]) -> Result<InfoArgs, ParseError> {
    let file = rest.first().ok_or(ParseError::MissingArg("FILE"))?.clone();
    Ok(InfoArgs { file })
}

fn parse_duration(s: &str) -> Result<Duration, ParseError> {
    let bad = || ParseError::BadValue {
        flag: "duration",
        got: s.to_string(),
    };
    let (num, unit) = s
        .find(|c: char| c.is_alphabetic())
        .map_or((s, "s"), |idx| (&s[..idx], &s[idx..]));
    let n: u64 = num.parse().map_err(|_| bad())?;
    let secs = match unit {
        "s" | "" => n,
        "m" => n.checked_mul(60).ok_or_else(bad)?,
        "h" => n.checked_mul(3600).ok_or_else(bad)?,
        _ => return Err(bad()),
    };
    Ok(Duration::from_secs(secs))
}

/// Versucht den Header eines `.zddsrec` zu lesen und liefert
/// menschen-lesbare Zusammenfassung.
///
/// # Errors
/// I/O- oder Format-Fehler.
pub fn read_header_summary(path: &str) -> std::io::Result<HeaderSummary> {
    use zerodds_recorder::reader::RecordReader;
    let bytes = std::fs::read(path)?;
    let mut r = RecordReader::new(&bytes);
    let h = r
        .parse_header()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    Ok(HeaderSummary {
        time_base_unix_ns: h.time_base_unix_ns,
        participants: h.participants.len(),
        topics: h.topics.iter().map(|t| t.name.clone()).collect(),
    })
}

/// Komprimierte Header-Info zum Drucken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderSummary {
    /// Zeit-Anker in Unix-ns.
    pub time_base_unix_ns: i64,
    /// Anzahl Participants im Header.
    pub participants: usize,
    /// Topic-Namen.
    pub topics: Vec<String>,
}

/// Zählt Frames pro Topic in einem `.zddsrec` und liefert
/// Topic-Name → Sample-Count Map.
///
/// # Errors
/// I/O- oder Format-Fehler.
pub fn count_frames_per_topic(path: &str) -> std::io::Result<Vec<(String, u64)>> {
    use zerodds_recorder::reader::RecordReader;
    let bytes = std::fs::read(path)?;
    let mut r = RecordReader::new(&bytes);
    let h = r
        .parse_header()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?;
    let topic_names: Vec<String> = h.topics.iter().map(|t| t.name.clone()).collect();
    let mut counts = vec![0u64; topic_names.len()];

    while let Some(frame) = r
        .next_frame_view()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e:?}")))?
    {
        let idx = frame.topic_idx as usize;
        if idx < counts.len() {
            counts[idx] = counts[idx].saturating_add(1);
        }
    }

    Ok(topic_names.into_iter().zip(counts).collect())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_record_minimal() {
        let cmd = parse_args(&s(&["record"])).unwrap();
        assert_eq!(cmd, Command::Record(RecordArgs::default()));
    }

    #[test]
    fn parse_record_full() {
        let cmd = parse_args(&s(&[
            "record",
            "-o",
            "out.zddsrec",
            "--domain",
            "7",
            "-t",
            "Sensor*",
            "--duration",
            "30s",
            "--max-sample-bytes",
            "65536",
        ]))
        .unwrap();
        let Command::Record(r) = cmd else {
            panic!("expected record");
        };
        assert_eq!(r.output.as_deref(), Some("out.zddsrec"));
        assert_eq!(r.domain, 7);
        assert_eq!(r.topics, vec!["Sensor*"]);
        assert_eq!(r.duration, Some(Duration::from_secs(30)));
        assert_eq!(r.max_sample_bytes, 65536);
    }

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration("5").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert!(matches!(
            parse_duration("3x"),
            Err(ParseError::BadValue { .. })
        ));
    }

    #[test]
    fn parse_info_subcommand() {
        let cmd = parse_args(&s(&["info", "capture.zddsrec"])).unwrap();
        assert_eq!(
            cmd,
            Command::Info(InfoArgs {
                file: "capture.zddsrec".into()
            })
        );
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        let err = parse_args(&s(&["uhoh"])).unwrap_err();
        assert!(matches!(err, ParseError::Unknown(_)));
    }

    #[test]
    fn parse_no_args_rejected() {
        let err = parse_args(&[]).unwrap_err();
        assert!(matches!(err, ParseError::Missing));
    }

    #[test]
    fn header_summary_reads_synthetic_file() {
        use zerodds_recorder::format::{Header, ParticipantEntry, TopicEntry};

        let mut buf = Vec::new();
        let h = Header {
            time_base_unix_ns: 1_700_000_000_000_000_000,
            participants: vec![ParticipantEntry {
                guid: [1u8; 16],
                name: "test-participant".into(),
            }],
            topics: vec![TopicEntry {
                name: "Foo".into(),
                type_name: "TestType".into(),
            }],
        };
        h.write(&mut buf);

        let path =
            std::env::temp_dir().join(format!("zds-rec-test-{}.zddsrec", std::process::id()));
        std::fs::write(&path, &buf).unwrap();
        let summary = read_header_summary(path.to_str().unwrap()).unwrap();
        assert_eq!(summary.time_base_unix_ns, 1_700_000_000_000_000_000);
        assert_eq!(summary.participants, 1);
        assert_eq!(summary.topics, vec!["Foo"]);
        let _ = std::fs::remove_file(path);
    }
}
