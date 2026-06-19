# `zerodds-amqp-endpoint`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-amqp-endpoint/badge.svg)](https://docs.rs/zerodds-amqp-endpoint)

OMG DDS-AMQP-1.0 (formal/2024-08-01) bidirectional endpoint stack:
SASL frame layer + connection/session state machine + sender/receiver
link acceptance + address routing + body-encoding mapping +
application properties + DDS-bridge trait surfaces (incl.
disposition-mapper wire-up §7.7.3) + resource limits + catalog +
audit producers + metrics + access control + coexistence + RPC
correlation + Annex-A configuration schema. Sits on top of
[`zerodds-amqp-bridge`](../amqp-bridge) (wire codec). `no_std + alloc`
with an optional `std` loader (XML config), `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-AMQP 1.0 (formal/2024-08-01) | §2.1 (endpoint profile), §6.1 (direct-embed topology), §7 (mapping incl. §7.3 address resolution, §7.4 settlement-mode mapping, §7.6.1 group-id, §7.7.2 inbound operation signals, §7.7.3 disposition mapping), §8.1 (body-encoding modes), §9.1-§9.2 (Annex-A IDL + XML config loader), §10.2 (SASL), §11 (errors), Annex A (configuration schema) |
| OASIS AMQP 1.0 | §2.4 (connection state), §2.5 (session state), §2.6 (link lifecycle), §3.4 (disposition states), §3.5.3 (terminus durability) |

## What's inside

- **`sasl`** — SASL frame layer (PLAIN / ANONYMOUS / EXTERNAL) §10.2.
- **`session`** — connection/session state machine + idle timeout +
  DoS caps §6.1.
- **`link`** — sender/receiver link acceptance + settlement tracking
  + `LinkSession::settle_with_mapper` (wire-up for
  `DispositionMapper` §7.7.3) + terminus-durability pre-attach check
  (§7.4.2 incl. `unsettled-state` reject with
  `amqp:not-implemented`).
- **`routing`** — address resolution + wildcard mapping §7.3.
- **`mapping`** — body-encoding-mode mapping (pass-through / JSON /
  AMQP-native) §8.1.
- **`properties`** — application-properties codec with
  `dds:operation` / `dds:instance-handle` / `dds:type-id` inspection.
- **`dds_bridge`** — trait surfaces:
  - `DdsOperationDispatcher` + `AcceptAllDispatcher` +
    `InstanceTrackingDispatcher` (§7.7.2 + §11.3).
  - `DispositionMapper` + `NoopDispositionMapper` (§7.7.3, wired in
    `link::LinkSession::settle_with_mapper`).
- **`management`** — catalog + audit producers + metrics snapshots.
- **`metrics`** — mandatory metric hub.
- **`security`** — access-control plugin surface + governance
  documents + identity tokens + StaticAllowList.
- **`coexistence`** — multi-bridge hop cap + inbound decision (DEFAULT
  hop-cap = 8, MAX = 64).
- **`rpc_correlation`** — outstanding calls + reply routing for
  DDS-RPC workflows.
- **`errors`** — spec §11 error conditions as typed
  `AmqpError` values.
- **`limits`** — `ResourceLimits` data model (max-connections,
  max-frame-size, idle-timeout) from Annex A.
- **`keyhash`** — SHA-256 group-id hashing for §7.6.1.
- **`annex_a`** — IDL mirror of the Annex-A configuration schema.
- **`config_xml`** (feature `std`) — XML configuration loader §9.2.

## Layer position

Layer 5 — Bridges. Substrate for the daemon crate
`tools/amqp-dds-endpoint/` (TCP listener + TLS termination + DDS-side
DCPS bridge).

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

// AMQP-only workflow without a DDS bridge: call the NoopMapper, which
// decrements the pending counter without a DDS-side state update.
let mapper = NoopDispositionMapper;
link.settle_with_mapper(&mapper, [0u8; 16], DispositionState::Accepted);
assert_eq!(link.pending_settlements, 0);
```

DDS-bridge workflow: the caller implements `DispositionMapper` itself
(typically a DCPS adapter) and passes it to `settle_with_mapper(...)`.

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | XML config loader (`config_xml` module, via `roxmltree`) + `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-capable: `default-features = false, features = ["alloc"]` —
without the XML loader, all other modules available.

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
spec-mandated wire expectations (DDS-AMQP-1.0 + AMQP-1.0) and error
discriminants are RC1-stable; breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-amqp-endpoint
```

237 tests green:
- 205 unit tests (all 19 src modules).
- 17 `annex_a_idl_roundtrip` (spec IDL-parser verification).
- 6 `e2e_multi_bridge_hop` (coexistence hop-cap workflow).
- 4 `fuzz_smoke` (wire-decode robustness).
- 6 `proptest_state_machine` (state-machine invariants).
- 1 doc test.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/amqp-endpoint.md`](../../docs/release/rc1-reviews/amqp-endpoint.md) — RC1 review (incl. the F-AMQP-EP-DISPOSITION-MAPPER-WIRED finding).
- [`zerodds-amqp-bridge`](../amqp-bridge) — AMQP-1.0 wire codec.
- [`zerodds-idl`](../idl) — IDL parser for the Annex-A roundtrip test.
