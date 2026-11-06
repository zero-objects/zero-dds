# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Removed

- The library modules `locate`, `notify`, `qos_translation`, `servant`,
  `sync` and `wire`, with `BridgeServant` at the crate root. Neither the
  daemon nor another crate or example used them: `bridge_daemon` answers
  LocateRequest (spec §4.7), maps requests, replies and notifications
  (§4.3, §4.4) and the QoS (§6) itself, and the modules described other
  designs (a POA servant, CosNotification structured events, a lifecycle
  sync, best-effort as oneway) that the spec does not have. Their 24 tests
  checked them against themselves; so did the 6 of `mapping`, which stays
  because two examples describe their mappings with it. The dependency on
  `zerodds-corba-poa` goes.

### Fixed

- The TAO programs of `tests/tao_e2e.rs` start with at most 1024 open
  files. TAO 2.5.24 sizes its tables by the limit; in the interop image
  (1073741816) a client's transport cache was not allocated and the first
  invocation ended with `SIGSEGV` in `Transport_Cache_Manager_T::bind_i`
  (gdb, pipeline 8912). Both tests pass there again (pipeline 8917) and run
  in `vendor-live-corba-tao` (OPEN-ITEMS 101).

- A DDS sample whose representation identifier is not `CDR_BE` or `CDR_LE`
  (DDS-XTypes 1.3 §7.6.3.1.2, Table 60) is no GIOP body (spec §4.3, §4.4):
  a reply sample is answered `MARSHAL`, a notify sample is dropped and
  counted. The daemon refused only XCDR2 and let a `PL_CDR` sample (an
  `@mutable` type in XCDR1, a parameter list) through as the body
  (OPEN-ITEMS 60).
- The IIOP and SSLIOP listeners are bound before the IORs are written to
  `ior_file` and bound in the NameService (spec §9.1 steps 5 and 6). The
  daemon published them first; a TAO client that resolved the name at once
  got `TRANSIENT`.
- A NameService that does not answer within `corba.ior.naming_retry_ms`
  ends the start with exit code 5 (spec §2), not 1.

### Removed

- The feature `cross-vendor-tests`: `tests/cross_vendor.rs` builds with
  `std`; its `catior` test is `#[ignore]` in the crate tier and runs with the
  omniORB and JacORB tests in the integration pillar's job
  `integration-bridges-vendor-corba`, the TAO tests in `vendor-live-corba-tao`.

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
