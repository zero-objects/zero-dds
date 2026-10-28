# Configuration

German original: [config.md](config.md)

This page is the reference for all configuration sources of ZeroDDS: the layer model (defaults, environment, files, `--set`) and `--check-config`, every setting of the runtime file `zerodds.yaml` including `ros2.*`, the keys of the daemon files (`ws-bridged.yaml`, `mqtt-bridged.yaml`, `coap-bridged.yaml`, `amqp-bridged.yaml`, `grpc-bridged.yaml`, `corba-bridged.yaml`), the route file of the router, QoS profiles via DDS-XML, the places where security files are configured, and all environment variables outside the table `SETTINGS`. The basis is the code on `main` (state `fac89e645c9`). Defaults come from the code; where they were also measured with a binary, this is stated. Statements that come only from doc comments are marked "(per doc comment)".

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## 1 Layer model

### 1.1 Result

Every ZeroDDS program that starts via `zerodds_dcps::cli::Configured` or `ConfigSource::for_program` reads four layers. The later one wins.

| Layer | Source | Origin shown by `--check-config` | Implementation |
|---|---|---|---|
| 1 | built-in defaults | `(built-in default)` | `RuntimeConfig::from_validated_source`, `crates/dcps/src/runtime.rs:4273`; daemons: `default_for_dev()` or `impl Default` |
| 2 | environment under the names of the table `SETTINGS` (plus program names such as `MQTT_PASSWORD`) | `(environment $NAME)` | `env_layer`, `crates/config/src/layers.rs:99`; table `crates/dcps/src/runtime.rs:3034` |
| 3 | files: base, drop-in directories, `include:` | `(path:line)` | `Loader`, `crates/config/src/load.rs`; selection `loader_for`, `crates/config/src/layers.rs:468` |
| 4 | `--set path=value`, repeatable, in order | ``(command line `--set …`)`` | `CommandLine::layer`/`apply`, `crates/config/src/layers.rs` |

A file therefore overrides an environment variable. `--config` adds no layer; it selects the base of layer 3. `ConfigSource::for_program` (`crates/dcps/src/runtime.rs:3320`) assembles the layers; `Configured::from_command_line` (`crates/dcps/src/cli.rs:155`) validates and warns.

### 1.2 File selection (layer 3)

| Case | Base file | Drop-ins, in this order | Base missing |
|---|---|---|---|
| without `--config`, Linux and other Unix | `/etc/zerodds/<stem>.yaml` | `/etc/zerodds/<stem>.d/`, then `/run/zerodds/<stem>.d/` | allowed, defaults apply |
| without `--config`, macOS | `/usr/local/etc/zerodds/<stem>.yaml` | `/usr/local/etc/zerodds/<stem>.d/` | allowed |
| without `--config`, Windows | `%PROGRAMDATA%\ZeroDDS\<stem>.yaml` (without `PROGRAMDATA`: `C:\ProgramData\ZeroDDS`) | `…\ZeroDDS\<stem>.d\` | allowed |
| `--config PATH` | `PATH` | `PATH` with extension `.d` (`Path::with_extension("d")`: `/x/a.yaml` → `/x/a.d/`) | error (`require_base`) |

Locations: `SystemLayout::dir`, `crates/config/src/layers.rs:421`; `SystemLayout::for_stem`, `:441`; `loader_for`, `:468`.

Rules of the loader (`crates/config/src/load.rs`, `crates/config/src/lib.rs`):

- From drop-in directories, only files with extension `.yaml` are read, sorted by file name. `.rpmnew`, `.dpkg-dist` and editor backups are excluded by this. A missing drop-in directory is not an error.
- `include:` takes a path or a list. Paths are relative to the including file. An include may be a file or a directory. Includes are read first, then the file's own values, independent of the text position of the key. Explicit includes must exist. Cycles (`IncludeCycle`) and more than `MAX_INCLUDE_DEPTH` = 16 levels (`IncludeTooDeep`) are rejected (`load.rs:19`, `:235`, `:270`).
- Merging: maps key by key, scalars and lists replace. `+name` appends to the list under `name`; if `name` is not a list, the result is `AppendToNonSeq`.
- A list value from the environment (comma-separated) is already a sequence at layer 3; a drop-in can append to it with `+name`. An empty value (`FOO=`) counts as set and yields the empty list.
- YAML subset: indentation with two spaces, `key: value`, `- item`, flat inline lists `[a, "b"]`, `#` comments. Anchors, inline maps, nested inline lists, multi-document streams and block scalars are rejected by name (`crates/config/src/lib.rs`, module comment; measured: `a: &x 1` → "anchors (`&name`) are not read here").
- `${VAR}` and `${VAR:-default}` are substituted from the process environment before parsing. An unset variable without a default becomes the empty string (`expand_env_vars`, `crates/config/src/parse.rs:24`).
- Nesting depth at most `MAX_NESTING_DEPTH` = 32 (`crates/config/src/lib.rs:106`).
- Values under paths containing `token`, `password`, `secret`, `passphrase`, `psk`, `sasl_users`, `credentials` appear in reports and origin information as `***` (`is_secret_path`, `crates/config/src/layers.rs:504`).

### 1.3 Which program reads which base file

| Program | `stem` | own keys | Source |
|---|---|---|---|
| all CLI tools (`zerodds-spy`, `-bench`, `-admin`, `-monitor`, `-record`, `-snitch`, `-mq`), `zerodds-router`, `zerodds-durability-svc` | `zerodds` | none | `Configured::from_args`/`from_env_args` |
| RMW `rmw_zerodds_cpp` | `zerodds` | `ros2.*` | `process_source`, `crates/rmw-zerodds-shim/src/ros2_config.rs:102` |
| `zerodds-ros2-shim` | `zerodds` | `ros2.*` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs:62` |
| C-API `zerodds_runtime_create` | `zerodds` | none | `crates/zerodds-c-api/src/lib.rs:1200` |
| C-API `zerodds_runtime_create_from_config(domain, path, sets, n_sets)` | `zerodds`, `path` = `--config`, `NULL` = system directory | none | `crates/zerodds-c-api/src/lib.rs:1232` |
| `zerodds-ws-bridged` | `ws-bridged` | 17 | `CONFIG_STEM`, `crates/websocket-bridge/src/daemon/config.rs:25` |
| `zerodds-mqtt-bridged` | `mqtt-bridged` | 25 | `crates/mqtt-bridge/src/daemon/config.rs:21` |
| `zerodds-coap-bridged` | `coap-bridged` | 32 | `crates/coap-bridge/src/daemon/config.rs:21` |
| `zerodds-amqp-bridged` | `amqp-bridged` | 28 | `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs:30` |
| `zerodds-grpc-bridged` | `grpc-bridged` | 19 | `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1457` |
| `zerodds-corba-bridged` | `corba-bridged` | 23 | `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs:30` |

Without `--config`, a bridge daemon reads its own system file (`/etc/zerodds/ws-bridged.yaml` etc.) and not `zerodds.yaml`. The runtime keys (`transport.*`, `discovery.*` …) may be placed in the same bridge file and apply there to the DDS side of the daemon. Measured: `transport.recv_batch: 64` in a `--config` file of `zerodds-ws-bridged` appears under `values:` with file and line.

The RMW takes `--config` and `--set` from the command line of the ROS process, outside the `--ros-args … --` blocks, which belong to rcl (`application_args`, `ros2_config.rs:74`). `--check-config` on the command line of a node is not evaluated; the report for it is `zerodds-ros2-shim --check-config` with the same flags (per doc comment, `ros2_config.rs:96`).

### 1.4 Behavior at startup

| Finding | Runtime and CLI tools | Bridge daemons | RMW |
|---|---|---|---|
| invalid value of a known setting | start aborted | start aborted | `rmw_init` fails |
| unknown key in a file | warning on stderr: `warning: <origin>: unknown setting <path> is not read` (`cli.rs:170`) | inside a known section: error from `from_config`; otherwise warning | unknown `ros2.*` key: error |
| `--set` on a path that nothing reads | error `--set <path>: not a known setting` | error | error |
| file unreadable, syntax error, include cycle | error | error | error |

The message always names the layer: `file:line`, `environment $NAME` or the `--set`. Measured with `ZERODDS_RECV_BATCH=abc zerodds-spy --check-config`: `error: environment $ZERODDS_RECV_BATCH: invalid transport.recv_batch: got `abc`, expected an integer > 0`.

### 1.5 `--check-config`

`--check-config` loads all four layers as a start does and then prints (`ConfigSource::check_report`, `crates/dcps/src/runtime.rs:3371`):

1. `sources:` the sources involved, in reading order (`environment`, files, `command line`), otherwise `(none; built-in defaults)`,
2. `values:` each of the 51 runtime settings with value and origin, defaults with `[default]`,
3. `program settings:` the program's own keys that are set (only bridges and `zerodds-ros2-shim`; secrets as `***`),
4. one line `error: …` per finding,
5. `configuration valid` or `configuration invalid`.

`zerodds-ros2-shim --check-config` appends a block `ros2:` with mangling, profiles and overrides.

Exit codes (code: `cli.rs:186` and the `main` functions; measured with the binaries in `target/debug`):

| Situation | CLI tools, router, durability service | Bridge daemons |
|---|---|---|
| valid | 0 | 0 |
| invalid value or unknown key | 1 | 1 |
| unreadable file, syntax error, include error, unknown `--set` path | 2 | 1 |

Measured: `zerodds-spy --config /nonexist.yaml --check-config` → 2; `zerodds-ws-bridged --config /nonexist.yaml --check-config` → 2, without `--check-config` → 1; `zerodds-amqp-bridged` with syntax error → 2 under `--check-config`, 1 at start; `zerodds-spy --set transport.tick=3 --check-config` → 2; `zerodds-spy --set runtime.tick_period_ms=0 --check-config` → 1.

## 2 Runtime settings (`zerodds.yaml`)

### 2.1 Result

- 101 settings are in `SETTINGS` (`crates/dcps/src/runtime.rs:3034`), each with file path and environment variable.
- 1 setting is possible only in files: `transport.interface_bindings` (`FILE_SETTINGS`, `runtime.rs:4089`), because its value is a list of maps.
- In total 102 runtime settings. All 101 `SETTINGS` entries are read through `source.var(…)` while `from_validated_source` (`runtime.rs:4273`) builds the configuration (cross-checked for every name): 89 there, the 7 `ports.*` among them through the helper `port`, `transport.shm_max_datagram` under its file path instead of the variable name (`ConfigSource::var` takes either); the 6 `security.*` forms in `SecurityVendorSettings::from_source` (`runtime.rs:2979`) and the 6 `transport.udp.*` in `udp_config_from` (`runtime.rs:4213`), which `from_validated_source` calls.
- In addition there are 6 further `ros2.*` paths that only the RMW reads (§2.4). `ros2.time_based_filter` is in both tables.

Column "Default": text from `runtime_default_text` (`runtime.rs:3686`), which `zerodds-spy --check-config` and `zerodds-ros2-shim --check-config` print without files and without environment; for this state read from the code and checked against `man/man5/zerodds.yaml.5`, not measured with the binaries. "Location" is the line in `crates/dcps/src/runtime.rs` at which the value is read, in `from_validated_source` or in the function named; value ranges from `valid_runtime_value` (`runtime.rs:3916`) and `runtime_expectation` (`runtime.rs:3819`).

Booleans: `true`/`false` or `1`/`0` (`runtime_boolean`, `runtime.rs:4182`). Lists: in files as a sequence, in the environment and `--set` as a comma-separated list (`runtime_list`, `runtime.rs:4058`).

### 2.2 Table

| Path | Type/values | Default | Environment variable | Effect | Location |
|---|---|---|---|---|---|
| `runtime.tick_period_ms` | ms, integer > 0 | 5 | `ZERODDS_TICK_PERIOD_MS` | period of the runtime tick (heartbeats, timers) | 4295 |
| `runtime.scheduler_tick` | bool | true | `ZERODDS_SCHEDULER_TICK` | event-driven deadline-heap scheduler; `false` = fixed `tick_loop` | 4692 |
| `transport.max_message_size` | bytes, ≥ `DEFAULT_MTU` | 14720 | `ZERODDS_MAX_MESSAGE_SIZE` | largest RTPS message when sending | 4343 |
| `transport.shm_max_datagram` | bytes, > 0 | 65536 | `ZERODDS_SHM_MAX_DATAGRAM` | datagram upper limit of the same-host SHM transport (feature SHM required) | 4325 |
| `transport.same_host_shm` | bool | `true` with feature `same-host-shm`, else `false` | `ZERODDS_SAME_HOST_SHM` | same-host SHM for a writer and reader on one host; `false` = every pair over the user transport; `true` without the feature makes `DcpsRuntime::start` refuse | 4332 |
| `transport.recv_batch` | integer > 0 | 32 | `ZERODDS_RECV_BATCH` | datagrams per `recv_batch` call; `1` = single `recv()` | 4353 |
| `transport.batch_at_zero_budget` | bool | false | `ZERODDS_BATCH_AT_ZERO_BUDGET` | measurement switch for batching at latency budget 0 | 4358 |
| `transport.interface` | IPv4 address | (auto) | `ZERODDS_INTERFACE` | interface for announce and bind; unset = route probe | 4409 |
| `transport.user_transport` | `UDPv4`, `UDPv6`, `TCPv4`, `TCPv6`; `SHM`, `UDS`, `TSN` only with feature | UDPv4 | `ZERODDS_USER_TRANSPORT` | transport of the user data; discovery stays UDPv4 | 4469 |
| `transport.user_transports` | list of the same values | (none) | `ZERODDS_USER_TRANSPORTS` | several user-data transports, first preferred; non-empty = `user_transport` ignored | 4473 |
| `transport.user_multicast` | bool | true | `ZERODDS_USER_MULTICAST` | joins the SPDP group on the port PB + DG · domainId + d2 (DDSI-RTPS 2.5 §9.6.2.3), receives user data there and announces the locator as the default multicast locator; a peer may then send user data by multicast, ZeroDDS writers send to unicast locators only; a refused join is logged and the participant runs without it; `false` for a host without multicast | 4340 |
| `transport.interface_bindings` | list of maps (§2.3), file only | (none) | – | interface bindings for `NetInterface`-bound security rules | 4492 |
| `reader.max_samples_per_proxy` | integer > 0 | 65536 | `ZERODDS_MAX_SAMPLES_PER_PROXY` | samples that a reader buffers per writer proxy during a gap | 4348 |
| `reader.max_reassembly_sample_bytes` | bytes, > 0 | 16777216 | `ZERODDS_MAX_SAMPLE_BYTES` | upper limit of a reassembled sample (DoS protection) | 4432 |
| `reader.heartbeat_response_delay_ms` | ms, integer ≥ 0 | 500 | `ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS` | how long a reliable reader waits between a HEARTBEAT and its response (DDSI-RTPS 2.5 §8.4.10.1 `heartbeatResponseDelay`; the spec's default); `0` answers on the next tick; applies to the application's readers, the built-in readers keep the default | 4522 |
| `reader.heartbeat_suppression_duration_ms` | ms, integer ≥ 0 | 0 | `ZERODDS_READER_HEARTBEAT_SUPPRESSION_DURATION_MS` | a HEARTBEAT that arrives less than this long after the previous one of the same writer is ignored (§8.4.10.1 `heartbeatSuppressionDuration`; the spec's default) | 4529 |
| `reader.honor_data_max_size_serialized` | bool | true | `ZERODDS_READER_HONOR_DATA_MAX_SIZE_SERIALIZED` | a reader refuses a SerializedPayload larger than the `dataMaxSizeSerialized` the writer announced (DDSI-RTPS 2.5 §8.4.10.4, Table 8.72); `false` takes it, for a writer that exceeds its own announcement (Fast DDS 3.6.1: 160 octets announced, 100 036 sent, OMG dds-rtps Test_LargeData_0) | 4533 |
| `reader.dispose_with_unregister` | bool | true | `ZERODDS_READER_DISPOSE_WITH_UNREGISTER` | a reader takes a lifecycle marker with the DisposedFlag and the UnregisteredFlag (PID_STATUS_INFO, DDSI-RTPS 2.5 §9.6.3.9) as a dispose and an unregister; `false` takes it as an unregister only, and the instance goes to NOT_ALIVE_NO_WRITERS once its last writer is gone (OMG dds-rtps Test_FinalInstanceState_2 with a publisher that disposes and unregisters in one marker when its writer is deleted, measured OpenDDS 3.35.0-dev) | 4536 |
| `reader.time_based_filter_clock` | `sender`, `receiver` | sender | `ZERODDS_READER_TIME_BASED_FILTER_CLOCK` | the clock on which TIME_BASED_FILTER measures `minimum_separation` (DDS 1.4 §2.2.3.12 leaves it open): `sender` = source timestamp of the change (INFO_TS), the reception time for a sample without one; `receiver` = reception time | 4379 |
| `writer.nack_response_delay_ms` | ms, integer ≥ 0 | 200 | `ZERODDS_NACK_RESPONSE_DELAY_MS` | how long a reliable writer waits between a NACK and the repair (DDSI-RTPS 2.5 §8.4.7.1 `nackResponseDelay`; the spec's default is 200) | 4507 |
| `writer.heartbeat_period_ms` | ms, integer > 0 | 100 | `ZERODDS_WRITER_HEARTBEAT_PERIOD_MS` | how often a reliable writer announces its history to a reader that has not acknowledged everything (DDSI-RTPS 2.5 §8.4.7.1 `heartbeatPeriod`; §8.4.7.1 names no value) | 4514 |
| `writer.nack_suppression_duration_ms` | ms, integer ≥ 0 | 0 | `ZERODDS_NACK_SUPPRESSION_DURATION_MS` | a NACK for a change sent less than this long ago is ignored (§8.4.7.1 `nackSuppressionDuration`) | 4539 |
| `writer.unacked_window_bytes` | bytes, integer ≥ 0 | 524288 | `ZERODDS_WRITER_UNACKED_WINDOW` | flow control of a writer with reliable readers: this many bytes of payload may be on their way unacknowledged, beyond it `write()` waits for acknowledgements up to `max_blocking_time`, then sends anyway (TIMEOUT comes only from the RESOURCE_LIMITS); `0` = none | 4546 |
| `writer.unregister_on_delete` | bool | true | `ZERODDS_WRITER_UNREGISTER_ON_DELETE` | on deleting a DataWriter send an unregister per instance and, with WRITER_DATA_LIFECYCLE `autodispose_unregistered_instances`, a dispose (DDS 1.4 §2.2.2.4.1.6, §2.2.3.21); `false` sends neither, the readers take the instances to NOT_ALIVE_NO_WRITERS with the publication's SEDP dispose, as with Connext, Fast DDS, CoreDX and HDDS as the publisher (OMG dds-rtps Test_FinalInstanceState_2) | 4560 |
| `writer.linger_ms` | ms, integer ≥ 0 | 1000 | `ZERODDS_WRITER_LINGER_MS` | how long deleting a reliable DataWriter waits for its reliable readers to acknowledge what it sent, the unregister and dispose markers of DDS 1.4 §2.2.2.4.1.6 included, before the writer goes and its SEDP dispose is sent; a writer whose readers have acknowledged everything goes at once; `0` = at once | 4563 |
| `writer.group_coherent_set_pid` | bool | true | `ZERODDS_WRITER_GROUP_COHERENT_SET_PID` | `PID_GROUP_COHERENT_SET` on the DATA of a group coherent set under PRESENTATION `access_scope` GROUP, as DDSI-RTPS 2.5 §8.7.6 requires; `false` leaves it out for every reader: HDDS and Connext read group coherent sets right only without it (OMG dds-rtps Test_CoherentSets_12, _18, _21), and a ZeroDDS subscriber then no longer knows a set's members | 4567 |
| `writer.group_info_on_wire` | bool | true | `ZERODDS_WRITER_GROUP_INFO_ON_WIRE` | send the group information of DDSI-RTPS 2.5 §8.7.5/§8.7.6 under PRESENTATION `access_scope` GROUP: `PID_GROUP_SEQ_NUM`, `PID_WRITER_GROUP_INFO`, `PID_GROUP_COHERENT_SET` and the GroupInfoFlag of HEARTBEAT and GAP; `false` sends each writer's coherent set as under TOPIC, the form HDDS and zzdds send and Connext, HDDS and zzdds read (OMG dds-rtps Test_CoherentSets_12, _18, _21); a ZeroDDS subscriber then orders by writer instead of by group. For the participants of one vendor only: `DcpsRuntime::set_vendor_rtps_behaviour` | 4570 |
| `writer.keyless_lifecycle_on_wire` | bool | false | `ZERODDS_WRITER_KEYLESS_LIFECYCLE_ON_WIRE` | send the dispose and unregister of a writer of a topic without a key; DDSI-RTPS 2.5 §8.2.10.1 gives both "no effect if the topicKind==NO_KEY", so by default only the writer's own instance state changes (DDS 1.4 §2.2.1.2.2, one instance), no marker is sent and `DcpsRuntime::keyless_lifecycle_not_sent` counts it; `true` sends the marker (status bits only) for a peer that expects it | 4573 |
| `dcps.presentation_instance_coherence` | `per_instance`, `none` | per_instance | `ZERODDS_PRESENTATION_INSTANCE_COHERENCE` | coherent sets under PRESENTATION `access_scope` INSTANCE: `per_instance` follows the table in DDS 1.4 §2.2.3.6 (the changes of a set to one instance are presented together once the set is closed; checked by OMG dds-rtps Test_CoherentSets_10 and _19), `none` the text below it (the samples are presented as they arrive) | 4383 |
| `discovery.participant_name` | non-empty string | zerodds:<GUID prefix in hex> | `ZERODDS_PARTICIPANT_NAME` | the name the participant announces as `PID_ENTITY_NAME` (EntityName_t, DDSI-RTPS 2.5 Table 9.18); the endpoints announce a name built from kind, topic and entity id | 4337 |
| `discovery.spdp_multicast_group` | IPv4 multicast | 239.255.0.1 | `ZERODDS_SPDP_MC_GROUP` | SPDP multicast group | 4399 |
| `discovery.spdp_period_ms` | ms, > 0 | 30000 | `ZERODDS_SPDP_PERIOD_MS` | SPDP announce period; without a value of its own, with a lease below 90 s a third of `discovery.participant_lease_duration_ms`, at least 100 ms (`default_spdp_period_for`) | 4283 |
| `discovery.spdp_new_sequence_per_period` | bool | false | `ZERODDS_SPDP_NEW_SEQUENCE_PER_PERIOD` | `true` = every SPDP announcement with a new sequence number, as Cyclone DDS and OpenDDS send; by default, after DDSI-RTPS 2.5 §8.5.3.1, a new number only with changed participant data | 4550 |
| `discovery.initial_announce_count` | integer ≥ 0 | 10 | `ZERODDS_INITIAL_ANNOUNCE_COUNT` | number of start announcements and of directed responses to a newly discovered participant; 0 disables the start announcements and leaves one response | 4303 |
| `discovery.initial_announce_period_ms` | ms, > 0 | 200 | `ZERODDS_INITIAL_ANNOUNCE_PERIOD_MS` | interval of the start announcements and of the responses | 4307 |
| `discovery.participant_lease_duration_ms` | ms, > 0 | 100000 | `ZERODDS_PARTICIPANT_LEASE_MS` | announced participant lease | 4275 |
| `discovery.wlp_period_ms` | ms, 0 = one third of the lease | 0 | `ZERODDS_WLP_PERIOD_MS` | WLP period | 4503 |
| `discovery.peers` | list of IPv4 or IPv4:port | (none) | `ZERODDS_PEERS` | unicast peers; without port expanded depending on the domain | 4442 |
| `discovery.max_peer_participants` | 1..=120 | 10 | `ZERODDS_MAX_PEER_PARTICIPANTS` | participant IDs per peer during expansion | 4446 |
| `discovery.no_multicast` | bool | false | `ZERODDS_NO_MULTICAST` | SPDP multicast sending off, pure unicast discovery | 4423 |
| `discovery.multicast_allowlist` | list of IPv4 multicast | (none) | `ZERODDS_MULTICAST_ALLOWLIST` | allowed multicast groups; empty = no restriction | 4414 |
| `discovery.server` | bool | false | `ZERODDS_DISCOVERY_SERVER` | participant works as SPDP relay | 4427 |
| `discovery.domain_tag` | text | (none) | `ZERODDS_DOMAIN_TAG` | DDSI-RTPS 2.5 §8.5.3.2: set → `PID_DOMAIN_TAG` in SPDP; participants with a different tag (also without a tag) are not discovered | 4373 |
| `ports.port_base` | 0..=65535 | 7400 | `ZERODDS_PORT_BASE` | PB of the port mapping of DDSI-RTPS 2.5 §9.6.2.3 (default: the spec's value); peers that use other values have to be given the same values | 4459 |
| `ports.domain_gain` | 0..=65535 | 250 | `ZERODDS_PORT_DOMAIN_GAIN` | DG of the port mapping (§9.6.2.3) | 4460 |
| `ports.participant_gain` | 0..=65535 | 2 | `ZERODDS_PORT_PARTICIPANT_GAIN` | PG of the port mapping (§9.6.2.3) | 4461 |
| `ports.d0` | 0..=65535 | 0 | `ZERODDS_PORT_D0` | SPDP multicast port = PB + DG · domainId + d0 | 4462 |
| `ports.d1` | 0..=65535 | 10 | `ZERODDS_PORT_D1` | SPDP unicast port = PB + DG · domainId + d1 + PG · participantId | 4463 |
| `ports.d2` | 0..=65535 | 1 | `ZERODDS_PORT_D2` | user-traffic multicast port = PB + DG · domainId + d2; bound when `transport.user_multicast` is on | 4464 |
| `ports.d3` | 0..=65535 | 11 | `ZERODDS_PORT_D3` | user-traffic unicast port = PB + DG · domainId + d3 + PG · participantId; not bound, the user-traffic unicast sockets take an ephemeral port, which the participant announces | 4465 |
| `security.secure_spdp` | bool | true | `ZERODDS_SECURE_SPDP` | reliable secure SPDP channel (0xff0101) of a secured participant; Fast DDS waits for it | 4499 |
| `security.enclave_dir` | non-empty path | (none) | `ZERODDS_SECURITY_DIR` | SROS2 enclave directory (§5); for consumers see §5.1 | 4370 |
| `security.kx_key_fallback` | `spec`, `accept` | spec | `ZERODDS_SEC_KX_KEY_FALLBACK` | `spec`: the Kx key belongs to the channel `BuiltinParticipantVolatileMessageSecure` only (DDS-Security 1.2 §10.5.2.1.2); `accept` also tries a submessage or payload that no key of its sender opens with that key, for a peer that protects discovery or user data with it; not measured against any vendor | 2970 (`SecurityVendorSettings::from_source`) |
| `security.compatibility_check` | `spec`, `ignore` | spec | `ZERODDS_SEC_COMPATIBILITY_CHECK` | `spec` keeps participants and endpoints apart whose announced security masks and algorithm infos are not compatible (DDS-Security 1.2 §7.3.10 to §7.3.24); `ignore` for a peer whose announced masks differ from what it enforces | 2971 (`SecurityVendorSettings::from_source`) |
| `security.algo_name_nul` | `spec`, `omit`, `mirror`, `by_vendor` | spec | `ZERODDS_SEC_ALGO_NAME_NUL` | how the algorithm names `c.dsign_algo` and `c.kagree_algo` of the handshake token are written: `spec` counts the terminating NUL in the length (DDS-Security 1.2 §7.3.3.1); `omit` never writes it (Fast DDS 3.6 writes and compares without it); `mirror` writes it as an initiator and answers in the form of the request; `by_vendor` writes it for an OpenDDS peer only (OpenDDS requires it); Fast DDS and OpenDDS from the sources read | 2972 (`SecurityVendorSettings::from_source`) |
| `security.algorithm_names` | `auto`, `1.2`, `1.1` | 1.2 | `ZERODDS_SEC_ALGORITHM_NAMES` | spelling of the algorithm names the initiator sends: `1.2` those of DDS-Security 1.2 Tables 25 and 26 (ECDSA+P256+SHA256, ECDHE-CEUM+P256, DHE+MODP-2048-256); `1.1` those marked there "deprecated in v 1.2" (ECDSA-SHA256, ECDH+prime256v1-CEUM, DH+MODP-2048-256), which Cyclone DDS, Fast DDS and OpenDDS compare with (sources read, not measured); `auto` sends those of 1.2 to a peer whose IdentityToken names 1.2 and those of 1.1 to a peer of 1.0; both are accepted when received | 2973 (`SecurityVendorSettings::from_source`) |
| `security.subject_name_form` | `rfc4514`, `x509_name_oneline`, `xn_flag_oneline` | rfc4514 | `ZERODDS_SEC_SUBJECT_NAME_FORM` | form of the subject names in `dds.cert.sn` and `dds.ca.sn` of the IdentityToken and `dds.perm_ca.sn` of the PermissionsToken: `rfc4514` after DDS-Security 1.2 §9.4.1.2.3 (`CN=y,O=x,C=DE`); `x509_name_oneline` the form `/C=DE/O=x/CN=y` of OpenSSL `X509_NAME_oneline` (Fast DDS 3.6 compares a received `dds.perm_ca.sn` with it, Cyclone DDS writes it); `xn_flag_oneline` the form `C = DE, O = x, CN = y` of `XN_FLAG_ONELINE` (OpenDDS compares a received `dds.ca.sn` with it); the comparisons are from the sources read, ZeroDDS compares no received subject name | 2974 (`SecurityVendorSettings::from_source`) |
| `security.origin_authentication_fallback` | `spec`, `accept` | spec | `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK` | what a participant whose governance asks for origin authentication (`rtps_protection_kind` or `metadata_protection_kind` `SIGN_WITH_ORIGIN_AUTHENTICATION`/`ENCRYPT_WITH_ORIGIN_AUTHENTICATION`) does with a message or submessage under a key without a receiver-specific key: `spec` refuses it (DDS-Security 1.2 §10.5.3.3 Table 80 `decode_rtps_message`); `accept` verifies it with the common MAC alone, as Table 80 does for a `receiver_specific_key_id` of zero; a key that carries a receiver-specific key is held to it under either value; for OpenDDS 3.34, which makes no receiver-specific keys | 2976 (`SecurityVendorSettings::from_source`) |
| `tsn.interface` | non-empty interface name | (none) | `ZERODDS_TSN_IFACE` | interface of the TSN transport; only read with `user_transport: TSN` | 4719 |
| `tsn.vlan` | 0..=4094 | 0 | `ZERODDS_TSN_VLAN` | VLAN ID of the TSN transport | 4721 |
| `tsn.pcp` | 0..=7 | 0 | `ZERODDS_TSN_PCP` | Priority Code Point of the TSN transport | 4725 |
| `threads.recv_priority` | SCHED_FIFO/SCHED_RR 1..=99 (SCHED_FIFO without `threads.*_policy`) | (none) | `ZERODDS_RECV_THREAD_PRIORITY` | real-time priority of the receive threads; needs `CAP_SYS_NICE` or `RLIMIT_RTPRIO` | 4599 |
| `threads.tick_priority` | SCHED_FIFO/SCHED_RR 1..=99 (SCHED_FIFO without `threads.*_policy`) | (none) | `ZERODDS_TICK_THREAD_PRIORITY` | real-time priority of the tick thread | 4602 |
| `threads.recv_policy` | `other`, `fifo`, `rr`, `deadline` | (none) | `ZERODDS_RECV_THREAD_POLICY` | scheduler class of the receive threads; unset with `threads.recv_priority` set means `fifo`, unset with no other `threads.*` setting means no real-time scheduling and no affinity; a priority with `other` or `deadline`, a `threads.recv_deadline` without `deadline`, `fifo` or `rr` without a priority and `deadline` without `threads.recv_deadline` are each a startup error naming the key | 4612 |
| `threads.tick_policy` | `other`, `fifo`, `rr`, `deadline` | (none) | `ZERODDS_TICK_THREAD_POLICY` | like `threads.recv_policy`, for the tick thread | 4616 |
| `threads.recv_deadline` | `runtime,deadline,period` in ns, runtime ≤ deadline ≤ period | (none) | `ZERODDS_RECV_THREAD_DEADLINE` | SCHED_DEADLINE reservation of the receive threads (with `threads.recv_policy: deadline`); always needs `CAP_SYS_NICE`; a reservation beyond the kernel's global bandwidth limit fails with `EBUSY` | 4620 |
| `threads.tick_deadline` | `runtime,deadline,period` in ns | (none) | `ZERODDS_TICK_THREAD_DEADLINE` | like `threads.recv_deadline`, for the tick thread | 4624 |
| `threads.recv_cpus` | list of CPU indices (< 4096) | (none) | `ZERODDS_RECV_THREAD_CPUS` | CPU affinity of the receive threads | 4628 |
| `threads.tick_cpus` | list of CPU indices | (none) | `ZERODDS_TICK_THREAD_CPUS` | CPU affinity of the tick thread | 4631 |
| `threads.extra_recv` | 0..=64 | 0 | `ZERODDS_EXTRA_RECV_THREADS` | additional receive workers | 4634 |
| `wire.rtps_dialect` | `cyclone`, `fastdds`, `opendds`, `maximally-interoperable` | maximally-interoperable | `ZERODDS_RTPS_DIALECT` | behavior table for vendor deviations | 4316 |
| `wire.codec_dialect` | the names `zerodds-idlc generate --dialect` takes (`specification`, `cyclone`, `rti`, `fastdds`, `opendds`, …) | specification | `ZERODDS_CODEC_DIALECT` | CDR dialect of the reflective runtime codecs: body check and key derivation for the language bindings (`SerializedTypeSupport`), webdds-bridge, ROS 2 dynamic type support, default of `spy`/`record --dialect`; the dialect the types were generated in | 4321 |
| `wire.encapsulation_ids` | `table-60`, `table-39` | table-60 | `ZERODDS_WIRE_ENCAPSULATION_IDS` | identifiers in the encapsulation header of a user sample: `table-60` those of DDS-XTypes 1.3 Table 60 (§7.6.3.1.2: 0x0006 to 0x000b for XCDR2, 0x0004 for XML), which Cyclone DDS, Fast DDS, OpenDDS and RTI Connext send; `table-39` those of Table 39 (§7.4.3.4: 0x0010 to 0x0015, {0x01, 0x00} for XML); both are read, the XCDR1 identifiers do not change | 4703 |
| `wire.pl_cdr1_headers` | `dialect`, `specification`, `short-rounded-length`, `always-extended`, `short-true-length`, `extended-for-constructed`, `extended-for-constructed-in-large-samples` | dialect | `ZERODDS_WIRE_PL_CDR1_HEADERS` | PL_CDR1 header policy of the reflective codecs, in place of that of `wire.codec_dialect`; `dialect` keeps the dialect's; `specification` writes and expects the exact parameter length of the member (DDS-XTypes 1.3 §7.4.1.2), `short-rounded-length` the length rounded up to four (DDSI-RTPS 2.5 §9.4.2.11); holds for the whole process | 4707 |
| `wire.pl_cdr1_reading` | `strict`, `tolerant` | strict | `ZERODDS_WIRE_PL_CDR1_READING` | `strict`: the reflective PL_CDR1 reader takes the framing it is configured for and reports the flags, header form and length convention of another framing; `tolerant` accepts any flags on PID_EXTENDED and PID_LIST_END, the short and the extended header and a length that is the member's own or rounded up to four (DDS-XTypes 1.3 §7.4.1.2.1, §7.4.1.2.2); holds for the whole process | 4710 |
| `wire.type_lookup_encoding` | `xcdr1`, `xcdr2` | xcdr2 | `ZERODDS_WIRE_TYPE_LOOKUP_ENCODING` | encoding of the requests and replies of the TypeLookup service: `xcdr2` = CDR2_LE {0x00,0x07} with encoding version 2, the form of DDS-XTypes 1.4 Beta 1 §7.6.3.3.3 (OMG issue DDSXTY14-29), as Cyclone DDS 11.0.1, Fast DDS 3.6.1, OpenDDS 3.34.0, RTI Connext 7.7.0 and InterCom DDS 4.3.1 write; `xcdr1` = CDR_LE {0x00,0x01} with version 1, the reading of DDS-XTypes 1.3 and DDSI-RTPS 2.5 §10.4 Table 10.2; Cyclone DDS, OpenDDS and RTI Connext refuse CDR_LE, Fast DDS answers it (measured); a received sample is read in the encoding its identifier names | 4714 |
| `wire.data_representation_offer` | list of `XCDR1`/`XCDR`/`1`, `XCDR2`/`2` | XCDR1 | `ZERODDS_DATA_REPR_OFFER` | offered DataRepresentation, first = encoding of the writers; default of XTypes 1.3 §7.6.3.1.1; a reader also requests XCDR2 and XCDR | 4642 |
| `transport.tcp_locator_kinds` | `vendor`, `fast-dds` | vendor | `ZERODDS_TCP_LOCATOR_KINDS` | LocatorKind of TCP locators: own values in the vendor range (§9.3.2.4) or 4 and 8 as Fast DDS; reading accepts both | 4660 |
| `wire.data_representation_match` | `strict`, `tolerant` | strict | `ZERODDS_DATA_REPR_MATCH` | matching rule of the DataRepresentation | 4654 |
| `wire.big_endian` | bool | false | `ZERODDS_WIRE_BIG_ENDIAN` | writers encode big endian (`*_BE`) | 4361 |
| `wire.clock_skew_threshold_s` | s, integer ≥ 0 | 0 (off) | `ZERODDS_WIRE_CLOCK_SKEW_THRESHOLD_S` | CLOCK_SKEW_DETECTED (DDSI-RTPS 2.5 §8.3.7.4): largest difference between the reception time and the `rtpsSendTimestamp` of the HeaderExtension; beyond it `clockSkewDetected` is set and the following InfoTimestamp is moved by the difference (§8.3.8.11); the spec gives no threshold, `0` = off | 4553 |
| `wire.accept_empty_acknack_base_zero` | bool | false | `ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO` | accept an ACKNACK whose `readerSNState` is empty (numBits 0) with `bitmapBase` 0; DDSI-RTPS 2.5 §9.4.2.6 requires `bitmapBase` ≥ 1, so by default such an ACKNACK is invalid and the rest of its message with it; OpenDDS 3.34.0 and RTI Connext 7.7.0 send it as the ACKNACK of a reader that holds nothing; holds for the whole process and is never turned off again | 4557 |
| `wire.omit_heartbeat_group_digests` | bool | false | `ZERODDS_WIRE_OMIT_HEARTBEAT_GROUP_DIGESTS` | leave `writerSet` and `secureWriterSet` out of the HEARTBEAT that carries group information (GroupInfoFlag); DDSI-RTPS 2.5 Table 8.44 and §8.7.5 ask for both, the figure of §9.4.5.7 ends the HEARTBEAT after `lastGSN`; `true` sends that form, a reader accepts both | 4576 |
| `discovery.type_object_v1` | bool | false | `ZERODDS_TYPE_OBJECT_V1` | Endpoints announce their type also as the `TypeObjectV1` (`type`, PID 0x0072) and the `TypeIdV1` (`type_id`, 0x0069) of XTypes 1.1 Annex B, the optional members of the built-in topic data of XTypes 1.3 Annex D; a type that has no form in Annex B of 1.1 goes out without them | 4364 |
| `xtypes.key_member_bounds` | `enforced`, `ignored` | enforced | `ZERODDS_KEY_MEMBER_BOUNDS` | XTypes 1.3 table 19 rules for key members in assignability | 4673 |
| `xtypes.strong_assignability` | `equivalence`, `names_ignored` | equivalence | `ZERODDS_STRONG_ASSIGNABILITY` | what strong assignability (XTypes 1.3 §7.2.4.3) asks of an element or member type that is not delimited: equivalence under the MINIMAL relation, member names included, or, with `names_ignored`, assignability in both directions without member names | 4678 |
| `xtypes.type_consistency_absent` | `disallow`, `allow` | disallow | `ZERODDS_TYPE_CONSISTENCY_ABSENT` | what a remote reader that announces no TypeConsistencyEnforcement (PID 0x0074) requests; `disallow` is XTypes 1.3 7.6.3.4.1 | 4663 |
| `xtypes.annotation_parameter_order` | `member-index`, `name` | member-index | `ZERODDS_ANNOTATION_PARAMETER_ORDER` | order of the parameters of a `CompleteAnnotationType`: `member-index` (declaration order) is XTypes 1.3 §7.3.4.5, `name` the comment in Annex B; the order is in the object and so in the hash of `annotation_typeid`; applies to the whole process | 4668 |
| `durability.dir` | non-empty path | (temp dir)/zerodds-durability | `ZERODDS_DURABILITY_DIR` | root of the store for PERSISTENT writers (`publisher.rs:858`) | 4367 |
| `delivery.mode` | `portable`, `raw-same-host`, `iceoryx` | portable | `ZERODDS_DELIVERY_MODE` | delivery path of the loan writers (C-API) and of the RMW publishers with a loanable type; `DataWriter::write` is always `portable`; where the mode does not apply, a warning (`zerodds-delivery-modes-1.0` §4) | 4375 |
| `time.source` | `system`, `tai` (Linux only, `CLOCK_TAI`) | system | `ZERODDS_TIME_SOURCE` | clock behind SOURCE_TIMESTAMP, DESTINATION_ORDER BY_SOURCE_TIMESTAMP, the defaults of `write_w_timestamp` and the reader's reception clock for LIFESPAN/BY_RECEPTION_TIMESTAMP (`crates/dcps/src/time_source.rs`); `tai` is refused on a build or host that cannot honour it, with no fallback to `system`; a programmatic source is set on `RuntimeConfig::time_source` directly | 4597 |
| `diagnostics.observability` | `off`/`none`/`0`/`false`/empty or `stderr`/`json`/`stderr-json`/`1`/`on`/`true` | off | `ZERODDS_OBSERVABILITY` | event sink; `stderr-json` = JSON lines on stderr | 4588 |
| `diagnostics.trace_send_errors` | bool | false | `ZERODDS_TRACE_SEND_ERR` | failed sends on stderr | 4731 |
| `diagnostics.phase_timing` | bool | false | `ZERODDS_PHASE_TIMING` | latency counters per phase | 4734 |
| `diagnostics.phase_dump` | bool | false | `ZERODDS_PHASE_DUMP` | print phase counters when the runtime is dropped | 4733 |
| `ros2.time_based_filter` | list of `SECONDS` or `/topic=SECONDS`, ≥ 0 | (none) | `ZERODDS_TIME_BASED_FILTER` | TIME_BASED_FILTER for ROS topics (rmw_qos_profile_t has no field for it) | 4387 |
| `transport.udp.connected_cache` | bool | true | `ZERODDS_UDP_CACHE_ENABLE` | cache of connected send sockets per destination; `false` = classic `send_to` | 4204 (`udp_config_from`) |
| `transport.udp.cache_max_entries` | > 0 | 16 | `ZERODDS_UDP_CACHE_MAX` | entries of the cache, FIFO eviction | 4207 (`udp_config_from`) |
| `transport.udp.cache_sndbuf` | bytes, 0 = kernel default | 262144 | `ZERODDS_UDP_CACHE_SNDBUF` | `SO_SNDBUF` of the cache sockets | 4210 (`udp_config_from`) |
| `transport.udp.recv_buffer` | bytes, 0 = kernel default | 1048576 | `ZERODDS_UDP_RCVBUF` | `SO_RCVBUF` of every bound UDP socket | 4211 (`udp_config_from`) |
| `transport.udp.dont_route` | bool | false | `ZERODDS_UDP_DONTROUTE` | `SO_DONTROUTE`; only without routing across subnet boundaries | 4213 (`udp_config_from`) |
| `transport.udp.max_message_size` | bytes, 20 to 65507 | 1472 | `ZERODDS_UDP_MAX_MESSAGE_SIZE` | largest RTPS message per datagram; the fragment size of a writer follows the smallest such size of its transports (DDSI-RTPS 2.5 §8.4.14.1) | 4216 (`udp_config_from`) |

Number of rows: 101. Man page `man/man5/zerodds.yaml.5`, example `packaging/linux/configs/zerodds.yaml.example` (all keys with default); `runtime_config_layering.rs::the_shipped_example_and_the_man_page_name_every_setting` keeps both in line with `SETTINGS`.

Programmatic presets without file keys: `RuntimeConfig::ros_defaults()` (`runtime.rs:4882`), `multi_robot()` (`:4894`, multicast off), `discovery_server()` (`:4920`). `RuntimeConfig::default()` reads neither files nor the environment (`runtime.rs:4247`).

### 2.3 `transport.interface_bindings`

Sequence of maps (`parse_interface_bindings`, `runtime.rs:4097`). Only with feature `security`; without the feature every entry is rejected ("needs a build with the `security` feature").

| Key | Required | Values |
|---|---|---|
| `name` | yes | name of the binding |
| `bind` | yes | `ip` or `ip:port` |
| `kind` | yes | `loopback`, `localhost`, `wan`, `subnet`, `named:<class>` |
| `subnet` | yes | CIDR, e.g. `10.0.0.0/24`; an address without prefix is /32 or /128 |
| `default` | no | `true`/`false` |

Other keys are rejected with "entry i: unknown key".

### 2.4 `ros2.*` (RMW only)

Reader: `Ros2Config::from_config`, `crates/rmw-zerodds-shim/src/ros2_config.rs`; key list `KEYS` (`ros2_config.rs:56`). Man page `man/man5/zerodds-ros2.yaml.5`, example `packaging/linux/configs/ros2.yaml.example` (intended as drop-in `zerodds.d/50-ros2.yaml`). Other programs do not flag `ros2.*` (`is_rmw_path`, `runtime.rs:4083`); the RMW checks strictly.

| Path | Values | Default | Effect |
|---|---|---|---|
| `ros2.topic_mangling` | `rmw`, `none`, `custom` | rmw | `rmw`: prefix `rt/` (REP-2007), services `rq/`…`Request`, `rr/`…`Reply`; `none`: name without leading `/`; `custom`: prefixes from `custom_mangling` |
| `ros2.custom_mangling.topic_prefix` | text | – | required with `custom`, otherwise rejected |
| `ros2.custom_mangling.request_prefix` | text | `rq/` | only with `custom` |
| `ros2.custom_mangling.response_prefix` | text | `rr/` | only with `custom` |
| `ros2.qos_profiles.<name>.{reliability,durability,history,depth,deadline_ms,lifespan_ms,liveliness,liveliness_lease_ms}` | reliability: `reliable`/`best_effort`/`system_default`; durability: `volatile`/`transient_local`/`system_default`; history: `keep_last`/`keep_all`/`system_default`; liveliness: `automatic`/`manual_by_topic`/`system_default`; `*_ms`: 0 = none | REP-2009 table: `sensor_data`, `services`, `parameters`, `parameter_events`, `default` | replaces fields of a standard profile or creates a new one |
| `ros2.topic_overrides[]` | entries with `ros_topic` and exactly one of `qos_profile` or `qos` (map as above) | empty | replaces, per ROS topic, the named fields of the requested QoS |
| `ros2.time_based_filter` | see §2.2 | – | see §2.2 |

Rejected with a reason (`OWNED_ELSEWHERE`, `ros2_config.rs:125`): `ros2.namespace` (rcl, `--ros-args -r __ns:=`), `ros2.ros_domain_id` (`ROS_DOMAIN_ID`), `ros2.enclave` (→ `security.enclave_dir`), `logging` (rcutils, `--ros-args --log-level`), `discovery.multicast` (→ `discovery.no_multicast`), `discovery.unicast_initial_peers` (→ `discovery.peers`).

Measured: `zerodds-ros2-shim --config packaging/linux/configs/ros2.yaml.example --check-config` → `configuration valid`, two overrides (`/scan -> rt/scan (sensor_data)`, `/map -> rt/map (qos)`).

## 3 Daemon files

### 3.1 Overview

| File | Daemon | Keys (`KEYS`) | own environment variables | Man page | Example |
|---|---|---|---|---|---|
| `ws-bridged.yaml` | `zerodds-ws-bridged` | 17, `crates/websocket-bridge/src/daemon/config.rs:29` | – | `man/man5/zerodds-ws-bridged.yaml.5` | `packaging/linux/configs/ws-bridged.yaml.example` |
| `mqtt-bridged.yaml` | `zerodds-mqtt-bridged` | 25, `crates/mqtt-bridge/src/daemon/config.rs:25` | `MQTT_PASSWORD` → `mqtt.password` (`:55`) | `man/man5/zerodds-mqtt-bridged.yaml.5` | `packaging/linux/configs/mqtt-bridged.yaml.example` |
| `coap-bridged.yaml` | `zerodds-coap-bridged` | 32, `crates/coap-bridge/src/daemon/config.rs:24` | – | `man/man5/zerodds-coap-bridged.yaml.5` | `packaging/linux/configs/coap-bridged.yaml.example` |
| `amqp-bridged.yaml` | `zerodds-amqp-bridged` | 28, `crates/amqp-endpoint/src/bridge_daemon.rs:243` | `AMQP_PASSWORD` → `amqp.sasl.password` (`:281`) | `man/man5/zerodds-amqp-bridged.yaml.5` | `packaging/linux/configs/amqp-bridged.yaml.example` |
| `grpc-bridged.yaml` | `zerodds-grpc-bridged` | 19, `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1460` | – | `man/man5/zerodds-grpc-bridged.yaml.5` | `packaging/linux/configs/grpc-bridged.yaml.example` |
| `corba-bridged.yaml` | `zerodds-corba-bridged` | 23, `crates/corba-dds-bridge/src/bridge_daemon.rs:275` | – | `man/man5/zerodds-corba-bridged.yaml.5` | `packaging/linux/configs/corba-bridged.yaml.example` |

All six example files were checked with the respective binary (`--config <example> --check-config`): each `configuration valid`, exit 0. Every name from `KEYS` occurs in the corresponding man page (text search for the last path segment).

Common to all six:

- `domain` (DDS domain, default 0), `log_level` (`trace`, `debug`, `info`, `warn`, `error`; default `info`).
- `metrics.enabled` (default false), `metrics.listen`. If `metrics.enabled: true` and `metrics.listen` is empty, each daemon binds its own port: ws `127.0.0.1:9091`, mqtt `:9092`, coap `:9093`, amqp `:9094`, grpc `:9095`, corba `:9096` (`DEFAULT_METRICS_LISTEN` per daemon; one test per crate checks the value against the man5 page). The example files set the same ports.
- Topic entries (`topics[]`, for CORBA `mappings[]`) may be written as a map or as short-form text; the short form is the form in which `--topic` reaches the `--set` layer.
- Unknown keys inside a known section are rejected by `Section::only` with file and line.
- On the signal SIGHUP the daemons read the same layers again (`reload_command_line`, e.g. `zerodds-ws-bridged.rs`).

### 3.2 `ws-bridged.yaml`

Defaults: `DaemonConfig::default_for_dev`, `crates/websocket-bridge/src/daemon/config.rs`.

| Key | Default | Meaning |
|---|---|---|
| `listen` | `127.0.0.1:8080` | WebSocket bind address |
| `tls.enabled`, `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` | false, empty | TLS; `client_ca_file` for mTLS |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject` | – | token and associated subject for `bearer` |
| `auth.jwt.public_key`, `.issuer`, `.audience`, `.required_claim` | – | JWT verification for `jwt` |
| `topics[]` | empty | `name` (required), `type` (default = `name`), `direction` (`in` = client → DDS, `out` = DDS → client, `bidir`; default `bidir`), `ws_path` (default `/topics/<slug>`, `Chat::Message` → `/topics/chat/message`), `qos.reliability` (`reliable`, `best_effort`), `qos.durability` (`volatile`, `transient_local`, `transient`, `persistent`), `qos.history.depth` (≥ 1, default 10), `acl.read`, `acl.write` (subject lists) |

### 3.3 `mqtt-bridged.yaml`

Defaults: `default_for_dev`, `crates/mqtt-bridge/src/daemon/config.rs`.

| Key | Default | Meaning |
|---|---|---|
| `mqtt.broker_url` | `mqtt://127.0.0.1:1883` | broker; parsed at load time |
| `mqtt.client_id` | `zerodds-bridge` | MQTT client ID |
| `mqtt.username`, `mqtt.password` | – | CONNECT login; `MQTT_PASSWORD` ranks below the files |
| `mqtt.keep_alive_secs` | 60 | 0..=65535 |
| `mqtt.drain_timeout_secs` | 30 | wait time for broker acks at shutdown |
| `mqtt.clean_start` | true | Clean Start |
| `mqtt.tls.enabled`, `.ca_file`, `.cert_file`, `.key_file`, `.server_name` | false, empty | TLS to the broker, mTLS, SNI name |
| `auth.mode` | `none` | `none`, `bearer`, `sasl`, `sasl_plain`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject` | – | token as CONNECT password, local subject |
| `auth.username`, `auth.password` | – | outgoing CONNECT login (SASL-PLAIN) |
| `auth.sasl_users.<user>` | empty | user → password |
| `acl.default_deny` | true | takes effect without a rule and without an auth mode only when set explicitly |
| `acl.rules[]` | empty | `subject`, `allow_publish`, `allow_subscribe` (topic patterns) |
| `topics[]` | empty | `dds_name`, `dds_type`, `mqtt_topic`, `direction` (default `bidir`), `mqtt_qos` (`0`, `1`, `2`), `retain`, `qos.reliability`, `qos.durability`, `qos.history.depth` (default 10); short form `DDS[:MQTT]` |

### 3.4 `coap-bridged.yaml`

Defaults: `default_for_dev` and `OscoreSettings::default`, `crates/coap-bridge/src/daemon/config.rs:150`, `:190`.

| Key | Default | Meaning |
|---|---|---|
| `coap.bind` | `0.0.0.0:5683` | UDP bind |
| `coap.bind_dtls` | – | DTLS bind |
| `coap.max_message_size` | 1152 | bytes |
| `coap.ack_timeout_ms` | 2000 | CoAP ACK_TIMEOUT |
| `coap.max_retransmit` | 4 | 0..=255 |
| `coap.observe_max_age_secs` | 60 | Max-Age for Observe |
| `coap.dtls.enabled`, `.cert_file`, `.key_file`, `.ca_file` | false, – | DTLS; `cert_file` and `key_file` only together |
| `coap.dtls.cipher_suites` | – | list; an unknown name is rejected |
| `coap.dtls.psk[]` | – | `identity` + `secret_hex` or text `identity=secret_hex` |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject`, `auth.jwt.*` | – | as for WebSocket |
| `acl.default_deny` | true | as for MQTT |
| `acl.rules[]` | empty | `subject`, `allow_post`, `allow_observe` |
| `oscore.enabled` | false | OSCORE; requires `master_secret_hex` |
| `oscore.master_secret_hex`, `.master_salt_hex`, `.id_context_hex`, `.sender_id_hex` | empty, empty, –, `01` | hex, spaces allowed, even number of digits |
| `oscore.replay_window` | 32 | replay window |
| `topics[]` | empty | `dds_name`, `dds_type` (default = `dds_name`), `coap_uri_path`, `direction` (default `bidir`), `qos.*` as above (`history.depth` default 10); short form `DDS[:URI]` |

### 3.5 `amqp-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/amqp-endpoint/src/bridge_daemon.rs`.

| Key | Default | Meaning |
|---|---|---|
| `amqp.broker_url` | empty | `amqp://` or `amqps://host[:port]` |
| `amqp.container_id` | `zerodds-bridge-<HOSTNAME>` (dots → `-`, 32 characters), without `HOSTNAME` `zerodds-bridge-pid<PID>` | AMQP container ID |
| `amqp.channel_max` | 256 | 0..=65535 |
| `amqp.max_frame_size` | 65536 | ≥ 512 |
| `amqp.idle_time_out_ms` | 30000 | 0 = none |
| `amqp.hostname` | empty = broker host | `open.hostname` |
| `amqp.sasl.mechanism` | `ANONYMOUS` | `PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2` |
| `amqp.sasl.username`, `amqp.sasl.password` | empty | with `XOAUTH2` the password is the token |
| `amqp.tls.enabled`, `.ca_file`, `.cert_file`, `.key_file` | false, empty | TLS |
| `amqp.tls.verify_hostname` | – | only `true` permitted |
| `amqp.tls.alpn` | – | only `amqp` permitted |
| `amqp.reconnect.initial_delay_ms`, `.max_delay_ms`, `.factor` | 500, 30000, 2.0 | backoff; `factor` ≥ 1.0 |
| `acl.default_deny` | false | |
| `acl.rules[]` | empty | `topic`, `read`, `write` |
| `metrics.path` | – | only `/metrics` permitted |
| `catalog.enabled` | true | send the catalog on connect |
| `catalog.address` | empty = `zerodds.bridge.<container-id>.catalog` | destination address |
| `topics[]` | empty | `dds_name`, `dds_type`, `keyed` (false), `amqp_address`, `direction` (`bidir`), `sender.settled` (false), `sender.durable` (0..=2, 0), `sender.snd_settle_mode` (0..=2, 0), `sender.rcv_settle_mode` (0..=1, 0), `receiver.credit` (≥ 1, 256), `qos.reliability` (reliable), `qos.durability` (volatile), `qos.history.kind`/`depth` (keep_last, 1); short form `DDS=ADDRESS`; a duplicate `dds_name` is rejected |

### 3.6 `grpc-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1424`.

| Key | Default | Meaning |
|---|---|---|
| `grpc.bind` | `0.0.0.0:50051` | HTTP/2 bind |
| `grpc.tls.enabled`, `.cert_file`, `.key_file` | false, empty | TLS (ALPN `h2`); without a certificate exit 4 |
| `grpc.tls.client_ca_file`, `grpc.tls.require_client_cert` | empty, false | mTLS |
| `grpc.reflection.enabled` | false | server reflection |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls`; `mtls` requires `grpc.tls.enabled` and `client_ca_file` |
| `auth.bearer_token`, `auth.bearer_subject` (default `anonymous`), `auth.jwt.*` | – | as for WebSocket |
| `topics[]` | empty | `dds_name` and `grpc_service` (both required), `direction` (`bidir`), `acl.publish`, `acl.subscribe`; short form `DDS=SERVICE` or `DDS:SERVICE` |

### 3.7 `corba-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/corba-dds-bridge/src/bridge_daemon.rs:234`.

| Key | Default | Meaning |
|---|---|---|
| `corba.orb_id` | `zerodds-corba-bridge` | ORB ID |
| `corba.iiop.bind` | `0.0.0.0:6833` | IIOP bind |
| `corba.iiop.giop_version` | `1.2` | `1.0`, `1.1`, `1.2` |
| `corba.iiop.fragment_size` | 65536 | ≥ 64 |
| `corba.iiop.max_message_size` | 33554432 | ≥ 1024 |
| `corba.ssliop.enabled`, `.bind`, `.cert_file`, `.key_file`, `.ca_file`, `.require_client_cert` | false, `0.0.0.0:6834`, empty, false | SSLIOP; `enabled` requires certificate and key, `require_client_cert` requires `ca_file` |
| `corba.ior.ior_file` | empty | file for the IOR lines |
| `corba.ior.naming_service` | empty | is read, see deviations |
| `corba.ior.publish_to_naming` | – | only `false` permitted |
| `corba.csiv2.enabled` | – | only `false` permitted |
| `acl.default_deny` | false | |
| `acl.rules[]` | empty | `subject` (subject DN per RFC 4514, `mtls:<SHA-256>` or `*`), `allow_invoke` (repository ID patterns) |
| `metrics.path` | – | only `/metrics` permitted |
| `mappings[]` | empty | `repo_id`, `operation` (`invoke`), `direction` (`corba_to_dds`, `dds_to_corba`, `bidir`; default `corba_to_dds`), `request_topic`/`reply_topic`/`notify_topic` (`dds_name`, `dds_type`), `correlation_id` (only `request_id`), `timeout_ms` (5000), `qos.reliability`, `qos.durability` (`volatile`, `transient_local`), `targets` (corbaloc URLs or IORs); `target_object_keys` is rejected; short form `REPO_ID=TOPIC` |

`corba.iiop.advertise_host` (default `127.0.0.1`, environment `ZERODDS_CORBA_ADVERTISE_HOST`) is the host in the IORs when `corba.iiop.bind` is a wildcard address.

### 3.8 Programs without their own file

- `zerodds-durability-svc`: only flags (`--domain`, `--store sqlite|file|lakehouse|postgres`, `--path`, `--auto`, `--topic`) plus the runtime file `zerodds.yaml` (`crates/durability-service-bin/src/main.rs`). The variables `ZERODDS_DURABILITY_DOMAIN`, `_STORE`, `_PATH`, `_EXTRA` in `packaging/linux/systemd/zerodds-durability-svc.service` are systemd substitutions in `ExecStart`; the program does not read them.
- `zerodds-ros2-shim` and the RMW: `ros2.*` in `zerodds.yaml` (§2.4).
- `zerodds-router`: runtime from `zerodds.yaml`, routes from `--routes <file>` (§3.9). `--config` with extension `.json` or `.xml` is rejected with exit 2 (`crates/routing-service/src/bin/zerodds-router.rs`).

### 3.9 Route file (`zerodds-router --routes`)

Format by extension: `.xml` → XML, otherwise JSON (serde). Model: `crates/routing-service/src/config.rs`; XML: `crates/routing-service/src/xml.rs` (root `<zerodds_routing>`, or RTI subset `<dds><routing_service>`/`<routing_service>`). Example: `packaging/linux/configs/router.json.example`. Man page: `man/man1/zerodds-router.1`.

| Field | Default | Meaning |
|---|---|---|
| `name` | `zerodds-router` | name in logs and metrics |
| `routes[].name` | required, unique | metric label and loop-guard tag |
| `routes[].input`, `routes[].output` | required | `domain`, `topic`, `type_name` (default `*`), `keyed` (false; input and output must match), `partition` (list), `qos.reliable` (true), `qos.durability` (`volatile`, `transient_local`, `transient`, `persistent`), `qos.ownership` (`shared`, `exclusive`), `qos.ownership_strength` (0), `qos.data_representation` (list, 0 = XCDR1, 2 = XCDR2) |
| `routes[].filter` | – | `expression` (DDS-SQL), `parameters`; needs a type shape (`--types`) |
| `routes[].transform` | – | `rename[{to,from}]`, `set_const[{field,value}]`, `drop[]` |
| `routes[].loop_guard` | true | input and output participant ignore each other |
| `routes[].preserve_source_timestamp` | false | keep the source timestamp |
| `routes[].max_samples_per_second` | – | throttling |
| `routes[].tenant` | – | tenant for `access_control` |
| `access_control.allow[]` | – | `tenant`, `domain`, `topics` (globs `*`, `?`) |

## 4 QoS via DDS-XML

### 4.1 Result

- Reader: crate `zerodds-xml` (`crates/xml`). `zerodds-xml-compose` (`crates/xml-compose`) reads only type definitions (`<types>`) into the common AST and has nothing to do with QoS.
- No runtime setting, no environment variable and no daemon loads a QoS XML file automatically. `crates/dcps` does not depend on `zerodds-xml` (`crates/dcps/Cargo.toml`). A Rust program loads profiles itself:

```rust
let reg = zerodds_xml::QosProfileRegistry::from_file("qos.xml")?;   // std
let wq  = reg.writer_qos("MyLib::HighPerf")?;                       // zerodds_qos::WriterQos
let dw_qos: zerodds_dcps::DataWriterQos = wq.into();                // From, crates/dcps/src/qos.rs:453
```

- `QosProfileRegistry` (`crates/xml/src/registry.rs`): `from_xml`, `from_file` (feature `std`), `writer_qos`, `reader_qos`. Profile reference `Library::Profile` or `Profile` (then the first library); full `base_name` inheritance, also across libraries, with cycle detection.
- Further readers: `zerodds-admin qos validate <file.xml>` and `zerodds-admin qos check <file.xml>` (RxO check between writers and readers of the participant libraries, `tools/admin/src/main.rs:571`, `:603`), `zerodds-xmlc` (`tools/xmlc`; both tools: `--skip-unknown` leaves unknown elements out with a path message, `--vendor-forms` reads vendor forms), `RpcQos::from_xml_profile` (`crates/rpc/src/qos_profile.rs:228`), the XRCE file configuration with `qos_profile` attributes (`crates/xrce/src/xml_config.rs`, feature `xml-config`).
- C-API, C++, Python, Java bindings: no function for loading QoS XML found (search for `xml`/`profile` in `extern "C"` names of `crates/zerodds-c-api/src`).

### 4.2 Supported elements (`crates/xml/src/qos_parser.rs`)

| Level | Elements |
|---|---|
| Document | `<dds>` with `<qos_library name>` (`parse_qos_libraries`) |
| Library | `<qos_profile name base_name>`; short form: a named QoS container directly in the library is a profile of its own (`parse_single_qos_shortcut`) |
| Profile | `datawriter_qos`, `datareader_qos`, `topic_qos`, `publisher_qos`, `subscriber_qos`, `domain_participant_qos`; topic filter as attribute `topic_filter` of the container. Vendor forms (`domainparticipant_qos`, `participant_qos`, `<topic_filter>` as child element) are an error that names the specification's form; `ReadOptions { vendor_forms: VendorForms::Accept, .. }` (`zerodds-admin --vendor-forms`, `zerodds-xmlc --vendor-forms`) reads them and reports each with its path |
| Policies (`parse_entity_qos`, line 489) | `durability`, `durability_service`, `presentation`, `deadline`, `latency_budget`, `ownership`, `ownership_strength`, `liveliness`, `time_based_filter`, `partition`, `reliability`, `transport_priority`, `lifespan`, `destination_order`, `history`, `resource_limits`, `entity_factory`, `writer_data_lifecycle`, `reader_data_lifecycle`, `user_data`, `topic_data`, `group_data` |
| Values | constants `LENGTH_UNLIMITED`, `DURATION_INFINITE_SEC`/`_NSEC`, `DURATION_ZERO_*`; booleans only `true`, `false`, `1`, `0` (`parse_bool_strict`) |

An unknown child element of a profile or container is an error (`XmlError::UnknownElement`) that names it; so is a policy that is no field of the container in DDS 1.4 §2.2.3. Further building blocks of the same crate: domain libraries, participant libraries, applications, types, samples (`crates/xml/src/lib.rs`).

## 5 Security files

The concepts are in [security.en.md](security.en.md). This section covers only where files and paths are set.

### 5.1 Runtime: `security.enclave_dir`

`security.enclave_dir` (environment `ZERODDS_SECURITY_DIR`) names an SROS2 enclave directory. `SecurityProfile::from_enclave_dir` (`crates/security-runtime/src/profile.rs:583`) expects in it:

| File | Role |
|---|---|
| `identity_ca.cert.pem` | identity CA |
| `cert.pem` | identity certificate |
| `key.pem` | private key |
| `permissions_ca.cert.pem` | permissions CA |
| `governance.p7s` | signed governance |
| `permissions.p7s` | signed permissions |

Consumers (search for `security_enclave_dir` in `crates/`): the RMW (`rmw_zerodds_init`, `crates/rmw-zerodds-shim/src/lib.rs:706`, via `runtime_create_secure_ros_from_source`, `crates/zerodds-c-api/src/security_ffi.rs:421`), the C function `zerodds_runtime_create_secure_from_env` (`security_ffi.rs:372`) and `zerodds-ros2-shim doctor`. A named enclave that does not load is a hard error in the RMW. Rust programs via `Configured` (CLI tools, bridges) read the value but do not apply it.

`transport.interface_bindings` (§2.3) and `security.secure_spdp` (§2.2) are the remaining security-related runtime settings.

### 5.2 Participant properties (`dds.sec.*`)

Set programmatically via the `PropertyQosPolicy` of the participant, not via `zerodds.yaml` (`crates/security-runtime/src/qos_properties.rs:73–190`, `crates/security-logging/src/from_properties.rs:50–60`):

| Property | Content |
|---|---|
| `dds.sec.auth.identity_ca`, `dds.sec.auth.identity_certificate`, `dds.sec.auth.private_key`, `dds.sec.auth.password`, `dds.sec.auth.key_establishment_algorithm` | authentication (DDS-Security 1.2 §9.3.1) |
| `dds.sec.access.permissions_ca`, `dds.sec.access.governance`, `dds.sec.access.permissions` | access control (§9.4.1.2) |
| `dds.sec.log.plugin`, `org.zerodds.sec.log.level`, `org.zerodds.sec.log.jsonl.path`, `org.zerodds.sec.log.syslog.addr`, `.app`, `.host` | security logging |

Values are URIs. `file:` is resolved, `data:,` is the content itself (RFC 2397) and `pkcs11:` names an object on a PKCS#11 token (RFC 7512, DDS-Security 1.2 §9.3.1 Table 44); `pkcs11:` needs the crate's `pkcs11` feature, otherwise `SecurityPropertyError::Pkcs11FeatureDisabled` (`qos_properties.rs`).

C-API: `zerodds_security_set_identity_ca_path`, `_identity_cert_path`, `_private_key_path`, `_permissions_ca_path`, `_governance_path`, `_permissions_path` (`crates/zerodds-c-api/src/security_ffi.rs:215–275`).

### 5.3 TLS files of the bridges

| Daemon | Keys |
|---|---|
| ws | `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` |
| mqtt | `mqtt.tls.ca_file`, `mqtt.tls.cert_file`, `mqtt.tls.key_file` |
| coap | `coap.dtls.cert_file`, `coap.dtls.key_file`, `coap.dtls.ca_file`, `coap.dtls.psk[]`, `oscore.*_hex` |
| amqp | `amqp.tls.ca_file`, `amqp.tls.cert_file`, `amqp.tls.key_file` |
| grpc | `grpc.tls.cert_file`, `grpc.tls.key_file`, `grpc.tls.client_ca_file` |
| corba | `corba.ssliop.cert_file`, `corba.ssliop.key_file`, `corba.ssliop.ca_file` |

The example files use `/etc/zerodds/certs/`.

## 6 Environment variables outside `SETTINGS`

Search: `env::var`/`env::var_os` in `crates/` and `tools/` (Rust, excluding `tests/`, `benches/`, `examples/`, `build.rs` and excluding `#[test]` functions), `getenv` in C, `os.environ` in Python, `System.getenv` in Java, `process.env` in TypeScript.

### 6.1 Product programs and libraries

| Variable | Reader | Location | Effect |
|---|---|---|---|
| `MQTT_PASSWORD` | `zerodds-mqtt-bridged` | `crates/mqtt-bridge/src/daemon/config.rs:55` | layer 2 for `mqtt.password` |
| `AMQP_PASSWORD` | `zerodds-amqp-bridged` | `crates/amqp-endpoint/src/bridge_daemon.rs:281` | layer 2 for `amqp.sasl.password` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | all six bridge daemons | ws `daemon/runtime_common.rs:370`, mqtt `daemon/runtime_common.rs:327`, coap `daemon/runtime_common.rs:308`, amqp `daemon_runtime.rs:497`, grpc `daemon_runtime.rs:303`, corba `daemon_runtime.rs:316` | OTLP export active, `host:port`; the only `OTEL_*` variable read |
| `ZERODDS_CORBA_ADVERTISE_HOST` | `zerodds-corba-bridged` | `crates/corba-dds-bridge/src/bridge_daemon.rs` (`ENV`) | layer 2 for `corba.iiop.advertise_host` |
| `HOSTNAME` | amqp daemon; `zerodds-dcps`; C-API | `bridge_daemon.rs:196`; `crates/dcps/src/participant.rs:437`; `crates/zerodds-c-api/src/shm_loan_ffi.rs:851` | default `amqp.container_id`; fallback for the host ID after `gethostname`; host name in the SHM loan path |
| `COMPUTERNAME` | `zerodds-dcps` | `participant.rs:438` | host ID fallback (Windows) |
| `PROGRAMDATA` | `zerodds-config` | `crates/config/src/layers.rs:424` | system directory on Windows |
| `${NAME}` in configuration files | `zerodds-config` | `crates/config/src/parse.rs:36` | any variable, substitution before parsing |
| `ROS_DOMAIN_ID` | `zerodds-ros2-shim` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs:172`, `:566` | domain of the subcommands, default 0; `doctor` |
| `ROS_DISTRO`, `RMW_IMPLEMENTATION`, `ROS_SECURITY_ENABLE` | `zerodds-ros2-shim` | `zerodds-ros2-shim.rs:234`, `:536`, `:553`, `:611` | display and diagnostics only (`info`, `doctor`) |
| `ZERODDS_LOAN_ALIGNMENT` | `rmw_zerodds_cpp` (C) | `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c:621` | alignment of loaned buffers; `page` or a power of two ≥ pointer size |
| `ZERODDS_LIB` | Python package, Node binding | `crates/py/python/zerodds/loader.py:83`, `crates/ts-node/src/native.ts:52` | path of the native library |
| `ZERODDS_RTPS` | Java `DdsWireBridge` | `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/DdsWireBridge.java:25` | `1`/`true` enables the RTPS bridge (alongside the property `zerodds.rtps.enable`) |
| `CHAOS_CLOCK_SKEW_NS`, `CHAOS_CLOCK_DRIFT_PPM` | `chaos-clock-skew` (LD_PRELOAD shim) | `crates/chaos-clock-skew/src/lib.rs:114`, `:121` | offset and drift of `clock_gettime` |

### 6.2 Tools of the test and measurement infrastructure

| Variable | Reader | Location |
|---|---|---|
| `ZERODDS_ID_CA`, `ZERODDS_ID_CERTIFICATE`, `ZERODDS_ID_PRIVATE_KEY`, `ZERODDS_PERMISSIONS_CA` | `tools/omg-security-main` (fallback for the flags) | `tools/omg-security-main/src/cli.rs:288–309` |
| `ORACLE_READY_FILE` | `tools/iceoryx2-oracle-consumer` | `src/main.rs:103` |
| `ZERODDS_CORPUS`, `ZERODDS_C_RUNTIME`, `CC`, `CFLAGS`, `AR` | `wireprobe-c-matrix` | `tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs:458`, `:1188`, `:790`, `:1111`, `:1218` |
| `ZERODDS_CORPUS`, `ZERODDS_CPP_RUNTIME`, `CXX`, `CXXFLAGS`, `AR` | `wireprobe-cpp-matrix` | `tools/wireprobe/wireprobe-zerodds-cpp-gen/src/bin/wireprobe-cpp-matrix.rs:221`, `:968`, `:917`, `:929`, `:1005` |
| `CARGO` | `tools/cargo-dag` | `src/lib.rs:118` |
| `OUT_DIR` | `zerodds-build` (build script helper) | `crates/zerodds-build/src/lib.rs:611` |
| `ZERODDS_SPHINX_THEME`, `XTYPES_PDF`, `INTEROP_OUT`, `CI_COMMIT_SHA`, `ROS_PREFIX` | Python scripts | `crates/py/docs/conf.py:71`, `tools/xtypes-1.3-gap-scan/roster.py:51`, `tools/interop-results/verify_record.py:78`, `:101`, `tools/wireprobe/rosidl-generate.py:80` |

Variables that are read only in tests, examples or build scripts (e.g. `ZERODDS_SCALE_N`, `ZERODDS_OMG_DDSXML_DIR`, `ZERODDS_RUN_SECS`, `ZD_*_OUT`, `ICEORYX_LIB_DIR`) are not listed.

## 7 Deviations

None open. Measured: under `--check-config`, the six bridge daemons exit with 2 on a missing or unparseable file; at startup they exit with 1 (at startup, 2 is the bind error of the bridge specs); `crates/dcps/src/cli.rs` `HELP` names the 2 only for `--check-config`.

Checked without findings: all `KEYS` entries of the six daemons occur in the respective man page. All six example files and `ros2.yaml.example` pass `--check-config`. `ZERODDS_CONFIG` is not read in the code, as the deployment spec says. `discovery.multicast` and `discovery.unicast_initial_peers` appear in `docs/specs/zerodds-ros2-bridge-1.0.md` and `zerodds-ros2.yaml.5` only as rejected names.

## 8 Not determined

- Whether the XML route file also skips unknown elements: not measured, only JSON.
- Which values `transport.user_transport` accepts in the shipped packages: `SHM`, `UDS`, `TSN` depend on the features `same-host-shm`, `same-host-uds`, `tsn-live` (Linux); the feature selection of the package builds was not checked.
- Effect of `transport.interface_bindings` on the routing of outgoing packets: only the parser was read, not `OutboundSocketPool::route` in detail.
- Behavior on Windows and macOS (system directory, missing `/run`): derived from the code, not measured on these platforms.
- The man pages were checked only for the occurrence of the key names, not value by value for defaults and value ranges.
