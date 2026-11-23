# `zerodds-corba-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-bridge-1.0.md`

Implementation:

- `crates/corba-dds-bridge/` — CORBA↔DDS-Bridge (DDS4CCM-Connector).

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §2 bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6 — mehrere DomainParticipants je Daemon; eine
Mandantenbindung je IIOP-Verbindung.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs` (`TenantConfig`, `DaemonConfig::of_tenant`), `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{Tenants, accept_loop}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ior_lines`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate` (eine SSLIOP-Sitzung mit dem Zertifikat des Mandanten erreicht sein Objekt, einfaches IIOP und eine Sitzung ohne das Zertifikat bekommen `OBJECT_NOT_EXIST`), `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_1_a_tenants_ior_line_names_the_tenant`.

**Status:** done
## §2 CLI-Surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — `--config`/`--set`/`--check-config`, die Kurzformen
`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/
`--tls-cert`+`--tls-key`/`--topic`/`--log-level`/`--metrics` als `--set`
ihres Datei-Schlüssels; Exit-Codes 0–5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`, `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`, `man/man1/zerodds-corba-bridged.1`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_version_flag_emits_one_line, corbabridge_2_unknown_arg_yields_exit_1, corbabridge_2_dump_iors_writes_stringified_ior, corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_a_port_in_use_is_exit_2, corbabridge_2_a_participant_that_does_not_start_is_exit_3, corbabridge_2_a_name_service_that_does_not_answer_is_exit_5}`.

**Status:** done
## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`corba`/`mappings`/`acl`/
`metrics`; `zerodds-config`-Teilmenge mit `include:`, Drop-ins und
`${VAR}`; unbekannte Schlüssel und falsche Typen verhindern den Start;
nicht Gebautes wird abgelehnt.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs` (`DaemonConfig::from_config`, `read_mapping`, `is_key`), `crates/config/`, `packaging/linux/configs/corba-bridged.yaml.example`, `man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::{tests::corbabridge_3_the_example_of_the_spec_loads, tests::corbabridge_3_the_shipped_example_reads_as_it_says, tests::corbabridge_3_what_the_daemon_does_not_build_is_refused, tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read}`, `crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done
## §4 GIOP/IIOP-Wire-Protocol

### §4.1 IOR-Generation IIOP/SSLIOP/Components

**Spec:** §4.1 — ein IOR pro Mapping mit `type_id` = `repo_id`,
IIOP-Profil (host/port/object_key) und Components; SSL-Component bei
SSLIOP; `ior_file`; NameService-Bind.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/ior.rs` (`build_ior`, `ior_for_key`, `profile_components`, `ssl_options`, `csiv2_mech_list`, `tls_sec_trans`, `ior_lines`), `crates/corba-dds-bridge/src/naming_client.rs`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::{tests::corbabridge_4_1_the_ior_carries_code_sets_and_the_ssl_component, tests::the_ior_carries_the_csiv2_mech_list_when_enabled, tests::corbabridge_4_1_an_iiop_1_0_profile_has_no_components, tests::the_advertise_host_is_a_setting, tests::corbabridge_4_1_a_tenants_ior_line_names_the_tenant}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_dump_iors_writes_stringified_ior`, `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/naming_service_e2e.rs::{the_bridge_rebinds_into_a_real_naming_svc_and_unbinds_on_stop, the_bridge_rebinds_over_ssliop, the_bridge_rebinds_through_a_rir_initial_reference}`, `crates/corba-ior/src/components.rs::tests::a_code_sets_encapsulation_pads_after_the_byte_order_octet`.

**Status:** done — `TAG_ORB_TYPE` steht im Item §4.1a.
### §4.1a TAG_ORB_TYPE

**Spec:** §4.1 — die Komponente `TAG_ORB_TYPE` im IIOP-Profil der IOR.

**Repo:** —

**Tests:** —

**Status:** blocked — Zuweisung eines ORB-Typs durch die OMG (beantragt;
`TAG_ORB_TYPE` nach CORBA 3.3 Part 2 §7.6.6.1 trägt einen von der OMG
vergebenen Wert); hebt sich auf, sobald die OMG den Wert zuweist. Bis dahin
trägt die IOR der Bridge die Komponente nicht; sie ist optional, ein Client
wählt ohne sie keine herstellerspezifischen Abweichungen.

### §4.2 GIOP-Frame-Format Header + Body

**Spec:** §4.2 — 12-Byte-Header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR-Body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{serve_connection, message_error}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs`, Codec in `crates/corba-giop/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_4_2_a_message_that_is_not_giop_gets_message_error`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`, `crates/corba-giop/src/header.rs::{magic_bytes_match_spec, header_size_is_12_bytes, round_trip_big_endian_request_header}`, `crates/corba-giop/src/message_type.rs::message_type_values_match_spec_table_15_1`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §4.3 Request-Reply-Mapping CORBA→DDS

**Spec:** §4.3 — GIOP-Request → Mapping-Lookup → DDS-Request-Sample
(`u64 request_id` + Argumente, XCDR1 in GIOP-Byte-Reihenfolge) →
korrelierter Reply-Sample (`request_id`, `reply_status`, `reserved`,
Body) → GIOP-Reply; `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`,
`NO_PERMISSION`, `NO_IMPLEMENT` (GIOP < 1.2), `TRANSIENT`, `MARSHAL`
(auch für ein Reply-Sample, das kein einfaches CDR ist); `CancelRequest`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{on_request, refuse}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{on_reply, timeout_loop, cancel}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::system_exception_body`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{tests::corbabridge_4_3_a_mapped_operation_over_giop_1_1_is_no_implement, tests::corbabridge_4_3_a_failed_dds_write_is_transient, tests::corbabridge_4_3_the_target_and_the_operation_decide}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{tests::corbabridge_4_3_a_reply_sample_answers_its_request, tests::corbabridge_4_3_the_reply_status_and_the_representation_decide, tests::corbabridge_4_3_a_reply_without_its_request_is_dropped, tests::corbabridge_4_3_cancel_request_removes_the_request_of_its_connection}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::tests::corbabridge_4_3_a_system_exception_body_is_the_giop_layout`, `crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_3_a_reply_sample_that_is_not_plain_cdr_is_marshal`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::tests::corbabridge_4_3_the_correlation_member_has_the_name_of_correlation_id`.

**Status:** done — `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`, `NO_PERMISSION`, `NO_IMPLEMENT`, `TRANSIENT`, `MARSHAL` und `CancelRequest` sind belegt. Schritt 5: das Korrelationsfeld ist das erste Member; `correlation_id` benennt es im erzeugten Typ (§5.2).
### §4.4 Notify-Mapping DDS→CORBA

**Spec:** §4.4 — DDS-Sample auf `notify_topic` → oneway GIOP-1.2-Request
an jedes Ziel in `targets` (`corbaloc:` oder IOR); ein Sample, das kein
einfaches CDR ist (XCDR2, `PL_CDR`), verworfen und gezählt; Neuaufbau nach
Fehler.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::{pump_notify, on_notify_sample, notifiers_of}`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs::{Notifier, closed}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::resolve_target`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::{tests::corbabridge_4_4_a_sample_is_a_oneway_request_with_the_sample_as_body, tests::corbabridge_4_4_a_sample_that_is_not_plain_cdr_is_refused, tests::corbabridge_4_4_every_target_gets_the_sample, tests::corbabridge_4_4_an_unreachable_target_does_not_stop_the_others}`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs::{tests::corbabridge_4_4_a_closed_connection_is_opened_again, tests::resolve_follows_object_forward_then_confirms_object_here, tests::send_follows_location_forward_on_the_reply_to_the_oneway_itself, tests::send_reports_an_unexpected_reply_status_instead_of_treating_it_as_delivered}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_4_corbaloc_and_ior_targets_resolve`, `crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_4_a_notify_sample_that_is_not_plain_cdr_is_not_sent`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §4.5 Object-Key-Generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::{object_key, canonical_topics}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::check_mappings`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::{tests::corbabridge_4_5_the_key_of_a_request_reply_mapping_is_the_sha_256_prefix, tests::corbabridge_4_5_the_key_of_a_notify_mapping_is_of_the_notify_topic, tests::corbabridge_4_5_a_key_is_16_octets, tests::corbabridge_4_5_a_bidir_mapping_takes_the_request_reply_form, tests::corbabridge_4_5_repo_id_and_topics_name_the_key, tests::corbabridge_4_5_two_mappings_with_one_key_are_refused}` (die erwarteten Schlüssel sind die ersten 16 Oktette von `shasum -a 256` derselben Oktette), `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`.

**Status:** done
### §4.6 Fragment-Handling GIOP 1.1+

**Spec:** §4.6 — Nachrichten über `fragment_size` als Fragmente, jedes
außer dem letzten auf einer 8-Oktett-Grenze der Nachricht endend;
Reassembly inbound.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::{write_giop, fragments, Reassembly}`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::{tests::corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries, tests::corbabridge_4_6_a_giop_1_1_reply_is_fragmented_without_request_ids, tests::corbabridge_4_6_a_message_within_the_fragment_size_goes_out_whole, tests::corbabridge_4_6_a_fragment_out_of_place_is_refused, tests::corbabridge_4_6_a_fragmented_message_is_put_back_together}`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant` (`echo` über 200 000 Byte, `fragment_size` 16384, in beide Richtungen fragmentiert, gegen omniORB 4.3.3).

**Status:** done
### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` mit `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{locate_status, knows, forward_ior, forward_request, serve_connection}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ior_for_key`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{tests::corbabridge_4_7_locate_names_known_and_unknown_objects, tests::corbabridge_4_7_the_forward_ior_of_ssliop_only_names_the_ssl_port}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_7_ssliop_only_forwards_a_plain_iiop_client` (`OBJECT_FORWARD` und `LOCATION_FORWARD` mit Port 0 und `TAG_SSL_SEC_TRANS`, über SSLIOP antwortet das Objekt), `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`.

**Status:** done
## §5 Topic-Mapping

### §5.1 DDS-Topic-Slug per Operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` als Default; Override per `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::slug`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_5_1_the_slug_of_an_operation`.

**Status:** done
### §5.2 Type-Discovery aus der IDL des Interfaces

**Spec:** §5.2 — mit `idl_file` liest der Daemon die IDL des Interfaces
und erzeugt pro gemappter Operation `Request`, `Reply` (Union über den
`ReplyStatusType`, Benutzerausnahmen als Variante über `exception_kind`)
und `Notify`; die IDL steht in `corba.idl_dir`. Eine Benutzerausnahme im
Sample wird in die GIOP-Form umgeschrieben. `wchar`, `wstring`, `fixed`,
`any`, Objektreferenzen, Valuetypes, `native` und `@mutable` werden
abgelehnt. Ohne `idl_file`: das feste Layout aus §4.3.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::{generate, units, apply, files, user_exception_body}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::read_mapping`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::on_reply`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::{tests::corbabridge_5_2_request_and_reply_are_generated_from_the_operation, tests::corbabridge_5_2_the_exceptions_are_the_variant_exception_kind, tests::corbabridge_5_2_out_and_inout_arguments_and_the_used_types, tests::corbabridge_5_2_oneway_attributes_and_notify, tests::corbabridge_5_2_what_has_no_dds_form_is_refused, tests::corbabridge_5_2_a_user_exception_becomes_its_giop_body}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_5_2_idl_file_gives_the_mapping_its_types`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_5_2_a_user_exception_sample_is_the_giop_exception`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_5_2_the_daemon_writes_the_generated_idl_into_idl_dir`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_5_2_cyclone_dds_builds_the_generated_types_and_raises_a_user_exception`.

**Status:** done — gemessen gegen omniORB 4.3 und Cyclone DDS 0.10.5: idlc übersetzt die erzeugte IDL, ein Cyclone-DDS-Programm auf diesen Typen antwortet, omniORB erhält das Ergebnis mit `out`-Argument, die Ausnahme mit Membern und die ohne.

## §6 QoS-Translation

### §6 DDS-QoS → CORBA-Behavior Map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/
Partition; `BEST_EFFORT` nur für Notify.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::read_mapping` (`qos.*`), `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{endpoint_qos, build_dispatch}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{ReplayStore, deadline_missed, check_notify_deadlines}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::on_request`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::tests::corbabridge_6_the_qos_of_a_mapping_are_its_endpoint_policies`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{tests::corbabridge_6_the_replay_store_keeps_the_last_replies_for_their_lifespan, tests::corbabridge_6_a_missed_deadline_answers_with_transient}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_6_a_repeated_request_is_answered_from_the_replay_store`, `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::tests::corbabridge_6_a_notify_sample_restarts_the_deadline`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::{tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read, tests::corbabridge_3_what_the_daemon_does_not_build_is_refused}`.

**Status:** done
## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL-Component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` aktiviert TLS auf
`ssliop.bind`, `require_client_cert` gegen `ca_file`; SSL-Component im
IOR.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{ssliop_server_config, accept_loop, TlsShared}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ssl_options`, `crates/bridge-security/src/tls.rs::load_server_config_with_optional_client_auth`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs::{corbabridge_7_1_build_corba_tls_config_loads_self_signed, corbabridge_7_1_build_corba_tls_config_with_mtls_succeeds, corbabridge_7_1_build_corba_tls_config_rejects_missing_cert}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate, corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}` (mit `ca_file` ohne `require_client_cert` kommt eine Sitzung ohne Zertifikat zustande, eine mit Zertifikat trägt dessen Subject), `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only` (omniORB 4.3.3 mit `libomnisslTP4`, Linux-Testhost).

**Status:** done
### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP-User/Pass-Fallback.

**Repo:** `crates/corba-dds-bridge/src/csiv2_wire.rs`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::on_request`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::{csiv2_mech_list, tls_sec_trans}`, `crates/corba-dds-bridge/src/bridge_daemon/metrics.rs::csiv2_reason`, `crates/corba-csiv2/`.

**Tests:** `crates/corba-dds-bridge/tests/csiv2_e2e.rs::{csiv2_wrong_password_is_no_permission_with_a_context_error, corbabridge_7_2_missing_credentials_are_refused_by_csiv2, csiv2_accepted_credentials_proceed_past_acl_to_a_dds_timeout}`, `crates/corba-dds-bridge/src/csiv2_wire.rs::tests` (6), `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_2_without_gssup_or_certificate_csiv2_refuses`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::the_ior_carries_the_csiv2_mech_list_when_enabled`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_gssup_credentials_pass_csiv2_and_the_acl`.

**Status:** done
### §7.3 ACL pro Mapping

**Spec:** §7.3 — Subject aus dem Subject-DN des Client-Zertifikats;
Regel mit passendem Subject führt die `repo_id` in `allow_invoke`
(Präfix mit `*`); sonst `default_deny`; verweigert: `NO_PERMISSION`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{acl_allows, on_request}`, `crates/bridge-security/src/ctx.rs::extract_mtls_subject`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`, `crates/bridge-security/src/ctx.rs::tests::corbabridge_7_3_the_mtls_subject_is_the_dn_and_the_acl_also_takes_the_fingerprint`, `crates/corba-dds-bridge/tests/csiv2_e2e.rs::csiv2_accepted_credentials_proceed_past_acl_to_a_dds_timeout` (`gssup:alice` als Subject).

**Status:** done
## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log mit `timestamp`, `level`, `event` und den
Feldern des Ereignisses; `--log-level`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/log.rs` (`line`, `emit`, `rfc3339`, `escape`), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::finish`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/log.rs::{tests::corbabridge_8_1_a_line_is_a_json_object_with_the_spec_fields, tests::rfc3339_5_6_the_utc_date_and_time, tests::rfc8259_7_text_is_escaped, tests::corbabridge_8_1_the_log_level_is_the_threshold}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange`.

**Status:** done
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — zehn Familien `zerodds_corba_bridge_*`
(requests/replies/pending/latency/timeouts/…).

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/metrics.rs` (`SpecMetrics`, `CSIV2_REASONS`), `crates/corba-dds-bridge/src/daemon_runtime.rs` (`BridgeMetrics`, `serve_admin_endpoints`), `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` (`metrics_addr_for`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::{corbabridge_8_2_metrics_register_idempotent, corbabridge_8_2_admin_endpoint_serves_metrics_path}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_2_without_gssup_or_certificate_csiv2_refuses` (`csiv2_failures_total{reason="missing"}`).

**Status:** done
### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` → Spans pro
GIOP-Exchange (`giop.request`, `dds.publish`, `dds.subscribe`,
`giop.reply`).

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/trace.rs` (`Tracer`, `Exchange`, `spans`), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::finish`, `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/trace.rs::{tests::corbabridge_8_3_an_exchange_is_a_request_span_with_three_children, tests::corbabridge_8_3_a_refused_request_is_the_root_alone, tests::corbabridge_8_3_the_dds_children_carry_the_topic, tests::corbabridge_8_3_a_tracer_without_exporter_is_off, tests::corbabridge_8_3_span_ids_differ}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange`, `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::corbabridge_8_3_otlp_config_parses_endpoint`.

**Status:** done
## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer pro Mapping →
IIOP/SSLIOP-Bind → NameService-Rebind → Signal-Handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`, `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_1_the_port_is_bound_before_an_ior_is_published, corbabridge_2_a_participant_that_does_not_start_is_exit_3, corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist}`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Drain max. 30 s: neue Requests `TRANSIENT`, offene
bekommen Reply oder `TIMEOUT`, der Rest nach 30 s `TRANSIENT`; dann
`CloseConnection`; SIGHUP lädt TLS und ACL neu.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::drain`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{DRAIN_LIMIT, apply_reload}`, `crates/corba-dds-bridge/src/daemon_runtime.rs::install_signal_watcher`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection` (Request während des Drains → `TRANSIENT`, offener Request → `TIMEOUT`, dann `CloseConnection` und EOF), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_9_2_the_drain_answers_what_is_left_with_transient_maybe`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one, corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}`.

**Status:** done
## §10 Cross-Vendor

### §10 RTPS-Peer + TAO/JacORB/omniORB/Ice-Java

**Spec:** §10 — Daemon ist normaler RTPS-Peer; CORBA-Seite gegen
TAO/JacORB/omniORB/Ice-Java-CORBA-Compat.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`,
`crates/corba-dds-bridge/tests/omniorb/`, `crates/corba-dds-bridge/tests/jacorb/`, `crates/corba-dds-bridge/tests/tao/` (Client und Servant je ORB).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::{corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant,
corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop,
corbabridge_10_omniorb_nameclt_binds_lists_resolves_and_unbinds_on_zerodds_naming_svc}`,
`crates/corba-dds-bridge/tests/jacorb_e2e.rs::{corbabridge_10_jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant,
corbabridge_10_jacorb_gssup_credentials_pass_csiv2_and_the_acl,
corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop,
corbabridge_10_jacorb_resolves_the_bridge_through_zerodds_naming_svc}`,
`crates/corba-dds-bridge/tests/tao_e2e.rs::{corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant,
corbabridge_10_tao_resolves_the_bridge_through_zerodds_naming_svc}`,
`crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`.
Alle `#[ignore]`; omniORB 4.3 und JacORB 3.9 im Job
`integration-bridges-vendor-corba`, TAO (aus dem OpenDDS-Build des
Interop-Images) im Job `vendor-live-corba-tao`.

**Status:** done — gegen omniORB 4.3, JacORB 3.9 und TAO gemessen. Ice spricht kein GIOP und ist kein Gegenüber; §10 sagt es (Ice-Handbuch, „The Ice Protocol“, Vergleich mit IIOP).
## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-corba-bridged`; Configs/Services/Docker;
Manuals; IOR-Backup.

**Repo:** `packaging/linux/systemd/zerodds-corba-bridged.service`,
`packaging/macos/launchd/org.zerodds.corba-bridged.plist`,
`packaging/macos/homebrew/zerodds-corba-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/corba-bridged/`,
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man1/zerodds-corba-bridged.1`,
`man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_3_the_shipped_example_reads_as_it_says`
(die ausgelieferte Beispiel-Konfiguration durch den echten Loader),
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page,
the_man_page_names_no_key_the_example_does_not_declare,
every_man_page_names_its_parser_and_what_an_unknown_key_does}`,
`tools/traceability/tests/deployment_layout.rs`.

**Status:** done
## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/giop_codec/iiop_transport/ior/object_key/
csiv2/dds_pump/request_correlator je ≥ 5 Tests.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs`, `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs`, `crates/corba-dds-bridge/src/bridge_daemon/pump.rs`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs`, `crates/corba-dds-bridge/src/bridge_daemon/log.rs`, `crates/corba-dds-bridge/src/bridge_daemon/trace.rs`, `crates/corba-dds-bridge/src/csiv2_wire.rs`, `crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-ior/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_3_the_example_of_the_spec_loads` (10 Tests im Modul), `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::tests::corbabridge_4_6_a_giop_1_1_reply_is_fragmented_without_request_ids` (6 Tests im Modul), `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_1_an_iiop_1_0_profile_has_no_components` (7 Tests im Modul), `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::tests::corbabridge_4_5_the_key_of_a_request_reply_mapping_is_the_sha_256_prefix` (6 Tests im Modul), `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::tests::corbabridge_4_4_every_target_gets_the_sample` (5 Tests im Modul), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_4_3_a_reply_sample_answers_its_request` (7 Tests im Modul), `crates/corba-dds-bridge/src/csiv2_wire.rs::tests::valid_credentials_are_accepted_with_the_username_as_principal` (6 Tests im Modul); `crates/corba-giop/` (GIOP-Codec) und `crates/corba-iiop/` (IIOP-Transport) mit ihren eigenen Tests, `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::tests::corbabridge_5_2_request_and_reply_are_generated_from_the_operation` (7 Tests im Modul).

**Status:** done
### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Daemon, ein fremder CORBA-Client, ein
DDS-Service-Prozess, Roundtrip über die Bridge.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist, corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection, corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_dump_iors_writes_stringified_ior, corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate, corbabridge_4_7_ssliop_only_forwards_a_plain_iiop_client, corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange}`.

**Status:** done
### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + TAO-Client + ZeroDDS-
CORBA-Bridge im Compose.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/cyclone/` (`calc_backend.c`, `CalcDds.idl`), `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `ci/integration_scenarios.py` (Job `integration-bridges-vendor-corba`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_12_3_an_omniorb_client_reaches_a_cyclone_dds_backend` (omniORB 4.3 ruft `add`, Cyclone DDS 0.10.5 beantwortet den Request in XCDR1, omniORB erhält 42; Linux-Testhost), `crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`, `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_5_2_cyclone_dds_builds_the_generated_types_and_raises_a_user_exception`.

**Status:** done
### §10/§12 Cross-Vendor-Harness — TAO, JacORB, Ice-Java, Cyclone

**Spec:** §10 + §12.3 — die CORBA-Seite gegen TAO, JacORB und
Ice-Java; ein Cyclone-DDS-Backend hinter der Bridge.

**Repo:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/cyclone/`.

**Tests:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant`, `crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_12_3_an_omniorb_client_reaches_a_cyclone_dds_backend`.

**Status:** done — omniORB, JacORB und TAO auf der CORBA-Seite, Cyclone DDS als Backend nach dem Layout aus §4.3. Ice spricht kein GIOP (§10).
## §13 Cross-References

### §13 Verwandte Library + OMG-Specs + Daemons

**Spec:** §13 — Library `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
`crates/corba-giop/` und der Rust-Codegen in `crates/lang-rules/`;
OMG GIOP/IIOP/CSIv2/SSLIOP; Wire-Format; Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Mapping-Konfiguration,
Major=Wire-Protocol-Changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

27 done / 0 partial / 0 open / 1 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9875 von `main` (`python3 ci/coverage_results.py --document zerodds-corba-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (Job `merge-rust-bridge-security`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (Job `merge-rust-config`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-corba-dds-bridge --all-targets` (Job `merge-rust-corba-dds-bridge`) — 99 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (Job `merge-rust-corba-giop`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (Job `merge-rust-corba-ior`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 25 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-corba` (Job `integration-bridges-vendor-corba`) — 9 Tests grün.
* Job `vendor-live-corba-tao` — 2 Tests grün.

142 zitierte Tests grün, 2 nicht grün.

Offene Punkte: `zerodds-corba-bridge-1.0.open.md`.
