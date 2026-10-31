# Konfiguration

Diese Seite ist die Referenz aller Konfigurationsquellen von ZeroDDS: das Ebenenmodell (Defaults, Umgebung, Dateien, `--set`) und `--check-config`, jede Einstellung der Laufzeitdatei `zerodds.yaml` einschließlich `ros2.*`, die Schlüssel der Daemon-Dateien (`ws-bridged.yaml`, `mqtt-bridged.yaml`, `coap-bridged.yaml`, `amqp-bridged.yaml`, `grpc-bridged.yaml`, `corba-bridged.yaml`), die Routen-Datei des Routers, QoS-Profile über DDS-XML, die Stellen, an denen Security-Dateien konfiguriert werden, und alle Umgebungsvariablen außerhalb der Tabelle `SETTINGS`. Grundlage ist der Code auf `main` (Stand `fac89e645c9`). Defaults stammen aus dem Code; wo sie zusätzlich mit einem Binary gemessen wurden, steht das dabei. Aussagen, die nur aus Doku-Kommentaren stammen, sind mit „(laut Doku-Kommentar)“ markiert.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## 1 Ebenenmodell

### 1.1 Ergebnis

Jedes ZeroDDS-Programm, das über `zerodds_dcps::cli::Configured` oder `ConfigSource::for_program` startet, liest vier Ebenen. Die spätere gewinnt.

| Ebene | Quelle | Herkunftsangabe in `--check-config` | Implementierung |
|---|---|---|---|
| 1 | eingebaute Defaults | `(built-in default)` | `RuntimeConfig::from_validated_source`, `crates/dcps/src/runtime.rs:4273`; Daemon: `default_for_dev()` bzw. `impl Default` |
| 2 | Umgebung unter den Namen der Tabelle `SETTINGS` (plus Programm-Namen wie `MQTT_PASSWORD`) | `(environment $NAME)` | `env_layer`, `crates/config/src/layers.rs:99`; Tabelle `crates/dcps/src/runtime.rs:3034` |
| 3 | Dateien: Basis, Drop-in-Verzeichnisse, `include:` | `(Pfad:Zeile)` | `Loader`, `crates/config/src/load.rs`; Auswahl `loader_for`, `crates/config/src/layers.rs:468` |
| 4 | `--set pfad=wert`, wiederholbar, in Reihenfolge | ``(command line `--set …`)`` | `CommandLine::layer`/`apply`, `crates/config/src/layers.rs` |

Eine Datei überschreibt also eine Umgebungsvariable. `--config` fügt keine Ebene hinzu, es wählt die Basis von Ebene 3. `ConfigSource::for_program` (`crates/dcps/src/runtime.rs:3320`) setzt die Ebenen zusammen; `Configured::from_command_line` (`crates/dcps/src/cli.rs:155`) validiert und warnt.

### 1.2 Dateiauswahl (Ebene 3)

| Fall | Basisdatei | Drop-ins, in dieser Reihenfolge | Basis fehlt |
|---|---|---|---|
| ohne `--config`, Linux und andere Unix | `/etc/zerodds/<stem>.yaml` | `/etc/zerodds/<stem>.d/`, dann `/run/zerodds/<stem>.d/` | erlaubt, Defaults gelten |
| ohne `--config`, macOS | `/usr/local/etc/zerodds/<stem>.yaml` | `/usr/local/etc/zerodds/<stem>.d/` | erlaubt |
| ohne `--config`, Windows | `%PROGRAMDATA%\ZeroDDS\<stem>.yaml` (ohne `PROGRAMDATA`: `C:\ProgramData\ZeroDDS`) | `…\ZeroDDS\<stem>.d\` | erlaubt |
| `--config PATH` | `PATH` | `PATH` mit Endung `.d` (`Path::with_extension("d")`: `/x/a.yaml` → `/x/a.d/`) | Fehler (`require_base`) |

Fundstellen: `SystemLayout::dir`, `crates/config/src/layers.rs:421`; `SystemLayout::for_stem`, `:441`; `loader_for`, `:468`.

Regeln des Loaders (`crates/config/src/load.rs`, `crates/config/src/lib.rs`):

- Aus Drop-in-Verzeichnissen werden nur Dateien mit Endung `.yaml` gelesen, nach Dateinamen sortiert. `.rpmnew`, `.dpkg-dist` und Editor-Backups fallen dadurch heraus. Ein fehlendes Drop-in-Verzeichnis ist kein Fehler.
- `include:` nimmt einen Pfad oder eine Liste. Pfade sind relativ zur einbindenden Datei. Ein Include darf eine Datei oder ein Verzeichnis sein. Includes werden zuerst gelesen, danach die eigenen Werte der Datei, unabhängig von der Textposition des Schlüssels. Explizite Includes müssen existieren. Zyklen (`IncludeCycle`) und mehr als `MAX_INCLUDE_DEPTH` = 16 Ebenen (`IncludeTooDeep`) werden abgelehnt (`load.rs:19`, `:235`, `:270`).
- Zusammenführen: Maps Schlüssel für Schlüssel, Skalare und Listen ersetzen. `+name` hängt an die Liste unter `name` an; ist `name` keine Liste, folgt `AppendToNonSeq`.
- Ein Listenwert aus der Umgebung (Komma-getrennt) ist in Ebene 3 bereits eine Sequenz, ein Drop-in kann mit `+name` anhängen. Ein leerer Wert (`FOO=`) gilt als gesetzt und ergibt die leere Liste.
- YAML-Teilmenge: Einrückung mit zwei Leerzeichen, `key: value`, `- item`, flache Inline-Listen `[a, "b"]`, `#`-Kommentare. Anker, Inline-Maps, verschachtelte Inline-Listen, Multi-Dokument-Streams und Block-Skalare werden mit Namen abgelehnt (`crates/config/src/lib.rs`, Modulkommentar; gemessen: `a: &x 1` → „anchors (`&name`) are not read here“).
- `${VAR}` und `${VAR:-default}` werden vor dem Parsen aus der Prozessumgebung ersetzt. Eine ungesetzte Variable ohne Default wird zum leeren Text (`expand_env_vars`, `crates/config/src/parse.rs:24`).
- Verschachtelungstiefe höchstens `MAX_NESTING_DEPTH` = 32 (`crates/config/src/lib.rs:106`).
- Werte unter Pfaden mit `token`, `password`, `secret`, `passphrase`, `psk`, `sasl_users`, `credentials` erscheinen in Berichten und Herkunftsangaben als `***` (`is_secret_path`, `crates/config/src/layers.rs:504`).

### 1.3 Welches Programm liest welche Basisdatei

| Programm | `stem` | eigene Schlüssel | Quelle |
|---|---|---|---|
| alle CLI-Tools (`zerodds-spy`, `-bench`, `-admin`, `-monitor`, `-record`, `-snitch`, `-mq`), `zerodds-router`, `zerodds-durability-svc` | `zerodds` | keine | `Configured::from_args`/`from_env_args` |
| RMW `rmw_zerodds_cpp` | `zerodds` | `ros2.*` | `process_source`, `crates/rmw-zerodds-shim/src/ros2_config.rs:102` |
| `zerodds-ros2-shim` | `zerodds` | `ros2.*` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs:62` |
| C-API `zerodds_runtime_create` | `zerodds` | keine | `crates/zerodds-c-api/src/lib.rs:1200` |
| C-API `zerodds_runtime_create_from_config(domain, path, sets, n_sets)` | `zerodds`, `path` = `--config`, `NULL` = Systemverzeichnis | keine | `crates/zerodds-c-api/src/lib.rs:1232` |
| `zerodds-ws-bridged` | `ws-bridged` | 17 | `CONFIG_STEM`, `crates/websocket-bridge/src/daemon/config.rs:25` |
| `zerodds-mqtt-bridged` | `mqtt-bridged` | 25 | `crates/mqtt-bridge/src/daemon/config.rs:21` |
| `zerodds-coap-bridged` | `coap-bridged` | 32 | `crates/coap-bridge/src/daemon/config.rs:21` |
| `zerodds-amqp-bridged` | `amqp-bridged` | 28 | `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs:30` |
| `zerodds-grpc-bridged` | `grpc-bridged` | 19 | `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1457` |
| `zerodds-corba-bridged` | `corba-bridged` | 23 | `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs:30` |

Ein Bridge-Daemon liest ohne `--config` seine eigene Systemdatei (`/etc/zerodds/ws-bridged.yaml` usw.) und nicht `zerodds.yaml`. Die Laufzeit-Schlüssel (`transport.*`, `discovery.*` …) dürfen in derselben Bridge-Datei stehen und gelten dort für die DDS-Seite des Daemons. Gemessen: `transport.recv_batch: 64` in einer `--config`-Datei von `zerodds-ws-bridged` erscheint unter `values:` mit Datei und Zeile.

Der RMW nimmt `--config` und `--set` aus der Kommandozeile des ROS-Prozesses, außerhalb der `--ros-args … --`-Blöcke, die rcl gehören (`application_args`, `ros2_config.rs:74`). `--check-config` auf der Kommandozeile eines Knotens wird nicht ausgewertet; der Bericht dazu ist `zerodds-ros2-shim --check-config` mit denselben Flags (laut Doku-Kommentar, `ros2_config.rs:96`).

### 1.4 Verhalten beim Start

| Befund | Laufzeit und CLI-Tools | Bridge-Daemons | RMW |
|---|---|---|---|
| ungültiger Wert einer bekannten Einstellung | Start abgebrochen | Start abgebrochen | `rmw_init` schlägt fehl |
| unbekannter Schlüssel in einer Datei | Warnung auf stderr: `warning: <Herkunft>: unknown setting <pfad> is not read` (`cli.rs:170`) | innerhalb einer bekannten Sektion: Fehler aus `from_config`; sonst Warnung | unbekannter `ros2.*`-Schlüssel: Fehler |
| `--set` auf einen Pfad, den nichts liest | Fehler `--set <pfad>: not a known setting` | Fehler | Fehler |
| Datei unlesbar, Syntaxfehler, Include-Zyklus | Fehler | Fehler | Fehler |

Die Meldung nennt immer die Ebene: `Datei:Zeile`, `environment $NAME` oder das `--set`. Gemessen mit `ZERODDS_RECV_BATCH=abc zerodds-spy --check-config`: `error: environment $ZERODDS_RECV_BATCH: invalid transport.recv_batch: got `abc`, expected an integer > 0`.

### 1.5 `--check-config`

`--check-config` lädt alle vier Ebenen wie ein Start und gibt dann aus (`ConfigSource::check_report`, `crates/dcps/src/runtime.rs:3371`):

1. `sources:` die beteiligten Quellen in Lesereihenfolge (`environment`, Dateien, `command line`), sonst `(none; built-in defaults)`,
2. `values:` jede der 51 Laufzeit-Einstellungen mit Wert und Herkunft, Defaults mit `[default]`,
3. `program settings:` die eigenen Schlüssel des Programms, die gesetzt sind (nur Bridges und `zerodds-ros2-shim`; Geheimnisse als `***`),
4. je Befund eine Zeile `error: …`,
5. `configuration valid` oder `configuration invalid`.

`zerodds-ros2-shim --check-config` hängt einen Block `ros2:` mit Mangling, Profilen und Overrides an.

Exit-Codes (Code: `cli.rs:186` und die `main`-Funktionen; gemessen mit den Binaries in `target/debug`):

| Situation | CLI-Tools, Router, Durability-Service | Bridge-Daemons |
|---|---|---|
| gültig | 0 | 0 |
| ungültiger Wert oder unbekannter Schlüssel | 1 | 1 |
| unlesbare Datei, Syntaxfehler, Include-Fehler, unbekannter `--set`-Pfad | 2 | 1 |

Gemessen: `zerodds-spy --config /nonexist.yaml --check-config` → 2; `zerodds-ws-bridged --config /nonexist.yaml --check-config` → 2, ohne `--check-config` → 1; `zerodds-amqp-bridged` mit Syntaxfehler → 2 unter `--check-config`, 1 beim Start; `zerodds-spy --set transport.tick=3 --check-config` → 2; `zerodds-spy --set runtime.tick_period_ms=0 --check-config` → 1.

## 2 Laufzeit-Einstellungen (`zerodds.yaml`)

### 2.1 Ergebnis

- 101 Einstellungen stehen in `SETTINGS` (`crates/dcps/src/runtime.rs:3034`), jede mit Dateipfad und Umgebungsvariable.
- 1 Einstellung ist nur in Dateien möglich: `transport.interface_bindings` (`FILE_SETTINGS`, `runtime.rs:4089`), weil ihr Wert eine Liste von Maps ist.
- Zusammen 102 Laufzeit-Einstellungen. Alle 101 `SETTINGS`-Einträge werden beim Aufbau in `from_validated_source` (`runtime.rs:4273`) über `source.var(…)` gelesen (gegengeprüft für jeden Namen): 89 dort selbst, die 7 `ports.*` davon über die Hilfsfunktion `port`, `transport.shm_max_datagram` unter seinem Dateipfad statt unter dem Variablennamen (`ConfigSource::var` nimmt beides); die 6 `security.*`-Formen in `SecurityVendorSettings::from_source` (`runtime.rs:2979`) und die 6 `transport.udp.*` in `udp_config_from` (`runtime.rs:4213`), die `from_validated_source` aufruft.
- Dazu kommen 6 weitere `ros2.*`-Pfade, die nur der RMW liest (§2.4). `ros2.time_based_filter` steht in beiden Tabellen.

Spalte „Default“: Text aus `runtime_default_text` (`runtime.rs:3686`), den `zerodds-spy --check-config` und `zerodds-ros2-shim --check-config` ohne Dateien und ohne Umgebung ausgeben; für diesen Stand aus dem Code gelesen und mit `man/man5/zerodds.yaml.5` abgeglichen, nicht mit den Binaries gemessen. „Fundstelle“ ist die Zeile in `crates/dcps/src/runtime.rs`, an der der Wert gelesen wird, in `from_validated_source` oder in der genannten Funktion; Wertebereiche aus `valid_runtime_value` (`runtime.rs:3916`) und `runtime_expectation` (`runtime.rs:3819`).

Wahrheitswerte: `true`/`false` oder `1`/`0` (`runtime_boolean`, `runtime.rs:4182`). Listen: in Dateien als Sequenz, in Umgebung und `--set` als Komma-Liste (`runtime_list`, `runtime.rs:4058`).

### 2.2 Tabelle

| Pfad | Typ/Werte | Default | Umgebungsvariable | Wirkung | Fundstelle |
|---|---|---|---|---|---|
| `runtime.tick_period_ms` | ms, Ganzzahl > 0 | 5 | `ZERODDS_TICK_PERIOD_MS` | Periode des Runtime-Ticks (Heartbeats, Timer) | 4295 |
| `runtime.scheduler_tick` | bool | true | `ZERODDS_SCHEDULER_TICK` | ereignisgesteuerter Deadline-Heap-Scheduler; `false` = fester `tick_loop` | 4692 |
| `transport.max_message_size` | Bytes, ≥ `DEFAULT_MTU` | 14720 | `ZERODDS_MAX_MESSAGE_SIZE` | größte RTPS-Nachricht beim Senden | 4343 |
| `transport.shm_max_datagram` | Bytes, > 0 | 65536 | `ZERODDS_SHM_MAX_DATAGRAM` | Datagramm-Obergrenze des Same-Host-SHM-Transports (Feature SHM nötig) | 4325 |
| `transport.same_host_shm` | bool | `true` mit Feature `same-host-shm`, sonst `false` | `ZERODDS_SAME_HOST_SHM` | Same-Host-SHM für Writer/Reader auf demselben Rechner; `false` = jedes Paar über den User-Transport; `true` ohne das Feature verweigert `DcpsRuntime::start` | 4332 |
| `transport.recv_batch` | Ganzzahl > 0 | 32 | `ZERODDS_RECV_BATCH` | Datagramme pro `recv_batch`-Aufruf; `1` = einzelnes `recv()` | 4353 |
| `transport.batch_at_zero_budget` | bool | false | `ZERODDS_BATCH_AT_ZERO_BUDGET` | Messschalter für Batching bei Latenzbudget 0 | 4358 |
| `transport.interface` | IPv4-Adresse | (auto) | `ZERODDS_INTERFACE` | Interface für Announce und Bind; unset = Routenprobe | 4409 |
| `transport.user_transport` | `UDPv4`, `UDPv6`, `TCPv4`, `TCPv6`; `SHM`, `UDS`, `TSN` nur mit Feature | UDPv4 | `ZERODDS_USER_TRANSPORT` | Transport der Nutzdaten; Discovery bleibt UDPv4 | 4469 |
| `transport.user_transports` | Liste derselben Werte | (none) | `ZERODDS_USER_TRANSPORTS` | mehrere Nutzdaten-Transporte, erster bevorzugt; nicht leer = `user_transport` ignoriert | 4473 |
| `transport.user_multicast` | bool | true | `ZERODDS_USER_MULTICAST` | tritt der SPDP-Gruppe auf dem Port PB + DG · domainId + d2 bei (DDSI-RTPS 2.5 §9.6.2.3), empfängt dort Nutzdaten und kündigt den Locator in der `defaultMulticastLocatorList` an; ein Peer darf dann Nutzdaten per Multicast senden, die Writer von ZeroDDS senden nur an Unicast-Locatoren; ein abgelehnter Beitritt wird protokolliert, der Participant läuft ohne; `false` für einen Host ohne Multicast | 4340 |
| `transport.interface_bindings` | Liste von Maps (§2.3), nur Datei | (none) | – | Interface-Bindungen für `NetInterface`-gebundene Security-Regeln | 4492 |
| `reader.max_samples_per_proxy` | Ganzzahl > 0 | 65536 | `ZERODDS_MAX_SAMPLES_PER_PROXY` | Samples, die ein Reader je Writer-Proxy bei einer Lücke puffert | 4348 |
| `reader.max_reassembly_sample_bytes` | Bytes, > 0 | 16777216 | `ZERODDS_MAX_SAMPLE_BYTES` | Obergrenze eines reassemblierten Samples (DoS-Schutz) | 4432 |
| `reader.heartbeat_response_delay_ms` | ms, Ganzzahl ≥ 0 | 500 | `ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS` | Wartezeit eines zuverlässigen Readers zwischen HEARTBEAT und Antwort (DDSI-RTPS 2.5 §8.4.10.1 `heartbeatResponseDelay`; Spec-Default); `0` antwortet im nächsten Tick; gilt für die Reader der Anwendung, die eingebauten Reader behalten den Default | 4522 |
| `reader.heartbeat_suppression_duration_ms` | ms, Ganzzahl ≥ 0 | 0 | `ZERODDS_READER_HEARTBEAT_SUPPRESSION_DURATION_MS` | ein HEARTBEAT, der weniger als diese Zeit nach dem vorigen desselben Writers ankommt, wird ignoriert (§8.4.10.1 `heartbeatSuppressionDuration`; Spec-Default) | 4529 |
| `reader.honor_data_max_size_serialized` | bool | true | `ZERODDS_READER_HONOR_DATA_MAX_SIZE_SERIALIZED` | ein Reader weist einen SerializedPayload zurück, der größer ist als das `dataMaxSizeSerialized`, das der Writer angekündigt hat (DDSI-RTPS 2.5 §8.4.10.4, Table 8.72); `false` nimmt ihn an, für einen Writer, der seine eigene Ankündigung überschreitet (Fast DDS 3.6.1: 160 Oktette angekündigt, 100 036 gesendet, OMG dds-rtps Test_LargeData_0) | 4533 |
| `reader.dispose_with_unregister` | bool | true | `ZERODDS_READER_DISPOSE_WITH_UNREGISTER` | ein Reader nimmt einen Lifecycle-Marker mit DisposedFlag und UnregisteredFlag (PID_STATUS_INFO, DDSI-RTPS 2.5 §9.6.3.9) als Dispose und Unregister; `false` nimmt ihn nur als Unregister, die Instanz wird NOT_ALIVE_NO_WRITERS, sobald ihr letzter Writer weg ist (OMG dds-rtps Test_FinalInstanceState_2 mit einem Publisher, der beim Löschen des Writers in einem Marker disposed und deregistriert, gemessen OpenDDS 3.35.0-dev) | 4536 |
| `reader.time_based_filter_clock` | `sender`, `receiver` | sender | `ZERODDS_READER_TIME_BASED_FILTER_CLOCK` | Uhr, auf der TIME_BASED_FILTER `minimum_separation` misst (DDS 1.4 §2.2.3.12 lässt es offen): `sender` = Quellzeitstempel der Änderung (INFO_TS), ohne einen die Empfangszeit; `receiver` = Empfangszeit | 4379 |
| `writer.nack_response_delay_ms` | ms, Ganzzahl ≥ 0 | 200 | `ZERODDS_NACK_RESPONSE_DELAY_MS` | Wartezeit eines zuverlässigen Writers zwischen NACK und Reparatur (DDSI-RTPS 2.5 §8.4.7.1 `nackResponseDelay`; Spec-Default) | 4507 |
| `writer.heartbeat_period_ms` | ms, Ganzzahl > 0 | 100 | `ZERODDS_WRITER_HEARTBEAT_PERIOD_MS` | wie oft ein zuverlässiger Writer einem Reader, der nicht alles bestätigt hat, seine History ankündigt (DDSI-RTPS 2.5 §8.4.7.1 `heartbeatPeriod`; §8.4.7.1 nennt keinen Wert) | 4514 |
| `writer.nack_suppression_duration_ms` | ms, Ganzzahl ≥ 0 | 0 | `ZERODDS_NACK_SUPPRESSION_DURATION_MS` | ein NACK für eine Änderung, die vor weniger als dieser Zeit gesendet wurde, wird ignoriert (§8.4.7.1 `nackSuppressionDuration`) | 4539 |
| `writer.unacked_window_bytes` | Bytes, Ganzzahl ≥ 0 | 524288 | `ZERODDS_WRITER_UNACKED_WINDOW` | Flusskontrolle eines Writers mit zuverlässigen Readern: so viele Bytes Nutzdaten dürfen unbestätigt unterwegs sein, darüber wartet `write()` auf ACKs, bis `max_blocking_time`, und sendet dann trotzdem (TIMEOUT kommt nur von den RESOURCE_LIMITS); `0` = keine | 4546 |
| `writer.unregister_on_delete` | bool | true | `ZERODDS_WRITER_UNREGISTER_ON_DELETE` | beim Löschen eines DataWriters ein Unregister je Instanz senden und, mit WRITER_DATA_LIFECYCLE `autodispose_unregistered_instances`, ein Dispose (DDS 1.4 §2.2.2.4.1.6, §2.2.3.21); `false` sendet beides nicht, die Reader setzen die Instanzen auf NOT_ALIVE_NO_WRITERS, sobald das Dispose der Publikation über SEDP ankommt, wie bei Connext, Fast DDS, CoreDX und HDDS als Publisher (OMG dds-rtps Test_FinalInstanceState_2) | 4560 |
| `writer.linger_ms` | ms, Ganzzahl ≥ 0 | 1000 | `ZERODDS_WRITER_LINGER_MS` | wie lange das Löschen eines zuverlässigen DataWriters auf die Bestätigung seiner zuverlässigen Reader wartet, die Marker für Unregister und Dispose aus DDS 1.4 §2.2.2.4.1.6 eingeschlossen, bevor der Writer geht und sein Dispose über SEDP gesendet wird; ein Writer, dessen Reader alles bestätigt haben, geht sofort; `0` = sofort | 4563 |
| `writer.group_coherent_set_pid` | bool | true | `ZERODDS_WRITER_GROUP_COHERENT_SET_PID` | `PID_GROUP_COHERENT_SET` auf den DATA eines gruppenkohärenten Sets unter PRESENTATION `access_scope` GROUP, wie DDSI-RTPS 2.5 §8.7.6 verlangt; `false` lässt es für jeden Reader weg: HDDS und Connext lesen gruppenkohärente Sets nur ohne es richtig (OMG dds-rtps Test_CoherentSets_12, _18, _21), ein Subscriber von ZeroDDS kennt dann die Mitglieder eines Sets nicht mehr | 4567 |
| `writer.group_info_on_wire` | bool | true | `ZERODDS_WRITER_GROUP_INFO_ON_WIRE` | Gruppeninformation nach DDSI-RTPS 2.5 §8.7.5/§8.7.6 unter PRESENTATION `access_scope` GROUP senden: `PID_GROUP_SEQ_NUM`, `PID_WRITER_GROUP_INFO`, `PID_GROUP_COHERENT_SET` und das GroupInfoFlag in HEARTBEAT und GAP; `false` sendet das Coherent-Set jedes Writers wie unter TOPIC, die Form, die HDDS und zzdds senden und Connext, HDDS und zzdds lesen (OMG dds-rtps Test_CoherentSets_12, _18, _21); ein Subscriber von ZeroDDS ordnet dann nach Writer statt nach Gruppe. Für die Teilnehmer eines Herstellers allein: `DcpsRuntime::set_vendor_rtps_behaviour` | 4570 |
| `writer.keyless_lifecycle_on_wire` | bool | false | `ZERODDS_WRITER_KEYLESS_LIFECYCLE_ON_WIRE` | Dispose und Unregister eines Writers eines Topics ohne Key senden; DDSI-RTPS 2.5 §8.2.10.1 gibt beiden „no effect if the topicKind==NO_KEY“, daher ändert sich per Default nur der Instanzzustand des Writers (DDS 1.4 §2.2.1.2.2, eine Instanz), kein Marker geht hinaus und `DcpsRuntime::keyless_lifecycle_not_sent` zählt ihn; `true` sendet den Marker (nur Statusbits) für einen Peer, der ihn erwartet | 4573 |
| `dcps.presentation_instance_coherence` | `per_instance`, `none` | per_instance | `ZERODDS_PRESENTATION_INSTANCE_COHERENCE` | kohärente Sets unter PRESENTATION `access_scope` INSTANCE: `per_instance` folgt der Tabelle in DDS 1.4 §2.2.3.6 (die Änderungen eines Sets an einer Instanz erscheinen zusammen, sobald das Set geschlossen ist; geprüft von OMG dds-rtps Test_CoherentSets_10 und _19), `none` dem Text darunter (die Samples erscheinen beim Eintreffen) | 4383 |
| `discovery.participant_name` | nichtleerer Text | zerodds:<GUID prefix in hex> | `ZERODDS_PARTICIPANT_NAME` | Name, den der Participant als `PID_ENTITY_NAME` ankündigt (EntityName_t, DDSI-RTPS 2.5 Table 9.18); die Endpunkte kündigen einen Namen aus Art, Topic und Entity-ID an | 4337 |
| `discovery.spdp_multicast_group` | IPv4-Multicast | 239.255.0.1 | `ZERODDS_SPDP_MC_GROUP` | SPDP-Multicast-Gruppe | 4399 |
| `discovery.spdp_period_ms` | ms, > 0 | 30000 | `ZERODDS_SPDP_PERIOD_MS` | SPDP-Announce-Periode; ohne eigenen Wert bei einer Lease unter 90 s ein Drittel von `discovery.participant_lease_duration_ms`, mindestens 100 ms (`default_spdp_period_for`) | 4283 |
| `discovery.spdp_new_sequence_per_period` | bool | false | `ZERODDS_SPDP_NEW_SEQUENCE_PER_PERIOD` | `true` = jede SPDP-Ankündigung mit neuer Sequenznummer, wie Cyclone DDS und OpenDDS; per Default nach DDSI-RTPS 2.5 §8.5.3.1 eine neue Nummer nur mit geänderten Participant-Daten | 4550 |
| `discovery.initial_announce_count` | Ganzzahl ≥ 0 | 10 | `ZERODDS_INITIAL_ANNOUNCE_COUNT` | Anzahl der Start-Announcements und der gerichteten Antworten an einen neu entdeckten Teilnehmer; 0 schaltet die Start-Announcements ab, eine Antwort bleibt | 4303 |
| `discovery.initial_announce_period_ms` | ms, > 0 | 200 | `ZERODDS_INITIAL_ANNOUNCE_PERIOD_MS` | Abstand der Start-Announcements und der Antworten | 4307 |
| `discovery.participant_lease_duration_ms` | ms, > 0 | 100000 | `ZERODDS_PARTICIPANT_LEASE_MS` | angekündigte Participant-Lease | 4275 |
| `discovery.wlp_period_ms` | ms, 0 = ein Drittel der Lease | 0 | `ZERODDS_WLP_PERIOD_MS` | WLP-Periode | 4503 |
| `discovery.peers` | Liste IPv4 oder IPv4:Port | (none) | `ZERODDS_PEERS` | Unicast-Peers; ohne Port domänenabhängig expandiert | 4442 |
| `discovery.max_peer_participants` | 1..=120 | 10 | `ZERODDS_MAX_PEER_PARTICIPANTS` | Participant-IDs je Peer bei der Expansion | 4446 |
| `discovery.no_multicast` | bool | false | `ZERODDS_NO_MULTICAST` | SPDP-Multicast-Senden aus, reine Unicast-Discovery | 4423 |
| `discovery.multicast_allowlist` | Liste IPv4-Multicast | (none) | `ZERODDS_MULTICAST_ALLOWLIST` | erlaubte Multicast-Gruppen; leer = keine Einschränkung | 4414 |
| `discovery.server` | bool | false | `ZERODDS_DISCOVERY_SERVER` | Participant arbeitet als SPDP-Relay | 4427 |
| `discovery.domain_tag` | Text | (none) | `ZERODDS_DOMAIN_TAG` | DDSI-RTPS 2.5 §8.5.3.2: gesetzt → `PID_DOMAIN_TAG` im SPDP; Participants mit anderem Tag (auch ohne Tag) werden nicht entdeckt | 4373 |
| `ports.port_base` | 0..=65535 | 7400 | `ZERODDS_PORT_BASE` | PB der Port-Abbildung von DDSI-RTPS 2.5 §9.6.2.3 (Default: Spec-Wert); Peers mit anderen Werten müssen dieselben Werte bekommen | 4459 |
| `ports.domain_gain` | 0..=65535 | 250 | `ZERODDS_PORT_DOMAIN_GAIN` | DG der Port-Abbildung (§9.6.2.3) | 4460 |
| `ports.participant_gain` | 0..=65535 | 2 | `ZERODDS_PORT_PARTICIPANT_GAIN` | PG der Port-Abbildung (§9.6.2.3) | 4461 |
| `ports.d0` | 0..=65535 | 0 | `ZERODDS_PORT_D0` | SPDP-Multicast-Port = PB + DG · domainId + d0 | 4462 |
| `ports.d1` | 0..=65535 | 10 | `ZERODDS_PORT_D1` | SPDP-Unicast-Port = PB + DG · domainId + d1 + PG · participantId | 4463 |
| `ports.d2` | 0..=65535 | 1 | `ZERODDS_PORT_D2` | Multicast-Port der Nutzdaten = PB + DG · domainId + d2; gebunden, wenn `transport.user_multicast` an ist | 4464 |
| `ports.d3` | 0..=65535 | 11 | `ZERODDS_PORT_D3` | Unicast-Port der Nutzdaten = PB + DG · domainId + d3 + PG · participantId; wird nicht gebunden, die Unicast-Sockets der Nutzdaten nehmen einen ephemeren Port, den der Participant ankündigt | 4465 |
| `security.secure_spdp` | bool | true | `ZERODDS_SECURE_SPDP` | zuverlässiger Secure-SPDP-Kanal (0xff0101) eines gesicherten Teilnehmers; Fast DDS wartet auf ihn | 4499 |
| `security.enclave_dir` | nichtleerer Pfad | (none) | `ZERODDS_SECURITY_DIR` | SROS2-Enclave-Verzeichnis (§5); Konsumenten siehe §5.1 | 4370 |
| `security.kx_key_fallback` | `spec`, `accept` | spec | `ZERODDS_SEC_KX_KEY_FALLBACK` | `spec`: der Kx-Schlüssel gehört nur dem Kanal `BuiltinParticipantVolatileMessageSecure` (DDS-Security 1.2 §10.5.2.1.2); `accept` versucht eine Submessage oder Nutzlast, die kein Schlüssel ihres Senders öffnet, auch mit diesem Schlüssel, für einen Peer, der Discovery oder Nutzdaten damit schützt; gegen keinen Hersteller gemessen | 2970 (`SecurityVendorSettings::from_source`) |
| `security.compatibility_check` | `spec`, `ignore` | spec | `ZERODDS_SEC_COMPATIBILITY_CHECK` | `spec` hält Participants und Endpunkte getrennt, deren angekündigte Sicherheitsmasken und Algorithmusangaben nicht zusammenpassen (DDS-Security 1.2 §7.3.10 bis §7.3.24); `ignore` für einen Peer, dessen angekündigte Masken von dem abweichen, was er durchsetzt | 2971 (`SecurityVendorSettings::from_source`) |
| `security.algo_name_nul` | `spec`, `omit`, `mirror`, `by_vendor` | spec | `ZERODDS_SEC_ALGO_NAME_NUL` | wie die Algorithmusnamen `c.dsign_algo` und `c.kagree_algo` im Token des Handshakes geschrieben werden: `spec` zählt das abschließende NUL in der Länge mit (DDS-Security 1.2 §7.3.3.1); `omit` schreibt es nie (Fast DDS 3.6 schreibt und vergleicht ohne); `mirror` schreibt es als Initiator und antwortet in der Form der Anfrage; `by_vendor` schreibt es nur für einen Peer mit OpenDDS (OpenDDS verlangt es); Fast DDS und OpenDDS aus den Quellen gelesen | 2972 (`SecurityVendorSettings::from_source`) |
| `security.algorithm_names` | `auto`, `1.2`, `1.1` | 1.2 | `ZERODDS_SEC_ALGORITHM_NAMES` | Schreibweise der Algorithmusnamen, die der Initiator sendet: `1.2` die von DDS-Security 1.2 Tables 25 und 26 (ECDSA+P256+SHA256, ECDHE-CEUM+P256, DHE+MODP-2048-256); `1.1` die dort als „deprecated in v 1.2“ markierten (ECDSA-SHA256, ECDH+prime256v1-CEUM, DH+MODP-2048-256), mit denen Cyclone DDS, Fast DDS und OpenDDS vergleichen (Quellen gelesen, nicht gemessen); `auto` sendet die von 1.2 an einen Peer, dessen IdentityToken 1.2 nennt, und die von 1.1 an einen Peer von 1.0; empfangen werden beide | 2973 (`SecurityVendorSettings::from_source`) |
| `security.subject_name_form` | `rfc4514`, `x509_name_oneline`, `xn_flag_oneline` | rfc4514 | `ZERODDS_SEC_SUBJECT_NAME_FORM` | Form der Namen in `dds.cert.sn` und `dds.ca.sn` des IdentityToken und `dds.perm_ca.sn` des PermissionsToken: `rfc4514` nach DDS-Security 1.2 §9.4.1.2.3 (`CN=y,O=x,C=DE`); `x509_name_oneline` die Form `/C=DE/O=x/CN=y` von OpenSSL `X509_NAME_oneline` (Fast DDS 3.6 vergleicht `dds.perm_ca.sn` damit, Cyclone DDS schreibt sie); `xn_flag_oneline` die Form `C = DE, O = x, CN = y` von `XN_FLAG_ONELINE` (OpenDDS vergleicht `dds.ca.sn` damit); die Vergleiche aus den Quellen gelesen, ZeroDDS vergleicht keinen empfangenen Namen | 2974 (`SecurityVendorSettings::from_source`) |
| `security.origin_authentication_fallback` | `spec`, `accept` | spec | `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK` | was ein Participant, dessen Governance Origin Authentication verlangt (`rtps_protection_kind` oder `metadata_protection_kind` `SIGN_WITH_ORIGIN_AUTHENTICATION`/`ENCRYPT_WITH_ORIGIN_AUTHENTICATION`), mit einer Nachricht oder Submessage unter einem Schlüssel ohne empfängerspezifischen Schlüssel tut: `spec` lehnt sie ab (DDS-Security 1.2 §10.5.3.3 Table 80 `decode_rtps_message`); `accept` prüft sie nur mit dem gemeinsamen MAC, wie Table 80 bei `receiver_specific_key_id` 0; ein Schlüssel mit empfängerspezifischem Schlüssel wird in beiden Fällen daran gehalten; für OpenDDS 3.34, das keine empfängerspezifischen Schlüssel erzeugt | 2976 (`SecurityVendorSettings::from_source`) |
| `tsn.interface` | nichtleerer Interfacename | (none) | `ZERODDS_TSN_IFACE` | Interface des TSN-Transports; nur bei `user_transport: TSN` gelesen | 4719 |
| `tsn.vlan` | 0..=4094 | 0 | `ZERODDS_TSN_VLAN` | VLAN-ID des TSN-Transports | 4721 |
| `tsn.pcp` | 0..=7 | 0 | `ZERODDS_TSN_PCP` | Priority Code Point des TSN-Transports | 4725 |
| `threads.recv_priority` | SCHED_FIFO/SCHED_RR 1..=99 (SCHED_FIFO ohne `threads.*_policy`) | (none) | `ZERODDS_RECV_THREAD_PRIORITY` | Echtzeitpriorität der Empfangs-Threads; braucht `CAP_SYS_NICE` oder `RLIMIT_RTPRIO` | 4599 |
| `threads.tick_priority` | SCHED_FIFO/SCHED_RR 1..=99 (SCHED_FIFO ohne `threads.*_policy`) | (none) | `ZERODDS_TICK_THREAD_PRIORITY` | Echtzeitpriorität des Tick-Threads | 4602 |
| `threads.recv_policy` | `other`, `fifo`, `rr`, `deadline` | (none) | `ZERODDS_RECV_THREAD_POLICY` | Klasse des Schedulers für die Empfangs-Threads; ungesetzt mit gesetztem `threads.recv_priority` gilt `fifo`, ungesetzt ohne weitere `threads.*`-Einstellung gibt es weder Echtzeitplanung noch Affinität; eine Priorität mit `other` oder `deadline`, ein `threads.recv_deadline` ohne `deadline`, `fifo` oder `rr` ohne Priorität und `deadline` ohne `threads.recv_deadline` sind Startfehler mit dem Namen des Schlüssels | 4612 |
| `threads.tick_policy` | `other`, `fifo`, `rr`, `deadline` | (none) | `ZERODDS_TICK_THREAD_POLICY` | wie `threads.recv_policy`, für den Tick-Thread | 4616 |
| `threads.recv_deadline` | `runtime,deadline,period` in ns, runtime ≤ deadline ≤ period | (none) | `ZERODDS_RECV_THREAD_DEADLINE` | Reservierung mit SCHED_DEADLINE für die Empfangs-Threads (mit `threads.recv_policy: deadline`); braucht immer `CAP_SYS_NICE`; eine Reservierung über der globalen Bandbreitengrenze des Kernels scheitert mit `EBUSY` | 4620 |
| `threads.tick_deadline` | `runtime,deadline,period` in ns | (none) | `ZERODDS_TICK_THREAD_DEADLINE` | wie `threads.recv_deadline`, für den Tick-Thread | 4624 |
| `threads.recv_cpus` | Liste CPU-Indizes (< 4096) | (none) | `ZERODDS_RECV_THREAD_CPUS` | CPU-Affinität der Empfangs-Threads | 4628 |
| `threads.tick_cpus` | Liste CPU-Indizes | (none) | `ZERODDS_TICK_THREAD_CPUS` | CPU-Affinität des Tick-Threads | 4631 |
| `threads.extra_recv` | 0..=64 | 0 | `ZERODDS_EXTRA_RECV_THREADS` | zusätzliche Empfangs-Worker | 4634 |
| `wire.rtps_dialect` | `cyclone`, `fastdds`, `opendds`, `maximally-interoperable` | maximally-interoperable | `ZERODDS_RTPS_DIALECT` | Verhaltenstabelle für Vendor-Abweichungen | 4316 |
| `wire.codec_dialect` | die Namen von `zerodds-idlc generate --dialect` (`specification`, `cyclone`, `rti`, `fastdds`, `opendds`, …) | specification | `ZERODDS_CODEC_DIALECT` | CDR-Dialekt der reflektiven Laufzeit-Codecs: Body-Prüfung und Key-Ableitung der Sprach-Bindings (`SerializedTypeSupport`), webdds-bridge, ROS-2-Dynamic-Typesupport, Voreinstellung von `spy`/`record --dialect`; gleich dem Dialekt, in dem die Typen generiert sind | 4321 |
| `wire.encapsulation_ids` | `table-60`, `table-39` | table-60 | `ZERODDS_WIRE_ENCAPSULATION_IDS` | Kennungen der Encapsulation eines Samples mit Nutzdaten: `table-60` die von DDS-XTypes 1.3 Table 60 (§7.6.3.1.2: 0x0006 bis 0x000b für XCDR2, 0x0004 für XML), die Cyclone DDS, Fast DDS, OpenDDS und RTI Connext senden; `table-39` die von Table 39 (§7.4.3.4: 0x0010 bis 0x0015, {0x01, 0x00} für XML); gelesen werden beide, die Kennungen von XCDR1 ändern sich nicht | 4703 |
| `wire.pl_cdr1_headers` | `dialect`, `specification`, `short-rounded-length`, `always-extended`, `short-true-length`, `extended-for-constructed`, `extended-for-constructed-in-large-samples` | dialect | `ZERODDS_WIRE_PL_CDR1_HEADERS` | Form der Parameterköpfe von PL_CDR1 in den reflektiven Codecs, statt der von `wire.codec_dialect`; `dialect` behält die des Dialekts; `specification` schreibt und erwartet die genaue Parameterlänge des Members (DDS-XTypes 1.3 §7.4.1.2), `short-rounded-length` die auf vier aufgerundete (DDSI-RTPS 2.5 §9.4.2.11); gilt für den ganzen Prozess | 4707 |
| `wire.pl_cdr1_reading` | `strict`, `tolerant` | strict | `ZERODDS_WIRE_PL_CDR1_READING` | `strict`: der reflektive Leser von PL_CDR1 nimmt die Rahmung, auf die er eingestellt ist, und meldet Flags, Kopfform und Längenregel einer anderen; `tolerant` nimmt beliebige Flags an PID_EXTENDED und PID_LIST_END, kurzen und erweiterten Kopf und eine Länge, die die des Members oder auf vier aufgerundet ist (DDS-XTypes 1.3 §7.4.1.2.1, §7.4.1.2.2); gilt für den ganzen Prozess | 4710 |
| `wire.type_lookup_encoding` | `xcdr1`, `xcdr2` | xcdr2 | `ZERODDS_WIRE_TYPE_LOOKUP_ENCODING` | Kodierung der Requests und Replies des Dienstes TypeLookup: `xcdr2` = CDR2_LE {0x00,0x07} mit Kodierungsversion 2, die Form von DDS-XTypes 1.4 Beta 1 §7.6.3.3.3 (OMG-Issue DDSXTY14-29), wie Cyclone DDS 11.0.1, Fast DDS 3.6.1, OpenDDS 3.34.0, RTI Connext 7.7.0 und InterCom DDS 4.3.1 schreiben; `xcdr1` = CDR_LE {0x00,0x01} mit Version 1, die Lesart von DDS-XTypes 1.3 und DDSI-RTPS 2.5 §10.4 Table 10.2; Cyclone DDS, OpenDDS und RTI Connext lehnen CDR_LE ab, Fast DDS beantwortet es (gemessen); empfangen wird jede Kodierung nach ihrer Kennung | 4714 |
| `wire.data_representation_offer` | Liste `XCDR1`/`XCDR`/`1`, `XCDR2`/`2` | XCDR1 | `ZERODDS_DATA_REPR_OFFER` | angebotene DataRepresentation, erste = Encoding der Writer; Default XTypes 1.3 §7.6.3.1.1; ein Reader fragt zusätzlich XCDR2 und XCDR an | 4642 |
| `transport.tcp_locator_kinds` | `vendor`, `fast-dds` | vendor | `ZERODDS_TCP_LOCATOR_KINDS` | LocatorKind der TCP-Locatoren: eigene Werte im Herstellerbereich (§9.3.2.4) oder 4 und 8 wie Fast DDS; Lesen nimmt beide | 4660 |
| `wire.data_representation_match` | `strict`, `tolerant` | strict | `ZERODDS_DATA_REPR_MATCH` | Matching-Regel der DataRepresentation | 4654 |
| `wire.big_endian` | bool | false | `ZERODDS_WIRE_BIG_ENDIAN` | Writer kodieren Big Endian (`*_BE`) | 4361 |
| `wire.clock_skew_threshold_s` | s, Ganzzahl ≥ 0 | 0 (off) | `ZERODDS_WIRE_CLOCK_SKEW_THRESHOLD_S` | CLOCK_SKEW_DETECTED (DDSI-RTPS 2.5 §8.3.7.4): größte Differenz zwischen Empfangszeit und `rtpsSendTimestamp` der HeaderExtension; darüber wird `clockSkewDetected` gesetzt und das folgende InfoTimestamp um die Differenz verschoben (§8.3.8.11); die Spec nennt keinen Schwellwert, `0` = aus | 4553 |
| `wire.accept_empty_acknack_base_zero` | bool | false | `ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO` | ein ACKNACK mit leerem `readerSNState` (numBits 0) und `bitmapBase` 0 annehmen; DDSI-RTPS 2.5 §9.4.2.6 verlangt `bitmapBase` ≥ 1, per Default ist ein solches ACKNACK ungültig und der Rest seiner Nachricht mit ihm; OpenDDS 3.34.0 und RTI Connext 7.7.0 senden es als ACKNACK eines Readers, der nichts hält; gilt für den ganzen Prozess und wird nicht wieder abgeschaltet | 4557 |
| `wire.omit_heartbeat_group_digests` | bool | false | `ZERODDS_WIRE_OMIT_HEARTBEAT_GROUP_DIGESTS` | `writerSet` und `secureWriterSet` im HEARTBEAT mit Gruppeninformation (GroupInfoFlag) weglassen; DDSI-RTPS 2.5 Table 8.44 und §8.7.5 verlangen beide, die Abbildung in §9.4.5.7 endet nach `lastGSN`; `true` sendet diese Form, ein Reader nimmt beide | 4576 |
| `discovery.type_object_v1` | bool | false | `ZERODDS_TYPE_OBJECT_V1` | Endpunkte kündigen ihren Typ zusätzlich als `TypeObjectV1` (`type`, PID 0x0072) und `TypeIdV1` (`type_id`, 0x0069) von XTypes 1.1 Annex B an, die optionalen Member der Built-in-Topic-Daten von XTypes 1.3 Annex D; ein Typ ohne Form in Annex B von 1.1 geht ohne sie hinaus | 4364 |
| `xtypes.key_member_bounds` | `enforced`, `ignored` | enforced | `ZERODDS_KEY_MEMBER_BOUNDS` | XTypes-1.3-Tabelle-19-Regeln für Key-Member bei der Zuweisbarkeit | 4673 |
| `xtypes.strong_assignability` | `equivalence`, `names_ignored` | equivalence | `ZERODDS_STRONG_ASSIGNABILITY` | was die starke Zuweisbarkeit (XTypes 1.3 §7.2.4.3) von einem Element- oder Member-Typ verlangt, der nicht delimited ist: Äquivalenz nach der MINIMAL-Relation mit Membernamen, oder mit `names_ignored` Zuweisbarkeit in beide Richtungen ohne Membernamen | 4678 |
| `xtypes.type_consistency_absent` | `disallow`, `allow` | disallow | `ZERODDS_TYPE_CONSISTENCY_ABSENT` | was ein Remote-Reader anfordert, der keine TypeConsistencyEnforcement (PID 0x0074) ankündigt; `disallow` ist XTypes 1.3 7.6.3.4.1 | 4663 |
| `xtypes.annotation_parameter_order` | `member-index`, `name` | member-index | `ZERODDS_ANNOTATION_PARAMETER_ORDER` | Reihenfolge der Parameter eines `CompleteAnnotationType`: `member-index` (Deklarationsreihenfolge) ist XTypes 1.3 §7.3.4.5, `name` der Kommentar in Annex B; die Reihenfolge steht im Objekt und damit im Hash von `annotation_typeid`; gilt für den ganzen Prozess | 4668 |
| `durability.dir` | nichtleerer Pfad | (temp dir)/zerodds-durability | `ZERODDS_DURABILITY_DIR` | Wurzel des Speichers für PERSISTENT-Writer (`publisher.rs:858`) | 4367 |
| `delivery.mode` | `portable`, `raw-same-host`, `iceoryx` | portable | `ZERODDS_DELIVERY_MODE` | Zustellweg der Loan-Writer (C-API) und der RMW-Publisher mit loanbarem Typ; `DataWriter::write` ist immer `portable`; wo der Modus nicht greift, Warnung (`zerodds-delivery-modes-1.0` §4) | 4375 |
| `time.source` | `system`, `tai` (nur Linux, `CLOCK_TAI`) | system | `ZERODDS_TIME_SOURCE` | Uhr hinter SOURCE_TIMESTAMP, DESTINATION_ORDER BY_SOURCE_TIMESTAMP, den Defaults von `write_w_timestamp` und der Empfangsuhr des Readers für LIFESPAN/BY_RECEPTION_TIMESTAMP (`crates/dcps/src/time_source.rs`); `tai` wird abgelehnt, wo Build oder Host es nicht können, ohne Rückfall auf `system`; eine programmatische Quelle wird direkt an `RuntimeConfig::time_source` gesetzt | 4597 |
| `diagnostics.observability` | `off`/`none`/`0`/`false`/leer oder `stderr`/`json`/`stderr-json`/`1`/`on`/`true` | off | `ZERODDS_OBSERVABILITY` | Ereignissenke; `stderr-json` = JSON-Zeilen auf stderr | 4588 |
| `diagnostics.trace_send_errors` | bool | false | `ZERODDS_TRACE_SEND_ERR` | fehlgeschlagene Sends auf stderr | 4731 |
| `diagnostics.phase_timing` | bool | false | `ZERODDS_PHASE_TIMING` | Latenzzähler je Phase | 4734 |
| `diagnostics.phase_dump` | bool | false | `ZERODDS_PHASE_DUMP` | Phasenzähler beim Drop der Runtime ausgeben | 4733 |
| `ros2.time_based_filter` | Liste `SEKUNDEN` oder `/topic=SEKUNDEN`, ≥ 0 | (none) | `ZERODDS_TIME_BASED_FILTER` | TIME_BASED_FILTER für ROS-Topics (rmw_qos_profile_t hat kein Feld dafür) | 4387 |
| `transport.udp.connected_cache` | bool | true | `ZERODDS_UDP_CACHE_ENABLE` | Cache verbundener Sende-Sockets je Ziel; `false` = klassisches `send_to` | 4204 (`udp_config_from`) |
| `transport.udp.cache_max_entries` | > 0 | 16 | `ZERODDS_UDP_CACHE_MAX` | Einträge des Caches, FIFO-Verdrängung | 4207 (`udp_config_from`) |
| `transport.udp.cache_sndbuf` | Bytes, 0 = Kernel-Default | 262144 | `ZERODDS_UDP_CACHE_SNDBUF` | `SO_SNDBUF` der Cache-Sockets | 4210 (`udp_config_from`) |
| `transport.udp.recv_buffer` | Bytes, 0 = Kernel-Default | 1048576 | `ZERODDS_UDP_RCVBUF` | `SO_RCVBUF` jedes gebundenen UDP-Sockets | 4211 (`udp_config_from`) |
| `transport.udp.dont_route` | bool | false | `ZERODDS_UDP_DONTROUTE` | `SO_DONTROUTE`; nur ohne Routing über Subnetzgrenzen | 4213 (`udp_config_from`) |
| `transport.udp.max_message_size` | Bytes, 20 bis 65507 | 1472 | `ZERODDS_UDP_MAX_MESSAGE_SIZE` | größte RTPS-Nachricht je Datagramm; die Fragmentgröße eines Writers folgt dem kleinsten Wert seiner Transporte (DDSI-RTPS 2.5 §8.4.14.1) | 4216 (`udp_config_from`) |

Anzahl der Zeilen: 101. Man-Page `man/man5/zerodds.yaml.5`, Beispiel `packaging/linux/configs/zerodds.yaml.example` (alle Schlüssel mit Default); `runtime_config_layering.rs::the_shipped_example_and_the_man_page_name_every_setting` hält beide an `SETTINGS`.

Programmatische Voreinstellungen ohne Dateischlüssel: `RuntimeConfig::ros_defaults()` (`runtime.rs:4882`), `multi_robot()` (`:4894`, Multicast aus), `discovery_server()` (`:4920`). `RuntimeConfig::default()` liest weder Dateien noch Umgebung (`runtime.rs:4247`).

### 2.3 `transport.interface_bindings`

Sequenz von Maps (`parse_interface_bindings`, `runtime.rs:4097`). Nur mit Feature `security`; ohne das Feature wird jeder Eintrag abgelehnt („needs a build with the `security` feature“).

| Schlüssel | Pflicht | Werte |
|---|---|---|
| `name` | ja | Name der Bindung |
| `bind` | ja | `ip` oder `ip:port` |
| `kind` | ja | `loopback`, `localhost`, `wan`, `subnet`, `named:<class>` |
| `subnet` | ja | CIDR, z. B. `10.0.0.0/24`; eine Adresse ohne Präfix ist /32 bzw. /128 |
| `default` | nein | `true`/`false` |

Andere Schlüssel werden mit „entry i: unknown key“ abgelehnt.

### 2.4 `ros2.*` (nur RMW)

Reader: `Ros2Config::from_config`, `crates/rmw-zerodds-shim/src/ros2_config.rs`; Schlüsselliste `KEYS` (`ros2_config.rs:56`). Man-Page `man/man5/zerodds-ros2.yaml.5`, Beispiel `packaging/linux/configs/ros2.yaml.example` (als Drop-in `zerodds.d/50-ros2.yaml` gedacht). Andere Programme lassen `ros2.*` unbeanstandet (`is_rmw_path`, `runtime.rs:4083`); der RMW prüft strikt.

| Pfad | Werte | Default | Wirkung |
|---|---|---|---|
| `ros2.topic_mangling` | `rmw`, `none`, `custom` | rmw | `rmw`: Präfix `rt/` (REP-2007), Services `rq/`…`Request`, `rr/`…`Reply`; `none`: Name ohne führenden `/`; `custom`: Präfixe aus `custom_mangling` |
| `ros2.custom_mangling.topic_prefix` | Text | – | bei `custom` Pflicht, sonst abgelehnt |
| `ros2.custom_mangling.request_prefix` | Text | `rq/` | nur bei `custom` |
| `ros2.custom_mangling.response_prefix` | Text | `rr/` | nur bei `custom` |
| `ros2.qos_profiles.<name>.{reliability,durability,history,depth,deadline_ms,lifespan_ms,liveliness,liveliness_lease_ms}` | reliability: `reliable`/`best_effort`/`system_default`; durability: `volatile`/`transient_local`/`system_default`; history: `keep_last`/`keep_all`/`system_default`; liveliness: `automatic`/`manual_by_topic`/`system_default`; `*_ms`: 0 = keine | REP-2009-Tabelle: `sensor_data`, `services`, `parameters`, `parameter_events`, `default` | ersetzt Felder eines Standardprofils oder legt ein neues an |
| `ros2.topic_overrides[]` | Einträge mit `ros_topic` und genau einem von `qos_profile` oder `qos` (Map wie oben) | leer | ersetzt je ROS-Topic die genannten Felder der angeforderten QoS |
| `ros2.time_based_filter` | siehe §2.2 | – | siehe §2.2 |

Abgelehnt mit Begründung (`OWNED_ELSEWHERE`, `ros2_config.rs:125`): `ros2.namespace` (rcl, `--ros-args -r __ns:=`), `ros2.ros_domain_id` (`ROS_DOMAIN_ID`), `ros2.enclave` (→ `security.enclave_dir`), `logging` (rcutils, `--ros-args --log-level`), `discovery.multicast` (→ `discovery.no_multicast`), `discovery.unicast_initial_peers` (→ `discovery.peers`).

Gemessen: `zerodds-ros2-shim --config packaging/linux/configs/ros2.yaml.example --check-config` → `configuration valid`, zwei Overrides (`/scan -> rt/scan (sensor_data)`, `/map -> rt/map (qos)`).

## 3 Daemon-Dateien

### 3.1 Überblick

| Datei | Daemon | Schlüssel (`KEYS`) | eigene Umgebungsvariablen | Man-Page | Beispiel |
|---|---|---|---|---|---|
| `ws-bridged.yaml` | `zerodds-ws-bridged` | 17, `crates/websocket-bridge/src/daemon/config.rs:29` | – | `man/man5/zerodds-ws-bridged.yaml.5` | `packaging/linux/configs/ws-bridged.yaml.example` |
| `mqtt-bridged.yaml` | `zerodds-mqtt-bridged` | 25, `crates/mqtt-bridge/src/daemon/config.rs:25` | `MQTT_PASSWORD` → `mqtt.password` (`:55`) | `man/man5/zerodds-mqtt-bridged.yaml.5` | `packaging/linux/configs/mqtt-bridged.yaml.example` |
| `coap-bridged.yaml` | `zerodds-coap-bridged` | 32, `crates/coap-bridge/src/daemon/config.rs:24` | – | `man/man5/zerodds-coap-bridged.yaml.5` | `packaging/linux/configs/coap-bridged.yaml.example` |
| `amqp-bridged.yaml` | `zerodds-amqp-bridged` | 28, `crates/amqp-endpoint/src/bridge_daemon.rs:243` | `AMQP_PASSWORD` → `amqp.sasl.password` (`:281`) | `man/man5/zerodds-amqp-bridged.yaml.5` | `packaging/linux/configs/amqp-bridged.yaml.example` |
| `grpc-bridged.yaml` | `zerodds-grpc-bridged` | 19, `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1460` | – | `man/man5/zerodds-grpc-bridged.yaml.5` | `packaging/linux/configs/grpc-bridged.yaml.example` |
| `corba-bridged.yaml` | `zerodds-corba-bridged` | 23, `crates/corba-dds-bridge/src/bridge_daemon.rs:275` | – | `man/man5/zerodds-corba-bridged.yaml.5` | `packaging/linux/configs/corba-bridged.yaml.example` |

Alle sechs Beispiel-Dateien wurden mit dem jeweiligen Binary geprüft (`--config <beispiel> --check-config`): jeweils `configuration valid`, Exit 0. Jeder Name aus `KEYS` kommt in der zugehörigen Man-Page vor (Textsuche nach dem letzten Pfadsegment).

Gemeinsam für alle sechs:

- `domain` (DDS-Domain, Default 0), `log_level` (`trace`, `debug`, `info`, `warn`, `error`; Default `info`).
- `metrics.enabled` (Default false), `metrics.listen`. Ist `metrics.enabled: true` und `metrics.listen` leer, bindet jeder Daemon seinen eigenen Port: ws `127.0.0.1:9091`, mqtt `:9092`, coap `:9093`, amqp `:9094`, grpc `:9095`, corba `:9096` (`DEFAULT_METRICS_LISTEN` je Daemon; ein Test je Crate prüft den Wert gegen die man5-Seite). Die Beispiel-Dateien setzen dieselben Ports.
- Topic-Einträge (`topics[]`, bei CORBA `mappings[]`) dürfen als Map oder als Kurzform-Text stehen; die Kurzform ist die Form, in der `--topic` in die Ebene `--set` gelangt.
- Unbekannte Schlüssel innerhalb einer bekannten Sektion lehnt `Section::only` mit Datei und Zeile ab.
- Beim Signal SIGHUP lesen die Daemons dieselben Ebenen erneut (`reload_command_line`, z. B. `zerodds-ws-bridged.rs`).

### 3.2 `ws-bridged.yaml`

Defaults: `DaemonConfig::default_for_dev`, `crates/websocket-bridge/src/daemon/config.rs`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `listen` | `127.0.0.1:8080` | WebSocket-Bind-Adresse |
| `tls.enabled`, `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` | false, leer | TLS; `client_ca_file` für mTLS |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject` | – | Token und zugehöriges Subjekt für `bearer` |
| `auth.jwt.public_key`, `.issuer`, `.audience`, `.required_claim` | – | JWT-Prüfung für `jwt` |
| `topics[]` | leer | `name` (Pflicht), `type` (Default = `name`), `direction` (`in` = Client → DDS, `out` = DDS → Client, `bidir`; Default `bidir`), `ws_path` (Default `/topics/<slug>`, `Chat::Message` → `/topics/chat/message`), `qos.reliability` (`reliable`, `best_effort`), `qos.durability` (`volatile`, `transient_local`, `transient`, `persistent`), `qos.history.depth` (≥ 1, Default 10), `acl.read`, `acl.write` (Subjektlisten) |

### 3.3 `mqtt-bridged.yaml`

Defaults: `default_for_dev`, `crates/mqtt-bridge/src/daemon/config.rs`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `mqtt.broker_url` | `mqtt://127.0.0.1:1883` | Broker; beim Laden geparst |
| `mqtt.client_id` | `zerodds-bridge` | MQTT-Client-ID |
| `mqtt.username`, `mqtt.password` | – | CONNECT-Anmeldung; `MQTT_PASSWORD` liegt unter den Dateien |
| `mqtt.keep_alive_secs` | 60 | 0..=65535 |
| `mqtt.drain_timeout_secs` | 30 | Wartezeit auf Broker-Acks beim Beenden |
| `mqtt.clean_start` | true | Clean Start |
| `mqtt.tls.enabled`, `.ca_file`, `.cert_file`, `.key_file`, `.server_name` | false, leer | TLS zum Broker, mTLS, SNI-Name |
| `auth.mode` | `none` | `none`, `bearer`, `sasl`, `sasl_plain`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject` | – | Token als CONNECT-Passwort, lokales Subjekt |
| `auth.username`, `auth.password` | – | ausgehende CONNECT-Anmeldung (SASL-PLAIN) |
| `auth.sasl_users.<user>` | leer | Benutzer → Passwort |
| `acl.default_deny` | true | wirkt ohne Regel und ohne Auth-Modus nur, wenn explizit gesetzt |
| `acl.rules[]` | leer | `subject`, `allow_publish`, `allow_subscribe` (Topic-Muster) |
| `topics[]` | leer | `dds_name`, `dds_type`, `mqtt_topic`, `direction` (Default `bidir`), `mqtt_qos` (`0`, `1`, `2`), `retain`, `qos.reliability`, `qos.durability`, `qos.history.depth` (Default 10); Kurzform `DDS[:MQTT]` |

### 3.4 `coap-bridged.yaml`

Defaults: `default_for_dev` und `OscoreSettings::default`, `crates/coap-bridge/src/daemon/config.rs:150`, `:190`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `coap.bind` | `0.0.0.0:5683` | UDP-Bind |
| `coap.bind_dtls` | – | DTLS-Bind |
| `coap.max_message_size` | 1152 | Bytes |
| `coap.ack_timeout_ms` | 2000 | CoAP ACK_TIMEOUT |
| `coap.max_retransmit` | 4 | 0..=255 |
| `coap.observe_max_age_secs` | 60 | Max-Age bei Observe |
| `coap.dtls.enabled`, `.cert_file`, `.key_file`, `.ca_file` | false, – | DTLS; `cert_file` und `key_file` nur gemeinsam |
| `coap.dtls.cipher_suites` | – | Liste; unbekannter Name wird abgelehnt |
| `coap.dtls.psk[]` | – | `identity` + `secret_hex` oder Text `identity=secret_hex` |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls` |
| `auth.bearer_token`, `auth.bearer_subject`, `auth.jwt.*` | – | wie bei WebSocket |
| `acl.default_deny` | true | wie bei MQTT |
| `acl.rules[]` | leer | `subject`, `allow_post`, `allow_observe` |
| `oscore.enabled` | false | OSCORE; verlangt `master_secret_hex` |
| `oscore.master_secret_hex`, `.master_salt_hex`, `.id_context_hex`, `.sender_id_hex` | leer, leer, –, `01` | Hex, Leerzeichen erlaubt, gerade Stellenzahl |
| `oscore.replay_window` | 32 | Replay-Fenster |
| `topics[]` | leer | `dds_name`, `dds_type` (Default = `dds_name`), `coap_uri_path`, `direction` (Default `bidir`), `qos.*` wie oben (`history.depth` Default 10); Kurzform `DDS[:URI]` |

### 3.5 `amqp-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/amqp-endpoint/src/bridge_daemon.rs`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `amqp.broker_url` | leer | `amqp://` oder `amqps://host[:port]` |
| `amqp.container_id` | `zerodds-bridge-<HOSTNAME>` (Punkte → `-`, 32 Zeichen), ohne `HOSTNAME` `zerodds-bridge-pid<PID>` | AMQP-Container-ID |
| `amqp.channel_max` | 256 | 0..=65535 |
| `amqp.max_frame_size` | 65536 | ≥ 512 |
| `amqp.idle_time_out_ms` | 30000 | 0 = keine |
| `amqp.hostname` | leer = Broker-Host | `open.hostname` |
| `amqp.sasl.mechanism` | `ANONYMOUS` | `PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2` |
| `amqp.sasl.username`, `amqp.sasl.password` | leer | bei `XOAUTH2` ist das Passwort das Token |
| `amqp.tls.enabled`, `.ca_file`, `.cert_file`, `.key_file` | false, leer | TLS |
| `amqp.tls.verify_hostname` | – | nur `true` zulässig |
| `amqp.tls.alpn` | – | nur `amqp` zulässig |
| `amqp.reconnect.initial_delay_ms`, `.max_delay_ms`, `.factor` | 500, 30000, 2.0 | Backoff; `factor` ≥ 1.0 |
| `acl.default_deny` | false | |
| `acl.rules[]` | leer | `topic`, `read`, `write` |
| `metrics.path` | – | nur `/metrics` zulässig |
| `catalog.enabled` | true | Katalog beim Verbinden senden |
| `catalog.address` | leer = `zerodds.bridge.<container-id>.catalog` | Zieladresse |
| `topics[]` | leer | `dds_name`, `dds_type`, `keyed` (false), `amqp_address`, `direction` (`bidir`), `sender.settled` (false), `sender.durable` (0..=2, 0), `sender.snd_settle_mode` (0..=2, 0), `sender.rcv_settle_mode` (0..=1, 0), `receiver.credit` (≥ 1, 256), `qos.reliability` (reliable), `qos.durability` (volatile), `qos.history.kind`/`depth` (keep_last, 1); Kurzform `DDS=ADRESSE`; doppelter `dds_name` wird abgelehnt |

### 3.6 `grpc-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs:1424`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `grpc.bind` | `0.0.0.0:50051` | HTTP/2-Bind |
| `grpc.tls.enabled`, `.cert_file`, `.key_file` | false, leer | TLS (ALPN `h2`); ohne Zertifikat Exit 4 |
| `grpc.tls.client_ca_file`, `grpc.tls.require_client_cert` | leer, false | mTLS |
| `grpc.reflection.enabled` | false | Server-Reflection |
| `auth.mode` | `none` | `none`, `bearer`, `jwt`, `mtls`; `mtls` verlangt `grpc.tls.enabled` und `client_ca_file` |
| `auth.bearer_token`, `auth.bearer_subject` (Default `anonymous`), `auth.jwt.*` | – | wie bei WebSocket |
| `topics[]` | leer | `dds_name` und `grpc_service` (beide Pflicht), `direction` (`bidir`), `acl.publish`, `acl.subscribe`; Kurzform `DDS=SERVICE` oder `DDS:SERVICE` |

### 3.7 `corba-bridged.yaml`

Defaults: `impl Default for DaemonConfig`, `crates/corba-dds-bridge/src/bridge_daemon.rs:234`.

| Schlüssel | Default | Bedeutung |
|---|---|---|
| `corba.orb_id` | `zerodds-corba-bridge` | ORB-ID |
| `corba.iiop.bind` | `0.0.0.0:6833` | IIOP-Bind |
| `corba.iiop.giop_version` | `1.2` | `1.0`, `1.1`, `1.2` |
| `corba.iiop.fragment_size` | 65536 | ≥ 64 |
| `corba.iiop.max_message_size` | 33554432 | ≥ 1024 |
| `corba.ssliop.enabled`, `.bind`, `.cert_file`, `.key_file`, `.ca_file`, `.require_client_cert` | false, `0.0.0.0:6834`, leer, false | SSLIOP; `enabled` verlangt Zertifikat und Schlüssel, `require_client_cert` verlangt `ca_file` |
| `corba.ior.ior_file` | leer | Datei für die IOR-Zeilen |
| `corba.ior.naming_service` | leer | wird gelesen, siehe Abweichungen |
| `corba.ior.publish_to_naming` | – | nur `false` zulässig |
| `corba.csiv2.enabled` | – | nur `false` zulässig |
| `acl.default_deny` | false | |
| `acl.rules[]` | leer | `subject` (Subjekt-DN nach RFC 4514, `mtls:<SHA-256>` oder `*`), `allow_invoke` (Repository-ID-Muster) |
| `metrics.path` | – | nur `/metrics` zulässig |
| `mappings[]` | leer | `repo_id`, `operation` (`invoke`), `direction` (`corba_to_dds`, `dds_to_corba`, `bidir`; Default `corba_to_dds`), `request_topic`/`reply_topic`/`notify_topic` (`dds_name`, `dds_type`), `correlation_id` (nur `request_id`), `timeout_ms` (5000), `qos.reliability`, `qos.durability` (`volatile`, `transient_local`), `targets` (corbaloc-URLs oder IORs); `target_object_keys` wird abgelehnt; Kurzform `REPO_ID=TOPIC` |

`corba.iiop.advertise_host` (Default `127.0.0.1`, Umgebung `ZERODDS_CORBA_ADVERTISE_HOST`) ist der Host in den IORs, wenn `corba.iiop.bind` eine Wildcard-Adresse ist.

### 3.8 Programme ohne eigene Datei

- `zerodds-durability-svc`: nur Flags (`--domain`, `--store sqlite|file|lakehouse|postgres`, `--path`, `--auto`, `--topic`) plus Laufzeitdatei `zerodds.yaml` (`crates/durability-service-bin/src/main.rs`). Die Variablen `ZERODDS_DURABILITY_DOMAIN`, `_STORE`, `_PATH`, `_EXTRA` in `packaging/linux/systemd/zerodds-durability-svc.service` sind systemd-Ersetzungen in `ExecStart`; das Programm liest sie nicht.
- `zerodds-ros2-shim` und der RMW: `ros2.*` in `zerodds.yaml` (§2.4).
- `zerodds-router`: Laufzeit aus `zerodds.yaml`, Routen aus `--routes <datei>` (§3.9). `--config` mit Endung `.json` oder `.xml` wird mit Exit 2 abgelehnt (`crates/routing-service/src/bin/zerodds-router.rs`).

### 3.9 Routen-Datei (`zerodds-router --routes`)

Format nach Endung: `.xml` → XML, sonst JSON (serde). Modell: `crates/routing-service/src/config.rs`; XML: `crates/routing-service/src/xml.rs` (Wurzel `<zerodds_routing>`, oder RTI-Teilmenge `<dds><routing_service>`/`<routing_service>`). Beispiel: `packaging/linux/configs/router.json.example`. Man-Page: `man/man1/zerodds-router.1`.

| Feld | Default | Bedeutung |
|---|---|---|
| `name` | `zerodds-router` | Name in Logs und Metriken |
| `routes[].name` | Pflicht, eindeutig | Metrik-Label und Loop-Guard-Tag |
| `routes[].input`, `routes[].output` | Pflicht | `domain`, `topic`, `type_name` (Default `*`), `keyed` (false; Ein- und Ausgang müssen übereinstimmen), `partition` (Liste), `qos.reliable` (true), `qos.durability` (`volatile`, `transient_local`, `transient`, `persistent`), `qos.ownership` (`shared`, `exclusive`), `qos.ownership_strength` (0), `qos.data_representation` (Liste, 0 = XCDR1, 2 = XCDR2) |
| `routes[].filter` | – | `expression` (DDS-SQL), `parameters`; braucht eine Typform (`--types`) |
| `routes[].transform` | – | `rename[{to,from}]`, `set_const[{field,value}]`, `drop[]` |
| `routes[].loop_guard` | true | Ein- und Ausgangs-Participant ignorieren sich gegenseitig |
| `routes[].preserve_source_timestamp` | false | Quell-Zeitstempel übernehmen |
| `routes[].max_samples_per_second` | – | Drosselung |
| `routes[].tenant` | – | Mandant für `access_control` |
| `access_control.allow[]` | – | `tenant`, `domain`, `topics` (Globs `*`, `?`) |

## 4 QoS über DDS-XML

### 4.1 Ergebnis

- Leser: Crate `zerodds-xml` (`crates/xml`). `zerodds-xml-compose` (`crates/xml-compose`) liest nur Typdefinitionen (`<types>`) in das gemeinsame AST und hat mit QoS nichts zu tun.
- Keine Laufzeit-Einstellung, keine Umgebungsvariable und kein Daemon lädt eine QoS-XML-Datei automatisch. `crates/dcps` hängt nicht von `zerodds-xml` ab (`crates/dcps/Cargo.toml`). Ein Rust-Programm lädt Profile selbst:

```rust
let reg = zerodds_xml::QosProfileRegistry::from_file("qos.xml")?;   // std
let wq  = reg.writer_qos("MyLib::HighPerf")?;                       // zerodds_qos::WriterQos
let dw_qos: zerodds_dcps::DataWriterQos = wq.into();                // From, crates/dcps/src/qos.rs:453
```

- `QosProfileRegistry` (`crates/xml/src/registry.rs`): `from_xml`, `from_file` (Feature `std`), `writer_qos`, `reader_qos`. Profilreferenz `Bibliothek::Profil` oder `Profil` (dann erste Bibliothek); volle `base_name`-Vererbung, auch bibliotheksübergreifend, mit Zykluserkennung.
- Weitere Leser: `zerodds-admin qos validate <datei.xml>` und `zerodds-admin qos check <datei.xml>` (RxO-Prüfung zwischen Writern und Readern der Participant-Bibliotheken, `tools/admin/src/main.rs:571`, `:603`), `zerodds-xmlc` (`tools/xmlc`; beide Werkzeuge: `--skip-unknown` lässt unbekannte Elemente mit Pfadmeldung aus, `--vendor-forms` liest Herstellerformen), `RpcQos::from_xml_profile` (`crates/rpc/src/qos_profile.rs:228`), die XRCE-Dateikonfiguration mit `qos_profile`-Attributen (`crates/xrce/src/xml_config.rs`, Feature `xml-config`).
- C-API, C++-, Python-, Java-Bindings: keine Funktion zum Laden von QoS-XML gefunden (Suche nach `xml`/`profile` in `extern "C"`-Namen von `crates/zerodds-c-api/src`).

### 4.2 Unterstützte Elemente (`crates/xml/src/qos_parser.rs`)

| Ebene | Elemente |
|---|---|
| Dokument | `<dds>` mit `<qos_library name>` (`parse_qos_libraries`) |
| Bibliothek | `<qos_profile name base_name>`; Kurzform: ein benannter QoS-Container direkt in der Bibliothek ist ein eigenes Profil (`parse_single_qos_shortcut`) |
| Profil | `datawriter_qos`, `datareader_qos`, `topic_qos`, `publisher_qos`, `subscriber_qos`, `domain_participant_qos`; Topic-Filter als Attribut `topic_filter` des Containers. Herstellerformen (`domainparticipant_qos`, `participant_qos`, `<topic_filter>` als Kindelement) sind ein Fehler, der die Form der Spezifikation nennt; `ReadOptions { vendor_forms: VendorForms::Accept, .. }` (`zerodds-admin --vendor-forms`, `zerodds-xmlc --vendor-forms`) liest sie und meldet jede mit Pfad |
| Policies (`parse_entity_qos`, Zeile 489) | `durability`, `durability_service`, `presentation`, `deadline`, `latency_budget`, `ownership`, `ownership_strength`, `liveliness`, `time_based_filter`, `partition`, `reliability`, `transport_priority`, `lifespan`, `destination_order`, `history`, `resource_limits`, `entity_factory`, `writer_data_lifecycle`, `reader_data_lifecycle`, `user_data`, `topic_data`, `group_data` |
| Werte | Konstanten `LENGTH_UNLIMITED`, `DURATION_INFINITE_SEC`/`_NSEC`, `DURATION_ZERO_*`; Booleans nur `true`, `false`, `1`, `0` (`parse_bool_strict`) |

Unbekannte Kindelemente eines Profils oder Containers sind ein Fehler (`XmlError::UnknownElement`), der das Element nennt; dasselbe gilt für eine Policy, die in DDS 1.4 §2.2.3 kein Feld des Containers ist. Weitere Bausteine desselben Crates: Domain-Bibliotheken, Participant-Bibliotheken, Applications, Typen, Samples (`crates/xml/src/lib.rs`).

## 5 Security-Dateien

Die Konzepte stehen in [security.md](security.md). Hier nur, wo Dateien und Pfade eingestellt werden.

### 5.1 Laufzeit: `security.enclave_dir`

`security.enclave_dir` (Umgebung `ZERODDS_SECURITY_DIR`) nennt ein SROS2-Enclave-Verzeichnis. `SecurityProfile::from_enclave_dir` (`crates/security-runtime/src/profile.rs:583`) erwartet darin:

| Datei | Rolle |
|---|---|
| `identity_ca.cert.pem` | Identity-CA |
| `cert.pem` | Identity-Zertifikat |
| `key.pem` | privater Schlüssel |
| `permissions_ca.cert.pem` | Permissions-CA |
| `governance.p7s` | signierte Governance |
| `permissions.p7s` | signierte Permissions |

Konsumenten (Suche nach `security_enclave_dir` in `crates/`): der RMW (`rmw_zerodds_init`, `crates/rmw-zerodds-shim/src/lib.rs:706`, über `runtime_create_secure_ros_from_source`, `crates/zerodds-c-api/src/security_ffi.rs:421`), die C-Funktion `zerodds_runtime_create_secure_from_env` (`security_ffi.rs:372`) und `zerodds-ros2-shim doctor`. Ein benanntes Enclave, das nicht lädt, ist beim RMW ein harter Fehler. Rust-Programme über `Configured` (CLI-Tools, Bridges) lesen den Wert, wenden ihn aber nicht an.

`transport.interface_bindings` (§2.3) und `security.secure_spdp` (§2.2) sind die übrigen Security-bezogenen Laufzeit-Einstellungen.

### 5.2 Participant-Properties (`dds.sec.*`)

Programmatisch über die `PropertyQosPolicy` des Participants, nicht über `zerodds.yaml` (`crates/security-runtime/src/qos_properties.rs:73–190`, `crates/security-logging/src/from_properties.rs:50–60`):

| Property | Inhalt |
|---|---|
| `dds.sec.auth.identity_ca`, `dds.sec.auth.identity_certificate`, `dds.sec.auth.private_key`, `dds.sec.auth.password`, `dds.sec.auth.key_establishment_algorithm` | Authentifizierung (DDS-Security 1.2 §9.3.1) |
| `dds.sec.access.permissions_ca`, `dds.sec.access.governance`, `dds.sec.access.permissions` | Zugriffskontrolle (§9.4.1.2) |
| `dds.sec.log.plugin`, `org.zerodds.sec.log.level`, `org.zerodds.sec.log.jsonl.path`, `org.zerodds.sec.log.syslog.addr`, `.app`, `.host` | Security-Logging |

Werte sind URIs. `file:` wird aufgelöst, `data:,` ist der Inhalt selbst (RFC 2397) und `pkcs11:` benennt ein Objekt auf einem PKCS#11-Token (RFC 7512, DDS-Security 1.2 §9.3.1 Tabelle 44); `pkcs11:` braucht das Crate-Feature `pkcs11`, sonst `SecurityPropertyError::Pkcs11FeatureDisabled` (`qos_properties.rs`).

C-API: `zerodds_security_set_identity_ca_path`, `_identity_cert_path`, `_private_key_path`, `_permissions_ca_path`, `_governance_path`, `_permissions_path` (`crates/zerodds-c-api/src/security_ffi.rs:215–275`).

### 5.3 TLS-Dateien der Bridges

| Daemon | Schlüssel |
|---|---|
| ws | `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` |
| mqtt | `mqtt.tls.ca_file`, `mqtt.tls.cert_file`, `mqtt.tls.key_file` |
| coap | `coap.dtls.cert_file`, `coap.dtls.key_file`, `coap.dtls.ca_file`, `coap.dtls.psk[]`, `oscore.*_hex` |
| amqp | `amqp.tls.ca_file`, `amqp.tls.cert_file`, `amqp.tls.key_file` |
| grpc | `grpc.tls.cert_file`, `grpc.tls.key_file`, `grpc.tls.client_ca_file` |
| corba | `corba.ssliop.cert_file`, `corba.ssliop.key_file`, `corba.ssliop.ca_file` |

Die Beispiel-Dateien verwenden `/etc/zerodds/certs/`.

## 6 Umgebungsvariablen außerhalb von `SETTINGS`

Suche: `env::var`/`env::var_os` in `crates/` und `tools/` (Rust, ohne `tests/`, `benches/`, `examples/`, `build.rs` und ohne `#[test]`-Funktionen), `getenv` in C, `os.environ` in Python, `System.getenv` in Java, `process.env` in TypeScript.

### 6.1 Produktprogramme und Bibliotheken

| Variable | Leser | Fundstelle | Wirkung |
|---|---|---|---|
| `MQTT_PASSWORD` | `zerodds-mqtt-bridged` | `crates/mqtt-bridge/src/daemon/config.rs:55` | Ebene 2 für `mqtt.password` |
| `AMQP_PASSWORD` | `zerodds-amqp-bridged` | `crates/amqp-endpoint/src/bridge_daemon.rs:281` | Ebene 2 für `amqp.sasl.password` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | alle sechs Bridge-Daemons | ws `daemon/runtime_common.rs:370`, mqtt `daemon/runtime_common.rs:327`, coap `daemon/runtime_common.rs:308`, amqp `daemon_runtime.rs:497`, grpc `daemon_runtime.rs:303`, corba `daemon_runtime.rs:316` | OTLP-Export aktiv, `host:port`; einzige gelesene `OTEL_*`-Variable |
| `ZERODDS_CORBA_ADVERTISE_HOST` | `zerodds-corba-bridged` | `crates/corba-dds-bridge/src/bridge_daemon.rs` (`ENV`) | Ebene 2 für `corba.iiop.advertise_host` |
| `HOSTNAME` | amqp-Daemon; `zerodds-dcps`; C-API | `bridge_daemon.rs:196`; `crates/dcps/src/participant.rs:437`; `crates/zerodds-c-api/src/shm_loan_ffi.rs:851` | Default `amqp.container_id`; Fallback für die Host-ID nach `gethostname`; Hostname im SHM-Loan-Pfad |
| `COMPUTERNAME` | `zerodds-dcps` | `participant.rs:438` | Host-ID-Fallback (Windows) |
| `PROGRAMDATA` | `zerodds-config` | `crates/config/src/layers.rs:424` | Systemverzeichnis unter Windows |
| `${NAME}` in Konfigurationsdateien | `zerodds-config` | `crates/config/src/parse.rs:36` | beliebige Variable, Ersetzung vor dem Parsen |
| `ROS_DOMAIN_ID` | `zerodds-ros2-shim` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs:172`, `:566` | Domain der Unterbefehle, Default 0; `doctor` |
| `ROS_DISTRO`, `RMW_IMPLEMENTATION`, `ROS_SECURITY_ENABLE` | `zerodds-ros2-shim` | `zerodds-ros2-shim.rs:234`, `:536`, `:553`, `:611` | nur Anzeige und Diagnose (`info`, `doctor`) |
| `ZERODDS_LOAN_ALIGNMENT` | `rmw_zerodds_cpp` (C) | `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c:621` | Ausrichtung geliehener Puffer; `page` oder Zweierpotenz ≥ Zeigergröße |
| `ZERODDS_LIB` | Python-Paket, Node-Binding | `crates/py/python/zerodds/loader.py:83`, `crates/ts-node/src/native.ts:52` | Pfad der nativen Bibliothek |
| `ZERODDS_RTPS` | Java `DdsWireBridge` | `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/DdsWireBridge.java:25` | `1`/`true` schaltet die RTPS-Brücke ein (neben der Property `zerodds.rtps.enable`) |
| `CHAOS_CLOCK_SKEW_NS`, `CHAOS_CLOCK_DRIFT_PPM` | `chaos-clock-skew` (LD_PRELOAD-Shim) | `crates/chaos-clock-skew/src/lib.rs:125`, `:132` | Versatz und Drift von `clock_gettime` (`CLOCK_REALTIME`) und `gettimeofday` |

### 6.2 Werkzeuge der Test- und Messinfrastruktur

| Variable | Leser | Fundstelle |
|---|---|---|
| `ZERODDS_ID_CA`, `ZERODDS_ID_CERTIFICATE`, `ZERODDS_ID_PRIVATE_KEY`, `ZERODDS_PERMISSIONS_CA` | `tools/omg-security-main` (Fallback der Flags) | `tools/omg-security-main/src/cli.rs:288–309` |
| `ORACLE_READY_FILE` | `tools/iceoryx2-oracle-consumer` | `src/main.rs:103` |
| `ZERODDS_CORPUS`, `ZERODDS_C_RUNTIME`, `CC`, `CFLAGS`, `AR` | `wireprobe-c-matrix` | `tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs:458`, `:1188`, `:790`, `:1111`, `:1218` |
| `ZERODDS_CORPUS`, `ZERODDS_CPP_RUNTIME`, `CXX`, `CXXFLAGS`, `AR` | `wireprobe-cpp-matrix` | `tools/wireprobe/wireprobe-zerodds-cpp-gen/src/bin/wireprobe-cpp-matrix.rs:221`, `:968`, `:917`, `:929`, `:1005` |
| `CARGO` | `tools/cargo-dag` | `src/lib.rs:118` |
| `OUT_DIR` | `zerodds-build` (Build-Skript-Helfer) | `crates/zerodds-build/src/lib.rs:611` |
| `ZERODDS_SPHINX_THEME`, `XTYPES_PDF`, `INTEROP_OUT`, `CI_COMMIT_SHA`, `ROS_PREFIX` | Python-Skripte | `crates/py/docs/conf.py:71`, `tools/xtypes-1.3-gap-scan/roster.py:51`, `tools/interop-results/verify_record.py:78`, `:101`, `tools/wireprobe/rosidl-generate.py:80` |

Variablen, die nur in Tests, Beispielen oder Build-Skripten gelesen werden (z. B. `ZERODDS_SCALE_N`, `ZERODDS_OMG_DDSXML_DIR`, `ZERODDS_RUN_SECS`, `ZD_*_OUT`, `ICEORYX_LIB_DIR`), sind nicht aufgeführt.

## 7 Abweichungen

Keine offen. Gemessen: Die sechs Bridge-Daemons enden bei fehlender oder nicht parsebarer Datei unter `--check-config` mit 2, beim Start mit 1 (beim Start ist 2 der Bind-Fehler der Bridge-Specs); `crates/dcps/src/cli.rs` `HELP` nennt die 2 nur für `--check-config`.

Geprüft ohne Befund: Alle `KEYS`-Einträge der sechs Daemons kommen in der jeweiligen Man-Page vor. Alle sechs Beispiel-Dateien und `ros2.yaml.example` bestehen `--check-config`. `ZERODDS_CONFIG` wird im Code nicht gelesen, wie die Deployment-Spec sagt. `discovery.multicast` und `discovery.unicast_initial_peers` stehen in `docs/specs/zerodds-ros2-bridge-1.0.md` und `zerodds-ros2.yaml.5` nur als abgelehnte Namen.

## 8 Nicht ermittelt

- Ob die XML-Routen-Datei unbekannte Elemente ebenfalls übergeht: nicht gemessen, nur JSON.
- Welche Werte `transport.user_transport` in den ausgelieferten Paketen annimmt: `SHM`, `UDS`, `TSN` hängen von den Features `same-host-shm`, `same-host-uds`, `tsn-live` (Linux) ab; die Feature-Auswahl der Paket-Builds wurde nicht geprüft.
- Wirkung von `transport.interface_bindings` auf das Routing ausgehender Pakete: nur der Parser wurde gelesen, nicht `OutboundSocketPool::route` im Detail.
- Verhalten auf Windows und macOS (Systemverzeichnis, fehlendes `/run`): aus dem Code abgeleitet, nicht auf diesen Plattformen gemessen.
- Die Man-Pages wurden nur auf das Vorkommen der Schlüsselnamen geprüft, nicht Wert für Wert auf Defaults und Wertebereiche.
