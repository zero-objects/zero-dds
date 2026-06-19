# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-ccm` crate.

### Spec references

- **OMG CCM 4.0** (`formal/2006-04-01`): §6 (Component Model),
  §7 (Container Programming Model), §13 (Lightweight CCM Profile).
- **OMG CORBA 3.3 Part 3**: §6.13 (CCM conformance), §7
  (Generic Interaction), §14 (LwCCM Profile).
- **OMG Time Service 1.1**: §2.2 (TimerEventService) — adapter
  for `CosEventComm::PushConsumer` as a TimerEventHandler under the
  `cos-event` feature.

### Public API

**Component model (no_std + alloc):**
- `cidl::{Composition, HomeExecutor, StorageHome, StorageType}`
- `cif::{ComponentExecutor, ExecutorLocator, KeyedExecutor, SessionExecutor}`
- `component_def::{ComponentDef, HomeDef, AttributeDef, EventSinkDef,
  EventSourceDef, FacetDef, ReceptacleDef}`
- `context::ComponentContext`
- `home::{HomeDef, HomeFinder}`
- `port::{ConnectionId, EventStream, PortRegistry}`
- `dynamic_api::*` (DynamicComponent + DynamicHome)
- `orb_extensions::*`

**Container runtime (feature `std`):**
- `container::{Container, ContainerError, ContainerType, LifecycleState}`
- `lifecycle::*` (CCM 4.0 §6.2 Lifecycle-State-Machine)
- `orb_core::Orb` (ORB configuration layer stub)
- `pss::*` (Persistent State Service stub)
- `time_psm::*` (Time PSM helpers)
- `timer::{TimerEventService, TimerHandle, TimerKind}`

**CosEventService bridge (feature `cos-event` + `std`):**
- `cos_event_bridge::EventChannelTimerCallback` — adapter that
  hooks a `CosEventComm::PushConsumer` in as a TimerEventHandler.

**Conformance markers:**
- `CCM_CONFORMANCE_BASIC_LEVEL` / `..._JAVA`
- `LIGHTWEIGHT_CCM_LEVEL` / `LWCCM_RESTRICTIONS_ENFORCED` /
  `LWCCM_FILTER_ACTIVE`
- `CORBA_PART3_6_13_CCM_CONFORMANCE` /
  `CORBA_PART3_7_GENERIC_INTERACTION` /
  `CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
- `CORBA_PART2_10_6_CSIV2_LEVEL_{0,1,2}`
- `CCM_OPTIONAL_EXTENDED_LEVEL` / `CCM_ORB_VENDOR_STUB`

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

CIDL model with all 5 composition categories (Service / Session / Process /
Entity / + Empty for Lightweight). CIF trait `ComponentExecutor` with
correct `set_session_context` / `ccm_activate` / `ccm_passivate` /
`ccm_remove` hooks per CCM 4.0 §6.6.

`TimerEventService` (spec §2.2 Time Service) provides a complete
reactor loop for `OneShot` / `Periodic` timers with thread-safe
cancellation. The optional `EventChannelTimerCallback` adapter lets a
CosEventService `PushConsumer` act directly as a timer handler
(the `cos-event` feature activates the cross-crate wire-up to
`zerodds-corba-cos-event`).

LwCCM filter: the `LWCCM_FILTER_ACTIVE` marker documents the §13.3-
compliant subset validation; the CIDL filter forbids generic navigation
and type-specific generic ops in the Lightweight Profile.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** `zerodds-corba-cos-event` (optional via the
  `cos-event` feature — spec §2.2 TimerEventHandler wire-up).
- **Dependents (out):** `zerodds-corba-ccm-lib` (DDS bridge,
  persistence, telemetry components), `zerodds-corba-ccm-ejb`
  (CCM↔EJB bridge), `zerodds-corba-dnc` (D&C ContainerHost),
  `zerodds-rtc` (RTC = CCM + RT hooks).
- **Feature flags:** `std` (default), `alloc` (via std), `cos-event`
  (Time Service bridge to CosEventService).

### Stability

- Public API: RC1-stable.
- Spec §6 Component Model: fully covered.
- Spec §7 Container Programming Model: fully covered.
- Spec §13 Lightweight Profile: subset filter active via marker.
- Conformance strings fixed by OMG.
