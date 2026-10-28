# Interop and dialects

German original: [interop-and-dialects.md](interop-and-dialects.md)

This page describes how ZeroDDS is connected on the wire to a running system of another vendor: domain and ports, discovery, vendor IDs and vendor-specific parameter IDs, type compatibility (TypeObject, TypeLookup, extensibility, XCDR1/XCDR2, big endian), security interop, and all switches that adapt the wire behaviour to a peer. For each switch the page lists name, location (runtime setting, `zerodds-idlc` flag, tool flag, API), values, default, the peer it concerns and evidence. The basis is the code on `main`. Statements that come only from a documentation file or a code comment are marked "(per docs)" or "(per code comment)". Migrating an application is described in [migration.en.md](migration.en.md).

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## 1 Summary

- **Discovery and ports** follow DDSI-RTPS 2.5 §9.6.1.4.1 without deviation; the parameters PB, DG, PG and d0–d3 of the formula are the settings `ports.*`, their defaults are the specification's values (`crates/rtps/src/wire_types.rs:1015–1154`, `PortMapping::SPEC` at `:1055`; read in `crates/dcps/src/runtime.rs:4450–4467`).
- **Seven runtime settings** adapt the wire to a peer: `wire.data_representation_offer`, `wire.data_representation_match`, `wire.rtps_dialect`, `wire.codec_dialect`, `wire.big_endian`, `xtypes.key_member_bounds`, `xtypes.type_consistency_absent` (`crates/dcps/src/runtime.rs:3034–3238`, table `SETTINGS`). In practice almost only `wire.data_representation_offer` is needed.
- **CDR dialects for generated code.** Generated application code reaches the twelve dialects (`zerodds_ast_layout::Dialect`) and their axes through `zerodds-idlc generate --dialect <name>` and `--<axis> <value>` (`tools/idlc/src/main.rs:601`) and through `zerodds-build` (`Config::dialect`, `Config::axis`, `crates/zerodds-build/src/lib.rs:283`, `:325`); the default is `specification`. The names and values of the axis flags live once in `crates/ast-layout/src/axes.rs` (`AXES`, 26 flags) and are the same for `wireprobe-zerodds` and the `wireprobe-zerodds-*-gen`. The reflective runtime codecs read and write in the dialect of the setting `wire.codec_dialect` (`ZERODDS_CODEC_DIALECT`, default `specification`): the body check and key derivation for the language bindings (`SerializedTypeSupport`, set in `create_serialized_topic` and when the runtime builds key-hash support), webdds-bridge (gateway and RPC), the ROS 2 dynamic type support (`crates/rmw-zerodds-shim/src/dynamic.rs`, once per process from the same configuration layers), and as the default of `zerodds-spy`/`zerodds-record --dialect`. It is the dialect the application's types were generated in; like `wire.rtps_dialect` it is configured and never inferred from a peer's vendor id. Still in the specification's: `wire-probe::probe`/`unprobe` (shorthand API) and `omg-xtypes-test-main`.
- **The IDL switch relevant for applications** is `zerodds-idlc --default-extensibility` (Cyclone: `final`).
- **Some adaptations happen automatically** based on the peer's vendor ID (OpenDDS handshake, eProsima PID 0x800f, Cyclone ROS services) or fixed for all peers (INFO_DST in SEDP and `VolatileSecure`).
- **Measured** live and at the serialisation level are Cyclone DDS 11.0.1, Fast DDS 3.6.1, OpenDDS 3.34.0 and RTI Connext 7.7.0; OpenSplice CE 6.9 live (OMG `dds-rtps`, `-x 1`) and at the serialisation level (191 cells with octets, dialect `opensplice`); HDDS 1.4.8 live (OMG `dds-rtps`) and with `hddsgen` 1.2.0 at the serialisation level (`tools/interop-results/src/systems.rs`, `documentation/07-migration/from-opensplice.md`).

## 2 Domain, ports, discovery

### 2.1 Ports

| Quantity | Value | Formula | Source |
|---|---|---|---|
| SPDP multicast group | `239.255.0.1` | – | `wire_types.rs:1016` (`SPDP_DEFAULT_MULTICAST_ADDRESS`) |
| SPDP multicast port | 7400 + 250·D | `PB + DG·D + d0` | `wire_types.rs:1122` |
| SPDP unicast port | 7410 + 250·D + 2·P | `PB + DG·D + d1 + PG·P` | `wire_types.rs:1132` |
| User multicast port | 7401 + 250·D | `PB + DG·D + d2` | `wire_types.rs:1142` |
| User unicast port | 7411 + 250·D + 2·P | `PB + DG·D + d3 + PG·P` | `wire_types.rs:1152` |

D = domain ID, P = participant index. ZeroDDS takes the first free index 0..119 (`crates/dcps/src/runtime.rs:9147–9165`); if all are taken, it falls back to an ephemeral port and can then only be found via multicast (comment `runtime.rs:9141`). The multicast group can be changed via `discovery.spdp_multicast_group`, PB, DG, PG and d0–d3 via `ports.*`; the table shows the default values.

### 2.2 Discovery settings for foreign networks

| Setting | Environment | Values | Default | Effect |
|---|---|---|---|---|
| `discovery.peers` | `ZERODDS_PEERS` | list of `ip` or `ip:port` | empty | unicast SPDP in addition to multicast; `ip` without a port is expanded to the SPDP unicast ports of the first N indices (`runtime.rs:5114–5162`) |
| `discovery.max_peer_participants` | `ZERODDS_MAX_PEER_PARTICIPANTS` | 1..120 | 10 | N from the row above; counterpart to Cyclone's `MaxAutoParticipantIndex` (comment `runtime.rs:5114`) |
| `discovery.no_multicast` | `ZERODDS_NO_MULTICAST` | `true`/`false` | `false` | not a single SPDP multicast packet (`runtime.rs:1851–1856`) |
| `discovery.multicast_allowlist` | `ZERODDS_MULTICAST_ALLOWLIST` | IPv4 multicast addresses | empty | see [config.en.md](config.en.md) |
| `transport.interface` | `ZERODDS_INTERFACE` | IPv4 address | automatic | all IP sockets bind to this address (comment `runtime.rs:9143–9146`, comparison with Cyclone `NetworkInterface`) |
| `discovery.initial_announce_count`, `discovery.initial_announce_period_ms` | `ZERODDS_INITIAL_ANNOUNCE_*` | integers | 10, 200 ms (`runtime.rs:253–256`) | SPDP sequence at startup |
| `discovery.spdp_period_ms` | `ZERODDS_SPDP_PERIOD_MS` | ms > 0 | 30000 (`runtime.rs:166`); with a lease below 90 s a third of the lease, at least 100 ms (`default_spdp_period_for`, `:5082`) | SPDP period |
| `discovery.server` | `ZERODDS_DISCOVERY_SERVER` | `true`/`false` | `false` | own SPDP relay; SEDP stays peer-to-peer (`runtime.rs:1858–1868`). No client for the Fast DDS Discovery Server protocol |

Predefined profiles in Rust: `RuntimeConfig::ros_defaults()` (`[XCDR1, XCDR2]`), `RuntimeConfig::multi_robot()` (without multicast, lease 300 s) (`runtime.rs:4872–4922`).

### 2.3 Response to a new SPDP

On the first beacon of a new participant ZeroDDS responds immediately. Axis `SpdpResponseTargets` (`crates/rtps/src/dialect.rs`):

| Value | Target | Who needs it |
|---|---|---|
| `AnnouncedLocatorsAndBeaconSource` (default) | announced locators and sender address | OpenDDS 3.34.0 under `rtps_protection != NONE`; measured 0/5 with the other value, 5/5 with this one (per code comment) |
| `AnnouncedLocatorsOnly` | announced locators only | relay and NAT scenarios; Cyclone and Fast DDS are reached with both values (26/26, per code comment) |

Set via `wire.rtps_dialect` (6.1.3): `cyclone` and `fastdds` select `AnnouncedLocatorsOnly`, `maximally-interoperable` and `opendds` the default.

## 3 Vendor IDs and vendor-specific parameter IDs

### 3.1 Vendor IDs

| Constant | Value | Source |
|---|---|---|
| `VendorId::ZERODDS` | `0x01F0` (interim value, not on the OMG list) | `crates/rtps/src/wire_types.rs:87–101` |
| `VendorId::OPENDDS` | `0x0103` | `wire_types.rs:104` |
| `VendorId::FASTDDS` | `0x010F` | `wire_types.rs:107` |
| `VendorId::CYCLONE` | `0x0110` | `wire_types.rs:110` |
| RTI | `0x0101`, only in tests (`crates/rtps/src/publication_data.rs:1977`, `:2032`) | – |

### 3.2 Vendor-specific PIDs

DDSI-RTPS 2.5 §9.4.2.11.2 Table 9.6 assigns the range from `0x8000` to all vendors in common; such a PID means what the sending vendor defines (`crates/rtps/src/parameter_list.rs:761`, `publication_data.rs:1216–1236`).

| PID | Name | Direction | Rule | Source |
|---|---|---|---|---|
| `0x8001` | `SHM_LOCATOR` | ZeroDDS writes it into its own announcements; no reader in the code (per code comment) | same-host locator, without must-understand bit | `parameter_list.rs:231–247` |
| `0x8002` | `PID_ZERODDS_TYPE_ID` | ZeroDDS writes it only under its own vendor ID; read only from `VendorId::ZERODDS` | RTI 7.7.0 sends its own `0x8002`; formerly read as `TK_BOOLEAN`, all RTI endpoints rejected | `publication_data.rs:689–706`, `:1004–1043`, `subscription_data.rs:482` |
| `0x800f` | eProsima `PID_RELATED_SAMPLE_IDENTITY` | ZeroDDS writes it next to `0x0083` in every reply with a reference; read only from Fast DDS and ZeroDDS | Fast DDS 2.6 reads only `0x800f` | `crates/rtps/src/inline_qos.rs:119–127`, `crates/dcps/src/runtime.rs:11167–11175`, `:17854–17862` |
| `0x801c` | Cyclone `PID_CYCLONE_REQUESTS_KEYHASH` | ZeroDDS does not send it; when `PID_KEY_HASH` is missing, the reader computes the KeyHash from its type | Cyclone sends `PID_KEY_HASH` only to readers with this PID | `crates/dcps/src/runtime.rs:25692` (`compute_alive_key_material`) |
| `0x0D00` | `PID_VENDOR_TRACE_CONTEXT` | ZeroDDS writes it, in the standard range without must-understand bit | foreign receivers ignore it | `parameter_list.rs:257–263` |

Must-understand: an unknown PID with bit `0x4000` discards the message (`parameter_list.rs:758`, `validate_must_understand_in_data_pipeline` in `crates/rtps/src/datagram.rs:743`, `:789`, `:896`). `PID_DOMAIN_TAG` (`0x4014`) is a standard PID: ZeroDDS sends it when `discovery.domain_tag` is set and ignores participants with a different tag (`participant_data.rs`, `handle_spdp_datagram`).

### 3.3 Where the vendor ID controls behaviour

The rule for RTPS dialects is "configured, never inferred" (`crates/rtps/src/dialect.rs`, module header; `runtime.rs:11539–11547`). Independently of that, the code reads the vendor ID at these places:

| Location | Condition | Effect |
|---|---|---|
| `publication_data.rs:1027`, `subscription_data.rs:482` | peer ≠ ZeroDDS | `0x8002` is not read, type identity from `PID_TYPE_INFORMATION` |
| `runtime.rs:11170–11175` (`speaks_eprosima_pids`) | peer = Fast DDS or ZeroDDS | `0x800f` is read as related sample identity |
| `crates/discovery/src/security/stack.rs:1098–1103` (`prime_plugin`, called in `begin_handshake_with` `:1025` and `on_stateless_message` `:1694`) | peer = OpenDDS, only with `security.algo_name_nul: by_vendor` | NUL-terminated `c.dsign_algo`/`c.kagree_algo` in the handshake; the default `spec` writes the NUL for every peer (`crates/security/src/vendor_forms.rs:47–53`) |
| `crates/rmw-zerodds-shim/src/services.rs:425–433`, `:891` | server = Cyclone | ROS 2 service special path for `rmw_cyclonedds` |

## 4 Type compatibility

### 4.1 TypeObject and TypeLookup

- ZeroDDS announces type information when the writer or reader was created with a TypeObject (Rust `DdsType`, C `*_create_typed`). The plain C functions `*_create` announce `TypeIdentifier::None` (`crates/zerodds-c-api/README.md`, "Type model").
- Foreign announcements: `PID_TYPE_INFORMATION` is translated into a comparable `TypeIdentifier`; on a parse error the raw block is kept and the match layer reports it (`publication_data.rs:1046–1055`).
- TypeLookup server and client run in every `DcpsRuntime` (`runtime.rs:8372–8381`, `:9560–9562`). The server returns the closure of all dependencies named by hash, because RTI Connext 7.7.0 requests the COMPLETE form and expects the dependencies with it (`crates/discovery/src/type_lookup/server.rs:116–286`, per code comment). Cyclone evidence: the `(TypeLookup)` cells of `tests/interop/ci/run_vendor.sh cyclone`; locally `crates/discovery/tests/type_lookup_requests.rs`, `cyclone_typelookup_responder.rs`.
- OpenDDS requires complete XTypes information from the remote writer by default; against ZeroDDS writers without a TypeObject, `use_xtypes=no` must be set there (comment `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`).
- Key members and `IS_MUST_UNDERSTAND`: XTypes 1.3 §7.2.2.4.4.4.8 sets the flag on every key member; ZeroDDS and Cyclone do. The type support fastddsgen generates for Fast DDS 3.6.1 does not (measured: member flags `0x0021` in its TypeLookup reply), opendds_idl 3.34 only when `@must_understand` is written. The type identifiers then differ, and a Fast DDS reader does not match a ZeroDDS writer that announces its TypeInformation; with `@must_understand` on the key in the peer's IDL they agree (`tools/dispose-reader/fastdds/KeyedPointA.idl`, `…/opendds/KeyedPointA.idl`).
- Key member bounds (XTypes 1.3 Table 19): setting `xtypes.key_member_bounds`, 6.1.5.

### 4.2 Extensibility and encapsulation

The writer sets the encapsulation header according to the first offered format and the declared extensibility (`crates/dcps/src/runtime.rs:714–893`, `crates/dcps/src/publisher.rs:3585–3603`):

| Representation | `@final` | `@appendable` | `@mutable` |
|---|---|---|---|
| XCDR1 LE | `0x0001` CDR_LE | `0x0001` CDR_LE | `0x0003` PL_CDR_LE |
| XCDR2 LE | `0x0007` PLAIN_CDR2_LE | `0x0009` D_CDR2_LE | `0x000b` PL_CDR2_LE |
| Big endian | ID − 1 in each case (`0x0000`, `0x0002`, `0x0006`, `0x0008`, `0x000a`) | | |

The reader decodes according to the received header, including big endian (`crates/dcps/src/subscriber.rs:56–79`). Measured writer forms of the vendors (per docs, `docs/spec-coverage/cross-vendor-validation.md`, "What each writer puts on the wire"): Cyclone `@final` XCDR1, extensible XCDR2; Fast DDS and RTI everything XCDR1, `@mutable` as PL_CDR; OpenDDS everything XCDR2.

### 4.3 Data representation

- A writer serialises each sample once, in the first element of its offer list (`crates/rtps/src/publication_data.rs:75–94`). A writer therefore cannot serve an XCDR1-only reader and an XCDR2-only reader at the same time (per docs, `cross-vendor-validation.md`).
- Default `[XCDR1]` (XTypes 1.3 §7.6.3.1.1; `[XCDR1, XCDR2]` is a setting): fits readers with the DDS default list `[XCDR1]` (Cyclone and Fast DDS `shape_main`, RTI DataReader). A reader that accepts only XCDR2 (Cyclone `shape_main -x 2`) does not match; case `N1` in `tools/dcps-shapes-interop.sh:249–255`.
- A runtime from the C API (`zerodds_runtime_create*`) offers `[XCDR2, XCDR1]`, because its callers hand it XCDR2 bytes (`ffi_data_representation_offer`, `crates/zerodds-c-api/src/lib.rs`). A Fast DDS 3.6.1 reader with default QoS sends no `PID_DATA_REPRESENTATION`, so it requests XCDR only, and does not match such a writer (measured); XTypes 1.3 §7.6.3.1.1 requires a reader of version 1.2 or later to request XCDR2. Remedy on the Fast DDS side: add `XCDR2_DATA_REPRESENTATION` to `representation()` (`tools/dispose-reader/fastdds/reader.cpp`).
- Per writer or reader: `DataWriterQos.data_representation`, `DataReaderQos.data_representation` (`Option<Vec<i16>>`, `crates/dcps/src/qos.rs:74`, `:235`).

### 4.4 Byte order

All vendors write little endian on x86. ZeroDDS always reads big endian and writes it with `wire.big_endian` (6.1.4). Cross-check per docs: Cyclone 59/59, Fast DDS 60/60, OpenDDS 40/40, RTI 60/60 decode ZeroDDS big endian (`cross-vendor-validation.md`, "Big-endian axis").

## 5 Security interop

Details in [security.en.md](security.en.md). For connecting to a foreign system:

- Same identity CA, same governance and permissions documents in OMG format; ZeroDDS loads them e.g. via `security.enclave_dir` (SROS2 layout, `runtime.rs:2326–2340`).
- Handshake role: the smaller GUID initiates (comment `crates/discovery/src/security/stack.rs:925–928`, checked against Cyclone).
- Automatic and cannot be switched off: INFO_DST before every directed SEDP datagram (`crates/discovery/src/sedp/writer.rs:565–597`) and before every `VolatileSecure` datagram, because Cyclone's filtered `VolatileSecure` reader matches on the full GUID (`crates/discovery/src/security/volatile_secure.rs:91–94`).
- Measured matrix (per docs, `docs/spec-coverage/cross-vendor-validation.md`, "DDS-Security, cross-vendor"): ZeroDDS↔Cyclone secured in both directions; ZeroDDS↔OpenDDS for the rich type; ZeroDDS↔Fast DDS only with RSA identity certificates; RTI not measured (licence).
- `docs/interop-harnesses.md` ("The gaps"): no cell starts two secured participants from one governance/permissions set and checks a decoded sample.

## 6 Switches that adapt the wire to a peer

### 6.1 Runtime settings

Location: `zerodds.yaml`/`zerodds.d/`, environment, `--set path=value`; precedence default < environment < file < flag (`runtime.rs:3013–3024`). Invalid values in a file prevent startup and report the location (`runtime.rs:3521–3579`). Invalid values only in the environment fall back to the default; for `wire.rtps_dialect` the runtime then reports `config.rtps_dialect.unknown` on the observability sink (`runtime.rs:5030–5074`, `:9446–9458`).

#### 6.1.1 `wire.data_representation_offer`

| | |
|---|---|
| Environment | `ZERODDS_DATA_REPR_OFFER` |
| Values | `XCDR1`, `XCDR2`, as comma list or YAML list, order is significant |
| Default | `XCDR1` (`DEFAULT_OFFER`, `publication_data.rs`) |
| Effect | offer list in SEDP; first element = format of every sample |
| Peer | `[XCDR2]` for readers that accept only XCDR2 (Cyclone with `-x 2`); default for RTI, Fast DDS, Cyclone default, ROS 2 |
| Evidence | `tools/dcps-shapes-interop.sh` cases M1–M16 and N1; `crates/dcps/tests/sedp_data_representation_pid.rs`; test `crates/dcps/tests/runtime_config_layering.rs:302–316` |
| Special case | The C API sets the ROS list for `zerodds_runtime_create_ros_defaults` when the value is not set (`crates/zerodds-c-api/src/lib.rs:1087`) |

#### 6.1.2 `wire.data_representation_match`

| | |
|---|---|
| Environment | `ZERODDS_DATA_REPR_MATCH` |
| Values | `strict` (XTypes 1.3 §7.6.3.1.1: the writer's offered representation in the reader list), `tolerant` (any overlap) |
| Default | `strict`, the clause (`runtime.rs:4653–4658`; `DataRepMatchMode::default()`, `publication_data.rs:129`) |
| Effect | only on the reader side; on the writer side both modes decide the same, because the writer sends only its first element (comment `publication_data.rs:102–126`) |
| Peer | none measured; `tolerant` serves a remote writer whose first offered element is missing from the reader list while a later one is in it |
| Evidence | `runtime_config_layering.rs:875` (`the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back`), `sedp_data_representation_pid.rs:387`; OMG `Test_DataRepresentation_0..3` with `strict` against Connext, Cyclone, Fast DDS, both directions: 24/24 (Linux test host) |

#### 6.1.3 `wire.rtps_dialect`

| | |
|---|---|
| Environment | `ZERODDS_RTPS_DIALECT` |
| Values | `maximally-interoperable`, `cyclone`, `fastdds`, `opendds` (`crates/rtps/src/dialect.rs:425–516`) |
| Default | `maximally-interoperable` |
| Effect | two axes: form of the SEDP dispose (`DisposeKeyForm`) and target of the SPDP response (`SpdpResponseTargets`) |
| Per peer | `DcpsRuntime::set_peer_rtps_dialect(prefix, dialect)`, `set_peer_rtps_behaviour(prefix, behaviour)` (`runtime.rs:11539–11570`); for every participant of one vendor `set_vendor_rtps_behaviour(vendor, behaviour)` (`runtime.rs:11597–11626`); API only |
| Evidence | `crates/rtps/tests/dialect_no_drift.rs`; `runtime_config_layering.rs:304`, `:435` |

| Value | Dispose form | SPDP response | Measured version |
|---|---|---|---|
| `maximally-interoperable` | `K=1`, key in payload **and** `PID_KEY_HASH` | locators + sender | – |
| `cyclone` | `K=1`, key in payload, no `PID_KEY_HASH` | locators only | Cyclone DDS 11.0.1 |
| `fastdds` | `K=0`, `PID_KEY_HASH` only | locators only | Fast DDS 3.6.1 |
| `opendds` | as default | locators + sender | OpenDDS 3.34.0 |

The default is accepted by all three measured implementations (per code comment `dialect.rs`, `DisposeKeyForm::KeyInPayloadWithHash`). The code gives no reason to change the value for one of the three peers; `cyclone`/`fastdds` reproduce their own form. The third axis `DirectedAddressing` is fixed to `PrefixEveryDatagram` for SEDP writers (`sedp/writer.rs:555–563`). Four more axes concern user writers only and belong to no dialect name: `GroupCoherentSetPid` (`writer.group_coherent_set_pid`), `HeartbeatGroupDigests` (`wire.omit_heartbeat_group_digests`), `GroupWireForm` (`writer.group_info_on_wire`) and `EndOfSetForm` (API only; the end of a set as a GAP, for InterCOM 4.3.1).

#### 6.1.4 `wire.big_endian`

| | |
|---|---|
| Environment | `ZERODDS_WIRE_BIG_ENDIAN` |
| Values | `1`/`0` (in files; the help text also names `true`/`false`, `runtime.rs:3819–3822`, `:3898–3901`) |
| Default | `false` |
| Effect | writers write `*_BE` encapsulation and a big-endian body |
| Peer | none requires it; measurement and interop switch (`runtime.rs:2310–2313`) |
| Evidence | `crates/dcps/tests/shapes_type_idl_agreement.rs` (four modes); `cross-vendor-validation.md`, "Big-endian axis" (per docs) |

#### 6.1.5 `xtypes.key_member_bounds`

| | |
|---|---|
| Environment | `ZERODDS_KEY_MEMBER_BOUNDS` |
| Values | `enforced`, `ignored` (`crates/types/src/assignability.rs:228–252`) |
| Default | `enforced` (clause) |
| Effect | key member rules from XTypes 1.3 Table 19 (bound, literals) in the assignability check |
| Peer | `ignored` corresponds to RTI 7.7.0, OpenDDS 3.35, CoreDX 6.15.3, HDDS 1.3.0 (all four `struct_key_*` pairs match); `enforced` corresponds to Cyclone 11.0.1 (refuses three of four); measured 2026-09-13 in self-pairs of the OMG suite (per code comment `assignability.rs:207–227`) |
| Evidence | `crates/types/tests/table19_key_members.rs`; `docs/omg-xtypes-issues.md` §C1 |

#### 6.1.6 `xtypes.type_consistency_absent`

| | |
|---|---|
| Environment | `ZERODDS_TYPE_CONSISTENCY_ABSENT` |
| Values | `disallow`, `allow` (`crates/dcps/src/runtime.rs:1368`, `AbsentTypeConsistency`) |
| Default | `disallow` (clause, XTypes 1.3 §7.6.3.4.1) |
| Effect | a remote reader whose announcement carries no PID 0x0074 (`SubscriptionBuiltinTopicData::type_consistency_announced` false) is held to DISALLOW_TYPE_COERCION, which means equivalence; `allow` takes the policy's default ALLOW_TYPE_COERCION for it (`runtime.rs:15061`, `remote_reader_consistency`). A reader that announces DISALLOW itself keeps it |
| Peer | none measured; for a peer that does not write PID 0x0074 and is to be matched by assignability |
| Evidence | `runtime_config_layering.rs:875`; `subscription_data.rs`, test of the stripped PID 0x0074 |

#### 6.1.7 Other runtime levers without a file key

| Lever | Location | Purpose |
|---|---|---|
| `DataWriterQos.data_representation`, `DataReaderQos.data_representation` | `crates/dcps/src/qos.rs:74`, `:235` | representation per endpoint |
| `set_peer_rtps_dialect`, `set_peer_rtps_behaviour` | `runtime.rs:11548–11570` | RTPS dialect per peer |
| `RuntimeConfig::ros_defaults()`, `multi_robot()` | `runtime.rs:4872–4922` | profiles |
| `delivery.mode` (`portable`, `raw-same-host`, `iceoryx`) | setting, `runtime.rs:3876` | same-host path, `iceoryx` for Cyclone iceoryx ([crates-bridges-services.en.md](crates-bridges-services.en.md)) |

### 6.2 `zerodds-idlc`

`tools/idlc/src/main.rs:1336–1390`. `--dialect <name>` and the axis flags `--<axis> <value>` apply to `generate` only and select the CDR dialect of the generated code (section 6.3; `main.rs:601`, `:637`).

| Flag | Values | Default | Peer relevance |
|---|---|---|---|
| `--strict` | – | off (`Dialect::ALL_VENDORS`) | rejects every vendor extension (`keylist`, `#pragma keylist`, `DCPS_*`); portability check |
| `--default-extensibility` | `final`, `appendable`, `mutable` | `appendable` | Cyclone IDL without annotation: `final` (Cyclone column `default-extensibility` = `final` in `docs/wire-axis-flags.md`) |
| `--encoding` | `xcdr1`, `xcdr2`, `pl-cdr1`, `pl-cdr2`, `delimited-cdr` | `xcdr2` | selects the XCDR version of the generated codec; `xcdr2`, `delimited-cdr`, `pl-cdr2` produce the same text (`main.rs:50–60`) |
| `--endian` / `--xcdr-endianness` | `le`/`be` or `LE`/`BE` | `le` | changes only the preamble; the writer selects the byte order (`main.rs:60–65`) |
| `--deny-warnings` | – | off | vendor warnings as errors |
| `--dialect` | the names in section 6.3 | `specification` | `generate` only: one peer's answers to every wire axis at once, before the axis flags (`main.rs:601`) |
| `--ros2` | – | off | ROS 2 dialect: an unqualified member type that differs from the member name only in case (`Pose pose;`) resolves to the type (`main.rs:431`) |

The parser dialect (`zerodds_idl_parse::Dialect`) has five boolean fields `rti`, `cyclone`, `fast_dds`, `open_dds`, `ros2` (`crates/idl-parse/src/dialect.rs:12–24`). `zerodds-idlc` knows the two positions `ALL_VENDORS` and `STRICT` (`--strict`) and additionally sets `ros2` with `--ros2` (`tools/idlc/src/main.rs:188–198`); individual vendors cannot be selected from the command line.

### 6.3 CDR dialect and axes

`zerodds_ast_layout::Dialect` (`crates/ast-layout/src/key.rs:4911`, `ALL` at `:5028`): `specification` (default), `cyclone`, `rti`, `fastdds`, `opendds`, `opensplice`, `ros-fastrtps`, `ros-cyclone`, `omniorb`, `tao`, `jacorb`, `hdds`. An unknown value yields `None` instead of the default (`Dialect::parse`).

Where reachable:

| Location | Kind |
|---|---|
| `zerodds-idlc generate` (`tools/idlc/src/main.rs:601`) | `--dialect` and the axis flags; the generated codec and the TypeSupport companions |
| `zerodds-build` (`crates/zerodds-build/src/lib.rs:283`, `:325`) | `Config::dialect`, `Config::axis` from a `build.rs` |
| `wireprobe-zerodds` (`crates/wire-probe/src/main.rs`) | `--dialect` and all axis flags below; write bytes (`--values`) or read them (`--in`) |
| `tools/wireprobe/wireprobe-zerodds-gen`, `-c-gen`, `-zig-gen` | `--dialect` and axis flags; probe programs |
| `interop-results --dialects` (`tools/interop-results/src/dialect_fit.rs`) | searches, per system, for the axis assignment that reproduces its recorded bytes |
| `zerodds_lang_rules::generate::source(ast, language, key: LayoutKey)` (`crates/lang-rules/src/generate.rs:679`) | programmatic: a caller passes `LayoutKey::in_dialect(...)`; `zerodds-idlc` passes the key `--dialect` selected |
| `crates/corba-interop/build.rs:328` | ORB dialects for the CORBA bridge at build time |

Values of the axis flags (`wireprobe-zerodds --help`, `crates/wire-probe/src/main.rs`, constant `USAGE`):

| Flag | Values | Value of `specification` |
|---|---|---|
| `--length-codes` | `compact`, `universal-nextint` (dialects additionally `reuse-nested-delimiter`, `short-where-it-fits`, `sized-or-nextint`) | `compact` |
| `--collection-delimiters` | `specification`, `enum-is-primitive`, `never` | `specification` |
| `--wstring-length` | `specification`, `character-count`, `character-count-terminated`, `specification-wide-units`, `ros-type-support` or coordinates `octets\|characters/terminated\|unterminated/2\|4` | `specification` (octets, no terminator, 2) |
| `--wchar-framing` | `fixed`, `giop-length-prefixed` | `fixed` |
| `--wchar-width` | `2`, `4` | `2` |
| `--map-delimiter` | `either-half`, `value-only` | `either-half` |
| `--wstring-units` | `stream`, `big-endian-always`, `byte-order-mark`, `host-unswapped` | `stream` |
| `--map-length` | `sequence-equivalent`, `rule-15-omits` | see `map_count_omitted_when_delimited` (`key.rs`) |
| `--fixed-digits` | `declared`, `value-precision`, `value-string-digits` | `declared` |
| `--enumerator-value` | `declared`, `ordinal` | `declared` |
| `--union-framing` | `specification`, `branches-from-zero` | `specification` |
| `--short-pid-range` | `up-to-reserved`, `all-fourteen-bits` | `up-to-reserved` |
| `--map-entries` | `declaration`, `ascending-key` | `declaration` |
| `--pl-cdr1-headers` | `specification`, `always-extended`, `short-true-length` | `specification` |
| `--pid-flags` | `sentinel-must-understand`, `bare`, `impl-extension-members` | `sentinel-must-understand` |
| `--enum-holder` | `declared`, `bit-bound-dropped` | `declared` |
| `--union-labels` | `sign-extended`, `negative-unmatched-below-32` | `sign-extended` |
| `--key-holder` | `forced-final`, `as-declared` | – |
| `--key-holder-order` | `ascending-member-id`, `declaration` | – |
| `--key-holder-alignment` | `aligned`, `data-encoding`, `packed` | `aligned` |
| `--default-extensibility` | `final`, `appendable`, `mutable` | `appendable` |
| `--defaulted-discriminator-member` | `type-default`, `stream-value` | `type-default` |
| `--element-padding` | `aligned`, `packed` | `aligned` |
| `--aggregate-delimiter` | `specification`, `never` | `specification` |
| `--sequence-element-delimiter` | `never`, `before-each-non-final-element` | `never` |
| `--wstring-code-units` | `utf16`, `utf8` | `utf16` |
| `hashed-id-counter` | no flag; `ComposeOptions::hashed_id_advances_the_counter` during lowering | "advances" (`key.rs`, `hashed_id_advances_the_counter`) |

Vendor values of the axes on which DDS implementations differ (per docs, `docs/wire-axis-flags.md`, generated by `interop-results --dialects`; values in parentheses there = not fixed by a recording, omitted here and replaced by "·"; superscript digit = XCDR version):

| Axis | Cyc¹ | Cyc² | RTI¹ | RTI² | FD¹ | FD² | ODDS¹ | ODDS² |
|---|---|---|---|---|---|---|---|---|
| `collection-delimiters` | · | specification | · | never | · | enum-is-primitive | · | specification |
| `default-extensibility` | · | final | · | appendable | · | appendable | · | appendable |
| `enum-holder` | declared | declared | · | · | bit-bound-dropped | bit-bound-dropped | bit-bound-dropped | bit-bound-dropped |
| `enumerator-value` | declared | declared | declared | declared | declared | declared | ordinal | ordinal |
| `hashed-id-counter` | advances | advances | skipped | skipped | advances | advances | skipped | skipped |
| `length-codes` | · | compact | · | reuse-nested-delimiter | · | short-where-it-fits | · | sized-or-nextint |
| `map-delimiter` | · | · | · | · | · | value-only | · | · |
| `map-entries` | · | · | · | · | ascending-key | ascending-key | ascending-key | ascending-key |
| `must-understand` | declared | declared | declared | declared | omitted | omitted | · | declared-and-key |
| `pid-flags` | sentinel-must-understand | · | sentinel-must-understand | · | bare | · | impl-extension-members | · |
| `pl-cdr1-headers` | always-extended | · | specification | · | short-true-length | · | short-true-length | · |
| `short-pid-range` | · | · | up-to-reserved | · | up-to-reserved | · | all-fourteen-bits | · |
| `union-framing` | · | · | specification | specification | specification | specification | branches-from-zero | branches-from-zero |
| `union-labels` | negative-unmatched-below-32 | negative-unmatched-below-32 | sign-extended | sign-extended | sign-extended | sign-extended | sign-extended | sign-extended |
| `wchar-width` | 2 | 2 | 4 | 2 | 2 | 2 | 2 | 2 |
| `wstring-counts` | octets | octets | characters | octets | characters | characters | octets | octets |
| `wstring-terminator` | unterminated | unterminated | terminated | unterminated | unterminated | unterminated | unterminated | unterminated |
| `wstring-unit` | 2 | 2 | 4 | 2 | 2 | 2 | 2 | 2 |
| `wstring-unit-order` | host-unswapped | host-unswapped | stream | stream | stream | stream | stream | stream |

Gate: `tools/interop-results/tests/dialect_completeness.rs` checks that `--dialect <name>` alone reproduces every recorded row of the system; row counts Cyclone 1148, RTI 1090, Fast DDS 1412, OpenDDS 1148, OpenSplice 191 (161 byte-for-byte, 30 padding-only), HDDS 326 (per docs, `docs/wire-axes-followup.md`, "The gate"). The same file checks the read direction through `dialect_read_completeness.rs`. `hdds` is **opt-in** like every other dialect: `--dialect hdds` and nothing else. Its four recorded departures from XTypes 1.3 are packed collection elements (`ElementPadding::Packed`), every aggregate written as `@final` with no DHEADER and no EMHEADER (`AggregateDelimiter::Never`), a four-octet length in front of each element of a `sequence` whose element type is not `@final` (`SequenceElementDelimiter::BeforeEachNonFinalElement`), and a `wstring` written in UTF-8 (`WstringCodeUnits::Utf8`). `systems.rs:144–175` records what the column is.

Consequence for applications: if a peer deviates from `specification` on an axis and the construct occurs in the type, application and peer exchange different bytes. There is no runtime setting for this. Affected constructs per peer:

| Peer | Constructs with a deviating wire form |
|---|---|
| Cyclone | `wstring` (units in host order), negative union labels with a discriminator < 32 bit without `default:`, `@mutable` under XCDR1 (always extended PL header), extensibility without annotation |
| RTI | `wstring`/`wchar` under XCDR1 (characters, terminator, 4 bytes), sequence delimiters and length codes under XCDR2 |
| Fast DDS | `wstring` (characters), enum with `@bit_bound`, `map` (order, delimiter), `@mutable` (PID flags, must-understand, short PL headers), sequences of enums under XCDR2 |
| OpenDDS | enumerators with `@value`, enum with `@bit_bound`, `map`, `@mutable` (PID flags, length codes), `@mutable union` |
| HDDS | `@appendable` and `@mutable` (framed as `@final`, no DHEADER/EMHEADER/parameter header), collection elements packed, a per-element length in front of a `sequence` of a non-`@final` type, `wstring` as UTF-8, big endian (no encoder). Opt-in via `--dialect hdds` (`systems.rs:144–175`) |

### 6.4 Automatic adaptations without a switch

| Behaviour | Trigger | Source |
|---|---|---|
| encapsulation header according to declared extensibility | every writer | `crates/dcps/src/publisher.rs:3585–3603` (reason: OpenDDS `EncapsulationHeader::to_encoding`) |
| key-only ALIVE DATA is acknowledged and skipped | OpenDDS `register_instance` | `crates/rtps/src/reliable_reader.rs` (per docs, `cross-vendor-validation.md`) |
| PID flags `& 0x3FFF` when reading PL_CDR1 | RTI `PID_EXTENDED` with must-understand (`0x7F01`) | `crates/wire/src/reader.rs` (mask `0x3FFF`); tests `crates/wire/tests/vendor_reader.rs`, `crates/wire-probe/tests/pid_flag_positions.rs`. `cross-vendor-validation.md` still names the removed `crates/cdr/src/xcdr1.rs` |
| `0x0083` and `0x800f` side by side | every reply with related sample identity | `runtime.rs:17939–17947` |
| INFO_DST before SEDP and `VolatileSecure` | always | `sedp/writer.rs:565–597`, `security/volatile_secure.rs:91–94` |
| reader decodes XCDR1/XCDR2 and LE/BE according to the header | always | `crates/dcps/src/subscriber.rs:56–79` |

## 7 Peer, settings, evidence

| Peer | Required settings | Evidence |
|---|---|---|
| Cyclone DDS 11.0.1 | none for standard cases. `wire.data_representation_offer: [XCDR2]` only against readers with an XCDR2 list (`-x 2`). IDL without annotation with `zerodds-idlc --default-extensibility final`. Avoid constructs from 6.3. Cyclone side with `Discovery/Tag`: the same value in `discovery.domain_tag` | `crates/discovery/tests/cyclone_live_sedp.rs`; `crates/dcps/tests/cyclone_live_wlp.rs`; `crates/rtps/tests/cyclone_compliance.rs`; CI gate `tests/interop/ci/`; `tools/dcps-shapes-interop.sh` A/B, M cases, N1 |
| Fast DDS 3.6.1 | none for standard cases. Security only with RSA identity certificates (per docs). Avoid constructs from 6.3 | `crates/discovery/tests/fastdds_live_spdp.rs`; `crates/dcps/tests/fastdds_live_pub.rs`, `fastdds_live_sub.rs`, `fastdds_qos_matrix.rs`; CI gate `tests/interop/ci/`; `tools/dcps-shapes-interop.sh` C/D |
| RTI Connext 7.7.0 | ZeroDDS: none. `xtypes.key_member_bounds: ignored` if RTI writers are to send wider keys. RTI side: `transport_builtin` = `UDPv4` on the same host; extend the reader representation by XCDR2 if ZeroDDS writes XCDR2; `NDDSHOME` set | `tools/dcps-shapes-interop.sh` E/F; `tests/interop/ci/rti/USER_QOS_PROFILES.xml` (comment); `crates/rtps/src/publication_data.rs:1004–1043` (`0x8002`); `crates/discovery/src/type_lookup/server.rs:116–286` |
| OpenDDS 3.34.0 | ZeroDDS: `wire.data_representation_offer: [XCDR2]` against XCDR2-only readers; leave `wire.rtps_dialect` at the default or `opendds` (SPDP response to sender). OpenDDS side: `DCPSDefaultDiscovery` RTPS, `transport_type=rtps_udp`, `use_xtypes=no` against ZeroDDS writers without TypeObject. IDL: `DCPS_DATA_KEY` is read; a key path (`p.x`) makes `p` and `x` in the inner type keys, an array element (`a[0]`) is refused | `tools/dcps-shapes-interop.sh` G/H, M15/M16; `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`; `crates/wire-probe/tests/opendds_mutable_framing.rs`; `crates/rtps/src/dialect.rs` (`SpdpResponseTargets`, 0/5 versus 5/5) |
| OpenSplice CE 6.9 | XCDR1 only, measured with `-x 1` on both sides. IDL: `#pragma keylist` is read as `idlpp` reads it (inside a module, commas, path `a.b`); no struct inheritance and no annotations on the OpenSplice side. OpenSplice sends `PID_KEY_HASH` without padding between keys of different widths (measured); `wire.codec_dialect: opensplice` makes ZeroDDS compute it the same way, the default stays §7.6.8 | `tools/opensplice/README.md`; `tools/interop-results/src/systems.rs`; `crates/idl-lower/tests/keys_and_inheritance.rs` |
| HDDS (hddsgen 1.2.0) | only little endian; the reflective runtime codecs reach it opt-in with `wire.codec_dialect: hdds`. `zerodds-idlc generate --dialect hdds` generates the Rust codec and all twenty generated codecs, which carry all three axes; each runtime tests each axis against a recorded HDDS file. Only a caller that names no target is still refused, naming `element-padding`, `sequence-element-delimiter` and `wstring-code-units` (`zerodds_lang_rules::encode::axes_the_rust_codec_carries` / `axes_the_codecs_carry_for` / `axes_the_codecs_carry`, `crates/lang-rules/tests/hdds_axes_are_refused.rs`, `crates/lang-rules/tests/hdds_rust_codec.rs`; the generated Rust against HDDS's octets: `tools/idlc/tests/dialect_byte_equality.rs::generated_rust_under_hdds_writes_and_reads_what_hdds_wrote`). RTPS runtime operation not measured | `tools/interop-results/src/systems.rs:144–175`; `crates/wire-probe/tests/hdds_fill.rs`; `crates/wire-probe/tests/hdds_dialect.rs`; `docs/edge-case-map.md` (per docs: 5 of 17 HDDS cells octet-identical with ZeroDDS) |
| dust_dds 0.16.1 | `xtypes.key_member_bounds: ignored`: dust_dds announces the key `color` of ShapeType unbounded where the suite's IDL has `string<128>`; under XTypes 1.3 Table 19 a ZeroDDS reader refuses the writer otherwise | `tools/omg-shape-main/src/main.rs` (`suite_settings`); `tools/omg-shape-main/README.md` |
| ROS 2 (`rmw_cyclonedds_cpp`, `rmw_fastrtps_cpp`) | none via `rmw_zerodds_cpp`; `RuntimeConfig::ros_defaults` | [ros2.en.md](ros2.en.md); `crates/ros2-rmw/tests/cross_vendor.rs`; `crates/ros2-rmw/interop/run_multicast_free_xvendor.sh` |

## 8 Procedure "Setting dialect flags correctly"

1. **Determine peer and version.** `zerodds-admin discovery snapshot <domain>` shows the participants with their SPDP content (`tools/admin/src/main.rs:394`). The vendor ID is in `PID_VENDORID`. Versions that are not listed in `tools/interop-results/src/systems.rs` are not measured.
2. **Check the vendor side for standard wire.** RTPS discovery, UDP instead of shared memory, standard ports, same domain ID, no domain tags. On the ZeroDDS side these points look like an interop error (`tools/dcps-shapes-interop.sh`, header comment "Three configuration traps").
3. **Compare the type before setting a flag.** Generate both sides from the same IDL. Annotate extensibility or set `--default-extensibility` as the peer does. Remove or replace constructs from the table in 6.3 for this peer. With `wireprobe-zerodds --dialect <peer>` the bytes the peer expects can be compared against `--dialect specification`.
4. **Set the data representation.** Determine the reader lists of the other side. If a foreign reader accepts only XCDR2: `wire.data_representation_offer: [XCDR2]`, otherwise leave the default. If a writer of a peer writes XCDR2 and another reader requires XCDR1, separate this writer per endpoint via `DataWriterQos.data_representation`; one writer does not serve both.
5. **Align the type check.** If endpoints do not match despite the same type name: check whether the ZeroDDS endpoint announces a TypeObject (C: `*_typed`), whether the peer requires XTypes information (OpenDDS `use_xtypes`), and whether key bounds differ (`xtypes.key_member_bounds`).
6. **Change the RTPS dialect only with a reason.** Leave `wire.rtps_dialect` at the default as long as no peer rejects the dispose form or the SPDP response. If a value is set, set it in `zerodds.yaml` (typos prevent startup) instead of in the environment (typos silently fall back to the default; report only on the observability sink).
7. **Enable vendor special paths that a feature requires.** `wire.big_endian` only for measurements.
8. **Check with sample counts in both directions.** A match is not proof; the measure is the number of decoded samples (`docs/interop-harnesses.md`, "The five stages of a live proof"). Template for a run: `tools/dcps-shapes-interop.sh`.
9. **Record the flag.** Store each vendor-related setting as its own drop-in (e.g. `zerodds.d/50-opendds.yaml`), so that it can be removed again together with the peer; `Config::origin_of` or the source attribution of the runtime names the file ([config.en.md](config.en.md)).

## 9 Open and contradictory points

| Point | Status |
|---|---|
| Application-side CDR dialects | The twelve layout dialects (`zerodds_ast_layout::Dialect`) are selectable only in `wireprobe-zerodds`; `zerodds-idlc` and the runtime codecs produce the spec layout. `docs/wire-axes-followup.md` ("The dialect pass: a name that reaches the product") describes them as a path to the product (D3). |
| `0x801c` request to Cyclone | has no effect: Cyclone reads vendor-specific PIDs only with the table of the sender's vendor ID (Eclipse/ADLINK, `ddsi_plist.c` 11.0.x); measured against Cyclone 11.0.1: 0 of 167 DATA with `PID_KEY_HASH` (D7). |
| GROUP-coherent sets against RTI | measured against Connext 7.7.0 (OMG `Test_CoherentSets_12`/`_21`, 3 runs each): `_21` OK, `_12` not. |
| OpenDDS reader representation list | named only in the code comment, not measured individually; M15/M16 only show that ZeroDDS reads the OpenDDS writer (XCDR2). |
| OpenSplice | live `ShapeType` (OMG `dds-rtps`, `-x 1`) and the KeyHash column of `tools/keyhash-probe` (six types, `PID_KEY_HASH` off the wire): no padding between keys of different widths, as `KEYHASH_HAS_PADDING 0` in its source says; XTypes 1.3 §7.6.8 pads. Axis `KeyHolderAlignment`, `packed` in the `opensplice` dialect (D9). |
| KeyHash under XCDR1 | Cyclone 11.0.1 and RTI 7.7.0 align the key holder to eight under XCDR1 (`@key short s; @key long long ll` → `0102 000000000000 0304…`) and to four under XCDR2 as §7.6.8 says; measured with `tools/keyhash-probe`. RTI sends it in `PID_KEY_HASH`. The `rti` and `cyclone` dialects reproduce it under XCDR1 for generated code and `wireprobe-zerodds` (`data-encoding`); the runtime always computes the KeyHash over the canonical XCDR2 holder, as it already does for order and nesting. |
| HDDS | serialisation with crate `hdds` 1.1.2/`hddsgen` 1.2.0, dds-xtypes suite with `hdds-1.3.0_test_main`, dds-rtps suite with `hdds-1.4.8_shape_main`. The generated serialiser's four departures from XTypes 1.3 are carried by `Dialect::Hdds`, opt-in; the dialect reproduces all 326 recorded rows. |

Fixed or measured: `idlc --dialect` in the comment (`e86e0a5739b`), the vendor ID exceptions in the `dialect` module header, the default writer representation in `cross-vendor-validation.md`, `PID_DOMAIN_TAG` (built, measured against Cyclone), `wire.data_representation_match: strict` (OMG `Test_DataRepresentation_0..3` against Connext, Cyclone, Fast DDS, both directions, 24/24), the security statement of `docs/interop-harnesses.md` (refers to the stages of its own chain), boolean runtime values (`true`/`false` are normalised to `1`/`0`, test `every_boolean_setting_reads_true_and_false_as_the_digits`).
