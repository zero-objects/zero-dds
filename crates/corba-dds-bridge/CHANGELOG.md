# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-dds-bridge`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 1 §11**: POA-Servant-Pfad fuer eingehende GIOP-
  Requests (via `corba-poa`).
- **OMG CORBA 3.3 Part 2 §15**: GIOP-Request-Form fuer den Wire-Side-
  Decode (via `corba-giop`).
- **OMG CORBA 3.3 Part 2 §13.6**: IOR-Form fuer object_key-Lookup
  (via `corba-ior`).
- **OMG DDS 1.4 §2.2.2.2.1**: register/unregister_instance-Pfad fuer
  CORBA-Lifecycle ↔ DDS-Discovery-Sync.

### Public-API

- `mapping::{BridgeMapping, BridgeRoute, Direction, OperationMapping,
  TopicQosRef}` — Many-to-Many-Mapping CORBA-Object ↔ DDS-Topic.
  `Direction` deckt CorbaToDds, DdsToCorba und Bidirectional ab.
- `servant::BridgeServant` — `corba-poa::Servant`-kompatibler Servant,
  der GIOP-Requests in DDS-Publish-Operations uebersetzt.
- `sync::{LifecycleEvent, LifecycleSync}` — propagiert
  CORBA-`activate_object`/`deactivate_object` als
  DDS-`register_instance`/`unregister_instance` und umgekehrt.
- `wire::{RequestSummary, decode_giop_request_bytes,
  object_key_from_ior}` — Wire-Helpers zu `corba-giop` und
  `corba-ior` (siehe unten).

### Wire-Up zu corba-giop + corba-ior (Resolution F-WORKSPACE-DEAD-DEPS-AUDIT-2/3)

Die beiden Cross-Crate-Deps `corba-giop` und `corba-ior` sind seit
RC1 produktiv genutzt:

- `wire::decode_giop_request_bytes(&[u8]) -> Option<RequestSummary>`
  ruft `corba_giop::decode_message`, matched auf
  `Message::Request(r)` und extrahiert
  `request_id`/`operation`/`body`.
- `wire::object_key_from_ior(&Ior) -> Option<Vec<u8>>` iteriert die
  `Ior::profiles`, sucht das erste `ProfileId::InternetIop`-Profile
  und ruft `TaggedProfile::as_iiop()` zur Body-Decapsulation; gibt
  `IiopProfileBody::object_key` zurueck.

Beide Helfer sind unit-getestet und beheben das DEAD-DEPS-Audit
(`F-WORKSPACE-DEAD-DEPS-AUDIT` Items 1 + 2).

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`BridgeServant` implementiert `corba-poa::Servant` mit voll typisierter
`primary_repository_id`. `DdsPublishSink`-Trait erlaubt Caller-Layer-
Hosting der DDS-Seite (gegen `dcps`-Crate-Reader/Writer-Handles).

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-C).
- **Dependencies (in):** `zerodds-corba-giop`, `zerodds-corba-ior`,
  `zerodds-corba-poa`.
- **Dependents (out):** Hosting-Anwendungen.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- DDS-Sink-Bindung: Caller-Layer.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: SSLIOP TaggedComponent 0x06 via rustls 0.23 +
  GIOP-Service-Context-Auth (CSIv2 SAS-Token) + Topic-ACL via
  `zerodds-bridge-security` (Bridge-Spec §7.1/§7.2/§7.3 + CORBA §24).
- CosNotification-Fanout-Modul (`notify.rs`) +
  CORBA-Locate-Cache (`locate.rs`) +
  CSIv2-Wire-Hooks (`csiv2_wire.rs`) +
  Cross-Vendor-Interop-Modul.
- DDS-QoS → CORBA-IIOP-Behavior-Translation in `qos_translation`.
