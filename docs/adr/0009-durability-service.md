# 0009 — Durability service (TRANSIENT + PERSISTENT) as a standalone, adapter-driven daemon

- **Status:** accepted
- **Date:** 2026-06-10
- **Authors:** @sandra
- **Context:** `crates/durability-store`, `crates/durability-service`, `crates/durability-client`, DDS 1.4 §2.2.3.4/§2.2.3.5

## Context

The DDS 1.4 durability ladder has four levels: `VOLATILE`,
`TRANSIENT_LOCAL`, `TRANSIENT`, `PERSISTENT`. The first two are
writer-local and finished in ZeroDDS. `TRANSIENT` and `PERSISTENT` are the
optional "service" levels: samples must survive the **death of the creating
writer process** (TRANSIENT) or a **system restart** (PERSISTENT)
and be delivered to late joiners without the original writer
still being alive.

Today only a **writer-embedded** backend exists
(`crates/dcps/src/durability_service.rs`, in-memory + file-per-sample). It
covers late joiners **as long as the writer process lives**, but is not a
real service: if the writer process dies, the TRANSIENT samples are gone;
after a reboot nobody re-announces the PERSISTENT files. The
vendor feature matrix accordingly shows "Transient/Persistent (Service)" as a
deficit (cross-vendor wire replay is missing entirely).

Two existing planning docs contradicted each other:
`internal/plans/milestone-v1.2.md` WP 2.2c wanted a fresh daemon backed by sqlite WAL;
`internal/roadmap/track-dds-persistence-service.md` (RC3-C)
wanted to wrap a non-existent "RC2-A-Datalake-Engine" stack in a spec-conformant
way. This ADR resolves the contradiction.

The design session produced clear requirements: full storage
interface; warehouse/lake adapters bring their **own** analytics
interfaces (no generic query layer in our code);
retention is **exclusively** the QoS contract; the only service-side
control is the **hot cache size in memory**, disk is the lossless
inverse.

## Decision

A **standalone durability daemon per domain** (`bin/zerodds-durability-svc`)
over an **adapter-driven storage abstraction**. Three-layer model:

```
① Daemon (durability-service): DCPSDurability discovery, RTPS ingest/replay, startup sync
②a Hot tier  (TieredStore): memory working set, LRU, write-/read-through  ← SERVICE CONFIG: memory_budget
②b Storage abstraction (DurabilityStore trait, FULL): store/query/replay/unregister/cleanup/stats
   │                                                                       ← BASIS: topic QoS (contract)
③ Cold adapter: sqlite-WAL · file · lakehouse(Parquet+DuckDB)
       └─ each one additionally with its OWN native read/query interface (analytics, out-of-band)
```

**Invariants (normative):**

1. **QoS is the contract.** `DurabilityServiceQosPolicy` + `Lifespan`
   (history_depth, max_samples, max_instances, max_samples_per_instance,
   service_cleanup_delay) determine **how much must be held in total and
   delivered to every late joiner**. Only this contract limits retention.
   The cold store enforces it (drop-oldest on cap violation + `RESOURCE_LIMITS`
   event, never silently).

2. **The memory budget is NOT a retention knob.** It only determines how much
   of the contracted history lies hot in RAM. The rest lies on disk and is
   **always** streamed in losslessly on late join — service config must not
   cap that, otherwise it would break the contract.

3. **The cold adapter is authoritative for the full contracted history**
   (write-through). The hot tier is a bounded read-/write-through cache.
   `replay`/`query` **stream** lazily/paginated from the cold store (`SampleStream`,
   never "everything in RAM").

4. **TRANSIENT_LOCAL stays writer-local** (no service needed);
   **TRANSIENT/PERSISTENT** move into the service.

5. **Cross-vendor falls out of the protocol**: from the RTPS point of view
   the service is a normal participant — it ingests as a
   `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL` reader and replays as a
   `TRANSIENT_LOCAL(KEEP_ALL)` writer. A Cyclone/FastDDS `TRANSIENT` writer
   is thus recorded automatically and replicated to any reader.

**Adapter boundary:** The trait covers only the DDS durability path
(store/replay/historical-query). Analytics/read access goes through the
**native interface of the adapter** (SQL on sqlite, DuckDB/Parquet for the
lakehouse) — out-of-band, not through the trait.

**Crate split:**

- `crates/durability-store` — trait + model (`DurabilitySample`, `Selector`,
  `SampleStream`) + `TieredStore` + retention engine (adapter-agnostic).
- `crates/durability-store-sqlite` — sqlite WAL adapter (default, "simple DB").
- `crates/durability-store-file` — file-per-sample adapter (extracted from `dcps`, no dependencies).
- `crates/durability-store-lakehouse` — Parquet + DuckDB; house mode
  (structured SQL tables/views) **and** lake mode (raw Parquet,
  schema-on-read) in one.
- `crates/durability-client` — participant-side client, registers local
  TRANSIENT/PERSISTENT topics with the service.
- `crates/durability-service` — daemon logic.
- `bin/zerodds-durability-svc` — binary (systemd/Docker), **one daemon per domain**.

## Alternatives

1. **Keep/extend the writer-embedded backend** (status quo) — rejected: not a
   real service, dies with the writer process, no cross-vendor replay,
   no PERSISTENT reboot guarantee.
2. **Wrap the "RC2-A-Datalake-Engine"** (roadmap RC3-C) — rejected: the engine
   does not exist (no crate), the plan was hanging in the air. This ADR replaces
   it: the lakehouse is *an adapter*, not an engine stack to be built first.
3. **Only sqlite, no lakehouse** (milestone-v1.2 WP 2.2c, narrow) — rejected: the
   adapter-driven split costs little more and delivers warehouse/lake in
   one go; "house vs lake" is the same adapter via DuckDB over Parquet.
4. **Configurable late-join disk budget** (intermediate state of the design session)
   — rejected: would have silently cut the QoS contract. Disk is the
   inverse of the memory budget, not cappable independently (invariant 2).
5. **Multi-domain daemon** — deferred: one daemon per domain is the
   simpler, isolated default; multi-domain as a later config extension.

## Consequences

**Positive:** real TRANSIENT/PERSISTENT semantics (survives the writer process or
a reboot); cross-vendor replay practically for free via RTPS; one trait, any
adapters (from simple sqlite to lakehouse analytics); vendor matrix
"Transient/Persistent (Service)" becomes ✓; clean separation of contract (QoS) vs.
performance (memory budget).

**Negative/risks:** new deployment component (daemon) — operating effort
(systemd/Docker, backup); DuckDB/Parquet pull in C dependencies (keep the lakehouse
adapter feature-gated); cold-store streaming must be paginated, otherwise
RAM blowup with large contracts.

**Follow-up tasks:** P1 store core + sqlite/file; P2 daemon + client + discovery
+ RTPS; P3 lakehouse; P4 crash recovery (5 scenarios) + cross-vendor; P5
packaging + deployment doc + matrix update.

## Addendum: origin of the contract and multiple writers

Invariant 1 says "QoS is the contract", but did not specify *where* the
daemon takes a topic's `DurabilityServiceQosPolicy` from. The first daemon
(`enable_auto_discovery`) never read it from the discovered writer — every
auto-discovered topic got the daemon's own `default_contract`, regardless
of what the writer itself announced in its `PID_DURABILITY_SERVICE` (DATA(w)).
`crates/durability-service/src/lib.rs` now reads the contract
via `Contract::from_qos` from the discovered policy, falling back to
`default_contract` only if the announced policy exactly equals the DDS default
(`KEEP_LAST(1)`, all caps unlimited) — on the wire indistinguishable from
"never set".

**Two writers, one topic, different `DurabilityServiceQosPolicy`:**
DDS 1.4 §2.2.3.5 defines no merge rule for this, and changing the contract of a
store afterwards while it already holds data under the old one
is not a well-defined operation (KEEP_LAST(2) → KEEP_LAST(1) in the middle of
operation would have to decide which held sample to discard, without
the writer giving a signal for it). Decision: the **first** discovered
writer fixes the contract for the lifetime of the topic; a later
writer with a *differing* policy does not change it — the deviation is
reported (`ServiceEvent::ConflictingDurabilityService`), not silently
ignored. An explicit contract entry in the daemon configuration
(`durability.topics[].contract`, `zerodds-durability-svc(5)`) overrides both.

Same addendum for keyedness: `enable_auto_discovery` so far set `keyed`
hard to `false` ("Increment-1 scope: unkeyed"); a keyed topic
ran as a single fake instance, KEEP_LAST/Dispose/`max_instances` acted
incorrectly for every keyed topic. Keyedness now comes from the RTPS `entityKind`
octet of the discovered writer (DDSI-RTPS 2.5 §9.3.1.2), not from an
assumption.

## References

- DDS 1.4 §2.2.3.4 (Durability), §2.2.3.5 (DurabilityService)
- `internal/plans/milestone-v1.2.md` §WP 2.2c (made concrete by this ADR)
- `internal/roadmap/track-dds-persistence-service.md` (replaced by this ADR)
- `crates/dcps/src/durability_service.rs` (writer-embedded predecessor)
- `crates/qos/src/policies/durability_service.rs` (contract QoS)
