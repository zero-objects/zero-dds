// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PostgreSQL cold adapter for the Durability-Service (ADR 0009) — the
//! shared/long-term `PERSISTENT` tier for large fleets.
//!
//! Crate `zerodds-durability-store-postgres`. Safety classification: **STANDARD**.
//!
//! Where the sqlite adapter is the single-file default, this one is the tier
//! for many producers writing into one shared, network-reachable database —
//! the "PostgreSQL cold tier" of the RAM/SSD/PostgreSQL story. It implements
//! the same [`DurabilityStore`] contract with identical semantics: one row per
//! sample, `(topic, instance, sequence)` is the sample identity (a re-send is
//! an idempotent replace), and retention is bounded ONLY by the topic
//! [`Contract`] (ADR 0009 inv. 1).
//!
//! Connection uses `NoTls` — put the daemon and the database on a trusted
//! network segment, or front the database with a TLS-terminating proxy; the
//! connection string is a standard libpq keyword/URI form.
//!
//! With the `timescaledb` feature the samples table is a TimescaleDB hypertable
//! partitioned on `created_nanos` (needs the extension in the target database);
//! sample identity is preserved by an explicit delete-then-insert on the
//! `(topic, instance, sequence)` key, so idempotency holds regardless of the
//! partitioning primary key.
//!
//! For analytics, connect to the same database with any SQL/BI tool — the
//! schema is stable and documented here (ADR 0009: adapters expose their own
//! native read interface alongside the DDS path).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use postgres::types::ToSql;
use postgres::{Client, NoTls};
use zerodds_durability_store::{
    Contract, Cursor, DurabilitySample, DurabilityStore, Page, Result, Selector, StoreError,
    StoreStats,
};
use zerodds_qos::policies::history::HistoryKind;

const DEFAULT_PAGE: usize = 1024;

#[cfg(not(feature = "timescaledb"))]
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS samples (
    topic           TEXT     NOT NULL,
    instance        BYTEA    NOT NULL,
    sequence        BIGINT   NOT NULL,
    created_nanos   BIGINT   NOT NULL,
    payload         BYTEA    NOT NULL,
    representation  SMALLINT NOT NULL DEFAULT 1,
    big_endian      BOOLEAN  NOT NULL DEFAULT false,
    source_guid     BYTEA    NOT NULL DEFAULT '\\x00000000000000000000000000000000',
    source_sequence BIGINT   NOT NULL DEFAULT -1,
    PRIMARY KEY (topic, instance, sequence)
);
CREATE INDEX IF NOT EXISTS idx_samples_topic ON samples(topic, instance, sequence);
CREATE TABLE IF NOT EXISTS unregistered (
    topic     TEXT   NOT NULL,
    instance  BYTEA  NOT NULL,
    at_nanos  BIGINT NOT NULL,
    PRIMARY KEY (topic, instance)
);
";

// TimescaleDB requires the partitioning column in every unique index, so the
// primary key includes `created_nanos`; sample identity on
// `(topic, instance, sequence)` is upheld by the delete-then-insert in `store`.
#[cfg(feature = "timescaledb")]
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS samples (
    topic           TEXT     NOT NULL,
    instance        BYTEA    NOT NULL,
    sequence        BIGINT   NOT NULL,
    created_nanos   BIGINT   NOT NULL,
    payload         BYTEA    NOT NULL,
    representation  SMALLINT NOT NULL DEFAULT 1,
    big_endian      BOOLEAN  NOT NULL DEFAULT false,
    source_guid     BYTEA    NOT NULL DEFAULT '\\x00000000000000000000000000000000',
    source_sequence BIGINT   NOT NULL DEFAULT -1,
    PRIMARY KEY (topic, instance, sequence, created_nanos)
);
CREATE INDEX IF NOT EXISTS idx_samples_topic ON samples(topic, instance, sequence);
CREATE EXTENSION IF NOT EXISTS timescaledb;
SELECT create_hypertable('samples', 'created_nanos',
    chunk_time_interval => 86400000000000, if_not_exists => TRUE, migrate_data => TRUE);
CREATE TABLE IF NOT EXISTS unregistered (
    topic     TEXT   NOT NULL,
    instance  BYTEA  NOT NULL,
    at_nanos  BIGINT NOT NULL,
    PRIMARY KEY (topic, instance)
);
";

/// PostgreSQL-backed durability store.
pub struct PostgresStore {
    client: Mutex<Client>,
    contracts: Mutex<BTreeMap<String, Contract>>,
    default_contract: Contract,
}

/// A fixed advisory-lock key that serializes `CREATE TABLE IF NOT EXISTS`
/// across connections. `IF NOT EXISTS` is not safe against a concurrent create
/// (two sessions both pass the existence check, then collide on the system
/// catalog — `pg_type_typname_nsp_index`), which happens when several daemons
/// (one per domain) share a database. Holding this lock around the DDL lets
/// exactly one session create the schema; the rest then see it already exists.
const SCHEMA_LOCK_KEY: i64 = 0x7A64_6473_6368_656D; // "zddsschem"

fn backend(ctx: &str, e: postgres::Error) -> StoreError {
    // `postgres::Error`'s own `Display` is terse ("db error"); the useful text
    // (the server's message) lives in the attached `DbError`.
    let detail = e
        .as_db_error()
        .map(|d| d.message().to_string())
        .unwrap_or_else(|| e.to_string());
    StoreError::Backend(format!("postgres store: {ctx}: {detail}"))
}

fn nanos_of(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn time_of(nanos: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(nanos.max(0) as u64)
}

impl PostgresStore {
    /// Connects to PostgreSQL (libpq keyword/URI connection string, e.g.
    /// `postgres://user@host/db` or `host=… user=… dbname=…`) and ensures the
    /// schema. The samples survive a full process/system restart in the
    /// database; contracts live in memory and are re-registered on startup.
    ///
    /// # Errors
    /// Connection, schema, or (with `timescaledb`) hypertable-setup failure.
    pub fn connect(conn_str: &str, default_contract: Contract) -> Result<Self> {
        let client = Client::connect(conn_str, NoTls).map_err(|e| backend("connect", e))?;
        Self::init(client, default_contract)
    }

    fn init(mut client: Client, default_contract: Contract) -> Result<Self> {
        // Serialize schema creation across connections (see SCHEMA_LOCK_KEY):
        // the advisory lock is held only for the DDL and released right after,
        // so concurrent daemons initialise safely without a catalog race.
        client
            .execute("SELECT pg_advisory_lock($1)", &[&SCHEMA_LOCK_KEY])
            .map_err(|e| backend("schema lock", e))?;
        let schema_result = client.batch_execute(SCHEMA);
        let unlock_result = client.execute("SELECT pg_advisory_unlock($1)", &[&SCHEMA_LOCK_KEY]);
        schema_result.map_err(|e| backend("schema", e))?;
        unlock_result.map_err(|e| backend("schema unlock", e))?;
        Ok(Self {
            client: Mutex::new(client),
            contracts: Mutex::new(BTreeMap::new()),
            default_contract,
        })
    }

    fn lock_client(&self) -> Result<std::sync::MutexGuard<'_, Client>> {
        self.client
            .lock()
            .map_err(|_| StoreError::Poisoned("postgres client"))
    }

    fn contract_for(&self, topic: &str) -> Result<Contract> {
        Ok(self
            .contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("postgres contracts"))?
            .get(topic)
            .copied()
            .unwrap_or(self.default_contract))
    }

    fn count(client: &mut Client, sql: &str, p: &[&(dyn ToSql + Sync)]) -> Result<i64> {
        let row = client.query_one(sql, p).map_err(|e| backend("count", e))?;
        Ok(row.get::<_, i64>(0))
    }
}

impl DurabilityStore for PostgresStore {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("postgres contracts"))?
            .insert(topic.to_string(), contract);
        Ok(())
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let contract = self.contract_for(&sample.topic)?;
        let mut client = self.lock_client()?;
        let inst = &sample.instance_key[..];
        let seq = sample.sequence as i64;

        // A re-send of an already-stored (topic, instance, sequence) is
        // idempotent (reliable retransmit) — it grows nothing, so a KEEP_ALL
        // cap must not reject it.
        let is_resend = Self::count(
            &mut client,
            "SELECT COUNT(*) FROM samples WHERE topic=$1 AND instance=$2 AND sequence=$3",
            &[&sample.topic, &inst, &seq],
        )? > 0;

        // Contract caps (identical to the sqlite adapter).
        if !is_resend
            && contract.samples_bounded()
            && matches!(contract.history_kind, HistoryKind::KeepAll)
        {
            let n = Self::count(
                &mut client,
                "SELECT COUNT(*) FROM samples WHERE topic=$1",
                &[&sample.topic],
            )?;
            if n >= i64::from(contract.max_samples) {
                return Err(StoreError::OutOfResources("max_samples"));
            }
        }
        if contract.instances_bounded() {
            let exists = Self::count(
                &mut client,
                "SELECT COUNT(*) FROM samples WHERE topic=$1 AND instance=$2",
                &[&sample.topic, &inst],
            )? > 0;
            if !exists {
                let insts = Self::count(
                    &mut client,
                    "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic=$1",
                    &[&sample.topic],
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
                &mut client,
                "SELECT COUNT(*) FROM samples WHERE topic=$1 AND instance=$2",
                &[&sample.topic, &inst],
            )?;
            if n >= i64::from(contract.max_samples_per_instance) {
                return Err(StoreError::OutOfResources("max_samples_per_instance"));
            }
        }

        // Delete-then-insert on the (topic, instance, sequence) identity: an
        // explicit upsert that holds whether or not `created_nanos` is part of
        // the primary key (it is under the `timescaledb` feature).
        let created = nanos_of(sample.created_at);
        let rep = i16::from(sample.representation);
        let src_guid = sample.source_guid.to_vec();
        client
            .execute(
                "DELETE FROM samples WHERE topic=$1 AND instance=$2 AND sequence=$3",
                &[&sample.topic, &inst, &seq],
            )
            .map_err(|e| backend("replace delete", e))?;
        client
            .execute(
                "INSERT INTO samples(topic,instance,sequence,created_nanos,payload,representation,big_endian,source_guid,source_sequence) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
                &[
                    &sample.topic,
                    &inst,
                    &seq,
                    &created,
                    &sample.payload,
                    &rep,
                    &sample.big_endian,
                    &src_guid,
                    &sample.source_sequence,
                ],
            )
            .map_err(|e| backend("insert", e))?;

        // KEEP_LAST: trim to the newest `depth` per instance.
        if matches!(contract.history_kind, HistoryKind::KeepLast) {
            let depth = contract.effective_depth() as i64;
            client
                .execute(
                    "DELETE FROM samples WHERE topic=$1 AND instance=$2 AND sequence NOT IN \
                     (SELECT sequence FROM samples WHERE topic=$1 AND instance=$2 \
                      ORDER BY sequence DESC LIMIT $3)",
                    &[&sample.topic, &inst, &depth],
                )
                .map_err(|e| backend("keep_last trim", e))?;
        }
        Ok(())
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        let mut client = self.lock_client()?;
        let limit = selector.limit.unwrap_or(DEFAULT_PAGE);
        let mut sql = String::from(
            "SELECT instance,sequence,created_nanos,payload,representation,big_endian,source_guid,source_sequence \
             FROM samples WHERE topic=$1",
        );
        let topic_owned = topic.to_string();
        let mut binds: Vec<Box<dyn ToSql + Sync>> = vec![Box::new(topic_owned.clone())];
        if let Some(k) = selector.instance_key {
            binds.push(Box::new(k.to_vec()));
            sql.push_str(&format!(" AND instance=${}", binds.len()));
        }
        if let Some(lo) = selector.seq_from {
            binds.push(Box::new(lo as i64));
            sql.push_str(&format!(" AND sequence>=${}", binds.len()));
        }
        if let Some(hi) = selector.seq_to {
            binds.push(Box::new(hi as i64));
            sql.push_str(&format!(" AND sequence<=${}", binds.len()));
        }
        if let Some(t0) = selector.time_from {
            binds.push(Box::new(nanos_of(t0)));
            sql.push_str(&format!(" AND created_nanos>=${}", binds.len()));
        }
        if let Some(t1) = selector.time_to {
            binds.push(Box::new(nanos_of(t1)));
            sql.push_str(&format!(" AND created_nanos<=${}", binds.len()));
        }
        if let Some((ck, cs)) = selector.after {
            binds.push(Box::new(ck.to_vec()));
            let i1 = binds.len();
            binds.push(Box::new(ck.to_vec()));
            let i2 = binds.len();
            binds.push(Box::new(cs as i64));
            let i3 = binds.len();
            sql.push_str(&format!(
                " AND (instance>${i1} OR (instance=${i2} AND sequence>${i3}))"
            ));
        }
        // Fetch limit+1 to detect a following page.
        binds.push(Box::new((limit + 1) as i64));
        sql.push_str(&format!(
            " ORDER BY instance,sequence LIMIT ${}",
            binds.len()
        ));

        let params: Vec<&(dyn ToSql + Sync)> = binds.iter().map(|b| b.as_ref()).collect();
        let rows = client
            .query(&sql, params.as_slice())
            .map_err(|e| backend("query", e))?;

        let mut samples = Vec::with_capacity(rows.len());
        for r in &rows {
            let inst: Vec<u8> = r.get(0);
            let mut key = [0u8; 16];
            if inst.len() == 16 {
                key.copy_from_slice(&inst);
            }
            let sg: Vec<u8> = r.get(6);
            let mut source_guid = [0u8; 16];
            if sg.len() == 16 {
                source_guid.copy_from_slice(&sg);
            }
            samples.push(DurabilitySample {
                topic: topic_owned.clone(),
                instance_key: key,
                sequence: r.get::<_, i64>(1) as u64,
                created_at: time_of(r.get::<_, i64>(2)),
                payload: r.get(3),
                representation: r.get::<_, i16>(4) as u8,
                big_endian: r.get::<_, bool>(5),
                source_guid,
                source_sequence: r.get::<_, i64>(7),
            });
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
        let mut client = self.lock_client()?;
        let inst = &instance_key[..];
        let at = nanos_of(now);
        client
            .execute(
                "INSERT INTO unregistered(topic,instance,at_nanos) VALUES ($1,$2,$3) \
                 ON CONFLICT (topic,instance) DO UPDATE SET at_nanos=EXCLUDED.at_nanos",
                &[&topic, &inst, &at],
            )
            .map_err(|e| backend("unregister", e))?;
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let mut client = self.lock_client()?;
        let rows = client
            .query("SELECT topic,instance,at_nanos FROM unregistered", &[])
            .map_err(|e| backend("cleanup scan", e))?;
        let due: Vec<(String, Vec<u8>)> = rows
            .iter()
            .filter_map(|r| {
                let topic: String = r.get(0);
                let inst: Vec<u8> = r.get(1);
                let at: i64 = r.get(2);
                let delay = self.contract_for(&topic).ok()?.cleanup_delay;
                let deadline = time_of(at).checked_add(delay)?;
                (now >= deadline).then_some((topic, inst))
            })
            .collect();
        let mut removed = 0usize;
        for (topic, inst) in due {
            client
                .execute(
                    "DELETE FROM samples WHERE topic=$1 AND instance=$2",
                    &[&topic, &inst],
                )
                .map_err(|e| backend("cleanup delete samples", e))?;
            client
                .execute(
                    "DELETE FROM unregistered WHERE topic=$1 AND instance=$2",
                    &[&topic, &inst],
                )
                .map_err(|e| backend("cleanup delete marker", e))?;
            removed += 1;
        }
        Ok(removed)
    }

    fn stats(&self, topic: &str) -> Result<StoreStats> {
        let mut client = self.lock_client()?;
        let samples = Self::count(
            &mut client,
            "SELECT COUNT(*) FROM samples WHERE topic=$1",
            &[&topic],
        )? as usize;
        let instances = Self::count(
            &mut client,
            "SELECT COUNT(DISTINCT instance) FROM samples WHERE topic=$1",
            &[&topic],
        )? as usize;
        let bytes = Self::count(
            &mut client,
            "SELECT COALESCE(SUM(LENGTH(payload)),0)::BIGINT FROM samples WHERE topic=$1",
            &[&topic],
        )? as u64;
        Ok(StoreStats {
            samples,
            instances,
            bytes,
        })
    }
}
