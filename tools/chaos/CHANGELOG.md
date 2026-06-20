# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

This is internal tooling — no normative OMG spec. Policy semantics
are defined in the tool's `src/`.

### CLI sub-commands

* `proxy` — in-process UDP chaos proxy with `--drop`, `--delay`,
  `--reorder`, `--dup`, `--seed`.
* `tc-apply` — wrap `tc qdisc add` with a `netem`-friendly schema
  (`--delay`, `--loss`, `--reorder`, `--rate`, `--corrupt`).
* `tc-clear` — wrap `tc qdisc del` for cleanup.
* `endpoint-flap` — open + close a discovery endpoint at a
  configurable cadence to stress SPDP / SEDP.
* `partition` — split a UDP path with a one-way blackhole.

### Public API (library `zerodds_chaos`)

* `proxy::*` — proxy entry points.
* `partition::*` — partition primitives.
* `endpoint_flap::*` — flap orchestrator.
* `tc::*` — `tc` / `netem` wrapper.
* `prng::*` — deterministic seeded PRNG.

### Implementation

The proxy uses two `std::net::UdpSocket` instances on `tokio`-free
blocking threads; chaos decisions are made by a per-stream PRNG
seeded from `--seed`. The `tc` module shells out to the system
`tc` binary and parses its return code; arguments are validated
against an allowlist before being passed.

`endpoint-flap` opens a UDP socket at `--addr`, sleeps `--down`,
closes it, sleeps `--up`, repeats. With ZeroDDS discovery this
produces a steady stream of `PARTICIPANT_LOST` / `MATCHED` events
on subscribers — useful for matching-state stress tests.

### Architecture

* Layer: Tools.
* Dependencies (in): none beyond `std`.

### Stability

CLI is RC1-stable. Breaking changes require a major version bump.
