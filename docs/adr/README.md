# Architecture Decision Records

Hier werden nicht-triviale Architektur-Entscheidungen als ADR dokumentiert.
Verbindlich gemaess `docs/architecture/07_risks_and_strategy.md §4.2 Bus-Factor`.

## Nummerierung

ADRs sind durchnummeriert als `NNNN-kurzer-titel.md`, beginnend bei `0001`.
Einmal vergebene Nummern werden nicht wiederverwendet — superseded ADRs
bleiben bestehen und verweisen auf den Nachfolger.

## Template

Neue ADRs starten mit `_template.md` als Kopiervorlage.

## Status-Lifecycle

```
proposed  ->  accepted  ->  superseded
                     \->  rejected
                     \->  deprecated
```

## Index

| Nr. | Titel | Status | Datum |
|---|---|---|---|
| 0001 | [Vendor-Spec-Strategie für Lücken im OMG-Ökosystem](0001-vendor-spec-strategie.md) | accepted | 2026-05-04 |
| 0002 | [async-DDS-API runtime-agnostic mit Tokio-Glue als Optional](0002-async-api-runtime-agnostic.md) | accepted | 2026-05-04 |
| 0003 | [Flatdata Backend-Trait (in-memory + POSIX-mmap)](0003-flatdata-backend-trait.md) | accepted | 2026-05-04 |
| 0004 | [Iceoryx2 als optional Backend (Build + Config)](0004-iceoryx2-bridge-optional.md) | accepted | 2026-05-04 |
| 0005 | [Flatdata Dual-Stack: DCPS-Integration als opt-in Feature](0005-flatdata-dual-stack-opt-in.md) | accepted | 2026-05-04 |
| 0006 | [PID_SHM_LOCATOR als Vendor-PID 0x8001](0006-pid-shm-locator-vendor-pid.md) | accepted | 2026-05-04 |
