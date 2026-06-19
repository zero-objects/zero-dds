# ZeroDDS — architecture overview

> **Status:** Draft v0.2 · **Audience:** Engineering, Leadership, Stakeholders
> **Next review:** after the Phase-0 kickoff

## 1 Mission

**ZeroDDS** is a complete, sovereign DDS implementation (OMG Data Distribution Service) in Rust with bindings for C, C++, C#, Java, Python and Rust. The name reflects the architectural core promise:

- **Zero external dependencies** in the safe core (only `core` + `alloc` + curated no_std crates)
- **Zero panic** in all non-comfort crates (clippy-enforced)
- **Zero unsafe** where structurally possible, every exception block with a SAFETY comment
- **Zero copy** in the shared-memory transport path
- **Zero vendor lock-in** through strictly open standards and the Apache-2.0 license

The goal is an alternative to existing commercial and open-source DDS providers that closes the gaps critical for our use cases: supply-chain sovereignty, safety qualifiability, modern observability and deep embedded integration.

## 1.1 Execution model: bootstrap before expansion

ZeroDDS is developed as an internal core project. We are our own first customer — the stack is validated against a concrete internal application (a distributed sensor and decision system on the Jetson Thor platform). External positioning, OMG membership, patent clearance, safety certification and community building only follow once a stable baseline system exists that can hold its own in internal benchmarks against eProsima Fast DDS and Eclipse Cyclone DDS.

The rationale is sober: external partnerships, audit budgets and certifications can only be mobilized once the internal proof exists. The other way around would be inefficient.

## 2 Strategic rationale

The current DDS landscape forces unacceptable trade-offs:

| Pain point | Today's reality | Our answer |
|---|---|---|
| **Transatlantic dependency** | RTI (US, ITAR/EAR-exposed), OpenDDS (US) dominate the safety segment | EU-based development, sovereign supply chain, no export-control risks |
| **Safety path** | Only RTI Connext Cert and eProsima Safe DDS offer cert evidence; both expensive or young | Safety-by-architecture from day 1, Ferrocene-based cert path to ISO 26262 ASIL D and DO-178C |
| **Security** | DDS-Security 1.1/1.2 mostly implemented, but no post-quantum crypto, no EU crypto suites | Plugin-based crypto, swappable suites, post-quantum-ready |
| **Performance tooling** | At best proprietary admin tools, hardly any OpenTelemetry integration | Native OTel instrumentation, W3C Trace Context, deterministic replay |
| **Embedded/MCU** | Fragmented: eProsima Micro XRCE-DDS for micro-ROS, RTI Micro separate | Unified codebase, XRCE client for Cortex-M with PlatformIO integration |
| **License exposure** | Commercial vendors with per-unit licensing, proprietary source base | Open license option, single-vendor lock-in avoidable |

## 3 Core properties of the target architecture

- **Spec conformance:** complete OMG DDS spec family (DCPS 1.4, RTPS 2.5, XTypes 1.3, Security 1.2, RPC 1.0, XML 1.0, XRCE 1.0) plus IDL4 with mappings to C, C++, C#, Java, Python, Rust.
- **Four deployment profiles** from one codebase: Full (desktop/server), Standard (embedded Linux/RTOS), Safe (certifiable), Micro (Cortex-M via XRCE).
- **Six language bindings:** C, C++, C#, Java, Python, Rust — all with IDL4 mapping.
- **Platform coverage:** Linux x86_64/ARM64, Windows, macOS, QNX Neutrino, VxWorks, INTEGRITY, PikeOS, Deos, Zephyr, FreeRTOS, ESP-IDF, STM32Cube, bare-metal Cortex-M.
- **Safety-ready:** safe-subset crates are no_std, no-panic, no-dynamic-alloc, Ferrocene-only. Audit path to ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+ planned.
- **Observability-first:** OpenTelemetry instrumentation throughout, Prometheus metrics, deterministic wire recording with replay, Tauri-based live dashboard.
- **PlatformIO-native:** embedded distribution as a PlatformIO library with prebuilt targets for the common framework stacks.

## 4 Success criteria

Success is measured in two stages, corresponding to the bootstrap-before-expansion strategy.

### 4.1 Bootstrap-proof criteria (internal)

The stack is considered internally proven when the following criteria are met:

1. RTPS reliable protocol implemented and successfully validated in interop tests with Cyclone DDS and Fast DDS.
2. DCPS 1.4 Minimum Profile plus Ownership and Content Subscription Profile functional.
3. DDS-Security 1.2 with the standard builtin plugin suite (AES-GCM, RSA/ECDSA) running and interop-validated.
4. C and C++ bindings functional, IDL4 mapping for those two languages complete.
5. Core application (internal use case) runs in production on ZeroDDS, solving the application's classic pub-sub requirements.
6. Latency and throughput on reference hardware (ARM Jetson class and x86_64 server) within ±30% of the eProsima Fast DDS values on the same test setup.

### 4.2 Expansion criteria (external, when funds are available)

After confirmed internal proof:

1. Interop certification at the OMG Plug-Fest with at least RTI Connext, Cyclone DDS, Fast DDS.
2. All six language bindings functional, all IDL4 mappings validated.
3. XRCE client running on at least three embedded platforms.
4. Observability stack complete: OpenTelemetry emission, Prometheus exporter, wire recorder, Tauri dashboard.
5. Safety audit readiness for the safe subset confirmed by an external auditor.
6. Latency and throughput within ±20% of the peak values of established vendors.

## 5 Explicit non-goals

Deliberate restrictions we keep out of scope to maintain focus:

- **DLRL (Data Local Reconstruction Layer):** the OMG spec part is largely orphaned, no relevant deployments. We do not implement it.
- **CORBA interop:** historical legacy. No customer demand in our target segment.
- **Proprietary wire extensions:** we stay strictly with the RTPS standard, no vendor-specific extensions like RTI FlatData or OpenSplice DDSI2E.
- **Legacy language bindings:** Ada, Fortran, JavaScript, Go are not in the initial scope.
- **Commercial SaaS management plane:** cloud-hosted admin tools are not a goal. Observability is on-premises or via customer cloud.

## 6 Project structure at a glance

- **Core team (bootstrap phase):** 2–4 senior engineers in the internal core team. No external hiring ramp until the internal proof is reached.
- **Claude-Teams augmentation:** throughout, realistically 4–8× acceleration depending on the work area. With a small core team, Claude Teams is the primary force multiplier.
- **External partnerships:** deferred to the post-proof phase. Ferrous Systems, OMG membership, patent-attorney engagement and community building are activated once the baseline system is internally proven and external funds are available.
- **Governance:** internal core project of the sponsoring company. No foundation model, no external governance framework. Apache-2.0 license chosen to preserve later optionality (donation to a foundation remains possible but requires no planning now).
- **License:** Apache 2.0 (decided).
- **Time horizon:** bootstrap phase 10–14 months to MVP with the internal application; afterwards iterative further development depending on resource availability.

## 7 Documentation suite

The following documents together form the architectural foundation:

| # | Document | Purpose |
|---|---|---|
| 00 | `00_overview.md` | This document — strategic mission |
| 01 | `01_scope_and_specs.md` | OMG spec coverage and conformance goals |
| 02 | `02_architecture.md` | System architecture and crate workspace |
| 03 | `03_profiles_and_platforms.md` | Four profiles, platform matrix, binding matrix |
| 04 | `04_safety_by_architecture.md` | Safe-subset contract and CI enforcement |
| 05 | `05_observability_and_tooling.md` | Live insights, recording, replay, UI |
| 06 | `06_roadmap.md` | Phase plan, milestones, resources |
| 07 | `07_risks_and_strategy.md` | Patent, IP, community, competitive response |

Each of these documents is independently readable and can be used in parallel by different stakeholder groups. On changes, cross-reference consistency is to be maintained — a Claude-Teams-supportable pattern.
