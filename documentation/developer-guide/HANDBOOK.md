# ZeroDDS Developer Handbook

For contributors working **on** ZeroDDS itself: extending crates,
fixing bugs, shipping new bindings, advancing the spec coverage.

---

## 1. Codebase tour

ZeroDDS is a Cargo workspace organised into nine layers, each
of which depends only on the layers below.

| Layer | Purpose | Crates (selection) |
|---|---|---|
| 0 — substrate | Allocators, time, IDs, error model | `zerodds-time`, `zerodds-id`, `zerodds-error`, `flatdata` |
| 1 — wire codec | XCDR1 / XCDR2 (DDS-XTypes 1.3) | `cdr`, `xcdr2` |
| 2 — RTPS protocol | Submessages, fragmentation, reliable state machines | `rtps`, `rtps-test`, `discovery` |
| 3 — schema / IDL | OMG-IDL 4.2 parser, type system, codegen | `idl`, `idl-cpp`, `idl-java`, `idl-python`, `idl-ts`, `idl-csharp` |
| 4 — DCPS | Public DDS 1.4 API | `dcps`, `dcps-async` |
| 5 — security | DDS-Security 1.2 plugins | `security-pki`, `security-crypto`, `security-acl` |
| 6 — bridges | Non-DDS-protocol bridges | `mqtt-bridge`, `amqp-bridge`, `coap-bridge`, `ws-bridge`, `grpc-bridge` |
| 7 — bindings | Language ABIs + native APIs | `zerodds-c-api`, `zerodds-cpp`, `zerodds-py`, `zerodds-ts`, `zerodds-java`, `zerodds-csharp` |
| 8 — tooling / runtime | Daemons, recorder, dashboard, RMW shim | `zerodds-monitor`, `zerodds-recorder`, `rmw-zerodds`, `zerodds-dashboard` |

Per-crate purpose and dependency graph: see
`02-architecture/components.md`.

---

## 2. Cargo workspace + MSRV

Workspace root: `Cargo.toml`. Every crate inherits version,
edition, and dependency versions from
`[workspace.package]` / `[workspace.dependencies]`.

### MSRV

`rust-toolchain.toml` pins the minimum supported Rust:

```toml
[toolchain]
channel = "1.85"
components = ["rustfmt", "clippy"]
```

CI verifies on MSRV and on stable. Bumping MSRV requires:

1. RFC issue with two-cycle deprecation notice.
2. Update `rust-toolchain.toml` and the workspace `rust-version`.
3. Update CHANGELOG with the rationale.

### Feature flags

Cross-crate feature gates flow top-down. Common flags:

- `security` — enable DDS-Security plugins.
- `tcp-transport` — enable the RTPS-over-TCP PSM.
- `flatdata-shm` — POSIX-shm zero-copy allocator.
- `iceoryx-shm` — Iceoryx-compatible shm allocator.
- `tracing` — emit `tracing` spans on the hot path.

Default features are conservative: a `cargo build` produces a
minimal participant with UDP transport, no security, no shm.

---

## 3. Adding a new crate

The release-readiness checklist (see `RC1_GUARDRAILS.md` in the
internal repo) requires every crate to pass:

1. **Manifest** — `[package]` block with `description`,
   `license = "Apache-2.0"`, `repository`, `readme = "README.md"`.
2. **README.md** — purpose, public API, one usage example, link
   to the trail station that documents it.
3. **Doc-comments** — `#![doc = include_str!("../README.md")]`
   on the lib root; rustdoc on every `pub` item.
4. **Tests** — `cargo test -p <crate>` green; at least one
   integration test in `tests/`.
5. **Lints** — `cargo clippy -p <crate> -- -D warnings` clean.
6. **Format** — `cargo fmt -p <crate> --check` clean.
7. **No-internal-deps** — only depends on workspace crates from
   the same or lower layer (enforced by CI).
8. **License headers** — `// SPDX-License-Identifier: Apache-2.0`
   on every `.rs` file.
9. **Forbidden tokens** — no Sprint markers, internal hostnames,
   or German-only comments in `pub` doc-comments.

Skeleton:

```bash
cargo new --lib crates/my-feature
# edit Cargo.toml, README.md
cargo build -p zerodds-my-feature
cargo test -p zerodds-my-feature
cargo clippy -p zerodds-my-feature -- -D warnings
```

---

## 4. Wire-format conventions

Every byte that ZeroDDS sends or receives is governed by a
formal spec. There are no ad-hoc framings.

### XCDR2 (DDS-XTypes 1.3 §7.4)

- Default encoding for sample payloads.
- Little-endian on the wire by default; representation header
  encodes the byte order (`00 07 00 00` = XCDR2 LE).
- Mutability is a per-type property (`@final`, `@appendable`,
  `@mutable`); the type's TypeObject travels via SEDP.
- Optional members serialise with a `is_present` flag byte.

### XCDR1 legacy

- Used only for cross-vendor interop with `@final` types when
  the peer announces XCDR1 in the participant data.
- Not the default; tests in `crates/cdr/tests/` cover the
  compatibility matrix.

### RTPS 2.5 (DDSI-RTPS 2.5)

- Submessages are 4-byte aligned, little-endian or big-endian as
  per the SubmessageHeader's E-flag.
- Fragmentation: `DATA_FRAG` + `NACK_FRAG`. The fragment-
  assembler enforces DoS caps (max fragments, max in-flight
  bytes per writer).
- Reliable state machine: tick-driven; default tick 5 ms.
- Vendor extensibility: a `SubmessageExtension` plugin layer
  routes `extra_flags`, reserved-bits, and DDS-Security
  submessages; the base parser stays strict.

### DDS-Security 1.2

- SRTPS wrap is per-participant (CryptoPlugin §7.4.6.6).
- Header AAD covers the RTPS header + submessage prefix.
- Body is encrypted with AES-GCM-256; key derivation per
  the AccessControl plugin.

The full byte-level breakdown lives in `crates/rtps/README.md`
and `crates/cdr/README.md`.

---

## 5. Test categories

CI runs the full matrix; local development typically picks one
category at a time.

| Category | Where | Run with |
|---|---|---|
| Unit | per crate, `src/.../tests` modules | `cargo test -p <crate> --lib` |
| Integration | per crate, `tests/` directory | `cargo test -p <crate> --test '*'` |
| Workspace | root `tests/` | `cargo test --workspace` |
| Doc-tests | `///` examples | `cargo test --doc --workspace` |
| Property-based | `proptest!` macros, marked `#[cfg(test)]` | `cargo test --features proptest` |
| Fuzz | `crates/*/fuzz/` (`cargo-fuzz`) | `cargo +nightly fuzz run <target>` |
| Cross-vendor | `tests/cross-vendor/` (Cyclone / Fast-DDS / RTI) | `tests/cross-vendor/run.sh` |
| Conformance | OMG conformance fixtures | `cargo test -p conformance --test '*'` |
| Real-time | `cyclictest` + load harness | `tools/rt-bench/run.sh` |
| Bench | `criterion` micro-benchmarks | `cargo bench --workspace` |

### Test budget

A clean `cargo test --workspace --lib` runs ~5400 tests in
under three minutes on a 16-core host. Adding a feature without
adding at least one test rejects in code review.

---

## 6. Spec-coverage workflow

Spec coverage is tracked per OMG specification in
`docs/spec-coverage/<spec-id>.md` (internal repo only). Every
section of every formal spec is one of `done` / `partial` /
`open` / `n/a`, with a per-item reference to the test or code
that backs the claim.

Workflow when implementing a new section:

1. Read the spec-section verbatim.
2. Find or write the test that demonstrates the behaviour.
3. Implement.
4. Update `docs/spec-coverage/<spec-id>.md` from `partial` /
   `open` to `done`, with the test path as evidence.
5. Re-run the spec-coverage tally script
   (`tools/spec-coverage/tally.sh`) and commit the result.

The rule: an item is `done` only when there is a coherent
implementation, fully wired, with at least one test. Stubs and
TODOs do not count. (See `feedback_no_hidden_todos_full_spec`
in the project memory.)

---

## 7. Public-mirror sync (`github/`, `website/`)

The internal repository holds the full working tree (planning
notes, internal architecture in German, spec-coverage matrix).
The public OSS mirror lives at
<https://github.com/zero-objects/zero-dds>.

Sync rules:

- `github/` is generated from internal directories by mirror
  passes; never hand-edit the mirror in place if the source
  exists upstream.
- Forbidden tokens (Sprint markers, internal hostnames,
  German-only narrative outside `docs/`) are stripped during
  the mirror pass.
- Internal-only paths (`docs/`, `.planning/`) are not mirrored
  and are referenced from `github/` content as
  *"(internal repo only)"*.

---

## 8. Release process

1. **Tag-cut prerequisites**
   - `cargo test --workspace --lib` green on Linux + macOS + Windows CI.
   - `cargo clippy --workspace -- -D warnings` clean.
   - `cargo fmt --check` clean per crate (no `--all` — see the
     project memory note on path-deps).
   - Spec-coverage tally regenerated; no regressions.
   - CHANGELOG.md updated with the new section.

2. **Tag**
   ```bash
   git tag -a v1.0.0 -m "ZeroDDS 1.0.0"
   git push origin v1.0.0
   ```

3. **Publish artefacts**
   - Crate: `cargo publish -p <crate>` from the lowest layer up.
   - Binary: `cargo dist build --tag v1.0.0` (the `release.yml`
     GitHub Action does this on tag push).
   - Container: `docker build -t ghcr.io/zero-objects/zero-dds:v1.0.0 .`
   - SBOM: `cargo cyclonedx --output sbom.cdx.json` and attach
     to the GitHub release.

4. **Announce**
   - Release notes in `documentation/release-notes/`.
   - GitHub release with the SBOM and signed artefacts.
   - Cross-post to the `zerodds-announce` mailing list.

---

## Where to next

- `02-architecture/README.md` — bird's-eye view of every layer.
- `04-idl/README.md` — IDL semantics and codegen pipeline.
- `CONTRIBUTING.md` — code-review etiquette and PR workflow.
