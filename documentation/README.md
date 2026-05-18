# ZeroDDS Documentation Trail

Welcome. This is the guided learning path through ZeroDDS. Follow it
in order if you are new; jump in at any station if you know what you
need.

ZeroDDS is a pure-Rust implementation of [OMG DDS 1.4][dds] +
[DDSI-RTPS 2.5][rtps] + [DDS-Security 1.2][sec] + [DDS-XTypes 1.3][xtypes],
with bindings for C, C++, C#, Java, Python, TypeScript, and a ROS-2
RMW plugin.

## The Trail

| Station | Purpose | Read it when … |
|---|---|---|
| **[01 – Getting Started](01-getting-started/README.md)** | Install + first publisher / subscriber + DDS concepts | You want a working `cargo run` in 5 minutes. |
| **[02 – Architecture](02-architecture/README.md)** | How the pieces fit together: layers, crates, data flow | You want to understand or extend the codebase. |
| **[03 – Configuration](03-configuration/README.md)** | Runtime, QoS, security, observability — every knob | You are deploying ZeroDDS to production. |
| **[04 – IDL Reference](04-idl/README.md)** | OMG-IDL syntax, annotations, codegen via `zerodds-idlc` | You design wire-types and schemas. |
| **[05 – Integration per Language](05-integration/README.md)** | How to call ZeroDDS from Rust / C / C++ / C# / Java / Python / TypeScript / ROS-2 | You ship a non-Rust application. |
| **[06 – Operations](06-operations/README.md)** | Deployment, monitoring, troubleshooting | You run ZeroDDS in production. |
| **[07 – Migration Guides](07-migration/README.md)** | Port from Cyclone / Fast DDS / RTI / OpenDDS to ZeroDDS | You're switching DDS vendors. |

## Learning paths

Different roles, different reading orders.

### Application developer (writes a DDS app)

```
01 → 04 → 05 → 03
```

Get hello-world running, learn the IDL, pick your language guide,
then tune the QoS knobs you actually need.

### System integrator (deploys ZeroDDS into a fleet)

```
01 → 03 → 06 → 02
```

Hello-world, configuration matrix, deployment + monitoring; come
back to architecture only if a config option puzzles you.

### Contributor (works on the ZeroDDS codebase itself)

```
02 → ../docs/architecture/ (internal) → 04 → 03
```

Architecture is the entry point; `../docs/architecture/` (German,
internal developer focus) is the deeper reference; IDL and
configuration are the next layers.

### Spec implementor (cross-vendor / standards work)

```
02 → 04 → specs/ → ../docs/spec-coverage/
```

Architecture for the bird's-eye view, IDL for the wire-type
contract, the published vendor specs (`specs/dds-amqp-1.0`,
`specs/dds-ts-1.0`) for what we contribute back, and the spec-
coverage matrix for what's done.

## What format?

Every station ships as Markdown for browsing and as a PDF for
printing or offline review.

```bash
make -C documentation pdfs                # all stations + vendor specs
make -C documentation pdf-arch            # only architecture (LaTeX)
make -C documentation pdf-getting-started # one specific station
make -C documentation api                 # generate API reference
```

PDFs land in `documentation/dist/`. The build uses
[pandoc][pandoc] (markdown → PDF) plus [tectonic][tectonic] (PDF
engine) — both are single-binary installs, no TeXLive needed.

API reference (`make api`) generates per-language docs into
`documentation/api/{rust,c,cpp,java,python,typescript,csharp}/`
via the language's native tool (`cargo doc`, `doxygen`,
`javadoc`, `pdoc`, `typedoc`, `docfx`). See
[api/README.md](api/README.md) for prerequisites.

## Cross-references

- `../docs/` — internal developer documentation in German, architecture
  ADRs, RFCs, plans. Read this if you contribute to ZeroDDS.
- `specs/` — formal vendor specifications we publish: DDS-AMQP 1.0,
  DDS-TS 1.0. Read these if you do cross-vendor wire-level interop.
- `api/` — generated rustdoc per crate. Read this for the Rust-API
  ground truth.
- `../crates/<name>/README.md` — per-crate quick-start, kept tight.

## Legacy structure

Earlier doc layout used four bins — `user-guide/`, `developer-guide/`,
`operator-guide/`, `api/`. The trail subsumes those:

| Old | New |
|---|---|
| `user-guide/` | `01-getting-started/` + `05-integration/` |
| `developer-guide/` | `02-architecture/` + `04-idl/` |
| `operator-guide/` | `03-configuration/` + `06-operations/` |
| `api/` | `api/` (kept for rustdoc-generated reference) |

The old `README.md` files in those subdirs remain in place as
breadcrumbs that point at the new locations.

## Versioning

Documentation tracks the workspace `cargo` version (currently `0.0.0`,
pre-release). Doc anchors and API signatures are not yet stable — when
the workspace cuts a `0.1.0` tag, the docs get the same stamp.

[dds]: https://www.omg.org/spec/DDS/1.4/
[rtps]: https://www.omg.org/spec/DDSI-RTPS/2.5/
[sec]: https://www.omg.org/spec/DDS-SECURITY/1.2/
[xtypes]: https://www.omg.org/spec/DDS-XTypes/1.3/
[tectonic]: https://tectonic-typesetting.github.io/
[pandoc]: https://pandoc.org/
