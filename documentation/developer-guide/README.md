# Developer Guide

For contributors working **on** ZeroDDS (the stack itself).

## Planned Sections

- **Build & Test** — toolchain setup, workspace layout, cargo recipes,
  cross-compile matrix, interop-test harness.
- **Coding Standards** — Rust style, lint configuration, `unsafe` policy,
  Safety-Subset rules (canonical source: `docs/architecture/04_safety_by_architecture.md`).
- **Testing** — unit, integration, property-based, fuzz, model-checking
  with Kani, interop against Cyclone DDS / Fast DDS.
- **Release Process** — versioning, CHANGELOG, tagging, artifact
  publication, SBOM generation.
- **Documentation Style** — tone, structure, cross-referencing
  conventions for both `docs/` (German, internal) and `documentation/`
  (English, public).
- **Working with Claude Teams** — prompts, agent patterns, review flow.

## Canonical Reference Documents

Developer-guide content draws heavily from the internal architecture docs:

| Internal doc | Topics covered here |
|---|---|
| `docs/architecture/02_architecture.md` | Crate catalog, dependency rules, feature flags |
| `docs/architecture/04_safety_by_architecture.md` | Safe-Subset contract, lints, unsafe policy |
| `docs/architecture/05_observability_and_tooling.md` | Metrics/traces conventions, recorder |

The developer guide translates the relevant parts to English and adds
concrete how-to recipes.

## Status

This directory is a legacy breadcrumb. The current developer-
oriented content lives in [`../02-architecture/`](../02-architecture/)
and [`../04-idl/`](../04-idl/).
