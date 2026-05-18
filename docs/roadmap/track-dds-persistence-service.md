# Track RC3-C — OMG DDS Persistence-Service (spec-conformant Wrapper)

**Goal:** der in RC2-A gebaute Datalake-Engine-Stack bekommt einen
spec-conformanten Wrapper, der das OMG-DDS-Persistence-Service-Pattern
erfüllt, sodass externe Vendoren das als Standard-Conformance-Item
ansprechen können.

**Status:** 📋 todo (gated auf RC2-A Datalake-Engine)

**Estimate:** 2-3 Personenwochen (gering, weil Engine schon existiert).

## Motivation

DDS 1.4 spec mentions Persistence-Service als optionales Profil. Bisher
existiert kein OMG-Standard-Spec dazu, aber die Vendoren (RTI, Cyclone)
haben de-facto-Patterns:
- DataWriter mit `Durability=PERSISTENT` schreibt in den Service
- Service-side ist persistente Storage
- DataReader können nach Restart die durable samples replay-en

Wenn unser Datalake-Engine (RC2-A) als spec-conformant gewrappt wird,
können wir auf der Website klar behaupten "DDS-PS-conformant" — das ist
ein deutlicher Unterscheidungsmerkmal.

## In-Scope

### Crate

- `crates/dcps-persistence-service` — Daemon-Crate, wraps die Datalake-
  Engine in das DDS-Persistence-Pattern
- `tools/zerodds-persistence-service-bridged` — Daemon

### Pattern

```
Publisher [Durability=PERSISTENT]
    │
    ▼
zerodds-persistence-service-bridged  (separate participant)
    │ ── persistiert über Datalake-Engine (Hot/Warm/Cold)
    │
    ▼
Late-Joining Reader [Durability=PERSISTENT]
    └── empfängt durable samples-replay
```

### Konfiguration

```yaml
persistence_service:
  domain_id: 0
  topics:                         # which topics get persistence
    - "Sensors/Critical.*"
    - "Commands/.*"
  storage:
    engine: from-datalake-config  # reuse RC2-A engine
    config_path: /etc/zerodds/persistence.yaml
  durability_qos: persistent      # vs. transient (in-memory replay)
  replay_on_match:
    history: keep_last(1000)
    timeout: 30s
```

### Differenz zu RC2-A Datalake

| Aspekt | RC2-A Datalake | RC3-C Persistence-Service |
|---|---|---|
| Trigger | per-topic-config | auto on Durability=PERSISTENT-QoS |
| Pattern | jeder Reader hat persistence-API | DataReader[PERSISTENT] auto-replay |
| Spec-Conformance | ZeroDDS-Vendor-Spec | DDS-Spec-Profile-Conformance |
| Use-Case | Datalake / Long-Term-Analytics | Late-Joiner-Replay |

Die beiden sind **komplementär** — Datalake ist die untere Schicht,
PS ist der spec-konforme Wrapper.

### Spec-Coverage

- Existierender `dds-dcps-1.4.md` bekommt §8.x (Optional Profile
  Persistence) als done annotiert (war vorher n/a-default)
- Neue Vendor-Spec `zerodds-persistence-service-1.0.md` als
  Implementation-Note

### Tests

- Crash-and-Replay: Publisher schreibt 1000 samples mit
  Durability=PERSISTENT, Service crasht und restartet, Late-Joining-
  Reader bekommt alle 1000 zurück
- Cross-vendor: ZeroDDS-Persistence-Service kann einen Cyclone-DDS-
  Late-Joiner mit Durability=PERSISTENT bedienen

## Out-of-Scope

- **Eigene Storage-Engine** — wir nutzen die RC2-A Datalake-Engine
- **Multi-Service-Replication** — wenn 2 PS-Instances am gleichen Topic
  hängen, ist Konflikt-Resolution Sache von Datalake-Engine, nicht PS-
  Crate

## Acceptance

1. Service-Daemon startet, persistiert PERSISTENT-Topics
2. Restart + Late-Join: alle samples zurück
3. Cross-vendor: Cyclone-Late-Joiner mit `--durability=persistent`
   bekommt samples
4. Vendor-Spec published, 0/0 partial/open
5. dds-dcps-1.4-coverage erweitert um Persistence-Profile

## Dependencies

- RC2-A Datalake-Engine (Pflicht — wir wrappen sie nur)
- DCPS-Public-API für Late-Joiner-Replay (✅)
