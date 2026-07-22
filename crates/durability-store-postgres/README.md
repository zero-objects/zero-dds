# zerodds-durability-store-postgres

PostgreSQL cold adapter for the ZeroDDS Durability-Service ([ADR 0009]) — the
shared, long-term `PERSISTENT` tier for large fleets, the *PostgreSQL* leg of
the RAM/SSD/PostgreSQL tiered story.

It implements `zerodds_durability_store::DurabilityStore` with the same
semantics as the default sqlite adapter: one row per sample,
`(topic, instance, sequence)` is the sample identity (a reliable re-send is an
idempotent replace), and retention is bounded **only** by the topic QoS
`Contract` (ADR 0009 invariant 1).

```rust,no_run
use zerodds_durability_store::Contract;
use zerodds_durability_store_postgres::PostgresStore;

let store = PostgresStore::connect(
    "postgres://zerodds@db/zerodds",
    Contract::keep_all(),
)?;
# Ok::<(), zerodds_durability_store::StoreError>(())
```

## Tiering

Wrap it in a `TieredStore` for a bounded RAM hot-cache in front of the shared
database:

```rust,no_run
# use zerodds_durability_store::{Contract, TieredStore};
# use zerodds_durability_store_postgres::PostgresStore;
let cold = PostgresStore::connect("postgres://zerodds@db/zerodds", Contract::keep_all())?;
let store = TieredStore::new(cold, 256 * 1024 * 1024); // 256 MiB hot budget
# Ok::<(), zerodds_durability_store::StoreError>(())
```

## Features

- `timescaledb` — partition the `samples` table as a TimescaleDB hypertable on
  `created_nanos` (requires the extension in the target database). Sample
  identity is preserved by an explicit delete-then-insert on
  `(topic, instance, sequence)`.

## Connection security

The client connects with `NoTls`. Put the daemon and database on a trusted
segment, or front the database with a TLS-terminating proxy.

## Analytics

Connect to the same database with any SQL/BI tool; the schema is stable and
documented in the crate docs (ADR 0009: adapters expose their own native read
interface alongside the DDS path).

[ADR 0009]: https://github.com/zero-objects/zero-dds/blob/main/docs/adr/0009-durability-service.md
