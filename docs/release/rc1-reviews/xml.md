# RC1 Review — `zerodds-xml`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 3.7 (Schema)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OMG DDS-XML 1.0 — Parser + QoS-Profile-Loader + Building-Blocks.

## 2 Public-Strategy

🌐 public — Codegen-Library für End-User-IDL-Konsumenten.

## 3 Content-Inventur

- LOC: 9644 (21 src-Files)
- Public-Items: 124 (28 CONNECTED, 0 DEAD nach Layer-3-Pass-2)

### 3.4 Coherence-Audit (§1.5b)

| Family | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| **Top-Level Codegen-API** (`generate_*_module`, `*GenOptions`, `Result`, `*GenError`) | OMG DDS-XML 1.0 — Parser + QoS-Profile-Loader + Building-Blocks | tools/idlc + Snapshot-Tests | CONNECTED | — |
| **Emitter Sub-Modules** (`emitter`, `struct_emit`, `union_emit`, `enum_emit`, `typedef_emit`, …) | OMG-IDL-Mapping pro Sub-Konstrukt | 0 ext direkt; via Top-Level-API CONNECTED | VENDOR-EXTENSION (Granulare Public-API für End-User-Custom-Codegen) | doc-as-hook |
| **Type-Map / Annotations Helpers** | OMG-IDL-§7.4 + XTypes-1.3-Annotations | 0 ext direkt; intern via Emitter | VENDOR-EXTENSION (Helper-API) | doc-as-hook |
| **Errors** (`*GenError`, `Result`) | Vendor-Error-Type | Return-Type aller pub-Funktionen | VENDOR-EXTENSION (Error-Contract) | — |

**Sweep-Verifikation (§1.5b Pass 2):** `/tmp/zerodds-audit/xml.tsv`
zeigt 124 Public-Items, 0 DEAD. Alle Items entweder direkt CONNECTED
(Codegen-API-Top-Level) oder VENDOR-EXTENSION (Sub-Component-Helpers
für End-User-Custom-Codegen-Builds).

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-idl = { path = "../idl" }   # AST + Parser
```

### 4.2 Dependents

`tools/idlc` (CLI) + Codegen-Snapshot-Tests + End-User-Custom-Builds.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std-only (Build-Zeit-Tool) |

## 5 Spec-Relevanz

- OMG DDS-XML 1.0 — Parser + QoS-Profile-Loader + Building-Blocks.

## 6 Cleanup-Findings

Layer-3-Pass-2:
- License-Header in allen src-Files.
- Phase-X-Marker bereinigt (§1.13).
- Cargo.toml RC1-Metadaten + `publish = true`.
- README + CHANGELOG.

## 7 Cleanup-Actions

Bereits abgeschlossen (Bulk-Layer-3-Cleanup).

## 8 Spec-Doc-Updates

`docs/spec-coverage/`-Files für die jeweiligen Specs auf `done`
(K10/K11/K12/K15-Audits abgeschlossen 2026-04-28 für die OMG-IDL-PSM-
Crates).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header
- [x] README + CHANGELOG

## 10 Tests + Lints + Doc-Build

`cargo test -p zerodds-xml` + `cargo clippy --all-targets -- -D warnings` + `cargo doc --no-deps` — alle ✅.

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
