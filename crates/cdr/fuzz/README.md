# `zerodds-cdr` fuzz targets

Coverage-guided fuzzing of the XCDR1/PL_CDR1 decoders via `cargo-fuzz`
(libFuzzer, nightly-only opt-in).

```bash
cd crates/cdr
cargo +nightly fuzz run read_pl_cdr1_member
cargo +nightly fuzz run read_all_pl_cdr1_members
```

Stable smoke variant: `crates/cdr/tests/fuzz_smoke.rs`
(runs under a normal `cargo test`).
