# Deploying the ZeroDDS Durability-Service

The `zerodds-durability-svc` daemon (ADR 0009) provides the DDS `TRANSIENT` and
`PERSISTENT` durability levels: it ingests samples for the topics it serves and
replays them to late-joining readers **after the original writer's process has
died** (`TRANSIENT`) and **across a full service/system restart**
(`PERSISTENT`). One daemon per DDS domain.

> `TRANSIENT_LOCAL` needs **no** service — the writer holds its own history.
> Only `TRANSIENT` and `PERSISTENT` use the daemon.

## Quick start

```bash
# sqlite-backed (default), auto-serving every TRANSIENT/PERSISTENT topic on domain 0
zerodds-durability-svc --domain 0 --store sqlite --path /var/lib/zerodds/durability/dur.db --auto

# explicit topics instead of auto-discovery
zerodds-durability-svc --domain 0 --store file --path /var/lib/zerodds/durability \
    --topic SensorData --topic Commands
```

`--auto` observes the standard `DCPSPublication` discovery and serves any topic
whose writers declare `TRANSIENT`/`PERSISTENT` durability — no application-side
code needed. `--topic NAME` (repeatable) serves a fixed set; combine or use
either.

## Store adapters (`--store`)

| Adapter | `--path` | Survives restart | Use for |
|---|---|---|---|
| `sqlite` (default) | a `.db` file | ✅ (WAL, ACID) | the general case — transactional, low overhead |
| `file` | a directory | ✅ (one file per sample) | dependency-free; simple inspection / rsync backup |
| `lakehouse` | a `.duckdb` file | ✅ | when you also want SQL analytics (DuckDB) / Parquet export of the history |

Omitting `--path` for `sqlite`/`lakehouse` uses an **in-memory** store — fast but
**not durable across restart** (the daemon warns). `file` requires `--path`.

Retention is the **topic's QoS contract** (`DurabilityServiceQosPolicy` —
history depth, `max_samples`/`max_instances`, cleanup delay); the daemon's
default contract is `KEEP_ALL` (hold the full history). The store enforces it
(drop-oldest / `RESOURCE_LIMITS`); the memory footprint of a tier-cache is a
library knob (`TieredStore`) not yet exposed on the CLI.

## systemd

```bash
sudo install -m644 packaging/linux/systemd/zerodds-durability-svc.service \
    /etc/systemd/system/
sudo install -d -o zerodds -g zerodds /var/lib/zerodds/durability
# Edit the Environment= lines (domain, store, path) or drop a per-domain override:
sudo systemctl edit zerodds-durability-svc        # e.g. set ZERODDS_DURABILITY_DOMAIN=7
sudo systemctl enable --now zerodds-durability-svc
```

`Type=exec`, graceful `SIGTERM` stop (the daemon joins its pumps and exits 0),
`Restart=on-failure`. The unit is hardened (`ProtectSystem=strict`, the store
dir is the only `ReadWritePaths`). For several domains, copy the unit to
`zerodds-durability-svc@.service` style or one unit per domain with distinct
`--path`.

## Docker

```bash
docker build -f packaging/docker/durability-svc/Dockerfile -t zerodds/durability-svc:1.0 .
docker run -d --name dur --network host \
    -v zerodds-dur:/var/lib/zerodds/durability \
    zerodds/durability-svc:1.0 --domain 0 --store sqlite \
    --path /var/lib/zerodds/durability/dur.db --auto
```

`--network host` is the simplest way to share the host's DDS multicast with the
applications. The DuckDB lakehouse adapter is bundled (the image carries
`libstdc++6`).

## Backup

The store **is** the durable state — back up `--path`:

* **sqlite**: copy the `.db` (+ `-wal`, `-shm`) while idle, or use
  `sqlite3 dur.db ".backup '/backup/dur.db'"` online (WAL-safe).
* **file**: `rsync -a /var/lib/zerodds/durability/ /backup/`.
* **lakehouse**: copy the `.duckdb`, or `COPY … TO '….parquet'` for an
  analytics-friendly export (see the adapter's `export_parquet`).

On restart the daemon re-primes its replay writers from whatever the store
holds (startup-sync) — so a restored backup resumes serving that history.

## Operational notes

* **Restart safety**: every `store` is committed durably, so an abrupt `kill -9`
  loses nothing committed; the next start re-serves from the store (validated:
  `crates/durability-service-bin/tests/crash_recovery.rs`).
* **One daemon per domain**: running two on the same domain+topic is redundant
  and both will replay (readers may see duplicates).
* **Cross-vendor**: from RTPS the service is an ordinary `TRANSIENT_LOCAL(KEEP_ALL)`
  writer, so a foreign-vendor reader can consume its replay over standard RTPS.
  (Foreign-vendor *writers* feeding the service are a separate validation axis.)
