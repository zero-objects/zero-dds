# `zerodds-security-runtime`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-runtime/badge.svg)](https://docs.rs/zerodds-security-runtime)

Security-Runtime fuer den [ZeroDDS](https://zerodds.org)-Stack:
Governance-driven Plugin-Lifecycle, Peer-Capabilities-Cache,
Built-in DataTagging, Anti-Squatter, Heterogeneous-Mesh-Gateway-Bridge.
Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §8.5.3 (Anti-Squatter), §9.5 (Inbound/Outbound) |
| OMG DDS-Security 1.2 | §8.7 (DataTagging) |
| ZeroDDS-Architektur §09 | Heterogeneous-Mesh + Delegation |

## Was ist drin

- `SecurityGate` — High-Level-Adapter Governance ↔ Crypto ↔ RTPS-Wrap.
- `engine::GovernancePolicyEngine` — Default-PolicyEngine.
- `caps::*` + `caps_wire::*` — Peer-Capabilities + SPDP-Wire-Codec inkl. Delegation-Chain.
- `peer_class::*` — `<peer_class>`-Match.
- `data_tagging::*` — `DataTaggingPlugin`-Default-Impl.
- `builtin_topics::*`, `anti_squatter::*`, `gateway_bridge::*`.

## Schichten-Position

Layer 4. Konsumiert alle 7 Security-Schwester-Crates.

## Stabilitaet

`1.0.0-rc.1`.

## Tests

```bash
cargo test -p zerodds-security-runtime
```

214+ Tests grün.

## Lizenz

Apache-2.0.
