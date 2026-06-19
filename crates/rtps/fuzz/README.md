# `zerodds-rtps` fuzz targets

Coverage-guided fuzzing via `cargo-fuzz` (libFuzzer).

## Requirements

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Running

```bash
# 1) Seed the corpus (one-time, from tests/fixtures/cyclone)
bash crates/rtps/fuzz/scripts/seed-corpus.sh

# 2) Start the fuzzer
cd crates/rtps
cargo +nightly fuzz run decode_datagram
cargo +nightly fuzz run fragment_assembler
cargo +nightly fuzz run submessage_decoders
```

Each target runs endlessly (`Ctrl-C` to stop). Crash inputs land in
`fuzz/artifacts/<target>/`. The seed corpus in `fuzz/corpus/` is
gitignored — regenerate it on first checkout via `seed-corpus.sh`.

## Quick-fuzz alternative (stable)

For continuous integration without nightly see
`crates/rtps/tests/fuzz_smoke.rs` — pseudorandom byte streams on all
wire decoders. Catches trivial panics, but no coverage guidance.

## Targets

| Target | Input | Protects against |
|---|---|---|
| `decode_datagram` | Random bytes | Panics in the top-level decoder |
| `fragment_assembler` | Random DATA_FRAG | Panics on pathological fragments, DoS-cap bypass |
| `submessage_decoders` | Random bytes per submessage type | Per-submessage parser robustness |

## Phase-2 follow-up

- AFL.rs as a second fuzzing tool (coverage algorithms complement each other)
- Better corpus seeds once Cyclone DATA_FRAG captures are available
  (currently: only DATA + HEARTBEAT in the Cyclone corpus, no DATA_FRAG)
- CI integration: nightly job with a 10-minute budget per target.
