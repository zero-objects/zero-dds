# Werkzeuge, Manpages und Paketierung

Diese Seite beschreibt (a) jeden Eintrag im Verzeichnis `tools/` mit Zweck, Aufruf und Einordnung als Anwenderwerkzeug, CI-Gate oder Test-/Messumgebung, (b) die Manpages unter `man/` mit dem jeweils dokumentierten Programm, den Programmen ohne Manpage, der Anzeige und Installation sowie den festgestellten Abweichungen zum Programm, (c) das Verzeichnis `packaging/`. Die Optionen der Programme selbst stehen in [cli-options.md](cli-options.md). Die Einordnung „CI-Gate“ stützt sich auf Fundstellen in `ci/*.yml`, `.github/workflows/ci.yml`, `scripts/pre-commit.sh` und `ci/*.py`; ein Eintrag ohne Fundstelle ist als manuell markiert.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Kurzfassung

- `tools/` hat 61 Einträge: 31 Rust-Crates, 11 einzelne Skripte bzw. Dateien, 19 Verzeichnisse mit Skripten, Fällen und Vorlagen.
- CI-Gates mit Fundstelle: `silent-drop-guard.sh`, `staged-tree-guard.sh`, `man-gate`, `endpoints-gate`, `byte-gate/verify.sh`, `interop-results` (`--verify`, `verify-record.sh`), `wireprobe/three-column-gate`, `pairwise-run`, `bench-suite` (`regression`), `zerodds-lint`.
- `man/man1` enthält 20 Seiten, `man/man5` 8 Seiten. Sieben installierte Programme haben keine Manpage (`zerodds-lint`, `-traceability`, `-secure-permissions`, `-chaos`, `-perf`, `-dashboard`, `-recorder-bridge`).
- `tools/man-gate --list` meldet 0 Abweichungen (Optionsnamen und Exit-Codes aller Seiten gegen die Programme).

## (a) Das Verzeichnis `tools/`

Einordnung: **A** = Anwenderwerkzeug (paketiert oder für Nutzer gedacht), **G** = CI-Gate, **T** = Test-, Interop- oder Messumgebung (lokal oder auf dem Messhost), **B** = Build-/Release-Hilfe, **L** = Bibliothek.

### Rust-Crates

| Eintrag | Binary | Art | Zweck | Aufruf |
|---|---|---|---|---|
| `admin` | `zerodds-admin` | A | Operator-CLI: Domain live untersuchen, DDS-XML offline prüfen | `zerodds-admin domain inspect 0` |
| `amqp-dds-endpoint` | `amqp-dds-endpoint` | T | AMQP-1.0-Server nach Spec §9.2, XML-Konfiguration | `amqp-dds-endpoint [--config F] [--listen A]` |
| `bench` | `zerodds-bench` | A | Latenz/Durchsatz im Prozess | `zerodds-bench latency` |
| `bench-suite` | `regression`, `roundtrip-1us`, `roundtrip-typed` | G/T | `regression`: prozessübergreifendes Roundtrip-Gate ohne externe Hosts (CI-Job `bench-regression`, `ci/full.yml`); die beiden anderen: Latenzvergleich mit Vendor-Apps | `cargo run -p zerodds-bench-suite --bin regression` |
| `cargo-dag` | `cargo-dag` | B | Publish-Reihenfolge der Workspace-Crates | `cargo-dag . --only-publishable` |
| `chaos` | `zerodds-chaos` | A | UDP-Störproxy, netem, Partitionen, Link-Flaps | `zerodds-chaos proxy --listen … --forward …` |
| `cli-common` | — | L | gemeinsame CLI-Hilfen (`parse_duration`, Signal-Handler, GUID-Präfix) für record/bench/monitor/spy/snitch/pcap/mq | |
| `dashboard` | `zerodds-dashboard` | A | HTTP-Dashboard (SPA + JSON), `--demo` mit synthetischen Daten | `zerodds-dashboard --bind 127.0.0.1:8089` |
| `dashboard-tauri` | `zerodds-dashboard-tauri` | A | Tauri-2.0-Hülle um `dashboard` (`.dmg`, `.AppImage`, `.msi`) | siehe `tools/dashboard-tauri/README.md` |
| `iceoryx2-oracle-consumer` | `iceoryx2-oracle-consumer` | T | iceoryx2-Subscriber ohne ZeroDDS-Abhängigkeit; prüft, dass ZeroDDS echtes iceoryx2 schreibt | neben einem ZeroDDS-Publisher starten |
| `idlc` | `zerodds-idlc` | A | IDL-4.2-Prüfer, Formatierer, Codegenerator (21 Sprachen); `tests/` enthält CI-Skripte (`csharp-support`, `java-typelookup`) | `zerodds-idlc check a.idl` |
| `interop-matrix` | `interop-matrix` | T | JSON → HTML-Matrix, `--fail-on-red` | `interop-matrix in.json --out m.html` |
| `interop-results` | `interop-results` + Skripte | G | Ergebnisbaum `tests/interop-results/`, Ledger und Tabellen; `--verify` ist der CI-Befehl (GitHub `ci.yml`, `ci/full.yml` Jobs `interop-results`, `interop-results-record`); Skripte `record.sh`, `verify-record.sh`, `regenerate.sh`, `*-sweep.sh` | `cargo run -p zerodds-interop-results --bin interop-results -- --verify` |
| `isolation-smoke` | `isolation-smoke` | T | Sender/Empfänger für `testing/isolation/` (UDP, UDS) | `isolation-smoke --transport=udp --role=receiver …` |
| `monitor` | `zerodds-monitor` | A | Metrik-Snapshot, `/metrics`-Server | `zerodds-monitor serve` |
| `mq` | `zerodds-mq` | A | Rohsamples zwischen zwei Domains | `zerodds-mq -t T --src-domain 0 --dst-domain 1` |
| `omg-security-main` | `shapetype_main` | T | Referenzclient für die OMG-Suite `dds-security`; `run_matrix.py`, `compatibility_matrix.tsv` | siehe README |
| `omg-shape-main` | `shape_main` | T | Referenzclient für die OMG-Suite `dds-rtps`; `run_vendors.sh` | siehe README |
| `omg-xtypes-main` | `xtypes_main` | T | Referenzclient für die OMG-Suite `dds-xtypes`; `run_matrix.py` | siehe README |
| `omg-xtypes-test-main` | `zerodds_test_main` | T | Nachbau von `test_main.cxx` der XTypes-Suite; `run_configs.sh` | Kurzoptionen `-P/-S -t -y …` |
| `pcap` | `zerodds-pcap` | A | RTPS-Dekoder für pcap-Dateien | `zerodds-pcap stats cap.pcap` |
| `perf` | `zerodds-perf` | A | AES-GCM-Durchsatz, CPU-Merkmale, Registry als Prometheus-Text | `zerodds-perf aes-gcm` |
| `qos-matrix` | `qos-matrix` | T | QoS-Kompatibilitätstabelle aus `check_compatibility` | `qos-matrix --format csv` |
| `record` | `zerodds-record` | A | Aufnahme nach `.zddsrec`, optional dekodiert | `zerodds-record record -t T` |
| `recorder-bridge` | `zerodds-recorder-bridge` | A | Aufnahme mit typisierten Readern `NAME:TYPE` | `zerodds-recorder-bridge --out f --topic T:Ty` |
| `replay` | `zerodds-replay` | A | Wiedergabe von `.zddsrec`/NDJSON/SQLite | `zerodds-replay replay f.zddsrec --inject` |
| `secure-permissions` | `zerodds-secure-permissions` | A | CMS-Signatur von Governance/Permissions | `… sign --signer-cert … --in … --out …` |
| `snitch` | `zerodds-snitch` | A | SPDP/SEDP-Probe | `zerodds-snitch -f json` |
| `spy` | `zerodds-spy` | A | Topic mitlesen | `zerodds-spy -t T` |
| `traceability` | `zerodds-traceability` | A | Anforderung→Code-Matrix aus `docs/spec-coverage` | `zerodds-traceability --format md` |
| `xmlc` | `zerodds-xmlc` | A | DDS-XML prüfen und darstellen | `zerodds-xmlc validate d.xml` |

Die Unterverzeichnisse `tools/wireprobe/*-gen` (22 Crates, Generatoren je Sprache) sind unter `wireprobe` beschrieben.

### Einzelne Skripte und Dateien

| Eintrag | Art | Zweck | Aufruf / Einbindung |
|---|---|---|---|
| `annotate-snippets.js` | B | ordnet Code-Blöcken der Website passende Demo-Verzeichnisse zu | manuell (in `scripts/mirror-exclude.txt` gelistet) |
| `dcps-shapes-interop.sh` | T | ZeroDDS ↔ Fremd-DDS, ein Shape-Topic, beide Richtungen, vier Stacks; Fall gilt nur bei Empfängerzählung > 0 | `./tools/dcps-shapes-interop.sh [B D …]`; vorher `cargo build -p zerodds-dcps --example omg_shapes --example classic_shapes` |
| `endpoints-gate` | G | baut und testet die committeten Suiten aller Bindings unter `endpoints/` | `tools/endpoints-gate [--langs go,zig] [--list] [--require …]`; CI: Job `endpoints-gate` in `ci/full.yml`, `ci/run_merge_scope.py` |
| `interop-env.sh` | T | Umgebung der Vendor-Toolchains für die Probes (ersetzt das verlorene `/root/tc-env.sh`); fehlende Toolchain → Variable ungesetzt, eine Zeile Meldung | `. tools/interop-env.sh` |
| `interop-gate` | T | alle Interop-Tests in einem Befehl, ein Ergebnis pro Test; gedacht für Zeiten ohne CI-Runner | `tools/interop-gate [--tests a,b] [--require …] [--list]`; keine CI-Fundstelle |
| `interop-vendors.sh` | T | Orte der Fremd-Toolchains als Umgebungsvariablen mit Messhost-Vorgabe; fehlende Toolchain wird mit Zeilenzahl gemeldet, nie still übersprungen | von den Interop-Harnesses in `tools/` gesourct |
| `man-gate` | G | vergleicht Langoptionen und Exit-Codes aus `--help` der gebauten Binaries (`target/debug`) mit `man/man1/*.1` | `tools/man-gate` (Exit 1 bei Differenz, 2 wenn ein Binary fehlt), `--list`; CI: Job `man-pages` in `ci/full.yml` (wöchentliche Regeln) |
| `ros-extensibility-scan.sh` | T | prüft, ob ein ROS-2-Typ eine Extensibility deklariert (Grundlage für FINAL im RMW); Exit ≠ 0 bei Fund | manuell |
| `silent-drop-allowlist.tsv` | G | Allowlist mit Begründung pro Fundstelle für `silent-drop-guard.sh` (Schlüssel: Muster, Datei, Zeilentext) | Daten |
| `silent-drop-guard.sh` | G | sucht elf Muster stiller Datenverluste, schlägt bei jeder Fundstelle außerhalb der Allowlist fehl | `bash tools/silent-drop-guard.sh`; `scripts/pre-commit.sh`, `.github/workflows/ci.yml`, `ci/full.yml` |
| `staged-tree-guard.sh` | G | baut den Index-Stand statt des Arbeitsverzeichnisses | `bash tools/staged-tree-guard.sh [--head]`; `scripts/pre-commit.sh`, `.github/workflows/ci.yml` (`--head`), `ci/full.yml` |

### Verzeichnisse mit Skripten, Fällen und Vorlagen

| Eintrag | Art | Zweck | Aufruf |
|---|---|---|---|
| `byte-gate/` | G/T | Bytes der Vendor-Serialisierer für einen Typ und ein Sample, XCDR1/XCDR2 × LE/BE; Ergebnis `crates/ast-layout/tests/fixtures/vendor_bytes.md` und `.rows.tsv` | `tools/byte-gate/gate.sh`; CI: `bash tools/byte-gate/verify.sh` (Job `interop-vendor-bytes`, `ci/full.yml`) |
| `corba-oracle/` | T | ORBs lesen von ZeroDDS geschriebene GIOP-Bytes (`corba-read.sh`), Sweep (`corba-sweep.sh`), `targeted/` | `tools/corba-oracle/corba-read.sh read.tsv` |
| `dds-roundtrip-codegen/` | — | leeres Verzeichnis | |
| `evolution-gate/` | T | Typ-Evolution: alter Typ schreibt, neuer liest und umgekehrt (`evo.sh`, `cases/`, `smoke/`) | `tools/evolution-gate/evo.sh …`; keine CI-Fundstelle |
| `hashid-gate/` | T | welchen Namen `@hashid` hasht, über vier Implementierungen | `tools/hashid-gate/gate.sh [--cases …]`; keine CI-Fundstelle |
| `idl-4.2-gap-scan/` | T | misst die IDL-4.2-Lücke gegen `expected.tsv`; Ergebnis `docs/spec-coverage/idl-4.2.gaps.md` | `scan.sh`, `--record`, `--report`, `IDLC=…` |
| `inventory-probes/` | T | Einzelsonden zu `docs/wire-feature-inventory.md` (keyholder, bound-overrun, encapsulation-be, parameter-edges …) | `tools/inventory-probes/run.sh [name]` |
| `keyhash-probe/` | T | KeyHash je Vendor für rund 60 Typen (`keyhash.idl`, RTPS §9.6.4.8) | `run-all.sh [vendor]`, `run-wire.sh` |
| `measurements/` | T | Transkripte von Messläufen, die in Dokumenten zitiert werden; laut README nicht gepflegt | `m1.sh`, `m1b.sh`, `m1c.sh`, `final-record.sh` |
| `optional-bits/` | T | dritter Byte-Gate-Fallsatz: Füllbytes und `@optional` in Schlüsseltypen | `gate.sh --cases pad2` (Optionen wie `byte-gate/gate.sh`) |
| `pairwise-run/` | G/T | gesamte Lese-Matrix über vier Kombinationen, eine TSV pro Kombination | `run.sh`, `verify.sh`, `combos.py`, `summarise.py`; CI: `ci/pairwise_shards.py` |
| `struct-compare/` | T | erzeugte Struktur eines Korpusfalls in allen Sprachen nebeneinander, zur Durchsicht | `collect.sh <case> > review.md` |
| `try-construct-gate/` | T | schmalerer Reader-Typ liest breitere Bytes, XTypes 1.3 §7.2.2.7 | `gate.sh`; keine CI-Fundstelle (nur in einem Kommentar von `scripts/cpd-gate.sh`) |
| `typeobject-bytes/` | T | serialisierte `TypeObject`-Bytes dreier Vendoren gegen ZeroDDS | `record.sh` |
| `unspecified-reach/` | T | vier Harnesses zu Konstrukten, die der Standard nicht festlegt; Ergebnis `crates/ast-layout/tests/fixtures/unspecified_constructs.md` | `bitset-bytes.sh`, `fixed-bytes.sh`, `front-ends.sh`, `map-length.sh` |
| `wire-capture/` | T | zweite Messebene: was Implementierungen auf den Socket legen (Fill-Lauf, Korpuslauf mit 107 Fällen, 9 Implementierungen laut README) | `capture-rtps-session.sh`, `check-run.py`, `decode-rtps.py`, `decode-giop.py` |
| `wire-oracle/` | T | zweiter Byte-Gate-Fallsatz und ORB-Einzelfälle (JacORB, omniORB) | `gate.sh --cases …`, `fixed-orbs.sh` |
| `wireprobe/` | G/T | Probe-Sammlung für den Interop-Korpus: Vendor-Probes (`wireprobe-cyclone`, `-fastdds`, `-rti`, `-opendds`, `-hdds`, `-ros`, `-tao`, `-omniorb`, `-jacorb`), ZeroDDS-Sprachprobes (`wireprobe-zerodds-<lang>` + `-gen`-Crates), Endpoint-Probes (`wireprobe-endpoint-*`), Vorlagen (`*-template.*`), Treiber (`wireprobe-batch`, `wireprobe-pairwise`, `wireprobe-bytes`) und Gates (`three-column-gate`, `endpoint-mutation-gate`); kein README | CI: `bash tools/wireprobe/three-column-gate --full` (Job `three-column-gate-full`, `ci/full.yml`), `ci/zd-test.sh` |
| `xtypes-1.3-gap-scan/` | T | misst die XTypes-1.3-Lücke; `probe/` ist ein Rust-Programm (stdin → Messwerte) | `roster.py [--check]`, `scan.sh [--record\|--report]` |

## (b) Manpages

### Bestand

| Datei | Gegenstand (Abschnitt NAME) |
|---|---|
| `man/man1/zerodds-admin.1` | ZeroDDS operator CLI for live domains and DDS-XML configs |
| `man/man1/zerodds-amqp-bridged.1` | DDS to AMQP 1.0 bridge daemon |
| `man/man1/zerodds-bench.1` | end-to-end latency and throughput benchmarking on the ZeroDDS DCPS pipeline |
| `man/man1/zerodds-coap-bridged.1` | DDS to CoAP bridge daemon |
| `man/man1/zerodds-corba-bridged.1` | DDS to CORBA GIOP/IIOP bridge daemon |
| `man/man1/zerodds-durability-svc.1` | ZeroDDS Durability-Service daemon |
| `man/man1/zerodds-grpc-bridged.1` | DDS to gRPC bridge daemon |
| `man/man1/zerodds-idlc.1` | read OMG IDL 4.2 and say what is in it |
| `man/man1/zerodds-monitor.1` | live snapshot or Prometheus /metrics server for the ZeroDDS runtime |
| `man/man1/zerodds-mq.1` | cross-domain DDS bridge |
| `man/man1/zerodds-mqtt-bridged.1` | DDS to MQTT 5 bridge daemon |
| `man/man1/zerodds-pcap.1` | offline RTPS submessage decoder for pcap files |
| `man/man1/zerodds-record.1` | capture and inspect ZeroDDS .zddsrec files |
| `man/man1/zerodds-replay.1` | replay recorded ZeroDDS captures to stdout or a live domain |
| `man/man1/zerodds-ros2-shim.1` | ROS-2 RMW shim and diagnostics |
| `man/man1/zerodds-router.1` | DDS routing-service daemon |
| `man/man1/zerodds-snitch.1` | DDS discovery probe (SPDP / SEDP snapshot) |
| `man/man1/zerodds-spy.1` | subscribe to a DDS topic and dump samples |
| `man/man1/zerodds-ws-bridged.1` | DDS to WebSocket bridge daemon |
| `man/man1/zerodds-xmlc.1` | DDS-XML validator, deployment renderer, type lister |
| `man/man5/zerodds-amqp-bridged.yaml.5` | configuration file for zerodds-amqp-bridged(1) |
| `man/man5/zerodds-coap-bridged.yaml.5` | configuration file for zerodds-coap-bridged(1) |
| `man/man5/zerodds-corba-bridged.yaml.5` | configuration file for zerodds-corba-bridged(1) |
| `man/man5/zerodds-grpc-bridged.yaml.5` | configuration file for zerodds-grpc-bridged(1) |
| `man/man5/zerodds-mqtt-bridged.yaml.5` | configuration file for zerodds-mqtt-bridged(1) |
| `man/man5/zerodds-ros2.yaml.5` | the ros2 section of the ZeroDDS runtime configuration |
| `man/man5/zerodds-ws-bridged.yaml.5` | configuration file for zerodds-ws-bridged(1) |
| `man/man5/zerodds.yaml.5` | the ZeroDDS runtime configuration |

Keine Seite trägt eine Version im Kopf (`.TH … "" "ZeroDDS"`); die Version nennt `--version`.

Ohne Manpage:

- installierte Programme: `zerodds-lint`, `zerodds-traceability`, `zerodds-secure-permissions`, `zerodds-chaos`, `zerodds-perf`, `zerodds-dashboard`, `zerodds-recorder-bridge`;
- Dateiformat: keine man5-Seite für die Routendatei des Routers.

Die Seiten verweisen auf die Spezifikationen als Datei (`docs/specs/<name>.md`), nicht als Handbuchseite; die externen Seiten `mosquitto(8)` und `launchd(8)` bleiben Verweise.

### Anzeigen

| System | Befehl |
|---|---|
| Linux (man-db) | `man -l man/man1/zerodds-spy.1` |
| macOS | `mandoc man/man1/zerodds-spy.1 \| less` oder `man ./man/man1/zerodds-spy.1` |
| installiert | `man zerodds-spy`, `man 5 zerodds-ws-bridged.yaml` |

`mandoc -T lint` meldet auf keiner Seite eine Warnung außer dem leeren Datum im `.TH` (gemessen). `website/_tools/render_man_pages.py` erzeugt aus `man/man1/*.1` die HTML-Seiten unter `website/topics/man-pages/`.

### Installation

| Paket | Wie |
|---|---|
| rpm (`packaging/linux/rpm/zerodds.spec`) | kopiert alle `man/man1/*.1` und `man/man5/*.5` nach `%{_mandir}`; `%files` ordnet sie den Unterpaketen zu (`cli`, je Bridge, `ros2`, `router`) |
| deb (`packaging/linux/deb/assemble-debs.sh`) | `zerodds-core`: `zerodds.yaml.5`; `zerodds-cli`: `man/man1/zerodds-$tool.1`, falls vorhanden; Bridges: `.1` und `.yaml.5`; `zerodds-ros2`: `zerodds-ros2-shim.1`, `zerodds-ros2.yaml.5`; `zerodds-router`: `zerodds-router.1`; `zerodds-durability`: `zerodds-durability-svc.1` |
| Arch (`packaging/linux/arch/PKGBUILD`) | alle `man1/*.1`, `man5/*.5` nach `/usr/share/man` |
| Homebrew (`packaging/macos/homebrew/zerodds.rb`) | `man1.install Dir["share/man/man1/zerodds-*.1"]`, `man5.install Dir["share/man/man5/zerodds*.yaml.5"]` |
| macOS-pkg (`packaging/macos/pkg/build-pkg.sh`) | kopiert nach `/usr/local/share/man/man1` und `man5` |

### Abweichungen Manpage ↔ Programm

Keine offen. `tools/man-gate --list` vergleicht Optionsnamen und Exit-Codes aller Seiten mit den Programmen und meldet 0 Abweichungen; Einzelheiten in [cli-options.md](cli-options.md#abweichungen-zwischen-hilfetext-code-und-manpage).

## (c) `packaging/`

| Pfad | Inhalt |
|---|---|
| `linux/deb/` | `assemble-debs.sh` baut die Unterpakete (`zerodds-core`, `-cli`, `-dev`, je Bridge, `-ros2`, `-router`) aus `target/<triple>/release`; `build-rmw-deb.sh` baut `ros-<distro>-rmw-zerodds-cpp` aus dem ament-Projekt (Parameter `--distro`, Standard jazzy); `control.tmpl`, `postinst.tmpl`, `prerm.tmpl`, `postrm.tmpl`; `publish-deb.yml` ist der cargo-dist-Publish-Job |
| `linux/rpm/` | `zerodds.spec` (Unterpakete `cli`, `devel`, je Bridge, `ros2`, `router`; baut ohne `zerodds-durability-service-bin`), `zerodds.pc` (pkg-config) |
| `linux/arch/` | `PKGBUILD` |
| `linux/appimage/` | `build.sh` (musl-static AppImage pro Daemon/CLI, braucht `appimagetool`), `AppRun.template` |
| `linux/systemd/` | Units für die sechs Bridges, `zerodds-router.service`, `zerodds-durability-svc.service`; `zerodds-sysusers.conf`, `zerodds-tmpfiles.conf` |
| `linux/configs/` | Beispielkonfigurationen `*-bridged.yaml.example` (sechs Bridges), `ros2.yaml.example`, `router.json.example` |
| `macos/homebrew/` | `zerodds.rb` (alle Binaries, Manpages, Dienst `zerodds-ws-bridged`), `zerodds-ros2.rb`, je eine reine Service-Formel pro Bridge (kein eigenes Archiv; das Binary kommt aus `zerodds`) |
| `macos/bundle/` | `build-bundle.sh` packt `zerodds-<v>-<target>.tar.gz` (das Layout, das `zerodds.rb` installiert) plus `.sha256`; aufgerufen vom Job `macos-bundle` (`ci/full.yml`) |
| `macos/launchd/` | `org.zerodds.<bridge>-bridged.plist` für die sechs Bridges, jeweils `--config /usr/local/etc/zerodds/<stem>.yaml` |
| `macos/pkg/` | `build-pkg.sh` baut ein `.pkg` mit Binaries und Manpages unter `/usr/local` |
| `windows/msi/` | `zerodds.wxs` (WiX; `bin`, `lib`, `include`, PATH-Eintrag) |
| `windows/chocolatey/` | `zerodds.nuspec`, `packaging/windows/chocolatey/tools/chocolateyinstall.ps1`, `chocolateyuninstall.ps1` |
| `windows/scoop/` | `zerodds.json` (Release-ZIPs x86_64 und aarch64) |
| `windows/services/` | `Install-Services.ps1`, `Uninstall-Services.ps1` für die Dienste `ZeroDDSWSBridge`, `ZeroDDSMQTTBridge`, `ZeroDDSCoAPBridge`, `ZeroDDSAMQPBridge`, `ZeroDDSGrpcBridge`, `ZeroDDSCORBABridge` |
| `docker/` | Dockerfiles je Bridge (`*-bridged/`), `durability-svc/`, `ros2-shim/`, `cli/` (18 CLI-Binaries), Laufzeit-Images `py-runtime/`, `cpp-runtime/`, `ts-node-runtime/` (je mit README); `docker-compose.yml` mit den Bridges, `cli`, `mosquitto`, `rabbitmq` |
| `python/` | `sync-version.py` gleicht die Versionen der Python-Distributionen mit `[workspace.package].version` ab (`--check`) |
| `github-actions/` | `sign-artifacts.sh` (minisign, cosign), `sign-macos.sh` (codesign, Notarisierung, `.pkg`), `render-homebrew.sh` (SHA-256 in die Formeln, Kopie in den Tap), `aur-publish.sh` (PKGBUILD/.SRCINFO für `zerodds-bin` und `zerodds`) |
| `keys/` | `minisign.pub` |

Hinweise aus dem Abgleich:

- deb `zerodds-cli` installiert `zerodds-secure-permissions` nicht, rpm `zerodds-cli` schon.
- Die systemd-Unit des Durability-Dienstes wird im rpm übersprungen (`zerodds.spec`, Kommentar zu libduckdb); ein Paket für das Binary gibt es nicht.
