# ZeroDDS — architecture documentation

This document suite forms the architectural foundation for the development of **ZeroDDS** — a sovereign, complete DDS implementation in Rust with bindings for C, C++, C#, Java, Python and Rust.

The name ZeroDDS reflects the core promise: _zero dependencies_ (safe core), _zero panic_ (contract), _zero unsafe_ (where structurally possible), _zero copy_ (SHM path), _zero vendor lock-in_.

## Execution model

ZeroDDS is developed as an **internal core project** (Apache 2.0-licensed, optionality for later opening preserved). We are our own first customer: the stack is validated against a concrete internal application before external partnerships, OMG membership, patent clearance or safety certification are activated.

**Bootstrap era** (Phases 0–2, ~10–14 months): internal MVP.
**Proof era** (Phases 3–4, ~6–10 months): benchmark parity with eProsima, external readiness.
**Expansion era** (Phase 5+, conditional): OMG, Ferrous Systems, safety audit, community.

## Reading order by role

### For leadership and stakeholders
1. `00_overview.md` — executive summary, mission, success criteria

### For tech leads and architects
1. `00_overview.md`
2. `01_scope_and_specs.md` — OMG spec coverage
3. `02_architecture.md` — system and crate architecture
4. `03_profiles_and_platforms.md` — deployment profiles
5. `07_risks_and_strategy.md` — strategic risks

### For safety engineers
1. `00_overview.md`
2. `02_architecture.md`
3. `04_safety_by_architecture.md` — safe-subset contract (primary document)
4. `03_profiles_and_platforms.md` — safe-profile details
5. `06_roadmap.md` §8 — Phase-5 audit path

### For product engineers (per sub-system)
1. `02_architecture.md` — identify your own crate responsibility
2. `01_scope_and_specs.md` — which OMG specs concern you
3. `04_safety_by_architecture.md` — coding rules (if a safe crate)
4. `05_observability_and_tooling.md` — instrumentation expectations

### For platform/DevOps
1. `03_profiles_and_platforms.md` — build matrix
2. `05_observability_and_tooling.md` — monitoring design
3. `06_roadmap.md` — release plan

## Document overview

| File | Scope | Review interval |
|---|---|---|
| `00_overview.md` | Mission and vision | Quarterly |
| `01_scope_and_specs.md` | OMG spec coverage and conformance | Per release |
| `02_architecture.md` | System and crate architecture | Quarterly |
| `03_profiles_and_platforms.md` | Profiles and platform matrix | Per release |
| `04_safety_by_architecture.md` | Safety contract | Per phase or on rule changes |
| `05_observability_and_tooling.md` | Observability and tooling | Quarterly |
| `06_roadmap.md` | Phases and milestones | Monthly |
| `07_risks_and_strategy.md` | Risks and strategic options | Monthly |
| `08_heterogeneous_security.md` | Per-peer/per-interface security for systems-of-systems (WP 4.9) | Per release |
| `09_delegation.md` | Gateway/bridge delegation for vehicle mesh + edge peers without their own cert (WP 4H-j) | Per release |

## Change process

All architecture documents live in the main repository under `docs/architecture/`. Changes are made via pull request and require:
- At least one review by a tech lead or safety engineer (for `04_safety_by_architecture.md`)
- A rationale in the PR description
- An update of the relevant `Status:` field and, if applicable, a version increment

## Related resources (outside this suite)

- Standards registry under `docs/standards/` for all external specs (OMG, W3C, IETF, CNCF, OASIS, ISO/IEC) on which ZeroDDS builds
- Requirements tracker (Polarion, DOORS, or GitHub Issues with `REQ-...` labels)
- ADR directory under `docs/adr/` for Architecture Decision Records
- Safety-waiver register under `docs/safety-waivers/`
- Design RFCs under `docs/rfcs/` for larger technical proposals
- Security advisories under `docs/security/advisories/`

## Conventions

- **Language:** English for the architecture suite and all public-facing material (GitHub README, spec files, public API docs).
- **Format:** Markdown, CommonMark + GitHub-Flavored Markdown features.
- **Cross-references:** relative (`02_architecture.md §3.1`) not absolute.
- **Terminology:** established English technical terms are kept as-is (crate, workspace, feature flag, discovery).
- **Code examples:** Rust as the main language, with syntax highlighting.

## Status

This document set is in the **Draft v0.2** state (updated for the ZeroDDS name and the bootstrap-before-expansion strategy). It is formally reviewed, adjusted and released in Phase 0 of the project.
