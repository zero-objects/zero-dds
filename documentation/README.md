# ZeroDDS Documentation

ZeroDDS is a pure-Rust implementation of [OMG DDS 1.4][dds] +
[DDSI-RTPS 2.5][rtps] + [DDS-Security 1.2][sec] + [DDS-XTypes 1.3][xtypes],
with bindings for C, C++, C#, Java, Python, TypeScript, and a ROS-2
RMW plugin.

For the per-crate API reference see docs.rs; for the full website see
<https://zerodds.org>.

## Getting started

- [Overview](01-getting-started/README.md) · [Concepts](01-getting-started/concepts.md)
  · [Installation](01-getting-started/installation.md)
  · [First publisher / subscriber](01-getting-started/first-publisher.md)

## Architecture

- [Component / crate map](02-architecture/components.md) — how the
  ~120 crates group into layers, and the dependency direction.
- [Data flow on `write`](02-architecture/data-flow.md) — the hot path
  when application code publishes a sample.

## Configuration

- [Overview](03-configuration/README.md) ·
  [Runtime config](03-configuration/runtime-config.md) ·
  [QoS policies](03-configuration/qos-policies.md) ·
  [Transport](03-configuration/transport.md) ·
  [Security](03-configuration/security.md) ·
  [Observability](03-configuration/observability.md)

## IDL & wire types

- [IDL reference](04-idl/README.md) — OMG-IDL 4.2 as ZeroDDS' wire-type
  language, compiled by `zerodds-idlc`.
- [`zerodds-idlc` handbook](04-idl/idlc-handbook.md) — the end-user
  compiler manual: CLI, annotations, build integration, cookbook.
- [CDR wire format](04-idl/cdr-wire-format.md) — the XCDR1 / XCDR2
  byte form of every IDL construct.

## Integration per language

- [Java](05-integration/java.md) — the pure-Java `org.omg.dds.*` PSM.
- [TypeScript / WASM](05-integration/typescript-wasm.md) — the browser
  XCDR codec + WebSocket-bridge pattern.

## Operations

- [Overview](06-operations/README.md) ·
  [Deployment](06-operations/deployment.md) ·
  [Monitoring](06-operations/monitoring.md) ·
  [Troubleshooting](06-operations/troubleshooting.md)

## Migration from another DDS

- [Overview](07-migration/README.md) — from
  [Cyclone DDS](07-migration/from-cyclonedds.md),
  [Fast DDS](07-migration/from-fastdds.md),
  [OpenDDS](07-migration/from-opendds.md), or
  [RTI Connext](07-migration/from-rti-connext.md).

## Where else to look

- **Per-crate API reference** — published on [docs.rs][docsrs]; each
  crate also ships a `README.md` quick-start under `crates/<name>/`.
- **Full documentation & guides** — <https://zerodds.org>.
- **Specifications** — the OMG standards ZeroDDS implements, linked below.

[dds]: https://www.omg.org/spec/DDS/1.4/
[rtps]: https://www.omg.org/spec/DDSI-RTPS/2.5/
[sec]: https://www.omg.org/spec/DDS-SECURITY/1.2/
[xtypes]: https://www.omg.org/spec/DDS-XTypes/1.3/
[docsrs]: https://docs.rs/releases/search?query=zerodds
