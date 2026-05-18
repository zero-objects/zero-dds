# `zerodds-ros2-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ros2-bridge-1.0.md`

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-ros2-shim Subcommands

**Spec:** §2 — Subcommands `info`/`topics`/`qos`/`enclaves`/`validate`/
`selftest`; Optionen `--config`/`--domain`/`--enclave`/`--log-level`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`,
`crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`,
`::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`,
`::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `ros2`/`discovery`/`logging`; QoS-Profile-Map;
ENV-Substitution.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (Config-Parser),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::validate_with_minimal_yaml_succeeds`.

**Status:** done

## §4 Wire-Protocol

### §4 RTPS direkt + REP-2007/2008/2009-Mangling

**Spec:** §4 — Native RTPS-Peer; Wire-Format gemäß `zerodds-xcdr2-
bindings-conformance-1.0` §3.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`,
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`.

**Status:** done

### §4.1 RMW-API-Mapping rmw_init/create_*/publish/take/...

**Spec:** §4.1 — Mapping rmw_*-Calls auf DCPS-Symbole.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`,
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`
(rmw_init+publish+take Loopback).

**Status:** done

### §4.2 Service-Pattern Request-Reply

**Spec:** §4.2 — Request-Topic + Reply-Topic + sample_identity-
Korrelation (DDS-RPC).

**Repo:** `crates/ros2-rmw/src/service.rs` (Cluster-C Service-Pair +
sample_identity-Korrelation),
`crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `service.rs`;
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (Service-Pair-Setup
via Cluster-C).

**Status:** done

### §4.3 Action-Pattern (5 Topics)

**Spec:** §4.3 — Actions sind composit aus 5 Topics; Shim wraps ohne
Sondermapping.

**Repo:** `crates/ros2-rmw/src/action.rs` (Cluster-C Action-Pattern
mit 5 Topics).

**Tests:** Inline `#[cfg(test)] mod tests` in `action.rs` (Action-
Server/Client mit allen 5 Topics via Cluster-C).

**Status:** done

## §5 Topic-Mapping

### §5.1 REP-2007 Topic-Mangling rt/rq/rr-Prefix

**Spec:** §5.1 — `/chatter` → `rt/chatter`; Service-Req → `rq/...Request`,
Reply → `rr/...Reply`.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (mangling helper),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::topic_mangle_emits_rt_prefix`.

**Status:** done

### §5.2 REP-2008 Type-Mapping .msg/.srv → IDL

**Spec:** §5.2 — `geometry_msgs/Pose` → `IDL:geometry_msgs/msg/dds_/
Pose_:1.0`.

**Repo:** `crates/ros2-rmw/src/msg_to_idl.rs` (REP-2008 Type-Mapping
Cluster-C),
`crates/ros2-rmw/src/type_mapping.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `msg_to_idl.rs`
(TypeObject-Discovery via Cluster-C).

**Status:** done

### §5.3 Bridge-Mode none

**Spec:** §5.3 — `topic_mangling: "none"` deaktiviert Prefix für Co-
Existence mit Non-ROS-DDS-Apps.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (Mangling-Mode-Switch).

**Tests:** Inline `#[cfg(test)] mod tests` in mangling-Hilfsfunktion.

**Status:** done

## §6 QoS-Translation

### §6 REP-2009 QoS-Profile sensor_data/services/parameters/...

**Spec:** §6 — sensor_data/services/parameters/parameter_events/
default-Profile-Map.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (QoS-Profile-Lookup),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`.

**Status:** done

## §7 Security

### §7.1 SROS2-Enclaves → DDS-Security 1.2

**Spec:** §7.1 — Enclave-Cert/Key/Permissions/Governance auf
`dds.sec.*`-Plugin-Properties. Decision-Record:
`docs/adr/0008-ros2-sros2-rejected-rc1.md` — DDS-Security 1.2 (K6
closed) deckt das gleiche Bedrohungsmodell direkt; SROS2-Enclave-
Mapping ist alternative Format-Form ohne Customer-Pull.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (enclave-Hook,
Spec-Schema), `crates/ros2-rmw/`,
`crates/security-permissions/` (DDS-Security 1.2 §9.4 deckt
Permissions-Schema direkt).

**Tests:** —

**Status:** n/a (rejected) — siehe ADR-0008.

### §7.2 ACL via Permissions-XML

**Spec:** §7.2 — Permissions-XML-Driven allow/deny pro Topic via
`dds.sec.access`. Decision-Record:
`docs/adr/0008-ros2-sros2-rejected-rc1.md` — Permissions-XML-Mapping
aus ROS-2-Sicht deferred; DDS-Security 1.2 Permissions-Plugin direkt
nutzbar.

**Repo:** delegiert an `crates/security-permissions/` über
`dds.sec.access`-Plugin.

**Tests:** —

**Status:** n/a (rejected) — siehe ADR-0008.

## §8 Operations + Observability

### §8.1 rcutils-Logging + JSON

**Spec:** §8.1 — `logging.format: "ros"` nutzt rcutils, `"json"`
strukturiert.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (Logging-Format-Switch),
`crates/ros2-rmw/src/json_log.rs` (Cluster-C rcutils + JSON-Sink-
Wireup).

**Tests:** Inline `#[cfg(test)] mod tests` in `json_log.rs`;
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (Logging-Format-Switch
via Cluster-C).

**Status:** done

### §8.2 Prometheus-Metrics via separater Exporter

**Spec:** §8.2 — `zerodds-ros2-metrics-exporter`-Prozess mit 8
Counter/Gauge-Familien. Cluster-A wired den Metrics-Endpoint direkt im
`zerodds-ros2-shim`-Diagnose-Binary statt als separaten Exporter.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (Cluster-A Counter/Gauge-
Familien Wireup), `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (Metrics-
Endpoint via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP rmw_publish/rmw_take Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (OTLP-Init via
`zerodds-observability-otlp`), `crates/ros2-rmw/` (Span-Emit pro
rmw_publish/rmw_take).

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (OTLP-
Endpoint via Cluster-A-Wireup).

**Status:** done

## §9 Lifecycle

### §9 RMW-API-Lifecycle (rcl-driven)

**Spec:** §9 — kein eigener Lifecycle; folgt rcl_init/node_init/...;
Signal-Handling übernimmt rcl/rclcpp.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`,
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`
(rcl-Sequence Loopback).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + FastRTPS/CycloneDDS/Connext

**Spec:** §10 — ZeroDDS-RMW-Shim ist nativer DDS-Peer; getestet mit
rmw_fastrtps_cpp/rmw_cyclonedds_cpp/rmw_connextdds.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`,
`crates/ros2-rmw/`.

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer; FastRTPS/CycloneDDS/Connext-Matrix).

**Status:** done

## §11 Packaging

### §11 librmw_zerodds_cpp.so + Diagnose-Binary

**Spec:** §11 — `librmw_zerodds_cpp.so` (Library, kein Daemon); .deb
pro ROS-Distro; `zerodds-ros2-shim` Diagnose-Binary; Configs/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-ros2-shim.service`,
`packaging/macos/launchd/org.zerodds.ros2-shim.plist`,
`packaging/macos/homebrew/zerodds-ros2.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ros2-shim/`,
`packaging/linux/configs/ros2-shim.yaml.example`,
`man/man1/zerodds-ros2-shim.1`,
`man/man5/zerodds-ros2-shim.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — mangling/qos_profile/node_registry/service_pair/
enclave je ≥ 5 Tests.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (inline tests),
`crates/ros2-rmw/`.

**Tests:** Inline `#[cfg(test)] mod tests` in `lib.rs`.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — ROS-2-talker/listener mit `RMW_IMPLEMENTATION=
rmw_zerodds_cpp`; selftest mit Service-Call.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`,
`::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`,
`::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — ROS-2-Container mit ZeroDDS-RMW + FastRTPS/Cyclone
auf gleichem ROS_DOMAIN_ID; Distros Humble/Iron/Jazzy.

**Repo:** `crates/ros2-rmw/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs` (FastRTPS/Cyclone
mit ROS_DOMAIN_ID-Matrix Humble/Iron/Jazzy).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + REPs + DDS-Security

**Spec:** §13 — Library `crates/ros2-rmw/`/`crates/rmw-zerodds-shim/`,
REP-2007/2008/2009 + SROS2, Wire-Format, Deployment, DDS-Security 1.2.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive QoS-Profile / ROS-Distro,
Major=RMW-API-Breaking.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

19 done / 0 partial / 0 open / 3 n/a (informative) / 2 n/a (rejected).

Test-Lauf: `cargo test -p rmw-zerodds-shim` — Tests grün, 0 failed.

Offene Punkte und Decision-Records: siehe `zerodds-ros2-bridge-1.0.open.md`.
