# ROS 2

Diese Seite beschreibt die ROS-2-Anbindung von ZeroDDS: die RMW-Implementierung `rmw_zerodds_cpp` (Crate `crates/rmw-zerodds-shim` mit C-Schicht `rmw_c/`), die Mapping-Bibliothek `crates/ros2-rmw`, die Typkonvertierung `crates/spatial-ros2`, das Diagnose-CLI `zerodds-ros2-shim`, die `ros2.*`-Settings der Runtime-Konfiguration, die gemessene Interop und die bekannten Lücken. Grundlage ist der Code. Angaben, die nur in Specs, READMEs, Skript-Köpfen oder `internal/release/OPEN-ITEMS.md` stehen, sind mit „(laut Doku)“ markiert.

## Verwandte Seiten

- [README.md](README.md)
- [architecture.md](architecture.md)
- [crates-core.md](crates-core.md)
- [crates-bridges-services.md](crates-bridges-services.md)
- [crates-languages-codegen.md](crates-languages-codegen.md)
- [config.md](config.md)
- [cli-options.md](cli-options.md)
- [tools-and-man.md](tools-and-man.md)
- [system-setup.md](system-setup.md)
- [security.md](security.md)
- [migration.md](migration.md)
- [interop-and-dialects.md](interop-and-dialects.md)
- [ros2.md](ros2.md)

## Kurzfassung

- `librmw_zerodds_cpp.so` entsteht gegen ein installiertes ROS 2 über `crates/rmw-zerodds-shim/rmw_c/build_librmw_zerodds_cpp.sh` oder das ament-Paket `crates/rmw-zerodds-shim/ament` (`rmw_zerodds_cpp`). Als deb-Paket `ros-<distro>-rmw-zerodds-cpp` baut sie `packaging/linux/deb/build-rmw-deb.sh` (Startwert jazzy), kein rpm-Paket enthält sie; `packaging/docker/ros2-shim/Dockerfile` installiert sie aus dem Paket, das Image ist nicht gebaut (A18).
- Aktivierung über `RMW_IMPLEMENTATION=rmw_zerodds_cpp`. Der Identifier steht in `rmw_c/rmw_zerodds.c:93` und `crates/ros2-rmw/src/ffi_api.rs:51`.
- Die RMW liest ihre Settings aus denselben Ebenen wie jedes ZeroDDS-Programm: Umgebung, Dateien (`--config` auf der Kommandozeile des Knotens, sonst `zerodds.yaml` und `zerodds.d/` im Systemverzeichnis), `--set` über allem. Die Flags stehen außerhalb der `--ros-args … --`-Blöcke (`ros2_config::process_source`, `crates/rmw-zerodds-shim/src/ros2_config.rs`).
- Alle Messungen stammen aus ROS 2 Humble (RoboStack-Umgebung auf dem Linux-Testhost). Für Iron und Jazzy ist ein CI-Job definiert, der in keiner Pipeline eingebunden ist.
- Services laufen mit `rmw_cyclonedds_cpp` und `rmw_fastrtps_cpp` in beide Richtungen, jeweils in der Korrelationsform des Gegenübers. Gemessen auf dem Linux-Testhost (ROS 2 Jazzy, `std_srvs/SetBool`, Server und Client in zwei Prozessen, fünf Paare mit ZeroDDS auf mindestens einer Seite, je fünf Läufe à zehn Aufrufe): 250 von 250 Antworten. Vorher ging mit einem ZeroDDS-Server der erste Aufruf zeitweise verloren (ZeroDDS-Client 2 von 3 Läufen, Fast-DDS-Client 2 von 5); zwei Ursachen in `zerodds-rtps`, beide behoben: Ein Reader-Proxy, der hinter dem Cache beitrat, blieb für den Writer dauerhaft „zurück“, jede weitere SEDP-Ankündigung wartete auf den nächsten HEARTBEAT (100 ms), und DATA eines Writers, der vor dem eigenen Match schrieb, wurde erst nach dessen nächstem HEARTBEAT angefordert (Fast DDS: 3 s).

## Quick Start

Voraussetzungen: Linux (Skript nutzt `cc -shared` und `.so`), ROS 2 mit Dev-Headern (`/opt/ros/<distro>` oder eine RoboStack-Umgebung), Rust-Toolchain.

```bash
# 1) Rust-Bridge (librmw_zerodds.so) + C-API (libzerodds.so)
cargo build -p rmw-zerodds-shim -p zerodds-c-api --release

# 2) C-Schicht mit dem unpräfixierten rmw_*-ABI (librmw_zerodds_cpp.so)
ROS_PREFIX=/opt/ros/humble ZERODDS_TARGET=$PWD/target/release \
  bash crates/rmw-zerodds-shim/rmw_c/build_librmw_zerodds_cpp.sh

# 3) registrieren, wie es die Test-Runner tun
cp crates/rmw-zerodds-shim/rmw_c/librmw_zerodds_cpp.so /opt/ros/humble/lib/
cp target/release/librmw_zerodds.so target/release/libzerodds.so /opt/ros/humble/lib/
mkdir -p /opt/ros/humble/share/ament_index/resource_index/rmw_implementation
touch /opt/ros/humble/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp

# 4) benutzen
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
ros2 run demo_nodes_cpp talker

# 5) prüfen
cargo run -p rmw-zerodds-shim --bin zerodds-ros2-shim -- doctor
```

Optional: ein `ros2`-Drop-in nach `/etc/zerodds/zerodds.d/50-ros2.yaml`. Die Vorlage liegt in `packaging/linux/configs/ros2.yaml.example`.

## Crates

| Crate | Pfad | Inhalt | Workspace |
|---|---|---|---|
| `rmw-zerodds-shim` (lib `rmw_zerodds`) | `crates/rmw-zerodds-shim` | Rust-Bridge als `cdylib`/`staticlib`/`rlib` mit `rmw_zerodds_*`-Funktionen, `ros2_config.rs`, `qos.rs`, `services.rs`, Binary `zerodds-ros2-shim`, Header `include/rmw_zerodds.h` (von `build.rs` per cbindgen erzeugt) | Member (`Cargo.toml:165`) |
| C-Schicht | `crates/rmw-zerodds-shim/rmw_c` | `rmw_zerodds.c` (3181 Zeilen, unpräfixiertes `rmw_*`-ABI), `rmw_zerodds_cxx.cpp` (std::string-Teil des Introspection-Codecs), `rmw_zerodds_qos.cpp` (`rmw_qos_profile_check_compatible` über `rmw_dds_common`), `rmw_zerodds_ros2_ext.h` (REP-2007/REP-2008-Erweiterungen), Build-Skript, Test-Runner, C++-Testprogramme | kein Cargo-Teil |
| ament-Paket | `crates/rmw-zerodds-shim/ament` | `CMakeLists.txt`, `package.xml` (Paket `rmw_zerodds_cpp`) | – |
| `zerodds-ros2-rmw` | `crates/ros2-rmw` | no_std+alloc-Bibliothek: Topic-Mangling (`rt/`, `rq/`, `rr/`, `rs/`), Service- und Action-Topicnamen, QoS-Profile, rmw↔DDS-QoS-Mapping, Typnamen-Mapping, `.msg`→IDL, REP-2004-Qualitätsstufen, JSON-Log | Member (`Cargo.toml:285`) |
| `zerodds-spatial-ros2` | `crates/spatial-ros2` | Konvertierung SpatialDDS ↔ ROS-2-Nachrichten (`geometry_msgs`, `sensor_msgs`, `vision_msgs`, `tf2_msgs`, `geographic_msgs`) aus `idl/ros_msgs.idl`, dazu `rt/`-Topic- und `pkg::msg::dds_::Type_`-Typnamen. Kein RMW, keine laufende Bridge | Member (`Cargo.toml:171`); siehe Widersprüche |

Aufbau zur Laufzeit: rclcpp/rclpy → `librmw_zerodds_cpp.so` (C, `rmw_*`) → `librmw_zerodds.so` (Rust, `rmw_zerodds_*`) → `libzerodds.so` (`crates/zerodds-c-api`) → DCPS-Runtime. `rmw-zerodds-shim` nutzt aus `zerodds-ros2-rmw` nur `topic_mangling::mangle_topic_name` und `ffi_api::{RMW_IMPLEMENTATION_IDENTIFIER, RmwNode}`.

## Bauen gegen ein installiertes ROS 2

`build_librmw_zerodds_cpp.sh` (57 Zeilen):

- Pflicht-Umgebung: `ROS_PREFIX` (Install-Präfix), `ZERODDS_TARGET` (Cargo-Release-Verzeichnis mit `librmw_zerodds.so` und `libzerodds.so`). Optional `OUT`.
- Include-Pfade mit `-idirafter` für jedes `$ROS_PREFIX/include/*/`. Laut Kommentar im Skript verdeckt sonst Cyclones `dds/features.h` die glibc-Datei `<features.h>`.
- Linkt `-lrmw_zerodds -lzerodds -lrmw -lrmw_dds_common -lrcutils -lrosidl_typesupport_introspection_c -lrosidl_runtime_c -lstdc++`, setzt rpath auf `ZERODDS_TARGET` und `$ROS_PREFIX/lib`.
- Registriert nichts. Am Ende gibt es die Befehle zum Registrieren aus (Kopie nach `$ROS_PREFIX/lib`, Marker `share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp`).

Cargo-Features von `rmw-zerodds-shim` (`Cargo.toml`):

| Feature | Default | Wirkung |
|---|---|---|
| `flatdata-loan` | ja | Loan-Pfad für `delivery.mode: raw-same-host` (SHM) |
| `delivery-iceoryx` | nein | Loan-Pfad über iceoryx2 für `delivery.mode: iceoryx` |
| `security` | nein | `rmw_zerodds_init` baut aus `security.enclave_dir` einen DDS-Security-Participant |

Die Test-Runner bauen ohne `security`. `run_loaned_message_test.sh` und `run_qos_translation_test.sh` schalten `delivery-iceoryx` bei `ZERODDS_TEST_ICEORYX=1` zu.

Distributionen:

- Gemessen: Humble. Belegt sind `rmw_cyclonedds_cpp` 1.3.4 (`docs/ros-column.md`, `src/ros2_config.rs`) und `rmw_fastrtps` 6.2.7 (`src/services.rs`).
- Iron und Jazzy: `ci/jobs/rmw-distro-build.yml` definiert `rmw-humble`, `rmw-iron` und `rmw-jazzy`. Die Datei wird von keinem `include:` in `.gitlab-ci.yml` oder `ci/*.yml` referenziert (`git grep rmw-distro`). Ein Lauf ist damit nicht belegt.
- `zerodds-ros2-shim doctor` wertet `ROS_DISTRO` in `humble|iron|jazzy|rolling` als `ok`, alle anderen als `warn` (`src/bin/zerodds-ros2-shim.rs:554`).

ament-Weg (`ament/CMakeLists.txt`, README-Abschnitt „In a ROS-2 workspace“): Das Paket `rmw_zerodds_cpp` baut die C-Schicht gegen die ROS-Pakete, installiert `librmw_zerodds_cpp` zusammen mit `librmw_zerodds.<ext>` und `libzerodds.<ext>` aus `$ZERODDS_HOME/target/release` nach `lib/` und registriert die Implementierung im ament-Index (`CMakeLists.txt:1-12`, `135-139`); geprüft von `rmw_c/run_ament_package_test.sh`.

## Auswahl und Initialisierung

- `rmw_get_implementation_identifier` liefert `rmw_zerodds_cpp`.
- `rmw_init` übernimmt `options->domain_id` und setzt `RMW_DEFAULT_DOMAIN_ID` auf 0 (`rmw_zerodds.c`, `rmw_init`). Danach ruft es `rmw_zerodds_init(domain)` auf.
- `rmw_zerodds_init` (`src/lib.rs:706`) läuft so ab:
  1. `ros2_config::process_source()` liest `--config`/`--set` aus der Kommandozeile des Prozesses (außerhalb der `--ros-args … --`-Blöcke) und lädt darüber Umgebung, Dateien und `--set` (`ConfigSource::for_program`, Stamm `zerodds`), dann `Ros2Config::from_config`. Bei einem Fehler meldet es `rmw_zerodds_init: invalid configuration: …` auf stderr, gibt NULL zurück, und `rmw_init` scheitert.
  2. Mit Feature `security` und gesetztem `security.enclave_dir` entsteht der Participant aus derselben `ConfigSource` über `security_ffi::runtime_create_secure_ros_from_source`. Ein gesetztes, aber fehlerhaftes Enclave ergibt NULL; ein stiller Rückfall ist nicht vorgesehen.
  3. Sonst entsteht er über `runtime_create_ros_from_source` (`crates/zerodds-c-api/src/lib.rs`): Der Reader bietet XCDR1 und XCDR2 an.
  4. Es legt den Writer und Reader für `ros_discovery_info` an (NoKey, RELIABLE, TRANSIENT_LOCAL, KEEP_LAST(1)).

Systemverzeichnis (`crates/config/src/layers.rs:406-434`): `/etc/zerodds` unter Linux, `/usr/local/etc/zerodds` unter macOS, `%PROGRAMDATA%\ZeroDDS` unter Windows. Linux liest zusätzlich `/run/zerodds/zerodds.d`. Fehlende Dateien gelten als Defaults.

## Settings aus der Runtime-Konfiguration

Leser: `crates/rmw-zerodds-shim/src/ros2_config.rs` (`KEYS`, `Ros2Config::from_config`) für `ros2.*` außer `time_based_filter`. `ros2.time_based_filter`, `delivery.mode` und `security.enclave_dir` liest `RuntimeConfig` in `crates/dcps/src/runtime.rs`. `--set` wirkt in der RMW wie im Diagnose-CLI; `--check-config` auf der Kommandozeile eines Knotens wirkt nicht, den Bericht gibt `zerodds-ros2-shim --check-config` mit denselben Flags.

| Setting | Werte | Env | Wirkung | Fundstelle |
|---|---|---|---|---|
| `ros2.topic_mangling` | `rmw` (Default) \| `none` \| `custom` | – | `rmw`: `rt/<name>`, Services `rq/<name>Request` und `rr/<name>Reply`; `none`: Name ohne führenden `/`; `custom`: Präfix + Name | `Ros2Config::dds_topic`, `dds_service_topics` |
| `ros2.custom_mangling.topic_prefix` | String | – | Pflicht bei `custom`; ohne `custom` abgelehnt | `from_config` |
| `ros2.custom_mangling.request_prefix` / `response_prefix` | String, Default `rq/` / `rr/` | – | Service-Präfixe bei `custom` | `from_config` |
| `ros2.qos_profiles.<name>` | Felder `reliability`, `durability`, `history`, `depth`, `deadline_ms`, `lifespan_ms`, `liveliness`, `liveliness_lease_ms` | – | ersetzt Felder eines Standardprofils (`sensor_data`, `services`, `parameters`, `parameter_events`, `default`) oder legt ein neues an | `standard_profiles`, `read_patch` |
| `ros2.topic_overrides[]` | `ros_topic` + genau eines von `qos_profile` oder `qos` | – | ersetzt die genannten Felder der QoS, die die Anwendung anfordert; greift für Publisher, Subscriptions und Services | `effective_qos`, Aufrufe `src/lib.rs:1362`, `:1940`, `src/services.rs:396` |
| `ros2.time_based_filter` | Liste aus `SEKUNDEN` oder `/topic=SEKUNDEN`; `0` schaltet ab | `ZERODDS_TIME_BASED_FILTER` (Kommaliste) | `minimum_separation` pro Subscription; ein Topic-Eintrag geht vor dem Default | `runtime.rs:1527-1564`, `time_based_filter_for` (`:4724`), Test `runtime_config_layering.rs::delivery_mode_and_time_based_filter_are_typed_settings` |
| `delivery.mode` | `portable` (Default) \| `raw-same-host` \| `iceoryx` | `ZERODDS_DELIVERY_MODE` | Loan-Pfad der C-Schicht über `rmw_zerodds_delivery_mode` | `runtime.rs:1493-1511`, `src/lib.rs:1810` |
| `security.enclave_dir` | Verzeichnis | `ZERODDS_SECURITY_DIR` | SROS2-Enclave, nur mit Feature `security` | `crates/zerodds-c-api/src/security_ffi.rs:354` |
| übrige Runtime-Settings (`discovery.*`, `transport.*` …) | siehe [config.md](config.md) | – | gelten auch für die RMW | `ffi_runtime_config` |

Abgelehnt mit Begründung (`OWNED_ELSEWHERE`): `ros2.namespace` (rcl, `--ros-args -r __ns:=`), `ros2.ros_domain_id` (`ROS_DOMAIN_ID`), `ros2.enclave` (→ `security.enclave_dir`), `logging` (rcutils), `discovery.multicast` (→ `discovery.no_multicast`) und `discovery.unicast_initial_peers` (→ `discovery.peers`). Unbekannte Schlüssel in `ros2` und in den Profil- und Override-Maps führen ebenfalls zum Abbruch (`Section::only`). Andere ZeroDDS-Programme übergehen `ros2.*` (`is_rmw_path`, `runtime.rs:4083`).

`avoid_ros_namespace_conventions` verwendet den ROS-Namen samt führendem `/`, ohne Präfix. Bei Services wird nur `Request`/`Reply` angehängt.

Beispiel (Auszug aus `packaging/linux/configs/ros2.yaml.example`, Reader-Test `the_shipped_example_reads_as_it_says`):

```yaml
ros2:
  topic_mangling: rmw
  qos_profiles:
    sensor_data:
      depth: 5
  topic_overrides:
    - ros_topic: /scan
      qos_profile: sensor_data
    - ros_topic: /map
      qos:
        durability: transient_local
        history: keep_last
        depth: 1
  time_based_filter:
    - 0.1
    - /scan=0.5
```

Die `time_based_filter`-Zeilen stammen aus dem Test in `runtime_config_layering.rs`. Die Vorlage zeigt die Form `time_based_filter: "/scan=0.1"` nur auskommentiert. Ob die Datei-Form als String gelesen wird, ist nicht geprüft.

## Was die RMW umsetzt

Aus `rmw_zerodds.c`, `src/lib.rs` und `src/services.rs`:

- **Pub/Sub:** `rmw_publish`, `rmw_take`, `rmw_take_with_info`, serialisierte Varianten (Introspection-CDR, C- und C++-Typsupport), `rmw_serialize`/`rmw_deserialize` und `rmw_get_serialized_message_size`. DDS-Typname nach ROS-Konvention (`std_msgs::msg::dds_::Int32_`), NoKey-Endpunkte.
- **Wait:** `rmw_wait` wartet auf eine Condvar des Kontexts, die der Reader-Listener anstößt (Kommentar in `src/lib.rs`). Guard-Conditions und `rmw_*_set_on_new_*_callback` sind umgesetzt.
- **QoS:** Alle Felder von `rmw_qos_profile_t` gehen an den Endpunkt (`src/qos.rs`, `qos::resolve`). `rmw_*_get_actual_qos` liefert den angewandten Stand. `rmw_publisher_assert_liveliness` und `rmw_publisher_wait_for_all_acked` sind umgesetzt. `rmw_qos_profile_check_compatible` läuft über `rmw_dds_common` (`rmw_zerodds_qos.cpp`).
- **Services/Clients:** Topics `rq/<svc>Request` und `rr/<svc>Reply`, Typen `<pkg>::srv::dds_::<Srv>_Request_`/`_Response_`. Zwei Korrelationsformen (`src/services.rs`):
  - Cyclone-Form: `u64 guid` und `i64 seq` vor dem Body, `clientid=`/`serviceid=` in USER_DATA. Der Server wartet bis 100 ms auf den Reply-Reader.
  - Related-Identity-Form: Inline-QoS PID 0x800f (Fast DDS) und zusätzlich 0x0083 (DDS-RPC). ZeroDDS nutzt diese Form auch untereinander.
  - Der Server antwortet in der Form der Anfrage. Der Client wählt die Cyclone-Form nur, wenn alle gematchten Server Cyclone DDS sind.
  - `rmw_service_server_is_available` ist umgesetzt.
- **Graph:** `rmw_get_topic_names_and_types` (mit `no_demangle`), `rmw_get_service_names_and_types`, `rmw_count_publishers/subscribers/services/clients`, die vier `rmw_get_*_names_and_types_by_node`, `rmw_get_publishers_info_by_topic`/`rmw_get_subscriptions_info_by_topic`, `rmw_get_node_names` und `rmw_get_node_names_with_enclaves` über `ros_discovery_info`.
- **Loaned Messages:** `rmw_borrow/publish/take_loaned_message`, `can_loan_messages` nur für Typen fester Größe (`zerodds_msg_can_loan`). Delivery-Modi `portable`, `raw-same-host` (POSIX-SHM, kein Wire) und `iceoryx` (Feature `delivery-iceoryx`). Ohne das Feature liefert `rmw_zerodds_publisher_enable_iceoryx` `RMW_RET_UNSUPPORTED`, der Aufrufer bleibt laut Kommentar in `src/lib.rs:2379` bei `portable`.
- **REP-Erweiterungen** in `rmw_zerodds_ros2_ext.h`: REP-2007-Typadaption (`rmw_zerodds_publish_adapted`/`take_adapted`, auch Request und Response), REP-2008-Loan-Memory (`rmw_zerodds_*_get_loan_memory`, `ZERODDS_LOAN_ALIGNMENT`, Kernel-Levels 2 und 3), REP-2009-Typhash (`rmw_zerodds_compute_type_hash`, RIHS SHA-256).
- **Security:** nur mit Feature `security`, Quelle ist `security.enclave_dir`. Erwartete Dateien (`crates/security-runtime/src/profile.rs:583`): `cert.pem`, `key.pem`, `identity_ca.cert.pem`, `permissions_ca.cert.pem`, `governance.p7s`, `permissions.p7s`. rcl übergibt seine Security-Optionen (`security_root_path` aus `ROS_SECURITY_KEYSTORE` und der Enclave, `enforce_security` aus `ROS_SECURITY_STRATEGY=Enforce`) an `rmw_zerodds_init_with_security`: der Pfad ist dann das Enclave-Verzeichnis; `Enforce` ohne Enclave und ein Build ohne `security` mit ROS-Security sind Fehler; `security.enclave_dir` und rcl-Enclave zugleich werden abgelehnt (`docs/specs/zerodds-ros2-bridge-1.0.md` §7.1). Gemessen (`run_ros2_security_test.sh`, Linux-Testhost Humble): gesicherter zerodds-Talker zu gesichertem zerodds-Listener 12 Nachrichten, zu ungesichertem 0, zu gesichertem `rmw_cyclonedds_cpp` 13.

QoS-Events, Content-Filter, Message-Info (`rmw_zerodds.c`):

- **QoS-Events:** `rmw_publisher_event_init`/`rmw_subscription_event_init` für OFFERED/REQUESTED_DEADLINE_MISSED, LIVELINESS_LOST/CHANGED, OFFERED/REQUESTED_QOS_INCOMPATIBLE, MESSAGE_LOST und, wo die Distribution sie hat (`__has_include`), PUBLICATION/SUBSCRIPTION_MATCHED und *_INCOMPATIBLE_TYPE. Der Zustand liegt in der Runtime (`zerodds_writer_get_status`/`zerodds_reader_get_status`); ein Event ist bereit, wenn sein Zähler sich seit dem letzten `rmw_take_event` bewegt hat, und `rmw_take_event` meldet die Änderung. Der Status-Hook der Runtime (`DcpsRuntime::add_status_hook`) weckt `rmw_wait`. `rmw_event_set_callback` über einen Dispatcher-Thread je Kontext (`src/events.rs`). Gemessen (Linux-Testhost, Humble, `test_rmw_qos_events.py`): Deadline auf beiden Seiten, Incompatible-QoS (RELIABILITY) auf beiden Seiten. LIVELINESS_CHANGED zählt Übergänge wie der DCPS-`DataReader`, nicht die Zahl der gerade lebenden Writer.
- **Content-Filter:** `rmw_subscription_set/get_content_filter` und `content_filter_options` bei der Erzeugung; Ausdruck nach DDS 1.4 Annex B (`zerodds-sql-filter`), Felder per Name aus den Introspection-Membern (`a.b` verschachtelt); gilt für `rmw_take`, die Raw-Modi, Loan-, serialisierte und angepasste (REP-2007) Takes; in der SEDP-Subscription angekündigt. Test: `run_content_filter_test.sh` (rclcpp; rclpy in Humble hat keinen Content-Filter).
- **Message-Info:** `publisher_gid` ist die Writer-GUID (auch `rmw_get_gid_for_publisher`), `source_timestamp` der INFO_TS-Zeitstempel, `publication_sequence_number` die RTPS-Sequenznummer; Samples ohne diese Angaben (gleiche Runtime, Durability-Replay) melden 0 bzw. `RMW_MESSAGE_INFO_SEQUENCE_NUMBER_UNSUPPORTED`. `rmw_feature_supported` meldet beide Sequenznummern.
- **Weitere:** `rmw_take_sequence` nimmt bis zu `count` Nachrichten; `rmw_get_gid_for_client` liefert die GUID des Request-Writers; `rmw_publisher/subscription_get_network_flow_endpoints` die per SEDP angekündigten Unicast-Locators (UDP/TCP, IPv4/IPv6; in Humble über rclpy nicht erreichbar, nicht gemessen).
- **Nicht umgesetzt:** `rmw_init/fini_publisher_allocation`, `rmw_init/fini_subscription_allocation` liefern `RMW_RET_UNSUPPORTED` (wie `rmw_fastrtps_cpp`); `rmw_set_log_severity` ist ohne Wirkung.

## Diagnose-CLI `zerodds-ros2-shim`

Binary `zerodds-ros2-shim` (`crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`, `VERSION = "1.0.0"`). cargo-dist-Alias `zddsros2` (`Cargo.toml:498`). `zerodds-ros2` ist der Name des deb-Pakets, das dieses Binary enthält; ein Binary `zerodds-ros2` gibt es nicht. Das CLI ist kein Daemon. Es liest dieselben Ebenen wie die RMW (`ConfigSource::for_program(…, "zerodds", ros2_config::is_key, …)`).

| Subcommand | Funktion |
|---|---|
| `info` | Version, `rmw_implementation`, `ROS_DISTRO`, Domain, Mangling, Anzahl Profile und Overrides, Multicast, Peers, Enclave |
| `topics` | konfigurierte `topic_overrides` |
| `topic <ROS_TOPIC>` | DDS-Topicname unter dem aktiven Mangling |
| `qos <PROFILE>` | Profil aus `ros2.qos_profiles` (`default`, `sensor_data`, `services`, `parameters`, `parameter_events`, `configured`) |
| `enclaves` | `security.enclave_dir` |
| `validate <FILE>` | FILE als Runtime-Basis mit `FILE.d/` prüfen |
| `selftest` | Mangling- und QoS-Lookup-Rundlauf, liest keine Konfiguration |
| `catalog` | JSON-Katalog |
| `metrics` | Katalog der Prometheus-Metriknamen; die RMW zählt nichts davon (laut Doku, `docs/spec-coverage/zerodds-ros2-bridge-1.0.open.md`) |
| `doctor` | prüft `RMW_IMPLEMENTATION`, `ROS_DISTRO`, `ROS_DOMAIN_ID`, Discovery (Multicast oder Peers), Security und Mangling; Exit 5 bei `fail` |
| `graph` | lokaler Participant und konfigurierte Topics; keine entdeckten Remote-Endpunkte (Kommentar über `cmd_graph`) |

| Option | Bedeutung |
|---|---|
| `--config FILE` / `--config=FILE` | andere Runtime-Basis mit `FILE.d/`; dieselbe Wahl trifft ein Knoten mit `--config` auf seiner Kommandozeile |
| `--set PATH=VALUE` | ein Setting über allen Dateien, wiederholbar |
| `--check-config` | Dateien und `ros2`-Sektion prüfen, jeden Wert mit Herkunft ausgeben |
| `--domain ID` | Domain statt `ROS_DOMAIN_ID`; nur getrennt geschrieben, `--domain=ID` endet mit `unknown argument` |
| `--version`, `-V`; `--help`, `-h` | Version bzw. Hilfe |
| `--runtime-config`, `--log-level`, `--enclave` | abgelehnt, mit Hinweis auf den Ersatz (`REFUSED_FLAGS`) |

Exit-Codes: 0 bei Erfolg, 1 bei Aufruf- oder Konfigurationsfehlern, 5 bei `doctor` mit mindestens einem `fail`. Tests: `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` (16 `#[test]`).

Verteilung: deb-Paket `zerodds-ros2` mit `/usr/bin/zerodds-ros2-shim`, `/usr/share/zerodds/configs/ros2.yaml.example`, `man 1 zerodds-ros2-shim` und `man 5 zerodds-ros2.yaml` (`packaging/linux/deb/assemble-debs.sh:152-163`). Außerdem im Docker-Image `packaging/docker/ros2-shim/Dockerfile` (Default-Kommando `doctor`) und in `packaging/docker/cli/Dockerfile`. Es gibt keine systemd-Unit und keine launchd-Plist (`tools/traceability/tests/deployment_layout.rs::the_ros2_diagnostics_cli_is_not_installed_as_a_service`).

RMW-Paket: `ros-<distro>-rmw-zerodds-cpp` (`packaging/linux/deb/build-rmw-deb.sh`, `--distro`, Standard jazzy) installiert in `/opt/ros/<distro>`: `lib/librmw_zerodds_cpp.so`, die beiden von ihm per DT_NEEDED und RPATH `$ORIGIN` aufgelösten Bibliotheken `lib/librmw_zerodds.so` und `lib/libzerodds.so`, `include/rmw_zerodds_cpp/*.h`, die ament-Index-Einträge unter `share/ament_index/resource_index/{packages,rmw_typesupport}/rmw_zerodds_cpp` und `share/rmw_zerodds_cpp/package.xml` samt CMake-Dateien. Die Dateiliste erzeugt allein `cmake --install` des ament-Projekts (wie `zerodds-dev` in `assemble-debs.sh`); Maintainer-Skripte gibt es nicht, denn es ist eine Bibliothek und kein Daemon. Unterstützte Distros sind `humble jazzy kilted rolling`; der wöchentliche Job `ci/full.yml::ros-rmw-deb` läuft als Matrix einmal pro Distro (Images `ros:humble-ros-base-jammy`, `ros:jazzy-ros-base-noble`, `ros:kilted-ros-base-noble`, `ros:rolling-ros-base`), prüft die Dateiliste des `.deb`, installiert es mit `dpkg -i`, lässt rcl es über `RMW_IMPLEMENTATION` laden und tauscht danach mit `packaging/linux/deb/rmw-pubsub-check.py` in zwei Prozessen `std_msgs/String` „0“…„19“. Danach prüft `packaging/linux/deb/rmw-type-hash-check.py` den Typ-Hash (`typehash=RIHS01_…;` in USER_DATA), den ein zweiter Prozess liest, gegen das Paket selbst und in beiden Richtungen gegen `rmw_fastrtps_cpp` (nicht auf Humble, das keine Typ-Hashes kennt), und auf Rolling `packaging/linux/deb/rmw-service-info-check.py` Server und Clients eines Service.

## Gemessene Interop

Alle Runner liegen in `crates/rmw-zerodds-shim/rmw_c/`. Jeder baut `rmw-zerodds-shim` und `zerodds-c-api`, dann `librmw_zerodds_cpp.so`, und registriert die Bibliothek in `$ROS_PREFIX`. Die Ergebnisse stammen aus Doku oder Skript-Kopf; im Rahmen dieser Seite wurde nichts ausgeführt.

| Runner | Paare / Inhalt | Notiertes Ergebnis | Quelle des Ergebnisses |
|---|---|---|---|
| `run_ros2_services_test.sh` | AddTwoInts (`demo_nodes_py`): zerodds↔zerodds, cyclonedds→zerodds, zerodds→cyclonedds, fastrtps→zerodds, zerodds→fastrtps (bewertet); cyclonedds↔fastrtps (Referenz); 5 Graph-Läufe (`ros2 service list -t`, `ros2 node info`) | `2 + 3 = 5` in allen fünf bewerteten Paaren; Cyclone↔Fast DDS ohne Aufruf. Graph-Läufe nicht gesondert notiert | `internal/release/OPEN-ITEMS.md` (Services-Eintrag) |
| `run_ros2_config_test.sh` (als root, schreibt `/etc/zerodds/zerodds.d/90-ros2-config-test.yaml`) | 5a Override zerodds→zerodds; 5b `topic_mangling none`, Cyclone liest den DDS-Namen; 5c Gegenprobe ohne Datei; 5d `avoid_ros_namespace_conventions` zerodds→Cyclone | 4/4 | OPEN-ITEMS (`ros2.qos_profiles`-Eintrag) |
| `run_qos_translation_test.sh` | TRANSIENT_LOCAL-KEEP_LAST(1)-Late-Joiner; `RMW_PUB`/`RMW_SUB` für die Richtungen zu anderen Herstellern | Topics in beide Richtungen mit Cyclone und Fast DDS: `std_msgs/Int32`, `std_msgs/String`, `geometry_msgs/Twist`; Node-Graph in beide Richtungen | `docs/specs/zerodds-ros2-bridge-1.0.md` §10 (laut Doku) |
| `run_ros2_pytest.sh` | rclpy-Tests `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py` | 2 PASSED, Humble/RoboStack, 2026-06-13 | Skript-Kopf |
| `run_loaned_message_test.sh` | rclcpp-Loan in `portable`, `raw-same-host`, optional `iceoryx` | `can_loan=1 got=42 PASS` | Skript-Kopf, README |
| `run_type_adaptation_test.sh` | REP-2007 über `rmw_zerodds_ros2_ext.h` | kein Ergebnis notiert | – |
| `run_type_negotiation_test.sh` | REP-2009-Algorithmus von Hand über rmw_zerodds | kein Ergebnis notiert | – |
| `run_accel_loan_memory_test.sh` | REP-2008-Loan-Memory in zwei Delivery-Modi | kein Ergebnis notiert | – |

Auf DDS-Ebene ohne ROS-Installation (`crates/ros2-rmw/interop/`):

- `run_interop.sh` und das Beispiel `crates/dcps/examples/ros2_chatter_publisher.rs` gegen Cyclone DDS 11.0.1 auf `rt/chatter`: 20/20 Samples in beide Richtungen (laut `GROUND_TRUTH.md`).
- `run_multicast_free.sh` (zwei ZeroDDS-Prozesse, `ZERODDS_NO_MULTICAST=1`, Domain 0, Linux-Testhost, zwei Läufe): mit `ZERODDS_PEERS=127.0.0.1` 20 Samples, ohne Peers 0. Vorher in beiden Fällen 20: `spdp_multicast_send` wurde an keiner Sendestelle geprüft, und `ros2_chatter_subscriber` las über `RuntimeConfig::ros_defaults()` weder Umgebung noch Konfigurationsdateien.
- Weitere Skripte ohne notiertes Ergebnis in dieser Seite: `run_capture.sh`, `run_largedata.sh`, `run_multicast_free_xvendor.sh`, `run_wifi_largedata.sh`.

Nicht gemessen: `rmw_connextdds` (laut Spec §10 nicht installiert), Iron, Jazzy, Rolling, macOS, SROS2 gegen `rmw_fastrtps_cpp`, `topic_mangling: custom` gegen andere RMWs.

Serialisierung von ROS-Typen im Byte-Vergleich mit `rosidl_typesupport_fastrtps_c` und `rmw_cyclonedds_cpp`: `docs/ros-column.md`, siehe [interop-and-dialects.md](interop-and-dialects.md).

Rust-Tests (Anzahl der `#[test]`-Attribute gezählt, nicht ausgeführt):

- `rmw-zerodds-shim`: `src/lib.rs` 24, `src/ros2_config.rs` 7, `src/qos.rs` 4, `src/services.rs` 3, `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` 16, `crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs` 4.
- `zerodds-ros2-rmw`: 81 in `src/`, dazu 4 in `tests/cross_vendor.rs`. Davon sind die Tests mit ROS-Installation `#[ignore]`.
- `zerodds-spatial-ros2`: 7 in `crates/spatial-ros2/tests/mapping.rs`.

## Funktion | Stand | Beleg

| Funktion | Stand | Beleg |
|---|---|---|
| Laden über `RMW_IMPLEMENTATION=rmw_zerodds_cpp` | umgesetzt, Humble gemessen | `rmw_zerodds.c:93`; `run_ros2_pytest.sh` (Skript-Kopf) |
| Pub/Sub mit Cyclone und Fast DDS | umgesetzt, Humble gemessen (laut Doku) | Spec §10; `run_qos_translation_test.sh` |
| Services/Clients mit Cyclone und Fast DDS | umgesetzt, Jazzy auf dem Linux-Testhost gemessen: 5 Paare × 5 Läufe × 10 Aufrufe, 250/250 | `src/services.rs`; `run_ros2_services_test.sh`; OPEN-ITEMS |
| Graph-Abfragen (Topics, Services, Nodes, by_node, info_by_topic) | umgesetzt | `rmw_zerodds.c` ab 2197; Graph-Läufe in `run_ros2_services_test.sh` |
| QoS-Übersetzung, `get_actual_qos`, Kompatibilitätsprüfung | umgesetzt | `src/qos.rs`; `rmw_zerodds_qos.cpp`; `qos_translation_test.cpp` |
| QoS-Events (`rmw_take_event`, `rmw_event_set_callback`) | umgesetzt; Deadline und Incompatible-QoS auf dem Linux-Testhost (Humble) gemessen | `rmw_zerodds.c`; `src/events.rs`; `test_rmw_qos_events.py` |
| Content-Filter | umgesetzt | `rmw_zerodds.c`; `run_content_filter_test.sh` |
| `ros2.topic_mangling` (rmw/none/custom) | umgesetzt; `none` gegen Cyclone gemessen (laut Doku) | `ros2_config.rs`; `run_ros2_config_test.sh` 5b |
| `ros2.qos_profiles`, `ros2.topic_overrides` | umgesetzt; Override zerodds↔zerodds gemessen (laut Doku) | `ros2_config.rs`; `run_ros2_config_test.sh` 5a |
| `ros2.time_based_filter` | umgesetzt, Parser getestet | `runtime.rs:1527`; `src/lib.rs:2007`; `runtime_config_layering.rs` |
| `delivery.mode` portable / raw-same-host / iceoryx | umgesetzt; Loan-Test grün (laut Doku) | `src/lib.rs:1810`; `run_loaned_message_test.sh` |
| SROS2 über rcl-Security-Optionen oder `security.enclave_dir` | umgesetzt hinter Feature `security`; gemessen zerodds↔zerodds und zerodds → `rmw_cyclonedds_cpp` | `src/lib.rs` (`rmw_zerodds_init_with_security`); `run_ros2_security_test.sh` |
| REP-2007/2008/2009-Erweiterungen | umgesetzt; Runner vorhanden, Ergebnis nicht notiert | `rmw_zerodds_ros2_ext.h`; `crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs` |
| Diagnose-CLI | umgesetzt, 16 CLI-Tests | `src/bin/zerodds-ros2-shim.rs`; `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` |
| Prometheus-Metriken, OTLP-Spans im rmw-Pfad | offen (laut Doku) | `docs/spec-coverage/zerodds-ros2-bridge-1.0.open.md` |
| Paket `ros-<distro>-rmw-zerodds-cpp`, Image mit RMW | umgesetzt, nicht gelaufen | `packaging/linux/deb/build-rmw-deb.sh`; `ci/full.yml::ros-rmw-deb`; `packaging/docker/ros2-shim/Dockerfile`; Tests in `crates/rmw-zerodds-shim/tests/ros2_deb_package.rs` |
| Konfigurationsdatei pro Node oder ohne root | `--config`/`--set` auf der Kommandozeile des Knotens | `rmw_c/run_ros2_config_test.sh` 5e–5g, Linux-Testhost Humble gegen rmw_cyclonedds_cpp |
| CI für Humble/Iron/Jazzy | Job definiert, nicht eingebunden | `ci/jobs/rmw-distro-build.yml`; `git grep rmw-distro` |
| SpatialDDS↔ROS-2-Konvertierung | umgesetzt, nur Unit-Tests; keine Live-Kommunikation belegt | `crates/spatial-ros2/src/convert.rs`; `crates/spatial-ros2/tests/mapping.rs` |

## Bekannte Lücken

Aus `internal/release/OPEN-ITEMS.md` und dem Code:

- Kein rpm-Paket führt `librmw_zerodds_cpp.so`; das deb-Paket `ros-<distro>-rmw-zerodds-cpp` und das Image, das es installiert, sind gebaut, aber nicht gelaufen: der wöchentliche Job `ci/full.yml::ros-rmw-deb` ist nie gelaufen, das Image ist nicht gebaut (A18).
- SROS2 fehlt im Standard-Build (Feature `security`).
- Kein belegter Lauf für Iron, Jazzy und Rolling; macOS und Windows werden vom Build-Skript nicht abgedeckt.
- Prometheus-Zähler und OTLP-Spans im rmw-Pfad fehlen (laut Doku).
- Laut `README.md` des Crates: Die Bereitschaft in den Raw-Modi ist ein Prefetch pro Wait-Wakeup. Den prozessübergreifenden Futex über einen c-api-Notify-Getter in die `rmw_wait`-Condvar einzuhängen ist offen (`docs/specs/zerodds-delivery-modes-1.0.md` §11).

## Widersprüche zwischen Doku und Code

Keine offen; die hier früher aufgeführten sind behoben (README, Spec §12,
`package.xml`/`ament/CMakeLists.txt`, Manpage, deb-Beschreibung,
`spatial-ros2`, Kommentar an `RmwZerodsContext`).
