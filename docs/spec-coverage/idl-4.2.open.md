# OMG IDL 4.2 — Open + Partial Items (Aggregat)

Auto-generiert aus `idl-4.2.md` Stand `Spec-Check 4.0` (Post-K1-Vollendung).
Vor jedem Audit-Lauf neu generieren (PROCESS.md).

## Open-Items

### §7.4.1.4.3-r-longdouble-open — Long-Double Voll-Präzisions-Arithmetik

**Status:** **MISSING — BLOCKED** (Iron-Rule-Eskalations-Klausel).

**Spec:** §7.4.1.4.3 — Long-Double-Sub-Expression-Präzision +
IEEE-754 double-extended (≥80-bit Mantisse).

**Blocker:** Rust-stable hat keinen `f128`-Type
([rust-lang/rust#116909](https://github.com/rust-lang/rust/issues/116909)).
Realistisch frühestens 2027.

**Verworfene Alternativen:** nightly-Switch (CI-Stabilität),
3rd-party `f128`-Crate (FFI-Risiko), eigene 128-bit-Soft-Float
(~500 LOC; Maintainer-Quality > Eigen-Impl-Quality).

**Re-Audit-Trigger:** `f128` in Rust-stable verfügbar →
`ConstValue::LongDouble` von `[u8; 16]` zu `f128`,
`parse_floating`/`apply_binary` Long-Double-Arms spec-konform.

## Partial-Items (alle Long-Double-related, alle BLOCKED-verlinkt)

### §7.4.1.4.3 — Long-Double-Type-Tag (idl-4.2.md L3396)

`ConstValue::LongDouble` als 16-Byte-Stub akzeptiert; Arithmetik
degradiert auf f64. Verlinkt zu `§7.4.1.4.3-r-longdouble-open`.

### §7.4.1.4.3 — Floating-Point-Expression-Eval-Regel (idl-4.2.md L3514)

`promote_float`-Long-Double-Arm ist Stub. Double-Branch + Range-Check
für Double sind Spec-konform.

### §7.4.1.4.3 — Float-Range Long-Double-Promotion (idl-4.2.md L3776)

`parse_floating` Float-Range für Long-Double-Promotion abhängig vom
BLOCKED-Tracker. Float- und Double-Range sind Spec-konform.

### §7.4.1.4.4.2.3 — Long-Double IEEE-Bit-Format (idl-4.2.md L3993)

`ConstValue::LongDouble([u8; 16])` als Stub. `f32`/`f64`
(Float/Double) sind IEEE-754-konform.
