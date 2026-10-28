# Migration zu ZeroDDS

Diese Seite beschreibt, wie ein bestehendes System von Cyclone DDS, Eclipse OpenSplice, RTI Connext, eProsima Fast DDS oder OpenDDS auf ZeroDDS umgestellt wird: welche ZeroDDS-API an die Stelle der Vendor-API tritt, ob die IDL unverändert durch `zerodds-idlc` geht, was aus QoS-XML und Vendor-Konfiguration wird, wie ZeroDDS neben dem alten Vendor auf dem Wire mitläuft und welche Unterschiede bleiben. Grundlage ist der Code auf `main`. Aussagen, die nur aus einer Doku-Datei oder einem Code-Kommentar stammen, sind mit „(laut Doku)“ bzw. „(laut Code-Kommentar)“ markiert. Aussagen über die Vendor-Seite, die im Repository nicht belegt sind, tragen „(Vendor-Seite, nicht im Repo geprüft)“. Die Wire-Einstellungen im Detail stehen in [interop-and-dialects.md](interop-and-dialects.md).

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## 1 Ergebnis in Kürze

- **Eine Vendor-API-Kompatibilitätsschicht gibt es nicht.** Im Code existiert kein `dds_create_participant` (Cyclone-C), kein `DDS_DomainParticipantFactory_*` (OpenSplice-SAC, RTI-C), kein `DDSDomainParticipantFactory` (RTI-Legacy-C++), kein `eprosima::fastdds::dds::*` und kein `TheParticipantFactory` (OpenDDS). Treffer auf diese Namen liegen ausschließlich in Vendor-Testprogrammen unter `tools/wire-capture/`, `tools/keyhash-probe/`, `tools/optional-bits/` und in Test-Harnesses.
- **Vorhanden sind die OMG-PSMs:** DDS-PSM-Cxx 1.0 unter `#include <dds/dds.hpp>` (`crates/cpp/include/dds/`), die OMG DDS Java PSM `org.omg.dds.*` (`crates/java-omgdds/`) und eine C-API mit DCPS-Struktur und eigenen Namen `zerodds_*` (`crates/zerodds-c-api/include/zerodds.h`). Wer heute die ISO-C++-API (Cyclone-CXX, OpenSplice-ISOCPP2, RTI Modern C++) oder die OMG-Java-API benutzt, hat den kürzesten Weg.
- **IDL:** `zerodds-idlc` liest standardmäßig unter `Dialect::ALL_VENDORS` (`crates/idl-parse/src/dialect.rs:40`). `#pragma keylist` (Cyclone/OpenSplice) und `keylist` (RTI) machen Member zu Keys. `#pragma DCPS_DATA_KEY` (OpenDDS) macht den genannten Member zum Key; ein Pfad in einen Member (`p.x`) wird abgewiesen (siehe 2.2).
- **QoS-XML:** Gelesen wird nur OMG DDS-XML 1.0 (`<dds>` oder `<qos_library>` als Wurzel, `crates/xml/src/parser.rs:81`). RTI-`USER_QOS_PROFILES.xml` hat diese Form; ein RTI-Element, das der Leser nicht abbildet, beendet das Laden mit einem Fehler, der es nennt (`UnknownElement`, `crates/xml/src/qos_parser.rs`). Fast-DDS-`<profiles>` wird abgewiesen. Ein Lader existiert nur in Rust (`zerodds_xml::QosProfileRegistry`) und in Werkzeugen, nicht in C, C++, Java oder Python.
- **Konfiguration:** Kein Vendor-Konfigurationsformat wird gelesen (`CYCLONEDDS_URI`, `OSPL_URI`, `NDDS_QOS_PROFILES`, Fast-DDS-Profile, OpenDDS-`.ini`). ZeroDDS liest `zerodds.yaml` plus Drop-ins, `ZERODDS_*`-Umgebungsvariablen und `--set` (`crates/dcps/src/runtime.rs:3034`, Tabelle `SETTINGS`).
- **Mischbetrieb:** ZeroDDS nutzt die Standard-Ports und die Standard-Multicast-Gruppe aus DDSI-RTPS 2.5 §9.6.1.4.1 (`crates/rtps/src/wire_types.rs:1015–1154`). Live gemessen sind Cyclone, Fast DDS, OpenDDS, RTI und OpenSplice CE 6.9 (4.5).

| Vendor | API-Weg in ZeroDDS | IDL unverändert | QoS-XML lesbar | Vendor-Konfig lesbar | Live gemessen |
|---|---|---|---|---|---|
| Cyclone DDS | C: neu auf `zerodds_*`; C++: `<dds/dds.hpp>` | ja, `--default-extensibility final` beachten | DDS-XML-Form ja | nein | ja (Tests `crates/*/tests/cyclone_*`) |
| Eclipse OpenSplice | C (SAC): neu; C++ (ISOCPP2): `<dds/dds.hpp>`; Java 5: `org.omg.dds` | `#pragma keylist` ja | nicht geprüft | nein | ja (OMG `dds-rtps`, 4.5) |
| RTI Connext | C: neu; Modern C++: `<dds/dds.hpp>`; Java/C#: neu | `keylist`, `//@key` ja; `@RTI_*` ignoriert | ja, lax | nein | ja (`tools/dcps-shapes-interop.sh`, Fälle E/F) |
| Fast DDS | C++: neu (keine eProsima-API) | ja | nein (`<profiles>`) | nein | ja (Tests `crates/*/tests/fastdds_*`) |
| OpenDDS | C++: neu (keine TAO-IDL-Mapping-API) | ja, `DCPS_DATA_KEY` auf Member ohne Pfad | nicht zutreffend | nein | ja (Fälle G/H) |

## 2 Gemeinsame Grundlagen

### 2.1 ZeroDDS-Bindings, die an die Stelle einer Vendor-API treten

| Sprache | Crate | Einstieg | API-Form | Belegstelle |
|---|---|---|---|---|
| C | `crates/zerodds-c-api` | `#include <zerodds.h>`, `-lzerodds` | DCPS-Struktur mit eigenen Präfixen: `zerodds_dpf_*`, `zerodds_dp_*`, `zerodds_pub_*`, `zerodds_sub_*`, `zerodds_topic_*`, `zerodds_dw_*`, `zerodds_dr_*`; dazu die flache Runtime-API `zerodds_runtime_*`, `zerodds_writer_*`, `zerodds_reader_*` | `include/zerodds.h:4970` (`zerodds_dpf_get_instance`), `:5350` (`zerodds_dp_create_topic`), `:5826` (`zerodds_pub_create_datawriter`), `:5947` (`zerodds_dw_write`) |
| C++ | `crates/cpp` | `#include <dds/dds.hpp>` | DDS-PSM-Cxx 1.0 (`dds::domain`, `dds::topic`, `dds::pub`, `dds::sub`, `dds::core`) über der C-API | `include/dds/dds.hpp:1–76` |
| C++ (kurz) | `crates/cpp` | `#include <zerodds/dds.hpp>` | `zerodds::Runtime`, `Writer`, `Reader`, `TypedWriter<T>` | `include/zerodds/dds.hpp:1–36` |
| Java | `crates/java-omgdds` | `org.omg.dds.core.ServiceEnvironment.createInstance(...)` | OMG DDS Java PSM; eigener reiner Java-RTPS-Stack `org.zerodds.rtps`, keine JNI | `java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` |
| Python | `crates/py` | `import zerodds` | `zerodds.DomainParticipantFactory.instance()` … | `crates/py/README.md` |
| C# | `crates/cs` | P/Invoke gegen `zerodds` | eigene API | `crates/cs/` |
| TypeScript | `crates/ts-node` | `@zerodds/node` | an DDS-PSM-Cxx angelehnt (laut README) | `crates/ts-node/src/native.ts:154–171` |
| Rust | `crates/dcps`, `crates/rs` | `zerodds_dcps::*` | DCPS-Entitäten + `DcpsRuntime` | [crates-core.md](crates-core.md) |

Eigenschaften der C-API, die für jede Portierung gelten:

- Samples gehen als CDR-Bytes über die Grenze (`zerodds_dw_write(dw, payload, len, handle)`, `zerodds.h:5947`). Den Encoder liefert der generierte Code der Zielsprache (`zerodds-idlc generate --lang c`), nicht die Bibliothek. Cyclone-C arbeitet dagegen mit einem `dds_topic_descriptor_t`, das die Bibliothek selbst serialisiert (Vendor-Seite, nicht im Repo geprüft).
- Die plain `*_create`-Funktionen annoncieren `TypeIdentifier::None`; nur `zerodds_writer_create_typed`/`zerodds_reader_create_typed` bzw. `zerodds_pub_create_datawriter_typed` übergeben ein serialisiertes COMPLETE-`TypeObject` (`crates/zerodds-c-api/README.md`, Abschnitt „Type model“). Ohne TypeObject matcht ein Peer nur über Topic- und Typnamen. OpenDDS verlangt dafür `use_xtypes=no` (Kommentar in `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`).
- Jede `*_create`-Ressource wird mit dem passenden `*_destroy`/`*_delete` freigegeben; `zerodds_buffer_free` gibt Bytes aus `zerodds_reader_take` frei (Kopfkommentar `zerodds.h:1–19`).
- Konfigurationsgetriebener Start: `zerodds_runtime_create_from_config(domain_id, config_path, sets, n_sets)` (`zerodds.h:2188`) liest eine `zerodds.yaml` und `--set`-artige Überschreibungen.

Java-Umstellung: Die Implementierungsklasse wird über die Systemeigenschaft `org.omg.dds.serviceClassName` oder das Argument von `ServiceEnvironment.createInstance` gewählt; die ZeroDDS-Klasse ist `org.zerodds.ServiceEnvironmentImpl` (`ServiceEnvironment.java`, Klassenkommentar und Konstante `IMPLEMENTATION_CLASS_NAME_PROPERTY`). Code, der nur gegen `org.omg.dds.*` geschrieben ist, bleibt bei einem Wechsel dieser Eigenschaft unverändert (laut Code-Kommentar; ein Lauf mit Vendor-Anwendungscode ist im Repo nicht belegt).

C++-Umstellung: `crates/cpp/include/dds/` enthält `dds.hpp`, `core/*`, `domain/DomainParticipant.hpp`, `topic/{Topic,TopicTraits,ContentFilteredTopic,BuiltinTopic}.hpp`, `pub/{Publisher,DataWriterListener}.hpp`, `sub/{Subscriber,DataReaderListener}.hpp`, `sub/cond/ReadCondition.hpp`, `core/cond/Condition.hpp` (mit `WaitSet`). Es gibt keine eigenen Header `dds/pub/DataWriter.hpp`, `dds/sub/DataReader.hpp` und keinen `QosProvider`. Code, der diese Header direkt einbindet oder `dds::core::QosProvider` benutzt, muss auf `<dds/dds.hpp>` umgestellt bzw. ohne QosProvider geschrieben werden.

### 2.2 IDL

Aufruf: `zerodds-idlc check FILE…` prüft, `zerodds-idlc generate --lang <sprache> --out DIR FILE…` erzeugt Code. Gültige `--lang`-Werte sind die Namen aus `Language::as_str` (`crates/lang-cst/src/common.rs:102`): `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin`. Weitere Schalter: `--strict`, `--deny-warnings`, `--default-extensibility`, `--encoding`, `--endian` (`tools/idlc/src/main.rs:1336–1390`).

Vendor-Konstrukte, wie der Code sie behandelt:

| Konstrukt | Vendor | Verhalten | Belegstelle |
|---|---|---|---|
| `#pragma keylist T a b` | Cyclone, OpenSplice | Member werden Keys (`is_key = true`); Warnung `VendorExtension`, unter `--strict` Fehler | `crates/idl-lower/src/lower.rs:668–687`, `crates/idl-lower/src/complete.rs:460–627`; Test `a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist` (`crates/idl-lower/tests/dialects.rs:710`) |
| `keylist T (a, b);` | RTI | wie oben, Vendor-Flag `Rti` | `crates/idl-parse/src/decl.rs:779–809`; Test `an_rti_keylist_reaches_the_vendor_facet_and_the_members` (`dialects.rs:370`) |
| `//@key` | RTI (alt), OpenSplice | gleichwertig zu `@key` (XTypes 1.3 §7.3.1.2.5) | `crates/idl-lower/src/annotation.rs:214–220` |
| `#pragma DCPS_DATA_TYPE "T"` | OpenDDS | markiert den Typ im OpenDDS-Facet | `lower.rs:689–712`, `complete.rs:309–320` |
| `#pragma DCPS_DATA_KEY T a` | OpenDDS | Member wird Key (`is_key = true`), Facet `dcps_data_keys` behält die Schreibweise; ein unbekannter Name wird gemeldet, ein Pfad (`p.x`, `a[0]`) abgewiesen | `crates/idl-lower/src/complete.rs` (`apply_open_dds_pragmas`); Tests `an_opendds_data_key_makes_the_member_a_key`, `an_opendds_data_key_the_type_cannot_take_is_reported` (`dialects.rs`) |
| `@RTI_…`, `@allowed_data_representation(…)` | RTI | unbekannte Annotation, ignoriert (IDL 4.2 §7.4.15.4.2), Argumente werden nicht aufgelöst | `annotation.rs:254–301` |
| `#pragma prefix/version/ID` | CORBA-Erbe | ausgewertet (Repository-IDs) | `lower.rs:645–667` |
| `#pragma dds_xtopics` | XTypes | ausgewertet und aufbewahrt | `lower.rs:775–780` |
| jedes andere `#pragma` (z. B. OpenSplice `cats`) | – | aufbewahrt, Warnung | `lower.rs:781–813` |

Warnungen brechen `zerodds-idlc` nicht ab; `--deny-warnings` macht sie zum Fehler (`tools/idlc/src/main.rs:370`, `report.rs:44`).

Extensibility ohne Annotation: `zerodds-idlc` nimmt standardmäßig `@appendable` an (Hilfetext `main.rs:1349`). Gemessen verwenden Cyclone unter XCDR2 `final`, RTI/Fast DDS/OpenDDS unter XCDR2 `appendable` (`docs/wire-axis-flags.md`, Zeile `default-extensibility`; Ergebnis der Auswertung `tools/interop-results/src/dialect_fit.rs`). Für IDL, die mit Cyclone- oder OpenSplice-Werkzeugen ohne Extensibility-Annotation geschrieben wurde: `--default-extensibility final` setzen oder die Typen ausdrücklich annotieren.

Vendor-Fixtures, die der Parser liest: `crates/idl-parse/tests/fixtures/{cyclonedds,fastdds,rti,omg}/` (Test `crates/idl-parse/tests/parse.rs`). OpenDDS- und OpenSplice-Fixtures gibt es dort nicht.

### 2.3 QoS-XML

- Parser: `crates/xml` (DDS-XML 1.0, `roxmltree`). Wurzel `<dds>` oder `<qos_library>`, sonst Fehler `expected <dds> or <qos_library> root` (`crates/xml/src/parser.rs:81–99`).
- Profile: `<qos_library name>` → `<qos_profile name base_name topic_filter>`; `base_name`-Vererbung mit Zyklenerkennung (`crates/xml/src/inheritance.rs`, `crates/xml/src/errors.rs:39`); `topic_filter` als Attribut und als Kind-Element (`qos_parser.rs:283`, `:315–351`).
- Unbekannte Elemente in einem Profil und in Entity-QoS sind ein Fehler, der das Element nennt (`XmlError::UnknownElement`); ebenso Policies, die kein Feld des Containers sind (DDS 1.4 §2.2.3), und ein doppeltes Feld. Aufzählungen nehmen nur die IDL-Literale (`RELIABLE_RELIABILITY_QOS`, nicht `RELIABLE`). Boolesche Werte werden strikt als `true`/`false`/`1`/`0` gelesen (`parse_bool_strict`). `<partition>` ist `<name><element>…</element></name>` (§7.2.4.1); Oktett-Policies sind `<value>` (Kommaliste) oder `<valueB64>` (Base64) (§7.2.4.2).
- Anwendung in Rust: `zerodds_xml::QosProfileRegistry::from_file(path)?.writer_qos("Lib::Profil")?` liefert `zerodds_qos::WriterQos`; `DataWriterQos::from(...)` übernimmt alle gemeinsamen Policies, `data_representation` bleibt auf dem Standard (`crates/xml/src/registry.rs:1–147`, `crates/dcps/src/qos.rs:448–472`). `crates/dcps` hängt nicht von `zerodds-xml` ab; die Anwendung bindet beide Crates ein.
- Werkzeuge: `zerodds-admin [--skip-unknown] [--vendor-forms] qos validate <file.xml>`, `zerodds-admin qos check <file.xml>` (RxO-Prüfung), `zerodds-admin config inspect <file.xml>` (`tools/admin/src/main.rs:12–14`); `zerodds-xmlc validate|types|render FILE…` (`tools/xmlc/README.md`).
- Kein XML-Lader in C-API, C++-PSM, Java, Python, C#. Die Java-Klasse `org.omg.dds.core.policy.QosProfile` ist ein unveränderliches QoS-Bündel ohne XML-Anbindung.

### 2.4 Konfiguration

ZeroDDS liest keine Vendor-Konfiguration. Die Laufzeit liest (niedrigste Schicht zuerst): eingebaute Standardwerte, `ZERODDS_*`-Umgebungsvariablen, `zerodds.yaml` plus `zerodds.d/` (Linux `/etc/zerodds`, macOS `/usr/local/etc/zerodds`, Windows `%PROGRAMDATA%\ZeroDDS`, zusätzlich `/run/zerodds/zerodds.d` auf Linux), `--config`/`--set` (`crates/dcps/src/runtime.rs:3013–3024`, `:3233–3247`). Eine ungültige Angabe in einer Datei verhindert den Start mit Fundstelle (`runtime.rs:3521–3579`, Test `crates/dcps/tests/runtime_config_layering.rs:433–445`). Alle Schlüssel: [config.md](config.md).

Schlüssel, die bei einer Migration am häufigsten gebraucht werden:

| Zweck | `zerodds.yaml` | Umgebung | Standard |
|---|---|---|---|
| Unicast-Peers (ohne Multicast) | `discovery.peers` (Liste `ip` oder `ip:port`) | `ZERODDS_PEERS` | leer |
| Multicast abschalten | `discovery.no_multicast` | `ZERODDS_NO_MULTICAST` | `false` |
| SPDP-Multicast-Gruppe | `discovery.spdp_multicast_group` | `ZERODDS_SPDP_MC_GROUP` | `239.255.0.1` |
| Participant-Indizes pro Peer ohne Port | `discovery.max_peer_participants` (1..120) | `ZERODDS_MAX_PEER_PARTICIPANTS` | 10 (`runtime.rs:5121`) |
| Netzwerkschnittstelle festlegen | `transport.interface` (IPv4) | `ZERODDS_INTERFACE` | automatisch |
| SPDP-Periode | `discovery.spdp_period_ms` | `ZERODDS_SPDP_PERIOD_MS` | 30000 (`runtime.rs:166`); bei einer Lease unter 90 s ein Drittel der Lease, mindestens 100 ms (`:5082`) |
| Participant-Lease | `discovery.participant_lease_duration_ms` | `ZERODDS_PARTICIPANT_LEASE_MS` | siehe [config.md](config.md) |
| Maximale Nachrichtengröße | `transport.max_message_size` | `ZERODDS_MAX_MESSAGE_SIZE` | siehe [config.md](config.md) |
| Datenrepräsentation des Writers | `wire.data_representation_offer` | `ZERODDS_DATA_REPR_OFFER` | `XCDR1` |
| RTPS-Dialekt | `wire.rtps_dialect` | `ZERODDS_RTPS_DIALECT` | `maximally-interoperable` |
| DDS-Security (SROS2-Layout) | `security.enclave_dir` | `ZERODDS_SECURITY_DIR` | leer |

Die Domain-ID wird beim Anlegen des Participants übergeben. Die Parameter der Port-Formel sind die Einstellungen `ports.*` mit den Standardwerten `PB=7400`, `DG=250`, `PG=2`, `d0=0`, `d1=10`, `d2=1`, `d3=11` (`crates/rtps/src/wire_types.rs:1015–1154`, gelesen in `crates/dcps/src/runtime.rs:4450–4467`). Setzt eine Vendor-Einstellung abweichende Ports, brauchen beide Seiten dieselben Werte.

### 2.5 Mischbetrieb, Schema für alle Vendoren

1. **Vendor-Seite auf Standard-Wire bringen.** RTPS-Discovery statt proprietärer Discovery, UDPv4-Transport neben oder statt Shared Memory, Standard-Ports, gleiche Domain-ID. Vendor-spezifische Punkte stehen in den Kapiteln 3 bis 7.
2. **Gemeinsame IDL festlegen.** Beide Seiten generieren aus derselben Datei. Extensibility ausdrücklich annotieren oder `--default-extensibility` passend setzen. Für Konstrukte, deren Wire-Form zwischen Vendoren abweicht, mit `zerodds-idlc generate --dialect <peer>` generieren oder sie meiden (Tabelle in [interop-and-dialects.md](interop-and-dialects.md), Abschnitt 6.3).
3. **Konfiguration übertragen.** Die Vendor-Konfiguration in `zerodds.yaml` nachbilden (Tabellen je Vendor), QoS-Profile als DDS-XML beibehalten oder in Code übernehmen.
4. **Einen lesenden ZeroDDS-Prozess dazustellen.** `zerodds-spy` (`tools/spy`) oder `zerodds-admin domain inspect <id>` / `discovery snapshot <id>` (`tools/admin/src/main.rs:7–11`) zeigen, ob Participants und Endpoints gefunden werden. Maßstab ist die Zahl dekodierter Samples, nicht der Match (`docs/interop-harnesses.md`, Abschnitt „The five stages of a live proof“).
5. **Einen Subscriber ersetzen**, danach einen Publisher, jeweils mit gleicher Topic- und Typbezeichnung und gleicher QoS. `wire.data_representation_offer` an die Reader der Gegenseite anpassen.
6. **Prozess für Prozess umstellen.** Nach jedem Schritt beide Richtungen prüfen.
7. **Vendor-Installation entfernen**, wenn kein Vendor-Prozess mehr läuft. Vendor-spezifische Einstellungen in `zerodds.yaml` (z. B. `wire.data_representation_offer: [XCDR2]` nur für einen bestimmten Peer) danach zurücknehmen.

## 3 Cyclone DDS

### 3.1 API

- Cyclone-C (`dds_*` mit `dds_entity_t`-Handles) hat kein Gegenstück gleichen Namens. Zuordnung auf die C-API mit DCPS-Struktur:

| Cyclone-C | ZeroDDS-C |
|---|---|
| `dds_create_participant(domain, qos, listener)` | `zerodds_dpf_create_participant(zerodds_dpf_get_instance(), domain, qos)` |
| `dds_create_topic(pp, &desc, name, qos, listener)` | `zerodds_dp_create_topic(dp, name, type_name, qos)` bzw. `zerodds_topic_create_typed` |
| `dds_create_publisher` / `dds_create_subscriber` | `zerodds_dp_create_publisher` / `zerodds_dp_create_subscriber` |
| `dds_create_writer` / `dds_create_reader` | `zerodds_pub_create_datawriter[_typed]` / `zerodds_sub_create_datareader[_typed]` |
| `dds_write(wr, &sample)` | `zerodds_dw_write(dw, bytes, len, handle)` mit CDR aus generiertem Code |
| `dds_take` / `dds_read` | `zerodds_dr_take` / `zerodds_dr_read`, `zerodds_dr_return_loan` |
| `dds_wait_for_acks` | `zerodds_dw_wait_for_acknowledgments` |
| `dds_delete(e)` | `zerodds_dpf_delete_participant`, `zerodds_pub_delete_datawriter` usw. |

- Cyclone-CXX (`dds::domain::DomainParticipant` usw.) → `#include <dds/dds.hpp>` aus `crates/cpp`. Header-Layout siehe 2.1.
- Shared Memory mit Cyclone auf demselben Host: `crates/iceoryx-cyclone` und `delivery.mode` (`portable`, `raw-same-host`, `iceoryx`; `runtime.rs:3876`). Details in [crates-bridges-services.md](crates-bridges-services.md).
- ROS 2: `RMW_IMPLEMENTATION=rmw_zerodds_cpp` statt `rmw_cyclonedds_cpp` ([ros2.md](ros2.md)).

### 3.2 IDL

Unverändert lesbar, `#pragma keylist` wird zu Keys (2.2). Cyclone-idlc nimmt ohne Annotation `final` an (Messung `docs/wire-axis-flags.md`); ZeroDDS nimmt `appendable` an. Deshalb `zerodds-idlc generate --default-extensibility final …` oder ausdrückliche `@final`. Unter XCDR1 sind `final` und `appendable` auf dem Wire gleich; das TypeObject unterscheidet sie weiterhin.

### 3.3 QoS

Cyclone-QoS in DDS-XML-Form wird gelesen; Test `all_four_libraries_roundtrip` (`crates/xml/tests/dds_xml_full.rs:52`).

### 3.4 Konfiguration (`CYCLONEDDS_URI`)

Die Cyclone-Schlüssel links stammen aus `tests/interop/cyclonedds.xml` bzw. der Cyclone-Doku (Vendor-Seite, nicht im Repo geprüft, sofern nicht in dieser Datei).

| Cyclone | `zerodds.yaml` |
|---|---|
| `General/NetworkInterfaceAddress`, `General/Interfaces/NetworkInterface` | `transport.interface` (eine IPv4-Adresse) bzw. `transport.interface_bindings` (nur Datei, `runtime.rs:4089`) |
| `General/AllowMulticast=false` | `discovery.no_multicast: true` |
| `General/MaxMessageSize` | `transport.max_message_size` |
| `Discovery/Peers/Peer@address` | `discovery.peers` |
| `Discovery/MaxAutoParticipantIndex` | `discovery.max_peer_participants` (Kommentar `runtime.rs:5114` nennt die Entsprechung) |
| `Discovery/SPDPMulticastAddress` | `discovery.spdp_multicast_group` |
| `Discovery/SPDPInterval` | `discovery.spdp_period_ms` |
| `Discovery/ParticipantIndex` | kein Schlüssel; ZeroDDS belegt den ersten freien Index 0..119 (`runtime.rs:9148–9165`) |
| `Discovery/Tag` | `discovery.domain_tag` (`ZERODDS_DOMAIN_TAG`), `PID_DOMAIN_TAG` 0x4014 |
| `Discovery/Ports/*` | `ports.port_base`, `ports.domain_gain`, `ports.participant_gain`, `ports.d0` bis `ports.d3` (2.4) |
| `Tracing/*` | `diagnostics.*`, `diagnostics.observability` |

### 3.5 Mischbetrieb

- Standard-Discovery ohne Zusatzkonfiguration; Beleg `crates/discovery/tests/cyclone_live_sedp.rs`, `crates/dcps/tests/cyclone_live_wlp.rs`, CI-Gate `tests/interop/ci/` mit Cyclone und Fast DDS (`docs/interop-harnesses.md`, Tabelle „What each harness covers today“).
- Ein Cyclone-Reader, der nur XCDR2 annimmt (`shape_main -x 2`), matcht den ZeroDDS-Standard-Writer nicht; Fall `N1` in `tools/dcps-shapes-interop.sh:249–255`. Abhilfe `wire.data_representation_offer: [XCDR2]`.
- Cyclone sendet `PID_KEY_HASH` nur an Reader, die `0x801c` annoncieren (`docs/peer-defects.md`), liest diesen herstellerspezifischen PID aber nur von Eclipse- und ADLINK-Participants (`ddsi_plist.c`, 11.0.x). Von ZeroDDS (Vendor 01.240) nimmt Cyclone ihn nicht an; gemessen gegen Cyclone 11.0.1: 0 von 167 DATA mit `PID_KEY_HASH` (`docs/peer-defects.md`). ZeroDDS sendet `0x801c` nicht; fehlt `PID_KEY_HASH`, berechnet der Reader den KeyHash aus seinem Typ (`crates/dcps/src/runtime.rs:25692`).

### 3.6 Unterschiede und Grenzen

- `wstring`-Code-Units bleiben bei Cyclone in Host-Byte-Reihenfolge; negative Union-Labels unter 32 Bit wählen keinen Zweig (`docs/peer-defects.md`, zwei Cyclone-Abschnitte). ZeroDDS-Anwendungscode folgt standardmäßig der Spezifikation; `zerodds-idlc generate --dialect cyclone` erzeugt Cyclones Form, eine Laufzeit-Einstellung dafür gibt es nicht ([interop-and-dialects.md](interop-and-dialects.md), 6.3).
- `xtypes.key_member_bounds`: Cyclone 11.0.1 verweigert 3 von 4 OMG-`struct_key_*`-Paaren, wie der ZeroDDS-Standard `enforced` (`crates/types/src/assignability.rs:207–227`).

## 4 Eclipse OpenSplice

### 4.1 API

- OpenSplice-C (SAC, `DDS_DomainParticipantFactory_get_instance()` usw.): keine gleichnamige API. Die ZeroDDS-C-API hat dieselbe DCPS-Gliederung (`zerodds_dpf_get_instance`, `zerodds_dpf_create_participant`, `zerodds_dp_create_publisher`, `zerodds_pub_create_datawriter`, `zerodds_dw_write` …); Typen gehen als CDR-Bytes über die Grenze.
- ISOCPP2 (`dds::…`) → `#include <dds/dds.hpp>`.
- Java 5 PSM (`org.omg.dds.*`) → `crates/java-omgdds`, Systemeigenschaft `org.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl`. Der Klassenkommentar von `ServiceEnvironment.java` nennt das Bootstrapping „exactly the RTI Connext / OpenSplice bootstrap idiom“ (laut Code-Kommentar).
- Klassisches OpenSplice-C++ (CCPP, `DDS::DomainParticipantFactory::get_instance()`): keine Entsprechung.

### 4.2 IDL

`#pragma keylist` wird gelesen (Vendor-Flag `Cyclone`, Kommentar `crates/zerodds-build/tests/compile.rs:45` nennt Cyclone, OpenSplice und RTI als Nutzer dieser Schreibweise). `#pragma cats` und andere OpenSplice-Pragmas bleiben erhalten und erzeugen eine Warnung (2.2). OpenSplice schreibt nur XCDR1, und dort ergeben `@final` und `@appendable` dieselben Bytes; `--default-extensibility` ist für einen OpenSplice-Peer nicht nötig. Der CDR-Dialekt ist `opensplice` (`zerodds-idlc generate --dialect opensplice`); was die OpenSplice-IDL nicht ausdrücken kann, steht in `documentation/07-migration/from-opensplice.md`.

### 4.3 QoS

Das XML-Format des OpenSplice-QosProvider wurde im Repo nicht gegen `crates/xml` getestet. Liegt es als `<dds>`/`<qos_library>` vor, gelten die Regeln aus 2.3.

### 4.4 Konfiguration (`OSPL_URI`)

`ospl.xml` wird nicht gelesen. Nachzubilden sind mindestens (Vendor-Schlüssel nach Vendor-Doku, nicht im Repo geprüft):

| OpenSplice | `zerodds.yaml` |
|---|---|
| `Domain/Id` | Domain-ID beim Anlegen des Participants |
| `DDSI2Service/General/NetworkInterfaceAddress` | `transport.interface` |
| `DDSI2Service/General/AllowMulticast` | `discovery.no_multicast` |
| `DDSI2Service/Discovery/Peers` | `discovery.peers` |
| Shared-Memory-Domain (`Database`) | kein Gegenstück; Same-Host-Pfade über `delivery.mode` |

### 4.5 Mischbetrieb

Gemessen mit der OMG-Suite `dds-rtps` (105 Tests, `-x 1`, OpenSplice CE 6.9 aus `tools/opensplice/`): ZeroDDS → ZeroDDS 105, OpenSplice → OpenSplice 90, OpenSplice → ZeroDDS 98, ZeroDDS → OpenSplice 80. Die Einordnung jedes nicht bestandenen Tests steht in `documentation/07-migration/from-opensplice.md`; ZeroDDS zuzurechnen ist davon keiner mehr; `Test_Ownership_5`/`_6` (zwei ZeroDDS-Publisher, ein OpenSplice-Subscriber) schlugen fehl, solange ZeroDDS aus OpenSplices gebündelter SPDP-Nachricht nur den ersten Teilnehmer las, und bestehen jetzt 8 von 8 Läufen (vorher 3 von 8). OpenSplice schreibt und liest nur XCDR1 und kündigt keine Datenrepräsentation an; der ZeroDDS-Standard `XCDR1` passt dazu. OpenSplice bündelt Submessages mehrerer eigener Teilnehmer mit `INFO_SRC`; ZeroDDS liest den Empfängerzustand je Submessage (`ParsedDatagram::receiver_walk`, `crates/rtps/src/datagram.rs`).

### 4.6 Unterschiede und Grenzen

- Kohärente Mengen mit TOPIC- oder GROUP-Scope von einem fremden Writer liefert ein OpenSplice-Reader nicht aus, sobald der Writer `PID_COHERENT_SET` setzt; gemessen mit ZeroDDS, Cyclone, Fast DDS und OpenDDS als Publisher.
- Ein VOLATILE-Reader hinter einem OpenSplice-Writer bekommt Samples, die vor seinem Beitritt geschrieben wurden (`Test_Durability_16`); gemessen mit ZeroDDS, Cyclone, Fast DDS, RTI und OpenDDS als Subscriber.
- `docs/architecture/00_overview.md:79` schließt proprietäre Wire-Erweiterungen wie OpenSplice-DDSI2E aus.

## 5 RTI Connext

### 5.1 API

- RTI-C (`DDS_*`) und RTI-Legacy-C++ (`DDSDomainParticipantFactory::get_instance()`): keine Entsprechung gleichen Namens; Zuordnung wie in 4.1.
- RTI Modern C++ (`dds::…`, `<dds/dds.hpp>`) → `crates/cpp`, gleiches Include. RTI-spezifische Namensräume (`rti::…`) haben kein Gegenstück.
- RTI-Java (`com.rti.dds.*`) → keine Entsprechung; Umstellung auf `org.omg.dds.*` in `crates/java-omgdds`.
- RTI-Werkzeuge: `rtiddsgen` → `zerodds-idlc`, `rtiddsspy` → `zerodds-spy`, `rtiddsperf` → `zerodds-perf` (Binärnamen `tools/spy/Cargo.toml`, `tools/perf/Cargo.toml`).

### 5.2 IDL

`keylist`, `//@key`, `@key`, `@nested`, `@topic`, `@default_nested` werden gelesen (2.2 und Liste der eingebauten Annotationen in `crates/idl-lower/src/*.rs`). `@RTI_*` und `@allowed_data_representation` werden ignoriert. Wer die Datenrepräsentation per Annotation festgelegt hat, setzt sie in ZeroDDS über `DataWriterQos.data_representation` oder `wire.data_representation_offer`.

### 5.3 QoS (`NDDS_QOS_PROFILES`, `USER_QOS_PROFILES.xml`)

- Die Datei hat die Form `<dds><qos_library><qos_profile>` und wird von `crates/xml` gelesen. Beispiel im Repo: `tests/interop/ci/rti/USER_QOS_PROFILES.xml`.
- `is_default_qos="true"` wird nicht ausgewertet (kein Treffer in `crates/xml/src`). RTI-Erweiterungen wie `<transport_builtin>`, `<protocol>`, `<resource_limits>` auf Participant-Ebene, `<representation>` beenden das Laden mit einem Fehler, der das Element nennt (`qos_parser.rs`). Sie aus der Datei entfernen oder die Datei mit `zerodds-admin --skip-unknown …` lesen (Rust: `ReadOptions::report_and_skip()`, `QosProfileRegistry::from_file_with`): jedes ausgelassene Element erscheint mit Pfad auf stderr. Die Policies von Hand übertragen und `zerodds-admin qos check` laufen lassen.
- `<participant_qos>` ist kein Container von DDS-XML 1.0 (der Name dort ist `domain_participant_qos`). Das Laden endet mit einem Fehler, der die Schreibweise der Spezifikation nennt. Mit `--vendor-forms` (Rust: `ReadOptions::accept_vendor_forms()`) wird die Herstellerform gelesen und mit Pfad gemeldet; die RTI-Elemente darin bleiben ohne `--skip-unknown` ein Fehler.
- Die Umgebungsvariable `NDDS_QOS_PROFILES` liest ZeroDDS nicht.

### 5.4 Konfiguration

RTI legt Discovery- und Transporteinstellungen in `<participant_qos>` ab. Zuordnung (Vendor-Schlüssel nach Vendor-Doku, nicht im Repo geprüft, außer `transport_builtin`):

| RTI | `zerodds.yaml` |
|---|---|
| `<discovery><initial_peers>` / `NDDS_DISCOVERY_PEERS` | `discovery.peers` |
| `<transport_builtin><mask>` | `transport.user_transport(s)` |
| `<discovery><multicast_receive_addresses>` | `discovery.spdp_multicast_group` |
| `<participant_liveliness_lease_duration>` | `discovery.participant_lease_duration_ms` |
| `<property>` `com.rti.serv.load_plugin` (Security) | `security.enclave_dir` bzw. Security-API ([security.md](security.md)) |

### 5.5 Mischbetrieb

- Belegt über `tools/dcps-shapes-interop.sh` (Fälle E, F: klassischer `@final`-ShapeType gegen `ShapeType_publisher/subscriber`) und `zerodds-examples/cross-vendor-shapes/` (laut `docs/interop-harnesses.md`, Tabelle „What each harness covers today“). RTI ist nicht Teil des CI-Gates `tests/interop/ci/`.
- RTI bevorzugt auf demselben Host Shared Memory; ohne `transport_builtin` = `UDPv4` sieht ein fremder Stack den Participant, matcht und empfängt null Samples (Kommentar in `tests/interop/ci/rti/USER_QOS_PROFILES.xml`).
- Der RTI-DataReader nimmt standardmäßig nur XCDR1 an (gleicher Kommentar). Der ZeroDDS-Standard `XCDR1` passt; ein ZeroDDS-Writer mit `[XCDR2]` braucht auf RTI-Seite `XCDR2_DATA_REPRESENTATION` im Reader.
- GROUP-kohärente Sets ZeroDDS → Connext 7.7.0: `Test_CoherentSets_21` besteht, `_12` nicht (je 3 Läufe).
- Ohne `NDDSHOME` startet RTI keinen Participant (Falle 1 im Kopf von `tools/dcps-shapes-interop.sh`).

### 5.6 Unterschiede und Grenzen

- `wstring` und `wchar` unter XCDR1: RTI zählt Zeichen, schreibt einen Terminator und nutzt 4-Byte-Einheiten (`docs/wire-axis-flags.md`, Spalte `RTI¹`). ZeroDDS-Anwendungscode schreibt die Spezifikationsform. Für diese Typen gibt es keine Laufzeit-Einstellung; betroffene Member meiden oder XCDR2 verwenden.
- RTI-Security ist nicht gemessen (Lizenz; `docs/spec-coverage/cross-vendor-validation.md`, Fußnote 2).
- Routing Service, Cloud Discovery Service, Persistence Service: eigene ZeroDDS-Dienste mit eigener Konfiguration ([crates-bridges-services.md](crates-bridges-services.md)); kein RTI-Konfigurationsformat.

## 6 eProsima Fast DDS

### 6.1 API

Die Fast-DDS-C++-API (`eprosima::fastdds::dds::DomainParticipantFactory::get_instance()->create_participant(...)`) hat kein Gegenstück. Ziel ist `<dds/dds.hpp>` (DDS-PSM-Cxx) oder `<zerodds/dds.hpp>`. Die DCPS-Namen (DomainParticipant, Publisher, DataWriter, …) bleiben; Namensraum, Erzeugung (Konstruktoren statt `create_*` mit Rohzeigern) und Typunterstützung (`TopicDataType`/`TypeSupport` → generiertes `topic_type_support<T>`) ändern sich. `fastddsgen` → `zerodds-idlc generate --lang c++`.

### 6.2 IDL

Unverändert lesbar; Fixtures `crates/idl-parse/tests/fixtures/fastdds/`. Fast DDS ignoriert `@bit_bound` an Enums und sortiert Map-Einträge nach Schlüssel (`docs/wire-axis-flags.md`, Zeilen `enum-holder`, `map-entries`). IDL, die sich darauf verlässt, auf 32-Bit-Enums und ohne `map` umstellen, wenn beide Seiten dieselben Bytes brauchen.

### 6.3 QoS

Fast-DDS-XML (`<profiles>`) wird nicht gelesen: Wurzel `<profiles>` ergibt `InvalidXml`, `<dds><profiles>` ergibt null Bibliotheken (`crates/xml/src/parser.rs:81–99`). Profile in Code übertragen oder als DDS-XML neu schreiben.

### 6.4 Konfiguration (`FASTDDS_DEFAULT_PROFILES_FILE`, `DEFAULT_FASTDDS_PROFILES.xml`)

Nicht gelesen. Zuordnung (Vendor-Schlüssel nach Vendor-Doku, nicht im Repo geprüft):

| Fast DDS | `zerodds.yaml` |
|---|---|
| `<initialPeersList>` | `discovery.peers` |
| `<metatrafficUnicastLocatorList>`, `<interfaceWhiteList>` | `transport.interface`, `transport.interface_bindings` |
| `<useBuiltinTransports>false` + UDPv4-Transport | `transport.user_transport: UDPv4` |
| Discovery-Server-Client (`<discoveryProtocol>CLIENT`) | kein Client für das Fast-DDS-Discovery-Server-Protokoll gefunden; ZeroDDS hat einen eigenen SPDP-Relay (`discovery.server`, `runtime.rs:1858–1868`) |
| `<leaseDuration>` | `discovery.participant_lease_duration_ms` |

### 6.5 Mischbetrieb

- Standard-Discovery; Belege `crates/discovery/tests/fastdds_live_spdp.rs`, `crates/dcps/tests/fastdds_live_pub.rs`, `fastdds_live_sub.rs`, `fastdds_qos_matrix.rs`, CI-Gate `tests/interop/ci/`. Ein SPDP-Handshake mit einem laufenden Fast-DDS-Discovery-Server ist als `#[ignore]`-Test vorhanden (`fastdds_live_spdp.rs:131`).
- ROS-2-Services mit `rmw_fastrtps`: ZeroDDS schreibt `PID_RELATED_SAMPLE_IDENTITY` zweimal, als 0x0083 und als eProsima-PID 0x800f (`crates/dcps/src/runtime.rs:17939–17947`, `crates/rtps/src/inline_qos.rs:119–127`). Keine Einstellung nötig.
- DDS-Security mit Fast DDS braucht RSA-Identitätszertifikate (laut Doku, `docs/spec-coverage/cross-vendor-validation.md`, Abschnitt „DDS-Security, cross-vendor“).

### 6.6 Unterschiede und Grenzen

- `wstring` zählt Zeichen (`docs/wire-axis-flags.md`, Zeile `wstring-counts`); keine Laufzeit-Einstellung in ZeroDDS.
- Fast DDS schreibt `@final` und `@appendable` als XCDR1 und `@mutable` als PL_CDR1 (laut Doku, `cross-vendor-validation.md`, Tabelle der Encapsulation-IDs). Der ZeroDDS-Reader dekodiert nach Encapsulation-Header (`crates/dcps/src/subscriber.rs:56–79`).

## 7 OpenDDS

### 7.1 API

OpenDDS nutzt die IDL-zu-C++-Abbildung von TAO (`DDS::DomainParticipantFactory_var dpf = TheParticipantFactoryWithArgs(argc, argv)`, `_var`-Typen, `TypeSupport`-Klassen, `-DCPSConfigFile`). Nichts davon existiert in ZeroDDS. Ziel ist `<dds/dds.hpp>` oder die C-API. Die Umstellung betrifft Entitätserzeugung, Speicherverwaltung (`_var` → RAII) und Typunterstützung.

### 7.2 IDL

`#pragma DCPS_DATA_TYPE` und `#pragma DCPS_DATA_KEY` werden gelesen; `DCPS_DATA_KEY` macht den Member zum Key (2.2). Ein Key-Pfad in einen Member (`p.x`, `a[0]`) wird abgewiesen und muss als `@key` am inneren Typ geschrieben werden. OpenDDS ignoriert `@value` an Enumeratoren und `@bit_bound` (`docs/wire-axes-followup.md`, Tabelle „What moved into a dialect“, Zeilen `EnumeratorValue::Ordinal`, `EnumHolderWidth`).

### 7.3 QoS

OpenDDS hat kein XML-QoS-Profil im Repo-Kontext; QoS liegt im Anwendungscode und wird dorthin übertragen.

### 7.4 Konfiguration (`-DCPSConfigFile *.ini`)

Beispiel im Repo: `tests/perf/dds-roundtrip-bench/opendds_rtps.ini`.

| OpenDDS | `zerodds.yaml` bzw. Hinweis |
|---|---|
| `[common] DCPSDefaultDiscovery=rtps_disc` | Pflicht auf OpenDDS-Seite; OpenDDS-Standard ist InfoRepo (Kommentar in der `.ini`) |
| `[transport/…] transport_type=rtps_udp` | Pflicht auf OpenDDS-Seite |
| `[rtps_discovery/…] use_xtypes=no` | nötig, wenn ZeroDDS-Writer ohne TypeObject angelegt werden (plain `*_create`); mit `*_typed` nicht nötig |
| `[rtps_discovery/…] SpdpSendAddrs`, statische Peers | `discovery.peers` |
| `ResendPeriod`, `LeaseDuration` | `discovery.spdp_period_ms`, `discovery.participant_lease_duration_ms` |

### 7.5 Mischbetrieb

- Fälle G, H in `tools/dcps-shapes-interop.sh` (klassischer ShapeType, `-DCPSConfigFile rtps.ini`), M15/M16 (OpenDDS-Writer schreibt XCDR2 `0x0007`, 194 Samples, laut Kommentar im Skript). OpenDDS ist nicht Teil des CI-Gates.
- Ein OpenDDS-Reader, der nur XCDR2 annimmt, braucht `wire.data_representation_offer: [XCDR2]`: das erste Element des Writers muss in der Liste des Readers stehen (`negotiate`, Standard `strict`, `crates/rtps/src/publication_data.rs:166–173`); welche Liste der OpenDDS-Reader annonciert, ist im Repo nicht einzeln gemessen).
- Strikte Encapsulation-Prüfung auf OpenDDS-Seite (`EncapsulationHeader::to_encoding`): ZeroDDS setzt den Header nach der deklarierten Extensibility (`crates/dcps/src/publisher.rs:3585–3603`).
- Secure SPDP gegen OpenDDS: der Standard `SpdpResponseTargets::AnnouncedLocatorsAndBeaconSource` ist für OpenDDS nötig (`crates/rtps/src/dialect.rs`, Abschnitt `SpdpResponseTargets`; gemessen 0/5 gegen 5/5). `wire.rtps_dialect: cyclone` oder `fastdds` schaltet diesen Wert ab.

### 7.6 Unterschiede und Grenzen

- OpenDDS-Security: In der Matrix `docs/spec-coverage/cross-vendor-validation.md`, Abschnitt „DDS-Security, cross-vendor“, matcht der einfache Typ gesichert nicht (`NO_MATCH`/`FAIL`), der reiche Typ in beiden Richtungen (194 µs bzw. 125 µs p50) (laut Doku). Die Algorithmusnamen im Handshake schreibt ZeroDDS per Standard (`security.algo_name_nul: spec`) mit dem abschließenden NUL, das OpenDDS verlangt (`crates/security/src/vendor_forms.rs:10–19`, `crates/discovery/src/security/stack.rs:1098–1103`).
- Enumeratorwerte, Union-Rahmung und `@mutable`-Member-Header unter XCDR1 weichen ab (`docs/wire-axis-flags.md`, Spalten `ODDS¹`, `ODDS²`); keine Laufzeit-Einstellung.

## 8 Offene und widersprüchliche Punkte

| Punkt | Stand im Code | Widerspruch bzw. Lücke |
|---|---|---|
| OpenSplice | keine Messung, kein Harness | alle OpenSplice-Aussagen dieser Seite sind unbelegt |
