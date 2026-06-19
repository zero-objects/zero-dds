# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-runtime` crate.

### Spec references

- **OMG DDS-Security 1.1** §8.5.3, §9.5.
- **OMG DDS-Security 1.2** §8.7 (data tagging).
- **ZeroDDS architecture §09** heterogeneous mesh + delegation.

### Public API

- `SecurityGate`.
- `engine::{GovernancePolicyEngine, PolicyEngine}`.
- `policy::{PolicyDecision, OutboundDecision, InboundVerdict}`.
- `caps::{PeerCapabilities, PeerCapabilitiesCache, ProtectionLevel, CapabilityWindow}`.
- `caps_wire::{encode_peer_capabilities, decode_peer_capabilities}`.
- `peer_class::{PeerClassMatch, CidrPattern}`.
- `endpoint::*`.
- `data_tagging::DataTaggingDefault`.
- `builtin_topics::*`.
- `anti_squatter::*`.
- `gateway_bridge::GatewayBridge`.
- `shared::*` shared verdict types.

### Implementation

`SecurityGate` holds the governance + crypto plugin as mut-refs and exposes `encode_outbound`/`decode_inbound`. `GovernancePolicyEngine` searches the `<topic_access_rule>` list for a domain-topic combination and returns a `PolicyDecision` with suite + protection kind + receiver-MAC set.

`PeerCapabilitiesCache` holds, per peer GUID, the last-seen capabilities, the offered protection level, a validity window, and optionally a `DelegationChain`. SPDP wire codec via the `caps_wire` module.

`peer_class::*` does the `<peer_class>` match for heterogeneous-mesh setups (vehicle ↔ C4I backend) — CIDR patterns, subject patterns, profile lookup.

`data_tagging::DataTaggingDefault` is the built-in DataTaggingPlugin impl (spec 1.2 §8.7).

`anti_squatter` implements spec §8.5.3: a replier must be registered at the ENT endpoint, otherwise reject.

`gateway_bridge::GatewayBridge` is the edge ↔ backend hop for the ZeroDDS heterogeneous mesh.

`forbid(unsafe_code)`.

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** all 7 security sibling crates + `zerodds-rtps` + `zerodds-qos`.
- **Dependents (out):** `dcps` (feature `security`), end-user builds.
- **Feature flags:** `std` (default).

### Stability

Public API + peer-caps SPDP mapping + data-tagging wire RC1-stable.
