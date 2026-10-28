# Interop und Dialekte

Diese Seite beschreibt, wie ZeroDDS an ein laufendes System eines anderen Vendors auf dem Wire angeschlossen wird: Domain und Ports, Discovery, Vendor-IDs und vendorspezifische Parameter-IDs, Typkompatibilität (TypeObject, TypeLookup, Extensibility, XCDR1/XCDR2, Big Endian), Security-Interop, und alle Schalter, die das Wire-Verhalten an einen Peer anpassen. Für jeden Schalter stehen Name, Ort (Laufzeit-Einstellung, `zerodds-idlc`-Flag, Werkzeug-Flag, API), Werte, Standard, zuständiger Peer und Beleg. Grundlage ist der Code auf `main`. Aussagen, die nur aus einer Doku-Datei oder einem Code-Kommentar stammen, sind mit „(laut Doku)“ bzw. „(laut Code-Kommentar)“ markiert. Die Umstellung einer Anwendung steht in [migration.md](migration.md).

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## 1 Ergebnis in Kürze

- **Discovery und Ports** folgen DDSI-RTPS 2.5 §9.6.1.4.1 ohne Abweichung; die Parameter PB, DG, PG und d0–d3 der Formel sind die Einstellungen `ports.*`, Standard sind die Werte der Spezifikation (`crates/rtps/src/wire_types.rs:1015–1154`, `PortMapping::SPEC` bei `:1055`; gelesen in `crates/dcps/src/runtime.rs:4450–4467`).
- **Sieben Laufzeit-Einstellungen** passen das Wire an einen Peer an: `wire.data_representation_offer`, `wire.data_representation_match`, `wire.rtps_dialect`, `wire.codec_dialect`, `wire.big_endian`, `xtypes.key_member_bounds`, `xtypes.type_consistency_absent` (`crates/dcps/src/runtime.rs:3034–3238`, Tabelle `SETTINGS`). In der Praxis wird fast nur `wire.data_representation_offer` gebraucht.
- **CDR-Dialekte für generierten Code.** Die zwölf Dialekte (`zerodds_ast_layout::Dialect`) und ihre Achsen erreicht generierter Anwendungscode über `zerodds-idlc generate --dialect <name>` und `--<achse> <wert>` (`tools/idlc/src/main.rs:601`) und über `zerodds-build` (`Config::dialect`, `Config::axis`, `crates/zerodds-build/src/lib.rs:283`, `:325`); Standard ist `specification`. Namen und Werte der Achsen-Flags stehen einmal in `crates/ast-layout/src/axes.rs` (`AXES`, 26 Flags) und gelten gleich für `wireprobe-zerodds` und die `wireprobe-zerodds-*-gen`. Die reflektiven Laufzeit-Codecs lesen und schreiben im Dialekt der Einstellung `wire.codec_dialect` (`ZERODDS_CODEC_DIALECT`, Standard `specification`): Body-Prüfung und Key-Ableitung der Sprach-Bindings (`SerializedTypeSupport`, gesetzt in `create_serialized_topic` und beim Key-Hash-Aufbau im Runtime), webdds-bridge (Gateway und RPC), ROS-2-Dynamic-Typesupport (`crates/rmw-zerodds-shim/src/dynamic.rs`, einmal pro Prozess aus denselben Konfigurationsschichten), und als Voreinstellung `zerodds-spy`/`zerodds-record --dialect`. Sie ist der Dialekt, in dem die Typen der Anwendung generiert sind; wie `wire.rtps_dialect` wird sie konfiguriert und nicht aus der Vendor-ID eines Peers abgeleitet. Unverändert in der Spezifikation: `wire-probe::probe`/`unprobe` (Kurzform-API) und `omg-xtypes-test-main`.
- **Der für Anwendungen relevante IDL-Schalter** ist `zerodds-idlc --default-extensibility` (Cyclone: `final`).
- **Einige Anpassungen geschehen automatisch** anhand der Vendor-ID des Peers (OpenDDS-Handshake, eProsima-PID 0x800f, Cyclone-ROS-Services) oder fest für alle Peers (INFO_DST in SEDP und `VolatileSecure`).
- **Gemessen** sind Cyclone DDS 11.0.1, Fast DDS 3.6.1, OpenDDS 3.34.0 und RTI Connext 7.7.0 live und auf Serialisierungsebene; OpenSplice CE 6.9 live (OMG `dds-rtps`, `-x 1`) und auf Serialisierungsebene (191 Zellen mit Oktetten, Dialekt `opensplice`); HDDS 1.4.8 live (OMG `dds-rtps`) und mit `hddsgen` 1.2.0 auf Serialisierungsebene (`tools/interop-results/src/systems.rs`, `documentation/07-migration/from-opensplice.md`).

## 2 Domain, Ports, Discovery

### 2.1 Ports

| Größe | Wert | Formel | Belegstelle |
|---|---|---|---|
| SPDP-Multicast-Gruppe | `239.255.0.1` | – | `wire_types.rs:1016` (`SPDP_DEFAULT_MULTICAST_ADDRESS`) |
| SPDP-Multicast-Port | 7400 + 250·D | `PB + DG·D + d0` | `wire_types.rs:1122` |
| SPDP-Unicast-Port | 7410 + 250·D + 2·P | `PB + DG·D + d1 + PG·P` | `wire_types.rs:1132` |
| User-Multicast-Port | 7401 + 250·D | `PB + DG·D + d2` | `wire_types.rs:1142` |
| User-Unicast-Port | 7411 + 250·D + 2·P | `PB + DG·D + d3 + PG·P` | `wire_types.rs:1152` |

D = Domain-ID, P = Participant-Index. ZeroDDS belegt den ersten freien Index 0..119 (`crates/dcps/src/runtime.rs:9147–9165`); sind alle belegt, fällt es auf einen ephemeren Port zurück und ist dann nur über Multicast auffindbar (Kommentar `runtime.rs:9141`). Die Multicast-Gruppe ist über `discovery.spdp_multicast_group` änderbar, PB, DG, PG und d0–d3 über `ports.*`; die Tabelle zeigt die Standardwerte.

### 2.2 Discovery-Einstellungen für fremde Netze

| Einstellung | Umgebung | Werte | Standard | Wirkung |
|---|---|---|---|---|
| `discovery.peers` | `ZERODDS_PEERS` | Liste `ip` oder `ip:port` | leer | Unicast-SPDP zusätzlich zu Multicast; `ip` ohne Port wird auf die SPDP-Unicast-Ports der ersten N Indizes erweitert (`runtime.rs:5114–5162`) |
| `discovery.max_peer_participants` | `ZERODDS_MAX_PEER_PARTICIPANTS` | 1..120 | 10 | N aus der Zeile darüber; Entsprechung zu Cyclones `MaxAutoParticipantIndex` (Kommentar `runtime.rs:5114`) |
| `discovery.no_multicast` | `ZERODDS_NO_MULTICAST` | `true`/`false` | `false` | kein einziges SPDP-Multicast-Paket (`runtime.rs:1851–1856`) |
| `discovery.multicast_allowlist` | `ZERODDS_MULTICAST_ALLOWLIST` | IPv4-Multicast-Adressen | leer | siehe [config.md](config.md) |
| `transport.interface` | `ZERODDS_INTERFACE` | IPv4-Adresse | automatisch | alle IP-Sockets binden an diese Adresse (Kommentar `runtime.rs:9143–9146`, Vergleich mit Cyclone `NetworkInterface`) |
| `discovery.initial_announce_count`, `discovery.initial_announce_period_ms` | `ZERODDS_INITIAL_ANNOUNCE_*` | Ganzzahlen | 10, 200 ms (`runtime.rs:253–256`) | SPDP-Folge beim Start |
| `discovery.spdp_period_ms` | `ZERODDS_SPDP_PERIOD_MS` | ms > 0 | 30000 (`runtime.rs:166`); bei einer Lease unter 90 s ein Drittel der Lease, mindestens 100 ms (`default_spdp_period_for`, `:5082`) | SPDP-Periode |
| `discovery.server` | `ZERODDS_DISCOVERY_SERVER` | `true`/`false` | `false` | eigener SPDP-Relay; SEDP bleibt Peer-zu-Peer (`runtime.rs:1858–1868`). Kein Client für das Fast-DDS-Discovery-Server-Protokoll |

Vorgefertigte Profile in Rust: `RuntimeConfig::ros_defaults()` (`[XCDR1, XCDR2]`), `RuntimeConfig::multi_robot()` (ohne Multicast, Lease 300 s) (`runtime.rs:4872–4922`).

### 2.3 Antwort auf ein neues SPDP

Beim ersten Beacon eines neuen Participants antwortet ZeroDDS sofort. Achse `SpdpResponseTargets` (`crates/rtps/src/dialect.rs`):

| Wert | Ziel | Wer ihn braucht |
|---|---|---|
| `AnnouncedLocatorsAndBeaconSource` (Standard) | annoncierte Locators und Absenderadresse | OpenDDS 3.34.0 unter `rtps_protection != NONE`; gemessen 0/5 mit dem anderen Wert, 5/5 mit diesem (laut Code-Kommentar) |
| `AnnouncedLocatorsOnly` | nur annoncierte Locators | Relay- und NAT-Szenarien; Cyclone und Fast DDS werden von beiden Werten erreicht (26/26, laut Code-Kommentar) |

Gesetzt über `wire.rtps_dialect` (6.1.3): `cyclone` und `fastdds` wählen `AnnouncedLocatorsOnly`, `maximally-interoperable` und `opendds` den Standard.

## 3 Vendor-IDs und vendorspezifische Parameter-IDs

### 3.1 Vendor-IDs

| Konstante | Wert | Belegstelle |
|---|---|---|
| `VendorId::ZERODDS` | `0x01F0` (Übergangswert, nicht in der OMG-Liste) | `crates/rtps/src/wire_types.rs:87–101` |
| `VendorId::OPENDDS` | `0x0103` | `wire_types.rs:104` |
| `VendorId::FASTDDS` | `0x010F` | `wire_types.rs:107` |
| `VendorId::CYCLONE` | `0x0110` | `wire_types.rs:110` |
| RTI | `0x0101`, nur in Tests (`crates/rtps/src/publication_data.rs:1977`, `:2032`) | – |

### 3.2 Vendorspezifische PIDs

DDSI-RTPS 2.5 §9.4.2.11.2 Tabelle 9.6 gibt den Bereich ab `0x8000` allen Vendoren gemeinsam; ein solcher PID bedeutet, was der sendende Vendor festlegt (`crates/rtps/src/parameter_list.rs:761`, `publication_data.rs:1216–1236`).

| PID | Name | Richtung | Regel | Belegstelle |
|---|---|---|---|---|
| `0x8001` | `SHM_LOCATOR` | ZeroDDS schreibt ihn in eigene Announcements; kein Leser im Code (laut Code-Kommentar) | Same-Host-Locator, ohne Must-Understand-Bit | `parameter_list.rs:231–247` |
| `0x8002` | `PID_ZERODDS_TYPE_ID` | ZeroDDS schreibt nur unter eigener Vendor-ID; gelesen nur von `VendorId::ZERODDS` | RTI 7.7.0 sendet einen eigenen `0x8002`; früher als `TK_BOOLEAN` gelesen, alle RTI-Endpoints abgelehnt | `publication_data.rs:689–706`, `:1004–1043`, `subscription_data.rs:482` |
| `0x800f` | eProsima `PID_RELATED_SAMPLE_IDENTITY` | ZeroDDS schreibt ihn neben `0x0083` bei jeder Antwort mit Bezug; gelesen nur von Fast DDS und ZeroDDS | Fast DDS 2.6 liest nur `0x800f` | `crates/rtps/src/inline_qos.rs:119–127`, `crates/dcps/src/runtime.rs:11167–11175`, `:17854–17862` |
| `0x801c` | Cyclone `PID_CYCLONE_REQUESTS_KEYHASH` | ZeroDDS sendet ihn nicht; fehlt `PID_KEY_HASH`, berechnet der Reader den KeyHash aus seinem Typ | Cyclone sendet `PID_KEY_HASH` nur an Reader mit diesem PID | `crates/dcps/src/runtime.rs:25692` (`compute_alive_key_material`) |
| `0x0D00` | `PID_VENDOR_TRACE_CONTEXT` | ZeroDDS schreibt, im Standardbereich ohne Must-Understand-Bit | fremde Empfänger ignorieren ihn | `parameter_list.rs:257–263` |

Must-Understand: Ein unbekannter PID mit Bit `0x4000` verwirft die Nachricht (`parameter_list.rs:758`, `validate_must_understand_in_data_pipeline` in `crates/rtps/src/datagram.rs:743`, `:789`, `:896`). `PID_DOMAIN_TAG` (`0x4014`) ist Standard-PID: ZeroDDS sendet ihn bei gesetztem `discovery.domain_tag` und ignoriert Participants mit anderem Tag (`participant_data.rs`, `handle_spdp_datagram`).

### 3.3 Wo die Vendor-ID das Verhalten steuert

Die Regel für RTPS-Dialekte lautet „configured, never inferred“ (`crates/rtps/src/dialect.rs`, Modulkopf; `runtime.rs:11539–11547`). Davon unabhängig liest der Code die Vendor-ID an diesen Stellen:

| Stelle | Bedingung | Wirkung |
|---|---|---|
| `publication_data.rs:1027`, `subscription_data.rs:482` | Peer ≠ ZeroDDS | `0x8002` wird nicht gelesen, Typidentität aus `PID_TYPE_INFORMATION` |
| `runtime.rs:11170–11175` (`speaks_eprosima_pids`) | Peer = Fast DDS oder ZeroDDS | `0x800f` wird als Related Sample Identity gelesen |
| `crates/discovery/src/security/stack.rs:1098–1103` (`prime_plugin`, aufgerufen in `begin_handshake_with` `:1025` und `on_stateless_message` `:1694`) | Peer = OpenDDS, nur mit `security.algo_name_nul: by_vendor` | NUL-terminierte `c.dsign_algo`/`c.kagree_algo` im Handshake; der Standard `spec` schreibt das NUL für jeden Peer (`crates/security/src/vendor_forms.rs:47–53`) |
| `crates/rmw-zerodds-shim/src/services.rs:425–433`, `:891` | Server = Cyclone | ROS-2-Service-Sonderweg für `rmw_cyclonedds` |

## 4 Typkompatibilität

### 4.1 TypeObject und TypeLookup

- ZeroDDS annonciert Typinformation, wenn der Writer oder Reader mit TypeObject angelegt wurde (Rust-`DdsType`, C `*_create_typed`). Die plain C-Funktionen `*_create` annoncieren `TypeIdentifier::None` (`crates/zerodds-c-api/README.md`, „Type model“).
- Fremde Announcements: `PID_TYPE_INFORMATION` wird in einen vergleichbaren `TypeIdentifier` übersetzt; bei einem Parse-Fehler bleibt der Rohblock erhalten und die Match-Ebene meldet das (`publication_data.rs:1046–1055`).
- TypeLookup-Server und -Client laufen in jeder `DcpsRuntime` (`runtime.rs:8372–8381`, `:9560–9562`). Der Server liefert den Abschluss aller per Hash benannten Abhängigkeiten, weil RTI Connext 7.7.0 die COMPLETE-Form anfragt und die Abhängigkeiten mit erwartet (`crates/discovery/src/type_lookup/server.rs:116–286`, laut Code-Kommentar). Cyclone-Belege: die `(TypeLookup)`-Zellen von `tests/interop/ci/run_vendor.sh cyclone`; lokal `crates/discovery/tests/type_lookup_requests.rs`, `cyclone_typelookup_responder.rs`.
- OpenDDS verlangt standardmäßig vollständige XTypes-Information vom Remote-Writer; gegen ZeroDDS-Writer ohne TypeObject ist dort `use_xtypes=no` zu setzen (Kommentar `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`).
- Key-Member und `IS_MUST_UNDERSTAND`: XTypes 1.3 §7.2.2.4.4.4.8 setzt das Flag für jeden Key-Member; ZeroDDS und Cyclone tun das. Die von fastddsgen für Fast DDS 3.6.1 erzeugte Typunterstützung setzt es nicht (gemessen: Member-Flags `0x0021` in ihrer TypeLookup-Antwort), opendds_idl 3.34 nur bei geschriebenem `@must_understand`. Die TypeIdentifier unterscheiden sich dann, und ein Fast-DDS-Reader matcht einen ZeroDDS-Writer, der seine TypeInformation ankündigt, nicht; mit `@must_understand` am Key in der IDL der Gegenseite stimmen sie überein (`tools/dispose-reader/fastdds/KeyedPointA.idl`, `…/opendds/KeyedPointA.idl`).
- Key-Member-Grenzen (XTypes 1.3 Tabelle 19): Einstellung `xtypes.key_member_bounds`, 6.1.5.

### 4.2 Extensibility und Encapsulation

Der Writer setzt den Encapsulation-Header nach erstem angebotenem Format und deklarierter Extensibility (`crates/dcps/src/runtime.rs:714–893`, `crates/dcps/src/publisher.rs:3585–3603`):

| Repräsentation | `@final` | `@appendable` | `@mutable` |
|---|---|---|---|
| XCDR1 LE | `0x0001` CDR_LE | `0x0001` CDR_LE | `0x0003` PL_CDR_LE |
| XCDR2 LE | `0x0007` PLAIN_CDR2_LE | `0x0009` D_CDR2_LE | `0x000b` PL_CDR2_LE |
| Big Endian | jeweils ID − 1 (`0x0000`, `0x0002`, `0x0006`, `0x0008`, `0x000a`) | | |

Der Reader dekodiert nach dem empfangenen Header, einschließlich Big Endian (`crates/dcps/src/subscriber.rs:56–79`). Gemessene Writer-Formen der Vendoren (laut Doku, `docs/spec-coverage/cross-vendor-validation.md`, „What each writer puts on the wire“): Cyclone `@final` XCDR1, extensible XCDR2; Fast DDS und RTI alles XCDR1, `@mutable` als PL_CDR; OpenDDS alles XCDR2.

### 4.3 Datenrepräsentation

- Ein Writer serialisiert jedes Sample einmal, im ersten Element seiner Angebotsliste (`crates/rtps/src/publication_data.rs:75–94`). Ein Writer kann daher nicht gleichzeitig einen reinen XCDR1- und einen reinen XCDR2-Reader bedienen (laut Doku, `cross-vendor-validation.md`).
- Standard `[XCDR1]` (XTypes 1.3 §7.6.3.1.1; `[XCDR1, XCDR2]` ist Einstellung): passt zu Readern mit der DDS-Standardliste `[XCDR1]` (Cyclone- und Fast-DDS-`shape_main`, RTI-DataReader). Ein Reader, der nur XCDR2 annimmt (Cyclone `shape_main -x 2`), matcht nicht; Fall `N1` in `tools/dcps-shapes-interop.sh:249–255`.
- Eine Runtime aus der C-API (`zerodds_runtime_create*`) bietet `[XCDR2, XCDR1]` an, weil ihre Aufrufer XCDR2-Bytes übergeben (`ffi_data_representation_offer`, `crates/zerodds-c-api/src/lib.rs`). Ein Fast-DDS-3.6.1-Reader mit Standard-QoS sendet kein `PID_DATA_REPRESENTATION`, fordert also nur XCDR an, und matcht einen solchen Writer nicht (gemessen); XTypes 1.3 §7.6.3.1.1 verlangt von einem Reader ab Version 1.2, XCDR2 anzufordern. Abhilfe auf Fast-DDS-Seite: `representation()` um `XCDR2_DATA_REPRESENTATION` ergänzen (`tools/dispose-reader/fastdds/reader.cpp`).
- Pro Writer oder Reader: `DataWriterQos.data_representation`, `DataReaderQos.data_representation` (`Option<Vec<i16>>`, `crates/dcps/src/qos.rs:74`, `:235`).

### 4.4 Byte-Reihenfolge

Alle Vendoren schreiben auf x86 Little Endian. ZeroDDS liest Big Endian immer und schreibt es mit `wire.big_endian` (6.1.4). Gegenprobe laut Doku: Cyclone 59/59, Fast DDS 60/60, OpenDDS 40/40, RTI 60/60 dekodieren ZeroDDS-Big-Endian (`cross-vendor-validation.md`, „Big-endian axis“).

## 5 Security-Interop

Details in [security.md](security.md). Für den Anschluss an ein Fremdsystem gilt:

- Gleicher Identitäts-CA, gleiche Governance- und Permissions-Dokumente im OMG-Format; ZeroDDS lädt sie z. B. über `security.enclave_dir` (SROS2-Layout, `runtime.rs:2326–2340`).
- Handshake-Rolle: die kleinere GUID initiiert (Kommentar `crates/discovery/src/security/stack.rs:925–928`, gegen Cyclone geprüft).
- Automatisch und nicht abschaltbar: INFO_DST vor jedem gerichteten SEDP-Datagramm (`crates/discovery/src/sedp/writer.rs:565–597`) und vor jedem `VolatileSecure`-Datagramm, weil Cyclones gefilterter `VolatileSecure`-Reader auf die volle GUID matcht (`crates/discovery/src/security/volatile_secure.rs:91–94`).
- Gemessene Matrix (laut Doku, `docs/spec-coverage/cross-vendor-validation.md`, „DDS-Security, cross-vendor“): ZeroDDS↔Cyclone gesichert in beiden Richtungen; ZeroDDS↔OpenDDS für den reichen Typ; ZeroDDS↔Fast DDS nur mit RSA-Identitätszertifikaten; RTI nicht gemessen (Lizenz).
- `docs/interop-harnesses.md` („The gaps“): keine Zelle startet zwei gesicherte Participants aus einem Governance-/Permissions-Satz und prüft ein dekodiertes Sample.

## 6 Schalter, die das Wire an einen Peer anpassen

### 6.1 Laufzeit-Einstellungen

Ort: `zerodds.yaml`/`zerodds.d/`, Umgebung, `--set path=value`; Rangfolge Standard < Umgebung < Datei < Flag (`runtime.rs:3013–3024`). Ungültige Werte in einer Datei verhindern den Start mit Fundstelle (`runtime.rs:3521–3579`). Ungültige Werte nur in der Umgebung fallen auf den Standard zurück; für `wire.rtps_dialect` meldet die Laufzeit dann `config.rtps_dialect.unknown` auf dem Observability-Sink (`runtime.rs:5030–5074`, `:9446–9458`).

#### 6.1.1 `wire.data_representation_offer`

| | |
|---|---|
| Umgebung | `ZERODDS_DATA_REPR_OFFER` |
| Werte | `XCDR1`, `XCDR2`, als Kommaliste oder YAML-Liste, Reihenfolge bedeutsam |
| Standard | `XCDR1` (`DEFAULT_OFFER`, `publication_data.rs`) |
| Wirkung | Angebotsliste in SEDP; erstes Element = Format jedes Samples |
| Peer | `[XCDR2]` für Reader, die nur XCDR2 annehmen (Cyclone mit `-x 2`); Standard für RTI, Fast DDS, Cyclone-Standard, ROS 2 |
| Beleg | `tools/dcps-shapes-interop.sh` Fälle M1–M16 und N1; `crates/dcps/tests/sedp_data_representation_pid.rs`; Test `crates/dcps/tests/runtime_config_layering.rs:302–316` |
| Sonderfall | Die C-API setzt für `zerodds_runtime_create_ros_defaults` die ROS-Liste, wenn der Wert nicht gesetzt ist (`crates/zerodds-c-api/src/lib.rs:1087`) |

#### 6.1.2 `wire.data_representation_match`

| | |
|---|---|
| Umgebung | `ZERODDS_DATA_REPR_MATCH` |
| Werte | `strict` (XTypes 1.3 §7.6.3.1.1: die angebotene Repräsentation des Writers in der Reader-Liste), `tolerant` (irgendeine Überschneidung) |
| Standard | `strict`, die Klausel (`runtime.rs:4653–4658`; `DataRepMatchMode::default()`, `publication_data.rs:129`) |
| Wirkung | nur auf der Reader-Seite; auf der Writer-Seite entscheiden beide Modi gleich, weil der Writer nur sein erstes Element sendet (Kommentar `publication_data.rs:102–126`) |
| Peer | keiner gemessen; `tolerant` dient einem entfernten Writer, dessen erstes angebotenes Element in der Reader-Liste fehlt, während ein späteres darin steht |
| Beleg | `runtime_config_layering.rs:875` (`the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back`), `sedp_data_representation_pid.rs:387`; OMG `Test_DataRepresentation_0..3` mit `strict` gegen Connext, Cyclone, Fast DDS, beide Richtungen: 24/24 (Linux-Testhost) |

#### 6.1.3 `wire.rtps_dialect`

| | |
|---|---|
| Umgebung | `ZERODDS_RTPS_DIALECT` |
| Werte | `maximally-interoperable`, `cyclone`, `fastdds`, `opendds` (`crates/rtps/src/dialect.rs:425–516`) |
| Standard | `maximally-interoperable` |
| Wirkung | zwei Achsen: Form des SEDP-Dispose (`DisposeKeyForm`) und Ziel der SPDP-Antwort (`SpdpResponseTargets`) |
| Pro Peer | `DcpsRuntime::set_peer_rtps_dialect(prefix, dialect)`, `set_peer_rtps_behaviour(prefix, behaviour)` (`runtime.rs:11539–11570`); für alle Teilnehmer eines Herstellers `set_vendor_rtps_behaviour(vendor, behaviour)` (`runtime.rs:11597–11626`); nur per API |
| Beleg | `crates/rtps/tests/dialect_no_drift.rs`; `runtime_config_layering.rs:304`, `:435` |

| Wert | Dispose-Form | SPDP-Antwort | gemessene Version |
|---|---|---|---|
| `maximally-interoperable` | `K=1`, Key im Payload **und** `PID_KEY_HASH` | Locators + Absender | – |
| `cyclone` | `K=1`, Key im Payload, kein `PID_KEY_HASH` | nur Locators | Cyclone DDS 11.0.1 |
| `fastdds` | `K=0`, nur `PID_KEY_HASH` | nur Locators | Fast DDS 3.6.1 |
| `opendds` | wie Standard | Locators + Absender | OpenDDS 3.34.0 |

Der Standard wird von allen drei gemessenen Implementierungen angenommen (laut Code-Kommentar `dialect.rs`, `DisposeKeyForm::KeyInPayloadWithHash`). Einen Grund, den Wert gegenüber einem der drei Peers zu ändern, nennt der Code nicht; `cyclone`/`fastdds` reproduzieren deren eigene Form. Die dritte Achse `DirectedAddressing` ist für SEDP-Writer fest auf `PrefixEveryDatagram` (`sedp/writer.rs:555–563`). Vier weitere Achsen betreffen nur User-Writer und hängen an keinem Dialektnamen: `GroupCoherentSetPid` (`writer.group_coherent_set_pid`), `HeartbeatGroupDigests` (`wire.omit_heartbeat_group_digests`), `GroupWireForm` (`writer.group_info_on_wire`) und `EndOfSetForm` (nur per API; Satzende als GAP, für InterCOM 4.3.1).

#### 6.1.4 `wire.big_endian`

| | |
|---|---|
| Umgebung | `ZERODDS_WIRE_BIG_ENDIAN` |
| Werte | `1`/`0` (in Dateien; Hilfetext nennt auch `true`/`false`, `runtime.rs:3819–3822`, `:3898–3901`) |
| Standard | `false` |
| Wirkung | Writer schreiben `*_BE`-Encapsulation und Big-Endian-Body |
| Peer | keiner verlangt es; Mess- und Interop-Schalter (`runtime.rs:2310–2313`) |
| Beleg | `crates/dcps/tests/shapes_type_idl_agreement.rs` (vier Modi); `cross-vendor-validation.md`, „Big-endian axis“ (laut Doku) |

#### 6.1.5 `xtypes.key_member_bounds`

| | |
|---|---|
| Umgebung | `ZERODDS_KEY_MEMBER_BOUNDS` |
| Werte | `enforced`, `ignored` (`crates/types/src/assignability.rs:228–252`) |
| Standard | `enforced` (Klausel) |
| Wirkung | Key-Member-Regeln aus XTypes 1.3 Tabelle 19 (Bound, Literale) beim Assignability-Check |
| Peer | `ignored` entspricht RTI 7.7.0, OpenDDS 3.35, CoreDX 6.15.3, HDDS 1.3.0 (alle vier `struct_key_*`-Paare matchen); `enforced` entspricht Cyclone 11.0.1 (verweigert drei von vier); gemessen 2026-09-13 in Selbstpaaren der OMG-Suite (laut Code-Kommentar `assignability.rs:207–227`) |
| Beleg | `crates/types/tests/table19_key_members.rs`; `docs/omg-xtypes-issues.md` §C1 |

#### 6.1.6 `xtypes.type_consistency_absent`

| | |
|---|---|
| Umgebung | `ZERODDS_TYPE_CONSISTENCY_ABSENT` |
| Werte | `disallow`, `allow` (`crates/dcps/src/runtime.rs:1368`, `AbsentTypeConsistency`) |
| Standard | `disallow` (Klausel, XTypes 1.3 §7.6.3.4.1) |
| Wirkung | ein entfernter Reader, dessen Ankündigung keine PID 0x0074 trägt (`SubscriptionBuiltinTopicData::type_consistency_announced` false), wird an DISALLOW_TYPE_COERCION gemessen, also an Äquivalenz; `allow` nimmt für ihn den Standard der Policy, ALLOW_TYPE_COERCION (`runtime.rs:15061`, `remote_reader_consistency`). Ein Reader, der DISALLOW selbst ankündigt, behält es |
| Peer | keiner gemessen; für einen Peer, der PID 0x0074 nicht schreibt und nach Zuweisbarkeit gematcht werden soll |
| Beleg | `runtime_config_layering.rs:875`; `subscription_data.rs`, Test der entfernten PID 0x0074 |

#### 6.1.7 Weitere Laufzeit-Hebel ohne Datei-Schlüssel

| Hebel | Ort | Zweck |
|---|---|---|
| `DataWriterQos.data_representation`, `DataReaderQos.data_representation` | `crates/dcps/src/qos.rs:74`, `:235` | Repräsentation pro Endpoint |
| `set_peer_rtps_dialect`, `set_peer_rtps_behaviour` | `runtime.rs:11548–11570` | RTPS-Dialekt pro Peer |
| `RuntimeConfig::ros_defaults()`, `multi_robot()` | `runtime.rs:4872–4922` | Profile |
| `delivery.mode` (`portable`, `raw-same-host`, `iceoryx`) | Einstellung, `runtime.rs:3876` | Same-Host-Pfad, `iceoryx` für Cyclone-iceoryx ([crates-bridges-services.md](crates-bridges-services.md)) |

### 6.2 `zerodds-idlc`

`tools/idlc/src/main.rs:1336–1390`. `--dialect <name>` und die Achsen-Flags `--<achse> <wert>` gelten nur für `generate` und wählen den CDR-Dialekt des erzeugten Codes (Abschnitt 6.3; `main.rs:601`, `:637`).

| Flag | Werte | Standard | Peer-Bezug |
|---|---|---|---|
| `--strict` | – | aus (`Dialect::ALL_VENDORS`) | lehnt jede Vendor-Erweiterung ab (`keylist`, `#pragma keylist`, `DCPS_*`); Portabilitätsprüfung |
| `--default-extensibility` | `final`, `appendable`, `mutable` | `appendable` | Cyclone-IDL ohne Annotation: `final` (Cyclone-Spalte `default-extensibility` = `final` in `docs/wire-axis-flags.md`) |
| `--encoding` | `xcdr1`, `xcdr2`, `pl-cdr1`, `pl-cdr2`, `delimited-cdr` | `xcdr2` | wählt die XCDR-Version des erzeugten Codecs; `xcdr2`, `delimited-cdr`, `pl-cdr2` erzeugen denselben Text (`main.rs:50–60`) |
| `--endian` / `--xcdr-endianness` | `le`/`be` bzw. `LE`/`BE` | `le` | ändert nur die Präambel; die Byte-Reihenfolge wählt der Writer (`main.rs:60–65`) |
| `--deny-warnings` | – | aus | Vendor-Warnungen als Fehler |
| `--dialect` | die Namen aus Abschnitt 6.3 | `specification` | nur `generate`: die Antworten eines Peers auf alle Wire-Achsen auf einmal, vor den Achsen-Flags (`main.rs:601`) |
| `--ros2` | – | aus | ROS-2-Dialekt: ein unqualifizierter Membertyp, der sich nur in der Groß-/Kleinschreibung vom Membernamen unterscheidet (`Pose pose;`), löst auf den Typ auf (`main.rs:431`) |

Der Parser-Dialekt (`zerodds_idl_parse::Dialect`) hat fünf boolesche Felder `rti`, `cyclone`, `fast_dds`, `open_dds`, `ros2` (`crates/idl-parse/src/dialect.rs:12–24`). `zerodds-idlc` kennt die zwei Stellungen `ALL_VENDORS` und `STRICT` (`--strict`) und setzt mit `--ros2` zusätzlich `ros2` (`tools/idlc/src/main.rs:188–198`); einzelne Vendoren lassen sich über die Kommandozeile nicht wählen.

### 6.3 CDR-Dialekt und Achsen

`zerodds_ast_layout::Dialect` (`crates/ast-layout/src/key.rs:4911`, `ALL` bei `:5028`): `specification` (Standard), `cyclone`, `rti`, `fastdds`, `opendds`, `opensplice`, `ros-fastrtps`, `ros-cyclone`, `omniorb`, `tao`, `jacorb`, `hdds`. Ein unbekannter Wert ergibt `None` statt des Standards (`Dialect::parse`).

Wo erreichbar:

| Ort | Art |
|---|---|
| `zerodds-idlc generate` (`tools/idlc/src/main.rs:601`) | `--dialect` und die Achsen-Flags; der generierte Codec und die TypeSupport-Begleiter |
| `zerodds-build` (`crates/zerodds-build/src/lib.rs:283`, `:325`) | `Config::dialect`, `Config::axis` aus einem `build.rs` |
| `wireprobe-zerodds` (`crates/wire-probe/src/main.rs`) | `--dialect` und alle Achsen-Flags unten; Bytes schreiben (`--values`) oder lesen (`--in`) |
| `tools/wireprobe/wireprobe-zerodds-gen`, `-c-gen`, `-zig-gen` | `--dialect` und Achsen-Flags; Probe-Programme |
| `interop-results --dialects` (`tools/interop-results/src/dialect_fit.rs`) | sucht pro System die Achsenbelegung, die dessen aufgezeichnete Bytes reproduziert |
| `zerodds_lang_rules::generate::source(ast, language, key: LayoutKey)` (`crates/lang-rules/src/generate.rs:679`) | programmatisch: ein Aufrufer übergibt `LayoutKey::in_dialect(...)`; `zerodds-idlc` übergibt den mit `--dialect` gewählten Schlüssel |
| `crates/corba-interop/build.rs:328` | ORB-Dialekte für die CORBA-Brücke zur Build-Zeit |

Werte der Achsen-Flags (`wireprobe-zerodds --help`, `crates/wire-probe/src/main.rs`, Konstante `USAGE`):

| Flag | Werte | Wert von `specification` |
|---|---|---|
| `--length-codes` | `compact`, `universal-nextint` (Dialekte zusätzlich `reuse-nested-delimiter`, `short-where-it-fits`, `sized-or-nextint`) | `compact` |
| `--collection-delimiters` | `specification`, `enum-is-primitive`, `never` | `specification` |
| `--wstring-length` | `specification`, `character-count`, `character-count-terminated`, `specification-wide-units`, `ros-type-support` oder Koordinaten `octets\|characters/terminated\|unterminated/2\|4` | `specification` (Oktette, ohne Terminator, 2) |
| `--wchar-framing` | `fixed`, `giop-length-prefixed` | `fixed` |
| `--wchar-width` | `2`, `4` | `2` |
| `--map-delimiter` | `either-half`, `value-only` | `either-half` |
| `--wstring-units` | `stream`, `big-endian-always`, `byte-order-mark`, `host-unswapped` | `stream` |
| `--map-length` | `sequence-equivalent`, `rule-15-omits` | siehe `map_count_omitted_when_delimited` (`key.rs`) |
| `--fixed-digits` | `declared`, `value-precision`, `value-string-digits` | `declared` |
| `--enumerator-value` | `declared`, `ordinal` | `declared` |
| `--union-framing` | `specification`, `branches-from-zero` | `specification` |
| `--short-pid-range` | `up-to-reserved`, `all-fourteen-bits` | `up-to-reserved` |
| `--map-entries` | `declaration`, `ascending-key` | `declaration` |
| `--pl-cdr1-headers` | `specification`, `always-extended`, `short-true-length` | `specification` |
| `--pid-flags` | `sentinel-must-understand`, `bare`, `impl-extension-members` | `sentinel-must-understand` |
| `--enum-holder` | `declared`, `bit-bound-dropped` | `declared` |
| `--union-labels` | `sign-extended`, `negative-unmatched-below-32` | `sign-extended` |
| `--key-holder` | `forced-final`, `as-declared` | – |
| `--key-holder-order` | `ascending-member-id`, `declaration` | – |
| `--key-holder-alignment` | `aligned`, `data-encoding`, `packed` | `aligned` |
| `--default-extensibility` | `final`, `appendable`, `mutable` | `appendable` |
| `--defaulted-discriminator-member` | `type-default`, `stream-value` | `type-default` |
| `--element-padding` | `aligned`, `packed` | `aligned` |
| `--aggregate-delimiter` | `specification`, `never` | `specification` |
| `--sequence-element-delimiter` | `never`, `before-each-non-final-element` | `never` |
| `--wstring-code-units` | `utf16`, `utf8` | `utf16` |
| `hashed-id-counter` | kein Flag; `ComposeOptions::hashed_id_advances_the_counter` beim Lowering | „advances“ (`key.rs`, `hashed_id_advances_the_counter`) |

Vendorwerte der Achsen, auf denen sich DDS-Implementierungen unterscheiden (laut Doku, `docs/wire-axis-flags.md`, erzeugt von `interop-results --dialects`; Werte in Klammern dort = nicht durch eine Aufzeichnung festgelegt, hier weggelassen und durch „·“ ersetzt; hochgestellte Ziffer = XCDR-Version):

| Achse | Cyc¹ | Cyc² | RTI¹ | RTI² | FD¹ | FD² | ODDS¹ | ODDS² |
|---|---|---|---|---|---|---|---|---|
| `collection-delimiters` | · | specification | · | never | · | enum-is-primitive | · | specification |
| `default-extensibility` | · | final | · | appendable | · | appendable | · | appendable |
| `enum-holder` | declared | declared | · | · | bit-bound-dropped | bit-bound-dropped | bit-bound-dropped | bit-bound-dropped |
| `enumerator-value` | declared | declared | declared | declared | declared | declared | ordinal | ordinal |
| `hashed-id-counter` | advances | advances | skipped | skipped | advances | advances | skipped | skipped |
| `length-codes` | · | compact | · | reuse-nested-delimiter | · | short-where-it-fits | · | sized-or-nextint |
| `map-delimiter` | · | · | · | · | · | value-only | · | · |
| `map-entries` | · | · | · | · | ascending-key | ascending-key | ascending-key | ascending-key |
| `must-understand` | declared | declared | declared | declared | omitted | omitted | · | declared-and-key |
| `pid-flags` | sentinel-must-understand | · | sentinel-must-understand | · | bare | · | impl-extension-members | · |
| `pl-cdr1-headers` | always-extended | · | specification | · | short-true-length | · | short-true-length | · |
| `short-pid-range` | · | · | up-to-reserved | · | up-to-reserved | · | all-fourteen-bits | · |
| `union-framing` | · | · | specification | specification | specification | specification | branches-from-zero | branches-from-zero |
| `union-labels` | negative-unmatched-below-32 | negative-unmatched-below-32 | sign-extended | sign-extended | sign-extended | sign-extended | sign-extended | sign-extended |
| `wchar-width` | 2 | 2 | 4 | 2 | 2 | 2 | 2 | 2 |
| `wstring-counts` | octets | octets | characters | octets | characters | characters | octets | octets |
| `wstring-terminator` | unterminated | unterminated | terminated | unterminated | unterminated | unterminated | unterminated | unterminated |
| `wstring-unit` | 2 | 2 | 4 | 2 | 2 | 2 | 2 | 2 |
| `wstring-unit-order` | host-unswapped | host-unswapped | stream | stream | stream | stream | stream | stream |

Gate: `tools/interop-results/tests/dialect_completeness.rs` prüft, dass `--dialect <name>` allein jede aufgezeichnete Zeile des Systems reproduziert; Zeilenzahlen Cyclone 1148, RTI 1090, Fast DDS 1412, OpenDDS 1148, OpenSplice 191 (161 oktettgleich, 30 nur Padding), HDDS 326 (laut Doku, `docs/wire-axes-followup.md`, „The gate“). Dieselbe Datei prüft die Leserichtung über `dialect_read_completeness.rs`. `hdds` ist wie jeder andere Dialekt **opt-in**: `--dialect hdds` und sonst nichts. Seine vier gemessenen Abweichungen von XTypes 1.3 sind gepackte Sammlungselemente (`ElementPadding::Packed`), jedes Aggregat als `@final` ohne DHEADER und ohne EMHEADER (`AggregateDelimiter::Never`), eine Vier-Oktett-Länge vor jedem Element einer `sequence`, deren Elementtyp nicht `@final` ist (`SequenceElementDelimiter::BeforeEachNonFinalElement`), und ein `wstring` in UTF-8 (`WstringCodeUnits::Utf8`). `systems.rs:144–175` hält fest, was die Spalte ist.

Folge für Anwendungen: Weicht ein Peer auf einer Achse von `specification` ab und kommt das Konstrukt im Typ vor, tauschen Anwendung und Peer andere Bytes aus. Es gibt dafür keine Laufzeit-Einstellung. Betroffene Konstrukte je Peer:

| Peer | Konstrukte mit abweichender Wire-Form |
|---|---|
| Cyclone | `wstring` (Einheiten in Host-Reihenfolge), negative Union-Labels mit Diskriminator < 32 Bit ohne `default:`, `@mutable` unter XCDR1 (immer erweiterter PL-Header), Extensibility ohne Annotation |
| RTI | `wstring`/`wchar` unter XCDR1 (Zeichen, Terminator, 4 Byte), Sequenz-Delimiter und Längencodes unter XCDR2 |
| Fast DDS | `wstring` (Zeichen), Enum mit `@bit_bound`, `map` (Reihenfolge, Delimiter), `@mutable` (PID-Flags, Must-Understand, kurze PL-Header), Sequenzen von Enums unter XCDR2 |
| OpenDDS | Enumeratoren mit `@value`, Enum mit `@bit_bound`, `map`, `@mutable` (PID-Flags, Längencodes), `@mutable union` |
| HDDS | `@appendable` und `@mutable` (als `@final` gerahmt, kein DHEADER/EMHEADER/Parameter-Header), Sammlungselemente gepackt, eine Länge vor jedem Element einer `sequence` eines nicht-`@final`-Typs, `wstring` als UTF-8, Big Endian (kein Encoder). Opt-in über `--dialect hdds` (`systems.rs:144–175`) |

### 6.4 Automatische Anpassungen ohne Schalter

| Verhalten | Auslöser | Belegstelle |
|---|---|---|
| Encapsulation-Header nach deklarierter Extensibility | jeder Writer | `crates/dcps/src/publisher.rs:3585–3603` (Anlass: OpenDDS `EncapsulationHeader::to_encoding`) |
| Key-only-ALIVE-DATA wird bestätigt und übersprungen | OpenDDS `register_instance` | `crates/rtps/src/reliable_reader.rs` (laut Doku, `cross-vendor-validation.md`) |
| PID-Flags `& 0x3FFF` beim PL_CDR1-Lesen | RTI `PID_EXTENDED` mit Must-Understand (`0x7F01`) | `crates/wire/src/reader.rs` (Maske `0x3FFF`); Tests `crates/wire/tests/vendor_reader.rs`, `crates/wire-probe/tests/pid_flag_positions.rs`. `cross-vendor-validation.md` nennt noch das entfernte `crates/cdr/src/xcdr1.rs` |
| `0x0083` und `0x800f` nebeneinander | jede Antwort mit Related Sample Identity | `runtime.rs:17939–17947` |
| INFO_DST vor SEDP und `VolatileSecure` | immer | `sedp/writer.rs:565–597`, `security/volatile_secure.rs:91–94` |
| Reader dekodiert XCDR1/XCDR2 und LE/BE nach Header | immer | `crates/dcps/src/subscriber.rs:56–79` |

## 7 Gegenüber, Einstellungen, Beleg

| Gegenüber | nötige Einstellungen | Beleg |
|---|---|---|
| Cyclone DDS 11.0.1 | keine für Standardfälle. `wire.data_representation_offer: [XCDR2]` nur gegen Reader mit XCDR2-Liste (`-x 2`). IDL ohne Annotation mit `zerodds-idlc --default-extensibility final`. Konstrukte aus 6.3 meiden. Cyclone-Seite mit `Discovery/Tag`: denselben Wert in `discovery.domain_tag` | `crates/discovery/tests/cyclone_live_sedp.rs`; `crates/dcps/tests/cyclone_live_wlp.rs`; `crates/rtps/tests/cyclone_compliance.rs`; CI-Gate `tests/interop/ci/`; `tools/dcps-shapes-interop.sh` A/B, M-Fälle, N1 |
| Fast DDS 3.6.1 | keine für Standardfälle. Security nur mit RSA-Identitätszertifikaten (laut Doku). Konstrukte aus 6.3 meiden | `crates/discovery/tests/fastdds_live_spdp.rs`; `crates/dcps/tests/fastdds_live_pub.rs`, `fastdds_live_sub.rs`, `fastdds_qos_matrix.rs`; CI-Gate `tests/interop/ci/`; `tools/dcps-shapes-interop.sh` C/D |
| RTI Connext 7.7.0 | ZeroDDS: keine. `xtypes.key_member_bounds: ignored`, wenn RTI-Writer breitere Keys senden sollen. RTI-Seite: `transport_builtin` = `UDPv4` auf demselben Host; Reader-Repräsentation um XCDR2 erweitern, wenn ZeroDDS XCDR2 schreibt; `NDDSHOME` gesetzt | `tools/dcps-shapes-interop.sh` E/F; `tests/interop/ci/rti/USER_QOS_PROFILES.xml` (Kommentar); `crates/rtps/src/publication_data.rs:1004–1043` (`0x8002`); `crates/discovery/src/type_lookup/server.rs:116–286` |
| OpenDDS 3.34.0 | ZeroDDS: `wire.data_representation_offer: [XCDR2]` gegen XCDR2-only-Reader; `wire.rtps_dialect` auf Standard oder `opendds` lassen (SPDP-Antwort an Absender). OpenDDS-Seite: `DCPSDefaultDiscovery` RTPS, `transport_type=rtps_udp`, `use_xtypes=no` gegen ZeroDDS-Writer ohne TypeObject. IDL: `DCPS_DATA_KEY` wird gelesen; ein Key-Pfad (`p.x`) macht `p` und `x` im inneren Typ zu Keys, ein Array-Element (`a[0]`) wird abgelehnt | `tools/dcps-shapes-interop.sh` G/H, M15/M16; `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`; `crates/wire-probe/tests/opendds_mutable_framing.rs`; `crates/rtps/src/dialect.rs` (`SpdpResponseTargets`, 0/5 gegen 5/5) |
| OpenSplice CE 6.9 | nur XCDR1, beide Seiten mit `-x 1` gemessen. IDL: `#pragma keylist` wird gelesen wie von `idlpp` (im Modul, Komma, Pfad `a.b`); keine Struct-Vererbung, keine Annotationen auf OpenSplice-Seite. OpenSplice sendet `PID_KEY_HASH` ohne Padding zwischen Keys unterschiedlicher Breite (gemessen); `wire.codec_dialect: opensplice` lässt ZeroDDS ihn ebenso rechnen, Standard bleibt §7.6.8 | `tools/opensplice/README.md`; `tools/interop-results/src/systems.rs`; `crates/idl-lower/tests/keys_and_inheritance.rs` |
| HDDS (hddsgen 1.2.0) | nur Little Endian; die reflektiven Laufzeit-Codecs erreichen ihn opt-in über `wire.codec_dialect: hdds`. `zerodds-idlc generate --dialect hdds` erzeugt den Rust-Codec und alle zwanzig generierten Codecs, die alle drei Achsen tragen; jede Laufzeit prüft jede Achse gegen eine aufgezeichnete HDDS-Datei. Abgelehnt wird nur noch ein Aufruf ohne Ziel, mit `element-padding`, `sequence-element-delimiter` und `wstring-code-units` benannt (`zerodds_lang_rules::encode::axes_the_rust_codec_carries` / `axes_the_codecs_carry_for` / `axes_the_codecs_carry`, `crates/lang-rules/tests/hdds_axes_are_refused.rs`, `crates/lang-rules/tests/hdds_rust_codec.rs`; der erzeugte Rust-Code gegen die Bytes von HDDS: `tools/idlc/tests/dialect_byte_equality.rs::generated_rust_under_hdds_writes_and_reads_what_hdds_wrote`). RTPS-Laufzeitbetrieb nicht gemessen | `tools/interop-results/src/systems.rs:144–175`; `crates/wire-probe/tests/hdds_fill.rs`; `crates/wire-probe/tests/hdds_dialect.rs`; `docs/edge-case-map.md` (laut Doku: 5 von 17 HDDS-Zellen oktettgleich mit ZeroDDS) |
| dust_dds 0.16.1 | `xtypes.key_member_bounds: ignored`: dust_dds kündigt den Key `color` von ShapeType unbeschränkt an, wo die IDL der Suite `string<128>` hat; nach XTypes 1.3 Tabelle 19 lehnt ein ZeroDDS-Reader den Writer sonst ab | `tools/omg-shape-main/src/main.rs` (`suite_settings`); `tools/omg-shape-main/README.md` |
| ROS 2 (`rmw_cyclonedds_cpp`, `rmw_fastrtps_cpp`) | über `rmw_zerodds_cpp` keine; `RuntimeConfig::ros_defaults` | [ros2.md](ros2.md); `crates/ros2-rmw/tests/cross_vendor.rs`; `crates/ros2-rmw/interop/run_multicast_free_xvendor.sh` |

## 8 Verfahren „Dialekt-Flags richtig setzen“

1. **Peer und Version feststellen.** `zerodds-admin discovery snapshot <domain>` zeigt die Participants mit ihrem SPDP-Inhalt (`tools/admin/src/main.rs:394`). Die Vendor-ID steht in `PID_VENDORID`. Versionen, die nicht in `tools/interop-results/src/systems.rs` stehen, sind nicht gemessen.
2. **Vendor-Seite auf Standard-Wire prüfen.** RTPS-Discovery, UDP statt Shared Memory, Standard-Ports, gleiche Domain-ID, keine Domain-Tags. Diese Punkte sehen auf ZeroDDS-Seite aus wie ein Interop-Fehler (`tools/dcps-shapes-interop.sh`, Kopfkommentar „Three configuration traps“).
3. **Typ vergleichen, bevor ein Flag gesetzt wird.** Beide Seiten aus derselben IDL generieren. Extensibility annotieren oder `--default-extensibility` wie der Peer setzen. Konstrukte aus der Tabelle in 6.3 für diesen Peer entfernen oder ersetzen. Mit `wireprobe-zerodds --dialect <peer>` lassen sich die Bytes, die der Peer erwartet, gegen `--dialect specification` vergleichen.
4. **Datenrepräsentation setzen.** Reader-Listen der Gegenseite ermitteln. Nimmt ein fremder Reader nur XCDR2 an: `wire.data_representation_offer: [XCDR2]`, sonst Standard lassen. Schreibt ein Writer eines Peers XCDR2 und ein anderer Reader verlangt XCDR1, diesen Writer pro Endpoint über `DataWriterQos.data_representation` trennen; ein Writer bedient nicht beide.
5. **Typprüfung abstimmen.** Wenn Endpoints trotz gleichem Typnamen nicht matchen: prüfen, ob der ZeroDDS-Endpoint ein TypeObject annonciert (C: `*_typed`), ob der Peer XTypes-Information verlangt (OpenDDS `use_xtypes`), und ob Key-Bounds abweichen (`xtypes.key_member_bounds`).
6. **RTPS-Dialekt nur mit Grund ändern.** `wire.rtps_dialect` auf dem Standard lassen, solange kein Peer die Dispose-Form oder die SPDP-Antwort ablehnt. Wird ein Wert gesetzt, in `zerodds.yaml` (Tippfehler verhindern den Start) statt in der Umgebung (Tippfehler fallen still auf den Standard zurück; Meldung nur auf dem Observability-Sink).
7. **Vendor-Sonderpfade einschalten, die ein Feature verlangt.** `wire.big_endian` nur für Messungen.
8. **Prüfen mit Sample-Zählung in beiden Richtungen.** Ein Match ist kein Nachweis; Maßstab ist die Zahl dekodierter Samples (`docs/interop-harnesses.md`, „The five stages of a live proof“). Vorlage für einen Lauf: `tools/dcps-shapes-interop.sh`.
9. **Flag festhalten.** Jede vendorbezogene Einstellung als eigenes Drop-in (z. B. `zerodds.d/50-opendds.yaml`) ablegen, damit sie mit dem Peer wieder entfernt werden kann; `Config::origin_of` bzw. die Quellenangabe der Laufzeit nennt die Datei ([config.md](config.md)).

## 9 Offene und widersprüchliche Punkte

| Punkt | Stand |
|---|---|
| Anwendungsseitige CDR-Dialekte | Die zwölf Layout-Dialekte (`zerodds_ast_layout::Dialect`) sind nur in `wireprobe-zerodds` wählbar; `zerodds-idlc` und die Laufzeit-Codecs erzeugen das Spec-Layout. `docs/wire-axes-followup.md` („The dialect pass: a name that reaches the product“) beschreibt sie als Weg zum Produkt (D3). |
| `0x801c`-Anfrage an Cyclone | wirkt nicht: Cyclone liest herstellerspezifische PIDs nur mit der Tabelle der Sender-Vendor-ID (Eclipse/ADLINK, `ddsi_plist.c` 11.0.x); gemessen gegen Cyclone 11.0.1 0 von 167 DATA mit `PID_KEY_HASH` (D7). |
| GROUP-kohärente Sets gegen RTI | gemessen gegen Connext 7.7.0 (OMG `Test_CoherentSets_12`/`_21`, je 3 Läufe): `_21` OK, `_12` nicht. |
| OpenDDS-Reader-Repräsentationsliste | nur im Code-Kommentar genannt, nicht einzeln gemessen; M15/M16 zeigen nur, dass ZeroDDS den OpenDDS-Writer (XCDR2) liest. |
| OpenSplice | live `ShapeType` (OMG `dds-rtps`, `-x 1`) und die KeyHash-Spalte von `tools/keyhash-probe` (sechs Typen, `PID_KEY_HASH` vom Draht): ohne Padding zwischen Keys unterschiedlicher Breite, wie `KEYHASH_HAS_PADDING 0` im Quelltext; XTypes 1.3 §7.6.8 füllt auf. Achse `KeyHolderAlignment`, im Dialekt `opensplice` `packed` (D9). |
| KeyHash unter XCDR1 | Cyclone 11.0.1 und RTI 7.7.0 richten den Key-Holder unter XCDR1 bis 8 Byte aus (`@key short s; @key long long ll` → `0102 000000000000 0304…`), unter XCDR2 bis 4 wie §7.6.8; gemessen mit `tools/keyhash-probe`. RTI sendet ihn in `PID_KEY_HASH`. Die Dialekte `rti` und `cyclone` bilden das unter XCDR1 für generierten Code und `wireprobe-zerodds` ab (`data-encoding`); die Laufzeit rechnet den KeyHash stets über den kanonischen XCDR2-Holder, wie schon bei Reihenfolge und Verschachtelung. |
| HDDS | Serialisierung mit Crate `hdds` 1.1.2/`hddsgen` 1.2.0, dds-xtypes-Suite mit `hdds-1.3.0_test_main`, dds-rtps-Suite mit `hdds-1.4.8_shape_main`. Die vier Abweichungen des generierten Serialisierers von XTypes 1.3 trägt `Dialect::Hdds`, opt-in; der Dialekt reproduziert alle 326 aufgezeichneten Zeilen. |

Behoben oder gemessen: `idlc --dialect` im Kommentar (`e86e0a5739b`), die Vendor-ID-Ausnahmen im `dialect`-Modulkopf, die Standard-Writer-Repräsentation in `cross-vendor-validation.md`, `PID_DOMAIN_TAG` (gebaut, gegen Cyclone gemessen), `wire.data_representation_match: strict` (OMG `Test_DataRepresentation_0..3` gegen Connext, Cyclone, Fast DDS, beide Richtungen, 24/24), die Security-Aussage von `docs/interop-harnesses.md` (bezieht sich auf die Stufen der eigenen Kette), boolesche Laufzeitwerte (`true`/`false` werden zu `1`/`0` normalisiert, Test `every_boolean_setting_reads_true_and_false_as_the_digits`).
