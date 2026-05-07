# `zerodds-rtps` Fuzz-Targets

Coverage-guided Fuzzing via `cargo-fuzz` (libFuzzer).

## Requirements

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Running

```bash
# 1) Corpus seeden (einmalig, aus tests/fixtures/cyclone)
bash crates/rtps/fuzz/scripts/seed-corpus.sh

# 2) Fuzzer starten
cd crates/rtps
cargo +nightly fuzz run decode_datagram
cargo +nightly fuzz run fragment_assembler
cargo +nightly fuzz run submessage_decoders
```

Jedes Target läuft endlos (`Ctrl-C` zum Stoppen). Crash-Inputs landen in
`fuzz/artifacts/<target>/`. Das Seed-Corpus in `fuzz/corpus/` ist
gitignored — beim ersten Checkout via `seed-corpus.sh` neu erzeugen.

## Quick-Fuzz-Alternative (stable)

Für Continuous-Integration ohne nightly siehe
`crates/rtps/tests/fuzz_smoke.rs` — pseudorandom Byte-Streams auf allen
Wire-Decodern. Erwischt triviale Panics, aber keine Coverage-Guidance.

## Targets

| Target | Input | Schützt vor |
|---|---|---|
| `decode_datagram` | Random Bytes | Panics im Top-Level-Decoder |
| `fragment_assembler` | Random DATA_FRAG | Panics bei pathologischen Fragments, DoS-Cap-Bypass |
| `submessage_decoders` | Random Bytes je Submessage-Typ | Per-Submessage-Parser-Robustheit |

## Phase-2-Follow-up

- AFL.rs als zweites Fuzzing-Tool (coverage-Algorithmen ergänzen sich)
- Bessere Corpus-Seeds sobald Cyclone-DATA_FRAG-Captures verfügbar sind
  (aktuell: nur DATA + HEARTBEAT im Cyclone-Corpus, kein DATA_FRAG)
- CI-Integration: nightly-Job mit 10-Minuten-Budget pro Target.
