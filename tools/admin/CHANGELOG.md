# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the `zerodds-admin` CLI.

### Subcommands

- `zerodds-admin domain inspect <id>` — list participants, publishers,
  subscribers, topics in a running DDS domain.
- `zerodds-admin qos validate <file>` — validate a QoS XML profile against
  the DDS-XML 1.0 schema.
- `zerodds-admin discovery snapshot <id>` — capture a SPDP/SEDP snapshot
  for offline analysis.

### Implementation

Single-binary, links against `zerodds-dcps`, `zerodds-discovery`,
`zerodds-qos`, no external network beyond DDS. Output formats:
plain-text (default), JSON (`--json`), table (`--table`).

### Architecture

- Layer: Tools
- Dependencies: `zerodds-dcps`, `zerodds-discovery`, `zerodds-qos`,
  `clap` (CLI), `serde` + `serde_json` (output)

### Stability

CLI surface is stable for `1.0.x`. New subcommands are additive minor
bumps; flag removals require a major bump.
