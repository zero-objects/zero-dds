# ZeroDDS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-success.svg)](https://doc.rust-lang.org/edition-guide/)
[![OMG Vendor-ID](https://img.shields.io/badge/OMG%20Vendor--ID-pending-yellow.svg)](#omg-vendor-id-status)

> ⚠️ **OMG Vendor-ID pending.** ZeroDDS does **not yet have an official
> OMG-assigned Vendor-ID**. Until the OMG registers the Vendor-ID,
> Discovery packets (SPDP/SEDP `vendor_id` field in the
> `ProtocolHeader`) carry a **provisional marker** and are therefore
> **not cleared for production cross-vendor deployments**. Wire
> compatibility against Cyclone DDS / RTI Connext / eProsima Fast DDS
> is byte-identical; the `vendor_id` field is the sole deviation. The
> application has been filed with the OMG; this README will be updated
> once the Vendor-ID is assigned.

Production-grade pure-Rust implementation of the **OMG Data Distribution
Service** with bindings for C, C++, C#, Java, Python, TypeScript and
Flutter, plus seven protocol bridges and a complete CORBA/CCM stack.

> **Status:** `1.0.0-rc.6` Release Candidate. 125 crates published on
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

ZeroDDS implements the full OMG DDS family plus modern bridging:

| Spec | Version | Status |
|---|---|---|
| OMG DDS DCPS | 1.4 | 100/100 + 2 n/a |
| OMG DDSI-RTPS | 2.5 | 121/121 + 3 n/a |
| OMG DDS-XTypes | 1.3 | 82/82 + 1 n/a |
| OMG DDS-Security | 1.2 | 50/50 + 3 n/a |
| OMG DDS-XML | 1.0 | 73/73 + 15 n/a |
| OMG DDS-XRCE | 1.0 | 82/82 + 13 n/a |
| OMG DDS-RPC | 1.0 | 94/94 + 10 n/a |
| OMG DDS-PSM C++ | 1.0 | 103/103 + 19 n/a |
| OMG DDS-Java-PSM | 1.0 | 156/156 + 15 n/a |
| OMG DDS4CCM | 1.1 | 24/24 + 10 n/a |
| OMG IDL | 4.2 | 649/649 + 24 n/a |
| OMG CORBA | 3.3 | 51/51 + 12 n/a |

Plus seven ZeroDDS-published Vendor Specifications:
**DDS-AMQP**, **DDS-TS**, **DDS-WebSocket-Bridge**, **DDS-MQTT-Bridge**,
**DDS-CoAP-Bridge**, **DDS-gRPC-Bridge**, **DDS-CORBA-Bridge** plus
**ZeroDDS-FFI-Loader** and **ZeroDDS-Deployment** specs. Full coverage
matrix in [`docs/spec-coverage/`](docs/spec-coverage/).

## Quickstart

```bash
# Library, via Cargo — zerodds-dcps pulls in discovery + rtps transitively
cargo add zerodds-dcps

# Or the CLI tools as pre-built packages
brew install zero-objects/zerodds/zerodds   # macOS (Homebrew tap)
sudo apt install zerodds-cli                 # Debian/Ubuntu (.deb)
sudo dnf install zerodds-cli                 # RHEL/Fedora (.rpm)

# Or via Docker (one image per tool/bridge)
docker pull ghcr.io/zero-objects/zerodds-cli:1.0.0-rc.6
```

Hello world publisher (compiles + runs against the published `1.0.0-rc.6`
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

## Quickstart CMake

```cmake
cmake_minimum_required(VERSION 3.16 FATAL_ERROR)

project(
  MyProject
  LANGUAGES C CXX
)

set(CMAKE_CXX_STANDARD 17) # Tested also with CMAKE_CXX_STANDARD == 23
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(FetchContent)
FetchContent_Declare(
  zerodds
  GIT_REPOSITORY https://github.com/zero-objects/zero-dds.git
  GIT_TAG        main # Or any other branch/tag
)
FetchContent_MakeAvailable(zerodds)

set(IDL_SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/idl/test.idl)

zerodds_idlc_generate(TestIdl
  IDL_FILES ${IDL_SOURCES}
  GENERATOR_BACKEND "cpp"
  OUTPUT_DIR ${CMAKE_CURRENT_BINARY_DIR}/include
  BASE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/idl
  LIBRARY_NAMESPACE MyNamespace
)

add_executable(${PROJECT_NAME} main.cpp)
target_link_libraries(${PROJECT_NAME} PUBLIC
    zerodds::zerodds-c    # C API
    zerodds::zerodds-cpp  # CPP API (links to zerodds-c, thus the upper one is optional)
    MyNamespace::TestIdl  # Links compiled IDL files
)

# Copy *.dll into target binary folder to allow direct execution
if(WIN32 AND BUILD_SHARED_LIBS)
    add_custom_command(
        TARGET ${PROJECT_NAME}
        POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different 
            $<TARGET_RUNTIME_DLLS:${PROJECT_NAME}>
            $<TARGET_FILE_DIR:${PROJECT_NAME}>
    )
endif()

```

## Architecture

Nine layers, ~95 crates, single Cargo workspace:

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

**Language Bindings** (Layer 6):

| Language | Crate | Wire |
|---|---|---|
| Rust | `dds-dcps` (native) | XCDR2 |
| C | `zerodds-c-api` (cdylib + staticlib) | XCDR2 |
| C++17 | `zerodds-cpp` (header + ABI) | XCDR2 |
| C# / .NET 8 | `zerodds-cs` (P/Invoke) | XCDR2 |
| Java 21 | `zerodds-java-jni` (JNI) | XCDR2 |
| Python 3.10+ | `zerodds-py` (PyO3) | XCDR2 |
| TypeScript Node | `zerodds-ts-node` (NAPI) | XCDR2 |
| TypeScript Browser | `zerodds-ts-wasm` (WASM) | XCDR2 |
| Flutter / Dart | `zerodds-flutter` (`dart:ffi`) | XCDR2 |

**Protocol Bridges** (Layer 5) — daemons under `target/release/zerodds-*-bridged`:

| Bridge | Spec | Wire |
|---|---|---|
| WebSocket | RFC 6455 + RFC 7692 | DDS-WebSocket-Bridge 1.0 |
| MQTT | OASIS MQTT 5.0 | DDS-MQTT-Bridge 1.0 |
| CoAP | RFC 7252 + 7641 + 7959 | DDS-CoAP-Bridge 1.0 |
| AMQP | OASIS AMQP 1.0 | DDS-AMQP 1.0 |
| gRPC | gRPC Protocol + gRPC-Web | DDS-gRPC-Bridge 1.0 |
| CORBA | OMG CORBA 3.3 GIOP/IIOP | DDS-CORBA-Bridge 1.0 |
| ROS-2 | REP-2007/2008/2009 | DDS-ROS2-Bridge 1.0 |

All bridges share `bridge-security`: TLS 1.3 (rustls 0.23), bearer/JWT-RS256/
mTLS/SASL-PLAIN auth modes, ACL with wildcard and group matching, SIGHUP
cert rotation.

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

- **Wire format**: Discovery packets are byte-identical to Cyclone /
  RTI / Fast DDS — only the `vendor_id` field differs.
- **Cross-vendor discovery**: other vendors do not yet recognise
  ZeroDDS Participants in their Vendor-ID list and treat them as
  "unknown vendor". Byte-accurate interop is nevertheless validated
  in the cross-vendor conformance suite.
- **Production deployments**: cross-vendor setups using ZeroDDS should
  be regarded as **experimental until the Vendor-ID has been
  assigned**.

This section will be updated as soon as the OMG registers the
Vendor-ID. See <https://zerodds.org> for the latest status.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
