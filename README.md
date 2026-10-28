# ZeroDDS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-success.svg)](https://doc.rust-lang.org/edition-guide/)

ZeroDDS is a Rust implementation of OMG DDS, with language bindings,
code generation and protocol bridges in one source workspace.

> **Source status: RC7 preparation.** This checkout is not release approval
> or a statement that RC7 crates, installers and container images are published.
> The instructions below follow this source revision. External channels can
> still describe an earlier release.

## Evidence and scope

Interop is evaluated per case, representation, direction and toolchain.
A successful case is not a promise that all constructs or configurations
interoperate, and successful round trips do not imply identical bytes.

The [interop ledger](website/interop-ledger.html) presents the measurements.
The [recorded results](tests/interop-results/ledger.tsv) and
[specification coverage sources](docs/spec-coverage/) retain their own
scope and revision. Historical checklist totals are not a whole-product
RC7 conformance certificate. See the case evidence before extrapolating a
result to a deployment.

The repository still documents its Vendor-ID as provisional. The
[RTPS source](crates/rtps/src/wire_types.rs) is the authority for the emitted
identifier, not an inferred OMG registration. This README does not assert
that a Vendor-ID is the only possible cross-vendor difference.

## Quickstart from this checkout

Use Rust 1.88 or newer and a native linker. From the checkout root:

```bash
cargo build --locked -j 1 -p zerodds-dcps \
  --example hello_dds_publisher --example hello_dds_subscriber
# Terminal 1
./target/debug/examples/hello_dds_subscriber
# Terminal 2
./target/debug/examples/hello_dds_publisher
```

Use the corresponding .exe paths on Windows. The maintained
[publisher](crates/dcps/examples/hello_dds_publisher.rs) and
[subscriber](crates/dcps/examples/hello_dds_subscriber.rs) use domain 0 and
topic Chatter. They demonstrate opaque RawBytes payloads, not generated-type
or cross-vendor conformance. Confirm matching output in the subscriber.
Both run until interrupted.

See [setup and configuration](documentation/01-getting-started/installation.md)
and the [complete walkthrough](documentation/01-getting-started/first-publisher.md).
For generated types, see [zerodds-build](crates/zerodds-build/README.md).

## Source map

The [workspace manifest](Cargo.toml) identifies current crates and versions.
Do not infer the current architecture from old crate counts.

- [DCPS](crates/dcps/README.md): participants, topics, readers and writers.
- [IDL compiler](tools/idlc/README.md) and [build integration](crates/zerodds-build/README.md):
  the current generation entrypoints.
- [Configuration loader](crates/config/README.md): base files, sorted drop-ins,
  explicit includes and provenance. Runtime selection is a separate consumer
  contract described in the getting-started guide.
- [Architecture documentation](docs/architecture/): design references whose
  dates and source links must be considered alongside the current manifest.

## Language entrypoints

These links identify implementations in the checkout, not availability of
new packages in public registries. Generated codecs and native DDS endpoints
are distinct layers.

| Language | Current source entrypoint |
|---|---|
| Rust | [zerodds-dcps](crates/dcps/README.md), [Rust facade](crates/rs/README.md) |
| C | [C API](crates/zerodds-c-api/README.md) |
| C++ | [C++ binding](crates/cpp/README.md) |
| C# | [Native binding](crates/cs/README.md), with separate CDR/wire runtime dependencies |
| Java | [Native Java DDS PSM](crates/java-omgdds/README.md), not an assumed JNI implementation |
| Python | [PyO3 binding](crates/py/README.md), Python 3.9+, exact zerodds-wire dependency in [pyproject.toml](crates/py/pyproject.toml) |
| TypeScript / Node | [koffi-based native binding](crates/ts-node/README.md); generated codecs use the separate @zerodds/wire package |
| TypeScript / browser | [Browser binding](crates/ts-wasm/README.md) |

The presence of a language entrypoint does not establish equal API coverage,
transport support or wire representations across all bindings. Follow its
current source and measured examples.

## Packaging and deployment

[Packaging sources](packaging/) describe platform-specific build inputs.
Their presence does not mean that every package repository, installer,
architecture or container image is available or verified for RC7.

Start with the scoped source build above. Full-workspace builds include
optional bridges, native bindings and system-library integrations and require
their platform dependencies. For example, the durability lakehouse adapter
requires libduckdb. Do not assume every other crate is dependency-free.

The [toolchain file](rust-toolchain.toml) pins Rust 1.88.0.
The [deployment specification](docs/specs/zerodds-deployment-1.0.md) describes
configuration and layout contracts, not a release-channel availability check.

## Documentation

- [Documentation Trail](documentation/README.md): the guided entrypoint.
- [Specification coverage](docs/spec-coverage/): scoped audit records.
- [ADRs](docs/adr/): architectural decisions and their history.
- [Examples](examples/): source recipes and demonstrations.
- [Vendor specifications](docs/specs/): specification and design documents.

These links do not collectively approve every historical guide or claim.
Release coordination, infrastructure notes and unfinished investigations belong
in internal/, separate from product-facing instructions.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md), [Code of Conduct](CODE_OF_CONDUCT.md)
and [SECURITY.md](SECURITY.md). Do not post vulnerability details in public
issues.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
