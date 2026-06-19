# `zerodds-amqp-bridge` Fuzz-Targets

Coverage-guided fuzzing via `cargo-fuzz` (libFuzzer). Spec OASIS
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

Each target runs indefinitely (`Ctrl-C` to stop). Crash inputs land in
`fuzz/artifacts/<target>/`. The seed corpus in `fuzz/corpus/` is
gitignored — the initial seeds are kept in the repo, new
coverage finds land locally and are checked in when a crash is found.

## Targets

| Target | Spec | Protects against |
|---|---|---|
| `decode_frame_header` | transport §2.3 | Length / offset / DOFF validation, extended-header paths |
| `decode_value` | types §1.3-§1.6 | Primitive, variable-width, compound-type paths, list/map length-prefix bypass, UTF-8 |
| `decode_performative` | transport §2.7 | Descriptor switch (open/begin/attach/...) + composite body |
| `decode_section` | messaging §3.2 | Header/Properties/AppProperties/Body/Footer switch + body |

## CI-Integration

A nightly job with a 10-minute budget per target is planned under
WP TS-1 (see `docs/test-harness-plan.md`).

## Smoke alternative without nightly

If nightly is not available: `crates/amqp-bridge/src/types.rs::tests`
covers the wire round-trips for valid inputs; fuzzing complements these
with adversarial inputs.
