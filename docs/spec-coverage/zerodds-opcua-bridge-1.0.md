# `zerodds-opcua-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-opcua-bridge-1.0.md`

Implementation:

- `crates/opcua-bridge/` — der Daemon-Prozess (`zerodds-opcua-bridged`) über
  den vier Bibliothekscrates `opcua-gateway`, `opcua-uacp`, `opcua-server`,
  `opcua-pubsub`.

## §1 Conformance-Levels

### §1 Vier Gateway-1.0-Conformance-Points deklariert

**Spec:** §1/§2 — vier Conformance-Points (OPC UA→DDS basic/complete,
DDS→OPC UA basic/complete).

**Repo:** `crates/opcua-gateway/src/conformance.rs::DECLARED_CONFORMANCE`
(Bibliotheksebene, unverändert übernommen); Daemon verdrahtet §9 (DDS→OPC
UA), §8 (beide Richtungen), Part 14 und §9.3.4 (historisch, Point 4
„complete") über `opcua.server.historical.topics`.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::*` (Bibliothek);
`crates/opcua-server/src/client.rs::tests::e2e_loopback_history_read_*`
(HistoryRead-Service); `daemon_e2e.rs::
historical_topic_config_makes_history_read_work_end_to_end`.

**Status:** done — alle vier Conformance-Points inklusive Historical
Access jetzt im Daemon verdrahtet.

## §2 CLI-Surface

### §2 zerodds-opcua-bridged CLI

**Spec:** §2 — `--config`/`--check-config`/`--set`/`--bind`/`--domain`/
`--idl`/`--gateway-config`/`--log-level`/`--metrics`/`--version`/`--help`;
Exit-Codes 0/1/2/3/4.

**Repo:** `crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`,
`crates/opcua-bridge/src/daemon/cli.rs`.

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::*` (8 Tests:
Flag-Parsing, `=`-Form, `--idl`-Wiederholung, `sets()`-Rendering).

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader, geschichtet

**Spec:** §3 — `domain`/`log_level`/`opcua.{idl,gateway_config,server,
client,pubsub}`/`acl`/`metrics`; unbekannter Schlüssel in bekannter Sektion
stoppt den Start mit Datei+Zeile.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{DaemonConfig,
KEYS, is_key, from_config}`.

**Tests:** `crates/opcua-bridge/src/daemon/config.rs::tests::*` (20 Tests:
Minimalkonfiguration, unbekannte Sektion, Client-/PubSub-Einträge,
ACL-Regeln, ACL-Nutzer, gesicherte Policy ohne Zertifikat, Historical-
Topics, PubSub-Security-Schlüssel, OTLP-Endpoint).

**Status:** done — Config-Oberfläche um `opcua.server.historical.topics`,
`opcua.pubsub.connections[].security`, `acl.users`/`acl.allow_anonymous`
und `metrics.otlp_endpoint` erweitert.

## §4 Wire-Protokolle

### §4.1 opc.tcp + SecureChannel

**Spec:** §4.1 — Part 6 §7.1/§6.7, SecurityMode None immer, gesicherte
Policies unter Feature `crypto`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{accept_loop,
serve_connection_locked, build_server_security}` — ein
`Arc<Mutex<Server>>` über alle Verbindungen (Part 4 §5.6.2: mehrere
SecureChannels, Subscription-Transfer).

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
dcps_writer_reaches_the_opcua_client_read`,
`::opcua_client_write_reaches_the_dds_reader` (beide über echten
`opc.tcp`-Socket, `zerodds_opcua_server::client::TcpTransport`).

**Status:** done — inklusive gesicherter Policies:
`daemon_e2e.rs::secured_basic256sha256_sign_and_encrypt_session_
through_the_daemon` erzeugt echte RSA-signierte X.509-Zertifikate
(`rcgen`, Feature `crypto`) und fährt einen echten Basic256Sha256-
SignAndEncrypt-Handshake gegen den laufenden Daemon; vorher gab es dafür
keinen Test, nur der Bibliotheksebene-Test mit
`b"client-cert-der"`-Platzhalter-Bytes.

### §4.2 Part-4-Services

**Spec:** §4.2 — Session/Attribute/View/Method/Subscription/HistoryRead,
wie von `zerodds-opcua-server` implementiert, plus `Write`-Hook.

**Repo:** `zerodds-opcua-server::server::{Server::set_write_hook,
handle_service}` — neuer Hook, der jedes `Write`-Ziel vor dem
AddressSpace-Zugriff mit der aufrufenden Session-Identität anbietet;
der Daemon installiert ihn (`daemon/server.rs::start_with_reload`) für
die per-Session-ACL (§7.2). Ansonsten unverändert die Bibliothek; der
Daemon liefert Transport + AddressSpace.

**Tests:** siehe §4.1; `zerodds-opcua-server::client::tests::
write_hook_can_refuse_a_write_before_it_reaches_the_address_space`,
`::write_hook_observes_the_calling_sessions_identity`;
`daemon_e2e.rs::per_session_write_acl_allows_the_named_subject_and_
refuses_the_other`.

**Status:** done (Bibliotheksebene 68 Tests mit Feature `crypto`, siehe
`opcua-client-server-1.05.md`).

### §4.3 Part-14 UADP

**Spec:** §4.3 — ein Publisher/Subscriber-Paar pro
`opcua.pubsub.connections[]`, UDP-Multicast.

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security}`; Feature `pubsub-security`
(Passthrough zu `zerodds-opcua-pubsub/security`);
`daemon/config.rs::PubSubSecurityConfig`;
`daemon/server.rs::build_pubsub_security`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::*` (4, davon 2
Feature-`pubsub-security`: `secured_publisher_and_subscriber_round_
trip_a_dds_sample`, `subscriber_with_the_wrong_key_refuses_not_
silently_drops`); `daemon_e2e.rs::
dds_publish_reaches_an_in_tree_pubsub_subscriber` (echtes UDP-Multicast).

**Status:** done — PubSub-Message-Security (`SecurityGroup`/SKS) über
`opcua.pubsub.connections[].security` verdrahtet: ein statischer
aktueller Schlüssel, keine Live-`GetSecurityKeys`/SKS-Rotation (benannt,
nicht verschwiegen).

## §5 Topic-Mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — Typauflösung ohne kompilierten Rust-Typ.

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; opportunistischer Pfad
`crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::*` (3).

**Status:** done (konfigurierter IDL-Pfad, deterministisch getestet);
`resolve_discovered` ist **ungemessen** — kein Test treibt eine echte
Cross-Participant-`getTypes`-Antwort in die Registry, siehe §6.2 der
Spezifikationsdatei.

### §5.2 AddressSpace-Form (§9.3)

**Spec:** §5.2 — Domain→Topic→Sample-Baum.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{ensure_domain,
ensure_topic, ensure_objects_folder, materialize_instance,
materialize_instance_with_override, ensure_browse_path, apply_sample}`.

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::*`
(13 Tests, inkl. `dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable`,
`node_id_override_replaces_the_default_root_identity`,
`node_id_override_colliding_with_an_existing_node_is_refused`,
`browse_path_organizes_the_root_from_the_objects_folder`,
`objects_folder_is_browsable_after_materializing_a_topic`);
`daemon_e2e.rs::node_id_and_browse_path_override_is_applied_not_just_
parsed`, `::colliding_node_id_override_is_a_load_time_error_not_a_log_
line`.

**Status:** done — `<browse_path>`/`<node_id>`-Override aus der Spec-§10-XML
wird jetzt angewandt: `<node_id>` ersetzt die Sample-Root-Identität (nach
Kollisionsprüfung — eine Kollision ist ein Ladezeit-Fehler, kein Start);
`<browse_path>` fügt eine `Organizes`-Referenzkette vom `Objects`-Ordner
hinzu. Nebenbefund behoben: der `Objects`-Ordner selbst (ns0;i=85) hatte
bis dahin keinen `NodeMeta`-Eintrag, sodass Browse ab dem Standard-
Einstiegspunkt immer `Bad_NodeIdUnknown` lieferte.

### §5.3 Typformen-Abdeckung

**Spec:** §5.3 — welche DDS-XTypes-Formen einen Live-Wert bekommen.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::*` (10 Tests,
inkl. `sequence_of_struct_is_reported_skipped_not_dropped_silently`,
`wide_char_maps_to_uint16_and_back`, `wide_string_maps_to_string_and_
back`, `map_of_primitives_decomposes_to_one_leaf_per_key_and_back`,
`map_partial_write_keeps_the_untouched_key`).

**Status:** `partial` — Primitive/Strings (narrow und wide, `wchar`→
`UInt16`/`wstring`→`String`)/Enums/Bitmasks/verschachtelte Structs/
`map<K,V>` (ein Leaf pro Eintrag, Pfad `<feld>.<schlüssel>`)/Arrays-
Sequences von alledem: done. Array/Sequence von Struct/Union, `bitset`,
`fixed<P,S>`, `float128`: offen, benannt statt angenähert — kein
OPC-UA-Builtin-`VariantValue`-Fall passt (Part 6 §5.1.2s Typtabelle hat
keine Zeile für keinen der drei; aus dieser Abwesenheit gefolgert, nicht
aus einer konkreten Gateway-1.0-§9.2-Textstelle, die dieses Repo vorliegen
hätte). Offen, nicht stillschweigend: ein neuer `map`-Schlüssel, den eine
Probe noch nie gesehen hat, wird als unbekannter Pfad gemeldet, nicht
live als neuer Knoten materialisiert (`AddressSpace`/`PublishedDataSet`
bauen ihre Knoten-/Feldmenge einmalig aus dem Table-9-Default).

### §5.4 Historical Access (§9.3.4)

**Spec:** §9.3.4.4 — `Historizing`/`AccessLevel`-HistoryRead-Bit, HistoryRead-
Service.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{HistoricalConfig,
HistoricalTopicConfig}`; `address_space_build.rs::materialize_root`
(Historizing-Markierung); `zerodds-opcua-server::server::Server::
history_read`; `zerodds-opcua-gateway::historical::adapter::
InMemoryHistoryCache::push_bounded`.

**Tests:** `zerodds-opcua-server::client::tests::e2e_loopback_history_
read_*` (6 Tests); `zerodds-opcua-server::services::tests::history_
read_*_round_trips` (2); `zerodds-opcua-gateway::historical::adapter::
tests::push_bounded_evicts_the_oldest_sample_past_max_depth`,
`::ensure_stream_makes_an_empty_read_ok_not_unknown_instance`;
`daemon_e2e.rs::historical_topic_config_makes_history_read_work_end_
to_end`.

**Status:** done — Raw-Range und AtTime, echte Pagination über ein
opakes Continuation-Point; `ReadRawModifiedDetails.is_read_modified ==
true` wird abgelehnt (DDS kennt keine „modified value history"), nicht
stillschweigend als einfacher Read beantwortet. Nebenbefund behoben: die
Pump-Schleife übergab vorher konstant `0` als `source_timestamp`, was
die Retention jedes Knotens auf eine Probe kollabierte.

## §6 OPC UA ↔ DDS Bridging-Verhalten

### §6.1 DDS → OPC UA

**Spec:** §6.1.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop` (Teil
„dds_to_ua_jobs").

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
dcps_writer_reaches_the_opcua_client_read`.

**Status:** done

### §6.2 OPC UA → DDS

**Spec:** §6.2 — zwei Mechanismen: Poll-basierte Diff-Erkennung für Writes
gegen den eigenen Server; Client-Rolle gegen konfigurierte Fremd-Server.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop` (Teil
„ua_to_dds_jobs"); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::
opcua_client_write_reaches_the_dds_reader`;
`crates/opcua-bridge/src/ua_client.rs::tests::
client_job_writes_a_monitored_scalar_node_into_dds`.

**Status:** done — der Write-Hook (§4.2) prüft `acl.rules[].subject` jetzt
synchron pro Session, *bevor* ein Write das AddressSpace erreicht; die
in der Spezifikationsdatei §6.2 benannte engere Einschränkung bleibt für
einen bereits autorisierten Write bestehen (weiterhin Poll-basierte
DDS-Weiterleitung, kein Fehler-Rückkanal zum Client bei einem
fehlschlagenden DDS-Write) — in diesem Pass unverändert, nicht behoben.

## §7 Security

### §7.1 SecureChannel-Materialbeschaffung

**Spec:** §7.1 — Zertifikat/Key-Dateien, zwei getrennte Peer-Dateien.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{load_private_key_pem,
load_public_key_pem, load_der}`.

**Tests:** `daemon_e2e.rs::secured_basic256sha256_sign_and_encrypt_
session_through_the_daemon` — echte RSA-signierte X.509-Zertifikate
(`rcgen`), über `load_der`/`load_private_key_pem`/`load_public_key_pem`
geladen, echter Basic256Sha256-SignAndEncrypt-Handshake gegen den
laufenden Daemon.

**Status:** done — Konfigurationsvalidierung (Paar-Pflicht) weiterhin
getestet (`config.rs::tests::secured_policy_without_certificate_is_
refused`); jetzt zusätzlich das Laden echter PEM/DER-Dateien und der
vollständige Handshake, vorher ungetestet.

### §7.2 ACL

**Spec:** §7.2 — themenweise Allow-Liste, plus Subject-Dimension pro
Session.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{Acl, AclOp,
topic_exposed, allow_write, client_subject}`;
`daemon/config.rs::{AclUserConfig, acl_users, acl_allow_anonymous}`;
`daemon/server.rs::start_with_reload` (Write-Hook-Installation,
`UserStore`-Aufbau).

**Tests:** `crates/opcua-bridge/src/daemon/security.rs::tests::*` (6
Tests, inkl. `client_subject_renders_each_identity_kind`);
`daemon/config.rs::tests::acl_users_parse_with_default_policy_id`,
`::acl_user_without_password_is_refused`; `daemon_e2e.rs::
per_session_write_acl_allows_the_named_subject_and_refuses_the_other`
(zwei echte Sessions, alice erlaubt, bob nicht — über echtes TCP + echtes
DDS).

**Status:** done — beide Ebenen erzwungen: Themen-Exposition zur
Konfigurationsladezeit *und*, für Writes gegen den eigenen Server,
pro-Session über den Write-Hook. `acl.users`/`acl.allow_anonymous` geben
dem `UserStore` echte, unterscheidbare Identitäten — ohne sie war jede
Session Anonymous und `acl.rules[].subject` hatte nichts, worauf es sich
unterscheiden könnte.

## §8 Betrieb + Observability

### §8.1 Logging

**Spec:** §8.1.

**Repo:** durchgängig `eprintln!` mit Themen-/Pfad-Namen in
`crates/opcua-bridge/src/daemon/server.rs`.

**Tests:** — (manuell verifiziert, kein Test parst stderr).

**Status:** done

### §8.2 Prometheus-Metriken

**Spec:** §8.2.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics,
serve_metrics}`.

**Tests:** manuell verifiziert (`curl /metrics` gegen einen laufenden
Prozess während der Entwicklung); kein automatisierter Test liest den
Endpunkt.

**Status:** `partial` — Endpunkt funktioniert (gemessen), kein
automatisierter Test.

### §8.3 OTLP-Spans

**Spec:** §8.3.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{otlp_config_from_
endpoint, emit_lifecycle_span, new_trace_id, new_span_id}`;
`daemon/config.rs::metrics_otlp_endpoint`.

**Tests:** `daemon_e2e.rs::otlp_endpoint_receives_a_span_on_opc_tcp_
accept` (ein echter Fake-OTLP/HTTP-Collector empfängt den echten POST
des laufenden Daemons bei einem echten `opc.tcp`-Accept).

**Status:** done — ein Span
(`zerodds-opcua-bridged.opc_tcp.accept`, `SpanKind::Server`) pro
akzeptierter `opc.tcp`-Verbindung, alle 5 Sekunden und einmal beim
Shutdown geflusht. Grobkörnig, wie `zerodds_foundation::tracing`s eigene
Designlinie vorgibt („endpoint lifecycle... not per-sample latencies") —
keine Per-Sample-Spans.

## §9 Lifecycle

### §9.1/§9.2 Start + Shutdown

**Spec:** §9.1/§9.2.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload,
DaemonHandle::shutdown}`.

**Tests:** manuell gemessen (SIGTERM gegen einen laufenden Prozess,
Exit-Code 0, siehe Commit-Historie); `crates/opcua-bridge/tests/daemon_e2e.rs`s vier Tests rufen
`handle.shutdown()` am Ende jeweils selbst auf.

**Status:** done — ein echter Fehler wurde dabei gefunden und behoben:
`zerodds_monitor::server::serve_prometheus`s Accept-Loop hatte keinen
Shutdown-Pfad (blockierendes `accept()`, kein Stop-Flag, kein von außen
erreichbares Listener-Handle zum Droppen); ersetzt durch
`serve_metrics` mit Non-Blocking-Accept + Stop-Flag-Polling.

### §9.3 SIGHUP

**Spec:** §9.3 — nur `acl.default_deny`/`acl.rules` neu geladen.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs` (Reload-Thread in
`start_with_reload`).

**Tests:** — (kein automatisierter Test sendet SIGHUP).

**Status:** `partial` — Code-Pfad vorhanden, nicht automatisiert getestet;
Reload-Umfang bewusst nicht erweitert: `opcua.server.historical.topics`,
`acl.users`/`acl.allow_anonymous`, `opcua.pubsub.connections[].security`
und `metrics.otlp_endpoint` brauchen alle einen Neustart, wie zuvor
`<node_id>`/`<browse_path>` und der IDL/Gateway-Config-Themensatz.

## §10 Cross-Vendor-Interop

**Spec:** §10 — open62541 1.5.4.

**Repo:** README verweist auf `crates/opcua-server/proofs/open62541.sh` +
`crates/opcua-pubsub/proofs/open62541_uadp.sh` (Bibliotheksebene);
`crates/opcua-bridge/tests/open62541_interop.rs` +
`tests/foreign/{open62541_client.c,README.md}` für die Daemon-Ebene: ein
kleiner C-Client gegen `libopen62541`s öffentliche API (open62541 liefert
kein generisches Browse/Read/Write-CLI wie `mosquitto_pub`/`coap-client`),
Browse/Read/Write gegen einen echten laufenden `zerodds-opcua-bridged`.

**Tests:** `open62541_interop.rs::open62541_browses_reads_and_writes_through_the_daemon`
— `#[ignore]`, `OPCUA_OPEN62541=1` gesetzt und
open62541 nicht gefunden schlägt hart fehl (kein stiller Erfolg), gemäß
dem Muster von `crates/coap-bridge/tests/libcoap_interop_e2e.rs`. Von
diesem Agenten **nicht gelaufen** (kein open62541 auf diesem Host,
macOS statt Linux) — der Skip-Pfad und der Hart-Fehl-Pfad ohne
Werkzeug sind gemessen; der eigentliche Handshake gegen echtes
open62541 ist ungemessen.

**Status:** `partial` — Daemon-Ebene-Test existiert und ist strukturell
verifiziert (Skip/Hart-Fehl-Pfade), aber nicht gegen ein echtes
open62541 gelaufen; Bibliotheksebene weiterhin gemessen
(`crates/opcua-server/proofs/open62541.sh`).

## §11 Packaging

**Spec:** §11.

**Repo:** `packaging/linux/systemd/zerodds-opcua-bridged.service`,
`packaging/linux/configs/opcua-bridged.yaml.example`,
`man/man1/zerodds-opcua-bridged.1`,
`man/man5/zerodds-opcua-bridged.yaml.5`, Einträge in
`packaging/linux/{deb/assemble-debs.sh,rpm/zerodds.spec,arch/PKGBUILD,
appimage/build.sh}`.

**Tests:** —

**Status:** done

## §12 Testing

**Spec:** §12.

**Repo:** siehe §1–§9 dieser Datei für die Einzelabdeckung.

**Tests:** `cargo test -p zerodds-opcua-bridge --all-features` — 76
Lib-Tests + 10 Integrationstests (`crates/opcua-bridge/tests/daemon_e2e.rs`, davon einer
Feature-`crypto`) + 1 (`crates/opcua-bridge/tests/example_yaml_loadback.rs`) + 1 `#[ignore]`d
(`open62541_interop.rs`, §10). Ohne `pubsub-security` 74 Lib-Tests (2
Feature-gated). `crates/opcua-bridge/tests/daemon_e2e.rs` ist gemessen unter Standard-Parallelität
flaky (jeder Test einzeln und mit `--test-threads=1` grün) — dieselbe
DDS-Domain-Lease-Eigenschaft, keine Regression.

Auf Linux gemessener und behobener Fund: jeder UDP-Multicast-Empfänger-
Socket in diesem Crate (der Daemon-eigene `subscribers[]`-konfigurierte
`SubscriberJob` *und* die eigens gebaute Verifikations-Subscriber im
Test) band an den eigenen, meist zufälligen Port von `local_addr`
(Default `0.0.0.0:0`) statt an den Port des Multicast-Ziels — UDP
demultiplext nach Ziel-(IP,Port) im Kernel, ein an einen beliebigen Port
gebundener Socket empfängt nie ein an den echten Port adressiertes
Datagramm. Auf macOS lief der fehlerhafte Code zufällig noch durch
(keine verlässliche Eigenschaft); auf Linux gemessen als Ausfall.
Behoben (`daemon/server.rs`s Subscriber-Schleife bindet jetzt
`SocketAddr::new(local.ip(), multicast.port())`); neuer Test
`external_pubsub_publisher_reaches_the_daemons_own_subscriber` deckt den
zuvor ungetesteten Daemon-eigenen Subscriber-Pfad ab.

**Status:** done (Unit + In-Process-E2E inklusive gesichertem
SecureChannel, Historical Access, PubSub-Security, OTLP, per-Session-ACL);
Multi-Vendor-Automatisierung teilweise (§10 — Test existiert,
`#[ignore]`d, nicht gegen echtes open62541 gelaufen).

## §13/§14 Querverweise + Versionierung

**Status:** `n/a (informative)`

## Audit-Status

18 done / 4 teilweise / 0 offen / 2 n/a (informativ, §13+§14). Round 2
schloss alle drei zuvor „offen"-Punkte: §5.3 (jetzt teilweise statt
offen — `wchar`/`wstring`/`map` gemappt, nur noch Array-von-Aggregat/
`bitset`/`fixed<P,S>`/`float128` ohne Live-Wert), §8.3 (OTLP, jetzt
done) und §10 (open62541-Daemon-Test jetzt vorhanden, jetzt teilweise
statt offen — nicht gegen echtes open62541 gelaufen).
