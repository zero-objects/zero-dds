# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the `zerodds-xmlc` DDS-XML CLI.

### Subcommands

- `zerodds-xmlc validate <file.xml>` — validate a DDS-XML deployment
  descriptor against the OMG DDS-XML 1.0 XSD suite (14 normative XSDs).
- `zerodds-xmlc render <file.xml>` — render a deployment descriptor
  into per-participant runtime configuration files (YAML).
- `zerodds-xmlc lint <file.xml>` — apply best-practice lints (unused
  profiles, conflicting QoS combinations).

### Spec References

- OMG DDS-XML 1.0 (`formal/2024-09-01`) — full schema coverage
- OMG DDS-DCPS 1.4 §7.1 — QoS-policy semantics
- OMG DDS-Security 1.2 §9.2 — governance + permissions schemas

### Architecture

- Layer: Tools
- Dependencies: `zerodds-qos`, `zerodds-types`, `quick-xml` (parser),
  `clap` (CLI)

### Stability

CLI surface stable for `1.0.x`. Schema-version detection means inputs
targeting future XSD revisions remain readable.
