# zerodds-discovery

[![docs.rs](https://img.shields.io/docsrs/zerodds-discovery)](https://docs.rs/zerodds-discovery)
[![crates.io](https://img.shields.io/crates/v/zerodds-discovery)](https://crates.io/crates/zerodds-discovery)

DDSI-RTPS-Discovery für ZeroDDS. Layer 2 (Wire-Implementation).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`, Safety-Klasse **SAFE**.

## Spec

- **DDSI-RTPS 2.5 §8.5.3** — Simple Participant Discovery (SPDP).
- **DDSI-RTPS 2.5 §8.5.4** — Simple Endpoint Discovery (SEDP).
- **XTypes 1.3 §7.6.3.3.4** — TypeLookup-Service (4 Builtin-Endpoints,
  Service-Instance-Name).
- **DDS-Security 1.2 §7.4.4 + §7.4.5** — Stateless + Volatile-Secure
  Builtin-Endpoint-Slots.

Coverage-Doc:
[`docs/spec-coverage/ddsi-rtps-2.5.md`](../../docs/spec-coverage/ddsi-rtps-2.5.md).

## Was liefert dieses Crate

### SPDP (`spdp` Modul)

- `SpdpBeacon` — Beacon-Sender (DATA-Datagram mit `ParticipantBuiltinTopicData`).
- `SpdpReader` — Beacon-Receiver-Parser.
- `DiscoveredParticipantsCache` — discovered-Participants-Set mit
  `last_seen`-Lease-Tracking.

### SEDP (`sedp` Modul)

- `SedpStack` — integrierte SEDP-State-Machine
  (Participant-Lifecycle → SEDP-Proxy-Wiring).
- `SedpPublicationsWriter` / `SedpPublicationsReader` — Reliable
  Pub/Sub-Discovery.
- `SedpSubscriptionsWriter` / `SedpSubscriptionsReader` — Reliable
  Sub-Discovery.
- `DiscoveredEndpointsCache` — discovered-Endpoints-Set.

### TypeLookup (`type_lookup` Modul)

- `TypeLookupServer` — server-side Handler mit Pagination.
- `TypeLookupClient` — client-side Correlation-Table mit
  pending-Request-Cap.
- `TypeLookupEndpoints` — 4 Builtin-Endpoint-GUIDs +
  service-instance-name Formatter.

### Security (`security` Modul)

- `SecurityBuiltinStack` — DDS-Security Stateless +
  Volatile-Secure-Endpoints (BuiltinEndpointSet-Bits 22..25).

### Match-Logic (`endpoint_match` Modul)

- Topic-Identität + Type-Kompatibilität (TypeMatcher) +
  QoS-Kompatibilität.

## Layer-Boundary an DCPS

Discovery liefert die Wire-Format-Primitives. Die Instantiierung der
Reliable-Writer/Reader-Pairs auf den Builtin-Endpoint-GUIDs liegt im
DCPS-Layer (`crates/dcps/src/runtime.rs`):

| Endpoint | Reliability | Wired in DCPS |
|---|---|---|
| SPDP | Best-Effort | ✅ |
| SEDP | Reliable | ✅ |
| TypeLookup | Reliable | siehe Layer-3-Review (DCPS) |
| DDS-Security Stateless | Best-Effort | ✅ |
| DDS-Security Volatile-Secure | Reliable | ✅ |

## Tests

```bash
cargo test -p zerodds-discovery
```

144+ Tests grün (lib + integration). Live-Interop-Tests gegen Cyclone
DDS verfügbar via `--features live-interop`.

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
