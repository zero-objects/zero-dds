# TS-6 — Platform-Matrix (macOS / ARM64 / Windows)

Stand 2026-05-02. **Status: extern blockiert** (Hardware-Runner für
Plattformen außer x86_64-linux).

## Ziel

Build + Test der Workspace auf allen Tier-1-Plattformen, nicht nur
Linux-x86_64. Drei Stufen, die unabhängig blockierbar/freischaltbar sind:

1. **aarch64-unknown-linux-gnu** (Cross-Compile von x86_64-Runner)
2. **x86_64-apple-darwin** + **aarch64-apple-darwin** (macOS-Runner)
3. **x86_64-pc-windows-msvc** (Windows-Runner)

## Stufe 1: aarch64-Linux-Cross-Compile

**Status: ✅ done 2026-05-02** — CI-Image + Job + Linker-Konfig live.

### Voraussetzung

CI-Image (`ci/Dockerfile.rust`) braucht:

```dockerfile
# Cross-Compile-Toolchain fuer aarch64-linux-gnu
RUN apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
    && rustup target add aarch64-unknown-linux-gnu \
    && mkdir -p /root/.cargo \
    && echo '[target.aarch64-unknown-linux-gnu]' >> /root/.cargo/config.toml \
    && echo 'linker = "aarch64-linux-gnu-gcc"'   >> /root/.cargo/config.toml
```

### CI-Job

```yaml
build-aarch64-linux:
  stage: build
  needs: [fmt, clippy]
  <<: *cargo-target-cache
  rules:
    - if: '$CI_COMMIT_BRANCH == "main"'
    - if: '$CI_PIPELINE_SOURCE == "merge_request_event"'
  script:
    - cargo build --workspace --target aarch64-unknown-linux-gnu
    # Tests laufen nicht direkt — qemu-static brauchts:
    # - apt install qemu-user-static && cargo test --target aarch64-... \
    #     --workspace -- --test-threads=1
```

### Bekannte Risiken

* **`ring`** macht plattform-spezifische ASM-Optimierung — sollte aber
  aarch64 supporten.
* **dependency-DLLs** falls Crates auf libfastrtps verweisen — wir
  haben das nur in `tests/interop/fastdds_pub.cpp`, nicht im Build-Pfad.

## Stufe 2: macOS-Runner

**Status: extern blockiert** (Hardware).

### Optionen

1. **GitLab Shared Runners** mit macOS — kostet $0.04/min,
   monthly-Budget zu definieren.
2. **Eigene Mac-Mini** als Self-Hosted-Runner, registriert mit Tag
   `macos`.
3. **MacStadium / AWS-Mac** (Cloud-managed, ~$70/Monat).

Empfehlung: Option 1 für PoC, später Option 2 wenn Budget knapp.

### CI-Job-Skelett

```yaml
build-macos-x86_64:
  stage: build
  tags: [macos, x86_64]
  rules:
    - if: '$MACOS_RUNNER_AVAILABLE == "true"'
      when: on_success
    - when: never
  script:
    - cargo build --workspace --target x86_64-apple-darwin

build-macos-arm64:
  stage: build
  tags: [macos, arm64]
  rules:
    - if: '$MACOS_RUNNER_AVAILABLE == "true"'
      when: on_success
    - when: never
  script:
    - cargo build --workspace --target aarch64-apple-darwin
```

### Bekannte Risiken

* **Multicast-Tests** auf macOS: Loopback-Multicast ist unzuverlässig
  (siehe Memory `pve_multicast_setup` und Lifespan-Test-Flakiness).
  Auf macOS-Runner würden DCPS-Integration-Tests vermutlich
  permanent flaken — daher start nur mit unit + cdr-tests, dcps
  ignored.

## Stufe 3: Windows-Runner

**Status: extern blockiert** (Hardware + Toolchain).

### CI-Job-Skelett

```yaml
build-windows-msvc:
  stage: build
  tags: [windows, msvc]
  rules:
    - if: '$WINDOWS_RUNNER_AVAILABLE == "true"'
      when: on_success
    - when: never
  script:
    - cargo build --workspace --target x86_64-pc-windows-msvc
```

### Bekannte Risiken

* **`rustls`** mit Windows-CryptoAPI vs ring-default — Build-Plattform-
  Mismatch kann zu Linker-Failures führen.
* **`tcpdump`-Wire-Captures** funktionieren auf Windows nicht direkt;
  Wireshark-CLI (tshark.exe) als Alternative.

## Folgeschritte (in Reihenfolge)

* [ ] Stufe 1 — aarch64-cross: CI-Image-Update + neuer Job (frei,
      blockiert nur durch Image-Build-Schedule)
* [ ] Stufe 2a — macOS-x86_64-Runner (GitLab-Shared oder Self-Hosted)
* [ ] Stufe 2b — macOS-arm64-Runner
* [ ] Stufe 3 — Windows-msvc-Runner

Stufe 1 könnte heute angegangen werden (only Image-Update); Stufe 2/3
brauchen Hardware/Budget-Entscheidung.
