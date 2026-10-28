# CLI programs and options

German original: [cli-options.md](cli-options.md)

This page lists every program that the workspace builds as a binary (`[[bin]]` sections, `src/main.rs`, `src/bin/*.rs` under `crates/` and `tools/`), with purpose, invocation, options with default values, exit codes, configuration file read, and man page. The basis is the argument parser in the source code; for the 18 binaries present in `target/debug`, `--help` and `--version` were also run and compared with the code and the man page. Programs that serve only test or measurement purposes are collected at the end. The discrepancies between help text, code and man page are in the last section.

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Overview

84 binary targets, of which 27 are programs for users or operations (sections below) and 57 are internal test, measurement and generator programs (section [Internal programs](#internal-programs)). Fuzz targets under `*/fuzz/` are not counted.

| Program | Source | Configuration | Man page | Package (rpm/deb) |
|---|---|---|---|---|
| `zerodds-admin` | `tools/admin/src/main.rs` | runtime (`zerodds.yaml`) | `man1/zerodds-admin.1` | `zerodds-cli` |
| `zerodds-spy` | `tools/spy/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-spy.1` | `zerodds-cli` |
| `zerodds-record` | `tools/record/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-record.1` | `zerodds-cli` |
| `zerodds-replay` | `tools/replay/src/main.rs` | no flags; `--inject` reads the standard paths | `man1/zerodds-replay.1` | `zerodds-cli` |
| `zerodds-monitor` | `tools/monitor/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-monitor.1` | `zerodds-cli` |
| `zerodds-mq` | `tools/mq/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-mq.1` | `zerodds-cli` |
| `zerodds-snitch` | `tools/snitch/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-snitch.1` | `zerodds-cli` |
| `zerodds-bench` | `tools/bench/src/main.rs`, `lib.rs` | runtime | `man1/zerodds-bench.1` | `zerodds-cli` |
| `zerodds-pcap` | `tools/pcap/src/main.rs`, `lib.rs` | none | `man1/zerodds-pcap.1` | `zerodds-cli` |
| `zerodds-router` | `crates/routing-service/src/bin/zerodds-router.rs` | runtime + route file | `man1/zerodds-router.1` | `zerodds-router` |
| `zerodds-durability-svc` | `crates/durability-service-bin/src/main.rs` | runtime | none | not packaged (rpm excludes it) |
| `zerodds-idlc` | `tools/idlc/src/main.rs` | none | `man1/zerodds-idlc.1` | `zerodds-cli` |
| `zerodds-xmlc` | `tools/xmlc/src/main.rs` | none | `man1/zerodds-xmlc.1` | `zerodds-cli` |
| `zerodds-lint` | `crates/lint/src/bin/zerodds-lint.rs` | none | none | `zerodds-cli` |
| `zerodds-traceability` | `tools/traceability/src/main.rs` | none | none | `zerodds-cli` |
| `zerodds-ros2-shim` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs` | runtime + `ros2` section | `man1/zerodds-ros2-shim.1`, `man5/zerodds-ros2.yaml.5` | `zerodds-ros2` |
| `zerodds-ws-bridged` | `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs` | `ws-bridged.yaml` | `man1/…ws-bridged.1`, `man5/…ws-bridged.yaml.5` | `zerodds-ws-bridge` |
| `zerodds-mqtt-bridged` | `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` | `mqtt-bridged.yaml` | `man1`, `man5` | `zerodds-mqtt-bridge` |
| `zerodds-coap-bridged` | `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` | `coap-bridged.yaml` | `man1`, `man5` | `zerodds-coap-bridge` |
| `zerodds-amqp-bridged` | `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` | `amqp-bridged.yaml` | `man1`, `man5` | `zerodds-amqp-bridge` |
| `zerodds-grpc-bridged` | `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` | `grpc-bridged.yaml` | `man1`, `man5` | `zerodds-grpc-bridge` |
| `zerodds-corba-bridged` | `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` | `corba-bridged.yaml` | `man1`, `man5` | `zerodds-corba-bridge` |
| `zerodds-secure-permissions` | `tools/secure-permissions/src/main.rs` | none | none | rpm `zerodds-cli` only |
| `zerodds-chaos` | `tools/chaos/src/main.rs` | none | none | `zerodds-cli` |
| `zerodds-perf` | `tools/perf/src/main.rs` | none | none | `zerodds-cli` |
| `zerodds-dashboard` | `tools/dashboard/src/main.rs` | none | none | `zerodds-cli` |
| `zerodds-recorder-bridge` | `tools/recorder-bridge/src/main.rs` | none | none | `zerodds-cli` |

Package assignment from `packaging/linux/rpm/zerodds.spec` (`%files`) and `packaging/linux/deb/assemble-debs.sh`. The deb loop for `zerodds-cli` does not contain `zerodds-secure-permissions`; the rpm list contains it.

## Common configuration options

All programs that start a DDS runtime read the same three flags. Parser: `CommandLine::extract` in `crates/config/src/layers.rs`; integration and help text `HELP`: `crates/dcps/src/cli.rs` (`Configured`).

| Option | Meaning |
|---|---|
| `--config PATH`, `--config=PATH` | Base file; drop-ins in the sibling directory `PATH.d/`. Given twice: error. Without the option, the system layout (below) applies. |
| `--set PATH=VALUE`, `--set=PATH=VALUE` | One value above all files, repeatable, later ones win. `--set +a.b=V` appends to a list. Unknown path: error. |
| `--check-config` | Loads all layers as at startup, prints every value with its origin (`sources:`, `values:`, final line `configuration valid` / `configuration invalid`) and exits the program. |

Parsing stops at `--`; everything after it belongs to the program.

Precedence, lowest first: built-in defaults, environment (`SETTINGS` in `crates/dcps/src/runtime.rs`, e.g. `ZERODDS_TICK_PERIOD_MS`), files (base, drop-ins, `include:`), `--set`.

System layout (`SystemLayout::dir`, `for_stem` in `crates/config/src/layers.rs`):

| Platform | Directory | Drop-ins |
|---|---|---|
| Linux | `/etc/zerodds` | `/etc/zerodds/<stem>.d`, then `/run/zerodds/<stem>.d` |
| macOS | `/usr/local/etc/zerodds` | `<dir>/<stem>.d` |
| Windows | `%PROGRAMDATA%\ZeroDDS` (fallback `C:\ProgramData\ZeroDDS`) | `<dir>\<stem>.d` |

The base file is `<dir>/<stem>.yaml`. `stem` is `zerodds` for the tools, the router, the durability service and `zerodds-ros2-shim`; the bridges use `ws-bridged`, `mqtt-bridged`, `coap-bridged`, `amqp-bridged`, `grpc-bridged`, `corba-bridged`.

The bridges map each of their own options to `--set` of its file key (tables per bridge below). These entries are in the flag layer and appear in `--check-config` with their origin, e.g. ``domain = 3  (command line `--set domain=3`)`` (measured with `zerodds-ws-bridged --domain=3 --check-config`).

Exit codes related to configuration, according to `HELP`: 0 valid, 1 invalid value or unknown key, 2 unreadable file, syntax error, include cycle. Measured (target/debug):

| Case | Tools, router, durability | Bridges |
|---|---|---|
| valid, `--check-config` | 0 | 0 |
| invalid value (`--set runtime.tick_period_ms=abc`) | 1 | 1 |
| unknown `--set` path | 2 (`zerodds-spy`) | 1 (`zerodds-ws-bridged`) |
| `--config /nonexistent.yaml` | 2 | 1 |

The bridges return 1 for every configuration error, but print the same `HELP` text with "2 unreadable file".

Programs with this mechanism: `zerodds-admin`, `-spy`, `-record`, `-monitor`, `-mq`, `-snitch`, `-bench`, `-router`, `-durability-svc` (via `Configured::from_args`/`from_env_args`), the six bridges (via `CommandLine::extract` + `Configured::from_command_line`), `zerodds-ros2-shim` (via `CommandLine::extract` + `ConfigSource::for_program`). Without these flags: `zerodds-replay`, `-pcap`, `-idlc`, `-xmlc`, `-lint`, `-traceability`, `-secure-permissions`, `-chaos`, `-perf`, `-dashboard`, `-recorder-bridge`.

The keys themselves are described in [config.en.md](config.en.md).

## Tools for operations and diagnostics

The tools `spy`, `record`, `monitor`, `mq`, `snitch`, `bench` share: `-h/--help` and `-V/--version` are checked before everything else (anywhere in the argument list), durations `DUR` accept `5`, `30s`, `2m`, `1h` (`parse_duration`, `tools/cli-common`), exit codes 0 success, 2 parse error, 3 DDS or I/O error.

### zerodds-admin

Operator CLI: joins a running domain and lists participants and endpoints, or analyzes DDS-XML files offline.

```
zerodds-admin <group> <verb> [args] [--json] [--timeout-ms T]
```

| Command | Effect |
|---|---|
| `domain inspect <id>` | live: participants with writer/reader endpoints (topic, type, reliability, durability) |
| `discovery snapshot <id>` | live: SPDP/SEDP snapshot, participant list, pub/sub counts |
| `config inspect <file.xml>` | offline: load deployment, topology by domain ID |
| `qos validate <file.xml>` | offline: well-formedness, parse every library |
| `qos check <file.xml>` | offline: RxO compatibility writer/reader (DDS 1.4 §2.2.3), exit 1 on incompatibility |

| Option | Meaning | Default |
|---|---|---|
| `--json` | machine-readable output | off |
| `--timeout-ms <T>` | observation window for `domain`/`discovery`; only the form with a space | 2000 |
| `-h`, `--help` | help; also without arguments, exit 0 | |

`--version`: version, exit 0. Exit codes: 0 ok, 1 validation or probe error, 2 usage error.

### zerodds-spy

Subscribes to a topic as raw bytes and prints metadata and a hex excerpt per sample; optionally with typed decoding.

```
zerodds-spy [subscribe] -t TOPIC [OPTIONS]
```

| Option | Meaning | Default |
|---|---|---|
| `-d`, `--domain <ID>` | domain ID | 0 |
| `-t`, `--topic <NAME>` | topic, required | |
| `--type <NAME>` | announce the type name directly, no type tracking | the writers' types are discovered and tracked |
| `-n`, `--count <N>` | stop after N samples | unlimited |
| `--duration <DUR>` | stop after the duration | unlimited |
| `-x`, `--hex <BYTES>` | first BYTES as hex, 0 = off | 32 |
| `--decode` | samples as typed JSON (needs `--type-file`) | off |
| `--type-file <IDL>` | IDL file with the types | |
| `--dialect <NAME>` | wire dialect to decode in: `cyclone`, `rti`, `fastdds`, `opendds`, ... | `wire.codec_dialect` (`specification`) |

Source of the options: `parse_subscribe` in `tools/spy/src/lib.rs`.

### zerodds-record

Writes live samples to a `.zddsrec` file and reads existing files. Format: `docs/specs/zddsrec-1.0.md`.

```
zerodds-record record [OPTIONS]
zerodds-record info FILE
zerodds-record list FILE
```

| Option (`record`) | Meaning | Default |
|---|---|---|
| `-o`, `--output <FILE>` | output file | `capture-<unix-ts>.zddsrec` |
| `-d`, `--domain <ID>` | domain ID | 0 |
| `-t`, `--topic <NAME>` | topic, repeatable, at least one | |
| `--duration <DUR>` | recording duration | until SIGINT |
| `--max-sample-bytes <N>` | upper limit per sample | 1048576 |
| `--decode` | additionally decode as typed data (needs `--type-file`) | off |
| `--type-file <IDL>` | IDL with the types | |
| `--map <TOPIC=Type>` | set the IDL type for a topic, repeatable | |
| `--out-json <FILE>` | decoded samples as NDJSON | |
| `--out-sqlite <FILE>` | decoded samples in SQLite, one table per topic; an existing FILE is replaced, as with `-o` and `--out-json` | |
| `--dialect <NAME>` | wire dialect to decode in: `cyclone`, `rti`, `fastdds`, `opendds`, ... | `wire.codec_dialect` (`specification`) |

`info` prints the header (time base, participants, topics), `list` the frame count per topic. Source: `tools/record/src/lib.rs`.

### zerodds-replay

Reads `.zddsrec`, NDJSON or SQLite and prints the frames or feeds them into a domain with the original timing.

```
zerodds-replay inspect FILE
zerodds-replay dump FILE
zerodds-replay replay FILE [--time-scale F] [--topic NAME]... [--inject [--inject-domain N]]
```

| Command / option | Meaning | Default |
|---|---|---|
| `inspect FILE` | header, frames, payload bytes, time range | |
| `dump FILE` | CSV `t_delta_ns,participant,topic,kind,payload_len` | |
| `replay FILE` | frames in recorded order | |
| `--time-scale F` | factor relative to the wall clock | 1.0 |
| `--topic NAME` | filter, repeatable | all topics |
| `--inject` | write into a domain via one writer per topic | off |
| `--inject-domain N` | domain for `--inject` | 0 |
| `-h`, `--help`, `help` | help (also without arguments), exit 0 | |

Format detection by extension (`detect_format`): `.json`, `.ndjson`, `.jsonl` → NDJSON; `.db`, `.sqlite`, `.sqlite3` → SQLite; otherwise `.zddsrec`. Exit codes: 0, 1 on read or runtime error, 2 on usage error. `--version`: exit 0. `--inject` creates the runtime via `zerodds_runtime_create` (C API), which reads the standard paths of the system layout (`ffi_runtime_config` → `ConfigSource::from_standard_paths` in `crates/zerodds-c-api/src/lib.rs`); there is no `--config`/`--set`.

### zerodds-monitor

Prints the metrics registry of the runtime once or serves a Prometheus endpoint `/metrics`.

```
zerodds-monitor snapshot [-d ID] [--duration DUR] [-f text|prometheus]
zerodds-monitor serve [-d ID] [-a ADDR] [--duration DUR]
zerodds-monitor names
```

| Option | Command | Default |
|---|---|---|
| `-d`, `--domain <ID>` | snapshot, serve | 0 |
| `--duration <DUR>` | snapshot: collection window; serve: automatic stop | snapshot 5s; serve until SIGINT |
| `-f`, `--format <FORMAT>` | snapshot: `text`, `prometheus` (alias `prom`) | `text` |
| `-a`, `--addr <ADDR>` | serve: listen address | `127.0.0.1:9991` |

`names` lists the known metric names.

### zerodds-mq

Forwards raw samples between two domains, payload unchanged.

```
zerodds-mq [bridge] -t TOPIC --src-domain N --dst-domain M [OPTIONS]
```

| Option | Meaning | Default |
|---|---|---|
| `--src-domain <ID>` | source domain | 0 |
| `--dst-domain <ID>` | destination domain; must differ (check in `main.rs`) | 1 |
| `-t`, `--topic <NAME>` | same topic name on both sides, required | |
| `--src-topic <N>`, `--dst-topic <N>` | different names, override `-t` | |
| `--duration <DUR>` | stop after the duration | until SIGINT |
| `--bidirectional` | mirror in both directions | off |

### zerodds-snitch

Listens to SPDP/SEDP for a duration and prints the participants seen and the endpoint counts.

```
zerodds-snitch [probe] [-d ID] [--duration DUR] [-f text|json]
```

| Option | Default |
|---|---|
| `-d`, `--domain <ID>` | 0 |
| `--duration <DUR>` | 5s |
| `-f`, `--format <FORMAT>` (`text`, `json`) | `text` |

### zerodds-bench

Latency and throughput measurement over the DCPS pipeline with writer and reader in the same process.

```
zerodds-bench latency|throughput [-d ID] [-t NAME] [-p N] [--duration DUR]
zerodds-bench info
```

| Option | Default |
|---|---|
| `-d`, `--domain <ID>` | 0 |
| `-t`, `--topic <NAME>` | `zerodds/bench/loopback` |
| `-p`, `--payload <N>` (bytes) | 64 |
| `--duration <DUR>` | 5s |

`info` prints the capabilities of the backend.

### zerodds-pcap

Offline parser for libpcap files: finds RTPS payloads and decodes the submessages. Does not join a domain.

```
zerodds-pcap parse FILE
zerodds-pcap stats FILE
```

Options only `-h/--help`, `-V/--version`. Exit codes 0, 2 usage error, 3 file, pcap or RTPS decoding error.

### zerodds-router

DDS-to-DDS forwarding between domains, topics or partitions with optional filter and transform.

```
zerodds-router run --routes <file> [--types <shapes.json>]
zerodds-router validate --routes <file>
```

| Option | Meaning |
|---|---|
| `--routes <file>` | route file, required; extension `.xml` → XML, otherwise JSON |
| `--types <shapes.json>` | JSON array of type shapes, needed for filter/transform |
| `--config`, `--set`, `--check-config` | runtime configuration (`zerodds.yaml`); `--config` with extension `.json`/`.xml` is rejected with a reference to `--routes`, exit 2 |

With `-h/--help`: help on stderr, exit 0; `-V/--version`: version, exit 0. Without a command or with an unknown command: help, exit 2. Error in `run`/`validate`: exit 1. Unreadable configuration file: exit 2. Signals: SIGINT/SIGTERM terminate, then metrics per route (`forwarded`, `dropped_loop`, `dropped_filter`, `lifecycle`, `errors`). The systemd unit starts `zerodds-router run --routes /etc/zerodds/router.json` (`packaging/linux/systemd/zerodds-router.service`); example `packaging/linux/configs/router.json.example`.

### zerodds-durability-svc

Durability service (ADR 0009): keeps the TRANSIENT/PERSISTENT samples of a domain in a store and serves late-joining readers.

```
zerodds-durability-svc --domain N [--store KIND] [--path PATH] [--auto] [--topic NAME]...
```

| Option | Meaning | Default |
|---|---|---|
| `--domain N` | domain ID, required | |
| `--store KIND` | `sqlite`, `file`, `lakehouse` (feature `lakehouse`), `postgres` (feature `postgres`) | `sqlite` |
| `--path PATH` | sqlite/lakehouse: database file (missing: in memory, with a warning); file: directory, required; postgres: connection string, required | |
| `--auto` | serve every topic whose writers declare TRANSIENT or PERSISTENT | off |
| `--topic NAME` | serve a topic explicitly, repeatable | |
| `-h`, `--help` | help, exit 0 | |

At least `--auto` or one `--topic`. Exit codes: 0; 1 for every error in `run`, including unknown arguments and a missing `--domain` (code: `parse_args` runs inside `run`); 2 for configuration errors. `--version`: exit 0. No man page. The systemd unit `packaging/linux/systemd/zerodds-durability-svc.service` sets the arguments from `ZERODDS_DURABILITY_DOMAIN`, `_STORE`, `_PATH`, `_EXTRA` (defaults 0, `sqlite`, `/var/lib/zerodds/durability/dur.db`, `--auto`); only the unit reads these variables, the program does not.

### zerodds-idlc

Reads OMG IDL 4.2: checks, lints, formats, prints the model and dependencies, generates source code for one language.

```
zerodds-idlc <SUBCOMMAND> [OPTIONS] FILE...
```

Subcommands: `check`, `lint`, `format`, `dump-ast`, `print-deps`, `generate`. Options and files may be mixed.

| Option | Applies to | Meaning | Default |
|---|---|---|---|
| `--strict` | all | IDL 4.2 only, reject every vendor extension | off |
| `--json` | all | machine-readable diagnostics | text |
| `--deny-warnings` | all | non-zero exit as soon as anything was reported | off |
| `--default-extensibility <final\|appendable\|mutable>` | all | extensibility for aggregates that do not specify one | `appendable` |
| `--check` | `format` | name non-canonical files, exit 1 | |
| `--write` | `format` | overwrite non-canonical files; incompatible with `--check` | |
| `--lang <NAME>` | `generate`, required | `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin` (`Language::as_str`, `crates/lang-cst/src/common.rs`) | |
| `--out <DIR>` | `generate` | target directory | `.` |
| `--encoding <ENC>` | `generate` | `xcdr1`, `xcdr2`, `pl-cdr1`, `pl-cdr2`, `delimited-cdr` | `xcdr2` |
| `--endian <le\|be>`, `--xcdr-endianness <BE\|LE>` | `generate` | byte order in the preamble text | `le` |

Format options outside `format` and generate options outside `generate` are rejected with the option name. Exit codes: 0 nothing reported, 1 diagnostic or `--check` found a file, 2 usage error, 3 file unreadable.

### zerodds-xmlc

Reads OMG DDS-XML 1.0 documents.

```
zerodds-xmlc validate|types|render FILE...
```

`validate` parses every library (types, qos, domain, participant, application) and counts errors, `types` lists type definitions, `render` shows domains and participants. Every option except `-h/--help`, `-V/--version` is rejected. Exit codes 0, 1 library not parsable, 2 usage error, 3 file unreadable.

### zerodds-lint

Project-specific lint rules over the workspace (clap-based).

```
zerodds-lint check [--root <ROOT>] [--fail-on-warning]
```

| Option | Meaning | Default |
|---|---|---|
| `--root <ROOT>` | workspace root | search upwards from the current directory |
| `--fail-on-warning` | warnings also lead to exit 1 | off |
| `-h`, `--help`, `-V`, `--version` | clap default | |

Exit codes: 0, 1 errors found (or warnings with `--fail-on-warning`), 2 internal error. In CI: `cargo run -p zerodds-lint -- check` (`ci/full.yml`).

### zerodds-traceability

Generates from `docs/spec-coverage/*.md` a matrix requirement → status, code, tests.

```
zerodds-traceability [--format text|md|json] [--status <s>] [<path>...]
```

| Option | Meaning | Default |
|---|---|---|
| `<path>` | coverage file or directory; `.en.md` is skipped when walking a directory | `docs/spec-coverage` |
| `--format` | `text`, `md`, `json` | `text` |
| `--status <s>` | only rows whose normalized status starts with `<s>` | |
| `-h`, `--help` | help | |

Exit codes 0, 2 usage error or unreadable path. No `--version`.

### zerodds-ros2-shim

Diagnostic tool for `rmw_zerodds_cpp`. Reads the same runtime configuration as the RMW library (`zerodds.yaml`, drop-ins, `ros2` section) and shows mangling, QoS profiles, discovery and security.

```
zerodds-ros2-shim <SUBCOMMAND> [--config FILE] [--set PATH=VALUE]... [--check-config] [--domain ID]
```

| Subcommand | Effect |
|---|---|
| `info` | RMW compat level, active mangling |
| `topics` | configured topic overrides |
| `topic <ROS_TOPIC>` | DDS topic for a ROS topic |
| `qos <PROFILE>` | profile from `ros2.qos_profiles` (`default`, `sensor_data`, `services`, `parameters`, `parameter_events`, custom ones) |
| `enclaves` | SROS2 enclave (`security.enclave_dir`, environment `ZERODDS_SECURITY_DIR`) |
| `validate <FILE>` | check the file as runtime base with `FILE.d/` |
| `selftest` | round trip mangling + QoS lookup |
| `catalog` | JSON catalog |
| `metrics` | catalog of the Prometheus metric names |
| `doctor` | check RMW, environment, discovery, security |
| `graph` | local participant and topic graph |

| Option | Meaning | Default |
|---|---|---|
| `--domain <ID>` | overrides `ROS_DOMAIN_ID`; only the form with a space | `ROS_DOMAIN_ID`, otherwise 0 |
| `--runtime-config`, `--log-level`, `--enclave` | are rejected with a hint, exit 1 (`REFUSED_FLAGS`) | |
| `-V`, `--version`; `-h`, `--help` | version; help (on stderr) | |

Exit codes: 0; 1 for every error, also without a subcommand.

## Bridge daemons

Common to all: configuration file `<stem>.yaml` with `<stem>.d/` in the system directory, unless `--config` names a different one; each of their own options is `--set` of its key. The systemd units start each daemon with `--config /etc/zerodds/<stem>.yaml` (`packaging/linux/systemd/*.service`), the launchd plists with `--config /usr/local/etc/zerodds/<stem>.yaml`. SIGTERM/SIGINT terminate, SIGHUP rereads the configuration (scope per bridge in the man page). `OTEL_EXPORTER_OTLP_ENDPOINT` enables an OTLP export (`otlp_config_from_env`; called in ws, mqtt, coap, amqp, corba; no call found in `zerodds-grpc-bridged.rs`).

Two parser families:

- `ws`, `mqtt`, `coap`: `daemon/cli.rs` per crate; accepts `--opt VALUE` and `--opt=VALUE`; help on stdout.
- `amqp`, `grpc`, `corba`: parser in the binary; accepts only `--opt VALUE`. `--opt=VALUE` leads to `unknown argument` (measured: `--domain=3` → exit 1 for all three). Help on stderr.

### zerodds-ws-bridged

DDS ↔ WebSocket. Specification `docs/specs/zerodds-ws-bridge-1.0.md`.

| Option | Key | Default |
|---|---|---|
| `--listen <ADDR>` | `listen` | `0.0.0.0:8080` |
| `--domain <ID>` | `domain` | 0 |
| `--topic <NAME>` | `+topics` | |
| `--tls-cert <FILE>` | `tls.cert_file`, `tls.enabled=true` | |
| `--tls-key <FILE>` | `tls.key_file`, `tls.enabled=true` | |
| `--auth-token <SECRET>` | `auth.mode=bearer`, `auth.bearer_token` | `auth.mode` = `none` |
| `--log-level <LEVEL>` | `log_level` | `info` |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | admin endpoint off; enabled without an address: `127.0.0.1:9091` |
| `-V`, `--version`; `-h`, `--help` | | |

Exit codes (`main`): 0, 1 configuration or I/O, 2 bind, 3 DDS, 4 TLS. `daemon/server.rs` sets up the rustls handshake, bearer check and admin endpoint (code read, not measured in operation).

### zerodds-mqtt-bridged

DDS ↔ MQTT 5.

| Option | Key | Default (`default_for_dev`) |
|---|---|---|
| `--broker <URL>` (`mqtt://`, `mqtts://`, `ws://`, `wss://`) | `mqtt.broker_url` | `mqtt://127.0.0.1:1883` |
| `--client-id <ID>` | `mqtt.client_id` | `zerodds-bridge` |
| `--domain <ID>` | `domain` | 0 |
| `--username <USER>`, `--password <PASS>` | `mqtt.username`, `mqtt.password` | |
| `--tls-ca`, `--tls-cert`, `--tls-key <FILE>` | `mqtt.tls.ca_file`, `.cert_file`, `.key_file`; each sets `mqtt.tls.enabled=true` | |
| `--topic <DDS:MQTT>` | `+topics` | |
| `--log-level <LEVEL>` | `log_level` | `info` |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | |

Environment: `MQTT_PASSWORD` → `mqtt.password` (`config::ENV`). Exit codes: 0, 1 configuration/I/O, 2 broker connection, 3 DDS, 4 TLS, 5 authentication.

### zerodds-coap-bridged

DDS ↔ CoAP (RFC 7252) over UDP and DTLS.

| Option | Key | Default |
|---|---|---|
| `--bind <ADDR>` | `coap.bind` | `0.0.0.0:5683` |
| `--domain <ID>` | `domain` | 0 |
| `--dtls-cert <FILE>` + `--dtls-key <FILE>` (only as a pair) | `coap.dtls.cert_file`, `.key_file`, `coap.dtls.enabled=true` | |
| `--dtls-psk-id <ID>` + `--dtls-psk <SECRET>` (only as a pair, hex) | `+coap.dtls.psk=ID=SECRET`, `coap.dtls.enabled=true` | |
| `--topic <DDS:URI>` | `+topics` | |
| `--log-level <LEVEL>` | `log_level` | |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | |

Exit codes: 0, 1 configuration/I/O, 2 bind, 3 DDS, 4 DTLS and OSCORE (`ServerError::Oscore` → 4).

### zerodds-amqp-bridged

DDS ↔ AMQP 1.0.

| Option | Key |
|---|---|
| `--broker <URL>` (`amqp://`, `amqps://`), required (file or flag) | `amqp.broker_url` |
| `--container-id <ID>` | `amqp.container_id` |
| `--domain <ID>` | `domain` |
| `--sasl-mechanism <NAME>` (`PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2`) | `amqp.sasl.mechanism` |
| `--user`, `--password` | `amqp.sasl.username`, `amqp.sasl.password` |
| `--tls-ca`, `--tls-cert` | `amqp.tls.ca_file`, `.cert_file`, both set `amqp.tls.enabled=true` |
| `--tls-key` | `amqp.tls.key_file` (does not set `enabled`) |
| `--topic <DDS=ADDR>` | `+topics` |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` (without an address `127.0.0.1:9094`) |
| `--once` | test aid: serve one connection, then exit |
| `--connect-timeout-ms <N>` | test aid: give up the connection attempt, default 5000 |

Environment: `AMQP_PASSWORD` → `amqp.sasl.password`. Exit codes according to the `DaemonError` documentation in `crates/amqp-endpoint/src/bridge_daemon.rs`: 1 configuration, 2 connection, 3 DDS; the code produces exactly these three. The help text names no exit codes.

### zerodds-grpc-bridged

DDS ↔ gRPC over HTTP/2.

| Option | Key | Default |
|---|---|---|
| `--bind <ADDR>` | `grpc.bind` | `0.0.0.0:50051` |
| `--domain <ID>` | `domain` | 0 |
| `--tls-cert` + `--tls-key` (only as a pair) | `grpc.tls.cert_file`, `.key_file`, `grpc.tls.enabled=true` | |
| `--tls-client-ca <FILE>` | `grpc.tls.client_ca_file` | |
| `--reflection` | `grpc.reflection.enabled=true` | off |
| `--topic <DDS:SVC>` | `+topics` | |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` | |
| `--once` | test aid: one request, then exit | |
| `--idle-timeout-ms <N>` | test aid: exit after N ms without a request | 0 (off) |

Exit codes: 0, 1 configuration, 2 bind/I/O, 3 DDS, 4 TLS.

### zerodds-corba-bridged

DDS ↔ CORBA GIOP/IIOP and SSLIOP.

| Option | Key | Default |
|---|---|---|
| `--iiop-bind <ADDR>` | `corba.iiop.bind` | `0.0.0.0:6833` |
| `--ssliop-bind <ADDR>` | `corba.ssliop.bind`, `corba.ssliop.enabled=true` | `0.0.0.0:6834` |
| `--domain <ID>` | `domain` | |
| `--naming-service <CORBANAME>` | `corba.ior.naming_service` (registration not built, according to the man page) | |
| `--orb-id <ID>` | `corba.orb_id` | |
| `--tls-cert` + `--tls-key` (only as a pair) | `corba.ssliop.cert_file`, `.key_file`, `corba.ssliop.enabled=true` | |
| `--topic <REPO_ID=TOPIC>` | `+mappings` | |
| `--dump-iors` | one line per mapping `repo_id operation IOR:…`, then exit; host from `corba.iiop.advertise_host` (environment `ZERODDS_CORBA_ADVERTISE_HOST`) | `127.0.0.1` |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` | |
| `--idle-timeout-ms <N>` | test aid: stop after N ms | 0 (off) |

Exit codes according to the man page: 0, 1 configuration, 2 bind, 3 DDS, 4 TLS. The binary itself produces only code 1; `crates/corba-dds-bridge/src/bridge_daemon.rs` produces 1, 2, 3 and 4 (mapping to bind/DDS/TLS not traced individually).

## Other user programs

| Program | Purpose | Invocation and options | Exit codes |
|---|---|---|---|
| `zerodds-secure-permissions` | CMS/PKCS#7 signing and verification of DDS Security governance and permissions XML | `sign --signer-cert <ca.pem> --signer-key <ca_key.pem> --in <doc.xml> --out <doc.p7s>`; `verify --ca <ca.pem> --p7s <doc.p7s> [--out <doc.xml>]`; the key must be PKCS#8 | 0, 1 signing/verification error, 2 usage error |
| `zerodds-chaos` | network faults for DDS tests | `proxy --listen A --forward A [--loss F] [--burst N] [--duplicate F] [--reorder F] [--jitter MS\|LO:HI] [--seed N] [--duration N(ms\|s)]`; `tc-apply --interface IF [--loss] [--delay] [--jitter] [--duplicate] [--reorder] [--correlation]` (root, netem); `tc-clear`; `partition --group-a … --group-b … [--duration]`; `partition-clear`; `endpoint-flap --interface IF [--interval] [--total] [--start-down]`; `--dev` is an alias for `--interface` (not named in the help text) | 1, 2 |
| `zerodds-perf` | crypto and registry measurements | `hw-info`; `aes-gcm [--bytes N] [--block N] [--suite 128\|256]` (defaults 100 MiB, 1 KiB, 128); `monitor`; `help` | |
| `zerodds-dashboard` | HTTP server with SPA and JSON endpoints | `[--bind ADDR:PORT]` (default `127.0.0.1:8089`), `[--prometheus ADDR:PORT]` (off), `[--demo]` | 0, 1 |
| `zerodds-recorder-bridge` | live recording to `.zddsrec` with one reader per `NAME:TYPE` | `--out FILE --topic NAME:TYPE… [--domain N] [--duration N(ms\|s)]`, domain default 0 | 0, 1, 2 |

## Internal programs

Not packaged. Purpose taken from the `//!` comments of the respective `main`; options only roughly read from the parser, not run.

| Program | Source | Purpose |
|---|---|---|
| `cargo-dag` | `tools/cargo-dag` | topological publish order of the workspace crates; `[ROOT] [--only-publishable] [--format flat\|verbose\|json]` |
| `qos-matrix` | `tools/qos-matrix` | all combinations of the QoS policy enums against `check_compatibility`; `--format markdown\|csv` |
| `interop-matrix` | `tools/interop-matrix` | JSON → HTML matrix; `--out`, `--fail-on-red` (exit 1 on red cells) |
| `interop-results` | `tools/interop-results` | result tree `tests/interop-results/` and tables; `--regenerate`, `--verify`, `--tally`, `--dialects`, others |
| `isolation-smoke` | `tools/isolation-smoke` | sender/receiver over UDP or UDS for `testing/isolation/`; `--transport= --role= --local= --peer= --count=`; exit 0–4 |
| `amqp-dds-endpoint` | `tools/amqp-dds-endpoint` | AMQP 1.0 server with XML configuration; `--config`, `--listen` (default `0.0.0.0:5672`), `--hash-password`; feature `tls` |
| `iceoryx2-oracle-consumer` | `tools/iceoryx2-oracle-consumer` | external iceoryx2 subscriber without a ZeroDDS dependency, checks the bytes |
| `shape_main` | `tools/omg-shape-main` | reference client for the OMG suite `dds-rtps` |
| `shapetype_main` | `tools/omg-security-main` | reference client for the OMG suite `dds-security` |
| `xtypes_main` | `tools/omg-xtypes-main` | reference client for the OMG suite `dds-xtypes` |
| `zerodds_test_main` | `tools/omg-xtypes-test-main` | reimplementation of `test_main.cxx` of the XTypes suite; short options `-P -S -t -y -d -r -b -p …` |
| `regression`, `roundtrip-1us`, `roundtrip-typed` | `tools/bench-suite/src/bin` | roundtrip regression gate (CI) and latency measurements |
| `wireprobe-zerodds` | `crates/wire-probe/src/main.rs` | one IDL type, one JSON value, the bytes; same options as the vendor probes |
| `wireprobe-compare`, `wireprobe-corba-idl`, `wireprobe-dump`, `wireprobe-fill`, `wireprobe-ros-package` | `crates/wire-probe/src/bin` | helper programs of the probe chain (auto-discovered binaries, no `[[bin]]`) |
| `zerodds-cyclone-iox-reader`, `zerodds-cyclone-iox-writer` | `crates/iceoryx-cyclone/src/bin` | iceoryx (POSH) SHM proof against Cyclone DDS; feature `posh` |
| `proof-cyclone-dcps` | `crates/iceoryx-cyclone/proofs/dcps-interop` | typed DCPS proof over iceoryx against Cyclone |
| `bench_features`, `echo_bench`, `echo_bench_codegen`, `interop_client`, `interop_server`, `ssliop_bench`, `ssliop_client`, `ssliop_server` | `crates/corba-interop/src/bin` | CORBA latency measurements and cross-ORB interop (omniORB/TAO/JacORB) |
| `peer` | `crates/endpoint-e2e/src/bin/peer.rs` | UDP peer for the endpoint examples |
| `zerodds-dashboard-tauri` | `tools/dashboard-tauri/src-tauri` | Tauri 2.0 shell around `zerodds-dashboard` |
| (probe) | `tools/xtypes-1.3-gap-scan/probe` | reads `id op arg` from stdin, writes measured values for `scan.sh` |
| `wireprobe-endpoint-gen`, `wireprobe-c-matrix`, `wireprobe-cpp-matrix`, `wireprobe-python-matrix`, `wireprobe-zerodds-<lang>-gen` (20 languages: ada, ada83, c, cpp, csharp, d, elixir, fsharp, go, java, julia, kotlin, lua, nim, node, ocaml, python, swift, typescript, zig) | `tools/wireprobe/*-gen` | generators for the language probes of the interop corpus |

Example programs under `examples/` and `crates/*/examples/` are not listed.

## Discrepancies between help text, code and man page

None open. Measured with the binaries in `target/debug` after `fe3b0ca185c`
and `tools/man-gate --list` (0 undocumented, 0 documented and gone, 0 with
different exit codes): the man pages name `--config`, `--set` and
`--check-config` and the notation `--opt VALUE`; the bridges name their
environment variables and exit codes; `--version` answers in `zerodds-admin`,
`-router`, `-replay`, `-xmlc`, `-durability-svc`, `-spy`, `-chaos`,
`-ros2-shim` with exit 0; `zerodds-router` returns exit 2 on a usage error,
as its man page says. The tools that
[tools-and-man.en.md](tools-and-man.en.md) lists remain without a man page.
