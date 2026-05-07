# `zerodds-amqp-endpoint`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-amqp-endpoint/badge.svg)](https://docs.rs/zerodds-amqp-endpoint)

OMG DDS-AMQP-1.0 (formal/2024-08-01) bidirektionaler Endpoint-Stack:
SASL-Frame-Layer + Connection/Session-State-Machine + Sender-/Receiver-
Link-Acceptance + Address-Routing + Body-Encoding-Mapping +
Application-Properties + DDS-Bridge-Trait-Surfaces (incl.
Disposition-Mapper-Wire-up §7.7.3) + Resource-Limits + Catalog +
Audit-Producers + Metrics + Access-Control + Coexistence + RPC-
Correlation + Annex-A-Configuration-Schema. Sitzt auf
[`zerodds-amqp-bridge`](../amqp-bridge) (Wire-Codec). `no_std + alloc`
mit optionalem `std`-Loader (XML-Config), `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-AMQP 1.0 (formal/2024-08-01) | §2.1 (Endpoint-Profile), §6.1 (Direct-Embed-Topology), §7 (Mapping inkl. §7.3 Address-Resolution, §7.4 Settlement-Mode-Mapping, §7.6.1 group-id, §7.7.2 Inbound-Operation-Signals, §7.7.3 Disposition-Mapping), §8.1 (Body-Encoding-Modes), §9.1-§9.2 (Annex-A IDL + XML-Config-Loader), §10.2 (SASL), §11 (Errors), Annex A (Configuration-Schema) |
| OASIS AMQP 1.0 | §2.4 (Connection-State), §2.5 (Session-State), §2.6 (Link-Lifecycle), §3.4 (Disposition-States), §3.5.3 (Terminus-Durability) |

## Was ist drin

- **`sasl`** — SASL-Frame-Layer (PLAIN / ANONYMOUS / EXTERNAL) §10.2.
- **`session`** — Connection/Session-State-Machine + Idle-Timeout +
  DoS-Caps §6.1.
- **`link`** — Sender-/Receiver-Link-Acceptance + Settlement-Tracking
  + `LinkSession::settle_with_mapper` (Wire-up fuer
  `DispositionMapper`-§7.7.3) + Terminus-Durability-Pre-Attach-Pruefung
  (§7.4.2 inkl. `unsettled-state`-Reject mit
  `amqp:not-implemented`).
- **`routing`** — Address-Resolution + Wildcard-Mapping §7.3.
- **`mapping`** — Body-Encoding-Mode-Mapping (Pass-Through / JSON /
  AMQP-Native) §8.1.
- **`properties`** — Application-Properties-Codec mit
  `dds:operation` / `dds:instance-handle` / `dds:type-id`-Inspection.
- **`dds_bridge`** — Trait-Surfaces:
  - `DdsOperationDispatcher` + `AcceptAllDispatcher` +
    `InstanceTrackingDispatcher` (§7.7.2 + §11.3).
  - `DispositionMapper` + `NoopDispositionMapper` (§7.7.3, gewired in
    `link::LinkSession::settle_with_mapper`).
- **`management`** — Catalog + Audit-Producers + Metrics-Snapshots.
- **`metrics`** — Mandatory-Metric-Hub.
- **`security`** — Access-Control-Plugin-Surface + Governance-
  Documents + Identity-Tokens + StaticAllowList.
- **`coexistence`** — Multi-Bridge-Hop-Cap + Inbound-Decision (DEFAULT
  hop-cap = 8, MAX = 64).
- **`rpc_correlation`** — Outstanding-Calls + Reply-Routing fuer DDS-
  RPC-Workflows.
- **`errors`** — Spec-§11 Error-Conditions als typisierte
  `AmqpError`-Werte.
- **`limits`** — `ResourceLimits`-Datenmodell (max-connections,
  max-frame-size, idle-timeout) aus Annex A.
- **`keyhash`** — SHA-256 group-id-Hashing fuer §7.6.1.
- **`annex_a`** — IDL-Spiegelung des Annex-A-Configuration-Schemas.
- **`config_xml`** (Feature `std`) — XML-Configuration-Loader §9.2.

## Schichten-Position

Layer 5 — Bridges. Substrat fuer den Daemon-Crate
`tools/amqp-dds-endpoint/` (TCP-Listener + TLS-Termination + DDS-side
DCPS-Bruecke).

## Quickstart

```rust
use zerodds_amqp_endpoint::{
    DispositionState, LinkRole, LinkSession, NoopDispositionMapper, SettlementMode,
};

let mut link = LinkSession::new(
    "outbound-1".into(),
    0,
    LinkRole::Sender,
    SettlementMode::Unsettled,
);
link.grant_credit(2);
link.deliver().expect("ok");

// AMQP-only-Workflow ohne DDS-Bridge: NoopMapper aufrufen, der
// pending-Counter dekrementiert ohne DDS-side State-Update.
let mapper = NoopDispositionMapper;
link.settle_with_mapper(&mapper, [0u8; 16], DispositionState::Accepted);
assert_eq!(link.pending_settlements, 0);
```

DDS-Bridge-Workflow: Caller implementiert `DispositionMapper` selbst
(typisch ein DCPS-Adapter) und uebergibt es `settle_with_mapper(...)`.

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | XML-Config-Loader (`config_xml`-Modul, via `roxmltree`) + `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]` —
ohne XML-Loader, alle anderen Module verfuegbar.

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Spec-mandatierte Wire-Erwartungen (DDS-AMQP-1.0 + AMQP-1.0) und
Fehler-Diskriminanten sind RC1-stabil; Breaking-Changes erfordern
Major-Bump.

## Tests

```bash
cargo test -p zerodds-amqp-endpoint
```

237 Tests grün:
- 205 Unit-Tests (alle 19 src-Module).
- 17 `annex_a_idl_roundtrip` (Spec-IDL-Parser-Verifikation).
- 6 `e2e_multi_bridge_hop` (Coexistence-Hop-Cap-Workflow).
- 4 `fuzz_smoke` (Wire-Decode-Robustheit).
- 6 `proptest_state_machine` (State-Machine-Invarianten).
- 1 Doc-Test.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/amqp-endpoint.md`](../../docs/release/rc1-reviews/amqp-endpoint.md) — RC1-Review (inkl. F-AMQP-EP-DISPOSITION-MAPPER-WIRED Finding).
- [`zerodds-amqp-bridge`](../amqp-bridge) — AMQP-1.0-Wire-Codec.
- [`zerodds-idl`](../idl) — IDL-Parser fuer Annex-A-Roundtrip-Test.
