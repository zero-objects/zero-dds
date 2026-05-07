# `zerodds-amqp-bridge` Fuzz-Targets

Coverage-guided Fuzzing via `cargo-fuzz` (libFuzzer). Spec OASIS
AMQP-1.0 (transport / types / messaging).

## Requirements

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Running

```bash
cd crates/amqp-bridge
cargo +nightly fuzz run decode_frame_header
cargo +nightly fuzz run decode_value
cargo +nightly fuzz run decode_performative
cargo +nightly fuzz run decode_section
```

Jedes Target läuft endlos (`Ctrl-C` zum Stoppen). Crash-Inputs landen in
`fuzz/artifacts/<target>/`. Das Seed-Corpus in `fuzz/corpus/` ist
gitignored — die initialen Seeds werden im Repo gehalten, neue
Coverage-Funde landen lokal und werden bei Crash-Befund eingecheckt.

## Targets

| Target | Spec | Schützt vor |
|---|---|---|
| `decode_frame_header` | transport §2.3 | Length-/Offset-/DOFF-Validation, Extended-Header-Pfade |
| `decode_value` | types §1.3-§1.6 | Primitive-, Variable-Width-, Compound-Type-Pfade, list/map-Length-Prefix-Bypass, UTF-8 |
| `decode_performative` | transport §2.7 | Descriptor-Switch (open/begin/attach/...) + Composite-Body |
| `decode_section` | messaging §3.2 | Header/Properties/AppProperties/Body/Footer-Switch + Body |

## CI-Integration

Ein nightly-Job mit 10-Minuten-Budget pro Target ist Plan unter
WP TS-1 (siehe `docs/test-harness-plan.md`).

## Smoke-Alternative ohne nightly

Falls nightly nicht verfügbar: `crates/amqp-bridge/src/types.rs::tests`
deckt die Wire-Roundtrips für valide Inputs ab; Fuzzing ergänzt diese
mit Adversarial-Inputs.
