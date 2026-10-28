# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

* The crate set is read from `cargo metadata --format-version 1 --no-deps`
  instead of the `workspace.members` list in the root `Cargo.toml`. Cargo
  makes every path dependency of a listed member a member too: the list
  names 162 of the workspace's 180 packages, and all 18 implicit members
  are publishable. Those 18 — the `zerodds-transport*` and
  `zerodds-security*` families, `zerodds-recorder`, `zerodds-sql-filter`,
  `zerodds-inspect-endpoint`, `zerodds-iceoryx-cyclone` — were absent from
  the publish order, so a publish run would have skipped dependencies and
  uploaded their dependents first. The publish order now holds all 129
  publishable packages, and a newly added path dependency enters it with no
  list to edit.

### Changed

* `collect_crates` returns `DagError::Spawn` / `DagError::Metadata` /
  `DagError::Json` in place of the former `Io` / `MissingMembers` /
  `MissingPackageName`; the hand-written TOML walker is gone. A package is
  outside the publish run when its manifest `publish` key names a registry
  list that does not contain `crates-io` (`publish = false` reaches
  `cargo metadata` as the empty list).

### Added

* `tests/workspace_publish_set.rs` — runs against this repository: the
  publish order must equal the publishable members `cargo metadata`
  reports, must contain the implicit members, and must order every crate
  after its dependencies.
* `src/json.rs` — dependency-free reader for the metadata document.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### CLI

* `--workspace <path>` — root of the workspace (defaults to `.`).
* Outputs one crate name per line in publish order.

### Public API (library)

* `topological_sort(workspace_root)` — returns `Vec<String>` in
  publish order.
* `WorkspaceGraph` — node + edge model.

### Implementation

A pure-Rust Cargo.toml parser (manual TOML walker, no
`toml` crate dependency) builds the dependency graph from
`[workspace.members]` plus each member's `[dependencies]` /
`[dev-dependencies]` blocks. Cycles produce an error; ties are
broken alphabetically for determinism.

### Stability

The library API and CLI surface are RC1-stable for the publish-
script contract. Breaking changes are coordinated with that
script.

### Internal-only justification

This tool consumes the in-tree workspace graph; it is not
meaningful as a published crates.io crate.
