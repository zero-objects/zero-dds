# OMG IDL 4.2 — Open + Partial Items (aggregate)

Auto-generated from `idl-4.2.md` at `Spec-Check 4.0` (post-K1 completion).
Regenerate before each audit run (PROCESS.md).

## Open items

### §7.4.1.4.3-r-longdouble-open — long-double full-precision arithmetic

**Status:** **MISSING — BLOCKED** (iron-rule escalation clause).

**Spec:** §7.4.1.4.3 — long-double sub-expression precision +
IEEE-754 double-extended (≥80-bit mantissa).

**Blocker:** Rust stable has no `f128` type
([rust-lang/rust#116909](https://github.com/rust-lang/rust/issues/116909)).
Realistically 2027 at the earliest.

**Rejected alternatives:** nightly switch (CI stability), a third-party
`f128` crate (FFI risk), an own 128-bit soft-float (~500 LOC; maintainer
quality > own-impl quality).

**Re-audit trigger:** `f128` available in Rust stable →
`ConstValue::LongDouble` from `[u8; 16]` to `f128`,
`parse_floating`/`apply_binary` long-double arms spec-conformant.

## Partial items (all long-double-related, all BLOCKED-linked)

### §7.4.1.4.3 — long-double type tag (idl-4.2.md L3396)

`ConstValue::LongDouble` accepted as a 16-byte stub; arithmetic degrades
to f64. Linked to `§7.4.1.4.3-r-longdouble-open`.

### §7.4.1.4.3 — floating-point expression eval rule (idl-4.2.md L3514)

The `promote_float` long-double arm is a stub. The double branch +
range check for double are spec-conformant.

### §7.4.1.4.3 — float-range long-double promotion (idl-4.2.md L3776)

`parse_floating` float range for long-double promotion depends on the
BLOCKED tracker. Float and double range are spec-conformant.

### §7.4.1.4.4.2.3 — long-double IEEE bit format (idl-4.2.md L3993)

`ConstValue::LongDouble([u8; 16])` as a stub. `f32`/`f64` (float/double)
are IEEE-754-conformant.
