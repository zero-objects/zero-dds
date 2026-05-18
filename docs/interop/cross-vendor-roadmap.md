# Cross-Vendor Interop — Test-Roadmap

Plan fuer ZeroDDS-Interop-Nachweis gegen andere DDS-Implementationen
(Cyclone DDS, eProsima Fast-DDS, RTI Connext, OpenDDS) ueber drei
Stufen mit jeweils hoeherem Realismus-Grad.

## Stand 2026-05-03

Foundation + Application-Level-Interop:

- SPDP / SEDP wire-kompatibel gegen Cyclone (WP 0.6 + 1.4).
- SPDP-Live-Discovery gegen Cyclone-Container (WP 0.7).
- SEDP-Live-Discovery gegen Cyclone-Container (WP 1.4).
- User-Payload XCDR2-Encapsulation-Header (Spec §9.4.2.13) in jedem
  gesendeten DATA-Submessage.
- **WLP-Live-Tests gegen Cyclone**:
  `crates/dcps/tests/cyclone_live_wlp{,_manual}.rs` — Liveliness
  Built-in-Topic-Wire-Roundtrip.
- **TypeLookup-Live-Tests**:
  `crates/discovery/tests/cyclone_live_typelookup.rs` +
  `cyclone_typelookup_responder.rs` — XTypes-1.3 TypeLookup-Service
  end-to-end.
- **Fast-DDS-Live-Tests**:
  `crates/discovery/tests/fastdds_live_spdp.rs` +
  `crates/dcps/tests/fastdds_live_{pub,sub,qos_matrix}.rs` —
  Cross-Vendor Pub/Sub + QoS-Matrix-Validation.
- **ShapesDemo-Live**: `crates/dcps/tests/shapes_{api_e2e,type_wire}.rs`
  — voller Application-Type-Roundtrip mit `ShapeType`.
- **Cyclone-Compliance**: `crates/rtps/tests/cyclone_compliance.rs` +
  `cyclone_he_must_understand.rs` — Wire-byte-identische Replays.

Stufe-1- und Stufe-2-Interop sind damit produktiv. Stufe-3
(NGVA Cross-Domain mit 3 Hosts × 3 Domains × 2 Vendors) bleibt
als nächster Test-Harness-Schritt.

## Stufe 1 — ShapesDemo

**Ziel:** Ersten nachweisbaren End-to-End-Payload-Flow zwischen
ZeroDDS und Cyclone / Fast-DDS ueber ein Topic mit realem, XCDR-
serialisiertem Application-Type.

**Warum ShapesDemo:** De-facto-Interop-Standard. Alle Vendors (RTI,
Cyclone, Fast-DDS, OpenDDS, Twin Oaks, Gurum) liefern einen
ShapesDemo-Client aus. Die OMG-DDS-RTPS-Testsuite
(<https://github.com/omg-dds/zerodds-rtps>) basiert darauf.

**IDL:**

```idl
struct ShapeType {
    @key string color;
    long x;
    long y;
    long shapesize;
};

struct ShapeTypeExtended : ShapeType {
    ShapeFillKind fillKind;
    float angle;
};
```

**Scope Stufe 1:**

1. `ShapeType` als `impl DdsType for ShapeType` mit XCDR2-LE-Encoder
   (nicht raw wie `RawBytes`) — primitive-felder + key-aware string.
2. `examples/shapes_demo_publisher.rs` und `_subscriber.rs` — Topic
   `"Square"`, `"Circle"`, `"Triangle"`.
3. Docker-Harness erweitern um Cyclones `ShapesDemo`-Container.
4. `tests/interop/shapes_e2e.sh` — ZeroDDS-Pub ↔ Cyclone-Sub
   und rueckwaerts; Success-Kriterium: ≥ 10 Samples delivered in
   30 s.
5. Test in GitLab-CI als optionalen Job (Linux-only, Multicast).

**Aufwand:** 1–2 Tage. Key-Handling auf Reader-Seite ist in v1.2
vereinfacht (noch keine Instance-Map).

**Success:** Cyclone-`rtiddsgen`-generierter ShapesDemo-Client
zeichnet ZeroDDS-Samples als Formen auf einem Canvas. Umgekehrt
genauso.

## Stufe 2 — ROS2 `sensor_msgs/msg/Image`

**Ziel:** Fragmentation-, Bandbreite- und Multi-Subscriber-
Stresstest mit einem echten Robotics-Workload. Nebeneffekt:
Fundament fuer ROS2-RMW-Adapter.

**Warum Image:** Jeder ROS2-Node spricht das ueber rmw_cyclonedds
oder rmw_fastdds. VGA (640×480 RGB8 ≈ 921 kB) triggert 15+
Fragmente pro Frame; HD (1920×1080 RGB8 ≈ 6.2 MB) triggert 100+.
Bei 30 fps × 2 Subscribers = 180 Fragment-NACK-Resend-Zyklen/s.
Das ist ein realistischer Load-Test.

**IDL (vereinfacht):**

```idl
module std_msgs { module msg {
    struct Header {
        builtin_interfaces::msg::Time stamp;
        string frame_id;
    };
};};

module builtin_interfaces { module msg {
    struct Time {
        int32 sec;
        uint32 nanosec;
    };
};};

module sensor_msgs { module msg {
    struct Image {
        std_msgs::msg::Header header;
        uint32 height;
        uint32 width;
        string encoding;    // "rgb8", "bgr8", "mono8" ...
        uint8  is_bigendian;
        uint32 step;        // row stride
        sequence<uint8> data;
    };
};};
```

**Scope Stufe 2:**

1. IDL-Types in Rust: Header, Time, Image als `DdsType` mit
   XCDR2-Nested-Struct-Encoding. CDR-Alignment Rules beachten
   (`long double`/`int32` aligned auf 4, `string` auf 4, `sequence`
   auf 4).
2. `examples/image_publisher.rs` — generiert synthetisches
   RGB8-Pattern, 30 fps.
3. `examples/image_subscriber.rs` — misst Delivery-Latenz + lost
   frames.
4. Bench-Integration: Image-Publisher gegen Cyclone-Subscriber
   unter `tools/bench-suite` fuer vergleichbare Zahlen.
5. Interop-Test gegen ROS2-Node (rclpy-Script mit demselben
   Topic-Namen + `sensor_msgs.msg.Image`).

**Aufwand:** 3–4 Tage. Nested-Struct-Encoding + String-Handling
ist die nicht-triviale Arbeit.

**Success:**

- VGA-Frame @ 30 fps mit 0 % Loss ueber Loopback (Reliable).
- HD-Frame @ 30 fps mit < 1 % Loss ueber Gigabit-Link.
- ROS2 `ros2 topic echo /camera/image_raw` zeigt ZeroDDS-Frames.

## Stufe 3 — NGVA (NATO Generic Vehicle Architecture)

**Ziel:** Strategischer Cross-Domain × Cross-Device × Cross-Vendor
Demo-Case. Zeigt ZeroDDS in einem realistischen Multi-ECU-Szenario
mit komplexem Datenmodell.

**Warum NGVA:** NATO-Standard (STANAG 4754) fuer Landfahrzeug-
Kommunikation. Definiert ein komplettes Datenmodell ueber DDS fuer
alle Sub-Systeme: Navigation, C2, Sensoren (EO/IR/Radar/LIDAR),
Effektoren, Health-Monitoring, Video-Streaming. OpenDDS hat eine
Referenz-Implementation, andere Vendors haben NGVA-Bindings fuer
Defence-Kunden.

**Scope Stufe 3:**

1. NGVA-Subset IDL importieren (UML-Modelle liegen bei NATO STO
   offen; Translator zu IDL ebenfalls).
2. Cross-Domain-Szenario:
   - Domain 0: Navigation + GPS-Broadcast.
   - Domain 1: EO-Kamera + Video-Stream.
   - Domain 2: C2-Decisions.
3. Cross-Device: mindestens 3 Hosts (kann VMs auf demselben
   Server sein) mit je 1-2 ZeroDDS-Participants.
4. Cross-Vendor: mindestens ein Host laeuft Cyclone oder Fast-DDS
   als zusaetzlicher Consumer.
5. Chaos-Test: Packet-Loss-Injection (tc netem 5–30 %) zur
   Reliable-Validierung unter realistischen Bedingungen.

**Aufwand:** 2–3 Wochen. Haengt stark davon ab, wie viel vom
NGVA-Modell wir tatsaechlich importieren.

**Success:**

- End-to-End-Demo mit > 20 aktiven DataWriters / DataReaders
  parallel ueber 3 Hosts × 3 Domains × 2 Vendors.
- Keine verlorenen C2-Messages (Reliable, Durable) auch bei
  Packet-Loss.
- Video-Stream ueber 10 min stabil mit < 100 ms median Latency.

## Bestehende DDS-Test-Harnesses (Recherche 2026-04-21)

Wir bauen nicht alles selbst neu — die DDS-Welt hat fuer fast jede
Interop- und QoS-Dimension existierende Test-Tools, gegen die wir uns
messen koennen oder die wir direkt uebernehmen.

### QoS + Interop-Compliance

| Tool | Scope | Zugang | Rolle fuer uns |
|------|-------|--------|---------------|
| **OMG zerodds-rtps Testsuite** (`omg-dds/zerodds-rtps`) | Offizielle OMG-Interop-Validierung. Shape Application pro Vendor, matrix-testet QoS-Policies (Reliability, Durability, Ownership, Deadline, Lifespan, History, Presentation) zwischen allen compliant DDS-Implementationen. GitHub-Actions-Automatisierung, oeffentliche Reports. | GitHub public. Needs pro Vendor ein `publisher_<vendor>` + `subscriber_<vendor>` Binary, das die Shape-App implementiert. | **Goldstandard** — sobald wir ShapesDemo (Stufe 1) haben, koennen wir uns dort anmelden und erscheinen im offiziellen Interop-Report. |
| **test_rmw_implementation** (`ros2/rmw_implementation`) | ROS2-RMW-Layer-Tests. QoS-Query-API-Coverage, QoS-Compatibility-Rules aus ROS2-Perspektive. | ROS2-Stack. Wir brauechten einen `rmw_zerodds`-Adapter, um es laufen zu lassen. | **Fernziel** — wenn wir einen rmw-Adapter haben (nach Stufe 2), ist das der Pfad zur ROS2-Zertifizierung. |
| **dust-dds interop tests** | Rust-native DDS-Impl mit eigener Test-Suite — sinnvoller Quervergleich fuer Rust-spezifische Aspekte. | GitHub public (`s2e-systems/dust-dds`). | **Quervergleich** — nicht direkt uebernehmen, aber Testcase-Ideen fuer Rust-Pitfalls. |

### Performance-Benchmarks

| Tool | Scope | Rolle fuer uns |
|------|-------|---------------|
| **Cyclone `ddsperf`** | Throughput + Ping-Pong-Latency, sequence<octet>-Payloads parametrisierbar. Command-Line. | **Baseline-Peer** — unser Publisher gegen `ddsperf subscribe`, unser Subscriber gegen `ddsperf publish` → vergleichbare Zahlen gegen Cyclone-Baseline. |
| **RTI Perftest** (`rticommunity/rtiperftest`) | RTI-eigener Latency/Throughput-Tester. Sehr konfigurierbar (batching, async, reliability). | **Optional** — RTI-Connext-License-abhaengig, aber wenn verfuegbar wertvoll fuer RTI-Cross-Vendor-Vergleich. |
| **eProsima benchmarking** (`eProsima/benchmarking`) | Fast-DDS-eigene Benchmark-Suite. | **Direkter Vergleich** — unser tools/bench-suite gegen Fast-DDS' eigene Zahlen. |
| **DDS-Perf Cross-Vendor** | Cross-Vendor-Benchmark-Harness der Cyclone / Fast-DDS / RTI parallel testet. | **Inspiration** — Setup-Pattern fuer unsere cross-vendor-Benchmark-Runs. |

### QoS-Conformance-Forschung

- **"Systematic Analysis of DDS Implementations"** (ACM Middleware
  2023, <https://dl.acm.org/doi/10.1145/3590140.3629118>) — systematische
  QoS-Conformance-Tests ueber alle grossen DDS-Impls. Paper liefert
  eine komplette Test-Matrix (welche Kombinationen verschiedene
  Vendors akzeptieren/ablehnen). **Direkt wiederverwendbar** als
  Katalog von Test-Cases fuer unsere eigene QoS-Implementation.

### Lessons aus den existierenden Harnesses

1. **QoS-Kompatibilitaet ist der Knackpunkt.** Interop-Tests zwischen
   Cyclone und RTI zeigen bekannte Inkonsistenzen bei
   `autodispose_unregistered_instances`, und endless-loop-Bugs bei
   ACKNACK-Edge-Cases. Unsere eigene Test-Suite muss diese bekannten
   Problemfelder explizit abdecken, nicht nur die Happy-Paths.

2. **Matrix-Approach wirkt.** Der OMG-Testsuite-Report listet pro
   QoS-Policy × pro Vendor-Paar einen Eintrag. Das ist der Standard
   fuer publizierbare Interop-Zahlen — wir kopieren das Pattern fuer
   unseren eigenen Report.

3. **Shape-Application als Basiswerkzeug.** Zu simpel um komplexe
   Bugs zu finden, aber perfekt fuer reproduzierbare Wire-Checks.
   Deshalb ist Stufe 1 ShapesDemo und nicht gleich ein Kompliziertes.

## Unser eigener Test-Harness — Aufsatzpunkt

Auf Basis der obigen Recherche planen wir eine
**QoS-Matrix-Test-Suite** parallel zum Vendor-Interop:

```
tools/qos-matrix/
├── policies.yaml           # Liste aller 22 QoS-Policies + Werte
├── matrix_runner.sh        # fuer jedes Writer×Reader-QoS-Paar
│                           # ein ZeroDDS-Pub und ein Vendor-Sub
│                           # (Cyclone/FastDDS) oder umgekehrt
├── report_template.md      # markdown matrix, gruen/gelb/rot
└── results/<date>-<vendor>.md
```

Scope v1.2: 7 Kern-Policies (Reliability, Durability, History,
Deadline, Lifespan, Ownership, Partition). Rest in v1.3.

## Zwischenschritte

Zwischen Stufe 1 und 2 sinnvoll:

- IDL-zu-DdsType Code-Generator (idlgen-Stub in `tools/idlc`) mit
  Template fuer XCDR2. Wenn das steht, ist Stufe 2 ueberwiegend
  IDL-Import statt hand-geschriebene Encoder.
- `ros2 msg generate`-Kompatibilitaet: gleiche Code-Shape wie
  `rosidl_generator_rust` waere ein direkter Migrations-Hebel.

## Referenzen

- [DDS Interoperability Tests v1.1 (2024)](https://omg-dds.github.io/zerodds-rtps/introduction.html)
- [omg-dds/zerodds-rtps — Validation Suite](https://github.com/omg-dds/zerodds-rtps)
- [atolab/dds-ishapes](https://github.com/atolab/dds-ishapes)
- [NGVA Data Model + OpenDDS (Galleon)](https://galleonec.com/ngva-data-model-and-opendds/)
- [DDS Foundation — Interoperability](https://www.zerodds-foundation.org/interoperability/)
- [ROS2 sensor_msgs/msg/Image](https://docs.ros.org/en/humble/p/sensor_msgs/msg/Image.html)
- [OMG zerodds-rtps Test Descriptions](https://omg-dds.github.io/zerodds-rtps/test_description.html)
- [ROS2 test_rmw_implementation](https://index.ros.org/p/test_rmw_implementation/)
- [Cyclone ddsperf Manpage](https://manpages.ubuntu.com/manpages/resolute/man1/ddsperf.1.html)
- [RTI rtiperftest](https://github.com/rticommunity/rtiperftest)
- [eProsima benchmarking](https://github.com/eProsima/benchmarking)
- [Systematic Analysis of DDS Implementations (ACM Middleware 2023)](https://dl.acm.org/doi/10.1145/3590140.3629118)
