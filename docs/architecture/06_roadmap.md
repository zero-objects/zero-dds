# Roadmap and phase plan

> **Status:** Draft v0.2
> **Dependencies:** all preceding documents

## 1 Base assumptions

- **Execution model:** bootstrap before expansion. Internal proof first, external partnerships afterwards.
- **Core team (bootstrap era):** 2–4 senior engineers internal.
- **Claude-Teams augmentation:** throughout, realistically a 4–8× throughput multiplier depending on the work area.
- **External partnership (Ferrous Systems):** only in the expansion era, after internal proof and once external funds are mobilized.
- **OMG membership, patent clearance, community building:** also expansion era.
- **Total horizon to internal proof:** 10–14 calendar months.
- **Total horizon to external cert-ready:** +18–24 months after proof, depending on resources.

## 2 Three eras at a glance

| Era | Duration | Team | Goal |
|---|---|---|---|
| **Bootstrap era** (Phases 0–2) | 10–14 months | 2–4 internal + Claude Teams | Internal MVP, the stack carries the internal target application |
| **Proof era** (Phases 3–4) | 6–10 months | 3–5 internal + Claude Teams | Benchmarks against eProsima/Cyclone passed, external readiness |
| **Expansion era** (Phases 5+) | depending on funds | 5+ internal + partners | OMG, Ferrous, cert, community |

## 3 Phase 0: Foundation (months 1–2) · bootstrap era

**Goal:** the architecture is accepted, the workspace skeleton stands, first technical spikes passed. Focus on code readiness, no external activities.

### Results

- All architecture documents (00–07) reviewed and internally released
- Workspace skeleton with all crates (empty, but CI-wired) in the repository
- CI pipeline: build matrix for Full + Standard for Linux x86_64/ARM64
- Custom `zerodds-lint` clippy plugin with the most important safety rules (even without immediate Ferrocene integration)
- **IDL4 parser (grammar-driven)**: complete OMG IDL 4.2 parser with an Earley engine,
  BNF as runtime data, delta composition for version/vendor variants,
  at least one vendor delta (RTI or Cyclone) as proof of concept.
  Migration capability is an explicit sales argument against eProsima/RTI
  (see `07_risks_and_strategy.md §3.2`). Scope and milestones:
  `docs/rfcs/0001-idl-parser-architecture.md` and
  `.planning/wp-0.3-idl-parser/PLAN.md`
- CDR/XCDR1 prototype: serializes/deserializes the base types from the XTypes spec annex
- UDP transport and a simplest best-effort writer/reader communicate between two test binaries
- Hello-World interop: a self-built writer publishes to a Cyclone DDS subscriber successfully

**Deliberately not in Phase 0:** OMG vendor-ID application, Ferrous Systems engagement, patent clearance, external announcements. These activities belong in the expansion era.

### Work packages

| Package | Responsibility | Claude-Teams use |
|---|---|---|
| WP 0.1: Architecture review cycle | Team internal, moderated by the tech lead | Document coherence checks |
| WP 0.2: Workspace skeleton + CI | Platform | Template generation, CI config |
| WP 0.3: IDL4 parser (grammar-driven, full scope) | Protocol | Earley engine, BNF transcription, AST, vendor-delta PoC. See `docs/rfcs/0001-idl-parser-architecture.md` and `.planning/wp-0.3-idl-parser/PLAN.md`. **Scope change 2026-04-17:** expanded from "prototype parses samples" to a full IDL 4.2 parser with migration capability; duration 5–7 weeks instead of 1–2 weeks. **Status 2026-04-18: COMPLETE** (M1–M7 reached; 442 lib + integration tests green, RTI delta as PoC, `zerodds-idlc --parse-only` functional; see `crates/idl/README.md`) |
| WP 0.4: CDR prototype | Protocol | XCDR2 encoder/decoder (primitives/composite/extensibility). **Status 2026-04-18: COMPLETE** (4-week spike; 84 lib + 7 integration tests; see `crates/cdr/README.md` and `.planning/wp-0.4-cdr-prototyp/PLAN.md`) |
| WP 0.5: UDP + minimal RTPS writer | Protocol | RTPS wire types + submessages (DATA/HEARTBEAT/ACKNACK/GAP) + UDP transport + best-effort writer/reader. **Status 2026-04-18: COMPLETE** (4-week spike; 88 zerodds-rtps + 6 zerodds-transport + 9 zerodds-transport-udp tests; E2E write→UDP→read functional; see `crates/rtps/README.md`) |
| WP 0.6: Cyclone interop harness | Platform | Wire compliance against Cyclone DDS reference frames + docker-compose harness for future live captures. **Status 2026-04-18: COMPLETE** (2-week spike; 11 compliance tests; ZeroDDS DATA bytes byte-identical to the Cyclone layout. Live interop needs discovery from Phase 1 — see `tests/interop/COMPLIANCE.md`) |
| WP 0.7-A: SPDP live discovery (pulled forward from Phase 1) | Protocol | Multicast UDP + ParameterList + ParticipantBuiltinTopicData + SpdpBeacon/Reader + cache + demo. **Status 2026-04-18: COMPLETE** (3-week spike + 1-day polish; pulled forward from Phase 1, because live interop tests are otherwise impossible; 9 SPDP + 24 new zerodds-rtps tests; spdp_demo verifies E2E loopback discovery; **LIVE MULTI-VENDOR INTEROP VERIFIED**: ZeroDDS discovers Eclipse Cyclone DDS (vendor 0x0110) and eProsima Fast-DDS (vendor 0x010F) on Debian 12 — see `tests/interop/INTEROP_REPORT.md`. SEDP + reliable stay in Phase 1) |
| WP 0.7: `zerodds-lint` plugin (safety rules) | Quality | Lint rules from the safety spec. **Status 2026-04-18: COMPLETE** (Phase-0 variant: AST-based on stable Rust, no nightly/dylint. 6 lints implemented: `dds_require_safety_comment`, `dds_no_dyn_in_safe`, `dds_safety_classification_present`, `dds_no_panic_in_safe`, `dds_no_alloc_in_hot_path`, `dds_bounded_recursion`. CI job `zerodds-lint` active. Existing code sweep-clean after 22 recursion markers in `crates/idl`. Real clippy-plugin variant with type info → Phase 1) |

An interim provisional vendor ID from the OMG developer range is used until a formal application is filed in the expansion era.

### Risks in Phase 0

- **Spec interpretation dispute:** OMG specs are ambiguous in some places. Mitigation: reference Cyclone DDS and Fast DDS as an "existence proof" and document deviations.
- **Scope creep from a small team:** with 2–4 FTE, hard priorities must be set. Mitigation: strict phase-gate discipline, no pulling forward of expansion-era features.
- **Claude-Teams dependency real:** with a very small internal team, Claude augmentation becomes the critical path. Mitigation: pair-programming pattern instead of solo-agent generation; human review on all protocol decisions.

## 4 Phase 1: Protocol core (months 3–8) · bootstrap era

**Goal:** RTPS reliable protocol complete, discovery works, XTypes core implemented. Focus on correctness and interop with Cyclone DDS.

**Team:** 2–3 internal engineers + Claude Teams.

### Results

- `zerodds-rtps`: complete RTPS 2.5 wire protocol
  - Reliable writer/reader state machines
  - Heartbeats, acknacks, gaps, fragmentation
  - Best-effort path
  - Multi-writer, multi-reader
- `zerodds-discovery`: SPDP + SEDP
- `zerodds-types`: XTypes 1.3 core (TypeLookup as a stretch goal, if needed Phase 2)
- `zerodds-qos`: all standard QoS policies, request/offered compatibility
- `zerodds-transport-tcp`: TCP PSM functional
- `zerodds-transport-shm`: shared-memory transport functional (Linux)
- Interop tests in CI against Cyclone DDS and Fast DDS
- First internal release: `v0.1.0-alpha`

### Work packages

| Package | Weeks | Claude-Teams leverage |
|---|---|---|
| WP 1.1: Reliable protocol | 8 | High for boilerplate, low for correctness details |
| WP 1.2: Fragmentation | 3 | Medium |
| WP 1.3: SPDP | 4 | High — built-in-topic serialization |
| WP 1.4: SEDP | 5 | High |
| WP 1.5: TypeLookup | 4 | High |
| WP 1.6: XTypes type compatibility | 6 | Medium — rules formally derivable from spec |
| WP 1.7: QoS policies + matrix | 4 | High — the matrix is table-based |
| WP 1.8: TCP transport | 3 | High |
| WP 1.9: Shared-memory transport | 4 | Medium |
| WP 1.10: Compliance test suite | 6 | High — tests generatable from spec vectors |
| WP 1.11: CI interop harness | 2 | High |

### Milestones

- **M1.1 (month 4):** reliable-protocol prototype works between two own nodes
- **M1.2 (month 5):** discovery complete, the first third-party peer sees us in its discovery
- **M1.3 (month 7):** XTypes with type evolution works
- **M1.4 (month 8):** CI runs green against Cyclone and Fast DDS on Hello-World + chat test

## 5 Phase 2: Bootstrap MVP (months 9–13) · bootstrap era

**Goal:** the stack carries the internal target application. DCPS API in production, DDS-Security functional, C/C++ bindings complete. Further bindings deferred to the proof era.

**Team:** 3–4 internal engineers + Claude Teams.

### Results

- `zerodds-dcps`: complete DCPS API in idiomatic Rust
- `zerodds-security`: DDS-Security 1.2 plugin framework + built-in Auth/AC/Crypto
  - PKI-based authentication (Ed25519, RSA, ECDSA) — **standard suite as default for interop**
  - Permissions documents
  - AES-GCM + AES-GMAC for data protection — **standard suite as default**
  - HSM support and EU crypto suites: plugin-ready, concrete implementation in a later phase
- `zerodds-sys`: stable C-ABI (frozen from the end of this phase)
- `zerodds-cpp`: C++17 wrapper + IDL4-C++ mapping functional
- `zerodds-xml`: DDS-XML parser + QoS-profile loader
- The internal target application runs on ZeroDDS with a real multi-node topology
- Internal release: `v0.2.0-mvp`

**Deliberately not in Phase 2:** C#, Java, Python bindings; OMG Plug-Fest participation; external pilot users. All proof era.

### Work packages

| Package | Weeks | Claude-Teams leverage |
|---|---|---|
| WP 2.1: DCPS public API | 6 | High |
| WP 2.2: DDS-Security auth plugin (standard suite) | 5 | Medium — crypto details need reviews |
| WP 2.3: DDS-Security AC plugin | 4 | High |
| WP 2.4: DDS-Security crypto plugin (standard suite) | 4 | Medium |
| WP 2.5: C-ABI design + freeze | 3 | Medium — must be stable |
| WP 2.6: C++ binding + IDL4-C++ | 5 | High |
| WP 2.7: DDS-XML | 3 | High |
| WP 2.8: Internal application integration | 4 | Medium |

### Milestones

- **M2.1 (month 10):** Rust DCPS API fully functional
- **M2.2 (month 11):** security auth flow against a Cyclone DDS security peer successful
- **M2.3 (month 12):** C/C++ bindings compile and Hello-World runs
- **M2.4 (month 13):** **bootstrap proof reached** — the internal application runs on ZeroDDS

## 6 Phase 3: Internal hardening and tooling (months 14–17) · proof era

**Goal:** ZeroDDS hardened in the internal application, tooling and observability at a production level for our own operational purposes.

**Team:** 3–4 internal engineers + Claude Teams.

### Results

- Internal deployment hardening: chaos testing against real networks, packet-loss scenarios, reboot tests
- `zerodds-recorder`: deterministic recording + replay (important for internal post-mortems and regression)
- `zerodds-monitor`: OpenTelemetry instrumentation throughout
- `zerodds-dashboard`: Tauri app with discovery graph, metrics, replay browser
- `zerodds-admin`: admin CLI
- `zerodds-perf`: load generator + benchmark suite
- First benchmark measurements against eProsima Fast DDS and Cyclone DDS on the same test setup
- Internal release: `v0.3.0`

### Work packages

| Package | Weeks | Claude-Teams leverage |
|---|---|---|
| WP 3.1: Chaos test suite | 4 | High |
| WP 3.2: Recording format + recorder | 4 | High |
| WP 3.3: Replay engine | 4 | High |
| WP 3.4: OTel instrumentation | 3 | Very high — mechanical |
| WP 3.5: Metric catalog + exporter | 2 | Very high |
| WP 3.6: Tauri dashboard UI | 6 | High |
| WP 3.7: Admin CLI | 2 | High |
| WP 3.8: Performance benchmark harness + eProsima comparison | 3 | Medium |
| WP 3.9: Internal application hardening | 4 | Low — real bugs need human debugging |

### Milestones

- **M3.1 (month 15):** the dashboard shows a live discovery graph from a real internal system
- **M3.2 (month 16):** first benchmarks against eProsima, gap quantified
- **M3.3 (month 17):** the stack is production-capable for the internal application without a safety mandate

## 7 Phase 4: External readiness (months 18–22) · proof era

**Goal:** the stack is ready for external visibility. Further bindings, XRCE, PlatformIO, performance parity against eProsima.

**Team:** 4–5 internal engineers + Claude Teams. External expertise selectively (performance consulting, cross-compile support).

### Results

- Remaining bindings: C# (NuGet + NativeAOT), Java (Pure-Java DDS-Java-PSM), Python (PyO3) all functional with IDL4 mappings
- `zerodds-rpc`: DDS-RPC 1.0 framework complete
- `zerodds-xrce-client` + `zerodds-xrce-agent`: micro profile for Cortex-M33/M7/M4, ESP32, STM32
- PlatformIO library repository with examples for ESP32 and STM32
- Performance hardening: zero-copy SHM paths, TSN-optional
- Performance parity with eProsima Fast DDS within ±20% on reference hardware
- Stress and chaos tests: network partitions, packet loss, clock skew
- Documentation consolidated: user guide, developer guide, operator guide
- First potentially public release: `v1.0.0-rc` (decision about going public at the phase end)

### Work packages

| Package | Weeks | Priority |
|---|---|---|
| WP 4.1: C# binding + IDL4-C# | 5 | High |
| WP 4.2: Java binding + IDL4-Java | 5 | High |
| WP 4.3: Python binding | 3 | Medium |
| WP 4.4: DDS-RPC | 4 | Medium |
| WP 4.5: XRCE client + agent | 6 | High |
| WP 4.6: PlatformIO package + examples | 4 | High |
| WP 4.7: Zero-copy SHM path | 4 | High |
| WP 4.8: Chaos and performance test suite | 4 | High |
| WP 4.9: Documentation consolidation | 4 | High |

### Milestones

- **M4.1 (month 20):** all six bindings functional
- **M4.2 (month 21):** XRCE client runs on ESP32 and STM32, PlatformIO alpha
- **M4.3 (month 22):** **proof era complete** — the stack is production-capable and benchmark-competitive. A decision about the expansion era can be made informed.

## 8 Phase 5+: Expansion era (conditional) · from approx. month 22

The expansion era activates **after** confirmed proof and **as soon as** external funds can be mobilized. The activities are conditional and time-decoupled; each can start independently when resources and strategic conditions are right.

### 8.1 Expansion activity tracks

**Track A: OMG ecosystem integration**
- OMG membership (Contributing or higher, depending on budget)
- OMG vendor-ID application formal
- OMG Plug-Fest participation
- Cross-vendor interop certification
- Effort: 3–6 months calendar time, 1–2 FTE plus membership fees

**Track B: Ferrous Systems partnership and Ferrocene safe profile**
- Engineering partnership with Ferrous Systems
- Ferrocene integration in CI for safe-profile builds
- Target-port engineering for QNX/INTEGRITY (if customer demand)
- Effort: 6–9 months, 1–2 FTE internal plus external hours

**Track C: Safety certification**
- Formal requirements extraction and traceability-matrix consolidation
- MC/DC coverage push for the safe subset
- Safety manual
- External audit (TÜV Süd or alternative)
- Target standards: ISO 26262 ASIL D, IEC 61508 SIL 3, IEC 62304 Class C, DO-178C DAL B audit-ready
- Effort: 6–10 months, 2–3 FTE + external auditors
- Cost: mid six-figure range for audit fees

**Track D: Patent clearance and legal**
- Patent-attorney engagement for a freedom-to-operate analysis
- Trademark research and registration for "ZeroDDS"
- EU dual-use classification (BAFA)
- Cyber Resilience Act compliance assessment
- Effort: 2–4 months, 0.5 FTE internal plus an external firm

**Track E: Community and open-source release**
- Public GitHub release (if strategically desired)
- Governance model (if a donation to a foundation is pursued)
- Community-management role
- Conference presence (ROSCon, Embedded World, Eclipse Conference)
- Effort: continuous, 0.5–1 FTE

### 8.2 Recommended track prioritization

Order if funds are limited:
1. Track D (trademark protection for the ZeroDDS name) — lowest cost, highest protective value
2. Track A (OMG) — establishes external credibility
3. Track B (Ferrous) — enables Track C
4. Track C (safety) — opens up market segments
5. Track E (community) — in parallel to A/C depending on the release strategy

## 9 Post-v1.1 roadmap (outlook)

After the initial certification, the following further developments are planned:

- **Post-quantum crypto** as a security-plugin option (hybrid crypto suites)
- **DO-178C DAL A** full certification (if customer demand exists)
- **Additional RTOS ports:** Green Hills INTEGRITY-178, LynxOS-178 safety partitions
- **Additional language bindings:** Go, JavaScript/TypeScript via Deno, if community demand
- **WebDDS bridge** for web-UI integration
- **Web-based dashboard** as a complement to the Tauri desktop app
- **FDA support** for medical use cases (IEC 62304 Class C packages)

## 10 Resource ramp overview (bootstrap + proof)

| Month | FTE internal | Phase / era | Main activity |
|---|---|---|---|
| 1 | 2 | 0 — bootstrap | Architecture finalization |
| 2 | 2–3 | 0 — bootstrap | Spikes, skeleton |
| 3–4 | 3 | 1 — bootstrap | RTPS reliable start |
| 5–6 | 3 | 1 — bootstrap | Discovery, XTypes |
| 7–8 | 3 | 1 — bootstrap | QoS, interop |
| 9–10 | 3–4 | 2 — bootstrap | DCPS, security standard |
| 11–12 | 3–4 | 2 — bootstrap | C/C++ bindings, XML |
| 13 | 3–4 | 2 — bootstrap | **proof point: the internal application runs** |
| 14–15 | 3–4 | 3 — proof | Hardening, tooling |
| 16–17 | 4 | 3 — proof | Dashboard, benchmarks |
| 18–20 | 4–5 | 4 — proof | Further bindings, XRCE, PlatformIO |
| 21–22 | 4–5 | 4 — proof | Performance parity, docs, v1.0-rc |

**Cumulative internal staffing to the end of the proof era:** approx. 75–85 person-months = ~6–7 person-years. A Claude-Teams multiplier of ~4–6× corresponds to a classic effort of 25–40 person-years.

**The expansion era** is not included in this table, because ramp and timeframe depend on track decisions and external fund availability.

## 11 Early validation points

The stack is not validated only after 2+ years, but at clear checkpoints:

| Month | Validation |
|---|---|
| 4 | Technology choice (Rust) holds — go/no-go for the architecture |
| 8 | Interop with Cyclone and Fast DDS works — go/no-go for the MVP push |
| 13 | **Bootstrap proof:** the internal application runs on ZeroDDS |
| 17 | Benchmark gap to eProsima quantified — informs performance tuning |
| 22 | **Proof era complete:** v1.0-rc ready, decision about the expansion era |
| ~30 | Cert-ready (if Track B+C were activated) |

Every checkpoint can lead to a course correction, including a pause/re-scope/kill decision when fundamental assumptions break.
