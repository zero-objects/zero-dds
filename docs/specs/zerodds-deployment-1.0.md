# `zerodds-deployment` v1.0 — Linux/macOS/Windows Packaging-Konventionen

ZeroDDS Vendor-Spec. Spezifiziert die Packaging-, Installations- und
Deployment-Konventionen für sämtliche ZeroDDS-Binaries und die
shared-Library `libzerodds`.

## Motivation

ZeroDDS liefert sieben Bridge-Daemons, eine ABI-Library und siebzehn
CLI-Tools. Ohne einheitliche Packaging-Konvention werden
Operations-Teams pro Plattform unterschiedlich beliefert; Patch-
Rollouts dauern länger, Config-Pfade differenzieren, Logs landen an
unerwarteten Orten.

Diese Spec normiert pro Plattform:
- Wo Binaries leben.
- Wo Configs leben.
- Wo Logs leben.
- Welche Service-Manager-Integration gilt (systemd / launchd / Win-
  Service).
- Welcher Installer-Mechanismus die Pflicht-Distribution ist.
- Welche Container-Images auf der Registry liegen.

Diese Spec wird referenziert von §11 jeder Bridge-Spec
(`zerodds-ws-bridge-1.0`, `zerodds-mqtt-bridge-1.0`, etc.) und von
§6 `zerodds-ffi-loader-1.0`.

## §1 Targets

ZeroDDS-Releases umfassen drei Klassen von Artefakten:

### §1.1 Daemons (7 Bridges)

| Daemon | Spec-Cross-Ref |
|--------|----------------|
| `zerodds-ws-bridged` | `zerodds-ws-bridge-1.0` §11 |
| `zerodds-mqtt-bridged` | `zerodds-mqtt-bridge-1.0` §11 |
| `zerodds-coap-bridged` | `zerodds-coap-bridge-1.0` §11 |
| `zerodds-amqp-bridged` | `zerodds-amqp-bridge-daemon-1.0` §11 |
| `zerodds-grpc-bridged` | `zerodds-grpc-bridge-1.0` §11 |
| `zerodds-corba-bridged` | `zerodds-corba-bridge-1.0` §11 |
| `zerodds-ros2-shim` | `zerodds-ros2-bridge-1.0` §11 (Diagnose-Tool, kein klassischer Daemon) |

### §1.2 Library

| Artefakt | Spec-Cross-Ref |
|----------|----------------|
| `libzerodds.so` / `.dylib` / `.dll` + `zerodds.h` | `zerodds-ffi-loader-1.0` §6 |

### §1.3 CLI-Tools (17)

```
zerodds-admin                   # Admin-CLI (Domain-Inspect, QoS-Validate/Check, Discovery-Snapshot)
zerodds-bench                   # Benchmark-Suite (latency/throughput/loss)
zerodds-bench-suite             # Multi-Scenario-Bench-Runner
zerodds-idlc                    # IDL-Compiler → Rust/C++/Java/C#/Python/TS Codegen
zerodds-spy                     # Topic-Spy (live-Sample-Dump)
zerodds-record                  # Recorder (zddsrec-Format)
zerodds-replay                  # Replay aus zddsrec
zerodds-snitch                  # Discovery-Probe (SPDP/SEDP-Dumper)
zerodds-monitor                 # Monitor-CLI (`zerodds-monitor-1.1`)
zerodds-mq                      # Multi-Domain-Bridge (DDS↔DDS)
zerodds-pcap                    # PCAP-Dumper für RTPS-Wire
zerodds-perf                    # Performance-Probe
zerodds-ros2-shim               # ROS-2-Diagnose
zerodds-secure-keygen           # DDS-Security Cert/Key-Erzeugung
zerodds-secure-permissions      # Permissions/Governance-XML-Tool
zerodds-typeobject              # TypeObject-Inspect
zerodds-xmlc                    # DDS-XML-Config-Validator/Renderer
```

## §2 Linux

### §2.1 Datei-Layout (FHS)

```
/usr/bin/zerodds-*                       # Binaries (alle 7 Daemons + 17 CLIs)
/usr/lib/libzerodds.so                   # ABI-Library
/usr/lib/libzerodds.so.1                 # Major-Symlink
/usr/lib/libzerodds.so.1.0.0             # Versioned-Symlink
/usr/lib/pkgconfig/zerodds.pc            # pkg-config
/usr/include/zerodds.h                   # C-Header
/usr/include/zerodds/                    # C++-Headers
/usr/lib/systemd/system/zerodds-*.service
/etc/zerodds/                            # Config-Default
  ├── ws-bridged.yaml
  ├── mqtt-bridged.yaml
  ├── coap-bridged.yaml
  ├── amqp-bridged.yaml
  ├── grpc-bridged.yaml
  ├── corba-bridged.yaml
  └── certs/
/var/log/zerodds/                        # Log-Default
/var/lib/zerodds/                        # State (IORs, recorder-files)
/usr/share/man/man1/zerodds-*.1.gz       # Manuals
/usr/share/man/man5/zerodds-*.yaml.5.gz
/usr/share/doc/zerodds/                  # CHANGELOG, LICENSE, README
/usr/share/bash-completion/completions/  # bash/zsh/fish completions
```

### §2.2 Distributions

#### §2.2.1 Debian/Ubuntu (`.deb`)

Pakete:
- `zerodds-core` — `libzerodds`, headers, manuals (Pflicht-Dep)
- `zerodds-ws-bridge` — `zerodds-ws-bridged` + service
- `zerodds-mqtt-bridge` — analog
- `zerodds-coap-bridge`, `-amqp-bridge`, `-grpc-bridge`, `-corba-bridge`, `-ros2`
- `zerodds-cli` — alle 17 CLI-Tools
- `zerodds-dev` — `zerodds.h`, pkg-config, Static-Libs

Repo: `deb https://packages.zerodds.io/apt stable main`. Signed mit
Release-GPG-Key (`/etc/apt/trusted.gpg.d/zerodds.gpg`).

```bash
curl -fsSL https://packages.zerodds.io/apt/zerodds.gpg | sudo tee /etc/apt/trusted.gpg.d/zerodds.gpg > /dev/null
echo "deb https://packages.zerodds.io/apt stable main" | sudo tee /etc/apt/sources.list.d/zerodds.list
sudo apt update
sudo apt install zerodds-ws-bridge
```

#### §2.2.2 RHEL/Fedora (`.rpm`)

Analoges Paket-Set unter `dnf install`. Repo `https://packages.zerodds.io/yum/`.
GPG-Key in `/etc/pki/rpm-gpg/RPM-GPG-KEY-zerodds`.

#### §2.2.3 Arch Linux (PKGBUILD)

AUR-Pakete `zerodds-core`, `zerodds-ws-bridge`, etc. Pacman-Hook
für Service-Reload bei Update.

#### §2.2.4 AppImage (Static-Build)

Pro Daemon ein eigenes AppImage:
- `zerodds-ws-bridged-1.0.0-x86_64.AppImage`
- `zerodds-mqtt-bridged-1.0.0-x86_64.AppImage`
- ...

Statisch gegen musl gelinkt für Distro-Unabhängigkeit. Hosting auf
GitHub-Releases + S3-Mirror.

### §2.3 systemd-Unit (Beispiel)

```ini
# /usr/lib/systemd/system/zerodds-ws-bridged.service
[Unit]
Description=ZeroDDS WebSocket-Bridge
Documentation=man:zerodds-ws-bridged(1)
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart=/usr/bin/zerodds-ws-bridged --config /etc/zerodds/ws-bridged.yaml
ExecReload=/bin/kill -HUP $MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=30
Restart=on-failure
RestartSec=5
User=zerodds
Group=zerodds
StandardOutput=journal
StandardError=journal
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/zerodds /var/lib/zerodds
PrivateTmp=true
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

System-User `zerodds:zerodds` wird vom Postinst-Script per `useradd
--system` angelegt.

## §3 macOS

### §3.1 Datei-Layout

```
/usr/local/bin/zerodds-*                       # Binaries (Apple-Silicon-Universal)
/usr/local/lib/libzerodds.dylib                # ABI-Library
/usr/local/include/zerodds.h
/usr/local/include/zerodds/
/usr/local/lib/pkgconfig/zerodds.pc
/usr/local/etc/zerodds/                        # Config-Default
/usr/local/var/log/zerodds/                    # Log-Default
/usr/local/var/lib/zerodds/
/Library/LaunchDaemons/org.zerodds.*.plist     # System-Daemons
~/Library/LaunchAgents/org.zerodds.*.plist     # User-Agents (optional)
/usr/local/share/man/man1/zerodds-*.1
```

### §3.2 Distributions

#### §3.2.1 Homebrew

Tap: `brew tap zero-objects/zerodds`.

Formulae:
```ruby
class Zerodds < Formula
  desc "ZeroDDS — Rust-native DDS implementation"
  homepage "https://zerodds.io"
  version "1.0.0"

  if Hardware::CPU.arm?
    url "https://github.com/zero-objects/zerodds/releases/download/v1.0.0/zerodds-1.0.0-aarch64-apple-darwin.tar.gz"
    sha256 "..."
  else
    url ".../zerodds-1.0.0-x86_64-apple-darwin.tar.gz"
    sha256 "..."
  end

  def install
    bin.install Dir["bin/zerodds-*"]
    lib.install "lib/libzerodds.dylib"
    include.install "include/zerodds.h", "include/zerodds"
    man1.install Dir["share/man/man1/*"]
  end

  service do
    run [opt_bin/"zerodds-ws-bridged", "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
  end
end
```

`brew services start zerodds` startet den ws-Daemon (oder per
Sub-Formula `brew install zerodds-mqtt-bridge`).

#### §3.2.2 PKG-Installer

Signed `.pkg`-Installer (Apple-Developer-ID notarized) für Enterprise-
Rollout. Ships in `releases/` auf GitHub.

### §3.3 launchd-Plist (Beispiel)

```xml
<!-- /Library/LaunchDaemons/org.zerodds.ws-bridged.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
                       "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.zerodds.ws-bridged</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/zerodds-ws-bridged</string>
        <string>--config</string>
        <string>/usr/local/etc/zerodds/ws-bridged.yaml</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/usr/local/var/log/zerodds/ws-bridged.log</string>
    <key>StandardErrorPath</key>
    <string>/usr/local/var/log/zerodds/ws-bridged.err</string>
    <key>UserName</key>
    <string>_zerodds</string>
</dict>
</plist>
```

`sudo launchctl load /Library/LaunchDaemons/org.zerodds.ws-bridged.plist`.

## §4 Windows

### §4.1 Datei-Layout

```
%PROGRAMFILES%\ZeroDDS\
  ├── bin\
  │   ├── zerodds.dll
  │   ├── zerodds-ws-bridged.exe
  │   ├── zerodds-mqtt-bridged.exe
  │   └── ...
  ├── include\zerodds.h
  ├── lib\zerodds.lib
  └── doc\

%PROGRAMDATA%\ZeroDDS\
  ├── ws-bridged.yaml
  ├── mqtt-bridged.yaml
  ├── ...
  ├── certs\
  └── logs\
      ├── ws-bridged.log
      ├── mqtt-bridged.log
      └── ...
```

### §4.2 Distributions

#### §4.2.1 MSI-Installer (WiX)

`zerodds-1.0.0-x64.msi` — signed mit EV-Code-Signing-Cert. Installiert
Binaries, registriert Services, schreibt Start-Menü-Einträge.

Optional-Features pro Bridge-Daemon (Custom-Install).

#### §4.2.2 Scoop

```
scoop bucket add zerodds https://github.com/zero-objects/scoop-zerodds
scoop install zerodds zerodds-ws-bridge
```

Manifest pro Komponente.

#### §4.2.3 Chocolatey

```powershell
choco install zerodds
choco install zerodds-ws-bridge
```

### §4.3 Win-Service

```powershell
sc.exe create ZeroDDSWSBridge `
  binPath= "\"%PROGRAMFILES%\ZeroDDS\bin\zerodds-ws-bridged.exe\" --config \"%PROGRAMDATA%\ZeroDDS\ws-bridged.yaml\"" `
  start= auto `
  obj= "NT SERVICE\ZeroDDSWSBridge" `
  DisplayName= "ZeroDDS WebSocket Bridge"

sc.exe description ZeroDDSWSBridge "ZeroDDS DDS↔WebSocket-Bridge"
sc.exe failure ZeroDDSWSBridge reset= 60 actions= restart/5000/restart/10000/restart/60000

sc.exe start ZeroDDSWSBridge
```

Pro Bridge-Daemon ein Service. Im MSI-Installer als
`ServiceInstall`-Custom-Action automatisch registriert.

## §5 Docker

Pro Daemon ein eigenes Image; Multi-Stage-Build mit `cargo-chef` für
Cache-Effizienz.

### §5.1 Image-Liste

```
zerodds/ws-bridged:1.0
zerodds/mqtt-bridged:1.0
zerodds/coap-bridged:1.0
zerodds/amqp-bridged:1.0
zerodds/grpc-bridged:1.0
zerodds/corba-bridged:1.0
zerodds/ros2-humble:1.0    # mit RMW-Shim vorinstalliert
zerodds/ros2-iron:1.0
zerodds/ros2-jazzy:1.0
zerodds/cli:1.0            # alle 17 CLI-Tools (debug/admin)
```

Multi-arch: `linux/amd64`, `linux/arm64`. Push auf
`docker.io/zerodds/*` und `ghcr.io/zero-objects/*`.

### §5.2 Beispiel-Dockerfile

```dockerfile
# stage 1: build
FROM rust:1.85-bookworm AS builder
RUN cargo install cargo-chef --version 0.1.66
WORKDIR /build
COPY --from=planner /build/recipe.json .
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin zerodds-ws-bridged

# stage 2: runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 999 zerodds
COPY --from=builder /build/target/release/zerodds-ws-bridged /usr/local/bin/
USER zerodds
ENV ZERODDS_CONFIG=/etc/zerodds/ws-bridged.yaml
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/zerodds-ws-bridged"]
CMD ["--config", "/etc/zerodds/ws-bridged.yaml"]
```

### §5.3 Docker-Compose-Beispiel

```yaml
version: "3.9"
services:
  ws-bridge:
    image: zerodds/ws-bridged:1.0
    network_mode: host
    volumes:
      - ./conf/ws-bridged.yaml:/etc/zerodds/ws-bridged.yaml:ro
      - ./certs:/etc/zerodds/certs:ro
    restart: unless-stopped

  mqtt-bridge:
    image: zerodds/mqtt-bridged:1.0
    network_mode: host
    volumes:
      - ./conf/mqtt-bridged.yaml:/etc/zerodds/mqtt-bridged.yaml:ro
    restart: unless-stopped

  mosquitto:
    image: eclipse-mosquitto:2
    ports: ["1883:1883", "8883:8883"]
    volumes:
      - ./mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
```

## §6 Cargo-Dist-Setup

Workspace-Cargo.toml:

```toml
[workspace.metadata.dist]
cargo-dist-version    = "0.20.0"
ci                    = ["github"]
installers            = ["shell", "powershell", "homebrew", "msi"]
targets               = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc"
]
pr-run-mode           = "plan"
allow-dirty           = ["ci"]
github-release-body   = "auto"
homebrew-tap          = "zero-objects/homebrew-zerodds"
publish-jobs          = ["homebrew", "scoop"]
checksum              = "sha512"
sign-artifacts        = ["minisign", "cosign"]
```

`cargo dist init` + `cargo dist build` + GitHub-Actions-Workflow
erzeugt Releases auf Tag-Push.

## §7 Version-Sync

Alle ZeroDDS-Crates teilen die Version aus Workspace-Root:

```toml
# Cargo.toml (Workspace-Root)
[workspace.package]
version       = "1.0.0-rc.1"
edition       = "2021"
rust-version  = "1.88"
license       = "Apache-2.0 OR MIT"
authors       = ["Sandra Keßler <sandra@ifyna.de>", "ZeroDDS Contributors"]
homepage      = "https://zerodds.io"
repository    = "https://github.com/zero-objects/zerodds"
description   = "ZeroDDS — Rust-native DDS implementation"
```

Pro Crate:
```toml
[package]
name    = "zerodds-ws-bridge"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
```

Bump-Process: ein einzelner Commit ändert `[workspace.package].version`,
`cargo dist plan` validiert, alle Bridges + Library + CLIs werden
gemeinsam released. Pre-Release-Tags (`-rc.1`, `-rc.2`) erlaubt;
finaler Tag `1.0.0`.

## §8 Cross-References zu Bridge-Specs

Diese Spec definiert die Konventionen, die in den folgenden Bridge-
Spec-`§11 Packaging`-Sektionen referenziert werden:

- `zerodds-ws-bridge-1.0` §11
- `zerodds-mqtt-bridge-1.0` §11
- `zerodds-coap-bridge-1.0` §11
- `zerodds-amqp-bridge-daemon-1.0` §11
- `zerodds-grpc-bridge-1.0` §11
- `zerodds-corba-bridge-1.0` §11
- `zerodds-ros2-bridge-1.0` §11
- `zerodds-ffi-loader-1.0` §6 (Library-Packaging)

Erweiterte Cross-Refs:
- Observability: `zerodds-monitor-1.1`, `zerodds-observability-otlp-1.0`
- Recorder-Format: `zddsrec-1.0`
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0`

## §9 Versioning

`1.0` initial. Patch für Bugfixes in Installer-Skripten + Service-
Files, Minor für additive Distros (z.B. NixOS, Alpine-apk-Repo,
SUSE-OBS), Major bei tiefgreifenden Layout-Änderungen (z.B. von
`/usr/lib/` auf `/opt/zerodds/`).
