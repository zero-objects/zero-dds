# Safety-by-Architecture

> **Status:** Draft v0.2
> **Dependencies:** `02_architecture.md`, `03_profiles_and_platforms.md`
> **Owner:** Safety Engineering Lead

This document is the binding contract for all safety-relevant architectural decisions. Every code change in safe-classified crates must satisfy this document. Violations are blocked by CI.

## 1 Philosophy

Safety-by-architecture means: the codebase is structured such that safety certification (ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+) is **possible** — without every change requiring continuous safety reviews. The architectural rules are anchored once and enforced automatically throughout. The safety audit at the end is then a documentation and validation step, not a refactoring step.

Three principles:

1. **Static enforcement before runtime checks.** Rule violations are detected at compile time or in CI, not at runtime.
2. **Separation of concerns between Safe and Comfort.** Safety-critical crates have different rules than comfort crates. The boundary is clear and physically manifested in the workspace.
3. **Traceability as a by-product, not as a follow-up.** Commits, tests and code annotations produce the artifacts an auditor needs during ongoing work — not retrospectively.

## 2 Safe-Subset contract

The following crates are classified as the **safe subset**:

```
zerodds-foundation
zerodds-cdr
zerodds-types
zerodds-qos
zerodds-rtps
zerodds-discovery
zerodds-transport (trait-only)
zerodds-transport-udp (without the tokio feature)
zerodds-security (core plugin API, without plugin implementations)
zerodds-xrce-client
zerodds-sys (stable C-ABI surface)
```

This safe subset is bundled in the meta crate **`crates/safe-crates-only`**;
`cargo build -p safe-crates-only --no-default-features --features safety` builds it
as the no_std safe profile (see the §3 gates). `zerodds-transport-shm` and
`zerodds-dcps` are **STANDARD**-classified (not in the safe subset), the
`zerodds-idl*` codegen tools are std-only — all three do not belong in the
no_std profile build.

These crates must comply with the following contract.

### 2.1 Language restrictions

| Rule | Enforcement |
|---|---|
| No `panic!()`, `unreachable!()`, `todo!()`, `unimplemented!()` outside `#[cfg(debug_assertions)]` or tests | `clippy::panic = "deny"`, `clippy::unreachable = "deny"`, `clippy::todo = "deny"`, `clippy::unimplemented = "deny"` |
| No `.unwrap()`, `.expect()` outside tests | `clippy::unwrap_used = "deny"`, `clippy::expect_used = "deny"` |
| No `.unwrap_or_default()` when the default is undesired | manual review |
| No `unsafe` code without a SAFETY comment | custom lint `dds_require_safety_comment` |
| No `dyn Trait` except in explicitly marked plugin boundaries | custom lint `dds_no_dyn_in_safe` |
| No `std` dependencies (only `core` + `alloc`) | `#![no_std]` + `extern crate alloc` |
| No `tokio` or other async runtimes (instead executor-agnostic via `core::future::Future` and `futures-core`) | dependency check in CI |
| No `HashMap` (uses randomness for DoS resistance, not deterministic) | clippy lint: `disallowed-types` |
| `Vec` only with bounded capacity; prefer `heapless::Vec` | review + custom lint |
| No recursion without a documented upper depth bound | review rule, no automatic lint |

### 2.2 Memory discipline

All safe crates adhere to strict memory rules:

- **No dynamic allocation in hot paths.** A "hot path" is defined as any code that runs in a sample processing path (receive, deserialize, deliver).
- **Static allocation first.** Internal data structures use `heapless::Vec`, `heapless::FnvIndexMap`, pre-allocated pools from `zerodds-foundation::pool`.
- **Bounded queues.** Every queue has an explicit upper bound. Overflow behavior is policy-controlled (drop, block, reject), never undefined.
- **No unbounded `Vec::push` on user input.** Length limits are checked before every growth.
- **Owned vs. borrowed clear.** Hot-path APIs accept `&[u8]` or `Bytes`, never `Vec<u8>`.

### 2.3 Error handling

- Every fallible call returns a `Result<T, E>`. Error enums are defined via `thiserror`, exhaustive, and stable (SemVer obligation).
- No `io::Error` in the public APIs of safe crates (dependency on `std::io`).
- Panics are only acceptable on invariant violations that are logically impossible — and even then they are converted to `Result<_, InvariantViolation>` where an error path exists.

### 2.4 Concurrency

- **No `std::sync::Mutex`** (uses a futex on Linux, not analyzable in a safety context).
- Instead: `spin::Mutex` (with documented wait-behavior guarantees) or OS-specific primitives qualified by the target RTOS.
- **No `std::thread::spawn`** in safe crates; concurrency is injected externally (executor-agnostic future API).
- **Atomic operations** are allowed, but every memory-ordering annotation (`Ordering::Acquire` etc.) must be commented and justified.

### 2.5 Generics and monomorphization

- Generics are preferred over `dyn Trait` in performance-critical paths to guarantee devirtualization.
- Explicit `Box<dyn Trait>` is allowed and required in plugin boundaries (security plugins, transport plugins), but must be in trait objects with a `'static` bound and marked as `#[allow(dds_no_dyn_in_safe)]`.
- Monomorphization explosion is avoided through facet traits and secondary object-safe traits for the API surface.

## 3 CI enforcement

The following CI pipeline runs on every PR and must be green for a merge. Safety-relevant jobs are not skippable.

### 3.1 Build jobs

| Job | Purpose | Blocking? |
|---|---|---|
| `cargo build --all --all-features` | Builds the full profile | ✓ |
| `cargo build -p safe-crates-only --no-default-features --features safety` | Builds the safe profile without std | ✓ |
| `cargo build --target aarch64-unknown-none -p zerodds-rtps -p zerodds-xrce-client` | Cross-compile for bare metal | ✓ |
| `cargo build --target thumbv7em-none-eabihf -p zerodds-xrce-client` | Micro profile Cortex-M7 | ✓ |
| Ferrocene build for safe crates | Qualified toolchain | ✓ |

### 3.2 Lint jobs

| Job | Content | Blocking? |
|---|---|---|
| `cargo clippy -- -D warnings` | Workspace-wide clippy lints | ✓ |
| `cargo clippy -p <safe-crate> --features safety -- -D clippy::unwrap_used -D clippy::panic -D clippy::unreachable` | Safe-specific lints | ✓ |
| Custom `zerodds-lint` (own clippy plugin) | Project-specific rules (see below) | ✓ |
| `cargo fmt -- --check` | Formatting | ✓ |
| `cargo deny check` | License + security audit | ✓ |

### 3.3 Test jobs

| Job | Content | Blocking? |
|---|---|---|
| `cargo test --workspace` | Unit + integration tests | ✓ |
| `cargo miri test -p zerodds-cdr -p zerodds-rtps` | Undefined-behavior detection | ✓ |
| `cargo kani -p zerodds-foundation -p zerodds-cdr -p zerodds-qos` | Model checking for formalizable properties | Nightly |
| `cargo fuzz run rtps_parser` | Fuzz testing for the wire parser | Nightly, at least 1h per run |
| OMG conformance test suite | Spec compliance tests | ✓ |
| Interop against CycloneDDS, Fast DDS | docker-compose harness | ✓ |
| Performance regression (Criterion) | No >5% regression in hot-path benchmarks | ✓ |

### 3.4 Custom lints (`zerodds-lint` crate)

`zerodds-lint` is a dedicated binary crate (`crates/lint`) that enforces the following
project rules **AST-based on stable Rust** — no
nightly toolchain, no dylint, no type info. Invocation in CI:
`cargo run -p zerodds-lint -- check` (see the GitLab CI job `zerodds-lint`).

As of WP 0.7 (Phase 0):

| Lint | Status | Exception marker |
|---|---|---|
| `dds_require_safety_comment` | implemented | `// SAFETY: <rationale>` directly before the unsafe block/fn/impl |
| `dds_no_dyn_in_safe` | implemented | file marker `zerodds-lint: allow no_dyn_in_safe` |
| `dds_safety_classification_present` | implemented | every crate with a `lib.rs` needs `Safety classification: **<CLASS>**` in the doc header |
| `dds_no_panic_in_safe` | implemented | tests/examples excluded, file marker `zerodds-lint: allow no_panic_in_safe` |
| `dds_no_alloc_in_hot_path` | implemented | enabled via the doc marker `/// zerodds-lint: hot-path` on a function or module |
| `dds_bounded_recursion` | implemented (Phase-0 approximation: intra-file, max. 1-hop indirect) | doc marker `/// zerodds-lint: recursion-depth N` |
| `dds_spec_annotated` | not active | the existing code needs migration; Phase 1 |

**Phase-0 limitations** (all intentional):

- No type info: `.unwrap()` is flagged regardless of the receiver type.
- Custom attributes (`#[dds_hot_path]`, `#[dds_recursion_depth(N)]`) are not
  syntactically allowed on stable Rust without `register_tool` and a proc macro
  — we replace them with doc-comment markers
  (`/// zerodds-lint: hot-path`, `/// zerodds-lint: recursion-depth N`) that are parsed as
  regular `#[doc = "..."]` attributes.
- Recursion detection is intra-file and at most 1-hop indirect; cycles
  longer than two functions or cross-file recursion (trait impls,
  mod splits) are not captured.
- Tests, examples and benches are excluded comprehensively from the lints.

A real clippy-plugin variant with type info (dylint or rustc-driver) is
planned for Phase 1, once the requirements crystallize from real
usage.

## 4 Traceability infrastructure

### 4.1 Commit convention

All commits follow Conventional Commits with an additional requirements tag:

```
<type>(<scope>): <description> [REQ-<id>]

<body>

<footer>
```

Examples:

```
feat(rtps): implement Heartbeat submessage [REQ-RTPS-0047]

Implements OMG DDSI-RTPS 2.5 §8.3.7.3 Heartbeat Submessage per spec.
Serializer validates first/last sequence number invariant.

Tests: tests/heartbeat_roundtrip.rs, tests/heartbeat_spec_vectors.rs
Covers: REQ-RTPS-0047, REQ-RTPS-0048, REQ-RTPS-0049
```

`REQ-<id>` references entries in the requirements tracker (Polarion, DOORS, or project-owned).

### 4.2 Code annotations

```rust
/// Implements DDSI-RTPS 2.5 §8.3.7.3 Heartbeat Submessage.
///
/// # Safety
/// The `first_sn` and `last_sn` fields must satisfy `first_sn <= last_sn + 1`
/// per spec. This invariant is checked on deserialization.
#[spec(rtps = "2.5", section = "8.3.7.3")]
#[satisfies(req = ["REQ-RTPS-0047", "REQ-RTPS-0048"])]
pub struct Heartbeat {
    pub reader_id: EntityId,
    pub writer_id: EntityId,
    pub first_sn: SequenceNumber,
    pub last_sn: SequenceNumber,
    pub count: Count,
}
```

The annotations are aggregated by the `zerodds-traceability` tool into a matrix:
- Requirements → code (which code implements which req)
- Code → tests (which tests cover which code)
- Requirements → tests (which tests verify which req)

### 4.3 Test annotations

```rust
/// Verifies DDSI-RTPS 2.5 §8.3.7.3.1 Heartbeat validity invariant.
#[test]
#[verifies(req = "REQ-RTPS-0047")]
#[spec_vector(source = "OMG DDSI-RTPS 2.5 Annex B, Vector 23")]
fn heartbeat_validates_sn_ordering() {
    // ...
}
```

## 5 Ferrocene integration (expansion era)

Ferrocene is the qualified Rust compiler required for formal safety certification of the safe subset. Ferrocene is TÜV-Süd-qualified per ISO 26262 ASIL D, IEC 61508 SIL 3, IEC 62304 Class C, and supports qualification efforts up to SIL 4 and DO-178C DAL C.

**Current plan status:** Ferrocene integration is an **expansion-era topic** (Track B in `06_roadmap.md` §8.1). In the bootstrap and proof eras the safe subset is built with stable Rust. The architectural discipline (no_panic, no_alloc-in-hot-path, structural separation) is in force from day 1 and enforceable by stable Rust — Ferrocene adds the formal qualification but does not change the code style.

So that the switch to Ferrocene is possible later without refactoring cost, the following rules already apply in the bootstrap era:

- Safe crates use only APIs contained in the Ferrocene Certified Core Subset (ISO 26262 ASIL B / IEC 61508 SIL 2), as far as known. The clippy `disallowed-methods` configuration is maintained accordingly.
- Toolchain pinning is prepared, just currently still on stable Rust (`rust-toolchain.toml`). The switch-over to the Ferrocene channel requires only a configuration change.
- Target triples are already chosen in CI so that they are compatible with the Ferrocene target portfolio.

### 5.1 Ferrocene release pinning (activated in Track B)

On the expansion-era switch a specific Ferrocene release is pinned:

```toml
[toolchain]
channel = "ferrocene-XX.YY.Z"
components = ["rust-src"]
targets = ["aarch64-unknown-nto-qnx710", "..."]
```

Release upgrades are drawn formally through a safety review.

### 5.2 Certified Core Subset (relevant from the expansion era)

The Ferrocene Certified Core Subset (currently ISO 26262 ASIL B, IEC 61508 SIL 2) is already used in safe crates as a design benchmark in the bootstrap era. Available APIs include `Option`, `Result`, `Clone`, `str`, pointer types, most primitives, `core::slice`, `core::iter`, `core::ffi`. Non-certified APIs are forbidden in safe crates via the `disallowed-methods` clippy configuration.

### 5.3 Target platforms for safe builds (expansion era)

Target platforms currently qualified by Ferrocene (as checked at Track-B start):
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu` (check current release coverage)
- `aarch64-unknown-none` (bare metal)
- `x86_64-pc-nto-qnx710`, `aarch64-unknown-nto-qnx710`
- `armv8r-none-eabihf` (Cortex-R)

Further targets are negotiated with Ferrous Systems as an engineering partnership depending on project need.

## 6 Audit path (expansion era)

The formal safety audit is an **expansion-era topic** (Track C in `06_roadmap.md` §8.1). The preparation for it, however, runs along already in the bootstrap and proof eras.

### 6.1 Bootstrap and proof era preparation

- Safety-by-architecture discipline is established from day 1 and enforced automatically (lints, CI).
- Traceability annotations and commit conventions are lived from day 1 (see §4).
- At the end of the proof era (Phase 4) the audit artifacts will fundamentally be producible — without anyone having actively packaged them as artifacts.

This saves considerable retrofit cost, if and when Track C is activated.

### 6.2 Expansion-era work packages

On Track-C activation the following steps are required:
- Add a safety engineer to the team (dedicated role)
- Requirements extraction and formal traceability matrix consolidation
- MC/DC coverage push (for DAL B+)
- Safety-case documentation
- External audit by TÜV Süd or an alternative

### 6.3 Required artifacts for an audit

The following list defines which artifacts must be present on Track-C activation. Most are already partly present through the architectural discipline; the safety engineer consolidates and supplements.

1. **Safety Plan** — document describing how safety is implemented in the project (this document + supplements).
2. **Requirements Specification** — formal requirements for the safe subset, each with a unique ID.
3. **Architecture Specification** — `02_architecture.md` + addendum for safe-subset internals.
4. **Module Specification** — a detailed module spec per crate in the safe subset.
5. **Test Specification** — which tests verify which requirements.
6. **Verification Report** — results of all tests, coverage reports (incl. MC/DC for DAL B+), static-analysis reports.
7. **Validation Report** — evidence that the product works correctly in target environments (interop tests, target-hardware tests).
8. **Safety Manual** — guidance for integrators on how the product is used correctly in a certified system.
9. **Change Management Log** — complete git history with `[REQ-...]` tags, aggregated as a change log.
10. **Tool Qualification Report** — Ferrocene qualification artifacts, linked from Ferrous Systems.
11. **SBOM** — CycloneDX Software Bill of Materials per release.
12. **Vulnerability Analysis** — `cargo-audit` reports, threat-modeling documents, CVE tracking.

### 6.4 Target standards

On Track-C activation these standards are targeted (in priority):

1. **ISO 26262 ASIL D** (automotive) — primary, for automotive SDV customers.
2. **IEC 61508 SIL 3** (industrial baseline) — basis for further industrial standards.
3. **DO-178C DAL B** initially, DAL A prospectively (avionics).
4. **IEC 62304 Class C** (medical) — secondary, depending on customer demand.
5. **EN 50128/50716** (railway) — secondary.

## 7 Violations protocol

When a commit violates the safety rules:

1. **CI blocks the merge.** Lint or test failures appear in the PR.
2. **Auto-fix where possible.** Claude-team agents can fix many violations automatically (e.g. replace `.unwrap()` with explicit error handling).
3. **Escalation on genuine conflicts.** When a safety rule must be broken out of technical necessity (e.g. a performance-critical `unsafe` block), a formal safety-waiver request is required. It requires:
   - A technical justification
   - An alternatives analysis (why other ways do not work)
   - A risk assessment
   - Compensating measures (e.g. additional tests, fuzz coverage)
   - Approval from the Safety Engineering Lead and a senior engineer from another team
4. **Documentation in the safety-waiver register.** All granted waivers are documented in the repo under `docs/safety-waivers/` and included in the audit.

## 8 Retrospective and review

This safety architecture is formally reviewed at least once per project phase. Changes require Safety Engineering Lead approval and are noted prominently in the release notes.
