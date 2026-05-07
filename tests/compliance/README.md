# Compliance-Test-Suite (WP 1.10)

Golden-Vector-Tests fuer Wire-Level-Kompatibilitaet mit DDS-Spec.

## Stand Phase 1

Aktuelle Vectors sind aus **Spec-Definition** (RFC 1321 MD5, DDS-Spec-
Tabellen) und aus **unserem eigenen Encoder** erzeugt. Sie erkennen
Encoder-Regressionen, aber nicht Spec-Drift gegen Cyclone/Fast-DDS.
Cross-Impl-Captures (Wireshark-Dumps) folgen, sobald der Live-Interop-
Harness (WP 1.11) stabil gegen Cyclone/Fast-DDS laeuft.

## Struktur

```text
tests/compliance/
├── README.md                    — diese Datei
├── rtps/                        — RTPS-Submessage-Vectors (T2)
│   └── heartbeat_minimal_le.hex
├── xcdr2/                       — XCDR2-Encode/Decode-Vectors (T3)
│   └── int32_le.hex
├── typeobject/                  — EquivalenceHash + TypeObject (T4)
│   └── md5_empty_string.hex
└── qos_pid/                     — PL_CDR-Value-Bytes pro QoS (T5)
    └── durability_transient_local_le.hex
```

Jeder Vector ist ein `.hex`-File mit Kommentarzeilen (`# …`) und
Rohbytes (`DE AD BE EF …`).

## Test-Setup

Tests leben **pro Crate** in `tests/compliance_<domain>.rs`:

- `crates/rtps/tests/compliance_rtps.rs`  → RTPS-Submessages.
- `crates/cdr/tests/compliance_xcdr2.rs`  → XCDR2-Primitive.
- `crates/types/tests/compliance_typeobject.rs` → Hash/TypeObject.
- `crates/qos/tests/compliance_qos_pid.rs` → QoS-Wire.

Lauf per `cargo test --workspace --test compliance_*`.

## CI-Integration

GitLab-CI-Job `compliance` (Stage `test`) laeuft die vier Test-Targets
auf jeder Pipeline. Live-Interop gegen Cyclone/Fast-DDS ist separater
Job `live-interop` (Stage `interop`, manual, WP 1.11).

## Erweitern

Neuen Vector hinzufuegen:

1. `.hex`-File unter `tests/compliance/<domain>/` ablegen, mit
   Spec-Referenz im Header-Kommentar.
2. `#[test]`-Funktion in `crates/<crate>/tests/compliance_<domain>.rs`:
   Fixture laden, dekodieren, gegen Erwartung pruefen, byte-identisch
   re-encoden.
3. Neues File im jeweiligen Subdir-README eintragen.
