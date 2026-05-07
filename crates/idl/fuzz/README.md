# `zerodds-idl` Fuzz-Targets

Coverage-guided Fuzzing des IDL-4.2-Parsers.

```bash
cd crates/idl
cargo +nightly fuzz run parse
```

Bekannte Findings (siehe `docs/test-harness/plan.md`):
* Finding 1: Stack-Overflow bei tiefer Modul-Verschachtelung (~128).
* Finding 2: Quadratisches Verhalten bei vielen Annotations.
