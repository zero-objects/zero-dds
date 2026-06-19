// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DuckDB lakehouse cold adapter for the Durability-Service (ADR 0009).
//!
//! Crate `zerodds-durability-store-lakehouse`. Safety classification: **STANDARD**.
//!
//! One adapter, two paradigms (ADR 0009: "house is generally better than lake,
//! but some want to throw structured data into lakes"):
//!
//! * **Warehouse / house** — durable, transactional in-process SQL. Samples
//!   live in a typed DuckDB table; run arbitrary analytics SQL on the same
//!   `.db` file (open it with the `duckdb` CLI or any DuckDB client).
//! * **Lake** — [`export_parquet`](LakehouseStore::export_parquet) writes a
//!   topic's samples as a Parquet partition (schema-on-read), queryable by
//!   DuckDB / Spark / Polars / Arrow out-of-band.
//!
//! Implements [`DurabilityStore`] for the DDS path (store/query/replay), with
//! the same contract semantics as the sqlite adapter. The DuckDB storage file
//! survives a full process/system restart (the `PERSISTENT` guarantee).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use duckdb::{Connection, params};
use zerodds_durability_store::{
    Contract, Cursor, DurabilitySample, DurabilityStore, Page, Result, Selector, StoreError,
    StoreStats,
};
use zerodds_qos::policies::history::HistoryKind;

const DEFAULT_PAGE: usize = 1024;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS samples (
    topic         VARCHAR NOT NULL,
    instance      BLOB    NOT NULL,
    sequence      BIGINT  NOT NULL,
    created_nanos BIGINT  NOT NULL,
    payload       BLOB    NOT NULL,
    PRIMARY KEY (topic, instance, sequence)
);
CREATE TABLE IF NOT EXISTS unregistered (
    topic     VARCHAR NOT NULL,
    instance  BLOB    NOT NULL,
    at_nanos  BIGINT  NOT NULL,
    PRIMARY KEY (topic, instance)
);
";

/// DuckDB-backed lakehouse durability store.
pub struct LakehouseStore {
    conn: Mutex<Connection>,
    contracts: Mutex<BTreeMap<String, Contract>>,
    default_contract: Contract,
}

fn backend<E: core::fmt::Display>(ctx: &str, e: E) -> StoreError {
    StoreError::Backend(format!("lakehouse store: {ctx}: {e}"))
}

fn nanos_of(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn time_of(nanos: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(nanos.max(0) as u64)
}

impl LakehouseStore {
    /// Opens (creating if absent) a DuckDB lakehouse at `path`.
    ///
    /// # Errors
    /// DuckDB open/schema failure.
    pub fn open<P: AsRef<Path>>(path: P, default_contract: Contract) -> Result<Self> {
        let conn = Connection::open(path.as_ref()).map_err(|e| backend("open", e))?;
        Self::init(conn, default_contract)
    }

    /// In-memory lakehouse (tests / ephemeral analytics).
    ///
    /// # Errors
    /// DuckDB open/schema failure.
    pub fn open_in_memory(default_contract: Contract) -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| backend("open mem", e))?;
        Self::init(conn, default_contract)
    }

    fn init(conn: Connection, default_contract: Contract) -> Result<Self> {
        conn.execute_batch(SCHEMA)
            .map_err(|e| backend("schema", e))?;
        Ok(Self {
            conn: Mutex::new(conn),
            contracts: Mutex::new(BTreeMap::new()),
            default_contract,
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| StoreError::Poisoned("lakehouse conn"))
    }

    fn contract_for(&self, topic: &str) -> Result<Contract> {
        Ok(self
            .contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("lakehouse contracts"))?
            .get(topic)
            .copied()
            .unwrap_or(self.default_contract))
    }

    fn count(conn: &Connection, sql: &str, p: &[&dyn duckdb::ToSql]) -> Result<i64> {
        conn.query_row(sql, p, |r| r.get::<_, i64>(0))
            .map_err(|e| backend("count", e))
    }

    /// **Lake export**: writes `topic`'s samples to `out_path` as a Parquet
    /// partition (ordered by `(instance, sequence)`), for schema-on-read
    /// analytics with DuckDB/Spark/Polars. This is the adapter's own native
    /// read interface (ADR 0009) — distinct from the DDS [`DurabilityStore`]
    /// path.
    ///
    /// # Errors
    /// DuckDB COPY failure.
    pub fn export_parquet(&self, topic: &str, out_path: &Path) -> Result<()> {
        let conn = self.lock_conn()?;
        // The path is a COPY-TO literal (not a bind parameter in DuckDB); the
        // topic stays a bound parameter. Single-quotes in the path are escaped.
        let path = out_path.display().to_string().replace('\'', "''");
        conn.execute(
            &format!(
                "COPY (SELECT topic, instance, sequence, created_nanos, payload \
                 FROM samples WHERE topic = ? ORDER BY instance, sequence) \
                 TO '{path}' (FORMAT PARQUET)"
            ),
            params![topic],
        )
        .map_err(|e| backend("export parquet", e))?;
        Ok(())
    }
}

impl DurabilityStore for LakehouseStore {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("lakehouse contracts"))?
            .insert(topic.to_string(), contract);
        Ok(())
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let contract = self.contract_for(&sample.topic)?;
        let conn = self.lock_conn()?;
        let inst = &sample.instance_key[..];

        // A re-send of an already-stored (topic, instance, sequence) is an
        // idempotent REPLACE — it grows neither the topic nor the instance, so
        // it must not be rejected by a KEEP_ALL cap (reliable retransmit).
        let is_resend = Self::count(
            &conn,
            "SELECT COUNT(*) FROM samples WHERE topic = ? AND instance = ? AND sequence = ?",
            params![sample.topic, inst, sample.sequence as i64],
        )? > 0;

        if !is_resend
            && contract.samples_bounded()
            && matches!(contract.history_kind, HistoryKind::KeepAll)
        {
            let n = Self::count(
                &conn,
                "SELECT COUNT(*) FROM samples WHERE topic = ?",
                params![sample.topic],
            )?;
            if n >= i64::from(contract.max_samples) {
                return Err(StoreError::OutOfResources("max_samples"));
            }
        }
        if contract.instances_bounded() {
            let exists = Self::count(
                &conn,
                "SELECT COUNT(*) FROM samples WHERE topic = ? AND instance = ?",
                params![sample.topic, inst],
            )? > 0;
            if !exists {
                let insts = Self::count(
                    &conn,
                    "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic = ?",
                    params![sample.topic],
                )?;
                if insts >= i64::from(contract.max_instances) {
                    return Err(StoreError::OutOfResources("max_instances"));
                }
            }
        }
        if !is_resend
            && contract.per_instance_bounded()
            && matches!(contract.history_kind, HistoryKind::KeepAll)
        {
            let n = Self::count(
                &conn,
                "SELECT COUNT(*) FROM samples WHERE topic = ? AND instance = ?",
                params![sample.topic, inst],
            )?;
            if n >= i64::from(contract.max_samples_per_instance) {
                return Err(StoreError::OutOfResources("max_samples_per_instance"));
            }
        }

        conn.execute(
            "INSERT OR REPLACE INTO samples(topic, instance, sequence, created_nanos, payload) \
             VALUES (?, ?, ?, ?, ?)",
            params![
                sample.topic,
                inst,
                sample.sequence as i64,
                nanos_of(sample.created_at),
                sample.payload
            ],
        )
        .map_err(|e| backend("insert", e))?;

        if matches!(contract.history_kind, HistoryKind::KeepLast) {
            conn.execute(
                "DELETE FROM samples WHERE topic = ? AND instance = ? AND sequence NOT IN \
                 (SELECT sequence FROM samples WHERE topic = ? AND instance = ? \
                  ORDER BY sequence DESC LIMIT ?)",
                params![
                    sample.topic,
                    inst,
                    sample.topic,
                    inst,
                    contract.effective_depth() as i64
                ],
            )
            .map_err(|e| backend("keep_last trim", e))?;
        }
        Ok(())
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        let conn = self.lock_conn()?;
        let limit = selector.limit.unwrap_or(DEFAULT_PAGE);
        let mut sql = String::from(
            "SELECT instance, sequence, created_nanos, payload FROM samples WHERE topic = ?",
        );
        let mut binds: Vec<Box<dyn duckdb::ToSql>> = vec![Box::new(topic.to_string())];
        if let Some(k) = selector.instance_key {
            sql.push_str(" AND instance = ?");
            binds.push(Box::new(k.to_vec()));
        }
        if let Some(lo) = selector.seq_from {
            sql.push_str(" AND sequence >= ?");
            binds.push(Box::new(lo as i64));
        }
        if let Some(hi) = selector.seq_to {
            sql.push_str(" AND sequence <= ?");
            binds.push(Box::new(hi as i64));
        }
        if let Some(t0) = selector.time_from {
            sql.push_str(" AND created_nanos >= ?");
            binds.push(Box::new(nanos_of(t0)));
        }
        if let Some(t1) = selector.time_to {
            sql.push_str(" AND created_nanos <= ?");
            binds.push(Box::new(nanos_of(t1)));
        }
        if let Some((ck, cs)) = selector.after {
            sql.push_str(" AND (instance > ? OR (instance = ? AND sequence > ?))");
            binds.push(Box::new(ck.to_vec()));
            binds.push(Box::new(ck.to_vec()));
            binds.push(Box::new(cs as i64));
        }
        sql.push_str(&format!(" ORDER BY instance, sequence LIMIT {}", limit + 1));

        let mut stmt = conn.prepare(&sql).map_err(|e| backend("prepare", e))?;
        let param_refs: Vec<&dyn duckdb::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        let topic_owned = topic.to_string();
        let rows = stmt
            .query_map(param_refs.as_slice(), |r| {
                let inst: Vec<u8> = r.get(0)?;
                let mut key = [0u8; 16];
                if inst.len() == 16 {
                    key.copy_from_slice(&inst);
                }
                Ok(DurabilitySample {
                    topic: topic_owned.clone(),
                    instance_key: key,
                    sequence: r.get::<_, i64>(1)? as u64,
                    created_at: time_of(r.get::<_, i64>(2)?),
                    payload: r.get(3)?,
                })
            })
            .map_err(|e| backend("query_map", e))?;
        let mut samples = Vec::new();
        for row in rows {
            samples.push(row.map_err(|e| backend("row", e))?);
        }
        let exhausted = samples.len() <= limit;
        samples.truncate(limit);
        let next: Option<Cursor> = if exhausted {
            None
        } else {
            samples.last().map(|s| (s.instance_key, s.sequence))
        };
        Ok(Page { samples, next })
    }

    fn unregister(&self, topic: &str, instance_key: &[u8; 16], now: SystemTime) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO unregistered(topic, instance, at_nanos) VALUES (?, ?, ?)",
            params![topic, &instance_key[..], nanos_of(now)],
        )
        .map_err(|e| backend("unregister", e))?;
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT topic, instance, at_nanos FROM unregistered")
            .map_err(|e| backend("prepare cleanup", e))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| backend("cleanup scan", e))?;
        let mut all = Vec::new();
        for row in rows {
            all.push(row.map_err(|e| backend("cleanup row", e))?);
        }
        let due: Vec<(String, Vec<u8>)> = all
            .into_iter()
            .filter_map(|(topic, inst, at)| {
                let delay = self.contract_for(&topic).ok()?.cleanup_delay;
                let deadline = time_of(at).checked_add(delay)?;
                if now >= deadline {
                    Some((topic, inst))
                } else {
                    None
                }
            })
            .collect();
        drop(stmt);
        let mut removed = 0usize;
        for (topic, inst) in due {
            conn.execute(
                "DELETE FROM samples WHERE topic = ? AND instance = ?",
                params![topic, inst],
            )
            .map_err(|e| backend("cleanup delete samples", e))?;
            conn.execute(
                "DELETE FROM unregistered WHERE topic = ? AND instance = ?",
                params![topic, inst],
            )
            .map_err(|e| backend("cleanup delete marker", e))?;
            removed += 1;
        }
        Ok(removed)
    }

    fn stats(&self, topic: &str) -> Result<StoreStats> {
        let conn = self.lock_conn()?;
        let samples = Self::count(
            &conn,
            "SELECT COUNT(*) FROM samples WHERE topic = ?",
            params![topic],
        )? as usize;
        let instances = Self::count(
            &conn,
            "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic = ?",
            params![topic],
        )? as usize;
        // DuckDB has no LENGTH(BLOB); OCTET_LENGTH gives a blob's byte count.
        let bytes = Self::count(
            &conn,
            "SELECT COALESCE(SUM(OCTET_LENGTH(payload)), 0) FROM samples WHERE topic = ?",
            params![topic],
        )? as u64;
        Ok(StoreStats {
            samples,
            instances,
            bytes,
        })
    }
}
