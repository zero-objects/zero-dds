# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-dds-bridge` crate.

### Spec references

- **OMG CORBA 3.3 Part 1 §11**: POA servant path for incoming GIOP
  requests (via `corba-poa`).
- **OMG CORBA 3.3 Part 2 §15**: GIOP request form for the wire-side
  decode (via `corba-giop`).
- **OMG CORBA 3.3 Part 2 §13.6**: IOR form for object_key lookup
  (via `corba-ior`).
- **OMG DDS 1.4 §2.2.2.2.1**: register/unregister_instance path for
  CORBA lifecycle ↔ DDS discovery sync.

### Public API

- `mapping::{BridgeMapping, BridgeRoute, Direction, OperationMapping,
  TopicQosRef}` — many-to-many mapping CORBA-Object ↔ DDS-Topic.
  `Direction` covers CorbaToDds, DdsToCorba, and Bidirectional.
- `servant::BridgeServant` — a `corba-poa::Servant`-compatible servant
  that translates GIOP requests into DDS publish operations.
- `sync::{LifecycleEvent, LifecycleSync}` — propagates
  CORBA `activate_object`/`deactivate_object` as
  DDS `register_instance`/`unregister_instance` and vice versa.
- `wire::{RequestSummary, decode_giop_request_bytes,
  object_key_from_ior}` — wire helpers to `corba-giop` and
  `corba-ior` (see below).

### Wire-up to corba-giop + corba-ior (resolution F-WORKSPACE-DEAD-DEPS-AUDIT-2/3)

The two cross-crate deps `corba-giop` and `corba-ior` are in
production use since RC1:

- `wire::decode_giop_request_bytes(&[u8]) -> Option<RequestSummary>`
  calls `corba_giop::decode_message`, matches on
  `Message::Request(r)`, and extracts
  `request_id`/`operation`/`body`.
- `wire::object_key_from_ior(&Ior) -> Option<Vec<u8>>` iterates the
  `Ior::profiles`, finds the first `ProfileId::InternetIop` profile,
  and calls `TaggedProfile::as_iiop()` for body decapsulation; it
  returns `IiopProfileBody::object_key`.

Both helpers are unit-tested and resolve the DEAD-DEPS audit
(`F-WORKSPACE-DEAD-DEPS-AUDIT` items 1 + 2).

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`BridgeServant` implements `corba-poa::Servant` with a fully typed
`primary_repository_id`. The `DdsPublishSink` trait allows caller-layer
hosting of the DDS side (against `dcps` crate reader/writer handles).

### Architecture

- **Layer:** 8 (CORBA stack, Tier-C).
- **Dependencies (in):** `zerodds-corba-giop`, `zerodds-corba-ior`,
  `zerodds-corba-poa`.
- **Dependents (out):** hosting applications.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- DDS sink binding: caller-layer.

### Added — daemon wire-up

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), the catalog/healthz/metrics admin endpoint
  (§5.2), a signal watcher for graceful shutdown (§9.2), and the
  OTLP span exporter (§8.3).
- Bridge security: SSLIOP TaggedComponent 0x06 via rustls 0.23 +
  GIOP service-context auth (CSIv2 SAS token) + topic ACL via
  `zerodds-bridge-security` (Bridge-Spec §7.1/§7.2/§7.3 + CORBA §24).
- CosNotification fanout module (`notify.rs`) +
  CORBA locate cache (`locate.rs`) +
  CSIv2 wire hooks (`csiv2_wire.rs`) +
  cross-vendor interop module.
- DDS-QoS → CORBA-IIOP behavior translation in `qos_translation`.
