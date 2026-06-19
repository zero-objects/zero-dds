# ROS 2 ↔ DDS Schmerzpunkte → systematische ZeroDDS-Lösung

Strategie-Snapshot. Grundlage: Open-Robotics-Discourse-Suche „dds" (30 Threads),
plus Web-Recherche über GitHub-Issues (Fast-DDS, CycloneDDS, rmw_fastrtps,
rmw_cyclonedds, rmw_zenoh, ros2/rmw), Robotics Stack Exchange, ROS Answers,
Reddit und Vendor-/Blog-Quellen. **Smoking Gun:** der offizielle *ROS 2
Alternative Middleware Report* (OSRF/Intrinsic/ZettaScale, 2023) — die
DDS-Unzufriedenheit ist strukturell + offiziell anerkannt, nicht anekdotisch;
sie führte zu `rmw_zenoh` (experimentell in Jazzy, gebundelt ab Kilted).

---

## 0. Ausgangslage ZeroDDS (was schon da ist)

- **`rmw-zerodds-shim`** (`rmw_zerodds`, RMW-C-ABI) + **`ros2-rmw`** (`zerodds-ros2-rmw`, REP-2003/2004/2005/2007/2008/2009) — der Andock-Punkt für rclcpp/rclpy steht.
- **Voller DDS-Stack**: DDSI-RTPS 2.5, DCPS 1.4, XTypes 1.3, DDS-Security 1.2, XML, XRCE, RPC — alle Spec-Audits done.
- **Cross-Vendor-Interop nachgewiesen** (Security-Matrix: cyclone/fastdds/opendds/rti) — genau dort, wo FastDDS↔Cyclone in der Praxis bricht.
- **`transport-shm`** (Zero-Copy), Bridge-Crates (gRPC/MQTT/CoAP/WS), Pure-Rust `no_std+alloc` (Embedded-Footprint-Story).

Daraus folgt die **strategische These**: ZeroDDS kann *„das DDS sein, das die Gründe behebt, aus denen ROS DDS verlässt"* — die strukturellen Schmerzpunkte adressieren, die `rmw_zenoh` getrieben haben, **ohne** RTPS-Interop und Standard-Konformität aufzugeben. Man muss DDS/RTPS nicht verlassen, um dem DDS-Schmerz zu entkommen.

---

## 1. Der „ROS-Extra-Mile"-Block (Ausführungs-Schicht, analog CORBA)

Bevor die Schmerzpunkt-getriebenen Features: erst den bestehenden ROS-Stack auf
CORBA-Reifegrad bringen.

1. **Audit-Inkonsistenz klären** — `ros2-rmw.md` Footer sagt „0 partial", das Aggregat-Open zeigt aber ein `partial` (rmw-QoS-Sentinel-Formen). Verifizieren + Footer/Aggregat synchronisieren.
2. **Das 1 partial schließen** — `rmw_qos_profile_*`: `SYSTEM_DEFAULT` als echte Sentinel-Form (pro QoS-Feld `*_SYSTEM_DEFAULT`) statt Alias; `rmw_qos_profile_unknown` als Konstante. (~0.5 PW lt. Ledger.)
3. **Rejects gegen Spec-Vollständigkeit gegenprüfen** — ros2-rmw (3 rejected) + ros2-bridge (2 rejected, u.a. SROS2-Enclaves→DDS-Security). Wie bei CORBA die 7 Ex-Rejects: „andere haben das nicht" ist kein Reject-Grund.
4. **Live gegen echtes ROS 2** (codepit) — `rmw_zerodds` in eine echte ROS-2-Installation bauen, `talker`/`listener` (rclcpp + rclpy) über ZeroDDS, **Cross-RMW-Interop** ZeroDDS↔`rmw_cyclonedds`/`rmw_fastrtps` (das Pendant zur Cross-ORB-JacORB/omniORB/TAO-Validierung). Harness-Frage prüfen (bei CORBA war es `competitors/`).

---

## 2. Foren-Survey

| Quelle | Rolle |
|---|---|
| **Open Robotics Discourse** (Snapshot, 30 „dds"-Threads) | Grassroots-Schmerz, Ankündigungen, „Consolidated User Insights", „Design vs reality", „I'm done tuning", „middleware complaint" |
| **Offizieller Alternative Middleware Report** (discourse 33771 + Investigation 32642, 56 Replies/10.5k Views) | **Smoking Gun** — offizielle DDS-Schwächen-Liste, die zu Zenoh führte |
| **GitHub Issues** Fast-DDS / CycloneDDS / rmw_fastrtps / rmw_cyclonedds / ros2/rmw / rmw_zenoh | Konkrete Bugs + Zahlen (262 kB-Ceiling, Participant-Ceiling, SHM-Konflikte, Cross-Vendor-Mismatch) |
| **Robotics Stack Exchange / ROS Answers** | „warum reden meine Nodes nicht" — Discovery, QoS-Silent-Drop, Same-Machine-FastDDS-Fails |
| **Vendor/Blog** (zenoh.io, ZettaScale, eProsima, Vulcanexus, Husarnet) | Quantifizierte Kontraste (97–99 % weniger Discovery-Traffic), WiFi/Docker-FAQ |

---

## 3. Schmerzpunkt-Landschaft — gruppiert in 9 Cluster

Jeder Cluster mit Kern-Beleg(en). Quellen in den Recherche-Notizen.

### C1 — Discovery: N²-Skalierung, Multicast-Abhängigkeit, zu breite Defaults  ⭐ #1-Schmerz
- Fully-connected Participant-Graph + verbose SPDP/SEDP → O(n²) Discovery-Traffic; „packet storms", netzwerkweite Crashes. >100 Contexts/Host → Participants fallen aus; >120 Prozesse → Port-Kollision mit Nachbar-Domains (ros2/rmw #324).
- Default verbindet „mit dem ganzen Netz"; Multicast auf Enterprise/Cloud/WiFi/Docker oft geblockt → **stiller** Discovery-Fail.
- **Das ist der Hauptgrund für rmw_zenoh** (Zenoh: 97–99 % weniger Discovery-Traffic).

### C2 — QoS: stille Failures, Mismatch, fehlende Backpressure-Isolation
- QoS-Mismatch (RxO) → **kein Match, keine Daten, keine Meldung** (ROS-Docs geben's selbst zu).
- Best-Effort verhält sich wie Reliable; ein langsamer/lossy Remote-Subscriber **drosselt alle** — sogar den lokalen SHM-Pfad; RViz-über-WiFi bringt Roboter zum Stillstand. Implementierungs-übergreifend (Fast+Cyclone).

### C3 — Große Daten / Transport: Fragmentierungs-Hang, Large-Message-Drops, Zero-Copy-Grenzen
- Nachrichten >~262 kB „droppen" (Fast-DDS #3053); **IP-Fragmentierung**: 1 verlorenes Fragment auf WiFi → Reassembly-Buffer (256 kB) voll → 30 s `ipfrag_time`-Stall, **kein Empfang mehr** (Kernel-Level, vendor-agnostisch).
- Iceoryx-Zero-Copy nur **fixed-size** → genau PointCloud2/Bilder (variabel, groß) nicht abgedeckt; manuelle RouDi-Pool-Dimensionierung, „no silver bullet".

### C4 — Konfiguration / Tuning-Komplexität (Querschnitt)
- „Hunderte Parameter, keine Ahnung wo anfangen"; Stunden/Tage manuelles XML-Tuning → nur „good enough"; pro Vendor eigener XML-Dialekt (Fast-DDS-Profiles vs Cyclone-XML vs Connext-QoS); robotics-/WiFi-taugliche Defaults sind **nicht** die Shipped-Defaults; zusätzlich Kernel-Tuning (`rmem_max`, `ipfrag_*`).

### C5 — Cross-Vendor / RMW-Interop in der Praxis kaputt
- DDS-Standard *verspricht* Interop, FastDDS↔Cyclone-Serialisierung matcht aber nicht → „nothing works" („die große Ironie des DDS-Standards"). Services/Actions sind ROS-Implementation-Detail über pub/sub → **vendor-locked**, brechen bei heterogenen Flotten.

### C6 — Multi-Robot / WAN / Cross-Subnet
- RTPS ist LAN-orientiert (Multicast + Direct-Peer); WAN/Cross-Subnet braucht Discovery-Server/Static-Peers/Tunnel; „globe-spanning remote robotics" = unmet need; Domain-Cross-Talk; `ROS_LOCALHOST_ONLY` verhindert Cross-Talk nicht zuverlässig.

### C7 — Security-Setup-Bürde
- DDS-Security: Identity-CA + Permissions-CA + per-Participant-X.509 + signierte governance.xml/permissions.xml pro Enclave → „burdensome ... teams forgo security entirely". Kein Launch-File-Support.

### C8 — Tooling / Monitoring / Introspektion
- CLI prüft `RMW_IMPLEMENTATION` nicht; Introspektion bricht, sobald man die C1/C2-Workarounds nutzt (Multicast aus / Discovery-Server) — „fix one thing, break another"; QoS-Mismatch nur als Laufzeit-Failure sichtbar statt statisch validierbar.

### C9 — Embedded-Footprint / CPU-Overhead
- rmw-Conversion „frisst fast einen Core" bei High-Frequency; CycloneDDS-Footprint sprengt embedded-Speicher (Turtlebot). → Vendor-Swapping als Workaround.

---

## 4. Systematische ZeroDDS-Lösung pro Cluster

Prinzip: **Standard-konformes DDS bleiben (RTPS-Interop!), aber die strukturellen
Schmerzpunkte beheben** — gute Defaults + neue Mechanismen, nicht „noch ein
Vendor mit denselben Problemen".

| Cluster | ZeroDDS-Hebel (vorhanden → Lücke) | Differenzierung |
|---|---|---|
| **C1 Discovery** | RTPS-Discovery vorhanden → **eingebauter, multicast-freier Discovery-Pfad** (Unicast-Initial-Peers / eingebauter Discovery-Server / Gossip-Scouting), **robotics-Defaults** (domain-/host-scoped statt „ganzes Netz"), Skalierungs-Tests >100 Participants | „DDS ohne Packet-Storm" — der Zenoh-Killer, aber RTPS-interop bleibt |
| **C2 QoS** | RxO-Logik vorhanden → **laute, statische QoS-Kompatibilitäts-Validierung** (launch-/compile-time, klare Fehler) + **echte Best-Effort-Isolation** (per-Subscriber-Flow-Control; ein lossy Remote stallt nie lokale/SHM-Reader) | „QoS-Mismatch ist laut, nicht still"; „ein langsamer Subscriber kann den Roboter nicht anhalten" |
| **C3 Große Daten** | `transport-shm` (Zero-Copy) → **variable-size Zero-Copy** (kein Fixed-Pool-Zwang wie Iceoryx) + **RTPS-Fragmentierung, die WiFi-Loss übersteht** (App-Level-Frag + selektiver Retransmit statt IP-Fragmentierung/30 s-Stall) | „PointCloud2 über WiFi funktioniert einfach" |
| **C4 Config** | **robotics-taugliche Defaults out-of-the-box** + **eine kohärente Config** (kein XML-Dialekt-Zoo) + optionaler Auto-Tuner | „kein XML-Tuning nötig" |
| **C5 Cross-Vendor** | **bereits nachgewiesene RTPS-2.5-Interop** (cyclone/fastdds/opendds/rti) + Standard-DDS-RPC für Services | „interoperiert dort, wo Fast↔Cyclone bricht" — direkt belegbar |
| **C6 WAN/Multi-Robot** | Bridge-Crates + Discovery-Server → **routed/WAN-Topologie** (Cross-Subnet-Router), Domain-Isolation per Default | „lokal P2P, über Subnetze geroutet" (Zenoh-Parität, DDS-nativ) |
| **C7 Security** | volle DDS-Security 1.2 + secured-Interop-Arbeit → **„secure by default" / vereinfachte Enclave-UX** (weniger XML/Cert-Zeremonie, Launch-Integration) | „Security, die man nicht abschaltet, weil sie zu kompliziert ist" |
| **C8 Tooling** | **Introspektion, die unabhängig von Discovery-Config funktioniert** + QoS-Mismatch-Diagnose + RMW-Konsistenz-Check | „graph + qos sichtbar, auch mit Discovery-Server" |
| **C9 Embedded** | Pure-Rust `no_std+alloc` → **kleiner Footprint** + geringer rmw-Conversion-Overhead | „läuft, wo Cyclone nicht mehr reinpasst" |

---

## 5. Empfohlene Reihenfolge

1. **Extra-Mile-Block (§1)** zuerst — Stack auf CORBA-Reifegrad + **Live-rmw_zerodds gegen echtes ROS 2** (Glaubwürdigkeits-Anker, wie Cross-ORB bei CORBA).
2. Dann **die zwei Cluster mit dem größten Hebel + bestem ZeroDDS-Fit zuerst**:
   - **C5 Cross-Vendor-Interop** (haben wir schon belegt → schnell zu „Marketing-fähig" + sticht direkt gegen die „DDS-Ironie").
   - **C2 QoS-laut-statt-still + Backpressure-Isolation** (klar abgegrenzt, hoher Alltagsschmerz, klare Demo).
3. Danach die strukturellen Zenoh-Killer: **C1 Discovery (multicast-frei + Defaults)** und **C3 große Daten (variable Zero-Copy + WiFi-robuste Fragmentierung)** — die beiden Punkte, die ROS überhaupt zu Zenoh getrieben haben.
4. C4/C6/C7/C8/C9 als Querschnitt mitziehen.

**Positionierung in einem Satz:** *ZeroDDS = standard-konformes, cross-vendor-interoperables Pure-Rust-DDS, das die strukturellen Gründe behebt (Discovery-Storms, QoS-Silent-Drop, Large-Data-WiFi-Stall, Config-Hölle), aus denen ROS 2 sonst zu Zenoh ausweicht — als drop-in `rmw_zerodds`, ohne RTPS-Interop aufzugeben.*

---

## 6. Fortschritt (Stand 2026-06-08)

| Schritt | Status | Beleg |
|---|---|---|
| **§1 Extra-Mile / Live gegen echtes ROS 2** | ✅ done | `rmw_zerodds` ↔ CycloneDDS (= `rmw_cyclonedds`) **bidirektional 20/20** auf `rt/chatter`; entityKind-Cross-Vendor-Bug gefixt. `crates/ros2-rmw/interop/GROUND_TRUTH.md`, `docs/spec-coverage/cross-vendor-validation.md` (Addendum). |
| **C5 Cross-Vendor** | ✅ belegt | Live-ROS-Wire-Interop ist der direkte C5-Beweis („interoperiert dort, wo Fast↔Cyclone bricht") am realen ROS-Typ. |
| **C2 QoS laut-statt-still** | ✅ Kern done | (1) Inkompatibler QoS-Match emittiert lautes `Warn` (`qos.incompatible.{offered,requested}` + Topic + Policy) statt still zu droppen (`incompatible_qos_match_emits_loud_warning`). (2a) **Statische Validierung:** `zerodds_qos::compute_compatibility` deckt alle 9 RxO-Policies ab + listet ALLE Inkompatibilitäten (getestet). (2b) **Backpressure-Isolation:** unter KEEP_LAST (rmw-Default) blockt ein stalled/lossy Reader die Writes NICHT — er bekommt GAPs, fresh-writes laufen weiter (`keep_last_stalled_reader_does_not_block_fresh_writes`, rtps). **Launch-Surface:** `qos_check`-CLI (zerodds-qos example) prüft Writer/Reader-QoS ahead-of-time mit Exit-Code (CI/Launch). C2 voll. |
| **C1 Discovery** | ✅ done (Kern + Cross-Vendor) | well-known SPDP-Unicast-Port + `ZERODDS_PEERS` + `ZERODDS_NO_MULTICAST`. e2e: (a) 2 ZeroDDS-Prozesse multicast-frei 19/20 via Unicast, Negativ-Kontrolle 0; (b) **ZeroDDS↔CycloneDDS multicast-frei `matched=1`, 20/20** (`run_multicast_free_xvendor.sh`) — das WiFi/Cloud-VPC-Szenario. Unit `multicast_free_discovery_via_initial_peers`. **Rest:** nur >100-Participant-Skalierung. |
| **C3 Große Daten / WiFi-Frag** | ✅ done (Bug+WiFi+SHM) | Mechanismus (App-Frag 1344 B + NACK_FRAG-Retransmit) war da; **1-MiB-Silent-Drop-Cap gefixt** (16 MiB Default), 2/4/8 MB DCPS-e2e (`run_largedata.sh`); **Real-WiFi cross-machine** (m1 WiFi→codepit): **2+4 MB byte-perfekt** multicast-frei (`run_wifi_largedata.sh`); **Throughput 10,8 MiB/s** über WiFi; **Latenz Loopback p50=40 µs/p99=83 µs**; **variable-Zero-Copy via SHM** (`ZERODDS_SHM_MAX_DATAGRAM`, B8). **WiFi-Bidir-RTT: p50=4342 µs \*** (mymac wired ↔ m1 WiFi, 256 B, 15 RTTs, 0 lost, voller Discovery `participants=1 pubs=1 subs=1`; min=2873/p90=7444/p99=8166 µs). **Root-Cause der vorigen `participants=0`-Saga gefunden + A/B-bewiesen: 802.11-Power-Save auf dem WiFi-Client** verwirft die latenzsensitiven Idle-Discovery-Unicasts — `sudo tcpdump` (Promiscuous hält die NIC wach) → Discovery+RTT laufen sofort; ohne tcpdump nach 12 s Idle → `participants=0`/Timeout; identisches Pinning/Befehle/Hosts, einzige Variable = Promiscuous. **Kein ZeroDDS-Defekt** (Loopback p50=40 µs, m1→codepit largedata = Dauersender, NIC wach). \* andere Combi als der codepit-Stack, mit wachgehaltener WiFi-NIC gemessen. Doc: `docs/interop/ros2-c3-large-data-wifi-followup.md`. |
| **C4 Config** | ✅ Kern done | **`RuntimeConfig::ros_defaults()`** — ROS-Profil out-of-the-box (`data_representation [XCDR1,XCDR2]` matcht ROS/Cyclone-XCDR1-Writer + 16-MiB-Cap), ohne globalen Default zu ändern. e2e codepit: Chatter-Sub mit ros_defaults empfängt **20/20 von Cyclone OHNE Env-Workaround**. Kein XML-Dialekt-Zoo (Rust-struct + wenige Env). **Rest:** Auto-Tuner (optional). |
| **C6 WAN/Multi-Robot** | 🟡 Fundament da | **7 Bridge-Crates** (amqp/coap/grpc/mqtt/websocket/**zenoh**/corba-dds) + die heute gebaute **C1-Unicast-Initial-Peer-Discovery** = Cross-Subnet-Fundament (kein Multicast nötig). **Gap:** Routed/WAN-Topologie-Doku + Multi-Robot-Domain-Isolation-Profil. |
| **C7 Security-UX** | 🟡 Fundament da | Volle DDS-Security 1.2 + **`SecurityProfile`/`SecurityProfileConfig`** (security-runtime) + FFI `runtime_create_secure` vereinfachen den Setup bereits. **Gap:** „secure by default"-Launch-Integration + weniger Cert/XML-Zeremonie (Enclave-UX). |
| **C8 Tooling** | 🟡 Fundament da | Crates **`monitor`** + **`inspect-endpoint`** (Reality-Inspector-Linie) für Graph/QoS-Introspektion; C2-Teil-1 macht QoS-Mismatch bereits sichtbar (lautes Log). **Gap:** Introspektion, die unabhängig von der Discovery-Config funktioniert (auch mit Discovery-Server/Multicast aus) + RMW-Konsistenz-Check. |
| **C9 Embedded** | ✅ MCU-Build belegt | Pure-Rust **`no_std+alloc`**: **`zerodds-foundation` + `zerodds-cdr` + `zerodds-rtps` kompilieren alle für `thumbv7em-none-eabihf` (Cortex-M4F, bare-metal, `--no-default-features`)** — der volle RTPS-Wire-Core läuft ohne std/OS, wo CycloneDDS (C, größer) nicht reinpasst. Linux-App-Footprint zum Vergleich: 1,6 MB stripped (voller Pub-Stack). **Rest:** gelinkte Flash-Größe (Final-Binary) + rmw-Conversion-Overhead-Benchmark. |

**C2-Anknüpfung:** Der ROS-2-entityKind-Bug war selbst ein *stiller* QoS-artiger
Reject (Cyclones `DDS_INVALID_QOS_POLICY_ID` ohne Log) — die teure Diagnose hat
den C2-Schmerz „Mismatch ist still" am eigenen Leib belegt und direkt motiviert.
