# RC1 Review — `zerodds-http2`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 5 (Bridges)
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public
>
> Track-Materialisierung via git: `git log docs/release/rc1-reviews/http2.md`.

---

## 1 Purpose

HTTP/2 (RFC 9113) Wire-Codec — no_std Frame-Layer + Stream-State-Machine + Flow-Control + Connection-Preface + SETTINGS-Codec.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begruendung:** Substrat-Crate fuer den HTTP/2-Stack (gRPC-Bridge), fuer no_std-Builds geeignet — diese Niche ist auf crates.io schwach besetzt.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs       # Crate-Header + Public-Re-Exports
├── error.rs     # ErrorCode + Http2Error (§7)
├── flow.rs      # FlowControl + Window-Update-Codec (§5.2 + §6.9)
├── frame.rs     # Frame-Layer + alle 10 Frame-Types (§4 + §6)
├── preface.rs   # Connection-Preface (§3.4)
├── settings.rs  # SETTINGS-Frame-Codec + Defaults (§6.5)
└── stream.rs    # Stream-State-Machine (§5.1)
```

### 3.2 Public-API-Surface

```rust
// Module (alle pub):
pub mod error;
pub mod flow;
pub mod frame;
pub mod preface;
pub mod settings;
pub mod stream;

// Re-Exports aus lib.rs:
pub use error::{ErrorCode, Http2Error};
pub use flow::FlowControl;
pub use frame::{Flags, Frame, FrameHeader, FrameType, decode_frame, encode_frame};
pub use preface::{CLIENT_PREFACE, check_preface};
pub use settings::{Setting, SettingId, Settings};
pub use stream::{StreamId, StreamState};
```

### 3.3 Tests

- `cargo test -p zerodds-http2` lokal: ✅ **45 passed + 1 doc-test passed**.
- Aufgliederung:
  - `error` 3 Tests (`from_u32_round_trip`, `unknown_code_maps_to_internal_error`, `error_display_does_not_panic`).
  - `flow` 10 Tests (consume/window-update/initial-size-change/round-trip-codec/overflow/zero-update/r-bit/wrong-payload-size).
  - `frame` 9 Tests (encode-decode-round-trip + buffer-bounds + flags + r-bit + 5 weitere).
  - `preface` 5 Tests.
  - `settings` 8 Tests.
  - `stream` 10 Tests (alle §5.1-Transitions + Reject-Pfade).
  - lib.rs Quickstart-Doc-Test (PING-Frame Encode↔Decode).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `Frame` / `FrameHeader` / `FrameType` / `Flags` / `encode_frame` / `decode_frame` | RFC 9113 §4 + §6 | `grpc-bridge` (HTTP/2-Connection-Path), `conformance` (Cross-Vendor) | 9 lokal | CONNECTED | — |
| `CLIENT_PREFACE` / `check_preface` | RFC 9113 §3.4 | `grpc-bridge` (Connection-Setup) | 5 lokal | CONNECTED | — |
| `Settings` / `Setting` / `SettingId` + `decode_settings` / `encode_settings` (Modul) | RFC 9113 §6.5 | `grpc-bridge` | 8 lokal | CONNECTED | — |
| `StreamId` / `StreamState` + `transition` / `is_*_initiated` (Modul) | RFC 9113 §5.1 | `grpc-bridge` | 10 lokal | CONNECTED | — |
| `FlowControl` + `encode/decode_window_update` (Modul) | RFC 9113 §5.2 + §6.9 | `grpc-bridge` | 10 lokal | CONNECTED | — |
| `ErrorCode` / `Http2Error` | RFC 9113 §7 | `grpc-bridge` (Error-Propagation) | 3 lokal | CONNECTED | — |

**Akzeptanz:** 6/6 Item-Familien CONNECTED. 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
# none — pure no_std + alloc (core + alloc only)
```

### 4.2 Dependents (used-by)

```bash
$ rg -l 'zerodds-http2' --type-add 'cargo:*Cargo.toml' -t cargo crates/ | grep -v '^crates/http2/'
crates/conformance/Cargo.toml
crates/grpc-bridge/Cargo.toml
```

Liste: `zerodds-grpc-bridge`, `zerodds-conformance`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | `std::error::Error`-Impls fuer alle Fehler-Typen. |
| `alloc` | ✅ (via std) | `Vec` fuer `decode_settings`/`encode_settings`-Output. Crate ist `no_std`-fahig. |

## 5 Spec-Relevanz

- **Spec(s):** RFC 9113 (HTTP/2). Vorgaenger RFC 7540 wurde durch 9113 abgeloest; §-Nummern sind weitgehend identisch (9113 entfernt einige ungenutzte Features, klaert mehrere Edge-Cases). Diese Crate folgt 9113.
- **Coverage-Doc(s):** keine eigene Coverage-Doc. RFC 9113 ist self-contained-Wire-Spec; Conformance-Vektoren leben in den Unit-Tests (Frame-Roundtrip, Preface-Octets, Window-Update-Codec).
- **Abgedeckte §-Sektionen:** §3.4 (Connection-Preface), §4 (Frame-Layer mit allen 10 Frame-Types), §5.1 (Stream-State-Machine), §5.2 (Flow-Control), §6.1-§6.10 (Frame-Definitions), §6.5 (SETTINGS), §6.9 (`WINDOW_UPDATE`), §7 (Error-Codes).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -g '!target/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' -e 'gitlab\.sandra-kessler' \
  -e 'fishermen21' -e '/Users/sandrakessler' -e 'admin@ifyna' \
  -e 'PDE-Spec' -e 'zero_concept' -e 'zero-principle' -e 'Zero-Principle' \
  -e 'Ghost-Inject' -e 'R-09[7-9]' -e 'R-10[0-4]' -e 'R-110' \
  -e '\bseesaw\b' -e 'IfynaNeu' -e 'paperless' \
  -e '\bglr1\b' -e '\bglr2\b' -e '/tmp/cyc\.xml' \
  crates/http2
```

Treffer: **0**.

### 6.1b Sprint-/Phase-/Datums-Marker

Treffer: **0** (Body); CHANGELOG-Datum erlaubt per Guardrails §2.1c.

### 6.2 Soft-Review-Treffer (TODO/FIXME/HACK)

Treffer: **0**.

### 6.2b Spec-Conformance-Sweep (Guardrails §1.13)

```bash
rg -in 'TODO|FIXME|XXX|HACK|Phase-?[0-9]|deferred|out.of.scope|scheduled.for' crates/http2/src/
rg -in 'layering.violation|layer.break|bewusst.designen' crates/http2/src/
rg -in 'intra-zerodds|cross.vendor.*nicht|interop.bleibt' crates/http2/src/
```

Treffer: **0**.

### 6.3 Tech-Debt + Dead Code

Keine. Public-Surface ist minimal (16 re-exportierte Items + 6 Module).

### 6.4 Public-API-Leaks

Keine `pub use crate::internal::*;`-Patterns. `#![warn(missing_docs)]` aktiv. Keine Sealed-Trait-Anforderungen.

## 7 Cleanup-Actions

1. `Cargo.toml` — `publish=false → publish=true`, `homepage` / `documentation` / `readme` / `keywords` / `categories` ergaenzt; description erweitert; Spec-Ref `RFC 7540 → RFC 9113`.
2. `lib.rs`-Header — Crate-Statement um Safety-Class, Layer-Position, Public-API-Aufzaehlung und Quickstart-Doc-Test ergaenzt; `RFC 7540 → RFC 9113` mit historischer Note.
3. License-Header (`SPDX-License-Identifier: Apache-2.0` + Copyright) auf allen 7 src-Files.
4. Spec-Refs `RFC 7540 → RFC 9113` in 6 src-Files (insgesamt 7 Treffer aktualisiert; lib.rs behaelt eine Erwaehnung in der Vorgaenger-Note).
5. `README.md` aus crate-readme-Stub auf RC1-Format gehoben.
6. `CHANGELOG.md` neu mit `[1.0.0-rc.1]`-Initial-Materialisierung (vollstaendige Public-API-Aufzaehlung pro Modul).
7. Public-Mirror in `github/crates/http2/` materialisiert.
8. `website/docs/http2.md` als Public-Doc-Page.
9. `docs/release/RC1_TRACKER.md` von `📋 todo` auf `✅ rc1-ready`.

## 8 Spec-Doc-Updates

Keine separate Spec-Coverage-Doc. RFC 9113 ist self-contained.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollstaendig.
- [x] `lib.rs`-Crate-Header mit Safety-Class (STANDARD) + Spec-Ref (RFC 9113) + Layer (5 Bridges) + API-Aufzaehlung.
- [x] `README.md` aus Template.
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry.
- [x] Doc-tested Code-Example in `lib.rs` (PING-Frame Encode↔Decode).

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-http2           # ✅ 45 + 1 doc-test
cargo clippy -p zerodds-http2 --tests -- -D warnings   # ✅
cargo fmt -p zerodds-http2 -- --check                  # ✅
cargo doc -p zerodds-http2 --no-deps                   # ✅ keine Warnungen
cargo run --bin zerodds-lint -- check                  # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md aus Template
- [x] §1.4 CHANGELOG.md mit RC1-Entry (initial-Materialisierung-Format)
- [x] §1.5 Public-API-Audit (`#![warn(missing_docs)]` aktiv)
- [x] §1.5b Coherence-Audit (Tabelle in §3.4 ausgefuellt; alle CONNECTED)
- [x] §1.6 Spec-Coverage-Update (n/a — RFC 9113 self-contained)
- [x] §1.7 Forbidden-Token-Sweep (0 Treffer)
- [x] §1.8 License-Header pro File (alle 7 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build gruen
- [x] §1.10 Review-Doc ausgefuellt (= dieses Dokument)
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/http2/` + `website/docs/http2.md`)
- [x] §1.13 Spec-Conformance-Audit (0 Inline-Deferral-Marker)
- [x] Findings-Tracker (keine Findings — Crate war pristine pre-Review)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** claude
- **Tracker-Eintrag aktualisiert:** ✅
