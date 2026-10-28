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

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — `--config`/`--set`/`--check-config`, the short forms
`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/
`--tls-cert`+`--tls-key`/`--topic`/`--log-level`/`--metrics` as `--set`
of their file key; exit codes 0–5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(thin CLI), `crates/corba-dds-bridge/src/bridge_daemon.rs::run`
(exit codes 2 bind, 3 DDS, 4 TLS), `man/man1/zerodds-corba-bridged.1`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{version_flag_emits_one_line,
unknown_arg_yields_exit_1, dump_iors_writes_stringified_ior,
a_certificate_that_does_not_load_is_exit_4}`.

**Status:** partial — exit code 5 (NameService) does not occur while
the NameService registration is not built (§4.1); exit codes 2 and 3
are set in the code but have no test of their own.

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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{the_shipped_example_reads_as_it_says,
what_the_daemon_does_not_build_is_refused}`,
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done

## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation IIOP/SSLIOP/Components

**Spec:** §4.1 — one IOR per mapping with `type_id` = `repo_id`, an
IIOP profile (host/port/object_key) and components; SSL component for
SSLIOP; `ior_file`; NameService bind.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`build_ior`, `ior_lines`, `corba.iiop.advertise_host`).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::dump_iors_writes_stringified_ior`,
`crates/corba-dds-bridge/tests/cross_vendor.rs::catior_reads_the_ior_the_daemon_prints`
(omniORB `catior` reads the IOR the daemon prints; behind
`--features cross-vendor-tests` and `#[ignore]`, measured on the Linux test host),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB calls through the IOR from `ior_file`).

`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::the_ior_carries_code_sets_and_the_ssl_component`,
`crates/corba-ior/src/components.rs::tests::a_code_sets_encapsulation_pads_after_the_byte_order_octet`
(the encapsulation aligns from its byte-order octet; `catior` called the
unpadded form "Broken component with tag 1").

**Status:** partial — `TAG_CODE_SETS` and `TAG_SSL_SEC_TRANS` built,
`catior` reads both; no `TAG_ORB_TYPE` (no OMG-assigned ORB type);
`publish_to_naming: true` is refused (NameService not built).

### §4.2 GIOP frame format header + body

**Spec:** §4.2 — 12-byte header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`
(header read, `max_message_size`, `MessageError`), codec in
`crates/corba-giop/`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-giop/src/header.rs::{magic_bytes_match_spec,
header_size_is_12_bytes, round_trip_big_endian_request_header}`,
`crates/corba-giop/src/message_type.rs::message_type_values_match_spec_table_15_1`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done

### §4.3 Request-reply mapping CORBA→DDS

**Spec:** §4.3 — GIOP request → mapping lookup → DDS request sample
(`u64 request_id` + arguments, XCDR1 in the GIOP byte order) →
correlated reply sample (`request_id`, `reply_status`, `reserved`,
body) → GIOP reply; `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`,
`NO_PERMISSION`, `NO_IMPLEMENT` (GIOP < 1.2), `TRANSIENT`, `MARSHAL`;
`CancelRequest`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`, `timeout_loop`, `system_exception_body`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB 4.3.3 on the Linux test host: `add` → 42 through a DDS backend, `never` →
`TIMEOUT`, wrong operation → `BAD_OPERATION`),
`crates/corba-dds-bridge/tests/bridge_e2e.rs::a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::a_system_exception_body_is_the_giop_layout`.

**Status:** partial — `NO_PERMISSION`, `NO_IMPLEMENT`, `TRANSIENT` on a
DDS write error, `MARSHAL` and `CancelRequest` are built but have no
test of their own; a `correlation_id` other than `request_id` is
refused.

### §4.4 Notify mapping DDS→CORBA

**Spec:** §4.4 — DDS sample on `notify_topic` → oneway GIOP 1.2
request to every target in `targets` (`corbaloc:` or IOR); an XCDR2
sample is dropped and counted; reconnect after an error.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`Notifier`, `resolve_target`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(three DDS samples reach an omniORB servant as `on_value`),
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbaloc_and_ior_targets_resolve`.

**Status:** done

### §4.5 Object-key generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::object_key`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::the_shutdown_answers_pending_requests_then_closes_the_connection`
(the test addresses the mapped operation through `object_key`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB addresses through the key in the IOR).

**Status:** done

### §4.6 Fragment handling GIOP 1.1+

**Spec:** §4.6 — messages above `fragment_size` as fragments, each but
the last ending on an 8-octet boundary of the message; inbound
reassembly.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`write_giop`, `Reassembly`).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{outbound_fragments_end_on_8_octet_boundaries,
a_fragmented_message_is_put_back_together}`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(`echo` of 200 000 bytes, `fragment_size` 16384, fragmented in both
directions, against omniORB 4.3.3).

**Status:** done

### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` with `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`
(`LocateRequest` branch).

**Tests:** —

**Status:** partial — `OBJECT_HERE` and `UNKNOWN_OBJECT` are built but
untested in the daemon (`tests/cross_vendor.rs::locate_request_handler_classifies_known_and_unknown`
checks the library's `src/locate.rs`, not the daemon);
`OBJECT_FORWARD` is not built.

## §5 Topic mapping

### §5.1 DDS-topic slug per operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` as default; override via `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::slug`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::the_slug_of_section_5_1`.

**Status:** done

### §5.2 Type discovery via idl-rust codegen

**Spec:** §5.2 — one generated request/reply type per mapping with an
exception variant; IDL in `/var/lib/zerodds/corba-bridge/`. Built: the
fixed layout of §4.3.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`: fixed layout).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{the_shipped_example_reads_as_it_says,
what_the_daemon_does_not_build_is_refused}`.

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

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs::{build_corba_tls_config_loads_self_signed,
build_corba_tls_config_with_mtls_succeeds,
build_corba_tls_config_rejects_missing_cert}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::a_certificate_that_does_not_load_is_exit_4`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(omniORB 4.3.3 with `libomnisslTP4`, `clientTransportRule * ssl`, port from
`TAG_SSL_SEC_TRANS`, client certificate required; the Linux test host).

**Status:** done

### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP user/password fallback.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`csiv2.enabled: true` is refused).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::what_the_daemon_does_not_build_is_refused`.

**Status:** open — CSIv2 is not built in the daemon. The building
blocks `crates/corba-csiv2/` and
`crates/corba-dds-bridge/src/csiv2_wire.rs` are not wired to the
daemon. Plan: read the `SAS_ContextElement` from each request's service
context, check GSSUP against a user file and pass the subject to the
ACL of §7.3.

### §7.3 ACL per mapping

**Spec:** §7.3 — subject from the client certificate's subject DN;
a rule with a matching subject lists the `repo_id` in `allow_invoke`
(prefix with `*`); otherwise `default_deny`; refused: `NO_PERMISSION`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`subject_allowed`, `on_request`),
`crates/bridge-security/src/ctx.rs::extract_mtls_subject`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::acl_rules_match_subjects_and_repo_id_prefixes`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(certificate A passes and runs into `TIMEOUT`, certificate B of the same
CA gets `NO_PERMISSION`; omniORB 4.3.3, Linux test host).

The subject is the subject DN per RFC 4514
(`crates/bridge-security/src/ctx.rs::mtls_subject_from_der`, checked against
`openssl x509 -nameopt RFC2253`); a rule may name the fingerprint
`mtls:<SHA-256>` instead (as the omniORB test does). The GSSUP username of
§7.2 depends on CSIv2 (§7.2, open).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log with `timestamp`, `level`, `event` and the
fields of the event; `--log-level`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`log`, `event_level`, `level_rank`).

**Tests:** —

**Status:** partial — one JSON line per event on stderr with `level`,
`event` and the event fields, filtered by `log_level`; `timestamp` is
missing, and no test checks the form of a line.

### §8.2 Prometheus metrics

**Spec:** §8.2 — ten families `zerodds_corba_bridge_*`
(requests/replies/pending/latency/timeouts/…).

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs`
(`BridgeMetrics`, `serve_admin_endpoints`),
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(`metrics_addr_for`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::{metrics_register_idempotent,
admin_endpoint_serves_metrics_path}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::the_metrics_of_section_8_2_count_a_timed_out_request`
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

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::otlp_config_parses_endpoint`.

**Status:** partial — the exporter starts from the environment, but
`bridge_daemon.rs` creates no spans.

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer per mapping →
IIOP/SSLIOP bind → NameService rebind → signal handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::run`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::a_request_to_an_unknown_object_is_object_not_exist`.

**Status:** partial — the NameService rebind does not occur while
`publish_to_naming` is not built; the other steps run in the order of
the spec.

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — drain of at most 30 s: new requests `TRANSIENT`,
pending ones get their reply or `TIMEOUT`, the rest `TRANSIENT` after
30 s; then `CloseConnection`; SIGHUP reloads TLS and ACL.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`drain`, `DRAIN_LIMIT`, `Conn`),
`crates/corba-dds-bridge/src/daemon_runtime.rs::install_signal_watcher`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::the_shutdown_answers_pending_requests_then_closes_the_connection`
(request during the drain → `TRANSIENT`, pending request → `TIMEOUT`,
then `CloseConnection` and EOF; red without `drain`).

`crates/corba-dds-bridge/tests/bridge_e2e.rs::{a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one,
a_sighup_serves_new_connections_with_the_new_certificate}` (SIGHUP to the
binary; both red without `apply_reload`).

**Status:** partial — the branch after 30 s (`drain_expired`) has no
test.

## §10 Cross-vendor

### §10 RTPS peer + TAO/JacORB/omniORB/Ice-Java

**Spec:** §10 — the daemon is a normal RTPS peer; CORBA side against
TAO/JacORB/omniORB/Ice-Java CORBA compat.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/omniorb/{Calc.idl, calc_client.cc,
listener_server.cc}`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(`#[ignore]`, needs omniidl, g++ and libomniORB4; green on the Linux test host
against omniORB 4.3.3),
`crates/corba-dds-bridge/tests/cross_vendor.rs::catior_reads_the_ior_the_daemon_prints`.

**Status:** partial — measured against omniORB. TAO, JacORB and
Ice-Java are not measured; the rest is the `open` item §10/§12.

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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::the_shipped_example_reads_as_it_says`
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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{the_shipped_example_reads_as_it_says,
the_slug_of_section_5_1, what_the_daemon_does_not_build_is_refused,
a_system_exception_body_is_the_giop_layout,
outbound_fragments_end_on_8_octet_boundaries,
a_fragmented_message_is_put_back_together,
corbaloc_and_ior_targets_resolve,
acl_rules_match_subjects_and_repo_id_prefixes}`.

**Status:** partial — the daemon has eight unit tests in one module;
the clause's split with ≥ 5 tests per module does not exist, and
`csiv2` is not built.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — daemon, a foreign CORBA client, a DDS service
process, round trip through the bridge.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::{a_request_to_an_unknown_object_is_object_not_exist,
the_shutdown_answers_pending_requests_then_closes_the_connection,
a_certificate_that_does_not_load_is_exit_4,
dump_iors_writes_stringified_ior}`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone DDS subscriber + TAO client + ZeroDDS CORBA
bridge in a compose.

**Repo:** `crates/corba-dds-bridge/tests/cross_vendor.rs`.

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs::{omniidl_smoke_check_present,
catior_reads_the_ior_the_daemon_prints}` — both behind
`--features cross-vendor-tests` and `#[ignore]`.

**Status:** partial — the DDS side behind the bridge is ZeroDDS; a
Cyclone DDS backend and a TAO client are not measured. Cross-ref: the
`open` item below.

### §10/§12 Cross-vendor harness — TAO, JacORB, Ice-Java, Cyclone

**Spec:** §10 + §12.3 — the CORBA side against TAO, JacORB and
Ice-Java; a Cyclone DDS backend behind the bridge.

**Repo:** —

**Tests:** —

**Status:** open — measured is omniORB with a ZeroDDS backend
(`tests/omniorb_e2e.rs`). Plan: run the same cases with a TAO and a
JacORB client from `crates/corba-interop/competitors/` and with a
Cyclone DDS backend after the layout of §4.3 on the Linux test host.

## §13 Cross-references

### §13 Related library + OMG specs + daemons

**Spec:** §13 — library `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
`crates/corba-giop/` and the Rust codegen in `crates/lang-rules/`;
OMG GIOP/IIOP/CSIv2/SSLIOP; wire format; deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive mapping configuration,
major=wire-protocol changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

11 done / 13 partial / 2 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-corba-dds-bridge` — 51 lib tests,
6 `bridge_e2e`, 9 `security_e2e` green, 0 failed (macOS).
`tests/omniorb_e2e.rs` (`#[ignore]`) and `tests/cross_vendor.rs`
(`--features cross-vendor-tests`, `#[ignore]`) run on the Linux test host against
omniORB 4.3.3.

Open points and the reasons for the `partial` items:
`zerodds-corba-bridge-1.0.open.en.md`. Decision records: none.
