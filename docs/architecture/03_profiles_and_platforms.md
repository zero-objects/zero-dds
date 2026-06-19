# Deployment profiles and platform matrix

> **Status:** Draft v0.2
> **Dependencies:** `02_architecture.md`

## 1 The four profiles

Four deployment profiles are produced from a single codebase. Each profile has a clear purpose, its own feature-flag combination and its own platform target set. There are no separate code trees.

### 1.1 Full profile

**Purpose:** development, testing, server deployments, edge gateways, developer workstations.

**Feature flags:**
```
--features std,alloc,tcp,shm,security,xtypes,async-tokio,otel,recording
```

**Included crates:** all.

**Footprint:** 10–30 MB runtime, no upper bound.

**Use:** reference implementation. All features are developed and tested here before they are released into more restrictive profiles.

### 1.2 Standard profile

**Purpose:** production deployments on embedded Linux, RTOS without a safety mandate.

**Feature flags:**
```
--features std,alloc,tcp,shm,security,xtypes,async-tokio
```

**Excluded:** `zerodds-monitor` (OpenTelemetry optional), `zerodds-dashboard`, `zerodds-recorder`, `zerodds-perf`.

**Footprint:** 3–8 MB runtime.

**Use:** ROS 2 nodes, vehicle ECUs without an ASIL mandate, medical devices without Class C, industrial automation.

### 1.3 Safe profile

**Purpose:** safety-qualifiable deployments with target certification per ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+.

**Feature flags:**
```
--features alloc,xtypes,security,safety --no-default-features
```

**Compiler:** exclusively Ferrocene (qualified Rust toolchain).

**Excluded:** all comfort crates, `tokio`, `std` (only `alloc` from the certified core subset).

**Footprint:** 500 KB – 2 MB runtime.

**Built-in constraints:**
- No `panic!`, `.unwrap()`, `.expect()` in release builds (enforced by clippy lints + the custom `deny_panic` lint).
- No dynamic allocation in hot paths; statically allocated memory pools.
- All loops with an upper iteration bound.
- No recursion (except unit-test-verified tail recursion with bounded depth).
- All unsafe blocks with a SAFETY comment and external review.

**Use:** safety-certified automotive components, avionics, medical Class-C devices, railway signaling, nuclear I&C.

### 1.4 Micro profile

**Purpose:** microcontroller-class devices (Cortex-M, RISC-V without MMU, ESP32) that do not have sufficient resources for a full DDS stack.

**Build command (per micro crate):**
```
cargo build -p zerodds-xrce-client --no-default-features --target <thumb-target>
```

No feature flag `no-alloc` — `alloc` is deactivated by omitting the
`alloc` feature (implicitly via `--no-default-features`). The
safe-subset crates (`zerodds-foundation`, `zerodds-cdr`, `zerodds-types`,
`zerodds-xrce-client`) compile in this configuration as `#![no_std]` without
`extern crate alloc;`. The `safety` feature can optionally be switched on
in addition if the safe-profile rule sets are to be enabled.

**Included crates:** only `zerodds-foundation`, `zerodds-cdr`, `zerodds-types`,
`zerodds-xrce-client` (all built without default features).

**Footprint:** 15–30 KB flash, 4–16 KB RAM depending on configuration.

**Protocol:** no full RTPS. Instead DDS-XRCE to an agent on a full/standard-profile node in the same network.

**Use:** IoT edge, wireless sensor networks, smart-factory sensing, vehicle-side ECUs without a safety mandate, education/maker context via PlatformIO.

## 2 Feature-flag matrix

| Feature | Full | Standard | Safe | Micro |
|---|---|---|---|---|
| `std` | ✓ | ✓ | — | — |
| `alloc` | ✓ | ✓ | ✓ | — |
| `safety` (strict lints) | — | — | ✓ | ✓ |
| `xtypes` | ✓ | ✓ | ✓ | light variant |
| `security` | ✓ | ✓ | ✓ | light variant |
| `tcp` | ✓ | ✓ | optional | — |
| `shm` | ✓ | ✓ | optional | — |
| `async-tokio` | ✓ | ✓ | — | — |
| `async-embassy` | — | optional | optional | ✓ |
| `otel` | ✓ | optional | — | — |
| `recording` | ✓ | optional | — | — |
| `xrce-client` | — | — | — | ✓ |
| `xrce-agent` | ✓ | optional | — | — |

## 3 Platform support

The following matrix shows which profile is built and tested on which platform in CI.

### 3.1 Desktop and server class

| Platform | Target triple | Full | Standard | Safe | Micro |
|---|---|---|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | ✓ | ✓ | — | — |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | ✓ | ✓ | — | — |
| Windows x86_64 | `x86_64-pc-windows-msvc` | ✓ | ✓ | — | — |
| macOS ARM64 | `aarch64-apple-darwin` | ✓ | ✓ | — | — |
| macOS x86_64 | `x86_64-apple-darwin` | ✓ | ✓ | — | — |

### 3.2 RTOS and safety class

| Platform | Target triple | Full | Standard | Safe | Micro |
|---|---|---|---|---|---|
| QNX Neutrino 7.1 ARM64 | `aarch64-unknown-nto-qnx710` | — | ✓ | ✓ | — |
| QNX Neutrino 7.1 x86_64 | `x86_64-pc-nto-qnx710` | — | ✓ | ✓ | — |
| VxWorks ARM64 | `aarch64-wrs-vxworks` | — | ✓ | (path-dependent) | — |
| INTEGRITY ARM64 | custom | — | — | ✓ | — |
| PikeOS ARM64 | custom | — | — | ✓ | — |
| Zephyr ARM Cortex-A | `aarch64-unknown-none` | — | ✓ | — | — |

### 3.3 Embedded class (micro profile)

| Platform | Target triple | Typical devices |
|---|---|---|
| ARM Cortex-M33 | `thumbv8m.main-none-eabihf` | STM32L5, NXP LPC55, nRF5340 |
| ARM Cortex-M7 | `thumbv7em-none-eabihf` | STM32H7, i.MX RT |
| ARM Cortex-M4 | `thumbv7em-none-eabihf` | STM32F4, nRF52 |
| ARM Cortex-M0/M0+ | `thumbv6m-none-eabi` | RP2040, STM32F0 |
| ARM Cortex-R5/R52 | `armv7r-none-eabihf`, `armv8r-none-eabihf` | Automotive safety MCUs |
| Xtensa ESP32 | `xtensa-esp32-none-elf` | ESP32, ESP32-S3 |
| RISC-V ESP32 | `riscv32imc-esp-espidf` | ESP32-C3, ESP32-C6 |
| RISC-V bare-metal | `riscv32imac-unknown-none-elf` | Various RISC-V MCUs |

## 4 Binding availability by platform

Not all bindings make sense on all platforms. The following matrix gives orientation:

| Platform | Rust | C | C++ | C# | Java | Python |
|---|---|---|---|---|---|---|
| Linux x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Linux ARM64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Windows x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| macOS ARM64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| macOS x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| QNX ARM64 | ✓ | ✓ | ✓ | — | — | evaluate |
| QNX x86_64 | ✓ | ✓ | ✓ | — | — | evaluate |
| VxWorks ARM64 | ✓ | ✓ | ✓ | — | — | — |
| INTEGRITY, PikeOS | ✓ | ✓ | ✓ | — | — | — |
| Zephyr | ✓ | ✓ | ✓ | — | — | — |
| FreeRTOS (Cortex-M) | ✓ | ✓ | ✓ | — | — | — |
| ESP-IDF (ESP32) | ✓ | ✓ | ✓ | — | — | — |
| STM32Cube (Cortex-M) | ✓ | ✓ | ✓ | — | — | — |
| Bare metal Cortex-M | ✓ | ✓ | ✓ | — | — | — |

**Notes:**
- Java bindings on RTOS are theoretically possible via Eclipse OpenJ9 or Azul Zing, but are not prioritized in our initial scope.
- C# via .NET NativeAOT on QNX is technically possible, but currently not officially supported by Microsoft. Evaluation in Phase 4.
- Python on embedded is typically only sensible on platforms with MicroPython or CPython on a Linux-based OS; not included in the micro profile.

## 5 Build and release matrix

The CI workflow builds the following matrix. Each row is a separate CI job.

| Profile | Target | Compiler | Release artifact |
|---|---|---|---|
| Full | x86_64-linux | stable Rust | `.deb`, `.rpm`, `.tar.gz` |
| Full | aarch64-linux | stable Rust | `.deb`, `.rpm`, `.tar.gz` |
| Full | x86_64-windows | stable Rust | `.msi`, `.zip` |
| Full | aarch64-apple | stable Rust | `.pkg`, `.tar.gz` |
| Standard | aarch64-linux | stable Rust | `.tar.gz` |
| Standard | aarch64-qnx710 | stable Rust | `.tar.gz` |
| Safe | aarch64-qnx710 | Ferrocene | `.tar.gz` + safety manual |
| Safe | aarch64-integrity | Ferrocene | `.tar.gz` + safety manual |
| Micro | thumbv7em-eabihf | stable Rust | static lib + `library.json` |
| Micro | thumbv8m.main-eabihf | stable Rust | static lib + `library.json` |
| Micro | xtensa-esp32-elf | stable Rust + espup | static lib + `library.json` |
| Micro | riscv32imc-esp-espidf | stable Rust + espup | static lib + `library.json` |

Bindings are built separately for each matching host profile:
- Python wheels: `manylinux_2_27`, `macOS universal`, `win_amd64`
- NuGet packages for C# bindings: runtime identifiers `linux-x64`, `linux-arm64`, `win-x64`, `osx-arm64`
- Maven artifacts for Java bindings: `zerodds-java-{version}-{os}-{arch}.jar`

## 6 PlatformIO integration for the micro profile

The micro profile is published as a PlatformIO library:

**Repository structure:**
```
zerodds-xrce-platformio/
├── library.json            # PlatformIO manifest
├── library.properties      # Arduino compatibility
├── src/                    # C++ public API headers + wrapper
├── include/                # C headers
├── lib/                    # pre-compiled static libs per target
│   ├── arm_cortex_m4/
│   ├── arm_cortex_m33/
│   ├── esp32/
│   ├── esp32c3/
│   └── ...
├── examples/               # one example per framework
│   ├── arduino-esp32-publisher/
│   ├── esp-idf-subscriber/
│   ├── stm32cube-cortex-m33/
│   ├── zephyr-nrf5340/
│   └── freertos-stm32h7/
└── CHANGELOG.md
```

**`library.json` core part:**
```json
{
  "name": "zerodds-xrce",
  "version": "1.0.0",
  "frameworks": ["arduino", "espidf", "stm32cube", "zephyr", "mbed"],
  "platforms": ["espressif32", "ststm32", "nordicnrf52", "nordicnrf53", "raspberrypi"],
  "build": {
    "libArchive": false
  }
}
```

The static libraries are generated per release from our CI. One CI job per target triple, artifacts are bundled into a release distribution that is PlatformIO-registry-conformant.

## 7 Profile compatibility and interop

Within the DDS network, nodes of different profiles must be able to talk to each other:

- **Full ↔ Full, Standard ↔ Standard, Full ↔ Standard:** full RTPS interop, all QoS combinations.
- **Full/Standard ↔ Safe:** full RTPS interop, but a safe node can reject QoS policies that would violate its compliance guarantees (e.g. `HISTORY = KEEP_ALL` with unbounded size).
- **Full/Standard ↔ Micro:** no direct RTPS; communication runs through the XRCE agent in the full/standard node. The agent represents the micro client as a full-fledged DDS participant in the RTPS network.
- **Safe ↔ Micro:** only via a standard/full agent as an intermediate layer. Direct Safe-to-Micro XRCE is not planned (safety certification of an XRCE agent is a separate effort).

## 8 Profile decision flowchart

Users should select their suitable profile based on the following questions:

1. **Does the code run on an MCU class (RAM < 1 MB, flash < 2 MB)?** → Micro profile.
2. **Is there a formal safety-certification requirement (ISO 26262, DO-178C, IEC 61508)?** → Safe profile.
3. **Is the target platform an embedded Linux or RTOS without a formal safety requirement?** → Standard profile.
4. **Other cases (desktop, server, developer workstation, edge gateway):** → Full profile.

Multiple assignment is possible: a node can be Standard and additionally manage an XRCE agent (i.e. micro counterparts) in the process.
