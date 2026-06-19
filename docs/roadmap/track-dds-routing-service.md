# Track RC3-B — OMG DDS Routing-Service

**Goal:** Implementation des OMG DDS-RS (Routing-Service) als
Daemon-Crate. Erlaubt cross-domain bridging und sample-transformation.

**Status:** 🔨 Kern geliefert (v1–v3) — `crates/routing-service`. Rest offen
(siehe „Delivered" unten).

**Estimate:** 3-4 Personenwochen.

## Delivered (2026-06-16) — `crates/routing-service`

Geliefert und getestet (19 Unit + 4 e2e gegen echte DDS-Participants, fmt +
clippy `--all-features --tests` sauber):

- **L1 Single-route domain-bridge** ✅ — Reader auf Domain A → Writer auf Domain
  B in einem Prozess, mit Topic-Rename.
- **L2 Multi-route + Filter** ✅ — beliebig viele Routes pro Instanz; DDS-SQL-
  Content-Filter pro Sample (`sql-filter`-Crate) gegen den dekodierten Body.
- **L3 Field-Rename** ✅ — rename / const-set / drop von Membern (Input-Shape →
  Output-Shape, byte-exakter XCDR1/XCDR2/appendable-DHEADER-Codec).
- **L4 Bidirektional** ✅ — Source ↔ Target; Participant-Loop-Guard verhindert
  router-interne Rückkopplung (e2e `loop_guard`).
- Zusätzlich: typ-agnostisches representation-treues Byte-Forwarding, per-
  Endpoint-QoS-Mapping (reliability/durability/ownership/partition/data-
  representation), keyed-Instance-Lifecycle-Forwarding (Dispose/Unregister,
  Spec §9.6.3.9), JSON- **und** XML-Config (nativ + RTI-Routing-Service-Subset),
  per-Route-Metriken im `zerodds-monitor`-Registry, Daemon `zerodds-router`
  (`run`/`validate`).

Noch offen (separat zu ziehen, kein Tech-Blocker):

- **L3 Throttle / Rate-Limit / Sample-Drop-Policy** — noch nicht implementiert.
- **L5 Persistence-Backed-Routes (Crash-Recovery)** — opt-in Durability-Backend
  (Andocken an Durability-Service-Track).
- **L6 Multi-Tenancy + ACL pro Route.**
- **Vendor-Spec** `docs/specs/zerodds-routing-service-1.0.md` (noch nicht
  publiziert) und Cross-Vendor-RS↔RS-Interop-Nachweis (Acceptance 5+6).
- YAML-Config (aktuell JSON/XML; YAML ist additiv).

## Motivation

DDS-Domains sind isoliert (per Domain-ID). In großen Deployments will
man oft Samples zwischen Domains routen (z.B. Lab-Domain ↔ Production-
Domain via Filtered-Mirror). Außerdem sample-transformation (rename
fields, filter, throttle) als deklarative Route-Config.

OMG-Spec: kein dedizierter "DDS-RS"-OMG-Standard, aber RTI Connext +
Cyclone DDS haben beide Routing-Services als de-facto-Standard.
ZeroDDS bietet eine kompatible OSS-Variante.

## In-Scope

### Crate

- `crates/routing-service` — Library-Crate
- `tools/zerodds-routing-bridged` — Daemon

### Konfiguration

```yaml
routes:
  - name: lab-to-prod-tempsensors
    source:
      domain: 7
      topic: "Sensors/Temperature.*"
      partition: "lab"
      qos:
        reliability: reliable
    target:
      domain: 0
      topic_prefix: "lab/"
      partition: "production"
      qos:
        reliability: best_effort
        durability: volatile
    transform:
      throttle: 10/s                    # max samples per second
      filter: "value > 100"             # SQL-filter expression
      rename_fields:
        - { from: "temp_c", to: "temperature_celsius" }
      rate_limit: 1MiB/s                # bytes-per-second cap
```

### Features

- Multi-domain (1 Routing-Service-Instance kann beliebig viele DDS-
  Participants über alle Domains halten)
- Bidirektional (Source ↔ Target)
- Filter (SQL-Filter, gleiches Crate wie sql-filter)
- Throttle, Rate-Limit, Sample-Drop-Policy
- Field-Rename (für API-Versionierung)
- Crash-Recovery (alle samples sind opt-in durable, persistence-Track-RS-A
  als optional Backend)

### Vendor-Spec

`docs/specs/zerodds-routing-service-1.0.md` mit:
- L1: Single-route domain-bridge
- L2: Multi-route + Filter
- L3: Field-Rename + Throttle
- L4: Bidirektional
- L5: Persistence-Backed-Routes (Crash-Recovery)
- L6: Multi-Tenancy + ACL pro Route

## Out-of-Scope

- **Wire-Protocol-Translation** zwischen DDS-Vendoren (das ist Bridges-
  Crate-Job, nicht Routing) — z.B. ZeroDDS↔Connext live ist via RTPS-
  Standard schon möglich, kein extra Routing-Service nötig
- **Aggregation/Joining** mehrerer Topics zu einem — das ist Stream-
  Processing-Domain, separater Track

## Acceptance

1. Daemon startet, liest YAML-Config, baut Routes
2. Sample auf source-domain wird auf target-domain geroutet
3. Filter-Expressions funktionieren (10 SQL-Filter-Tests grün)
4. Throttle: bei 10/s Config, source 1000/s, target ≤ 10/s
5. Vendor-Spec published, 0/0 partial/open
6. Cross-vendor: ZeroDDS-Routing-Service ↔ Cyclone-DDS-Routing-Service
   können einen Topic gegenseitig beobachten

## Dependencies

- DCPS-Public-API (✅)
- sql-filter Crate (✅)
- ggf. Datalake-Engine als opt-in Persistence-Backend
