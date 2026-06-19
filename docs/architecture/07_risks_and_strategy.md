# Risks and strategic considerations

> **Status:** Draft v0.2
> **Dependencies:** all preceding documents

This document identifies strategic and technical risks of the project and documents mitigation strategies. It is reviewed periodically, especially at phase transitions.

## 1 Technical risks

### 1.1 RTPS reliable-protocol bugs

**Risk:** The RTPS reliable protocol is one of the subtlest pieces in the DDS stack. Fast DDS and Cyclone DDS have found bugs in this area over years. Errors often only show up under network stress, at high fragmentation density, or in specific race conditions between heartbeats, acknacks and data flows.

**Impact:** Production bugs in reliable delivery lead to data loss, data duplication or deadlocks. That is unacceptable in mission-critical contexts.

**Mitigation:**
- Property-based testing and model checking (Kani) from day 1 for state machines
- Fuzz testing of the wire parser (at least nightly, 1h per run)
- Comprehensive interop tests against at least three independent peer implementations
- Chaos engineering with network disturbance (packet loss, reorder, delay, partition)
- Mandatory code review by at least two senior engineers for all reliable-protocol changes
- Reference implementation: Cyclone DDS as informal orientation, with documented deviations

### 1.2 XTypes interoperability

**Risk:** XTypes implementations have historically varied between vendors. Interop problems, especially with more complex type mutations (appendable, mutable), are common.

**Impact:** End nodes cannot find each other, even though types are "actually" compatible. That is a frequent support case across all DDS vendors.

**Mitigation:**
- Strict adherence to the XTypes 1.3 spec
- Interop tests per type-mutation pattern (all @final/@appendable/@mutable combinations)
- Documented deviations from other vendors (when we are spec-conformant and they are not)
- Type-lookup service as a fallback for unknown types

### 1.3 Performance parity

**Risk:** Established vendors have 10–20 years of performance tuning behind them. A new implementation could lose in benchmarks, which impairs sales arguments and customer acceptance.

**Impact:** Customers choose an established vendor despite sovereignty arguments, when the performance gap is too large.

**Mitigation:**
- Early benchmark baseline (from Phase 1) against Cyclone DDS and Fast DDS
- Performance-regression tests in CI with a 5% threshold
- Zero-copy SHM as a priority in Phase 4
- Profile-guided optimization (PGO) for release builds
- Flamegraph-based hot-path analysis continuously
- Rust gives us structural advantages (zero-cost abstractions, no GC pauses); that should lead to at least parity

### 1.4 Ferrocene target gap (deferred to the expansion era)

**Risk:** Ferrocene is currently qualified for a limited set of targets. When customers need exotic targets, Ferrocene's qualification scope might not suffice.

**Impact:** The safe profile does not work on customer hardware or only with additional effort.

**Current status:** In the bootstrap and proof eras we use stable Rust. Safe-subset crates are written safety-ready (see `04_safety_by_architecture.md`), but built without Ferrocene qualification. Ferrocene integration is a Track-B topic (see `06_roadmap.md` §8.1).

**Mitigation for later:**
- Early alignment with Ferrous Systems on target platforms at the start of Track B
- Contract engineering for target ports (Ferrous offers this as a service)
- Dual-path strategy: safe profile primarily on mainstream targets (Linux x86_64/ARM64, QNX Neutrino), exotic targets only on customer demand

### 1.5 Claude-Teams productivity disappointment

**Risk:** The planned multiplier of 4–8× is not reached, because certain tasks (RTPS debugging, interop issues, performance tuning) are not accelerated by LLM augmentation.

**Impact:** Roadmap slips, costs rise.

**Mitigation:**
- Clear buckets per task type with realistic multiplier expectations (see `06_roadmap.md`)
- Continuous metric: which tasks ran how fast with vs. without AI augmentation?
- No reliance on a 100× multiplier; the plan is calculated conservatively
- Fallback buffer planned in Phases 4 and 5

## 2 License and patent risks

### 2.1 RTI patent exposure

**Risk:** RTI holds various patents around DDS performance techniques (especially shared memory, FlatData, specific discovery optimizations). An own implementation can inadvertently touch patents.

**Impact:** Patent suit, license mandate, or feature withdrawal. Even the mere threat can deter investors and customers.

**Mitigation:**
- **Phase-0 patent clearance** by a specialized attorney with IP experience
- Systematically map RTI's patent portfolio
- Design-around for identified patents
- Freedom-to-operate opinion before a public release
- OMG spec features are royalty-free (OMG Essential Claims) — rely on those where possible
- Partnership option: prior-art defense fund (an Open Invention Network equivalent in the EU)

### 2.2 Export control

**Risk:** Our DDS will serve defense customers. The EU Dual-Use Regulation (EU 2021/821) can require export licenses for certain combinations of crypto + destination country.

**Impact:** Blocked exports, delayed deals, compliance overhead.

**Mitigation:**
- Phase-0 legal advice on EU dual-use classification
- Separate crypto-plugin model: core distribution without strong crypto, crypto separately licensable depending on destination
- Check: BAFA classification in Germany, analogous in other EU states
- Align the open-source release strategy with a view to US-EAR-like EU regulations

### 2.3 Open-source license choice

**Risk:** A wrong license choice either excludes commercial customers (e.g. GPL) or gives competitors free code without reciprocity.

**Impact:** Business-model erosion or a community-adoption gap.

**Recommendation:**
- **Dual license Apache 2.0 + MIT**: maximum adoption, EU-friendly, commercially frictionless
- **Alternative: EPL 2.0**: if we go to the Eclipse Foundation, which is the usual license there
- **Against GPL**: excludes embedded vendor integration
- **Against AGPL**: excludes cloud SaaS deployments
- Commercial support + services as the primary business model, not license sales

### 2.4 Trademark and naming

**Risk:** The DDS market already has RTI Connext, eProsima Fast DDS, Cyclone DDS — finding a new name that is unique and does not collide with existing trademarks requires research.

**Mitigation:**
- Phase-0 trademark research (EUIPO, USPTO screening)
- Check domain availability (`.eu`, `.org`, `.io`)
- Reserve the GitHub organization name
- Do not choose DDS names with a "Euro-" or "Sovereign-" prefix (politicized, restricts use cases)

## 3 Strategic and competitive risks

### 3.1 RTI reaction

**Risk:** RTI will react to an EU sovereignty competitor. Possible reactions range from acquisition offers through aggressive price undercutting to legal steps.

**Impact:** Market access made harder, investors nervous, team poaching.

**Mitigation:**
- **Financial runway:** at least 18 months cash runway independent of early revenue
- **IP protection:** own patent applications for innovations (observability features, XRCE extensions)
- **Community moat:** a strong open-source community as a defensive line against proprietarization
- **Sovereign-customer anchor:** early commitments from EU defense or aerospace customers as strategic backing
- **Team retention:** vesting structures, long-term incentives; key engineers not replaceable

### 3.2 eProsima positioning

**Risk:** eProsima is EU-based (Madrid), has Safe DDS with ASIL D, and already serves many of our target customers. They are the most natural competitor in our segment.

**Impact:** Customers who already use eProsima have high switching costs.

**Mitigation:**
- Clear differentiation: we offer **more comprehensive tooling, Rust-based safety, PlatformIO integration** — things eProsima does not have
- Migration tools and a co-existence mode (our stack and eProsima in the same network)
- Position not as "replace eProsima" but as the "next generation of DDS" — less confrontational
- Possibly cooperation talks with eProsima (shared interop test bed, OMG Plug-Fest coordination)

### 3.3 Market timing

**Risk:** In 22–27 months the market may have shifted. Software-defined-vehicle trend, post-quantum-crypto mandate, new standards — timing risk is real.

**Mitigation:**
- Phased release (beta from month 14, v1.0 from month 22, cert from month 27) with market-feedback integration
- Flexible feature scope: identify critical vs. nice-to-have, defer if needed
- Continuous market intelligence (customer interviews, conference attendance, competitor watch)

### 3.4 Community building

**Risk:** An open-source project without a community is just code. Without external contributors, user feedback and external reviews the project will stay isolated.

**Mitigation:**
- Public GitHub from Phase 1 (even if not yet production-ready)
- Transparent roadmap and design documents (this set)
- Conference presence: ROSCon, Embedded World, Eclipse Conference, OMG meetings
- A governance model that welcomes external contributors (SIG structure like Kubernetes)
- An appointed community manager from Phase 3
- Partnerships with universities (research projects, student contributions)

### 3.5 Governance (decided)

**Decision:** ZeroDDS is an internal core project of the sponsoring company. No external governance framework, no foundation donation planned. License is **Apache 2.0**. This choice preserves optionality — a later switch to a more open governance model remains possible without re-licensing friction, if strategic reasons arise. Conversely, no planning, staffing or attention is invested now.

Community-related questions (open-source release, contributor license agreement, donation target) are expansion-era topics and are handled in `06_roadmap.md` §8.1 Track E, if they become relevant.

## 4 Team and organization risks

### 4.1 Senior-talent acquisition

**Risk:** DDS experts are rare. RTPS expertise rarer still. In a competitive market (especially in Germany), recruiting is challenging.

**Mitigation:**
- Competitive compensation including equity participation
- Remote-first option in the EU time zone, not restricted to a location cluster
- Conference presence and technical publications to build the employer brand
- University partnerships for a junior-talent pipeline
- No requirement for ex-DDS-vendor experience; Rust + distributed-systems experts can learn DDS

### 4.2 Bus factor

**Risk:** Few people with deep protocol understanding. The loss of individual key engineers causes project risk.

**Mitigation:**
- At least two senior engineers per core subsystem (Protocol, Platform, Quality)
- Documentation obligation: every non-trivial decision in the code as an ADR (Architecture Decision Record)
- Code-review culture: every senior reviews code from all others regularly, avoids silos

### 4.3 Claude-Teams dependency

**Risk:** When the entire development model is based on Claude augmentation and Claude availability/pricing/capabilities change, platform lock-in arises.

**Mitigation:**
- Prompts and workflows versioned in git, reproducible
- Artifacts (generated code) are portable to other LLM providers
- No vendor lock-in on Anthropic-specific features beyond standard LLM capabilities
- Continuity plan: what if Claude is gone tomorrow? The team must be able to continue without it, slower but functional.

## 5 Compliance and regulation risks

### 5.1 Safety-audit failure (expansion era, Track C)

**Risk:** If Track C is activated, the formal audit could find deficiencies that require larger refactorings.

**Impact:** Certification is delayed, the product is not usable for the safety segment.

**Mitigation:**
- Safety-by-architecture discipline from day 1 in the bootstrap era (see `04_safety_by_architecture.md`) reduces retrofit risk considerably
- Audit artifacts are built up continuously, not only at Track-C activation
- A safety engineer becomes part of the team with the Track-C start; internal audit simulation before the external audit
- Buffer planned in the Track-C schedule (6–10 months calendar time incl. remediation)

### 5.2 Post-quantum-crypto mandate

**Risk:** EU regulatory requirements or customer requirements could force PQC support before the Phase-3 release.

**Mitigation:**
- The plugin architecture of the `zerodds-security` crate is PQC-ready
- Monitoring of NIST PQC standardization and EU regulation
- Early-implementation option: Kyber/Dilithium prototype in Phase 2/3 as a feature preview
- Hybrid crypto suites (classical + PQC) as default at release

### 5.3 CRA (Cyber Resilience Act)

**Risk:** The EU Cyber Resilience Act (2024) imposes requirements on manufacturers of software with digital elements, including security updates and vulnerability disclosure.

**Mitigation:**
- SBOM production per release (CycloneDX)
- Vulnerability-disclosure policy documented and implemented (security.md, GPG key)
- CVE process: a PSIRT-like function in the team
- Continuous `cargo audit` in CI, auto-patch for critical CVEs

## 6 Risk register (summary)

| ID | Risk | Likelihood | Impact | Era | Trend |
|---|---|---|---|---|---|
| T-01 | RTPS reliable bug | High | High | Bootstrap | stable |
| T-02 | XTypes interop issues | Medium | Medium | Bootstrap | stable |
| T-03 | Performance gap | Medium | Medium | Proof | falling with Rust maturity |
| T-04 | Ferrocene target gap | Low | Medium | Expansion | falling |
| T-05 | Claude multiplier disappointment | Medium | Medium | Bootstrap | falling with experience |
| L-01 | RTI patent exposure | Medium | High | Expansion | stable |
| L-02 | Export control | Low | Medium | Expansion | rising with geopolitics |
| L-03 | Wrong license choice | — | — | decided | Apache 2.0 chosen |
| S-01 | RTI reaction | Low → High | Medium | depends on visibility | rising with traction |
| S-02 | eProsima competition | High | Medium | all | stable |
| S-03 | Market timing | Medium | Medium | all | stable |
| S-04 | Community building | — | Low | Expansion Track E | conditional |
| S-05 | Governance-home decision | — | — | decided | internal core chosen |
| O-01 | Senior-talent acquisition | Medium | High | all | stable |
| O-02 | Bus factor (elevated at 2–4 FTE) | High | High | Bootstrap | falling in the proof era |
| O-03 | Claude dependency | Medium | Medium | Bootstrap | stable |
| O-04 | Scope creep from a small team | High | High | Bootstrap | controllable with discipline |
| O-05 | The internal use case shifts | Medium | High | Bootstrap | project-specific |
| C-01 | Safety-audit failure | Low | High | Expansion Track C | falling with architecture discipline |
| C-02 | PQC mandate | Low | Medium | all | rising |
| C-03 | CRA compliance | Medium | Medium | Proof → Expansion | rising |

The register is reviewed and updated monthly in the lead round.

**New in this version:**
- O-04 (scope creep): critical with a small team, phase-gate discipline essential
- O-05 (use-case shift): we are our own first customer; if the internal use case changes, the validation target shifts

## 7 Strategic decisions: status

### 7.1 For the bootstrap era (decided)

- **Project name:** ZeroDDS (trademark clearance in the expansion era, see §2.4)
- **License:** Apache 2.0
- **Governance:** internal core project, no foundation
- **Team model:** 2–4 internal engineers + Claude-Teams augmentation
- **First customer:** ourselves (the internal target application as a validation anchor)
- **Crypto default:** standard suite from DDS-Security 1.2 (AES-GCM, RSA/ECDSA) for interop day one
- **Safety path:** safety-by-architecture from day 1, but Ferrocene integration and the formal audit deferred to the expansion era

### 7.2 For the expansion era (deferred)

The following decisions are made only once the bootstrap proof is reached and external funds are mobilized:

1. **Ferrous-Systems partnership contract** — scope and conditions (Track B)
2. **OMG membership tier** (Track A)
3. **Patent-attorney engagement** for freedom-to-operate and trademark (Track D)
4. **EU crypto-plugin strategy** — which suites as a second choice after the standard suite
5. **Open-source release strategy** (Track E) — if a community build is desired
6. **HSM integration via PKCS#11** — as a commercial feature or default?
7. **Governance evolution** — stay an internal core project or donate to a foundation?

This deferral is deliberate. Funds for external engagements are allocated more rationally when a working stack exists as a negotiation basis.
