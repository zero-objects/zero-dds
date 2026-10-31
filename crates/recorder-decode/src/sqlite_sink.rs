// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed per-topic relational SQLite sink for decoded samples.
//!
//! Schema (per topic `T`, names sanitised):
//! - `t_<T>` — one row per sample. Columns: `sample_id` (the record marker,
//!   PK), `recv_ts_ns`, `writer`, `raw_cdr` (BLOB, for byte-exact replay),
//!   plus one typed column per scalar top-level member; a 1:1 nested struct
//!   and a `bitset` are flattened to `field__sub` columns.
//! - `t_<T>__<member>` — a child table per 1:N member, joinable back to the
//!   parent by `sample_id`. A sequence or array of scalars gets `idx` +
//!   `value`; of structs, `idx` + one column per flattened element field; a
//!   `map` gets `idx` + `key` + `value`; a `bitmask` gets `idx` + `value`,
//!   one row per set bit, by name.
//! - `_types(topic, type_name, idl)` — full type data per topic.
//!
//! [`SqliteSink::helper_queries`] emits ready-to-run SQL snippets (SQLite has
//! no stored procedures).
//!
//! ## What is stored exactly, and where SQLite cannot
//!
//! SQLite's `INTEGER` is 64-bit signed and its `REAL` is an `f64`. Three IDL
//! values do not fit, and each is stored as `TEXT` holding its exact decimal
//! rather than as the nearest number:
//!
//! - a `uint64` above `2^63`. The previous version of this file cast it with
//!   `as i64`, which stored a negative number and reported success;
//! - a `fixed<P, S>`, which is a decimal — an `f64` makes `fixed<28,4>` a
//!   different value;
//! - a `long double`. It was declared `REAL` here, so 113 bits of significand
//!   were written into 53 and the loss was silent.
//!
//! SQLite columns are dynamically typed, so the exact text lands in the same
//! column as the numbers. A reader that finds text in a numeric column is
//! looking at a value no 64-bit column could hold, which is the one thing
//! worth knowing about it.
//!
//! Residual: a sequence or array whose element is itself 1:N (sequence,
//! array, map, bitmask) or a union, a `map` whose key or value is not scalar,
//! and a union member have no flat column shape and are skipped —
//! [`SqliteSink::skipped_members`] names each one rather than leaving the
//! table silently short.

use std::collections::BTreeSet;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params, params_from_iter};
use zerodds_types::dynamic::DynamicData;
use zerodds_types::dynamic::descriptor::TypeKind;
use zerodds_types::dynamic::type_::DynamicType;

/// SQLite sink error.
#[derive(Debug)]
pub enum SqliteError {
    /// Underlying rusqlite failure.
    Db(String),
    /// Unsupported construct for the relational schema.
    Unsupported(String),
    /// A file of the database could not be removed.
    Io(String),
}
impl core::fmt::Display for SqliteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Db(m) => write!(f, "sqlite: {m}"),
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}
impl std::error::Error for SqliteError {}
impl From<rusqlite::Error> for SqliteError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.to_string())
    }
}
type R<T> = Result<T, SqliteError>;

/// A scalar column with the member path to navigate during insert.
///
/// The path is by **name**. A `@mutable` type reaches the reader in the order
/// the writer chose, so a positional path would read the right octets into
/// the wrong column whenever two peers ordered their members differently.
struct Col {
    name: String,
    sql_ty: &'static str,
    path: Vec<String>,
}

/// What a 1:N child table holds per row.
enum ChildShape {
    /// A single `value` column of this SQL type.
    Scalar(&'static str),
    /// One column per flattened field of a composite element.
    Composite(Vec<Col>),
    /// A `key` and a `value` column — a `map`.
    KeyValue(&'static str, &'static str),
}

/// A 1:N child (a member → child table joined by `sample_id`).
struct Child {
    member: String,
    shape: ChildShape,
}

/// Typed per-topic SQLite sink.
pub struct SqliteSink {
    conn: Connection,
    ensured: BTreeSet<String>,
    skipped: Vec<String>,
}

impl SqliteSink {
    /// Open (or create) a SQLite database at `path`.
    ///
    /// # Errors
    /// [`SqliteError::Db`] on open failure.
    pub fn open(path: &str) -> R<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Create an empty database at `path`, replacing one that is there.
    ///
    /// A recording's `--out-json` and `-o` files are created with
    /// `File::create`, which truncates. [`open`](Self::open) appends: its
    /// tables are `CREATE TABLE IF NOT EXISTS`, so a second recording to the
    /// same path added its rows to the first one's, and the file held two
    /// captures as one. Replaying it paced the second capture's samples
    /// behind the first's by the hours between the two recordings.
    ///
    /// The `-wal` and `-shm` files of the old database go with it; a WAL
    /// left behind would be replayed into the new file on open.
    ///
    /// # Errors
    /// [`SqliteError::Io`] when a file exists and cannot be removed,
    /// [`SqliteError::Db`] on open failure.
    pub fn create(path: &str) -> R<Self> {
        for file in [
            path.to_string(),
            format!("{path}-wal"),
            format!("{path}-shm"),
        ] {
            match std::fs::remove_file(&file) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(SqliteError::Io(format!("remove {file}: {e}"))),
            }
        }
        Self::open(path)
    }

    /// In-memory database (tests).
    ///
    /// # Errors
    /// [`SqliteError::Db`].
    pub fn open_in_memory() -> R<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> R<Self> {
        // Write-ahead logging, and `synchronous = NORMAL` with it.
        //
        // In the default rollback-journal mode with `synchronous =
        // FULL`, every commit waits for an `fsync`; on the Linux test host's ZFS
        // that is 374 ms, which is slower than a 10 Hz writer publishes
        // and makes the recorder useless for anything but the slowest
        // traffic. Under WAL, `NORMAL` fsyncs at a checkpoint rather
        // than at every commit.
        //
        // What that costs is stated plainly: a committed transaction
        // survives a crash of this process, and may be lost if the
        // machine loses power. For a capture of live traffic that is
        // the right side of the trade — the alternative is a file that
        // is durable and missing nine tenths of the samples. A capture
        // that has to survive power loss belongs on a filesystem that
        // says so, not on a per-sample fsync.
        //
        // Both pragmas are no-ops on an in-memory database, which is
        // what `open_in_memory` hands in.
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _types (topic TEXT PRIMARY KEY, type_name TEXT, idl TEXT);",
        )?;
        Ok(Self {
            conn,
            ensured: BTreeSet::new(),
            skipped: Vec::new(),
        })
    }

    /// Borrow the connection (queries / tests).
    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Every member no table was created for, as `topic::member`, in the
    /// order they were met.
    ///
    /// A schema that is short by one member and says nothing is the failure
    /// this exists to prevent: the query returns rows, they are simply
    /// missing a column that was never mentioned.
    #[must_use]
    pub fn skipped_members(&self) -> &[String] {
        &self.skipped
    }

    /// Create the per-topic tables for `ty` (idempotent) and record its type.
    ///
    /// # Errors
    /// [`SqliteError`].
    pub fn ensure_topic(
        &mut self,
        topic: &str,
        type_name: &str,
        idl: &str,
        ty: &DynamicType,
    ) -> R<()> {
        if !self.ensured.insert(topic.to_string()) {
            return Ok(());
        }
        let main = main_table(topic);
        let cols = scalar_columns(ty, "", &[]);
        let mut ddl = format!(
            "CREATE TABLE IF NOT EXISTS {main} (\n  \
             sample_id INTEGER PRIMARY KEY AUTOINCREMENT,\n  \
             recv_ts_ns INTEGER,\n  writer TEXT,\n  raw_cdr BLOB"
        );
        for c in &cols {
            ddl.push_str(&format!(",\n  {} {}", c.name, c.sql_ty));
        }
        ddl.push_str("\n);");
        self.conn.execute_batch(&ddl)?;

        let mut skipped = Vec::new();
        for ch in children(ty, &mut skipped) {
            let ct = child_table(topic, &ch.member);
            let ddl = match &ch.shape {
                ChildShape::Scalar(vt) => format!(
                    "CREATE TABLE IF NOT EXISTS {ct} (\n  \
                     sample_id INTEGER,\n  idx INTEGER,\n  value {vt},\n  \
                     PRIMARY KEY (sample_id, idx)\n);"
                ),
                ChildShape::KeyValue(kt, vt) => format!(
                    "CREATE TABLE IF NOT EXISTS {ct} (\n  \
                     sample_id INTEGER,\n  idx INTEGER,\n  key {kt},\n  value {vt},\n  \
                     PRIMARY KEY (sample_id, idx)\n);"
                ),
                ChildShape::Composite(element_cols) => {
                    let mut s = format!(
                        "CREATE TABLE IF NOT EXISTS {ct} (\n  sample_id INTEGER,\n  idx INTEGER"
                    );
                    for c in element_cols {
                        s.push_str(&format!(",\n  {} {}", c.name, c.sql_ty));
                    }
                    s.push_str(",\n  PRIMARY KEY (sample_id, idx)\n);");
                    s
                }
            };
            self.conn.execute_batch(&ddl)?;
        }
        self.skipped
            .extend(skipped.into_iter().map(|m| format!("{topic}::{m}")));

        self.conn.execute(
            "INSERT OR REPLACE INTO _types(topic, type_name, idl) VALUES (?1, ?2, ?3)",
            params![topic, type_name, idl],
        )?;
        Ok(())
    }

    /// Insert one decoded sample. Returns the new `sample_id` (record marker).
    ///
    /// # Errors
    /// [`SqliteError`].
    pub fn insert_sample(
        &mut self,
        topic: &str,
        ty: &DynamicType,
        recv_ts_ns: i64,
        writer_hex: &str,
        raw_cdr: &[u8],
        data: &DynamicData,
    ) -> R<i64> {
        // One transaction for the whole sample. Without it every
        // statement below commits on its own, and a sample with a
        // three-element sequence costs four commits — four `fsync`s.
        // Measured on the Linux test host, whose `/tmp` is on ZFS: 374 ms per
        // autocommit insert, so about 1.5 s for one such sample, and
        // `zerodds-record` wrote four samples where a 10 Hz writer sent
        // fifty. The transaction is also the more honest shape: a
        // sample and its child rows belong together, and a failure
        // halfway through used to leave the parent row behind with some
        // of its children missing.
        self.conn.execute_batch("BEGIN")?;
        let result = self.insert_sample_committed(topic, ty, recv_ts_ns, writer_hex, raw_cdr, data);
        if result.is_ok() {
            self.conn.execute_batch("COMMIT")?;
        } else {
            // The rollback's own failure is not reported: it would
            // replace the error that actually happened with a less
            // informative one.
            let _ = self.conn.execute_batch("ROLLBACK");
        }
        result
    }

    fn insert_sample_committed(
        &mut self,
        topic: &str,
        ty: &DynamicType,
        recv_ts_ns: i64,
        writer_hex: &str,
        raw_cdr: &[u8],
        data: &DynamicData,
    ) -> R<i64> {
        let main = main_table(topic);
        let cols = scalar_columns(ty, "", &[]);

        let mut names = vec![
            "recv_ts_ns".to_string(),
            "writer".to_string(),
            "raw_cdr".to_string(),
        ];
        let mut vals: Vec<SqlValue> = vec![
            SqlValue::Integer(recv_ts_ns),
            SqlValue::Text(writer_hex.to_string()),
            SqlValue::Blob(raw_cdr.to_vec()),
        ];
        for c in &cols {
            names.push(c.name.clone());
            vals.push(navigate(data, &c.path).map_or(SqlValue::Null, scalar_param));
        }
        let placeholders = (1..=vals.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {main} ({}) VALUES ({placeholders})",
            names.join(", ")
        );
        self.conn.execute(&sql, params_from_iter(vals.iter()))?;
        let sample_id = self.conn.last_insert_rowid();

        for ch in children(ty, &mut Vec::new()) {
            let Some(items) = data.member(&ch.member).and_then(collection_items) else {
                continue;
            };
            let ct = child_table(topic, &ch.member);
            match &ch.shape {
                ChildShape::Scalar(_) => {
                    let mut stmt = self.conn.prepare_cached(&format!(
                        "INSERT INTO {ct}(sample_id, idx, value) VALUES (?1,?2,?3)"
                    ))?;
                    for (idx, el) in items.iter().enumerate() {
                        stmt.execute(params![sample_id, idx as i64, scalar_param(el)])?;
                    }
                }
                ChildShape::KeyValue(_, _) => {
                    let mut stmt = self.conn.prepare_cached(&format!(
                        "INSERT INTO {ct}(sample_id, idx, key, value) VALUES (?1,?2,?3,?4)"
                    ))?;
                    for (idx, entry) in items.iter().enumerate() {
                        // A map entry is a two-element collection, `[key, value]`.
                        let Some(pair) = collection_items(entry) else {
                            continue;
                        };
                        let (Some(k), Some(v)) = (pair.first(), pair.get(1)) else {
                            continue;
                        };
                        stmt.execute(params![
                            sample_id,
                            idx as i64,
                            scalar_param(k),
                            scalar_param(v)
                        ])?;
                    }
                }
                ChildShape::Composite(element_cols) => {
                    let mut names = vec!["sample_id".to_string(), "idx".to_string()];
                    names.extend(element_cols.iter().map(|c| c.name.clone()));
                    let placeholders = (1..=names.len())
                        .map(|i| format!("?{i}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let sql = format!(
                        "INSERT INTO {ct} ({}) VALUES ({placeholders})",
                        names.join(", ")
                    );
                    for (idx, el) in items.iter().enumerate() {
                        let mut vals: Vec<SqlValue> =
                            vec![SqlValue::Integer(sample_id), SqlValue::Integer(idx as i64)];
                        for c in element_cols {
                            vals.push(navigate(el, &c.path).map_or(SqlValue::Null, scalar_param));
                        }
                        self.conn.execute(&sql, params_from_iter(vals.iter()))?;
                    }
                }
            }
        }
        Ok(sample_id)
    }

    /// Ready-to-run SQL helper snippets for `topic` (SQLite has no procedures).
    #[must_use]
    pub fn helper_queries(topic: &str) -> String {
        let m = main_table(topic);
        format!(
            "-- nth record (0-based):\nSELECT * FROM {m} ORDER BY sample_id LIMIT 1 OFFSET :n;\n\n\
             -- records where a property matches:\nSELECT * FROM {m} WHERE :column = :value;\n\n\
             -- full record by join (replace <child> per 1:N member):\n\
             SELECT p.*, c.idx, c.value FROM {m} p\n  \
             LEFT JOIN {m}__<child> c ON c.sample_id = p.sample_id\n  \
             WHERE p.sample_id = :sample_id ORDER BY c.idx;\n"
        )
    }
}

// --- schema walk ---

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
fn main_table(topic: &str) -> String {
    format!("t_{}", sanitize(topic))
}
fn child_table(topic: &str, member: &str) -> String {
    format!("t_{}__{}", sanitize(topic), sanitize(member))
}

/// The SQL type a value of this kind is stored in.
///
/// `Float128` and `Fixed` are `TEXT` and not `REAL`: both hold more than an
/// `f64`, and a column that rounds them would do it silently. An enumerator
/// is `TEXT` because it is stored by name.
const fn sql_type(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Float32 | TypeKind::Float64 => "REAL",
        TypeKind::Float128
        | TypeKind::String8
        | TypeKind::String16
        | TypeKind::Char8
        | TypeKind::Char16
        | TypeKind::Enumeration => "TEXT",
        _ => "INTEGER",
    }
}

/// The kind a member is stored as, with any alias resolved.
fn effective(ty: &DynamicType) -> DynamicType {
    ty.resolve_alias().unwrap_or_else(|_| ty.clone())
}

/// Flatten the type's scalar members (incl. 1:1 nested structs and bitsets)
/// into columns.
/// zerodds-lint: recursion-depth 64 (walks the type; bounded by type nesting).
fn scalar_columns(ty: &DynamicType, prefix: &str, path: &[String]) -> Vec<Col> {
    let mut out = Vec::new();
    for m in ty.members() {
        let Ok(mt) = m.dynamic_type() else { continue };
        let mt = effective(&mt);
        let mut p = path.to_vec();
        p.push(m.name().to_string());
        let col = if prefix.is_empty() {
            sanitize(m.name())
        } else {
            format!("{prefix}__{}", sanitize(m.name()))
        };
        match mt.kind() {
            // A bitset's fields are named and fixed in number, so they
            // flatten exactly as a nested struct does.
            TypeKind::Structure | TypeKind::Bitset => out.extend(scalar_columns(&mt, &col, &p)),
            // 1:N or no flat shape — handled by `children`, or skipped there.
            TypeKind::Sequence
            | TypeKind::Array
            | TypeKind::Map
            | TypeKind::Bitmask
            | TypeKind::Union => {}
            k => out.push(Col {
                name: col,
                sql_ty: sql_type(k),
                path: p,
            }),
        }
    }
    out
}

/// The 1:N members: sequences, arrays, maps and bitmasks.
///
/// `skipped` collects the members no table shape fits, by name.
fn children(ty: &DynamicType, skipped: &mut Vec<String>) -> Vec<Child> {
    let mut out = Vec::new();
    for m in ty.members() {
        let Ok(mt) = m.dynamic_type() else { continue };
        let mt = effective(&mt);
        let member = m.name().to_string();
        match mt.kind() {
            TypeKind::Sequence | TypeKind::Array => {
                let Some(elem) = mt.element_type().map(|e| effective(&e)) else {
                    skipped.push(member);
                    continue;
                };
                match elem.kind() {
                    TypeKind::Structure | TypeKind::Bitset => out.push(Child {
                        member,
                        shape: ChildShape::Composite(scalar_columns(&elem, "", &[])),
                    }),
                    // An element that is itself 1:N, or a union, has no flat
                    // row shape; a nested child table keyed by two indices is
                    // a schema this sink does not build.
                    TypeKind::Sequence
                    | TypeKind::Array
                    | TypeKind::Map
                    | TypeKind::Bitmask
                    | TypeKind::Union => skipped.push(member),
                    k => out.push(Child {
                        member,
                        shape: ChildShape::Scalar(sql_type(k)),
                    }),
                }
            }
            TypeKind::Map => {
                let key = mt.key_element_type().map(|e| effective(&e));
                let val = mt.element_type().map(|e| effective(&e));
                match (key, val) {
                    (Some(k), Some(v)) if is_scalar(k.kind()) && is_scalar(v.kind()) => {
                        out.push(Child {
                            member,
                            shape: ChildShape::KeyValue(sql_type(k.kind()), sql_type(v.kind())),
                        });
                    }
                    _ => skipped.push(member),
                }
            }
            // A bitmask is the set of bits it sets, by name — one row each.
            TypeKind::Bitmask => out.push(Child {
                member,
                shape: ChildShape::Scalar("TEXT"),
            }),
            TypeKind::Union => skipped.push(member),
            _ => {}
        }
    }
    out
}

const fn is_scalar(kind: TypeKind) -> bool {
    !matches!(
        kind,
        TypeKind::Structure
            | TypeKind::Union
            | TypeKind::Sequence
            | TypeKind::Array
            | TypeKind::Map
            | TypeKind::Bitset
            | TypeKind::Bitmask
            | TypeKind::NoType
    )
}

/// The elements of a collection, a bitmask's set bits, or nothing.
fn collection_items(d: &DynamicData) -> Option<&[DynamicData]> {
    match d {
        DynamicData::Collection(items) | DynamicData::Bits(items) => Some(items),
        _ => None,
    }
}

/// Navigate a member name path into nested `DynamicData` to the leaf value.
fn navigate<'a>(data: &'a DynamicData, path: &[String]) -> Option<&'a DynamicData> {
    let mut cur = data;
    for name in path {
        cur = match cur {
            DynamicData::Aggregate(fields) | DynamicData::Bitfields(fields) => {
                fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// A leaf value as a SQLite parameter.
///
/// Everything a 64-bit column cannot hold exactly becomes its decimal text —
/// see the module docs.
fn scalar_param(v: &DynamicData) -> SqlValue {
    match v {
        DynamicData::Boolean(b) => SqlValue::Integer(i64::from(*b)),
        DynamicData::Integer { value, .. } => i64::try_from(*value)
            .map_or_else(|_| SqlValue::Text(value.to_string()), SqlValue::Integer),
        DynamicData::Float { value, .. } => SqlValue::Real(*value),
        DynamicData::LongDouble(x) => SqlValue::Text(x.to_string()),
        DynamicData::Character { value, .. }
        | DynamicData::Text(value)
        | DynamicData::Fixed(value)
        | DynamicData::Enumerator(value) => SqlValue::Text(value.clone()),
        _ => SqlValue::Null,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::type_source::TypeBook;
    use zerodds_ast_layout::key::{Encoding, Endianness};
    use zerodds_wire_codec::{ProbeRequest, decode, encode};

    const IDL: &str = r"
        module cuas {
            @final struct Inner { long a; };
            @final struct Track {
                unsigned long id;
                string name;
                Inner nested;
                sequence<long> xs;
            };
        };
    ";

    fn int(value: i128) -> DynamicData {
        DynamicData::Integer { kind: None, value }
    }

    /// Build the value, put it on the wire, and read it back — the table is
    /// filled from what a peer would have sent, not from the tree that was
    /// constructed here.
    fn decoded(book: &TypeBook, type_name: &str, value: &DynamicData) -> DynamicData {
        let request = ProbeRequest::new(type_name, Encoding::DelimitedCdr, Endianness::Little);
        let bytes = encode(book.ast(), &request, value).expect("encode");
        decode(book.ast(), &request, &bytes).expect("decode")
    }

    #[test]
    fn per_topic_schema_insert_and_query() {
        let book = TypeBook::from_idl(IDL).expect("idl");
        let ty = book.resolve("cuas::Track").expect("type");
        let value = DynamicData::Aggregate(vec![
            (String::from("id"), int(7)),
            (
                String::from("name"),
                DynamicData::Text(String::from("alpha")),
            ),
            (
                String::from("nested"),
                DynamicData::Aggregate(vec![(String::from("a"), int(42))]),
            ),
            (
                String::from("xs"),
                DynamicData::Collection(vec![int(10), int(20), int(30)]),
            ),
        ]);
        let d = decoded(&book, "cuas::Track", &value);

        let mut sink = SqliteSink::open_in_memory().expect("db");
        sink.ensure_topic("Track", "cuas::Track", IDL, &ty)
            .expect("schema");
        let sid = sink
            .insert_sample("Track", &ty, 1_700, "aabbccdd", &[0xde, 0xad], &d)
            .expect("insert");
        assert_eq!(sid, 1);

        let (id, name, inner_a): (i64, String, i64) = sink
            .conn()
            .query_row(
                "SELECT id, name, nested__a FROM t_Track WHERE sample_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("main row");
        assert_eq!((id, name.as_str(), inner_a), (7, "alpha", 42));

        let raw: Vec<u8> = sink
            .conn()
            .query_row("SELECT raw_cdr FROM t_Track WHERE sample_id=1", [], |r| {
                r.get(0)
            })
            .expect("raw");
        assert_eq!(raw, vec![0xde, 0xad]);

        let xs: Vec<i64> = sink
            .conn()
            .prepare("SELECT value FROM t_Track__xs WHERE sample_id=1 ORDER BY idx")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(xs, vec![10, 20, 30]);

        let stored_idl: String = sink
            .conn()
            .query_row("SELECT idl FROM _types WHERE topic='Track'", [], |r| {
                r.get(0)
            })
            .expect("types row");
        assert!(stored_idl.contains("struct Track"));

        sink.ensure_topic("Track", "cuas::Track", IDL, &ty)
            .expect("idempotent");
        assert!(sink.skipped_members().is_empty());
    }

    /// A `uint64` above `2^63` used to be written with `as i64` and landed as
    /// a negative number. It is now the exact decimal.
    #[test]
    fn an_unsigned_64_bit_value_is_not_wrapped_into_a_negative() {
        let big = i128::from(u64::MAX);
        assert_eq!(scalar_param(&int(big)), SqlValue::Text(big.to_string()));
        // What still fits stays a number.
        assert_eq!(scalar_param(&int(9)), SqlValue::Integer(9));
    }

    /// `long double` was declared `REAL`, so 113 bits of significand were
    /// written into 53.
    #[test]
    fn a_long_double_is_stored_as_text_not_as_a_double() {
        assert_eq!(sql_type(TypeKind::Float128), "TEXT");
        assert_eq!(sql_type(TypeKind::Float64), "REAL");
    }

    /// A map gets `key` and `value` columns, and a bitmask one row per set
    /// bit. Neither had a table before.
    #[test]
    fn a_map_and_a_bitmask_each_get_a_child_table() {
        const M: &str = r"
            module m {
                @bit_bound(8) bitmask Marks { FLAG_A, FLAG_B };
                @final struct S {
                    map<string, long> by_name;
                    Marks flags;
                };
            };
        ";
        let book = TypeBook::from_idl(M).expect("idl");
        let ty = book.resolve("m::S").expect("type");
        let mut sink = SqliteSink::open_in_memory().expect("db");
        sink.ensure_topic("S", "m::S", M, &ty).expect("schema");

        let value = DynamicData::Aggregate(vec![
            (
                String::from("by_name"),
                DynamicData::Collection(vec![DynamicData::Collection(vec![
                    DynamicData::Text(String::from("one")),
                    int(1),
                ])]),
            ),
            (
                String::from("flags"),
                DynamicData::Bits(vec![DynamicData::Text(String::from("FLAG_B"))]),
            ),
        ]);
        let d = decoded(&book, "m::S", &value);
        sink.insert_sample("S", &ty, 1, "00", &[], &d)
            .expect("insert");

        let (k, v): (String, i64) = sink
            .conn()
            .query_row("SELECT key, value FROM t_S__by_name WHERE idx=0", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("map row");
        assert_eq!((k.as_str(), v), ("one", 1));

        let bits: Vec<String> = sink
            .conn()
            .prepare("SELECT value FROM t_S__flags ORDER BY idx")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(bits, vec![String::from("FLAG_B")]);
        assert!(sink.skipped_members().is_empty());
    }

    /// A member with no flat row shape is named rather than silently absent.
    #[test]
    fn a_skipped_member_is_reported() {
        const N: &str = r"
            module m {
                @final struct S {
                    sequence<sequence<long> > nested;
                };
            };
        ";
        let book = TypeBook::from_idl(N).expect("idl");
        let ty = book.resolve("m::S").expect("type");
        let mut sink = SqliteSink::open_in_memory().expect("db");
        sink.ensure_topic("S", "m::S", N, &ty).expect("schema");
        assert_eq!(sink.skipped_members(), [String::from("S::nested")]);
    }

    /// A member path is followed by name, so a `@mutable` type whose members
    /// arrive in the writer's order still fills the right columns.
    #[test]
    fn navigation_is_by_name_and_not_by_position() {
        let d = DynamicData::Aggregate(vec![
            (String::from("b"), int(2)),
            (String::from("a"), int(1)),
        ]);
        assert_eq!(
            navigate(&d, &[String::from("a")]),
            Some(&int(1)),
            "the second field named `a` is the one `a` means"
        );
    }
}
