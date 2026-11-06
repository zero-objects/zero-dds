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

## §2 CLI-Surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — `--config`/`--set`/`--check-config`, die Kurzformen
`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/
`--tls-cert`+`--tls-key`/`--topic`/`--log-level`/`--metrics` als `--set`
ihres Datei-Schlüssels; Exit-Codes 0–5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(dünne CLI), `crates/corba-dds-bridge/src/bridge_daemon.rs::run`
(Exit-Codes 2 Bind, 3 DDS, 4 TLS, 5 NameService), `man/man1/zerodds-corba-bridged.1`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_2_version_flag_emits_one_line,
corbabridge_2_unknown_arg_yields_exit_1, corbabridge_2_dump_iors_writes_stringified_ior,
corbabridge_2_a_certificate_that_does_not_load_is_exit_4, corbabridge_2_a_port_in_use_is_exit_2,
corbabridge_2_a_name_service_that_does_not_answer_is_exit_5}`.

**Status:** partial — Exit 3 (DDS) ist im Code gesetzt, aber ohne eigenen
Test.

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`corba`/`mappings`/`acl`/
`metrics`; `zerodds-config`-Teilmenge mit `include:`, Drop-ins und
`${VAR}`; unbekannte Schlüssel und falsche Typen verhindern den Start;
nicht Gebautes wird abgelehnt.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`DaemonConfig::from_config`, `read_mapping`, `is_key`),
`crates/config/` (Parser, Schichtung, `${VAR}`),
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_3_what_the_daemon_does_not_build_is_refused}`,
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done

## §4 GIOP/IIOP-Wire-Protocol

### §4.1 IOR-Generation IIOP/SSLIOP/Components

**Spec:** §4.1 — ein IOR pro Mapping mit `type_id` = `repo_id`,
IIOP-Profil (host/port/object_key) und Components; SSL-Component bei
SSLIOP; `ior_file`; NameService-Bind.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`build_ior`, `ior_lines`, `corba.iiop.advertise_host`).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_dump_iors_writes_stringified_ior`,
`crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`
(omniORB-`catior` liest die IOR, die der Daemon ausgibt; `#[ignore]`, läuft im
Job `integration-bridges-vendor-corba`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB ruft über die IOR aus `ior_file`).

`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_1_the_ior_carries_code_sets_and_the_ssl_component`,
`crates/corba-ior/src/components.rs::tests::a_code_sets_encapsulation_pads_after_the_byte_order_octet`
(Encapsulation richtet ab dem Byte-Order-Oktett aus; die ungepolsterte Form
nannte `catior` „Broken component with tag 1“).

**Status:** partial — `TAG_CODE_SETS` und `TAG_SSL_SEC_TRANS` gebaut,
`catior` liest beide; kein `TAG_ORB_TYPE` (keine OMG-vergebene
ORB-Kennung). Den NameService-Bind prüfen
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`
und `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop`.

### §4.2 GIOP-Frame-Format Header + Body

**Spec:** §4.2 — 12-Byte-Header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR-Body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`
(Header lesen, `max_message_size`, `MessageError`), Codec in
`crates/corba-giop/`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-giop/src/header.rs::{magic_bytes_match_spec,
header_size_is_12_bytes, round_trip_big_endian_request_header}`,
`crates/corba-giop/src/message_type.rs::message_type_values_match_spec_table_15_1`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** done

### §4.3 Request-Reply-Mapping CORBA→DDS

**Spec:** §4.3 — GIOP-Request → Mapping-Lookup → DDS-Request-Sample
(`u64 request_id` + Argumente, XCDR1 in GIOP-Byte-Reihenfolge) →
korrelierter Reply-Sample (`request_id`, `reply_status`, `reserved`,
Body) → GIOP-Reply; `TIMEOUT`, `OBJECT_NOT_EXIST`, `BAD_OPERATION`,
`NO_PERMISSION`, `NO_IMPLEMENT` (GIOP < 1.2), `TRANSIENT`, `MARSHAL`
(auch für ein Reply-Sample, das kein einfaches CDR ist); `CancelRequest`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`, `plain_cdr`, `timeout_loop`, `system_exception_body`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB 4.3.3 auf dem Linux-Testhost: `add` → 42 über ein DDS-Backend, `never` →
`TIMEOUT`, falsche Operation → `BAD_OPERATION`),
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_3_a_system_exception_body_is_the_giop_layout`,
`crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_3_a_reply_sample_that_is_not_plain_cdr_is_marshal`
(ein Reply-Sample unter `PL_CDR_LE` wird `MARSHAL`), `NO_PERMISSION` in
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one`.

**Status:** partial — `NO_IMPLEMENT`, `TRANSIENT` bei DDS-Schreibfehler und
`CancelRequest` sind gebaut, aber ohne eigenen Test; `correlation_id` außer
`request_id` wird abgelehnt.

### §4.4 Notify-Mapping DDS→CORBA

**Spec:** §4.4 — DDS-Sample auf `notify_topic` → oneway GIOP-1.2-Request
an jedes Ziel in `targets` (`corbaloc:` oder IOR); ein Sample, das kein
einfaches CDR ist (XCDR2, `PL_CDR`), verworfen und gezählt; Neuaufbau nach
Fehler.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`Notifier`, `resolve_target`).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(drei DDS-Samples erreichen einen omniORB-Servant als oneway-Aufruf on_value),
`crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_4_4_corbaloc_and_ior_targets_resolve`,
`crates/corba-dds-bridge/tests/plain_cdr_e2e.rs::corbabridge_4_4_a_notify_sample_that_is_not_plain_cdr_is_not_sent`
(ein Sample unter `PL_CDR_LE` geht nicht hinaus).

**Status:** done

### §4.5 Object-Key-Generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::object_key`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`
(der Test adressiert die Mapping-Operation über `object_key`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(omniORB adressiert über den Key der IOR).

**Status:** done

### §4.6 Fragment-Handling GIOP 1.1+

**Spec:** §4.6 — Nachrichten über `fragment_size` als Fragmente, jedes
außer dem letzten auf einer 8-Oktett-Grenze der Nachricht endend;
Reassembly inbound.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`write_giop`, `Reassembly`).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries,
corbabridge_4_6_a_fragmented_message_is_put_back_together}`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(`echo` über 200 000 Byte, `fragment_size` 16384, in beide Richtungen
fragmentiert, gegen omniORB 4.3.3).

**Status:** done

### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` mit `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD`.

**Repo:** —

**Tests:** —

**Status:** open — `OBJECT_HERE` und `UNKNOWN_OBJECT` sind im Daemon gebaut (`crates/corba-dds-bridge/src/bridge_daemon.rs::serve_connection`, Zweig `LocateRequest`), aber ohne Test; `OBJECT_FORWARD` ist nicht gebaut.

## §5 Topic-Mapping

### §5.1 DDS-Topic-Slug per Operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` als Default; Override per `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::slug`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_5_1_the_slug_of_an_operation`.

**Status:** done

### §5.2 Type-Discovery via idl-rust Codegen

**Spec:** §5.2 — pro Mapping ein generierter Request-/Reply-Typ mit
Exception-Variante; IDL in `/var/lib/zerodds/corba-bridge/`. Gebaut:
das feste Layout aus §4.3.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`on_request`, `on_reply`: festes Layout).

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`
(das DDS-Backend deklariert Request- und Reply-Typ nach dem Layout).

**Status:** partial — die IDL-Generierung pro Mapping und die
Exception-Variante im Reply-Typ sind nicht gebaut; die Bridge kennt den
IDL-Typ nicht und reicht die Argument-Bytes durch.

## §6 QoS-Translation

### §6 DDS-QoS → CORBA-Behavior Map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/
Partition; `BEST_EFFORT` nur für Notify.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs::read_mapping`
(`qos.reliability`, `qos.durability`; `best_effort` für Request-Reply
abgelehnt).

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_3_what_the_daemon_does_not_build_is_refused}`.

**Status:** partial — Reliability und Durability gehen an die
DDS-Entities; ein Replay pro Request-Hash für `TRANSIENT_LOCAL`,
Lifespan, Deadline und Partition sind nicht gebaut.

## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL-Component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` aktiviert TLS auf
`ssliop.bind`, `require_client_cert` gegen `ca_file`; SSL-Component im
IOR.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`TlsShared`, `accept_loop`),
`crates/corba-dds-bridge/src/daemon_runtime.rs::build_corba_tls_config`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs::{corbabridge_7_1_build_corba_tls_config_loads_self_signed,
corbabridge_7_1_build_corba_tls_config_with_mtls_succeeds,
corbabridge_7_1_build_corba_tls_config_rejects_missing_cert}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_2_a_certificate_that_does_not_load_is_exit_4`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(omniORB 4.3.3 mit `libomnisslTP4`, `clientTransportRule * ssl`, Port aus
`TAG_SSL_SEC_TRANS`, Client-Zertifikat verlangt; Linux-Testhost).

**Status:** done

### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP-User/Pass-Fallback.

**Repo:** —

**Tests:** —

**Status:** open — CSIv2 ist im Daemon nicht gebaut; `csiv2.enabled: true` wird abgelehnt (`crates/corba-dds-bridge/src/bridge_daemon.rs`, Test `corbabridge_3_what_the_daemon_does_not_build_is_refused`). Die Bausteine `crates/corba-csiv2/` und `crates/corba-dds-bridge/src/csiv2_wire.rs` sind nicht an den Daemon angebunden.

### §7.3 ACL pro Mapping

**Spec:** §7.3 — Subject aus dem Subject-DN des Client-Zertifikats;
Regel mit passendem Subject führt die `repo_id` in `allow_invoke`
(Präfix mit `*`); sonst `default_deny`; verweigert: `NO_PERMISSION`.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`subject_allowed`, `on_request`),
`crates/bridge-security/src/ctx.rs::extract_mtls_subject`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes`.

`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_7_1_omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only`
(Zertifikat A passiert und läuft in `TIMEOUT`, Zertifikat B derselben CA
bekommt `NO_PERMISSION`; omniORB 4.3.3, Linux-Testhost).

Das Subject ist der Subject-DN nach RFC 4514
(`mtls_subject_from_der` in `crates/bridge-security/src/ctx.rs`, Test
`crates/bridge-security/src/ctx.rs::tests::corbabridge_7_3_the_mtls_subject_is_the_dn_and_the_acl_also_takes_the_fingerprint`, gegen
`openssl x509 -nameopt RFC2253` geprüft); eine Regel darf stattdessen den
Fingerprint `mtls:<SHA-256>` nennen (so der omniORB-Test). Der
GSSUP-Username aus §7.2 hängt an CSIv2 (§7.2, open).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log mit `timestamp`, `level`, `event` und den
Feldern des Ereignisses; `--log-level`.

**Repo:** —

**Tests:** —

**Status:** open — `crates/corba-dds-bridge/src/bridge_daemon.rs` (`log`, `event_level`, `level_rank`) schreibt eine JSON-Zeile pro Ereignis auf stderr mit `level`, `event` und den Ereignisfeldern, gefiltert nach `log_level`; `timestamp` fehlt, und kein Test prüft die Form einer Zeile.

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — zehn Familien `zerodds_corba_bridge_*`
(requests/replies/pending/latency/timeouts/…).

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs`
(`BridgeMetrics`, `serve_admin_endpoints`),
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(`metrics_addr_for`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::{corbabridge_8_2_metrics_register_idempotent,
corbabridge_8_2_admin_endpoint_serves_metrics_path}`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_8_2_the_metrics_of_section_8_2_count_a_timed_out_request`
(über `--metrics`: ein Request ohne Backend zählt als `system_exception`,
Timeout, gesendeter Reply, veröffentlichtes Sample; LocateRequest; ohne die
Zählung im Timeout-Pfad rot).

**Status:** done — die Familien aus §8.2 außer `csiv2_failures_total`
(CSIv2 nicht gebaut, §7.2).

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` → Spans pro
GIOP-Exchange (`giop.request`, `dds.publish`, `dds.subscribe`,
`giop.reply`).

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs`
(`otlp_config_from_env`, `spawn_otlp_flush_loop`).

**Tests:** `crates/corba-dds-bridge/src/daemon_runtime.rs::tests::corbabridge_8_3_otlp_config_parses_endpoint`.

**Status:** partial — der Exporter wird aus der Umgebung gestartet,
aber `bridge_daemon.rs` erzeugt keine Spans.

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer pro Mapping →
IIOP/SSLIOP-Bind → NameService-Rebind → Signal-Handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/corba-dds-bridge/src/bridge_daemon.rs::run`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_9_1_the_port_is_bound_before_an_ior_is_published`
(Bind vor dem Veröffentlichen der IORs; vorher schrieb der Daemon die IOR-Datei und
den NameService-Eintrag vor dem Bind, ein TAO-Client bekam darauf `TRANSIENT`),
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_10_the_bridge_rebinds_into_omninames_and_unbinds_on_stop`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Drain max. 30 s: neue Requests `TRANSIENT`, offene
bekommen Reply oder `TIMEOUT`, der Rest nach 30 s `TRANSIENT`; dann
`CloseConnection`; SIGHUP lädt TLS und ACL neu.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`
(`drain`, `DRAIN_LIMIT`, `Conn`),
`crates/corba-dds-bridge/src/daemon_runtime.rs::install_signal_watcher`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection`
(Request während des Drains → `TRANSIENT`, offener Request →
`TIMEOUT`, dann `CloseConnection` und EOF; ohne `drain` rot).

`crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_9_2_a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one,
corbabridge_9_2_a_sighup_serves_new_connections_with_the_new_certificate}` (SIGHUP an das
Binary; ohne `apply_reload` beide rot).

**Status:** partial — der Zweig nach 30 s (`drain_expired`) hat keinen
Test.

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

**Status:** partial — gegen omniORB, JacORB und TAO gemessen; Ice-Java
nicht.

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

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::corbabridge_3_the_shipped_example_reads_as_it_says`
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

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon.rs`,
`crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-ior/`.

**Tests:** `crates/corba-dds-bridge/src/bridge_daemon.rs::tests::{corbabridge_3_the_shipped_example_reads_as_it_says,
corbabridge_5_1_the_slug_of_an_operation, corbabridge_3_what_the_daemon_does_not_build_is_refused,
corbabridge_4_3_a_system_exception_body_is_the_giop_layout,
corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries,
corbabridge_4_6_a_fragmented_message_is_put_back_together,
corbabridge_4_4_corbaloc_and_ior_targets_resolve,
corbabridge_7_3_acl_rules_match_subjects_and_repo_id_prefixes}`.

**Status:** partial — der Daemon hat acht Unit-Tests in einem Modul;
die Aufteilung der Klausel mit ≥ 5 Tests pro Modul gibt es nicht, und
`csiv2` ist nicht gebaut.

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Daemon, ein fremder CORBA-Client, ein
DDS-Service-Prozess, Roundtrip über die Bridge.

**Repo:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::{corbabridge_4_2_a_request_to_an_unknown_object_is_object_not_exist,
corbabridge_4_5_the_shutdown_answers_pending_requests_then_closes_the_connection,
corbabridge_2_a_certificate_that_does_not_load_is_exit_4,
corbabridge_2_dump_iors_writes_stringified_ior}`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + TAO-Client + ZeroDDS-
CORBA-Bridge im Compose.

**Repo:** `crates/corba-dds-bridge/tests/cross_vendor.rs`.

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs::corbabridge_4_1_catior_reads_the_ior_the_daemon_prints`
(`#[ignore]`, Job `integration-bridges-vendor-corba`); TAO-Client:
`crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`
(Job `vendor-live-corba-tao`).

**Status:** partial — die DDS-Seite hinter der Bridge ist ZeroDDS; ein
Cyclone-DDS-Backend ist nicht gemessen. Cross-Ref: das Item darunter.

### §10/§12 Cross-Vendor-Harness — TAO, JacORB, Ice-Java, Cyclone

**Spec:** §10 + §12.3 — die CORBA-Seite gegen TAO, JacORB und
Ice-Java; ein Cyclone-DDS-Backend hinter der Bridge.

**Repo:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs`, `crates/corba-dds-bridge/tests/tao_e2e.rs`, `crates/corba-dds-bridge/tests/omniorb_e2e.rs`.

**Tests:** `crates/corba-dds-bridge/tests/jacorb_e2e.rs::corbabridge_10_jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant`,
`crates/corba-dds-bridge/tests/tao_e2e.rs::corbabridge_10_tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant`,
`crates/corba-dds-bridge/tests/omniorb_e2e.rs::corbabridge_4_1_omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant`.

**Status:** partial — omniORB, JacORB und TAO sind gemessen (§10);
Ice-Java und ein Cyclone-DDS-Backend nach dem Layout aus §4.3 nicht.

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

12 done / 11 partial / 3 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-corba-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (Job `merge-rust-bridge-security`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (Job `merge-rust-config`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-corba-dds-bridge --all-targets` (Job `merge-rust-corba-dds-bridge`) — 29 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (Job `merge-rust-corba-giop`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (Job `merge-rust-corba-ior`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 25 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-corba` (Job `integration-bridges-vendor-corba`) — 9 Tests grün.
* Job `vendor-live-corba-tao` — 2 Tests grün.

72 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-corba-bridge-1.0.open.md`.
