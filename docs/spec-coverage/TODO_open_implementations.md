# Offene Spec-Implementierungen — TODO

Stand 2026-04-28. Konsolidiert die Items, die in den K10-K15-Audits
zunaechst als "done — alternative Implementations-Wahl" oder
"n/a — Optional-Profile" markiert waren, aber nach Review als
**echter Implementations-Bedarf** identifiziert wurden.

---

## A) IDL Legacy-Konstrukte (Cross-Sprach)

**Spec:** IDL 4.2 §7.4.13 (`fixed`), §7.4.14 (`any`), §7.4.13.4
(`bitset`), §7.4.13.5 (`bitmask`), §7.6 (`valuetype`), §7.5
(non-service `interface`).

**Begruendung Korrektur:** Diese Konstrukte sind **nicht** als
"implementation-may-reject" abdeckbar, weil Legacy-Migration
(CORBA/RTI/OpenSplice/PrismTech) ein zentrales Sales-/Migrations-
Argument von ZeroDDS ist (siehe Memory-Eintrag
`project_migration_as_sales_driver`). Wer aus einem CORBA- oder
DDS-1.2-Bestand migriert, bringt diese Typen mit; ein hartes Reject
verhindert Migration.

**Scope:**
- **`fixed` (IDL Decimal)**: Mapping auf Java `BigDecimal` /
  C# `decimal` / C++ `dds::core::Fixed` (Wrapper). XCDR-Wire-Form
  per Spec §7.4.13.
- **`any`**: Mapping auf Java `Object` (Reflection) / C# `object` /
  C++ `dds::core::Any`. TypeObject-Wire via Discriminator-Hash.
- **`bitset`**: Mapping auf Java `java.util.BitSet` / C# `BitArray` /
  C++ `std::bitset<N>`. XTypes-Bitset-Wire (siehe XTypes 1.3 §7.2.2.4.6).
- **`bitmask`**: Java `EnumSet` / C# `[Flags] enum` / C++ `enum class
  : uint32_t` mit OR/AND-Operatoren.
- **`valuetype`**: CORBA-Inheritance-Pfad mit Identity-Semantik.
  Mindestens IDL-Parser-Akzeptanz + Codegen-Stub.
- **non-service `interface`**: Pure-virtual-Class-Stub (C++) bzw.
  Interface-Stub (C#/Java) auch ohne `@service`-Annotation.

**Aktueller Status:** IDL-Parser akzeptiert alle Konstrukte. Status
pro Item:
- ✅ **`bitset`** — DONE 2026-04-28.
- ✅ **`bitmask`** — DONE 2026-04-28.
- ✅ **`fixed`** — DONE 2026-04-28 (cpp `dds::core::Fixed<D,S>`,
  csharp `decimal`, java `BigDecimal`).
- ✅ **`any`** — DONE 2026-04-28 (cpp `dds::core::Any`, csharp
  `Omg.Types.Any`, java `Object`).
- ✅ **non-service `interface`** — DONE 2026-04-28 (cpp pure-virtual
  class, csharp/java native `interface`).
- ✅ **`valuetype`** — DONE 2026-04-28 (cpp class+factory,
  csharp 2-Klassen-Abstract+Concrete, java 2-Files
  Abstract+Concrete; alle 9 Tests gruen).

**Re-Audit-Trigger:** Wenn Legacy-Migration-Sprint weiter geht.
Erwartete Reihenfolge:
1. ~~`bitset`/`bitmask`~~ DONE.
2. `fixed` (Decimal-Library-Integration).
3. `any` (Reflection-Heavy).
4. non-service `interface` + `valuetype` (CORBA-Profile).

**Audit-Items:**
- `idl4-cpp-1.0.md` §3.5 (`fixed`), §3.7 (`any`), §7.5 (non-service
  interface), §7.6 (valuetype): `partial`. §7.14.3.2/3 (bitset/
  bitmask): **done** seit 2026-04-28.
- `idl4-csharp-1.0.md`: analog (4 partials, 2 done).
- `idl4-java-1.0.md`: nur `fixed`/`any`/`valuetype`/non-service-
  interface partial; bitset/bitmask schon laenger live.

---

## B) IDL §8.1.5 `@Shared` Plain-Language-Annotation — DONE 2026-04-28

**Spec:** IDL4-cpp 1.0 §8.1.5 — `@Shared` markiert Felder als
Pointer-Type fuer Sharing zwischen Instances.

**Aufloesung:**
- IDL-Lowering: `BuiltinAnnotation::Shared` in
  `crates/idl/src/semantics/annotations.rs` + dynamic_apply
  (mapped wie `@external` auf `MemberDescriptor.is_shared`).
- Codegen:
  - C++: `crates/idl-cpp/src/emitter.rs` -> `std::shared_ptr<T>`
    (mit `<memory>`-Include); kombinierbar mit `@optional`.
  - C#: `crates/idl-csharp/src/annotations.rs` -> `[Shared]`-
    Marker-Attribute.
  - Java: `crates/idl-java/runtime/Shared.java` +
    `crates/idl-java/src/annotations.rs` ->
    `@org.zerodds.types.Shared` Marker.
- 4 neue Conformance-Tests (2 cpp + 1 csharp + 1 java).
- `dds-psm-cxx-1.0.md` §8.1.5 von `open` -> `done`.

---

## C/D/E/F) Native Java-PSM Foundation — DONE 2026-04-28

**Aufloesung:** `crates/java-omgdds/java/` als echtes Maven-Projekt
mit Pure-Java-Implementation (kein JNI, keine Rust-Native-Lib-
Voraussetzung). Module: `org.omg.dds.core` (ReturnCode mit allen 14
Spec-Codes, Time, Duration mit INFINITE-Sentinel, InstanceHandle mit
NIL-Detection, Entity-Interface), `org.omg.dds.core.policy`
(Reliability/Durability/History + QosProfile mit RxO-Compatibility-
Check), `org.omg.dds.topic` (Topic<T> + TopicTypeSupport<T>),
`org.omg.dds.domain` (DomainParticipant + Factory-Singleton),
`org.omg.dds.pub` (Publisher + DataWriter<T>::write), `org.omg.dds.sub`
(Subscriber + DataReader<T>::read/take + Sample<T>),
`org.zerodds.internal` (InProcessBus + Xcdr2Codec mit Primitive-
Encoder + UTF-8-String + Alignment-Cap=4). 18 JUnit-Tests gruen via
`mvn test`. Open-Items C/D/F als Foundation done; E (Dynamic-Types)
als partial. Pure-Java-RTPS-Stack fuer Cross-Process-Interop bleibt
Folge-Sprint (siehe `zerodds-java-psm-1.0.md` Update-Section).



**Spec:** zerodds-java-psm 1.0 §1, §2, §7.

**Begruendung Korrektur:** Die Pure-Java-Implementation-Realisierung ist **nicht**
Spec-konform abnahmefaehig:
- JNI verlangt eine native Lib, die **pro Java-Ziel-Plattform** gebaut
  werden muss (Linux x86_64, macOS aarch64, Windows x86_64, ARM, ...).
- Wir koennen auf den Build-Hosts unserer Anwender keinen Rust-
  Compiler voraussetzen.
- Spec sagt nicht "wie", aber das **Resultat** ist eine `omgdds.jar`,
  die ein Java-Anwender ohne Native-Toolchain laden kann.

**Scope:**
- Volles `org.omg.dds.*`-Package in **reinem Java** geschrieben (kein
  JNI). Realisiert das DCPS-Protocol direkt in Java (UDP-Sockets,
  XCDR-Codec, RTPS-Stack).
- Alternativ-Pfad: Java-Code generiert XCDR-Bytes selbst (gleiche
  Wire-Form wie Rust-Core), spricht direkt mit Cross-Vendor-DDS-
  Implementations.
- Build-Output ist eine portable JAR ohne Native-Lib-Dependency.

**Aktueller Status:** `crates/java-omgdds/java/` (1621 LOC) realisiert die
Pure-Java-Implementation — als Tooling fuer Code, der parallel mit Rust-Anwendungen
laufen will, weiterhin nuetzlich, aber **nicht** als Spec-Java-PSM-
Realisierung.

**Re-Audit-Trigger:** Wenn Java-PSM-Sprint startet. Schaetzaufwand:
gross (kompletter DCPS-Stack in Java; alternativ: Java→XCDR→Rust-
Side-Cars-Approach).

**Audit-Items, die zurueckgestuft werden:**
- `zerodds-java-psm-1.0.md` §1.1, §1.2, §2.0-§2.4: von `done — Pure-Java-Implementation`
  auf `open — Native Java-PSM noetig`.
- §7.2-§7.10 alle Pure-Java-Implementation-Refs: von `done` auf `partial — JNI-
  Bridge ersetzt, Native-Java-Package fehlt`.

---

## D) Cross-Vendor-Coexistenz via Java-API-Class-Identity (§7.2.2)

**Spec:** zerodds-java-psm 1.0 §7.2.2.1, §7.2.2.2.

**Begruendung Korrektur:** Wire-Form-Compliance allein erfuellt §7.2.2
nicht. Spec verlangt:
- Value-Type-Instance erzeugt von Vendor-A's Library MUSS in eine
  Method-Signatur von Vendor-B's Library passen.
- Read/Take-Sample von Vendor-A's DataReader MUSS direkt
  in Vendor-B's DataWriter::write(...) gehen.

Dies erfordert:
- **Stable Java-API** in `org.omg.dds.*` (alle Vendoren teilen den
  Spec-Header).
- **Class-Identity** zwischen Vendoren (`Reliability` aus Vendor-A
  ist binary-kompatibel mit `Reliability` aus Vendor-B).

**Scope:** Folgt direkt aus Punkt C — Native Java-PSM mit Spec-API.

**Audit-Items:**
- `zerodds-java-psm-1.0.md` §7.2.2.1, §7.2.2.2: von `done — Wire-Form`
  auf `open — braucht stable Java-API`.

---

## E) Single-Universal `omgdds.jar` mit Dynamic-Types (§7.2.1.2)

**Spec:** zerodds-java-psm 1.0 §7.2.1.2.

**Begruendung Korrektur:** Per-Application-JAR-Approach scheitert bei
Dynamic Types:
- Anwendung A definiert TopicType `Foo` zur Compile-Zeit.
- Discovery liefert TopicType `Bar` zur Laufzeit (vorher unbekannt).
- Per-Application-JAR enthaelt nur `Foo`-Wrapper — `Bar` kann nicht
  instantiiert werden.

Single-Universal `omgdds.jar` enthaelt:
- Volles DDS-DCPS-API in `org.omg.dds.*`.
- DynamicType + DynamicData fuer Runtime-Type-Erzeugung
  (XTypes-Pfad, siehe `crates/xtypes/`).

**Scope:** Folgt aus C+D. JAR ist conceptually unabhaengig von
spezifischen Topic-Types — User-Code uses entweder generated Wrapper
(idl-java-Codegen) ODER DynamicData (zur Laufzeit).

**Audit-Items:**
- `zerodds-java-psm-1.0.md` §7.2.1.2: von `done — Per-Application-JAR`
  auf `open — Single-Universal-JAR mit Dynamic-Types`.

---

## F) Auto-Close via Native Java (§7.2.3.3)

**Spec:** zerodds-java-psm 1.0 §7.2.3.3.

**Begruendung Korrektur:** "JNI weiss nicht wann disposed werden
darf" entfaellt mit Native Java — Java-GC kann mit `Cleaner`
(Java 9+) oder `PhantomReference` Auto-Close korrekt implementieren.

**Scope:** Folgt aus C. `Cleaner.register(...)`-basierter Auto-Close
fuer alle Reference-Type-Entities, sobald Native-Java-PSM da ist.

**Audit-Items:**
- `zerodds-java-psm-1.0.md` §7.2.3.3: von `done — explicit close`
  auf `open — Cleaner-basierter Auto-Close mit Native Java-PSM`.

---

## H) Sekundaer-Kunden Spec-Coverage (neu 2026-04-28)

Drei OMG-Specs sind via `docs/standards/fetch.sh` heruntergeladen
worden, aber noch ohne Implementation/Audit:

### H1) DDS-WEB 1.0 — Object-Model + REST-PSM DONE 2026-04-28

`crates/web/` neu mit voll modelliertem WebDDS-Object-Model
(Root/Application/Client/AccessController/DomainParticipant +
SessionId + ReturnStatus mit allen 8 Spec-Codes), allen 30+
REST-URI-Routes via parametrisiertem RestRoute-Enum (CreateApp/
DeleteApp/Participant/Topic/Pub/Sub/DataWriter::write/DataReader::read/
QosLibrary/QosProfile/Type/WaitSet — alle mit Parameter-Extraktion),
HTTP-Method-+Status-Code-Mapping (alle 8 ReturnStatus → 201/204/200/
409/422/404/401/403/500), XML-Element-Tag-Registry §8.3.4 Tab 6,
HTTP-Header-Set §8.3.5 Tab 7+8 mit Required-Validation
(OMG-DDS-API-Key + Accept + Cache-Control), fnmatch-Wildcard-Subset
fuer get_applications. 44 Tests gruen. Audit
`docs/spec-coverage/zerodds-web-1.0.md` mit 9 done / 4 partial (Multi-
Profile + AccessController-Rules + QosLib/Type-Schema-Storage + DCPS-
Bridge — alle Caller-Layer) / 1 open (§7.4.8 SampleSelector-BNF-
Parser) / 4 n/a (§3-§6 Glossar/Acks + §8.4 SOAP-Platform).

### H2) DDS-TSN 1.0 — PIM-Configuration + Ethernet-PSM DONE 2026-04-28

`crates/transport-tsn/` neu mit voll modelliertem Configuration-
Modell (8 wire-relevante PIM-Tables: MacAddress/VLAN-Tag mit
Bit-Layout-Validation/IPv4-Tuple/IPv6-Tuple/TrafficSpec mit
4 Transmission-Selection-Algorithmen/TimeAware mit Window+Jitter-
Validation/DataFrameSpec/Talker+Listener-Stream-Matching) +
DSCP-Modell (RFC 2474, DEFAULT/EF/AF11/AF21/AF31/AF41 + ToS-Octet-
Round-Trip) + Ethernet-PSM (Annex A: Frame-Header mit/ohne
VLAN-Tag, Round-Trip, Truncation-Detection, IPv4-EtherType-Non-VLAN-
Branch). 38 Tests gruen. Audit `docs/spec-coverage/dds-tsn-1.0.md`
mit 12 done / 0 partial / 2 open (§7.2.1-§7.2.2 DDS-Application/
Deployment-Config-Tables + §7.3 XML/JSON/YANG-PSM — Caller-Layer
Wire-Format-Konvertierung) / 6 n/a (§2 keine Conformance, §3
References, §4-§6 Glossar/Acks, Annex B informational).

### H3) DDS4CCM 1.1 (DDS for Lightweight CCM) — Audit DONE 2026-04-28

`docs/spec-coverage/dds4ccm-1.1.md` angelegt; XML-QoS-Profile (§8)
ist VOLL ABGEDECKT ueber `crates/xml/src/qos.rs`. CCM-Component-
Mapping (§7) bleibt Phase-2 (out-of-scope).

**Spec:** `docs/standards/cache/omg/dds4ccm-1.1.pdf`.

**Begruendung:** Schon Cross-Ref aus dds-psm-cxx §7.6.2.1 +
zerodds-java-psm §3.1.2 (XML-QoS-Profile-Quelle); eigener Audit
fehlte. Volle Coverage benoetigt CCM-Component-Bridge fuer
Container-Deployment-Use-Cases.

**Scope:**
- Audit-File `docs/spec-coverage/dds4ccm-1.1.md`.
- XML-QoS-Loader ist live (`crates/xml/src/qos.rs`); Component-
  Mapping ist erst noetig wenn ein CCM-Container produktiv wird.

### H4) DDS-OPCUA 1.0 (DDS-OPC-UA-Gateway) — Type-System + GDS DONE 2026-04-28

`crates/opcua-gateway/` neu mit voll modelliertem Type-System
(BuiltinTypeKind alle 25 Werte; Primitive-Mapping Tab 8.1; Complex-
Types Tab 8.2 inkl. Guid, NodeId mit 4 Identifier-Cases + 4096-Byte-
Limit, ExpandedNodeId, Variant mit 21 VariantValue-Cases, DataValue,
ExtensionObject, NodeClass mit allen 8 Bit-Flag-Werten); GDS-Mapping
(DomainNode mit BrowseName "Domain<id>", TopicNode mit BrowseName =
Topic-Name, SampleVariable mit value_rank=-1 fuer Scalar). 33 Tests
gruen. Audit `docs/spec-coverage/dds-opcua-1.0.md` mit 8 done /
2 partial / 5 open / 5 n/a (Service-Sets §8.3 + Subscription-Behavior
§8.4 + Historical §9.3.4 + XML-Config §10 + volle Bridge-Conformance
verlangen externen OPC-UA-Stack — Caller-Layer).

---

## I) DDS TypeScript Interface — DONE Foundation 2026-04-28

**Spec:** kein OMG-Standard. ZeroDDS-eigene Wahl analog
idl4-cpp/csharp/java.

**Aufloesung Foundation:**
- `crates/idl-ts/` neu — IDL → TypeScript-Codegen (5 Tests gruen).
- Type-Mapping: Primitive (boolean/number/bigint/string), Sequence
  (Array<T>), Struct (interface), Enum (export enum), Module
  (namespace).
- Audit-File `docs/spec-coverage/idl4-ts-1.0.md`.

**Phase-2-Items DONE 2026-04-28:**
- ✅ Union → discriminated-union TS algebraic-data-type.
- ✅ Typedef → `export type X = ...`.
- ✅ Bitset → interface + Bit-Width-Konstanten (Const-Eval-Hook
  Caller-Layer).
- ✅ Bitmask → const-Object mit Shift-Konstanten.
- ✅ Decorators-Markers (@Key/@Optional/@Mutable/@Final/@Appendable)
  exported.

**Verbleibend `open`:**
- WASM-Runtime (`crates/dcps-wasm/`) fuer Browser.
- Node-FFI via napi-rs.
- RPC-Codegen (analog idl-cpp/rpc.rs).
- Bitset-Width-Const-Eval-Pipeline-Verbindung.

---

## J) RTPS-over-TCP Transport — DONE (existing)

`crates/transport-tcp/` ist bereits seit Phase 1 implementiert
(~2050 LOC inkl. handshake, framing, connection-pool, tests).

Siehe Punkt N unten fuer die Aufloesung.

---

## K) Weitere uncovered OMG-Specs (Survey 2026-04-28)

Stand des Spec-Cache (`docs/standards/cache/omg/`):

**Voll abgedeckt (mit Audit-File):** zerodds-dcps-1.4, ddsi-rtps-2.5,
dds-xtypes-1.3, zerodds-security-1.2, idl-4.2, zerodds-rpc-1.0, zerodds-xml-1.0,
zerodds-xrce-1.0, idl4-cpp-1.0, idl4-csharp-1.0, idl4-java-1.0,
dds-psm-cxx-1.0, zerodds-java-psm-1.0.

**Heruntergeladen, Audit ausstehend (Punkte H1-H4):** zerodds-web-1.0,
dds-tsn-1.0-beta2, dds4ccm-1.1, dds-opcua-1.0.

**Nicht im OMG-Katalog (recherchiert 2026-04-28):**
- "DDS-Cloud" (kein Spec — Cloud-Discovery ist Vendor-Feature).
- "DDS-TS" (kein OMG-Standard; offene RFP, siehe Punkt I).
- "DDS-RPC-CCM" (404, kein Spec).
- "RTPS-TCP" (Teil von DDSI-RTPS, kein eigener Spec; Punkt J).

**Noch nicht recherchiert** (potentiell Sekundaer-Relevant):
- ISO/IEC 19516:2020 (IDL — paywalled, OMG-IDL-4.2 ist gleichwertig).
- DDS-Vendor-Extensions (RTI-Specific, OpenSplice-Specific) — kein
  OMG-Standard.

---

## L) OMG-Adjacent Specs (im fetch.sh seit 2026-04-28)

### L1) TIME 1.1 (Time Service)

**Datei:** `docs/standards/cache/omg/time-1.1.pdf`.

**Scope:** Time-Service-Spec; relevant fuer DDS-Security (Clock-
Skew-Validation in Auth-Tokens) und DDS-DCPS Time/Duration. Audit-
File `dds-time-1.1.md` ausstehend.

### L2) AMI4CCM 1.1 (Async Method Invocation for CCM) — DONE 2026-04-28

**Datei:** `docs/standards/cache/omg/ami4ccm-1.1.pdf`.

**Aufloesung:** `crates/ami4ccm/` neu — Implied-IDL-Transformation
(`transform.rs`), Pragma-Parser (`pragma.rs`),
`CCM_AMI::ExceptionHolder`-Modell (`exception_holder.rs`); 27 Tests
gruen. Audit-File `docs/spec-coverage/omg-ami4ccm-1.1.md` mit
PROCESS.md-konformer Item-Liste (16 done, 3 partial, 1 open, 8 n/a).
CCM-Connector + D&C-Deployment (§7.6/§7.8) bleiben `n/a` ohne
CCM-Container-Runtime; Begruendung im Crate-Doc-Header und
Audit-File.

### L3) CCM 4.0 (CORBA Component Model) — DONE 2026-04-28

**Datei:** `docs/standards/cache/omg/ccm-4.0.pdf`.

**Aufloesung:** `crates/ccm/` neu — §6 Component Model
Equivalent-IDL-Transformation (Provides/Uses/Emits/Publishes/Consumes/
Attribute), Home-Equivalent (Explicit + Implicit + Equivalent fuer
keyless + keyed), EventType-Transformation; §13 Lightweight CCM Profile
Filter (Configurator-Drop); `Components::*`-Datentypen-Modell. 25 Tests
gruen. Audit-File `docs/spec-coverage/omg-ccm-4.0.md` mit 25 done /
13 partial / 2 open / 28 n/a (Container/CIDL/Deployment ORB-bound mit
Begruendung pro Item).

### L4) RTC 1.0 (Robotic Technology Component) — DONE 2026-04-28

**Datei:** `docs/standards/cache/omg/rtc-1.0.pdf`.

**Aufloesung:** `crates/rtc/` neu — Lightweight RTC, ExecutionContext,
Lifecycle-State-Machine (Created/Inactive/Active/Error mit Pre-
Condition-Enforcement), ComponentAction-Trait mit 9 Callbacks,
DataFlow/FSM/MultiMode-Profile (Spec §5.3.1/§5.3.2/§5.3.3); Local PSM
(§6.3) als Implementation-Form. 37 Tests gruen. Audit
`docs/spec-coverage/omg-rtc-1.0.md` mit 21 done / 4 partial / 2 open
(Introspection §5.4) / 6 n/a (LwCCM-PSM + CORBA-PSM ORB-bound).

---

## M) Non-OMG DDS-Bridges (im fetch.sh seit 2026-04-28)

### M1) CoAP (RFC 7252 + RFC 7641 Observe) — DONE 2026-04-28

**Dateien:** `docs/standards/cache/ietf/rfc7252.txt`,
`rfc7641.txt`.

**Aufloesung:** `crates/coap-bridge/` neu — voller Wire-Codec
(Header, Token, Options mit Delta-Encoding + Extended-Forms 13/14,
Payload-Marker), Option-Number-Registry §5.10 + RFC 7641 §2 Observe,
Code-Registry §12.1 + Klassifikations-Praedikate. 34 Tests gruen
inkl. Bit-Patterns aller well-known-Codes (GET=0x01, POST=0x02, ...,
2.05=0x45, 4.04=0x84, 5.00=0xA0). Audit
`docs/spec-coverage/coap-rfc-7252.md` mit 14 done / 1 partial / 0 open
/ 16 n/a (Reliability/DTLS/Multicast/URI ausserhalb Codec-Scope).

### M2) WebSocket Protocol (RFC 6455) — DONE 2026-04-28

**Datei:** `docs/standards/cache/ietf/rfc6455.txt`.

**Aufloesung:** `crates/websocket-bridge/` neu — voller Base-Framing-
Codec (§5.2 + §5.3): FIN/RSV1-3/Opcode/MASK/Payload-Length mit
3-Variant-Encoding + Minimal-Length-Validation + 64-bit-MSB-Check,
XOR-Masking (symmetrisch + key-mod-4), Control-Frame-Constraints
(payload <= 125 + FIN=1) enforced, Close/Ping/Pong/Text/Binary
Frame-Konstruktoren. 32 Tests gruen. Audit
`docs/spec-coverage/websocket-rfc-6455.md` mit 11 done / 3 partial
(PRNG/UTF-8/Status-Code-Validation Caller-Aufgabe) / 0 open / 11 n/a
(HTTP-Handshake/State-Machine/Extensions ausserhalb Codec-Scope).

### M3) MQTT 5.0 (OASIS) — DONE 2026-04-28

**Datei:** `docs/standards/cache/oasis/mqtt-5.0.html`.

**Aufloesung:** `crates/mqtt-bridge/` neu — voller Wire-Codec:
§1.5 Data Types (VBI mit allen Boundaries 0/127/128/16383/16384/
2097151/2097152/MAX, 5-byte-Reject; UTF-8 String mit u16 BE
Length-Prefix; Binary Data; Two/Four Byte Integer), §2.1 Fixed Header
mit allen 15 Control-Packet-Types, §2.2.2 Properties-Registry mit
15 named-IDs (PayloadFormatIndicator, ContentType, ResponseTopic,
SessionExpiryInterval, ReceiveMaximum, TopicAlias, UserProperty, ...)
+ Other-Variant, §3.3 PUBLISH-Packet voll (alle QoS-Levels +
DUP/RETAIN + Packet-Identifier + Properties + Pre-Condition-
Enforcement). 36 Tests gruen. Audit
`docs/spec-coverage/mqtt-5.0.md` mit 12 done / 7 partial (CONNECT/
CONNACK/PUBACK/SUBSCRIBE/UNSUBSCRIBE/DISCONNECT/AUTH Body-Codecs +
Property-Wert-Decoding) / 0 open / 6 n/a (§4 Broker-Behavior, §5
Security, §6-§7 Conformance/Acks).

### M4) AMQP 1.0 (OASIS) — DONE 2026-04-28

**Dateien:** `docs/standards/cache/oasis/amqp-1.0-{overview,types,
transport,messaging,security}.html` (5 Files).

**Aufloesung:** `crates/amqp-bridge/` neu — Type-System §1.6 Subset
(null/boolean/ulong/long/binary/string/symbol mit Compact-Form-
Selection ulong0/smallulong/vbin8/vbin32/str8/str32/sym8/sym32 +
non-ASCII-Symbol-Reject), Format-Code-Subcategorization §1.2
(Fixed/Variable/Compound/Array), Frame-Header §2.3.1 mit voller
Pre-Condition-Enforcement (DOFF>=2 + SIZE>=8 + body_offset<=SIZE +
AMQP/SASL/Reserved-Frame-Type). 27 Tests gruen. Audit
`docs/spec-coverage/amqp-1.0.md` mit 12 done / 2 partial (Composite/
Described Caller, Integer-Subtypen Konstanten exposed) / 7 open
(float/decimal/timestamp/uuid/list/map/array + Performatives + Message-
Sections — alle Caller-Layer ueber dem Type-System) / 3 n/a (Overview
informativ, SASL Crypto-Layer).

### M5) ROS 2 RMW Bridge — DONE 2026-04-28

**Dateien:** `docs/standards/cache/ros2/rep-{2003,2004,2005,2007,
2008,2009}.html`.

**Aufloesung:** `crates/ros2-rmw/` neu — Topic-Name-Mangling
(rt/rq/rr/rs Prefixes mit Round-Trip + Reject von invalid-leading-
Char + Empty-Name-Reject + Demangle-Reject von unbekannten Prefixes),
REP-2003 Map (Reliable+TransientLocal+KeepLast(1)) und Sensor-Data
(BestEffort+Volatile+KeepLast(5)) QoS-Profiles, REP-2004 Quality-Level
Q1-Q5 mit numeric/from_numeric, alle Standard RMW QoS-Profiles
(DEFAULT/SENSOR_DATA/MAP/PARAMETERS/SERVICES_DEFAULT/PARAMETER_EVENTS/
SYSTEM_DEFAULT). 23 Tests gruen. Audit
`docs/spec-coverage/ros2-rmw.md` mit 8 done / 0 partial / 0 open /
4 n/a (REP-2005 informational, REP-2007 Compile-Time-C++, REP-2008
Driver-Layer, REP-2009 rclcpp-Layer).

### M6) gRPC (HTTP/2 + Web) — DONE 2026-04-28

**Dateien:** `docs/standards/cache/grpc/protocol-{http2,web}.md`.

**Aufloesung:** `crates/grpc-bridge/` neu — Length-Prefixed-Message
(Compressed-Flag + 4-byte BE Length + Bytes, gRPC-Web Trailer-Frame
mit MSB=1), Path-Parsing `/<service>/<method>` mit allen Edge-Cases
(relative-Reject, empty-segment-Reject, method-with-slash-Reject),
Timeout-Header (alle 6 Units H/M/S/m/u/n + 8-Digit-Limit + Round-Trip),
Status-Code-Set (alle 17 kanonischen Codes 0=OK..16=UNAUTHENTICATED
mit numeric/from_numeric/name screaming-snake-case + is_ok-Praedikat).
32 Tests gruen. Audit `docs/spec-coverage/grpc-protocol.md` mit
7 done / 2 partial (Generic-HTTP-Headers Caller-Layer) / 0 open /
3 n/a (HTTP/2 Transport-Mapping RFC 7540, Custom-Metadata Binary/ASCII
generic HTTP, gRPC-Web Base64-Subprotocol).

---

## N) RTPS-over-TCP Annex (DDSI-RTPS 2.5 §9.4 + Annex) — DONE 2026-04-28

**Aufloesung:** `crates/transport-tcp/` existiert bereits voll
implementiert (~2050 LOC):
- `framing.rs` — length-prefixed RTPS-Framing.
- `handshake.rs` — DDS-TCP-PSM-Handshake-kompatibel.
- `tcp_transport.rs` — Transport-Trait-Impl mit Connection-Pooling.
- LocatorKind `TCPv4`/`TCPv6` in `crates/rtps/src/wire_types.rs`
  (Werte 4/8 per DDSI-RTPS §9.4.5).
- Tests in `crates/transport-tcp/tests/loopback.rs`.

Cross-Ref `crates/transport-tcp/src/lib.rs` — Cyclone-DDS-`ddsi_tcp`-
Compatibility-Mode ist live; voller DDS-TCP-PSM-Handshake siehe
`handshake.rs`.

**Status:** done — bereits Bestandteil des RTPS-Transport-Stacks.

---

## G) XSD Type Representation (XTypes 1.3 §7.3.3) — DONE

**Status:** Aufgeloest 2026-04-28 (separater Commit, siehe
`dds-xtypes-1.3.md` §7.3.3).

XSD-Loader implementiert in `crates/xml/src/xsd/loader.rs`:
- W3C XSD 1.0/1.1 → XTypes TypeObject Mapping.
- Built-In-Types (`xsd:string`, `xsd:int`, ...) → DDS-Primitives.
- `<xsd:complexType>` → Aggregate-Type.
- `<xsd:simpleType>` mit Restriction → Enum oder Range-annotated.
- `<xsd:sequence>` → DDS-Sequence.

Tests: `crates/xml/src/xsd/loader.rs::tests::*`.

---

## Reihenfolge / Priorisierung (Update 2026-04-28)

1. **G) XSD-Loader** — DONE.
2. **B) `@Shared`** — DONE 2026-04-28.
3. **A1) `bitset`/`bitmask`** — DONE 2026-04-28.
4. **A2) `fixed`** — DONE 2026-04-28.
5. **A3) `any` + non-service `interface`** — DONE 2026-04-28.
6. **A4) `valuetype`** — DONE 2026-04-28.
4. **N) RTPS-over-TCP** — Constrained-Sensor-Imls **brennend**
   (User-Markierung 2026-04-28).
5. **L1) TIME 1.1 Audit** — "clocks sind ein brennendes thema
   immer" (User-Markierung). Cross-Ref zerodds-security/dcps-time.
6. **A2) `fixed`** — BigDecimal-Wrapper + XCDR-Decimal-Codec.
7. **H3) DDS4CCM Audit** — XML-QoS-Loader schon live.
8. **M2) WebSocket-Bridge** — Begleitspec zu H1 DDS-WEB.
9. **H1) DDS-WEB Audit + Implementation** — RESTful-Bridge.
10. **M5) ROS 2 RMW** — "ROS2 ist das womit alle werbung machen,
    wir muessen da mitziehen" (User). Eigener Sprint
    `crates/rmw-zerodds/`.
11. **M6) gRPC-Bridge** — "wie websockets etwas was uns abhebt"
    (User). Cloud-Bridge.
12. **M1) CoAP-Bridge** — IoT-Sensor-Bridge, Sensor-Sekundaerkunden.
13. **M3) MQTT-Bridge** — IoT-Pub-Sub-Bridge.
14. **M4) AMQP-Bridge** — Enterprise-Messaging-Bridge.
15. **A3) `any` + `valuetype` + non-service `interface`** — CORBA.
16. **I) DDS-TS** — TypeScript-Codegen + WASM/Node-FFI.
17. **L2-L4) AMI4CCM/CCM/RTC** — Audit-Files (jeweils klein).
18. **H4) DDS-OPCUA** — Industrie-Gateway.
19. **H2) DDS-TSN** — abhaengig von TSN-Hardware-Lab.
20. **C/D/E/F) Native Java-PSM** — groesster Brocken; eigener
    Sprint `crates/java-omgdds/`.

---

*Dieses Dokument ist die Single-Source-of-Truth fuer alle K10-K15-
Items, die Spec-konform aber nicht spec-vollstaendig waren. Audit-
Files referenzieren auf diese TODO-Liste.*
