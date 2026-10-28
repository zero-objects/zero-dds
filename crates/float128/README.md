# `zerodds-float128`

IEEE 754 binary128, in software, for the sixteen bytes CDR gives
`long double`.

```rust
use zerodds_float128::Float128;

// Sixteen bytes off the wire and sixteen back, unchanged.
let value = Float128::from_be_bytes(bytes);
assert_eq!(value.to_be_bytes(), bytes);

// A value a double can hold converts; one it cannot is refused by name.
let narrow: f64 = value.to_f64()?;
```

## Why it exists

CORBA 3.3 part 2 §9.3.1.3 gives `long double` one sign bit, a
fifteen-bit exponent and a 112-bit fraction, and DDS-XTypes 1.3 Table 31
names the result: quadruple precision, sixteen bytes, aligned to eight.
Rust has no stable `f128`, so this project used to write a `double` and
eight zero bytes for it — the right length, the wrong value, and no
error. Carrying the value needs no arithmetic, only bit manipulation and
long division, so the gap was a gap by choice.

## What it does

- Lossless sixteen-byte round trip in both byte orders. CORBA Figure 9.2
  reverses the whole sixteen bytes between orders, not two halves, and
  that is what RTI Connext was measured doing.
- Exact conversion from `f32`, `f64` and the integers, and back with the
  inexact cases returned as an `Inexact` carrying the rounded value —
  never rounded in silence.
- IEEE comparison, plus `total_cmp` for a `@key` member and a sorted
  expectation, where a `None` for NaN is not an answer.
- Decimal in and out, correctly rounded against the exact value, so a
  JSON value file can carry one. `to_shortest_decimal` writes the fewest
  digits that read back; `to_exact_decimal` writes all of them.
- The x87 80-bit extended format, in both directions, because Fast DDS
  and OpenDDS put *that* on the wire on x86-64 and a reader that only
  knows the conformant form cannot read the field.

## What it does not do

Arithmetic. No `Add`, no `Mul`, no `sqrt`. Nothing on the wire path needs
them and a half-correct one is worse than none;
`docs/float128-column.md` §6 says what adding them would take and how it
would be checked.

## The oracle

`tests/oracle/` holds 518 values run through GCC's `__float128` on the
Linux test host, compared in both directions: their printer against our
parser, and their parser against our printer. A crate that implements a
format cannot be its own second opinion.

See `docs/float128-column.md` for the vendor measurements this crate is
answering.
