# ZeroDDS — Advanced Roadmap (v1.2 → v2.0)

Strategisch abgeleitet aus der [DDS-Vendor Feature-Matrix](./vendor-feature-matrix.md).

## Mission / Competitive-Target

**Definiertes Ziel: Feature-Parität mit RTI Connext, minus
Sicherheits-Zertifizierung** (diese kommt als Partnerschaft oder im
v2.0-Zeitfenster, nicht als Blocker).

**Und: am RTI-Release-Zyklus vorbei.** ZeroDDS sitzt am direkten
Ohr der OMG — Spec-Revisionen (DDS-XTypes 2.0, DDS-Security 1.2
Errata, DDS-TSN-Updates, DDSI-RTPS 2.6) erreichen uns durch die
Gremien-Beteiligung **vor** der offiziellen Publikation. Wir starten
Implementation waehrend RTI noch im internen Proposal-Review ist
und haben produktive Releases im Markt, bevor RTI seine
typischerweise 6–9-monatigen Release-Zyklen durchgelaufen hat.

Das bedeutet fuer die Roadmap:

- **Release-Cadence ambitionierter als RTI** — monatliche
  minor-Releases, Feature-Flagship alle 3 Monate statt 6–9.
- **OMG-Proposal-Tracking** als eigener Prozess — sobald ein
  Issue/RTF oder eine Revised-Submission durchkommt, ist T-0 fuer
  Implementation.
- **Feature-Flags fuer Spec-Preview** — Beta-Implementationen von
  noch-nicht-finalisierten Specs als opt-in ausliefern, damit frueh-
  adaptierende Kunden sofort testen koennen (RTI exponiert das sonst
  erst nach Final-Release).

Rechtfertigung warum das aufgeht: RTI ist 400+ Mitarbeiter, wir sind
Startup-schlank. Deren Release-Engineering hat Enterprise-Overhead
(Sec-Review, Backwards-Compat-Matrix gegen 7 Jahre alte Versionen,
kommerzielle QA-Zyklen). Unser Rust-Stack mit strict-compile-time-
checks erlaubt sehr viel aggressivere Refactors. Die Rechnung geht
auf, solange wir die Qualitaets-Messlatte (99 % Branch-Coverage,
Cyclone-Interop-Tests grün) nicht aufweichen.

---

Stand: 2026-05-03.

## Wo wir heute stehen

- **33 / 40 Features** voll implementiert — über Cyclone (29) und
  Fast-DDS OSS (29), praktisch gleichauf mit Fast-DDS Pro (34) und
  RTI Connext (34) — einziges Spec-Defizit ist Safety-Cert.
- **Wire-kompatibel** (SPDP + SEDP + User-Data + XCDR2-Encapsulation,
  Cyclone-/Fast-DDS-Live-Tests grün).
- **Spec-Coverage strict-auditiert**: 31/32 Spec-Coverage-Files voll
  grün gegen die jeweiligen OMG-/IETF-/OASIS-Specs (siehe
  `docs/spec-coverage/`).
- **Einzigartige Kombination**: `no_std` + voller XTypes-1.3 +
  DDS-Security 1.2 + DDS-RPC + DDS-TSN + DLRL + 5 Bridge-Stacks
  (AMQP/MQTT/CoAP/WebSocket/gRPC) + CCM/CORBA-Migrations-Coexistence —
  kein anderer Vendor hat diese Kombination.

## v1.2 Closure — Interop-Nachweis

Die drei Interop-Stufen aus der [Cross-Vendor-Roadmap](./cross-vendor-roadmap.md):

| Stufe | Deliverable | Ziel-Metrik |
|-------|-------------|-------------|
| 1 — ShapesDemo | `ShapeType(Extended)` als `DdsType`, Docker-Harness gegen Cyclone-ShapesDemo | ≥ 10 Samples delivered, bidirektional |
| 2 — sensor_msgs/Image | ROS2-kompatible Image-IDL, Fragmentation-Stresstest VGA/HD | VGA @ 30 fps 0 % Loss loopback; HD @ 30 fps < 1 % Loss Gigabit |
| 3 — NGVA Cross-Domain | NGVA-Subset, 3 Hosts × 3 Domains × 2 Vendors | > 20 Endpoints parallel, Video stabil 10 min |

Plus **`tools/qos-matrix/`** — QoS-Compliance-Test-Harness, Matrix-
Report für Reliability/Durability/History/Deadline/Lifespan/Ownership/
Partition gegen Cyclone + Fast-DDS.

## v1.3 — QoS-Closure + Sprach-Reichweite

Sprach-Bindings für ROS2-/Scripting-/native-Drop-in (alle 22
Standard-QoS-Policies sind heute live):

1. **Python-Binding** — PyO3-Wrapper auf DCPS-API. Massiver
   Reichweite-Hebel (ros2-python, scripting, jupyter-notebook-demos).
2. **C-Binding (FFI)** — `dds-c`-Crate mit `cbindgen`-generiertem
   Header auf DCPS-Public-API.
3. **Voller C++/Java-Runtime-Wrapper** — heute Codegen + PSM-Skelett;
   v1.3 liefert die Runtime-Bindings auf den Rust-DCPS-Stack.

**Meilenstein v1.3**: **35 / 40 Features**, alle Sprach-Bindings live,
ROS2-Python-ready.

## v1.4 — Differenzierung

Bereiche wo wir von der Spitze differenzieren — weil wir `no_std` +
Rust + moderne Architektur haben:

1. **rmw_zerodds** (ROS2-Middleware-Adapter) — freischaltet die ganze
   ROS2-Welt. Micro-ROS-Konkurrenz ebenso denkbar (eProsima Micro
   XRCE-DDS ist die aktuelle Default-Basis, aber dort ist's ein
   Client-Broker — peer-to-peer wäre unser Unterscheidungsmerkmal).
2. **Zero-Copy / FlatData** für SHM-Transport — `transport-shm` da,
   fehlt die Builder-Generator-Integration (idlc emittiert
   `FlatStruct`-Bindings, Writer legt direkt in SHM-Blöcke).
3. **`async`-API parallel zur sync-API** (wie dust-dds) — Rust-
   idiomatisch, wichtig für Tokio/async-std-basierte Integrations.
4. **Tracing/Observability** — RTPS hat structured `tracing::`
   events; *live* Monitor analog RTI Connext Admin-Console. Kein
   anderer Rust-DDS hat das.
5. **Tooling-Suite** — `zerodds spy` (CLI live Topics/Writer/Reader),
   `zerodds monitor` (Web-GUI live Entity-Graph + QoS-Mismatches),
   `zerodds designer` (XML/YAML System-Topologie + Code-Gen),
   `zerodds record` / `replay` (SQLite/Parquet-Dumps).

## v2.0 — Full Enterprise Stack

Enterprise-Layer + Defense-/Avionics-Pfad:

- **Persistence-Service** mit HA-Setup — Replication zwischen mehreren
  `zerodds-persistence`-Instanzen, Failover, Snapshot/Restore.
- **Multi-Vendor-Bridge-Erweiterung** — heute haben wir AMQP, MQTT,
  CoAP, WebSocket, gRPC; v2.0 ergänzt:
  - Zenoh ↔ DDS (ZettaScale pusht Zenoh als DDS-Nachfolger; Bridge
    sowohl defensiv als auch offensiv).
  - Kafka ↔ DDS (Streaming-Analytics).
- **NGVA-Referenz-Stack** — Defense. `no_std`-Teil-Suite + Safety-
  Cert + DDS-Security als Europa-Alternative zu Kongsberg InterCOM /
  MilDDS.
- **FACE-Konformitaet** (Future Airborne Capability Environment) —
  Avionics-Markt, erfordert DO-178C DAL-C aufwaerts.
- **Safety-Zertifikate** — als **Partnerschafts-Track** (TTTech-
  Modell, Zertifizierungs-Dienstleister bringt den Stack durch die
  jeweilige Norm, wir liefern Code + Artefakte). Zielnormen:
  ISO-26262 ASIL-D, IEC-61508 SIL-3, DO-178C DAL-B, EN 50128 SIL-4.
- **Commercial Support-Tier** — Ticket-System, SLA, 24/7-Hotline,
  private CVE-Pipeline. Finanziert OSS-Kern.
- **OMG-Spec-Early-Access-Channel** — dedizierter Release-Track für
  unfertige OMG-Proposals (in-Progress-RTF, Beta-Submissions) als
  Feature-Flag-protected-Code. Ziel: Kunden, die eine neue
  Spec-Revision evaluieren müssen, bekommen die bei uns 6–12 Monate
  vor RTI.

## Positionierung-Evolution

| Release | One-Line-Pitch |
|---------|----------------|
| v1.2 heute | "Rust-native DDS mit Pro-Feature-Parität (33/40), voller XTypes-Stack + Security 1.2 + RPC + TSN + alle 22 Standard-QoS-Policies, Migrations-Coexistence (DLRL/CCM/CORBA + 5 Bridge-Stacks), `no_std`." |
| v1.3 | "QoS-Closure (35/40) + Python/C/C++/Java-Runtime-Bindings, ROS2-Python-ready." |
| v1.4 | "rmw_zerodds + Tooling-Suite + Zero-Copy/FlatData + async-API + Live-Tracing-Monitor." |
| v2.0 | "Full Enterprise (Persistence-HA, Safety-Cert als Partnerschaft, NGVA/FACE), plus Spec-Revisions schneller im Markt als RTI." |

## Abhängigkeits-Reihenfolge

```
v1.3 Sprach-Bindings
 │
 ├─ Python-Binding via PyO3 (eigener Track)
 ├─ C-Binding via cbindgen (eigener Track)
 └─ Voller C++/Java-Runtime-Wrapper (nach C-Binding)
     │
     └─ v1.4 Differenzierung
         │
         ├─ rmw_zerodds (braucht Python oder C-Binding)
         ├─ FlatData / Zero-Copy (braucht idlc-Codegen)
         ├─ async-API (isoliert)
         ├─ Live-Tracing-Monitor (isoliert)
         └─ Tooling-Suite: spy/monitor/designer/record/replay
             │
             └─ v2.0 Enterprise
                 ├─ Persistence-Service-HA
                 ├─ Bridge-Erweiterung: Zenoh, Kafka
                 ├─ NGVA-Referenz
                 ├─ FACE-Konformität
                 └─ Safety-Cert (Partnerschafts-Track)
```

## Tooling-Ökosystem — eigener Track

Vendor-Parität ist nicht nur Protokoll + QoS, sondern genauso die
**Tool-Suite**, die Ingenieure im Alltag brauchen. RTI hat mit Admin
Console + System Designer + Spy + Monitor einen erheblichen Lock-in-
Graben gebaut. Eclipse/ZettaScale kontert mit `cyclonedds` CLI +
zerodds-recorder + ddsperf. Ohne äquivalente Tools ist Protokoll-Parität
aus Kundensicht wertlos.

Kern-Tooling, das wir bauen:

| Tool | Vorbild | Zweck |
|------|---------|-------|
| **`zerodds spy`** | Cyclone `ddsls` + RTI DDS Spy | CLI, live Topics/Writer/Reader/Samples eines Domains anzeigen. |
| **`zerodds monitor`** (GUI) | RTI Admin Console | Web-basiert (Leptos/Dioxus), Live-Graph der Entities, QoS-Mismatches, Sample-Flow-Raten. |
| **`zerodds designer`** | RTI System Designer | XML-/YAML-basierte System-Topologie + QoS-Profile-Editor, Code-Gen. |
| **`zerodds record` / `replay`** | Cyclone zerodds-recorder | Dump aller Samples eines Topics zu SQLite/Parquet, deterministische Wiedergabe. |
| **`zerodds perf`** | ddsperf / rtiperftest | Throughput + Ping-Pong-Latency, Cross-Vendor-kompatibel. |
| **`zerodds shapes`** | RTI / Cyclone ShapesDemo | Referenz-Demo + Interop-Test-Client. |
| **Wireshark-Dissector** | Cyclone hat schon einen | RTPS-Analyse live im Wireshark; wir liefern Lua-Plugin + C-Plugin. |
| **QoS-Profile-Manager** | Fast-DDS QoS Profiles Manager | XML/YAML-Profile validieren, visualisieren, in Code exportieren. |
| **idlc** (schon begonnen) | RTI rtiddsgen | IDL 4.x → Rust/C/C++/Python/C#/Java Code-Gen mit XCDR1/2 + FlatData + Typ-Evolution. |
| **`zerodds doctor`** | Cyclone config-validator | Aktives Netzwerk-Diagnose-Tool — Multicast-Reachability, MTU, IGMP-Snooping-Tests, SEDP-Cache-Inspect. |
| **`zerodds bench`** (haben wir) | — | Weiterbauen zu Cross-Vendor-Matrix-Bench. |

## Persistence + Durability — vollständig

**Alle** Durability-Modi inkl. dedizierter Services gehören in den
v1.x-Korridor.

| Modus | Spec | Architektur-Hinweis |
|-------|------|---------------------|
| Volatile | ✓ heute | — |
| Transient-Local | ✓ heute | Writer-History-Cache + late-joiner-Replay |
| Transient | v1.4 | Federated Persistence Service (optional per-Domain), hält Samples auch nach Writer-Exit |
| Persistent | v1.5 | Persistence Service mit Disk-Backend (SQLite / Sled-DB), überlebt Node-Crashes + Neustarts |

Der Persistence-Service wird als **eigenständiger Daemon-Binary**
ausgeführt (`zerodds-persistence`), registriert sich als Subscriber
auf Topics mit `durability=Transient/Persistent`, cacht die Samples
und re-published bei late-joiner-Discovery. Architektur-Pattern orientiert
sich an OpenSplice' federated-daemon-Ansatz, aber mit modernem Rust-
Tooling (async I/O, strukturiertes Logging, Prometheus-Metrics).

## Language Bindings — volle Reichweite

Alle ROS2-RMW-Stacks, Enterprise-Integrations und Embedded-Plattformen
wollen ihre nativen Sprachen. Plan in Reihenfolge nach Kunden-Impact:

| Binding | Tool | Status / Release |
|---------|------|------------------|
| **Rust** | nativ | ✓ heute |
| **C++** | Codegen + PSM-Skelett ✓ heute, Runtime-Wrapper auf C-Binding aufsetzend | v1.3 (Runtime) |
| **Java (Pure-Java)** | Codegen + Pure-Java DDS-Java-PSM (`zerodds-java-omgdds`, kein JNI) ✓ RC1; gRPC-Bridge zu libzerodds-Server für Multi-Process Phase-2 | v1.3 (gRPC-Bridge) |
| **C#** | Codegen ✓ heute, csbindgen / .NET-FFI Runtime | v1.3 (Runtime) |
| **Python** | PyO3 | v1.3 (Reichweite-Hebel) |
| **C** | cbindgen + handgeschriebener wrapper | v1.3 (Voraussetzung für rmw_zerodds, ROS2, alle anderen Runtime-Bindings) |

Jedes Binding kriegt:
- eigenen CI-Job (Build + Unit-Test + Sample-Loopback)
- eigene hello-world-Examples
- eigenes API-Doc-Rendering (sphinx/javadoc/docfx)

## Zertifizierung — Mehrgleisig

Parallel zum Code-Track:

- **ISO 26262** (Automotive Functional Safety) — ASIL-B für v1.4,
  ASIL-D als v2.0-Ziel. Gegengewicht zu Motionwise-Cyclone.
- **IEC 61508** (Industrial Functional Safety) — SIL-2 für v1.4,
  SIL-3 für v2.0. Öffnet Industrial-Automation + Energy.
- **DO-178C** (Aviation) — DAL-C v2.0, DAL-B v2.x-Horizont. Öffnet
  Avionics (FACE, NGVA-Luftfahrt).
- **EN 50128** (Railway) — Add-on wenn Kunden fragen.
- **Common Criteria EAL4+** — Kopplung an DDS-Security-1.2 + TSN.
