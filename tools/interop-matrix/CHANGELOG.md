# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the internal `interop-matrix` tool.

### Purpose

Reads CI test results (JUnit XML or JSON) from cross-vendor interop runs
and renders a vendor-vs-vendor compatibility table as Markdown or HTML.
Output feeds the public interoperability dashboard.

### Subcommands

- `interop-matrix render <results-dir> --output <file>` — render the
  matrix in Markdown (default), HTML, or CSV.
- `interop-matrix diff <baseline> <current>` — show regressions vs. the
  last published baseline.

### Architecture

- Layer: Tools (internal, `publish = false`)
- Dependencies: `serde`, `serde_json`, `clap`

### Stability

Internal contract; output format may evolve without SemVer guarantees.
