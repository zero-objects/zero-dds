# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-ccm-lib`-Crate.

### Spec-Referenzen

- **OMG CCM 4.0** (`formal/2006-04-01`): §6 (Component Model),
  §10 (Persistent State Service), §6.10 (Events).
- **OMG DDS 1.4**: §2.2 (DCPS) — Topic-Mapping CCM-Port→DDS-Topic.
- **ZeroDDS Monitor-Topic-Spec** — `__ZeroDDS_CcmTelemetry`.

### Public-API

- `dds_bridge::{BridgeError, DdsBridgeComponent, MappingDirection,
  TopicMapping}` — bidirektionale CCM↔DDS-Bridge-Component, mappt
  CCM-EventSinks und EventSources auf DDS-DataReader/Writer.
- `persistence::{PersistenceError, PersistenceStorageComponent,
  StorageEntry}` — Persistent-State-Service §10 Storage-Component.
- `telemetry::{TelemetryComponent, TelemetryEvent, TelemetryKind}`
  — Component-Lifecycle-Telemetrie auf DCPS-Topic
  `__ZeroDDS_CcmTelemetry`.

### Implementierung

`#![no_std]` mit `extern crate alloc`; `#![forbid(unsafe_code)]`.
Alle drei Komponenten implementieren `corba-ccm::ComponentExecutor`
und durchlaufen den Standard-Lifecycle (`set_session_context` →
`ccm_activate` → Operation → `ccm_passivate` → `ccm_remove`).

`DdsBridgeComponent` traegt eine Liste von `TopicMapping`-Eintraegen
mit `MappingDirection::SinkSubscribesTopic` (DDS-Reader speist CCM-Sink)
oder `SourcePublishesTopic` (CCM-Source speist DDS-Writer). Die
DDS-Seite wird Caller-Layer-resolved (Hosting muss `dcps`-Handles
fuer Reader/Writer bereitstellen).

`PersistenceStorageComponent` speichert `StorageEntry`-Records
key→value in einem in-memory `BTreeMap`-Storage gemaess §10
Storage-Home-Semantik.

`TelemetryComponent` emittiert `TelemetryEvent`s mit `TelemetryKind::
{Activated, Passivated, Removed, ConfigurationCompleted}` auf das
ZeroDDS-Monitor-Topic.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-B).
- **Dependencies (in):** `zerodds-corba-ccm`.
- **Dependents (out):** Hosting-Anwendungen (Caller-Layer).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- DCPS-Anbindung: Caller-Layer-Trait-Implementations.
