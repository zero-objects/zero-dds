# ZeroDDS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-success.svg)](https://doc.rust-lang.org/edition-guide/)
[![OMG Vendor-ID](https://img.shields.io/badge/OMG%20Vendor--ID-pending-yellow.svg)](#omg-vendor-id-status)

> ⚠️ **OMG Vendor-ID pending.** ZeroDDS does **not yet have an official
> OMG-assigned Vendor-ID**. Until the OMG registers the Vendor-ID,
> Discovery packets (SPDP/SEDP `vendor_id` field in the
> `ProtocolHeader`) carry a **provisional marker** and are therefore
> **not cleared for production cross-vendor deployments**. ZeroDDS speaks
> **native OMG DDSI-RTPS 2.5**; cross-vendor wire interop against Cyclone
> DDS / RTI Connext / eProsima Fast DDS is validated in the conformance
> suite, and the `vendor_id` field is the sole deviation. The application
> has been filed with the OMG; this README will be updated once the
> Vendor-ID is assigned.

Production-grade pure-Rust implementation of the **OMG Data Distribution
Service** — seven native language PSMs (C-FFI, C++, C#, Java, Python,
TypeScript, Rust), ten first-class protocol bridges, and a complete CORBA/CCM
stack.

> **Status:** `1.0.0-rc.4` Release Candidate. 122 crates published on
> crates.io, all bridge layers conformance-verified against Cyclone DDS,
> RabbitMQ, mosquitto, omniORB and gRPC reflection.

## Why ZeroDDS

- **Zero dependencies** in the safe core — no transitive cargo bloat in
  the foundation crates.
- **Zero unsafe** wherever structurally possible, every `unsafe` block
  carries a `// SAFETY:` justification verified by `dds-lint`.
- **Zero panic** in the hot path, contractual `Result`-everywhere.
- **Zero copy** on the shared-memory transport (iceoryx2-compatible).
- **Zero vendor lock-in** — native OMG-DDSI-RTPS-2.5 wire, interoperable
  with Cyclone DDS, OpenDDS and RTI Connext (see Vendor-ID note above).

## Spec Coverage

ZeroDDS implements the full OMG DDS family — **DDS-DCPS 1.4, DDSI-RTPS 2.5,
DDS-XTypes 1.3, DDS-Security 1.2, DDS-XML 1.0, DDS-XRCE 1.0, DDS-RPC 1.0**, the
**DDS-PSM-Cxx** and **DDS-Java-PSM** language mappings, **OMG IDL 4.2**,
**DDS4CCM 1.1** and **CORBA 3.3** — plus the ZeroDDS-published vendor
specifications (the protocol bridges, the per-language XCDR2 binding
conformance, delivery modes, zero-copy, and more).

The complete, per-section coverage matrix — every normative section traced to
source + tests, always current — is published live at
**<https://zerodds.org/spec-coverage/>** (source under
[`docs/spec-coverage/`](docs/spec-coverage/)).

## Quickstart

```bash
# Library, via Cargo — zerodds-dcps pulls in discovery + rtps transitively
cargo add zerodds-dcps

# Or the CLI tools as pre-built packages
brew install zero-objects/zerodds/zerodds   # macOS (Homebrew tap)
sudo apt install zerodds-cli                 # Debian/Ubuntu (.deb)
sudo dnf install zerodds-cli                 # RHEL/Fedora (.rpm)

# Or via Docker (one image per tool/bridge)
docker pull ghcr.io/zero-objects/zerodds-cli:1.0.0-rc.4
```

Hello world publisher (compiles + runs against the published `1.0.0-rc.4`
crate — see [`examples/`](https://github.com/zero-objects/zerodds-examples)):

```rust
use zerodds_dcps::*;

fn main() {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<RawBytes>("Greetings", TopicQos::default())
        .expect("create_topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("create_datawriter");

    writer.write(&RawBytes::new(b"Hello, DDS!".to_vec())).expect("write");
}
```

A full publish→subscribe roundtrip (two participants over UDP) and 14 further
chapters in [`examples/tutorials/dds-chat/`](examples/tutorials/dds-chat/).

## Architecture

Nine layers, 135 workspace crates (122 published on crates.io), single Cargo workspace:

```
Layer 8  CORBA + CCM (17 crates)            corba-ior, corba-iiop, ami4ccm, ...
Layer 7  Bridging Services (11 crates)      ros2-rmw, dlrl, opcua-gateway, xrce, ...
Layer 6  Language Bindings (11 crates)      cpp, cs, java, py, ts-node, ts-wasm, c-api
Layer 5  Protocol Bridges (10 crates)       ws, mqtt, coap, amqp, grpc, corba-dds, bridge-security
Layer 4  Schema (8 crates)                  cdr, idl, idl-{cpp,csharp,java,rust,ts}
Layer 3  Core Services (12 crates)          dcps, qos, security, types, sql-filter, ...
Layer 2  Wire Protocol (6 crates)           rtps, discovery, builtin-topics, ...
Layer 1  Transport (5 crates)               transport-{udp,tcp,shm,tsn}, flatdata
Layer 0  Foundation (8 crates)              foundation, monitor, observability, time, ...
```

Per-layer details in [`docs/architecture/`](docs/architecture/).

## Bindings + Bridges

**Language bindings** (Layer 6) — seven native PSMs on one Pure-Rust core;
**no JNI, no P/Invoke, no thin wrappers** — each is a first-class crate:

| Language | Crate | PSM |
|---|---|---|
| Rust | `zerodds-rs` (idiomatic SDK) / `zerodds-dcps` (core) | ZeroDDS vendor PSM |
| C | `zerodds-c-api` (stable C ABI, cdylib + staticlib) | C-FFI |
| C++17 | `zerodds-cpp` — ABI-stable headers via `zerodds-idlc --cpp` | **OMG DDS-PSM-Cxx 1.0** |
| C# / .NET 8 | `zerodds-cs` (netstandard2.1, NuGet-packageable) | ZeroDDS vendor PSM |
| Java 21 | `zerodds-java-omgdds` (`org.omg.dds.*`, pure Java) | **OMG DDS-Java-PSM** |
| Python 3.8–3.14 | `zerodds-py` (PyO3, abi3 wheels) | ZeroDDS vendor PSM |
| TypeScript (Node) | `zerodds-ts-node` | ZeroDDS vendor PSM |
| TypeScript (browser) | `zerodds-ts-wasm` (WASM, WebSocket transport) | ZeroDDS vendor PSM |

**Protocol bridges** (Layer 5) — ten first-class bridge daemons, each a
workspace crate with its own protocol-conformance suite:

| Bridge | External spec | Crate |
|---|---|---|
| MQTT | OASIS MQTT 5.0 (+ 3.1.1) | `zerodds-mqtt-bridge` |
| AMQP | OASIS AMQP 1.0 (+ 0.9.1) | `zerodds-amqp-bridge` |
| CoAP | IETF RFC 7252 (+ 7641 observe, 7959 block-wise) | `zerodds-coap-bridge` |
| WebSocket | IETF RFC 6455 | `zerodds-websocket-bridge` |
| gRPC | HTTP/2 + Protobuf 3 (DDS-RPC flows here) | `zerodds-grpc-bridge` |
| ROS 2 | REP-2007/2008/2009 (drop-in `rmw`) | `rmw-zerodds-shim` |
| DDS-XRCE | OMG DDS-XRCE 1.0 (MCU clients over serial/UDP) | `zerodds-xrce-agent` |
| Zenoh | Eclipse Zenoh wire protocol | `zerodds-zenoh-bridge` |
| OPC UA | OMG DDS-OPCUA 1.0 gateway | `zerodds-opcua-gateway` |
| SOAP | WSDL / Web Services | `zerodds-soap` |

Shared `zerodds-bridge-security`: TLS 1.3 (rustls), bearer/JWT-RS256/mTLS/
SASL-PLAIN auth, ACL with wildcard + group matching, SIGHUP cert rotation.
In-DDS domain routing via `zerodds-routing-service`; the full CORBA/CCM
GIOP/IIOP stack lives in Layer 8.

## Linux / macOS / Windows

Tier-1 platforms with native installers:

| Platform | Build | Install |
|---|---|---|
| Linux | `cargo build --release` | `apt install zerodds` (`.deb`), `dnf install` (`.rpm`), `pacman -S` (Arch PKGBUILD), AppImage |
| macOS | `cargo build --release` | `brew install zerodds`, launchd plists |
| Windows | `cargo build --release` | MSI (WiX 4) registers as Windows service, Scoop, Chocolatey |
| Docker | n/a | Multi-arch `linux/amd64` + `linux/arm64`, ready-made `docker-compose.yml` |

Packaging artifacts under [`packaging/`](packaging/), production deployment
spec in [`docs/specs/zerodds-deployment-1.0.md`](docs/specs/zerodds-deployment-1.0.md).

## Documentation

- **[Documentation Trail](documentation/README.md)** — six-station guided
  tour for users (English): Install → Architecture → Configuration → IDL →
  Per-Language Integration → Operations.
- **[Spec Coverage](docs/spec-coverage/)** — per-spec audit docs, every
  normative section traced to source + tests.
- **[ADRs](docs/adr/)** — Architecture Decision Records.
- **[Examples](examples/)** — `dds-chat` 15-chapter tutorial, `dds-warehouse`
  10-station industrial-IoT demo, `otel` observability sample.
- **[Vendor Specs](docs/specs/)** — published in OMG-DDS-stylistic format
  with conformance profiles.

## Building

```bash
# Full workspace build
cargo build --workspace --release

# Tests
cargo test --workspace

# Clippy
cargo clippy --workspace --all-targets -- -D warnings

# Docs
cargo doc --workspace --no-deps
```

> **Note:** the `durability-store-lakehouse` adapter links the system
> **libduckdb** (v1.5.3). Either install it (`DUCKDB_LIB_DIR`/`DUCKDB_INCLUDE_DIR`,
> see `ci/Dockerfile.rust`) or exclude the two crates that need it:
> `cargo build --workspace --exclude zerodds-durability-store-lakehouse --exclude zerodds-durability-service-bin`.
> Every other crate builds with no system dependencies.

`rust-toolchain.toml` pins to **1.88.0** (MSRV).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the public contribution flow,
DCO sign-off, PR conventions, and per-layer review expectations.
Code of Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Security issues: [SECURITY.md](SECURITY.md). Do **not** open public issues
for security vulnerabilities.

## OMG Vendor-ID Status

The OMG assigns each DDS implementation a **2-byte Vendor-ID** carried
in the `vendor_id` field of every DDSI-RTPS header (see DDSI-RTPS 2.5
§8.3.3.1 Table 8.1). Known Vendor-IDs include:

| Vendor | ID |
| --- | --- |
| RTI Connext | 0x0101 |
| eProsima Fast DDS | 0x010F |
| Eclipse Cyclone DDS | 0x0110 |
| OpenDDS | 0x0103 |

**ZeroDDS has not yet been assigned a Vendor-ID.** Until the OMG
processes the application, ZeroDDS runs with a provisional marker.
Implications:

- **Wire format**: native DDSI-RTPS 2.5 — only the `vendor_id` field
  differs from Cyclone / RTI / Fast DDS.
- **Cross-vendor discovery**: other vendors do not yet recognise
  ZeroDDS Participants in their Vendor-ID list and treat them as
  "unknown vendor". Cross-vendor interop is nevertheless validated
  in the conformance suite.
- **Production deployments**: cross-vendor setups using ZeroDDS should
  be regarded as **experimental until the Vendor-ID has been
  assigned**.

This section will be updated as soon as the OMG registers the
Vendor-ID. See <https://zerodds.org> for the latest status.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
