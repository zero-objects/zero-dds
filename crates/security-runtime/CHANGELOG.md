# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-runtime`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §8.5.3, §9.5.
- **OMG DDS-Security 1.2** §8.7 (DataTagging).
- **ZeroDDS-Architektur §09** Heterogeneous-Mesh + Delegation.

### Public-API

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
- `shared::*` Shared-Verdict-Types.

### Implementierung

`SecurityGate` haelt Governance + Crypto-Plugin als Mut-Refs und exponiert `encode_outbound`/`decode_inbound`. `GovernancePolicyEngine` durchsucht `<topic_access_rule>`-Liste fuer eine Domain-Topic-Kombi und liefert `PolicyDecision` mit Suite + Protection-Kind + Receiver-MAC-Set.

`PeerCapabilitiesCache` haelt pro Peer-GUID die zuletzt gesehenen Capabilities, das angebotene Protection-Level, ein Validity-Window, sowie optional eine `DelegationChain`. SPDP-Wire-Codec via `caps_wire`-Modul.

`peer_class::*` macht den `<peer_class>`-Match fuer Heterogeneous-Mesh-Setups (Vehicle ↔ C4I-Backend) — CIDR-Pattern, Subject-Patterns, Profile-Lookup.

`data_tagging::DataTaggingDefault` ist die Built-in DataTaggingPlugin-Impl (Spec 1.2 §8.7).

`anti_squatter` implementiert Spec §8.5.3: ein Replier muss am ENT-Endpoint registriert sein, sonst Reject.

`gateway_bridge::GatewayBridge` ist der Edge-↔-Backend-Hop fuer ZeroDDS-Heterogeneous-Mesh.

`forbid(unsafe_code)`.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** alle 7 Security-Schwester-Crates + `zerodds-rtps` + `zerodds-qos`.
- **Dependents (out):** `dcps` (Feature `security`), end-user-Builds.
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + Peer-Caps-SPDP-Mapping + DataTagging-Wire RC1-stabil.
