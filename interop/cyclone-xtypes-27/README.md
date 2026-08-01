# CycloneDDS ↔ ZeroDDS XTypes interop matrix (issue #27)

A reproducible, live DCPS-over-UDP interop matrix between ZeroDDS and
CycloneDDS on domain 100, topic `robot` (`struct Robot { uint32 id; uint32 label; }`).

It pins down issue #27: an un-annotated struct is `@final` under CycloneDDS'
generator default but `@appendable` under ZeroDDS' default. Under XCDR1 that is
invisible (an `@appendable` type emits no DHEADER, so the wire is identical to
`@final`); force XCDR2 and the framing differs (DHEADER present vs not), so a
`@final` writer and an `@appendable` reader stop understanding each other.

## What it checks (separately: match, samples, decode errors)

| Case | Writer (Cyclone) | Reader (ZeroDDS) | Expected |
|---|---|---|---|
| 1 | final, XCDR1 | `@appendable` (default) | match, samples > 0, 0 errors |
| 2 | appendable, XCDR2 | `@appendable` (default) | match, samples > 0, 0 errors |
| 3 | final, XCDR2 | `@appendable` (default) | **match, 0 samples, errors > 0** (the #27 symptom) |
| 4 | final, XCDR2 | `@final` (`--cyclone`) | match, samples > 0, 0 errors (the fix) |
| 5 | — (reverse) | ZeroDDS `@appendable`/XCDR2 writer → Cyclone reader | samples > 0 |

Case 3 is the reporter's failure: the endpoints **match** (no incompatible
QoS), but every sample fails to decode. Crucially the decode error is *not*
silent — the ZeroDDS reader's `take()` returns `WireError`; the counters here
report `errors > 0`. Case 4 shows the fix: generating the ZeroDDS reader type
with `zerodds-idlc --cyclone` (which defaults un-annotated aggregates to
`@final`) makes the same Cyclone writer interoperate.

## Requirements & running

Needs a Python that can `import cyclonedds` plus the CycloneDDS C library.
The script **loud-skips (exit 0)** when they are absent, so it is safe to
invoke unconditionally in CI.

```
PYBIN=/path/to/venv/bin/python3 CYCLONEDDS_HOME=/path/to/cyclone \
  interop/cyclone-xtypes-27/run_matrix.sh
```

`PYBIN` must point at a Python that can `import cyclonedds`; `CYCLONEDDS_HOME`
at the matching CycloneDDS C install prefix. The runner exits non-zero if any
case deviates from the table above.

**Reference vendor:** CycloneDDS 11.0.1. CycloneDDS 0.10.5 is a manual
compatibility check (same outcomes observed); it is not the CI reference.

## Layout

- `robot.idl` — the shared type.
- `reader/` — standalone ZeroDDS reader/writer crate (own `[workspace]`, so a
  root `cargo build` ignores it). `src/robot.rs` is generated per case by the
  runner (git-ignored) — `@appendable` by default, `@final` via `--cyclone`.
- `writers/cyclone_writer.py`, `writers/cyclone_reader.py` — CycloneDDS peers,
  parameterized by extensibility and representation.
- `run_matrix.sh` — orchestrator; reports match / sample / error counts per case.
