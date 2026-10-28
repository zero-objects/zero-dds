# System setup

German original: [system-setup.md](system-setup.md)

This page describes the building blocks a ZeroDDS system consists of and how they relate: application, runtime configuration, transports, discovery, persistence, routing, recording, monitoring, bridges and ROS 2. After that follow recipes for typical setups with the files and commands from the repository. The basis is the code. Statements that come only from a doc comment are marked "(per doc comment)". Nothing on this page was built or run for this page.

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Building blocks overview

| Building block | Required | Artifact | Location |
|---|---|---|---|
| Application | yes | Rust crate `zerodds-rs` / `zerodds-dcps`, or C library `libzerodds` with a language binding | `crates/rs`, `crates/dcps`, `crates/zerodds-c-api` (`[lib] name = "zerodds"`), `endpoints/*` |
| Runtime configuration | no (defaults apply) | `zerodds.yaml`, `zerodds.d/`, environment `ZERODDS_*`, `--set` | `crates/config`, `crates/dcps/src/runtime.rs` (`SETTINGS`) |
| Transport | yes (UDPv4 default) | UDP, TCP, SHM, UDS, TSN | `crates/transport-*`, `transport.user_transport(s)` |
| Discovery | yes | SPDP/SEDP, multicast or unicast peers | `discovery.*` |
| Persistence | no | `zerodds-durability-svc` + store | `crates/durability-service-bin`, `crates/durability-store-*` |
| Routing | no | `zerodds-router` | `crates/routing-service` |
| Recording | no | `zerodds-record`, `zerodds-replay` | `tools/record`, `tools/replay`, `crates/recorder` |
| Monitoring | no | `zerodds-monitor`, Prometheus endpoints of the bridges, OTLP | `tools/monitor`, `crates/monitor`, `crates/observability-otlp` |
| Operations tools | no | `zerodds-admin`, `zerodds-spy` | `tools/admin`, `tools/spy` |
| Bridges | no | `zerodds-{ws,mqtt,coap,grpc,amqp,corba}-bridged` | `crates/*-bridge`, `crates/amqp-endpoint`, `crates/corba-dds-bridge` |
| ROS 2 | no | `rmw_zerodds_cpp` | `crates/rmw-zerodds-shim`, `crates/ros2-rmw` |

## Application

- Rust: `zerodds-rs` is the facade over `zerodds-dcps`, `zerodds-dcps-async`, `zerodds-qos`, `zerodds-types`, `zerodds-monitor`, `zerodds-recorder` (`crates/rs/Cargo.toml`). `zerodds-rs` has no `security` feature; a program that needs DDS-Security depends on `zerodds-dcps` directly with `features = ["security"]` (see [security.en.md](security.en.md)).
- Creating a participant: `DomainParticipantFactory::create(domain_id)` returns a `ParticipantBuilder` with `with_qos`, `with_config`, `with_security` (under feature `security`), `build` (`crates/dcps/src/factory.rs:378`). Without `with_config`, `build` reads the standard layers via `RuntimeConfig::from_standard_paths()`.
- Other languages: C ABI `libzerodds` (`crates/zerodds-c-api`, header `crates/zerodds-c-api/include/zerodds.h`). The endpoint SDKs are under `endpoints/` (ada, c, cpp, csharp, go, java, python, rust, typescript and others). Details: [crates-languages-codegen.en.md](crates-languages-codegen.en.md).
- The domain ID is not a configuration key of the runtime (not in `SETTINGS`). It is passed in the code, for tools with `-d`, for bridges with the YAML key `domain`, for ROS 2 with `ROS_DOMAIN_ID`.

## Runtime configuration

Checked in `crates/config/src/layers.rs`, `crates/config/src/load.rs`, `crates/dcps/src/runtime.rs`.

Precedence, lowest first:

1. compiled-in defaults,
2. environment (`ZERODDS_*`, table `SETTINGS` in `crates/dcps/src/runtime.rs:3034`),
3. files: base file, then drop-in directories (files sorted by name, only `.yaml`), then `include:`,
4. `--set PATH=VALUE` (repeatable; `--set +PATH=VALUE` appends to a list).

System directory (`SystemLayout::dir`):

| Platform | Base file | Drop-ins |
|---|---|---|
| Linux | `/etc/zerodds/zerodds.yaml` | `/etc/zerodds/zerodds.d/`, `/run/zerodds/zerodds.d/` |
| macOS | `/usr/local/etc/zerodds/zerodds.yaml` | `/usr/local/etc/zerodds/zerodds.d/` |
| Windows | `%PROGRAMDATA%\ZeroDDS\zerodds.yaml` | `%PROGRAMDATA%\ZeroDDS\zerodds.d\` |

- A missing base file is not an error (defaults apply). With `--config PATH` it is required, and instead of the system directory only `PATH` plus the sibling directory `PATH.d/` apply (`loader_for`).
- Every program that uses `zerodds_dcps::cli::Configured` accepts `--config`, `--set`, `--check-config`. `--check-config` prints every effective value with its origin; exit 0 valid, 1 invalid value or unknown key, 2 unreadable file, syntax error, include cycle (`crates/dcps/src/cli.rs:46`).
- Unknown keys under `--set` are rejected. `ros2.*` belongs to the RMW and is not reported as unknown by other programs (`is_rmw_path`).
- Bridges read their own base file (`<stem>.yaml`, e.g. `ws-bridged.yaml`) with the same layers; runtime keys such as `discovery.peers` are also permitted there (`ConfigSource::for_program`). A bridge that starts with `--config /etc/zerodds/ws-bridged.yaml` does not read `/etc/zerodds/zerodds.yaml`, except via `include:` (derived from `loader_for`).

Complete key list: [config.en.md](config.en.md).

## Transports

Discovery (SPDP/SEDP) always runs over UDPv4. The selected transport carries only the user data (`select_user_transport`, `crates/dcps/src/runtime.rs:1608`).

| Transport | Setting | Prerequisite | Use |
|---|---|---|---|
| UDPv4 | default; `transport.user_transport: UDPv4` | – | standard, across hosts and vendors |
| UDPv6 | `UDPv6` | – | IPv6 user data; SPDP stays IPv4 |
| TCPv4/TCPv6 | `TCPv4` / `TCPv6` | – | networks that drop UDP user data; discovery stays UDP |
| SHM | `SHM` | feature `same-host-shm` (default in `zerodds-c-api`, not in `zerodds-dcps`) | same host, POSIX shared memory |
| UDS | `UDS` | feature `same-host-uds` | same host without `/dev/shm` (containers); with both features SHM wins (per doc comment in `crates/dcps/Cargo.toml`) |
| TSN | `TSN`, `tsn.interface`, `tsn.vlan`, `tsn.pcp` | feature `tsn-live`, Linux only | RTPS directly on Ethernet, EtherType 0x88B5 |

- `transport.user_transports` is a list in order of preference. Each send goes to the first transport whose locator kind matches the destination; receiving happens on all of them. Example `[SHM, UDPv4]`. If the list is set, `transport.user_transport` is ignored.
- `transport.interface` (IPv4) binds all IP sockets to one interface (SPDP unicast, SPDP multicast sending, user data UDP/TCP).
- Automatic SHM path: with feature `same-host-shm`, two ZeroDDS participants on the same host open an SHM channel once SPDP reports the other; every unicast datagram between them goes through it, and what the ring refuses goes over UDP (`crates/dcps/src/same_host_channel.rs`). Switched off with `transport.same_host_shm` / `ZERODDS_SAME_HOST_SHM=0`.
- `delivery.mode` (`portable` | `raw-same-host` | `iceoryx`, env `ZERODDS_DELIVERY_MODE`) is the participant default for the delivery kind. It is read in the C-API loan path (`crates/zerodds-c-api/src/shm_loan_ffi.rs`) and in the ROS 2 shim. `raw-same-host` does not go on the wire. `iceoryx` needs an iceoryx2 service per writer (`zerodds_dw_enable_iceoryx`, feature `delivery-iceoryx`; the RMW derives it from the topic). Where the mode does not apply, the writer sends `portable` and reports this (C-API once per writer on stderr, RMW with a reason via `RCUTILS_LOG_WARN`). `DataWriter::write` in Rust is always `portable`.

## Discovery

Ports per RTPS 2.5 §9.6.1.4.1 (`crates/rtps/src/wire_types.rs:1019`): PB = 7400, DG = 250, PG = 2, d0 = 0, d1 = 10.

| Purpose | Port | Domain 0 |
|---|---|---|
| SPDP multicast (group 239.255.0.1) | 7400 + 250·d | 7400 |
| SPDP unicast, participant index i | 7400 + 250·d + 10 + 2·i | 7410, 7412, … |
| user-data unicast | ephemeral port (bind to port 0) | – |

- Participant index: at runtime start, i = 0 … 119 is tried and the first free port is taken. If all are taken, the runtime binds ephemerally; then only multicast discovery is possible (`crates/dcps/src/runtime.rs:9150`).
- User data runs over an ephemeral port, not over the port d3 of the specification. A firewall must therefore open more than the SPDP ports (derived from `select_user_transport`; not measured).

Keys (environment name in parentheses):

| Key | Effect |
|---|---|
| `discovery.peers` (`ZERODDS_PEERS`) | List of `ip` or `ip:port`. A bare `ip` is expanded to the SPDP unicast ports of the indices 0 … `discovery.max_peer_participants − 1` (default 10, range 1..=120). Beacons go to these addresses in addition to multicast. |
| `discovery.no_multicast` (`ZERODDS_NO_MULTICAST`) | `true`: not a single SPDP multicast packet, only `discovery.peers`. |
| `discovery.server` (`ZERODDS_DISCOVERY_SERVER`) | `true`: the participant forwards SPDP between its clients; SEDP runs directly between the participants. Without DDS-Security relay (per doc comment). |
| `discovery.spdp_multicast_group` | different multicast group (IPv4) |
| `discovery.multicast_allowlist` | only these groups are joined; others are rejected at creation |
| `discovery.spdp_period_ms`, `discovery.participant_lease_duration_ms`, `discovery.initial_announce_count`, `discovery.initial_announce_period_ms` | timing and lease |

Predefined profiles in the code: `RuntimeConfig::multi_robot()` (without multicast, lease 300 s; `crates/dcps/src/runtime.rs:4916`) and `RuntimeConfig::discovery_server()` (`runtime.rs:4942`).

## Persistence

- In process: a PERSISTENT writer creates its store under `durability.dir` (env `ZERODDS_DURABILITY_DIR`, default `(temp dir)/zerodds-durability`).
- Standalone: `zerodds-durability-svc`, one process per domain (`crates/durability-service-bin/src/main.rs`).

```text
zerodds-durability-svc --domain N [--store sqlite|file|lakehouse|postgres] [--path PATH] [--auto] [--topic NAME]...
```

| Store | Crate | Build | `--path` |
|---|---|---|---|
| `sqlite` (default) | `durability-store-sqlite` | always | DB file; without a path in memory, not persistent |
| `file` | `durability-store-file` | always | directory, required |
| `lakehouse` (DuckDB) | `durability-store-lakehouse` | `--features lakehouse`, needs `libduckdb` | DB file |
| `postgres` | `durability-store-postgres` | `--features postgres` (`postgres-timescaledb` for hypertables) | libpq connection string, required |

`--auto` serves every topic whose writers declare TRANSIENT or PERSISTENT. `--auto` or at least one `--topic` is required.

## Routing between domains

`zerodds-router` (`crates/routing-service/src/bin/zerodds-router.rs`):

```text
zerodds-router run      --routes <file.json|file.xml> [--types <shapes.json>] [--config <yaml>] [--set k=v]
zerodds-router validate --routes <file>
```

- Route: `input`/`output` with `domain`, `topic`, `type_name`, `keyed`, `partition`, `qos` (`reliable`, `durability`, `ownership`, `ownership_strength`, `data_representation`); in addition `filter`, `transform`, `loop_guard`, `preserve_source_timestamp`, `max_samples_per_second`, `tenant` (`crates/routing-service/src/config.rs`). Optional `access_control`.
- Example: `packaging/linux/configs/router.json.example` (domain 0 → 1, partition fan-out, type `zerodds::RawBytes`).
- Filter and transformation need `--types`.
- All routes of a process share one `RuntimeConfig`. A route cannot choose its own network interface; separation is done via domain and partition (derived from the fields of `Endpoint`).
- No SIGHUP reload; a route change requires a restart (comment in `packaging/linux/systemd/zerodds-router.service`).

## Recording and replay

| Command | Purpose |
|---|---|
| `zerodds-record record -t TOPIC … [-o FILE] [-d DOMAIN] [--duration DURATION]` | recording into `.zddsrec` |
| `zerodds-record record … --decode --type-file IDL [--out-json F] [--out-sqlite F]` | additionally decoded as typed data |
| `zerodds-record info FILE`, `list FILE` | header, frames per topic |
| `zerodds-replay inspect|dump FILE` | view content |
| `zerodds-replay replay FILE [--time-scale F] [--topic NAME]… [--inject [--inject-domain N]]` | replay; with `--inject` into a running domain |

`replay` accepts `.zddsrec`, `.json`/`.ndjson` and `.db`/`.sqlite` (help text in `tools/replay/src/main.rs`). Format: `docs/specs/zddsrec-1.0.md`.

## Monitoring

- Metric registry: `zerodds-monitor` (crate, `crates/monitor`), 31 standard metrics and 9 spans according to the crate docs, Prometheus text format via `render_prometheus`, HTTP endpoint `serve_prometheus` (feature `prometheus-server`). `zerodds-dcps` wires in the hooks with the default feature `metrics`.
- Tool `zerodds-monitor` (`tools/monitor`): `snapshot [-d ID] [--duration DURATION] [-f text|prometheus]`, `serve [-d ID] [-a ADDRESS]` (default `127.0.0.1:9991`), `names`. The tool outputs the registry of its own process (`default_registry()` after joining the domain). Metrics of another application do not appear there; the application must serve its registry itself (derived from `run_snapshot`; not measured).
- Bridges: block `metrics: { enabled, listen }` with `/metrics`, `/catalog`, `/healthz`. Default addresses in the example configurations: ws 9091, mqtt 9092, coap 9093, amqp 9094, grpc 9095, corba 9096 (all `127.0.0.1`).
- OTLP: `crates/observability-otlp`, OTLP/HTTP/JSON to `/v1/traces`, `/v1/metrics`, `/v1/logs`. The bridges and the ROS 2 shim depend on the crate; the export is activated via `OTEL_EXPORTER_OTLP_ENDPOINT` (checked for `zerodds-ws-bridged`, `otlp_config_from_env`). Errors when sending are discarded.
- Runtime events: `diagnostics.observability` (`off`/`none` or `stderr`/`json`) writes JSON events to stderr.

## Operations tools

| Tool | Purpose |
|---|---|
| `zerodds-admin domain inspect <id>` | participants and endpoints of a running domain |
| `zerodds-admin discovery snapshot <id>` | raw SPDP/SEDP view |
| `zerodds-admin config inspect|qos validate|qos check <file.xml>` | check DDS-XML offline |
| `zerodds-spy -t TOPIC [-d ID] [--decode --type-file IDL]` | print the samples of a topic |

Further tools (`zerodds-pcap`, `zerodds-dashboard`, `zerodds-perf` and others): [tools-and-man.en.md](tools-and-man.en.md).

## Bridges

Six daemons with a common security layer `crates/bridge-security` (TLS, auth, topic ACL; see [security.en.md](security.en.md)):

| Daemon | Crate | Example configuration |
|---|---|---|
| `zerodds-ws-bridged` | `crates/websocket-bridge` | `packaging/linux/configs/ws-bridged.yaml.example` |
| `zerodds-mqtt-bridged` | `crates/mqtt-bridge` | `mqtt-bridged.yaml.example` |
| `zerodds-coap-bridged` | `crates/coap-bridge` | `coap-bridged.yaml.example` |
| `zerodds-grpc-bridged` | `crates/grpc-bridge` | `grpc-bridged.yaml.example` |
| `zerodds-amqp-bridged` | `crates/amqp-endpoint` | `amqp-bridged.yaml.example` |
| `zerodds-corba-bridged` | `crates/corba-dds-bridge` | `corba-bridged.yaml.example` |

Further integrations without a daemon of their own in this list (zenoh, OPC UA, XRCE, iceoryx): [crates-bridges-services.en.md](crates-bridges-services.en.md).

## ROS 2

- RMW: `RMW_IMPLEMENTATION=rmw_zerodds_cpp` (`crates/rmw-zerodds-shim/README.md`). Package files in `crates/rmw-zerodds-shim/ament/`.
- Configuration: section `ros2.*` in the same runtime files. Example drop-in `packaging/linux/configs/ros2.yaml.example` → `/etc/zerodds/zerodds.d/50-ros2.yaml`. Check with `zerodds-ros2-shim --check-config`.
- Namespace, domain and logging belong to rcl and are rejected in the section `ros2`. Discovery via `discovery.no_multicast`/`discovery.peers`, enclave via `security.enclave_dir`.

Details: [ros2.en.md](ros2.en.md).

## Recipes

### 1. Two processes on one host

1. No configuration needed. Both processes use the same domain ID. The first participant binds SPDP unicast 7410, the second 7412 (domain 0).
2. Optionally shared memory instead of UDP for the user data:
   ```yaml
   # /etc/zerodds/zerodds.d/10-transport.yaml
   transport:
     user_transports: [SHM, UDPv4]
   ```
   Prerequisite: feature `same-host-shm` (default in `libzerodds`; for Rust `zerodds-dcps` with `features = ["same-host-shm"]`). Without this feature the value `SHM` is rejected as invalid at startup (`parse_user_transport`, `valid_runtime_value`).
3. For C-API writers with the loan path: `delivery.mode: raw-same-host` if all readers are on the same host (nothing is sent on the wire).
4. Check: `zerodds-admin domain inspect 0`, `zerodds-spy -t <Topic>`.

Note: discovery tests over loopback are unreliable on macOS (comments in `crates/dcps/tests/security_live_e2e.rs`, `security_matrix_e2e.rs`).

### 2. Several hosts with multicast

1. All hosts in the same L2 segment, same domain ID.
2. Firewall: UDP 7400 + 250·d (multicast 239.255.0.1), 7410 + 250·d + 2·i for each participant index, plus the ephemeral user-data ports.
3. With several interfaces, set the interface:
   ```yaml
   transport:
     interface: 192.168.10.5
   ```
4. Optionally restrict to your own group: `discovery.multicast_allowlist: [239.255.0.1]`.
5. Check: `<program> --check-config` shows the effective values with their origin; `zerodds-admin discovery snapshot 0` shows the discovered participants.

### 3. Hosts without multicast

Variant A, static peers:

```yaml
# /etc/zerodds/zerodds.d/20-discovery.yaml
discovery:
  no_multicast: true
  peers:
    - 10.0.0.11          # expanded to 7410, 7412, … (indices 0..9)
    - 10.0.0.12:7412     # exactly one port
  max_peer_participants: 10
```

Equivalent in the environment: `ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=10.0.0.11,10.0.0.12:7412`.

Variant B, discovery server:

- Server: `discovery.server: true`, `discovery.no_multicast: true` (corresponds to `RuntimeConfig::discovery_server()`).
- Clients: `discovery.no_multicast: true`, `discovery.peers: [<server-ip>]`.
- SEDP and user data then run directly between the participants; the clients must reach each other via UDP.

Variant C, TCP for user data:

- `transport.user_transport: TCPv4`. Discovery stays UDP; variant A or B is still needed. TCP binds an ephemeral port (`TcpTransport::bind_v4(pinned, 0)`).

### 4. System with persistence and a bridge

1. Durability service per domain:
   ```sh
   zerodds-durability-svc --domain 0 --store sqlite --path /var/lib/zerodds/durability/dur.db --auto
   ```
2. Application writers declare TRANSIENT or PERSISTENT; `--auto` picks them up without application code.
3. Bridge, here MQTT:
   ```sh
   cp /usr/share/zerodds/configs/mqtt-bridged.yaml.example /etc/zerodds/mqtt-bridged.yaml
   zerodds-mqtt-bridged --config /etc/zerodds/mqtt-bridged.yaml --check-config
   zerodds-mqtt-bridged --config /etc/zerodds/mqtt-bridged.yaml
   ```
   (Path `/usr/share/zerodds/configs/` from `packaging/linux/deb/postinst.tmpl`.) With `--config`, runtime keys such as `discovery.peers` belong in the bridge file or in `mqtt-bridged.d/`.
4. Security of the bridge: [security.en.md](security.en.md), section Bridges.
5. Metrics: `metrics.enabled: true`, `metrics.listen: "127.0.0.1:9092"`.

### 5. Installation as a service

systemd (Linux), files in `packaging/linux/systemd/`:

| File | Target (according to header comment) |
|---|---|
| `zerodds-<daemon>.service` | `/usr/lib/systemd/system/` |
| `zerodds-sysusers.conf` | `/usr/lib/sysusers.d/zerodds.conf` (user `zerodds`) |
| `zerodds-tmpfiles.conf` | `/usr/lib/tmpfiles.d/zerodds.conf` (`/var/log/zerodds`, `/var/lib/zerodds`, `/etc/zerodds`, `/etc/zerodds/certs` with 0700) |

```sh
install -m 0644 packaging/linux/systemd/zerodds-sysusers.conf /usr/lib/sysusers.d/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-tmpfiles.conf /usr/lib/tmpfiles.d/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-ws-bridged.service /usr/lib/systemd/system/
systemd-sysusers && systemd-tmpfiles --create
cp packaging/linux/configs/ws-bridged.yaml.example /etc/zerodds/ws-bridged.yaml
systemctl daemon-reload
systemctl enable --now zerodds-ws-bridged
systemctl reload zerodds-ws-bridged     # ExecReload = SIGHUP: TLS material and ACL reloaded
```

The commands are standard systemd; in the repository, `daemon-reload`, `deb-systemd-helper preset` and `try-restart` are in `packaging/linux/deb/postinst.tmpl`. The packages do not enable services on first installation.

| Unit | `Type` | `ExecStart` | Reload |
|---|---|---|---|
| `zerodds-{ws,mqtt,coap,grpc,amqp,corba}-bridged` | `notify` | `/usr/bin/zerodds-<x>-bridged --config /etc/zerodds/<x>-bridged.yaml` | SIGHUP |
| `zerodds-router` | `exec` | `/usr/bin/zerodds-router run --routes /etc/zerodds/router.json` | none |
| `zerodds-durability-svc` | `exec` | parameters via `Environment=ZERODDS_DURABILITY_{DOMAIN,STORE,PATH,EXTRA}` | none |

- `zerodds-durability-svc.service` is not installed by the RPM (`packaging/linux/rpm/zerodds.spec:197`); it must be copied by hand. One instance per domain.
- Router and durability service read the runtime configuration from the system directory (`/etc/zerodds/zerodds.yaml`), since their `ExecStart` does not set `--config`.
- All units: `User=zerodds`, `ProtectSystem=strict`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`. A store outside `ReadWritePaths` is not writable.

launchd (macOS), files in `packaging/macos/launchd/` (only the six bridges, no router, no durability service):

```sh
sudo cp packaging/macos/launchd/org.zerodds.ws-bridged.plist /Library/LaunchDaemons/
sudo cp packaging/linux/configs/ws-bridged.yaml.example /usr/local/etc/zerodds/ws-bridged.yaml
sudo launchctl load -w /Library/LaunchDaemons/org.zerodds.ws-bridged.plist
```

- The plists start `/usr/local/bin/zerodds-<x>-bridged --config /usr/local/etc/zerodds/<x>-bridged.yaml` as `_zerodds`, logs under `/usr/local/var/log/zerodds/`.
- The user `_zerodds` is created by the preinstall script in `packaging/macos/pkg/build-pkg.sh` via `dscl`; the postinstall loads only `org.zerodds.ws-bridged.plist`.

Windows: `packaging/windows/services/Install-Services.ps1` (not evaluated).

## Open items from the code

None.
