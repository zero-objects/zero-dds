# Track RC3-B — OMG DDS Routing-Service

**Goal:** Implementation des OMG DDS-RS (Routing-Service) als
Daemon-Crate. Erlaubt cross-domain bridging und sample-transformation.

**Status:** 📋 todo

**Estimate:** 3-4 Personenwochen.

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
