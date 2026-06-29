// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed per-topic relational SQLite sink for decoded samples.
//!
//! Schema (per topic `T`, names sanitised):
//! - `t_<T>` — one row per sample. Columns: `sample_id` (the record marker,
//!   PK), `recv_ts_ns`, `writer`, `raw_cdr` (BLOB, for byte-exact replay), plus
//!   one typed column per scalar top-level member; a 1:1 nested struct is
//!   flattened to `field__sub` columns.
//! - `t_<T>__<member>` — a child table per sequence/array member (1:N), with
//!   `sample_id` (FK) + `idx` + a typed `value` column, joinable back to the
//!   parent by `sample_id` (the record marker).
//! - `_types(topic, type_name, idl)` — full type data per topic.
//!
//! [`SqliteSink::helper_queries`] emits ready-to-run SQL snippets (SQLite has no
//! stored procedures): "record by property", "nth record", "full record by
//! join". Residual: composite-element sequences (the codec residual) are skipped.

use std::collections::BTreeSet;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params, params_from_iter};
use zerodds_types::dynamic::collection;
use zerodds_types::dynamic::data::{DynamicData, DynamicValue};
use zerodds_types::dynamic::descriptor::TypeKind;
use zerodds_types::dynamic::type_::DynamicType;

/// SQLite sink error.
#[derive(Debug)]
pub enum SqliteError {
    /// Underlying rusqlite failure.
    Db(String),
    /// Unsupported construct for the relational schema.
    Unsupported(String),
}
impl core::fmt::Display for SqliteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Db(m) => write!(f, "sqlite: {m}"),
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
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
struct Col {
    name: String,
    sql_ty: &'static str,
    path: Vec<u32>,
}

/// A 1:N child (sequence/array member → child table joined by `sample_id`).
struct SeqChild {
    member: String,
    member_id: u32,
    /// Scalar element → `Some(sql_ty)` for the single `value` column.
    /// Composite element → `None`; the element's flattened columns are in
    /// [`SeqChild::element_cols`].
    value_sql_ty: Option<&'static str>,
    /// Flattened scalar columns of a composite element (empty for scalar).
    element_cols: Vec<Col>,
}

/// Typed per-topic SQLite sink.
pub struct SqliteSink {
    conn: Connection,
    ensured: BTreeSet<String>,
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

    /// In-memory database (tests).
    ///
    /// # Errors
    /// [`SqliteError::Db`].
    pub fn open_in_memory() -> R<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> R<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _types (topic TEXT PRIMARY KEY, type_name TEXT, idl TEXT);",
        )?;
        Ok(Self {
            conn,
            ensured: BTreeSet::new(),
        })
    }

    /// Borrow the connection (queries / tests).
    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
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

        for ch in seq_children(ty) {
            let ct = child_table(topic, &ch.member);
            let ddl = if let Some(vt) = ch.value_sql_ty {
                format!(
                    "CREATE TABLE IF NOT EXISTS {ct} (\n  \
                     sample_id INTEGER,\n  idx INTEGER,\n  value {vt},\n  \
                     PRIMARY KEY (sample_id, idx)\n);"
                )
            } else {
                // Composite element → one column per flattened element field.
                let mut s = format!(
                    "CREATE TABLE IF NOT EXISTS {ct} (\n  sample_id INTEGER,\n  idx INTEGER"
                );
                for c in &ch.element_cols {
                    s.push_str(&format!(",\n  {} {}", c.name, c.sql_ty));
                }
                s.push_str(",\n  PRIMARY KEY (sample_id, idx)\n);");
                s
            };
            self.conn.execute_batch(&ddl)?;
        }
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

        for ch in seq_children(ty) {
            let Some(DynamicValue::Sequence(items)) = data.get_value(ch.member_id) else {
                continue;
            };
            let ct = child_table(topic, &ch.member);
            if ch.value_sql_ty.is_some() {
                // Scalar element → single `value` column.
                let mut stmt = self.conn.prepare_cached(&format!(
                    "INSERT INTO {ct}(sample_id, idx, value) VALUES (?1,?2,?3)"
                ))?;
                for (idx, el) in items.iter().enumerate() {
                    let v = el.get_value(0).map_or(SqlValue::Null, scalar_param);
                    stmt.execute(params![sample_id, idx as i64, v])?;
                }
            } else {
                // Composite element → one column per flattened element field.
                let mut names = vec!["sample_id".to_string(), "idx".to_string()];
                names.extend(ch.element_cols.iter().map(|c| c.name.clone()));
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
                    for c in &ch.element_cols {
                        vals.push(navigate(el, &c.path).map_or(SqlValue::Null, scalar_param));
                    }
                    self.conn.execute(&sql, params_from_iter(vals.iter()))?;
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
             -- full record by join (replace <child> per sequence member):\n\
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

const fn sql_type(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Float32 | TypeKind::Float64 | TypeKind::Float128 => "REAL",
        TypeKind::String8 | TypeKind::String16 => "TEXT",
        _ => "INTEGER",
    }
}

/// Flatten the type's scalar members (incl. 1:1 nested structs) into columns.
/// zerodds-lint: recursion-depth 64 (walks decoded DynamicData; bounded by type nesting).
fn scalar_columns(ty: &DynamicType, prefix: &str, path: &[u32]) -> Vec<Col> {
    let mut out = Vec::new();
    for m in ty.members() {
        let mt = m.dynamic_type();
        let mut p = path.to_vec();
        p.push(m.id());
        let col = if prefix.is_empty() {
            sanitize(m.name())
        } else {
            format!("{prefix}__{}", sanitize(m.name()))
        };
        match mt.kind() {
            TypeKind::Structure => out.extend(scalar_columns(mt, &col, &p)),
            TypeKind::Sequence | TypeKind::Array | TypeKind::Union => {} // child table / unsupported
            k => out.push(Col {
                name: col,
                sql_ty: sql_type(k),
                path: p,
            }),
        }
    }
    out
}

/// Sequence/array members → child tables. Scalar elements get a single `value`
/// column; struct elements get one column per flattened element field (joined by
/// `sample_id` + `idx`). Union elements are skipped (no flat column shape).
fn seq_children(ty: &DynamicType) -> Vec<SeqChild> {
    let mut out = Vec::new();
    for m in ty.members() {
        let mt = m.dynamic_type();
        if !matches!(mt.kind(), TypeKind::Sequence | TypeKind::Array) {
            continue;
        }
        let Some(elem) = mt.descriptor().element_type.as_ref() else {
            continue;
        };
        match elem.kind {
            TypeKind::Structure => {
                if let Some(et) = collection::resolved_element(mt) {
                    out.push(SeqChild {
                        member: m.name().to_string(),
                        member_id: m.id(),
                        value_sql_ty: None,
                        element_cols: scalar_columns(et, "", &[]),
                    });
                }
            }
            TypeKind::Union => {} // no flat column shape
            k => out.push(SeqChild {
                member: m.name().to_string(),
                member_id: m.id(),
                value_sql_ty: Some(sql_type(k)),
                element_cols: Vec::new(),
            }),
        }
    }
    out
}

/// Navigate a member `path` into nested `DynamicData` to the leaf scalar value.
fn navigate<'a>(data: &'a DynamicData, path: &[u32]) -> Option<&'a DynamicValue> {
    let (&first, rest) = path.split_first()?;
    let mut cur = data.get_value(first)?;
    for &id in rest {
        match cur {
            DynamicValue::Complex(d) => cur = d.get_value(id)?,
            _ => return None,
        }
    }
    Some(cur)
}

fn scalar_param(v: &DynamicValue) -> SqlValue {
    match v {
        DynamicValue::Bool(b) => SqlValue::Integer(i64::from(*b)),
        DynamicValue::Byte(x) | DynamicValue::UInt8(x) | DynamicValue::Char8(x) => {
            SqlValue::Integer(i64::from(*x))
        }
        DynamicValue::Int8(x) => SqlValue::Integer(i64::from(*x)),
        DynamicValue::Int16(x) => SqlValue::Integer(i64::from(*x)),
        DynamicValue::UInt16(x) | DynamicValue::Char16(x) => SqlValue::Integer(i64::from(*x)),
        DynamicValue::Int32(x) => SqlValue::Integer(i64::from(*x)),
        DynamicValue::UInt32(x) => SqlValue::Integer(i64::from(*x)),
        DynamicValue::Int64(x) => SqlValue::Integer(*x),
        DynamicValue::UInt64(x) => SqlValue::Integer(*x as i64),
        DynamicValue::Float32(x) => SqlValue::Real(f64::from(*x)),
        DynamicValue::Float64(x) => SqlValue::Real(*x),
        DynamicValue::String(s) => SqlValue::Text(s.clone()),
        DynamicValue::WString(u) => SqlValue::Text(String::from_utf16_lossy(u)),
        DynamicValue::Complex(_) | DynamicValue::Sequence(_) | DynamicValue::None => SqlValue::Null,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::type_source::dynamic_type_from_idl;
    use zerodds_types::dynamic::{DynamicDataFactory, DynamicValue as DV};

    const IDL: &str = r#"
        module cuas {
            @final struct Inner { long a; };
            @final struct Track {
                unsigned long id;
                string name;
                Inner inner;
                sequence<long> xs;
            };
        };
    "#;

    fn sample(ty: &DynamicType) -> DynamicData {
        let mut d = DynamicDataFactory::create_data(ty).unwrap();
        d.set_uint32_value(0, 7).unwrap();
        d.set_string_value(1, "alpha").unwrap();
        let inner_ty = ty.member_by_index(2).unwrap().dynamic_type().clone();
        let mut inner = DynamicDataFactory::create_data(&inner_ty).unwrap();
        inner.set_int32_value(0, 42).unwrap();
        d.set_complex_value(2, inner).unwrap();
        let mut xs = Vec::new();
        for v in [10_i32, 20, 30] {
            let mut e =
                DynamicDataFactory::create_data(&DynamicType::new_primitive(TypeKind::Int32))
                    .unwrap();
            e.set_value_raw(0, DV::Int32(v));
            xs.push(e);
        }
        d.set_sequence_value(3, xs).unwrap();
        d
    }

    #[test]
    fn per_topic_schema_insert_and_query() {
        let ty = dynamic_type_from_idl(IDL, "Track").unwrap();
        let mut sink = SqliteSink::open_in_memory().unwrap();
        sink.ensure_topic("Track", "cuas::Track", IDL, &ty).unwrap();
        let d = sample(&ty);
        let sid = sink
            .insert_sample("Track", &ty, 1_700, "aabbccdd", &[0xde, 0xad], &d)
            .unwrap();
        assert_eq!(sid, 1);

        // main row: scalars + flattened nested inner__a
        let (id, name, inner_a): (i64, String, i64) = sink
            .conn()
            .query_row(
                "SELECT id, name, inner__a FROM t_Track WHERE sample_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((id, name.as_str(), inner_a), (7, "alpha", 42));

        // raw_cdr stored for replay
        let raw: Vec<u8> = sink
            .conn()
            .query_row("SELECT raw_cdr FROM t_Track WHERE sample_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(raw, vec![0xde, 0xad]);

        // child table joinable by sample_id (the record marker)
        let xs: Vec<i64> = sink
            .conn()
            .prepare("SELECT value FROM t_Track__xs WHERE sample_id=1 ORDER BY idx")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(xs, vec![10, 20, 30]);

        // _types carries the full IDL
        let stored_idl: String = sink
            .conn()
            .query_row("SELECT idl FROM _types WHERE topic='Track'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(stored_idl.contains("struct Track"));

        // ensure_topic is idempotent
        sink.ensure_topic("Track", "cuas::Track", IDL, &ty).unwrap();
    }
}
