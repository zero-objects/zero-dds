# `zerodds-ros2-bridge` v1.0 — ROS-2 RMW-Shim & Topic-Mangling Setup

ZeroDDS Vendor-Spec. Spezifiziert die Aktivierung und Konfiguration
des ROS-2-Middleware-Wrappers (`rmw_zerodds`) als nativer DDS-Vendor
unter ROS-2.

## Motivation

ROS-2 (Humble, Iron, Jazzy, Rolling) verwendet RMW (ROS Middleware
Wrapper) als Vendor-Abstraction-Layer. Die offiziellen Vendoren sind
`rmw_fastrtps_cpp`, `rmw_cyclonedds_cpp`, `rmw_connextdds`. ZeroDDS
fügt sich als nativer RMW-Provider in dieses Schema ein — kein eigener
Daemon, sondern ein `rmw_zerodds_cpp`-Modul, das ROS-2-Apps direkt
laden.

Im Gegensatz zu den anderen Bridges in dieser Spec-Familie
(WS/MQTT/CoAP/AMQP/gRPC/CORBA) ist hier **kein separater Daemon**
nötig: ZeroDDS ist nativer RTPS-Peer, ROS-2-Topic-Discovery erfolgt
über Standard-SPDP/SEDP mit ROS-Topic-Name-Mangling gemäß REP-2007.

Diese Spec ergänzt die Library-Crates `crates/ros2-rmw/` (Pure-Rust-
RMW-Logik) und `crates/rmw-zerodds-shim/` (C-ABI-Shim für
`rmw_zerodds_cpp.so`) um Operations-/Konfigurations-/Compatibility-
Vorgaben.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Native RTPS-Peer (kein separates Wire-Protokoll); konformes ROS-Topic-Name-Mangling per REP-2007. |
| **L2 — DDS** | DCPS-Public-API über C-ABI exposed; alle ROS-2-QoS-Profile übersetzbar (REP-2009). |
| **L3 — Bridging** | RMW-Shim implementiert die ROS-2 RMW-API (rmw.h, rcl.h-Aufrufseite). |
| **L4 — Config** | YAML-Profile + ROS-Enclave-Config; Hot-Reload nicht anwendbar (kein Daemon). |
| **L5 — Auth** | SROS2-Security-Enclaves (DDS-Security 1.2 mappt auf SROS2). |
| **L6 — Multi-Tenant** | ROS-Namespaces mappen auf DDS-Partitions; ROS-Enclaves auf DDS-Domain-Permissions. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

`zerodds-ros2-shim` ist **kein Daemon**, sondern ein Diagnose-Tool:

```
zerodds-ros2-shim <SUBCOMMAND>

Subcommands:
  info                    Zeige RMW-Compat-Level, ROS-Distro, geladene Topics
  topics                  Liste aller aktiven ROS-Topics + DDS-Mapping
  qos <ROS_TOPIC>         Zeige effektive DDS-QoS für gegebenen ROS-Topic
  enclaves                Liste aller verfügbaren Security-Enclaves
  validate <CONFIG>       Validiere YAML-Config gegen Schema
  selftest                Round-trip-Test mit Loopback-Pub/Sub

Options:
  --config <FILE>          Path zur Config (Default $ROS_HOME/zerodds.yaml)
  --domain <ID>            ROS_DOMAIN_ID-Override
  --enclave <PATH>         ROS-Enclave-Pfad
  --log-level <LEVEL>      trace/debug/info/warn/error (Default info)
  --version                Versions-Info
  --help                   Hilfe

Exit-Codes:
  0   normaler Exit
  1   Config-Fehler
  2   RMW-Init-Fehler
  3   ROS-Distro-Inkompatibilität
  4   Self-Test-Fehler
```

Aktivierung der Shim erfolgt per ENV:
```bash
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
export ZERODDS_CONFIG=/etc/zerodds/ros2.yaml
ros2 run demo_nodes_cpp talker
```

## §3 Config-File-Format

YAML-Schema:

```yaml
# /etc/zerodds/ros2.yaml (oder $ZERODDS_CONFIG)
ros2:
  namespace: "/zerodds"               # Default-Namespace falls nicht im Code
  topic_mangling: "rmw"               # rmw (default) | none | custom
  custom_mangling:
    topic_prefix: "rt/"
    request_prefix: "rq/"
    response_prefix: "rr/"
  enclave: "/security_enclaves/foo"   # SROS2-Enclave-Pfad
  ros_domain_id: 0                    # ROS_DOMAIN_ID

  # Mapping ROS-2 Standard-QoS-Profile → DDS-QoS (REP-2009)
  qos_profiles:
    sensor_data:
      reliability: "best_effort"
      durability: "volatile"
      history: { kind: "keep_last", depth: 5 }
      deadline: { sec: 0, nsec: 0 }
    services:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
    parameters:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 1000 }
    parameter_events:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_all" }
    default:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }

  # Per-Topic-Override (nach Mangling)
  topic_overrides:
    - ros_topic: "/chatter"
      qos:
        reliability: "reliable"
        history: { kind: "keep_last", depth: 100 }

discovery:
  unicast_initial_peers: ["192.168.10.5", "192.168.10.6"]
  multicast: true
  participant_lease_secs: 30

logging:
  level: "info"
  format: "ros"                       # ros | json
```

ENV-Substitution: `${VAR}` und `${VAR:-default}`.

## §4 Wire-Protocol

ZeroDDS-RMW-Shim spricht **direkt RTPS** — kein separater Daemon.
Discovery erfolgt über Standard-SPDP/SEDP. Wire-Format ist gemäß
`zerodds-xcdr2-bindings-conformance-1.0` §3.

### §4.1 RMW-API-Mapping

```
rmw_init                  → DCPS::DomainParticipantFactory::create_participant
rmw_create_node           → Logical-Node = QoS-Group im Participant
rmw_create_publisher      → DataWriter
rmw_create_subscription   → DataReader
rmw_publish               → DataWriter::write
rmw_take                  → DataReader::take
rmw_create_service        → Request-Reader + Reply-Writer-Pair
rmw_create_client         → Request-Writer + Reply-Reader-Pair
rmw_destroy_*             → DCPS::delete_*
rmw_get_topic_names_and_types → DomainParticipant::get_discovered_topics
```

### §4.2 Service-Pattern (Request-Reply)

ROS-2-Services nutzen DDS-RPC (OMG DDS-RPC 1.0) mit:
- Request-Topic: `<request_prefix><namespace>/<service>Request`
- Reply-Topic: `<response_prefix><namespace>/<service>Reply`
- Correlation per `sample_identity` im SampleInfo (DDS-RPC-Header).

### §4.3 Action-Pattern (ROS-2 Actions)

Actions sind composit aus 5 Topics:
- `<service>/_action/send_goal` (Service)
- `<service>/_action/cancel_goal` (Service)
- `<service>/_action/get_result` (Service)
- `<service>/_action/feedback` (Topic)
- `<service>/_action/status` (Topic)

Shim wird das ohne Sondermapping ab — sind nur Topics + Services.

## §5 Topic-Mapping

### §5.1 ROS-2 Topic-Name-Mangling (REP-2007)

| ROS-Name | DDS-Topic | Direction |
|----------|-----------|-----------|
| `/chatter` | `rt/chatter` | Pub/Sub |
| `/foo/bar` | `rt/foo/bar` | Pub/Sub |
| Service `add_two_ints` (Req) | `rq/add_two_intsRequest` | DataReader (Server) |
| Service `add_two_ints` (Reply) | `rr/add_two_intsReply` | DataWriter (Server) |

Mode `rmw` (default): exakt REP-2007. Mode `none`: pass-through ohne
Prefix (für Bridge-Use-Cases). Mode `custom`: nutzt `custom_mangling`-
Werte.

### §5.2 Type-Mapping (REP-2008)

ROS-`.msg`/`.srv`-Files → IDL via `rosidl_generator_dds_idl`:
```
geometry_msgs/Pose → IDL:geometry_msgs/msg/dds_/Pose_:1.0
sensor_msgs/Image → IDL:sensor_msgs/msg/dds_/Image_:1.0
```

Type-Discovery: ROS-2-Apps registrieren TypeObjects beim
DataWriter/DataReader-Create — andere ROS-2-Vendoren tun dasselbe,
TypeLookup funktioniert cross-vendor.

### §5.3 Bridge-Mode (zu/von Non-ROS-DDS)

`topic_mangling: "none"` deaktiviert Prefix → ROS-`/chatter` wird DDS-
`chatter`. Erlaubt Co-Existence mit Non-ROS-DDS-Apps auf demselben
Topic.

## §6 QoS-Translation

REP-2009 definiert die Standard-Profile, die in `qos_profiles`-Config
gesetzt sind:

| ROS-2-Profile | DDS-QoS |
|---------------|---------|
| `sensor_data` | BEST_EFFORT, VOLATILE, KEEP_LAST(5) |
| `services` | RELIABLE, VOLATILE, KEEP_LAST(10) |
| `parameters` | RELIABLE, VOLATILE, KEEP_LAST(1000) |
| `parameter_events` | RELIABLE, VOLATILE, KEEP_ALL |
| `default` | RELIABLE, VOLATILE, KEEP_LAST(10) |

Per-Topic-Override per `topic_overrides`-Liste.

## §7 Security

### §7.1 SROS2-Enclaves

ROS-2 nutzt SROS2-Enclaves: pro Node ein Enclave-Verzeichnis
`<enclave>/cert.pem` + `<enclave>/key.pem` + Permissions-XML.

Mapping auf DDS-Security-1.2:
- Enclave-Cert → `dds.sec.auth.identity_certificate`
- Enclave-Key → `dds.sec.auth.private_key`
- Permissions-XML → `dds.sec.access.permissions`
- Governance-XML → `dds.sec.access.governance`

ENV `ROS_SECURITY_ENABLE=true` + `ROS_SECURITY_STRATEGY=Enforce` aktiviert.

### §7.2 ACL via Permissions

Permissions-XML pro Enclave definiert allow/deny pro Topic; Shim
mappt direkt auf `dds.sec.access`-Plugin-Calls.

## §8 Operations + Observability

### §8.1 Logging

ROS-Standard-rcutils-Log-System wird verwendet wenn
`logging.format: "ros"`. JSON-Format wenn `"json"`.

### §8.2 Prometheus-Metrics

Optional via separater Prozess `zerodds-ros2-metrics-exporter`:
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

`OTEL_EXPORTER_OTLP_ENDPOINT` setzt: rmw_publish/rmw_take Spans.

## §9 Lifecycle

ZeroDDS-RMW-Shim hat keinen eigenen Lifecycle — er folgt dem ROS-2-
rcl-Lifecycle:

| ROS-2-Phase | RMW-Shim-Action |
|-------------|------------------|
| `rcl_init` | `rmw_init` → DDS-Participant-Factory init |
| `rcl_node_init` | `rmw_create_node` → Logical-Node-Registry-Eintrag |
| `rcl_publisher_init` | `rmw_create_publisher` → DataWriter |
| `rcl_node_fini` | `rmw_destroy_node` → Cleanup aller Writer/Reader |
| `rcl_shutdown` | `rmw_shutdown` → Participant-Cleanup |

Signal-Handling übernimmt rcl/rclcpp.

## §10 Cross-Vendor

ZeroDDS-RMW-Shim ist nativer DDS-Peer. Cross-Vendor mit:
- `rmw_fastrtps_cpp` (Default-Vendor von ROS-2)
- `rmw_cyclonedds_cpp` (Open Robotics)
- `rmw_connextdds` (RTI)

Verifiziert in `crates/rmw-zerodds-shim/tests/cross_vendor.rs`:
- ROS-2-Talker (FastRTPS) → ROS-2-Listener (ZeroDDS) auf `/chatter`.
- ZeroDDS-Talker → CycloneDDS-Listener.
- Bidirectionale Service-Calls.

ROS-Distros: Humble (LTS), Iron, Jazzy (LTS), Rolling. Pro Distro
eigenes Build-Target (RMW-API-Stabilitäts-Levels).

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Library: `librmw_zerodds_cpp.so` (nicht Daemon)
- Pro ROS-Distro ein .deb-Package: `ros-humble-rmw-zerodds-cpp`,
  `ros-iron-rmw-zerodds-cpp`, etc. — auf `packages.ros.org`-Mirror.
- Diagnose-Binary: `zerodds-ros2-shim`
- Config-Default: `$ROS_HOME/zerodds.yaml` oder `/etc/zerodds/ros2.yaml`
- Manual: `man 1 zerodds-ros2-shim` + `man 5 zerodds-ros2.yaml`
- Docker: `zerodds/ros2-humble:1.0`, `zerodds/ros2-iron:1.0`,
  `zerodds/ros2-jazzy:1.0` (mit ros2-Image als Base + ZeroDDS-RMW
  vorinstalliert)

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`mangling`, `qos_profile`, `node_registry`, `service_pair`,
`enclave`) ≥ 5 Tests in `crates/ros2-rmw/`, `crates/rmw-zerodds-shim/`.

### §12.2 Integration-Tests

`crates/rmw-zerodds-shim/tests/bridge_e2e.rs`:
- ROS-2-Demo-Node `talker` mit `RMW_IMPLEMENTATION=rmw_zerodds_cpp`.
- ROS-2-Demo-Node `listener` mit gleichem RMW.
- Verify dass Messages ankommen.
- Selftest mit Service-Call.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs`: ROS-2-Container mit ZeroDDS-RMW + ROS-2-
Container mit FastRTPS / CycloneDDS auf gleichem `ROS_DOMAIN_ID`,
Topic-Roundtrip verifiziert.

ROS-Distro-Matrix: Humble, Iron, Jazzy.

## §13 Cross-References

- Library: `crates/ros2-rmw/`, `crates/rmw-zerodds-shim/`.
- ROS-Standards: REP-2007 (Topic-Mapping), REP-2008 (Type-Mapping),
  REP-2009 (QoS-Profiles), SROS2 (Security).
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- DDS-Security-Mapping: OMG DDS-Security 1.2 (formal/2018-09-01).

## §14 Versioning

`1.0` initial. Patch für Bugfixes, Minor für additive QoS-Profile
oder neue ROS-Distro-Targets, Major bei RMW-API-Breaking-Changes
(z.B. ROS-2 → ROS-3).
