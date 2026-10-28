# `zerodds-ros2-bridge` v1.0 — ROS 2 RMW shim & topic mangling setup

ZeroDDS vendor spec. Specifies the activation and configuration
of the ROS 2 middleware wrapper (`rmw_zerodds`) as a native DDS vendor
under ROS 2.

## Motivation

ROS 2 (Humble, Iron, Jazzy, Rolling) uses RMW (ROS Middleware
Wrapper) as the vendor abstraction layer. The official vendors are
`rmw_fastrtps_cpp`, `rmw_cyclonedds_cpp`, `rmw_connextdds`. ZeroDDS
fits into this scheme as a native RMW provider — not a separate
daemon, but an `rmw_zerodds_cpp` module that ROS 2 apps load
directly.

Unlike the other bridges in this spec family
(WS/MQTT/CoAP/AMQP/gRPC/CORBA), **no separate daemon** is
needed here: ZeroDDS is a native RTPS peer, ROS 2 topic discovery happens
via standard SPDP/SEDP with ROS topic name mangling according to REP-2007.

This spec adds operations/configuration/compatibility
requirements to the library crates `crates/ros2-rmw/` (pure Rust
RMW logic) and `crates/rmw-zerodds-shim/` (C ABI shim for
`rmw_zerodds_cpp.so`).

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | Native RTPS peer (no separate wire protocol); conformant ROS topic name mangling per REP-2007. |
| **L2 — DDS** | DCPS public API exposed via C ABI; all ROS 2 QoS profiles translatable (REP-2009). |
| **L3 — Bridging** | The RMW shim implements the ROS 2 RMW API (rmw.h, rcl.h call side). |
| **L4 — Config** | YAML profiles + ROS enclave config; hot reload not applicable (no daemon). |
| **L5 — Auth** | SROS2 security enclaves (DDS-Security 1.2 maps to SROS2). |
| **L6 — Multi-Tenant** | ROS namespaces map to DDS partitions; ROS enclaves to DDS domain permissions. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

`zerodds-ros2-shim` is **not a daemon**, but a diagnostic tool:

```
zerodds-ros2-shim <SUBCOMMAND>

Subcommands:
  info                    show RMW compat level, ROS distro, loaded topics
  topics                  list of all active ROS topics + DDS mapping
  qos <ROS_TOPIC>         show the effective DDS QoS for the given ROS topic
  enclaves                list of all available security enclaves
  validate <CONFIG>       validate a YAML config against the schema
  selftest                round-trip test with loopback pub/sub

Options:
  --config <FILE>          runtime base with FILE.d/ (default: zerodds.yaml in the
                           system directory, which the RMW reads)
  --set <PATH=VALUE>       one value above all files, repeatable
  --check-config           check the runtime files and their ros2 section,
                           print every value with its origin
  --domain <ID>            ROS_DOMAIN_ID override
  --version                version info
  --help                   help

Rejected, with a reference to the place that carries the value:
  --runtime-config         → --config
  --log-level              → --ros-args --log-level (rcutils)
  --enclave                → security.enclave_dir (ZERODDS_SECURITY_DIR)

Exit codes:
  0   normal exit
  1   config error
  2   RMW init error
  3   ROS distro incompatibility
  4   self-test error
```

The shim is activated via ENV:
```bash
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
ros2 run demo_nodes_cpp talker
```

## §3 Config file format

The RMW reads its settings from the runtime configuration: `zerodds.yaml`
in the system directory, `zerodds.d/`, `include:`, the environment below it, see
[deployment configuration](zerodds-deployment-1.0.md#runtime-configuration-from-rc7-on).
The ROS settings are there in the `ros2` section, usually as the fragment
`zerodds.d/50-ros2.yaml` (template: `packaging/linux/configs/ros2.yaml.example`).
There is no separate shim file. Like every ZeroDDS program, the RMW takes
`--config` and `--set` from the command line, here the one of the ROS process:
arguments outside the `--ros-args … --` blocks, which belong to rcl.
`--config FILE` replaces the system directory as the base (together with `FILE.d/`),
`--set path=value` stands above all files:

```bash
ros2 run demo_nodes_cpp talker --config /home/robot/talker.yaml \
  --set ros2.topic_mangling=none --ros-args -r __node:=talker
```

In a launch file the flags are in `arguments=[…]` of the `Node`.
`--check-config` on the command line of a node has no effect; the report
is given by `zerodds-ros2-shim --check-config` with the same flags. The diagnostic CLI
reads the same layers. Other programs on the same host read the same `zerodds.yaml` and
leave `ros2.*` out of their check; the RMW checks the section strictly at startup
and does not start on an error (`rmw_init` returns an error,
the message is on stderr).

```yaml
ros2:
  topic_mangling: rmw            # rmw (rt/, REP-2007, default) | none | custom
  custom_mangling:               # only with topic_mangling: custom
    topic_prefix: "site/"        # then mandatory
    request_prefix: "rq/"        # service requests, default rq/
    response_prefix: "rr/"       # service replies, default rr/

  # REP-2009 table (§6). An entry replaces the fields that it names;
  # a new name adds a profile.
  qos_profiles:
    sensor_data:
      reliability: best_effort   # reliable | best_effort | system_default
      durability: volatile       # volatile | transient_local | system_default
      history: keep_last         # keep_last | keep_all | system_default
      depth: 5
      deadline_ms: 0             # 0 = unlimited
      lifespan_ms: 0
      liveliness: automatic      # automatic | manual_by_topic | system_default
      liveliness_lease_ms: 0

  # Per ROS topic (name before mangling): replaces field by field what the
  # application requests; either a profile of the table or a qos map.
  topic_overrides:
    - ros_topic: /scan
      qos_profile: sensor_data
    - ros_topic: /map
      qos:
        durability: transient_local
        depth: 1

  # TIME_BASED_FILTER minimum_separation in seconds, for all topics
  # (`0.1`) or per topic (`/scan=0.1`), comma list; env
  # ZERODDS_TIME_BASED_FILTER. rmw_qos_profile_t has no field for it.
  time_based_filter: "/scan=0.1"
```

Default table if the file replaces nothing: `sensor_data` KEEP_LAST(5)
BEST_EFFORT VOLATILE; `services` KEEP_LAST(10) RELIABLE VOLATILE; `parameters`
KEEP_LAST(1000) RELIABLE VOLATILE; `parameter_events` KEEP_ALL RELIABLE
VOLATILE; `default` KEEP_LAST(10) RELIABLE VOLATILE.

`avoid_ros_namespace_conventions` of an application omits every prefix,
including a `custom` prefix; the DDS topic is then the ROS name with a leading
`/` (`/chatter`), as with rmw_cyclonedds_cpp and rmw_fastrtps_cpp.

Rejected, each with a reason in the message:

| Key | Place that carries the value |
|-----------|----------------------------|
| `ros2.namespace` | rcl resolves names with the node namespace before the RMW sees them: `--ros-args -r __ns:=/ns`, `ROS_NAMESPACE` |
| `ros2.ros_domain_id` | rcl passes the domain to `rmw_init`: `ROS_DOMAIN_ID` |
| `ros2.enclave` | `security.enclave_dir` (`ZERODDS_SECURITY_DIR`) |
| `logging.*` | the RMW logs via rcutils: `--ros-args --log-level` |
| `discovery.multicast`, `discovery.unicast_initial_peers` | `discovery.no_multicast`, `discovery.peers` |

An unknown key under `ros2` is an error. ENV substitution:
`${VAR}` and `${VAR:-default}`.

## §4 Wire protocol

The ZeroDDS RMW shim speaks **RTPS directly** — no separate daemon.
Discovery happens via standard SPDP/SEDP. The wire format is according to
`zerodds-xcdr2-bindings-conformance-1.0` §3.

### §4.1 RMW API mapping

```
rmw_init                  → DCPS::DomainParticipantFactory::create_participant
rmw_create_node           → logical node = QoS group in the participant
rmw_create_publisher      → DataWriter
rmw_create_subscription   → DataReader
rmw_publish               → DataWriter::write
rmw_take                  → DataReader::take
rmw_create_service        → Request-Reader + Reply-Writer-Pair
rmw_create_client         → Request-Writer + Reply-Reader-Pair
rmw_destroy_*             → DCPS::delete_*
rmw_get_topic_names_and_types → DomainParticipant::get_discovered_topics
```

### §4.2 Service pattern (request-reply)

Topics `rq/<service>Request` and `rr/<service>Reply`, types
`<pkg>::srv::dds_::<Srv>_Request_` and `_Response_`, as rmw_cyclonedds_cpp
and rmw_fastrtps_cpp form them. The two correlate differently:

- **Cyclone form** (rmw_cyclonedds_cpp 1.3.4): before the CDR body there are
  `u64 guid` and `i64 seq`, the server repeats both. Client writer and
  reader carry `clientid=<id>;` in `USER_DATA`, the server `serviceid=<id>;`.
- **Related identity form** (rmw_fastrtps_cpp 6.2.7): no header in the payload.
  The request carries in the inline QoS a `related_sample_identity` with the GUID
  of the client's reply reader, the reply names this reader and the
  RTPS sequence number of the request. Fast DDS 2.6 only reads the
  vendor-specific PID 0x800f. ZeroDDS writes 0x800f and 0x0083
  (DDS-RPC 1.0 §7.8.2) and reads 0x800f only from eProsima and
  ZeroDDS writers.

A ZeroDDS server answers every request in the form in which it came; it
waits up to 100 ms until the client's reply reader is matched (like both
other RMWs), otherwise `RMW_RET_TIMEOUT`. A ZeroDDS client chooses the form
according to the vendor of the matched servers: Cyclone form if all are Cyclone DDS,
otherwise related identity form (also between ZeroDDS nodes).

Measured on ROS 2 Humble (Linux test host),
`crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh` with
`demo_nodes_py` AddTwoInts (2 + 3): server and client zerodds↔zerodds,
cyclonedds→zerodds, zerodds→cyclonedds, fastrtps→zerodds, zerodds→fastrtps
return `5`. Cyclone↔Fast DDS do not get calls through to each other (reference in
the same run). `ros2 node info` under Cyclone and Fast DDS lists the
ZeroDDS service with its type, and vice versa.

### §4.3 Action pattern (ROS 2 actions)

Actions are composed of 5 topics:
- `<service>/_action/send_goal` (service)
- `<service>/_action/cancel_goal` (service)
- `<service>/_action/get_result` (service)
- `<service>/_action/feedback` (topic)
- `<service>/_action/status` (topic)

The shim maps this without a special mapping — they are only topics + services.

## §5 Topic mapping

### §5.1 ROS 2 topic name mangling (REP-2007)

| ROS name | DDS topic | Direction |
|----------|-----------|-----------|
| `/chatter` | `rt/chatter` | Pub/Sub |
| `/foo/bar` | `rt/foo/bar` | Pub/Sub |
| Service `add_two_ints` (Req) | `rq/add_two_intsRequest` | DataReader (server) |
| Service `add_two_ints` (Reply) | `rr/add_two_intsReply` | DataWriter (server) |

Mode `rmw` (default): exactly REP-2007. Mode `none`: pass-through without
prefix (for bridge use cases). Mode `custom`: uses the `custom_mangling`
values.

### §5.2 Type mapping (REP-2008)

ROS `.msg`/`.srv` files → IDL via `rosidl_generator_dds_idl`:
```
geometry_msgs/Pose → IDL:geometry_msgs/msg/dds_/Pose_:1.0
sensor_msgs/Image → IDL:sensor_msgs/msg/dds_/Image_:1.0
```

Type discovery: ROS 2 apps register TypeObjects at
DataWriter/DataReader creation — other ROS 2 vendors do the same,
TypeLookup works cross-vendor.

### §5.3 Bridge mode (to/from non-ROS DDS)

`topic_mangling: "none"` disables the prefix → ROS `/chatter` becomes DDS
`chatter`. Allows coexistence with non-ROS DDS apps on the same
topic.

## §6 QoS translation

REP-2009 defines the standard profiles that are set in the `qos_profiles`
config:

| ROS 2 profile | DDS QoS |
|---------------|---------|
| `sensor_data` | BEST_EFFORT, VOLATILE, KEEP_LAST(5) |
| `services` | RELIABLE, VOLATILE, KEEP_LAST(10) |
| `parameters` | RELIABLE, VOLATILE, KEEP_LAST(1000) |
| `parameter_events` | RELIABLE, VOLATILE, KEEP_ALL |
| `default` | RELIABLE, VOLATILE, KEEP_LAST(10) |

Per-topic override via the `topic_overrides` list.

## §7 Security

### §7.1 SROS2 enclaves

ROS 2 uses SROS2 enclaves: one enclave directory per node,
`<enclave>/cert.pem` + `<enclave>/key.pem` + permissions XML.

Mapping to DDS-Security 1.2:
- Enclave cert → `dds.sec.auth.identity_certificate`
- Enclave key → `dds.sec.auth.private_key`
- Permissions XML → `dds.sec.access.permissions`
- Governance XML → `dds.sec.access.governance`

Enabled as with every RMW via rcl: `ROS_SECURITY_ENABLE=true`,
`ROS_SECURITY_STRATEGY=Enforce|Permissive`, `ROS_SECURITY_KEYSTORE` and the
enclave of the node (`--ros-args --enclave /name`). rcl resolves
`rmw_security_options_t` from these; `rmw_init` passes `security_root_path` and
`enforce_security` on to `rmw_zerodds_init_with_security`, which builds
the secured participant from them. Alternatively, `security.enclave_dir`
of the runtime configuration names the directory; if both name a different one,
the node does not start. `Enforce` without an enclave and an RMW without the feature
`security` with ROS security switched on fail as well.
Measured: `rmw_c/run_ros2_security_test.sh` (Linux test host, Humble): secured
zerodds talker to secured zerodds listener 12 messages, unsecured
listener 0 (counter-check), secured `rmw_cyclonedds_cpp` listener 13; the
script checks all three pairs.

### §7.2 ACL via permissions

The permissions XML per enclave defines allow/deny per topic; the shim
maps directly to `dds.sec.access` plugin calls.

## §8 Operations + observability

### §8.1 Logging

The ROS standard rcutils log system is used if
`logging.format: "ros"`. JSON format if `"json"`.

### §8.2 Prometheus metrics

Optional via the separate process `zerodds-ros2-metrics-exporter`:
```
zerodds_ros2_publishers_total          gauge{node, topic}
zerodds_ros2_subscriptions_total       gauge{node, topic}
zerodds_ros2_services_total            gauge{node, service}
zerodds_ros2_clients_total             gauge{node, service}
zerodds_ros2_messages_published_total  counter{topic}
zerodds_ros2_messages_taken_total      counter{topic}
zerodds_ros2_qos_violations_total      counter{topic, kind}
zerodds_ros2_discovered_participants   gauge
zerodds_ros2_discovered_topics         gauge
```

### §8.3 OTLP

`OTEL_EXPORTER_OTLP_ENDPOINT` set: rmw_publish/rmw_take spans.

## §9 Lifecycle

The ZeroDDS RMW shim has no lifecycle of its own — it follows the ROS 2
rcl lifecycle:

| ROS 2 phase | RMW shim action |
|-------------|------------------|
| `rcl_init` | `rmw_init` → DDS participant factory init |
| `rcl_node_init` | `rmw_create_node` → logical node registry entry |
| `rcl_publisher_init` | `rmw_create_publisher` → DataWriter |
| `rcl_node_fini` | `rmw_destroy_node` → cleanup of all writers/readers |
| `rcl_shutdown` | `rmw_shutdown` → participant cleanup |

Signal handling is done by rcl/rclcpp.

## §10 Cross-vendor

The ZeroDDS RMW shim is a native DDS peer. Cross-vendor with:
- `rmw_fastrtps_cpp` (default vendor of ROS 2)
- `rmw_cyclonedds_cpp` (Open Robotics)
- `rmw_connextdds` (RTI)

Measured on ROS 2 Humble (Linux test host), `rmw_c/run_qos_translation_test.sh`
with `RMW_PUB`/`RMW_SUB` and `ros2 topic pub`/`echo`:
- Topics in both directions with `rmw_cyclonedds_cpp` and
  `rmw_fastrtps_cpp`: `std_msgs/Int32`, `std_msgs/String`,
  `geometry_msgs/Twist`; a TRANSIENT_LOCAL KEEP_LAST(1) late joiner in the
  second process receives exactly the newest sample.
- Node graph in both directions: the respective other RMW assigns the
  endpoints to their nodes (`ros_discovery_info`).
- Services in both directions with `rmw_cyclonedds_cpp` and
  `rmw_fastrtps_cpp` (§4.2). The two do not exchange
  service calls with each other (measured).
- `rmw_connextdds` not measured (not installed in the test env).

ROS distros: Humble (LTS), Iron, Jazzy (LTS), Rolling. A separate build target
per distro (RMW API stability levels).

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Library: `librmw_zerodds_cpp.so` (not a daemon)
- One .deb package per ROS distro: `ros-humble-rmw-zerodds-cpp`,
  `ros-iron-rmw-zerodds-cpp`, etc. — on the `packages.ros.org` mirror.
- Diagnostic binary: `zerodds-ros2-shim` in the package `zerodds-ros2` (deb, rpm),
  without systemd unit and launchd plist: the tool is not a daemon.
- Configuration: the `ros2` section of the runtime configuration (§3), as
  the fragment `/etc/zerodds/zerodds.d/50-ros2.yaml`; template
  `/usr/share/zerodds/configs/ros2.yaml.example`.
- Manual: `man 1 zerodds-ros2-shim` + `man 5 zerodds-ros2.yaml`
- Docker: `zerodds/ros2-humble:1.0`, `zerodds/ros2-iron:1.0`,
  `zerodds/ros2-jazzy:1.0` (with the ros2 image as base + ZeroDDS RMW
  preinstalled)

Measured state: `librmw_zerodds_cpp.so` is in no package and no
image. Today it is only built via
`crates/rmw-zerodds-shim/rmw_c/build_librmw_zerodds_cpp.sh` against an
installed ROS 2. The Docker images contain the diagnostic binary
(default command `doctor`). The `ros-<distro>-rmw-zerodds-cpp` packages
and the preinstalled RMW are open.

## §12 Testing

### §12.1 Unit tests

`cargo test -p rmw-zerodds-shim` (modules `qos`, `services`, `events`,
`ros2_config` and the bridge in `lib.rs`) and `cargo test -p zerodds-ros2-rmw`
(`topic_mangling`, `qos_profiles`, `rmw_qos_mapping`, `service`, `action`,
`type_mapping`, `msg_to_idl`).

### §12.2 Integration tests

Without a ROS installation: `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs`
(`zerodds-ros2-shim`) and `tests/ros2_ext_surface.rs` (REP-2007/2008 surface).

Against a ROS 2 installation (`ROS_PREFIX=<install>`) the scripts
`crates/rmw-zerodds-shim/rmw_c/run_*.sh`: `run_ros2_pytest.sh` (rclpy tests
in `crates/py/python/tests/ros2/`: pub/sub, service, graph, QoS events,
network flow endpoints, content filter),
`run_loaned_message_test.sh`, `run_content_filter_test.sh`,
`run_type_adaptation_test.sh`, `run_accel_loan_memory_test.sh`,
`run_type_negotiation_test.sh`, `run_ros2_config_test.sh`,
`run_ament_package_test.sh`.

### §12.3 Multi-vendor

- Topics: `run_qos_translation_test.sh` with `RMW_PUB`/`RMW_SUB` against
  `rmw_cyclonedds_cpp` and `rmw_fastrtps_cpp`, both directions.
- Services: `run_ros2_services_test.sh` (AddTwoInts, graph queries), the same
  partners, server on both sides.
- Security: `run_ros2_security_test.sh` (§7.1).
- `crates/ros2-rmw/tests/cross_vendor.rs` checks the wire conventions of
  `rmw_cyclonedds_cpp` without ROS; the live parts carry `#[ignore]`.

ROS distro: measured on Humble. `ci/jobs/rmw-distro-build.yml` describes
builds for Humble, Iron and Jazzy, no pipeline includes the file; Iron
and Jazzy are not measured.

## §13 Cross-references

- Library: `crates/ros2-rmw/`, `crates/rmw-zerodds-shim/`.
- ROS standards: REP-2007 (topic mapping), REP-2008 (type mapping),
  REP-2009 (QoS profiles), SROS2 (security).
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- DDS-Security mapping: OMG DDS-Security 1.2 (formal/2018-09-01).

## §14 Versioning

`1.0` initial. Patch for bug fixes, minor for additive QoS profiles
or new ROS distro targets, major on RMW API breaking changes
(e.g. ROS 2 → ROS 3).
