# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-ccm-lib` crate.

### Spec references

- **OMG CCM 4.0** (`formal/2006-04-01`): §6 (Component Model),
  §10 (Persistent State Service), §6.10 (Events).
- **OMG DDS 1.4**: §2.2 (DCPS) — topic mapping CCM port → DDS topic.
- **ZeroDDS monitor topic spec** — `__ZeroDDS_CcmTelemetry`.

### Public API

- `dds_bridge::{BridgeError, DdsBridgeComponent, MappingDirection,
  TopicMapping}` — bidirectional CCM↔DDS bridge component, maps
  CCM EventSinks and EventSources onto DDS DataReader/Writer.
- `persistence::{PersistenceError, PersistenceStorageComponent,
  StorageEntry}` — Persistent State Service §10 storage component.
- `telemetry::{TelemetryComponent, TelemetryEvent, TelemetryKind}`
  — component lifecycle telemetry on DCPS topic
  `__ZeroDDS_CcmTelemetry`.

### Implementation

`#![no_std]` with `extern crate alloc`; `#![forbid(unsafe_code)]`.
All three components implement `corba-ccm::ComponentExecutor`
and run through the standard lifecycle (`set_session_context` →
`ccm_activate` → operation → `ccm_passivate` → `ccm_remove`).

`DdsBridgeComponent` carries a list of `TopicMapping` entries
with `MappingDirection::SinkSubscribesTopic` (DDS reader feeds the CCM sink)
or `SourcePublishesTopic` (CCM source feeds the DDS writer). The
DDS side is resolved by the caller layer (the host must provide `dcps`
handles for reader/writer).

`PersistenceStorageComponent` stores `StorageEntry` records
key→value in an in-memory `BTreeMap` storage following §10
storage-home semantics.

`TelemetryComponent` emits `TelemetryEvent`s with `TelemetryKind::
{Activated, Passivated, Removed, ConfigurationCompleted}` onto the
ZeroDDS monitor topic.

### Architecture

- **Layer:** 8 (CORBA stack, Tier B).
- **Dependencies (in):** `zerodds-corba-ccm`.
- **Dependents (out):** hosting applications (caller layer).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- DCPS binding: caller-layer trait implementations.
