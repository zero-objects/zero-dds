# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-ccm`-Crate.

### Spec-Referenzen

- **OMG CCM 4.0** (`formal/2006-04-01`): §6 (Component Model),
  §7 (Container Programming Model), §13 (Lightweight CCM Profile).
- **OMG CORBA 3.3 Part 3**: §6.13 (CCM-Conformance), §7
  (Generic-Interaction), §14 (LwCCM Profile).
- **OMG Time Service 1.1**: §2.2 (TimerEventService) — Adapter
  fuer `CosEventComm::PushConsumer` als TimerEventHandler unter
  Feature `cos-event`.

### Public-API

**Komponenten-Modell (no_std + alloc):**
- `cidl::{Composition, HomeExecutor, StorageHome, StorageType}`
- `cif::{ComponentExecutor, ExecutorLocator, KeyedExecutor, SessionExecutor}`
- `component_def::{ComponentDef, HomeDef, AttributeDef, EventSinkDef,
  EventSourceDef, FacetDef, ReceptacleDef}`
- `context::ComponentContext`
- `home::{HomeDef, HomeFinder}`
- `port::{ConnectionId, EventStream, PortRegistry}`
- `dynamic_api::*` (DynamicComponent + DynamicHome)
- `orb_extensions::*`

**Container-Runtime (Feature `std`):**
- `container::{Container, ContainerError, ContainerType, LifecycleState}`
- `lifecycle::*` (CCM 4.0 §6.2 Lifecycle-State-Machine)
- `orb_core::Orb` (ORB-Configuration-Layer-Stub)
- `pss::*` (Persistent-State-Service-Stub)
- `time_psm::*` (Time-PSM Helpers)
- `timer::{TimerEventService, TimerHandle, TimerKind}`

**CosEventService-Bruecke (Feature `cos-event` + `std`):**
- `cos_event_bridge::EventChannelTimerCallback` — Adapter, der
  einen `CosEventComm::PushConsumer` als TimerEventHandler einhaengt.

**Conformance-Markers:**
- `CCM_CONFORMANCE_BASIC_LEVEL` / `..._JAVA`
- `LIGHTWEIGHT_CCM_LEVEL` / `LWCCM_RESTRICTIONS_ENFORCED` /
  `LWCCM_FILTER_ACTIVE`
- `CORBA_PART3_6_13_CCM_CONFORMANCE` /
  `CORBA_PART3_7_GENERIC_INTERACTION` /
  `CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
- `CORBA_PART2_10_6_CSIV2_LEVEL_{0,1,2}`
- `CCM_OPTIONAL_EXTENDED_LEVEL` / `CCM_ORB_VENDOR_STUB`

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

CIDL-Modell volle 5 Composition-Kategorien (Service / Session / Process /
Entity / + Empty fuer Lightweight). CIF-Trait `ComponentExecutor` mit
korrekten `set_session_context` / `ccm_activate` / `ccm_passivate` /
`ccm_remove`-Hooks gemaess CCM 4.0 §6.6.

`TimerEventService` (Spec §2.2 Time-Service) liefert eine fertige
Reactor-Loop fuer `OneShot` / `Periodic`-Timer mit thread-safem
Cancel. Optionaler Adapter `EventChannelTimerCallback` erlaubt einem
CosEventService-`PushConsumer` direkt als Timer-Handler zu fungieren
(Feature `cos-event` aktiviert die Cross-Crate-Wire-Up zu
`zerodds-corba-cos-event`).

LwCCM-Filter: `LWCCM_FILTER_ACTIVE`-Marker dokumentiert die §13.3-
konforme Subset-Validation; CIDL-Filter verbietet Generic-Navigation
und Type-Specific-Generic-Ops im Lightweight-Profil.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-corba-cos-event` (optional via Feature
  `cos-event` — Spec §2.2 TimerEventHandler-Wire-Up).
- **Dependents (out):** `zerodds-corba-ccm-lib` (DDS-Bridge,
  Persistence, Telemetry-Components), `zerodds-corba-ccm-ejb`
  (CCM↔EJB-Bruecke), `zerodds-corba-dnc` (D&C ContainerHost),
  `zerodds-rtc` (RTC = CCM + RT-Hooks).
- **Feature-Flags:** `std` (default), `alloc` (via std), `cos-event`
  (Time-Service-Bridge zu CosEventService).

### Stabilitaet

- Public-API: RC1-stabil.
- Spec §6 Komponentenmodell: voll abgedeckt.
- Spec §7 Container-Programmiermodell: voll abgedeckt.
- Spec §13 Lightweight Profile: Subset-Filter aktiv via Marker.
- Conformance-Strings durch OMG fixiert.
