// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Read recorded samples back out of the decoded sinks for replay.
//!
//! All three capture formats (`.zddsrec`, NDJSON, SQLite) retain the raw CDR
//! bytes per sample, so `zerodds-replay` can re-publish byte-exact from any of
//! them. This module reads the two *decoded* formats (NDJSON + SQLite) into a
//! flat [`RecordedSample`] list; the `.zddsrec` path stays in `zerodds-recorder`
//! (`RecordReader`), which the replay tool drives directly.

use std::io::BufRead;

use crate::json_sink::from_hex;

/// One recorded sample, normalised across formats. The `raw_cdr` bytes are the
/// exact on-wire CDR body (no encapsulation header) for byte-exact re-publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSample {
    /// DDS topic name.
    pub topic: String,
    /// DDS type name (e.g. `cuas::Track`) — needed to create the replay writer.
    pub type_name: String,
    /// Receive timestamp in unix-ns (pacing / ordering).
    pub recv_ts_ns: i64,
    /// Raw CDR payload bytes.
    pub raw_cdr: Vec<u8>,
}

/// Error reading a replay source.
#[derive(Debug)]
pub enum ReplaySourceError {
    /// Malformed NDJSON line or missing field.
    Json(String),
    /// SQLite failure.
    Db(String),
    /// I/O failure.
    Io(String),
}
impl core::fmt::Display for ReplaySourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json(m) => write!(f, "json: {m}"),
            Self::Db(m) => write!(f, "sqlite: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}
impl std::error::Error for ReplaySourceError {}

type R<T> = Result<T, ReplaySourceError>;

/// Read NDJSON sample lines (as written by [`crate::json_sink::JsonSink`]).
/// Blank lines are skipped; a malformed line or a missing required field is an
/// error.
///
/// # Errors
/// [`ReplaySourceError::Json`] / [`ReplaySourceError::Io`].
pub fn from_ndjson<Rd: BufRead>(reader: Rd) -> R<Vec<RecordedSample>> {
    let mut out = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| ReplaySourceError::Io(e.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| ReplaySourceError::Json(format!("line {}: {e}", lineno + 1)))?;
        let field = |k: &str| -> R<&serde_json::Value> {
            v.get(k).ok_or_else(|| {
                ReplaySourceError::Json(format!("line {}: missing '{k}'", lineno + 1))
            })
        };
        let topic = field("topic")?
            .as_str()
            .ok_or_else(|| {
                ReplaySourceError::Json(format!("line {}: 'topic' not a string", lineno + 1))
            })?
            .to_string();
        let type_name = field("type")?
            .as_str()
            .ok_or_else(|| {
                ReplaySourceError::Json(format!("line {}: 'type' not a string", lineno + 1))
            })?
            .to_string();
        let recv_ts_ns = field("recv_ts_ns")?.as_i64().ok_or_else(|| {
            ReplaySourceError::Json(format!("line {}: 'recv_ts_ns' not an int", lineno + 1))
        })?;
        let raw_hex = field("raw_cdr")?.as_str().ok_or_else(|| {
            ReplaySourceError::Json(format!("line {}: 'raw_cdr' not a string", lineno + 1))
        })?;
        let raw_cdr = from_hex(raw_hex).ok_or_else(|| {
            ReplaySourceError::Json(format!("line {}: bad raw_cdr hex", lineno + 1))
        })?;
        out.push(RecordedSample {
            topic,
            type_name,
            recv_ts_ns,
            raw_cdr,
        });
    }
    Ok(out)
}

/// Sanitise a topic name to its table-name form — must match
/// `sqlite_sink::sanitize` so the per-topic tables resolve.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Read all samples out of a decoded SQLite DB (as written by
/// [`crate::sqlite_sink::SqliteSink`]). Joins each topic's `t_<topic>` rows to
/// its `_types` row, ordered globally by `recv_ts_ns` so replay pacing matches
/// capture order across topics.
///
/// # Errors
/// [`ReplaySourceError::Db`].
pub fn from_sqlite(path: &str) -> R<Vec<RecordedSample>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| ReplaySourceError::Db(e.to_string()))?;

    // topic → type_name
    let topics: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT topic, type_name FROM _types")
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        rows.collect::<Result<_, _>>()
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?
    };

    let mut out = Vec::new();
    for (topic, type_name) in topics {
        let table = format!("t_{}", sanitize(&topic));
        let sql = format!("SELECT recv_ts_ns, raw_cdr FROM {table} ORDER BY sample_id");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        for row in rows {
            let (recv_ts_ns, raw_cdr) = row.map_err(|e| ReplaySourceError::Db(e.to_string()))?;
            out.push(RecordedSample {
                topic: topic.clone(),
                type_name: type_name.clone(),
                recv_ts_ns,
                raw_cdr,
            });
        }
    }
    out.sort_by_key(|s| s.recv_ts_ns);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::json_sink::sample_line;
    use crate::sqlite_sink::SqliteSink;
    use crate::type_source::dynamic_type_from_idl;
    use zerodds_types::dynamic::DynamicDataFactory;

    const IDL: &str = r#"
        module cuas {
            @final struct Track {
                unsigned long id;
                string name;
            };
        };
    "#;

    #[test]
    fn ndjson_roundtrip() {
        let raw = [0x07u8, 0x00, 0x00, 0x00, 0xde, 0xad];
        let l1 = sample_line(
            "Track",
            "cuas::Track",
            100,
            "aa",
            serde_json::json!({"id": 7}),
            &raw,
        );
        let l2 = sample_line(
            "Track",
            "cuas::Track",
            200,
            "bb",
            serde_json::json!({"id": 8}),
            &[0x08, 0x00, 0x00, 0x00],
        );
        let buf = format!("{l1}\n\n{l2}\n");
        let got = from_ndjson(std::io::Cursor::new(buf)).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].topic, "Track");
        assert_eq!(got[0].type_name, "cuas::Track");
        assert_eq!(got[0].recv_ts_ns, 100);
        assert_eq!(got[0].raw_cdr, raw);
    }

    #[test]
    fn sqlite_roundtrip() {
        let path = format!(
            "file:replay_src_test_{}?mode=memory&cache=shared",
            std::process::id()
        );
        // Hold one connection open so the shared in-memory DB survives.
        let keep = rusqlite::Connection::open(&path).unwrap();
        let ty = dynamic_type_from_idl(IDL, "Track").unwrap();
        {
            let mut sink = SqliteSink::open(&path).unwrap();
            sink.ensure_topic("Track", "cuas::Track", IDL, &ty).unwrap();
            let mut d = DynamicDataFactory::create_data(&ty).unwrap();
            d.set_uint32_value(0, 7).unwrap();
            d.set_string_value(1, "alpha").unwrap();
            let raw = [0x07u8, 0x00, 0x00, 0x00];
            sink.insert_sample("Track", &ty, 100, "aa", &raw, &d)
                .unwrap();
        }
        let got = from_sqlite(&path).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].topic, "Track");
        assert_eq!(got[0].type_name, "cuas::Track");
        assert_eq!(got[0].raw_cdr, [0x07, 0x00, 0x00, 0x00]);
        drop(keep);
    }
}
