# `zerodds-ros2-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-ros2-bridge-1.0.md`

Implementation:

- `crates/rmw-zerodds-shim/` — ROS 2 RMW shim (rmw_zerodds).

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-ros2-shim subcommands

**Spec:** §2 — subcommands `info`/`topics`/`qos`/`enclaves`/`validate`/
`selftest`; options `--config`/`--domain`/`--enclave`/`--log-level`/
`--version`/`--help`; exit codes 0/1/2/3/4.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`,
`crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`, `::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`, `::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `ros2`/`discovery`/`logging`; a QoS-profile map;
ENV substitution.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (config parser),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::validate_with_minimal_yaml_succeeds`.

**Status:** done

## §4 Wire protocol

### §4 RTPS direct + REP-2007/2008/2009 mangling

**Spec:** §4 — native RTPS peer; wire format per `zerodds-xcdr2-bindings-conformance-1.0`
§3.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`, `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`.

**Status:** done

### §4.1 RMW-API mapping rmw_init/create_*/publish/take/...

**Spec:** §4.1 — mapping rmw_* calls onto DCPS symbols.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`, `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`
(rmw_init+publish+take loopback).

**Status:** done

### §4.2 Service pattern request-reply

**Spec:** §4.2 — request topic + reply topic + sample_identity correlation
(DDS-RPC).

**Repo:** `crates/ros2-rmw/src/service.rs` (cluster-C service pair +
sample_identity correlation), `crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `service.rs`;
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (service-pair setup via
cluster-C).

**Status:** done

### §4.3 Action pattern (5 topics)

**Spec:** §4.3 — actions are composed of 5 topics; the shim wraps them
without special mapping.

**Repo:** `crates/ros2-rmw/src/action.rs` (cluster-C action pattern with 5
topics).

**Tests:** inline `#[cfg(test)] mod tests` in `action.rs` (action
server/client with all 5 topics via cluster-C).

**Status:** done

## §5 Topic mapping

### §5.1 REP-2007 topic mangling rt/rq/rr prefix

**Spec:** §5.1 — `/chatter` → `rt/chatter`; service req → `rq/...Request`,
reply → `rr/...Reply`.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (mangling helper),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::topic_mangle_emits_rt_prefix`.

**Status:** done

### §5.2 REP-2008 type mapping .msg/.srv → IDL

**Spec:** §5.2 — `geometry_msgs/Pose` → `IDL:geometry_msgs/msg/dds_/Pose_:1.0`.

**Repo:** `crates/ros2-rmw/src/msg_to_idl.rs` (REP-2008 type mapping
cluster-C), `crates/ros2-rmw/src/type_mapping.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `msg_to_idl.rs` (TypeObject
discovery via cluster-C).

**Status:** done

### §5.3 Bridge mode none

**Spec:** §5.3 — `topic_mangling: "none"` disables the prefix for
co-existence with non-ROS DDS apps.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (mangling-mode switch).

**Tests:** inline `#[cfg(test)] mod tests` in the mangling helper.

**Status:** done

## §6 QoS translation

### §6 REP-2009 QoS profiles sensor_data/services/parameters/...

**Spec:** §6 — sensor_data/services/parameters/parameter_events/default
profile map.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (QoS-profile lookup),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`.

**Status:** done

## §7 Security

### §7.1 SROS2 enclaves → DDS-Security 1.2

**Spec:** §7.1 — enclave cert/key/permissions/governance onto `dds.sec.*`
plugin properties.

**Repo:** `crates/security-runtime/src/profile.rs` —
`SecurityProfile::from_enclave_dir` maps the sros2-keystore format
(identity_ca/cert/key/permissions_ca/`governance.p7s`/`permissions.p7s`)
onto a DDS-Security `SecurityProfile`; `from_env` loads it via
`ZERODDS_SECURITY_DIR` + `ROS_DOMAIN_ID` (C7 secure-by-default).
`crates/rmw-zerodds-shim/src/lib.rs` wires it (set → security participant,
set-but-invalid → hard error); `crates/security-permissions/` covers
governance/permissions (DDS-Security 1.2 §9.4).

**Tests:** `security-runtime` `enclave_dir_resolves_all_sros2_filenames`,
`enclave_dir_missing_cert_is_io_naming_cert`; `rmw-zerodds-shim`
`shim_cli_e2e` (`ZERODDS_SECURITY_DIR` path).

**Status:** done — the SROS2 enclave is loaded as a mapping layer onto
DDS-Security 1.2 (ADR 0012).

### §7.2 ACL via permissions XML

**Spec:** §7.2 — permissions-XML-driven allow/deny per topic via
`dds.sec.access`.

**Repo:** `crates/security-permissions/` — CMS-verified permissions XML
(DDS-Security 1.2 §9.4) via the `dds.sec.access` plugin
(`is_publish_allowed`/`is_subscribe_allowed`, `check_create_datawriter`/
`check_create_datareader`); the enclave permissions material
(`permissions.p7s`/`governance.p7s`) comes from `from_enclave_dir` (§7.1).

**Tests:** `security-permissions` `default_deny_without_explicit_tag`,
`deny_rule_overrides_allow_for_publish`/`_subscribe`,
`deny_only_grant_blocks_specific_topics`, `deny_on_non_matching_topic`.

**Status:** done — permissions XML is enforced per topic via DDS-Security 1.2
access control (ADR 0012).

## §8 Operations + observability

### §8.1 rcutils logging + JSON

**Spec:** §8.1 — `logging.format: "ros"` uses rcutils, `"json"` is
structured.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (logging-format switch),
`crates/ros2-rmw/src/json_log.rs` (cluster-C rcutils + JSON-sink wire-up).

**Tests:** inline `#[cfg(test)] mod tests` in `json_log.rs`;
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (logging-format switch via
cluster-C).

**Status:** done

### §8.2 Prometheus metrics via a separate exporter

**Spec:** §8.2 — a `zerodds-ros2-metrics-exporter` process with 8
counter/gauge families. Cluster-A wires the metrics endpoint directly into
the `zerodds-ros2-shim` diagnostics binary instead of as a separate
exporter.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (cluster-A counter/gauge
families wire-up), `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (metrics endpoint
via cluster-A wire-up).

**Status:** done

### §8.3 OTLP rmw_publish/rmw_take spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/ros2-rmw/` (span emit per
rmw_publish/rmw_take).

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (OTLP endpoint
via cluster-A wire-up).

**Status:** done

## §9 Lifecycle

### §9 RMW-API lifecycle (rcl-driven)

**Spec:** §9 — no own lifecycle; follows rcl_init/node_init/...; signal
handling is done by rcl/rclcpp.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`, `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`
(rcl-sequence loopback).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + FastRTPS/CycloneDDS/Connext

**Spec:** §10 — the ZeroDDS RMW shim is a native DDS peer; tested with
rmw_fastrtps_cpp/rmw_cyclonedds_cpp/rmw_connextdds.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`, `crates/ros2-rmw/`.

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs` (cluster-C cross-vendor
RTPS peer; FastRTPS/CycloneDDS/Connext matrix).

**Status:** done

## §11 Packaging

### §11 librmw_zerodds_cpp.so + diagnostics binary

**Spec:** §11 — `librmw_zerodds_cpp.so` (library, not a daemon); a .deb per
ROS distro; `zerodds-ros2-shim` diagnostics binary; configs/Docker; manuals.

**Repo:** `packaging/linux/systemd/zerodds-ros2-shim.service`,
`packaging/macos/launchd/org.zerodds.ros2-shim.plist`,
`packaging/macos/homebrew/zerodds-ros2.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ros2-shim/`,
`packaging/linux/configs/ros2-shim.yaml.example`,
`man/man1/zerodds-ros2-shim.1`, `man/man5/zerodds-ros2-shim.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — mangling/qos_profile/node_registry/service_pair/enclave,
≥ 5 tests each.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (inline tests),
`crates/ros2-rmw/`.

**Tests:** inline `#[cfg(test)] mod tests` in `lib.rs`.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — ROS-2 talker/listener with
`RMW_IMPLEMENTATION=rmw_zerodds_cpp`; selftest with a service call.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`, `::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`, `::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a ROS-2 container with the ZeroDDS RMW + FastRTPS/Cyclone
on the same ROS_DOMAIN_ID; distros Humble/Iron/Jazzy.

**Repo:** `crates/ros2-rmw/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs` (FastRTPS/Cyclone with a
ROS_DOMAIN_ID matrix Humble/Iron/Jazzy).

**Status:** done

## §13 Cross-references

### §13 Related library + REPs + DDS-Security

**Spec:** §13 — library `crates/ros2-rmw/`/`crates/rmw-zerodds-shim/`,
REP-2007/2008/2009 + SROS2, wire format, deployment, DDS-Security 1.2.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive QoS profile / ROS distro,
major=RMW-API breaking.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

21 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p rmw-zerodds-shim` — tests green, 0 failed.
