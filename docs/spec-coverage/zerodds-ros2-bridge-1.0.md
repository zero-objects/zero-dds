# `zerodds-ros2-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ros2-bridge-1.0.md`

Implementation:

- `crates/rmw-zerodds-shim/` — ROS-2-RMW-Shim (rmw_zerodds).

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §2 bis §12.

## §2 CLI-Surface

### §2 zerodds-ros2-shim Subcommands

**Spec:** §2 — Subcommands `info`/`topics`/`qos`/`enclaves`/`validate`/
`selftest`; Optionen `--config`/`--set`/`--check-config`/`--domain`/
`--version`/`--help`; abgelehnt mit Verweis: `--runtime-config`/`--log-level`/`--enclave`; Exit-Codes 0/1/2/3/4.

**Repo:** `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`,
`crates/rmw-zerodds-shim/src/lib.rs`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`,
`::topic_mangle_emits_rt_prefix`,
`::qos_sensor_data_is_best_effort`,
`::qos_unknown_profile_exits_nonzero`,
`::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`,
`::info_includes_rmw_compat_marker`,
`::flags_of_the_old_shim_file_are_refused_with_their_replacement`.

**Status:** done

## §3 Config-File-Format

### §3 `ros2`-Sektion der Runtime-Konfiguration

**Spec:** §3 — `ros2.topic_mangling`/`custom_mangling.topic_prefix`,
`qos_profiles` (REP-2009-Tabelle, feldweise ersetzbar), `topic_overrides`
(`qos_profile` oder `qos`-Map), `time_based_filter`; Schlüssel, die rcl
trägt, abgelehnt; ENV-Substitution.

**Repo:** `crates/rmw-zerodds-shim/src/ros2_config.rs` (`Ros2Config`),
`crates/rmw-zerodds-shim/src/lib.rs` (`rmw_zerodds_init`,
`rmw_zerodds_create_publisher`/`_subscription`: `effective_qos`,
`dds_topic`), `crates/dcps/src/runtime.rs` (`is_rmw_path`),
`packaging/linux/configs/ros2.yaml.example`, `man/man5/zerodds-ros2.yaml.5`.

**Tests:** `crates/rmw-zerodds-shim/src/ros2_config.rs::tests` (6),
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::topic_shows_the_configured_mangling_and_override`,
`::check_config_refuses_what_rcl_owns`,
`crates/dcps/tests/runtime_config_layering.rs::ros2bridge_3_the_rmw_section_is_not_an_unknown_key_for_other_programs`,
`tools/traceability/tests/bridge_config_manpages.rs`.

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

**Spec:** §4.2 — `rq/<service>Request`/`rr/<service>Reply` mit den Typen
`<pkg>::srv::dds_::<Srv>_Request_`/`_Response_`; Korrelation in der Form
des Gegenübers (Cyclone: `guid`/`seq` vor dem Body; Fast DDS:
`related_sample_identity`, PID 0x800f); Graph (`ros2 service list/type`,
`ros2 node info`).

**Repo:** `crates/rmw-zerodds-shim/src/services.rs`,
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c` (Service-Aufrufe, Graph),
`crates/dcps/src/runtime.rs` (`write_user_sample_related`,
`UserSample::Alive.related_sample_identity`, `participant_vendor`),
`crates/rtps/src/inline_qos.rs` (`PID_EPROSIMA_RELATED_SAMPLE_IDENTITY`).

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh`
(Linux-Testhost, Humble, demo_nodes_py AddTwoInts 2 + 3: Server/Client
zerodds↔zerodds, cyclonedds→zerodds, zerodds→cyclonedds,
fastrtps→zerodds, zerodds→fastrtps je `Result of add_two_ints: 5`),
`crates/rmw-zerodds-shim/src/lib.rs::tests::{ros2bridge_4_2_a_service_call_between_two_runtimes_correlates_by_the_related_identity,
ros2bridge_4_2_two_clients_in_one_runtime_each_get_their_own_reply}`,
`crates/rmw-zerodds-shim/src/services.rs::tests::{the_cyclone_header_reads_in_both_byte_orders,
user_data_keys_read_like_rmw_cyclonedds, a_full_table_drops_its_oldest_entry}`,
`crates/dcps/src/runtime.rs::tests::rpc_7_8_2_the_related_identity_is_0x0083_or_eprosimas_0x800f_from_a_writer_that_means_it`,
`crates/rtps/src/inline_qos.rs::tests::the_eprosima_pid_is_found_exactly_and_not_as_0x0083`,
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests::service_topics_are_the_rq_rr_names_of_the_other_rmws`.

**Status:** done

### §4.3 Action-Pattern (5 Topics)

**Spec:** §4.3 — Actions sind composit aus 5 Topics; Shim wraps ohne
Sondermapping.

**Repo:** `crates/ros2-rmw/src/action.rs` (Cluster-C Action-Pattern
mit 5 Topics).

**Tests:** `crates/ros2-rmw/src/action.rs::tests`: `ros2bridge_4_3_an_action_names_its_five_endpoints_under_action`.

**Status:** done

## §5 Topic-Mapping

### §5.1 REP-2007 Topic-Mangling rt/rq/rr-Prefix

**Spec:** §5.1 — `/chatter` → `rt/chatter`; Service-Req → `rq/...Request`,
Reply → `rr/...Reply`.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (mangling helper),
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::topic_mangle_emits_rt_prefix`,
`crates/ros2-rmw/src/topic_mangling.rs::tests::ros_names_prefix_each_kind_takes_its_prefix`,
`crates/ros2-rmw/src/service.rs::tests::ros2bridge_4_2_a_service_is_the_topics_rq_request_and_rr_reply`.

**Status:** done

### §5.2 REP-2008 Type-Mapping .msg/.srv → IDL

**Spec:** §5.2 — `geometry_msgs/Pose` → `IDL:geometry_msgs/msg/dds_/
Pose_:1.0`.

**Repo:** `crates/ros2-rmw/src/msg_to_idl.rs` (REP-2008 Type-Mapping
Cluster-C),
`crates/ros2-rmw/src/type_mapping.rs`.

**Tests:** `crates/ros2-rmw/src/msg_to_idl.rs::tests`: `ros2_msg_fields_are_type_and_name_without_comments_and_constants`, `ros2_msg_arrays_are_static_or_unbounded`, `ros2_msg_field_types_take_the_dds_type_of_the_table`,
`ros2bridge_5_2_a_message_renders_as_a_struct_in_its_package_module`.

**Status:** done

### §5.3 Bridge-Mode none

**Spec:** §5.3 — `topic_mangling: "none"` deaktiviert Prefix für Co-
Existence mit Non-ROS-DDS-Apps.

**Repo:** `crates/rmw-zerodds-shim/src/ros2_config.rs` (`Mangling::None`,
`dds_topic`, `dds_service_topics`).

**Tests:** `crates/rmw-zerodds-shim/src/ros2_config.rs::tests::{mangling_none_and_custom_name_the_dds_topic,
service_topics_are_the_rq_rr_names_of_the_other_rmws}`,
`crates/rmw-zerodds-shim/rmw_c/run_ros2_config_test.sh` (5b: Cyclone liest
den Topic ohne Präfix aus SEDP, Linux-Testhost).

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
`dds.sec.*`-Plugin-Properties.

**Repo:** `crates/security-runtime/src/profile.rs` —
`SecurityProfile::from_enclave_dir` mappt das sros2-keystore-Format
(identity_ca/cert/key/permissions_ca/`governance.p7s`/`permissions.p7s`)
auf einen DDS-Security-`SecurityProfile`; `from_env` lädt via
`ZERODDS_SECURITY_DIR` + `ROS_DOMAIN_ID` (C7 secure-by-default).
`crates/rmw-zerodds-shim/src/lib.rs` verdrahtet es (gesetzt →
Security-Participant, gesetzt-aber-invalid → harter Fehler);
`crates/security-permissions/` deckt Governance/Permissions (DDS-Security
1.2 §9.4).

**Tests:** `security-runtime` `ros2bridge_7_1_enclave_dir_resolves_all_sros2_filenames`,
`ros2bridge_7_1_enclave_dir_missing_cert_is_io_naming_cert`; `rmw-zerodds-shim`
`shim_cli_e2e` (`ZERODDS_SECURITY_DIR`-Pfad).

**Status:** done — SROS2-Enclave wird als Mapping-Layer auf DDS-Security 1.2
geladen (ADR 0012).

### §7.2 ACL via Permissions-XML

**Spec:** §7.2 — Permissions-XML-Driven allow/deny pro Topic via
`dds.sec.access`.

**Repo:** `crates/security-permissions/` — CMS-verifiziertes Permissions-XML
(DDS-Security 1.2 §9.4) über das `dds.sec.access`-Plugin
(`is_publish_allowed`/`is_subscribe_allowed`, `check_create_datawriter`/
`check_create_datareader`); das Enclave-Permissions-Material
(`permissions.p7s`/`governance.p7s`) kommt aus `from_enclave_dir` (§7.1).

**Tests:** `crates/security-permissions/src/xml.rs::tests::security_10_4_1_5_3_4_default_deny_without_explicit_tag`, `crates/security-permissions/src/plugin.rs::tests::{ros2bridge_7_2_deny_on_non_matching_topic, ros2bridge_7_2_deny_writer_when_only_subscribe_granted, ros2bridge_7_2_permit_reader_on_subscribe_allowed_topic, security_10_4_1_5_3_4_allow_rule_and_deny_rule_decide_the_same_xml_differently, security_10_4_1_5_3_4_2_deny_rule_also_binds_the_reader_side}` (Erlaubnis und Verbot pro Topic, Schreiber- und Leserseite).

**Status:** done — Permissions-XML wird via DDS-Security-1.2-Access-Control
pro Topic durchgesetzt (ADR 0012).

## §8 Operations + Observability

### §8.1 rcutils-Logging + JSON

**Spec:** §8.1 — `logging.format: "ros"` nutzt rcutils, `"json"`
strukturiert.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (Logging-Format-Switch),
`crates/ros2-rmw/src/json_log.rs` (Cluster-C rcutils + JSON-Sink-
Wireup).

**Tests:** `crates/ros2-rmw/src/json_log.rs::tests`: `rcutils_logging_the_five_severities_carry_rcutils_names`,
`ros2bridge_8_1_a_json_line_is_one_object_with_ts_level_logger_and_msg`, `rfc8259_7_quotes_backslashes_and_control_characters_are_escaped`.

**Status:** done

### §8.2 Prometheus-Metrics via separater Exporter

**Spec:** §8.2 — `zerodds-ros2-metrics-exporter`-Prozess mit 8
Counter/Gauge-Familien. Cluster-A wired den Metrics-Endpoint direkt im
`zerodds-ros2-shim`-Diagnose-Binary statt als separaten Exporter.

**Repo:** —

**Tests:** —

**Status:** open — `zerodds-ros2-shim metrics` druckt den Namens-Katalog, aber kein
Test in `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` ruft ihn auf, und
kein Zähler wird im rmw-Pfad hochgezählt.
### §8.3 OTLP rmw_publish/rmw_take Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** —

**Tests:** —

**Status:** open — weder `crates/rmw-zerodds-shim/` noch `crates/ros2-rmw/` nennt
`OTEL_EXPORTER_OTLP_ENDPOINT` oder emittiert einen Span.
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

**Spec:** §10 — ZeroDDS-RMW-Shim ist nativer DDS-Peer; Cross-Vendor mit rmw_fastrtps_cpp, rmw_cyclonedds_cpp und rmw_connextdds; ROS-Distros Humble, Iron, Jazzy, Rolling mit je eigenem Build-Ziel.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs`,
`crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_qos_translation_test.sh` (Linux-Testhost, Humble, mit `RMW_PUB`/`RMW_SUB`: Topics in beide Richtungen mit rmw_cyclonedds_cpp und rmw_fastrtps_cpp), `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh` (Services in beide Richtungen mit denselben Partnern); `crates/ros2-rmw/src/topic_mangling.rs::tests::ros_names_prefix_a_topic_loses_its_leading_slash_and_takes_rt`, `crates/ros2-rmw/src/service.rs::tests::ros2bridge_4_2_a_service_is_the_topics_rq_request_and_rr_reply`, `crates/ros2-rmw/src/action.rs::tests::ros2bridge_4_3_an_action_names_its_five_endpoints_under_action`; `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::{graph_shows_participant_and_discovery_mode, doctor_multicast_free_with_peers_ok}`.

**Status:** partial — Fast DDS und Cyclone DDS sind gemessen; rmw_connextdds und das Build-Ziel Iron fehlen (eigene Items).

### §10.1 Cross-Vendor mit rmw_connextdds

**Spec:** §10 — Cross-Vendor mit `rmw_connextdds` (RTI).

**Repo:** —

**Tests:** —

**Status:** open — nicht gemessen; die Spec selbst nennt es „not measured (not installed in the test env)“. Es fehlt ein Lauf von `run_qos_translation_test.sh` und `run_ros2_services_test.sh` gegen rmw_connextdds.

### §10.2 Build-Ziel ROS-Distro Iron

**Spec:** §10 — eigenes Build-Ziel pro Distro: Humble, Iron, Jazzy, Rolling; §11 — Paket `ros-iron-rmw-zerodds-cpp` und Image `zerodds/ros2-iron:1.0`.

**Repo:** —

**Tests:** —

**Status:** open — `packaging/linux/deb/build-rmw-deb.sh` kennt `humble jazzy kilted rolling` und lehnt jeden anderen Namen ab; `ci/release-candidate.yml::ros-rmw-deb` baut dieselben vier. `packaging/docker/ros2-shim/Dockerfile` nennt die Variante `iron`, deren Paketbau das Skript ablehnt (nicht gebaut, aus dem Skript gelesen).

## §11 Packaging

### §11 librmw_zerodds_cpp.so + Diagnose-Binary

**Spec:** §11 — `librmw_zerodds_cpp.so` (Library, kein Daemon); .deb
pro ROS-Distro; `zerodds-ros2-shim` Diagnose-Binary; Configs/Docker;
Manuals.

**Repo:** `packaging/linux/deb/build-rmw-deb.sh` (`ros-<distro>-rmw-zerodds-cpp`),
`ci/release-candidate.yml` (`ros-rmw-deb`),
`packaging/linux/deb/assemble-debs.sh` (`zerodds-ros2`),
`packaging/linux/rpm/zerodds.spec` (`%files ros2`),
`packaging/macos/homebrew/zerodds-ros2.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ros2-shim/`,
`packaging/linux/configs/ros2.yaml.example`,
`man/man1/zerodds-ros2-shim.1`,
`man/man5/zerodds-ros2.yaml.5`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::the_ros2_diagnostics_cli_is_not_installed_as_a_service`;
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests::the_shipped_example_reads_as_it_says`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel);
das Paket selbst prüft der Release-Kandidaten-Job `ros-rmw-deb` (`ci/release-candidate.yml`): eine Matrix einmal pro Distro (`humble`, `jazzy`, `kilted`, `rolling`) im jeweiligen ROS-2-Image, die Dateiliste des .deb, `dpkg -i` zurück in den ROS-Präfix, dann `std_msgs/String` zwischen zwei Prozessen mit `packaging/linux/deb/rmw-pubsub-check.py`; `packaging/linux/deb/rmw-type-hash-check.py` prüft den Typ-Hash, den ein zweiter Prozess liest (gegen das Paket selbst und in beiden Richtungen gegen rmw_fastrtps_cpp, nicht auf Humble), und auf Rolling `packaging/linux/deb/rmw-service-info-check.py` Server und Clients eines Service; auf `main` grün in Pipeline 8827.

**Status:** partial — `librmw_zerodds_cpp.so` liegt im deb-Paket `ros-<distro>-rmw-zerodds-cpp` (gebaut aus `crates/rmw-zerodds-shim/ament`), das Image installiert es aus diesem Paket; Diagnose-Binary, Konfiguration und Handbuchseiten sind ausgeliefert. Der Image-Bau ist nicht gelaufen. Es fehlen die Veröffentlichung auf dem `packages.ros.org`-Spiegel (§11.1) und Paket und Image für Iron (§10.2).

### §11.1 Veröffentlichung auf dem packages.ros.org-Spiegel

**Spec:** §11 — ein .deb pro ROS-Distro, `ros-humble-rmw-zerodds-cpp`, `ros-iron-rmw-zerodds-cpp` usw., auf dem `packages.ros.org`-Spiegel.

**Repo:** —

**Tests:** —

**Status:** open — `packaging/linux/deb/build-rmw-deb.sh` baut das Paket; kein Job und kein Skript lädt es auf `packages.ros.org` hoch.

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — mangling/qos_profile/node_registry/service_pair/
enclave je ≥ 5 Tests.

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs` (inline tests),
`crates/ros2-rmw/`.

**Tests:** `crates/ros2-rmw/src/topic_mangling.rs::tests` (6 Tests, darunter
`ros_names_prefix_each_kind_takes_its_prefix`, `ros_names_prefix_the_four_prefixes_read_back_as_their_kind`),
`crates/ros2-rmw/src/service.rs::tests` (2),
`crates/ros2-rmw/src/action.rs::tests` (1),
`crates/ros2-rmw/src/msg_to_idl.rs::tests` (4),
`crates/ros2-rmw/src/json_log.rs::tests` (3),
`crates/rmw-zerodds-shim/src/lib.rs::tests` (24, darunter `rmw_ret_types_h_the_shim_returns_the_header_codes`,
`fips180_4_b_1_the_type_name_hash_is_sha_256`, `rmw_h_rmw_wait_wakes_on_arrival_and_times_out_when_empty`);
`crates/rmw-zerodds-shim/src/ros2_config.rs::tests` (6).

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

### §12.3 Multi-Vendor

**Spec:** §12.3 — Topics über `run_qos_translation_test.sh` mit `RMW_PUB`/`RMW_SUB` gegen rmw_cyclonedds_cpp und rmw_fastrtps_cpp in beide Richtungen; Services über `run_ros2_services_test.sh`; Security über `run_ros2_security_test.sh`; die Namenskonventionen von rmw_cyclonedds_cpp und rmw_fastrtps_cpp prüfen Unit-Tests von `crates/ros2-rmw` ohne ROS.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/`, `crates/ros2-rmw/`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_qos_translation_test.sh` (`RMW_PUB`/`RMW_SUB`), `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh`, `crates/rmw-zerodds-shim/rmw_c/run_ros2_security_test.sh` (Linux-Testhost, von Hand; in keinem CI-Job); `crates/ros2-rmw/src/topic_mangling.rs::tests::ros_names_prefix_a_topic_loses_its_leading_slash_and_takes_rt`, `crates/ros2-rmw/src/service.rs::tests::ros2bridge_4_2_a_service_is_the_topics_rq_request_and_rr_reply`, `crates/ros2-rmw/src/action.rs::tests::ros2bridge_4_3_an_action_names_its_five_endpoints_under_action`.

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + REPs + DDS-Security

**Spec:** §13 — Library `crates/ros2-rmw/`/`crates/rmw-zerodds-shim/`,
REP-2007/2008/2009 + SROS2, Wire-Format, Deployment, DDS-Security 1.2.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive QoS-Profile / ROS-Distro,
Major=RMW-API-Breaking.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

17 done / 2 partial / 5 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-ros2-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-dcps --all-targets` (Job `merge-rust-dcps`) — 2 Tests grün.
* `cargo nextest run --profile merge -p rmw-zerodds-shim --all-targets` (Job `merge-rust-rmw-zerodds-shim`) — 23 Tests grün.
* `cargo nextest run --profile merge -p zerodds-ros2-rmw --all-targets` (Job `merge-rust-ros2-rmw`) — 12 Tests grün.
* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (Job `merge-rust-rtps`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-security-permissions --all-targets` (Job `merge-rust-security-permissions`) — 6 Tests grün.
* `cargo nextest run --profile merge -p zerodds-security-runtime --all-targets` (Job `merge-rust-security-runtime`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 5 Tests grün.

51 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-ros2-bridge-1.0.open.md`.
