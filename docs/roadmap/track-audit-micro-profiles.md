# Track RC2-E — Micro-Profile Audit

**Goal:** alle "minimal-target"-Build-Profile (no_std, no_alloc, embedded
Cortex-M, WASM-server, MCU) werden tatsächlich gebaut und getestet, mit
CI-Build-Targets eingebaut. Die Behauptung "ZeroDDS läuft auch auf MCUs"
muss reproduzierbar sein.

**Status:** 📋 todo

**Estimate:** 2 Personenwochen.

## Profile im Audit-Scope

### no_std + alloc (must-have)

Crates die `no_std` + `alloc` claimen müssen einen `cargo check
--no-default-features --features alloc --target <foo>`-Pass haben.
Listet sich auf:

| Crate | claimt no_std+alloc | echt geprüft? |
|---|---|---|
| `dds-foundation` | ✅ | tbd |
| `cdr` | ✅ | tbd |
| `idl` | ✅ | tbd |
| `qos` | ✅ | tbd |
| `types` | ✅ | tbd |
| `rtps` | ✅ | tbd |
| `dcps` (alloc-Mode) | ✅ | tbd |
| `xrce` | ✅ | tbd |
| `xrce-client` | ✅ | tbd |
| `flatdata` | ✅ | tbd |
| `corba-giop` | ✅ | tbd |
| `corba-ior` | ✅ | tbd |
| ~ 20 weitere | ... | ... |

CI-Job: cross-target build matrix
- `riscv32imac-unknown-none-elf`
- `thumbv7em-none-eabihf` (Cortex-M4F)
- `aarch64-unknown-none-softfloat`

### no_std + no_alloc (stretch)

XRCE-Client muss auf einem Cortex-M3 mit 32 KB RAM laufen können. Das
heißt:
- Keine `alloc`
- Stack-allocated Buffers (PoolBuffer<CAP> existiert schon)
- Kompilierung mit `-C panic=abort -C strip=symbols -C opt-level=z`

Ziel: `xrce-client` als bare-metal-binary ≤ 64 KB Flash, ≤ 16 KB RAM
auf STM32F103 (Bluepill-class).

### WASM (browser + server)

- `ts-wasm` (browser): muss in einem `wasmtime --headless` SmokeTest laufen
- `ts-wasm` als server-WASM (WASI): noch nicht claimed, evaluation wert

### Real-Time Linux (PREEMPT_RT)

- Kein eigenes Profile, aber QoS-relevante Tests müssen unter PREEMPT_RT-
  Kernel grün sein
- Setup: PREEMPT_RT-VM (Debian 12 + linux-image-rt), `chrt -f 80` für
  bridge-daemons

### AUTOSAR Classic (long-shot, post-1.0)

- Nicht für RC2 — als Backlog-Item

## Audit-Aktionen

### Per Crate

1. `cargo check --no-default-features --features alloc --target
   thumbv7em-none-eabihf` ausführen, jeden Compile-Error fixen
2. `cargo check --no-default-features --target riscv32imac-unknown-none-elf`
3. Wenn Test-only-Code rein-leakt: `#[cfg(feature = "std")]`-Gates ergänzen
4. Crate-README ergänzt um "Tested targets:" Zeile

### CI-Integration

- `.github/workflows/cross-build.yml` mit Matrix
- Ziel-Liste in `tools/cross-targets.toml` (wenn das nicht im CI-Track-
  Agent territory ist — koordinieren)

### Spec-Coverage

- `docs/spec-coverage/zerodds-no_std-profile-1.0.md` neu (Vendor-Spec für
  unsere Profile)
- pro Crate-Spec ergänzen: "Profile: no_std + alloc / no_std no_alloc /
  std-only"

### Demo / Smoke-Test

- Bare-metal Cortex-M Demo: STM32F4-Discovery-Board (oder QEMU-Emulator
  wenn HW nicht verfügbar) führt einen XRCE-Client aus, sendet einen
  Sensor-Wert via UART → XRCE-Agent → DDS-Subscriber empfängt
- Empfehlung: QEMU-mps2-an386 als CI-runable Target (kein HW nötig),
  STM32F4 als optional manuell-Test

## Acceptance

1. ≥ 20 Crates haben passing `cargo check` für 3 cross-targets
2. xrce-client baut bare-metal Cortex-M, ≤ 64 KB Flash
3. CI-Workflow cross-build.yml grün für alle 3 Targets, läuft on PR
4. QEMU-Smoke-Test: bare-metal-XRCE-Client → Linux-XRCE-Agent → DDS-Sub
5. PREEMPT_RT-VM-Test: bridge-daemon hält Latenz-Budget unter Last
6. `zerodds-no_std-profile-1.0.md` published

## Dependencies

- rustup-Targets installiert (CI runner)
- Optional: STM32F4-Discovery-HW oder QEMU-mps2-an386
- PREEMPT_RT-VM auf nr3 (LXC oder dedizierte VM)

## Risks

- **alloc-leak via tonic/serde**: einige transitive deps brauchen std,
  müssen feature-gated werden
- **Drift**: ohne CI bricht das ständig wieder. Mitigation: cross-build
  als Pflicht-CI-Job, blocking on PR
