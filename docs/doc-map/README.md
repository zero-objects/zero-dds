# ZeroDDS — Doku-Map

Englische Fassung: [README.en.md](README.en.md); jede Seite `<name>.md` hat eine `<name>.en.md`.

Einstieg in die Doku-Map: was es in ZeroDDS gibt, wo es liegt, wie die Teile
zusammenhängen, wie man sie konfiguriert, betreibt, absichert, anbindet und
von einem anderen DDS umstellt. Die Seiten sind neu aus dem Code geschrieben
und sollen als Grundlage einer besseren Doku dienen.

Konventionen auf allen Seiten:

- Pfade sind relativ zur Repository-Wurzel (`crates/rtps/src/reliable_writer.rs`).
- Aussagen stammen aus dem Code. Was nur aus einem Doku-Kommentar, einer Spec
  oder einer Markdown-Datei stammt, ist mit „(laut Doku-Kommentar)“ bzw.
  „(laut Doku)“ markiert.
- Gebaut oder ausgeführt wurde bei der Erhebung nichts, außer den schon
  gebauten Programmen mit `--help` und `--check-config`. Wo eine Seite
  Messwerte nennt, steht die Quelle (Test, Runner, Host) dabei.
- Jede Seite endet mit einem Abschnitt zu Widersprüchen und offenen Punkten;
  [befunde.md](befunde.md) sammelt sie.

## Seiten

| Seite | Inhalt |
|---|---|
| [architecture.md](architecture.md) | Schichtenmodell, Laufzeitmodell, Weg eines Samples vom `write` bis zum Reader, Discovery, Zuverlässigkeit, Durability, Typ-Discovery, Content-Filter, SHM, Security-Einhängung |
| [crates-core.md](crates-core.md) | Kern-Crates: Typsystem und Wire, RTPS und Discovery, Transporte, DCPS und Konfiguration, Beobachtung, DDS-Profile, Dienste, Test und Build |
| [crates-bridges-services.md](crates-bridges-services.md) | Die sechs Bridge-Daemons, `bridge-security`, CORBA-Familie, OPC UA, SOAP |
| [crates-languages-codegen.md](crates-languages-codegen.md) | IDL-Pipeline, `zerodds-idlc`/`zerodds-xmlc`, 21 Zielsprachen, Sprachanbindungen, Endpoint-SDKs |
| [config.md](config.md) | Ebenenmodell, alle 46 Runtime-Settings, Schlüssel jedes Daemons, DDS-XML-QoS, Security-Dateien, Umgebungsvariablen |
| [cli-options.md](cli-options.md) | Jedes Programm mit Synopsis, Optionen, Exit-Codes, Konfigdatei, man-Seite |
| [tools-and-man.md](tools-and-man.md) | Das Verzeichnis `tools/`, die man-Seiten, `packaging/` |
| [system-setup.md](system-setup.md) | Bausteine eines vollständigen Systems und Rezepte vom Ein-Host-Aufbau bis zum Dienst unter systemd/launchd |
| [security.md](security.md) | DDS-Security (Plugins, Governance/Permissions, Enclaves, Krypto-Backends) und Bridge-Sicherheit (TLS/DTLS, Auth, ACL), Schritt-für-Schritt-Listen |
| [migration.md](migration.md) | Umstellung von Cyclone DDS, OpenSplice, RTI Connext, Fast DDS, OpenDDS: API, IDL, QoS, Konfiguration, Mischbetrieb |
| [interop-and-dialects.md](interop-and-dialects.md) | Anbinden an ein bestehendes System auf dem Wire, Dialekt- und Wire-Achsen-Flags, Tabelle je Gegenüber |
| [ros2.md](ros2.md) | RMW `rmw_zerodds_cpp`, Build gegen ROS 2, `ros2.*`-Settings, Diagnose-CLI, gemessene Interop |
| [befunde.md](befunde.md) | Alle bei der Erhebung gefundenen Widersprüche zwischen Code, Doku, man-Seiten und Beispielen |

## Wegweiser

| Ich will … | Seite |
|---|---|
| verstehen, wie ein Sample durch das System läuft | [architecture.md §3](architecture.md#3-weg-eines-samples) |
| wissen, welches Crate was tut | [crates-core.md](crates-core.md), [crates-bridges-services.md](crates-bridges-services.md), [crates-languages-codegen.md](crates-languages-codegen.md) |
| eine Einstellung finden, samt Default und Umgebungsvariable | [config.md §2](config.md#2-laufzeit-einstellungen-zeroddsyaml) |
| wissen, in welcher Reihenfolge Dateien, Umgebung und `--set` gelten | [config.md §1](config.md#1-ebenenmodell) |
| die Optionen eines Programms nachschlagen | [cli-options.md](cli-options.md) |
| Typen aus IDL für meine Sprache erzeugen | [crates-languages-codegen.md §10](crates-languages-codegen.md#10-so-erzeugt-man-typen-für-eine-anwendung) |
| ein System aufbauen (ein Host, mehrere Hosts, ohne Multicast, mit Persistenz) | [system-setup.md, Rezepte](system-setup.md#rezepte) |
| Security einschalten | [security.md (c)](security.md#c-sicherheit-einrichten-schritt-für-schritt) |
| ZeroDDS neben Cyclone/Fast DDS/RTI/OpenDDS/OpenSplice betreiben | [interop-and-dialects.md](interop-and-dialects.md) |
| ein bestehendes System umstellen | [migration.md](migration.md) |
| MQTT, WebSocket, CoAP, AMQP, gRPC oder CORBA anbinden | [crates-bridges-services.md](crates-bridges-services.md) |
| ROS 2 mit ZeroDDS betreiben | [ros2.md](ros2.md) |
| eine man-Seite finden | [tools-and-man.md](tools-and-man.md) |

## Das System in einem Bild

```
 Anwendung ─┬─ Rust: crates/dcps (DomainParticipant, DataWriter, DataReader)
            ├─ C/C++: crates/zerodds-c-api (libzerodds, zerodds.h), crates/cpp
            ├─ Python/Java/C#/TypeScript/…: crates/py, crates/java-omgdds, …
            └─ ROS 2: crates/rmw-zerodds-shim (rmw_zerodds_cpp)
                 │
   Typen: IDL ── zerodds-idlc ──► Code je Sprache; XTypes TypeObject/TypeLookup
                 │
   DCPS-Runtime (crates/dcps): QoS, History, Reader/Writer-Proxies, Security-Gate
                 │
   RTPS (crates/rtps, crates/discovery): SPDP/SEDP, Heartbeat/AckNack, Fragmente
                 │
   Transporte: UDP (Multicast/Unicast), TCP, SHM (gleicher Host), UDS, TSN
                 │
   Dienste/Werkzeuge: Durability-Service, Router, Recorder/Replay, Monitor,
   Admin/Spy · Bridges: MQTT, WebSocket, CoAP, AMQP, gRPC, CORBA, OPC UA
```

Details: [architecture.md](architecture.md).

## Verzeichnisse der Repository-Wurzel

| Verzeichnis | Inhalt |
|---|---|
| `crates/` | Alle Rust-Crates (Bibliotheken, Daemons, Anbindungen); siehe die drei Crate-Seiten |
| `tools/` | Programme und Skripte: Anwenderwerkzeuge, CI-Gates, Test-Harnesses ([tools-and-man.md](tools-and-man.md)) |
| `man/` | `man1` (19 Programme), `man5` (7 Konfigdateien) |
| `packaging/` | deb/rpm/systemd, Homebrew/launchd/pkg, Windows, Docker, Python, Release-Skripte |
| `pkg/` | weitere Paketdefinitionen (debian, macos, rpm, windows) |
| `endpoints/` | Native Endpoint-SDKs je Sprache (C, C++, Ada, Go, Java, Python, Rust, …) |
| `runtime/` | Laufzeitdateien der erzeugten Sprachanbindungen je Sprache |
| `examples/` | Tutorials und Demos (`examples/tutorials/dds-chat`, `examples/demos/…`) |
| `zerodds-examples/` | Beispiele zu den Code-Ausschnitten der Website (lokaler Unterordner, nicht versioniert) |
| `docs/specs/` | 47 ZeroDDS-Specs (Bridges, Deployment, Codegen, …) |
| `docs/spec-coverage/` | Abdeckung je Spec mit Beleg und Status, `.open`-Aggregate |
| `docs/adr/` | Architekturentscheidungen |
| `documentation/` | Nutzerdoku aus einem früheren Stand (`documentation/README.md`); nicht gegen den Code geprüft |
| `build-integration/`, `cmake/` | `zerodds-idlc` als Build-Schritt in fremden Build-Systemen |
| `buildroot/`, `meta-zerodds/` | Buildroot-Paket, Yocto-Layer |
| `benches/` | Criterion-Benchmarks |
| `tests/`, `xtests/`, `testing/`, `interop/` | Interop-Korpus und -Ergebnisse, Crate-übergreifende Tests, Isolation, Interop-Fixtures |
| `ci/` | GitLab-CI-Jobs, Images, Hilfsskripte |
| `scripts/` | Pre-Commit, Crate-Publish, Mirror-Build |
| `website/` | Quellen von zerodds.org |
| `diagrams/` | Diagramm-Quellen (dot) |
| `internal/` | interne Arbeitsnotizen, Release-Unterlagen, OPEN-ITEMS; nicht öffentlich |
| `github/` | lokaler Spiegel für das öffentliche Repository (nicht Teil dieses Repos) |

## Pflege

Eine Seite gilt so lange, wie der beschriebene Code gilt. Wer Code ändert, der
auf einer dieser Seiten mit Pfad genannt ist, gleicht die Seite und ihre
`.en.md`-Fassung an. Die
Befundliste [befunde.md](befunde.md) wird abgearbeitet und nicht fortgeschrieben:
ein behobener Befund fliegt heraus.
