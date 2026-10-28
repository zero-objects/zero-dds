# Systemaufbau

Diese Seite beschreibt, aus welchen Bausteinen ein ZeroDDS-System besteht und wie sie zusammenhängen: Anwendung, Laufzeitkonfiguration, Transporte, Discovery, Persistenz, Routing, Aufzeichnung, Monitoring, Bridges und ROS 2. Danach folgen Rezepte für typische Aufbauten mit den Dateien und Befehlen aus dem Repository. Grundlage ist der Code. Aussagen, die nur aus einem Doku-Kommentar stammen, sind als „(laut Doku-Kommentar)“ markiert. Nichts auf dieser Seite wurde für diese Seite gebaut oder ausgeführt.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Bausteine im Überblick

| Baustein | Pflicht | Artefakt | Fundstelle |
|---|---|---|---|
| Anwendung | ja | Rust-Crate `zerodds-rs` / `zerodds-dcps`, oder C-Bibliothek `libzerodds` mit Sprachbindung | `crates/rs`, `crates/dcps`, `crates/zerodds-c-api` (`[lib] name = "zerodds"`), `endpoints/*` |
| Laufzeitkonfiguration | nein (Defaults gelten) | `zerodds.yaml`, `zerodds.d/`, Umgebung `ZERODDS_*`, `--set` | `crates/config`, `crates/dcps/src/runtime.rs` (`SETTINGS`) |
| Transport | ja (UDPv4 Default) | UDP, TCP, SHM, UDS, TSN | `crates/transport-*`, `transport.user_transport(s)` |
| Discovery | ja | SPDP/SEDP, Multicast oder Unicast-Peers | `discovery.*` |
| Persistenz | nein | `zerodds-durability-svc` + Store | `crates/durability-service-bin`, `crates/durability-store-*` |
| Routing | nein | `zerodds-router` | `crates/routing-service` |
| Aufzeichnung | nein | `zerodds-record`, `zerodds-replay` | `tools/record`, `tools/replay`, `crates/recorder` |
| Monitoring | nein | `zerodds-monitor`, Prometheus-Endpunkte der Bridges, OTLP | `tools/monitor`, `crates/monitor`, `crates/observability-otlp` |
| Betriebswerkzeuge | nein | `zerodds-admin`, `zerodds-spy` | `tools/admin`, `tools/spy` |
| Bridges | nein | `zerodds-{ws,mqtt,coap,grpc,amqp,corba}-bridged` | `crates/*-bridge`, `crates/amqp-endpoint`, `crates/corba-dds-bridge` |
| ROS 2 | nein | `rmw_zerodds_cpp` | `crates/rmw-zerodds-shim`, `crates/ros2-rmw` |

## Anwendung

- Rust: `zerodds-rs` ist die Fassade über `zerodds-dcps`, `zerodds-dcps-async`, `zerodds-qos`, `zerodds-types`, `zerodds-monitor`, `zerodds-recorder` (`crates/rs/Cargo.toml`). `zerodds-rs` hat kein `security`-Feature; wer DDS-Security braucht, hängt `zerodds-dcps` mit `features = ["security"]` direkt an (siehe [security.md](security.md)).
- Teilnehmer anlegen: `DomainParticipantFactory::create(domain_id)` liefert einen `ParticipantBuilder` mit `with_qos`, `with_config`, `with_security` (unter Feature `security`), `build` (`crates/dcps/src/factory.rs:378`). Ohne `with_config` liest `build` die Standardschichten über `RuntimeConfig::from_standard_paths()`.
- Andere Sprachen: C-ABI `libzerodds` (`crates/zerodds-c-api`, Header `crates/zerodds-c-api/include/zerodds.h`). Die Endpunkt-SDKs liegen unter `endpoints/` (ada, c, cpp, csharp, go, java, python, rust, typescript u. a.). Details: [crates-languages-codegen.md](crates-languages-codegen.md).
- Die Domain-ID ist kein Konfigurationsschlüssel der Laufzeit (nicht in `SETTINGS`). Sie wird im Code übergeben, bei Werkzeugen mit `-d`, bei Bridges mit dem YAML-Schlüssel `domain`, bei ROS 2 mit `ROS_DOMAIN_ID`.

## Laufzeitkonfiguration

Geprüft in `crates/config/src/layers.rs`, `crates/config/src/load.rs`, `crates/dcps/src/runtime.rs`.

Vorrang, niedrigster zuerst:

1. einkompilierte Defaults,
2. Umgebung (`ZERODDS_*`, Tabelle `SETTINGS` in `crates/dcps/src/runtime.rs:3034`),
3. Dateien: Basisdatei, dann Drop-in-Verzeichnisse (Dateien nach Namen sortiert, nur `.yaml`), dann `include:`,
4. `--set PFAD=WERT` (mehrfach; `--set +PFAD=WERT` hängt an eine Liste an).

Systemverzeichnis (`SystemLayout::dir`):

| Plattform | Basisdatei | Drop-ins |
|---|---|---|
| Linux | `/etc/zerodds/zerodds.yaml` | `/etc/zerodds/zerodds.d/`, `/run/zerodds/zerodds.d/` |
| macOS | `/usr/local/etc/zerodds/zerodds.yaml` | `/usr/local/etc/zerodds/zerodds.d/` |
| Windows | `%PROGRAMDATA%\ZeroDDS\zerodds.yaml` | `%PROGRAMDATA%\ZeroDDS\zerodds.d\` |

- Eine fehlende Basisdatei ist kein Fehler (Defaults gelten). Mit `--config PFAD` ist sie Pflicht, und statt des Systemverzeichnisses gilt nur `PFAD` plus Geschwisterverzeichnis `PFAD.d/` (`loader_for`).
- Jedes Programm, das `zerodds_dcps::cli::Configured` nutzt, kennt `--config`, `--set`, `--check-config`. `--check-config` gibt jeden wirksamen Wert mit Herkunft aus; Exit 0 gültig, 1 ungültiger Wert oder unbekannter Schlüssel, 2 unlesbare Datei, Syntaxfehler, Include-Zyklus (`crates/dcps/src/cli.rs:46`).
- Unbekannte Schlüssel unter `--set` werden abgelehnt. `ros2.*` gehört der RMW und wird von anderen Programmen nicht als unbekannt gemeldet (`is_rmw_path`).
- Bridges lesen ihre eigene Basisdatei (`<stem>.yaml`, z. B. `ws-bridged.yaml`) mit denselben Schichten; Laufzeitschlüssel wie `discovery.peers` sind dort ebenfalls zulässig (`ConfigSource::for_program`). Eine Bridge, die mit `--config /etc/zerodds/ws-bridged.yaml` startet, liest `/etc/zerodds/zerodds.yaml` nicht, außer über `include:` (abgeleitet aus `loader_for`).

Vollständige Schlüsselliste: [config.md](config.md).

## Transporte

Discovery (SPDP/SEDP) läuft immer über UDPv4. Der gewählte Transport trägt nur die Nutzdaten (`select_user_transport`, `crates/dcps/src/runtime.rs:1608`).

| Transport | Einstellung | Voraussetzung | Einsatz |
|---|---|---|---|
| UDPv4 | Default; `transport.user_transport: UDPv4` | – | Standard, host- und herstellerübergreifend |
| UDPv6 | `UDPv6` | – | IPv6-Nutzdaten; SPDP bleibt IPv4 |
| TCPv4/TCPv6 | `TCPv4` / `TCPv6` | – | Netze, die UDP-Nutzdaten verwerfen; Discovery bleibt UDP |
| SHM | `SHM` | Feature `same-host-shm` (in `zerodds-c-api` Default, in `zerodds-dcps` nicht) | gleicher Host, POSIX-Shared-Memory |
| UDS | `UDS` | Feature `same-host-uds` | gleicher Host ohne `/dev/shm` (Container); bei beiden Features gewinnt SHM (laut Doku-Kommentar in `crates/dcps/Cargo.toml`) |
| TSN | `TSN`, `tsn.interface`, `tsn.vlan`, `tsn.pcp` | Feature `tsn-live`, nur Linux | RTPS direkt auf Ethernet, EtherType 0x88B5 |

- `transport.user_transports` ist eine Liste in Vorzugsreihenfolge. Jede Sendung geht an den ersten Transport, dessen Locator-Art zum Ziel passt; empfangen wird auf allen. Beispiel `[SHM, UDPv4]`. Ist die Liste gesetzt, wird `transport.user_transport` ignoriert.
- `transport.interface` (IPv4) bindet alle IP-Sockets an eine Schnittstelle (SPDP-Unicast, SPDP-Multicast-Senden, Nutzdaten UDP/TCP).
- Automatischer SHM-Pfad: Mit Feature `same-host-shm` öffnen zwei ZeroDDS-Teilnehmer auf demselben Host einen SHM-Kanal, sobald SPDP den anderen meldet; jedes Unicast-Datagramm zwischen ihnen geht hindurch, was der Ring nicht annimmt, über UDP (`crates/dcps/src/same_host_channel.rs`). Abschalten mit `transport.same_host_shm` / `ZERODDS_SAME_HOST_SHM=0`.
- `delivery.mode` (`portable` | `raw-same-host` | `iceoryx`, env `ZERODDS_DELIVERY_MODE`) ist der Teilnehmer-Default der Zustellart. Gelesen wird er im C-API-Loan-Pfad (`crates/zerodds-c-api/src/shm_loan_ffi.rs`) und in der ROS-2-Shim. `raw-same-host` geht nicht auf den Draht. `iceoryx` braucht je Writer einen iceoryx2-Service (`zerodds_dw_enable_iceoryx`, Feature `delivery-iceoryx`; das RMW leitet ihn aus dem Topic ab). Wo der Modus nicht greift, sendet der Writer `portable` und meldet das (C-API einmal je Writer auf stderr, RMW mit Grund über `RCUTILS_LOG_WARN`). `DataWriter::write` in Rust ist immer `portable`.

## Discovery

Ports nach RTPS 2.5 §9.6.1.4.1 (`crates/rtps/src/wire_types.rs:1019`): PB = 7400, DG = 250, PG = 2, d0 = 0, d1 = 10.

| Zweck | Port | Domain 0 |
|---|---|---|
| SPDP-Multicast (Gruppe 239.255.0.1) | 7400 + 250·d | 7400 |
| SPDP-Unicast, Teilnehmerindex i | 7400 + 250·d + 10 + 2·i | 7410, 7412, … |
| Nutzdaten-Unicast | ephemerer Port (Bind auf Port 0) | – |

- Teilnehmerindex: Der Runtime-Start probiert i = 0 … 119 und nimmt den ersten freien Port. Sind alle belegt, bindet er ephemer; dann ist nur Multicast-Discovery möglich (`crates/dcps/src/runtime.rs:9150`).
- Nutzdaten laufen über einen ephemeren Port, nicht über den Port d3 der Spezifikation. Eine Firewall muss daher mehr als die SPDP-Ports freigeben (abgeleitet aus `select_user_transport`; nicht gemessen).

Schlüssel (Umgebungsname in Klammern):

| Schlüssel | Wirkung |
|---|---|
| `discovery.peers` (`ZERODDS_PEERS`) | Liste `ip` oder `ip:port`. Nur `ip` wird zu den SPDP-Unicast-Ports der Indizes 0 … `discovery.max_peer_participants − 1` erweitert (Default 10, Bereich 1..=120). Beacons gehen zusätzlich zu Multicast an diese Adressen. |
| `discovery.no_multicast` (`ZERODDS_NO_MULTICAST`) | `true`: kein einziges SPDP-Multicast-Paket, nur `discovery.peers`. |
| `discovery.server` (`ZERODDS_DISCOVERY_SERVER`) | `true`: Teilnehmer leitet SPDP zwischen seinen Clients weiter; SEDP läuft direkt zwischen den Teilnehmern. Ohne DDS-Security-Relay (laut Doku-Kommentar). |
| `discovery.spdp_multicast_group` | andere Multicast-Gruppe (IPv4) |
| `discovery.multicast_allowlist` | nur diese Gruppen werden beigetreten; andere werden bei der Erstellung abgelehnt |
| `discovery.spdp_period_ms`, `discovery.participant_lease_duration_ms`, `discovery.initial_announce_count`, `discovery.initial_announce_period_ms` | Taktung und Lease |

Vordefinierte Profile im Code: `RuntimeConfig::multi_robot()` (ohne Multicast, Lease 300 s; `crates/dcps/src/runtime.rs:4916`) und `RuntimeConfig::discovery_server()` (`runtime.rs:4942`).

## Persistenz

- Im Prozess: Ein PERSISTENT-Writer legt seinen Store unter `durability.dir` an (env `ZERODDS_DURABILITY_DIR`, Default `(temp dir)/zerodds-durability`).
- Eigenständig: `zerodds-durability-svc`, ein Prozess pro Domain (`crates/durability-service-bin/src/main.rs`).

```text
zerodds-durability-svc --domain N [--store sqlite|file|lakehouse|postgres] [--path PATH] [--auto] [--topic NAME]...
```

| Store | Crate | Build | `--path` |
|---|---|---|---|
| `sqlite` (Default) | `durability-store-sqlite` | immer | DB-Datei; ohne Pfad im Speicher, nicht dauerhaft |
| `file` | `durability-store-file` | immer | Verzeichnis, Pflicht |
| `lakehouse` (DuckDB) | `durability-store-lakehouse` | `--features lakehouse`, braucht `libduckdb` | DB-Datei |
| `postgres` | `durability-store-postgres` | `--features postgres` (`postgres-timescaledb` für Hypertables) | libpq-Verbindungszeichenkette, Pflicht |

`--auto` bedient jedes Topic, dessen Writer TRANSIENT oder PERSISTENT deklarieren. `--auto` oder mindestens ein `--topic` ist Pflicht.

## Routing zwischen Domains

`zerodds-router` (`crates/routing-service/src/bin/zerodds-router.rs`):

```text
zerodds-router run      --routes <datei.json|datei.xml> [--types <shapes.json>] [--config <yaml>] [--set k=v]
zerodds-router validate --routes <datei>
```

- Route: `input`/`output` mit `domain`, `topic`, `type_name`, `keyed`, `partition`, `qos` (`reliable`, `durability`, `ownership`, `ownership_strength`, `data_representation`); dazu `filter`, `transform`, `loop_guard`, `preserve_source_timestamp`, `max_samples_per_second`, `tenant` (`crates/routing-service/src/config.rs`). Optional `access_control`.
- Beispiel: `packaging/linux/configs/router.json.example` (Domain 0 → 1, Partition-Fanout, Typ `zerodds::RawBytes`).
- Filter und Transformation brauchen `--types`.
- Alle Routen eines Prozesses teilen eine `RuntimeConfig`. Eine Route kann keine eigene Netzwerkschnittstelle wählen; Trennung erfolgt über Domain und Partition (abgeleitet aus den Feldern von `Endpoint`).
- Kein SIGHUP-Reload; Routenänderung erfordert Neustart (Kommentar in `packaging/linux/systemd/zerodds-router.service`).

## Aufzeichnung und Wiedergabe

| Befehl | Zweck |
|---|---|
| `zerodds-record record -t TOPIC … [-o DATEI] [-d DOMAIN] [--duration DAUER]` | Mitschnitt in `.zddsrec` |
| `zerodds-record record … --decode --type-file IDL [--out-json F] [--out-sqlite F]` | zusätzlich typisiert dekodiert |
| `zerodds-record info DATEI`, `list DATEI` | Kopf, Frames pro Topic |
| `zerodds-replay inspect|dump DATEI` | Inhalt ansehen |
| `zerodds-replay replay DATEI [--time-scale F] [--topic NAME]… [--inject [--inject-domain N]]` | Wiedergabe; mit `--inject` in eine laufende Domain |

`replay` nimmt `.zddsrec`, `.json`/`.ndjson` und `.db`/`.sqlite` an (Hilfetext in `tools/replay/src/main.rs`). Format: `docs/specs/zddsrec-1.0.md`.

## Monitoring

- Metrik-Registry: `zerodds-monitor` (Crate, `crates/monitor`), 31 Standardmetriken und 9 Spans laut Crate-Doku, Prometheus-Textformat über `render_prometheus`, HTTP-Endpunkt `serve_prometheus` (Feature `prometheus-server`). `zerodds-dcps` hängt die Hooks mit Default-Feature `metrics` ein.
- Werkzeug `zerodds-monitor` (`tools/monitor`): `snapshot [-d ID] [--duration DAUER] [-f text|prometheus]`, `serve [-d ID] [-a ADRESSE]` (Default `127.0.0.1:9991`), `names`. Das Werkzeug gibt die Registry des eigenen Prozesses aus (`default_registry()` nach Beitritt zur Domain). Metriken einer anderen Anwendung erscheinen dort nicht; die Anwendung muss ihre Registry selbst ausliefern (abgeleitet aus `run_snapshot`; nicht gemessen).
- Bridges: Block `metrics: { enabled, listen }` mit `/metrics`, `/catalog`, `/healthz`. Default-Adressen in den Beispielkonfigurationen: ws 9091, mqtt 9092, coap 9093, amqp 9094, grpc 9095, corba 9096 (alle `127.0.0.1`).
- OTLP: `crates/observability-otlp`, OTLP/HTTP/JSON an `/v1/traces`, `/v1/metrics`, `/v1/logs`. Die Bridges und die ROS-2-Shim hängen das Crate an; aktiviert wird der Export über `OTEL_EXPORTER_OTLP_ENDPOINT` (geprüft für `zerodds-ws-bridged`, `otlp_config_from_env`). Fehler beim Senden werden verworfen.
- Laufzeit-Ereignisse: `diagnostics.observability` (`off`/`none` oder `stderr`/`json`) schreibt JSON-Ereignisse nach stderr.

## Betriebswerkzeuge

| Werkzeug | Zweck |
|---|---|
| `zerodds-admin domain inspect <id>` | Teilnehmer und Endpunkte einer laufenden Domain |
| `zerodds-admin discovery snapshot <id>` | rohe SPDP/SEDP-Sicht |
| `zerodds-admin config inspect|qos validate|qos check <datei.xml>` | DDS-XML offline prüfen |
| `zerodds-spy -t TOPIC [-d ID] [--decode --type-file IDL]` | Samples eines Topics ausgeben |

Weitere Werkzeuge (`zerodds-pcap`, `zerodds-dashboard`, `zerodds-perf` u. a.): [tools-and-man.md](tools-and-man.md).

## Bridges

Sechs Daemons mit gemeinsamer Sicherheitsschicht `crates/bridge-security` (TLS, Auth, Topic-ACL; siehe [security.md](security.md)):

| Daemon | Crate | Beispielkonfiguration |
|---|---|---|
| `zerodds-ws-bridged` | `crates/websocket-bridge` | `packaging/linux/configs/ws-bridged.yaml.example` |
| `zerodds-mqtt-bridged` | `crates/mqtt-bridge` | `mqtt-bridged.yaml.example` |
| `zerodds-coap-bridged` | `crates/coap-bridge` | `coap-bridged.yaml.example` |
| `zerodds-grpc-bridged` | `crates/grpc-bridge` | `grpc-bridged.yaml.example` |
| `zerodds-amqp-bridged` | `crates/amqp-endpoint` | `amqp-bridged.yaml.example` |
| `zerodds-corba-bridged` | `crates/corba-dds-bridge` | `corba-bridged.yaml.example` |

Weitere Kopplungen ohne eigenen Daemon in dieser Liste (zenoh, OPC UA, XRCE, iceoryx): [crates-bridges-services.md](crates-bridges-services.md).

## ROS 2

- RMW: `RMW_IMPLEMENTATION=rmw_zerodds_cpp` (`crates/rmw-zerodds-shim/README.md`). Paketdateien in `crates/rmw-zerodds-shim/ament/`.
- Konfiguration: Abschnitt `ros2.*` in denselben Laufzeitdateien. Beispiel-Drop-in `packaging/linux/configs/ros2.yaml.example` → `/etc/zerodds/zerodds.d/50-ros2.yaml`. Prüfen mit `zerodds-ros2-shim --check-config`.
- Namespace, Domain und Logging gehören rcl und werden im Abschnitt `ros2` abgelehnt. Discovery über `discovery.no_multicast`/`discovery.peers`, Enklave über `security.enclave_dir`.

Details: [ros2.md](ros2.md).

## Rezepte

### 1. Zwei Prozesse auf einem Host

1. Keine Konfiguration nötig. Beide Prozesse nutzen dieselbe Domain-ID. Der erste Teilnehmer bindet SPDP-Unicast 7410, der zweite 7412 (Domain 0).
2. Optional gemeinsamer Speicher statt UDP für die Nutzdaten:
   ```yaml
   # /etc/zerodds/zerodds.d/10-transport.yaml
   transport:
     user_transports: [SHM, UDPv4]
   ```
   Voraussetzung: Feature `same-host-shm` (Default in `libzerodds`; für Rust `zerodds-dcps` mit `features = ["same-host-shm"]`). Ohne dieses Feature wird der Wert `SHM` beim Start als ungültig abgelehnt (`parse_user_transport`, `valid_runtime_value`).
3. Für C-API-Writer mit Loan-Pfad: `delivery.mode: raw-same-host`, wenn alle Leser auf demselben Host sind (keine Sendung auf den Draht).
4. Kontrolle: `zerodds-admin domain inspect 0`, `zerodds-spy -t <Topic>`.

Hinweis: Discovery-Tests über Loopback sind auf macOS unzuverlässig (Kommentare in `crates/dcps/tests/security_live_e2e.rs`, `security_matrix_e2e.rs`).

### 2. Mehrere Hosts mit Multicast

1. Alle Hosts im selben L2-Segment, gleiche Domain-ID.
2. Firewall: UDP 7400 + 250·d (Multicast 239.255.0.1), 7410 + 250·d + 2·i für jeden Teilnehmerindex, dazu die ephemeren Nutzdatenports.
3. Bei mehreren Schnittstellen die Schnittstelle festlegen:
   ```yaml
   transport:
     interface: 192.168.10.5
   ```
4. Optional auf die eigene Gruppe beschränken: `discovery.multicast_allowlist: [239.255.0.1]`.
5. Prüfen: `<programm> --check-config` zeigt die wirksamen Werte mit Herkunft; `zerodds-admin discovery snapshot 0` zeigt die gefundenen Teilnehmer.

### 3. Hosts ohne Multicast

Variante A, statische Peers:

```yaml
# /etc/zerodds/zerodds.d/20-discovery.yaml
discovery:
  no_multicast: true
  peers:
    - 10.0.0.11          # erweitert auf 7410, 7412, … (Indizes 0..9)
    - 10.0.0.12:7412     # genau ein Port
  max_peer_participants: 10
```

Gleichwertig in der Umgebung: `ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=10.0.0.11,10.0.0.12:7412`.

Variante B, Discovery-Server:

- Server: `discovery.server: true`, `discovery.no_multicast: true` (entspricht `RuntimeConfig::discovery_server()`).
- Clients: `discovery.no_multicast: true`, `discovery.peers: [<server-ip>]`.
- SEDP und Nutzdaten laufen danach direkt zwischen den Teilnehmern; die Clients müssen sich gegenseitig per UDP erreichen.

Variante C, TCP für Nutzdaten:

- `transport.user_transport: TCPv4`. Discovery bleibt UDP; Variante A oder B wird weiterhin gebraucht. TCP bindet einen ephemeren Port (`TcpTransport::bind_v4(pinned, 0)`).

### 4. System mit Persistenz und einer Bridge

1. Durability-Service pro Domain:
   ```sh
   zerodds-durability-svc --domain 0 --store sqlite --path /var/lib/zerodds/durability/dur.db --auto
   ```
2. Anwendungs-Writer deklarieren TRANSIENT oder PERSISTENT; `--auto` erfasst sie ohne Anwendungscode.
3. Bridge, hier MQTT:
   ```sh
   cp /usr/share/zerodds/configs/mqtt-bridged.yaml.example /etc/zerodds/mqtt-bridged.yaml
   zerodds-mqtt-bridged --config /etc/zerodds/mqtt-bridged.yaml --check-config
   zerodds-mqtt-bridged --config /etc/zerodds/mqtt-bridged.yaml
   ```
   (Pfad `/usr/share/zerodds/configs/` aus `packaging/linux/deb/postinst.tmpl`.) Laufzeitschlüssel wie `discovery.peers` gehören bei `--config` in die Bridge-Datei oder in `mqtt-bridged.d/`.
4. Sicherheit der Bridge: [security.md](security.md), Abschnitt Bridges.
5. Metriken: `metrics.enabled: true`, `metrics.listen: "127.0.0.1:9092"`.

### 5. Installation als Dienst

systemd (Linux), Dateien in `packaging/linux/systemd/`:

| Datei | Ziel (laut Kopfkommentar) |
|---|---|
| `zerodds-<daemon>.service` | `/usr/lib/systemd/system/` |
| `zerodds-sysusers.conf` | `/usr/lib/sysusers.d/zerodds.conf` (Benutzer `zerodds`) |
| `zerodds-tmpfiles.conf` | `/usr/lib/tmpfiles.d/zerodds.conf` (`/var/log/zerodds`, `/var/lib/zerodds`, `/etc/zerodds`, `/etc/zerodds/certs` mit 0700) |

```sh
install -m 0644 packaging/linux/systemd/zerodds-sysusers.conf /usr/lib/sysusers.d/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-tmpfiles.conf /usr/lib/tmpfiles.d/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-ws-bridged.service /usr/lib/systemd/system/
systemd-sysusers && systemd-tmpfiles --create
cp packaging/linux/configs/ws-bridged.yaml.example /etc/zerodds/ws-bridged.yaml
systemctl daemon-reload
systemctl enable --now zerodds-ws-bridged
systemctl reload zerodds-ws-bridged     # ExecReload = SIGHUP: TLS-Material und ACL neu
```

Die Befehle sind Standard-systemd; im Repository stehen `daemon-reload`, `deb-systemd-helper preset` und `try-restart` in `packaging/linux/deb/postinst.tmpl`. Die Pakete aktivieren Dienste bei der Erstinstallation nicht.

| Unit | `Type` | `ExecStart` | Reload |
|---|---|---|---|
| `zerodds-{ws,mqtt,coap,grpc,amqp,corba}-bridged` | `notify` | `/usr/bin/zerodds-<x>-bridged --config /etc/zerodds/<x>-bridged.yaml` | SIGHUP |
| `zerodds-router` | `exec` | `/usr/bin/zerodds-router run --routes /etc/zerodds/router.json` | keiner |
| `zerodds-durability-svc` | `exec` | Parameter über `Environment=ZERODDS_DURABILITY_{DOMAIN,STORE,PATH,EXTRA}` | keiner |

- `zerodds-durability-svc.service` wird vom RPM nicht installiert (`packaging/linux/rpm/zerodds.spec:197`); sie muss von Hand kopiert werden. Eine Instanz pro Domain.
- Router und Durability-Service lesen die Laufzeitkonfiguration aus dem Systemverzeichnis (`/etc/zerodds/zerodds.yaml`), da ihr `ExecStart` kein `--config` setzt.
- Alle Units: `User=zerodds`, `ProtectSystem=strict`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`. Ein Store außerhalb von `ReadWritePaths` ist nicht beschreibbar.

launchd (macOS), Dateien in `packaging/macos/launchd/` (nur die sechs Bridges, kein Router, kein Durability-Service):

```sh
sudo cp packaging/macos/launchd/org.zerodds.ws-bridged.plist /Library/LaunchDaemons/
sudo cp packaging/linux/configs/ws-bridged.yaml.example /usr/local/etc/zerodds/ws-bridged.yaml
sudo launchctl load -w /Library/LaunchDaemons/org.zerodds.ws-bridged.plist
```

- Die Plists starten `/usr/local/bin/zerodds-<x>-bridged --config /usr/local/etc/zerodds/<x>-bridged.yaml` als `_zerodds`, Logs unter `/usr/local/var/log/zerodds/`.
- Den Benutzer `_zerodds` legt das Preinstall-Skript in `packaging/macos/pkg/build-pkg.sh` per `dscl` an; das Postinstall lädt nur `org.zerodds.ws-bridged.plist`.

Windows: `packaging/windows/services/Install-Services.ps1` (nicht ausgewertet).

## Offene Punkte aus dem Code

Keine.
