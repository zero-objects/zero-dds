# ZeroDDS — Documentation map

German original: [README.md](README.md)

Entry point to the documentation map: what ZeroDDS contains, where it lives,
how the parts fit together, and how to configure, operate, secure and connect
them and migrate to them from another DDS. The pages were written anew from
the code and are meant to serve as the basis for better documentation.

Conventions on all pages:

- Paths are relative to the repository root (`crates/rtps/src/reliable_writer.rs`).
- Statements come from the code. Anything that comes only from a doc comment,
  a spec or a Markdown file is marked “(per doc comment)” or
  “(per docs)”.
- Nothing was built or run during the survey, except the already built
  programs with `--help` and `--check-config`. Where a page gives measured
  values, the source (test, runner, host) is stated with them.
- Every page ends with a section on contradictions and open points;
  [befunde.en.md](befunde.en.md) collects them.

## Pages

| Page | Contents |
|---|---|
| [architecture.en.md](architecture.en.md) | Layer model, runtime model, path of a sample from `write` to the reader, discovery, reliability, durability, type discovery, content filters, SHM, security hook points |
| [crates-core.en.md](crates-core.en.md) | Core crates: type system and wire, RTPS and discovery, transports, DCPS and configuration, observability, DDS profiles, services, test and build |
| [crates-bridges-services.en.md](crates-bridges-services.en.md) | The six bridge daemons, `bridge-security`, CORBA family, OPC UA, SOAP |
| [crates-languages-codegen.en.md](crates-languages-codegen.en.md) | IDL pipeline, `zerodds-idlc`/`zerodds-xmlc`, 21 target languages, language bindings, endpoint SDKs |
| [config.en.md](config.en.md) | Layer model, all 46 runtime settings, keys of every daemon, DDS-XML QoS, security files, environment variables |
| [cli-options.en.md](cli-options.en.md) | Every program with synopsis, options, exit codes, config file, man page |
| [tools-and-man.en.md](tools-and-man.en.md) | The `tools/` directory, the man pages, `packaging/` |
| [system-setup.en.md](system-setup.en.md) | Building blocks of a complete system and recipes from a single-host setup to a service under systemd/launchd |
| [security.en.md](security.en.md) | DDS-Security (plugins, governance/permissions, enclaves, crypto backends) and bridge security (TLS/DTLS, auth, ACL), step-by-step lists |
| [migration.en.md](migration.en.md) | Migration from Cyclone DDS, OpenSplice, RTI Connext, Fast DDS, OpenDDS: API, IDL, QoS, configuration, mixed operation |
| [interop-and-dialects.en.md](interop-and-dialects.en.md) | Connecting to an existing system on the wire, dialect and wire-axis flags, table per peer |
| [ros2.en.md](ros2.en.md) | RMW `rmw_zerodds_cpp`, build against ROS 2, `ros2.*` settings, diagnostic CLI, measured interop |
| [befunde.en.md](befunde.en.md) | All contradictions between code, docs, man pages and examples found during the survey |

## Signposts

| I want to … | Page |
|---|---|
| understand how a sample travels through the system | [architecture.en.md §3](architecture.en.md#3-path-of-a-sample) |
| know which crate does what | [crates-core.en.md](crates-core.en.md), [crates-bridges-services.en.md](crates-bridges-services.en.md), [crates-languages-codegen.en.md](crates-languages-codegen.en.md) |
| find a setting, with its default and environment variable | [config.en.md §2](config.en.md#2-runtime-settings-zeroddsyaml) |
| know in which order files, environment and `--set` apply | [config.en.md §1](config.en.md#1-layer-model) |
| look up the options of a program | [cli-options.en.md](cli-options.en.md) |
| generate types from IDL for my language | [crates-languages-codegen.en.md §10](crates-languages-codegen.en.md#10-how-to-generate-types-for-an-application) |
| set up a system (one host, several hosts, without multicast, with persistence) | [system-setup.en.md, recipes](system-setup.en.md#recipes) |
| enable security | [security.en.md (c)](security.en.md#c-setting-up-security-step-by-step) |
| run ZeroDDS alongside Cyclone/Fast DDS/RTI/OpenDDS/OpenSplice | [interop-and-dialects.en.md](interop-and-dialects.en.md) |
| migrate an existing system | [migration.en.md](migration.en.md) |
| connect MQTT, WebSocket, CoAP, AMQP, gRPC or CORBA | [crates-bridges-services.en.md](crates-bridges-services.en.md) |
| run ROS 2 with ZeroDDS | [ros2.en.md](ros2.en.md) |
| find a man page | [tools-and-man.en.md](tools-and-man.en.md) |

## The system in one picture

```
 Application ─┬─ Rust: crates/dcps (DomainParticipant, DataWriter, DataReader)
              ├─ C/C++: crates/zerodds-c-api (libzerodds, zerodds.h), crates/cpp
              ├─ Python/Java/C#/TypeScript/…: crates/py, crates/java-omgdds, …
              └─ ROS 2: crates/rmw-zerodds-shim (rmw_zerodds_cpp)
                 │
   Types: IDL ── zerodds-idlc ──► code per language; XTypes TypeObject/TypeLookup
                 │
   DCPS runtime (crates/dcps): QoS, history, reader/writer proxies, security gate
                 │
   RTPS (crates/rtps, crates/discovery): SPDP/SEDP, Heartbeat/AckNack, fragments
                 │
   Transports: UDP (multicast/unicast), TCP, SHM (same host), UDS, TSN
                 │
   Services/tools: Durability Service, router, recorder/replay, monitor,
   admin/spy · Bridges: MQTT, WebSocket, CoAP, AMQP, gRPC, CORBA, OPC UA
```

Details: [architecture.en.md](architecture.en.md).

## Directories in the repository root

| Directory | Contents |
|---|---|
| `crates/` | All Rust crates (libraries, daemons, bindings); see the three crate pages |
| `tools/` | Programs and scripts: user tools, CI gates, test harnesses ([tools-and-man.en.md](tools-and-man.en.md)) |
| `man/` | `man1` (19 programs), `man5` (7 config files) |
| `packaging/` | deb/rpm/systemd, Homebrew/launchd/pkg, Windows, Docker, Python, release scripts |
| `pkg/` | further package definitions (debian, macos, rpm, windows) |
| `endpoints/` | Native endpoint SDKs per language (C, C++, Ada, Go, Java, Python, Rust, …) |
| `runtime/` | Runtime files of the generated language bindings, per language |
| `examples/` | Tutorials and demos (`examples/tutorials/dds-chat`, `examples/demos/…`) |
| `zerodds-examples/` | Examples for the code snippets on the website (local subfolder, not versioned) |
| `docs/specs/` | 47 ZeroDDS specs (bridges, deployment, codegen, …) |
| `docs/spec-coverage/` | Coverage per spec with evidence and status, `.open` aggregates |
| `docs/adr/` | Architecture decisions |
| `documentation/` | User documentation from an earlier state (`documentation/README.md`); not checked against the code |
| `build-integration/`, `cmake/` | `zerodds-idlc` as a build step in third-party build systems |
| `buildroot/`, `meta-zerodds/` | Buildroot package, Yocto layer |
| `benches/` | Criterion benchmarks |
| `tests/`, `xtests/`, `testing/`, `interop/` | Interop corpus and results, cross-crate tests, isolation, interop fixtures |
| `ci/` | GitLab CI jobs, images, helper scripts |
| `scripts/` | Pre-commit, crate publish, mirror build |
| `website/` | Sources of zerodds.org |
| `diagrams/` | Diagram sources (dot) |
| `internal/` | internal working notes, release documents, OPEN-ITEMS; not public |
| `github/` | local mirror for the public repository (not part of this repo) |

## Maintenance

A page is valid as long as the code it describes is valid. Whoever changes
code that is named with its path on one of these pages updates the page and
its German original. The
findings list [befunde.en.md](befunde.en.md) is worked through and not
extended: a fixed finding is removed.
