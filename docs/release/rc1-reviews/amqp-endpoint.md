# RC1 Review — `zerodds-amqp-endpoint`

> **Layer:** 5 (Bridges, Tier-B) | **Reviewer:** claude | **Public-Strategy:** 🌐 public

## 1 Purpose

OMG DDS-AMQP-1.0 bidirektionaler Endpoint-Stack: SASL + Session/Link-Lifecycle + Routing + Mapping + Properties + DDS-Bridge-Trait-Surfaces (incl. Disposition-Mapper-Wire-up §7.7.3) + Annex-A-Configuration.

## 2-3 Inhalt

- 19 src-Files (annex_a, codegen_helpers, coexistence, config_xml, dds_bridge, errors, keyhash, lib, limits, link, management, mapping, metrics, properties, routing, rpc_correlation, sasl, security, session).
- 4 tests-Files (annex_a_idl_roundtrip, e2e_multi_bridge_hop, fuzz_smoke, proptest_state_machine).
- **237 Tests gruen** (205 unit + 17 annex_a + 6 e2e + 4 fuzz + 6 proptest + 1 doc, davon **+2 neu** durch DispositionMapper-Wire-up).

## 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Item-Familie | Spec | External Production-Refs | Klassifikation |
|---|---|---|---|
| `LinkSession` / `settle` / `settle_with_mapper` | DDS-AMQP §7.4 + §7.7.3 | tools/amqp-dds-endpoint (tests) | CONNECTED |
| `DispositionMapper` / `NoopDispositionMapper` | DDS-AMQP §7.7.3 | self (`link::settle_with_mapper`) — nach Wire-up | CONNECTED ✅ |
| `DdsOperationDispatcher` / `Accept*Dispatcher` / `InstanceTracking*` | DDS-AMQP §7.7.2 + §11.3 | (Caller-Plugin, vorgesehen DCPS-Bruecke) | OPTIONAL-HOOK (dokumentiert) |
| Connection/Session-State-Machine (`advance_connection`) | OASIS AMQP §2.4-2.5 | self + Daemon | CONNECTED |
| `AddressRouter` / Routing | DDS-AMQP §7.3 | self + Daemon | CONNECTED |
| `BodyEncodingMode` / Mapping | DDS-AMQP §8.1 | self + Daemon | CONNECTED |
| `DdsOperation` / Properties / `produce_*` | DDS-AMQP §7.7 | self + Daemon | CONNECTED |
| `AmqpError` + Error-Helpers | DDS-AMQP §11 | self | CONNECTED |
| `ResourceLimits` / `keyhash` | DDS-AMQP §6.1 + §7.6.1 | self + Daemon | CONNECTED |
| `MetricsHub` / `management::*` | DDS-AMQP Mandatory-Metrics | self + Daemon | CONNECTED |
| `AccessControlPlugin` / Governance | DDS-AMQP §10 | (Caller-Plugin) | OPTIONAL-HOOK |
| `CoexistenceConfig` / Hop-Cap | DDS-AMQP §6.5 | self | CONNECTED |
| `OutstandingCalls` / RPC-Correlation | DDS-AMQP §7.8 | self | CONNECTED |
| `annex_a` / `config_xml` / `codegen_helpers` | DDS-AMQP §9 | self + Daemon | CONNECTED |

**Akzeptanz:** alle CONNECTED oder als OPTIONAL-HOOK explizit dokumentiert. **0 ❌-Klassen**.

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-amqp-bridge = { path = "../amqp-bridge", default-features = false, features = ["alloc"] }
sha2 = { workspace = true }
roxmltree = { version = "0.20", optional = true }   # Feature `std`
```

### 4.2 Dependents

`tools/amqp-dds-endpoint` (Daemon mit TCP-Listener + TLS-Termination + DCPS-Bruecke).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | XML-Config-Loader + `std::error::Error`-Impls. |
| `alloc` | ✅ | Compound-Datenstrukturen. |

## 5 Spec-Relevanz

OMG DDS-AMQP-1.0 (formal/2024-08-01) §2.1 + §6.1 + §7.3 + §7.4 + §7.6.1 + §7.7.2 + §7.7.3 + §8.1 + §9 + §10.2 + §11 + Annex A. OASIS AMQP-1.0 §2.4 + §2.5 + §2.6 + §3.4 + §3.5.3.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Treffer: **0**.

### 6.1b Sprint-/Phase-Marker

**Vor Cleanup:** 1 Treffer in `lib.rs:7` ("Phase-E des Spec-Cycle 5").
**Cleanup:** entfernt + Crate-Header neu nach RC1-Template.

### 6.2 Soft-Review (TODO/FIXME/HACK)

Treffer: **0**.

### 6.2b Stub/Noop-Audit

Per User-Aufforderung explizit gepruft. **Resolved Finding:**

#### F-AMQP-EP-DISPOSITION-MAPPER-WIRED ✅

**Pre-Review-Befund:** `DispositionMapper`-Trait + `NoopDispositionMapper`-Impl in `dds_bridge.rs:171-187` waren TEST-ONLY referenziert — der einzige Caller war `noop_disposition_mapper_does_nothing` im selben File. `apply()` wurde nirgends aus Production-Code aufgerufen. Workspace-weit 0 andere Implementations. Klassisches Stub-Signal mit `_`-Underscore-Args.

**Spec-Anker:** DDS-AMQP-1.0 §7.7.3 (Disposition-Mapping zu DDS-Sample-State).

**Decision:** `wire-up` (Option 1).

**Implementiert:**
- `LinkSession::settle_with_mapper<M: DispositionMapper>(&mut self, mapper: &M, sample_handle: [u8; 16], state: DispositionState)` als Spec-§7.7.3-konformer Wire-up-Pfad: ruft `mapper.apply(sample_handle, state)` UND dekrementiert den pending-Counter. Die alte `settle()` bleibt fuer AMQP-only-Workflows.
- 2 neue Tests: `settle_with_mapper_calls_apply_and_decrements_pending` (verifiziert mit RecordingMapper, dass `apply` mit korrekten Parametern in Reihenfolge aufgerufen wird) + `settle_with_mapper_underflow_safe_at_zero` (verifiziert dass Mapper auch bei pending=0 aufgerufen wird; counter underflowt nicht).
- Doc-Comments fuer `DispositionMapper` und `NoopDispositionMapper` aktualisiert mit Cross-Ref auf `link::LinkSession::settle_with_mapper`.

**Status:** ✅ resolved
**Beleg:** `crates/amqp-endpoint/src/link.rs::settle_with_mapper`; `dds_bridge.rs::DispositionMapper`-Doc; 205 unit-Tests grün (vorher 203). Klassifikation `DispositionMapper` jetzt CONNECTED (war TEST-ONLY).

### 6.3 Tech-Debt

- `lib.rs`-Header neu strukturiert (vorher: lange Spec-Cache-Path-Refs + "Was nicht abgedeckt"-Sektion).
- `dds-amqp-1.0-beta1.pdf`-Refs auf `DDS-AMQP-1.0` aktualisiert.
- `annex_a.rs`: doc-broken-link `[`config_xml`]` auf `crate::config_xml` aktualisiert.

### 6.4 Public-API-Leaks

Keine. `#![warn(missing_docs)]` aktiv.

## 7 Cleanup-Actions

1. **Wire-up:** `LinkSession::settle_with_mapper` neu hinzugefuegt; `DispositionMapper` jetzt CONNECTED (vorher TEST-ONLY).
2. Cargo.toml: publish=true + Metadata komplett.
3. lib.rs: vollstaendig RC1-Header neu, Sprint-Marker raus.
4. SPDX-License-Header auf alle 19 src-Files.
5. dds_bridge.rs / annex_a.rs / link.rs / lib.rs: Spec-Refs aktualisiert.
6. README + CHANGELOG (mit F-AMQP-EP-DISPOSITION-MAPPER-WIRED-Findings-Section).
7. Mirror unter `github/crates/amqp-endpoint/`.
8. `website/docs/amqp-endpoint.md`.
9. Tracker: 5.2 amqp-endpoint → ✅.

## 10-12 Gates

- `cargo test`: ✅ 237 tests (205 lib + 32 integration + 0 doc-test fail = +1 doc-test pass).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅ 0 warnings (`config_xml`-Doc-Link gefixt).

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `std`+Daemon-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon_runtime.rs` + `qos_translation.rs` Module.
- TLS-Client-Connector (rustls 0.23 ClientConnection) + SASL/Bearer +
  ACL via `zerodds-bridge-security` voll wired (Bridge-Spec
  §7.1/§7.2/§7.3).
- `backoff.rs` (Exponential-Backoff fuer Broker-Reconnect) +
  `cross_vendor.rs`-Modul.
- Tests gruen: 225 unit + 17 annex_a + 6 e2e + 4 fuzz + 6 proptest + 1 doc.

**Crate-Version:** `1.0.0-rc.1` | **Sign-off:** claude
