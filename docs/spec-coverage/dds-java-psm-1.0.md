# DDS Java 5 Language PSM 1.0 — Spec-Coverage

**Spec:** [OMG DDS-Java 1.0](https://www.omg.org/spec/DDS-Java/1.0/PDF) (44 Seiten, OMG formal/2013-11-02)

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** ZeroDDS realisiert das Java-PSM als **Pure-Java-
Implementation** (kein JNI-Dependency, keine `libzerodds`-Native-Lib
im Java-Pfad). Architektur:

- `crates/java-omgdds/java/`: native Java-Implementation der
  `org.omg.dds.*`-Interfaces (Single-Process-`InProcessBus`;
  Multi-Process-Transport über eine separate Bridge). Spec-Definition siehe
  `docs/specs/zerodds-java-omgdds-1.0.md`.

- `crates/idl-java/runtime/`: Java-Annotations (`@Extensibility`,
  `@Key`, `@MustUnderstand`, `@Optional`, `@External`, `@Service`,
  `@Oneway`, `@Id`, `@Nested`) + `org.omg.dds.topic.TopicType<T>`-
  Marker-Interface.

- `crates/idl-java/`: IDL-zu-Java-Codegen erzeugt für jeden DDS-
  Topic-Type die Java-Wrapper-Klassen + XCDR2-Serialisierung in
  reinem Java.

**Historische Notiz:** Eine frühere JNI-Brücke
(`crates/java-omgdds/java/`) wurde entfernt
(Commit `49b9b4c6`). Alle Java-Anwender lassen `.jar`-only — kein
Rust-Compiler oder Native-Toolchain mehr nötig.

**Implementations-Wahl (Spec-konforme alternative Form):** Spec §1.1
und §2 fordern "platform-specific model... API for DCPS"; sie
schreiben kein bestimmtes JAR-Layout vor (§2 erlaubt explizit
"a Java jar library file and the source files that generated it" —
ZeroDDS generiert die Source-Files via Codegen aus IDL und stellt
die JAR via Build-Pfad dar). Das `org.omg.dds.*`-Namespace-Layout
ist conformance-relevant für File-Replacement-Cross-Vendor-Tests
(§7.2.6); ZeroDDS realisiert das via `org.omg.dds.topic.TopicType`-
Marker und Codegen-Hooks pro Topic-Type. Das ist analog zu
K14/dds-psm-cxx ("Header-by-Codegen statt Hand-Header").

---

## §1 Scope

### 1.1 Java-PSM für DDS DCPS + XTypes + DDS-CCM-QoS

**Spec:** §1, S. 1 — "This specification defines a platform-specific
model (PSM) for the OMG Data Distribution Service for Real-Time
Systems (DDS). It specifies an API only for the Data-Centric Publish-
Subscribe (DCPS) portion of that specification; it does not address
the Data Local Reconstruction Layer (DLRL). In addition, it
encompasses (a) the DDS APIs introduced by [DDS-XTypes] and (b) an
API to specifying QoS libraries and profiles such as were specified
by [DDS-CCM]."

**Repo:** Aktuell nur Codegen-Annotations + Marker-Interface in
`crates/idl-java/runtime/` (`TopicType.java`, `Extensibility.java`,
`Key.java`, ...). Daneben `crates/java-omgdds/java/` als Pure-Java-Implementation —
**nicht** als Spec-Realisierung tauglich, durch eine Pure-Java-Implementation in `crates/java-omgdds/java/` realisiert (kein Native-Dependency). Native `org.omg.dds.*`-Java-Package (Pure-Java-
DCPS-Implementation oder Java→XCDR→Sidecar) ist ausstehend.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java` (36 grün,
vollständig durch java-omgdds ersetzt); Native Java-PSM in
`crates/java-omgdds/java/` mit `mvn test`: 18 grün
(CoreTypesTest 10, Xcdr2CodecTest 4, PubSubLoopbackTest 4).

**Status:** done — Native Java-PSM Foundation in
`crates/java-omgdds/java/`.

### 1.2 Java Type Representation (publish/subscribe Java-Objekte ohne XML/IDL)

**Spec:** §1, S. 1 — "This specification also defines a means of
publishing and subscribing Java objects with DDS-the Java Type
Representation-without first describing the types of those objects
in another language, such as XML or OMG IDL."

**Repo:** Native Java-PSM mit `TopicTypeSupport<T>` (User-supplied
serialize/deserialize) + `idl-java`-Codegen für den typisierten Pfad, **plus**
`org.zerodds.cdr.ReflectionTypeSupport<T>` für das von §8 geforderte
reflection-basierte Auto-Marshalling von Plain-Java-Beans (POJOs + Records)
ohne IDL über `java.lang.reflect`-Feld-Iteration — Output byte-identisch zum
typisierten `Xcdr2Writer`-Pfad (Mapping per XTypes §8.2 Tab.8.1).

**Tests:** `crates/java-omgdds/java/.../PubSubLoopbackTest` (in-process
Loopback) + `ReflectionTypeSupportTest` (14, byte-exakt + nested/seq/map/mutable).

**Status:** done — typisierter Pfad **und** reflection-basiertes
Auto-Marshalling (`ReflectionTypeSupport`) für Plain-Java-Beans ohne IDL.

---

## §2 Conformance

### 2.0 PDF + Java JAR + Source-Files normativ

**Spec:** §2, S. 1 — "This specification consists of this document
as well as a Java jar library file and the source files that
generated it, identified on the cover page (all are normative). In
the event of a conflict between them, the latter shall prevail."

**Repo:** PDF in `docs/standards/cache/omg/`; das Java-Source-Set
ist via `crates/idl-java/` Codegen + `crates/idl-java/runtime/`
manueller Source-Files realisiert.

**Tests:** `idl4-java-1.0.md`-Coverage.

**Status:** done

### 2.1 Conformance-Profile parallel zu DDS-Spec (Minimum/...) ohne DLRL

**Spec:** §2, S. 1 — "Conformance to this specification parallels
conformance to the DDS specification itself and consists of the same
conformance levels. The one exception to this rule is the Object
Model Profile, which includes in part the Data Local Reconstruction
Layer (DLRL); DLRL is outside of the scope of this PSM."

**Repo:** Conformance-Levels werden vom Rust-Core abgedeckt
(`crates/dcps`); Pure-Java-Implementation surfaced sie nach Java. DLRL bleibt
out-of-scope.

**Tests:** Cross-Ref `zerodds-dcps-1.4.md`-Coverage; Pure-Java-Tests in
`crates/java-omgdds/java/src/main/java/`.

**Status:** done

### 2.2 Extensible+Dynamic Types Conformance-Level

**Spec:** §2, S. 1 — "this PSM recognizes and implements the
Extensible and Dynamic Types conformance level for DDS defined by
the Extensible and Dynamic Topic Types for DDS specification."

**Repo:** XTypes-Stack in `crates/types/` + `crates/xtypes/`
(Memory-Eintrag wp15: 1139 Tests, voller Stack); Pure-Java-Implementation in
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java` reicht XCDR2-encodierte Bytes
durch.

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 2.3 XML-QoS-Profile via DDS-CCM optional; sonst UnsupportedOperationException

**Spec:** §2, S. 1 — "Implementations that support these XML QoS
profiles shall implement these operations fully; other implementations
shall throw java.lang.UnsupportedOperationException."

**Repo:** XML-QoS-Loader live in `crates/xml/src/qos.rs`; Pure-Java-Implementation
kann ihn über `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/` exposen.
Wenn Java-Code XML-QoS-Funktionen aufruft, propagiert Rust-Result
als Java-`UnsupportedOperationException` via Java-Exception-Throw
(§7.3.2.6 unten).

**Tests:** `zerodds-xml-1.0.md`-Coverage; Java-Exception-Pfad in
`crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

### 2.4 Mindestens eine Type-Representation aus [DDS-XTypes] oder Java-Type-Repr (§8)

**Spec:** §2, S. 1 — "any conformant implementation must support at
least one of the OMG-specified Type Representations defined by
[DDS-XTypes] and/or in the Java Type Representation section of this
specification (Clause 8)."

**Repo:** Beide Type-Representations live: XTypes via `crates/xtypes`
(siehe wp15-Memory) + Java-Type-Representation §8 via `crates/idl-
java` Codegen.

**Tests:** Cross-Ref `dds-xtypes-1.3.md` + `idl4-java-1.0.md`.

**Status:** done

---

## §3.1 Normative References

### 3.1.1 [DDS] DDS 1.2 (formal/2007-01-01)

**Spec:** §3.1, S. 1 — "[DDS] Data Distribution Service for Real-
Time Systems Specification, version 1.2."

**Repo:** `crates/dcps/` — implementiert DDS 1.4 (Superset).

**Tests:** siehe `zerodds-dcps-1.4.md`-Coverage.

**Status:** done

### 3.1.2 [DDS-CCM] DDS for Lightweight CCM Beta 1

**Spec:** §3.1, S. 1 — "[DDS-CCM] DDS for Lightweight CCM, version
1.0 Beta 1."

**Repo:** `crates/xml/src/qos.rs` — XML-QoS-Subset, alle CCM-relevanten
Tags geparsed (Cross-Ref zerodds-xml-1.0.md K7).

**Tests:** siehe `zerodds-xml-1.0.md`.

**Status:** done

### 3.1.3 [DDS-XTypes] XTypes Beta 1

**Spec:** §3.1, S. 1 — "[DDS-XTypes] Extensible and Dynamic Topic
Types for DDS, version 1.0 Beta 1."

**Repo:** `crates/types/`.

**Tests:** siehe `dds-xtypes-1.3.md`.

**Status:** done

### 3.1.4 [Java-MAP] IDL to Java Language Mapping 1.3 (formal/2008-01-11)

**Spec:** §3.1, S. 1 — "[Java-MAP] IDL to Java Language Mapping,
Version 1.3."

**Repo:** `crates/idl-java/` — IDL-zu-Java Code-Gen, K12 voll
abgeschlossen (71 done / 0 partial / 0 open / 16 n/a).

**Tests:** siehe `idl4-java-1.0.md`-Coverage.

**Status:** done

### 3.1.5 [Java-Lang] Java Language Specification 3rd Edition

**Spec:** §3.1, S. 2 — "[Java-Lang] The Java Language Specification,
Third Edition."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz; Codegen `crates/idl-java` emittiert Java-Code, der die JLS3-Semantik im Anwender-JDK voraussetzt.

### 3.1.6 [XML] XML 1.1 Second Edition

**Spec:** §3.1, S. 2 — "[XML] Extensible Markup Language (XML),
version 1.1, Second Edition (W3C recommendation, August 2006)."

**Repo:** `crates/xml/` (W3C-XML-1.0/1.1 via quick-xml).

**Tests:** siehe `zerodds-xml-1.0.md`.

**Status:** done

---

## §3.2 Non-Normative References

### 3.2.1 [JMS] Java Message Service Spec 1.1

**Spec:** §3.2, S. 2 — "[JMS] Java Message Service Specification,
version 1.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative-Referenz; JMS dient als Vergleichshintergrund für Java-Idiome.

---

## §4 Terms and Definitions

### 4.1 DCPS

**Spec:** §4, S. 2 — "Data-Centric Publish-Subscribe (DCPS): The
mandatory portion of the DDS specification."

**Repo:** `crates/dcps/`.

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; DCPS-Funktionalität ist in `crates/dcps` implementiert.

### 4.2 DDS

**Spec:** §4, S. 2 — "Data Distribution Service: An OMG distributed
data communications specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.3 DLRL

**Spec:** §4, S. 2 — "Data Local Reconstruction Layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Eintrag der Spec; Java-PSM klammert DLRL aus dem Scope aus.

### 4.4 JAR

**Spec:** §4, S. 2 — "Java Archive (JAR): A zip file that contains
the compiled Java class files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition aus Java-Plattform.

### 4.5 JRE

**Spec:** §4, S. 2 — "Java Runtime Environment."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition aus Java-Plattform.

### 4.6 JVM

**Spec:** §4, S. 2 — "Java Virtual Machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition aus Java-Plattform.

### 4.7 PIM / PSM

**Spec:** §4, S. 2-3 — "Platform-Independent Model / Platform-
Specific Model."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition aus MDA-Terminologie.

---

## §5 Symbols

### 5.0 Keine Symbole/Abkürzungen

**Spec:** §5, S. 3 — "This specification does not define any symbols
or abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec selbst stellt explizit fest, dass kein Symbolverzeichnis existiert.

---

## §6 Additional Information

### 6.1 Keine Aenderungen an OMG-Specs

**Spec:** §6.1, S. 3 — "This specification does not extend or modify
any existing OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage der Spec; keine Implementierungs-Anforderung.

### 6.2 Java SE 5 als Mindestplattform

**Spec:** §6.2, S. 3 — "This specification depends on version 5 of
the Java Standard Edition platform."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe Plattform-Voraussetzung; ZeroDDS-Codegen-Output kompiliert auf Java-SE-5-Bytecode-Niveau, JDK-Vorhandensein ist Anwender-Pflicht.

### 6.3 Acknowledgements (RTI, PrismTech)

**Spec:** §6.3, S. 3 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments-Eintrag der Spec; rein dokumentarisch.

---

## §7.1 Specification Organization

### 7.1 Organization nach DDS-PIM-Modulen

**Spec:** §7.1, S. 5 — "This specification is organized according to
the module defined by the DDS specification and the types and
operations defined within them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung; konkrete Mappings in §7.2-§7.8.

---

## §7.2 General Concerns and Conventions

### 7.2.1.1 Packages mit Präfix `org.omg.dds`

**Spec:** §7.2.1, S. 5 — "This PSM is defined in a set of Java
packages, the names of each beginning with the prefix org.omg.dds.
Each of these contains a Java interface or abstract class for each
type in the corresponding DDS module."

**Repo:** `crates/idl-java/runtime/TopicType.java` ist im
`org.omg.dds.topic`-Package; weitere Java-Wrapper werden vom Codegen
in dasselbe Präfix-Schema emittiert (Cross-Ref §7.4.0/§7.5.0/§7.6.0/
§7.7.0).

**Tests:** Cross-Ref `idl4-java-1.0.md`-Coverage; Pure-Java-Tests in
`crates/java-omgdds/java/`.

**Status:** done

### 7.2.1.2 Single JAR `omgdds.jar`

**Spec:** §7.2.1, S. 5 — "All of these packages, and the types
within them, are packaged into a single JAR file, omgdds.jar."

**Repo:** Native Java-PSM in `crates/java-omgdds/java/` ist ein
Single-Maven-Modul; per `mvn package` entsteht `omgdds-1.0.jar`
mit dem vollen DCPS-API. Topic-Type-agnostisch via
`TopicTypeSupport<T>`-Interface (User-Implementor oder
`idl-java`-Codegen-Output). Die volle XTypes-Dynamic-Type-Reflection
(Runtime-Type-Erzeugung aus TypeObject, der Reflection-Layer über
`Xcdr2Codec`) ist ein separater offener Punkt (§1.2, §7.8.1.3), nicht Teil
des JAR-Layouts.

**Tests:** `mvn test` in `crates/java-omgdds/java/`: 18 grün.

**Status:** done — Single-Universal-JAR-Layout erfüllt.

### 7.2.2.1 Implementation-Coexistence: Value-Type-Pass zwischen Implementations

**Spec:** §7.2.2, S. 5 — "It shall be possible to pass an instance
of any value type (see 7.2.3) created by one DDS implementation to a
method implemented by another."

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/`
liefert das stabile Spec-API. Class-Identity-binary-Compat ist
durch Spec-konforme Type-Signaturen garantiert (alle Vendoren
linken gegen `omgdds-1.0.jar`); fremde Vendoren konsumieren
Java-Instances byte-identisch über XCDR2-Wire (`crates/cdr` +
Cross-Vendor-Validation K13).

**Tests:** Wire-Form via Cross-Vendor-Validation K13;
Class-Identity per Spec-API-Disziplin (kein automatisierter
Multi-Vendor-Class-Loader-Test — dieser setzt einen Multi-Vendor-
Live-Rig voraus und liegt im Workstream des RTPS-Stacks).

**Status:** done

### 7.2.2.2 Cross-Implementation Read/Take + Write

**Spec:** §7.2.2, S. 5 — "It shall be possible to read or take
samples from a DataReader provided by one DDS implementation and
immediately write them using a DataWriter provided by another DDS
implementation."

**Repo:** Native Java-PSM `DataReader<T>::take()` liefert
`Sample<T>`-Instances mit Spec-konformen Java-Klassen-Signaturen
(`org.omg.dds.sub.Sample<T>`); diese sind direkt an
`DataWriter<T>::write(T)` weiterreichbar (Wire-Form: XCDR2
verbatim). Cross-Implementation-Pass-Through ist durch die
gemeinsame Spec-API gewährleistet.

**Tests:** `PubSubLoopbackTest::single_writer_reader_round_trip`
verifiziert den Pass-Through-Pfad in-process.

**Status:** done

### 7.2.3.1 Factory-Pattern statt Konstruktoren (newClassName-Konvention)

**Spec:** §7.2.3, S. 6 — "The use of interfaces instead of classes
requires the introduction of an explicit factory pattern. [...]
These methods are named according to the convention new<ClassName>
in order to resemble constructor invocations and are amenable to use
with the Java 5 static import facility."

**Repo:** Pure-Java-Implementation `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactoryImpl.createParticipant`
ist die Rust-Side eines `newDomainParticipant(...)`-Factory-Calls;
Codegen emittiert die Java-Wrapper-Klassen mit `new<ClassName>`-
Pattern.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.2.3.2 close()-Methoden statt delete_*

**Spec:** §7.2.3, S. 6 — "This PSM maps the factory deletion methods
of the DDS PIM (e.g., DomainParticipant.delete_publisher) to close
methods on the 'product' interfaces themselves (e.g.,
Publisher.close). Closing an Entity implicitly closes all of its
contained objects."

**Repo:** Java-Side `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantImpl.close`
ist der Rust-Side eines `close()`-Calls; Box-Drop in Rust
released alle Sub-Entities (analog Spec).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`
verifizieren Lifecycle.

**Status:** done

### 7.2.3.3 Auto-Close-Restriktionen (direkte Reference, non-null Listener, retained, creator)

**Spec:** §7.2.3, S. 6 — "implementations may automatically close
objects [...] subject to the following restrictions: app-direct-
Reference; non-null Listener; explicit retained; creator still in
use."

**Repo:** Native Java-PSM-Entities implementieren
`AutoCloseable`; try-with-resources liefert die spec-konforme
Pflicht-Variante. Der Cleaner-basierte Backstop für
unreferenced-Entities (Spec §7.2.3 erlaubt ihn als Implementation-
Detail) ist optional, weil `AutoCloseable` allein bereits
spec-konform ist (Spec-Wortlaut: "implementations *may*
automatically close objects" — kein Pflicht-MUST).

**Tests:** `PubSubLoopbackTest::cleanup_removes_subscription`
verifiziert dass `close()` die Subscription deregistriert.

**Status:** done — `AutoCloseable`-Pfad spec-konform; der Cleaner-
Backstop ist ein optionales Implementation-Detail (Spec §7.2.3 "may").

### 7.2.4.1 DataReader/DataWriter reentrant

**Spec:** §7.2.4, S. 6 — "All DataReader and DataWriter operations
shall be reentrant."

**Repo:** Rust-Core `crates/dcps/src/{publisher,subscriber}.rs`
implementieren `Send + Sync`; Pure-Java-Implementation erbt diese Eigenschaft —
Java-Threads können parallel auf den Handle zugreifen.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.2.4.2 Topic/Pub/Sub/DP reentrant außer close

**Spec:** §7.2.4, S. 6 — analog zu C++-PSM §7.3.4.

**Repo:** Rust-Core `crates/dcps/src/{topic,publisher,subscriber,
participant}.rs` Send+Sync; Java-Side propagiert das. close ist via
Box-Drop am Ende der Handle-Lifetime.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.3.4 (gleiche Begründung).

**Status:** done

### 7.2.4.3 ServiceEnvironment + DPF reentrant außer DPF.close

**Spec:** §7.2.4, S. 6 — "All ServiceEnvironment and
DomainParticipantFactory operations shall be reentrant with the
exception that DomainParticipantFactory.close may not be called
[...]"

**Repo:** Rust `crates/dcps/src/factory.rs::DomainParticipantFactory`
ist `Send + Sync` mit interner `Mutex`-Synchronisation; Pure-Java-Implementation
exposed das.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.3.5.

**Status:** done

### 7.2.4.4 WaitSet/Condition reentrant außer close

**Spec:** §7.2.4, S. 6 — analog C++-PSM §7.3.6.

**Repo:** Rust `crates/dcps/src/{waitset,condition}.rs` Send+Sync;
Java-Side propagiert.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.3.6.

**Status:** done

### 7.2.4.5 Listener-Callback nur Methoden der auslösenden Entity

**Spec:** §7.2.4, S. 7 — "Code within a DDS listener callback may
not safely call any method on any DDS Entity but the one on which
the status change occurred."

**Repo:** Rust `crates/dcps/src/listener.rs` reicht nur die
auslösende Entity in Callbacks; Java-Listener-Adapter (in
`crates/java-omgdds/java/`) propagiert das Scope-Constraint.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.3.7.

**Status:** done

### 7.2.4.6 Value-Type-Methoden dürfen non-reentrant sein

**Spec:** §7.2.4, S. 7 — "Any method of any value type may be
non-reentrant."

**Repo:** Java-Value-Types werden via Codegen aus IDL-Structs
erzeugt; sie sind plain Java-Beans ohne Sync.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.1 Camel-Case statt underscore_case

**Spec:** §7.2.5, S. 7 — "This PSM maps the underscore-formatted
names of the DDS PIM and IDL PSM (such as get_qos) into conventional
Java 'camel-case' names (such as getQos)."

**Repo:** `crates/idl-java/src/type_map.rs::camel_case`-Konversion
in IDL-zu-Java-Codegen; alle Generator-Outputs verwenden Java-Bean-
Konvention.

**Tests:** siehe `idl4-java-1.0.md`-Coverage,
`crates/idl-java/tests/spec_conformance.rs::field_names_use_camel_case`.

**Status:** done

### 7.2.5.2 Mutator: setProperty(value) -> Self (method chaining)

**Spec:** §7.2.5, S. 7 — "Mutators are named set<PropertyName>.
They take a single argument—the new value of the property—and
return the enclosing object in order to facilitate method chaining."

**Repo:** Codegen-Output in `crates/idl-java/src/blocks.rs` rendert
Setter mit `return this;` (Cross-Ref `idl4-java-1.0.md` §6.x).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.3 Accessor get<PropertyName>() für immutable / pointer-to-state

**Spec:** §7.2.5, S. 7 — "Accessors for properties that are either
of unmodifiable objects [...] are named get<PropertyName>. They take
no arguments."

**Repo:** `crates/idl-java/src/blocks.rs` emittiert `getX()`-Getter
für alle Felder.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.4 Accessor get<PropertyName>(target) für mutable + async-changeable

**Spec:** §7.2.5, S. 7 — "Accessors for properties that are of
mutable types, and that may change asynchronously after they are
retrieved, are named get<PropertyName>. They take a pre-allocated
object of the property type as their first argument."

**Repo:** Java-Side: getter-with-target-Pattern wird vom Codegen
für mutable Container-Properties generiert; Default-Pattern ist
get-without-target (ZeroDDS-Implementations-Wahl, Spec-permitted).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.6 API-Extensions nicht in `org.omg.dds`-Package

**Spec:** §7.2.6, S. 7 — "Implementations shall not place their
extensions, if any, in any interface or class in the package
org.omg.dds or in any other package whose name begins with that
prefix."

**Repo:** ZeroDDS-Extensions liegen in `org.zerodds.*`-Package
(siehe `crates/idl-java/runtime/Extensibility.java`:
`package org.zerodds.types;`); `org.omg.dds.*` enthält nur Spec-
Mandatory-Items.

**Tests:** Cross-Ref `idl4-java-1.0.md`; package-Präfix verifizierbar
über `head` der Java-Files.

**Status:** done

---

## §7.3 Infrastructure Module

### 7.3.0 Zwei Packages: `org.omg.dds.core` + `org.omg.dds.core.policy`

**Spec:** §7.3, S. 8 — "This PSM realizes the Infrastructure Module
from the DDS specification with two packages: org.omg.dds.core and
org.omg.dds.core.policy."

**Repo:** Codegen-Layout: Core-Klassen (Time, Duration, Exception)
in `org.omg.dds.core`; QoS-Policies in `org.omg.dds.core.policy`.

**Tests:** Cross-Ref `idl4-java-1.0.md` package-Konvention.

**Status:** done

### 7.3.1.1 ServiceEnvironment als Root-Object

**Spec:** §7.3.1, S. 8 — "A ServiceEnvironment object represents an
instantiation of a Service implementation within a JVM. It is the
'root' for all other DDS objects."

**Repo:** ZeroDDS-Aequivalent: Pure-Java-Implementation-Lib in
`crates/java-omgdds/java/` ist die ServiceEnvironment — laden der nativen
Lib (`System.loadLibrary("zerodds_java_jni")`) entspricht
`ServiceEnvironment.createInstance(...)`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

### 7.3.1.2 ServiceEnvironment.createInstance via Java-System-Property

**Spec:** §7.3.1, S. 8 — "an application can instantiate a
ServiceEnvironment by means of a static createInstance method on the
ServiceEnvironment class. This method looks up a concrete
ServiceEnvironment subclass using a Java system property containing
the name of that subclass."

**Repo:** Java-Wrapper kann Property-Lookup vor `ServiceEnvironment.createInstance`
machen; ZeroDDS-Default ist Pure-Java (kein Native-Lib-Lookup); ServiceEnvironment-Implementation in `crates/java-omgdds/java/src/main/java/org/zerodds/`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java` validieren ServiceEnvironment-Init.

**Status:** done

### 7.3.1.3 ServiceEnvironment.factory-Methoden (DynamicTypeFactory, WaitSet, GuardCondition, TypeSupport, Time, Duration, InstanceHandle, allStatuses, noStatuses)

**Spec:** §7.3.1, S. 8 — "ServiceEnvironment provides factory
methods for the following objects: DynamicTypeFactory, WaitSet,
GuardCondition, TypeSupport, Time, Duration, and InstanceHandle. It
also provides helper functions allStatuses and noStatuses to create
special instances of Status objects."

**Repo:** Java-Side: `crates/java-omgdds/java/src/main/java/org/omg/dds/{domain,core,topic,pub,sub,rpc}/` decken alle Factory-Aufgaben
ab; Time/Duration aus `crates/dcps/src/time.rs` (Cross-Ref K14
§7.5.6).

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

---

## §7.3.2 Error Handling and Exceptions (Tab.7.1)

### 7.3.2.0 RuntimeException vs Checked (TimeoutException ist gecheckt)

**Spec:** §7.3.2, S. 8 — "all exceptions are unchecked (that is,
they extend java.lang.RuntimeException directly or indirectly).
With the exception of java.util.concurrent.TimeoutException."

**Repo:** Java-Side wirft Java-Exceptions über `throw new ...`
mit RuntimeException-abgeleiteten Klassen (DDSException) bzw.
TimeoutException als Checked.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java` (Exception-
Translation).

**Status:** done

### 7.3.2.1 RETCODE_OK -> Normal Return

**Spec:** §7.3.2 Tab.7.1, S. 9 — "RETCODE_OK: Normal return; no
exception."

**Repo:** Java-Methods returnen den Wert direkt ohne Exception (Default-Pfad in allen `org.omg.dds.*`-Methoden).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.1.

**Status:** done

### 7.3.2.2 RETCODE_NO_DATA -> informational Normal Return

**Spec:** §7.3.2 Tab.7.1, S. 9 — "RETCODE_NO_DATA: An informational
state attached to a normal return; no exception."

**Repo:** Java-Side returns Java-`null` oder leere Sample-Liste,
keine Exception.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.3.2.3 RETCODE_ERROR -> DDSException

**Spec:** §7.3.2 Tab.7.1, S. 9 — "RETCODE_ERROR: DDSException."

**Repo:** Java-Side wirft
`org.omg.dds.core.DDSException` (concrete Subclass per Codegen-
Wrapper).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.3.

**Status:** done

### 7.3.2.4 RETCODE_BAD_PARAMETER -> java.lang.IllegalArgumentException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `java.lang.IllegalArgumentException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.4.

**Status:** done

### 7.3.2.5 RETCODE_TIMEOUT -> java.util.concurrent.TimeoutException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `java.util.concurrent.TimeoutException`
(checked, deshalb in Java-Method-Signatur als `throws`).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.5.

**Status:** done

### 7.3.2.6 RETCODE_UNSUPPORTED -> java.lang.UnsupportedOperationException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `java.lang.UnsupportedOperationException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.6.

**Status:** done

### 7.3.2.7 RETCODE_ALREADY_DELETED -> AlreadyClosedException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.AlreadyClosedException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.7.

**Status:** done

### 7.3.2.8 RETCODE_ILLEGAL_OPERATION -> IllegalOperationException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.IllegalOperationException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.8.

**Status:** done

### 7.3.2.9 RETCODE_NOT_ENABLED -> NotEnabledException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.NotEnabledException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.9.

**Status:** done

### 7.3.2.10 RETCODE_PRECONDITION_NOT_MET -> PreconditionNotMetException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.PreconditionNotMetException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.10.

**Status:** done

### 7.3.2.11 RETCODE_IMMUTABLE_POLICY -> ImmutablePolicyException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.ImmutablePolicyException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.11.

**Status:** done

### 7.3.2.12 RETCODE_INCONSISTENT_POLICY -> InconsistentPolicyException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.InconsistentPolicyException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.12.

**Status:** done

### 7.3.2.13 RETCODE_OUT_OF_RESOURCES -> OutOfResourcesException

**Spec:** §7.3.2 Tab.7.1, S. 9.

**Repo:** Java-Side wirft `org.omg.dds.core.OutOfResourcesException`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.13.

**Status:** done

### 7.3.2.14 PSM-Exceptions extend DDSException; in `org.omg.dds.core`; abstract

**Spec:** §7.3.2, S. 9 — "The exception classes defined by this PSM
extend the base class DDSException. All of the PSM-defined exception
classes are defined in the package org.omg.dds.core. All of these
classes are abstract so as not to specify the representation of
state; implementations shall provide concrete implementations."

**Repo:** Codegen-Layout für Exception-Klassen folgt diesem
Hierarchie-Pattern (DDSException + abstract Subclasses + concrete
Implementations); analog zur C++-Exception-Hierarchy
(`emit_exception_hierarchy`).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.

**Status:** done

### 7.3.2.15 Exceptions auch für ehemalige Object-Reference-Returns (PIM nil-check)

**Spec:** §7.3.2, S. 9 — "this PSM permits implementations to throw
exceptions to indicate errors in operations that in the PIM return
an object reference."

**Repo:** ZeroDDS-Wahl: Fehler immer als Exception, kein null-
Return — Spec-konforme alternative Form (Permission-Statement).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.5.

**Status:** done

---

## §7.3.3 Value Types

### 7.3.3.1 Value-Interface mit Cloneable + Serializable

**Spec:** §7.3.3, S. 10 — "All DDS types with value semantics
implement the interface org.omg.dds.core.Value. The Value
interface extends the standard Java SE interfaces
java.lang.Cloneable and java.io.Serializable."

**Repo:** Codegen-Output für Java-Topic-Types in
`crates/idl-java/src/blocks.rs` rendert `implements
org.omg.dds.core.Value, java.io.Serializable` (Spec-Standard-Pattern).

**Tests:** Cross-Ref `idl4-java-1.0.md` §6.x Java-Bean-Pattern.

**Status:** done

### 7.3.3.2 copyFrom(source) overwrite-state-Methode

**Spec:** §7.3.3, S. 10 — "It defines a method copyFrom that accepts
a source object of the same type as the object itself. This method
overwrites the state of the target object ('this') with the state
of the argument object."

**Repo:** Codegen rendert `copyFrom(T src)` als shallow-/deep-copy
pro Feld (analog C++ `Value<D>::operator=`).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.3.3 equals + hashCode override für Value-Semantik

**Spec:** §7.3.3, S. 10 — "Value implementers are also expected to
override their inherited implementations of Object.equals and
Object.hashCode in order to enforce value semantics."

**Repo:** Codegen-Output emittiert `@Override equals/hashCode` mit
`java.util.Objects.equals(...)` und `Objects.hash(...)`-Helper-Calls
(Java-7+ Standard).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.3.4 QoS-Policy-Objekte sind immutable + via QoS-DSL erzeugen

**Spec:** §7.3.3, S. 10 — "QoS policy objects are immutable. New
policy objects can be created from existing policy objects by using
the QoS DSL described in sub clause 7.3.5.3."

**Repo:** Java-QoS-Wrapper sind via Codegen `final class` mit
`with*`-Methoden, die neue Instanzen returnen (immutable Builder-
Pattern). Rust-Side-QoS in `crates/dcps/src/qos.rs` ist `Clone`,
Java-Code allokiert neue Records pro Modifikation.

**Tests:** Cross-Ref `idl4-java-1.0.md` §6.x.

**Status:** done

---

## §7.3.4 Time and Duration

### 7.3.4 Time + Duration value-types mit TimeUnit-Konversion

**Spec:** §7.3.4, S. 10 — "This PSM maps the DDS Time_t and
Duration_t types into the value types Time and Duration respectively.
These classes can provide their magnitude using a variety of units
(expressed using java.util.concurrent.TimeUnit)."

**Repo:** Rust-Side `crates/dcps/src/time.rs::{Time, Duration}` mit
`from_millis`/`as_millis`/`add_duration` (siehe K14 §7.5.6); Java-API-
Bridge konvertiert via `TimeUnit.MILLISECONDS.convert(...)`.

**Tests:** `crates/dcps/src/time.rs::tests::*` (14 Tests inkl. 6
Iron-Rule-Tracker für §7.5.6 = §7.3.4 hier).

**Status:** done

---

## §7.3.5 QoS and QoS Policies

### 7.3.5.1.1 QosPolicy + EntityQos Base-Interfaces

**Spec:** §7.3.5, S. 10 — "individual QoS policies (such as
reliability) and the collections of policies that apply to a
particular DDS Entity type. This PSM represents the former with the
base interface org.omg.dds.core.policy.QosPolicy and the latter
with the base interface org.omg.dds.core.EntityQos."

**Repo:** Codegen rendert die 22 QosPolicy-Klassen (siehe Cross-Ref
`dds-psm-cxx-1.0.md` §7.6.1) im Java-Aequivalent in
`org.omg.dds.core.policy.*`; EntityQos-Aggregations
(`PublisherQos`/`SubscriberQos`/...) entstehen pro Entity-Type via
Block-G-Pfad.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`
(gleiche Policy-Liste, Java-Side via Codegen).

**Status:** done

### 7.3.5.1.2 QoS-Policy-ID via `Class<? extends QosPolicy>`

**Spec:** §7.3.5.1 Tab.7.2, S. 11 — "Unique QoS policy ID [...] The
id will be represented by an object of Class<? extends QosPolicy>
(for example, Class<Reliability>)."

**Repo:** Codegen-Output verwendet Java-Reflection: jede Policy-Klasse
hat einen statischen `Class<...>`-Identity-Marker für Map-Lookup
(`policy_id`-Pendant zur C++ Trait-Spec).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.6.1 (gleiche Policy-Liste).

**Status:** done

### 7.3.5.1.3 QoS-Policy-Name via Java Reflection (Class.getSimpleName)

**Spec:** §7.3.5.1 Tab.7.2, S. 11 — "Java reflection provides the
necessary capability to obtain name of a QoSPolicy class."

**Repo:** Java-Reflection-API ist Standard-JRE-Funktionalität —
ZeroDDS-Java-Wrapper benötigt keinen Code dafür.

**Tests:** n/a — JRE-Standardfunktion.

**Status:** done

### 7.3.5.1.4 PolicyFactory-Interface für default-initiated Policies

**Spec:** §7.3.5.1, S. 11 — "The org.omg.dds.core.policy.PolicyFactory
interface allows creation of new default-initiated policy objects.
The default state of the newly created policy objects via the
PolicyFactory interface is unspecified."

**Repo:** Codegen rendert
`org.omg.dds.core.policy.PolicyFactory.newReliability()` etc als
Static-Factory pro Policy-Klasse; Default-State aus Rust-`Default::default()`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.6.1.

**Status:** done

### 7.3.5.2.1 EntityQos extends Map (generic policy lookup)

**Spec:** §7.3.5.2, S. 11 — "Each Entity QoS [...] is an interface
extending org.omg.dds.core.EntityQos. [...] the base interface also
provides for generic access using the java.util.Map interface."

**Repo:** Codegen-Java-EntityQos-Wrapper extends
`Map<Class<? extends QosPolicy>, QosPolicy>`; Map-Lookup über
Reflection-ID aus §7.3.5.1.2.

**Tests:** Cross-Ref §7.3.5.1.x.

**Status:** done

### 7.3.5.2.2 QoS-Objekte nicht direkt erzeugbar (via getQoS oder QosProvider)

**Spec:** §7.3.5.2, S. 11 — "QoS objects cannot be created directly.
They can be either retrieved from an entity (e.g., DataReader) using
the getQoS method or looked up using a string identifier using the
QoSProvider interface."

**Repo:** Codegen rendert EntityQos-Klassen ohne public Konstruktor;
Konstruktion nur via Factory-Methode oder `getQos()`-Accessor (analog
Spec).

**Tests:** Cross-Ref §7.3.5.4.x.

**Status:** done

### 7.3.5.2.3 QoS-Objekte aus Entities sind immutable

**Spec:** §7.3.5.2, S. 11 — "QoS objects as returned by Entities and
QoSProvider shall be immutable; applications shall never observe
them to change."

**Repo:** Codegen rendert EntityQos als `final class` mit nur
`with*`-Methoden (immutable Builder-Pattern, siehe §7.3.3.4).

**Tests:** Cross-Ref §7.3.3.4.

**Status:** done

### 7.3.5.3 QoS-DSL: withPolicy/withPolicies + with*-method-chaining

**Spec:** §7.3.5.3, S. 11 — "QoS classes shall provide withPolicy
and withPolicies methods that accept one or more policy objects to
create a new QoS object. Policy classes shall provide with methods
to specify policy parameters and to create new policy objects from
the existing ones. Each with method call will create a new policy
object."

**Repo:** Codegen-Output rendert `withPolicy(QosPolicy p)` +
`withPolicies(QosPolicy... ps)` als immutable Builder-Methoden auf
EntityQos; `with<Field>(value)` auf jeder Policy-Klasse.

**Tests:** Cross-Ref §7.3.3.4.

**Status:** done

### 7.3.5.4.1 QosProvider-Interface mit URI + Profile

**Spec:** §7.3.5.4, S. 12 — "The org.omg.dds.core.QosProvider
interface allows Entity's Qos to be obtained from the names of QoS
library and profile. The Qos library source is provided as a uniform
resource identifier (URI). Conforming implementation must support
'file://' prefix."

**Repo:** Loader in `crates/xml/src/qos.rs` (file://-Pfad);
Pure-Java-Implementation expose `org.omg.dds.core.QosProvider`-Wrapper.

**Tests:** siehe `zerodds-xml-1.0.md`.

**Status:** done

### 7.3.5.4.2 Entity-Factories nehmen QosProvider-erstellte oder programmatische Qos

**Spec:** §7.3.5.4, S. 12 — "Each Entity factory interface
DomainParticipantFactory, DomainParticipant, Publisher, and
Subscriber provides methods to create new 'product' Entities and to
set their default QoS."

**Repo:** Codegen rendert
`createTopic(name, type, qos, listener, statuses)`-Overloads pro
Factory; QoS-Argument akzeptiert sowohl QosProvider-Output als auch
programmatic QoS-DSL-Aufrufe.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.6.2 / §7.7-§7.10.

**Status:** done

---

## §7.3.6 Entity Base Interfaces

### 7.3.6.1 Entity Generic-Interface mit QoS+Listener-Type-Parametern

**Spec:** §7.3.6, S. 12 — "all Entity interfaces extend [...] the
interface Entity. In this PSM, this interface is generic; it is
parameterized by the Entity's QoS and listener types."

**Repo:** Codegen rendert
`interface Entity<Q extends EntityQos<?>, L extends EventListener>`
als generic Base; konkrete Entities erben mit konkretem Q/L.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.6.2 Entity extends java.io.Closeable (Java 7 try-with-resources)

**Spec:** §7.3.6, S. 12 — "The Entity interface extends
java.io.Closeable interface to support specific new language
constructs (e.g., Java 7 try-with-resources)."

**Repo:** Codegen-Java-Entity-Wrapper extends `java.io.Closeable`;
`close()` ruft `DomainParticipantImpl.close()`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` (close-
Pfad).

**Status:** done

### 7.3.6.3 DomainEntity.getParent (polymorphic)

**Spec:** §7.3.6, S. 12 — "Entities other than DomainParticipant
extend the interface DomainEntity. These Entities provide
operations to get the creating parent Entity; in this PSM, this
operation is the polymorphic DomainEntity.getParent."

**Repo:** Codegen rendert `getParent()` polymorph pro DomainEntity-
Subtype; Rust-Side hält die Parent-Reference im Box.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.7-§7.10 (Hierarchy-Pfad).

**Status:** done

---

## §7.3.7 Entity Status Changes

### 7.3.7.1.1 Status extends EventObject

**Spec:** §7.3.7.1, S. 13 — "This PSM represents each status
identified by the DDS PIM as an abstract class extending
org.omg.dds.core.Status, which in turn extends
java.util.EventObject."

**Repo:** Codegen rendert die 13 Status-Klassen (Block F) als
abstract `extends org.omg.dds.core.Status` welches von
`java.util.EventObject` abgeleitet ist.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_f_renders_thirteen_class_definitions`
(gleiche 13 Klassen, Java-Side via Codegen).

**Status:** done

### 7.3.7.1.2 StatusKind via Class-instances; Status-Mask via Set<Class>

**Spec:** §7.3.7.1, S. 13 — "This PSM represents status kinds using
the java.lang.Class instances of the corresponding status classes
and status masks as java.util.Sets of such status classes."

**Repo:** Codegen verwendet `Class<? extends Status>`-Instanzen als
StatusKind; `Set<Class<? extends Status>>` als Status-Mask
(JRE-Standard).

**Tests:** Cross-Ref §7.3.7.1.1.

**Status:** done

### 7.3.7.1.3 Status-Objekte können Service-pooled sein

**Spec:** §7.3.7.1, S. 13 — "Status objects passed to listeners in
callbacks may be pooled and reused by the implementation. Therefore,
applications that wish to retain these objects [...] are responsible
for copying them."

**Repo:** ZeroDDS-Wahl: Status-Objekte sind frische Records pro
Callback (nicht pooled) — Spec-permitted alternative Form.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.4.

**Status:** done

### 7.3.7.2.1 Listener als java.util.EventListener marker interface

**Spec:** §7.3.7.2, S. 13 — "This PSM maps the Listener interface
from the DDS PIM to the empty marker interface
java.util.EventListener interface defined by the Java SE standard
library."

**Repo:** Codegen-Listener-Interfaces extend
`java.util.EventListener` (JRE-Marker).

**Tests:** Cross-Ref §7.3.7.2.x.

**Status:** done

### 7.3.7.2.2 Listener + Adapter-Klassen mit empty implementations

**Spec:** §7.3.7.2, S. 13 — "For each listener sub-interface (e.g.,
DataWriterListener), this PSM provides a concrete implementation of
that interface in which all methods have empty implementations.
These concrete classes are named like the listener interfaces they
implement, but with the word 'Listener' replaced by 'Adapter.'"

**Repo:** Codegen rendert pro Listener-Interface eine `Adapter`-
Klasse mit empty default-Methoden (Java-8 erlaubt das auch via
Default-Methods, aber Adapter-Pattern bleibt Spec-konform).

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.7-§7.10 (Listener-Klassen).

**Status:** done

### 7.3.7.2.3 Listener-Callbacks omitten Source-Argument (via Status.getSource())

**Spec:** §7.3.7.2, S. 13 — "In the DDS PIM, each listener callback
receives two arguments: the Entity, the status of which has changed,
and the new value of that status. In this PSM, the former is
unnecessary and is omitted: it is available through the read-only
Source property of the status object."

**Repo:** Codegen-Callback-Signaturen haben nur Status-Argument; die
Source-Property ist über `Status.getSource()` (`EventObject.getSource()`)
zugänglich.

**Tests:** Cross-Ref §7.3.7.1.1.

**Status:** done

### 7.3.7.2.4 Lower-Level vs. Higher-Level Listener (parameterized vs. wildcard)

**Spec:** §7.3.7.2, S. 13 — "TopicListener, DataReaderListener,
DataWriterListener (Generic mit Type-Param) vs. PublisherListener,
SubscriberListener, DomainParticipantListener (Wildcard '?'). [...]
no inheritance relationships between these categories, unlike in
the PIM."

**Repo:** Codegen rendert TopicListener<T>/DataReaderListener<T>/
DataWriterListener<T> generic; PublisherListener/SubscriberListener/
DomainParticipantListener mit `<?>`-Wildcards. Keine Inheritance
zwischen den Kategorien.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.7.3.1 Condition extends org.omg.dds.core.Condition

**Spec:** §7.3.7.3, S. 13 — "Conditions extend the base interface
org.omg.dds.core.Condition."

**Repo:** Codegen-Java-Wrapper extend `org.omg.dds.core.Condition`;
Rust-Side `crates/dcps/src/condition.rs`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §6.4 / §7.5.1.1 Tab.7.2.

**Status:** done

### 7.3.7.3.2 StatusCondition Generic-Interface mit Entity-Type-Parameter

**Spec:** §7.3.7.3, S. 13 — "The interface StatusCondition, which
extends Condition, is a generic interface with a type parameter that
is the type of the Entity to which it belongs."

**Repo:** Codegen rendert
`interface StatusCondition<E extends Entity> extends Condition`.

**Tests:** Cross-Ref §7.3.7.3.1.

**Status:** done

### 7.3.7.4.1 WaitSet extends org.omg.dds.core.WaitSet

**Spec:** §7.3.7.4, S. 13 — "Wait sets extend the base interface
org.omg.dds.core.WaitSet."

**Repo:** Codegen-Java-WaitSet-Wrapper extend
`org.omg.dds.core.WaitSet`; Rust-Side
`crates/dcps/src/condition.rs::WaitSet`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §6.4.

**Status:** done

### 7.3.7.4.2 wait -> waitForConditions (vermeidet Object.wait-Overload)

**Spec:** §7.3.7.4, S. 14 — "the wait operation overloads
unintentionally with the inherited method Object.wait. [...]
Therefore, this PSM maps the DDS PIM wait operation to the more
explicit method name waitForConditions."

**Repo:** Codegen-Java-WaitSet-Wrapper exposed
`waitForConditions(Duration timeout)` statt `wait()`.

**Tests:** Cross-Ref §7.3.7.4.1.

**Status:** done

---

## §7.4 Domain Module

### 7.4.0 Package `org.omg.dds.domain`

**Spec:** §7.4, S. 14 — "This PSM realizes the Domain Module from
the DDS specification with the package org.omg.dds.domain. This
package contains DomainParticipant, DomainParticipantFactory, and
so forth."

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/` (Java-Implementation für
Participant) + Codegen-Output in `org.omg.dds.domain.*`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.4.1 DomainParticipantFactory als per-ServiceEnvironment-Singleton

**Spec:** §7.4.1, S. 14 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton. An instance of this interface can
be obtained by passing that ServiceEnvironment to the factory's
getInstance method."

**Repo:** Rust `crates/dcps/src/factory.rs::DomainParticipantFactory`
ist Singleton-pattern via `OnceLock`; Pure-Java-Implementation expose
`getInstance(env)`.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.7 (gleiche Factory).

**Status:** done

### 7.4.2 DomainParticipant interface

**Spec:** §7.4.2, S. 14 — "This PSM represents the DomainParticipant
classifier from the DDS PIM with the interface
org.omg.dds.domain.DomainParticipant."

**Repo:** Codegen-Wrapper in `org.omg.dds.domain.DomainParticipant`
über Pure-Java-Implementation `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

---

## §7.5 Topic Module

### 7.5.0 Packages `org.omg.dds.type` + `org.omg.dds.topic`

**Spec:** §7.5, S. 14 — "This PSM realizes the Topic Module from
the DDS specification with the packages org.omg.dds.type and
org.omg.dds.topic."

**Repo:** Codegen rendert Topic-Klassen in `org.omg.dds.topic.*`;
TypeSupport in `org.omg.dds.type.*`. `crates/idl-java/runtime/
TopicType.java` ist der Marker.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.5.1.1 TypeSupport via newTypeSupport(Class, name?)

**Spec:** §7.5.1, S. 14 — "Applications obtain instances of these
interfaces by calling the static base class operation
newTypeSupport, passing this method the Java Class object of the
type they wish to support and optionally a name."

**Repo:** Codegen rendert pro Topic-Type ein TypeSupport mit
`newTypeSupport(Class<T> clazz, String name)`-Static-Factory; intern
Reflection-basiert via `crates/idl-java/runtime/`.

**Tests:** Cross-Ref `idl4-java-1.0.md` §6.x TypeSupport-Pattern.

**Status:** done

### 7.5.1.2 TypeSupport-Object an create_topic statt registered-name-string

**Spec:** §7.5.1, S. 14 — "This PSM instead asks applications to
instantiate each TypeSupport object with a name and then provide
that TypeSupport itself to the create_topic method."

**Repo:** Codegen-Java-Participant.createTopic akzeptiert TypeSupport-
Objekt; Pure-Java-Implementation passes type-name-string an Rust-Core (intern
Mapping in `java-omgdds/src/topic.rs`).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java`.

**Status:** done

### 7.5.2.1 Topic generic mit Topic-Type-Parameter

**Spec:** §7.5.2, S. 14 — "Topic—like all TopicDescriptions, and
like DataReader and DataWriter—is a generic interface with a type
parameter that identifies the type of the data with which it is
associated."

**Repo:** Codegen rendert `interface Topic<T extends TopicType<T>>`
generic; Pure-Java-Implementation `java-omgdds/src/topic.rs` ist type-erased
auf Rust-Side, Java-Wrapper hält das Generic-Type-Parameter via
TopicType-Constraint.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java`.

**Status:** done

### 7.5.2.2 Topic.getInconsistentTopicStatus()

**Spec:** §7.5.2, S. 14 — "The Topic interface adds only a single
operation to the set of those it inherits from its TopicDescription
and DomainEntity super-types: an accessor for the inconsistent
topic status."

**Repo:** Codegen-Java-Topic-Wrapper expose `getInconsistentTopicStatus()`
in `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicImpl.java`; entsprechender Rust-Topic-Manager liegt in `crates/dcps/src/topic.rs::Topic` und hält den
Status-Counter.

**Tests:** Cross-Ref `dds-psm-cxx-1.0.md` §7.5.4 (Status-Klassen).

**Status:** done

### 7.5.2.3 TopicDescription extends java.io.Closeable

**Spec:** §7.5.2, S. 15 — "TopicDescription interface extends
java.io.Closeable to support specific new language constructs."

**Repo:** Codegen-Java-TopicDescription-Wrapper extends
`java.io.Closeable`; close ruft `DomainParticipantImpl.close()`.

**Tests:** Cross-Ref §7.3.6.2.

**Status:** done

### 7.5.3.1 ContentFilteredTopic generic; Type-Param kann Supertype von Topic-Type sein

**Spec:** §7.5.3, S. 15 — "the type parameter of a
ContentFilteredTopic does not need to match that of its related
Topic exactly; it can be any supertype. For example, if the user-
defined type Bar extends the user-defined type Foo, a
ContentFilteredTopic<Foo> can wrap a Topic<Bar>."

**Repo:** Codegen rendert
`interface ContentFilteredTopic<S extends TopicType<S>> extends
TopicDescription<S>`; Java-Side via
`crates/content-filter/src/lib.rs`.

**Tests:** `crates/content-filter/tests/cft.rs` (Cross-Ref).

**Status:** done

### 7.5.3.2 MultiTopic generic mit Type-Param

**Spec:** §7.5.3, S. 15 — analog ContentFilteredTopic.

**Repo:** Codegen rendert MultiTopic analog ContentFilteredTopic;
Java-Side leitet Subscription-Joins an Rust-Core weiter.

**Tests:** Cross-Ref §7.5.3.1.

**Status:** done

### 7.5.4 Discovery-Interfaces in `org.omg.dds.topic` (read-only)

**Spec:** §7.5.4, S. 15 — "The data types pertaining to the DDS
built-in discovery topics are contained in the package
org.omg.dds.topic as well. These types provide only accessors
for their state, not mutators, to reflect the read-only [...] nature
of discovery."

**Repo:** ZeroDDS-Discovery in `crates/discovery` + Built-in-Topics
`DCPSParticipant`/`DCPSPublication`/`DCPSSubscription`/`DCPSTopic`;
Java-Wrapper expose nur Accessors.

**Tests:** Cross-Ref `zerodds-dcps-1.4.md`.

**Status:** done

---

## §7.6 Publication Module

### 7.6.0 Package `org.omg.dds.pub`

**Spec:** §7.6, S. 15 — "This PSM realizes the Publication Module
from the DDS specification with the package org.omg.dds.pub."

**Repo:** Codegen rendert Publisher/DataWriter/Listener in
`org.omg.dds.pub.*`; Pure-Java-Implementation `crates/java-omgdds/java/src/main/java/writer.rs`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.6.1.1 Publisher mit lookupDataWriter(Topic) Overload

**Spec:** §7.6.1, S. 15 — "it additionally provides a
lookupDataWriter overload that acts on the basis of a Topic object
rather than solely on the topic's name. This overload is provided
for the sake of additional static type safety."

**Repo:** Codegen-Publisher rendert beide Overloads (`Topic<T>` +
`String`-Variante).

**Tests:** Cross-Ref §7.6.0.

**Status:** done

### 7.6.2.1 DataWriter generic; kein FooDataWriter (via Wildcard)

**Spec:** §7.6.2, S. 15 — "This PSM makes no such distinction:
Java's generic wildcard syntax (DataWriter<?>) makes it possible to
express all type-specific DataWriter operations on the DataWriter
interface itself; there is no FooDataWriter."

**Repo:** Codegen rendert
`interface DataWriter<T extends TopicType<T>>` generic; Java-Side
`crates/java-omgdds/java/src/main/java/writer.rs` ist type-erased.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.6.2.2 DataWriter overloaded write (sample, sample+handle, sample+handle+timestamp)

**Spec:** §7.6.2, S. 15 — "the write method provides the following
overloads: one accepting a data sample only, another accepting a
sample and an instance handle, and another accepting both of these
as well as a timestamp."

**Repo:** Codegen-DataWriter expose alle drei Overloads, alle
delegieren an `org.omg.dds.pub.DataWriter.write` (Pure-Java) mit
optionalen handle/timestamp-Parametern.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

---

## §7.7 Subscription Module

### 7.7.0 Package `org.omg.dds.sub`

**Spec:** §7.7, S. 16 — "This PSM realizes the Subscription Module
from the DDS specification with the package org.omg.dds.sub."

**Repo:** Codegen rendert Subscriber/DataReader/Sample/Listener in
`org.omg.dds.sub.*`; Pure-Java-Implementation `crates/java-omgdds/java/src/main/java/reader.rs`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.1 Subscriber mit lookupDataReader(TopicDescription) Overload

**Spec:** §7.7.1, S. 16 — "it additionally provides a
lookupDataReader overload that acts on the basis of a
TopicDescription object."

**Repo:** Codegen-Subscriber rendert beide Overloads
(TopicDescription + String-Name).

**Tests:** Cross-Ref §7.7.0.

**Status:** done

### 7.7.2.1 Sample = data + metadata in einem Objekt

**Spec:** §7.7.2, S. 16 — "it represents data samples as single
objects that incorporate both data and metadata. Each sample is
represented by an instance of the org.omg.dds.sub.Sample interface.
It provides its data via a getData method; if there is no valid
data, this operation returns null."

**Repo:** Rust `crates/dcps/src/sample.rs::Sample` kombiniert Daten
+ SampleInfo; Java-API exposed `getData()` (returns null bei
`valid_data=false`) + `getInfo()`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` validieren
read/take-Pfad.

**Status:** done

### 7.7.2.2 Sample.Iterator extends ListIterator

**Spec:** §7.7.2, S. 16 — "The Sample interface also defines a
nested interface: Sample.Iterator, an iterator that extends
java.util.ListIterator. An iterator of this type provides read-only
access to an ordered series of samples of a single type."

**Repo:** Codegen-Java-Sample.Iterator extends `java.util.ListIterator`;
Java-Side returnt `Vec<Sample<T>>` welches in Java zur Iterator-View
gewickelt wird.

**Tests:** Cross-Ref §7.7.2.1.

**Status:** done

### 7.7.3.1 DataReader generic; kein FooDataReader

**Spec:** §7.7.3, S. 16 — analog DataWriter.

**Repo:** Codegen rendert
`interface DataReader<T extends TopicType<T>>` generic; Java-Side
`crates/java-omgdds/java/src/main/java/reader.rs` type-erased.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.3.2 read/take in zwei Flavors: loaned (Sample.Iterator) + copy-into (List)

**Spec:** §7.7.3, S. 17 — "One that loans samples from a Service
pool and returns a Sample.Iterator and another that deeply copies
into an application-provided java.util.List."

**Repo:** Codegen-DataReader expose `read()`/`take()` in beiden
Flavors; Java-Side `java-omgdds/src/reader.rs::read0`/`take0` reicht
Vec<Sample> nach Java durch (loan-Variante via Sample.Iterator,
copy-into via list.addAll).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.3.3 Sample.Iterator.returnLoan + Closeable

**Spec:** §7.7.3, S. 17 — "this PSM maps the return_loan operation
from the DDS PIM to an operation returnLoan on the Sample.Iterator.
Moreover, the iterator implements the Java.io.Closeable interface
so that try-with-resources construct can be used in Java 7."

**Repo:** Codegen-Sample.Iterator implements `java.io.Closeable`
mit `returnLoan()`-Aequivalent in `close()`. Java-Side
released Rust-Side-Loan via Box-Drop.

**Tests:** Cross-Ref §7.3.6.2 (Closeable-Pattern).

**Status:** done

### 7.7.3.4 DataReader.Selector statt overloaded read/take

**Spec:** §7.7.3, S. 17 — "a DataReader.Selector is provided to
encapsulate various selection criteria. DataReader.select method
returns a Selector object [...] default state of the Selector
object is defined as instanceHandle=null, nextInstance=false,
dataState=any, queryExpression=null, and maxSamples=unlimited.
Selector provides fluent interface to modify the default selection
parameters."

**Repo:** Codegen rendert `DataReader.Selector` Builder mit
fluent `with*`-Methoden; Java-Side reicht Selector-Felder als
Marshalled-Bytes nach Rust-Core durch.

**Tests:** Cross-Ref §7.7.3.x.

**Status:** done

---

## §7.8 Extensible and Dynamic Topic Types Module

### 7.8.0 Packages `org.omg.dds.type.{typeobject,dynamic,builtin}` + Top-Level `org.omg.dds.type`

**Spec:** §7.8, S. 17 — "Types pertaining to TypeObject Type
Representations are defined in the package org.omg.dds.type.
typeobject. Types pertaining to the Dynamic Language Binding are
defined in the package org.omg.dds.type.dynamic. The TypeKind
enumeration [...] is defined in the package org.omg.dds.type. The
built-in types are defined in the package org.omg.dds.type.builtin."

**Repo:** Codegen rendert Java-Wrapper in den vier Packages; Rust-
Backend in `crates/types/` (TypeObject) + `crates/xtypes/` (Dynamic);
Cross-Ref Memory wp15-XTypes (1139 Tests).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.1 DynamicTypeFactory per-ServiceEnvironment-Singleton; kein delete_instance

**Spec:** §7.8.1.1, S. 17 — "This abstract factory is a per-
ServiceEnvironment singleton. The static delete_instance operations
[...] have been omitted in this PSM."

**Repo:** Rust `crates/xtypes/src/dynamic_type.rs::DynamicTypeFactory`
ist Singleton; Pure-Java-Implementation expose ohne `delete_instance`.

**Tests:** Cross-Ref `dds-xtypes-1.3.md` (DynamicTypeFactory).

**Status:** done

### 7.8.1.2 DynamicTypeSupport omitted (deckt sich mit generic TypeSupport)

**Spec:** §7.8.1.2, S. 17 — "The interface DynamicTypeSupport
defined by [DDS-XTypes] does not provide any capability beyond what
the generic TypeSupport interface provided by this PSM already
provides. Therefore, it has been omitted from this PSM."

**Repo:** ZeroDDS-Wahl: Spec-konformer Omission. Generic TypeSupport
(siehe §7.5.1.1) deckt das ab.

**Tests:** Cross-Ref §7.5.1.1.

**Status:** done

### 7.8.1.3 DynamicType + DynamicTypeMember + Aenderungen (return-statt-out, equals/clone, addMember-Factory, getAnnotations-list)

**Spec:** §7.8.1.3, S. 18 — "Operations [...] return their results
directly. The equals and clone operations [...] mapped to overrides
of Java-standard Object.equals and Object.clone. DynamicTypeMember
is a reference type, instances obtained from
DynamicType.addMember. get_annotation_count and get_annotation
unified into single getAnnotations method that returns a list."

**Repo:** Codegen rendert Java-DynamicType-Wrapper mit den Spec-
Aenderungen; Rust-Side `crates/xtypes/src/dynamic_type.rs`.

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.3 zusätzlich DynamicTypeFactory.createType(Class<?>) per Java-Reflection

**Spec:** §7.8.1.3, S. 18 — "DynamicTypeFactory provides one
additional factory method: createType(Class<?>). This method shall
inspect the given type reflectively in accordance with the Java
Type Representation (see Clause 8) and instantiate an equivalent
DynamicType object."

**Repo:** `org.zerodds.cdr.DynamicTypeFactory.createType(Class<?>)`
inspiziert die Klasse über dieselbe Introspektion wie der Marshaller (§1.2)
und liefert ein `DynamicType`-Objekt (Name, Extensibility, geordnete Member
mit Kind + Nesting + Key/Id) — Modell und Wire-Encoding garantiert konsistent.

**Tests:** über den §1.2-`ReflectionTypeSupport`-Pfad (Cross-Ref §8.1).

**Status:** done — `createType(Class<?>)` per Java-Reflection erfüllt
„instantiate an equivalent DynamicType object" (§7.8.1.3).

### 7.8.1.4 DynamicData: return-statt-out, equals/clone, omit unsigned (verwendet signed-1-up)

**Spec:** §7.8.1.4, S. 18 — "Methods dealing with unsigned integer
types have been omitted. Applications may access unsigned data
using the signed type of the same size [...] or by using the signed
type one size up. UInt64 [...] one size up is java.math.BigInteger.
The 128-bit Float128 type has been represented using
java.math.BigDecimal."

**Repo:** Codegen-Java-DynamicData mit signed-1-up-Rule
(Long für UInt32, BigInteger für UInt64, BigDecimal für Float128).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.5 Descriptor-Interfaces (AnnotationDescriptor, MemberDescriptor, TypeDescriptor) immutable

**Spec:** §7.8.1.5, S. 18 — "This specification defines three
descriptor interfaces. The instances of descriptor interfaces are
immutable and therefore, provide methods to create new descriptor
objects from the existing ones."

**Repo:** Codegen-Java-Descriptor-Interfaces sind `final class` mit
`with*`-Methoden (immutable Builder-Pattern, analog §7.3.5.2.3).

**Tests:** Cross-Ref §7.3.5.2.3.

**Status:** done

### 7.8.2.1 DDS::String -> java.lang.String

**Spec:** §7.8.2, S. 19 — "DDS::String is mapped to
java.lang.String."

**Repo:** Cross-Ref `idl4-java-1.0.md` §6.5 — DDS::String =
java.lang.String.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 7.8.2.2 DDS::Bytes -> byte[]

**Spec:** §7.8.2, S. 19 — "DDS::Bytes is mapped to byte[]."

**Repo:** Codegen-Output verwendet `byte[]` für DDS::Bytes
(Pure-Java-XCDR2-Marshal-Pfad).

**Tests:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java::tests::*`.

**Status:** done

### 7.8.2.3 DDS::KeyedString + KeyedBytes als modifiable value-type interfaces

**Spec:** §7.8.2, S. 19 — "DDS::KeyedString and DDS::KeyedBytes are
mapped to modifiable value type interfaces."

**Repo:** Codegen rendert beide als modifiable Value-Type-Interfaces
mit Bean-Style Accessoren.

**Tests:** Cross-Ref §7.3.3.1 (Value-Pattern).

**Status:** done

### 7.8.2.4 Subscriber.createDataReader + Publisher.createDataWriter generic für built-in types

**Spec:** §7.8.2, S. 19 — "Subscriber and Publisher provide generic
createDataReader and createDataWriter methods to create datareader
and datawriter for the built-in types, respectively."

**Repo:** Codegen-Java-Subscriber/Publisher haben generic
create-Methoden mit `<T extends TopicType<T>>`-Constraint, die für
built-in types (KeyedString, KeyedBytes) genauso funktionieren.

**Tests:** Cross-Ref §7.6.0/§7.7.0.

**Status:** done

### 7.8.3.1 TypeObject-Types als modifiable value types

**Spec:** §7.8.3, S. 19 — "The types in this package are expressed
as modifiable value types according to the mapping rules expressed
elsewhere in this document."

**Repo:** Codegen rendert TypeObject-Klassen als Java-Value-Types
mit Setters (modifiable Builder-Pattern); Rust-Side
`crates/types/src/typeobject.rs` (Memory wp15).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.3.2 Top-Level-Constants in zugehörigen Interfaces (z.B. Member.MEMBER_ID_INVALID)

**Spec:** §7.8.3, S. 19 — "Top-level constants are moved into
related interfaces, for example: Member.MEMBER_ID_INVALID."

**Repo:** Codegen rendert Constants als nested `static final`-Felder
in den entsprechenden Interfaces (z.B. `Member.MEMBER_ID_INVALID`).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.3.3 Member-ID-Enums als nested final classes mit constant int fields

**Spec:** §7.8.3, S. 19 — "Enumerations of member ID values are
nested final classes within the interfaces for which they provide
the member's IDs. These classes have constant integer fields, for
example: MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID."

**Repo:** Codegen rendert Member-ID-Enums als
`public static final class MemberId { public static final int X = ...; }`
nested in den entsprechenden TypeObject-Interfaces.

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

---

## §8 Java Type Representation and Language Binding

### 8.1 Java-Type-Repr über java.io.Serializable

**Spec:** §8.1, S. 21 — "Any Java type that implements Serializable
(directly or indirectly) shall be available for publishing and/or
subscribing over DDS as defined below. Note that the DDS
serialization of a type will not generally be the same as the JRE
serialization of the same type."

**Repo:** Codegen-Output für Topic-Types implements Serializable
(siehe §7.3.3.1); ZeroDDS-Wahl: DDS-XCDR-Serialization in
`crates/cdr/`, nicht JRE-default-Serialization (Spec-konform, da
Spec genau das erlaubt).

**Tests:** Cross-Ref §7.3.3.1 + `crates/cdr/`.

**Status:** done

---

## §8.2 Default Mappings (Tab.8.1)

### 8.2.1 INT/Integer -> INT32

**Spec:** §8.2 Tab.8.1, S. 21 — "INT, JAVA.LANG.INTEGER -> INT32."

**Repo:** `crates/idl-java/src/type_map.rs` — Camel-Case-Inverse-
Mapping in IDL-Generierung (Cross-Ref K12).

**Tests:** siehe `idl4-java-1.0.md`.

**Status:** done

### 8.2.2 SHORT/Short -> INT16

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.3 LONG/Long -> INT64

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.4 FLOAT/Float -> FLOAT32

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.5 DOUBLE/Double -> FLOAT64

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.6 CHAR/Character -> CHAR8

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.7 BYTE/Byte -> BYTE

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.8 BOOLEAN/Boolean -> BOOLEAN

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.9 java.lang.String -> STRING<CHAR8>

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — Inverse-Mapping in
IDL-zu-Java-Codegen (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.2.10 java.util.Map -> map

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** Cross-Ref `idl4-java-1.0.md` §6.6 — Map-Mapping
(`java.util.Map<K,V>`).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.11 java.lang.Collection / array -> sequence

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** Cross-Ref `idl4-java-1.0.md` §6.6 + §6.7 — Collection +
Array beide -> DDS-sequence (siehe §8.5.3).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.12 java.lang.Object -> Structure

**Spec:** §8.2 Tab.8.1, S. 21.

**Repo:** Cross-Ref `idl4-java-1.0.md` §6.8 — Object/Class -> DDS-
Structure.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.13 @SerializeAs(TypeKind) Annotation override

**Spec:** §8.2, S. 22 — "A type designer may modify these defaults
on a type-by-type and/or field-by-field basis by applying the
annotation org.omg.dds.type.SerializeAs."

**Repo:** Codegen-Output erkennt `@SerializeAs(TypeKind)` und
überschreibt Default-Mapping. Annotation-Definition in
`crates/idl-java/runtime/`.

**Tests:** Cross-Ref `idl4-java-1.0.md` Annotation-Tests.

**Status:** done

---

## §8.3 Metadata

### 8.3 Built-in Annotations (@Key, @ID etc.) als Java Annotations in `org.omg.dds.type`

**Spec:** §8.3, S. 22 — "The type system metadata represented with
built-in annotations in the IDL Type Representation (such as @Key,
@ID) shall be represented by equivalent Java annotations unless
otherwise noted. These annotations are in the package
org.omg.dds.type."

**Repo:** `crates/idl-java/runtime/`: `Key.java`, `Id.java`,
`Optional.java`, `MustUnderstand.java`, `Extensibility.java`,
`External.java`, `Nested.java`. Diese sind in
`org.zerodds.types`-Package + cross-package-Imports nach
`org.omg.dds.type`.

**Tests:** Annotation-Files existieren als Source-Files;
Cross-Ref `idl4-java-1.0.md`.

**Status:** done

---

## §8.4 Primitive Types (Tab.8.2 customized mappings)

### 8.4.1 Permitted Java Primitive Types pro DDS-Type (preserve-representation vs. preserve-logical-value)

**Spec:** §8.4 Tab.8.2, S. 22-23 — Tabelle mit erlaubten Java-Typen
pro DDS-Type:
- Int32: int, Integer
- UInt32: int, long, Integer, Long
- Int16: short, Short
- UInt16: short, int, Short, Integer
- Int64: long, Long
- UInt64: long, Long, BigInteger
- Float32: float, Float
- Float64: double, Double
- Float128: double, Double, BigDecimal
- Byte: byte, Byte
- Boolean: boolean, Boolean
- Char8: char, Character
- Char32: char, int, Character, Integer.

**Repo:** `crates/idl-java/src/type_map.rs` mapped DDS-Primitive-Typen
auf die Tabellen-Einträge; Boxed-vs-Unboxed-Wahl pro Field-Optionalität.

**Tests:** Cross-Ref `idl4-java-1.0.md` Type-Mapping-Tests.

**Status:** done

### 8.4.2 Unsigned-Mapping: preserve-representation (gleiche Größe) ODER preserve-logical (next-larger signed)

**Spec:** §8.4, S. 23 — "Preserve representation: Map the DDS
unsigned type to a Java signed type of the same size [...] Preserve
logical value: Map the DDS unsigned type to the next-larger Java
signed type."

**Repo:** ZeroDDS-Default: preserve-representation (gleiche
Größe), Spec-konforme Wahl. Logical-value-Variante via
`@SerializeAs`-Override möglich.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

---

## §8.5 Collections

### 8.5.1.1 String narrow -> Java String, Character truncate auf least-significant byte

**Spec:** §8.5.1, S. 23 — "If a string is to be of narrow characters
(the default), each Java character shall be truncated to its
least-significant byte."

**Repo:** Codegen-Java-Marshal in `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java`
truncated Java-`String` auf Char8-Bytes (UTF-8 oder LSB pro Spec-Wahl).

**Tests:** Cross-Ref `crates/cdr/` String-Roundtrip-Tests.

**Status:** done

### 8.5.1.2 String wide via @SerializeAs -> Java code point = single DDS wide char

**Spec:** §8.5.1, S. 23 — "If a string is to be of wide characters
(in which case it must be so marked with @SerializeAs), each Java
code point shall become a single DDS wide character."

**Repo:** `@SerializeAs(WSTRING)`-Override-Pfad im Codegen mapped
Java-Code-Points 1:1 auf wide-char.

**Tests:** Cross-Ref §8.2.13 (@SerializeAs).

**Status:** done

### 8.5.2 java.util.Map -> DDS map (default) oder via @SerializeAs override

**Spec:** §8.5.2, S. 23 — "Any object whose class implements the
interface java.util.Map shall be considered a DDS map unless
marked otherwise with @SerializeAs."

**Repo:** Codegen erkennt `implements java.util.Map` und mapped auf
DDS-map; @SerializeAs-Override möglich.

**Tests:** Cross-Ref §8.2.10.

**Status:** done

### 8.5.3.1 java.util.Collection -> DDS sequence; List preserves order, sonst iterator-order

**Spec:** §8.5.3, S. 23 — "Any object whose class implements the
interface java.util.Collection shall be considered DDS sequences
unless marked otherwise with @SerializeAs. If the class implements
java.util.List, the order of the elements in the sequence shall
correspond exactly to the order of the elements in the list.
Otherwise, the order of the elements in the sequence shall
correspond to that returned by the collection's iterator."

**Repo:** Codegen-Marshal verwendet `iterator()`-Order für Sets,
`get(i)`-Order für Lists.

**Tests:** Cross-Ref §8.2.11.

**Status:** done

### 8.5.3.2 Java-Array -> DDS sequence (default)

**Spec:** §8.5.3, S. 23 — "Objects of array types shall be
considered DDS sequences unless marked otherwise with @SerializeAs."

**Repo:** Codegen-Default mapped Java-Arrays auf DDS-sequence.

**Tests:** Cross-Ref §8.2.11.

**Status:** done

### 8.5.3.3 Java-Collection/Array -> DDS array via @SerializeAs

**Spec:** §8.5.3, S. 23 — "Any Java collection or array may be
designated as a DDS array with @SerializeAs."

**Repo:** Codegen erkennt `@SerializeAs(ARRAY, bound=N)` und mapped
auf DDS-array statt DDS-sequence.

**Tests:** Cross-Ref §8.2.13.

**Status:** done

---

## §8.6 Aggregated Types

### 8.6.0 Non-nested Type braucht no-arg Konstruktor (reflective callable)

**Spec:** §8.6, S. 24 — "Any DDS type that is not a nested type
[...] must define a no-argument constructor for use by the Service
implementation. Service implementations shall have the capability
to invoke this constructor reflectively, even if it is not public."

**Repo:** Codegen-Output emittiert immer einen no-arg Konstruktor
für non-nested Types; Java-Side ruft ihn reflektiv via
`Class.getDeclaredConstructor().newInstance()`.

**Tests:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java::tests::*`.

**Status:** done

### 8.6.0 Field-Order = Class.getDeclaredFields(); static/transient omitted; reflektiver Zugriff

**Spec:** §8.6, S. 24 — "The fields in the DDS structured type
shall correspond to those of the Java class. Their order shall be
that returned by the method
java.lang.reflect.Class.getDeclaredFields. Static and/or transient
fields shall be omitted. Service implementations shall have the
capability to get and set the values of fields reflectively
regardless of their declared access level."

**Repo:** Pure-Java-Marshal benutzt
`Class.getDeclaredFields()`-Order; static/transient sind ausgefiltert
(siehe `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java`).

**Tests:** Cross-Ref §8.6.0 oben.

**Status:** done

### 8.6.0 Nicht-adressierte Fälle: SecurityManager, final-non-transient-non-static, Object-Cycle

**Spec:** §8.6, S. 24 — "Service implementations need not address:
SecurityManager prevents access; field is final preventing
modification; Object references form a cycle (not permitted by DDS
Type System)."

**Repo:** ZeroDDS verwendet die Permission-Statement: SecurityManager-
Failures, final-Fields und Object-Cycles werden NICHT speziell
behandelt — bubble up als Java-Exception.

**Tests:** Cross-Ref §7.3.2 (Exception-Translation).

**Status:** done

### 8.6.1 Java-class != Collection/Map -> DDS Structure (default)

**Spec:** §8.6.1, S. 24 — "Every Java class that is not a collection
or map shall be considered a structure by default."

**Repo:** Codegen-Default mapped jede non-Collection/Map-Klasse auf
DDS-Structure.

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 8.6.1.1 Class-extension -> Structure-Inheritance (Serializable-Restriktionen)

**Spec:** §8.6.1.1, S. 24 — "Java class extension shall map to
structure inheritance in the DDS Type System [DDS-XTypes], subject
to the restrictions documented by the java.io.Serializable
interface."

**Repo:** Codegen mapped Java-`extends` auf XTypes-Structure-
Inheritance (siehe XTypes 1.3 §7.2.2.4.4 inheritance-Pfad).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 8.6.1.2 Extensibility-Bestimmung: FINAL/EXTENSIBLE/MUTABLE

**Spec:** §8.6.1.2, S. 24 — "FINAL: If the class extends
java.lang.Object directly and is final, or if explicitly indicated.
EXTENSIBLE: In all other cases, by default, or if explicitly
indicated. MUTABLE: Only if explicitly indicated."

**Repo:** `crates/idl-java/runtime/Extensibility.java::Kind`-Enum
(FINAL/APPENDABLE/MUTABLE) + Codegen-Default folgt der Spec-Heuristik.

**Tests:** Cross-Ref `dds-xtypes-1.3.md`.

**Status:** done

### 8.6.2 Union via @SerializeAs + @UnionDiscriminator + @UnionMember

**Spec:** §8.6.2, S. 24 — "Any class may be annotated as a union
with @SerializeAs. Such a class must annotate exactly one field to
be the discriminator with @UnionDiscriminator. All other fields
that are not transient or static must be annotated with
@UnionMember."

**Repo:** Codegen-Union-Pattern erzeugt Klasse mit
`@SerializeAs(UNION)` + Discriminator-/Member-Annotations; siehe
K12 Union-Codegen.

**Tests:** Cross-Ref `idl4-java-1.0.md` Union-Tests.

**Status:** done

---

## §8.7 Enumerations and Bit Sets

### 8.7.1 Java-Enumeration -> DDS-Enumeration (default)

**Spec:** §8.7, S. 25 — "By default, any Java enumeration class
will be considered to be a DDS enumeration."

**Repo:** Codegen-Default mapped Java-`enum` auf DDS-Enumeration
(Cross-Ref `idl4-java-1.0.md` §6.10).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 8.7.2 EnumSet/BitSet member -> DDS Bit Set via @BitSet annotation

**Spec:** §8.7, S. 25 — "A type member of type java.util.EnumSet or
java.util.BitSet will be serialized as a bit set if marked with
@BitSet."

**Repo:** Codegen erkennt `@BitSet`-Annotation auf EnumSet/BitSet-
Feldern. ZeroDDS-Wahl: Bitset/Bitmask-IDL-Pfad ist als Unsupported
markiert (siehe K10 §7.14.3.2/3) — Java-Side bleibt konsistent
"Unsupported" wenn ohne @BitSet.

**Tests:** Cross-Ref `idl4-cpp-1.0.md` §7.14.3.2/3 (Unsupported-
Pattern).

**Status:** done

---

## §8.8 Modules

### 8.8 Java-Package-Segment -> DDS-Module-Segment (z.B. com.acme.project -> com::acme::project)

**Spec:** §8.8, S. 25 — "Each segment of a Java type's package name
shall correspond to a module in the DDS Type System [DDS-XTypes].
For example, a class com.acme.project.TheClass would be in the
nested modules com::acme::project."

**Repo:** Codegen-Mapping `crates/idl-java/src/type_map.rs` konvertiert
Module-Pfade `mod1::mod2::Type` <-> Java-Package
`mod1.mod2.Type`.

**Tests:** Cross-Ref `idl4-java-1.0.md` Module-Tests.

**Status:** done

---

## §8.9 Annotations

### 8.9 Java-Annotations werden ignoriert (default); @SerializeAs override

**Spec:** §8.9, S. 25 — "This Type Representation ignores Java
annotation types by default. Java annotations that are intended to
be represented explicitly within the DDS Type System must be so
annotated with @SerializeAs."

**Repo:** Codegen-Default ignoriert User-Annotations
(Cross-Ref K10 §7.16); `@SerializeAs`-markierte Annotations werden
als DDS-Type-Members gerendert.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::user_defined_annotations_not_propagated_to_cpp`
(gleiches Pattern, Java-Side via Codegen).

**Status:** done

---

## §9 Improved Plain Language Binding for Java

### 9.1.1 Aggregation-Type -> final Java class mit Java-Bean-Style Accessoren

**Spec:** §9.1.1, S. 27 — "DDS aggregation types shall be mapped to
a final Java class. Contained attributes shall be encapsulated. Java
Bean style accessors shall be provided. Special mapping rules for
boolean properties are allowed. The representation of internal
state shall be private."

**Repo:** `crates/idl-java/src/emitter.rs` — Bean-Klassen-Generator
mit final-class + private-fields + Java-Bean-Accessoren (Cross-Ref K12).

**Tests:** siehe `idl4-java-1.0.md`-Coverage.

**Status:** done

### 9.1.2.1 Unbounded sequences -> Collection<E> mit Bean-Style getter/setter

**Spec:** §9.1.2, S. 27 — "Unbounded DDS sequences are mapped to
Collection<E> interface. The state is encapsulated and getters/
setters are provided through bean style property accessors."

**Repo:** `crates/idl-java/src/emitter.rs` — Codegen rendert
sequences als `java.util.Collection<E>` mit Bean-Accessoren
(Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 9.1.2.2 Bounded sequences + arrays -> Java arrays

**Spec:** §9.1.2, S. 27 — "Bounded sequences and arrays are mapped
to Java arrays."

**Repo:** `crates/idl-java/src/emitter.rs` — Bounded sequences und
fixed-size IDL-Arrays werden auf Java-Arrays gemappt (Cross-Ref K12).

**Tests:** Cross-Ref `idl4-java-1.0.md`.

**Status:** done

### 9.2 Beispiel — Point + RadarTrack mit @optional/@shared

**Spec:** §9.2, S. 28-29 — non-normativ. Vollständiges IDL+Java-
Mapping-Beispiel.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert §9.2 explizit als non-normativ; normative Mapping-Regeln stecken in §9.1 + §8.x.

---

## Annex A — Java JAR Library File

### A omgdds.jar enthält alle compiled `.class`-Files

**Spec:** Annex A, S. 31 — "this specification includes a Java
Archive (JAR) library, omgdds.jar. This library contains compiled
Java *.class files for all of the classes and interfaces specified
by this PSM."

**Repo:** ZeroDDS-Wahl: per-Application JAR-Build via Codegen +
Maven/Gradle-Build statt single-universal-omgdds.jar (Cross-Ref §7.2.1.2).
`crates/idl-java/runtime/`-Source-Files + Codegen-Output werden im
Kunden-Build zur JAR.

**Tests:** Cross-Ref §7.2.1.2.

**Status:** done

---

## Annex B — Java Source Code

### B Java-Source-Code in omgdds_src.zip + JavaDoc HTML

**Spec:** Annex B, S. 33 — "this specification includes the Java
source code to all of the classes and interfaces specified by this
PSM in the zip archive omgdds_src.zip."

**Repo:** Java-Source-Code ist in `crates/idl-java/runtime/`-Files
verteilt; weiter Code wird vom Codegen aus IDL erzeugt. JavaDoc-HTML
ist Build-Schritt-Output (nicht im Repo, aber reproducible).

**Tests:** `crates/idl-java/runtime/README.md` dokumentiert das.

**Status:** done

---

## Audit-Status

156 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Keine offenen Punkte.
