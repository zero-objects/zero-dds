# `zerodds-cdr` Fuzz-Targets

Coverage-guided Fuzzing der XCDR1/PL_CDR1-Decoder via `cargo-fuzz`
(libFuzzer, nightly-only opt-in).

```bash
cd crates/cdr
cargo +nightly fuzz run read_pl_cdr1_member
cargo +nightly fuzz run read_all_pl_cdr1_members
```

Stable-Smoke-Variante: `crates/cdr/tests/fuzz_smoke.rs`
(läuft in normalem `cargo test`).
