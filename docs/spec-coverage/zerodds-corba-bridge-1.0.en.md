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

## §2 CLI surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — `--config`/`--set`/`--check-config`, the short forms
`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/
`--tls-cert`+`--tls-key`/`--topic`/`--log-level`/`--metrics` as `--set`
of their file key; exit codes 0–5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(thin CLI), `crates/corba-dds-bridge/src/bridge_daemon.rs::run`
(exit codes 2 bind, 3 DDS, 4 TLS, 5 NameService), `man/man1/zerodds-corba-bridged.1`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_version_flag_emits_one_line,
corbabridge_2_unknown_arg_yields_exit_1, corbabridge_2_dump_iors_writes_stringified_ior,
corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_a_port_in_use_is_exit_2,
corbabridge_2_a_name_service_that_does_not_answer_is_exit_5}`.

**Status:** partial — exit code 3 (DDS) is set in the code but has no
test of its own.

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top level `domain`/`log_level`/`corba`/`mappings`/`acl`/
`metrics`; `zerodds-config` subset with `include:`, drop-ins and
`${VAR}`; unknown keys and wrong types prevent the start; what is not
built is refused.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`DaemonConfig::from_config`, `read_mapping`, `is_key`),
`crates/config/` (parser, layering, `${VAR}`),
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_3_what_the_daemon_does_not_build_is_refused}`,
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done

## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation IIOP/SSLIOP/Components

**Spec:** §4.1 — one IOR per mapping with `type_id` = `repo_id`, an
IIOP profile (host/port/object_key) and components; SSL component for
SSLIOP; `ior_file`; NameService bind.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`build_ior`, `ior_lines`, `corba.iiop.advertise_host`).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_dump_iors_writes_stringified_ior`,
`crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`
(omniORB `catior` reads the IOR the daemon prints; `#[ignore]`, runs in the
job `integration-bridges-vendor-corba`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB calls through the IOR from `ior_file`).

`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_1_the_ior_carries_code_sets_and_the_ssl_component`,
`crates/corba-ior/src/components.rs::tests::a_code_sets_encapsulation_pads_after_the_byte_order_octet`
(the encapsulation aligns from its byte-order octet; `catior` called the
unpadded form "Broken component with tag 1").

**Status:** partial — `TAG_CODE_SETS` and `TAG_SSL_SEC_TRANS` built,
`catior` reads both; no `TAG_ORB_TYPE` (no OMG-assigned ORB type). The
NameService bind is checked by
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`
and `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop`.

### §4.2 GIOP frame format header + body

**Spec:** §4.2 — 12-byte header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`
(header read, `max_message_size`, `MessageError`), codec in
`crates/corba-giop/`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-giop/src/header.rs::{magic_bytes_match_spec,
header_size_is_12_bytes, round_trip_big_endian_request_header}`,
`crates/corba-giop/src/message_type.rs::message_type_values_match_spec_table_15_1`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done

### §4.3 Request-reply mapping CORBA→DDS

**Spec:** §4.3 — GIOP request → mapping lookup → DDS request sample
(`u64 request_id` + arguments, XCDR1 in the GIOP byte order) →
correlated reply sample (`request_id`, `reply_status`, `reserved`,
body) → GIOP reply; `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`,
`NO_PERMISSION`, `NO_IMPLEMENT` (GIOP < 1.2), `TRANSIENT`, `MARSHAL`
(also for a reply sample that is not plain CDR); `CancelRequest`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`, `plain_cdr`, `timeout_loop`, `system_exception_body`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB 4.3.3 on the Linux test host: `add` → 42 through a DDS backend, `never` →
`TIMEOUT`, wrong operation → `BAD_OPERATION`),
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_3_a_system_exception_body_is_the_giop_layout`,
`crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_3_a_reply_sample_that_is_not_plain_cdr_is_marshal`
(a reply sample under `PL_CDR_LE` becomes `MARSHAL`), `NO_PERMISSION` in
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one`.

**Status:** partial — `NO_IMPLEMENT`, `TRANSIENT` on a DDS write error and
`CancelRequest` are built but have no test of their own; a `correlation_id`
other than `request_id` is refused.

### §4.4 Notify mapping DDS→CORBA

**Spec:** §4.4 — DDS sample on `notify_topic` → oneway GIOP 1.2
request to every target in `targets` (`corbaloc:` or IOR); a sample that
is not plain CDR (XCDR2, `PL_CDR`) is dropped and counted; reconnect after an
error.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`Notifier`, `resolve_target`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(three DDS samples reach an omniORB servant as oneway calls of on_value),
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_4_corbaloc_and_ior_targets_resolve`,
`crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_4_a_notify_sample_that_is_not_plain_cdr_is_not_sent`
(a sample under `PL_CDR_LE` does not go out).

**Status:** done

### §4.5 Object-key generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::object_key`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`
(the test addresses the mapped operation through `object_key`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB addresses through the key in the IOR).

**Status:** done

### §4.6 Fragment handling GIOP 1.1+

**Spec:** §4.6 — messages above `fragment_size` as fragments, each but
the last ending on an 8-octet boundary of the message; inbound
reassembly.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`write_giop`, `Reassembly`).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries,
corbabridge_4_6_a_fragmented_message_is_put_back_together}`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(`echo` of 200 000 bytes, `fragment_size` 16384, fragmented in both
directions, against omniORB 4.3.3).

**Status:** done

### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` with `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD`.

**Repo:** —

**Tests:** —

**Status:** open — `OBJECT_HERE` and `UNKNOWN_OBJECT` are built in the daemon (`crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`, branch `LocateRequest`), but without a test; `OBJECT_FORWARD` is not built.

## §5 Topic mapping

### §5.1 DDS-topic slug per operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` as default; override via `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::slug`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_5_1_the_slug_of_an_operation`.

**Status:** done

### §5.2 Type discovery via idl-rust codegen

**Spec:** §5.2 — one generated request/reply type per mapping with an
exception variant; IDL in `/var/lib/zerodds/corba-bridge/`. Built: the
fixed layout of §4.3.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`: fixed layout).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(the DDS backend declares request and reply type after the layout).

**Status:** partial — IDL generation per mapping and the exception
variant in the reply type are not built; the bridge does not know the
IDL type and passes the argument bytes through.

## §6 QoS translation

### §6 DDS-QoS → CORBA-behavior map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/
Partition; `BEST_EFFORT` only for notify.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::read_mapping`
(`qos.reliability`, `qos.durability`; `best_effort` refused for
request-reply).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_3_what_the_daemon_does_not_build_is_refused}`.

**Status:** partial — reliability and durability go to the DDS
entities; a replay per request hash for `TRANSIENT_LOCAL`, lifespan,
deadline and partition are not built.

## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` enables TLS on `ssliop.bind`,
`require_client_cert` against `ca_file`; SSL component in the IOR.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`TlsShared`, `accept_loop`),
`crates/corba-dds-bridge/src/daemon_runtime.rs::build_corba_tls_config`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs::{corbabridge_7_1_build_corba_tls_config_loads_self_signed,
corbabridge_7_1_build_corba_tls_config_with_mtls_succeeds,
corbabridge_7_1_build_corba_tls_config_rejects_missing_cert}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_a_certificate_that_does_not_load_is_exit_4`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(omniORB 4.3.3 with `libomnisslTP4`, `clientTransportRule * ssl`, port from
`TAG_SSL_SEC_TRANS`, client certificate required; the Linux test host).

**Status:** done

### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP user/password fallback.

**Repo:** —

**Tests:** —

**Status:** open — CSIv2 is not built in the daemon; `csiv2.enabled: true` is refused (`crates/corba-dds-bridge/src/bridge_daemon.rs`, test `corbabridge_3_what_the_daemon_does_not_build_is_refused`). The building blocks `crates/corba-csiv2/` and `crates/corba-dds-bridge/src/csiv2_wire.rs` are not wired to the daemon.

### §7.3 ACL per mapping

**Spec:** §7.3 — subject from the client certificate's subject DN;
a rule with a matching subject lists the `repo_id` in `allow_invoke`
(prefix with `*`); otherwise `default_deny`; refused: `NO_PERMISSION`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`subject_allowed`, `on_request`),
`crates/bridge-security/src/ctx.rs::extract_mtls_subject`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(certificate A passes and runs into `TIMEOUT`, certificate B of the same
CA gets `NO_PERMISSION`; omniORB 4.3.3, Linux test host).

The subject is the subject DN per RFC 4514
(`mtls_subject_from_der` in `crates/bridge-security/src/ctx.rs`, test
`crates/bridge-security/src/ctx.rs::tests::corbabridge_7_3_the_mtls_subject_is_the_dn_and_the_acl_also_takes_the_fingerprint`, checked against
`openssl x509 -nameopt RFC2253`); a rule may name the fingerprint
`mtls:<SHA-256>` instead (as the omniORB test does). The GSSUP username of
§7.2 depends on CSIv2 (§7.2, open).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log with `timestamp`, `level`, `event` and the
fields of the event; `--log-level`.

**Repo:** —

**Tests:** —

**Status:** open — `crates/corba-dds-bridge/src/bridge_daemon.rs` (`log`, `event_level`, `level_rank`) writes one JSON line per event to stderr with `level`, `event` and the event fields, filtered by `log_level`; `timestamp` is missing, and no test checks the form of a line.

### §8.2 Prometheus metrics

**Spec:** §8.2 — ten families `zerodds_corba_bridge_*`
(requests/replies/pending/latency/timeouts/…).

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs`
(`BridgeMetrics`, `serve_admin_endpoints`),
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(`metrics_addr_for`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::{corbabridge_8_2_metrics_register_idempotent,
corbabridge_8_2_admin_endpoint_serves_metrics_path}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`
(through `--metrics`: a request without a backend counts as
`system_exception`, timeout, reply sent, sample published; LocateRequest;
red without the count on the timeout path).

**Status:** done — the families of §8.2 except `csiv2_failures_total`
(CSIv2 not built, §7.2).

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` → spans per GIOP
exchange (`giop.request`, `dds.publish`, `dds.subscribe`, `giop.reply`).

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs`
(`otlp_config_from_env`, `spawn_otlp_flush_loop`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::corbabridge_8_3_otlp_config_parses_endpoint`.

**Status:** partial — the exporter starts from the environment, but
`bridge_daemon.rs` creates no spans.

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer per mapping →
IIOP/SSLIOP bind → NameService rebind → signal handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::run`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_9_1_the_port_is_bound_before_an_ior_is_published`
(bind before the IORs are published; before, the daemon wrote the IOR file
and the NameService entry ahead of the bind, and a TAO client got
`TRANSIENT`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — drain of at most 30 s: new requests `TRANSIENT`,
pending ones get their reply or `TIMEOUT`, the rest `TRANSIENT` after
30 s; then `CloseConnection`; SIGHUP reloads TLS and ACL.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`drain`, `DRAIN_LIMIT`, `Conn`),
`crates/corba-dds-bridge/src/daemon_runtime.rs::install_signal_watcher`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`
(request during the drain → `TRANSIENT`, pending request → `TIMEOUT`,
then `CloseConnection` and EOF; red without `drain`).

`crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one,
corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}` (SIGHUP to the
binary; both red without `apply_reload`).

**Status:** partial — the branch after 30 s (`drain_expired`) has no
test.

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

**Status:** partial — measured against omniORB, JacORB and TAO; Ice-Java
not.

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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_3_the_shipped_example_reads_as_it_says`
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

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`,
`crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-ior/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_5_1_the_slug_of_an_operation, corbabridge_3_what_the_daemon_does_not_build_is_refused,
corbabridge_4_3_a_system_exception_body_is_the_giop_layout,
corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries,
corbabridge_4_6_a_fragmented_message_is_put_back_together,
corbabridge_4_4_corbaloc_and_ior_targets_resolve,
corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes}`.

**Status:** partial — the daemon has eight unit tests in one module;
the clause's split with ≥ 5 tests per module does not exist, and
`csiv2` is not built.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — daemon, a foreign CORBA client, a DDS service
process, round trip through the bridge.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist,
corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection,
corbabridge_2_a_certificate_that_does_not_load_is_exit_4,
corbabridge_2_dump_iors_writes_stringified_ior}`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone DDS subscriber + TAO client + ZeroDDS CORBA
bridge in a compose.

**Repo:** `crates/corba-dds-bridge/tests/cross_vendor.rs`.

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`
(`#[ignore]`, job `integration-bridges-vendor-corba`); TAO client:
`crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`
(job `vendor-live-corba-tao`).

**Status:** partial — the DDS side behind the bridge is ZeroDDS; a
Cyclone DDS backend is not measured. Cross-ref: the item below.

### §10/§12 Cross-vendor harness — TAO, JacORB, Ice-Java, Cyclone

**Spec:** §10 + §12.3 — the CORBA side against TAO, JacORB and
Ice-Java; a Cyclone DDS backend behind the bridge.

**Repo:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant`,
`crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** partial — omniORB, JacORB and TAO are measured (§10);
Ice-Java and a Cyclone DDS backend after the layout of §4.3 are not.

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

12 done / 11 partial / 3 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-corba-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (job `merge-rust-config`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-corba-dds-bridge --all-targets` (job `merge-rust-corba-dds-bridge`) — 29 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (job `merge-rust-corba-giop`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (job `merge-rust-corba-ior`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 25 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-corba` (job `integration-bridges-vendor-corba`) — 9 tests green.
* job `vendor-live-corba-tao` — 2 tests green.

72 cited tests green, 0 not green.

Open items: `zerodds-corba-bridge-1.0.open.en.md`.
