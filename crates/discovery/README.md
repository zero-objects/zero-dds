# zerodds-discovery

[![docs.rs](https://img.shields.io/docsrs/zerodds-discovery)](https://docs.rs/zerodds-discovery)
[![crates.io](https://img.shields.io/crates/v/zerodds-discovery)](https://crates.io/crates/zerodds-discovery)

DDSI-RTPS discovery for ZeroDDS. Layer 2 (wire implementation).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`, safety class **SAFE**.

## Spec

- **DDSI-RTPS 2.5 §8.5.3** — Simple Participant Discovery (SPDP).
- **DDSI-RTPS 2.5 §8.5.4** — Simple Endpoint Discovery (SEDP).
- **XTypes 1.3 §7.6.3.3.4** — TypeLookup service (4 builtin endpoints,
  service-instance-name).
- **DDS-Security 1.2 §7.4.4 + §7.4.5** — Stateless + Volatile-Secure
  builtin-endpoint slots.

Coverage-Doc:
[`docs/spec-coverage/ddsi-rtps-2.5.md`](../../docs/spec-coverage/ddsi-rtps-2.5.md).

## What this crate provides

### SPDP (`spdp` module)

- `SpdpBeacon` — beacon sender (DATA datagram with `ParticipantBuiltinTopicData`).
- `SpdpReader` — beacon-receiver parser.
- `DiscoveredParticipantsCache` — discovered-participants set with
  `last_seen` lease tracking.

### SEDP (`sedp` module)

- `SedpStack` — integrated SEDP state machine
  (participant lifecycle → SEDP proxy wiring).
- `SedpPublicationsWriter` / `SedpPublicationsReader` — reliable
  pub/sub discovery.
- `SedpSubscriptionsWriter` / `SedpSubscriptionsReader` — reliable
  sub discovery.
- `DiscoveredEndpointsCache` — discovered-endpoints set.

### TypeLookup (`type_lookup` module)

- `TypeLookupServer` — server-side handler with pagination.
- `TypeLookupClient` — client-side correlation table with a
  pending-request cap.
- `TypeLookupEndpoints` — 4 builtin-endpoint GUIDs +
  service-instance-name formatter.

### Security (`security` module)

- `SecurityBuiltinStack` — DDS-Security Stateless +
  Volatile-Secure endpoints (BuiltinEndpointSet bits 22..25).

### Match logic (`endpoint_match` module)

- Topic identity + type compatibility (TypeMatcher) +
  QoS compatibility.

## Layer boundary toward DCPS

Discovery provides the wire-format primitives. Instantiating the
reliable writer/reader pairs on the builtin-endpoint GUIDs is the
responsibility of the DCPS layer (`crates/dcps/src/runtime.rs`):

| Endpoint | Reliability | Wired in DCPS |
|---|---|---|
| SPDP | Best-Effort | ✅ |
| SEDP | Reliable | ✅ |
| TypeLookup | Reliable | see layer-3 review (DCPS) |
| DDS-Security Stateless | Best-Effort | ✅ |
| DDS-Security Volatile-Secure | Reliable | ✅ |

## Tests

```bash
cargo test -p zerodds-discovery
```

144+ tests green (lib + integration). Live-interop tests against Cyclone
DDS available via `--features live-interop`.

## License

Apache-2.0 OR MIT — see workspace root.
