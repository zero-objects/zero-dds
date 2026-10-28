# `zerodds-opcua-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-opcua-bridge-1.0.md`

Implementation:

- `crates/opcua-bridge/` — the daemon process (`zerodds-opcua-bridged`) over
  the four library crates `opcua-gateway`, `opcua-uacp`, `opcua-server`,
  `opcua-pubsub`.

## §1 Conformance levels

### §1 Four Gateway 1.0 conformance points declared

**Spec:** §1/§2 — four conformance points (OPC UA→DDS basic/complete,
DDS→OPC UA basic/complete).

**Repo:** `crates/opcua-gateway/src/conformance.rs::DECLARED_CONFORMANCE`
(library level, unchanged); the daemon wires §9 (DDS→OPC UA), §8 (both
directions), Part 14, and §9.3.4 (historical, conformance point 4
"complete") via `opcua.server.historical.topics`.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::*` (library);
`crates/opcua-server/src/client.rs::tests::e2e_loopback_history_read_*`
(HistoryRead service); `daemon_e2e.rs::
historical_topic_config_makes_history_read_work_end_to_end`.

**Status:** done — all four conformance points, including Historical
Access, are now wired into the daemon.

## §2 CLI surface

### §2 zerodds-opcua-bridged CLI

**Spec:** §2 — `--config`/`--check-config`/`--set`/`--bind`/`--domain`/
`--idl`/`--gateway-config`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`,
`crates/opcua-bridge/src/daemon/cli.rs`.

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::*` (8 tests:
flag parsing, `=` form, `--idl` repetition, `sets()` rendering).

**Status:** done

## §3 Config-file format

### §3 Layered YAML loader

**Spec:** §3 — `domain`/`log_level`/`opcua.{idl,gateway_config,server,
client,pubsub}`/`acl`/`metrics`; an unknown key inside a known section
stops the start with file and line.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{DaemonConfig,
KEYS, is_key, from_config}`.

**Tests:** `crates/opcua-bridge/src/daemon/config.rs::tests::*` (20 tests:
minimal config, unknown section, client/PubSub entries, ACL rules, ACL
users, a secured policy without a certificate, historical topics,
PubSub security keys, OTLP endpoint).

**Status:** done — the config surface grew by
`opcua.server.historical.topics`, `opcua.pubsub.connections[].security`,
`acl.users`/`acl.allow_anonymous`, and `metrics.otlp_endpoint`.

## §4 Wire protocols

### §4.1 opc.tcp + SecureChannel

**Spec:** §4.1 — Part 6 §7.1/§6.7, SecurityMode None always, secured
policies under the `crypto` build feature.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{accept_loop,
serve_connection_locked, build_server_security}` — one
`Arc<Mutex<Server>>` shared by every connection (Part 4 §5.6.2: several
concurrent SecureChannels, Subscription transfer).

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
dcps_writer_reaches_the_opcua_client_read`,
`::opcua_client_write_reaches_the_dds_reader` (both over a real `opc.tcp`
socket, `zerodds_opcua_server::client::TcpTransport`).

**Status:** done — including secured policies:
`daemon_e2e.rs::secured_basic256sha256_sign_and_encrypt_session_
through_the_daemon` generates real RSA-signed X.509 certificates
(`rcgen`, feature `crypto`) and drives a real Basic256Sha256
SignAndEncrypt handshake against the running daemon; before this there
was no test for it, only the library-level test using
`b"client-cert-der"` placeholder bytes.

### §4.2 Part 4 services

**Spec:** §4.2 — Session/Attribute/View/Method/Subscription/HistoryRead,
as `zerodds-opcua-server` implements them, plus a Write hook.

**Repo:** `zerodds-opcua-server::server::{Server::set_write_hook,
handle_service}` — a new hook offering every `Write` target to a
caller-installed closure, with the calling session's identity, before it
reaches the AddressSpace; the daemon installs one
(`daemon/server.rs::start_with_reload`) for the per-session ACL (§7.2).
Otherwise the library, unchanged; the daemon supplies transport +
AddressSpace.

**Tests:** see §4.1; `zerodds-opcua-server::client::tests::
write_hook_can_refuse_a_write_before_it_reaches_the_address_space`,
`::write_hook_observes_the_calling_sessions_identity`;
`daemon_e2e.rs::per_session_write_acl_allows_the_named_subject_and_
refuses_the_other`.

**Status:** done (library level 68 tests with feature `crypto`, see
`opcua-client-server-1.05.md`).

### §4.3 Part 14 UADP

**Spec:** §4.3 — one Publisher/Subscriber pair per
`opcua.pubsub.connections[]`, UDP multicast.

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security}`; feature `pubsub-security` (passthrough to
`zerodds-opcua-pubsub/security`); `daemon/config.rs::
PubSubSecurityConfig`; `daemon/server.rs::build_pubsub_security`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::*` (4, 2 of
them feature-`pubsub-security`:
`secured_publisher_and_subscriber_round_trip_a_dds_sample`,
`subscriber_with_the_wrong_key_refuses_not_silently_drops`);
`crates/opcua-bridge/tests/daemon_e2e.rs::
dds_publish_reaches_an_in_tree_pubsub_subscriber` (real UDP multicast).

**Status:** done — PubSub message security (`SecurityGroup`/SKS) is
wired via `opcua.pubsub.connections[].security`: a single static current
key, not a live `GetSecurityKeys`/SKS rotation exchange (named, not
hidden).

## §5 Topic mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — type resolution without a compiled Rust type.

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; the opportunistic path
`crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::*` (3).

**Status:** done (configured-IDL path, deterministically tested);
`resolve_discovered` is **unmeasured** — no test drives a real
cross-participant `getTypes` reply into the registry, see §6.2 of the spec
file.

### §5.2 AddressSpace shape (§9.3)

**Spec:** §5.2 — Domain→Topic→Sample tree.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{ensure_domain,
ensure_topic, ensure_objects_folder, materialize_instance,
materialize_instance_with_override, ensure_browse_path, apply_sample}`.

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::*`
(13 tests, incl. `dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable`,
`node_id_override_replaces_the_default_root_identity`,
`node_id_override_colliding_with_an_existing_node_is_refused`,
`browse_path_organizes_the_root_from_the_objects_folder`,
`objects_folder_is_browsable_after_materializing_a_topic`);
`daemon_e2e.rs::node_id_and_browse_path_override_is_applied_not_just_
parsed`, `::colliding_node_id_override_is_a_load_time_error_not_a_log_
line`.

**Status:** done — the `<browse_path>`/`<node_id>` override from the
Spec §10 XML is now applied: a `<node_id>` replaces the connection's
sample root's identity (checked against every existing node first — a
collision is a load-time error, the daemon never starts under it); a
`<browse_path>` adds an `Organizes` reference chain from the well-known
`Objects` folder (ns0;i=85) to the same root. Found and fixed along the
way: the `Objects` folder itself had no `NodeMeta` entry, so a Browse
starting from the standard entry point always answered
`Bad_NodeIdUnknown`.

### §5.3 Type-shape coverage

**Spec:** §5.3 — which DDS-XTypes shapes get a live value.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::*` (10
tests, incl. `sequence_of_struct_is_reported_skipped_not_dropped_
silently`, `wide_char_maps_to_uint16_and_back`,
`wide_string_maps_to_string_and_back`,
`map_of_primitives_decomposes_to_one_leaf_per_key_and_back`,
`map_partial_write_keeps_the_untouched_key`).

**Status:** partial — primitives/strings (narrow and wide: `wchar`→
`UInt16`, `wstring`→`String`)/enums/bitmasks/nested structs/`map<K,V>`
(one leaf per entry, path `<field>.<key>`)/arrays-sequences of any of the
above: done. An array/sequence of a struct/union, `bitset`, `fixed<P,S>`,
`float128`: open, named rather than approximated — no OPC-UA builtin
`VariantValue` case fits any of the three (Part 6 §5.1.2's type table has
no row for any of them; inferred from that absence, not a specific
Gateway 1.0 §9.2 clause this crate has the text of). Named, not silent: a
`map` key a write introduces that no prior sample ever had is reported
as an unmatched path, not materialized live as a new node
(`AddressSpace`/`PublishedDataSet` build their node/field set once, from
the Table-9 default).

### §5.4 Historical access (§9.3.4)

**Spec:** §9.3.4.4 — `Historizing`/`AccessLevel` HistoryRead bit,
HistoryRead service.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{HistoricalConfig,
HistoricalTopicConfig}`; `address_space_build.rs::materialize_root`
(historizing marking); `zerodds-opcua-server::server::Server::
history_read`; `zerodds-opcua-gateway::historical::adapter::
InMemoryHistoryCache::push_bounded`.

**Tests:** `zerodds-opcua-server::client::tests::e2e_loopback_history_
read_*` (6 tests); `zerodds-opcua-server::services::tests::history_
read_*_round_trips` (2); `zerodds-opcua-gateway::historical::adapter::
tests::push_bounded_evicts_the_oldest_sample_past_max_depth`,
`::ensure_stream_makes_an_empty_read_ok_not_unknown_instance`;
`daemon_e2e.rs::historical_topic_config_makes_history_read_work_end_
to_end`.

**Status:** done — Raw range and AtTime, real pagination via an opaque
continuation point; `ReadRawModifiedDetails.is_read_modified == true` is
refused (DDS has no "modified value history"), never silently answered
as a plain read. Found and fixed along the way: the pump loop previously
passed a constant `0` as every sample's `source_timestamp`, which
collapsed every node's retention to one sample.

## §6 OPC UA ↔ DDS bridging behavior

### §6.1 DDS → OPC UA

**Spec:** §6.1.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`dds_to_ua_jobs` part).

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
dcps_writer_reaches_the_opcua_client_read`.

**Status:** done

### §6.2 OPC UA → DDS

**Spec:** §6.2 — two mechanisms: poll-based diff detection for writes
against this daemon's own server; the client role against configured
foreign servers.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`ua_to_dds_jobs` part); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
opcua_client_write_reaches_the_dds_reader`;
`crates/opcua-bridge/src/ua_client.rs::tests::
client_job_writes_a_monitored_scalar_node_into_dds`.

**Status:** done — the Write hook (§4.2) now checks `acl.rules[].
subject` synchronously per session, before a write ever reaches the
AddressSpace; the narrower limitation named in the spec file's §6.2
remains for an *already-authorized* write (still poll-based DDS
forwarding, still no error channel back to the client when the DDS write
itself fails) — unchanged in this pass, not fixed.

## §7 Security

### §7.1 SecureChannel material loading

**Spec:** §7.1 — certificate/key files, two separate peer files.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{load_private_key_pem,
load_public_key_pem, load_der}`.

**Tests:** `daemon_e2e.rs::secured_basic256sha256_sign_and_encrypt_
session_through_the_daemon` — real RSA-signed X.509 certificates
(`rcgen`), loaded through `load_der`/`load_private_key_pem`/
`load_public_key_pem`, a real Basic256Sha256 SignAndEncrypt handshake
against the running daemon.

**Status:** done — the config-validation part (pair required) remains
tested (`config.rs::tests::secured_policy_without_certificate_is_
refused`); loading real PEM/DER files and the full handshake are now
also tested, previously untested.

### §7.2 ACL

**Spec:** §7.2 — per-topic allow list, plus the subject dimension per
session.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{Acl, AclOp,
topic_exposed, allow_write, client_subject}`;
`daemon/config.rs::{AclUserConfig, acl_users, acl_allow_anonymous}`;
`daemon/server.rs::start_with_reload` (Write-hook installation,
`UserStore` construction).

**Tests:** `crates/opcua-bridge/src/daemon/security.rs::tests::*` (6
tests, incl. `client_subject_renders_each_identity_kind`);
`daemon/config.rs::tests::acl_users_parse_with_default_policy_id`,
`::acl_user_without_password_is_refused`; `daemon_e2e.rs::
per_session_write_acl_allows_the_named_subject_and_refuses_the_other`
(two real sessions, alice allowed, bob not — over real TCP + real DDS).

**Status:** done — both layers enforced: topic exposure at config-load
time *and*, for a Write against this daemon's own server, per session
via the Write hook. `acl.users`/`acl.allow_anonymous` give the
`UserStore` real, distinguishable identities — without them every
session was Anonymous and `acl.rules[].subject` had nothing to differ on
for the Write hook.

## §8 Operations + observability

### §8.1 Logging

**Spec:** §8.1.

**Repo:** consistently `eprintln!` with topic/path names throughout
`crates/opcua-bridge/src/daemon/server.rs`.

**Tests:** — (verified manually; no test parses stderr).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics,
serve_metrics}`.

**Tests:** verified manually (`curl /metrics` against a running process
during development); no automated test reads the endpoint.

**Status:** partial — the endpoint works (measured), no automated test.

### §8.3 OTLP spans

**Spec:** §8.3.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{otlp_config_from_
endpoint, emit_lifecycle_span, new_trace_id, new_span_id}`;
`daemon/config.rs::metrics_otlp_endpoint`.

**Tests:** `daemon_e2e.rs::otlp_endpoint_receives_a_span_on_opc_tcp_
accept` (a real fake OTLP/HTTP collector receives the running daemon's
real POST on a real `opc.tcp` accept).

**Status:** done — one span (`zerodds-opcua-bridged.opc_tcp.accept`,
`SpanKind::Server`) per accepted `opc.tcp` connection, flushed every 5
seconds and once more on shutdown. Coarse-grained, matching
`zerodds_foundation::tracing`'s own design line ("endpoint lifecycle...
not per-sample latencies") — no per-sample spans.

## §9 Lifecycle

### §9.1/§9.2 Startup + shutdown

**Spec:** §9.1/§9.2.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload,
DaemonHandle::shutdown}`.

**Tests:** measured manually (SIGTERM against a running process, exit
code 0, see the commit history); `crates/opcua-bridge/tests/daemon_e2e.rs`'s tests call
`handle.shutdown()` themselves at the end.

**Status:** done — a real bug was found and fixed in the process:
`zerodds_monitor::server::serve_prometheus`'s accept loop had no shutdown
path (blocking `accept()`, no stop flag, no externally reachable listener
handle to drop); replaced with `serve_metrics`, a non-blocking accept
polled against the same stop flag. A second bug was found the same way:
`DaemonHandle::shutdown` tried to join the signal-watching thread, which
only exits on an actually received signal — never delivered when
`shutdown` is called other than from a real SIGTERM/SIGINT/SIGHUP (every
in-process test in this crate calls it directly); fixed by raising a
self-SIGTERM (`signal_hook::low_level::raise`) inside `shutdown`.

### §9.3 SIGHUP

**Spec:** §9.3 — only `acl.default_deny`/`acl.rules` is reloaded.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs` (the reload thread in
`start_with_reload`).

**Tests:** — (no automated test sends SIGHUP).

**Status:** partial — the code path exists, not automated-tested;
reload scope deliberately not widened: `opcua.server.historical.topics`,
`acl.users`/`acl.allow_anonymous`, `opcua.pubsub.connections[].security`
and `metrics.otlp_endpoint` all need a restart, same as `<node_id>`/
`<browse_path>` and the IDL/gateway-config topic set already did.

## §10 Cross-vendor interop

**Spec:** §10 — open62541 1.5.4.

**Repo:** the README points at `crates/opcua-server/proofs/open62541.sh` +
`crates/opcua-pubsub/proofs/open62541_uadp.sh` (library level);
`crates/opcua-bridge/tests/open62541_interop.rs` +
`tests/foreign/{open62541_client.c,README.md}` for the daemon level: a
small C client against `libopen62541`'s public API (open62541 ships no
generic Browse/Read/Write CLI comparable to `mosquitto_pub`/
`coap-client`), Browse/Read/Write against a real running
`zerodds-opcua-bridged`.

**Tests:** `open62541_interop.rs::open62541_browses_reads_and_writes_through_the_daemon`
— `#[ignore]`d; `OPCUA_OPEN62541=1` set with
open62541 not found is a hard failure (never a silent pass), matching
`crates/coap-bridge/tests/libcoap_interop_e2e.rs`'s own convention. Not
run by the agent that wrote it (no open62541 install on this host,
macOS rather than Linux) — the skip path and the hard-failure-without-
the-tool path are measured; the actual handshake against a real
open62541 is unmeasured.

**Status:** partial — the daemon-level test exists and is structurally
verified (skip/hard-failure paths), but has not been run against a real
open62541; the library level remains measured
(`crates/opcua-server/proofs/open62541.sh`).

## §11 Packaging

**Spec:** §11.

**Repo:** `packaging/linux/systemd/zerodds-opcua-bridged.service`,
`packaging/linux/configs/opcua-bridged.yaml.example`,
`man/man1/zerodds-opcua-bridged.1`,
`man/man5/zerodds-opcua-bridged.yaml.5`, entries in
`packaging/linux/{deb/assemble-debs.sh,rpm/zerodds.spec,arch/PKGBUILD,
appimage/build.sh}`.

**Tests:** —

**Status:** done

## §12 Testing

**Spec:** §12.

**Repo:** see §1–§9 of this file for the per-item coverage.

**Tests:** `cargo test -p zerodds-opcua-bridge --all-features` — 76 lib
tests + 10 integration tests (`crates/opcua-bridge/tests/daemon_e2e.rs`, one of them feature-
`crypto`) + 1 (`crates/opcua-bridge/tests/example_yaml_loadback.rs`) + 1 `#[ignore]`d
(`open62541_interop.rs`, §10). Without `pubsub-security`: 74 lib tests (2
feature-gated). `crates/opcua-bridge/tests/daemon_e2e.rs` is measured to be flaky under `cargo
test`'s default parallelism (every test passes individually and with
`--test-threads=1`) — the same DDS-domain-lease property as before, not
a regression.

A finding measured and fixed on Linux: every UDP multicast *receiver*
socket in this crate (the daemon's own `subscribers[]`-configured
`SubscriberJob` *and* the test's own hand-built verification
`Subscriber`) bound to `local_addr`'s own, usually random port (default
`0.0.0.0:0`) instead of the multicast destination's port — UDP
demultiplexes by destination (ip, port) at the kernel, so a socket
bound to an arbitrary port never receives a datagram addressed to the
real one. This mistake happened to still work on macOS (not a portable
guarantee); measured as an outright failure on Linux. Fixed
(`daemon/server.rs`'s subscriber loop now binds
`SocketAddr::new(local.ip(), multicast.port())`); the new test
`external_pubsub_publisher_reaches_the_daemons_own_subscriber` covers
the daemon's own subscriber-role code path, previously untested by
anything.

**Status:** done (unit + in-process e2e, including secured SecureChannel,
Historical Access, PubSub security, OTLP, per-session ACL); multi-vendor
automation partial (§10 — the test exists, `#[ignore]`d, not run against
real open62541).

## §13/§14 Cross-references + versioning

**Status:** n/a (informative)

## Audit status

18 done / 4 partial / 0 open / 2 n/a (informative, §13+§14). Round 2
closed all three previously-open items: §5.3 (now partial rather than
open — `wchar`/`wstring`/`map` mapped, only an array of an aggregate/
`bitset`/`fixed<P,S>`/`float128` are left without a live value), §8.3
(OTLP, now done) and §10 (an open62541 daemon-level test now exists, now
partial rather than open — not run against real open62541).
