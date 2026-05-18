# RC1 Review — `zerodds-amqp-bridge`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 5 (Bridges)
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OASIS AMQP 1.0 Wire-Codec — Type-System + Frame-Format + Performatives + Message-Sections + DDS-AMQP-1.0 Codec-Lite-Profile-Marker. `no_std + alloc`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begruendung:** Substrat-Crate fuer den DDS-AMQP-Endpoint-Layer; `no_std`-faehiger AMQP-1.0-Codec ist auf crates.io schwach besetzt.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs              # Crate-Header + Public-Re-Exports
├── codec_profile.rs    # DDS-AMQP-1.0 §2.4 Codec-Lite-Marker
├── extended_types.rs   # Erweiterte Primitive + Compound (list/map/array)
├── frame.rs            # Frame-Format §2.3
├── performatives.rs    # Die 9 Performatives §2.7
├── sections.rs         # Die 9 Message-Sections (Messaging §3)
└── types.rs            # Type-System (stable Subset)

tests/
├── boundary_decoders.rs    # Mutation-Survival-Boundary-Tests
├── fuzz_smoke.rs           # Pseudo-Random-Bytes-Smoke
└── proptest_roundtrip.rs   # Roundtrip-Property-Tests

benches/
└── decode_hotpaths.rs      # Criterion-Regression-Benches

fuzz/                       # cargo-fuzz Targets (eigene Cargo.toml, opt-in)
```

### 3.2 Public-API-Surface

```rust
pub mod codec_profile;
pub mod extended_types;
pub mod frame;
pub mod performatives;
pub mod sections;
pub mod types;

// Re-Exports aus lib.rs (siehe lib.rs fuer vollstaendige Liste)
pub use extended_types::{AmqpExtValue, encode_*, decode_* (Integer-Tail/Float/Char/Timestamp/UUID/Decimal)};
pub use frame::{FrameError, FrameHeader, FrameType, decode_frame_header, encode_frame_header};
pub use performatives::{attach, begin, close, decode_performative, detach, disposition, encode_performative, end, flow, open, transfer};
pub use sections::{MessageSection, validate_section_sequence};
pub use types::{AmqpValue, FormatCode, TypeError, decode_value, encode_binary, encode_boolean, encode_long, encode_null, encode_string, encode_symbol, encode_ulong};
```

### 3.3 Tests

- `cargo test -p zerodds-amqp-bridge` lokal: ✅ **188 tests + 1 doc-test** (82 unit + 90 boundary + 8 proptest + 8 fuzz-smoke + 1 doc).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `AmqpValue` / `decode_value` / `encode_*` (Primitive) | OASIS AMQP 1.0 §1.6 + §3 | `amqp-endpoint` | viele lokal | CONNECTED | — |
| `AmqpExtValue` + extended `encode_*` / `decode_*` | OASIS AMQP 1.0 §1.6 Tail + §1.6.22-§1.6.24 | `amqp-endpoint` | 90 boundary + 82 unit | CONNECTED | — |
| `FrameHeader` / `FrameType` / `encode_frame_header` / `decode_frame_header` | OASIS AMQP 1.0 §2.3 | `amqp-endpoint` | mehrere lokal | CONNECTED | — |
| `open` / `begin` / ... / `close` + `encode_performative` / `decode_performative` | OASIS AMQP 1.0 §2.7 | `amqp-endpoint` | mehrere lokal | CONNECTED | — |
| `MessageSection` / `validate_section_sequence` | OASIS AMQP 1.0 Messaging §3 | `amqp-endpoint` | mehrere lokal | CONNECTED | — |
| `codec_profile::{CodecProfile, active_profile, is_codec_lite_*}` | DDS-AMQP-1.0 §2.4 | (Conformance-Marker; per Design ohne Wire-Konsumenten) | 5 lokal | OPTIONAL-HOOK | dokumentiert: §2.4-Conformance-Claim, kein Wire-Path-Effekt |

**Akzeptanz:** 5/6 CONNECTED, 1 OPTIONAL-HOOK (§2.4 Conformance-Marker — explizit per Design ein Caller-side-Pruefer ohne Code-Pfad-Effekt). 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
# none — pure no_std + alloc
```

### 4.2 Dependents (used-by)

```bash
$ rg -l 'zerodds-amqp-bridge' --type-add 'cargo:*Cargo.toml' -t cargo crates/ | grep -v '^crates/amqp-bridge/'
crates/amqp-endpoint/Cargo.toml
```

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / Compound-Types. |
| `codec-lite` | ❌ | DDS-AMQP-1.0 §2.4 Codec-Lite-Profile-Marker (Conformance-Claim). |

## 5 Spec-Relevanz

- **Spec(s):** OASIS AMQP 1.0 (Types / Transport / Messaging) + OMG DDS-AMQP 1.0 (formal/2024-08-01).
- **Coverage-Doc(s):** keine eigene Coverage-Doc; Spec ist self-contained, Conformance-Vektoren in Unit + Boundary + Proptest-Tests.
- **Abgedeckte §-Sektionen:** AMQP-Types §1.6 + §1.7 + §3, AMQP-Transport §2.3 + §2.7, AMQP-Messaging §3, DDS-AMQP §2.3 + §2.4 + §6.1 + §7 + §8.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -g '!target/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' ... crates/amqp-bridge
```

Treffer: **0**.

### 6.1b Sprint-/Phase-/Cluster-Marker

**Vor Cleanup:** 6 Treffer in `extended_types.rs`, `performatives.rs`, `sections.rs`, `types.rs`, `tests/boundary_decoders.rs` — alle „Phase-C-Stufe-A/B/C/D (Spec-Cycle 5)" oder „TS-1-Finding 7"-Sprache. Auch 4 weitere `Stufe A/B`-Section-Header.

**Cleanup-Action:** alle ersetzt durch fachliche Beschreibung (Integer-Tail / Compound / Floating + Char + ...). Keine Sprint-Marker mehr in src/ oder tests/.

### 6.2 Soft-Review-Treffer

`rg -i -e 'TODO|FIXME|XXX|HACK' crates/amqp-bridge`: **0** Treffer.

### 6.2b Spec-Conformance-Sweep (Guardrails §1.13)

0 Inline-Deferral-Marker, 0 Layering-Violation-Hinweise, 0 Intra-Vendor-Kompromisse.

### 6.3 Tech-Debt

`lib.rs`-Header behauptete vor dem Review faelschlich, Performatives + Message-Format + SASL waeren nicht abgedeckt. Performatives und Message-Sections SIND seit Spec-Cycle-5 voll implementiert; SASL und TLS bleiben Caller-Layer (das stimmt). Header korrigiert.

### 6.4 Public-API-Leaks

Keine. `#![warn(missing_docs)]` aktiv.

## 7 Cleanup-Actions

1. `Cargo.toml` — `publish=false → publish=true`, Metadata komplett (homepage / documentation / readme / keywords / categories), description erweitert, codec-lite-Feature-Doku entstaubt.
2. `lib.rs` — Crate-Header neu (Spec-Cache-Path → OASIS + DDS-AMQP-Spec-Refs, „Was nicht abgedeckt"-Sektion entfernt; volle Public-API-Liste; Quickstart-Doc-Test).
3. License-Header (SPDX-Apache-2.0) auf alle 7 src-Files.
4. Sprint-Marker entfernt: `extended_types.rs` (3 Stellen), `performatives.rs`, `sections.rs`, `types.rs` (2 Stellen), `tests/boundary_decoders.rs`.
5. `dds-amqp-1.0-beta1.pdf` Cross-Refs auf `DDS-AMQP-1.0` aktualisiert.
6. `README.md` aus crate-readme-Stub auf RC1-Format gehoben.
7. `CHANGELOG.md` mit `[1.0.0-rc.1]`-Initial-Materialisierung.
8. Public-Mirror unter `github/crates/amqp-bridge/` (ohne `fuzz/` und `mutants.out`).
9. `website/docs/amqp-bridge.md`.
10. Tracker: 5.1 amqp-bridge → ✅.

## 8 Spec-Doc-Updates

Keine separate Spec-Coverage-Doc — OASIS AMQP 1.0 ist self-contained.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollstaendig.
- [x] `lib.rs`-Crate-Header.
- [x] `README.md`.
- [x] `CHANGELOG.md`.
- [x] Doc-tested Code-Example in lib.rs (`encode_long` ↔ `decode_value`).

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-amqp-bridge       # ✅ 188 + 1 doc
cargo clippy -p zerodds-amqp-bridge --tests -- -D warnings   # ✅
cargo fmt -p zerodds-amqp-bridge -- --check                  # ✅
cargo doc -p zerodds-amqp-bridge --no-deps                   # ✅ 0 warnings (nach Fix der 11 redundant-link-Warnings)
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit
- [x] §1.6 Spec-Coverage-Update (n/a)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build gruen
- [x] §1.10 Review-Doc
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror
- [x] §1.13 Spec-Conformance-Audit (Sprint-Marker bereinigt)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** claude
- **Tracker-Eintrag aktualisiert:** ✅
