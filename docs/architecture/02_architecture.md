# System architecture and crate workspace

> **Status:** Draft v0.2
> **Dependencies:** `00_overview.md`, `01_scope_and_specs.md`

## 1 Architecture principles

The following principles are binding for all architecture decisions. Conflicts are resolved in this order:

1. **Correctness before performance.** RTPS interop and QoS semantics must not be sacrificed for microseconds.
2. **Safety qualifiability before convenience.** Core modules are written so that they remain safety-qualifiable, even when the current build variant does not require it.
3. **Spec conformance before feature innovation.** OMG deviations only with documented rationale and interop-test evidence.
4. **Modular separation before monoliths.** Every crate has a clear responsibility and dependencies. No circular dependencies.
5. **Feature flags before forks.** Differences between profiles are realized through feature gates, not through separate code trees.
6. **Generics before dynamic dispatch.** In safe-qualifiable crates, `dyn Trait` is only allowed with explicit justification. Devirtualization should always be possible for the compiler.
7. **No-panic contract.** In all crates above `dds-tools` and `zerodds-dashboard`, `.unwrap()`, `.expect()`, `panic!()`, `unreachable!()` (outside `#[cfg(debug_assertions)]`) are forbidden by a CI lint.

## 2 Layered architecture

The system is organized into five layers. Dependencies flow strictly top to bottom; cross-dependencies within a layer are allowed, upward dependencies forbidden.

```
┌─────────────────────────────────────────────────────────────┐
│  Public APIs: zerodds-rs, zerodds-sys, zerodds-cpp, zerodds-cs, zerodds-java,   │
│               zerodds-py                                         │
├─────────────────────────────────────────────────────────────┤
│  Core Services: zerodds-dcps, zerodds-rpc, zerodds-security, zerodds-xml,   │
│                 zerodds-recorder, zerodds-monitor, dds-tools        │
├─────────────────────────────────────────────────────────────┤
│  Protocol: zerodds-rtps, zerodds-discovery, zerodds-types, zerodds-cdr,     │
│            zerodds-qos                                           │
├─────────────────────────────────────────────────────────────┤
│  Transport: zerodds-transport (trait), zerodds-transport-udp,       │
│             zerodds-transport-tcp, zerodds-transport-shm            │
├─────────────────────────────────────────────────────────────┤
│  Foundation: zerodds-foundation                                  │
└─────────────────────────────────────────────────────────────┘
                     ↑
                     │
          Ferrocene qualified toolchain
          + certified core subset (Safe profile)
```

## 3 Crate catalog

The workspace comprises the following crates. Each row marks the safety classification, whether it is no_std-capable, and the primary responsibility.

### 3.1 Foundation layer

| Crate | Safety class | no_std | Responsibility |
|---|---|---|---|
| `zerodds-foundation` | Safe | Yes | Core types (InstanceHandle, Time, Duration, SequenceNumber, GUID), error-enum family, Result aliases |

### 3.2 Transport layer

| Crate | Safety class | no_std | Responsibility |
|---|---|---|---|
| `zerodds-transport` | Safe | Yes | Transport trait (`Transport`, `Listener`, `Locator`), abstract send/receive |
| `zerodds-transport-udp` | Safe | Optional | UDP/IP PSM, raw socket, multicast |
| `zerodds-transport-tcp` | Standard | No | DDSI TCP/IP PSM, connection pool |
| `zerodds-transport-shm` | Safe | Optional | Shared-memory segment management, zero-copy path |

### 3.3 Protocol layer

| Crate | Safety class | no_std | Responsibility |
|---|---|---|---|
| `zerodds-cdr` | Safe | Yes | XCDR1/XCDR2 encoder/decoder, endianness, alignment |
| `zerodds-types` | Safe | Yes | XTypes type system, TypeObject, TypeIdentifier, compatibility |
| `zerodds-qos` | Safe | Yes | QoS policies, request/offered compatibility matrix, typestate compatibility |
| `zerodds-idl` | Safe | No (std-only) | IDL4 parser, AST, semantic model (OMG IDL 4.2). Grammar-driven (Earley engine), build-time tool — no embedded use case. Consumed by `zerodds-idlc`. See `docs/rfcs/0001-idl-parser-architecture.md` |
| `zerodds-rtps` | Safe | Yes | Writer/reader state machines, Heartbeat/Acknack/Gap/Data submessages, fragmentation |
| `zerodds-discovery` | Safe | Yes | SPDP, SEDP, TypeLookup service |

### 3.4 Core services layer

| Crate | Safety class | no_std | Responsibility |
|---|---|---|---|
| `zerodds-dcps` | Standard | No | DomainParticipant, Publisher, Subscriber, Topic, DataReader, DataWriter |
| `zerodds-rpc` | Standard | No | Request/reply framework, service-definition runtime |
| `zerodds-security` | Safe (core) / Standard (plugins) | Partial | Authentication/AccessControl/Cryptographic plugin trait + default implementations |
| `zerodds-xml` | Standard | No | DDS-XML parser, QoS-profile loader, schema validator |
| `zerodds-xrce-client` | Safe | Yes (no alloc) | XRCE client for the micro profile, transport-agnostic |
| `zerodds-xrce-agent` | Standard | No | XRCE agent, runs in the full/standard profile |
| `zerodds-recorder` | Comfort | No | Deterministic record/replay service |
| `zerodds-monitor` | Comfort | No | OpenTelemetry instrumentation, Prometheus exporter, wire probe |
| `dds-tools` | Comfort | No | Admin CLI, config validator |

### 3.5 Binding/API layer

| Crate | Safety class | no_std | Responsibility |
|---|---|---|---|
| `zerodds-rs` | Standard | No | Idiomatic Rust SDK, async/await, streams |
| `zerodds-sys` | Safe (core) / Binding (FFI module) | Yes (core) | Stable C-ABI, basis for all non-Rust bindings. The `lib.rs` core is Safe/no_std; the C-ABI exports live isolated in `mod ffi` (see §4.4.3/§4.4.4) |
| `zerodds-cpp` | Standard | No | C++ wrapper, IDL4-C++ runtime |
| `zerodds-cs` | Standard | No | C# P/Invoke, NativeAOT-compatible, IDL4-C# runtime |
| `zerodds-java-omgdds` | Standard | No | Pure-Java DDS-Java-PSM (`org.omg.dds.*`) + IDL4-Java runtime; no JNI, no native lib on the Java side |
| `zerodds-py` | Comfort | No | PyO3 bindings, pandas/numpy-friendly |

### 3.6 Tooling (binary crates)

| Crate | Type | Responsibility |
|---|---|---|
| `zerodds-idlc` | bin | IDL4 compiler, backends: C, C++, C#, Java, Python, Rust. Uses `zerodds-idl` for parser/AST |
| `zerodds-admin` | bin | Admin CLI: domain inspector, QoS validator, discovery snapshot |
| `zerodds-xmlc` | bin | DDS-XML validator, schema checker, deployment renderer |
| `zerodds-dashboard` | bin | Tauri app for live monitoring, discovery graph, replay browser |
| `zerodds-perf` | bin | Load generator, latency profiler, benchmark suite |
| `zerodds-traceability` | bin | Requirements-to-code matrix generator |

### 3.7 Meta-tooling (lint plugin)

| Crate | Type | Responsibility |
|---|---|---|
| `zerodds-lint` | lib | Custom clippy lints (project rules per `04_safety_by_architecture.md §3.4`). No runtime code, not safety-classified. Loaded by CI as a clippy plugin |

## 4 Dependency rules

### 4.1 Allowed dependency directions

- Every layer may depend on layers **below** it.
- Within a layer, crates may depend on other crates of the same layer, as long as no cycles arise.
- `zerodds-sys` may only be used directly by `zerodds-rs` re-export crates and `zerodds-dcps`, to keep the C-ABI surface clean.

### 4.2 Forbidden patterns

- Binding crates (`zerodds-cpp`, `zerodds-cs`, `zerodds-java`, `zerodds-py`) may **not** access protocol or transport crates directly. Only via `zerodds-sys` or `zerodds-rs`.
- Safety crates may have **no** dependencies on standard or comfort crates.
- No crate may have `tokio` directly as a mandatory dep; instead executor-agnostic via `futures::Stream` traits. Tokio is only linked in comfort and optional standard builds.

### 4.3 Third-party dependency policy

- **Safe crates:** whitelist-based. Allowed crates: `heapless`, `bytes` (safe subset), `zerocopy`, `byteorder`. Every new dep requires explicit justification and a security review.
- **Standard crates:** curated list. Allowed: `serde`, `tokio` (optional feature), `tracing`, `thiserror`, `hex`, `sha2`, `ring` or `rustls` depending on the security plugin. New deps via pull-request review.
- **Comfort crates:** more open, but every dep is run through `cargo-audit`, `cargo-deny` and a license check in CI.

**`deny.toml` conventions** (source: project root; tweaks justified with inline comments):

- `[licenses] allow = [...]` is a **stock allowlist** (Apache-2.0, MIT, BSD-2/3, ISC, Unicode-3.0, Unicode-DFS-2016, CC0-1.0, Zlib, Apache-2.0-WITH-LLVM-exception). Entries remain even if no crate currently uses them; `unused-allowed-license = "allow"` suppresses the otherwise triggered warnings. GPL/AGPL are implicitly forbidden — see `07_risks_and_strategy.md` §2.3.
- `[bans] wildcards = "deny"` stays active, but `allow-wildcard-paths = true` exempts **workspace-internal `path = "../..."` deps**. This is the usual Cargo practice for unpublished sub-crates: they have no `version = "..."` and would otherwise be rejected as a wildcard. Registry wildcards (`foo = "*"`) stay blocked.
- `[advisories] yanked = "deny"`, `[sources] unknown-registry = "deny"`, `unknown-git = "deny"` stay hard — no software from unverified sources, no yanked crates.

### 4.4 Unsafe-code policy

Each safety class sets its own crate-wide default, enforced by the
corresponding inner attribute in `src/lib.rs`. Exceptions
are only permissible in clearly named, isolated modules — typically
`mod ffi;` — and there require a local lint override **plus**
a SAFETY comment per `unsafe` block.

#### 4.4.1 Crate-wide defaults by safety class

| Safety class | `lib.rs` default | Exceptions allowed? |
|---|---|---|
| **Safe** | `#![forbid(unsafe_code)]` | No at crate level. Only via structurally separated FFI/plugin modules (see §4.4.3). |
| **Standard** | `#![deny(unsafe_code)]` | Yes, in `#[allow(unsafe_code)]`-marked modules with a SAFETY-comment obligation. |
| **Comfort** | `#![warn(unsafe_code)]` | Yes, every `unsafe` block needs a SAFETY comment, CI lint `dds_require_safety_comment` (see `04_safety_by_architecture.md §3.4`). |

#### 4.4.2 SAFETY-comment convention

Every `unsafe` block, `unsafe fn` declaration or `unsafe impl` requires
an immediately preceding `// SAFETY:` comment with at least
one sentence that justifies the invariants. This rule is enforced by
`dds_require_safety_comment` (custom lint, `crates/lint`).

#### 4.4.3 FFI-module pattern

Crates with a C-ABI surface or a language binding (`zerodds-sys`, `zerodds-cpp`,
`zerodds-cs`, `zerodds-java`, `zerodds-py`) separate the **safe core** from the **FFI surface**
physically:

- `src/lib.rs` keeps the default corresponding to the safety class
  (`forbid` for `zerodds-sys`, `deny` for standard bindings, `warn` for
  comfort bindings). Only safely analyzable Rust code lives in `lib.rs`
  (types, enum constants, helpers).
- `src/ffi.rs` (or `src/ffi/` for larger surfaces) carries
  `#![allow(unsafe_code)]` at the module level and exports the actual
  `extern "C"` functions, `#[no_mangle]` symbols, PyO3 modules or
  P/Invoke stubs. Java needs no FFI layer: ZeroDDS' Java PSM
  (`zerodds-java-omgdds`) is pure Java.
- Within the FFI module the SAFETY-comment convention (§4.4.2)
  remains binding. Additionally, in safe crates and the safe core of
  `zerodds-sys`: the call path from the safe core into the FFI module must
  be upward-free (FFI may use the safe core, the safe core does not call into
  the FFI module).

#### 4.4.4 Special case `zerodds-sys`

`zerodds-sys` carries the classification **Safe (core)** despite a built-in
C-ABI. Resolution: the `lib.rs` core (types, opaque handles, error codes)
is fully Safe (`#![forbid(unsafe_code)]`, `#![no_std]`-capable).
The C-ABI exports live in a separate `ffi` module with
`#![allow(unsafe_code)]` and count as the **binding surface** — not
as part of the certifiable core. Safety audits of the `zerodds-sys` core
thus cover the `lib.rs` part; the `ffi` module is treated like other
binding crates.

## 5 Workspace organization

The root `Cargo.toml` is a virtual workspace:

```toml
[workspace]
resolver = "2"
members = [
    "crates/foundation",
    "crates/cdr",
    "crates/types",
    "crates/qos",
    "crates/idl",
    "crates/transport",
    "crates/transport-udp",
    "crates/transport-tcp",
    "crates/transport-shm",
    "crates/rtps",
    "crates/discovery",
    "crates/security",
    "crates/dcps",
    "crates/rpc",
    "crates/xml",
    "crates/xrce-client",
    "crates/xrce-agent",
    "crates/recorder",
    "crates/monitor",
    "crates/rs",
    "crates/sys",
    "crates/cpp",
    "crates/cs",
    "crates/java",
    "crates/py",
    "crates/lint",
    "tools/idlc",
    "tools/admin",
    "tools/xmlc",
    "tools/dashboard",
    "tools/perf",
    "tools/traceability",
]

[workspace.package]
rust-version = "1.85"
edition = "2024"
license = "Apache-2.0"
repository = "https://…"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
```

Crates have a consistent `Cargo.toml` structure with shared package metadata via `workspace = true`.

## 6 Feature-flag baseline regime

Global feature flags are used consistently at the workspace level:

| Flag | Meaning | Gated crates |
|---|---|---|
| `std` | Use of `std` allowed | All except the safe core |
| `alloc` | Use of `alloc` allowed | Like `std`, but stricter |
| `safety` | Enables all no-panic/no-alloc rules | Safe crates |
| `security` | DDS-Security enabled | `zerodds-dcps`, `zerodds-rtps` |
| `xtypes` | XTypes support | `zerodds-types`, `zerodds-rtps` |
| `tcp` | TCP transport enabled | `zerodds-transport-tcp` |
| `shm` | Shared-memory transport | `zerodds-transport-shm` |
| `async-tokio` | Tokio runtime | Standard builds |
| `async-embassy` | Embassy runtime | Embedded |
| `otel` | OpenTelemetry emission | `zerodds-monitor` |
| `recording` | Wire recorder | `zerodds-monitor` |

Details of the profile-to-feature mapping in `03_profiles_and_platforms.md`.

## 7 API stability tiers

Not all public APIs have the same stability guarantees. Three tiers are defined:

| Tier | Crates | SemVer policy |
|---|---|---|
| **Tier 1: stable binding APIs** | `zerodds-sys`, `zerodds-cpp`, `zerodds-cs`, `zerodds-java`, `zerodds-rs` | Strict SemVer. Breaking changes only in major releases. |
| **Tier 2: core runtime APIs** | `zerodds-dcps`, `zerodds-security`, `zerodds-rpc` | SemVer, but breaking changes in minor releases allowed before 1.0. |
| **Tier 3: internal APIs** | All other protocol, transport, foundation crates | Internal changes possible at any time. Users who access these crates directly assume the maintenance risk. |

## 8 Test architecture

Every crate has three test levels:

1. **Unit tests in `src/`:** private implementation tests, `#[cfg(test)]` modules.
2. **Integration tests in `tests/`:** public-API tests, also compliance tests against OMG spec vectors.
3. **Workspace level `xtests/`:** cross-crate integration, interop tests against real DDS peers (CycloneDDS, Fast DDS, RTI), end-to-end scenarios.

Test categories:

| Category | Tool | When |
|---|---|---|
| Unit | `cargo test` | On every commit, CI |
| Integration | `cargo test --test ...` | CI |
| Property-based | `proptest`, `quickcheck` | CI, especially for CDR and RTPS |
| Fuzz | `cargo-fuzz`, `AFL` | Nightly CI, especially for the wire parser |
| Model checking | `kani` | For safe crates, nightly CI |
| Interop | own harness with docker-compose | PR + nightly |
| Performance regression | Criterion.rs + custom harness | Nightly, alerts on >5% regression |

## 9 Claude-Teams collaboration model

The codebase is explicitly structured so that agentic development scales:

- **Crate-level agents:** a dedicated Claude agent can work per crate without conflicts with other crates. Crate-internal API changes stay local.
- **Spec sections as work packages:** OMG spec sections map to code modules with `#[spec(...)]` annotations. An agent takes a section, implements, tests against spec vectors.
- **Test-first workflow:** the agent reads an OMG spec chapter, generates conformance tests (property-based + example-based), implements against green tests.
- **Review layer:** human senior engineers review architectural decisions, protocol state machines and safety-critical changes. Routine implementations are reviewed agent-to-agent.
- **Documentation sync:** Claude Teams keep these architecture documents in sync with the code. On every relevant commit it is checked automatically whether documentation needs updating.
