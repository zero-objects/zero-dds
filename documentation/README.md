# ZeroDDS Documentation

ZeroDDS is a pure-Rust implementation of [OMG DDS 1.4][dds] +
[DDSI-RTPS 2.5][rtps] + [DDS-Security 1.2][sec] + [DDS-XTypes 1.3][xtypes],
with bindings for C, C++, C#, Java, Python, TypeScript, and a ROS-2
RMW plugin.

This folder holds selected reference pages. For the full guided
documentation see the project website, and for the per-crate API
reference see docs.rs.

## Reference pages

### Architecture

- [Component / crate map](02-architecture/components.md) — how the
  ~120 crates group into layers, and the dependency direction.
- [Data flow on `write`](02-architecture/data-flow.md) — what
  happens on the hot path when application code publishes a sample.

### IDL & wire types

- [IDL reference](04-idl/README.md) — OMG-IDL 4.2 as ZeroDDS' wire-type
  language, compiled by `zerodds-idlc`.
- [`zerodds-idlc` handbook](04-idl/idlc-handbook.md) — the end-user
  compiler manual: install, CLI, annotations, build integration,
  cookbook, troubleshooting.
- [CDR wire format](04-idl/cdr-wire-format.md) — the XCDR1 / XCDR2
  byte form of every IDL construct.

### Integration per language

- [Java](05-integration/java.md) — the pure-Java `org.omg.dds.*` PSM.
- [TypeScript / WASM](05-integration/typescript-wasm.md) — the
  browser XCDR codec + WebSocket-bridge pattern.

## Where else to look

- **Per-crate API reference** — published on [docs.rs][docsrs]; each
  crate also ships a `README.md` quick-start under `crates/<name>/`.
- **Full documentation & guides** — on the project website,
  <https://zerodds.org>.
- **The specifications** — the OMG standards ZeroDDS implements are
  linked below.

[dds]: https://www.omg.org/spec/DDS/1.4/
[rtps]: https://www.omg.org/spec/DDSI-RTPS/2.5/
[sec]: https://www.omg.org/spec/DDS-SECURITY/1.2/
[xtypes]: https://www.omg.org/spec/DDS-XTypes/1.3/
[docsrs]: https://docs.rs/releases/search?query=zerodds
