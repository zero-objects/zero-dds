# Track RC2-A — Tiered-Storage Datalake-Engine

**Goal:** ZeroDDS bekommt eine konfigurierbare Tiered-Storage-Engine die
DDS-Samples über lange Zeiträume aus zigtausenden Sensoren aggregiert,
mit klar abgegrenzten Hot/Warm/Cold-Tiers und einer Persistent-API die
sowohl als Builtin als auch über externe Engines (Postgres, Object-Store)
betrieben werden kann.

**Status:** 📋 todo

**Estimate:** 3-5 Personenwochen.

## Motivation

Production-Industrial-Deployments mit zigtausenden Sensoren (Predictive-
Maintenance, Asset-Performance-Management, Fleet-Telemetry) brauchen:

- **Hot-Tier** für die letzten Sekunden bis Minuten (Live-Dashboards,
  Trigger-Alerts) — RAM, lock-free
- **Warm-Tier** für Stunden bis Tage (Anomaly-Detection-Windows, Replay
  letzter Schicht) — local NVMe, kompakte Sample-Logs
- **Cold-Tier** für Monate bis Jahre (regulatorische Aufbewahrung,
  Trainings-Datasets, Long-Term-Analytics) — externe Engine: PostgreSQL
  + TimescaleDB für Time-Series, oder S3-kompatibler Object-Store
- **Configurable Promotion** zwischen Tiers (LRU, time-bucket,
  per-topic-policy)

DDS hat das spec-mäßig in der Form von **Persistence Service** (DDS 1.4
Optional Profile), aber die Implementierungs-Freiheit ist groß. ZeroDDS
liefert die Reference-Implementation.

## In-Scope

### Crates (neu)

- `crates/dcps-persistence` — Persistence-Service-Crate, hooks in den
  DCPS-DataWriter/-DataReader-Pfad
- `crates/persistence-engine` — Trait `StorageEngine` (Hot/Warm/Cold)
- `crates/persistence-builtin` — Default-Implementation (Hot=Ringbuffer,
  Warm=mmap-Log, Cold=Direct-PG-Connect-Pool)
- `crates/persistence-pg` — PostgreSQL-Engine (sqlx, optional
  timescaledb feature)
- `crates/persistence-s3` — S3-kompatibler Object-Store-Engine (rust-s3
  oder aws-sdk-s3)
- `crates/persistence-config` — YAML/TOML-Config-Loader für
  Tier-Promotion-Policies

### Konfiguration (Beispiel)

```yaml
persistence:
  tiers:
    hot:
      capacity_samples: 10000          # pro instance
      capacity_bytes: 256MiB           # cap übergreifend
      eviction: lru                    # lru | fifo | time-window
    warm:
      backend: file                    # file | none
      path: /var/lib/zerodds/warm
      capacity_bytes: 64GiB
      compaction: hourly               # interval | sample-count
      retention: 7d
    cold:
      backend: postgres                # postgres | s3 | none
      url: postgres://zerodds@db/zerodds
      schema: dcps_samples
      retention: 365d
      partitioning: monthly            # daily | monthly
      compression: zstd
  promotion:
    hot_to_warm:
      trigger: capacity-threshold      # capacity-threshold | age | manual
      age: 5m
    warm_to_cold:
      trigger: age
      age: 24h
  topics:                              # per-topic overrides
    "Sensors/Temperature.*":
      hot.capacity_samples: 50000      # higher hot capacity
      cold.retention: 5y               # regulatory hold
```

### Persistent API

Public-API auf der Reader/Writer-Schicht:

```rust
// new constructor that wires persistence
let writer = publisher.create_writer_persistent(
    &topic,
    &qos,
    &PersistenceConfig::from_path("/etc/zerodds/persistence.yaml")?,
)?;

// queries against the persistent store
let snapshot = participant.persistence_query()
    .topic("Sensors/Temperature.bay-12")
    .time_range(now() - Duration::from_days(30), now())
    .downsample(Aggregation::Mean, Duration::from_minutes(5))
    .stream()?;

// streaming-iterator that pulls from cold-tier on demand
for sample in snapshot {
    let s = sample?;
    println!("{} {}", s.timestamp, s.value);
}
```

### Tiered DataReader

Reader-Side: `take()` checks alle drei Tiers in Reihenfolge (hot → warm
→ cold), Cold-Hits sind opt-in (`reader.with_cold_lookup(true)`) weil
sie Latenz haben.

### CLI-Tool

`zerodds-persistence-admin`:

- `topology` — zeigt Tier-Belegung, Promotion-Stats
- `query` — Ad-hoc-Query auf den Cold-Store
- `compact` — manuelle Compaction
- `migrate` — Schema-Migrations für PG
- `export` — Cold-Tier zu Parquet/CSV/JSONL

### Spec-Coverage

Neue Vendor-Spec: `zerodds-persistence-1.0` mit Conformance-Profilen:
- L1: Persistence-Trait + Builtin-Engine
- L2: Postgres-Engine
- L3: S3/Object-Store-Engine
- L4: Time-Series-Aggregation + Downsampling
- L5: Multi-Tenancy + Per-Topic-Policies
- L6: Schema-Migrations + Hot-Reload

### Tests

- Unit: per-Tier (in-memory, mmap, in-process Postgres via testcontainers)
- Property-based: Promotion-Idempotency, Crash-Consistency
- Load: synthetic 10k Sensoren × 30 Tage (1.5 GiB simulierte Daten),
  100k Sensoren × 24h für Stress
- Cross-Vendor: Postgres 14/15/16 + TimescaleDB 2.x, MinIO + Ceph S3-Endpoint

### Performance-Targets

- Hot-Write: ≤ 1 µs add per sample (lock-free SPSC)
- Warm-Flush: ≥ 100k samples/s sustained, latency-budget < 50 µs p99
- Cold-Insert: batch-COPY in PG ≥ 50k rows/s
- Cold-Query: range-scan über 1M-Samples-Window in < 500 ms p50

## Out-of-Scope

- **GUI / Dashboard** für Persistence-Admin — UI ist anderes Sub-System
  (`tools/dashboard` deckt schon real-time, persistence-explorer kommt
  als RC3-Erweiterung dort)
- **Eigene DB-Engine** — wir nutzen PG/S3, bauen kein RocksDB-Fork
- **Stream-Processing-Operatoren** (window-functions, joins) — das ist
  Time-Series-DB-Job, wir liefern raw + Downsampling
- **HSM-/Erasure-Coding** für Cold-Store — kann via S3-Backend kommen,
  aber nicht in Engine

## Acceptance

1. Builtin-Engine startet, Topic mit 10k samples/sec läuft 30 min ohne
   Hot-Tier-OOM
2. Postgres-Engine: 100M Samples insertable + queryable mit p99 < 1s
3. S3-Engine: Daily-Partition-Upload + zstd-Compaction grün
4. Per-Topic-Override aus YAML wirkt zur Laufzeit
5. Crash-Recovery: WAL-style Warm-Tier rekonstruiert nach SIGKILL ohne
   Datenverlust
6. Spec `zerodds-persistence-1.0.md` published mit 0/0 partial/open
7. CLI `zerodds-persistence-admin query` funktioniert auf Live-Cold-Store

## Dependencies

- DCPS DataWriter/Reader Public-API (✅ stable seit RC1)
- crates/qos (✅ Durability-Policies bereits da)
- Externe: sqlx, aws-sdk-s3 (oder rust-s3), testcontainers für CI

## Risks

- **PG-Schema-Migration** in Production ist heikel. Mitigation: nur
  forward-Schema, sqlx-migrate baked-in, Schema-Version-Header.
- **Cold-Query-Latenz** kann user-Erwartungen sprengen. Mitigation: opt-in
  Flag, klare Doku dass Cold = Long-Term-Archive nicht Live-Source
- **Speicher-OOM** bei Hot-Tier-Backflow. Mitigation: configurable
  drop-vs-block-Policy + Prometheus-Counter
