# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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
