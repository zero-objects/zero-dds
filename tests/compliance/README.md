# Compliance test suite (WP 1.10)

Golden vector tests for wire-level compatibility with the DDS spec.

## State of phase 1

The current vectors are generated from the **spec definition** (RFC 1321 MD5, DDS spec
tables) and from **our own encoder**. They detect
encoder regressions, but not spec drift against Cyclone/Fast-DDS.
Cross-implementation captures (Wireshark dumps) follow as soon as the live interop
harness (WP 1.11) runs stably against Cyclone/Fast-DDS.

## Structure

```text
tests/compliance/
├── README.md                    — this file
├── rtps/                        — RTPS submessage vectors (T2)
│   └── heartbeat_minimal_le.hex
├── xcdr2/                       — XCDR2 encode/decode vectors (T3)
│   └── int32_le.hex
├── typeobject/                  — EquivalenceHash + TypeObject (T4)
│   └── md5_empty_string.hex
└── qos_pid/                     — PL_CDR value bytes per QoS (T5)
    └── durability_transient_local_le.hex
```

Each vector is a `.hex` file with comment lines (`# …`) and
raw bytes (`DE AD BE EF …`).

## Test setup

Tests live **per crate** in `tests/compliance_<domain>.rs`:

- `crates/rtps/tests/compliance_rtps.rs`  → RTPS submessages.
- `crates/cdr/tests/compliance_xcdr2.rs`  → XCDR2 primitives.
- `crates/types/tests/compliance_typeobject.rs` → hash/TypeObject.
- `crates/qos/tests/compliance_qos_pid.rs` → QoS wire.

Run with `cargo test --workspace --test compliance_*`.

## CI integration

The GitLab CI job `compliance` (stage `test`) runs the four test targets
in every pipeline. Live interop against Cyclone/Fast-DDS is the separate
job `live-interop` (stage `interop`, manual, WP 1.11).

## Extending

Adding a new vector:

1. Put the `.hex` file under `tests/compliance/<domain>/`, with the
   spec reference in the header comment.
2. `#[test]` function in `crates/<crate>/tests/compliance_<domain>.rs`:
   load the fixture, decode, check against the expectation, re-encode
   byte-identically.
3. Add the new file to the README of the respective subdirectory.
