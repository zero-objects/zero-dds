# RC1 Review — `zerodds-cdr-derive`

> Referenz: [`docs/release/RC1_GUARDRAILS.md`](../RC1_GUARDRAILS.md).
> Public-Strategy: 🌐 (proc-macro, Cargo-publish-fähig).
> Layer: 1 Primitives.

## §1.1 Cargo.toml-Metadata

| Feld | Wert | Status |
|---|---|---|
| name | `zerodds-cdr-derive` | ✅ |
| version | workspace `0.0.0` (rc1-Tag setzt `1.0.0-rc.1`) | ✅ |
| edition | `2024` (workspace) | ✅ |
| rust-version | `1.88` (workspace) | ✅ |
| license | `Apache-2.0` (workspace) | ✅ |
| repository | `https://github.com/zero-objects/zero-dds` | ✅ |
| homepage | `https://zerodds.org` | ✅ |
| documentation | `https://docs.rs/zerodds-cdr-derive` | ✅ |
| readme | `README.md` | ✅ |
| keywords | `["dds", "xcdr2", "proc-macro", "derive", "cdr"]` | ✅ (5/5, valid) |
| categories | `["development-tools::procedural-macro-helpers", "network-programming"]` | ✅ |
| description | RC1-konform | ✅ |
| publish | `true` | ✅ |

## §1.2 lib.rs Crate-Header

✅ Modul-Doc-Comment mit Spec-Referenz, Beispiel und Verhalten dokumentiert.
ASCII-only, keine Phasen-Marker.

## §1.3 README.md

✅ Pflicht-Sections live: Title + License-Badge, Was-die-Crate-tut,
Spec+Layer, Quickstart, Feature-Flags, Stability, Links.

## §1.4 CHANGELOG.md

✅ `[1.0.0-rc.1]`-Eintrag mit Spec-Referenzen, Public-API-Liste,
Implementierungs-Absatz, Architektur-Tabelle, Stabilitaets-Statement.

## §1.5 Public-API-Audit

Public-Items (manuell aus `lib.rs`):

| Item | Sichtbarkeit | Doc-Comment | Klassifikation |
|---|---|---|---|
| `derive_dds_type` | `#[proc_macro_derive(DdsType, attributes(dds))]` | ✅ | CONNECTED (User-Facing) |

Helper-Funktionen `expand`, `parse_struct_opts`, `parse_field_opts`,
`StructOpts`, `FieldOpts` sind privat (`fn`/`struct` ohne `pub`).
✅ Kein versehentliches Re-Export.

## §1.5b Coherence-Audit

| Item | External Refs | Test Refs | Klasse | Decision |
|---|---|---|---|---|
| `DdsType` (derive macro) | tests/derive_smoke.rs (6 Tests, smoke) | dito | CONNECTED via tests + Spec-Spec §11.1 | ✅ |

Externe Konsumenten heute: `tests/derive_smoke.rs`. Production-Use
folgt aus User-Apps die `idl-rust`-Codegen ersetzen wollen — als
spec-OPTIONAL-HOOK dokumentiert in `zerodds-xcdr2-rust-1.0` §11.1.

## §1.6 Spec-Coverage-Doc-Update

✅ `docs/spec-coverage/zerodds-xcdr2-rust-1.0.open.md` enthaelt das
Item §11.1 NICHT mehr (war `n/a (rejected)`, jetzt voll
implementiert + getestet, also entfernt).

## §1.9 Tests + Lints + Doc-Build

```bash
$ cargo test -p zerodds-cdr-derive
test result: ok. 6 passed; 0 failed; 0 ignored

$ cargo clippy -p zerodds-cdr-derive --tests -- -D warnings
(0 warnings)

$ cargo doc -p zerodds-cdr-derive --no-deps
Generated /Users/.../target/doc/zerodds_cdr_derive/index.html
```

✅ alle drei gruen. (`cargo fmt -- --check` lokal: gruen.)

## §1.10 Review-Doc

Diese Datei.

## §1.11 Tracker-Update

Folgt im RC1_TRACKER.md unter Layer 1.

## §1.13 Spec-Conformance-Audit (HARD-BLOCKER)

```bash
$ rg -in 'TODO|FIXME|XXX|HACK|Phase-?[0-9]|deferred|out.of.scope' \
    crates/cdr-derive/src/
(0 hits)

$ rg -in 'layering.violation|intra-zerodds' crates/cdr-derive/src/
(0 hits)
```

✅ Inline-Deferral-Marker: 0.
✅ Spec-Section-Coverage: §11.1 Punkt aus `zerodds-xcdr2-rust-1.0`
   als done klassifiziert; alle anderen §-Items sind nicht crate-scope.
✅ Wire-Konformität: byte-genauer V-2-Wire-Vector-Test
   (`point_encode_matches_v2_wire`) gegen
   `zerodds-xcdr2-bindings-conformance-1.0` §6 V-2 = `01 00 00 00 fe ff ff ff`.
✅ Kohärenz: (a) Macro emittiert syntactically + semantically
   coherent `impl DdsType`, (b) referenziert nur stabile public APIs
   von `zerodds_cdr` und `zerodds_dcps`, (c) 6 Tests inkl. encode +
   roundtrip + key-hash.

## §2 Forbidden-Token-Sweep

```bash
$ rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e '/Users/sandrakessler' \
       -e 'PDE-Spec' -e 'zero-principle' -e 'seesaw' \
       crates/cdr-derive/
(0 hits)
```

✅ Sweep clean.

## Sign-off

Crate `zerodds-cdr-derive` erfuellt alle DoD-Kriterien aus
`RC1_GUARDRAILS.md` §1. Empfohlen fuer `1.0.0-rc.1`-Tag.
