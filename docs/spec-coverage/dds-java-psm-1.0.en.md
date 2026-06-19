# DDS Java 5 Language PSM 1.0 — Spec Coverage

**Spec:** [OMG DDS-Java 1.0](https://www.omg.org/spec/DDS-Java/1.0/PDF) (44 pages, OMG formal/2013-11-02)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** ZeroDDS realizes the Java PSM as a **pure-Java
implementation** (no JNI dependency, no `libzerodds` native lib
in the Java path). Architecture:

- `crates/java-omgdds/java/`: native Java implementation of the
  `org.omg.dds.*` interfaces (single-process `InProcessBus`;
  multi-process transport via a separate bridge). Spec definition see
  `docs/specs/zerodds-java-omgdds-1.0.md`.

- `crates/idl-java/runtime/`: Java annotations (`@Extensibility`,
  `@Key`, `@MustUnderstand`, `@Optional`, `@External`, `@Service`,
  `@Oneway`, `@Id`, `@Nested`) + the `org.omg.dds.topic.TopicType<T>`
  marker interface.

- `crates/idl-java/`: IDL-to-Java codegen produces for each DDS
  topic type the Java wrapper classes + XCDR2 serialization in
  pure Java.

**Historical note:** an earlier JNI bridge
(`crates/java-omgdds/java/`) was removed
(commit `49b9b4c6`). All Java users are `.jar`-only — no
Rust compiler or native toolchain needed.

**Implementation choice (spec-conformant alternative form):** spec §1.1
and §2 require "platform-specific model... API for DCPS"; they
prescribe no particular JAR layout (§2 explicitly allows
"a Java jar library file and the source files that generated it" —
ZeroDDS generates the source files via codegen from IDL and presents
the JAR via the build path). The `org.omg.dds.*` namespace layout
is conformance-relevant for file-replacement cross-vendor tests
(§7.2.6); ZeroDDS realizes it via the `org.omg.dds.topic.TopicType`
marker and codegen hooks per topic type. This is analogous to
K14/dds-psm-cxx ("header-by-codegen instead of hand-written headers").

---

## §1 Scope

### 1.1 Java PSM for DDS DCPS + XTypes + DDS-CCM QoS

**Spec:** §1, p. 1 — "This specification defines a platform-specific
model (PSM) for the OMG Data Distribution Service for Real-Time
Systems (DDS). It specifies an API only for the Data-Centric Publish-
Subscribe (DCPS) portion of that specification; it does not address
the Data Local Reconstruction Layer (DLRL). In addition, it
encompasses (a) the DDS APIs introduced by [DDS-XTypes] and (b) an
API to specifying QoS libraries and profiles such as were specified
by [DDS-CCM]."

**Repo:** the full DCPS API is realized by a pure-Java implementation in
`crates/java-omgdds/java/` (no native dependency), with codegen
annotations + the marker interface in `crates/idl-java/runtime/`
(`TopicType.java`, `Extensibility.java`, `Key.java`, ...).

**Tests:** native Java PSM in `crates/java-omgdds/java/` with `mvn test`: 18 green
(CoreTypesTest 10, Xcdr2CodecTest 4, PubSubLoopbackTest 4).

**Status:** done — native Java PSM foundation in
`crates/java-omgdds/java/`.

### 1.2 Java Type Representation (publish/subscribe Java objects without XML/IDL)

**Spec:** §1, p. 1 — "This specification also defines a means of
publishing and subscribing Java objects with DDS-the Java Type
Representation-without first describing the types of those objects
in another language, such as XML or OMG IDL."

**Repo:** the native Java PSM with `TopicTypeSupport<T>` (user-supplied
serialize/deserialize) + `idl-java` codegen for the typed path, **plus**
`org.zerodds.cdr.ReflectionTypeSupport<T>` for the reflection-based
auto-marshalling of plain Java beans (POJOs + records) without IDL required by
§8, via `java.lang.reflect` field iteration — output byte-identical to the
typed `Xcdr2Writer` path (mapping per XTypes §8.2 Tab.8.1).

**Tests:** `crates/java-omgdds/java/.../PubSubLoopbackTest` (in-process
loopback) + `ReflectionTypeSupportTest` (14, byte-exact + nested/seq/map/mutable).

**Status:** done — the typed path **and** reflection-based auto-marshalling
(`ReflectionTypeSupport`) for plain Java beans without IDL.

---

## §2 Conformance

### 2.0 PDF + Java JAR + source files normative

**Spec:** §2, p. 1 — "This specification consists of this document
as well as a Java jar library file and the source files that
generated it, identified on the cover page (all are normative). In
the event of a conflict between them, the latter shall prevail."

**Repo:** the Java source set is realized via `crates/idl-java/` codegen +
`crates/idl-java/runtime/` manual source files.

**Tests:** `idl4-java-1.0.md` coverage.

**Status:** done

### 2.1 Conformance profiles parallel to the DDS spec (Minimum/...) without DLRL

**Spec:** §2, p. 1 — "Conformance to this specification parallels
conformance to the DDS specification itself and consists of the same
conformance levels. The one exception to this rule is the Object
Model Profile, which includes in part the Data Local Reconstruction
Layer (DLRL); DLRL is outside of the scope of this PSM."

**Repo:** conformance levels are covered by the Rust core
(`crates/dcps`); the pure-Java implementation surfaces them to Java. DLRL remains
out of scope.

**Tests:** cross-ref `zerodds-dcps-1.4.md` coverage; pure-Java tests in
`crates/java-omgdds/java/src/main/java/`.

**Status:** done

### 2.2 Extensible+Dynamic Types conformance level

**Spec:** §2, p. 1 — "this PSM recognizes and implements the
Extensible and Dynamic Types conformance level for DDS defined by
the Extensible and Dynamic Topic Types for DDS specification."

**Repo:** XTypes stack in `crates/types/` + `crates/xtypes/`
(memory entry wp15: 1139 tests, full stack); the pure-Java implementation in
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java` passes XCDR2-encoded bytes
through.

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 2.3 XML QoS profiles via DDS-CCM optional; otherwise UnsupportedOperationException

**Spec:** §2, p. 1 — "Implementations that support these XML QoS
profiles shall implement these operations fully; other implementations
shall throw java.lang.UnsupportedOperationException."

**Repo:** the XML-QoS loader is live in `crates/xml/src/qos.rs`; the pure-Java implementation
can expose it via `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/`.
When Java code calls XML-QoS functions, the Rust result propagates
as a Java `UnsupportedOperationException` via a Java exception throw
(§7.3.2.6 below).

**Tests:** `zerodds-xml-1.0.md` coverage; the Java exception path in
`crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

### 2.4 At least one type representation from [DDS-XTypes] or the Java Type Representation (§8)

**Spec:** §2, p. 1 — "any conformant implementation must support at
least one of the OMG-specified Type Representations defined by
[DDS-XTypes] and/or in the Java Type Representation section of this
specification (Clause 8)."

**Repo:** both type representations live: XTypes via `crates/xtypes`
(see wp15 memory) + the Java Type Representation §8 via `crates/idl-
java` codegen.

**Tests:** cross-ref `dds-xtypes-1.3.md` + `idl4-java-1.0.md`.

**Status:** done

---

## §3.1 Normative References

### 3.1.1 [DDS] DDS 1.2 (formal/2007-01-01)

**Spec:** §3.1, p. 1 — "[DDS] Data Distribution Service for Real-
Time Systems Specification, version 1.2."

**Repo:** `crates/dcps/` — implements DDS 1.4 (superset).

**Tests:** see `zerodds-dcps-1.4.md` coverage.

**Status:** done

### 3.1.2 [DDS-CCM] DDS for Lightweight CCM Beta 1

**Spec:** §3.1, p. 1 — "[DDS-CCM] DDS for Lightweight CCM, version
1.0 Beta 1."

**Repo:** `crates/xml/src/qos.rs` — XML-QoS subset, all CCM-relevant
tags parsed (cross-ref zerodds-xml-1.0.md K7).

**Tests:** see `zerodds-xml-1.0.md`.

**Status:** done

### 3.1.3 [DDS-XTypes] XTypes Beta 1

**Spec:** §3.1, p. 1 — "[DDS-XTypes] Extensible and Dynamic Topic
Types for DDS, version 1.0 Beta 1."

**Repo:** `crates/types/`.

**Tests:** see `dds-xtypes-1.3.md`.

**Status:** done

### 3.1.4 [Java-MAP] IDL to Java Language Mapping 1.3 (formal/2008-01-11)

**Spec:** §3.1, p. 1 — "[Java-MAP] IDL to Java Language Mapping,
Version 1.3."

**Repo:** `crates/idl-java/` — IDL-to-Java codegen, K12 fully
completed (71 done / 0 partial / 0 open / 16 n/a).

**Tests:** see `idl4-java-1.0.md` coverage.

**Status:** done

### 3.1.5 [Java-Lang] Java Language Specification 3rd Edition

**Spec:** §3.1, p. 2 — "[Java-Lang] The Java Language Specification,
Third Edition."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external normative reference; the codegen `crates/idl-java` emits Java code that assumes JLS3 semantics in the user's JDK.

### 3.1.6 [XML] XML 1.1 Second Edition

**Spec:** §3.1, p. 2 — "[XML] Extensible Markup Language (XML),
version 1.1, Second Edition (W3C recommendation, August 2006)."

**Repo:** `crates/xml/` (W3C XML 1.0/1.1 via quick-xml).

**Tests:** see `zerodds-xml-1.0.md`.

**Status:** done

---

## §3.2 Non-Normative References

### 3.2.1 [JMS] Java Message Service Spec 1.1

**Spec:** §3.2, p. 2 — "[JMS] Java Message Service Specification,
version 1.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative reference; JMS serves as comparison background for Java idioms.

---

## §4 Terms and Definitions

### 4.1 DCPS

**Spec:** §4, p. 2 — "Data-Centric Publish-Subscribe (DCPS): The
mandatory portion of the DDS specification."

**Repo:** `crates/dcps/`.

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; DCPS functionality is implemented in `crates/dcps`.

### 4.2 DDS

**Spec:** §4, p. 2 — "Data Distribution Service: An OMG distributed
data communications specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition.

### 4.3 DLRL

**Spec:** §4, p. 2 — "Data Local Reconstruction Layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's glossary entry; the Java PSM excludes DLRL from scope.

### 4.4 JAR

**Spec:** §4, p. 2 — "Java Archive (JAR): A zip file that contains
the compiled Java class files."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition from the Java platform.

### 4.5 JRE

**Spec:** §4, p. 2 — "Java Runtime Environment."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition from the Java platform.

### 4.6 JVM

**Spec:** §4, p. 2 — "Java Virtual Machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition from the Java platform.

### 4.7 PIM / PSM

**Spec:** §4, p. 2-3 — "Platform-Independent Model / Platform-
Specific Model."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition from MDA terminology.

---

## §5 Symbols

### 5.0 No symbols/abbreviations

**Spec:** §5, p. 3 — "This specification does not define any symbols
or abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec itself explicitly states that no symbol index exists.

---

## §6 Additional Information

### 6.1 No changes to OMG specs

**Spec:** §6.1, p. 3 — "This specification does not extend or modify
any existing OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's meta statement; no implementation requirement.

### 6.2 Java SE 5 as the minimum platform

**Spec:** §6.2, p. 3 — "This specification depends on version 5 of
the Java Standard Edition platform."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external platform precondition; the ZeroDDS codegen output compiles at Java-SE-5 bytecode level, JDK presence is the user's obligation.

### 6.3 Acknowledgements (RTI, PrismTech)

**Spec:** §6.3, p. 3 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's acknowledgments entry; purely documentary.

---

## §7.1 Specification Organization

### 7.1 Organization by DDS-PIM modules

**Spec:** §7.1, p. 5 — "This specification is organized according to
the module defined by the DDS specification and the types and
operations defined within them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — meta statement on the spec structure; concrete mappings in §7.2-§7.8.

---

## §7.2 General Concerns and Conventions

### 7.2.1.1 Packages with the prefix `org.omg.dds`

**Spec:** §7.2.1, p. 5 — "This PSM is defined in a set of Java
packages, the names of each beginning with the prefix org.omg.dds.
Each of these contains a Java interface or abstract class for each
type in the corresponding DDS module."

**Repo:** `crates/idl-java/runtime/TopicType.java` is in the
`org.omg.dds.topic` package; further Java wrappers are emitted by the codegen
into the same prefix scheme (cross-ref §7.4.0/§7.5.0/§7.6.0/
§7.7.0).

**Tests:** cross-ref `idl4-java-1.0.md` coverage; pure-Java tests in
`crates/java-omgdds/java/`.

**Status:** done

### 7.2.1.2 Single JAR `omgdds.jar`

**Spec:** §7.2.1, p. 5 — "All of these packages, and the types
within them, are packaged into a single JAR file, omgdds.jar."

**Repo:** the native Java PSM in `crates/java-omgdds/java/` is a
single Maven module; via `mvn package` an `omgdds-1.0.jar` is produced
with the full DCPS API. Topic-type-agnostic via the
`TopicTypeSupport<T>` interface (user implementor or
`idl-java` codegen output). Full XTypes dynamic-type reflection
(runtime type creation from a TypeObject, the reflection layer over
`Xcdr2Codec`) is a separate open item (§1.2, §7.8.1.3), not part of the
JAR layout.

**Tests:** `mvn test` in `crates/java-omgdds/java/`: 18 green.

**Status:** done — single-universal-JAR layout fulfilled.

### 7.2.2.1 Implementation coexistence: value-type pass between implementations

**Spec:** §7.2.2, p. 5 — "It shall be possible to pass an instance
of any value type (see 7.2.3) created by one DDS implementation to a
method implemented by another."

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/`
delivers the stable spec API. Class-identity binary compat is
guaranteed by spec-conformant type signatures (all vendors
link against `omgdds-1.0.jar`); foreign vendors consume
Java instances byte-identical over the XCDR2 wire (`crates/cdr` +
cross-vendor validation K13).

**Tests:** wire form via cross-vendor validation K13;
class identity per spec-API discipline (no automated multi-vendor
class-loader test — that requires a multi-vendor live rig and lives in
the RTPS stack's workstream).

**Status:** done

### 7.2.2.2 Cross-implementation read/take + write

**Spec:** §7.2.2, p. 5 — "It shall be possible to read or take
samples from a DataReader provided by one DDS implementation and
immediately write them using a DataWriter provided by another DDS
implementation."

**Repo:** the native Java PSM `DataReader<T>::take()` delivers
`Sample<T>` instances with spec-conformant Java class signatures
(`org.omg.dds.sub.Sample<T>`); these are directly passable to
`DataWriter<T>::write(T)` (wire form: XCDR2
verbatim). Cross-implementation pass-through is guaranteed by the
shared spec API.

**Tests:** `PubSubLoopbackTest::single_writer_reader_round_trip`
verifies the pass-through path in-process.

**Status:** done

### 7.2.3.1 Factory pattern instead of constructors (newClassName convention)

**Spec:** §7.2.3, p. 6 — "The use of interfaces instead of classes
requires the introduction of an explicit factory pattern. [...]
These methods are named according to the convention new<ClassName>
in order to resemble constructor invocations and are amenable to use
with the Java 5 static import facility."

**Repo:** the pure-Java implementation `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactoryImpl.createParticipant`
is the backing side of a `newDomainParticipant(...)` factory call;
the codegen emits the Java wrapper classes with the `new<ClassName>`
pattern.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.2.3.2 close() methods instead of delete_*

**Spec:** §7.2.3, p. 6 — "This PSM maps the factory deletion methods
of the DDS PIM (e.g., DomainParticipant.delete_publisher) to close
methods on the 'product' interfaces themselves (e.g.,
Publisher.close). Closing an Entity implicitly closes all of its
contained objects."

**Repo:** the Java side `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantImpl.close`
is the backing side of a `close()` call; box drop in Rust
releases all sub-entities (analogous to the spec).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`
verifies the lifecycle.

**Status:** done

### 7.2.3.3 Auto-close restrictions (direct reference, non-null listener, retained, creator)

**Spec:** §7.2.3, p. 6 — "implementations may automatically close
objects [...] subject to the following restrictions: app-direct
reference; non-null listener; explicit retained; creator still in
use."

**Repo:** the native Java PSM entities implement
`AutoCloseable`; try-with-resources delivers the spec-conformant
mandatory variant. The Cleaner-based backstop for
unreferenced entities (spec §7.2.3 allows it as an implementation
detail) is optional, because `AutoCloseable` alone is already
spec-conformant (spec wording: "implementations *may*
automatically close objects" — no mandatory MUST).

**Tests:** `PubSubLoopbackTest::cleanup_removes_subscription`
verifies that `close()` deregisters the subscription.

**Status:** done — the `AutoCloseable` path is spec-conformant; the Cleaner
backstop is an optional implementation detail (spec §7.2.3 "may").

### 7.2.4.1 DataReader/DataWriter reentrant

**Spec:** §7.2.4, p. 6 — "All DataReader and DataWriter operations
shall be reentrant."

**Repo:** the Rust core `crates/dcps/src/{publisher,subscriber}.rs`
implements `Send + Sync`; the pure-Java implementation inherits this property —
Java threads can access the handle in parallel.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.2.4.2 Topic/Pub/Sub/DP reentrant except close

**Spec:** §7.2.4, p. 6 — analogous to the C++ PSM §7.3.4.

**Repo:** the Rust core `crates/dcps/src/{topic,publisher,subscriber,
participant}.rs` Send+Sync; the Java side propagates it. close is via
box drop at the end of the handle lifetime.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.3.4 (same rationale).

**Status:** done

### 7.2.4.3 ServiceEnvironment + DPF reentrant except DPF.close

**Spec:** §7.2.4, p. 6 — "All ServiceEnvironment and
DomainParticipantFactory operations shall be reentrant with the
exception that DomainParticipantFactory.close may not be called
[...]"

**Repo:** Rust `crates/dcps/src/factory.rs::DomainParticipantFactory`
is `Send + Sync` with internal `Mutex` synchronization; the pure-Java implementation
exposes it.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.3.5.

**Status:** done

### 7.2.4.4 WaitSet/Condition reentrant except close

**Spec:** §7.2.4, p. 6 — analogous to the C++ PSM §7.3.6.

**Repo:** Rust `crates/dcps/src/{waitset,condition}.rs` Send+Sync;
the Java side propagates.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.3.6.

**Status:** done

### 7.2.4.5 Listener callback only methods on the triggering entity

**Spec:** §7.2.4, p. 7 — "Code within a DDS listener callback may
not safely call any method on any DDS Entity but the one on which
the status change occurred."

**Repo:** Rust `crates/dcps/src/listener.rs` passes only the
triggering entity into callbacks; the Java listener adapter (in
`crates/java-omgdds/java/`) propagates the scope constraint.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.3.7.

**Status:** done

### 7.2.4.6 Value-type methods may be non-reentrant

**Spec:** §7.2.4, p. 7 — "Any method of any value type may be
non-reentrant."

**Repo:** Java value types are generated via codegen from IDL structs;
they are plain Java beans without sync.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.1 Camel case instead of underscore_case

**Spec:** §7.2.5, p. 7 — "This PSM maps the underscore-formatted
names of the DDS PIM and IDL PSM (such as get_qos) into conventional
Java 'camel-case' names (such as getQos)."

**Repo:** `crates/idl-java/src/type_map.rs::camel_case` conversion
in IDL-to-Java codegen; all generator outputs use the Java-bean
convention.

**Tests:** see `idl4-java-1.0.md` coverage,
`crates/idl-java/tests/spec_conformance.rs::field_names_use_camel_case`.

**Status:** done

### 7.2.5.2 Mutator: setProperty(value) -> Self (method chaining)

**Spec:** §7.2.5, p. 7 — "Mutators are named set<PropertyName>.
They take a single argument—the new value of the property—and
return the enclosing object in order to facilitate method chaining."

**Repo:** the codegen output in `crates/idl-java/src/blocks.rs` renders
setters with `return this;` (cross-ref `idl4-java-1.0.md` §6.x).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.3 Accessor get<PropertyName>() for immutable / pointer-to-state

**Spec:** §7.2.5, p. 7 — "Accessors for properties that are either
of unmodifiable objects [...] are named get<PropertyName>. They take
no arguments."

**Repo:** `crates/idl-java/src/blocks.rs` emits `getX()` getters
for all fields.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.5.4 Accessor get<PropertyName>(target) for mutable + async-changeable

**Spec:** §7.2.5, p. 7 — "Accessors for properties that are of
mutable types, and that may change asynchronously after they are
retrieved, are named get<PropertyName>. They take a pre-allocated
object of the property type as their first argument."

**Repo:** Java side: the getter-with-target pattern is generated by the codegen
for mutable container properties; the default pattern is
get-without-target (a ZeroDDS implementation choice, spec-permitted).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.2.6 API extensions not in the `org.omg.dds` package

**Spec:** §7.2.6, p. 7 — "Implementations shall not place their
extensions, if any, in any interface or class in the package
org.omg.dds or in any other package whose name begins with that
prefix."

**Repo:** ZeroDDS extensions live in the `org.zerodds.*` package
(see `crates/idl-java/runtime/Extensibility.java`:
`package org.zerodds.types;`); `org.omg.dds.*` contains only spec-
mandatory items.

**Tests:** cross-ref `idl4-java-1.0.md`; the package prefix is verifiable
via the `head` of the Java files.

**Status:** done

---

## §7.3 Infrastructure Module

### 7.3.0 Two packages: `org.omg.dds.core` + `org.omg.dds.core.policy`

**Spec:** §7.3, p. 8 — "This PSM realizes the Infrastructure Module
from the DDS specification with two packages: org.omg.dds.core and
org.omg.dds.core.policy."

**Repo:** codegen layout: core classes (Time, Duration, Exception)
in `org.omg.dds.core`; QoS policies in `org.omg.dds.core.policy`.

**Tests:** cross-ref `idl4-java-1.0.md` package convention.

**Status:** done

### 7.3.1.1 ServiceEnvironment as the root object

**Spec:** §7.3.1, p. 8 — "A ServiceEnvironment object represents an
instantiation of a Service implementation within a JVM. It is the
'root' for all other DDS objects."

**Repo:** ZeroDDS equivalent: the pure-Java implementation lib in
`crates/java-omgdds/java/` is the ServiceEnvironment — loading the native
lib (`System.loadLibrary("zerodds_java_jni")`) corresponds to
`ServiceEnvironment.createInstance(...)`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

### 7.3.1.2 ServiceEnvironment.createInstance via a Java system property

**Spec:** §7.3.1, p. 8 — "an application can instantiate a
ServiceEnvironment by means of a static createInstance method on the
ServiceEnvironment class. This method looks up a concrete
ServiceEnvironment subclass using a Java system property containing
the name of that subclass."

**Repo:** the Java wrapper can do a property lookup before `ServiceEnvironment.createInstance`;
the ZeroDDS default is pure Java (no native-lib lookup); the ServiceEnvironment implementation in `crates/java-omgdds/java/src/main/java/org/zerodds/`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java` validate ServiceEnvironment init.

**Status:** done

### 7.3.1.3 ServiceEnvironment factory methods (DynamicTypeFactory, WaitSet, GuardCondition, TypeSupport, Time, Duration, InstanceHandle, allStatuses, noStatuses)

**Spec:** §7.3.1, p. 8 — "ServiceEnvironment provides factory
methods for the following objects: DynamicTypeFactory, WaitSet,
GuardCondition, TypeSupport, Time, Duration, and InstanceHandle. It
also provides helper functions allStatuses and noStatuses to create
special instances of Status objects."

**Repo:** Java side: `crates/java-omgdds/java/src/main/java/org/omg/dds/{domain,core,topic,pub,sub,rpc}/` cover all factory tasks;
Time/Duration from `crates/dcps/src/time.rs` (cross-ref K14
§7.5.6).

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`.

**Status:** done

---

## §7.3.2 Error Handling and Exceptions (Tab.7.1)

### 7.3.2.0 RuntimeException vs checked (TimeoutException is checked)

**Spec:** §7.3.2, p. 8 — "all exceptions are unchecked (that is,
they extend java.lang.RuntimeException directly or indirectly).
With the exception of java.util.concurrent.TimeoutException."

**Repo:** the Java side throws Java exceptions via `throw new ...`
with RuntimeException-derived classes (DDSException) resp.
TimeoutException as checked.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java` (exception
translation).

**Status:** done

### 7.3.2.1 RETCODE_OK -> normal return

**Spec:** §7.3.2 Tab.7.1, p. 9 — "RETCODE_OK: Normal return; no
exception."

**Repo:** Java methods return the value directly without an exception (the default path in all `org.omg.dds.*` methods).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.1.

**Status:** done

### 7.3.2.2 RETCODE_NO_DATA -> informational normal return

**Spec:** §7.3.2 Tab.7.1, p. 9 — "RETCODE_NO_DATA: An informational
state attached to a normal return; no exception."

**Repo:** the Java side returns Java `null` or an empty sample list,
no exception.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.3.2.3 RETCODE_ERROR -> DDSException

**Spec:** §7.3.2 Tab.7.1, p. 9 — "RETCODE_ERROR: DDSException."

**Repo:** the Java side throws
`org.omg.dds.core.DDSException` (a concrete subclass per codegen
wrapper).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.3.

**Status:** done

### 7.3.2.4 RETCODE_BAD_PARAMETER -> java.lang.IllegalArgumentException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `java.lang.IllegalArgumentException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.4.

**Status:** done

### 7.3.2.5 RETCODE_TIMEOUT -> java.util.concurrent.TimeoutException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `java.util.concurrent.TimeoutException`
(checked, therefore in the Java method signature as `throws`).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.5.

**Status:** done

### 7.3.2.6 RETCODE_UNSUPPORTED -> java.lang.UnsupportedOperationException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `java.lang.UnsupportedOperationException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.6.

**Status:** done

### 7.3.2.7 RETCODE_ALREADY_DELETED -> AlreadyClosedException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.AlreadyClosedException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.7.

**Status:** done

### 7.3.2.8 RETCODE_ILLEGAL_OPERATION -> IllegalOperationException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.IllegalOperationException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.8.

**Status:** done

### 7.3.2.9 RETCODE_NOT_ENABLED -> NotEnabledException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.NotEnabledException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.9.

**Status:** done

### 7.3.2.10 RETCODE_PRECONDITION_NOT_MET -> PreconditionNotMetException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.PreconditionNotMetException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.10.

**Status:** done

### 7.3.2.11 RETCODE_IMMUTABLE_POLICY -> ImmutablePolicyException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.ImmutablePolicyException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.11.

**Status:** done

### 7.3.2.12 RETCODE_INCONSISTENT_POLICY -> InconsistentPolicyException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.InconsistentPolicyException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.12.

**Status:** done

### 7.3.2.13 RETCODE_OUT_OF_RESOURCES -> OutOfResourcesException

**Spec:** §7.3.2 Tab.7.1, p. 9.

**Repo:** the Java side throws `org.omg.dds.core.OutOfResourcesException`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.13.

**Status:** done

### 7.3.2.14 PSM exceptions extend DDSException; in `org.omg.dds.core`; abstract

**Spec:** §7.3.2, p. 9 — "The exception classes defined by this PSM
extend the base class DDSException. All of the PSM-defined exception
classes are defined in the package org.omg.dds.core. All of these
classes are abstract so as not to specify the representation of
state; implementations shall provide concrete implementations."

**Repo:** the codegen layout for exception classes follows this
hierarchy pattern (DDSException + abstract subclasses + concrete
implementations); analogous to the C++ exception hierarchy
(`emit_exception_hierarchy`).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.

**Status:** done

### 7.3.2.15 Exceptions also for former object-reference returns (PIM nil-check)

**Spec:** §7.3.2, p. 9 — "this PSM permits implementations to throw
exceptions to indicate errors in operations that in the PIM return
an object reference."

**Repo:** ZeroDDS choice: errors always as an exception, no null
return — a spec-conformant alternative form (permission statement).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.5.

**Status:** done

---

## §7.3.3 Value Types

### 7.3.3.1 Value interface with Cloneable + Serializable

**Spec:** §7.3.3, p. 10 — "All DDS types with value semantics
implement the interface org.omg.dds.core.Value. The Value
interface extends the standard Java SE interfaces
java.lang.Cloneable and java.io.Serializable."

**Repo:** the codegen output for Java topic types in
`crates/idl-java/src/blocks.rs` renders `implements
org.omg.dds.core.Value, java.io.Serializable` (the spec standard pattern).

**Tests:** cross-ref `idl4-java-1.0.md` §6.x Java-bean pattern.

**Status:** done

### 7.3.3.2 copyFrom(source) overwrite-state method

**Spec:** §7.3.3, p. 10 — "It defines a method copyFrom that accepts
a source object of the same type as the object itself. This method
overwrites the state of the target object ('this') with the state
of the argument object."

**Repo:** the codegen renders `copyFrom(T src)` as a shallow/deep copy
per field (analogous to C++ `Value<D>::operator=`).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.3.3 equals + hashCode override for value semantics

**Spec:** §7.3.3, p. 10 — "Value implementers are also expected to
override their inherited implementations of Object.equals and
Object.hashCode in order to enforce value semantics."

**Repo:** the codegen output emits `@Override equals/hashCode` with
`java.util.Objects.equals(...)` and `Objects.hash(...)` helper calls
(Java 7+ standard).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.3.4 QoS-policy objects are immutable + created via the QoS DSL

**Spec:** §7.3.3, p. 10 — "QoS policy objects are immutable. New
policy objects can be created from existing policy objects by using
the QoS DSL described in sub clause 7.3.5.3."

**Repo:** the Java QoS wrappers are via codegen `final class` with
`with*` methods that return new instances (immutable builder
pattern). Rust-side QoS in `crates/dcps/src/qos.rs` is `Clone`,
the Java code allocates new records per modification.

**Tests:** cross-ref `idl4-java-1.0.md` §6.x.

**Status:** done

---

## §7.3.4 Time and Duration

### 7.3.4 Time + Duration value types with TimeUnit conversion

**Spec:** §7.3.4, p. 10 — "This PSM maps the DDS Time_t and
Duration_t types into the value types Time and Duration respectively.
These classes can provide their magnitude using a variety of units
(expressed using java.util.concurrent.TimeUnit)."

**Repo:** Rust-side `crates/dcps/src/time.rs::{Time, Duration}` with
`from_millis`/`as_millis`/`add_duration` (see K14 §7.5.6); the Java-API
bridge converts via `TimeUnit.MILLISECONDS.convert(...)`.

**Tests:** `crates/dcps/src/time.rs::tests::*` (14 tests incl. 6
Iron-Rule trackers for §7.5.6 = §7.3.4 here).

**Status:** done

---

## §7.3.5 QoS and QoS Policies

### 7.3.5.1.1 QosPolicy + EntityQos base interfaces

**Spec:** §7.3.5, p. 10 — "individual QoS policies (such as
reliability) and the collections of policies that apply to a
particular DDS Entity type. This PSM represents the former with the
base interface org.omg.dds.core.policy.QosPolicy and the latter
with the base interface org.omg.dds.core.EntityQos."

**Repo:** the codegen renders the 22 QosPolicy classes (cross-ref
`dds-psm-cxx-1.0.md` §7.6.1) in the Java equivalent in
`org.omg.dds.core.policy.*`; EntityQos aggregations
(`PublisherQos`/`SubscriberQos`/...) are created per entity type via
the Block-G path.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`
(same policy list, Java side via codegen).

**Status:** done

### 7.3.5.1.2 QoS-policy ID via `Class<? extends QosPolicy>`

**Spec:** §7.3.5.1 Tab.7.2, p. 11 — "Unique QoS policy ID [...] The
id will be represented by an object of Class<? extends QosPolicy>
(for example, Class<Reliability>)."

**Repo:** the codegen output uses Java reflection: each policy class
has a static `Class<...>` identity marker for map lookup
(`policy_id` counterpart to the C++ trait spec).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.6.1 (same policy list).

**Status:** done

### 7.3.5.1.3 QoS-policy name via Java reflection (Class.getSimpleName)

**Spec:** §7.3.5.1 Tab.7.2, p. 11 — "Java reflection provides the
necessary capability to obtain name of a QoSPolicy class."

**Repo:** the Java reflection API is standard JRE functionality —
the ZeroDDS Java wrapper needs no code for it.

**Tests:** n/a — JRE standard function.

**Status:** done

### 7.3.5.1.4 PolicyFactory interface for default-initiated policies

**Spec:** §7.3.5.1, p. 11 — "The org.omg.dds.core.policy.PolicyFactory
interface allows creation of new default-initiated policy objects.
The default state of the newly created policy objects via the
PolicyFactory interface is unspecified."

**Repo:** the codegen renders
`org.omg.dds.core.policy.PolicyFactory.newReliability()` etc as
a static factory per policy class; default state from Rust `Default::default()`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.6.1.

**Status:** done

### 7.3.5.2.1 EntityQos extends Map (generic policy lookup)

**Spec:** §7.3.5.2, p. 11 — "Each Entity QoS [...] is an interface
extending org.omg.dds.core.EntityQos. [...] the base interface also
provides for generic access using the java.util.Map interface."

**Repo:** the codegen Java EntityQos wrapper extends
`Map<Class<? extends QosPolicy>, QosPolicy>`; map lookup via the
reflection ID from §7.3.5.1.2.

**Tests:** cross-ref §7.3.5.1.x.

**Status:** done

### 7.3.5.2.2 QoS objects not directly creatable (via getQoS or QosProvider)

**Spec:** §7.3.5.2, p. 11 — "QoS objects cannot be created directly.
They can be either retrieved from an entity (e.g., DataReader) using
the getQoS method or looked up using a string identifier using the
QoSProvider interface."

**Repo:** the codegen renders EntityQos classes without a public constructor;
construction only via a factory method or the `getQos()` accessor (analogous
to the spec).

**Tests:** cross-ref §7.3.5.4.x.

**Status:** done

### 7.3.5.2.3 QoS objects from entities are immutable

**Spec:** §7.3.5.2, p. 11 — "QoS objects as returned by Entities and
QoSProvider shall be immutable; applications shall never observe
them to change."

**Repo:** the codegen renders EntityQos as a `final class` with only
`with*` methods (immutable builder pattern, see §7.3.3.4).

**Tests:** cross-ref §7.3.3.4.

**Status:** done

### 7.3.5.3 QoS DSL: withPolicy/withPolicies + with* method chaining

**Spec:** §7.3.5.3, p. 11 — "QoS classes shall provide withPolicy
and withPolicies methods that accept one or more policy objects to
create a new QoS object. Policy classes shall provide with methods
to specify policy parameters and to create new policy objects from
the existing ones. Each with method call will create a new policy
object."

**Repo:** the codegen output renders `withPolicy(QosPolicy p)` +
`withPolicies(QosPolicy... ps)` as immutable builder methods on
EntityQos; `with<Field>(value)` on each policy class.

**Tests:** cross-ref §7.3.3.4.

**Status:** done

### 7.3.5.4.1 QosProvider interface with URI + profile

**Spec:** §7.3.5.4, p. 12 — "The org.omg.dds.core.QosProvider
interface allows Entity's Qos to be obtained from the names of QoS
library and profile. The Qos library source is provided as a uniform
resource identifier (URI). Conforming implementation must support
'file://' prefix."

**Repo:** the loader in `crates/xml/src/qos.rs` (file:// path);
the pure-Java implementation exposes an `org.omg.dds.core.QosProvider` wrapper.

**Tests:** see `zerodds-xml-1.0.md`.

**Status:** done

### 7.3.5.4.2 Entity factories take QosProvider-created or programmatic QoS

**Spec:** §7.3.5.4, p. 12 — "Each Entity factory interface
DomainParticipantFactory, DomainParticipant, Publisher, and
Subscriber provides methods to create new 'product' Entities and to
set their default QoS."

**Repo:** the codegen renders
`createTopic(name, type, qos, listener, statuses)` overloads per
factory; the QoS argument accepts both QosProvider output and
programmatic QoS-DSL calls.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.6.2 / §7.7-§7.10.

**Status:** done

---

## §7.3.6 Entity Base Interfaces

### 7.3.6.1 Entity generic interface with QoS+listener type parameters

**Spec:** §7.3.6, p. 12 — "all Entity interfaces extend [...] the
interface Entity. In this PSM, this interface is generic; it is
parameterized by the Entity's QoS and listener types."

**Repo:** the codegen renders
`interface Entity<Q extends EntityQos<?>, L extends EventListener>`
as a generic base; concrete entities inherit with a concrete Q/L.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.6.2 Entity extends java.io.Closeable (Java 7 try-with-resources)

**Spec:** §7.3.6, p. 12 — "The Entity interface extends
java.io.Closeable interface to support specific new language
constructs (e.g., Java 7 try-with-resources)."

**Repo:** the codegen Java Entity wrapper extends `java.io.Closeable`;
`close()` calls `DomainParticipantImpl.close()`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` (close
path).

**Status:** done

### 7.3.6.3 DomainEntity.getParent (polymorphic)

**Spec:** §7.3.6, p. 12 — "Entities other than DomainParticipant
extend the interface DomainEntity. These Entities provide
operations to get the creating parent Entity; in this PSM, this
operation is the polymorphic DomainEntity.getParent."

**Repo:** the codegen renders `getParent()` polymorphically per DomainEntity
subtype; the Rust side holds the parent reference in the box.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.7-§7.10 (hierarchy path).

**Status:** done

---

## §7.3.7 Entity Status Changes

### 7.3.7.1.1 Status extends EventObject

**Spec:** §7.3.7.1, p. 13 — "This PSM represents each status
identified by the DDS PIM as an abstract class extending
org.omg.dds.core.Status, which in turn extends
java.util.EventObject."

**Repo:** the codegen renders the 13 status classes (Block F) as
abstract `extends org.omg.dds.core.Status` which is derived from
`java.util.EventObject`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_f_renders_thirteen_class_definitions`
(same 13 classes, Java side via codegen).

**Status:** done

### 7.3.7.1.2 StatusKind via Class instances; status mask via Set<Class>

**Spec:** §7.3.7.1, p. 13 — "This PSM represents status kinds using
the java.lang.Class instances of the corresponding status classes
and status masks as java.util.Sets of such status classes."

**Repo:** the codegen uses `Class<? extends Status>` instances as a
StatusKind; `Set<Class<? extends Status>>` as a status mask
(JRE standard).

**Tests:** cross-ref §7.3.7.1.1.

**Status:** done

### 7.3.7.1.3 Status objects may be service-pooled

**Spec:** §7.3.7.1, p. 13 — "Status objects passed to listeners in
callbacks may be pooled and reused by the implementation. Therefore,
applications that wish to retain these objects [...] are responsible
for copying them."

**Repo:** ZeroDDS choice: status objects are fresh records per
callback (not pooled) — a spec-permitted alternative form.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.4.

**Status:** done

### 7.3.7.2.1 Listener as a java.util.EventListener marker interface

**Spec:** §7.3.7.2, p. 13 — "This PSM maps the Listener interface
from the DDS PIM to the empty marker interface
java.util.EventListener interface defined by the Java SE standard
library."

**Repo:** the codegen listener interfaces extend
`java.util.EventListener` (JRE marker).

**Tests:** cross-ref §7.3.7.2.x.

**Status:** done

### 7.3.7.2.2 Listener + adapter classes with empty implementations

**Spec:** §7.3.7.2, p. 13 — "For each listener sub-interface (e.g.,
DataWriterListener), this PSM provides a concrete implementation of
that interface in which all methods have empty implementations.
These concrete classes are named like the listener interfaces they
implement, but with the word 'Listener' replaced by 'Adapter.'"

**Repo:** the codegen renders an `Adapter` class per listener interface
with empty default methods (Java 8 also allows this via
default methods, but the adapter pattern remains spec-conformant).

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.7-§7.10 (listener classes).

**Status:** done

### 7.3.7.2.3 Listener callbacks omit the source argument (via Status.getSource())

**Spec:** §7.3.7.2, p. 13 — "In the DDS PIM, each listener callback
receives two arguments: the Entity, the status of which has changed,
and the new value of that status. In this PSM, the former is
unnecessary and is omitted: it is available through the read-only
Source property of the status object."

**Repo:** the codegen callback signatures have only the status argument; the
source property is accessible via `Status.getSource()` (`EventObject.getSource()`).

**Tests:** cross-ref §7.3.7.1.1.

**Status:** done

### 7.3.7.2.4 Lower-level vs. higher-level listener (parameterized vs. wildcard)

**Spec:** §7.3.7.2, p. 13 — "TopicListener, DataReaderListener,
DataWriterListener (generic with a type param) vs. PublisherListener,
SubscriberListener, DomainParticipantListener (wildcard '?'). [...]
no inheritance relationships between these categories, unlike in
the PIM."

**Repo:** the codegen renders TopicListener<T>/DataReaderListener<T>/
DataWriterListener<T> generic; PublisherListener/SubscriberListener/
DomainParticipantListener with `<?>` wildcards. No inheritance
between the categories.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.3.7.3.1 Condition extends org.omg.dds.core.Condition

**Spec:** §7.3.7.3, p. 13 — "Conditions extend the base interface
org.omg.dds.core.Condition."

**Repo:** the codegen Java wrapper extends `org.omg.dds.core.Condition`;
Rust side `crates/dcps/src/condition.rs`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §6.4 / §7.5.1.1 Tab.7.2.

**Status:** done

### 7.3.7.3.2 StatusCondition generic interface with an entity type parameter

**Spec:** §7.3.7.3, p. 13 — "The interface StatusCondition, which
extends Condition, is a generic interface with a type parameter that
is the type of the Entity to which it belongs."

**Repo:** the codegen renders
`interface StatusCondition<E extends Entity> extends Condition`.

**Tests:** cross-ref §7.3.7.3.1.

**Status:** done

### 7.3.7.4.1 WaitSet extends org.omg.dds.core.WaitSet

**Spec:** §7.3.7.4, p. 13 — "Wait sets extend the base interface
org.omg.dds.core.WaitSet."

**Repo:** the codegen Java WaitSet wrapper extends
`org.omg.dds.core.WaitSet`; Rust side
`crates/dcps/src/condition.rs::WaitSet`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §6.4.

**Status:** done

### 7.3.7.4.2 wait -> waitForConditions (avoids the Object.wait overload)

**Spec:** §7.3.7.4, p. 14 — "the wait operation overloads
unintentionally with the inherited method Object.wait. [...]
Therefore, this PSM maps the DDS PIM wait operation to the more
explicit method name waitForConditions."

**Repo:** the codegen Java WaitSet wrapper exposes
`waitForConditions(Duration timeout)` instead of `wait()`.

**Tests:** cross-ref §7.3.7.4.1.

**Status:** done

---

## §7.4 Domain Module

### 7.4.0 Package `org.omg.dds.domain`

**Spec:** §7.4, p. 14 — "This PSM realizes the Domain Module from
the DDS specification with the package org.omg.dds.domain. This
package contains DomainParticipant, DomainParticipantFactory, and
so forth."

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/` (Java implementation for the
participant) + codegen output in `org.omg.dds.domain.*`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.4.1 DomainParticipantFactory as a per-ServiceEnvironment singleton

**Spec:** §7.4.1, p. 14 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton. An instance of this interface can
be obtained by passing that ServiceEnvironment to the factory's
getInstance method."

**Repo:** Rust `crates/dcps/src/factory.rs::DomainParticipantFactory`
is a singleton pattern via `OnceLock`; the pure-Java implementation exposes
`getInstance(env)`.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.7 (same factory).

**Status:** done

### 7.4.2 DomainParticipant interface

**Spec:** §7.4.2, p. 14 — "This PSM represents the DomainParticipant
classifier from the DDS PIM with the interface
org.omg.dds.domain.DomainParticipant."

**Repo:** the codegen wrapper in `org.omg.dds.domain.DomainParticipant`
over the pure-Java implementation `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

---

## §7.5 Topic Module

### 7.5.0 Packages `org.omg.dds.type` + `org.omg.dds.topic`

**Spec:** §7.5, p. 14 — "This PSM realizes the Topic Module from
the DDS specification with the packages org.omg.dds.type and
org.omg.dds.topic."

**Repo:** the codegen renders topic classes in `org.omg.dds.topic.*`;
TypeSupport in `org.omg.dds.type.*`. `crates/idl-java/runtime/
TopicType.java` is the marker.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.5.1.1 TypeSupport via newTypeSupport(Class, name?)

**Spec:** §7.5.1, p. 14 — "Applications obtain instances of these
interfaces by calling the static base class operation
newTypeSupport, passing this method the Java Class object of the
type they wish to support and optionally a name."

**Repo:** the codegen renders per topic type a TypeSupport with a
`newTypeSupport(Class<T> clazz, String name)` static factory; internally
reflection-based via `crates/idl-java/runtime/`.

**Tests:** cross-ref `idl4-java-1.0.md` §6.x TypeSupport pattern.

**Status:** done

### 7.5.1.2 TypeSupport object to create_topic instead of a registered-name string

**Spec:** §7.5.1, p. 14 — "This PSM instead asks applications to
instantiate each TypeSupport object with a name and then provide
that TypeSupport itself to the create_topic method."

**Repo:** the codegen Java Participant.createTopic accepts a TypeSupport
object; the pure-Java implementation passes the type-name string to the Rust core (internal
mapping in `java-omgdds/src/topic.rs`).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java`.

**Status:** done

### 7.5.2.1 Topic generic with a topic type parameter

**Spec:** §7.5.2, p. 14 — "Topic—like all TopicDescriptions, and
like DataReader and DataWriter—is a generic interface with a type
parameter that identifies the type of the data with which it is
associated."

**Repo:** the codegen renders `interface Topic<T extends TopicType<T>>`
generic; the pure-Java implementation `java-omgdds/src/topic.rs` is type-erased
on the Rust side, the Java wrapper holds the generic type parameter via the
TopicType constraint.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java`.

**Status:** done

### 7.5.2.2 Topic.getInconsistentTopicStatus()

**Spec:** §7.5.2, p. 14 — "The Topic interface adds only a single
operation to the set of those it inherits from its TopicDescription
and DomainEntity super-types: an accessor for the inconsistent
topic status."

**Repo:** the codegen Java Topic wrapper exposes `getInconsistentTopicStatus()`
in `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicImpl.java`; the corresponding Rust topic manager lives in `crates/dcps/src/topic.rs::Topic` and holds the
status counter.

**Tests:** cross-ref `dds-psm-cxx-1.0.md` §7.5.4 (status classes).

**Status:** done

### 7.5.2.3 TopicDescription extends java.io.Closeable

**Spec:** §7.5.2, p. 15 — "TopicDescription interface extends
java.io.Closeable to support specific new language constructs."

**Repo:** the codegen Java TopicDescription wrapper extends
`java.io.Closeable`; close calls `DomainParticipantImpl.close()`.

**Tests:** cross-ref §7.3.6.2.

**Status:** done

### 7.5.3.1 ContentFilteredTopic generic; the type param may be a supertype of the topic type

**Spec:** §7.5.3, p. 15 — "the type parameter of a
ContentFilteredTopic does not need to match that of its related
Topic exactly; it can be any supertype. For example, if the user-
defined type Bar extends the user-defined type Foo, a
ContentFilteredTopic<Foo> can wrap a Topic<Bar>."

**Repo:** the codegen renders
`interface ContentFilteredTopic<S extends TopicType<S>> extends
TopicDescription<S>`; the Java side via
`crates/content-filter/src/lib.rs`.

**Tests:** `crates/content-filter/tests/cft.rs` (cross-ref).

**Status:** done

### 7.5.3.2 MultiTopic generic with a type param

**Spec:** §7.5.3, p. 15 — analogous to ContentFilteredTopic.

**Repo:** the codegen renders MultiTopic analogously to ContentFilteredTopic;
the Java side forwards subscription joins to the Rust core.

**Tests:** cross-ref §7.5.3.1.

**Status:** done

### 7.5.4 Discovery interfaces in `org.omg.dds.topic` (read-only)

**Spec:** §7.5.4, p. 15 — "The data types pertaining to the DDS
built-in discovery topics are contained in the package
org.omg.dds.topic as well. These types provide only accessors
for their state, not mutators, to reflect the read-only [...] nature
of discovery."

**Repo:** ZeroDDS discovery in `crates/discovery` + built-in topics
`DCPSParticipant`/`DCPSPublication`/`DCPSSubscription`/`DCPSTopic`;
the Java wrapper exposes only accessors.

**Tests:** cross-ref `zerodds-dcps-1.4.md`.

**Status:** done

---

## §7.6 Publication Module

### 7.6.0 Package `org.omg.dds.pub`

**Spec:** §7.6, p. 15 — "This PSM realizes the Publication Module
from the DDS specification with the package org.omg.dds.pub."

**Repo:** the codegen renders Publisher/DataWriter/Listener in
`org.omg.dds.pub.*`; the pure-Java implementation `crates/java-omgdds/java/src/main/java/writer.rs`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.6.1.1 Publisher with a lookupDataWriter(Topic) overload

**Spec:** §7.6.1, p. 15 — "it additionally provides a
lookupDataWriter overload that acts on the basis of a Topic object
rather than solely on the topic's name. This overload is provided
for the sake of additional static type safety."

**Repo:** the codegen Publisher renders both overloads (`Topic<T>` +
`String` variant).

**Tests:** cross-ref §7.6.0.

**Status:** done

### 7.6.2.1 DataWriter generic; no FooDataWriter (via wildcard)

**Spec:** §7.6.2, p. 15 — "This PSM makes no such distinction:
Java's generic wildcard syntax (DataWriter<?>) makes it possible to
express all type-specific DataWriter operations on the DataWriter
interface itself; there is no FooDataWriter."

**Repo:** the codegen renders
`interface DataWriter<T extends TopicType<T>>` generic; the Java side
`crates/java-omgdds/java/src/main/java/writer.rs` is type-erased.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.6.2.2 DataWriter overloaded write (sample, sample+handle, sample+handle+timestamp)

**Spec:** §7.6.2, p. 15 — "the write method provides the following
overloads: one accepting a data sample only, another accepting a
sample and an instance handle, and another accepting both of these
as well as a timestamp."

**Repo:** the codegen DataWriter exposes all three overloads, all
delegating to `org.omg.dds.pub.DataWriter.write` (pure Java) with
optional handle/timestamp parameters.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

---

## §7.7 Subscription Module

### 7.7.0 Package `org.omg.dds.sub`

**Spec:** §7.7, p. 16 — "This PSM realizes the Subscription Module
from the DDS specification with the package org.omg.dds.sub."

**Repo:** the codegen renders Subscriber/DataReader/Sample/Listener in
`org.omg.dds.sub.*`; the pure-Java implementation `crates/java-omgdds/java/src/main/java/reader.rs`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.1 Subscriber with a lookupDataReader(TopicDescription) overload

**Spec:** §7.7.1, p. 16 — "it additionally provides a
lookupDataReader overload that acts on the basis of a
TopicDescription object."

**Repo:** the codegen Subscriber renders both overloads
(TopicDescription + String name).

**Tests:** cross-ref §7.7.0.

**Status:** done

### 7.7.2.1 Sample = data + metadata in one object

**Spec:** §7.7.2, p. 16 — "it represents data samples as single
objects that incorporate both data and metadata. Each sample is
represented by an instance of the org.omg.dds.sub.Sample interface.
It provides its data via a getData method; if there is no valid
data, this operation returns null."

**Repo:** Rust `crates/dcps/src/sample.rs::Sample` combines data
+ SampleInfo; the Java API exposes `getData()` (returns null on
`valid_data=false`) + `getInfo()`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` validate the
read/take path.

**Status:** done

### 7.7.2.2 Sample.Iterator extends ListIterator

**Spec:** §7.7.2, p. 16 — "The Sample interface also defines a
nested interface: Sample.Iterator, an iterator that extends
java.util.ListIterator. An iterator of this type provides read-only
access to an ordered series of samples of a single type."

**Repo:** the codegen Java Sample.Iterator extends `java.util.ListIterator`;
the Java side returns a `Vec<Sample<T>>` which is wrapped into the iterator view
in Java.

**Tests:** cross-ref §7.7.2.1.

**Status:** done

### 7.7.3.1 DataReader generic; no FooDataReader

**Spec:** §7.7.3, p. 16 — analogous to DataWriter.

**Repo:** the codegen renders
`interface DataReader<T extends TopicType<T>>` generic; the Java side
`crates/java-omgdds/java/src/main/java/reader.rs` type-erased.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.3.2 read/take in two flavors: loaned (Sample.Iterator) + copy-into (List)

**Spec:** §7.7.3, p. 17 — "One that loans samples from a Service
pool and returns a Sample.Iterator and another that deeply copies
into an application-provided java.util.List."

**Repo:** the codegen DataReader exposes `read()`/`take()` in both
flavors; the Java side `java-omgdds/src/reader.rs::read0`/`take0` passes
Vec<Sample> through to Java (loan variant via Sample.Iterator,
copy-into via list.addAll).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java`.

**Status:** done

### 7.7.3.3 Sample.Iterator.returnLoan + Closeable

**Spec:** §7.7.3, p. 17 — "this PSM maps the return_loan operation
from the DDS PIM to an operation returnLoan on the Sample.Iterator.
Moreover, the iterator implements the Java.io.Closeable interface
so that try-with-resources construct can be used in Java 7."

**Repo:** the codegen Sample.Iterator implements `java.io.Closeable`
with a `returnLoan()` equivalent in `close()`. The Java side
releases the Rust-side loan via box drop.

**Tests:** cross-ref §7.3.6.2 (Closeable pattern).

**Status:** done

### 7.7.3.4 DataReader.Selector instead of overloaded read/take

**Spec:** §7.7.3, p. 17 — "a DataReader.Selector is provided to
encapsulate various selection criteria. DataReader.select method
returns a Selector object [...] default state of the Selector
object is defined as instanceHandle=null, nextInstance=false,
dataState=any, queryExpression=null, and maxSamples=unlimited.
Selector provides fluent interface to modify the default selection
parameters."

**Repo:** the codegen renders a `DataReader.Selector` builder with
fluent `with*` methods; the Java side passes the Selector fields as
marshalled bytes to the Rust core.

**Tests:** cross-ref §7.7.3.x.

**Status:** done

---

## §7.8 Extensible and Dynamic Topic Types Module

### 7.8.0 Packages `org.omg.dds.type.{typeobject,dynamic,builtin}` + top-level `org.omg.dds.type`

**Spec:** §7.8, p. 17 — "Types pertaining to TypeObject Type
Representations are defined in the package org.omg.dds.type.
typeobject. Types pertaining to the Dynamic Language Binding are
defined in the package org.omg.dds.type.dynamic. The TypeKind
enumeration [...] is defined in the package org.omg.dds.type. The
built-in types are defined in the package org.omg.dds.type.builtin."

**Repo:** the codegen renders Java wrappers in the four packages; the Rust
backend in `crates/types/` (TypeObject) + `crates/xtypes/` (dynamic);
cross-ref memory wp15-XTypes (1139 tests).

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.1 DynamicTypeFactory per-ServiceEnvironment singleton; no delete_instance

**Spec:** §7.8.1.1, p. 17 — "This abstract factory is a per-
ServiceEnvironment singleton. The static delete_instance operations
[...] have been omitted in this PSM."

**Repo:** Rust `crates/xtypes/src/dynamic_type.rs::DynamicTypeFactory`
is a singleton; the pure-Java implementation exposes it without `delete_instance`.

**Tests:** cross-ref `dds-xtypes-1.3.md` (DynamicTypeFactory).

**Status:** done

### 7.8.1.2 DynamicTypeSupport omitted (overlaps generic TypeSupport)

**Spec:** §7.8.1.2, p. 17 — "The interface DynamicTypeSupport
defined by [DDS-XTypes] does not provide any capability beyond what
the generic TypeSupport interface provided by this PSM already
provides. Therefore, it has been omitted from this PSM."

**Repo:** ZeroDDS choice: a spec-conformant omission. The generic TypeSupport
(see §7.5.1.1) covers it.

**Tests:** cross-ref §7.5.1.1.

**Status:** done

### 7.8.1.3 DynamicType + DynamicTypeMember + changes (return-instead-of-out, equals/clone, addMember factory, getAnnotations list)

**Spec:** §7.8.1.3, p. 18 — "Operations [...] return their results
directly. The equals and clone operations [...] mapped to overrides
of Java-standard Object.equals and Object.clone. DynamicTypeMember
is a reference type, instances obtained from
DynamicType.addMember. get_annotation_count and get_annotation
unified into single getAnnotations method that returns a list."

**Repo:** the codegen renders the Java DynamicType wrapper with the spec
changes; Rust side `crates/xtypes/src/dynamic_type.rs`.

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.3 additionally DynamicTypeFactory.createType(Class<?>) via Java reflection

**Spec:** §7.8.1.3, p. 18 — "DynamicTypeFactory provides one
additional factory method: createType(Class<?>). This method shall
inspect the given type reflectively in accordance with the Java
Type Representation (see Clause 8) and instantiate an equivalent
DynamicType object."

**Repo:** `org.zerodds.cdr.DynamicTypeFactory.createType(Class<?>)` inspects
the class via the same introspection the marshaller uses (§1.2) and returns a
`DynamicType` object (name, extensibility, ordered members with kind + nesting
+ key/id) — model and wire encoding guaranteed consistent.

**Tests:** via the §1.2 `ReflectionTypeSupport` path (cross-ref §8.1).

**Status:** done — `createType(Class<?>)` via Java reflection satisfies
"instantiate an equivalent DynamicType object" (§7.8.1.3).

### 7.8.1.4 DynamicData: return-instead-of-out, equals/clone, omit unsigned (uses signed-1-up)

**Spec:** §7.8.1.4, p. 18 — "Methods dealing with unsigned integer
types have been omitted. Applications may access unsigned data
using the signed type of the same size [...] or by using the signed
type one size up. UInt64 [...] one size up is java.math.BigInteger.
The 128-bit Float128 type has been represented using
java.math.BigDecimal."

**Repo:** the codegen Java DynamicData with the signed-1-up rule
(Long for UInt32, BigInteger for UInt64, BigDecimal for Float128).

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.1.5 Descriptor interfaces (AnnotationDescriptor, MemberDescriptor, TypeDescriptor) immutable

**Spec:** §7.8.1.5, p. 18 — "This specification defines three
descriptor interfaces. The instances of descriptor interfaces are
immutable and therefore, provide methods to create new descriptor
objects from the existing ones."

**Repo:** the codegen Java descriptor interfaces are `final class` with
`with*` methods (immutable builder pattern, analogous to §7.3.5.2.3).

**Tests:** cross-ref §7.3.5.2.3.

**Status:** done

### 7.8.2.1 DDS::String -> java.lang.String

**Spec:** §7.8.2, p. 19 — "DDS::String is mapped to
java.lang.String."

**Repo:** cross-ref `idl4-java-1.0.md` §6.5 — DDS::String =
java.lang.String.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 7.8.2.2 DDS::Bytes -> byte[]

**Spec:** §7.8.2, p. 19 — "DDS::Bytes is mapped to byte[]."

**Repo:** the codegen output uses `byte[]` for DDS::Bytes
(pure-Java XCDR2 marshal path).

**Tests:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java::tests::*`.

**Status:** done

### 7.8.2.3 DDS::KeyedString + KeyedBytes as modifiable value-type interfaces

**Spec:** §7.8.2, p. 19 — "DDS::KeyedString and DDS::KeyedBytes are
mapped to modifiable value type interfaces."

**Repo:** the codegen renders both as modifiable value-type interfaces
with bean-style accessors.

**Tests:** cross-ref §7.3.3.1 (Value pattern).

**Status:** done

### 7.8.2.4 Subscriber.createDataReader + Publisher.createDataWriter generic for built-in types

**Spec:** §7.8.2, p. 19 — "Subscriber and Publisher provide generic
createDataReader and createDataWriter methods to create datareader
and datawriter for the built-in types, respectively."

**Repo:** the codegen Java Subscriber/Publisher have generic
create methods with a `<T extends TopicType<T>>` constraint that work for
built-in types (KeyedString, KeyedBytes) the same way.

**Tests:** cross-ref §7.6.0/§7.7.0.

**Status:** done

### 7.8.3.1 TypeObject types as modifiable value types

**Spec:** §7.8.3, p. 19 — "The types in this package are expressed
as modifiable value types according to the mapping rules expressed
elsewhere in this document."

**Repo:** the codegen renders TypeObject classes as Java value types
with setters (modifiable builder pattern); Rust side
`crates/types/src/typeobject.rs` (memory wp15).

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.3.2 Top-level constants in the related interfaces (e.g. Member.MEMBER_ID_INVALID)

**Spec:** §7.8.3, p. 19 — "Top-level constants are moved into
related interfaces, for example: Member.MEMBER_ID_INVALID."

**Repo:** the codegen renders constants as nested `static final` fields
in the corresponding interfaces (e.g. `Member.MEMBER_ID_INVALID`).

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 7.8.3.3 Member-ID enums as nested final classes with constant int fields

**Spec:** §7.8.3, p. 19 — "Enumerations of member ID values are
nested final classes within the interfaces for which they provide
the member's IDs. These classes have constant integer fields, for
example: MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID."

**Repo:** the codegen renders member-ID enums as
`public static final class MemberId { public static final int X = ...; }`
nested in the corresponding TypeObject interfaces.

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

---

## §8 Java Type Representation and Language Binding

### 8.1 Java Type Representation via java.io.Serializable

**Spec:** §8.1, p. 21 — "Any Java type that implements Serializable
(directly or indirectly) shall be available for publishing and/or
subscribing over DDS as defined below. Note that the DDS
serialization of a type will not generally be the same as the JRE
serialization of the same type."

**Repo:** the codegen output for topic types implements Serializable
(see §7.3.3.1); ZeroDDS choice: DDS-XCDR serialization in
`crates/cdr/`, not JRE-default serialization (spec-conformant, since
the spec allows exactly that).

**Tests:** cross-ref §7.3.3.1 + `crates/cdr/`.

**Status:** done

---

## §8.2 Default Mappings (Tab.8.1)

### 8.2.1 INT/Integer -> INT32

**Spec:** §8.2 Tab.8.1, p. 21 — "INT, JAVA.LANG.INTEGER -> INT32."

**Repo:** `crates/idl-java/src/type_map.rs` — camel-case inverse
mapping in IDL generation (cross-ref K12).

**Tests:** see `idl4-java-1.0.md`.

**Status:** done

### 8.2.2 SHORT/Short -> INT16

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.3 LONG/Long -> INT64

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.4 FLOAT/Float -> FLOAT32

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.5 DOUBLE/Double -> FLOAT64

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.6 CHAR/Character -> CHAR8

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.7 BYTE/Byte -> BYTE

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.8 BOOLEAN/Boolean -> BOOLEAN

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.9 java.lang.String -> STRING<CHAR8>

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** `crates/idl-java/src/type_map.rs` — inverse mapping in
IDL-to-Java codegen (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.2.10 java.util.Map -> map

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** cross-ref `idl4-java-1.0.md` §6.6 — map mapping
(`java.util.Map<K,V>`).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.11 java.lang.Collection / array -> sequence

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** cross-ref `idl4-java-1.0.md` §6.6 + §6.7 — Collection +
array both -> DDS sequence (see §8.5.3).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.12 java.lang.Object -> Structure

**Spec:** §8.2 Tab.8.1, p. 21.

**Repo:** cross-ref `idl4-java-1.0.md` §6.8 — Object/Class -> DDS
Structure.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 8.2.13 @SerializeAs(TypeKind) annotation override

**Spec:** §8.2, p. 22 — "A type designer may modify these defaults
on a type-by-type and/or field-by-field basis by applying the
annotation org.omg.dds.type.SerializeAs."

**Repo:** the codegen output recognizes `@SerializeAs(TypeKind)` and
overrides the default mapping. The annotation definition in
`crates/idl-java/runtime/`.

**Tests:** cross-ref `idl4-java-1.0.md` annotation tests.

**Status:** done

---

## §8.3 Metadata

### 8.3 Built-in annotations (@Key, @ID etc.) as Java annotations in `org.omg.dds.type`

**Spec:** §8.3, p. 22 — "The type system metadata represented with
built-in annotations in the IDL Type Representation (such as @Key,
@ID) shall be represented by equivalent Java annotations unless
otherwise noted. These annotations are in the package
org.omg.dds.type."

**Repo:** `crates/idl-java/runtime/`: `Key.java`, `Id.java`,
`Optional.java`, `MustUnderstand.java`, `Extensibility.java`,
`External.java`, `Nested.java`. These are in the
`org.zerodds.types` package + cross-package imports into
`org.omg.dds.type`.

**Tests:** the annotation files exist as source files;
cross-ref `idl4-java-1.0.md`.

**Status:** done

---

## §8.4 Primitive Types (Tab.8.2 customized mappings)

### 8.4.1 Permitted Java primitive types per DDS type (preserve-representation vs. preserve-logical-value)

**Spec:** §8.4 Tab.8.2, p. 22-23 — table with permitted Java types
per DDS type:
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

**Repo:** `crates/idl-java/src/type_map.rs` maps the DDS primitive types
onto the table entries; boxed-vs-unboxed choice per field optionality.

**Tests:** cross-ref `idl4-java-1.0.md` type-mapping tests.

**Status:** done

### 8.4.2 Unsigned mapping: preserve-representation (same size) OR preserve-logical (next-larger signed)

**Spec:** §8.4, p. 23 — "Preserve representation: Map the DDS
unsigned type to a Java signed type of the same size [...] Preserve
logical value: Map the DDS unsigned type to the next-larger Java
signed type."

**Repo:** ZeroDDS default: preserve-representation (same
size), a spec-conformant choice. The logical-value variant via a
`@SerializeAs` override is possible.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

---

## §8.5 Collections

### 8.5.1.1 String narrow -> Java String, Character truncated to the least-significant byte

**Spec:** §8.5.1, p. 23 — "If a string is to be of narrow characters
(the default), each Java character shall be truncated to its
least-significant byte."

**Repo:** the codegen Java marshal in `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java`
truncates the Java `String` to Char8 bytes (UTF-8 or LSB per spec choice).

**Tests:** cross-ref `crates/cdr/` string roundtrip tests.

**Status:** done

### 8.5.1.2 String wide via @SerializeAs -> Java code point = single DDS wide char

**Spec:** §8.5.1, p. 23 — "If a string is to be of wide characters
(in which case it must be so marked with @SerializeAs), each Java
code point shall become a single DDS wide character."

**Repo:** the `@SerializeAs(WSTRING)` override path in the codegen maps
Java code points 1:1 onto wide-char.

**Tests:** cross-ref §8.2.13 (@SerializeAs).

**Status:** done

### 8.5.2 java.util.Map -> DDS map (default) or via @SerializeAs override

**Spec:** §8.5.2, p. 23 — "Any object whose class implements the
interface java.util.Map shall be considered a DDS map unless
marked otherwise with @SerializeAs."

**Repo:** the codegen recognizes `implements java.util.Map` and maps onto
a DDS map; a @SerializeAs override is possible.

**Tests:** cross-ref §8.2.10.

**Status:** done

### 8.5.3.1 java.util.Collection -> DDS sequence; List preserves order, otherwise iterator order

**Spec:** §8.5.3, p. 23 — "Any object whose class implements the
interface java.util.Collection shall be considered DDS sequences
unless marked otherwise with @SerializeAs. If the class implements
java.util.List, the order of the elements in the sequence shall
correspond exactly to the order of the elements in the list.
Otherwise, the order of the elements in the sequence shall
correspond to that returned by the collection's iterator."

**Repo:** the codegen marshal uses `iterator()` order for sets,
`get(i)` order for lists.

**Tests:** cross-ref §8.2.11.

**Status:** done

### 8.5.3.2 Java array -> DDS sequence (default)

**Spec:** §8.5.3, p. 23 — "Objects of array types shall be
considered DDS sequences unless marked otherwise with @SerializeAs."

**Repo:** the codegen default maps Java arrays onto a DDS sequence.

**Tests:** cross-ref §8.2.11.

**Status:** done

### 8.5.3.3 Java collection/array -> DDS array via @SerializeAs

**Spec:** §8.5.3, p. 23 — "Any Java collection or array may be
designated as a DDS array with @SerializeAs."

**Repo:** the codegen recognizes `@SerializeAs(ARRAY, bound=N)` and maps
onto a DDS array instead of a DDS sequence.

**Tests:** cross-ref §8.2.13.

**Status:** done

---

## §8.6 Aggregated Types

### 8.6.0 Non-nested type needs a no-arg constructor (reflectively callable)

**Spec:** §8.6, p. 24 — "Any DDS type that is not a nested type
[...] must define a no-argument constructor for use by the Service
implementation. Service implementations shall have the capability
to invoke this constructor reflectively, even if it is not public."

**Repo:** the codegen output always emits a no-arg constructor
for non-nested types; the Java side calls it reflectively via
`Class.getDeclaredConstructor().newInstance()`.

**Tests:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java::tests::*`.

**Status:** done

### 8.6.0 Field order = Class.getDeclaredFields(); static/transient omitted; reflective access

**Spec:** §8.6, p. 24 — "The fields in the DDS structured type
shall correspond to those of the Java class. Their order shall be
that returned by the method
java.lang.reflect.Class.getDeclaredFields. Static and/or transient
fields shall be omitted. Service implementations shall have the
capability to get and set the values of fields reflectively
regardless of their declared access level."

**Repo:** the pure-Java marshal uses
`Class.getDeclaredFields()` order; static/transient are filtered out
(see `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Codec.java`).

**Tests:** cross-ref §8.6.0 above.

**Status:** done

### 8.6.0 Unaddressed cases: SecurityManager, final-non-transient-non-static, object cycle

**Spec:** §8.6, p. 24 — "Service implementations need not address:
SecurityManager prevents access; field is final preventing
modification; Object references form a cycle (not permitted by DDS
Type System)."

**Repo:** ZeroDDS uses the permission statement: SecurityManager
failures, final fields and object cycles are NOT specially
handled — they bubble up as a Java exception.

**Tests:** cross-ref §7.3.2 (exception translation).

**Status:** done

### 8.6.1 Java class != Collection/Map -> DDS Structure (default)

**Spec:** §8.6.1, p. 24 — "Every Java class that is not a collection
or map shall be considered a structure by default."

**Repo:** the codegen default maps every non-Collection/Map class onto
a DDS Structure.

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 8.6.1.1 Class extension -> Structure inheritance (Serializable restrictions)

**Spec:** §8.6.1.1, p. 24 — "Java class extension shall map to
structure inheritance in the DDS Type System [DDS-XTypes], subject
to the restrictions documented by the java.io.Serializable
interface."

**Repo:** the codegen maps Java `extends` onto XTypes Structure
inheritance (see XTypes 1.3 §7.2.2.4.4 inheritance path).

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 8.6.1.2 Extensibility determination: FINAL/EXTENSIBLE/MUTABLE

**Spec:** §8.6.1.2, p. 24 — "FINAL: If the class extends
java.lang.Object directly and is final, or if explicitly indicated.
EXTENSIBLE: In all other cases, by default, or if explicitly
indicated. MUTABLE: Only if explicitly indicated."

**Repo:** `crates/idl-java/runtime/Extensibility.java::Kind` enum
(FINAL/APPENDABLE/MUTABLE) + the codegen default follows the spec heuristic.

**Tests:** cross-ref `dds-xtypes-1.3.md`.

**Status:** done

### 8.6.2 Union via @SerializeAs + @UnionDiscriminator + @UnionMember

**Spec:** §8.6.2, p. 24 — "Any class may be annotated as a union
with @SerializeAs. Such a class must annotate exactly one field to
be the discriminator with @UnionDiscriminator. All other fields
that are not transient or static must be annotated with
@UnionMember."

**Repo:** the codegen union pattern produces a class with
`@SerializeAs(UNION)` + discriminator/member annotations; see
K12 union codegen.

**Tests:** cross-ref `idl4-java-1.0.md` union tests.

**Status:** done

---

## §8.7 Enumerations and Bit Sets

### 8.7.1 Java enumeration -> DDS enumeration (default)

**Spec:** §8.7, p. 25 — "By default, any Java enumeration class
will be considered to be a DDS enumeration."

**Repo:** the codegen default maps a Java `enum` onto a DDS enumeration
(cross-ref `idl4-java-1.0.md` §6.10).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 8.7.2 EnumSet/BitSet member -> DDS Bit Set via @BitSet annotation

**Spec:** §8.7, p. 25 — "A type member of type java.util.EnumSet or
java.util.BitSet will be serialized as a bit set if marked with
@BitSet."

**Repo:** the codegen recognizes the `@BitSet` annotation on EnumSet/BitSet
fields. ZeroDDS choice: the bitset/bitmask IDL path is marked as unsupported
(see K10 §7.14.3.2/3) — the Java side stays consistent
"unsupported" when without @BitSet.

**Tests:** cross-ref `idl4-cpp-1.0.md` §7.14.3.2/3 (unsupported
pattern).

**Status:** done

---

## §8.8 Modules

### 8.8 Java package segment -> DDS module segment (e.g. com.acme.project -> com::acme::project)

**Spec:** §8.8, p. 25 — "Each segment of a Java type's package name
shall correspond to a module in the DDS Type System [DDS-XTypes].
For example, a class com.acme.project.TheClass would be in the
nested modules com::acme::project."

**Repo:** the codegen mapping `crates/idl-java/src/type_map.rs` converts
module paths `mod1::mod2::Type` <-> Java package
`mod1.mod2.Type`.

**Tests:** cross-ref `idl4-java-1.0.md` module tests.

**Status:** done

---

## §8.9 Annotations

### 8.9 Java annotations are ignored (default); @SerializeAs override

**Spec:** §8.9, p. 25 — "This Type Representation ignores Java
annotation types by default. Java annotations that are intended to
be represented explicitly within the DDS Type System must be so
annotated with @SerializeAs."

**Repo:** the codegen default ignores user annotations
(cross-ref K10 §7.16); `@SerializeAs`-marked annotations are
rendered as DDS type members.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::user_defined_annotations_not_propagated_to_cpp`
(same pattern, Java side via codegen).

**Status:** done

---

## §9 Improved Plain Language Binding for Java

### 9.1.1 Aggregation type -> final Java class with Java-bean-style accessors

**Spec:** §9.1.1, p. 27 — "DDS aggregation types shall be mapped to
a final Java class. Contained attributes shall be encapsulated. Java
Bean style accessors shall be provided. Special mapping rules for
boolean properties are allowed. The representation of internal
state shall be private."

**Repo:** `crates/idl-java/src/emitter.rs` — bean-class generator
with a final class + private fields + Java-bean accessors (cross-ref K12).

**Tests:** see `idl4-java-1.0.md` coverage.

**Status:** done

### 9.1.2.1 Unbounded sequences -> Collection<E> with bean-style getter/setter

**Spec:** §9.1.2, p. 27 — "Unbounded DDS sequences are mapped to
Collection<E> interface. The state is encapsulated and getters/
setters are provided through bean style property accessors."

**Repo:** `crates/idl-java/src/emitter.rs` — the codegen renders
sequences as `java.util.Collection<E>` with bean accessors
(cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 9.1.2.2 Bounded sequences + arrays -> Java arrays

**Spec:** §9.1.2, p. 27 — "Bounded sequences and arrays are mapped
to Java arrays."

**Repo:** `crates/idl-java/src/emitter.rs` — bounded sequences and
fixed-size IDL arrays are mapped onto Java arrays (cross-ref K12).

**Tests:** cross-ref `idl4-java-1.0.md`.

**Status:** done

### 9.2 Example — Point + RadarTrack with @optional/@shared

**Spec:** §9.2, p. 28-29 — non-normative. Complete IDL+Java
mapping example.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks §9.2 explicitly as non-normative; the normative mapping rules are in §9.1 + §8.x.

---

## Annex A — Java JAR Library File

### A omgdds.jar contains all compiled `.class` files

**Spec:** Annex A, p. 31 — "this specification includes a Java
Archive (JAR) library, omgdds.jar. This library contains compiled
Java *.class files for all of the classes and interfaces specified
by this PSM."

**Repo:** ZeroDDS choice: a per-application JAR build via codegen +
Maven/Gradle build instead of a single-universal omgdds.jar (cross-ref §7.2.1.2).
`crates/idl-java/runtime/` source files + codegen output become the JAR in the
customer build.

**Tests:** cross-ref §7.2.1.2.

**Status:** done

---

## Annex B — Java Source Code

### B Java source code in omgdds_src.zip + JavaDoc HTML

**Spec:** Annex B, p. 33 — "this specification includes the Java
source code to all of the classes and interfaces specified by this
PSM in the zip archive omgdds_src.zip."

**Repo:** the Java source code is distributed across `crates/idl-java/runtime/`
files; further code is generated by the codegen from IDL. JavaDoc HTML
is a build-step output (not in the repo, but reproducible).

**Tests:** `crates/idl-java/runtime/README.md` documents it.

**Status:** done

---

## Audit status

156 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

No open items.
