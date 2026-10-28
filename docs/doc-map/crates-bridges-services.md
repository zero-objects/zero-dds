# Bridges, CORBA, OPC UA, SOAP

Diese Seite katalogisiert die Protokoll-Bridges (sechs Daemons und ihre Codec-Crates), die gemeinsame Sicherheitsschicht `zerodds-bridge-security`, die CORBA-Crate-Familie, die OPC-UA-Crates und `zerodds-soap`. Grundlage ist der Code unter `crates/` und `tools/amqp-dds-endpoint/`. Aussagen, die nur aus einem Doku-Kommentar stammen, sind mit „(laut Doku-Kommentar)“ markiert. Nicht geprüft wurde, ob die Tests grün laufen; es wurde nichts gebaut oder ausgeführt.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Übersicht der Daemons

| Daemon | Crate | Binary | Konfigdatei | Spec | man1 | man5 |
|---|---|---|---|---|---|---|
| WebSocket | `crates/websocket-bridge` (`zerodds-websocket-bridge`) | `zerodds-ws-bridged` | `ws-bridged.yaml` + `ws-bridged.d/` | `docs/specs/zerodds-ws-bridge-1.0.md` | `man/man1/zerodds-ws-bridged.1` | `man/man5/zerodds-ws-bridged.yaml.5` |
| MQTT | `crates/mqtt-bridge` (`zerodds-mqtt-bridge`) | `zerodds-mqtt-bridged` | `mqtt-bridged.yaml` + `mqtt-bridged.d/` | `docs/specs/zerodds-mqtt-bridge-1.0.md` | `man/man1/zerodds-mqtt-bridged.1` | `man/man5/zerodds-mqtt-bridged.yaml.5` |
| CoAP | `crates/coap-bridge` (`zerodds-coap-bridge`) | `zerodds-coap-bridged` | `coap-bridged.yaml` + `coap-bridged.d/` | `docs/specs/zerodds-coap-bridge-1.0.md` | `man/man1/zerodds-coap-bridged.1` | `man/man5/zerodds-coap-bridged.yaml.5` |
| AMQP 1.0 | `crates/amqp-endpoint` (`zerodds-amqp-endpoint`) | `zerodds-amqp-bridged` | `amqp-bridged.yaml` + `amqp-bridged.d/` | `docs/specs/zerodds-amqp-bridge-daemon-1.0.md` | `man/man1/zerodds-amqp-bridged.1` | `man/man5/zerodds-amqp-bridged.yaml.5` |
| gRPC | `crates/grpc-bridge` (`zerodds-grpc-bridge`) | `zerodds-grpc-bridged` | `grpc-bridged.yaml` + `grpc-bridged.d/` | `docs/specs/zerodds-grpc-bridge-1.0.md` | `man/man1/zerodds-grpc-bridged.1` | `man/man5/zerodds-grpc-bridged.yaml.5` |
| CORBA | `crates/corba-dds-bridge` (`zerodds-corba-dds-bridge`) | `zerodds-corba-bridged` | `corba-bridged.yaml` + `corba-bridged.d/` | `docs/specs/zerodds-corba-bridge-1.0.md` | `man/man1/zerodds-corba-bridged.1` | `man/man5/zerodds-corba-bridged.yaml.5` |
| AMQP-Endpoint (Server) | `tools/amqp-dds-endpoint` (`amqp-dds-endpoint`) | `amqp-dds-endpoint` | XML-Datei über `--config` | DDS-AMQP 1.0 §2.1, §9.2 | – | – |

Die Dateinamen stammen aus `CONFIG_STEM` im jeweiligen Config-Code. Beispieldateien liegen unter `packaging/linux/configs/<stem>.yaml.example`; jede Bridge hat einen Test `tests/example_yaml_loadback.rs` bzw. einen Unit-Test, der die Beispieldatei durch den Parser schickt.

Die Abdeckung pro Spec-Abschnitt steht in `docs/spec-coverage/zerodds-<name>-1.0.md` (DE) und `.en.md` (EN), offene Punkte in `…-1.0.open.md`.

### Gemeinsames Muster aller sechs Daemons (verifiziert im Code)

- **Konfiguration:** Der gemeinsame Loader `zerodds-config` liest Basisdatei, Drop-ins in `<stem>.d/`, Includes und `${VAR}`. Vorrang: Defaults < Umgebung < Dateien < `--set`. Jeder Daemon hat eine Liste `KEYS` und `is_key()` für die Prüfung unbekannter Schlüssel sowie `DaemonConfig::from_config()`. Innerhalb eines bekannten Abschnitts wird ein unbekannter Schlüssel mit Datei und Zeile abgelehnt (`Section::only`). Die DDS-Runtime-Schlüssel (`transport.*`, `discovery.*`) dürfen in derselben Datei stehen.
- **Signale:** `signal-hook` fängt `SIGTERM`, `SIGINT` und `SIGHUP` (`install_signal_watcher` in `runtime_common.rs` bzw. `daemon_runtime.rs`). `SIGTERM`/`SIGINT` setzen ein Stop-Flag, `SIGHUP` ein Reload-Flag.
- **SIGHUP-Reload:** Der Reload liest dieselben geschichteten Quellen neu (`reload_command_line` im Binary). Eine Konfiguration, die nicht lädt, ändert nichts („reload refused“). Was nur beim Start gebaut wird, bleibt und wird als „reload needs a restart for: …“ geloggt. Der Sicherheitskontext wird als Ganzes über `LiveSecurity::replace` getauscht.
- **Admin-Endpunkt:** `metrics.enabled` schaltet einen kleinen HTTP-Server mit `/metrics` (Prometheus), `/catalog` (JSON) und `/healthz`. Ist `metrics.listen` leer, hat jeder Daemon eine eigene Default-Adresse: ws `127.0.0.1:9091`, mqtt 9092, coap 9093, amqp 9094, grpc 9095, corba 9096.
- **OTLP:** `otlp_config_from_env` liest `OTEL_EXPORTER_OTLP_ENDPOINT`; ein Flush-Thread exportiert alle 5 s (`spawn_otlp_flush_loop`).
- **Interop-Tests:** Tests gegen fremde Gegenstellen sind `#[ignore]` und über Feature `cross-vendor-tests` bzw. Umgebungsvariablen (`MQTT_MOSQUITTO`, `COAP_LIBCOAP`, `AMQP_RABBITMQ`, `GRPC_GRPCURL`, `WS_PYTHON`) freigeschaltet. Mehrere Tests schlagen fehl, wenn das Fremdwerkzeug fehlt, statt still zu überspringen (z. B. `mosquitto_qos.rs`, `grpc-bridge/tests/cross_vendor.rs::require_grpcurl`, `websocat_can_connect_to_zerodds_daemon`). Diese Tests laufen in keiner CI; sie werden auf dem Linux-Testhost von Hand gefahren, zusammen mit `open62541_interop` (OPC UA) und den `durability-svc`-Tests mit PostgreSQL und DuckDB.

### `direction`-Semantik pro Daemon (verifiziert im Code)

| Daemon | `in` | `out` | Fundstelle |
|---|---|---|---|
| ws | DDS-Writer (WebSocket → DDS) | DDS-Reader (DDS → WebSocket-Clients) | `websocket-bridge/src/daemon/server.rs:526-527` (`want_writer` bei `in`/`bidir`) |
| mqtt | DDS-Writer + SUBSCRIBE beim Broker (MQTT → DDS) | DDS-Reader (DDS → MQTT) | `mqtt-bridge/src/daemon/server.rs:762` |
| coap | DDS-Writer (CoAP POST/PUT → DDS) | DDS-Reader (DDS → Observe) | `coap-bridge/src/daemon/server.rs:1603` |
| amqp | Receiver-Link + DDS-Writer (AMQP → DDS) | DDS-Reader + Sender-Link (DDS → AMQP) | `amqp-endpoint/src/bridge_daemon.rs` Modulkopf |
| grpc | DDS-Writer (`Publish` → DDS) | DDS-Reader (DDS → `Subscribe`) | `grpc-bridge/src/bin/zerodds-grpc-bridged.rs` `BridgeDds::start` (`bridged_into_dds`, `bridged_out_of_dds`) |
| corba | Werte `corba_to_dds`, `dds_to_corba`, `bidir` | – | `corba-dds-bridge/src/bridge_daemon.rs:703` |

---

## zerodds-websocket-bridge (`zerodds-ws-bridged`)

**Zweck.** WebSocket-Stack nach RFC 6455 mit permessage-deflate (RFC 7692), URI-Parser und UTF-8-Validator als `no_std + alloc`-Bibliothek; mit Feature `daemon` der Daemon, der DDS-Topics als WebSocket-Pfade anbietet.

**Abbildung.** Ein Topic ist ein HTTP-Pfad (`ws_path`, Default `/topics/<slug>` aus `config::default_ws_path`, `::` → Trenner, Kleinschreibung). Nachrichten sind JSON-Operationen (`src/dds_bridge.rs`): `{"op":"subscribe"|"unsubscribe"|"publish", "topic", …}`, der Daemon sendet `{"op":"notify","topic",…,"data"}`; ACL-Verstöße liefern `{"op":"error","code":403,"reason":"acl-deny-read|acl-deny-write"}` (`src/daemon/server.rs:1037-1095`).

**Einstiegspunkte.**
- `src/bin/zerodds-ws-bridged.rs` — `main`, `run()` (Warteschleife auf `stop_requested()`, dann `shutdown()`).
- `src/daemon/server.rs` — `start`, `start_with_runtime`, `start_with_reload`, `DaemonHandle::shutdown`, `reload()`.
- `src/daemon/config.rs` — `CONFIG_STEM`, `KEYS`, `is_key`, `DaemonConfig::from_config`, `default_ws_path`.
- `src/daemon/security.rs` — `ctx_from_daemon_config` (Start und Reload).
- `src/daemon/runtime_common.rs` — Metriken, `serve_admin_endpoints`, Signal-Watcher.
- Bibliothek: `frame.rs`, `codec.rs`, `handshake.rs`, `negotiation.rs`, `permessage_deflate.rs`, `masking.rs`, `close.rs`, `utf8.rs`, `uri.rs`, `dds_bridge.rs`.

**Features** (`Cargo.toml`): `default = ["std","daemon"]`, `std`, `alloc`, `daemon` (zieht `zerodds-dcps`, `-config`, `-rtps`, `-qos`, `-types`, `-monitor`, `-observability-otlp`, `-bridge-security`, `rustls`, `signal-hook`), `cross-vendor-tests`.

**Konfiguration** (`KEYS`): Wurzel `listen` (Default `127.0.0.1:8080`), `domain`, `log_level`; `tls.{enabled,cert_file,key_file,client_ca_file}`; `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.{public_key,issuer,audience,required_claim}`; `metrics.{enabled,listen}`; `topics[]` mit `name`, `type`, `direction` (`in` = Client → DDS, `out` = DDS → Client, Default `bidir`; Publish/Subscribe gegen die Richtung → Fehler-Frame 404), `ws_path`, `qos.{reliability,durability,history.depth}` (Default-Tiefe 10), `acl.{read,write}`. Ein Topic darf als bloßer Name angegeben werden.

**Sicherheit.** TLS (`wss://`) mit optionalem Client-CA (mTLS). `auth.mode`: `none|bearer|jwt|mtls`; `sasl` wird abgelehnt (Kommentar in `from_config`). ACL pro Topic aus `topics[].acl`.

**SIGHUP.** Mit Loader: ACL und Auth gelten sofort für alle Verbindungen, Zertifikatsdateien werden für neue Verbindungen neu gelesen (`RotatingTlsConfig::reload`). Neustart nötig bei geänderten TLS-Pfaden, `topics`, `listen`, `domain` (`server.rs:1190-1246`).

**Shutdown.** Stop-Flag, `/healthz` meldet ungesund, Selbstverbindung weckt `accept()`, `router.broadcast_shutdown()` sendet Close-Frame `1001 "going away"` (`server.rs:745`), danach Join aller Threads.

**Interop-Tests** (`tests/`):
- `websockets_py_interop_e2e.rs` — Python-Modul `websockets` gegen die Bibliothek (Text, Binär, Ping); Gate `WS_PYTHON=1`.
- `cross_vendor.rs` — `python_websockets_client_handshakes_and_pings_the_daemon` (Python `websockets` gegen den Daemon), `websocat_can_connect_to_zerodds_daemon` (websocat). `rfc6455_handshake_against_zerodds_daemon_returns_101` nutzt einen eigenen Client; der Modulkommentar sagt ausdrücklich, dass dies keine Fremdmessung ist.
- Weitere: `daemon_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## zerodds-mqtt-bridge (`zerodds-mqtt-bridged`)

**Zweck.** MQTT-5.0-Codec (Control-Packets CONNECT bis AUTH, Properties, VBI; laut Doku-Kommentar), MQTT 3.1.1 (`src/version.rs`, `ProtocolVersion`), Topic-Filter, Keep-Alive, ein In-Memory-Broker (`broker.rs`) und ein eigenständiger Broker-Server (`server.rs`, `MqttBrokerServer`); der Daemon ist ein MQTT-Client zu einem externen Broker.

**Abbildung.** DDS-Topic `dds_name` ↔ MQTT-Topic `mqtt_topic`. Für `in|bidir` sendet der Daemon SUBSCRIBE und schreibt empfangene PUBLISH in DDS; für `out|bidir` publiziert er DDS-Samples mit `mqtt_qos` (0/1/2) und `retain`.

**Einstiegspunkte.**
- `src/bin/zerodds-mqtt-bridged.rs`.
- `src/daemon/server.rs` — `start_with_reload`, `DaemonHandle::shutdown`, `drain()`, `reload()`.
- `src/daemon/client.rs` — Broker-Client; `src/daemon/ws_transport.rs` — Transport für `ws://`/`wss://`-Broker (nutzt `zerodds-websocket-bridge`).
- `src/daemon/config.rs` — `KEYS`, `ENV` (`MQTT_PASSWORD` → `mqtt.password`), `is_key` (plus `auth.sasl_users.*`), `BrokerUrl` (Schemata `mqtt` 1883, `mqtts` 8883, `ws` 80, `wss` 443).
- Bibliothek: `codec.rs`, `control_packets.rs`, `packet.rs`, `properties.rs`, `reason_codes.rs`, `topic_filter.rs`, `keep_alive.rs`, `broker.rs`, `server.rs`, `net.rs`, `dds_bridge.rs`.

**Features:** `default = ["std","daemon"]`, `std`, `alloc`, `daemon` (wie ws, zusätzlich `zerodds-websocket-bridge`), `cross-vendor-tests`.

**Konfiguration** (`KEYS`): `domain`, `log_level`; `mqtt.{broker_url,client_id,username,password,keep_alive_secs,drain_timeout_secs,clean_start}`, `mqtt.tls.{enabled,ca_file,cert_file,key_file,server_name}`; `auth.{mode,bearer_token,bearer_subject,username,password,sasl_users}`; `acl.{default_deny,rules}` (Regel: `subject`, `allow_publish`, `allow_subscribe`); `metrics.{enabled,listen}`; `topics[]` mit `dds_name`, `dds_type`, `mqtt_topic`, `direction`, `mqtt_qos`, `retain`, `qos`.

**Sicherheit.** TLS ausgehend zum Broker (`mqtts://`, `wss://`), optional Client-Zertifikat. `auth.mode`: `none|bearer|sasl|sasl_plain|mtls` — Bearer-Token geht als CONNECT-Passwort hinaus, SASL-PLAIN als CONNECT-Benutzer/Passwort. ACL nach Subjekt; `acl.default_deny` greift als Default erst, wenn eine Regel oder ein Auth-Modus existiert; ein ausdrückliches `true` sperrt immer (`config.rs`, Feld `acl_default_deny_set`).

**SIGHUP.** Nur die ACL gilt sofort; ein entzogenes Recht stoppt die Samples des Topics in beide Richtungen. Neustart nötig bei `topics`, `mqtt.broker_url/client_id`, `mqtt.tls`, `auth`, `domain` sowie bei einem neu gewährten Leserecht (Subscriptions entstehen beim Start) (`server.rs:246-250`, `659-710`).

**Shutdown** (§9.2, `server.rs:136-210`): DDS-Pumpen stoppen, auf Broker-Acks für QoS-1/2 warten (höchstens `mqtt.drain_timeout_secs`, Default 30), offene Acks werden geloggt, dann DISCONNECT mit Reason `0x00`, dann Join.

**Interop-Tests** (Fremdpeer Eclipse Mosquitto):
- `mosquitto_interop_e2e.rs` — MQTT 3.1.1 gegen Mosquitto-Broker, `mosquitto_pub/sub -V mqttv311`; Gate `MQTT_MOSQUITTO`.
- `mosquitto_qos.rs` — QoS-1/2-Flüsse und §9.2-Shutdown gegen Mosquitto; fehlende Binaries lassen den Test fehlschlagen.
- `websocket_broker.rs` — `ws://`/`wss://`-Broker-Schemata gegen Mosquittos WebSocket-Listener.
- `cross_vendor.rs` — Daemon gegen Mosquitto.
- Ohne Fremdpeer: `broker_server_e2e.rs` (eigener Broker, 3.1.1 ⇄ 5.0), `daemon_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## zerodds-coap-bridge (`zerodds-coap-bridged`)

**Zweck.** CoAP nach RFC 7252 mit Observe (RFC 7641), Block-Wise (RFC 7959), CoRE Link Format (RFC 6690), Multicast, Caching-Proxy, OSCORE (RFC 8613) und DTLS 1.2 als Option; der Daemon ist ein CoAP-Server über UDP.

**Abbildung.** DDS-Topic ↔ Ressourcenpfad `coap_uri_path`. POST/PUT → DDS-Write (mit Block1-Sammlung, Antwort `2.04 Changed` bzw. `2.31 Continue`); GET mit Observe registriert einen Beobachter, DDS-Samples werden als Notifications gesendet. `/.well-known/core` liefert den Ressourcenkatalog im Link-Format (ohne Auth) (`src/daemon/server.rs:857-974`).

**Einstiegspunkte.**
- `src/bin/zerodds-coap-bridged.rs`.
- `src/daemon/server.rs` — `start_with_reload`, `dispatch`, `shutdown`, `reload()` (`:1637`), DTLS-Rotation (`:1464`).
- `src/daemon/oscore_gateway.rs` — OSCORE-Serverrolle.
- `src/daemon/config.rs` — `KEYS`, `OscoreSettings`, `TopicConfig`.
- Bibliothek: `message.rs`, `codec.rs`, `option.rs`, `reliability.rs`, `observe.rs`, `blockwise.rs`, `core_link.rs`, `multicast.rs`, `caching_proxy.rs`, `matching.rs`, `uri.rs`, `dtls.rs`, `dtls_transport.rs`, `oscore/`.

**Features:** `default = ["std","daemon"]`, `std`, `alloc`, `daemon`, `oscore` (hmac/sha2/ccm/aes), `dtls` (webrtc-dtls, tokio; im Cargo-Kommentar als EXPERIMENTAL markiert, siehe ADR 0011), `cross-vendor-tests`.

**Konfiguration** (`KEYS`): `domain`, `log_level`; `coap.{bind,bind_dtls,max_message_size,ack_timeout_ms,max_retransmit,observe_max_age_secs}` (Default-Bind `0.0.0.0:5683`), `coap.dtls.{enabled,cert_file,key_file,ca_file,cipher_suites,psk}` (PSK-Einträge `identity`, `secret_hex`); `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.*`; `acl.{default_deny,rules}` (Regel: `subject`, `allow_post`, `allow_observe`); `metrics.{enabled,listen}`; `oscore.{enabled,master_secret_hex,master_salt_hex,id_context_hex,sender_id_hex,replay_window}`; `topics[]` mit `dds_name`, `dds_type`, `coap_uri_path`, `direction`, `qos`.

**Sicherheit.** DTLS (`coaps://`) nur mit Feature `dtls`; `coap.dtls.enabled: true` ohne das Feature verweigert den Start (`server.rs:417-423`). `auth.mode`: `none|bearer|jwt|mtls`; der Token steht in CoAP-Option 65000, Fehler ergeben `4.01`. ACL-Verstoß `4.03`. OSCORE: Daemon ist Server, Kontext pro Client-`kid` per HKDF.

**SIGHUP.** Mit Loader gelten ACL und Auth für folgende Requests; DTLS-Zugangsdaten werden immer neu gelesen, bestehende Sessions bleiben. Neustart nötig bei `topics`, Binds, `domain`, DTLS-Pfaden (`server.rs:321-326`, `:1658-1661`).

**Shutdown** (§9.2): keine neuen Requests und Samples, alle Observations werden beendet (`end_observations`), DTLS-Thread wird benachrichtigt, UDP-Empfang per Selbst-Datagramm geweckt, dann Join.

**Interop-Tests** (Fremdpeer libcoap):
- `libcoap_interop_e2e.rs` — libcoap-Client `coap-client` gegen einen Responder aus dem eigenen Codec; Gate `COAP_LIBCOAP`.
- `cross_vendor.rs` — libcoap-Client gegen den Daemon.
- `oscore_rfc8613_vectors.rs` — Testvektoren aus RFC 8613 Anhang C, bytegenau.
- Ohne Fremdpeer: `daemon_e2e.rs`, `daemon_dtls_e2e.rs`, `dtls_coap_e2e.rs`, `daemon_oscore_e2e.rs`, `security_e2e.rs`, `fuzz_smoke.rs`, `example_yaml_loadback.rs`.

---

## AMQP: zerodds-amqp-bridge, zerodds-amqp-endpoint, zerodds-amqp-0-9-1 (`zerodds-amqp-bridged`)

### zerodds-amqp-bridge

**Zweck.** OASIS-AMQP-1.0-Codec: Typsystem, Frames, Performatives, Message-Sections, dazu die Typabbildung von DDS-AMQP 1.0 §7 und die Body-Kodierungen §8.1 über den gemeinsamen AST.
**Einstiegspunkte:** `src/types.rs`, `src/extended_types.rs`, `src/frame.rs`, `src/performatives.rs`, `src/sections.rs`, `src/codec_profile.rs`, `src/codegen/{plan,encode,emit}.rs`; Fuzz-Ziele unter `fuzz/fuzz_targets/`, Benchmark `benches/decode_hotpaths.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`, `codec-lite` (nur Konformitätsmarker nach DDS-AMQP §2.4, ändert keinen Codepfad).

### zerodds-amqp-endpoint (enthält den Daemon)

**Zweck.** Protokollschichten über dem Codec (SASL, Session/Link, Routing, Mapping, Properties, Annex-A-Konfiguration) und der Bridge-Daemon `zerodds-amqp-bridged`: DDS-Participant auf der einen Seite, Client eines AMQP-1.0-Brokers auf der anderen.

**Abbildung** (Modulkopf `src/bridge_daemon.rs`, verifiziert): Pro Topic `amqp_address` (Knotenadresse). `out|bidir`: DDS-Reader + Sender-Link, jedes Sample eine Nachricht (Pass-Through-Body §8.1.1, Properties §8.2, `dds:*`-Application-Properties §8.3). `in|bidir`: Receiver-Link + DDS-Writer; Settlement `accepted` nach Write, `rejected` bei ungültigem Body, `released` bei Write-Fehler. Ohne Credit wartet ein Sample in der Link-Queue, begrenzt durch HISTORY (`KEEP_ALL_QUEUE_CAP = 65_536`). Schleifenschutz über `dds:bridge-id` bzw. Writer-GUID. Unbestätigte Deliveries werden nach Reconnect erneut gesendet.

**Einstiegspunkte.**
- `src/bin/zerodds-amqp-bridged.rs` — `CONFIG_STEM = "amqp-bridged"`, Signal-Watcher, Admin-Server, `run_with_reload`.
- `src/bridge_daemon.rs` — `KEYS`, `ENV` (`AMQP_PASSWORD` → `amqp.sasl.password`), `DaemonConfig::from_config`, `run`, `run_with_reload`, Reload (`:1098`), `drain` (`:2208`).
- `src/broker.rs` — `connect` (TCP oder TLS mit ALPN `amqp`, SASL PLAIN/ANONYMOUS/EXTERNAL/SCRAM-SHA-256/XOAUTH2), `BrokerWriter`.
- `src/daemon_runtime.rs` — Metriken, Admin-Endpunkte; `src/bridge_security.rs`.
- Protokollmodule: `sasl.rs`, `scram.rs`, `session.rs`, `link.rs`, `routing.rs`, `mapping.rs`, `properties.rs`, `dds_bridge.rs`, `management.rs`, `metrics.rs`, `security.rs`, `coexistence.rs`, `rpc_correlation.rs`, `errors.rs`, `limits.rs`, `keyhash.rs`, `annex_a.rs`, `codegen_helpers.rs`, `config_xml.rs`, `client.rs`, `backoff.rs`.

**Features:** `default = ["std"]`, `std` (zieht Daemon-Abhängigkeiten, `roxmltree`), `alloc`, `cross-vendor-tests`.

**Konfiguration** (`KEYS`): `domain`, `log_level`; `amqp.{broker_url,container_id,channel_max,max_frame_size,idle_time_out_ms,hostname}`, `amqp.sasl.{mechanism,username,password}` (Mechanismen `PLAIN|SCRAM-SHA-256|EXTERNAL|ANONYMOUS|XOAUTH2`, Default `ANONYMOUS`), `amqp.tls.{enabled,ca_file,cert_file,key_file,verify_hostname,alpn}`, `amqp.reconnect.{initial_delay_ms,max_delay_ms,factor}`; `acl.{default_deny,rules}` (Regel: `topic`, `read`, `write`); `metrics.{enabled,listen,path}`; `catalog.{enabled,address}` (Default-Adresse `zerodds.bridge.<container-id>.catalog`); `topics[]` mit `dds_name`, `dds_type`, `keyed`, `amqp_address`, `direction`, `sender.{settled,durable,snd_settle_mode,rcv_settle_mode}`, `receiver.credit`, `qos`.
Festwerte: `amqp.tls.verify_hostname` nur `true`, `metrics.path` nur `/metrics`, ALPN fest `amqp`.

**Sicherheit.** TLS zum Broker (`amqps://`), SASL wie oben. Kein `auth`-Abschnitt; die ACL nennt das SASL-Subjekt der Bridge.

**SIGHUP.** ACL gilt sofort (entzogenes Recht stoppt Samples und Transfers), TLS-Material gilt ab dem nächsten Connect. Neustart nötig bei neu gewährtem Recht, `topics`, `amqp.broker_url`, `domain` (`bridge_daemon.rs:1093-1151`).

**Shutdown.** DDS-Samples werden nicht mehr angenommen, laufende Transfers bis `DRAIN_BUDGET = 30 s` abgewartet, dann detach, end, close (`bridge_daemon.rs:1369`, `:1870`, `:2004`).

**Interop-Tests** (Fremdpeer RabbitMQ 4.0; Gate `AMQP_RABBITMQ`):
- `bridge_rabbitmq_e2e.rs` — Daemon gegen RabbitMQ, `pika` (AMQP 0.9.1) als Referenzclient.
- `rabbitmq_amqp10_e2e.rs` — `AmqpClient` → RabbitMQ → Referenzconsumer.
- `rabbitmq_cross_stack_e2e.rs` — eigener AMQP-1.0- und eigener 0.9.1-Stack über dieselbe RabbitMQ-Queue.
- `cross_vendor.rs` — gegen RabbitMQ (der Feature-Kommentar in `Cargo.toml` nennt zusätzlich qpid; im Test nicht geprüft).
- Ohne Fremdpeer: `broker_tls_sasl.rs` (SCRAM-SHA-256 und EXTERNAL gegen einen rustls-Testserver), `security_e2e.rs`, `bridge_e2e.rs`, `e2e_multi_bridge_hop.rs`, `annex_a_idl_roundtrip.rs`, `proptest_state_machine.rs`, `fuzz_smoke.rs`.

### zerodds-amqp-0-9-1

**Zweck.** AMQP-0.9.1-Codec (Class/Method, Frames, Field-Tables) und Broker-Client für RabbitMQ.
**Einstiegspunkte:** `src/client.rs` (`Amqp091Client`), `src/frame.rs`, `src/method.rs`, `src/types.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.
**Einbindung:** nur Dev-Abhängigkeit von `zerodds-amqp-endpoint` für `rabbitmq_cross_stack_e2e.rs`; der Daemon nutzt es nicht (`crates/amqp-endpoint/Cargo.toml:97-99`).
**Interop-Test:** `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs` gegen RabbitMQ 4.0; Gate `AMQP_RABBITMQ`.

### tools/amqp-dds-endpoint

**Zweck.** Synchroner AMQP-1.0-Server (Endpoint-Profil DDS-AMQP 1.0 §2.1), thread-per-connection, Konfiguration als XML (`zerodds_amqp_endpoint::config_xml::parse_config`).
**Einstiegspunkte:** `src/main.rs` (`amqp-dds-endpoint [--config <path>] [--listen <addr>]`, Default `0.0.0.0:5672`), `src/server.rs`, `src/handler.rs`, `src/frame_io.rs`, `src/auth.rs` (SCRAM-Server-Nonce über `zerodds-bridge-security`), `src/tls.rs`, `src/bridge.rs`, `src/dds_host.rs`.
**Features:** `default = ["tls"]`, `tls` (rustls, rustls-pemfile, `zerodds-bridge-security`); das Binary verlangt `tls`.
**DDS-Seite:** Die einzige Implementierung von `DdsHost` ist `InMemoryDdsHost` (`src/dds_host.rs:192`); eine Anbindung an `zerodds-dcps` gibt es in diesem Crate nicht.
**Tests:** `tests/c1_*` und `c2_*` (Connection-Open, SASL-PLAIN-Ablehnung, Settlement, Wildcard-Adressen, Idle-Timeout, Loop-Prevention, Reconnect), `tls_sasl_e2e.rs`, `sasl_server_checks.rs`, `readme_config.rs`. Kein Fremdpeer.

---

## gRPC: zerodds-grpc-bridge, zerodds-http2, zerodds-hpack (`zerodds-grpc-bridged`)

### zerodds-grpc-bridge

**Zweck.** gRPC-over-HTTP/2- und gRPC-Web-Codec (Length-Prefixed Message, Path, Timeout, Status, Metadata), Server-Skelett, Reflection (`grpc.reflection.v1alpha`, minimales Profil) und der Daemon.

**Abbildung.** Pro Topic ein gRPC-Service `grpc_service` (Treffer auch über den letzten Namensteil). Methoden `Publish` und `PublishOne` schreiben `Sample{bytes payload=1}` als opake Nutzdaten in einen DDS-Writer und antworten `PublishAck{accepted}`; `Subscribe` entnimmt pro Aufruf ein Sample aus dem DDS-Reader (Pull; leerer Body = Stream-Ende) (`src/bin/zerodds-grpc-bridged.rs:1000-1074`). `direction` bestimmt, welche Endpunkte entstehen: `in` = `Publish` → DDS-Writer, `out` = DDS-Reader → `Subscribe`, `bidir` beides. Ein Aufruf gegen die Richtung endet mit `FAILED_PRECONDITION`, ein fehlgeschlagener Write mit `UNAVAILABLE` (`dispatch`). Service-`.proto`-Erzeugung: `src/service_gen.rs` (Paket `zerodds.bridge.v1`).

**Einstiegspunkte.**
- `src/bin/zerodds-grpc-bridged.rs` — `main`, `KEYS` (`:1460`), `is_key`, `from_config`, `BridgeDds::start`, Verbindungsschleife mit GOAWAY, `apply_reload` (`:1672`).
- `src/daemon_runtime.rs` — Admin-Endpunkte, Signal-Watcher, `build_grpc_tls_config` (ALPN `h2`).
- Bibliothek: `frame.rs`, `path.rs`, `timeout.rs`, `status.rs`, `metadata.rs`, `server.rs`, `reflection.rs`, `service_gen.rs`, `qos_translation.rs`, `bridge_security.rs`.

**Features:** `default = ["std"]`, `std`, `alloc`, `cross-vendor-tests`, `dds-runtime` (leerer Alias für `std`; die DDS-Seite gehört zu `std`).

**Konfiguration** (`KEYS`): `domain`, `log_level`; `grpc.bind` (Default `0.0.0.0:50051`), `grpc.tls.{enabled,cert_file,key_file,client_ca_file,require_client_cert}`, `grpc.reflection.enabled`; `auth.{mode,bearer_token,bearer_subject}`, `auth.jwt.*`; `metrics.{enabled,listen}`; `topics[]` mit `dds_name`, `grpc_service`, `direction`, `acl.{publish,subscribe}`.

**Sicherheit.** TLS mit ALPN `h2`, optional mTLS. `auth.mode`: `none|bearer|jwt|mtls`, pro RPC geprüft über Metadata `authorization` oder Client-Zertifikat. ACL: `Publish`/`PublishOne` = Write, `Subscribe` = Read.

**SIGHUP.** TLS-Material, Auth und ACL werden ersetzt; Listener, DDS-Seite, Topic-Mapping und TLS an/aus behalten den Startwert und werden geloggt.

**Shutdown.** Jede Verbindung prüft das Stop-Flag im Takt `CONN_POLL`; ein laufender Aufruf endet zuerst, dann GOAWAY mit der letzten Stream-ID (RFC 7540 §6.8). Leerlaufende Verbindungen erhalten ebenfalls GOAWAY.

**Interop-Tests:**
- `cross_vendor.rs` — `grpcurl`; Gate `GRPC_GRPCURL=1`, fehlendes `grpcurl` lässt den Test fehlschlagen.
- Ohne Fremdpeer: `daemon_security_e2e.rs` (Binary mit Konfigdatei, HTTP/2 über TCP/TLS), `security_e2e.rs`, `dds_bridge_e2e.rs`, `bridge_e2e.rs`, `fuzz_smoke.rs`.

### zerodds-http2

**Zweck.** HTTP/2-Codec nach RFC 9113, `no_std`: Framing, Stream-Zustandsautomat, Flusskontrolle, Connection-Preface, Settings.
**Einstiegspunkte:** `src/frame.rs`, `src/stream.rs`, `src/flow.rs`, `src/preface.rs`, `src/settings.rs`, `src/error.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`. Nutzer: `zerodds-grpc-bridge`, `zerodds-conformance`.

### zerodds-hpack

**Zweck.** HPACK nach RFC 7541, `no_std`: statische und dynamische Tabelle, Integer-, String- und Huffman-Kodierung.
**Einstiegspunkte:** `src/encoder.rs`, `src/decoder.rs`, `src/table.rs`, `src/huffman.rs`, `src/integer.rs`, `src/string.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.

---

## zerodds-corba-dds-bridge (`zerodds-corba-bridged`)

**Zweck.** Bidirektionale Brücke CORBA-Objekt ↔ DDS-Topic: IIOP-Server (und SSLIOP) auf der einen, DDS-Participant auf der anderen Seite.

**Abbildung** (Modulkopf `src/bridge_daemon.rs`, verifiziert): Mapping über `repo_id` + `operation`.
- `corba_to_dds`: GIOP-Request → Sample auf `request_topic`; das Sample auf `reply_topic` mit gleicher `request_id` wird zur GIOP-Reply. Ohne Antwort in `timeout_ms` → `CORBA::TIMEOUT`.
- `dds_to_corba`: Sample auf `notify_topic` → oneway-GIOP-Request an jedes Ziel (`targets`, `target_object_keys`).
- Die Samples tragen die GIOP-Bytes unverändert (XCDR1, Layout `<Operation>Request`/`<Operation>Reply` als `@final struct`). Eine gemappte Operation über GIOP 1.0/1.1 wird mit `CORBA::NO_IMPLEMENT` beantwortet.
- IORs werden in `corba.ior.ior_file` geschrieben.

**Einstiegspunkte.**
- `src/bin/zerodds-corba-bridged.rs` — `CONFIG_STEM = "corba-bridged"`, Signal-Watcher, `run_with_reload`.
- `src/bridge_daemon.rs` — `KEYS` (`:275`), `is_key`, `from_config` (`:373`), Reload (`:1654`), `drain` (`:2975`).
- `src/daemon_runtime.rs`, `src/bridge_security.rs` (`build_corba_tls_config`, SSLIOP ohne ALPN), `src/csiv2_wire.rs`.
- Bibliothek: `mapping.rs`, `servant.rs`, `sync.rs`, `notify.rs`, `locate.rs`, `wire.rs`, `qos_translation.rs`.

**Features:** `default = ["std"]`, `std`, `alloc`, `cross-vendor-tests`.

**Konfiguration** (`KEYS`): `domain`, `log_level`; `corba.orb_id`, `corba.iiop.{bind,giop_version,fragment_size,max_message_size}` (Default-Bind `0.0.0.0:6833`, `giop_version` `1.0|1.1|1.2`), `corba.ssliop.{enabled,bind,cert_file,key_file,ca_file,require_client_cert}` (Default-Bind `0.0.0.0:6834`), `corba.ior.{naming_service,publish_to_naming,ior_file}`, `corba.csiv2.enabled`; `acl.{default_deny,rules}` (Regel: `subject`, `allow_invoke`); `metrics.{enabled,listen,path}`; `mappings[]` mit `repo_id`, `operation`, `direction`, `request_topic`, `reply_topic`, `notify_topic`, `correlation_id`, `timeout_ms`, `qos`, `targets`, `target_object_keys`.
`corba.ior.publish_to_naming` (Default `false`) bindet beim Start den Servant jedes empfangenden Mappings in `corba.ior.naming_service` und braucht diesen Wert (`bridge_daemon.rs:476`, `:523`, `:2793`). `corba.csiv2.enabled` (Default `false`) trägt `TAG_CSI_SEC_MECH_LIST` in die IOR ein und braucht `corba.csiv2.gssup.password_file` (`:541`, `:569`, `:951`).

**Sicherheit.** SSLIOP (TLS) mit optionalem Client-Zertifikat; das Subjekt des Client-Zertifikats ist die Identität für die ACL (`allow_invoke`).

**SIGHUP.** ACL und SSLIOP-Material werden ersetzt. Neustart nötig bei `mappings`, Binds, `corba.ssliop.enabled`, `domain` (`bridge_daemon.rs:1679-1688`).

**Shutdown** (§9.2, `bridge_daemon.rs:2975`): neue Requests erhalten `TRANSIENT`; offene erhalten ihre Reply oder `TIMEOUT`, höchstens `DRAIN_LIMIT = 30 s`; Rest `TRANSIENT` (completion MAYBE); danach `CloseConnection` an jede Verbindung.

**Interop-Tests** (Fremdpeer omniORB 4.3):
- `omniorb_e2e.rs` — omniORB-Programme aus `crates/corba-dds-bridge/tests/omniorb` (omniidl + g++) gegen den Daemon.
- `cross_vendor.rs` — gegen omniORB.
- Ohne Fremdpeer: `bridge_e2e.rs`, `security_e2e.rs`.

---

## zerodds-bridge-security

**Zweck.** Gemeinsame Sicherheitsschicht der sechs Bridge-Daemons: TLS (§7.1), Auth-Modi (§7.2), Topic-ACL (§7.3), austauschbarer Kontext für SIGHUP.

**Einstiegspunkte.**
- `src/tls.rs` — `load_server_config`, `load_server_config_with_client_auth`, `TlsConfigError`.
- `src/connection.rs` — `RotatingTlsConfig` (neue Verbindungen erhalten nach Reload das neue Zertifikat, bestehende behalten ihre Session), `serve_tls_handshake`, `build_client_tls_connector`, `parse_server_name`.
- `src/auth.rs` — `AuthMode` (`None`, `Bearer` mit Map Token → Subjekt, `Jwt` nur `alg=RS256` mit optionalem `iss`/`aud`/Pflicht-Claim, `Mtls`, `SaslPlain`), `AuthSubject`, `jwt_public_key_from_pem`, `JWT_CLOCK_LEEWAY_SECS`.
- `src/acl.rs` — `Acl`, `AclEntry`, `AclOp`; `Acl::deny_all`, `Acl::allow_all`, Wildcard- und Gruppenabgleich.
- `src/ctx.rs` — `SecurityConfig`, `SecurityCtx`, `build_ctx`, `authenticate`, `authorize`, `LiveSecurity` (`RwLock<Arc<SecurityCtx>>`, `current`/`replace`), `JwtSettings`, `extract_mtls_subject`, `mtls_subject_from_der`.
- `src/lib.rs` — `tls_provider()` je nach Backend; Re-Exporte `rustls`, `rustls_pemfile`, `rustls_pki_types`.

**mTLS-Subjekt** (verifiziert, `ctx.rs:295-306`): Name = Subject-DN des Leaf-Zertifikats als RFC-4514-String (`CN=alice,O=Example`); zusätzlich Fingerprint `mtls:<SHA-256 hex>`. Eine ACL-Regel darf beides nennen. Leerer oder nicht lesbarer DN → nur Fingerprint.

**Features:** `default = ["std","ring-backend"]`, `std`, `ring-backend` (ring + `rustls/ring`), `aws-lc` (aws-lc-rs + `rustls/aws_lc_rs`, FIPS-140-3-validierbar laut Cargo-Kommentar). Das Backend gilt für JWT-RS256-Verifikation und den rustls-Provider.

**Tests:** `tests/tls_e2e.rs` (TLS-Handshake über Loopback).

---

## CORBA-Familie

Alle Crates außer `corba-rust`, `corba-codegen` und `corba-interop` haben die Features `default = ["std"]`, `std = ["alloc"]`, `alloc = []`. Abweichungen stehen beim Crate. Abdeckung: `docs/spec-coverage/corba-3.3.md` (+ `.open.md`).

### zerodds-corba-giop
GIOP-Codec nach CORBA 3.3 Part 2 §15: alle acht Nachrichtentypen für GIOP 1.0/1.1/1.2, Fragmente, Bidir-GIOP.
Einstiegspunkte: `src/codec.rs`, `src/header.rs`, `src/request.rs`, `src/reply.rs`, `src/locate_request.rs`, `src/locate_reply.rs`, `src/cancel_request.rs`, `src/close_connection.rs`, `src/message_error.rs`, `src/fragment.rs`, `src/service_context.rs`, `src/code_set.rs`, `src/target_address.rs`.
Tests: `tests/vendor_frames.rs` (GIOP-Nachrichten, die JacORB, omniORB und TAO gesendet haben, dekodiert und wieder geschrieben), `crates/corba-giop/tests/giop_over_tcp.rs`.

### zerodds-corba-iiop
IIOP-TCP-Transport nach §14, §15.7, §15.9: ProfileBody 1.0–1.3 mit TaggedComponents, Connection/Connector/Acceptor mit Wiederverwendung, Bidir-GIOP.
Einstiegspunkte: `src/profile_body.rs`, `src/connection.rs`, `src/connector.rs`, `src/acceptor.rs`, `src/bidir.rs`, `src/framing.rs`, `src/tls.rs`.
Features: zusätzlich `tls` (rustls, rustls-pemfile, rustls-pki-types).

### zerodds-corba-ior
IOR nach §13.6: IOR-Struktur, Profil-Tags, 32 Standard-TaggedComponents inklusive CSIv2 `CompoundSecMechList`, `IOR:`-Hexform, `corbaloc:`/`corbaname:`-Parser.
Einstiegspunkte: `src/ior.rs`, `src/tagged_profile.rs`, `src/profile_tags.rs`, `src/component_tags.rs`, `src/components.rs`, `src/stringified.rs`, `src/url.rs`.

### zerodds-corba-poa
Portable Object Adapter nach CORBA 3.3 Part 1 §11: sieben Policies, POAManager-Zustandsautomat, Active Object Map, ServantManager (Activator/Locator).
Einstiegspunkte: `src/poa.rs`, `src/poa_manager.rs`, `src/policies.rs`, `src/active_object_map.rs`, `src/servant_manager.rs`, `src/servant.rs`, `src/object_id.rs`.

### zerodds-corba-ir
Interface Repository nach Part 1 §14: TypeCode (32 TCKinds), Repository-Hierarchie, `DefinitionKind`, RepositoryId-Parser (§10.7.3.1).
Einstiegspunkte: `src/type_code.rs`, `src/repository.rs`, `src/repository_id.rs`, `src/definition_kind.rs`.

### zerodds-corba-rt
Real-Time CORBA 1.0 (`formal/2005-01-04`): Priority, PriorityMapping, PriorityModel, Threadpools mit Lanes, PriorityBandedConnections, `RTCORBA::Current`.
Einstiegspunkte: `src/priority.rs`, `src/policy.rs`, `src/threadpool.rs`, `src/current.rs`, `src/propagation.rs`, `src/scheduling.rs`, `src/mutex.rs`, `src/any.rs`, `src/type_code.rs`, `src/object.rs`.
Spec: `docs/specs/zerodds-corba-rt-1.0.md`.

### zerodds-corba-rust
IDL → Rust-Generator für CORBA-Konstrukte (Interfaces, Valuetypes, Components, Homes, AMI).
Einstiegspunkte: `src/emitter.rs`, `src/interface_emit.rs`, `src/valuetype_emit.rs`, `src/component_emit.rs`, `src/ami_emit.rs`, `src/type_map.rs`, `src/value_wire.rs`, `src/runtime.rs`.
Features: keine `[features]`-Sektion.
Spec: `docs/specs/zerodds-corba-rust-1.1.md`.
Tests: `crates/corba-rust/tests/snapshot_codegen.rs` (+ `crates/corba-rust/tests/snapshots`), `crates/corba-rust/tests/compile_check.rs` (erzeugter Code wird kompiliert), `crates/corba-rust/tests/wire_giop.rs` (ValueBase-Stream §8.3/§4.4).

### zerodds-corba-codegen
Die 13 Sondertypen aus CORBA 3.3 Annex A.1, die Typ-Traits aus A.1.1 und die Stub-/Skeleton-Abbildungen über den gemeinsamen AST, pro Sprach-PSM.
Einstiegspunkte: `src/special_types.rs` (`special_type_of`), `src/traits.rs`, `src/stub.rs`, `src/skeleton.rs`, `src/repository_id.rs`.
Features: keine `[features]`-Sektion.
Tests: `crates/corba-codegen/tests/orb_agreement.rs` (Abgleich mit dem, was JacORB, omniORB und TAO erzeugen; Korpus in `crates/corba-codegen/tests/orb-corpus`), `crates/corba-codegen/tests/model_connection.rs`, `crates/corba-codegen/tests/annex_a1_traits.rs`.

### zerodds-corba-csiv2
CSIv2 nach CORBA 3.3 Part 3 §24: Association Options, Compound-Sec-Mech-List, GSSUP-Token, SAS-Protokoll, TLS-Mechanismus-OID.
Einstiegspunkte: `src/association_options.rs`, `src/mech_list.rs`, `src/gssup.rs`, `src/sas.rs`, `src/session.rs`.

### zerodds-corba-cosnaming
CosNaming 1.3 (`formal/2004-10-03`): NamingContext und NamingContextExt als In-Memory-Implementierung, fünf Exception-Klassen, String-Namen, `corbaname`.
Einstiegspunkte: `src/context.rs`, `src/name.rs`, `src/stringified.rs`, `src/error.rs`.

### zerodds-corba-cos-event
CosEventService 1.4 (`formal/2004-10-02`): Push-/Pull-Modell, EventChannelAdmin, Typed Events, DDS-Brücke.
Einstiegspunkte: `src/channel.rs`, `src/comm.rs`, `src/typed.rs`, `src/dds_bridge.rs`.
Abdeckung: `docs/spec-coverage/cos-event-service-1.4.md`.

### zerodds-corba-cos-notify
CosNotification 1.1 (`formal/2004-10-11`): Structured Events, EventChannel mit ConsumerAdmin/SupplierAdmin, Proxy-Hierarchie, Filter, QoS.
Einstiegspunkte: `src/event.rs`, `src/comm.rs`, `src/channel.rs`, `src/filter.rs`, `src/qos.rs`.
Features: `std` und `alloc` reichen an `zerodds-corba-rt` weiter.

### zerodds-corba-cos-trading
Trading Object Service: Register/Lookup/Offers über eine Teilmenge der OMG-Constraint-Sprache, dynamische Properties, Föderation.
Einstiegspunkte: `src/trader.rs`, `src/constraint.rs`, `src/service_type.rs`, `src/property.rs`, `src/dynamic.rs`, `src/federation.rs`.
Spec: `docs/specs/zerodds-corba-cos-trading-1.0.md`.

### zerodds-corba-cos-transactions
Object Transaction Service 1.4 (`formal/2003-09-02`): `otid_t`, PropagationContext, 2-Phase-Commit, Current/Coordinator/Resource, TransactionLog/RecoveryCoordinator.
Einstiegspunkte: `src/transaction.rs`, `src/two_phase.rs`, `src/propagation.rs`, `src/otid.rs`, `src/recovery.rs`.
Spec: `docs/specs/zerodds-corba-cos-transactions-1.0.md`.
Tests: `crates/corba-cos-transactions/tests/jacorb_live_capture.rs` (live aufgezeichneter PropagationContext von JacORB 3.9), `crates/corba-cos-transactions/tests/ots_distributed.rs`.

### zerodds-corba-dnc
Deployment & Configuration 4.0 (`formal/2006-04-02`): Plan-Datenmodell (DPD/CPD/IDD/PSD), XML-Loader (§10), RepositoryManager (§8), ExecutionManager und NodeManager (§9), ContainerHost-Anbindung an `corba-ccm`.
Einstiegspunkte: `src/plan.rs`, `src/xml.rs`, `src/repository.rs`, `src/execution.rs`, `src/node.rs`, `src/container_host.rs`.

### zerodds-corba-interop
Harness für Geschwindigkeit und ORB-übergreifende Interop: Acceptor↔POA-Request-Schleife (`serve`) und Client-Helfer (`invoke_on`) mit handmarshalltem `Echo`-Servant über den echten GIOP/IIOP/POA/CDR-Stack.
Einstiegspunkte: `src/lib.rs`, `src/runtime.rs`; Binaries in `src/bin/`: `echo_bench`, `echo_bench_codegen`, `bench_features`, `interop_client`, `interop_server`, `ssliop_bench`, `ssliop_client`, `ssliop_server`.
Features: keine `[features]`-Sektion.
Tests mit Fremd-ORB: `ami_cross_orb.rs` (JacORB/omniORB/TAO), `bidir_cross_orb.rs`, `csiv2_cross_orb.rs`, `valuetype_cross_orb.rs`, `jacorb_live_ots_handshake.rs`, `context_observer.rs` (JacORB); `codegen_roundtrip.rs` nennt omniORB/TAO. Ohne Fremd-ORB: `amh.rs`, `ami.rs`, `ami_generated.rs`, `bidir_giop.rs`, `csiv2.rs`, `dii_dsi.rs`, `giop_version.rs`, `uiop.rs` (Unix-Domain-Socket-Transport, Tag `TAG_ZERODDS_UDS_TRANS`), `valuetype_wire.rs`.

### zerodds-corba-ccm
CCM 4.0 (`formal/2006-04-01`) §6 und §13: Component Container, CIF, CIDL-Modell, Home, Ports, Lifecycle, Lightweight CCM, PSS, TimerEventService, Dynamic API.
Einstiegspunkte: `src/container.rs`, `src/cif.rs`, `src/cidl.rs`, `src/component_def.rs`, `src/home.rs`, `src/port.rs`, `src/lifecycle.rs`, `src/lwccm.rs`, `src/pss.rs`, `src/timer.rs`, `src/time_psm.rs`, `src/orb_core.rs`, `src/orb_extensions.rs`, `src/cos_event_bridge.rs`.
Features: `std` zieht zusätzlich `flate2`; `cos-event` (bindet `zerodds-corba-cos-event` ein).
Tests: `crates/corba-ccm/tests/pi_service_contexts.rs`.
Abdeckung: `docs/spec-coverage/omg-ccm-4.0.md`.

### zerodds-corba-ccm-ejb
CCM↔EJB-Brücke: CosTransactions ↔ JTA-`UserTransaction`-Status, ConnectorBean-Lifecycle, JNDI ↔ CosNaming, Java-Stub-Generator (CCM 4.0 §16, JTA 1.3 §3.2).
Einstiegspunkte: `src/tx.rs`, `src/connector_bean.rs`, `src/naming_glue.rs`, `src/stub_gen.rs`.
Features: `std` zieht `zerodds-corba-ccm`.

### zerodds-corba-ccm-lib
Komponentenbibliothek für `corba-ccm`: DDS-Brücke, Persistenz (PSS §10), Telemetrie.
Einstiegspunkte: `src/dds_bridge.rs`, `src/persistence.rs`, `src/telemetry.rs`.

### zerodds-ccm
CCM 4.0 §6 Component Model auf IDL-Ebene: Equivalent-IDL-Transformation (Component/Home/EventType), `Components::*`-Grundtypen, Lightweight-Profil (§13), DDS4CCM.
Einstiegspunkte: `src/transform.rs`, `src/model.rs`, `src/validate.rs`, `src/lightweight.rs`, `src/dds4ccm.rs`.
Tests: `crates/ccm/tests/equivalent_idl.rs`.
Abdeckung: `docs/spec-coverage/omg-ccm-4.0.md`, `docs/spec-coverage/dds4ccm-1.1.md`.

### zerodds-ami4ccm
AMI4CCM 1.1 (`formal/2015-08-03`): Implied-IDL-Transformation (§7.3, §7.5), ReplyHandler/ExceptionHolder (§7.4), Pragma-Parser (§7.7), Connector, Deployment, Multiplex.
Einstiegspunkte: `src/transform.rs`, `src/pragma.rs`, `src/exception_holder.rs`, `src/connector.rs`, `src/deployment.rs`, `src/multiplex.rs`, `src/scope_resolver.rs`.
Tests: `crates/ami4ccm/tests/implied_idl.rs`.
Abdeckung: `docs/spec-coverage/omg-ami4ccm-1.1.md`.

---

## OPC UA

Alle vier Crates: `no_std + alloc`, `default = ["std"]`. Die Tests liegen als Unit-Tests in `src/` (für `opcua-uacp` und `opcua-server` zusammen 81 `#[test]`, gezählt per grep); ein `tests/`-Verzeichnis gibt es nicht. Fremdpeer ist open62541 über Shell-Beweise in `proofs/`. Abdeckung: `docs/spec-coverage/dds-opcua-1.0.md`, `opcua-client-server-1.05.md`, `opcua-pubsub-1.05.md`. Einen `docs/specs/`-Eintrag und einen Daemon gibt es für OPC UA nicht.

### zerodds-opcua-uacp
OPC UA Connection Protocol (Part 6 §7.1): `opc.tcp`-Framing mit Hello/Acknowledge/Error/ReverseHello, Message-Chunks, Sequence-Header; SecureChannel.
Einstiegspunkte: `src/connection.rs`, `src/securechannel.rs`, `src/crypto.rs`.
Features: `std`/`alloc` reichen an `opcua-pubsub` und `opcua-gateway` weiter; `crypto` (aes, cbc, hmac, sha2, rsa, sha1) für gesicherte SecurityPolicies (Part 6 §6.7).

### zerodds-opcua-server
Client/Server nach Part 4 über SecureChannel: Session-Lifecycle, Discovery-, Attribute-, View-, Method-, MonitoredItem- und Subscription-Services, In-Memory-AddressSpace, UserIdentityTokens (§7.40), Server und Client.
Einstiegspunkte: `src/server.rs` (`Server`, `ServerSecurity`), `src/client.rs` (`Client::connect`, `serve_connection`), `src/services.rs`, `src/address_space.rs`, `src/attributes.rs`, `src/identity.rs`, `src/status.rs`, `src/wire.rs`.
Features: `std`, `alloc`, `crypto` (reicht an `opcua-uacp/crypto` weiter).
Interop: `proofs/open62541.sh` (ZeroDDS-Client gegen `tutorial_server_variable` von open62541), Beispiel `crates/opcua-server/examples/open62541_probe.rs`.

### zerodds-opcua-gateway
OMG DDS-OPCUA Gateway 1.0 (`formal/2020-01-01`): Typsystem-Abbildung §8.2/§9.2 (25 Built-in-Typen, NodeId, ExpandedNodeId, Variant, DataValue, ExtensionObject) und AddressSpace-Abbildung §9.3 (Domain/Topic → ObjectNode, Sample → Variable). Keine Netzwerk-I/O.
Einstiegspunkte: `src/types.rs`, `src/node_id.rs`, `src/data_value.rs`, `src/address_space.rs`, `src/dds_to_ua/`, `src/subscription_mapping/`, `src/service_sets/`, `src/historical/`, `src/conformance.rs`, `src/xml.rs` (nur mit `std`).
Features: `std` (zieht `roxmltree`), `alloc`.

### zerodds-opcua-pubsub
OPC UA PubSub Part 14 (UADP): Part-6-Binärkodierung, NetworkMessage/DataSetMessage, PubSub-Konfiguration, Discovery, SecurityGroup/SKS, Transporte, Publisher/Subscriber-Laufzeit (`daemon.rs`) und DataSet ↔ DDS-Topic-Brücke (`bridge.rs`).
Einstiegspunkte: `src/binary/`, `src/uadp/`, `src/writer.rs`, `src/reader.rs`, `src/config.rs`, `src/daemon.rs` (`Publisher::publish_cycle`, `Subscriber::poll`), `src/bridge.rs` (`DataSetTopicMapping`, `DdsSample`), `src/transport.rs`, `src/security.rs`, `src/infomodel.rs`, `src/json.rs`, `src/dynamic.rs`.
Features: `std`, `alloc`, `security` (AES-CTR, HMAC-SHA256, AES-GCM; Part 14 §8), `json` (Part 14 §7.2.3; serde_json, base64).
Interop: `proofs/open62541_uadp.sh`, `proofs/build-open62541.sh`, `proofs/uadp_target_check.c`, Beispiele `crates/opcua-pubsub/examples/open62541_uadp_probe.rs`, `crates/opcua-pubsub/examples/open62541_uadp_publisher.rs`.

---

## zerodds-soap

**Zweck.** DDS-SOAP-PSM: SOAP-1.2-Envelope, WSDL-1.1- und 2.0-Erzeugung aus IDL-Service-Definitionen, MTOM/XOP, WS-Addressing 1.0, WS-Security 1.1 (UsernameToken, X.509-Token, Timestamp, BinarySecurityToken).
**Einstiegspunkte:** `src/envelope.rs`, `src/fault.rs`, `src/wsdl.rs`, `src/mtom.rs`, `src/addressing.rs`, `src/security.rs`.
**Features:** `default = ["std"]`, `std`, `alloc`.
**Einbindung:** Workspace-Mitglied (`Cargo.toml:294`); kein anderes Crate im Workspace hängt davon ab (grep über `crates/*/Cargo.toml`, `tools/*/Cargo.toml`). Kein Daemon, keine Spec-Datei unter `docs/specs/` oder `docs/spec-coverage/`.

---

## Offene Punkte und Widersprüche

- **CORBA-Daemon:** CSIv2 und das Veröffentlichen der IORs im Naming Service sind im Daemon nicht gebaut, obwohl `src/csiv2_wire.rs` und `zerodds-corba-cosnaming` existieren. `corba.csiv2.enabled`, `corba.csiv2.target_supports`, `corba.csiv2.gssup` und `corba.ior.publish_to_naming` werden mit Fundstelle abgewiesen („CSIv2 is not built in the daemon; authenticate with SSLIOP client certificates“), nicht still übergangen.
- **`tools/amqp-dds-endpoint`:** Die DDS-Seite ist nur `InMemoryDdsHost`; der Daemon mit DDS-Domäne ist `zerodds-amqp-bridged`.
- **Nicht ermittelt:** ob `docs/spec-coverage/zerodds-deployment-1.0.md` zu `zerodds-corba-dnc` gehört.

Behoben: die Modulköpfe von `zerodds-amqp-endpoint`, `tools/amqp-dds-endpoint`, `opcua-uacp`, `corba-rust`, der Kopf von `coap-bridge/tests/security_e2e.rs` und von `websocket-bridge/tests/cross_vendor.rs`; der Doc-Kommentar von `extract_mtls_subject` war bereits richtig.
