# Bridges, CORBA, OPC UA, SOAP

German original: [crates-bridges-services.md](crates-bridges-services.md)

This page catalogs the protocol bridges (six daemons and their codec crates), the shared security layer `zerodds-bridge-security`, the CORBA crate family, the OPC UA crates and `zerodds-soap`. It is based on the code under `crates/` and `tools/amqp-dds-endpoint/`. Statements that come only from a doc comment are marked "(per doc comment)". Not checked: whether the tests pass; nothing was built or run.

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Overview of the daemons

| Daemon | Crate | Binary | Config file | Spec | man1 | man5 |
|---|---|---|---|---|---|---|
| WebSocket | `crates/websocket-bridge` (`zerodds-websocket-bridge`) | `zerodds-ws-bridged` | `ws-bridged.yaml` + `ws-bridged.d/` | `docs/specs/zerodds-ws-bridge-1.0.md` | `man/man1/zerodds-ws-bridged.1` | `man/man5/zerodds-ws-bridged.yaml.5` |
| MQTT | `crates/mqtt-bridge` (`zerodds-mqtt-bridge`) | `zerodds-mqtt-bridged` | `mqtt-bridged.yaml` + `mqtt-bridged.d/` | `docs/specs/zerodds-mqtt-bridge-1.0.md` | `man/man1/zerodds-mqtt-bridged.1` | `man/man5/zerodds-mqtt-bridged.yaml.5` |
| CoAP | `crates/coap-bridge` (`zerodds-coap-bridge`) | `zerodds-coap-bridged` | `coap-bridged.yaml` + `coap-bridged.d/` | `docs/specs/zerodds-coap-bridge-1.0.md` | `man/man1/zerodds-coap-bridged.1` | `man/man5/zerodds-coap-bridged.yaml.5` |
| AMQP 1.0 | `crates/amqp-endpoint` (`zerodds-amqp-endpoint`) | `zerodds-amqp-bridged` | `amqp-bridged.yaml` + `amqp-bridged.d/` | `docs/specs/zerodds-amqp-bridge-daemon-1.0.md` | `man/man1/zerodds-amqp-bridged.1` | `man/man5/zerodds-amqp-bridged.yaml.5` |
| gRPC | `crates/grpc-bridge` (`zerodds-grpc-bridge`) | `zerodds-grpc-bridged` | `grpc-bridged.yaml` + `grpc-bridged.d/` | `docs/specs/zerodds-grpc-bridge-1.0.md` | `man/man1/zerodds-grpc-bridged.1` | `man/man5/zerodds-grpc-bridged.yaml.5` |
| CORBA | `crates/corba-dds-bridge` (`zerodds-corba-dds-bridge`) | `zerodds-corba-bridged` | `corba-bridged.yaml` + `corba-bridged.d/` | `docs/specs/zerodds-corba-bridge-1.0.md` | `man/man1/zerodds-corba-bridged.1` | `man/man5/zerodds-corba-bridged.yaml.5` |
| AMQP endpoint (server) | `tools/amqp-dds-endpoint` (`amqp-dds-endpoint`) | `amqp-dds-endpoint` | XML file via `--config` | DDS-AMQP 1.0 §2.1, §9.2 | – | – |

The file names come from `CONFIG_STEM` in the respective config code. Example files are under `packaging/linux/configs/<stem>.yaml.example`; each bridge has a test `tests/example_yaml_loadback.rs` or a unit test that passes the example file through the parser.

Coverage per spec section is in `docs/spec-coverage/zerodds-<name>-1.0.md` (DE) and `.en.md` (EN), open items in `…-1.0.open.md`.

### Common pattern of all six daemons (verified in the code)

- **Configuration:** The shared loader `zerodds-config` reads the base file, drop-ins in `<stem>.d/`, includes and `${VAR}`. Precedence: defaults < environment < files < `--set`. Each daemon has a list `KEYS` and `is_key()` for checking unknown keys, plus `DaemonConfig::from_config()`. Within a known section, an unknown key is rejected with file and line (`Section::only`). The DDS runtime keys (`transport.*`, `discovery.*`) may be in the same file.
- **Signals:** `signal-hook` catches `SIGTERM`, `SIGINT` and `SIGHUP` (`install_signal_watcher` in `runtime_common.rs` or `daemon_runtime.rs`). `SIGTERM`/`SIGINT` set a stop flag, `SIGHUP` a reload flag.
- **SIGHUP reload:** The reload rereads the same layered sources (`reload_command_line` in the binary). A configuration that does not load changes nothing ("reload refused"). What is built only at startup stays and is logged as "reload needs a restart for: …". The security context is swapped as a whole via `LiveSecurity::replace`.
- **Admin endpoint:** `metrics.enabled` enables a small HTTP server with `/metrics` (Prometheus), `/catalog` (JSON) and `/healthz`. If `metrics.listen` is empty, each daemon has its own default address: ws `127.0.0.1:9091`, mqtt 9092, coap 9093, amqp 9094, grpc 9095, corba 9096.
- **OTLP:** `otlp_config_from_env` reads `OTEL_EXPORTER_OTLP_ENDPOINT`; a flush thread exports every 5 s (`spawn_otlp_flush_loop`).
- **Interop tests:** Tests against third-party peers are `#[ignore]` and are enabled via the feature `cross-vendor-tests` or environment variables (`MQTT_MOSQUITTO`, `COAP_LIBCOAP`, `AMQP_RABBITMQ`, `GRPC_GRPCURL`, `WS_PYTHON`). Several tests fail when the third-party tool is missing, instead of silently skipping (e.g. `mosquitto_qos.rs`, `grpc-bridge/tests/cross_vendor.rs::require_grpcurl`, `websocat_can_connect_to_zerodds_daemon`). These tests run in no CI; they are run by hand on the Linux test host, together with `open62541_interop` (OPC UA) and the `durability-svc` tests with PostgreSQL and DuckDB.

### `direction` semantics per daemon (verified in the code)

| Daemon | `in` | `out` | Location |
|---|---|---|---|
| ws | DDS writer (WebSocket → DDS) | DDS reader (DDS → WebSocket clients) | `websocket-bridge/src/daemon/server.rs:526-527` (`want_writer` for `in`/`bidir`) |
| mqtt | DDS writer + SUBSCRIBE at the broker (MQTT → DDS) | DDS reader (DDS → MQTT) | `mqtt-bridge/src/daemon/server.rs:762` |
| coap | DDS writer (CoAP POST/PUT → DDS) | DDS reader (DDS → Observe) | `coap-bridge/src/daemon/server.rs:1603` |
| amqp | receiver link + DDS writer (AMQP → DDS) | DDS reader + sender link (DDS → AMQP) | `amqp-endpoint/src/bridge_daemon.rs` module header |
| grpc | DDS writer (`Publish` → DDS) | DDS reader (DDS → `Subscribe`) | `grpc-bridge/src/bin/zerodds-grpc-bridged.rs` `BridgeDds::start` (`bridged_into_dds`, `bridged_out_of_dds`) |
| corba | values `corba_to_dds`, `dds_to_corba`, `bidir` | – | `corba-dds-bridge/src/bridge_daemon.rs:703` |

---

## zerodds-websocket-bridge (`zerodds-ws-bridged`)

**Purpose.** WebSocket stack per RFC 6455 with permessage-deflate (RFC 7692), URI parser and UTF-8 validator as a `no_std + alloc` library; with the feature `daemon`, the daemon that offers DDS topics as WebSocket paths.

**Mapping.** A topic is an HTTP path (`ws_path`, default `/topics/<slug>` from `config::default_ws_path`, `::` → separator, lowercase). Messages are JSON operations (`src/dds_bridge.rs`): `{"op":"subscribe"|"unsubscribe"|"publish", "topic", …}`, the daemon sends `{"op":"notify","topic",…,"data"}`; ACL violations return `{"op":"error","code":403,"reason":"acl-deny-read|acl-deny-write"}` (`src/daemon/server.rs:1037-1095`).

**Entry points.**
- `src/bin/zerodds-ws-bridged.rs` — `main`, `run()` (wait loop on `stop_requested()`, then `shutdown()`).
- `src/daemon/server.rs` — `start`, `start_with_runtime`, `start_with_reload`, `DaemonHandle::shutdown`, `reload()`.
- `src/daemon/config.rs` — `CONFIG_STEM`, `KEYS`, `is_key`, `DaemonConfig::from_config`, `default_ws_path`.
- `src/daemon/security.rs` — `ctx_from_daemon_config` (startup and reload).
- `src/daemon/runtime_common.rs` — metrics, `serve_admin_endpoints`, signal watcher.
- Library: `frame.rs`, `codec.rs`, `handshake.rs`, `negotiation.rs`, `permessage_deflate.rs`, `masking.rs`, `close.rs`, `utf8.rs`, `uri.rs`, `dds_bridge.rs`.

**Features** (`Cargo.toml`): `default = ["std","daemon"]`, `std`, `alloc`, `daemon` (pulls in `zerodds-dcps`, `-config`, `-rtps`, `-qos`, `-types`, `-monitor`, `-observability-otlp`, `-bridge-security`, `rustls`, `signal-hook`), `cross-vendor-tests`.

**Configuration** (`KEYS`): root `listen` (default `127.0.0.1:8080`), `domain`, `log_level`; `tls.{enabled,cert_file,key_file,client_ca_file}`; `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.{public_key,issuer,audience,required_claim}`; `metrics.{enabled,listen}`; `topics[]` with `name`, `type`, `direction` (`in` = client → DDS, `out` = DDS → client, default `bidir`; publish/subscribe against the direction → error frame 404), `ws_path`, `qos.{reliability,durability,history.depth}` (default depth 10), `acl.{read,write}`. A topic may be given as a bare name.

**Security.** TLS (`wss://`) with an optional client CA (mTLS). `auth.mode`: `none|bearer|jwt|mtls`; `sasl` is rejected (comment in `from_config`). ACL per topic from `topics[].acl`.

**SIGHUP.** With the loader: ACL and auth apply immediately to all connections, certificate files are reread for new connections (`RotatingTlsConfig::reload`). A restart is needed for changed TLS paths, `topics`, `listen`, `domain` (`server.rs:1190-1246`).

**Shutdown.** Stop flag, `/healthz` reports unhealthy, a self-connection wakes `accept()`, `router.broadcast_shutdown()` sends the close frame `1001 "going away"` (`server.rs:745`), then all threads are joined.

**Interop tests** (`tests/`):
- `websockets_py_interop_e2e.rs` — Python module `websockets` against the library (text, binary, ping); gate `WS_PYTHON=1`.
- `cross_vendor.rs` — `python_websockets_client_handshakes_and_pings_the_daemon` (Python `websockets` against the daemon), `websocat_can_connect_to_zerodds_daemon` (websocat). `rfc6455_handshake_against_zerodds_daemon_returns_101` uses its own client; the module comment states explicitly that this is not a measurement against a third-party implementation.
- Others: `daemon_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## zerodds-mqtt-bridge (`zerodds-mqtt-bridged`)

**Purpose.** MQTT 5.0 codec (control packets CONNECT to AUTH, properties, VBI; per doc comment), MQTT 3.1.1 (`src/version.rs`, `ProtocolVersion`), topic filters, keep-alive, an in-memory broker (`broker.rs`) and a standalone broker server (`server.rs`, `MqttBrokerServer`); the daemon is an MQTT client to an external broker.

**Mapping.** DDS topic `dds_name` ↔ MQTT topic `mqtt_topic`. For `in|bidir` the daemon sends SUBSCRIBE and writes received PUBLISH messages into DDS; for `out|bidir` it publishes DDS samples with `mqtt_qos` (0/1/2) and `retain`.

**Entry points.**
- `src/bin/zerodds-mqtt-bridged.rs`.
- `src/daemon/server.rs` — `start_with_reload`, `DaemonHandle::shutdown`, `drain()`, `reload()`.
- `src/daemon/client.rs` — broker client; `src/daemon/ws_transport.rs` — transport for `ws://`/`wss://` brokers (uses `zerodds-websocket-bridge`).
- `src/daemon/config.rs` — `KEYS`, `ENV` (`MQTT_PASSWORD` → `mqtt.password`), `is_key` (plus `auth.sasl_users.*`), `BrokerUrl` (schemes `mqtt` 1883, `mqtts` 8883, `ws` 80, `wss` 443).
- Library: `codec.rs`, `control_packets.rs`, `packet.rs`, `properties.rs`, `reason_codes.rs`, `topic_filter.rs`, `keep_alive.rs`, `broker.rs`, `server.rs`, `net.rs`, `dds_bridge.rs`.

**Features:** `default = ["std","daemon"]`, `std`, `alloc`, `daemon` (as for ws, additionally `zerodds-websocket-bridge`), `cross-vendor-tests`.

**Configuration** (`KEYS`): `domain`, `log_level`; `mqtt.{broker_url,client_id,username,password,keep_alive_secs,drain_timeout_secs,clean_start}`, `mqtt.tls.{enabled,ca_file,cert_file,key_file,server_name}`; `auth.{mode,bearer_token,bearer_subject,username,password,sasl_users}`; `acl.{default_deny,rules}` (rule: `subject`, `allow_publish`, `allow_subscribe`); `metrics.{enabled,listen}`; `topics[]` with `dds_name`, `dds_type`, `mqtt_topic`, `direction`, `mqtt_qos`, `retain`, `qos`.

**Security.** TLS outbound to the broker (`mqtts://`, `wss://`), optionally with a client certificate. `auth.mode`: `none|bearer|sasl|sasl_plain|mtls` — the bearer token goes out as the CONNECT password, SASL PLAIN as the CONNECT user/password. ACL by subject; `acl.default_deny` takes effect as a default only when a rule or an auth mode exists; an explicit `true` always blocks (`config.rs`, field `acl_default_deny_set`).

**SIGHUP.** Only the ACL applies immediately; a revoked permission stops the topic's samples in both directions. A restart is needed for `topics`, `mqtt.broker_url/client_id`, `mqtt.tls`, `auth`, `domain`, and for a newly granted read permission (subscriptions are created at startup) (`server.rs:246-250`, `659-710`).

**Shutdown** (§9.2, `server.rs:136-210`): DDS pumps stop, the daemon waits for broker acks for QoS 1/2 (at most `mqtt.drain_timeout_secs`, default 30), open acks are logged, then DISCONNECT with reason `0x00`, then join.

**Interop tests** (third-party peer Eclipse Mosquitto):
- `mosquitto_interop_e2e.rs` — MQTT 3.1.1 against the Mosquitto broker, `mosquitto_pub/sub -V mqttv311`; gate `MQTT_MOSQUITTO`.
- `mosquitto_qos.rs` — QoS 1/2 flows and §9.2 shutdown against Mosquitto; missing binaries make the test fail.
- `websocket_broker.rs` — `ws://`/`wss://` broker schemes against Mosquitto's WebSocket listener.
- `cross_vendor.rs` — daemon against Mosquitto.
- Without a third-party peer: `broker_server_e2e.rs` (own broker, 3.1.1 ⇄ 5.0), `daemon_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## zerodds-coap-bridge (`zerodds-coap-bridged`)

**Purpose.** CoAP per RFC 7252 with Observe (RFC 7641), block-wise transfer (RFC 7959), CoRE Link Format (RFC 6690), multicast, caching proxy, OSCORE (RFC 8613) and DTLS 1.2 as an option; the daemon is a CoAP server over UDP.

**Mapping.** DDS topic ↔ resource path `coap_uri_path`. POST/PUT → DDS write (with Block1 collection, response `2.04 Changed` or `2.31 Continue`); GET with Observe registers an observer, DDS samples are sent as notifications. `/.well-known/core` returns the resource catalog in link format (without auth) (`src/daemon/server.rs:857-974`).

**Entry points.**
- `src/bin/zerodds-coap-bridged.rs`.
- `src/daemon/server.rs` — `start_with_reload`, `dispatch`, `shutdown`, `reload()` (`:1637`), DTLS rotation (`:1464`).
- `src/daemon/oscore_gateway.rs` — OSCORE server role.
- `src/daemon/config.rs` — `KEYS`, `OscoreSettings`, `TopicConfig`.
- Library: `message.rs`, `codec.rs`, `option.rs`, `reliability.rs`, `observe.rs`, `blockwise.rs`, `core_link.rs`, `multicast.rs`, `caching_proxy.rs`, `matching.rs`, `uri.rs`, `dtls.rs`, `dtls_transport.rs`, `oscore/`.

**Features:** `default = ["std","daemon"]`, `std`, `alloc`, `daemon`, `oscore` (hmac/sha2/ccm/aes), `dtls` (webrtc-dtls, tokio; marked EXPERIMENTAL in the Cargo comment, see ADR 0011), `cross-vendor-tests`.

**Configuration** (`KEYS`): `domain`, `log_level`; `coap.{bind,bind_dtls,max_message_size,ack_timeout_ms,max_retransmit,observe_max_age_secs}` (default bind `0.0.0.0:5683`), `coap.dtls.{enabled,cert_file,key_file,ca_file,cipher_suites,psk}` (PSK entries `identity`, `secret_hex`); `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.*`; `acl.{default_deny,rules}` (rule: `subject`, `allow_post`, `allow_observe`); `metrics.{enabled,listen}`; `oscore.{enabled,master_secret_hex,master_salt_hex,id_context_hex,sender_id_hex,replay_window}`; `topics[]` with `dds_name`, `dds_type`, `coap_uri_path`, `direction`, `qos`.

**Security.** DTLS (`coaps://`) only with the feature `dtls`; `coap.dtls.enabled: true` without the feature refuses startup (`server.rs:417-423`). `auth.mode`: `none|bearer|jwt|mtls`; the token is carried in CoAP option 65000, errors result in `4.01`. ACL violation: `4.03`. OSCORE: the daemon is the server, context per client `kid` via HKDF.

**SIGHUP.** With the loader, ACL and auth apply to subsequent requests; DTLS credentials are always reread, existing sessions remain. A restart is needed for `topics`, binds, `domain`, DTLS paths (`server.rs:321-326`, `:1658-1661`).

**Shutdown** (§9.2): no new requests and samples, all observations are ended (`end_observations`), the DTLS thread is notified, UDP receive is woken by a self-addressed datagram, then join.

**Interop tests** (third-party peer libcoap):
- `libcoap_interop_e2e.rs` — libcoap client `coap-client` against a responder built from the project's own codec; gate `COAP_LIBCOAP`.
- `cross_vendor.rs` — libcoap client against the daemon.
- `oscore_rfc8613_vectors.rs` — test vectors from RFC 8613 Appendix C, byte-exact.
- Without a third-party peer: `daemon_e2e.rs`, `daemon_dtls_e2e.rs`, `dtls_coap_e2e.rs`, `daemon_oscore_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## AMQP: zerodds-amqp-bridge, zerodds-amqp-endpoint, zerodds-amqp-0-9-1 (`zerodds-amqp-bridged`)

### zerodds-amqp-bridge

**Purpose.** OASIS AMQP 1.0 codec: type system, frames, performatives, message sections, plus the type mapping of DDS-AMQP 1.0 §7 and the body encodings of §8.1 via the shared AST.
**Entry points:** `src/types.rs`, `src/extended_types.rs`, `src/frame.rs`, `src/performatives.rs`, `src/sections.rs`, `src/codec_profile.rs`, `src/codegen/{plan,encode,emit}.rs`; fuzz targets under `fuzz/fuzz_targets/`, benchmark `benches/decode_hotpaths.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`, `codec-lite` (only a conformance marker per DDS-AMQP §2.4, changes no code path).

### zerodds-amqp-endpoint (contains the daemon)

**Purpose.** Protocol layers above the codec (SASL, session/link, routing, mapping, properties, Annex A configuration) and the bridge daemon `zerodds-amqp-bridged`: a DDS participant on one side, a client of an AMQP 1.0 broker on the other.

**Mapping** (module header `src/bridge_daemon.rs`, verified): per topic `amqp_address` (node address). `out|bidir`: DDS reader + sender link, each sample one message (pass-through body §8.1.1, properties §8.2, `dds:*` application properties §8.3). `in|bidir`: receiver link + DDS writer; settlement `accepted` after the write, `rejected` for an invalid body, `released` on a write error. Without credit, a sample waits in the link queue, bounded by HISTORY (`KEEP_ALL_QUEUE_CAP = 65_536`). Loop protection via `dds:bridge-id` or the writer GUID. Unacknowledged deliveries are resent after a reconnect.

**Entry points.**
- `src/bin/zerodds-amqp-bridged.rs` — `CONFIG_STEM = "amqp-bridged"`, signal watcher, admin server, `run_with_reload`.
- `src/bridge_daemon.rs` — `KEYS`, `ENV` (`AMQP_PASSWORD` → `amqp.sasl.password`), `DaemonConfig::from_config`, `run`, `run_with_reload`, reload (`:1098`), `drain` (`:2208`).
- `src/broker.rs` — `connect` (TCP or TLS with ALPN `amqp`, SASL PLAIN/ANONYMOUS/EXTERNAL/SCRAM-SHA-256/XOAUTH2), `BrokerWriter`.
- `src/daemon_runtime.rs` — metrics, admin endpoints; `src/bridge_security.rs`.
- Protocol modules: `sasl.rs`, `scram.rs`, `session.rs`, `link.rs`, `routing.rs`, `mapping.rs`, `properties.rs`, `dds_bridge.rs`, `management.rs`, `metrics.rs`, `security.rs`, `coexistence.rs`, `rpc_correlation.rs`, `errors.rs`, `limits.rs`, `keyhash.rs`, `annex_a.rs`, `codegen_helpers.rs`, `config_xml.rs`, `client.rs`, `backoff.rs`.

**Features:** `default = ["std"]`, `std` (pulls in the daemon dependencies, `roxmltree`), `alloc`, `cross-vendor-tests`.

**Configuration** (`KEYS`): `domain`, `log_level`; `amqp.{broker_url,container_id,channel_max,max_frame_size,idle_time_out_ms,hostname}`, `amqp.sasl.{mechanism,username,password}` (mechanisms `PLAIN|SCRAM-SHA-256|EXTERNAL|ANONYMOUS|XOAUTH2`, default `ANONYMOUS`), `amqp.tls.{enabled,ca_file,cert_file,key_file,verify_hostname,alpn}`, `amqp.reconnect.{initial_delay_ms,max_delay_ms,factor}`; `acl.{default_deny,rules}` (rule: `topic`, `read`, `write`); `metrics.{enabled,listen,path}`; `catalog.{enabled,address}` (default address `zerodds.bridge.<container-id>.catalog`); `topics[]` with `dds_name`, `dds_type`, `keyed`, `amqp_address`, `direction`, `sender.{settled,durable,snd_settle_mode,rcv_settle_mode}`, `receiver.credit`, `qos`.
Fixed values: `amqp.tls.verify_hostname` only `true`, `metrics.path` only `/metrics`, ALPN fixed to `amqp`.

**Security.** TLS to the broker (`amqps://`), SASL as above. No `auth` section; the ACL names the SASL subject of the bridge.

**SIGHUP.** The ACL applies immediately (a revoked permission stops samples and transfers), TLS material applies from the next connect. A restart is needed for a newly granted permission, `topics`, `amqp.broker_url`, `domain` (`bridge_daemon.rs:1093-1151`).

**Shutdown.** DDS samples are no longer accepted, running transfers are awaited up to `DRAIN_BUDGET = 30 s`, then detach, end, close (`bridge_daemon.rs:1369`, `:1870`, `:2004`).

**Interop tests** (third-party peer RabbitMQ 4.0; gate `AMQP_RABBITMQ`):
- `bridge_rabbitmq_e2e.rs` — daemon against RabbitMQ, `pika` (AMQP 0.9.1) as the reference client.
- `rabbitmq_amqp10_e2e.rs` — `AmqpClient` → RabbitMQ → reference consumer.
- `rabbitmq_cross_stack_e2e.rs` — own AMQP 1.0 stack and own 0.9.1 stack over the same RabbitMQ queue.
- `cross_vendor.rs` — against RabbitMQ (the feature comment in `Cargo.toml` also names qpid; not checked in the test).
- Without a third-party peer: `broker_tls_sasl.rs` (SCRAM-SHA-256 and EXTERNAL against a rustls test server), `security_e2e.rs`, `bridge_e2e.rs`, `e2e_multi_bridge_hop.rs`, `annex_a_idl_roundtrip.rs`, `proptest_state_machine.rs`, `fuzz_smoke.rs`.

### zerodds-amqp-0-9-1

**Purpose.** AMQP 0.9.1 codec (class/method, frames, field tables) and broker client for RabbitMQ.
**Entry points:** `src/client.rs` (`Amqp091Client`), `src/frame.rs`, `src/method.rs`, `src/types.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.
**Integration:** only a dev dependency of `zerodds-amqp-endpoint` for `rabbitmq_cross_stack_e2e.rs`; the daemon does not use it (`crates/amqp-endpoint/Cargo.toml:97-99`).
**Interop test:** `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs` against RabbitMQ 4.0; gate `AMQP_RABBITMQ`.

### tools/amqp-dds-endpoint

**Purpose.** Synchronous AMQP 1.0 server (endpoint profile DDS-AMQP 1.0 §2.1), thread-per-connection, configuration as XML (`zerodds_amqp_endpoint::config_xml::parse_config`).
**Entry points:** `src/main.rs` (`amqp-dds-endpoint [--config <path>] [--listen <addr>]`, default `0.0.0.0:5672`), `src/server.rs`, `src/handler.rs`, `src/frame_io.rs`, `src/auth.rs` (SCRAM server nonce via `zerodds-bridge-security`), `src/tls.rs`, `src/bridge.rs`, `src/dds_host.rs`.
**Features:** `default = ["tls"]`, `tls` (rustls, rustls-pemfile, `zerodds-bridge-security`); the binary requires `tls`.
**DDS side:** The only implementation of `DdsHost` is `InMemoryDdsHost` (`src/dds_host.rs:192`); this crate has no binding to `zerodds-dcps`.
**Tests:** `tests/c1_*` and `c2_*` (connection open, SASL PLAIN rejection, settlement, wildcard addresses, idle timeout, loop prevention, reconnect), `tls_sasl_e2e.rs`, `sasl_server_checks.rs`, `readme_config.rs`. No third-party peer.

---

## gRPC: zerodds-grpc-bridge, zerodds-http2, zerodds-hpack (`zerodds-grpc-bridged`)

### zerodds-grpc-bridge

**Purpose.** gRPC-over-HTTP/2 and gRPC-Web codec (length-prefixed message, path, timeout, status, metadata), server skeleton, reflection (`grpc.reflection.v1alpha`, minimal profile) and the daemon.

**Mapping.** Per topic one gRPC service `grpc_service` (also matched by the last name segment). The methods `Publish` and `PublishOne` write `Sample{bytes payload=1}` as opaque payload into a DDS writer and reply with `PublishAck{accepted}`; `Subscribe` takes one sample per call from the DDS reader (pull; empty body = end of stream) (`src/bin/zerodds-grpc-bridged.rs:1000-1074`). `direction` determines which endpoints are created: `in` = `Publish` → DDS writer, `out` = DDS reader → `Subscribe`, `bidir` both. A call against the direction ends with `FAILED_PRECONDITION`, a failed write with `UNAVAILABLE` (`dispatch`). Service `.proto` generation: `src/service_gen.rs` (package `zerodds.bridge.v1`).

**Entry points.**
- `src/bin/zerodds-grpc-bridged.rs` — `main`, `KEYS` (`:1460`), `is_key`, `from_config`, `BridgeDds::start`, connection loop with GOAWAY, `apply_reload` (`:1672`).
- `src/daemon_runtime.rs` — admin endpoints, signal watcher, `build_grpc_tls_config` (ALPN `h2`).
- Library: `frame.rs`, `path.rs`, `timeout.rs`, `status.rs`, `metadata.rs`, `server.rs`, `reflection.rs`, `service_gen.rs`, `qos_translation.rs`, `bridge_security.rs`.

**Features:** `default = ["std"]`, `std`, `alloc`, `cross-vendor-tests`, `dds-runtime` (empty alias for `std`; the DDS side belongs to `std`).

**Configuration** (`KEYS`): `domain`, `log_level`; `grpc.bind` (default `0.0.0.0:50051`), `grpc.tls.{enabled,cert_file,key_file,client_ca_file,require_client_cert}`, `grpc.reflection.enabled`; `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.*`; `metrics.{enabled,listen}`; `topics[]` with `dds_name`, `grpc_service`, `direction`, `acl.{publish,subscribe}`.

**Security.** TLS with ALPN `h2`, optionally mTLS. `auth.mode`: `none|bearer|jwt|mtls`, checked per RPC via the metadata `authorization` or the client certificate. ACL: `Publish`/`PublishOne` = write, `Subscribe` = read.

**SIGHUP.** TLS material, auth and ACL are replaced; listener, DDS side, topic mapping and TLS on/off keep their startup value and are logged.

**Shutdown.** Each connection checks the stop flag at the interval `CONN_POLL`; a running call finishes first, then GOAWAY with the last stream ID (RFC 7540 §6.8). Idle connections also receive GOAWAY.

**Interop tests:**
- `cross_vendor.rs` — `grpcurl`; gate `GRPC_GRPCURL=1`, a missing `grpcurl` makes the test fail.
- Without a third-party peer: `daemon_security_e2e.rs` (binary with config file, HTTP/2 over TCP/TLS), `security_e2e.rs`, `dds_bridge_e2e.rs`, `bridge_e2e.rs`, `fuzz_smoke.rs`.

### zerodds-http2

**Purpose.** HTTP/2 codec per RFC 9113, `no_std`: framing, stream state machine, flow control, connection preface, settings.
**Entry points:** `src/frame.rs`, `src/stream.rs`, `src/flow.rs`, `src/preface.rs`, `src/settings.rs`, `src/error.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`. Users: `zerodds-grpc-bridge`, `zerodds-conformance`.

### zerodds-hpack

**Purpose.** HPACK per RFC 7541, `no_std`: static and dynamic table, integer, string and Huffman encoding.
**Entry points:** `src/encoder.rs`, `src/decoder.rs`, `src/table.rs`, `src/huffman.rs`, `src/integer.rs`, `src/string.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.

---

## zerodds-corba-dds-bridge (`zerodds-corba-bridged`)

**Purpose.** Bidirectional bridge CORBA object ↔ DDS topic: an IIOP server (and SSLIOP) on one side, a DDS participant on the other.

**Mapping** (module header `src/bridge_daemon.rs`, verified): mapping via `repo_id` + `operation`.
- `corba_to_dds`: GIOP request → sample on `request_topic`; the sample on `reply_topic` with the same `request_id` becomes the GIOP reply. Without a reply within `timeout_ms` → `CORBA::TIMEOUT`.
- `dds_to_corba`: sample on `notify_topic` → oneway GIOP request to each target (`targets`, `target_object_keys`).
- The samples carry the GIOP bytes unchanged (XCDR1, layout `<Operation>Request`/`<Operation>Reply` as `@final struct`). A mapped operation over GIOP 1.0/1.1 is answered with `CORBA::NO_IMPLEMENT`.
- IORs are written to `corba.ior.ior_file`.

**Entry points.**
- `src/bin/zerodds-corba-bridged.rs` — `CONFIG_STEM = "corba-bridged"`, signal watcher, `run_with_reload`.
- `src/bridge_daemon.rs` — `KEYS` (`:275`), `is_key`, `from_config` (`:373`), reload (`:1654`), `drain` (`:2975`).
- `src/daemon_runtime.rs`, `src/bridge_security.rs` (`build_corba_tls_config`, SSLIOP without ALPN), `src/csiv2_wire.rs`.
- Library: `mapping.rs`, `servant.rs`, `sync.rs`, `notify.rs`, `locate.rs`, `wire.rs`, `qos_translation.rs`.

**Features:** `default = ["std"]`, `std`, `alloc`, `cross-vendor-tests`.

**Configuration** (`KEYS`): `domain`, `log_level`; `corba.orb_id`, `corba.iiop.{bind,giop_version,fragment_size,max_message_size}` (default bind `0.0.0.0:6833`, `giop_version` `1.0|1.1|1.2`), `corba.ssliop.{enabled,bind,cert_file,key_file,ca_file,require_client_cert}` (default bind `0.0.0.0:6834`), `corba.ior.{naming_service,publish_to_naming,ior_file}`, `corba.csiv2.enabled`; `acl.{default_deny,rules}` (rule: `subject`, `allow_invoke`); `metrics.{enabled,listen,path}`; `mappings[]` with `repo_id`, `operation`, `direction`, `request_topic`, `reply_topic`, `notify_topic`, `correlation_id`, `timeout_ms`, `qos`, `targets`, `target_object_keys`.
`corba.ior.publish_to_naming` (default `false`) binds the servant of every receiving mapping into `corba.ior.naming_service` at start and needs that value (`bridge_daemon.rs:476`, `:523`, `:2793`). `corba.csiv2.enabled` (default `false`) adds `TAG_CSI_SEC_MECH_LIST` to the IOR and needs `corba.csiv2.gssup.password_file` (`:541`, `:569`, `:951`).

**Security.** SSLIOP (TLS) with an optional client certificate; the subject of the client certificate is the identity for the ACL (`allow_invoke`).

**SIGHUP.** ACL and SSLIOP material are replaced. A restart is needed for `mappings`, binds, `corba.ssliop.enabled`, `domain` (`bridge_daemon.rs:1679-1688`).

**Shutdown** (§9.2, `bridge_daemon.rs:2975`): new requests receive `TRANSIENT`; open ones receive their reply or `TIMEOUT`, for at most `DRAIN_LIMIT = 30 s`; the rest receive `TRANSIENT` (completion MAYBE); then `CloseConnection` is sent to each connection.

**Interop tests** (third-party peer omniORB 4.3):
- `omniorb_e2e.rs` — omniORB programs from `crates/corba-dds-bridge/tests/omniorb` (omniidl + g++) against the daemon.
- `cross_vendor.rs` — against omniORB.
- Without a third-party peer: `bridge_e2e.rs`, `security_e2e.rs`.

---

## zerodds-bridge-security

**Purpose.** Shared security layer of the six bridge daemons: TLS (§7.1), auth modes (§7.2), topic ACL (§7.3), swappable context for SIGHUP.

**Entry points.**
- `src/tls.rs` — `load_server_config`, `load_server_config_with_client_auth`, `TlsConfigError`.
- `src/connection.rs` — `RotatingTlsConfig` (after a reload, new connections receive the new certificate, existing ones keep their session), `serve_tls_handshake`, `build_client_tls_connector`, `parse_server_name`.
- `src/auth.rs` — `AuthMode` (`None`, `Bearer` with a map token → subject, `Jwt` only `alg=RS256` with optional `iss`/`aud`/required claim, `Mtls`, `SaslPlain`), `AuthSubject`, `jwt_public_key_from_pem`, `JWT_CLOCK_LEEWAY_SECS`.
- `src/acl.rs` — `Acl`, `AclEntry`, `AclOp`; `Acl::deny_all`, `Acl::allow_all`, wildcard and group matching.
- `src/ctx.rs` — `SecurityConfig`, `SecurityCtx`, `build_ctx`, `authenticate`, `authorize`, `LiveSecurity` (`RwLock<Arc<SecurityCtx>>`, `current`/`replace`), `JwtSettings`, `extract_mtls_subject`, `mtls_subject_from_der`.
- `src/lib.rs` — `tls_provider()` depending on the backend; re-exports `rustls`, `rustls_pemfile`, `rustls_pki_types`.

**mTLS subject** (verified, `ctx.rs:295-306`): name = subject DN of the leaf certificate as an RFC 4514 string (`CN=alice,O=Example`); additionally the fingerprint `mtls:<SHA-256 hex>`. An ACL rule may name either. Empty or unreadable DN → fingerprint only.

**Features:** `default = ["std","ring-backend"]`, `std`, `ring-backend` (ring + `rustls/ring`), `aws-lc` (aws-lc-rs + `rustls/aws_lc_rs`, FIPS 140-3 validatable per Cargo comment). The backend applies to JWT RS256 verification and to the rustls provider.

**Tests:** `tests/tls_e2e.rs` (TLS handshake over loopback).

---

## CORBA family

All crates except `corba-rust`, `corba-codegen` and `corba-interop` have the features `default = ["std"]`, `std = ["alloc"]`, `alloc = []`. Deviations are listed with the crate. Coverage: `docs/spec-coverage/corba-3.3.md` (+ `.open.md`).

### zerodds-corba-giop
GIOP codec per CORBA 3.3 Part 2 §15: all eight message types for GIOP 1.0/1.1/1.2, fragments, bidirectional GIOP.
Entry points: `src/codec.rs`, `src/header.rs`, `src/request.rs`, `src/reply.rs`, `src/locate_request.rs`, `src/locate_reply.rs`, `src/cancel_request.rs`, `src/close_connection.rs`, `src/message_error.rs`, `src/fragment.rs`, `src/service_context.rs`, `src/code_set.rs`, `src/target_address.rs`.
Tests: `tests/vendor_frames.rs` (GIOP messages sent by JacORB, omniORB and TAO, decoded and written again), `crates/corba-giop/tests/giop_over_tcp.rs`.

### zerodds-corba-iiop
IIOP TCP transport per §14, §15.7, §15.9: ProfileBody 1.0–1.3 with TaggedComponents, connection/connector/acceptor with reuse, bidirectional GIOP.
Entry points: `src/profile_body.rs`, `src/connection.rs`, `src/connector.rs`, `src/acceptor.rs`, `src/bidir.rs`, `src/framing.rs`, `src/tls.rs`.
Features: additionally `tls` (rustls, rustls-pemfile, rustls-pki-types).

### zerodds-corba-ior
IOR per §13.6: IOR structure, profile tags, 32 standard TaggedComponents including CSIv2 `CompoundSecMechList`, `IOR:` hex form, `corbaloc:`/`corbaname:` parser.
Entry points: `src/ior.rs`, `src/tagged_profile.rs`, `src/profile_tags.rs`, `src/component_tags.rs`, `src/components.rs`, `src/stringified.rs`, `src/url.rs`.

### zerodds-corba-poa
Portable Object Adapter per CORBA 3.3 Part 1 §11: seven policies, POAManager state machine, Active Object Map, ServantManager (Activator/Locator).
Entry points: `src/poa.rs`, `src/poa_manager.rs`, `src/policies.rs`, `src/active_object_map.rs`, `src/servant_manager.rs`, `src/servant.rs`, `src/object_id.rs`.

### zerodds-corba-ir
Interface Repository per Part 1 §14: TypeCode (32 TCKinds), repository hierarchy, `DefinitionKind`, RepositoryId parser (§10.7.3.1).
Entry points: `src/type_code.rs`, `src/repository.rs`, `src/repository_id.rs`, `src/definition_kind.rs`.

### zerodds-corba-rt
Real-Time CORBA 1.0 (`formal/2005-01-04`): Priority, PriorityMapping, PriorityModel, thread pools with lanes, PriorityBandedConnections, `RTCORBA::Current`.
Entry points: `src/priority.rs`, `src/policy.rs`, `src/threadpool.rs`, `src/current.rs`, `src/propagation.rs`, `src/scheduling.rs`, `src/mutex.rs`, `src/any.rs`, `src/type_code.rs`, `src/object.rs`.
Spec: `docs/specs/zerodds-corba-rt-1.0.md`.

### zerodds-corba-rust
IDL → Rust generator for CORBA constructs (interfaces, valuetypes, components, homes, AMI).
Entry points: `src/emitter.rs`, `src/interface_emit.rs`, `src/valuetype_emit.rs`, `src/component_emit.rs`, `src/ami_emit.rs`, `src/type_map.rs`, `src/value_wire.rs`, `src/runtime.rs`.
Features: no `[features]` section.
Spec: `docs/specs/zerodds-corba-rust-1.1.md`.
Tests: `crates/corba-rust/tests/snapshot_codegen.rs` (+ `crates/corba-rust/tests/snapshots`), `crates/corba-rust/tests/compile_check.rs` (the generated code is compiled), `crates/corba-rust/tests/wire_giop.rs` (ValueBase stream §8.3/§4.4).

### zerodds-corba-codegen
The 13 special types from CORBA 3.3 Annex A.1, the type traits from A.1.1 and the stub/skeleton mappings via the shared AST, per language PSM.
Entry points: `src/special_types.rs` (`special_type_of`), `src/traits.rs`, `src/stub.rs`, `src/skeleton.rs`, `src/repository_id.rs`.
Features: no `[features]` section.
Tests: `crates/corba-codegen/tests/orb_agreement.rs` (comparison with what JacORB, omniORB and TAO generate; corpus in `crates/corba-codegen/tests/orb-corpus`), `crates/corba-codegen/tests/model_connection.rs`, `crates/corba-codegen/tests/annex_a1_traits.rs`.

### zerodds-corba-csiv2
CSIv2 per CORBA 3.3 Part 3 §24: association options, compound sec mech list, GSSUP token, SAS protocol, TLS mechanism OID.
Entry points: `src/association_options.rs`, `src/mech_list.rs`, `src/gssup.rs`, `src/sas.rs`, `src/session.rs`.

### zerodds-corba-cosnaming
CosNaming 1.3 (`formal/2004-10-03`): NamingContext and NamingContextExt as an in-memory implementation, five exception classes, string names, `corbaname`.
Entry points: `src/context.rs`, `src/name.rs`, `src/stringified.rs`, `src/error.rs`.

### zerodds-corba-cos-event
CosEventService 1.4 (`formal/2004-10-02`): push/pull model, EventChannelAdmin, typed events, DDS bridge.
Entry points: `src/channel.rs`, `src/comm.rs`, `src/typed.rs`, `src/dds_bridge.rs`.
Coverage: `docs/spec-coverage/cos-event-service-1.4.md`.

### zerodds-corba-cos-notify
CosNotification 1.1 (`formal/2004-10-11`): structured events, EventChannel with ConsumerAdmin/SupplierAdmin, proxy hierarchy, filters, QoS.
Entry points: `src/event.rs`, `src/comm.rs`, `src/channel.rs`, `src/filter.rs`, `src/qos.rs`.
Features: `std` and `alloc` forward to `zerodds-corba-rt`.

### zerodds-corba-cos-trading
Trading Object Service: register/lookup/offers over a subset of the OMG constraint language, dynamic properties, federation.
Entry points: `src/trader.rs`, `src/constraint.rs`, `src/service_type.rs`, `src/property.rs`, `src/dynamic.rs`, `src/federation.rs`.
Spec: `docs/specs/zerodds-corba-cos-trading-1.0.md`.

### zerodds-corba-cos-transactions
Object Transaction Service 1.4 (`formal/2003-09-02`): `otid_t`, PropagationContext, 2-phase commit, Current/Coordinator/Resource, TransactionLog/RecoveryCoordinator.
Entry points: `src/transaction.rs`, `src/two_phase.rs`, `src/propagation.rs`, `src/otid.rs`, `src/recovery.rs`.
Spec: `docs/specs/zerodds-corba-cos-transactions-1.0.md`.
Tests: `crates/corba-cos-transactions/tests/jacorb_live_capture.rs` (PropagationContext recorded live from JacORB 3.9), `crates/corba-cos-transactions/tests/ots_distributed.rs`.

### zerodds-corba-dnc
Deployment & Configuration 4.0 (`formal/2006-04-02`): plan data model (DPD/CPD/IDD/PSD), XML loader (§10), RepositoryManager (§8), ExecutionManager and NodeManager (§9), ContainerHost binding to `corba-ccm`.
Entry points: `src/plan.rs`, `src/xml.rs`, `src/repository.rs`, `src/execution.rs`, `src/node.rs`, `src/container_host.rs`.

### zerodds-corba-interop
Harness for speed and cross-ORB interop: acceptor↔POA request loop (`serve`) and client helper (`invoke_on`) with a hand-marshalled `Echo` servant over the real GIOP/IIOP/POA/CDR stack.
Entry points: `src/lib.rs`, `src/runtime.rs`; binaries in `src/bin/`: `echo_bench`, `echo_bench_codegen`, `bench_features`, `interop_client`, `interop_server`, `ssliop_bench`, `ssliop_client`, `ssliop_server`.
Features: no `[features]` section.
Tests with a third-party ORB: `ami_cross_orb.rs` (JacORB/omniORB/TAO), `bidir_cross_orb.rs`, `csiv2_cross_orb.rs`, `valuetype_cross_orb.rs`, `jacorb_live_ots_handshake.rs`, `context_observer.rs` (JacORB); `codegen_roundtrip.rs` names omniORB/TAO. Without a third-party ORB: `amh.rs`, `ami.rs`, `ami_generated.rs`, `bidir_giop.rs`, `csiv2.rs`, `dii_dsi.rs`, `giop_version.rs`, `uiop.rs` (Unix domain socket transport, tag `TAG_ZERODDS_UDS_TRANS`), `valuetype_wire.rs`.

### zerodds-corba-ccm
CCM 4.0 (`formal/2006-04-01`) §6 and §13: component container, CIF, CIDL model, home, ports, lifecycle, Lightweight CCM, PSS, TimerEventService, Dynamic API.
Entry points: `src/container.rs`, `src/cif.rs`, `src/cidl.rs`, `src/component_def.rs`, `src/home.rs`, `src/port.rs`, `src/lifecycle.rs`, `src/lwccm.rs`, `src/pss.rs`, `src/timer.rs`, `src/time_psm.rs`, `src/orb_core.rs`, `src/orb_extensions.rs`, `src/cos_event_bridge.rs`.
Features: `std` additionally pulls in `flate2`; `cos-event` (includes `zerodds-corba-cos-event`).
Tests: `crates/corba-ccm/tests/pi_service_contexts.rs`.
Coverage: `docs/spec-coverage/omg-ccm-4.0.md`.

### zerodds-corba-ccm-ejb
CCM↔EJB bridge: CosTransactions ↔ JTA `UserTransaction` status, ConnectorBean lifecycle, JNDI ↔ CosNaming, Java stub generator (CCM 4.0 §16, JTA 1.3 §3.2).
Entry points: `src/tx.rs`, `src/connector_bean.rs`, `src/naming_glue.rs`, `src/stub_gen.rs`.
Features: `std` pulls in `zerodds-corba-ccm`.

### zerodds-corba-ccm-lib
Component library for `corba-ccm`: DDS bridge, persistence (PSS §10), telemetry.
Entry points: `src/dds_bridge.rs`, `src/persistence.rs`, `src/telemetry.rs`.

### zerodds-ccm
CCM 4.0 §6 component model at the IDL level: equivalent IDL transformation (Component/Home/EventType), `Components::*` base types, lightweight profile (§13), DDS4CCM.
Entry points: `src/transform.rs`, `src/model.rs`, `src/validate.rs`, `src/lightweight.rs`, `src/dds4ccm.rs`.
Tests: `crates/ccm/tests/equivalent_idl.rs`.
Coverage: `docs/spec-coverage/omg-ccm-4.0.md`, `docs/spec-coverage/dds4ccm-1.1.md`.

### zerodds-ami4ccm
AMI4CCM 1.1 (`formal/2015-08-03`): implied IDL transformation (§7.3, §7.5), ReplyHandler/ExceptionHolder (§7.4), pragma parser (§7.7), connector, deployment, multiplex.
Entry points: `src/transform.rs`, `src/pragma.rs`, `src/exception_holder.rs`, `src/connector.rs`, `src/deployment.rs`, `src/multiplex.rs`, `src/scope_resolver.rs`.
Tests: `crates/ami4ccm/tests/implied_idl.rs`.
Coverage: `docs/spec-coverage/omg-ami4ccm-1.1.md`.

---

## OPC UA

All four crates: `no_std + alloc`, `default = ["std"]`. The tests are unit tests in `src/` (for `opcua-uacp` and `opcua-server` together 81 `#[test]`, counted with grep); there is no `tests/` directory. The third-party peer is open62541, via shell proofs in `proofs/`. Coverage: `docs/spec-coverage/dds-opcua-1.0.md`, `opcua-client-server-1.05.md`, `opcua-pubsub-1.05.md`. There is no `docs/specs/` entry and no daemon for OPC UA.

### zerodds-opcua-uacp
OPC UA Connection Protocol (Part 6 §7.1): `opc.tcp` framing with Hello/Acknowledge/Error/ReverseHello, message chunks, sequence header; SecureChannel.
Entry points: `src/connection.rs`, `src/securechannel.rs`, `src/crypto.rs`.
Features: `std`/`alloc` forward to `opcua-pubsub` and `opcua-gateway`; `crypto` (aes, cbc, hmac, sha2, rsa, sha1) for secured SecurityPolicies (Part 6 §6.7).

### zerodds-opcua-server
Client/server per Part 4 over SecureChannel: session lifecycle, Discovery, Attribute, View, Method, MonitoredItem and Subscription services, in-memory AddressSpace, UserIdentityTokens (§7.40), server and client.
Entry points: `src/server.rs` (`Server`, `ServerSecurity`), `src/client.rs` (`Client::connect`, `serve_connection`), `src/services.rs`, `src/address_space.rs`, `src/attributes.rs`, `src/identity.rs`, `src/status.rs`, `src/wire.rs`.
Features: `std`, `alloc`, `crypto` (forwards to `opcua-uacp/crypto`).
Interop: `proofs/open62541.sh` (ZeroDDS client against `tutorial_server_variable` from open62541), example `crates/opcua-server/examples/open62541_probe.rs`.

### zerodds-opcua-gateway
OMG DDS-OPCUA Gateway 1.0 (`formal/2020-01-01`): type system mapping §8.2/§9.2 (25 built-in types, NodeId, ExpandedNodeId, Variant, DataValue, ExtensionObject) and AddressSpace mapping §9.3 (domain/topic → ObjectNode, sample → variable). No network I/O.
Entry points: `src/types.rs`, `src/node_id.rs`, `src/data_value.rs`, `src/address_space.rs`, `src/dds_to_ua/`, `src/subscription_mapping/`, `src/service_sets/`, `src/historical/`, `src/conformance.rs`, `src/xml.rs` (only with `std`).
Features: `std` (pulls in `roxmltree`), `alloc`.

### zerodds-opcua-pubsub
OPC UA PubSub Part 14 (UADP): Part 6 binary encoding, NetworkMessage/DataSetMessage, PubSub configuration, discovery, SecurityGroup/SKS, transports, publisher/subscriber runtime (`daemon.rs`) and DataSet ↔ DDS topic bridge (`bridge.rs`).
Entry points: `src/binary/`, `src/uadp/`, `src/writer.rs`, `src/reader.rs`, `src/config.rs`, `src/daemon.rs` (`Publisher::publish_cycle`, `Subscriber::poll`), `src/bridge.rs` (`DataSetTopicMapping`, `DdsSample`), `src/transport.rs`, `src/security.rs`, `src/infomodel.rs`, `src/json.rs`, `src/dynamic.rs`.
Features: `std`, `alloc`, `security` (AES-CTR, HMAC-SHA256, AES-GCM; Part 14 §8), `json` (Part 14 §7.2.3; serde_json, base64).
Interop: `proofs/open62541_uadp.sh`, `proofs/build-open62541.sh`, `proofs/uadp_target_check.c`, examples `crates/opcua-pubsub/examples/open62541_uadp_probe.rs`, `crates/opcua-pubsub/examples/open62541_uadp_publisher.rs`.

---

## zerodds-soap

**Purpose.** DDS-SOAP PSM: SOAP 1.2 envelope, WSDL 1.1 and 2.0 generation from IDL service definitions, MTOM/XOP, WS-Addressing 1.0, WS-Security 1.1 (UsernameToken, X.509 token, Timestamp, BinarySecurityToken).
**Entry points:** `src/envelope.rs`, `src/fault.rs`, `src/wsdl.rs`, `src/mtom.rs`, `src/addressing.rs`, `src/security.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.
**Integration:** workspace member (`Cargo.toml:294`); no other crate in the workspace depends on it (grep over `crates/*/Cargo.toml`, `tools/*/Cargo.toml`). No daemon, no spec file under `docs/specs/` or `docs/spec-coverage/`.

---

## Open items and contradictions

- **CORBA daemon:** CSIv2 and publishing the IORs to the Naming Service are not built in the daemon, although `src/csiv2_wire.rs` and `zerodds-corba-cosnaming` exist. `corba.csiv2.enabled`, `corba.csiv2.target_supports`, `corba.csiv2.gssup` and `corba.ior.publish_to_naming` are rejected with the source location ("CSIv2 is not built in the daemon; authenticate with SSLIOP client certificates"), not silently ignored.
- **`tools/amqp-dds-endpoint`:** The DDS side is only `InMemoryDdsHost`; the daemon with a DDS domain is `zerodds-amqp-bridged`.
- **Not determined:** whether `docs/spec-coverage/zerodds-deployment-1.0.md` belongs to `zerodds-corba-dnc`.

Fixed: the module headers of `zerodds-amqp-endpoint`, `tools/amqp-dds-endpoint`, `opcua-uacp`, `corba-rust`, the header of `coap-bridge/tests/security_e2e.rs` and of `websocket-bridge/tests/cross_vendor.rs`; the doc comment of `extract_mtls_subject` was already correct.
