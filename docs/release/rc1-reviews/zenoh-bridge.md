# RC1 Review — `zerodds-zenoh-bridge`

> **Layer:** 5 (Bridges, Tier-C) | **Reviewer:** claude | **Public-Strategy:** 🔒 public-feature-gated

## 1 Purpose

Bidirektionale Bridge zwischen ZeroDDS-DCPS und Eclipse-Zenoh: Topic-Mapping + QoS-Translation (pure-Rust-default) + optionaler Live-Runtime via Feature `zenoh-runtime`.

## 2 Public-Strategy

- **Marker:** 🔒 public-feature-gated
- **Begruendung:** Default-Build ist pure-Rust no_std-`alloc` Mapping-Layer; Live-Runtime mit `zenoh = 1` ist opt-in (kann hoehere MSRV als ZeroDDS-Default 1.88 erzwingen). Per Tracker `5.9 zenoh-bridge: rustc 1.86+ Feature` — die Feature-Gate-Klassifikation hier dient ausschliesslich der MSRV-Kompatibilitaet, nicht einem Embargo.

## 3 Inhalt

- 3 src-Files (lib, mapping, runtime).
- 0 tests-Files (Tests inline in mapping.rs).
- **6 Tests gruen** (5 unit + 1 doc).

## 3.4 Coherence-Audit

| Item-Familie | External Production-Refs | Klassifikation |
|---|---|---|
| `TopicMap` / `key_expr_for_topic` / `dds_qos_to_zenoh` (pure-Rust-Layer) | (Caller-Layer, Edge-Gateway-Daemon vorgesehen) | OPTIONAL-HOOK |
| `ZenohBridge` / `ZenohBridgeBuilder` / `BridgeError` (Feature `zenoh-runtime`) | (Caller-Layer) | OPTIONAL-HOOK |

Beide als OPTIONAL-HOOK explizit dokumentiert (Substrat-Crate fuer Caller-konstruierte Edge-Gateways). 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-dcps = { path = "../dcps" }
zerodds-qos = { path = "../qos" }

# Feature `zenoh-runtime` only:
zenoh = { version = "1", optional = true }
tokio = { version = "1", optional = true, ... }
thiserror = { version = "2", optional = true }
```

Beide Lower-Layer-Deps (`zerodds-dcps`, `zerodds-qos`) sind ✅ rc1-ready.

### 4.2 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Standard-Library. |
| `zenoh-runtime` | ❌ | Live-Bridge mit `zenoh` + `tokio`. |

## 5 Spec-Relevanz

Keine OMG-Spec; folgt dem De-facto-Pattern von ZettaScale's `zenoh-bridge-dds` als Library statt Plugin.

## 6 Cleanup-Findings

- **Forbidden-Token-Sweep:** 0 Treffer.
- **Sprint-/Phase-Marker:** 0 Treffer (Crate war pre-Review pristine).
- **TODO/FIXME/Stub/unimplemented!:** 0 Treffer.
- **Stub-/Noop-Audit:** 0 unused-`_arg`-Parameters, keine leeren Funktionsbodies.

## 7 Cleanup-Actions

1. Cargo.toml: `publish=false → publish=true` + Metadata komplett (homepage, documentation, readme, keywords, categories); description erweitert.
2. lib.rs: Crate-Header um SPDX + RC1-Form ergaenzt (vorhandene Architektur-Doku + QoS-Tabelle bleiben, da bereits sehr klar).
3. SPDX-License-Header auf alle 3 src-Files.
4. README.md neu (war nicht vorhanden).
5. CHANGELOG.md neu (war nicht vorhanden).
6. Public-Mirror unter `github/crates/zenoh-bridge/`.
7. `website/docs/zenoh-bridge.md`.
8. Tracker: 5.9 zenoh-bridge → ✅.

## 10-12 Gates

- `cargo test`: ✅ 6 tests (5 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅ 0 warnings.

**Crate-Version:** `1.0.0-rc.1` | **Sign-off:** claude
