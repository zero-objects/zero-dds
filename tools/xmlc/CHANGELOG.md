# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.3] — 2026-06-18

First working release of the `zerodds-xmlc` DDS-XML CLI (the earlier release
candidates carried only a scaffold). A thin front-end over the `zerodds-xml`
parsers (OMG DDS-XML 1.0); it parses a deployment file and reports, never
touching the network.

### Subcommands

- `zerodds-xmlc validate <file.xml>` — check well-formedness and parse every
  DDS-XML library (types, qos, domain, participant, application); print a
  one-line tally. Exit 1 on a parse or validation error.
- `zerodds-xmlc render <file.xml>` — print a structured deployment summary:
  domains and their topics, participants and their publishers/subscribers
  with the data writers/readers underneath.
- `zerodds-xmlc types <file.xml>` — list the `<types>` (XTypes) definitions,
  recursing into modules.

### Implementation

Single binary over `zerodds-xml`. Manual argument parsing (no `clap`).

### Architecture

- Layer: Tools
- Dependencies: `zerodds-xml`

### Stability

CLI surface stable for `1.0.x`. New subcommands are additive minor bumps;
flag removals require a major bump.
