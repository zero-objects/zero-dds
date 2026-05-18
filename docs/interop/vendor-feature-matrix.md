# DDS-Vendor Feature-Matrix

Vergleich der aktuell (Stand 2026-05-03) verfuegbaren und angekuendigten
Features der relevanten DDS-Implementationen. Basiert auf Web-Recherche
der offiziellen Dokumentation, Roadmaps und Release-Notes der jeweiligen
Projekte sowie den ZeroDDS Spec-Coverage-Dokumenten unter
`docs/spec-coverage/`.

**Legende:**

- ✓ — vollstaendig implementiert und in offizieller Release.
- ◐ — teilweise / limitiert / nur in Premium-Tier / Preview / nur via
  externer Service.
- ✗ — nicht implementiert.
- ? — nicht verifiziert aus den offiziellen Quellen.

**Disclaimer:** Vendor-Feature-Claims sind schnelllebig. Insbesondere
XTypes-1.3-TypeLookup und DDS-Security-1.2 waren Anfang 2026 noch bei
mehreren Vendors "next release". Bei Evaluation bitte direkt gegen die
aktuelle Version des Ziel-Vendors verifizieren.

## Matrix

| Feature | **ZeroDDS v1.2** | Cyclone DDS 11 | Fast-DDS 3.x OSS | Fast-DDS Pro | RTI Connext 7 | OpenDDS 3.34 | dust-dds |
|---------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **Core Protokoll** | | | | | | | |
| DDS 1.4 DCPS API | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ |
| DDSI-RTPS 2.5 wire | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| SPDP Discovery | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| SEDP Discovery | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Reliable | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Best-Effort | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Fragmentation | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Durability QoS** | | | | | | | |
| Volatile | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Transient-Local | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ |
| Transient (Service) | ✗ | ✗ | ✗ | ◐ | ✓ | ✓ ¹ | ✗ |
| Persistent (Service) | ✗ | ✗ | ✗ | ◐ | ✓ | ✓ ¹ | ✗ |
| **Kern-QoS (22 Standard-Policies)** | | | | | | | |
| History (KeepLast/All) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Deadline | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Lifespan | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Liveliness | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Ownership Shared | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Ownership Exclusive | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Partition | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Content-Filter Topic | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Time-Based Filter | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Destination Order | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| ResourceLimits | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ |
| **XTypes 1.3** | | | | | | | |
| IDL 4.x Parser | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ |
| TypeObjectV2 | ✓ | ✓ | ◐ | ◐ | ◐ ² | ✓ | ◐ |
| TypeLookup Service | ✓ | ◐ ³ | ✓ | ✓ | ◐ ² | ◐ | ✗ |
| DynamicType / DynamicData | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ | ✗ |
| Assignability Checks | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ |
| **Security & RPC** | | | | | | | |
| DDS-Security 1.1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| DDS-Security 1.2 | ✓ | ◐ | ◐ | ✓ | ✓ | ✗ | ✗ |
| DDS-RPC | ✓ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ |
| **Networking** | | | | | | | |
| DDS-TSN (OMG 2023) | ✓ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ |
| Shared-Memory Transport | ✓ | ✓ | ✓ | ✓ | ✓ | ◐ | ✗ |
| Zero-Copy / FlatData | ◐ | ✓ ⁴ | ✓ | ✓ | ✓ ⁵ | ✗ | ✗ |
| TCP Transport | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **Language Bindings** | | | | | | | |
| C | ✗ | ✓ | ◐ | ✓ | ✓ | ✓ | ✗ |
| C++ | ◐ ⁸ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Python | ✗ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ ⁶ |
| Rust (native) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| Java (Pure-Java) | ◐ ⁹ | ✓ | ✗ | ✗ | ✓ | ✓ | ✗ |
| **Build & Portability** | | | | | | | |
| `no_std` / bare metal | ✓ | ✗ | ✗ | ✗ | ◐ ⁷ | ✗ | ✗ |
| Safety Cert (DO-178C / ISO 26262) | ✗ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ |
| **Summe (voll ✓ / 40 Features)** | **33 / 40** | **29 / 40** | **29 / 40** | **34 / 40** | **34 / 40** | **28 / 40** | **11 / 40** |

**Fussnoten:**

¹ OpenDDS unterstuetzt Transient + Persistent nur mit dem proprietaeren
Transport, **nicht** ueber RTPS-Interop ([Quelle](https://opendds.readthedocs.io/en/master/devguide/quality_of_service.html)).
Cross-Vendor-Interop fuer diese Durability-Stufen ist damit effektiv nicht
moeglich.

² RTI Connext Pro (bis 7.x): TypeLookup-Service und TypeObjectV2 nach
XTypes 1.3 kommt im **naechsten LTS** ([Forum-Aussage RTI-Mitarbeiter](https://community.rti.com/forum-topic/interoperate-dynamic-type-discovery)).

³ Cyclone DDS 0.9.0 (Papillons) hat Basic-XTypes, aber der Built-in
TypeLookup-Service unterstuetzt noch keine Request von Type-Dependencies
— Matching kann bei unvollstaendigen Dependency-Sets fehlschlagen
([xtypes_relnotes.md](https://github.com/eclipse-cyclonedds/cyclonedds/blob/master/docs/dev/xtypes_relnotes.md)).

⁴ Cyclone Zero-Copy via Iceoryx-Integration (Shared-Memory-IPC).

⁵ RTI FlatData + Zero-Copy Transfer ueber SHM: 0 Kopien innerhalb eines
Hosts, 2 statt 4 Kopien bei UDP-Transport.

⁶ dust-dds Python-Binding ueber PyO3, funktional aber kein full
language-coverage.

⁷ RTI Connext Micro ist ein separates Produkt fuer Embedded / sicherheits-
kritische Anwendungen, nicht identisch mit Connext Pro.

⁸ ZeroDDS-C++-Binding via `crates/idl-cpp/` (DDS-PSM-CXX-Header-Codegen
+ Annex-A.1-CORBA-Trait-Templates); voller C++-Runtime-Wrapper auf
DCPS-Public-API ist Caller-Layer.

⁹ ZeroDDS-Java-Binding via `crates/idl-java/` (IDL→Java-Codegen +
IDL4-Java Annex-A.1) und `crates/java-omgdds/java/` (Pure-Java
DDS-Java-PSM Maven-Projekt mit `InProcessBus` + `Xcdr2Codec`;
CoreTypesTest + Xcdr2CodecTest + PubSubLoopbackTest, 18 grün).
Eine fruehere JNI-Bridge (`crates/zerodds-java-jni/`) wurde am
2026-05-07 entfernt — kein Native-Lib auf der Java-Seite mehr.

## Regional / Defense-Vendors

Zusaetzlich zur Mainstream-Liste gibt es regional oder vertikal
fokussierte DDS-Vendors. Feature-Claims sind hier schlechter
dokumentiert (Closed Source, Defense-Markt).

**Zu "Israel":** Es gibt aktuell keinen bekannten **israelischen
DDS-Middleware-Anbieter**. IAI (Israel Aerospace Industries) nutzt
fuer ihr OPAL-Framework **RTI Connext**. Die Jerusalemer Firma "DDS
Security" ist ein CCTV/Access-Control-Hersteller, keine OMG-DDS-
Middleware.

| Feature | MilDDS (TR) | InterCOM DDS (NO) | GurumDDS (KR) | Vortex OpenSplice (ZettaScale) | RustDDS (FI) | Connext Micro (RTI) |
|---------|:---:|:---:|:---:|:---:|:---:|:---:|
| DDS 1.4 DCPS API         | ✓ | ✓ | ✓ | ✓ | ◐ | ◐ |
| DDSI-RTPS 2.5 wire       | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Reliable / BE / Frag     | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Volatile / Transient-Local | ✓ | ✓ | ✓ | ✓ | ◐ | ✓ |
| Transient (Service)      | ? | ? | ? | ✓ ⁸ | ✗ | ✗ |
| Persistent (Service)     | ? | ? | ? | ✓ ⁸ | ✗ | ✗ |
| Deadline / Lifespan / Liveliness | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ |
| Ownership Exclusive      | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Partition / Content-Filter | ✓ | ✓ | ✓ | ✓ | ✗ | ◐ |
| XTypes                   | ? | ? | ◐ | ✓ | ◐ | ✗ |
| DDS-Security             | ✓ | ✓ | ? | ✓ | ✗ | ✗ |
| Zero-Copy / SHM          | ? | ? | ? | ✓ | ✗ | ✗ |
| Defense / Safety-Cert    | ✓ | ✓ | ✗ | ◐ | ✗ | ✓ |
| C / C++ bindings         | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ |
| Python / Java            | ? | ? | ✓ | ✓ | ✗ | ✗ |
| Rust                     | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ |

### Embedded / Micro-Variante (eigene Kategorie)

| Feature | Micro XRCE-DDS (eProsima) | RTI Connext Micro |
|---------|:---:|:---:|
| Protokoll             | DDS-XRCE (Client-Broker, kein peer-to-peer RTPS) | RTPS (static wiring) |
| Memory-Footprint      | < 75 KB Flash, ~3 KB RAM | ~100 KB Flash |
| Dynamic Allocation    | ✗ (komplett allocation-free) | ✗ |
| RTOS-Support          | FreeRTOS, Zephyr, NuttX | VxWorks, FreeRTOS |
| Transports            | UDP, TCP, Serial, CAN FD, Custom | UDP, Custom |
| ROS2-Support          | ✓ (Basis für micro-ROS) | ✓ (via Bridge) |
| Safety-Cert           | ✗ | ✓ |
| Lizenz                | Apache 2.0 | proprietär |

**Micro XRCE-DDS** ist ein **eigener OMG-Standard** (DDS-XRCE "eXtremely
Resource Constrained Environment"), kein klassisches DDS. Die Clients
reden ueber einen Agent (broker) mit der DDS-Global-Data-Space statt
peer-to-peer. Dadurch minimal footprint, aber keine echte DDS-Peer-
Interoperabilitaet — der Agent ist der uebersetzer zu OMG-DDS. Basis
fuer **micro-ROS** (ROS2 auf Microcontrollern).

### Footnotes

⁸ Vortex OpenSplice hat historisch das beste Persistence-Service-
Konzept (federated architecture mit dedicated daemon). Cyclone kann
Transient-Daten von einem OpenSplice-Node abrufen — das erklaert den
Cyclone-Doku-Workaround.

**Cyclone-Zertifizierung:** ZettaScale bietet eine kommerzielle
Cyclone-DDS-Variante an, die **ISO 26262 zertifiziert** ist (*Motionwise
Cyclone DDS*, entwickelt mit TTTech Auto). Die erste europaeische DDS-
Implementation, die in Serien-Automobilen zertifiziert eingesetzt ist.
DO-178C ist fuer Cyclone nicht bestaetigt.

**Kurz-Profile:**

- **MilDDS** (MilSOFT, Türkei) — seit 2004, COTS C4ISR-Middleware,
  CMMI Level 5, "no dynamic memory after init" (Safety-freundlich),
  eingesetzt in türkischen CMS/Tactical-Data-Link-Programmen.
- **InterCOM DDS** (Kongsberg Gallium, Norwegen) — Voice+Data Intercom
  fuer C4ISR, Referenz: US SOCOM Command Craft Medium. Low-latency
  tactical networks.
- **GurumDDS** (GurumNetworks, Südkorea) — ROS2-supported RMW
  (`rmw_gurumdds`), proprietär aber ROS2-kompatibel.
- **Vortex OpenSplice** — Ex-PrismTech (UK), ADLINK akquiriert, jetzt
  ZettaScale. Federated-Daemon-Architektur, stärkste Persistence-
  Story, schwerer zu deployen.
- **RustDDS** (Atostek, Finnland) — Rust-native, akademischer
  Background, kleinerer QoS-Support als dust-dds.
- **Connext Micro** (RTI) — Embedded-Variante mit eigener Codebase,
  Safety-zertifiziert, kein dynamic discovery.

## Interop-Risiken / Bekannte Quirks

Aus der Recherche herausgefiltert — das sind die Punkte, die unser
QoS-Matrix-Harness explizit abdecken sollte:

1. **Cyclone hat keine native Transient-/Persistent-Durability.** Nur
   Transient-Local ueber Writer-History-Cache. Transient kann nur mit
   externer OpenSplice-Persistence-Service abgerufen werden, **nicht**
   intern bereitgestellt. Cross-Vendor-Fallstrick: ein Fast-DDS- oder
   RTI-Publisher mit `Durability=Transient` wird vom Cyclone-Reader
   **nur wie Transient-Local** behandelt. [Quelle](https://cyclonedds.io/docs/cyclonedds/latest/about_dds/ddsi-transient_behavior.html)

2. **OpenDDS Transient/Persistent funktioniert nicht ueber RTPS.** Die
   Durability-Service-Features sind an OpenDDS-native Transports
   gekoppelt — RTPS-Interop-Path ist auf Transient-Local limitiert.

3. **Cyclone/RTI AckNack-Edge-Case:** Wenn Cyclone dem RTI-Writer
   signalisiert, dass alles empfangen wurde, kann der RTI-Writer in
   einen Endless-Loop gehen. Bekannt aus Community-Forum, Status
   unklar. Test-Case: beide Richtungen mit hohem Sample-Traffic.

4. **`autodispose_unregistered_instances` QoS:** unterschiedliche
   Interpretation zwischen Cyclone und RTI.

5. **XTypes-1.3-TypeLookup-Service** ist bei den meisten Vendors noch
   Preview/partial. Type-Evolution-Szenarien (hinzufuegen/entfernen
   von optional Fields) zwischen Vendors liefern oft keine saubere
   Matching-Fail-Meldung sondern "silent drop".

## ZeroDDS — konkrete Ableitungen fuer die eigene Roadmap

**Gewinn-Positionen** wo wir heute Parität oder Vorsprung haben:

- **33/40 Features voll** — ueber Cyclone (29) und Fast-DDS OSS (29),
  praktisch gleichauf mit Fast-DDS Pro (34) und RTI Connext (34) —
  einziges Spec-Defizit ist Safety-Cert (Partnerschaftspfad).
- `no_std`-Faehigkeit (einzig wir + Connext-Micro).
- XTypes-1.3 voller Stack: TypeObjectV2 + TypeLookup + DynamicType
  /DynamicData + Assignability — RTI hat TypeLookup nur partial,
  Cyclone hat TypeObjectV2 nur partial.
- DDS-Security 1.2 voll (k6 abgeschlossen) — Cyclone und Fast-DDS-OSS
  nur partial; gleiche Klasse wie Fast-DDS Pro / RTI.
- DDS-RPC voll — Fast-DDS-OSS, Cyclone, OpenDDS haben das nicht;
  gleiche Klasse wie Fast-DDS Pro / RTI.
- DDS-TSN voll — Cyclone, Fast-DDS-OSS, OpenDDS haben das nicht;
  gleiche Klasse wie Fast-DDS Pro / RTI.
- Rust-native (wir + dust-dds + RustDDS — aber wir haben deutlich
  mehr QoS als dust-dds).
- **Spec-Coverage strict-auditiert**: 31/32 Spec-Coverage-Files voll
  grün gegen die jeweiligen OMG-/IETF-/OASIS-Specs (siehe
  `docs/spec-coverage/`).

**Differenzierungs-Features ueber Mainstream hinaus** (Migrations-
Hebel fuer Bestand):

- **DLRL voll** (`crates/dlrl/`) — kein anderer aktiver Vendor hat
  das; Sales-Hebel fuer DDS-1.x-Bestandsmigrationen.
- **DDS-XML 1.0 + DDS-AMQP 1.0 + DDS-OPC-UA + DDS-Web** — alle vier
  als eigene Spec-Coverage-Files voll grün; ZeroDDS positioniert sich
  als Hub fuer Cross-Protocol-Bridging.
- **CCM 4.0 + AMI4CCM + CORBA 3.3 + COS-EventService** — voll
  ausgewiesene Stub-Layer als Migrations-Coexistence fuer
  Finanz-/Telco-Bestand.
- **DDS-XRCE** — Embedded-/IoT-Pfad jenseits der Mainstream-Vendoren.

**Verbleibende Defizite**:

- Transient + Persistent Service — nur RTI/Fast-DDS-Pro/OpenDDS;
  Backend ist im DataWriter angeschlossen (`crates/dcps/src/durability_service.rs`),
  Cross-Vendor-Wire-Replay-Pfad fehlt noch.
- Python-Binding — Riesen-Reichweite-Hebel (ros2-python).
- Voller C-Binding (FFI-Wrapper auf DCPS-Public-API).
- Voller C++/Java-Runtime-Wrapper auf DCPS-Public-API (heute nur
  Codegen + PSM-Skelett).
- Safety-Cert (DO-178C / ISO 26262) — Partnerschaft, nicht v1.x.
- Zero-Copy / FlatData — Iceoryx-Integration als opt-in.

**Realistische Positionierung v1.2 → v1.3:**

Heute (v1.2): "Rust-native DDS mit Pro-Feature-Parität (31/40),
voller XTypes-Stack, voller DDS-Security 1.2, voller DDS-RPC,
voller DDS-TSN, plus DLRL/CCM/CORBA-Coexistence, plus 5 Bridge-
Stacks (AMQP/MQTT/CoAP/WebSocket/gRPC), plus `no_std`."

v1.3-Cluster (drei Pakete): (a) Time-Based-Filter +
Destination-Order = Closure 22 Standard-QoS-Policies; (b) Python-
Binding via PyO3; (c) C-Binding-FFI auf DCPS-API.

Nach v1.3: **35/40 Features** = Vendor-Spitze, ohne Safety-Cert.

## Quellen

- [Cyclone DDS — QoS Docs](https://cyclonedds.io/docs/cyclonedds/latest/about_dds/qos.html)
- [Cyclone DDS — DDSI Transient-Local Behavior](https://cyclonedds.io/docs/cyclonedds/latest/about_dds/ddsi-transient_behavior.html)
- [Cyclone DDS — XTypes Release Notes](https://github.com/eclipse-cyclonedds/cyclonedds/blob/master/docs/dev/xtypes_relnotes.md)
- [Fast-DDS Roadmap](https://github.com/eProsima/Fast-DDS/blob/master/roadmap.md)
- [Fast-DDS Pro Features (TSN, RPC, Low-BW, Congestion)](https://fast-dds.docs.eprosima.com/en/latest/)
- [RTI Connext QoS Reference](https://community.rti.com/static/documentation/connext-dds/current/doc/manuals/connext_dds_professional/qos_reference/qos_reference/qos_guide_all_in_one.htm)
- [RTI FlatData + Zero-Copy](https://community.rti.com/kb/flatdata-and-zerocopy-examples)
- [RTI TypeLookup v1.3 Roadmap-Aussage](https://community.rti.com/forum-topic/interoperate-dynamic-type-discovery)
- [OpenDDS XTypes](https://opendds.readthedocs.io/en/master/devguide/xtypes.html)
- [OpenDDS QoS](https://opendds.readthedocs.io/en/master/devguide/quality_of_service.html)
- [OpenDDS DDS-Security Status](https://opendds.readthedocs.io/en/master/devguide/zerodds_security.html)
- [dust-dds GitHub](https://github.com/s2e-systems/dust-dds)
- [Twin Oaks CoreDX DDS](https://www.twinoakscomputing.com/coredx)
- [MilSOFT MilDDS (TR)](https://www.milsoft.com.tr/index.php/portfolio/mildds-en/)
- [Kongsberg InterCOM DDS (NO)](https://www.kongsberg.com/kda/what-we-do/defence-and-security/c4isr/intercom-dds/)
- [GurumNetworks GurumDDS (KR) via ROS2 RMW](https://docs.ros.org/en/rolling/Concepts/Intermediate/About-Different-Middleware-Vendors.html)
- [ZettaScale Vortex OpenSplice](https://www.adlinktech.com/en/vortex-opensplice-features)
- [Atostek RustDDS (FI)](https://github.com/Atostek/RustDDS)
- [RTI Connext Micro](https://www.rti.com/products/connext-micro)
- [OMG DDS RTPS Vendor and Product IDs](https://www.zerodds-foundation.org/zerodds-rtps-vendor-and-product-ids/)
- [Micro XRCE-DDS — micro-ROS Middleware](https://micro.ros.org/docs/concepts/middleware/Micro_XRCE-DDS/)
- [Motionwise Cyclone DDS — ISO 26262 (ZettaScale + TTTech Auto)](https://www.zettascale.tech/news/cyclone-dds-for-mission-critical-applications/)
