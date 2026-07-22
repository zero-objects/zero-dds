// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! sqlite-WAL cold adapter for the Durability-Service (ADR 0009) — the
//! default `PERSISTENT` backend.
//!
//! Crate `zerodds-durability-store-sqlite`. Safety classification: **STANDARD**.
//!
//! ACID via `PRAGMA journal_mode=WAL; synchronous=NORMAL` (crash-safe without
//! an fsync per insert). One row per sample; the `(topic, instance, sequence)`
//! primary key makes re-sends idempotent. Retention follows the topic
//! [`Contract`] (set via [`DurabilityStore::set_contract`]); contracts live in
//! memory and are re-registered by the daemon on startup, while the samples
//! themselves survive a full process/system restart in the database file.
//!
//! For analytics, open the same `.db` with any SQL tool — the schema is
//! stable and documented here (ADR 0009: adapters expose their own native
//! read interface alongside the DDS path).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use zerodds_durability_store::{
    Contract, Cursor, DurabilitySample, DurabilityStore, Page, Result, Selector, StoreError,
    StoreStats,
};
use zerodds_qos::policies::history::HistoryKind;

const DEFAULT_PAGE: usize = 1024;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS samples (
    topic         TEXT    NOT NULL,
    instance      BLOB    NOT NULL,
    sequence      INTEGER NOT NULL,
    created_nanos INTEGER NOT NULL,
    payload       BLOB    NOT NULL,
    representation INTEGER NOT NULL DEFAULT 1,
    big_endian    INTEGER NOT NULL DEFAULT 0,
    source_guid   BLOB    NOT NULL DEFAULT x'00000000000000000000000000000000',
    source_sequence INTEGER NOT NULL DEFAULT -1,
    PRIMARY KEY (topic, instance, sequence)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS idx_samples_topic ON samples(topic, instance, sequence);
CREATE TABLE IF NOT EXISTS unregistered (
    topic     TEXT    NOT NULL,
    instance  BLOB    NOT NULL,
    at_nanos  INTEGER NOT NULL,
    PRIMARY KEY (topic, instance)
);
";

/// sqlite-backed durability store.
pub struct SqliteStore {
    conn: Mutex<Connection>,
    contracts: Mutex<BTreeMap<String, Contract>>,
    default_contract: Contract,
}

fn backend<E: core::fmt::Display>(ctx: &str, e: E) -> StoreError {
    StoreError::Backend(format!("sqlite store: {ctx}: {e}"))
}

fn nanos_of(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn time_of(nanos: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(nanos.max(0) as u64)
}

impl SqliteStore {
    /// Opens (creating if absent) a sqlite store at `path` with WAL mode.
    ///
    /// # Errors
    /// sqlite open/pragma/schema failure.
    pub fn open<P: AsRef<Path>>(path: P, default_contract: Contract) -> Result<Self> {
        let conn = Connection::open(path.as_ref()).map_err(|e| backend("open", e))?;
        Self::init(conn, default_contract)
    }

    /// In-memory sqlite store (tests / pure-TRANSIENT-via-sqlite).
    ///
    /// # Errors
    /// sqlite open/pragma/schema failure.
    pub fn open_in_memory(default_contract: Contract) -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| backend("open mem", e))?;
        Self::init(conn, default_contract)
    }

    fn init(conn: Connection, default_contract: Contract) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| backend("pragma wal", e))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| backend("pragma synchronous", e))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| backend("schema", e))?;
        // O2 P5 migration: add the source-identity columns to a pre-P5 `samples`
        // table. New DBs already have them from SCHEMA; on an existing DB the
        // ALTER succeeds once and then returns "duplicate column name", which is
        // the benign already-migrated case — ignore only that.
        for stmt in [
            "ALTER TABLE samples ADD COLUMN source_guid BLOB NOT NULL \
             DEFAULT x'00000000000000000000000000000000'",
            "ALTER TABLE samples ADD COLUMN source_sequence INTEGER NOT NULL DEFAULT -1",
        ] {
            if let Err(e) = conn.execute(stmt, []) {
                if !e.to_string().contains("duplicate column name") {
                    return Err(backend("migrate p5", e));
                }
            }
        }
        Ok(Self {
            conn: Mutex::new(conn),
            contracts: Mutex::new(BTreeMap::new()),
            default_contract,
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| StoreError::Poisoned("sqlite conn"))
    }

    fn contract_for(&self, topic: &str) -> Result<Contract> {
        Ok(self
            .contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("sqlite contracts"))?
            .get(topic)
            .copied()
            .unwrap_or(self.default_contract))
    }

    fn count(conn: &Connection, sql: &str, p: &[&dyn rusqlite::ToSql]) -> Result<i64> {
        conn.query_row(sql, p, |r| r.get::<_, i64>(0))
            .map_err(|e| backend("count", e))
    }
}

impl DurabilityStore for SqliteStore {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("sqlite contracts"))?
            .insert(topic.to_string(), contract);
        Ok(())
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let contract = self.contract_for(&sample.topic)?;
        let conn = self.lock_conn()?;
        let inst = &sample.instance_key[..];

        // A re-send of an already-stored (topic, instance, sequence) is an
        // idempotent REPLACE (reliable retransmit) — it grows nothing, so a
        // KEEP_ALL cap must not reject it.
        let is_resend = Self::count(
            &conn,
            "SELECT COUNT(*) FROM samples WHERE topic=?1 AND instance=?2 AND sequence=?3",
            params![sample.topic, inst, sample.sequence as i64],
        )? > 0;

        // Contract caps.
        if !is_resend
            && contract.samples_bounded()
            && matches!(contract.history_kind, HistoryKind::KeepAll)
        {
            let n = Self::count(
                &conn,
                "SELECT COUNT(*) FROM samples WHERE topic=?1",
                params![sample.topic],
            )?;
            if n >= i64::from(contract.max_samples) {
                return Err(StoreError::OutOfResources("max_samples"));
            }
        }
        if contract.instances_bounded() {
            let exists = Self::count(
                &conn,
                "SELECT COUNT(*) FROM samples WHERE topic=?1 AND instance=?2",
                params![sample.topic, inst],
            )? > 0;
            if !exists {
                let insts = Self::count(
                    &conn,
                    "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic=?1",
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
                "SELECT COUNT(*) FROM samples WHERE topic=?1 AND instance=?2",
                params![sample.topic, inst],
            )?;
            if n >= i64::from(contract.max_samples_per_instance) {
                return Err(StoreError::OutOfResources("max_samples_per_instance"));
            }
        }

        conn.execute(
            "INSERT OR REPLACE INTO samples(topic,instance,sequence,created_nanos,payload,representation,big_endian,source_guid,source_sequence) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                sample.topic,
                inst,
                sample.sequence as i64,
                nanos_of(sample.created_at),
                sample.payload,
                i64::from(sample.representation),
                i64::from(sample.big_endian),
                sample.source_guid.to_vec(),
                sample.source_sequence
            ],
        )
        .map_err(|e| backend("insert", e))?;

        // KEEP_LAST: trim to the newest `depth` per instance.
        if matches!(contract.history_kind, HistoryKind::KeepLast) {
            conn.execute(
                "DELETE FROM samples WHERE topic=?1 AND instance=?2 AND sequence NOT IN \
                 (SELECT sequence FROM samples WHERE topic=?1 AND instance=?2 \
                  ORDER BY sequence DESC LIMIT ?3)",
                params![sample.topic, inst, contract.effective_depth() as i64],
            )
            .map_err(|e| backend("keep_last trim", e))?;
        }
        Ok(())
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        let conn = self.lock_conn()?;
        let limit = selector.limit.unwrap_or(DEFAULT_PAGE);
        // Build a dynamic WHERE with bound params (fetch limit+1 to detect more).
        let mut sql = String::from(
            "SELECT instance,sequence,created_nanos,payload,representation,big_endian,source_guid,source_sequence \
             FROM samples WHERE topic=?1",
        );
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(topic.to_string())];
        if let Some(k) = selector.instance_key {
            sql.push_str(" AND instance=?");
            sql.push_str(&(binds.len() + 1).to_string());
            binds.push(Box::new(k.to_vec()));
        }
        if let Some(lo) = selector.seq_from {
            sql.push_str(&format!(" AND sequence>=?{}", binds.len() + 1));
            binds.push(Box::new(lo as i64));
        }
        if let Some(hi) = selector.seq_to {
            sql.push_str(&format!(" AND sequence<=?{}", binds.len() + 1));
            binds.push(Box::new(hi as i64));
        }
        if let Some(t0) = selector.time_from {
            sql.push_str(&format!(" AND created_nanos>=?{}", binds.len() + 1));
            binds.push(Box::new(nanos_of(t0)));
        }
        if let Some(t1) = selector.time_to {
            sql.push_str(&format!(" AND created_nanos<=?{}", binds.len() + 1));
            binds.push(Box::new(nanos_of(t1)));
        }
        if let Some((ck, cs)) = selector.after {
            // (instance,sequence) strictly after the cursor.
            let i1 = binds.len() + 1;
            let i2 = binds.len() + 2;
            let i3 = binds.len() + 3;
            sql.push_str(&format!(
                " AND (instance>?{i1} OR (instance=?{i2} AND sequence>?{i3}))"
            ));
            binds.push(Box::new(ck.to_vec()));
            binds.push(Box::new(ck.to_vec()));
            binds.push(Box::new(cs as i64));
        }
        sql.push_str(&format!(" ORDER BY instance,sequence LIMIT {}", limit + 1));

        let mut stmt = conn.prepare(&sql).map_err(|e| backend("prepare", e))?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        let topic_owned = topic.to_string();
        let rows = stmt
            .query_map(param_refs.as_slice(), |r| {
                let inst: Vec<u8> = r.get(0)?;
                let mut key = [0u8; 16];
                if inst.len() == 16 {
                    key.copy_from_slice(&inst);
                }
                let sg: Vec<u8> = r.get(6)?;
                let mut source_guid = [0u8; 16];
                if sg.len() == 16 {
                    source_guid.copy_from_slice(&sg);
                }
                Ok(DurabilitySample {
                    topic: topic_owned.clone(),
                    instance_key: key,
                    sequence: r.get::<_, i64>(1)? as u64,
                    created_at: time_of(r.get::<_, i64>(2)?),
                    payload: r.get(3)?,
                    representation: r.get::<_, i64>(4)? as u8,
                    big_endian: r.get::<_, i64>(5)? != 0,
                    source_guid,
                    source_sequence: r.get::<_, i64>(7)?,
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
            "INSERT OR REPLACE INTO unregistered(topic,instance,at_nanos) VALUES (?1,?2,?3)",
            params![topic, &instance_key[..], nanos_of(now)],
        )
        .map_err(|e| backend("unregister", e))?;
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let conn = self.lock_conn()?;
        // Collect due (topic,instance) pairs, applying each topic's delay.
        let mut stmt = conn
            .prepare("SELECT topic,instance,at_nanos FROM unregistered")
            .map_err(|e| backend("prepare cleanup", e))?;
        let due: Vec<(String, Vec<u8>)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| backend("cleanup scan", e))?
            .filter_map(|row| row.ok())
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
                "DELETE FROM samples WHERE topic=?1 AND instance=?2",
                params![topic, inst],
            )
            .map_err(|e| backend("cleanup delete samples", e))?;
            conn.execute(
                "DELETE FROM unregistered WHERE topic=?1 AND instance=?2",
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
            "SELECT COUNT(*) FROM samples WHERE topic=?1",
            params![topic],
        )? as usize;
        let instances = Self::count(
            &conn,
            "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic=?1",
            params![topic],
        )? as usize;
        let bytes = conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(payload)),0) FROM samples WHERE topic=?1",
                params![topic],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| backend("stats bytes", e))?
            .unwrap_or(0) as u64;
        Ok(StoreStats {
            samples,
            instances,
            bytes,
        })
    }
}
