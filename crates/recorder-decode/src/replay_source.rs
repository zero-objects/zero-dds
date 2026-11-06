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
    /// The representation identifier of the encapsulation header `raw_cdr`
    /// came with (DDSI-RTPS 2.5 §10.5); `None` in a capture made before it
    /// was kept.
    pub representation_identifier: Option<[u8; 2]>,
}

/// Two octets from a hex string, as the sinks write the identifier.
fn identifier_from_hex(s: &str) -> Option<[u8; 2]> {
    let bytes = from_hex(s)?;
    <[u8; 2]>::try_from(bytes.as_slice()).ok()
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
        let representation_identifier = match v.get("representation_identifier") {
            None => None,
            Some(id) => Some(id.as_str().and_then(identifier_from_hex).ok_or_else(|| {
                ReplaySourceError::Json(format!(
                    "line {}: 'representation_identifier' is not two octets in hex",
                    lineno + 1
                ))
            })?),
        };
        out.push(RecordedSample {
            topic,
            type_name,
            recv_ts_ns,
            raw_cdr,
            representation_identifier,
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
        // A table written before the column existed has none.
        let has_identifier = conn
            .prepare(&format!(
                "SELECT 1 FROM pragma_table_info('{table}') WHERE name = 'representation_identifier'"
            ))
            .and_then(|mut s| s.exists([]))
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        let sql = if has_identifier {
            format!(
                "SELECT recv_ts_ns, raw_cdr, representation_identifier FROM {table} ORDER BY sample_id"
            )
        } else {
            format!("SELECT recv_ts_ns, raw_cdr, NULL FROM {table} ORDER BY sample_id")
        };
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .map_err(|e| ReplaySourceError::Db(e.to_string()))?;
        for row in rows {
            let (recv_ts_ns, raw_cdr, id) =
                row.map_err(|e| ReplaySourceError::Db(e.to_string()))?;
            let representation_identifier = match id {
                None => None,
                Some(id) => Some(<[u8; 2]>::try_from(id.as_slice()).map_err(|_| {
                    ReplaySourceError::Db(format!(
                        "{table}: a representation_identifier of {} octets",
                        id.len()
                    ))
                })?),
            };
            out.push(RecordedSample {
                topic: topic.clone(),
                type_name: type_name.clone(),
                recv_ts_ns,
                raw_cdr,
                representation_identifier,
            });
        }
    }
    out.sort_by_key(|s| s.recv_ts_ns);
    Ok(out)
}
