# RC1 Review — `zerodds-corba-ccm`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CCM 4.0 (`formal/2006-04-01`) §6 + §13 — voller Component-Container-Stack: Komponenten- und Home-Modell, CIDL-Datenmodell, CIF, Container-Runtime, ORB-Extensions, Persistent-State-Service-Stub, Time-PSM, TimerEventService inkl. optionaler CosEventService-Adapter via Feature `cos-event`.

## 2-3 Inhalt

- 16 src-Files (lib + 15 Module: cidl, cif, component_def, container, context, cos_event_bridge, dynamic_api, home, lifecycle, orb_core, orb_extensions, port, pss, time_psm, timer).
- **138 Unit-Tests + 1 Doc-Test grün.**

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_ccm' --type rust crates/ -g '!crates/corba-ccm/**'` → externe Konsumenten in corba-ccm-lib, corba-ccm-ejb, corba-dnc, rtc.

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `Composition` / `HomeExecutor` / `StorageHome` / `StorageType` (CIDL) | CCM 4.0 §5 / §6.7 | corba-dnc::container_host (Plan→Composition) | CONNECTED |
| `ComponentExecutor` / `KeyedExecutor` / `SessionExecutor` / `ExecutorLocator` (CIF) | CCM 4.0 §6.6 | corba-ccm-lib::dds_bridge / corba-ccm-lib::persistence / corba-ccm-lib::telemetry | CONNECTED |
| `ComponentDef` / `HomeDef` / `AttributeDef` / `FacetDef` / `ReceptacleDef` / `EventSinkDef` / `EventSourceDef` | CCM 4.0 §6.4-§6.5 | corba-ccm-lib (Component-Templates) | CONNECTED |
| `Container` / `LifecycleState` / `ContainerType` | CCM 4.0 §7.2-§7.4 | corba-ccm-ejb::ConnectorBean (Lifecycle), corba-dnc::container_host | CONNECTED |
| `TimerEventService` / `TimerHandle` / `TimerKind` | OMG Time-Service 1.1 §2.2 | rtc::executor (RT-Timer-Hooks) | CONNECTED |
| `EventChannelTimerCallback` (Feature `cos-event`) | CCM 4.0 §6.10 + Time §2.2.4 | corba-cos-event Cross-Crate-Test (F-CORBA-COS-EVENT-NOT-WIRED resolved) | CONNECTED |
| `pss::*` (Persistent-State-Service-Stub) | CCM 4.0 §10 | corba-ccm-lib::persistence | CONNECTED |
| `orb_core::Orb` (ORB-Configuration-Layer-Stub) | CCM 4.0 §6.13 (ORB-Vendor-Hook) | n/a (Caller-Hosting) | OPTIONAL-HOOK |
| `time_psm::*` | OMG Time §2.1 | rtc, corba-ccm-lib | CONNECTED |
| Conformance-Markers (`CCM_*`, `CORBA_PART3_*`, `LWCCM_*`) | CCM 4.0 §2 + CORBA P3 §6.13/§14 | Tooling-Capability-Detection | OPTIONAL-HOOK |

**Klassifikation:** Mehrheit CONNECTED via Tier-B-Konsumenten (corba-ccm-lib, corba-ccm-ejb, corba-dnc, rtc); ORB-Vendor-Hook und Conformance-Markers sind Spec-MAY-Capability-Marker (OPTIONAL-HOOK).

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett (homepage/documentation/keywords/categories).
2. lib.rs: SPDX bereits da + Doc-Test (`CCM_CONFORMANCE_BASIC_LEVEL`-Marker) ergaenzt.
3. SPDX auf alle 16 src-Files (15 neu).
4. README + CHANGELOG ueberschrieben.
5. Mirror unter `github/crates/corba-ccm/`.
6. `website/docs/corba-ccm.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** CIDL-AST + CIF-Trait-Surface + Container-Lifecycle decken §6/§7-Spec voll ab. LwCCM-Filter via `LWCCM_FILTER_ACTIVE`-Marker in §13.3-Konformitaet.
- **(b) Wire-up:** CONNECTED. Konsumenten sind corba-ccm-lib (3 Components), corba-ccm-ejb (Bean-Lifecycle), corba-dnc (Plan-Hosting), rtc (RT-Hooks).
- **(c) Getestet:** 138 Unit-Tests (Container-Lifecycle + CIDL-Roundtrips + CIF-Mocks + Timer-Periodic/OneShot + Conformance-Markers + cos_event_bridge) + 1 Doc-Test.

## 10-12 Gates

- `cargo test -p zerodds-corba-ccm`: ✅ 138 unit + 1 doc.
- `cargo clippy -p zerodds-corba-ccm --tests -- -D warnings`: ✅.
- `cargo doc --no-deps`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header mit Doc-Test ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (10 Item-Familien, 8 CONNECTED + 2 OPTIONAL-HOOK)
- §1.6 Spec-Coverage: ✅ (CCM 4.0 §6 + §7 + §13 + CORBA P3 §6.13/§7/§14 + Time-Service §2.2)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 16 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: CONNECTED.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
