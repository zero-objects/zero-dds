# Warum ZeroDDS für ROS 2

*Ein Pure-Rust-DDS mit voller Spec-Konformität, das die Gründe behebt, aus denen Leute DDS verlassen — ohne die RTPS-Interop aufzugeben.*

---

## Die Kurzfassung

ROS 2 läuft auf DDS. DDS ist ein guter Standard mit harter Realität: die Default-Konfiguration flutet Netze, bricht auf WiFi, scheitert still bei QoS-Mismatches, droppt große Nachrichten und braucht Experten-XML-Tuning, bevor es funktioniert. Das sind keine seltenen Edge-Cases — es sind die **meistgemeldeten, jüngsten** Probleme der ROS-Community, und der Grund, warum das Projekt 2023 eine alternative Middleware (Zenoh) adoptierte.

**ZeroDDS ist ein from-scratch Pure-Rust-DDS, das auf dem Draht bleibt (natives RTPS 2.5, interoperabel mit Fast DDS / Cyclone DDS / OpenDDS / Connext), aber die strukturellen Ursachen dieser Fehler beseitigt.** Es spricht die ROS-2-Middleware-ABI (`rmw_zerodds`) und ist damit eine drop-in `RMW_IMPLEMENTATION`, kein Fork von ROS.

Dieser Trail leistet drei Dinge, je Schmerz-Cluster:

1. **Beschreibt den Schmerz** — fundiert auf einem frischen Field-Scan von **349 echten Reports** (GitHub-Issues, ROS Discourse, Stack Exchange, Vendor-Blogs; siehe [`../ros2-dds-painpoints-research-2026-06.md`](../ros2-dds-painpoints-research-2026-06.md)).
2. **Zitiert das jüngste Ticket** als konkretes, prüfbares Beispiel.
3. **Erklärt, wie ZeroDDS ihn beseitigt** — und wie **du den Fix selbst reproduzieren** kannst, aus den offenen Harnessen.

> **Für Open-Source-Validatoren:** jede Performance- und Interop-Aussage unten kommt mit dem Befehl, der sie erzeugt hat. Wir *wollen*, dass du sie fährst, brichst und meldest, was du findest. Diese Seite ist ein Satz falsifizierbarer Aussagen, keine Broschüre.

---

## Warum das mehr zählt, als es aussieht

ROS 2 ist der De-facto-Standard für moderne Robotik-F&E und ein wachsender Anteil der Produktions-Robotik. Der Schmerz ist nicht theoretisch — er ist eine tägliche Steuer, gemessen in verlorenen Labor-Nachmittagen, abgestürzten Demos und „warum reden meine Nodes nicht"-Threads. Ein Scan des Feldes (2016–2026, neueste dominierend) gliedert sich so:

| Schmerz-Cluster | Reports | Jüngstes Beispiel |
|---|---|---|
| [Discovery](discovery.md) — Multicast-SDP, Discovery-Stürme, Nodes nicht gefunden | 62 | [Fast-DDS#6401](https://github.com/eProsima/Fast-DDS/issues/6401) (2026-05-18) |
| [Shared Memory](shared-memory.md) — Iceoryx/SHM-Segfaults, `/dev/shm`, Same-Host scheitert | 52 | [rmw_cyclonedds#585](https://github.com/ros2/rmw_cyclonedds/issues/585) (2026-06-02) |
| [QoS stilles No-Match](qos-silent-fail.md) — inkompatible QoS → keine Daten, kein Fehler | 36 | [ros2#1562](https://github.com/ros2/ros2/issues/1562) (2024-05-10) |
| [Multicast / WiFi](multicast-wifi.md) — blockiert, flutet, Aussetzer | 34 | [turtlebot4#673](https://github.com/turtlebot/turtlebot4/issues/673) (2026-02-04) |
| [Cross-Vendor- / Inter-Distro-Interop](interop.md) | 32 | [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577) (2026-04-02) |
| [Large-Data / Fragmentierung](large-data.md) — Bilder, Punktwolken, 262-kB-Decke | 29 | [Fast-DDS#5686](https://github.com/eProsima/Fast-DDS/issues/5686) (2025-03-05) |
| [DDS-Security / SROS2](security.md) | 22 | [Fast-DDS#5753](https://github.com/eProsima/Fast-DDS/issues/5753) (2025-04-08) |
| [Konfigurations-Komplexität](config-complexity.md) — XML-Tuning, versteckte Voraussetzungen | 21 | [Discourse „I'm done tuning DDS"](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415) (2026-04-30) |
| [Docker / Kubernetes / Cloud](docker-cloud.md) | 19 | [IsaacSim#407](https://github.com/isaac-sim/IsaacSim/issues/407) (2026-01-09) |
| [Performance / Latenz / CPU](performance.md) | 19 | [rmw_cyclonedds#559](https://github.com/ros2/rmw_cyclonedds/issues/559) (2026-03-03) |
| [Scaling / Flotten / viele Nodes](scaling.md) | 16 | [autoware#6759](https://github.com/autowarefoundation/autoware/issues/6759) (2026-01-24) |
| [Migration zu alternativer Middleware](migration.md) | 7 | [Alternative Middleware Report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771) (2023-09-27) |

Jede Zeile verlinkt auf eine Seite mit derselben Form: **der Schmerz → das jüngste Ticket → wie ZeroDDS ihn löst → selbst reproduzieren**.

---

## Der Standard: was ZeroDDS implementiert

ZeroDDS ist kein DDS-angehauchter Transport. Es ist eine vollständige, auditierte Implementierung der OMG-DDS-Spec-Familie — derselbe Stack, den RTI, eProsima, ZettaScale und OpenDDS implementieren, in sicherem Rust geschrieben.

| Spezifikation | Umfang | Status |
|---|---|---|
| **DDSI-RTPS 2.5** | Wire-Protokoll (SPDP/SEDP, reliable, Fragmentierung, HB/ACKNACK) | Voll — native Interop mit Fast DDS / Cyclone / OpenDDS / Connext |
| **DDS-DCPS 1.4** | Pub/Sub-API, QoS, Instances, Listener | Voll |
| **DDS-XTypes 1.3** | TypeObject/TypeLookup, Assignability, XCDR1 + XCDR2 | Voll |
| **DDS-Security 1.2** | Authentication, Access-Control, Crypto, Logging, Tagging | Voll — Cross-Vendor-Security-Matrix |
| **DDS-XML, DDS-XRCE, DDS-RPC** | XML-Profile, Micro-DDS-Agent/Client, Services | Voll |
| **Sprach-PSMs** | C / C++ (PSM-Cxx) / Java / C# / Python / TypeScript | Voll, codegen-getrieben |
| **ROS 2 RMW** | `rmw_zerodds` (REP-2003/2004/2005/2007/2008/2009) | Drop-in `RMW_IMPLEMENTATION`, live Cross-RMW-Interop mit `rmw_cyclonedds` |

RC1 ist publiziert: **97 Crates auf crates.io + docs.rs, 100 % dokumentiert.** Alles Open Source.

---

## Was wir können

- **Native RTPS-2.5-Interop** — spricht mit Fast DDS, Cyclone DDS, OpenDDS und Connext auf dem Draht. Ein ZeroDDS-Node und ein `rmw_cyclonedds`-Node finden sich und tauschen Daten bidirektional aus (verifiziert 20/20 auf `rt/chatter`).
- **Discovery ohne Multicast** — Unicast-Initial-Peers (`ZERODDS_PEERS`, `ZERODDS_NO_MULTICAST`) geben funktionierende Discovery auf WiFi, in Docker, über Subnetze, mit **keinem Discovery-Server zum Deployen und Babysitten**.
- **Laute statt stille Fehler** — ein inkompatibler QoS-Match löst ein `qos.incompatible`-Event mit der genauen verletzenden Policy aus, und ein statisches `qos_check`-CLI prüft Kompatibilität *vor* dem Launch.
- **Large-Data, die ankommt** — Application-Level-Fragmentierung mit selektivem NACK_FRAG-Retransmit und 16-MiB-Default-Reassembly-Cap (kein stiller Drop bei 1 MiB / 262 kB).
- **Variabel-große Zero-Copy-Shared-Memory** — ein längen-präfixierter SHM-Ring, kein fix-dimensionierter Iceoryx-Pool, den du von Hand auslegen musst.
- **Läuft vom MCU bis zum Server** — Pure-Rust `no_std + alloc`; der Kern baut für `thumbv7em-none-eabihf` (Cortex-M4F) mit ~1,6 MB Footprint und skaliert hoch bis zu Multi-Roboter-Flotten.
- **Memory-safe by construction** — sicheres Rust, `forbid(unsafe_code)` über den sicheren Kern; ganze Klassen der SHM-Segfaults und Buffer-Races, die gegen C++-Stacks gemeldet werden, sind nicht ausdrückbar.

---

## Wie schnell wir sind

Alle Zahlen sind aus den offenen Examples und Harnessen reproduzierbar. Hardware und Methode sind angegeben, damit du auf deinen eigenen Maschinen vergleichen kannst.

| Metrik | Zahl | Wie reproduzieren |
|---|---|---|
| Roundtrip-Latenz, Loopback, 256 B | **p50 = 40 µs / p99 = 83 µs** (200 Samples, 0 verloren) | `latency_ping` / `latency_pong` |
| Roundtrip-Latenz, Cross-Machine über WiFi, 256 B | **p50 ≈ 4,3 ms** (volle Discovery, 0 verloren) † | `latency_ping` / `latency_pong` über zwei Hosts |
| Large-Data-Durchsatz über WiFi (fragmentiert) | **10,8 MiB/s (~86 Mbit/s)** | `run_wifi_largedata.sh` |
| Große Samples intakt (2 / 4 / 8 MB) | byte-genau, multicast-frei | `run_largedata.sh` |
| All-to-all-Discovery, multicast-frei | 50 Participants in ~2,9 s, 100 in ~19,9 s | `ZERODDS_SCALE_N`-Scaling-Harness |
| Embedded-Footprint | ~1,6 MB, `thumbv7em-none-eabihf` | `cargo build --target thumbv7em-none-eabihf --no-default-features` |

† Die Cross-Machine-WiFi-Zahl erforderte, die WiFi-NIC wach zu halten; idle 802.11-Power-Save auf dem Client droppt sonst die latenz-sensitiven Unicast-Discovery-Frames. Das ist ein OS/AP-Power-Management-Artefakt (vendor-agnostisch, reproduzierbar A/B mit einem Packet-Capture), **keine** ZeroDDS-Limitation — dokumentiert in [`../interop/ros2-c3-large-data-wifi-followup.md`](../interop/ros2-c3-large-data-wifi-followup.md).

---

## Selbst validieren

Das ist der Teil, der für ein Open-Source-Publikum zählt. Wir bitten dich nicht, einem Benchmark-Slide zu trauen — wir liefern die Harnesse:

- **Cross-Vendor, multicast-freie Discovery** vs Cyclone DDS: `crates/ros2-rmw/interop/run_multicast_free_xvendor.sh` — ein ZeroDDS-Subscriber und ein Cyclone-Talker finden sich bei voll deaktiviertem Multicast und tauschen 20/20 Samples aus.
- **Live-ROS-2-Interop**: `crates/ros2-rmw/interop/run_interop.sh` — `rmw_zerodds` gegen einen echten `rmw_cyclonedds`-Talker/Listener auf `rt/chatter`.
- **Latenz / Durchsatz / Large-Data**: die `latency_*`-, `largedata_*`-Examples unter `crates/dcps/examples/`.

Wenn eine Aussage auf diesen Seiten auf deiner Hardware nicht reproduziert, ist das ein Bug-Report, den wir wollen. Der Schmerz-Korpus ([`../ros2-dds-painpoints-research-2026-06.md`](../ros2-dds-painpoints-research-2026-06.md)) ist ebenfalls offen — nimm irgendein Ticket, reproduziere es auf deiner aktuellen RMW, dann probiere es auf ZeroDDS.

---

## Ehrlicher Status

ZeroDDS ist bei **1.0.0-rc.1**. Der Spec-Stack ist vollständig und auditiert; die Cross-Vendor-Interop-, multicast-freie-Discovery-, Large-Data- und QoS-Lautheit-Aussagen sind e2e-verifiziert. Bereiche, die noch gehärtet und vermessen werden: Head-to-head-Latenz-/Durchsatz-Vergleichstabellen gegen jeden Vendor, und breitere Real-Flotten-Scaling-Zahlen. Wo eine Aussage verifiziert ist, sagen wir es; wo sie aspirational ist, markieren wir sie. Die Pro-Cluster-Seiten sind explizit, was was ist.

---

*Seiten in diesem Trail:*
[Discovery](discovery.md) ·
[Multicast / WiFi](multicast-wifi.md) ·
[QoS stilles No-Match](qos-silent-fail.md) ·
[Large-Data](large-data.md) ·
[Cross-Vendor-Interop](interop.md) ·
[Shared Memory](shared-memory.md) ·
[Konfigurations-Komplexität](config-complexity.md) ·
[Scaling](scaling.md) ·
[Docker / Cloud](docker-cloud.md) ·
[Security](security.md) ·
[Performance](performance.md) ·
[Migration](migration.md)
