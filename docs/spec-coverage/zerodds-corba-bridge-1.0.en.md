# `zerodds-corba-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-bridge-1.0.md`

Implementation:

- `crates/corba-dds-bridge/` — CORBA↔DDS bridge (DDS4CCM connector).

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §2 to §12.

### §1a L6 multi-tenant

**Spec:** §1 L6 — several DomainParticipants per daemon; one tenant
binding per IIOP connection.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs` (`TenantConfig`, `DaemonConfig::of_tenant`), `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{Tenants, accept_loop}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ior_lines`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate` (an SSLIOP session with the tenant's certificate reaches its object, plain IIOP and a session without that certificate get `OBJECT_NOT_EXIST`), `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_1_a_tenants_ior_line_names_the_tenant`.

**Status:** done
## §2 CLI surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — `--config`/`--set`/`--check-config`, the short forms
`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/
`--tls-cert`+`--tls-key`/`--topic`/`--log-level`/`--metrics` as `--set`
of their file key; exit codes 0–5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`, `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`, `man/man1/zerodds-corba-bridged.1`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_version_flag_emits_one_line, corbabridge_2_unknown_arg_yields_exit_1, corbabridge_2_dump_iors_writes_stringified_ior, corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_a_port_in_use_is_exit_2, corbabridge_2_a_participant_that_does_not_start_is_exit_3, corbabridge_2_a_name_service_that_does_not_answer_is_exit_5}`.

**Status:** done
## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top level `domain`/`log_level`/`corba`/`mappings`/`acl`/
`metrics`; `zerodds-config` subset with `include:`, drop-ins and
`${VAR}`; unknown keys and wrong types prevent the start; what is not
built is refused.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs` (`DaemonConfig::from_config`, `read_mapping`, `is_key`), `crates/config/`, `packaging/linux/configs/corba-bridged.yaml.example`, `man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::{tests::corbabridge_3_the_example_of_the_spec_loads, tests::corbabridge_3_the_shipped_example_reads_as_it_says, tests::corbabridge_3_what_the_daemon_does_not_build_is_refused, tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read}`, `crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done
## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation IIOP/SSLIOP/Components

**Spec:** §4.1 — one IOR per mapping with `type_id` = `repo_id`, an
IIOP profile (host/port/object_key) and components; SSL component for
SSLIOP; `ior_file`; NameService bind.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/ior.rs` (`build_ior`, `ior_for_key`, `profile_components`, `ssl_options`, `csiv2_mech_list`, `tls_sec_trans`, `ior_lines`), `crates/corba-dds-bridge/src/naming_client.rs`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::{tests::corbabridge_4_1_the_ior_carries_code_sets_and_the_ssl_component, tests::the_ior_carries_the_csiv2_mech_list_when_enabled, tests::corbabridge_4_1_an_iiop_1_0_profile_has_no_components, tests::the_advertise_host_is_a_setting, tests::corbabridge_4_1_a_tenants_ior_line_names_the_tenant}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_dump_iors_writes_stringified_ior`, `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/naming_service_e2e.rs::{the_bridge_rebinds_into_a_real_naming_svc_and_unbinds_on_stop, the_bridge_rebinds_over_ssliop, the_bridge_rebinds_through_a_rir_initial_reference}`, `crates/corba-ior/src/components.rs::tests::a_code_sets_encapsulation_pads_after_the_byte_order_octet`.

**Status:** done — `TAG_ORB_TYPE` is item §4.1a.
### §4.1a TAG_ORB_TYPE

**Spec:** §4.1 — the component `TAG_ORB_TYPE` in the IIOP profile of the IOR.

**Repo:** —

**Tests:** —

**Status:** blocked — assignment of an ORB type by the OMG (requested;
`TAG_ORB_TYPE` by CORBA 3.3 Part 2 §7.6.6.1 carries a value the OMG
assigns); lifted once the OMG assigns the value. Until then the bridge's IOR
does not carry the component; it is optional, and without it a client
selects no vendor-specific deviations.

### §4.2 GIOP frame format header + body

**Spec:** §4.2 — 12-byte header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{serve_connection, message_error}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs`, Codec in `crates/corba-giop/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_4_2_a_message_that_is_not_giop_gets_message_error`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`, `crates/corba-giop/src/header.rs::{magic_bytes_match_spec, header_size_is_12_bytes, round_trip_big_endian_request_header}`, `crates/corba-giop/src/message_type.rs::message_type_values_match_spec_table_15_1`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §4.3 Request-reply mapping CORBA→DDS

**Spec:** §4.3 — GIOP request → mapping lookup → DDS request sample
(`u64 request_id` + arguments, XCDR1 in the GIOP byte order) →
correlated reply sample (`request_id`, `reply_status`, `reserved`,
body) → GIOP reply; `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`,
`NO_PERMISSION`, `NO_IMPLEMENT` (GIOP < 1.2), `TRANSIENT`, `MARSHAL`
(also for a reply sample that is not plain CDR); `CancelRequest`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{on_request, refuse}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{on_reply, timeout_loop, cancel}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::system_exception_body`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{tests::corbabridge_4_3_a_mapped_operation_over_giop_1_1_is_no_implement, tests::corbabridge_4_3_a_failed_dds_write_is_transient, tests::corbabridge_4_3_the_target_and_the_operation_decide}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{tests::corbabridge_4_3_a_reply_sample_answers_its_request, tests::corbabridge_4_3_the_reply_status_and_the_representation_decide, tests::corbabridge_4_3_a_reply_without_its_request_is_dropped, tests::corbabridge_4_3_cancel_request_removes_the_request_of_its_connection}`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::tests::corbabridge_4_3_a_system_exception_body_is_the_giop_layout`, `crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_3_a_reply_sample_that_is_not_plain_cdr_is_marshal`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::tests::corbabridge_4_3_the_correlation_member_has_the_name_of_correlation_id`.

**Status:** done — `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`, `NO_PERMISSION`, `NO_IMPLEMENT`, `TRANSIENT`, `MARSHAL` and `CancelRequest` are verified. Step 5: the correlation field is the first member; `correlation_id` names it in the generated type (§5.2).
### §4.4 Notify mapping DDS→CORBA

**Spec:** §4.4 — DDS sample on `notify_topic` → oneway GIOP 1.2
request to every target in `targets` (`corbaloc:` or IOR); a sample that
is not plain CDR (XCDR2, `PL_CDR`) is dropped and counted; reconnect after an
error.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::{pump_notify, on_notify_sample, notifiers_of}`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs::{Notifier, closed}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::resolve_target`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::{tests::corbabridge_4_4_a_sample_is_a_oneway_request_with_the_sample_as_body, tests::corbabridge_4_4_a_sample_that_is_not_plain_cdr_is_refused, tests::corbabridge_4_4_every_target_gets_the_sample, tests::corbabridge_4_4_an_unreachable_target_does_not_stop_the_others}`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs::{tests::corbabridge_4_4_a_closed_connection_is_opened_again, tests::resolve_follows_object_forward_then_confirms_object_here, tests::send_follows_location_forward_on_the_reply_to_the_oneway_itself, tests::send_reports_an_unexpected_reply_status_instead_of_treating_it_as_delivered}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_4_corbaloc_and_ior_targets_resolve`, `crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_4_a_notify_sample_that_is_not_plain_cdr_is_not_sent`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §4.5 Object-key generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::{object_key, canonical_topics}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::check_mappings`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::{tests::corbabridge_4_5_the_key_of_a_request_reply_mapping_is_the_sha_256_prefix, tests::corbabridge_4_5_the_key_of_a_notify_mapping_is_of_the_notify_topic, tests::corbabridge_4_5_a_key_is_16_octets, tests::corbabridge_4_5_a_bidir_mapping_takes_the_request_reply_form, tests::corbabridge_4_5_repo_id_and_topics_name_the_key, tests::corbabridge_4_5_two_mappings_with_one_key_are_refused}` (the expected keys are the first 16 octets of `shasum -a 256` of the same octets), `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`.

**Status:** done
### §4.6 Fragment handling GIOP 1.1+

**Spec:** §4.6 — messages above `fragment_size` as fragments, each but
the last ending on an 8-octet boundary of the message; inbound
reassembly.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::{write_giop, fragments, Reassembly}`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::{tests::corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries, tests::corbabridge_4_6_a_giop_1_1_reply_is_fragmented_without_request_ids, tests::corbabridge_4_6_a_message_within_the_fragment_size_goes_out_whole, tests::corbabridge_4_6_a_fragment_out_of_place_is_refused, tests::corbabridge_4_6_a_fragmented_message_is_put_back_together}`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant` (`echo` of 200,000 bytes, `fragment_size` 16384, fragmented in both directions, against omniORB 4.3.3).

**Status:** done
### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` with `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{locate_status, knows, forward_ior, forward_request, serve_connection}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ior_for_key`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{tests::corbabridge_4_7_locate_names_known_and_unknown_objects, tests::corbabridge_4_7_the_forward_ior_of_ssliop_only_names_the_ssl_port}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_7_ssliop_only_forwards_a_plain_iiop_client` (`OBJECT_FORWARD` and `LOCATION_FORWARD` with port 0 and `TAG_SSL_SEC_TRANS`, over SSLIOP the object answers), `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`.

**Status:** done
## §5 Topic mapping

### §5.1 DDS-topic slug per operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` as default; override via `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::slug`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_5_1_the_slug_of_an_operation`.

**Status:** done
### §5.2 Type discovery from the IDL of the interface

**Spec:** §5.2 — with `idl_file` the daemon reads the IDL of the interface
and generates per mapped operation `Request`, `Reply` (a union over the
`ReplyStatusType`, user exceptions as a variant over `exception_kind`) and
`Notify`; the IDL is in `corba.idl_dir`. A user exception in the sample is
written in the GIOP form. `wchar`, `wstring`, `fixed`, `any`, object
references, valuetypes, `native` and `@mutable` are refused. Without
`idl_file`: the fixed layout of §4.3.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::{generate, units, apply, files, user_exception_body}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::read_mapping`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::on_reply`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::{tests::corbabridge_5_2_request_and_reply_are_generated_from_the_operation, tests::corbabridge_5_2_the_exceptions_are_the_variant_exception_kind, tests::corbabridge_5_2_out_and_inout_arguments_and_the_used_types, tests::corbabridge_5_2_oneway_attributes_and_notify, tests::corbabridge_5_2_what_has_no_dds_form_is_refused, tests::corbabridge_5_2_a_user_exception_becomes_its_giop_body}`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_5_2_idl_file_gives_the_mapping_its_types`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_5_2_a_user_exception_sample_is_the_giop_exception`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_5_2_the_daemon_writes_the_generated_idl_into_idl_dir`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_5_2_cyclone_dds_builds_the_generated_types_and_raises_a_user_exception`.

**Status:** done — measured against omniORB 4.3 and Cyclone DDS 0.10.5: idlc compiles the generated IDL, a Cyclone DDS program on these types answers, omniORB gets the result with its `out` argument, the exception with members and the one without.

## §6 QoS translation

### §6 DDS-QoS → CORBA-behavior map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/
Partition; `BEST_EFFORT` only for notify.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::read_mapping` (`qos.*`), `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{endpoint_qos, build_dispatch}`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{ReplayStore, deadline_missed, check_notify_deadlines}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::on_request`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::tests::corbabridge_6_the_qos_of_a_mapping_are_its_endpoint_policies`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::{tests::corbabridge_6_the_replay_store_keeps_the_last_replies_for_their_lifespan, tests::corbabridge_6_a_missed_deadline_answers_with_transient}`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_6_a_repeated_request_is_answered_from_the_replay_store`, `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::tests::corbabridge_6_a_notify_sample_restarts_the_deadline`, `crates/corba-dds-bridge/src/bridge_daemon/config.rs::{tests::corbabridge_6_the_qos_keys_and_the_tenants_are_read, tests::corbabridge_3_what_the_daemon_does_not_build_is_refused}`.

**Status:** done
## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` enables TLS on `ssliop.bind`,
`require_client_cert` against `ca_file`; SSL component in the IOR.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{ssliop_server_config, accept_loop, TlsShared}`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::ssl_options`, `crates/bridge-security/src/tls.rs::load_server_config_with_optional_client_auth`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs::{corbabridge_7_1_build_corba_tls_config_loads_self_signed, corbabridge_7_1_build_corba_tls_config_with_mtls_succeeds, corbabridge_7_1_build_corba_tls_config_rejects_missing_cert}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate, corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}` (with `ca_file` and without `require_client_cert` a session without a certificate is established, one with a certificate carries its subject), `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only` (omniORB 4.3.3 with `libomnisslTP4`, Linux test host).

**Status:** done
### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP user/password fallback.

**Repo:** `crates/corba-dds-bridge/src/csiv2_wire.rs`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::on_request`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::{csiv2_mech_list, tls_sec_trans}`, `crates/corba-dds-bridge/src/bridge_daemon/metrics.rs::csiv2_reason`, `crates/corba-csiv2/`.

**Tests:** `crates/corba-dds-bridge/tests/csiv2_e2e.rs::{csiv2_wrong_password_is_no_permission_with_a_context_error, corbabridge_7_2_missing_credentials_are_refused_by_csiv2, csiv2_accepted_credentials_proceed_past_acl_to_a_dds_timeout}`, `crates/corba-dds-bridge/src/csiv2_wire.rs::tests` (6), `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_2_without_gssup_or_certificate_csiv2_refuses`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::the_ior_carries_the_csiv2_mech_list_when_enabled`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_gssup_credentials_pass_csiv2_and_the_acl`.

**Status:** done
### §7.3 ACL per mapping

**Spec:** §7.3 — subject from the client certificate's subject DN;
a rule with a matching subject lists the `repo_id` in `allow_invoke`
(prefix with `*`); otherwise `default_deny`; refused: `NO_PERMISSION`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{acl_allows, on_request}`, `crates/bridge-security/src/ctx.rs::extract_mtls_subject`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`, `crates/bridge-security/src/ctx.rs::tests::corbabridge_7_3_the_mtls_subject_is_the_dn_and_the_acl_also_takes_the_fingerprint`, `crates/corba-dds-bridge/tests/csiv2_e2e.rs::csiv2_accepted_credentials_proceed_past_acl_to_a_dds_timeout` (`gssup:alice` as the subject).

**Status:** done
## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log with `timestamp`, `level`, `event` and the
fields of the event; `--log-level`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/log.rs` (`line`, `emit`, `rfc3339`, `escape`), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::finish`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/log.rs::{tests::corbabridge_8_1_a_line_is_a_json_object_with_the_spec_fields, tests::rfc3339_5_6_the_utc_date_and_time, tests::rfc8259_7_text_is_escaped, tests::corbabridge_8_1_the_log_level_is_the_threshold}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange`.

**Status:** done
### §8.2 Prometheus metrics

**Spec:** §8.2 — ten families `zerodds_corba_bridge_*`
(requests/replies/pending/latency/timeouts/…).

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/metrics.rs` (`SpecMetrics`, `CSIV2_REASONS`), `crates/corba-dds-bridge/src/daemon_runtime.rs` (`BridgeMetrics`, `serve_admin_endpoints`), `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` (`metrics_addr_for`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::{corbabridge_8_2_metrics_register_idempotent, corbabridge_8_2_admin_endpoint_serves_metrics_path}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::tests::corbabridge_7_2_without_gssup_or_certificate_csiv2_refuses` (`csiv2_failures_total{reason="missing"}`).

**Status:** done
### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` → spans per GIOP
exchange (`giop.request`, `dds.publish`, `dds.subscribe`, `giop.reply`).

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/trace.rs` (`Tracer`, `Exchange`, `spans`), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::finish`, `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/trace.rs::{tests::corbabridge_8_3_an_exchange_is_a_request_span_with_three_children, tests::corbabridge_8_3_a_refused_request_is_the_root_alone, tests::corbabridge_8_3_the_dds_children_carry_the_topic, tests::corbabridge_8_3_a_tracer_without_exporter_is_off, tests::corbabridge_8_3_span_ids_differ}`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange`, `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::corbabridge_8_3_otlp_config_parses_endpoint`.

**Status:** done
## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer per mapping →
IIOP/SSLIOP bind → NameService rebind → signal handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`, `crates/corba-dds-bridge/src/bridge_daemon/mod.rs::{build_dispatch, run_with_reload}`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_1_the_port_is_bound_before_an_ior_is_published, corbabridge_2_a_participant_that_does_not_start_is_exit_3, corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist}`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done
### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — drain of at most 30 s: new requests `TRANSIENT`,
pending ones get their reply or `TIMEOUT`, the rest `TRANSIENT` after
30 s; then `CloseConnection`; SIGHUP reloads TLS and ACL.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::drain`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs::{DRAIN_LIMIT, apply_reload}`, `crates/corba-dds-bridge/src/daemon_runtime.rs::install_signal_watcher`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection` (a request during the drain → `TRANSIENT`, a pending one → `TIMEOUT`, then `CloseConnection` and EOF), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_9_2_the_drain_answers_what_is_left_with_transient_maybe`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one, corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}`.

**Status:** done
## §10 Cross-vendor

### §10 RTPS peer + TAO/JacORB/omniORB/Ice-Java

**Spec:** §10 — the daemon is a normal RTPS peer; CORBA side against
TAO/JacORB/omniORB/Ice-Java CORBA compat.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`,
`crates/corba-dds-bridge/tests/omniorb/`, `crates/corba-dds-bridge/tests/jacorb/`, `crates/corba-dds-bridge/tests/tao/` (client and servant per ORB).

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
All `#[ignore]`; omniORB 4.3 and JacORB 3.9 in the job
`integration-bridges-vendor-corba`, TAO (from the OpenDDS build of the
interop image) in the job `vendor-live-corba-tao`.

**Status:** done — measured against omniORB 4.3, JacORB 3.9 and TAO. Ice does not speak GIOP and is no peer; §10 says so (Ice manual, "The Ice Protocol", comparison with IIOP).
## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-corba-bridged`; configs/services/Docker;
manuals; IOR backup.

**Repo:** `packaging/linux/systemd/zerodds-corba-bridged.service`,
`packaging/macos/launchd/org.zerodds.corba-bridged.plist`,
`packaging/macos/homebrew/zerodds-corba-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/corba-bridged/`,
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man1/zerodds-corba-bridged.1`,
`man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_3_the_shipped_example_reads_as_it_says`
(the shipped example configuration through the real loader),
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page,
the_man_page_names_no_key_the_example_does_not_declare,
every_man_page_names_its_parser_and_what_an_unknown_key_does}`,
`tools/traceability/tests/deployment_layout.rs`.

**Status:** done
## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/giop_codec/iiop_transport/ior/object_key/
csiv2/dds_pump/request_correlator with ≥ 5 tests each.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs`, `crates/corba-dds-bridge/src/bridge_daemon/framing.rs`, `crates/corba-dds-bridge/src/bridge_daemon/ior.rs`, `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs`, `crates/corba-dds-bridge/src/bridge_daemon/pump.rs`, `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs`, `crates/corba-dds-bridge/src/bridge_daemon/server.rs`, `crates/corba-dds-bridge/src/bridge_daemon/notify.rs`, `crates/corba-dds-bridge/src/bridge_daemon/log.rs`, `crates/corba-dds-bridge/src/bridge_daemon/trace.rs`, `crates/corba-dds-bridge/src/csiv2_wire.rs`, `crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-ior/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon/config.rs::tests::corbabridge_3_the_example_of_the_spec_loads` (10 tests in the module), `crates/corba-dds-bridge/src/bridge_daemon/framing.rs::tests::corbabridge_4_6_a_giop_1_1_reply_is_fragmented_without_request_ids` (6 tests in the module), `crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_1_an_iiop_1_0_profile_has_no_components` (7 tests in the module), `crates/corba-dds-bridge/src/bridge_daemon/object_key.rs::tests::corbabridge_4_5_the_key_of_a_request_reply_mapping_is_the_sha_256_prefix` (6 tests in the module), `crates/corba-dds-bridge/src/bridge_daemon/pump.rs::tests::corbabridge_4_4_every_target_gets_the_sample` (5 tests in the module), `crates/corba-dds-bridge/src/bridge_daemon/correlator.rs::tests::corbabridge_4_3_a_reply_sample_answers_its_request` (7 tests in the module), `crates/corba-dds-bridge/src/csiv2_wire.rs::tests::valid_credentials_are_accepted_with_the_username_as_principal` (6 tests in the module); `crates/corba-giop/` (the GIOP codec) and `crates/corba-iiop/` (the IIOP transport) with their own tests, `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs::tests::corbabridge_5_2_request_and_reply_are_generated_from_the_operation` (7 tests in the module).

**Status:** done
### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — daemon, a foreign CORBA client, a DDS service
process, round trip through the bridge.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist, corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection, corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_dump_iors_writes_stringified_ior, corbabridge_1_l6_a_tenant_is_bound_to_its_client_certificate, corbabridge_4_7_ssliop_only_forwards_a_plain_iiop_client, corbabridge_8_1_8_3_the_log_line_and_the_spans_of_an_exchange}`.

**Status:** done
### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone DDS subscriber + TAO client + ZeroDDS CORBA
bridge in a compose.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/cyclone/` (`calc_backend.c`, `CalcDds.idl`), `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `ci/integration_scenarios.py` (Job `integration-bridges-vendor-corba`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_12_3_an_omniorb_client_reaches_a_cyclone_dds_backend` (omniORB 4.3 calls `add`, Cyclone DDS 0.10.5 answers the request in XCDR1, omniORB gets 42; Linux test host), `crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`, `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_5_2_cyclone_dds_builds_the_generated_types_and_raises_a_user_exception`.

**Status:** done
### §10/§12 Cross-vendor harness — TAO, JacORB, Ice-Java, Cyclone

**Spec:** §10 + §12.3 — the CORBA side against TAO, JacORB and
Ice-Java; a Cyclone DDS backend behind the bridge.

**Repo:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs`, `crates/corba-dds-bridge/tests/cyclone/`.

**Tests:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant`, `crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_12_3_an_omniorb_client_reaches_a_cyclone_dds_backend`.

**Status:** done — omniORB, JacORB and TAO on the CORBA side, Cyclone DDS as the backend by the layout of §4.3. Ice does not speak GIOP (§10).
## §13 Cross-references

### §13 Related library + OMG specs + daemons

**Spec:** §13 — library `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
`crates/corba-giop/` and the Rust codegen in `crates/lang-rules/`;
OMG GIOP/IIOP/CSIv2/SSLIOP; wire format; deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive mapping configuration,
major=wire-protocol changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

27 done / 0 partial / 0 open / 1 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9875 of `main` (`python3 ci/coverage_results.py --document zerodds-corba-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (job `merge-rust-config`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-corba-dds-bridge --all-targets` (job `merge-rust-corba-dds-bridge`) — 99 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (job `merge-rust-corba-giop`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (job `merge-rust-corba-ior`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 25 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-corba` (job `integration-bridges-vendor-corba`) — 9 tests green.
* job `vendor-live-corba-tao` — 2 tests green.

142 cited tests green, 2 not green.

Open items: `zerodds-corba-bridge-1.0.open.en.md`.
