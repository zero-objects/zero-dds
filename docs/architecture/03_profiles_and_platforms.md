# Deployment-Profile und Plattform-Matrix

> **Status:** Draft v0.2
> **Abhängigkeiten:** `02_architecture.md`

## 1 Die vier Profile

Aus einer einzigen Codebasis werden vier Deployment-Profile produziert. Jedes Profil hat einen klaren Zweck, eine eigene Feature-Flag-Kombination und eine eigene Plattform-Zielmenge. Es gibt keine separaten Code-Bäume.

### 1.1 Full Profile

**Zweck:** Entwicklung, Testing, Server-Deployments, Edge-Gateways, Developer-Workstations.

**Feature-Flags:**
```
--features std,alloc,tcp,shm,security,xtypes,async-tokio,otel,recording
```

**Included Crates:** Alle.

**Footprint:** 10–30 MB Runtime, keine Obergrenze.

**Verwendung:** Referenz-Implementierung. Hier werden alle Features entwickelt und getestet, bevor sie in restriktiveren Profilen freigegeben werden.

### 1.2 Standard Profile

**Zweck:** Produktions-Deployments auf Embedded Linux, RTOS ohne Safety-Zwang.

**Feature-Flags:**
```
--features std,alloc,tcp,shm,security,xtypes,async-tokio
```

**Excluded:** `zerodds-monitor` (OpenTelemetry optional), `zerodds-dashboard`, `zerodds-recorder`, `zerodds-perf`.

**Footprint:** 3–8 MB Runtime.

**Verwendung:** ROS-2-Nodes, Fahrzeug-Steuergeräte ohne ASIL-Zwang, Medizintechnik ohne Class-C, industrielle Automatisierung.

### 1.3 Safe Profile

**Zweck:** Safety-qualifizierbare Deployments mit Ziel-Zertifizierung nach ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+.

**Feature-Flags:**
```
--features alloc,xtypes,security,safety --no-default-features
```

**Compiler:** Ausschließlich Ferrocene (qualified Rust toolchain).

**Excluded:** Alle Comfort-Crates, `tokio`, `std` (nur `alloc` von zertifizierter Core-Subset).

**Footprint:** 500 KB – 2 MB Runtime.

**Built-in Constraints:**
- Kein `panic!`, `.unwrap()`, `.expect()` in Release-Builds (durch Clippy-Lints + custom `deny_panic` Lint durchgesetzt).
- Keine dynamische Allocation in Hot Paths; statisch allozierte Memory-Pools.
- Alle Loops mit oberer Iterations-Schranke.
- Keine Rekursion (außer durch Unit-Test verifizierte Tail-Rekursion mit bounded depth).
- Alle Unsafe-Blocks mit SAFETY-Kommentar und externem Review.

**Verwendung:** Safety-zertifizierte Automotive-Komponenten, Avionik, medizinische Class-C-Geräte, Bahn-Signaltechnik, Kernkraft-I&C.

### 1.4 Micro Profile

**Zweck:** Mikrocontroller-Klasse-Geräte (Cortex-M, RISC-V ohne MMU, ESP32), die nicht ausreichend Ressourcen für einen vollen DDS-Stack haben.

**Build-Kommando (pro Micro-Crate):**
```
cargo build -p zerodds-xrce-client --no-default-features --target <thumb-target>
```

Kein Feature-Flag `no-alloc` — `alloc` wird durch Weglassen des
`alloc`-Features (implizit ueber `--no-default-features`) deaktiviert. Die
Safe-Subset-Crates (`zerodds-foundation`, `zerodds-cdr`, `zerodds-types`,
`zerodds-xrce-client`) kompilieren in dieser Konfiguration `#![no_std]` ohne
`extern crate alloc;`. Das Feature `safety` kann optional dazu geschaltet
werden, wenn Safe-Profile-Regel-Sets aktiviert werden sollen.

**Included Crates:** Nur `zerodds-foundation`, `zerodds-cdr`, `zerodds-types`,
`zerodds-xrce-client` (alle ohne Default-Features gebaut).

**Footprint:** 15–30 KB Flash, 4–16 KB RAM je nach Konfiguration.

**Protokoll:** Kein volles RTPS. Stattdessen DDS-XRCE zu einem Agent auf einem Full/Standard-Profile-Node im selben Netzwerk.

**Verwendung:** IoT-Edge, drahtlose Sensornetze, Smart-Factory-Sensorik, Fahrzeug-Seiten-ECUs ohne Safety-Zwang, Bildungs-/Maker-Kontext via PlatformIO.

## 2 Feature-Flag-Matrix

| Feature | Full | Standard | Safe | Micro |
|---|---|---|---|---|
| `std` | ✓ | ✓ | — | — |
| `alloc` | ✓ | ✓ | ✓ | — |
| `safety` (lints strikt) | — | — | ✓ | ✓ |
| `xtypes` | ✓ | ✓ | ✓ | Light-Variante |
| `security` | ✓ | ✓ | ✓ | Light-Variante |
| `tcp` | ✓ | ✓ | optional | — |
| `shm` | ✓ | ✓ | optional | — |
| `async-tokio` | ✓ | ✓ | — | — |
| `async-embassy` | — | optional | optional | ✓ |
| `otel` | ✓ | optional | — | — |
| `recording` | ✓ | optional | — | — |
| `xrce-client` | — | — | — | ✓ |
| `xrce-agent` | ✓ | optional | — | — |

## 3 Plattform-Unterstützung

Die folgende Matrix zeigt, welches Profil auf welcher Plattform in CI gebaut und getestet wird.

### 3.1 Desktop- und Server-Klasse

| Plattform | Target-Triple | Full | Standard | Safe | Micro |
|---|---|---|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | ✓ | ✓ | — | — |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | ✓ | ✓ | — | — |
| Windows x86_64 | `x86_64-pc-windows-msvc` | ✓ | ✓ | — | — |
| macOS ARM64 | `aarch64-apple-darwin` | ✓ | ✓ | — | — |
| macOS x86_64 | `x86_64-apple-darwin` | ✓ | ✓ | — | — |

### 3.2 RTOS- und Safety-Klasse

| Plattform | Target-Triple | Full | Standard | Safe | Micro |
|---|---|---|---|---|---|
| QNX Neutrino 7.1 ARM64 | `aarch64-unknown-nto-qnx710` | — | ✓ | ✓ | — |
| QNX Neutrino 7.1 x86_64 | `x86_64-pc-nto-qnx710` | — | ✓ | ✓ | — |
| VxWorks ARM64 | `aarch64-wrs-vxworks` | — | ✓ | (Pfad-abhängig) | — |
| INTEGRITY ARM64 | custom | — | — | ✓ | — |
| PikeOS ARM64 | custom | — | — | ✓ | — |
| Zephyr ARM Cortex-A | `aarch64-unknown-none` | — | ✓ | — | — |

### 3.3 Embedded Klasse (Micro Profile)

| Plattform | Target-Triple | Typische Devices |
|---|---|---|
| ARM Cortex-M33 | `thumbv8m.main-none-eabihf` | STM32L5, NXP LPC55, nRF5340 |
| ARM Cortex-M7 | `thumbv7em-none-eabihf` | STM32H7, i.MX RT |
| ARM Cortex-M4 | `thumbv7em-none-eabihf` | STM32F4, nRF52 |
| ARM Cortex-M0/M0+ | `thumbv6m-none-eabi` | RP2040, STM32F0 |
| ARM Cortex-R5/R52 | `armv7r-none-eabihf`, `armv8r-none-eabihf` | Automotive Safety MCUs |
| Xtensa ESP32 | `xtensa-esp32-none-elf` | ESP32, ESP32-S3 |
| RISC-V ESP32 | `riscv32imc-esp-espidf` | ESP32-C3, ESP32-C6 |
| RISC-V bare-metal | `riscv32imac-unknown-none-elf` | Diverse RISC-V MCUs |

## 4 Binding-Verfügbarkeit nach Plattform

Nicht alle Bindings sind auf allen Plattformen sinnvoll. Die folgende Matrix gibt Orientierung:

| Plattform | Rust | C | C++ | C# | Java | Python |
|---|---|---|---|---|---|---|
| Linux x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Linux ARM64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Windows x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| macOS ARM64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| macOS x86_64 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| QNX ARM64 | ✓ | ✓ | ✓ | — | — | evaluieren |
| QNX x86_64 | ✓ | ✓ | ✓ | — | — | evaluieren |
| VxWorks ARM64 | ✓ | ✓ | ✓ | — | — | — |
| INTEGRITY, PikeOS | ✓ | ✓ | ✓ | — | — | — |
| Zephyr | ✓ | ✓ | ✓ | — | — | — |
| FreeRTOS (Cortex-M) | ✓ | ✓ | ✓ | — | — | — |
| ESP-IDF (ESP32) | ✓ | ✓ | ✓ | — | — | — |
| STM32Cube (Cortex-M) | ✓ | ✓ | ✓ | — | — | — |
| Bare metal Cortex-M | ✓ | ✓ | ✓ | — | — | — |

**Anmerkungen:**
- Java-Bindings auf RTOS sind theoretisch über Eclipse OpenJ9 oder Azul Zing möglich, sind aber in unserem initialen Scope nicht priorisiert.
- C# über .NET NativeAOT auf QNX ist technisch möglich, aber derzeit nicht von Microsoft offiziell unterstützt. Evaluation in Phase 4.
- Python auf Embedded ist typisch nur auf Plattformen mit MicroPython oder CPython auf Linux-basiertem OS sinnvoll; nicht in Micro-Profile enthalten.

## 5 Build- und Release-Matrix

Der CI-Workflow baut die folgende Matrix. Jede Zeile ist ein separater CI-Job.

| Profile | Target | Compiler | Release-Artefakt |
|---|---|---|---|
| Full | x86_64-linux | stable Rust | `.deb`, `.rpm`, `.tar.gz` |
| Full | aarch64-linux | stable Rust | `.deb`, `.rpm`, `.tar.gz` |
| Full | x86_64-windows | stable Rust | `.msi`, `.zip` |
| Full | aarch64-apple | stable Rust | `.pkg`, `.tar.gz` |
| Standard | aarch64-linux | stable Rust | `.tar.gz` |
| Standard | aarch64-qnx710 | stable Rust | `.tar.gz` |
| Safe | aarch64-qnx710 | Ferrocene | `.tar.gz` + Safety-Manual |
| Safe | aarch64-integrity | Ferrocene | `.tar.gz` + Safety-Manual |
| Micro | thumbv7em-eabihf | stable Rust | static lib + `library.json` |
| Micro | thumbv8m.main-eabihf | stable Rust | static lib + `library.json` |
| Micro | xtensa-esp32-elf | stable Rust + espup | static lib + `library.json` |
| Micro | riscv32imc-esp-espidf | stable Rust + espup | static lib + `library.json` |

Bindings werden für jedes passende Host-Profile separat gebaut:
- Python-Wheels: `manylinux_2_27`, `macOS universal`, `win_amd64`
- NuGet-Packages für C#-Bindings: Runtime-Identifiers `linux-x64`, `linux-arm64`, `win-x64`, `osx-arm64`
- Maven-Artifacts für Java-Bindings: `zerodds-java-{version}-{os}-{arch}.jar`

## 6 PlatformIO-Integration für Micro Profile

Das Micro-Profile wird als PlatformIO-Library publiziert:

**Repository-Struktur:**
```
zerodds-xrce-platformio/
├── library.json            # PlatformIO-Manifest
├── library.properties      # Arduino-Kompatibilität
├── src/                    # C++ Public API Headers + Wrapper
├── include/                # C Headers
├── lib/                    # pre-compiled static libs per target
│   ├── arm_cortex_m4/
│   ├── arm_cortex_m33/
│   ├── esp32/
│   ├── esp32c3/
│   └── ...
├── examples/               # pro Framework ein Beispiel
│   ├── arduino-esp32-publisher/
│   ├── esp-idf-subscriber/
│   ├── stm32cube-cortex-m33/
│   ├── zephyr-nrf5340/
│   └── freertos-stm32h7/
└── CHANGELOG.md
```

**`library.json` Kernteil:**
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

Die statischen Libraries werden pro Release aus unserer CI generiert. Ein CI-Job pro Target-Triple, Artifacts werden zu einer Release-Distribution gebündelt, die PlatformIO-Registry-konform ist.

## 7 Profile-Kompatibilität und Interop

Innerhalb des DDS-Netzwerks müssen Nodes verschiedener Profile miteinander reden können:

- **Full ↔ Full, Standard ↔ Standard, Full ↔ Standard:** volle RTPS-Interop, alle QoS-Kombinationen.
- **Full/Standard ↔ Safe:** volle RTPS-Interop, aber Safe-Node kann QoS-Policies ablehnen, die seine Compliance-Garantien verletzen würden (z.B. `HISTORY = KEEP_ALL` mit unbeschränkter Größe).
- **Full/Standard ↔ Micro:** kein direkter RTPS; Kommunikation läuft über XRCE-Agent im Full/Standard-Node. Der Agent repräsentiert den Micro-Client als vollwertigen DDS-Participant im RTPS-Netzwerk.
- **Safe ↔ Micro:** nur über einen Standard/Full-Agent als Zwischenschicht. Direct Safe-to-Micro-XRCE ist nicht geplant (Safety-Zertifizierung eines XRCE-Agents ist separater Aufwand).

## 8 Profile-Entscheidungs-Flowchart

Anwender sollen anhand der folgenden Fragen ihr passendes Profil auswählen:

1. **Läuft der Code auf einer MCU-Klasse (RAM < 1 MB, Flash < 2 MB)?** → Micro Profile.
2. **Besteht eine formale Safety-Zertifizierungs-Anforderung (ISO 26262, DO-178C, IEC 61508)?** → Safe Profile.
3. **Ist die Ziel-Plattform ein Embedded-Linux oder RTOS ohne formale Safety-Anforderung?** → Standard Profile.
4. **Sonstige Fälle (Desktop, Server, Developer-Workstation, Edge-Gateway):** → Full Profile.

Mehrfachzuordnung ist möglich: ein Node kann Standard sein und im Prozess zusätzlich einen XRCE-Agent (also Micro-Gegenstücke) verwalten.
