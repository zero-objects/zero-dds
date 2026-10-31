# `zerodds-deployment` v1.0 — Linux/macOS/Windows packaging conventions

ZeroDDS vendor spec. Specifies the packaging, installation and
deployment conventions for all ZeroDDS binaries and the
shared library `libzerodds`.

## Motivation

ZeroDDS ships eight bridge daemons, an ABI library and seventeen
CLI tools. Without a uniform packaging convention,
operations teams are supplied differently per platform; patch
rollouts take longer, config paths diverge, logs end up in
unexpected places.

This spec standardizes per platform:
- Where binaries live.
- Where configs live.
- Where logs live.
- Which service manager integration applies (systemd / launchd / Windows
  service).
- Which installer mechanism is the mandatory distribution.
- Which container images are in the registry.

This spec is referenced by §11 of every bridge spec
(`zerodds-ws-bridge-1.0`, `zerodds-mqtt-bridge-1.0`, etc.) and by
§6 `zerodds-ffi-loader-1.0`.

## §1 Targets

ZeroDDS releases comprise three classes of artefacts:

### §1.1 Daemons (8 bridges)

| Daemon | Spec cross-ref |
|--------|----------------|
| `zerodds-ws-bridged` | `zerodds-ws-bridge-1.0` §11 |
| `zerodds-mqtt-bridged` | `zerodds-mqtt-bridge-1.0` §11 |
| `zerodds-coap-bridged` | `zerodds-coap-bridge-1.0` §11 |
| `zerodds-amqp-bridged` | `zerodds-amqp-bridge-daemon-1.0` §11 |
| `zerodds-grpc-bridged` | `zerodds-grpc-bridge-1.0` §11 |
| `zerodds-corba-bridged` | `zerodds-corba-bridge-1.0` §11 |
| `zerodds-zenoh-bridged` | no bridge spec document of its own; see `crates/zenoh-bridge/README.md`, `man/man1/zerodds-zenoh-bridged.1`, `man/man5/zerodds-zenoh-bridged.yaml.5` |
| `zerodds-ros2-shim` | `zerodds-ros2-bridge-1.0` §11 (diagnostic tool, not a classic daemon) |

### §1.2 Library

| Artefact | Spec cross-ref |
|----------|----------------|
| `libzerodds.so` / `.dylib` / `.dll` + `zerodds.h` | `zerodds-ffi-loader-1.0` §6 |

### §1.3 CLI tools (17)

```
zerodds-admin                   # admin CLI (domain inspect, QoS validate/check, discovery snapshot)
zerodds-bench                   # benchmark suite (latency/throughput/loss)
zerodds-bench-suite             # multi-scenario bench runner
zerodds-idlc                    # IDL compiler → Rust/C++/Java/C#/Python/TS codegen
zerodds-spy                     # topic spy (live sample dump)
zerodds-record                  # recorder (zddsrec format)
zerodds-replay                  # replay from zddsrec
zerodds-snitch                  # discovery probe (SPDP/SEDP dumper)
zerodds-monitor                 # monitor CLI (`zerodds-monitor-1.1`)
zerodds-mq                      # multi-domain bridge (DDS↔DDS)
zerodds-pcap                    # PCAP dumper for the RTPS wire
zerodds-perf                    # performance probe
zerodds-ros2-shim               # ROS-2 diagnostics
zerodds-secure-keygen           # DDS-Security cert/key generation
zerodds-secure-permissions      # permissions/governance XML tool
zerodds-typeobject              # TypeObject inspect
zerodds-xmlc                    # DDS-XML config validator/renderer
```

## §2 Linux

### §2.1 File layout (FHS)

```
/usr/bin/zerodds-*                       # binaries (all 8 daemons + 17 CLIs)
/usr/lib/libzerodds.so                   # ABI library
/usr/lib/libzerodds.so.1                 # major symlink
/usr/lib/libzerodds.so.1.0.0             # versioned symlink
/usr/lib/pkgconfig/zerodds.pc            # pkg-config
/usr/include/zerodds.h                   # C header
/usr/include/zerodds/                    # C++ headers
/usr/lib/systemd/system/zerodds-*.service
/etc/zerodds/                            # config default
  ├── zerodds.yaml                       # DDS runtime base
  ├── zerodds.d/                         # sorted runtime fragments
  ├── ws-bridged.yaml
  ├── mqtt-bridged.yaml
  ├── coap-bridged.yaml
  ├── amqp-bridged.yaml
  ├── grpc-bridged.yaml
  ├── corba-bridged.yaml
  ├── zenoh-bridged.yaml
  └── certs/
/var/log/zerodds/                        # log default
/var/lib/zerodds/                        # state (IORs, recorder files)
/usr/share/man/man1/zerodds-*.1.gz       # manuals
/usr/share/man/man5/zerodds*.yaml.5.gz
/usr/share/doc/zerodds/                  # CHANGELOG, LICENSE, README
/usr/share/bash-completion/completions/  # bash/zsh/fish completions
```

#### Runtime configuration from RC7 on

Every ZeroDDS program reads its configuration in four layers; the later one
wins:

1. built-in defaults,
2. environment variables, under the names of the table `SETTINGS`
   (`crates/dcps/src/runtime.rs`), e.g. `ZERODDS_RECV_BATCH` for
   `transport.recv_batch`,
3. files: with `--config PATH` this base (must exist) and the
   neighbouring `PATH.d/`; without `--config` the platform's system directory
   — `/etc/zerodds` on Linux (§2.1), `/usr/local/etc/zerodds` on macOS
   (§3.1), `%PROGRAMDATA%\ZeroDDS` on Windows (§4.1) — with
   `zerodds.yaml`, then YAML files sorted by file name from
   `zerodds.d/`, on Linux last from `/run/zerodds/zerodds.d`,
4. `--set path=value` on the command line, repeatable, in the
   given order; `--set +path=value` appends to a list.

Explicit `include:` entries can load further files or directories
relative to the including file, also nested; cycles and more than
16 levels are rejected. Includes come before the file's own values. Maps
are merged, lists are replaced and extended with `+name`. A list
from the environment (comma-separated) is a sequence in layer 3, to which a
drop-in can append with `+name`. Lists are written as a block list (`- a`) or
as a flat inline list of scalars (`[a, "b"]`); inline maps and
nested inline lists are rejected.

Missing implicit default paths are allowed. Invalid known values,
unreadable files and include errors prevent the start; the message names
the layer (file:line, `$VARIABLE` or the `--set`). A `--set` on a
path that no setting reads is rejected. Unknown keys in files
produce a warning at startup. `--check-config` loads all layers as at
startup, prints every value with its origin and exits with 0 (valid), 1
(invalid value or unknown key) or 2 (file unreadable,
syntax error, include cycle).

In C, `zerodds_runtime_create_from_config(domain, path, sets, n_sets)` selects
the same layers (`path` = `--config`, `NULL` = system directory; `sets` =
`"path=value"` entries like `--set`); `zerodds_runtime_create` reads the environment
and the system directory. In Rust,
`zerodds_dcps::cli::Configured::from_env_args()` provides these layers for a program; `ConfigSource::from_command_line` and
`ConfigSource::from_path` are the building blocks underneath. Explicit
caller configuration remains authoritative. `RuntimeConfig::default()` reads
neither files nor the environment. `ConfigSource::origins()` shows the origin.

A bridge daemon reads its own file instead of `zerodds.yaml`: without
`--config` `<stem>.yaml` with `<stem>.d/` in the system directory (`ws-bridged.yaml`
etc.), with `--config` the named base. The runtime keys may be in this
file and apply to the DDS side of the daemon; it does not read `zerodds.yaml`.
`ZERODDS_CONFIG` does not select a file in the runtime path. ROS selectors and
explicit security APIs keep their own contracts.

Keys, values and environment variables: `man/man5/zerodds.yaml.5`; every setting with its default: `packaging/linux/configs/zerodds.yaml.example` (installed under `/usr/share/zerodds/configs/`).

Executable layering examples: [runtime_config_layering.rs](../../crates/dcps/tests/runtime_config_layering.rs).
Website guide: [Runtime files](../../website/why-zerodds-for-ros/config-complexity.html#runtime-dateien).

### §2.2 Distributions

#### §2.2.1 Debian/Ubuntu (`.deb`)

Packages:
- `zerodds-core` — `libzerodds`, headers, manuals (mandatory dependency)
- `zerodds-ws-bridge` — `zerodds-ws-bridged` + service
- `zerodds-mqtt-bridge` — likewise
- `zerodds-coap-bridge`, `-amqp-bridge`, `-grpc-bridge`, `-corba-bridge`, `-zenoh-bridge`, `-ros2`
- `zerodds-cli` — all 17 CLI tools
- `zerodds-dev` — `zerodds.h`, pkg-config, static libs

Repo: `deb https://packages.zerodds.io/apt stable main`. Signed with the
release GPG key (`/etc/apt/trusted.gpg.d/zerodds.gpg`).

```bash
curl -fsSL https://packages.zerodds.io/apt/zerodds.gpg | sudo tee /etc/apt/trusted.gpg.d/zerodds.gpg > /dev/null
echo "deb https://packages.zerodds.io/apt stable main" | sudo tee /etc/apt/sources.list.d/zerodds.list
sudo apt update
sudo apt install zerodds-ws-bridge
```

#### §2.2.2 RHEL/Fedora (`.rpm`)

The same package set via `dnf install`. Repo `https://packages.zerodds.io/yum/`.
GPG key in `/etc/pki/rpm-gpg/RPM-GPG-KEY-zerodds`.

#### §2.2.3 Arch Linux (PKGBUILD)

AUR packages `zerodds-core`, `zerodds-ws-bridge`, etc. Pacman hook
for service reload on update.

#### §2.2.4 AppImage (static build)

One AppImage per daemon:
- `zerodds-ws-bridged-1.0.0-x86_64.AppImage`
- `zerodds-mqtt-bridged-1.0.0-x86_64.AppImage`
- ...

Statically linked against musl for distro independence. Hosted on
GitHub Releases + S3 mirror.

### §2.3 systemd unit (example)

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

The system user `zerodds:zerodds` is created by the postinst script via `useradd
--system`.

## §3 macOS

### §3.1 File layout

```
/usr/local/bin/zerodds-*                       # binaries (Apple Silicon universal)
/usr/local/lib/libzerodds.dylib                # ABI library
/usr/local/include/zerodds.h
/usr/local/include/zerodds/
/usr/local/lib/pkgconfig/zerodds.pc
/usr/local/etc/zerodds/                        # config default
/usr/local/var/log/zerodds/                    # log default
/usr/local/var/lib/zerodds/
/Library/LaunchDaemons/org.zerodds.*.plist     # system daemons
~/Library/LaunchAgents/org.zerodds.*.plist     # user agents (optional)
/usr/local/share/man/man1/zerodds-*.1
```

The configuration system directory on macOS is always
`/usr/local/etc/zerodds`, also on Apple Silicon. Homebrew's prefix
(`/opt/homebrew/etc`) belongs to Homebrew: the services of the formulae (§3.2.1)
pass `--config #{etc}/zerodds/<programm>.yaml`; a program started
by hand without `--config` reads `/usr/local/etc/zerodds`.

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

`brew services start zerodds` starts the ws daemon (or via a
sub-formula `brew install zerodds-mqtt-bridge`).

The sub-formulae depend on `zerodds` (`depends_on "zerodds"`) and take
the daemon binary from its keg. Homebrew requires a URL in every formula, so
each sub-formula names the meta formula's bundle with the same URL and
sha256; Homebrew keys its download cache by URL and fetches the archive once.
A sub-formula installs the example configuration from the bundle to
`#{pkgshare}/configs`, copies it to `#{etc}/zerodds` only if no configuration
exists there, and brings the `service do` block with
`--config #{etc}/zerodds/<programm>.yaml`.
`zerodds-ros2` is not a service — rcl loads the RMW into every node —
and only installs `ros2.yaml.example` as `ros2.yaml`.

The bundle is built by the macOS jobs of `.github/workflows/release.yml`
(`packaging/macos/bundle/build-bundle.sh`, binaries and `libzerodds.dylib`
signed with the Developer ID Application identity) and published with the
release, per target `zerodds-<v>-<target>.tar.gz` with the layout from §3.1:

```
bin/zerodds-*                            # 7 daemons + 17 CLIs (§1.1/§1.3)
lib/libzerodds.dylib
include/zerodds.h
include/zerodds/*                        # C++ convenience headers
include/dds/                             # DDS-PSM-Cxx headers
lib/pkgconfig/zerodds.pc
share/zerodds/configs/*.yaml.example
share/man/man1/zerodds-*.1
share/man/man5/zerodds*.yaml.5
share/launchd/*.plist
```

`<v>` is the workspace version from `Cargo.toml`. Next to each archive is
`zerodds-<v>-<target>.tar.gz.sha256`.

`.github/workflows/publish-distros.yml` writes the tap from the published
release: it downloads both bundles, `packaging/github-actions/render-homebrew.sh`
writes their sha256 into the formulae (it aborts with the name of a missing
file), a macOS runner installs the rendered formulae from a local tap
(`packaging/github-actions/homebrew-verify.sh`: meta formula, `brew test`,
one sub-formula), and only then are they pushed to
`zero-objects/homebrew-zerodds`.

The job `macos-bundle` (`ci/full.yml`) builds and renders the same bundle on a
GitLab macOS runner (tag `macos`, shell executor, Xcode Command Line Tools,
rustup) where one is registered.

#### §3.2.2 PKG installer

Signed `.pkg` installer (Apple Developer ID notarized) for enterprise
rollout. Ships in `releases/` on GitHub.

### §3.3 launchd plist (example)

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

### §4.1 File layout

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

#### §4.2.1 MSI installer (WiX)

`zerodds-1.0.0-x64.msi` — signed with an EV code signing cert. Installs
binaries, registers services, writes Start menu entries.

Optional features per bridge daemon (custom install).

#### §4.2.2 Scoop

```
scoop bucket add zerodds https://github.com/zero-objects/scoop-zerodds
scoop install zerodds zerodds-ws-bridge
```

One manifest per component.

#### §4.2.3 Chocolatey

```powershell
choco install zerodds
choco install zerodds-ws-bridge
```

### §4.3 Windows service

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

One service per bridge daemon. Registered automatically in the MSI installer as a
`ServiceInstall` custom action.

## §5 Docker

One image per daemon; multi-stage build with `cargo-chef` for
cache efficiency.

### §5.1 Image list

```
zerodds/ws-bridged:1.0
zerodds/mqtt-bridged:1.0
zerodds/coap-bridged:1.0
zerodds/amqp-bridged:1.0
zerodds/grpc-bridged:1.0
zerodds/corba-bridged:1.0
zerodds/zenoh-bridged:1.0
zerodds/ros2-humble:1.0    # diagnostic CLI; RMW library open, see ros2-bridge §11
zerodds/ros2-iron:1.0
zerodds/ros2-jazzy:1.0
zerodds/cli:1.0            # all 17 CLI tools (debug/admin)
```

Multi-arch: `linux/amd64`, `linux/arm64`. Push to
`docker.io/zerodds/*` and `ghcr.io/zero-objects/*`.

### §5.2 Example Dockerfile

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
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/zerodds-ws-bridged"]
CMD ["--config", "/etc/zerodds/ws-bridged.yaml"]
```

### §5.3 Docker Compose example

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

## §6 cargo-dist setup

Workspace Cargo.toml:

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

`cargo dist init` + `cargo dist build` + a GitHub Actions workflow
produce releases on tag push.

## §7 Version sync

All ZeroDDS crates share the version from the workspace root:

```toml
# Cargo.toml (workspace root)
[workspace.package]
version       = "1.0.0-rc.1"
edition       = "2021"
rust-version  = "1.88"
license       = "Apache-2.0 OR MIT"
authors       = ["ZeroDDS Contributors <admin@zerodds.org>"]
homepage      = "https://zerodds.io"
repository    = "https://github.com/zero-objects/zerodds"
description   = "ZeroDDS — Rust-native DDS implementation"
```

Per crate:
```toml
[package]
name    = "zerodds-ws-bridge"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
```

Bump process: a single commit changes `[workspace.package].version`,
`cargo dist plan` validates, all bridges + library + CLIs are
released together. Pre-release tags (`-rc.1`, `-rc.2`) are allowed;
final tag `1.0.0`.

## §8 Cross-references to bridge specs

This spec defines the conventions that are referenced in the following bridge
spec `§11 Packaging` sections:

- `zerodds-ws-bridge-1.0` §11
- `zerodds-mqtt-bridge-1.0` §11
- `zerodds-coap-bridge-1.0` §11
- `zerodds-amqp-bridge-daemon-1.0` §11
- `zerodds-grpc-bridge-1.0` §11
- `zerodds-corba-bridge-1.0` §11
- `zerodds-ros2-bridge-1.0` §11
- `zerodds-ffi-loader-1.0` §6 (library packaging)

Extended cross-refs:
- Observability: `zerodds-monitor-1.1`, `zerodds-observability-otlp-1.0`
- Recorder format: `zddsrec-1.0`
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0`

## §9 Versioning

`1.0` initial. Patch for bug fixes in installer scripts + service
files, minor for additive distros (e.g. NixOS, Alpine apk repo,
SUSE OBS), major for far-reaching layout changes (e.g. from
`/usr/lib/` to `/opt/zerodds/`).
