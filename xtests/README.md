# Cross-Crate Integration Tests (`xtests/`)

Tests that span multiple crates and cannot live inside any single
`crate/*/tests/` directory. Canonical definition:
`docs/architecture/02_architecture.md §8`.

Typical content:

- **End-to-end scenarios** — DCPS publisher and subscriber in the same
  process, across processes, across hosts.
- **Interop harness** — Dockerized Cyclone DDS and Fast DDS peers, test
  scenarios driven from a Rust harness.
- **Protocol conformance** — OMG spec test vectors replayed against our
  stack.
- **Chaos tests** — packet loss, reordering, partitions, clock skew.

Each sub-directory is a separate Rust crate. Add them to the workspace
`members` list in the root `Cargo.toml` when created.

## Status

Scaffolded. First sub-crates added in Phase 1 alongside the RTPS-reliable
work package (`docs/architecture/06_roadmap.md §4` WP 1.10, 1.11).
