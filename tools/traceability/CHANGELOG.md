# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the internal `traceability` tool.

### Purpose

Generates the requirements-to-code traceability matrix from
`#[spec(...)]` and `#[satisfies(req = [...])]` annotations across the
workspace. Output: a Markdown table mapping each REQ-ID to the source
file + function that implements it, plus the spec section it derives
from.

### Subcommands

- `traceability scan` — walk the workspace, collect annotations,
  emit Markdown.
- `traceability check` — verify every REQ-ID in `docs/requirements/`
  has at least one `#[satisfies]` reference.

### Architecture

- Layer: Tools (internal, `publish = false`)
- Dependencies: `syn`, `quote`, `walkdir`, `clap`

### Stability

Internal contract.
