# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.3] — 2026-06-18

First working release of the `zerodds-admin` operator CLI (the earlier
release candidates carried only a scaffold). Two command families — *live*
groups join a running DDS domain over RTPS, *offline* groups load a DDS-XML
deployment and analyze it without touching the network.

### Subcommands

Live (joins the domain over RTPS):

- `zerodds-admin domain inspect <id>` — list discovered participants, each
  with its writer/reader endpoints (topic, IDL type, RELIABLE vs
  BEST_EFFORT, TRANSIENT_LOCAL vs VOLATILE), grouped by participant GUID.
- `zerodds-admin discovery snapshot <id>` — raw SPDP/SEDP snapshot: the
  participant list plus discovered publication/subscription counts.

Offline (DDS-XML, no network):

- `zerodds-admin config inspect <file.xml>` — load a deployment and render a
  domain-id-centric topology.
- `zerodds-admin qos validate <file.xml>` — DDS-XML well-formedness plus
  parse of every library (qos/domain/participant).
- `zerodds-admin qos check <file.xml>` — Request-vs-Offered (RxO) QoS
  compatibility of every writer×reader pair sharing a topic (DDS 1.4
  §2.2.3); inline QoS and `qos_profile_ref` inheritance both resolved.
  Exit 1 on any incompatibility.

### Implementation

Single binary. Live groups use `zerodds-dcps`'s `DcpsRuntime`
discovery API; offline groups use `zerodds-xml` + `zerodds-qos`. Manual
argument parsing (no `clap`); `--json` output is hand-rolled (no `serde`),
matching the other ZeroDDS CLIs.

### Architecture

- Layer: Tools
- Dependencies: `zerodds-cli-common`, `zerodds-dcps`, `zerodds-discovery`,
  `zerodds-qos`, `zerodds-xml`

### Stability

CLI surface is stable for `1.0.x`. New subcommands are additive minor
bumps; flag removals require a major bump.
