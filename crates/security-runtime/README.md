# `zerodds-security-runtime`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-runtime/badge.svg)](https://docs.rs/zerodds-security-runtime)

Security runtime for the [ZeroDDS](https://zerodds.org) stack:
governance-driven plugin lifecycle, peer-capabilities cache,
built-in data tagging, anti-squatter, heterogeneous-mesh gateway bridge.
Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §8.5.3 (anti-squatter), §9.5 (inbound/outbound) |
| OMG DDS-Security 1.2 | §8.7 (data tagging) |
| ZeroDDS architecture §09 | heterogeneous mesh + delegation |

## What's inside

- `SecurityGate` — high-level adapter governance ↔ crypto ↔ RTPS wrap.
- `engine::GovernancePolicyEngine` — default PolicyEngine.
- `caps::*` + `caps_wire::*` — peer capabilities + SPDP wire codec incl. delegation chain.
- `peer_class::*` — `<peer_class>` match.
- `data_tagging::*` — `DataTaggingPlugin` default impl.
- `builtin_topics::*`, `anti_squatter::*`, `gateway_bridge::*`.

## Layer position

Layer 4. Consumes all 7 security sibling crates.

## Stability

`1.0.0-rc.1`.

## Tests

```bash
cargo test -p zerodds-security-runtime
```

214+ tests green.

## License

Apache-2.0.
