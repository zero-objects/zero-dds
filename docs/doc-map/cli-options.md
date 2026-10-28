# CLI-Programme und Optionen

Diese Seite führt jedes Programm auf, das im Workspace als Binary gebaut wird (`[[bin]]`-Abschnitte, `src/main.rs`, `src/bin/*.rs` unter `crates/` und `tools/`), mit Zweck, Aufruf, Optionen samt Vorgabewerten, Exit-Codes, gelesener Konfigurationsdatei und Manpage. Grundlage ist der Argument-Parser im Quelltext; für die 18 in `target/debug` vorhandenen Binaries wurde zusätzlich `--help` und `--version` ausgeführt und mit Code und Manpage verglichen. Programme, die nur Test- oder Messzwecken dienen, stehen gesammelt am Ende. Die Abweichungen zwischen Hilfetext, Code und Manpage stehen im letzten Abschnitt.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Übersicht

84 Binary-Targets, davon 27 Programme für Anwender oder Betrieb (Abschnitte unten) und 57 interne Test-, Mess- und Generator-Programme (Abschnitt [Interne Programme](#interne-programme)). Fuzz-Targets unter `*/fuzz/` sind nicht mitgezählt.

| Programm | Quelle | Konfiguration | Manpage | Paket (rpm/deb) |
|---|---|---|---|---|
| `zerodds-admin` | `tools/admin/src/main.rs` | Laufzeit (`zerodds.yaml`) | `man1/zerodds-admin.1` | `zerodds-cli` |
| `zerodds-spy` | `tools/spy/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-spy.1` | `zerodds-cli` |
| `zerodds-record` | `tools/record/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-record.1` | `zerodds-cli` |
| `zerodds-replay` | `tools/replay/src/main.rs` | keine Flags; `--inject` liest die Standardpfade | `man1/zerodds-replay.1` | `zerodds-cli` |
| `zerodds-monitor` | `tools/monitor/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-monitor.1` | `zerodds-cli` |
| `zerodds-mq` | `tools/mq/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-mq.1` | `zerodds-cli` |
| `zerodds-snitch` | `tools/snitch/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-snitch.1` | `zerodds-cli` |
| `zerodds-bench` | `tools/bench/src/main.rs`, `lib.rs` | Laufzeit | `man1/zerodds-bench.1` | `zerodds-cli` |
| `zerodds-pcap` | `tools/pcap/src/main.rs`, `lib.rs` | keine | `man1/zerodds-pcap.1` | `zerodds-cli` |
| `zerodds-router` | `crates/routing-service/src/bin/zerodds-router.rs` | Laufzeit + Routendatei | `man1/zerodds-router.1` | `zerodds-router` |
| `zerodds-durability-svc` | `crates/durability-service-bin/src/main.rs` | Laufzeit | keine | nicht paketiert (rpm schließt es aus) |
| `zerodds-idlc` | `tools/idlc/src/main.rs` | keine | `man1/zerodds-idlc.1` | `zerodds-cli` |
| `zerodds-xmlc` | `tools/xmlc/src/main.rs` | keine | `man1/zerodds-xmlc.1` | `zerodds-cli` |
| `zerodds-lint` | `crates/lint/src/bin/zerodds-lint.rs` | keine | keine | `zerodds-cli` |
| `zerodds-traceability` | `tools/traceability/src/main.rs` | keine | keine | `zerodds-cli` |
| `zerodds-ros2-shim` | `crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs` | Laufzeit + `ros2`-Abschnitt | `man1/zerodds-ros2-shim.1`, `man5/zerodds-ros2.yaml.5` | `zerodds-ros2` |
| `zerodds-ws-bridged` | `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs` | `ws-bridged.yaml` | `man1/…ws-bridged.1`, `man5/…ws-bridged.yaml.5` | `zerodds-ws-bridge` |
| `zerodds-mqtt-bridged` | `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` | `mqtt-bridged.yaml` | `man1`, `man5` | `zerodds-mqtt-bridge` |
| `zerodds-coap-bridged` | `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` | `coap-bridged.yaml` | `man1`, `man5` | `zerodds-coap-bridge` |
| `zerodds-amqp-bridged` | `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` | `amqp-bridged.yaml` | `man1`, `man5` | `zerodds-amqp-bridge` |
| `zerodds-grpc-bridged` | `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` | `grpc-bridged.yaml` | `man1`, `man5` | `zerodds-grpc-bridge` |
| `zerodds-corba-bridged` | `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` | `corba-bridged.yaml` | `man1`, `man5` | `zerodds-corba-bridge` |
| `zerodds-secure-permissions` | `tools/secure-permissions/src/main.rs` | keine | keine | nur rpm `zerodds-cli` |
| `zerodds-chaos` | `tools/chaos/src/main.rs` | keine | keine | `zerodds-cli` |
| `zerodds-perf` | `tools/perf/src/main.rs` | keine | keine | `zerodds-cli` |
| `zerodds-dashboard` | `tools/dashboard/src/main.rs` | keine | keine | `zerodds-cli` |
| `zerodds-recorder-bridge` | `tools/recorder-bridge/src/main.rs` | keine | keine | `zerodds-cli` |

Paketzuordnung aus `packaging/linux/rpm/zerodds.spec` (`%files`) und `packaging/linux/deb/assemble-debs.sh`. Die deb-Schleife für `zerodds-cli` enthält `zerodds-secure-permissions` nicht; die rpm-Liste enthält es.

## Gemeinsame Konfigurationsoptionen

Alle Programme, die eine DDS-Laufzeit starten, lesen dieselben drei Flags. Parser: `CommandLine::extract` in `crates/config/src/layers.rs`; Einbindung und Hilfetext `HELP`: `crates/dcps/src/cli.rs` (`Configured`).

| Option | Bedeutung |
|---|---|
| `--config PATH`, `--config=PATH` | Basisdatei; Drop-ins im Nachbarverzeichnis `PATH.d/`. Zweimal angegeben: Fehler. Ohne die Option gilt das Systemlayout (unten). |
| `--set PATH=VALUE`, `--set=PATH=VALUE` | Ein Wert oberhalb aller Dateien, wiederholbar, spätere gewinnen. `--set +a.b=V` hängt an eine Liste an. Unbekannter Pfad: Fehler. |
| `--check-config` | Lädt alle Schichten wie beim Start, gibt jeden Wert mit Herkunft aus (`sources:`, `values:`, Schlusszeile `configuration valid` / `configuration invalid`) und beendet das Programm. |

Die Auswertung endet bei `--`; alles danach gehört dem Programm.

Rangfolge, niedrigste zuerst: eingebaute Vorgaben, Umgebung (`SETTINGS` in `crates/dcps/src/runtime.rs`, z. B. `ZERODDS_TICK_PERIOD_MS`), Dateien (Basis, Drop-ins, `include:`), `--set`.

Systemlayout (`SystemLayout::dir`, `for_stem` in `crates/config/src/layers.rs`):

| Plattform | Verzeichnis | Drop-ins |
|---|---|---|
| Linux | `/etc/zerodds` | `/etc/zerodds/<stem>.d`, dann `/run/zerodds/<stem>.d` |
| macOS | `/usr/local/etc/zerodds` | `<dir>/<stem>.d` |
| Windows | `%PROGRAMDATA%\ZeroDDS` (Fallback `C:\ProgramData\ZeroDDS`) | `<dir>\<stem>.d` |

Basisdatei ist `<dir>/<stem>.yaml`. `stem` ist `zerodds` für die Werkzeuge, den Router, den Durability-Dienst und `zerodds-ros2-shim`; die Bridges verwenden `ws-bridged`, `mqtt-bridged`, `coap-bridged`, `amqp-bridged`, `grpc-bridged`, `corba-bridged`.

Die Bridges bilden jede eigene Option auf `--set` ihres Dateischlüssels ab (Tabellen je Bridge unten). Diese Einträge liegen in der Flag-Schicht und erscheinen in `--check-config` mit ihrer Herkunft, z. B. ``domain = 3  (command line `--set domain=3`)`` (gemessen mit `zerodds-ws-bridged --domain=3 --check-config`).

Exit-Codes rund um die Konfiguration, laut `HELP`: 0 gültig, 1 ungültiger Wert oder unbekannter Schlüssel, 2 unlesbare Datei, Syntaxfehler, Include-Zyklus. Gemessen (target/debug):

| Fall | Werkzeuge, Router, Durability | Bridges |
|---|---|---|
| gültig, `--check-config` | 0 | 0 |
| ungültiger Wert (`--set runtime.tick_period_ms=abc`) | 1 | 1 |
| unbekannter `--set`-Pfad | 2 (`zerodds-spy`) | 1 (`zerodds-ws-bridged`) |
| `--config /nonexistent.yaml` | 2 | 1 |

Die Bridges geben für jeden Konfigurationsfehler 1 zurück, drucken aber denselben `HELP`-Text mit „2 unreadable file“.

Programme mit diesem Mechanismus: `zerodds-admin`, `-spy`, `-record`, `-monitor`, `-mq`, `-snitch`, `-bench`, `-router`, `-durability-svc` (über `Configured::from_args`/`from_env_args`), die sechs Bridges (über `CommandLine::extract` + `Configured::from_command_line`), `zerodds-ros2-shim` (über `CommandLine::extract` + `ConfigSource::for_program`). Ohne diese Flags: `zerodds-replay`, `-pcap`, `-idlc`, `-xmlc`, `-lint`, `-traceability`, `-secure-permissions`, `-chaos`, `-perf`, `-dashboard`, `-recorder-bridge`.

Die Schlüssel selbst beschreibt [config.md](config.md).

## Werkzeuge für Betrieb und Diagnose

Die Werkzeuge `spy`, `record`, `monitor`, `mq`, `snitch`, `bench` teilen: `-h/--help` und `-V/--version` werden vor allem anderen geprüft (irgendwo in der Argumentliste), Dauerangaben `DUR` akzeptieren `5`, `30s`, `2m`, `1h` (`parse_duration`, `tools/cli-common`), Exit-Codes 0 Erfolg, 2 Parse-Fehler, 3 DDS- oder E/A-Fehler.

### zerodds-admin

Operator-CLI: tritt einer laufenden Domain bei und listet Teilnehmer und Endpunkte, oder analysiert DDS-XML-Dateien offline.

```
zerodds-admin <group> <verb> [args] [--json] [--timeout-ms T]
```

| Befehl | Wirkung |
|---|---|
| `domain inspect <id>` | live: Teilnehmer mit Writer-/Reader-Endpunkten (Topic, Typ, Reliability, Durability) |
| `discovery snapshot <id>` | live: SPDP/SEDP-Momentaufnahme, Teilnehmerliste, Pub/Sub-Zahlen |
| `config inspect <file.xml>` | offline: Deployment laden, Topologie nach Domain-ID |
| `qos validate <file.xml>` | offline: Wohlgeformtheit, jede Bibliothek parsen |
| `qos check <file.xml>` | offline: RxO-Kompatibilität Writer/Reader (DDS 1.4 §2.2.3), Exit 1 bei Inkompatibilität |

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `--json` | maschinenlesbare Ausgabe | aus |
| `--timeout-ms <T>` | Beobachtungsfenster für `domain`/`discovery`; nur Form mit Leerzeichen | 2000 |
| `-h`, `--help` | Hilfe; auch ohne Argumente, Exit 0 | |

`--version`: Version, Exit 0. Exit-Codes: 0 ok, 1 Validierungs- oder Probe-Fehler, 2 Aufruffehler.

### zerodds-spy

Abonniert ein Topic als Rohbytes und gibt Metadaten und einen Hex-Ausschnitt pro Sample aus; optional typisiert dekodiert.

```
zerodds-spy [subscribe] -t TOPIC [OPTIONS]
```

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `-d`, `--domain <ID>` | Domain-ID | 0 |
| `-t`, `--topic <NAME>` | Topic, Pflicht | |
| `--type <NAME>` | Typname direkt ankündigen, keine Typverfolgung | Typen der Writer werden entdeckt und verfolgt |
| `-n`, `--count <N>` | nach N Samples stoppen | unbegrenzt |
| `--duration <DUR>` | nach Dauer stoppen | unbegrenzt |
| `-x`, `--hex <BYTES>` | erste BYTES als Hex, 0 = aus | 32 |
| `--decode` | Samples als typisiertes JSON (braucht `--type-file`) | aus |
| `--type-file <IDL>` | IDL-Datei mit den Typen | |
| `--dialect <NAME>` | Wire-Dialekt, in dem dekodiert wird: `cyclone`, `rti`, `fastdds`, `opendds`, ... | `wire.codec_dialect` (`specification`) |

Quelle der Optionen: `parse_subscribe` in `tools/spy/src/lib.rs`.

### zerodds-record

Schreibt Live-Samples in eine `.zddsrec`-Datei und liest bestehende Dateien. Format: `docs/specs/zddsrec-1.0.md`.

```
zerodds-record record [OPTIONS]
zerodds-record info FILE
zerodds-record list FILE
```

| Option (`record`) | Bedeutung | Vorgabe |
|---|---|---|
| `-o`, `--output <FILE>` | Ausgabedatei | `capture-<unix-ts>.zddsrec` |
| `-d`, `--domain <ID>` | Domain-ID | 0 |
| `-t`, `--topic <NAME>` | Topic, wiederholbar, mindestens eins | |
| `--duration <DUR>` | Aufnahmedauer | bis SIGINT |
| `--max-sample-bytes <N>` | Obergrenze pro Sample | 1048576 |
| `--decode` | zusätzlich typisiert dekodieren (braucht `--type-file`) | aus |
| `--type-file <IDL>` | IDL mit den Typen | |
| `--map <TOPIC=Type>` | IDL-Typ für ein Topic festlegen, wiederholbar | |
| `--out-json <FILE>` | dekodierte Samples als NDJSON | |
| `--out-sqlite <FILE>` | dekodierte Samples in SQLite, eine Tabelle pro Topic; eine vorhandene FILE wird ersetzt, wie bei `-o` und `--out-json` | |
| `--dialect <NAME>` | Wire-Dialekt, in dem dekodiert wird: `cyclone`, `rti`, `fastdds`, `opendds`, ... | `wire.codec_dialect` (`specification`) |

`info` gibt Header (Zeitbasis, Teilnehmer, Topics) aus, `list` die Frame-Zahl pro Topic. Quelle: `tools/record/src/lib.rs`.

### zerodds-replay

Liest `.zddsrec`, NDJSON oder SQLite und gibt die Frames aus oder speist sie mit Originaltiming in eine Domain ein.

```
zerodds-replay inspect FILE
zerodds-replay dump FILE
zerodds-replay replay FILE [--time-scale F] [--topic NAME]... [--inject [--inject-domain N]]
```

| Befehl / Option | Bedeutung | Vorgabe |
|---|---|---|
| `inspect FILE` | Header, Frames, Payload-Bytes, Zeitbereich | |
| `dump FILE` | CSV `t_delta_ns,participant,topic,kind,payload_len` | |
| `replay FILE` | Frames in aufgezeichneter Reihenfolge | |
| `--time-scale F` | Faktor gegenüber Wanduhr | 1.0 |
| `--topic NAME` | Filter, wiederholbar | alle Topics |
| `--inject` | über einen Writer pro Topic in eine Domain schreiben | aus |
| `--inject-domain N` | Domain für `--inject` | 0 |
| `-h`, `--help`, `help` | Hilfe (auch ohne Argumente), Exit 0 | |

Formaterkennung nach Endung (`detect_format`): `.json`, `.ndjson`, `.jsonl` → NDJSON; `.db`, `.sqlite`, `.sqlite3` → SQLite; sonst `.zddsrec`. Exit-Codes: 0, 1 bei Lese- oder Laufzeitfehler, 2 bei Aufruffehler. `--version`: Exit 0. `--inject` erzeugt die Laufzeit über `zerodds_runtime_create` (C-API), das die Standardpfade des Systemlayouts liest (`ffi_runtime_config` → `ConfigSource::from_standard_paths` in `crates/zerodds-c-api/src/lib.rs`); `--config`/`--set` gibt es nicht.

### zerodds-monitor

Gibt die Metrik-Registry der Laufzeit einmalig aus oder bedient einen Prometheus-Endpunkt `/metrics`.

```
zerodds-monitor snapshot [-d ID] [--duration DUR] [-f text|prometheus]
zerodds-monitor serve [-d ID] [-a ADDR] [--duration DUR]
zerodds-monitor names
```

| Option | Befehl | Vorgabe |
|---|---|---|
| `-d`, `--domain <ID>` | snapshot, serve | 0 |
| `--duration <DUR>` | snapshot: Sammelfenster; serve: automatischer Stopp | snapshot 5s; serve bis SIGINT |
| `-f`, `--format <FORMAT>` | snapshot: `text`, `prometheus` (Alias `prom`) | `text` |
| `-a`, `--addr <ADDR>` | serve: Listen-Adresse | `127.0.0.1:9991` |

`names` listet die bekannten Metriknamen.

### zerodds-mq

Leitet Rohsamples zwischen zwei Domains weiter, Payload unverändert.

```
zerodds-mq [bridge] -t TOPIC --src-domain N --dst-domain M [OPTIONS]
```

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `--src-domain <ID>` | Quell-Domain | 0 |
| `--dst-domain <ID>` | Ziel-Domain; muss sich unterscheiden (Prüfung in `main.rs`) | 1 |
| `-t`, `--topic <NAME>` | gleicher Topic-Name auf beiden Seiten, Pflicht | |
| `--src-topic <N>`, `--dst-topic <N>` | abweichende Namen, überschreiben `-t` | |
| `--duration <DUR>` | Stopp nach Dauer | bis SIGINT |
| `--bidirectional` | in beide Richtungen spiegeln | aus |

### zerodds-snitch

Hört SPDP/SEDP für eine Dauer ab und gibt die gesehenen Teilnehmer und Endpunktzahlen aus.

```
zerodds-snitch [probe] [-d ID] [--duration DUR] [-f text|json]
```

| Option | Vorgabe |
|---|---|
| `-d`, `--domain <ID>` | 0 |
| `--duration <DUR>` | 5s |
| `-f`, `--format <FORMAT>` (`text`, `json`) | `text` |

### zerodds-bench

Latenz- und Durchsatzmessung über die DCPS-Pipeline mit Writer und Reader im selben Prozess.

```
zerodds-bench latency|throughput [-d ID] [-t NAME] [-p N] [--duration DUR]
zerodds-bench info
```

| Option | Vorgabe |
|---|---|
| `-d`, `--domain <ID>` | 0 |
| `-t`, `--topic <NAME>` | `zerodds/bench/loopback` |
| `-p`, `--payload <N>` (Bytes) | 64 |
| `--duration <DUR>` | 5s |

`info` gibt die Fähigkeiten des Backends aus.

### zerodds-pcap

Offline-Parser für libpcap-Dateien: findet RTPS-Payloads und dekodiert die Submessages. Kein Domain-Beitritt.

```
zerodds-pcap parse FILE
zerodds-pcap stats FILE
```

Optionen nur `-h/--help`, `-V/--version`. Exit-Codes 0, 2 Aufruffehler, 3 Datei-, pcap- oder RTPS-Dekodierfehler.

### zerodds-router

DDS-zu-DDS-Weiterleitung zwischen Domains, Topics oder Partitionen mit optionalem Filter und Transform.

```
zerodds-router run --routes <file> [--types <shapes.json>]
zerodds-router validate --routes <file>
```

| Option | Bedeutung |
|---|---|
| `--routes <file>` | Routendatei, Pflicht; Endung `.xml` → XML, sonst JSON |
| `--types <shapes.json>` | JSON-Array von Typformen, nötig für Filter/Transform |
| `--config`, `--set`, `--check-config` | Laufzeitkonfiguration (`zerodds.yaml`); `--config` mit Endung `.json`/`.xml` wird mit Verweis auf `--routes` abgelehnt, Exit 2 |

Mit `-h/--help`: Hilfe auf stderr, Exit 0; `-V/--version`: Version, Exit 0. Ohne oder mit unbekanntem Befehl: Hilfe, Exit 2. Fehler in `run`/`validate`: Exit 1. Unlesbare Konfigurationsdatei: Exit 2. Signale: SIGINT/SIGTERM beenden, danach Metriken pro Route (`forwarded`, `dropped_loop`, `dropped_filter`, `lifecycle`, `errors`). Die systemd-Unit startet `zerodds-router run --routes /etc/zerodds/router.json` (`packaging/linux/systemd/zerodds-router.service`); Beispiel `packaging/linux/configs/router.json.example`.

### zerodds-durability-svc

Durability-Dienst (ADR 0009): hält TRANSIENT/PERSISTENT-Samples einer Domain in einem Speicher und bedient spät beitretende Reader.

```
zerodds-durability-svc --domain N [--store KIND] [--path PATH] [--auto] [--topic NAME]...
```

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `--domain N` | Domain-ID, Pflicht | |
| `--store KIND` | `sqlite`, `file`, `lakehouse` (Feature `lakehouse`), `postgres` (Feature `postgres`) | `sqlite` |
| `--path PATH` | sqlite/lakehouse: Datenbankdatei (fehlt: im Speicher, mit Warnung); file: Verzeichnis, Pflicht; postgres: Verbindungs-String, Pflicht | |
| `--auto` | jedes Topic bedienen, dessen Writer TRANSIENT oder PERSISTENT erklären | aus |
| `--topic NAME` | Topic explizit bedienen, wiederholbar | |
| `-h`, `--help` | Hilfe, Exit 0 | |

Mindestens `--auto` oder ein `--topic`. Exit-Codes: 0; 1 für jeden Fehler in `run`, einschließlich unbekannter Argumente und fehlendem `--domain` (Code: `parse_args` läuft innerhalb von `run`); 2 für Konfigurationsfehler. `--version`: Exit 0. Keine Manpage. systemd-Unit `packaging/linux/systemd/zerodds-durability-svc.service` setzt die Argumente aus `ZERODDS_DURABILITY_DOMAIN`, `_STORE`, `_PATH`, `_EXTRA` (Vorgaben 0, `sqlite`, `/var/lib/zerodds/durability/dur.db`, `--auto`); diese Variablen liest nur die Unit, nicht das Programm.

### zerodds-idlc

Liest OMG IDL 4.2: prüft, lintet, formatiert, gibt Modell und Abhängigkeiten aus, erzeugt Quelltext für eine Sprache.

```
zerodds-idlc <SUBCOMMAND> [OPTIONS] FILE...
```

Unterbefehle: `check`, `lint`, `format`, `dump-ast`, `print-deps`, `generate`. Optionen und Dateien dürfen gemischt stehen.

| Option | Gilt für | Bedeutung | Vorgabe |
|---|---|---|---|
| `--strict` | alle | nur IDL 4.2, jede Herstellererweiterung abweisen | aus |
| `--json` | alle | Diagnosen maschinenlesbar | Text |
| `--deny-warnings` | alle | Exit ungleich 0, sobald etwas gemeldet wurde | aus |
| `--default-extensibility <final\|appendable\|mutable>` | alle | Extensibility für Aggregate ohne Angabe | `appendable` |
| `--check` | `format` | nicht kanonische Dateien nennen, Exit 1 | |
| `--write` | `format` | nicht kanonische Dateien überschreiben; mit `--check` unvereinbar | |
| `--lang <NAME>` | `generate`, Pflicht | `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin` (`Language::as_str`, `crates/lang-cst/src/common.rs`) | |
| `--out <DIR>` | `generate` | Zielverzeichnis | `.` |
| `--encoding <ENC>` | `generate` | `xcdr1`, `xcdr2`, `pl-cdr1`, `pl-cdr2`, `delimited-cdr` | `xcdr2` |
| `--endian <le\|be>`, `--xcdr-endianness <BE\|LE>` | `generate` | Byte-Reihenfolge im Präambeltext | `le` |

Format-Optionen außerhalb von `format` und Generate-Optionen außerhalb von `generate` werden mit Namen abgelehnt. Exit-Codes: 0 nichts gemeldet, 1 Diagnose oder `--check` fand eine Datei, 2 Aufruffehler, 3 Datei unlesbar.

### zerodds-xmlc

Liest OMG-DDS-XML-1.0-Dokumente.

```
zerodds-xmlc validate|types|render FILE...
```

`validate` parst jede Bibliothek (types, qos, domain, participant, application) und zählt Fehler, `types` listet Typdefinitionen, `render` zeigt Domains und Teilnehmer. Jede Option außer `-h/--help`, `-V/--version` wird abgelehnt. Exit-Codes 0, 1 Bibliothek nicht parsebar, 2 Aufruffehler, 3 Datei unlesbar.

### zerodds-lint

Projekteigene Lint-Regeln über den Workspace (clap-basiert).

```
zerodds-lint check [--root <ROOT>] [--fail-on-warning]
```

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `--root <ROOT>` | Workspace-Wurzel | Suche aufwärts vom aktuellen Verzeichnis |
| `--fail-on-warning` | Warnungen führen ebenfalls zu Exit 1 | aus |
| `-h`, `--help`, `-V`, `--version` | clap-Standard | |

Exit-Codes: 0, 1 Fehler gefunden (oder Warnungen mit `--fail-on-warning`), 2 interner Fehler. In CI: `cargo run -p zerodds-lint -- check` (`ci/full.yml`).

### zerodds-traceability

Erzeugt aus `docs/spec-coverage/*.md` eine Matrix Anforderung → Status, Code, Tests.

```
zerodds-traceability [--format text|md|json] [--status <s>] [<path>...]
```

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `<path>` | Coverage-Datei oder Verzeichnis; `.en.md` wird beim Verzeichnislauf übersprungen | `docs/spec-coverage` |
| `--format` | `text`, `md`, `json` | `text` |
| `--status <s>` | nur Zeilen, deren normalisierter Status mit `<s>` beginnt | |
| `-h`, `--help` | Hilfe | |

Exit-Codes 0, 2 Aufruffehler oder unlesbarer Pfad. Kein `--version`.

### zerodds-ros2-shim

Diagnosewerkzeug zu `rmw_zerodds_cpp`. Liest dieselbe Laufzeitkonfiguration wie die RMW-Bibliothek (`zerodds.yaml`, Drop-ins, `ros2`-Abschnitt) und zeigt Mangling, QoS-Profile, Discovery und Security.

```
zerodds-ros2-shim <SUBCOMMAND> [--config FILE] [--set PATH=VALUE]... [--check-config] [--domain ID]
```

| Unterbefehl | Wirkung |
|---|---|
| `info` | RMW-Compat-Level, aktives Mangling |
| `topics` | konfigurierte Topic-Overrides |
| `topic <ROS_TOPIC>` | DDS-Topic zu einem ROS-Topic |
| `qos <PROFILE>` | Profil aus `ros2.qos_profiles` (`default`, `sensor_data`, `services`, `parameters`, `parameter_events`, eigene) |
| `enclaves` | SROS2-Enclave (`security.enclave_dir`, Umgebung `ZERODDS_SECURITY_DIR`) |
| `validate <FILE>` | Datei als Laufzeitbasis mit `FILE.d/` prüfen |
| `selftest` | Round-Trip Mangling + QoS-Lookup |
| `catalog` | JSON-Katalog |
| `metrics` | Katalog der Prometheus-Metriknamen |
| `doctor` | RMW, Umgebung, Discovery, Security prüfen |
| `graph` | lokaler Teilnehmer- und Topic-Graph |

| Option | Bedeutung | Vorgabe |
|---|---|---|
| `--domain <ID>` | überschreibt `ROS_DOMAIN_ID`; nur Form mit Leerzeichen | `ROS_DOMAIN_ID`, sonst 0 |
| `--runtime-config`, `--log-level`, `--enclave` | werden mit Hinweis abgelehnt, Exit 1 (`REFUSED_FLAGS`) | |
| `-V`, `--version`; `-h`, `--help` | Version; Hilfe (auf stderr) | |

Exit-Codes: 0; 1 für jeden Fehler, auch ohne Unterbefehl.

## Bridge-Daemons

Gemeinsam: Konfigurationsdatei `<stem>.yaml` mit `<stem>.d/` im Systemverzeichnis, außer `--config` nennt eine andere; jede eigene Option ist `--set` ihres Schlüssels. Die systemd-Units starten jeden Daemon mit `--config /etc/zerodds/<stem>.yaml` (`packaging/linux/systemd/*.service`), die launchd-Plists mit `--config /usr/local/etc/zerodds/<stem>.yaml`. SIGTERM/SIGINT beenden, SIGHUP liest die Konfiguration neu (Umfang je Bridge in der Manpage). `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert einen OTLP-Export (`otlp_config_from_env`; aufgerufen in ws, mqtt, coap, amqp, corba; in `zerodds-grpc-bridged.rs` kein Aufruf gefunden).

Zwei Parser-Familien:

- `ws`, `mqtt`, `coap`: `daemon/cli.rs` je Crate; akzeptiert `--opt VALUE` und `--opt=VALUE`; Hilfe auf stdout.
- `amqp`, `grpc`, `corba`: Parser im Binary; akzeptiert nur `--opt VALUE`. `--opt=VALUE` führt zu `unknown argument` (gemessen: `--domain=3` → Exit 1 bei allen drei). Hilfe auf stderr.

### zerodds-ws-bridged

DDS ↔ WebSocket. Spezifikation `docs/specs/zerodds-ws-bridge-1.0.md`.

| Option | Schlüssel | Vorgabe |
|---|---|---|
| `--listen <ADDR>` | `listen` | `0.0.0.0:8080` |
| `--domain <ID>` | `domain` | 0 |
| `--topic <NAME>` | `+topics` | |
| `--tls-cert <FILE>` | `tls.cert_file`, `tls.enabled=true` | |
| `--tls-key <FILE>` | `tls.key_file`, `tls.enabled=true` | |
| `--auth-token <SECRET>` | `auth.mode=bearer`, `auth.bearer_token` | `auth.mode` = `none` |
| `--log-level <LEVEL>` | `log_level` | `info` |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | Admin-Endpunkt aus; aktiviert ohne Adresse: `127.0.0.1:9091` |
| `-V`, `--version`; `-h`, `--help` | | |

Exit-Codes (`main`): 0, 1 Konfiguration oder E/A, 2 Bind, 3 DDS, 4 TLS. `daemon/server.rs` baut rustls-Handshake, Bearer-Prüfung und Admin-Endpunkt auf (Code gelesen, nicht im Betrieb gemessen).

### zerodds-mqtt-bridged

DDS ↔ MQTT 5.

| Option | Schlüssel | Vorgabe (`default_for_dev`) |
|---|---|---|
| `--broker <URL>` (`mqtt://`, `mqtts://`, `ws://`, `wss://`) | `mqtt.broker_url` | `mqtt://127.0.0.1:1883` |
| `--client-id <ID>` | `mqtt.client_id` | `zerodds-bridge` |
| `--domain <ID>` | `domain` | 0 |
| `--username <USER>`, `--password <PASS>` | `mqtt.username`, `mqtt.password` | |
| `--tls-ca`, `--tls-cert`, `--tls-key <FILE>` | `mqtt.tls.ca_file`, `.cert_file`, `.key_file`; jede setzt `mqtt.tls.enabled=true` | |
| `--topic <DDS:MQTT>` | `+topics` | |
| `--log-level <LEVEL>` | `log_level` | `info` |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | |

Umgebung: `MQTT_PASSWORD` → `mqtt.password` (`config::ENV`). Exit-Codes: 0, 1 Konfiguration/E/A, 2 Broker-Verbindung, 3 DDS, 4 TLS, 5 Authentifizierung.

### zerodds-coap-bridged

DDS ↔ CoAP (RFC 7252) über UDP und DTLS.

| Option | Schlüssel | Vorgabe |
|---|---|---|
| `--bind <ADDR>` | `coap.bind` | `0.0.0.0:5683` |
| `--domain <ID>` | `domain` | 0 |
| `--dtls-cert <FILE>` + `--dtls-key <FILE>` (nur paarweise) | `coap.dtls.cert_file`, `.key_file`, `coap.dtls.enabled=true` | |
| `--dtls-psk-id <ID>` + `--dtls-psk <SECRET>` (nur paarweise, hex) | `+coap.dtls.psk=ID=SECRET`, `coap.dtls.enabled=true` | |
| `--topic <DDS:URI>` | `+topics` | |
| `--log-level <LEVEL>` | `log_level` | |
| `--metrics <ADDR>` | `metrics.listen`, `metrics.enabled=true` | |

Exit-Codes: 0, 1 Konfiguration/E/A, 2 Bind, 3 DDS, 4 DTLS und OSCORE (`ServerError::Oscore` → 4).

### zerodds-amqp-bridged

DDS ↔ AMQP 1.0.

| Option | Schlüssel |
|---|---|
| `--broker <URL>` (`amqp://`, `amqps://`), Pflicht (Datei oder Flag) | `amqp.broker_url` |
| `--container-id <ID>` | `amqp.container_id` |
| `--domain <ID>` | `domain` |
| `--sasl-mechanism <NAME>` (`PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2`) | `amqp.sasl.mechanism` |
| `--user`, `--password` | `amqp.sasl.username`, `amqp.sasl.password` |
| `--tls-ca`, `--tls-cert` | `amqp.tls.ca_file`, `.cert_file`, beide setzen `amqp.tls.enabled=true` |
| `--tls-key` | `amqp.tls.key_file` (setzt `enabled` nicht) |
| `--topic <DDS=ADDR>` | `+topics` |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` (ohne Adresse `127.0.0.1:9094`) |
| `--once` | Testhilfe: eine Verbindung bedienen, dann Ende |
| `--connect-timeout-ms <N>` | Testhilfe: Verbindungsaufbau aufgeben, Vorgabe 5000 |

Umgebung: `AMQP_PASSWORD` → `amqp.sasl.password`. Exit-Codes laut `DaemonError`-Doku in `crates/amqp-endpoint/src/bridge_daemon.rs`: 1 Konfiguration, 2 Verbindung, 3 DDS; der Code erzeugt genau diese drei. Hilfetext nennt keine Exit-Codes.

### zerodds-grpc-bridged

DDS ↔ gRPC über HTTP/2.

| Option | Schlüssel | Vorgabe |
|---|---|---|
| `--bind <ADDR>` | `grpc.bind` | `0.0.0.0:50051` |
| `--domain <ID>` | `domain` | 0 |
| `--tls-cert` + `--tls-key` (nur paarweise) | `grpc.tls.cert_file`, `.key_file`, `grpc.tls.enabled=true` | |
| `--tls-client-ca <FILE>` | `grpc.tls.client_ca_file` | |
| `--reflection` | `grpc.reflection.enabled=true` | aus |
| `--topic <DDS:SVC>` | `+topics` | |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` | |
| `--once` | Testhilfe: eine Anfrage, dann Ende | |
| `--idle-timeout-ms <N>` | Testhilfe: Ende nach N ms ohne Anfrage | 0 (aus) |

Exit-Codes: 0, 1 Konfiguration, 2 Bind/E/A, 3 DDS, 4 TLS.

### zerodds-corba-bridged

DDS ↔ CORBA GIOP/IIOP und SSLIOP.

| Option | Schlüssel | Vorgabe |
|---|---|---|
| `--iiop-bind <ADDR>` | `corba.iiop.bind` | `0.0.0.0:6833` |
| `--ssliop-bind <ADDR>` | `corba.ssliop.bind`, `corba.ssliop.enabled=true` | `0.0.0.0:6834` |
| `--domain <ID>` | `domain` | |
| `--naming-service <CORBANAME>` | `corba.ior.naming_service` (Registrierung nicht gebaut, laut Manpage) | |
| `--orb-id <ID>` | `corba.orb_id` | |
| `--tls-cert` + `--tls-key` (nur paarweise) | `corba.ssliop.cert_file`, `.key_file`, `corba.ssliop.enabled=true` | |
| `--topic <REPO_ID=TOPIC>` | `+mappings` | |
| `--dump-iors` | eine Zeile pro Mapping `repo_id operation IOR:…`, dann Ende; Host aus `corba.iiop.advertise_host` (Umgebung `ZERODDS_CORBA_ADVERTISE_HOST`) | `127.0.0.1` |
| `--log-level`, `--metrics` | `log_level`; `metrics.listen` + `metrics.enabled=true` | |
| `--idle-timeout-ms <N>` | Testhilfe: Stopp nach N ms | 0 (aus) |

Exit-Codes laut Manpage: 0, 1 Konfiguration, 2 Bind, 3 DDS, 4 TLS. Das Binary erzeugt selbst nur Code 1; `crates/corba-dds-bridge/src/bridge_daemon.rs` erzeugt 1, 2, 3 und 4 (Zuordnung zu Bind/DDS/TLS nicht einzeln nachverfolgt).

## Weitere Anwenderprogramme

| Programm | Zweck | Aufruf und Optionen | Exit-Codes |
|---|---|---|---|
| `zerodds-secure-permissions` | CMS/PKCS#7-Signatur und Prüfung von DDS-Security-Governance- und Permissions-XML | `sign --signer-cert <ca.pem> --signer-key <ca_key.pem> --in <doc.xml> --out <doc.p7s>`; `verify --ca <ca.pem> --p7s <doc.p7s> [--out <doc.xml>]`; Schlüssel muss PKCS#8 sein | 0, 1 Signatur-/Prüffehler, 2 Aufruffehler |
| `zerodds-chaos` | Netzstörungen für DDS-Tests | `proxy --listen A --forward A [--loss F] [--burst N] [--duplicate F] [--reorder F] [--jitter MS\|LO:HI] [--seed N] [--duration N(ms\|s)]`; `tc-apply --interface IF [--loss] [--delay] [--jitter] [--duplicate] [--reorder] [--correlation]` (root, netem); `tc-clear`; `partition --group-a … --group-b … [--duration]`; `partition-clear`; `endpoint-flap --interface IF [--interval] [--total] [--start-down]`; `--dev` ist Alias für `--interface` (im Hilfetext nicht genannt) | 1, 2 |
| `zerodds-perf` | Krypto- und Registry-Messungen | `hw-info`; `aes-gcm [--bytes N] [--block N] [--suite 128\|256]` (Vorgaben 100 MiB, 1 KiB, 128); `monitor`; `help` | |
| `zerodds-dashboard` | HTTP-Server mit SPA und JSON-Endpunkten | `[--bind ADDR:PORT]` (Vorgabe `127.0.0.1:8089`), `[--prometheus ADDR:PORT]` (aus), `[--demo]` | 0, 1 |
| `zerodds-recorder-bridge` | Live-Aufnahme nach `.zddsrec` mit einem Reader pro `NAME:TYPE` | `--out FILE --topic NAME:TYPE… [--domain N] [--duration N(ms\|s)]`, Domain-Vorgabe 0 | 0, 1, 2 |

## Interne Programme

Nicht paketiert. Zweck aus den `//!`-Kommentaren des jeweiligen `main`; Optionen nur grob aus dem Parser gelesen, nicht ausgeführt.

| Programm | Quelle | Zweck |
|---|---|---|
| `cargo-dag` | `tools/cargo-dag` | topologische Publish-Reihenfolge der Workspace-Crates; `[ROOT] [--only-publishable] [--format flat\|verbose\|json]` |
| `qos-matrix` | `tools/qos-matrix` | alle Kombinationen der QoS-Policy-Enums gegen `check_compatibility`; `--format markdown\|csv` |
| `interop-matrix` | `tools/interop-matrix` | JSON → HTML-Matrix; `--out`, `--fail-on-red` (Exit 1 bei roten Zellen) |
| `interop-results` | `tools/interop-results` | Ergebnisbaum `tests/interop-results/` und Tabellen; `--regenerate`, `--verify`, `--tally`, `--dialects`, weitere |
| `isolation-smoke` | `tools/isolation-smoke` | Sender/Empfänger über UDP oder UDS für `testing/isolation/`; `--transport= --role= --local= --peer= --count=`; Exit 0–4 |
| `amqp-dds-endpoint` | `tools/amqp-dds-endpoint` | AMQP-1.0-Server mit XML-Konfiguration; `--config`, `--listen` (Vorgabe `0.0.0.0:5672`), `--hash-password`; Feature `tls` |
| `iceoryx2-oracle-consumer` | `tools/iceoryx2-oracle-consumer` | externer iceoryx2-Subscriber ohne ZeroDDS-Abhängigkeit, prüft die Bytes |
| `shape_main` | `tools/omg-shape-main` | Referenzclient für die OMG-Suite `dds-rtps` |
| `shapetype_main` | `tools/omg-security-main` | Referenzclient für die OMG-Suite `dds-security` |
| `xtypes_main` | `tools/omg-xtypes-main` | Referenzclient für die OMG-Suite `dds-xtypes` |
| `zerodds_test_main` | `tools/omg-xtypes-test-main` | Nachbau von `test_main.cxx` der XTypes-Suite; Kurzoptionen `-P -S -t -y -d -r -b -p …` |
| `regression`, `roundtrip-1us`, `roundtrip-typed` | `tools/bench-suite/src/bin` | Roundtrip-Regressionsgate (CI) und Latenzmessungen |
| `wireprobe-zerodds` | `crates/wire-probe/src/main.rs` | ein IDL-Typ, ein JSON-Wert, die Bytes; gleiche Optionen wie die Vendor-Probes |
| `wireprobe-compare`, `wireprobe-corba-idl`, `wireprobe-dump`, `wireprobe-fill`, `wireprobe-ros-package` | `crates/wire-probe/src/bin` | Hilfsprogramme der Probe-Kette (automatisch erkannte Binaries, kein `[[bin]]`) |
| `zerodds-cyclone-iox-reader`, `zerodds-cyclone-iox-writer` | `crates/iceoryx-cyclone/src/bin` | iceoryx-(POSH-)SHM-Nachweis gegen Cyclone DDS; Feature `posh` |
| `proof-cyclone-dcps` | `crates/iceoryx-cyclone/proofs/dcps-interop` | typisierter DCPS-Nachweis über iceoryx gegen Cyclone |
| `bench_features`, `echo_bench`, `echo_bench_codegen`, `interop_client`, `interop_server`, `ssliop_bench`, `ssliop_client`, `ssliop_server` | `crates/corba-interop/src/bin` | CORBA-Latenzmessungen und Cross-ORB-Interop (omniORB/TAO/JacORB) |
| `peer` | `crates/endpoint-e2e/src/bin/peer.rs` | UDP-Gegenstelle für die Endpoint-Beispiele |
| `zerodds-dashboard-tauri` | `tools/dashboard-tauri/src-tauri` | Tauri-2.0-Hülle um `zerodds-dashboard` |
| (probe) | `tools/xtypes-1.3-gap-scan/probe` | liest `id op arg` von stdin, schreibt Messwerte für `scan.sh` |
| `wireprobe-endpoint-gen`, `wireprobe-c-matrix`, `wireprobe-cpp-matrix`, `wireprobe-python-matrix`, `wireprobe-zerodds-<lang>-gen` (20 Sprachen: ada, ada83, c, cpp, csharp, d, elixir, fsharp, go, java, julia, kotlin, lua, nim, node, ocaml, python, swift, typescript, zig) | `tools/wireprobe/*-gen` | Generatoren für die Sprach-Probes des Interop-Korpus |

Beispielprogramme unter `examples/` und `crates/*/examples/` sind nicht aufgeführt.

## Abweichungen zwischen Hilfetext, Code und Manpage

Keine offen. Gemessen mit den Binaries in `target/debug` nach `fe3b0ca185c`
und `tools/man-gate --list` (0 undocumented, 0 documented and gone, 0 with
different exit codes): Die Manpages nennen `--config`, `--set` und
`--check-config` und die Schreibweise `--opt VALUE`; die Bridges nennen ihre
Umgebungsvariablen und Exit-Codes; `--version` antwortet in `zerodds-admin`,
`-router`, `-replay`, `-xmlc`, `-durability-svc`, `-spy`, `-chaos`,
`-ros2-shim` mit Exit 0; `zerodds-router` gibt bei einem Aufruffehler Exit 2,
wie seine Manpage sagt. Ohne Manpage bleiben die Werkzeuge, die
[tools-and-man.md](tools-and-man.md) aufzählt.
