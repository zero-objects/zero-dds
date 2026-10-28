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
`selftest`; options `--config`/`--set`/`--check-config`/`--domain`/
`--version`/`--help`; refused with a pointer: `--runtime-config`/`--log-level`/`--enclave`; exit codes 0/1/2/3/4.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`,
`crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`, `::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`, `::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`,
`::flags_of_the_old_shim_file_are_refused_with_their_replacement`.

**Status:** done

## §3 Config-file format

### §3 The `ros2` section of the runtime configuration

**Spec:** §3 — `ros2.topic_mangling`/`custom_mangling.topic_prefix`,
`qos_profiles` (the REP-2009 table, replaceable field by field),
`topic_overrides` (`qos_profile` or a `qos` map), `time_based_filter`; keys
rcl owns are refused; ENV substitution.

**Repo:** `crates/rmw-zerodds-shim/src/ros2_config.rs` (`Ros2Config`),
`crates/rmw-zerodds-shim/src/lib.rs` (`rmw_zerodds_init`,
`rmw_zerodds_create_publisher`/`_subscription`: `effective_qos`,
`dds_topic`), `crates/dcps/src/runtime.rs` (`is_rmw_path`),
`packaging/linux/configs/ros2.yaml.example`, `man/man5/zerodds-ros2.yaml.5`.

**Tests:** `crates/rmw-zerodds-shim/src/ros2_config.rs::tests` (6),
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::topic_shows_the_configured_mangling_and_override`,
`::check_config_refuses_what_rcl_owns`,
`crates/dcps/tests/runtime_config_layering.rs::the_rmw_section_is_not_an_unknown_key_for_other_programs`,
`tools/traceability/tests/bridge_config_manpages.rs`.

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

**Spec:** §4.2 — `rq/<service>Request`/`rr/<service>Reply` with the types
`<pkg>::srv::dds_::<Srv>_Request_`/`_Response_`; correlation in the peer's
form (Cyclone: `guid`/`seq` before the body; Fast DDS:
`related_sample_identity`, PID 0x800f); graph (`ros2 service list`,
`ros2 node info`).

**Repo:** `crates/rmw-zerodds-shim/src/services.rs`,
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c` (service calls, graph),
`crates/dcps/src/runtime.rs` (`write_user_sample_related`,
`UserSample::Alive.related_sample_identity`, `participant_vendor`),
`crates/rtps/src/inline_qos.rs` (`PID_EPROSIMA_RELATED_SAMPLE_IDENTITY`).

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh`
(Linux test host, Humble, `demo_nodes_py` AddTwoInts 2 + 3: server/client
zerodds↔zerodds, cyclonedds→zerodds, zerodds→cyclonedds,
fastrtps→zerodds, zerodds→fastrtps each `Result of add_two_ints: 5`),
`crates/rmw-zerodds-shim/src/lib.rs::tests::{a_service_call_between_two_runtimes_correlates_by_the_related_identity,
two_clients_in_one_runtime_each_get_their_own_reply}`,
`crates/rmw-zerodds-shim/src/services.rs::tests::{the_cyclone_header_reads_in_both_byte_orders,
user_data_keys_read_like_rmw_cyclonedds, a_full_table_drops_its_oldest_entry}`,
`crates/dcps/src/runtime.rs::tests::the_related_identity_is_0x0083_or_eprosimas_0x800f_from_a_writer_that_means_it`,
`crates/rtps/src/inline_qos.rs::tests::the_eprosima_pid_is_found_exactly_and_not_as_0x0083`,
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests::service_topics_are_the_rq_rr_names_of_the_other_rmws`.

**Status:** done

### §4.3 Action pattern (5 topics)

**Spec:** §4.3 — actions are composed of 5 topics; the shim wraps them
without special mapping.

**Repo:** `crates/ros2-rmw/src/action.rs` (cluster-C action pattern with 5
topics).

**Tests:** `crates/ros2-rmw/src/action.rs::tests`: `topic_set_has_exactly_5_topics`,
`goal_topic_uses_rq_prefix_and_send_goal_suffix`,
`cancel_topic_uses_rq_prefix_and_cancel_suffix`,
`result_topic_uses_rr_prefix_and_reply_suffix`,
`feedback_and_status_use_rt_prefix`,
`handles_action_name_without_leading_slash`;
`crates/ros2-rmw/tests/cross_vendor.rs::action_topic_set_matches_cyclone_format`.

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

**Tests:** `crates/ros2-rmw/src/msg_to_idl.rs::tests`: `parse_simple_msg_three_fields`,
`parse_skips_comments_and_blank_lines`, `parse_array_spec`,
`parse_constants_are_ignored`, `idl_mapping_primitives`,
`idl_mapping_qualified_type`, `render_idl_produces_module_and_struct`.

**Status:** done

### §5.3 Bridge mode none

**Spec:** §5.3 — `topic_mangling: "none"` disables the prefix for
co-existence with non-ROS DDS apps.

**Repo:** `crates/rmw-zerodds-shim/src/ros2_config.rs` (`Mangling::None`,
`dds_topic`, `dds_service_topics`).

**Tests:** `crates/rmw-zerodds-shim/src/ros2_config.rs::tests::{mangling_none_and_custom_name_the_dds_topic,
service_topics_are_the_rq_rr_names_of_the_other_rmws}`,
`crates/rmw-zerodds-shim/rmw_c/run_ros2_config_test.sh` (5b: Cyclone reads
the topic without prefix from SEDP, Linux test host).

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
`deny_rule_overrides_allow_for_publish`,
`deny_rule_overrides_allow_for_subscribe`,
`deny_only_grant_blocks_specific_topics`, `deny_on_non_matching_topic`.

**Status:** done — permissions XML is enforced per topic via DDS-Security 1.2
access control (ADR 0012).

## §8 Operations + observability

### §8.1 rcutils logging + JSON

**Spec:** §8.1 — `logging.format: "ros"` uses rcutils, `"json"` is
structured.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (logging-format switch),
`crates/ros2-rmw/src/json_log.rs` (cluster-C rcutils + JSON-sink wire-up).

**Tests:** `crates/ros2-rmw/src/json_log.rs::tests`: `level_strings_match_rcutils`,
`render_json_simple`, `render_json_escapes_quotes_and_backslash`,
`render_json_escapes_newline_and_tab`, `render_json_escapes_control_char`.

**Status:** done

### §8.2 Prometheus metrics via a separate exporter

**Spec:** §8.2 — a `zerodds-ros2-metrics-exporter` process with 8
counter/gauge families. Cluster-A wires the metrics endpoint directly into
the `zerodds-ros2-shim` diagnostics binary instead of as a separate
exporter.

**Repo:** —

**Tests:** —

**Status:** open — `zerodds-ros2-shim metrics` prints the name catalogue, but no test
in `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` calls it, and no counter
is incremented on the rmw path.
### §8.3 OTLP rmw_publish/rmw_take spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** —

**Tests:** —

**Status:** open — neither `crates/rmw-zerodds-shim/` nor `crates/ros2-rmw/` names
`OTEL_EXPORTER_OTLP_ENDPOINT` or emits a span.
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

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs::topic_mangling_matches_rmw_dds_common_convention`,
`crates/ros2-rmw/tests/cross_vendor.rs::service_topic_pair_matches_cyclone_format`,
`crates/ros2-rmw/tests/cross_vendor.rs::action_topic_set_matches_cyclone_format` (the live cases in it are
`#[ignore]` and bound to a ROS 2 installation);
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::graph_shows_participant_and_discovery_mode`,
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::doctor_multicast_free_with_peers_ok`.

**Status:** done

## §11 Packaging

### §11 librmw_zerodds_cpp.so + diagnostics binary

**Spec:** §11 — `librmw_zerodds_cpp.so` (library, not a daemon); a .deb per
ROS distro; `zerodds-ros2-shim` diagnostics binary; configs/Docker; manuals.

**Repo:** `packaging/linux/deb/build-rmw-deb.sh` (`ros-<distro>-rmw-zerodds-cpp`),
`ci/full.yml` (`ros-rmw-deb`),
`packaging/linux/deb/assemble-debs.sh` (`zerodds-ros2`),
`packaging/linux/rpm/zerodds.spec` (`%files ros2`),
`packaging/macos/homebrew/zerodds-ros2.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ros2-shim/`,
`packaging/linux/configs/ros2.yaml.example`,
`man/man1/zerodds-ros2-shim.1`, `man/man5/zerodds-ros2.yaml.5`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::the_ros2_diagnostics_cli_is_not_installed_as_a_service`;
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests::the_shipped_example_reads_as_it_says`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys);
`crates/rmw-zerodds-shim/tests/ros2_deb_package.rs::{the_package_is_named_after_the_distro_and_the_distro_is_checked, the_package_carries_the_library_the_index_entries_and_package_xml, the_ros2_image_installs_the_rmw_from_the_package, the_ros2_deb_description_points_at_the_rmw_package, the_endpoint_info_scripts_exit_non_zero_on_a_mismatch}`,
`crates/rmw-zerodds-shim/tests/ros2_deb_ci_job.rs::{the_ci_job_builds_the_package_in_the_ros_image_and_checks_its_file_list, the_endpoint_info_checks_run_after_the_round_trip}`
(package name, install layout, file list out of the ament install, CI job and image — checkable without a ROS 2 installation);
executor: `ci/full.yml::ros-rmw-deb` (weekly) runs as a matrix, once per distro (`humble`, `jazzy`, `kilted`, `rolling`) in that distro's ROS 2 image, checks the .deb's file list, installs it back into the ROS prefix with `dpkg -i`, and then exchanges `std_msgs/String` between two processes with `packaging/linux/deb/rmw-pubsub-check.py`; `packaging/linux/deb/rmw-type-hash-check.py` checks the type hash a peer process reads (against the package itself and both ways against `rmw_fastrtps_cpp`, not on Humble), and on Rolling `packaging/linux/deb/rmw-service-info-check.py` the servers and clients of a service.

**Open:** `librmw_zerodds_cpp.so` is in the deb package
`ros-<distro>-rmw-zerodds-cpp` (`packaging/linux/deb/build-rmw-deb.sh`, built
from the ament project `crates/rmw-zerodds-shim/ament`, jazzy as the start
value) and the image installs it from that package. Not run: the weekly job and
the image build (A18); no rpm package carries the library.

**Status:** partial

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — mangling/qos_profile/node_registry/service_pair/enclave,
≥ 5 tests each.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (inline tests),
`crates/ros2-rmw/`.

**Tests:** `crates/ros2-rmw/src/topic_mangling.rs::tests` (11 tests, among them
`mangle_each_kind_uses_correct_prefix`, `demangle_round_trips_all_kinds`),
`crates/ros2-rmw/src/service.rs::tests` (5),
`crates/ros2-rmw/src/action.rs::tests` (6),
`crates/ros2-rmw/src/msg_to_idl.rs::tests` (7),
`crates/ros2-rmw/src/json_log.rs::tests` (5),
`crates/rmw-zerodds-shim/src/lib.rs::tests` (24, among them `rmw_codes_match_rep_2007`,
`type_hash_sha256_is_deterministic`, `event_driven_wait_roundtrip_inprocess`);
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests` (6).

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

**Tests:** `crates/ros2-rmw/tests/cross_vendor.rs::topic_mangling_matches_rmw_dds_common_convention`,
`crates/ros2-rmw/tests/cross_vendor.rs::service_topic_pair_matches_cyclone_format`,
`crates/ros2-rmw/tests/cross_vendor.rs::action_topic_set_matches_cyclone_format`; the live case
`ros2_topic_list_includes_chatter` is `#[ignore]` and runs only with ROS 2
installed.

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

18 done / 0 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p rmw-zerodds-shim` — tests green, 0 failed.

Open items: see `zerodds-ros2-bridge-1.0.open.en.md`. Decision records: none.
