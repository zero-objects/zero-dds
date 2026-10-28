# DDS-Java 1.0 (Java 5 Language PSM for DDS) — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-java-psm-1.0.pdf` (44 pages, OMG
formal/2013-11-02, November 2013) —
[OMG DDS-Java 1.0](https://www.omg.org/spec/DDS-Java/1.0/)

Item-by-item audit against the PDF following
`internal/spec-coverage/PROCESS.md`. Page numbers without a further note are PDF
pages of this PDF (PDF page = printed page + 10; the cover is PDF page 1, the
preface, printed iii and iv, is on PDF pages 9 and 10). Every normative sentence
of §1 to §9 and of the annexes, every row of Table 7.1, Table 7.2, Table 8.1 and
Table 8.2, every factory method §7.3.1 gives the `ServiceEnvironment`, and every
default of the `DataReader.Selector` is an item of its own. §2 requires all
programming interfaces of the conformance levels of [DDS]; by §3 [DDS] is
version 1.2 (`internal/standards/cache/omg/dds-1.2.pdf`, PDF page = printed page
+ 12). The operations of the DCPS classes ([DDS] 1.2 §7.1.2, p. 25–107) and the
built-in topics (§7.1.5, p. 147–150) are therefore one item each under §2;
§7.3.7.1 requires one class per status of [DDS] ([DDS] 1.2 §7.1.4.1, p. 132–133,
IDL p. 159–161) and §7.3.5.1 one interface per policy ([DDS] 1.2 §7.1.3, p.
108–120), one item per class; §7.3.5.2 one interface per entity QoS ([DDS] 1.2
IDL, p. 167–169), one item per QoS. Operations that differ only by
`_w_timestamp`, and pairs of `set_` and `get_`, are one item. `done` is used
only where code and a test cover the requirement; a cited Java test names file
and test method as `file::method`, a Rust test names the test function as
`file::module::name`. A repository reference names the file and the symbol, no line number. A requirement on a name, a package or a signature counts as
covered by a test when the test compiles against exactly that name; a property
that compiling does not reveal (`abstract`, `final`, a parent class) needs a
test that checks it. Every statement about behaviour is read from the code and the tests. The tests
written in the second pass were only compiled on the Mac; they run on the Linux
test host. Where code implements a requirement but no test checks it, the item
is `open` and the note names the place in the code.

**Context:** DDS-Java 1.0 is spread over these parts of ZeroDDS:

- `crates/java-omgdds/java/` — the Maven module `org.zerodds:omgdds`
  (`crates/java-omgdds/java/pom.xml`, Java 17, profile `java8` for Java 8) with
  the classes under `org.omg.dds.core`, `org.omg.dds.core.policy`,
  `org.omg.dds.core.status`, `org.omg.dds.domain`, `org.omg.dds.topic`,
  `org.omg.dds.pub` and `org.omg.dds.sub`, the implementation under
  `org.zerodds.*` and the JUnit 5 tests under `src/test/java`. The binding is
  a stack of its own in Java. It does not call the C API
  (`crates/zerodds-c-api`) and loads no native library (no JNI, no FFM;
  `grep -rn "loadLibrary\|java.lang.foreign" crates/java-omgdds/java/src`
  finds nothing). Samples reach the readers in the same process through
  `org.zerodds.internal.InProcessBus`; with `-Dzerodds.rtps.enable=true` or
  `ZERODDS_RTPS=1` also through the RTPS stack in Java under
  `org.zerodds.rtps` (`DdsWireBridge.enabled`).
- `crates/idl-java/runtime/` — the annotations `org.zerodds.types.Key`, `Id`,
  `Optional`, `MustUnderstand`, `External`, `Nested`, `Shared`, `Extensibility`
  and the interface `org.omg.dds.topic.TopicType`; the POM compiles them into
  the same JAR through `build-helper-maven-plugin`.
- `runtime/java/` — the codec `org.zerodds.wire.Writer`/`Reader` that
  `org.zerodds.cdr.ReflectionTypeSupport` and the generated types use; the POM
  compiles it into the JAR as well.
- The Java code generator: `zerodds-idlc generate --lang java`
  (`tools/idlc/src/generate.rs`, `tools/idlc/src/java_support.rs`) through
  `crates/lang-rules` (`adapter/java.rs`, `emit/java.rs`). It generates the
  topic types inside a class `ZdTypes` and, per type, a
  `ZdTypeSupports.Support_<tag>` that implements
  `org.zerodds.cdr.TopicTypeSupport<T>`.

The mapping of IDL to Java by IDL4 to Java is in
[`idl4-java-1.0.en.md`](idl4-java-1.0.en.md), the architecture of the module in
[`zerodds-java-omgdds-1.0.en.md`](zerodds-java-omgdds-1.0.en.md), the XCDR2
contract `org.zerodds.cdr.TopicTypeSupport` in
[`zerodds-xcdr2-java-1.0.en.md`](zerodds-xcdr2-java-1.0.en.md), the endpoint API
in [`zerodds-endpoint-java-1.0.en.md`](zerodds-endpoint-java-1.0.en.md). This
document covers the DDS API in Java by DDS-Java 1.0 and the rules DDS-Java
itself sets for the types (§8, §9). `org.omg.dds.rpc` under `runtime/java/`
belongs to DDS-RPC 1.0 and is not covered by this document; neither is the RTPS
stack under `org.zerodds.rtps`.

By §2 the PDF, the JAR `omgdds.jar` and the OMG source files are normative, and
JAR and sources prevail in a conflict. The archive
(cover page, Annex B) is in the cache (`internal/standards/cache/omg/dds-java-psm-1.0-omgdds_src.zip`);
every file under `org/omg/dds` of the module equals the sources except
`EntityQos.java` (an added `@SuppressWarnings`); `CoreApiShapeTest` checks this on every run
against the SHA-256 of the source files (`src/test/resources/omgdds-srcjava.sha256`).

**Open items:** none.

---

## §1 Scope

### §1 PSM for DCPS only, without DLRL

**Spec:** §1, p. 11 — "This specification defines a platform-specific model
(PSM) for the OMG Data Distribution Service for Real-Time Systems (DDS). It
specifies an API only for the Data-Centric Publish-Subscribe (DCPS) portion of
that specification; it does not address the Data Local Reconstruction Layer
(DLRL)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Scope statement of the specification, no requirement that can be met. crates/java-omgdds/java/src/main/java/org/omg/dds contains the packages core, domain, pub, sub, topic and type; there is no dlrl package. DLRL belongs to DLRL 1.2 (tools/idlc/src/dlrl.rs).
### §1 Scope: the APIs of [DDS-XTypes]

**Spec:** §1, p. 11 — "In addition, it encompasses (a) the DDS APIs introduced
by [DDS-XTypes] [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to §2 (Extensible and Dynamic Types level) and §7.8. The packages org.omg.dds.type, org.omg.dds.type.dynamic, org.omg.dds.type.typeobject and org.omg.dds.type.builtin exist in the code; the requirements are in the item "§2 Stufe Extensible and Dynamic Types".
### §1 Scope: an API for QoS libraries and profiles by [DDS-CCM]

**Spec:** §1, p. 11 — "[...] and (b) an API to specifying QoS libraries and
profiles such as were specified by [DDS-CCM]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to §2 and §7.3.5.4. The API is org.omg.dds.core.QosProvider, the implementation is org.zerodds.core.qos.QosProviderImpl; the requirements are in the item "§2 XML-QoS-Profile".
### §1 Java Type Representation without a description in another language

**Spec:** §1, p. 11 — "This specification also defines a means of publishing and
subscribing Java objects with DDS-the Java Type Representation-without first
describing the types of those objects in another language, such as XML or OMG
IDL."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference to §8. Implemented by org.zerodds.cdr.ReflectionTypeSupport and TypeSupportResolver.resolve (a class without generated code is mapped by reflection); the check is in the item "§2 Mindestens eine Type Representation".

---

## §2 Conformance

### §2 PDF, JAR and source files normative, JAR and sources prevail in a conflict

**Spec:** §2, p. 11; cover page, p. 1 — "This specification consists of this
document as well as a Java jar library file and the source files that generated
it, identified on the cover page (all are normative). In the event of a conflict
between them, the latter shall prevail."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/IllegalOperationException.java` (`IllegalOperationException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantOmgApiTest.java::everyExceptionOfTheBindingIsADdsObjectWithAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantOmgApiTest.java::theExceptionsHaveTheSerialVersionUidOfTheOmgSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantOmgApiTest.java::theApiIsTheOmgSourceCodeNotCountingCommentsAndWhiteSpace`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantOmgApiTest.java::theCommentAndWhiteSpaceFilterKeepsLiteralsAndDropsComments`

**Status:** `done` — DDSException implements DDSObject (org/omg/dds/core/DDSException.java), as the OMG source does; the seven exception classes in org.zerodds.core return the default environment from getEnvironment. The serialVersionUID of six exception classes is that of the OMG source. ParticipantOmgApiTest compares every file of org/omg/dds with the OMG source without comments and white space (skipped where .agent/omg-java/srcJava is missing) and checks DDSObject and the serialVersionUID. The one allowed deviation: EntityQos carries @SuppressWarnings("unchecked") on withPolicies, because the lint run (-Xlint:all -Werror) reports the declaration as a warning otherwise. (commit `8a4d4ba74d3`)
### §2 Conformance parallels the levels of [DDS]

**Spec:** §2, p. 11; [DDS] 1.2 §2, p. 14–15 (`dds-1.2.pdf`) — "Conformance to
this specification parallels conformance to the DDS specification itself and
consists of the same conformance levels. For example, an implementation may
conform to the DDS Minimum Profile with respect to this PSM, meaning that all of
the programming interfaces identified by the DDS specification as pertaining to
that conformance level must be implemented in this PSM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase`),
`crates/java-omgdds/README.md`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aWriterQosKeepsItsImmutablePoliciesAfterEnableAndTakesTheMutableOnes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ConformanceClaimTest.java::theReadmeNamesTheConformanceLevelsOfClause2`

**Status:** `done` — The conformance levels of Clause 2 are named in `crates/java-omgdds/README.md`: the Minimum, Content-Subscription, Persistence and Ownership Profiles and Extensible and Dynamic Types, without the Object Model Profile (DLRL); `ConformanceClaimTest` checks the statement. The interfaces of these levels are evidenced one by one in the items of this document: Entity (set_qos with IMMUTABLE_POLICY and INCONSISTENT_POLICY, listener propagation, StatusCondition), DomainParticipant (ignore_*, closeContainedEntities), the assignability of types, coherent sets, liveliness and the wait operations also towards peers in other processes. What is open in single operations is its own open item. (commit `f8598d5770f`)
### §2 Object Model Profile (DLRL) excluded

**Spec:** §2, p. 11 — "The one exception to this rule is the Object Model
Profile, which includes in part the Data Local Reconstruction Layer (DLRL); DLRL
is outside of the scope of this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Delimitation by the specification. The Object Model Profile comprises DLRL (not in the code) and access_scope = GROUP of PRESENTATION; the PRESENTATION policy exists as org.omg.dds.core.policy.Presentation, its effect was not examined here.
### §2 Extensible and Dynamic Types conformance level

**Spec:** §2, p. 11 — "In addition to the conformance levels defined in the DDS
specification itself, this PSM recognizes and implements the Extensible and
Dynamic Types conformance level for DDS defined by the Extensible and Dynamic
Topic Types for DDS specification [DDS-XTypes]."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeObjectRegistry.java` (`TypeObjectRegistry`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TypeConsistencyEnforcementImpl.java` (`TypeConsistencyEnforcementImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeAssignability.java` (`TypeAssignability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.registerReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.acceptsType`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.acceptsRemoteWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.acceptsRemoteReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectToDynamic.java` (`TypeObjectToDynamic.of`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjects.java` (`TypeObjects.dynamicTypeOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::sedpCarriesTheTypeInformationAndTheTypeNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::differentTypeNamesDoNotConnect`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::anAppendedMemberIsAssignableInBothDirections`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::aMemberOfTheSameIdAndAnotherTypeIsNotAssignable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::differentExtensibilityIsNotAssignable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::aDerivedTypeIsDeclaredAsItsBase`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::anAssignableReaderTakesTheSamplesOfAnotherTypeName`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::anExactReaderDoesNotTakeThem`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::anAssignableReaderDoesNotTakeAClashingType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::aDeclaredReaderTakesADerivedType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::anAssignableReaderTakesARemoteWriterOfAnotherTypeNameByItsTypeObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::anExactReaderDoesNotTakeThatRemoteWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::aWriterTakesARemoteReaderOfAnotherTypeNameThatAllowsCoercionAndHasAnAssignableType`

**Status:** `done` — The type consistency acts when a reader and a writer are matched. EXACT_TYPE_TYPE_CONSISTENCY and EXACT_NAME_TYPE_CONSISTENCY need the same type name (`InProcessBus` keeps a channel per domain, topic name and type name). A reader with ASSIGNABLE_TYPE_CONSISTENCY takes writers of another type name on the same topic when `TypeAssignability.assignableFrom` (the rules of XTypes 1.3 §7.2.4.4: aliases resolved, same primitives, character width of strings, equal array bounds, elements "strongly assignable", enumerations with equal names and values, structures by member ids and names, key, optional and must-understand rules, for APPENDABLE and FINAL the members at the same position) finds the type of the writer assignable; with DECLARED_TYPE_CONSISTENCY it takes only writers whose type names its own as a base type (`TypeAssignability.declared`). In the same process `InProcessBus` passes the samples on over links (`Link`). Over RTPS the reader decides from the TypeObject (PID 0x0072) the writer announces (`TypeObjectToDynamic`); the writer takes a reader of another type name that announces `ALLOW_TYPE_COERCION` in SEDP (ASSIGNABLE and DECLARED are written so, EXACT as `DISALLOW`) and whose TypeObject is assignable from its type. A peer without an announced TypeObject cannot be judged and is not connected. `@ignore_literal_names` and the per-label rules of unions belong to XTypes 1.3 §7.2.4.4 and are not evaluated. (commit `f221214b649`)
### §2 XML QoS profiles: operations in full or UnsupportedOperationException

**Spec:** §2, p. 11 — "This PSM furthermore defines methods to create Entities
and to set their QoS based on the XML QoS libraries and profiles defined by the
DDS for Lightweight CCM specification. Implementations that support these XML
QoS profiles shall implement these operations fully; other implementations shall
throw java.lang.UnsupportedOperationException."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.setQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theQosOfEveryEntityKindCarriesTheValuesOfTheDocument`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theWriterAndReaderQosCarryTheValuesOfTheDocumentAndKeepTheDefaultsOfWhatItLeavesOut`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aProfileInheritsFromItsBaseAndOverridesFieldByField`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::anotherSchemeIsUnsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::entitySetQosWithAnUriAndAProfileAppliesTheQosOfThatProfile`

**Status:** `done` — QosProviderImpl reads DDS-XML 1.0 libraries (`dds-xml_qos_definitions.xsd`) from `file://` URIs: `<qos_library>`, `<qos_profile name base_name>`, seven entity QoS elements, the 22 policy elements of the schema file with its literals (`RELIABLE_RELIABILITY_QOS`), durations as `<sec>`/`<nanosec>` with `DURATION_INFINITY`, `LENGTH_UNLIMITED`, base64 for octet sequences, a partition as `<name><element>`. Inheritance through `base_name` at profile and element level; a cycle, a base defined later and a missing base are `IllegalArgumentException`. Every other URI scheme raises `UnsupportedOperationException`. Unknown elements, fields, literals and malformed values are `IllegalArgumentException`; nothing is passed over silently. `Entity.setQos(String, String)` calls the provider through `EntityBase.setQos` with the first argument as the URI. The earlier reader expected a wrapper element per entity element and Java enum names and so did not read a DDS-XML document. Tested in `QosProviderTest` (all entity kinds, inheritance, error cases, other scheme, `setQos(uri, profile)` on all six entities). (commit `8c975167054`)
### §2 At least one Type Representation of [DDS-XTypes] or §8

**Spec:** §2, p. 11 — "Finally, any conformant implementation must support at
least one of the OMG-specified Type Representations defined by [DDS-XTypes]
and/or in the Java Type Representation section of this specification (Clause
8)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`tools/idlc/src/java_support.rs` (`source`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`

**Status:** `done` — The Java Type Representation is implemented: TypeSupportResolver.resolve maps a class without generated code through ReflectionTypeSupport. pointMatchesV2 compares the encoded bytes of a bean with the wire vector (01 00 00 00 FE FF FF FF) and decodes back; theKindsOfTable81RoundTrip round-trips BigInteger, BigDecimal, enum, Set and List; singleArgFactoryReflectsTypedBean writes and reads a bean through topic, writer and reader. The IDL Type Representation reaches the generated types through tools/idlc/src/java_support.rs. (commit `8a5744821fd`)

---

## §2 Programming interfaces of [DDS] 1.2: Entity

### §2 Entity: set_qos and get_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 25–26 (`dds-1.2.pdf`), table
Entity — "abstract set_qos (qos_list: QosPolicy []): ReturnCode_t; abstract
get_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkChange`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.writerPartitionsChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.readerPartitionsChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.announceQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.announceQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.updateQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireReader.updateQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.topicQosChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::anEnabledWriterTakesNewValuesOfTheChangeablePolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::anEnabledWriterRefusesAChangeOfAnImmutablePolicyAndKeepsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::settingTheSameValueOfAnImmutablePolicyIsNoChange`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::anEnabledReaderTakesATimeBasedFilterAndRefusesAnotherHistory`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aDeadlineShorterThanTheMinimumSeparationIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::anEnabledTopicTakesTopicDataAndRefusesAnotherReliability`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aPublisherTakesAPartitionAndGroupDataAndRefusesAnotherPresentation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aSubscriberTakesAPartitionAndRefusesAnotherPresentation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aParticipantAndTheFactoryTakeNewUserDataAndEntityFactory`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::beforeEnableAnyPolicyChangesAndAfterEnableTheImmutableOnesDoNot`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::aPublisherThatChangesItsPartitionEndsAndStartsMatches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::aSubscriberThatChangesItsPartitionEndsAndStartsMatches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::aChangeThatKeepsTheOverlapIsNoMatchEvent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosBuiltinUpdateTest.java::aChangedQosOfAWriterAndOfItsPublisherIsANewSampleOnThePublicationTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosBuiltinUpdateTest.java::aChangedQosOfAReaderAndOfItsSubscriberIsANewSampleOnTheSubscriptionTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::aChangedQosOfAWriterReachesTheReaderOfAnotherProcessOverSedp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinQosAnnouncementTest.java::aChangedTopicQosIsANewSampleOfTheTopicInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aHistoryDepthAboveMaxSamplesPerInstanceIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theBuiltinReaderSeesTheUserDataChangeOfAnotherParticipant`

**Status:** `done` — `getQos` exists on all six entities. `setQos` checks through `org.zerodds.core.qos.QosRules.checkChange` first the consistency of the policies (`InconsistentPolicyException`, also when an immutable policy changes at the same time) and, after `enable`, the policies whose column "Changeable" in DDS 1.4 §2.2.3 is "No" (DURABILITY, DURABILITY_SERVICE, PRESENTATION, OWNERSHIP, LIVELINESS, RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS, DATA_REPRESENTATION, TYPE_CONSISTENCY_ENFORCEMENT; `ImmutablePolicyException`); an equal value is no change; with both exceptions the QoS stays as it was. That holds for DataWriter, DataReader, Topic, Publisher, Subscriber, DomainParticipant and DomainParticipantFactory. After a change of DEADLINE or LIVELINESS the periods restart; a changed PARTITION makes writers and readers match again. The changed QoS of a writer, reader, publisher, subscriber, topic or participant is a new sample of the same instance in "DCPSPublication", "DCPSSubscription", "DCPSTopic" and "DCPSParticipant", and writers and readers connected over RTPS announce it again in SEDP (`WireWriter.updateQos`, `WireReader.updateQos`); the peer then matches again. The SEDP writers keep only the last announcement of each endpoint and peer (KEEP_LAST 1), so repeated announcements do not fill the history. (commit `4664a24a4e3`)
### §2 Entity: set_listener and get_listener

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity —
"abstract set_listener (a_listener: Listener, mask: StatusKind []):
ReturnCode_t; abstract get_listener: Listener"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.setListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.listenerWants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/ListenerUp.java` (`ListenerUp`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.listenerFor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::aWriterListenerThatDoesNotTakeAStatusLeavesItToThePublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::anEmptyMaskTakesNoStatusAndANullMaskTakesAll`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerAndAPublisherWithoutOnePassItToTheParticipantListener`

**Status:** `done` — The listeners of DomainParticipant, Publisher, Subscriber and Topic are called. A status goes to the listener of the entity when that listener has the status in its mask (null = all, empty = none), else to the one of the containing entity: DataWriter to Publisher and DomainParticipant, DataReader to Subscriber and DomainParticipant, Topic to DomainParticipant (ListenerUp, EntityBase.ownListener, DataWriterImpl.listenerFor, TopicImpl.listenerFor). Tests: propagation from the writer to publisher and participant (ListenerPropagationTest), status mask of a writer listener and the empty and null masks (ParticipantSubscriberTest), INCONSISTENT_TOPIC to topic and participant listener (TopicOperationsTest), OFFERED_INCOMPATIBLE_QOS to publisher and participant listener (PublisherOperationsTest). (commit `9426325d8b2`)
### §2 Entity: status mask and listener at creation

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26, §7.1.2.2.1, p. 34–35
(`dds-1.2.pdf`) — "set_listener (a_listener: Listener, mask: StatusKind [])";
"create_publisher (qos_list: QosPolicy [], a_listener: PublisherListener, mask:
StatusKind []): Publisher"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.createParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterListenerSetAtCreationSeesTheMatchOfEnable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createTopicWithAListenerAndAMaskKeepsThem`

**Status:** `done` — The create* operations with listener and status set set the listener before enable: DomainParticipantImpl.newPublisher, newSubscriber and newTopic, PublisherImpl.createDataWriter, SubscriberImpl.createDataReader; DomainParticipantFactoryImpl.createParticipant sets it before the participant is registered. A SUBSCRIPTION_MATCHED or PUBLICATION_MATCHED of an existing counterpart thus reaches the listener set at creation. Tests: writer and reader with listener and an existing counterpart (ListenerPropagationTest), publisher and subscriber with listener and a writer or reader created later, participant with listener and mask, topic with listener (ParticipantEntityTest). (commit `deff69a0210`)
### §2 Entity: enable

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity —
"enable: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.enableIfFactorySays`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.enableBuiltin`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aFactoryThatDoesNotAutoEnableCreatesADisabledParticipantWhoseChildrenStayDisabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aWriterOfADisabledPublisherIsDisabledAndEnablingThePublisherEnablesIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aPublisherThatDoesNotAutoEnableLeavesItsWritersToEnable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aReaderOfADisabledSubscriberIsDisabledAndEnablingTheSubscriberEnablesIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aSubscriberThatDoesNotAutoEnableLeavesItsReadersToEnable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::concurrentEnableRegistersTheEndpointOnce`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNotEnabledWriterRefusesToWrite`

**Status:** `done` — enable follows DDS 1.4 §2.2.2.1.1.7. An entity whose factory is not enabled is created disabled, and enable on it is PreconditionNotMetException (PublisherImpl.enable, SubscriberImpl.enable, DataWriterImpl.enable, DataReaderImpl.enable). DomainParticipantImpl.enable, PublisherImpl.enable and SubscriberImpl.enable enable the children created so far when ENTITY_FACTORY autoenable_created_entities holds; createDataWriter and createDataReader enable only when the publisher/subscriber is enabled and its autoenable_created_entities holds. The built-in publisher and subscriber are enabled with enableBuiltin. Tests: factory with autoenable = false (participant and children disabled, PreconditionNotMetException, enabling through enable), writer of a disabled publisher, publisher with autoenable = false, the same cases for reader and subscriber (ParticipantEntityTest). (commit `deff69a0210`)
### §2 Entity: get_statuscondition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity —
"get_statuscondition: StatusCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::theStatusChangesOfAWriterListThePendingStatusesUntilTheyAreRead`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::anInconsistentTopicTriggersItsStatusCondition`

**Status:** `done` — getStatusCondition returns a StatusConditionImpl whose trigger value follows the status kinds set by triggerStatus and cleared by reading the respective status. DataReaderImpl sets DATA_AVAILABLE, SAMPLE_REJECTED, SAMPLE_LOST, SUBSCRIPTION_MATCHED, REQUESTED_INCOMPATIBLE_QOS and REQUESTED_DEADLINE_MISSED through triggerStatus, DataWriterImpl PUBLICATION_MATCHED, OFFERED_INCOMPATIBLE_QOS, OFFERED_DEADLINE_MISSED and LIVELINESS_LOST, TopicImpl INCONSISTENT_TOPIC. Tests: wait set with the status condition of a reader (WaitSetTest), reset by reading (ReaderStatusTest), status condition of a writer and of a topic (WriterStatusTest, TopicOperationsTest). (commit `e72453fce1f`)
### §2 Entity: get_status_changes

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity —
"get_status_changes: StatusKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.triggerStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::theStatusChangesOfAWriterListThePendingStatusesUntilTheyAreRead`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`

**Status:** `done` — EntityBase.getStatusChanges returns the set of triggered status kinds without clearing it; a status is reset when the respective status is read (get<Status>Status, read and take for DATA_AVAILABLE) (EntityBase.clearStatus). Tests: reader after a match, reading the status and take (ReaderStatusTest), writer after a match and reading (WriterStatusTest), topic after INCONSISTENT_TOPIC (TopicOperationsTest). Deadline status: WriterStatusTest::aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt waits with getStatusChanges for OfferedDeadlineMissedStatus. (commit `5920d2fbe32`)
### §2 Entity: get_instance_handle

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.1, p. 26 (`dds-1.2.pdf`), table Entity —
"get_instance_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/Values.java` (`Values.ofUuid`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::containsEntityWalksTheContainment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredTopicDataCarriesTheTopicQos`

**Status:** `done` — Each entity holds an immutable handle built from a UUID in the constructor (EntityBase). containsEntityWalksTheContainment passes the handles of publisher, writer, subscriber, reader and topic to containsEntity (all true, nil handle false); the key in the data of the built-in participant topic equals the getInstanceHandle of the other participant, and getDiscoveredTopicData finds a topic by its handle. Uniqueness follows from the UUID and is not checked directly. (commit `e72453fce1f`)

---

## §2 Programming interfaces of [DDS] 1.2: DomainParticipantFactory

### §2 DomainParticipantFactory: create_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "create_participant (domain_id: DomainId_t, qos_list:
QosPolicy [], a_listener: DomainParticipantListener, mask: StatusKind []):
DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.createParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.createParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::entityFactoryDecidesWhetherWhatIsCreatedIsEnabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant`

**Status:** `done` — createParticipant(int, DomainParticipantQos, DomainParticipantListener, Collection) sets listener and status set before the participant is registered with the factory (DomainParticipantFactoryImpl.createParticipant). ParticipantEntityTest::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant calls the form with listener and mask {PUBLICATION_MATCHED} and checks that PUBLICATION_MATCHED of a writer without a listener of its own reaches the participant listener and SUBSCRIPTION_MATCHED, which is not in the mask, does not. (commit `cf0d29f7efb`)
### §2 DomainParticipantFactory: a new participant per call

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2.1, p. 46 (`dds-1.2.pdf`) — "This
operation creates a new DomainParticipant object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.createParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.register`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantFactoryTest.java::createParticipantCreatesANewParticipantEachCall`

**Status:** `done` — createParticipant creates a new DomainParticipantImpl on every call and adds it to a list per domain (DomainParticipantFactoryImpl.register). The test creates two participants in the same domain and asserts assertNotSame; the reference to computeIfAbsent in the previous item is outdated. (commit `cf0d29f7efb`)
### §2 DomainParticipantFactory: delete_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45, §7.1.2.2.2.2, p. 46
(`dds-1.2.pdf`) — "delete_participant (a_participant: DomainParticipant):
ReturnCode_t"; "This operation can only be invoked if all domain entities
belonging to the participant have already been deleted. Otherwise the error
PRECONDITION_NOT_MET is returned."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.forget`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService.clearDomain`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::transientIsClearedWhenTheParticipantCloses`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::theDataOfADomainSurvivesTheCloseOfAParticipantWhileAnotherOneIsOpen`

**Status:** `done` — DomainParticipantImpl.close closes the contained entities (§7.2.3), removes the participant from the factory and from the table of all participants of the process and clears the TRANSIENT data of the domain only if no participant of the domain is open afterwards (DomainParticipantFactoryImpl.participantsInDomain, DurabilityService.clearDomain). DomainParticipantFactory has no deleteParticipant operation (OMG API). ParticipantEntityTest::theDataOfADomainSurvivesTheCloseOfAParticipantWhileAnotherOneIsOpen writes TRANSIENT data, closes the writer and one of two participants (data stays), checks lookupParticipant, closes the second (lookupParticipant returns null, a new reader gets nothing). (commit `deff69a0210`)
### §2 DomainParticipantFactory: get_instance

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45, §7.1.2.2.2.3, p. 46
(`dds-1.2.pdf`) — "(static) get_instance: DomainParticipantFactory"; "it will
return the same DomainParticipantFactory instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getInstanceRejectsNullEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::threadsThatAskForTheFactoryOfANewEnvironmentTogetherGetTheSameOne`

**Status:** `done` — ZeroSPI.getParticipantFactory creates the factory with a volatile field and under a lock (ServiceEnvironmentImpl). ParticipantFactoryEnvironmentTest::threadsThatAskForTheFactoryOfANewEnvironmentTogetherGetTheSameOne has eight threads request the factory of a new environment at the same time (CyclicBarrier), 25 rounds, and expects the same object; the earlier tests check assertSame for two calls, assertNotSame between two environments and the NullPointerException for null. (commit `9579b04127c`)
### §2 DomainParticipantFactory: lookup_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "lookup_participant (domain_id: DomainId_t):
DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.lookupParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.lookupParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupParticipantFindsOneOfTheDomain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::theDataOfADomainSurvivesTheCloseOfAParticipantWhileAnotherOneIsOpen`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable`

**Status:** `done` — lookupParticipant(int) returns the first participant of the domain from the factory table, otherwise null; after close the participant is removed from the table. Tests: DomainParticipantOperationsTest::lookupParticipantFindsOneOfTheDomain (assertSame, another domain null); ParticipantEntityTest::theDataOfADomainSurvivesTheCloseOfAParticipantWhileAnotherOneIsOpen (null after closing); ParticipantFactoryEnvironmentTest::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable (the factory of another environment does not find the participant). (commit `cf0d29f7efb`)
### §2 DomainParticipantFactory: set_default_participant_qos and get_default_participant_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "set_default_participant_qos (qos_list: QosPolicy
[]): ReturnCode_t; get_default_participant_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setDefaultParticipantQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.setDefaultParticipantQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aDefaultQosWhosePoliciesAreNotConsistentIsRefusedAndKeepsTheOldOne`

**Status:** `done` — getDefaultParticipantQos returns the set value; a participant created afterwards with createParticipant(int) has exactly this QoS (assertEquals on getQos). DDS 1.4 requires a consistency check for set_default_participant_qos; for the policies of a DomainParticipantQos (USER_DATA, ENTITY_FACTORY) §2.2.3 names no rule that makes two values inconsistent, so the check is empty here. The default operations of publisher, subscriber, topic and writer check with QosRules.checkConsistent (ParticipantEntityTest, PublisherOperationsTest). (commit `cf0d29f7efb`)
### §2 DomainParticipantFactory: get_qos and set_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.2, p. 45 (`dds-1.2.pdf`), table
DomainParticipantFactory — "get_qos (out: qos_list: QosPolicy []): ReturnCode_t;
set_qos (qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aFactoryThatDoesNotAutoEnableCreatesADisabledParticipantWhoseChildrenStayDisabled`

**Status:** `done` — getQos and setQos of the factory exist; DomainParticipantFactoryImpl.autoEnable evaluates ENTITY_FACTORY. ParticipantEntityTest::aFactoryThatDoesNotAutoEnableCreatesADisabledParticipantWhoseChildrenStayDisabled sets a factory QoS with autoEnableCreatedEntities = false, creates a participant, expects isEnabled() = false, the refusal of enable on its children with PreconditionNotMetException and the enabled children after enable(); the factory QoS is reset in finally. (commit `cf0d29f7efb`)

---

## §2 Programming interfaces of [DDS] 1.2: DomainParticipant

### §2 DomainParticipant: create_publisher and delete_publisher

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 34–35 (`dds-1.2.pdf`), table
DomainParticipant — "create_publisher (qos_list: QosPolicy [], a_listener:
PublisherListener, mask: StatusKind []): Publisher; delete_publisher
(a_publisher: Publisher): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedPublisherAndSubscriberLeaveTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::publisherCloseClosesItsWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater`

**Status:** `done` — createPublisher() has the forms without argument, with PublisherQos and with listener and status set; the listener is set before enable (DomainParticipantImpl.newPublisher). A closed publisher leaves the participant's list through the close hook (containsEntity returns false afterwards). Tests: PublisherQos of the call and default (DomainParticipantOperationsTest, ParticipantEntityTest), overload with listener (ParticipantEntityTest::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater), containsEntity after close (CloseCleanupTest::aClosedPublisherAndSubscriberLeaveTheParticipant), closing the writers (CloseCascadeTest). (commit `b594326a2a0`)
### §2 DomainParticipant: create_subscriber and delete_subscriber

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_subscriber (qos_list: QosPolicy [], a_listener:
SubscriberListener, mask: StatusKind []): Subscriber; delete_subscriber
(a_subscriber: Subscriber): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::closingASubscriberClosesItsReadersAndLeavesTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedPublisherAndSubscriberLeaveTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aClosedEntityRefusesToCreateContainedEntities`

**Status:** `done` — createSubscriber() has the forms without argument, with SubscriberQos and with listener and status set; the listener is set before enable (DomainParticipantImpl.newSubscriber). SubscriberImpl.close closes the DataReaders, and the subscriber leaves the participant's list. Tests: ParticipantSubscriberTest::closingASubscriberClosesItsReadersAndLeavesTheParticipant (reader closed, read throws AlreadyClosedException, containsEntity false, createDataReader throws AlreadyClosedException), ParticipantEntityTest::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater. (commit `b594326a2a0`)
### §2 DomainParticipant: create_topic and delete_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_topic (topic_name: string, type_name: string,
qos_list: QosPolicy [], a_listener: TopicListener, mask: StatusKind []): Topic;
delete_topic (a_topic: Topic): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aClosedTopicIsGoneFromLookupFindAndDiscovery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createTopicWithAListenerAndAMaskKeepsThem`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aTopicInUseCannotBeClosedUntilItsWritersAndReadersAre`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`

**Status:** `done` — createTopic with Class, TypeSupport and DynamicType, with and without TopicQos, listener and status set; the listener is set before enable (DomainParticipantImpl.newTopic). TopicImpl.close throws PreconditionNotMetException while a DataWriter, DataReader, ContentFilteredTopic or MultiTopic uses the topic (TopicImpl.use/release, counted also by ContentFilteredTopicImpl and MultiTopicImpl), and the topic leaves the participant's list and DCPSTopic through the close hook. Tests: TypeSupport forms (TopicOperationsTest::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass, ::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName), a TopicQos passed (ParticipantEntityTest::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext), listener (ParticipantEntityTest::createTopicWithAListenerAndAMaskKeepsThem), close with users and afterwards (CloseCleanupTest, TopicOperationsTest). (commit `9579b04127c`)
### §2 DomainParticipant: create_contentfilteredtopic and delete_contentfilteredtopic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_contentfilteredtopic (name: string, related_topic:
Topic, filter_expression: string, expression_parameters: string []):
ContentFilteredTopic; delete_contentfilteredtopic (a_contentfilteredtopic:
ContentFilteredTopic): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.setExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/ContentFilter.java` (`ContentFilter.compile`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::setExpressionParametersFiltersWithTheNewParametersFromThenOn`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::expressionParameters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::aMalformedExpressionIsRejected`

**Status:** `done` — ContentFilteredTopicImpl.setExpressionParameters compiles the expression again with ContentFilter.compile and the new parameters; a parameter set the expression cannot take (a missing %n) is refused with IllegalArgumentException and the old one stays. close throws PreconditionNotMetException while a reader reads the topic, releases the related topic and removes the entry from the participant's list (lookupTopicDescription returns null); a closed topic is AlreadyClosedException for new readers. Tests: TopicOperationsTest::setExpressionParametersFiltersWithTheNewParametersFromThenOn, ::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant, ::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds, ::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant (lookupTopicDescription). (commit `9579b04127c`)
### §2 DomainParticipant: create_multitopic and delete_multitopic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35 (`dds-1.2.pdf`), table
DomainParticipant — "create_multitopic (name: string, type_name: string,
subscription_expression: string, expression_parameters: string []): MultiTopic;
delete_multitopic (a_multitopic: MultiTopic): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createMultiTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createMultiTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicEngine.java` (`MultiTopicEngine`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::deleteContainedEntitiesRemovesEveryTopicDescriptionFromTheLookups`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::aReaderOnTheMultiTopicReceivesTheMatchingSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::theAnnexBExampleJoinsProjectsRenamesAndFilters`

**Status:** `done` — MultiTopicImpl.close throws PreconditionNotMetException while a reader reads the MultiTopic, releases the topics of the FROM clause (MultiTopicImpl.bind holds them) and removes the entry from the participant's list; a closed MultiTopic is AlreadyClosedException for new readers. Tests: TopicOperationsTest::closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause, ::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds, ::deleteContainedEntitiesRemovesEveryTopicDescriptionFromTheLookups. (commit `9579b04127c`)
### §2 DomainParticipant: find_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 35–36 (`dds-1.2.pdf`), table
DomainParticipant — "find_topic (topic_name: string, timeout: Duration_t):
Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.findTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.findTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.awaitTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::findTopicWaitsForATopicAnotherThreadCreatesAndTimesOutOtherwise`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::findTopicReturnsTheTopicAndTimesOutOtherwise`

**Status:** `done` — findTopic(String, Duration) and findTopic(String, long, TimeUnit) return a topic the participant created, otherwise wait up to the timeout for one that another thread creates, and throw TimeoutException. DDS 1.4 §2.2.2.2.1.11 leaves it to the implementation to make remotely created topics available; there are none here. Tests: TopicOperationsTest::findTopicWaitsForATopicAnotherThreadCreatesAndTimesOutOtherwise (a second thread creates the topic after 200 ms, findTopic with a 30 s bound returns it; an unknown name ends in the TimeoutException after 300 ms, minimum duration 290 ms), DomainParticipantOperationsTest::findTopicReturnsTheTopicAndTimesOutOtherwise. (commit `8a5744821fd`)
### §2 DomainParticipant: lookup_topicdescription

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "lookup_topicdescription (name: string): TopicDescription"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.lookupTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.lookupTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.allTopicDescriptions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionFindsOnlyWhatWasCreated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicIsATopicDescriptionOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theParticipantCreatesTheFourBuiltinTopics`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant`

**Status:** `done` — lookupTopicDescription searches the topics, ContentFilteredTopics and MultiTopics of the participant; closed descriptions are removed from the lists. Tests: topic with assertSame, name and type name and unknown name null (DomainParticipantOperationsTest), MultiTopic (MultiTopicTest), built-in topics (BuiltinTopicsTest), ContentFilteredTopic and null after close (TopicOperationsTest::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant, ::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant). (commit `8a5744821fd`)
### §2 DomainParticipant: get_builtin_subscriber

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_builtin_subscriber: Subscriber"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getBuiltinSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getBuiltinSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/Ignores.java` (`Ignores.pairIgnored`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theBuiltinSubscriberHoldsOneReaderPerBuiltinTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::aBuiltinReaderHasTheEntitiesThatExistedBeforeAndNoneOfItsOwnParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::theBuiltinReaderOfAParticipantGivesNoDataAboutItsOwnEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::getBuiltinSubscriberIsTheSameObjectEveryTime`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`

**Status:** `done` — getBuiltinSubscriber creates once an enabled subscriber with one DataReader each for DCPSParticipant, DCPSPublication, DCPSSubscription and DCPSTopic (DURABILITY TRANSIENT_LOCAL, RELIABILITY RELIABLE, HISTORY KEEP_LAST 1, DDS 1.4 §2.2.5); lookupDataReader(topic name) finds them. The built-in writers offer TRANSIENT_LOCAL (Builtin.writer), so a reader created later gets the entities already known. The built-in readers give no data about entities of their own participant (Ignores.pairIgnored, §2.2.5) and are not announced on DCPSSubscription themselves. Tests: ParticipantDiscoveryTest::theBuiltinSubscriberHoldsOneReaderPerBuiltinTopic, ::aBuiltinReaderHasTheEntitiesThatExistedBeforeAndNoneOfItsOwnParticipant, ParticipantIgnoreTest::theBuiltinReaderOfAParticipantGivesNoDataAboutItsOwnEntities. (commit `deff69a0210`)
### §2 DomainParticipant: ignore_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_participant (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignoreParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignoreParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/Ignores.java` (`Ignores`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.rematchIgnored`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreParticipantAfterTheMatchEndsItAndStopsTheData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::anIgnoredParticipantDoesNotIgnoreTheOtherWay`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::ignoreParticipantEndsTheMatchWithItsReadersAndKeepsItEnded`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::discoveryListsTheParticipantsOfTheDomainAndIgnoreHidesOne`

**Status:** `done` — ignoreParticipant adds the handle to the ignore sets of the participant (Ignores.of(...).ignoreParticipant) and matches the pairs again (InProcessBus.rematchIgnored): no endpoint of the ignored participant is paired with an endpoint of this participant, existing pairs end, and the data does not arrive. For the RTPS path RtpsParticipant.ignoreParticipant adds the GUID prefix and removes matches; a later SEDP announcement does not pair again. getDiscoveredParticipants leaves the handle out, getDiscoveredParticipantData refuses it with PreconditionNotMetException. Tests: ParticipantIgnoreTest::ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway, ::ignoreParticipantAfterTheMatchEndsItAndStopsTheData, ::anIgnoredParticipantDoesNotIgnoreTheOtherWay; RTPS: WriterWireIgnoreTest::ignoreParticipantEndsTheMatchWithItsReadersAndKeepsItEnded, ParticipantWireDiscoveryTest::aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored. The RTPS participant of a process per domain is shared by all participants of the domain, an ignore there holds for all of them. (commit `deff69a0210`)
### §2 DomainParticipant: ignore_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_topic (handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/Ignores.java` (`Ignores.Rules.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::ignoreTopicEndsTheMatchWithTheReadersOfTheTopic`

**Status:** `done` — ignoreTopic resolves the topic handle to a topic name (own topics, topics of other participants of the domain, topics known from SEDP) and adds the name: no publication and no subscription of the topic is paired with an endpoint of this participant, existing pairs end. The topic is missing from getDiscoveredTopics, getDiscoveredTopicData refuses the handle with PreconditionNotMetException. RTPS: RtpsParticipant.ignoreTopic. Tests: ParticipantIgnoreTest::ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant; RTPS: WriterWireIgnoreTest::ignoreTopicEndsTheMatchWithTheReadersOfTheTopic. A handle that belongs to no known topic at the time of the call is only remembered and does not act on matching. (commit `deff69a0210`)
### §2 DomainParticipant: ignore_publication

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_publication (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignorePublication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignorePublication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreEndpoint`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignorePublicationStopsTheDataOfOneWriterOnly`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::ignoreEndpointEndsTheMatchWithOneReader`

**Status:** `done` — ignorePublication adds the writer handle: no sample of this writer reaches a reader of this participant, and the writer is paired with no reader of this participant (an existing pair ends). RTPS: RtpsParticipant.ignoreEndpoint with the endpoint GUID. Tests: ParticipantIgnoreTest::ignorePublicationStopsTheDataOfOneWriterOnly (a second writer of the same participant keeps delivering); RTPS: WriterWireIgnoreTest::ignoreEndpointEndsTheMatchWithOneReader. (commit `deff69a0210`)
### §2 DomainParticipant: ignore_subscription

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "ignore_subscription (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignoreSubscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignoreSubscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreEndpoint`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreSubscriptionStopsTheDataToOneReaderOnly`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::ignoreEndpointEndsTheMatchWithOneReader`

**Status:** `done` — ignoreSubscription adds the reader handle: no writer of this participant pairs with this reader or sends it data (an existing pair ends, getMatchedSubscriptions and PUBLICATION_MATCHED follow). RTPS: RtpsParticipant.ignoreEndpoint. Tests: ParticipantIgnoreTest::ignoreSubscriptionStopsTheDataToOneReaderOnly (a second reader keeps receiving); RTPS: WriterWireIgnoreTest::ignoreEndpointEndsTheMatchWithOneReader. (commit `deff69a0210`)
### §2 DomainParticipant: get_domain_id

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_domain_id: DomainId_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDomainId`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDomainId`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::getDomainIdIsTheDomainTheParticipantWasCreatedIn`

**Status:** `done` — getDomainId() returns the domain the participant was created with (field domainId in DomainParticipantImpl). ParticipantEntityTest::getDomainIdIsTheDomainTheParticipantWasCreatedIn creates participants in the domains 1200 and 1201 and compares. (commit `cf0d29f7efb`)
### §2 DomainParticipant: delete_contained_entities

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::deleteContainedEntitiesRemovesEveryTopicDescriptionFromTheLookups`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAParticipantWithFilteredTopicsMultiTopicsAndReadersDoesNotFail`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::closeContainedEntitiesClosesThemWithoutClosingTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`

**Status:** `done` — closeContainedEntities closes publishers, subscribers, ContentFilteredTopics, MultiTopics, topics and the built-in subscriber and leaves the participant open; the closed entities leave the participant's lists through close hooks. Tests: TopicOperationsTest::deleteContainedEntitiesRemovesEveryTopicDescriptionFromTheLookups (lookupTopicDescription returns null afterwards, getDiscoveredTopics empty, containsEntity false), DomainParticipantOperationsTest::closeContainedEntitiesClosesThemWithoutClosingTheParticipant, CloseCascadeTest::participantCloseClosesContainedEntities, TopicOperationsTest::closingAParticipantWithFilteredTopicsMultiTopicsAndReadersDoesNotFail. (commit `9579b04127c`)
### §2 DomainParticipant: assert_liveliness

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertedByParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.livelinessAsserted`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::assertLivelinessOfTheParticipantRenewsTheWritersWithManualByParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aWriteOfOneWriterAssertsTheLivelinessOfTheParticipantsOtherManualWriters`

**Status:** `done` — DomainParticipantImpl.assertLiveliness renews the lease of the participant's writers with LIVELINESS MANUAL_BY_PARTICIPANT (DataWriterImpl.assertedByParticipant); a write or assertLiveliness of a writer of this kind renews it for the others (PublisherImpl.writerAsserted, DomainParticipantImpl.livelinessAsserted), writers with MANUAL_BY_TOPIC stay untouched. Tests with a lease of 1500 ms and a duration of 3 s: WriterStatusTest::assertLivelinessOfTheParticipantRenewsTheWritersWithManualByParticipant (the MANUAL_BY_PARTICIPANT writer does not lose the lease, the MANUAL_BY_TOPIC writer does), ::aWriteOfOneWriterAssertsTheLivelinessOfTheParticipantsOtherManualWriters. (commit `deff69a0210`)
### §2 DomainParticipant: set_default_publisher_qos and get_default_publisher_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_publisher_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_publisher_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createPublisher`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`

**Status:** `done` — setDefaultPublisherQos checks consistency (QosRules.checkConsistent, INCONSISTENT_POLICY); the policies of a PublisherQos (GROUP_DATA, PARTITION, PRESENTATION, ENTITY_FACTORY) have no rule against each other in DDS 1.4 §2.2.3. The test sets a PublisherQos with PARTITION "mine", reads it back and checks that createPublisher().getQos() has it. (commit `cf0d29f7efb`)
### §2 DomainParticipant: set_default_subscriber_qos and get_default_subscriber_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_subscriber_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_subscriber_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultSubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultSubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createSubscriber`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`

**Status:** `done` — DomainParticipantImpl.createSubscriber() takes defaultSubscriberQos. ParticipantEntityTest::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext sets a SubscriberQos with GROUP_DATA, reads it back and checks that createSubscriber().getQos() equals the set QoS. (commit `cf0d29f7efb`)
### §2 DomainParticipant: set_default_topic_qos and get_default_topic_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "set_default_topic_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_topic_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::aDefaultQosWhosePoliciesAreNotConsistentIsRefusedAndKeepsTheOldOne`

**Status:** `done` — createTopic without QoS takes defaultTopicQos; setDefaultTopicQos checks consistency (HISTORY depth against RESOURCE_LIMITS max_samples_per_instance, max_samples against max_samples_per_instance). ParticipantEntityTest::theDefaultQosOfSubscriberAndTopicIsTheQosOfWhatIsCreatedNext sets a TopicQos with RELIABLE and checks that createTopic(...).getQos() has it and that a QoS passed applies; ::aDefaultQosWhosePoliciesAreNotConsistentIsRefusedAndKeepsTheOldOne checks INCONSISTENT_POLICY for both rules and that the old default stays. (commit `cf0d29f7efb`)
### §2 DomainParticipant: get_discovered_participants

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_participants (inout: participant_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredParticipants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.participantsIn`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin.attachRemote`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::aClosedParticipantIsNotDiscoveredAnyMore`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::discoveryListsTheParticipantsOfTheDomainAndIgnoreHidesOne`

**Status:** `done` — getDiscoveredParticipants returns the handles of the participants of the domain in this process (of all environments, this one included) and of those reported by SPDP (RtpsParticipant, handle from participantGuid), without the ignored ones; a closed participant is missing. Tests: ParticipantDiscoveryTest::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData, ::aClosedParticipantIsNotDiscoveredAnyMore, ParticipantFactoryEnvironmentTest::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable (participants of two environments), ParticipantWireDiscoveryTest::aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored (SPDP), ParticipantIgnoreTest. (commit `deff69a0210`)
### §2 DomainParticipant: get_discovered_participant_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 36–37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_participant_data (inout: participant_data:
ParticipantBuiltinTopicData, participant_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredParticipantData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipantData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredParticipantDataCarriesTheUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored`

**Status:** `done` — getDiscoveredParticipantData returns key and USER_DATA of a participant of the domain from the process or from SPDP; an ignored handle and an unknown handle are PreconditionNotMetException. Tests: ParticipantDiscoveryTest::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData (non-empty USER_DATA, participant of another domain), ParticipantIgnoreTest::ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway (ignored handle), BuiltinTopicsTest::getDiscoveredParticipantDataCarriesTheUserData (nil handle), ParticipantWireDiscoveryTest (key of a remote participant). (commit `deff69a0210`)
### §2 DomainParticipant: get_discovered_topics

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_topics (inout: topic_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredTopics`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredTopics`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theTopicsOfOtherParticipantsOfTheDomainAreDiscovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::aTopicOfAnIgnoredParticipantIsNotDiscovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredTopicDataCarriesTheTopicQos`

**Status:** `done` — getDiscoveredTopics returns the handles of the topics of all participants of the domain in this process and of the topics known from SEDP (handle from name and type name), without built-in topics, ignored topics and topics of ignored participants. Tests: ParticipantDiscoveryTest::theTopicsOfOtherParticipantsOfTheDomainAreDiscovered, ::aTopicOfAnIgnoredParticipantIsNotDiscovered, ParticipantIgnoreTest::ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant, ParticipantWireDiscoveryTest::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent, BuiltinTopicsTest::getDiscoveredTopicDataCarriesTheTopicQos. (commit `deff69a0210`)
### §2 DomainParticipant: get_discovered_topic_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_discovered_topic_data (inout: topic_data:
TopicBuiltinTopicData, topic_handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin.topicDataOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theTopicsOfOtherParticipantsOfTheDomainAreDiscovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::aTopicOfAnIgnoredParticipantIsNotDiscovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredTopicDataCarriesTheTopicQos`

**Status:** `done` — getDiscoveredTopicData looks the handle up among the topics of all participants of the domain in the process and among the topics known from SEDP; an ignored or unknown handle is PreconditionNotMetException. Tests: ParticipantDiscoveryTest::theTopicsOfOtherParticipantsOfTheDomainAreDiscovered (name, type name, RELIABILITY equal to the topic's value, exception for a participant handle, disappearing after close), ::aTopicOfAnIgnoredParticipantIsNotDiscovered, ParticipantIgnoreTest (ignored topic), ParticipantWireDiscoveryTest (topic from SEDP), BuiltinTopicsTest::getDiscoveredTopicDataCarriesTheTopicQos. (commit `deff69a0210`)
### §2 DomainParticipant: contains_entity

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "contains_entity (a_handle: InstanceHandle_t): boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.containsEntity`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.containsEntity`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::containsEntityWalksTheContainment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theBuiltinSubscriberHoldsOneReaderPerBuiltinTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedPublisherAndSubscriberLeaveTheParticipant`

**Status:** `done` — containsEntity searches publishers, subscribers, their DataWriters and DataReaders, topics, the built-in subscriber and its readers recursively; closed entities are no longer contained. Tests: DomainParticipantOperationsTest::containsEntityWalksTheContainment (handles of publisher, writer, subscriber, reader, topic true, nil handle false), ParticipantDiscoveryTest::theBuiltinSubscriberHoldsOneReaderPerBuiltinTopic (built-in readers), CloseCleanupTest::aClosedPublisherAndSubscriberLeaveTheParticipant (false after close). (commit `deff69a0210`)
### §2 DomainParticipant: get_current_time

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.1, p. 37 (`dds-1.2.pdf`), table
DomainParticipant — "get_current_time (inout: current_time: Time_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::getCurrentTimeFillsTheContainerOrReturnsANewOne`

**Status:** `done` — getCurrentTime() returns a new immutable Time, getCurrentTime(ModifiableTime) fills the container and returns it (assertSame); both use System.currentTimeMillis(), the source with which DataWriterImpl stamps samples. The test checks both forms with a value above 0 seconds. (commit `8a5744821fd`)
### §2 DomainParticipantListener

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.2.3, p. 47–48 (`dds-1.2.pdf`) — "The
DomainParticipantListener interface extends all other Listener interfaces and
has no additional operation beyond the ones defined by the more general
listeners."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.setListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerAndAPublisherWithoutOnePassItToTheParticipantListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant`

**Status:** `done` — DataWriterImpl and DataReaderImpl pass a status without a listener of their own (or without the status in its mask) on to the publisher or subscriber and then to the DomainParticipant (ListenerUp); the entity's listener gets it first. TopicImpl passes INCONSISTENT_TOPIC to the DomainParticipant. DomainParticipantListener extends PublisherListener and SubscriberListener and carries onInconsistentTopic. Tests: ListenerPropagationTest::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant, PublisherOperationsTest::aWriterWithoutListenerAndAPublisherWithoutOnePassItToTheParticipantListener, TopicOperationsTest::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener, ParticipantEntityTest::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant. (commit `9426325d8b2`)

---

## §2 Programming interfaces of [DDS] 1.2: topic classes

### §2 TopicDescription: get_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicCarriesItsExpression`

**Status:** `done` — TopicDescription.getParent returns the DomainParticipant: TopicImpl holds it, ContentFilteredTopicImpl asks the related topic, MultiTopicImpl holds it. Tests: TopicOperationsTest::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant (topic and ContentFilteredTopic, assertSame), MultiTopicTest::theMultiTopicCarriesItsExpression (MultiTopic). (commit `8a5744821fd`)
### §2 TopicDescription: get_type_name

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "readonly: type_name string; get_type_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getTypeName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionFindsOnlyWhatWasCreated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicCarriesItsExpression`

**Status:** `done` — Topic.getTypeName returns the name of the TypeSupport, ContentFilteredTopicImpl.getTypeName that of the related topic. Tests: TopicOperationsTest::aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant (topic and ContentFilteredTopic equal), DomainParticipantOperationsTest::lookupTopicDescriptionFindsOnlyWhatWasCreated ("java::lang::String"), DynamicTopicTest (DynamicType "DynReading"), MultiTopicTest::theMultiTopicCarriesItsExpression. (commit `8a5744821fd`)
### §2 TopicDescription: get_name

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.1, p. 49–50 (`dds-1.2.pdf`), table
TopicDescription — "readonly: name string; get_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionFindsOnlyWhatWasCreated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicCarriesItsExpression`

**Status:** `done` — The three implementations return the name given to createTopic, createContentFilteredTopic and createMultiTopic. The tests expect "Chatter" (Topic), "Hot" (ContentFilteredTopic) and "Hot" (MultiTopic). (commit `5e1603d1c72`)
### §2 Topic: get_inconsistent_topic_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.2, p. 50 (`dds-1.2.pdf`), table Topic —
"get_inconsistent_topic_status (out: status: InconsistentTopicStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.checkTopicConsistency`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.remoteEndpointSeen`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::topicsOfTheSameTypeOrInAnotherDomainAreConsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent`

**Status:** `done` — DomainParticipantImpl.checkTopicConsistency detects when a topic is created a topic of the same name with another type name at another participant of the domain in the process, DomainParticipantImpl.remoteEndpointSeen at an endpoint from SEDP; both sides call TopicImpl.onInconsistentTopic. getInconsistentTopicStatus returns total_count and total_count_change and resets the change. Tests: TopicOperationsTest::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener (total_count 1 on both topics, change reset), ::topicsOfTheSameTypeOrInAnotherDomainAreConsistent (no count for the same type, another domain, another name), ParticipantWireDiscoveryTest::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent (SEDP). (commit `deff69a0210`)
### §2 ContentFilteredTopic: filter_expression

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51–52 (`dds-1.2.pdf`), table
ContentFilteredTopic — "readonly: filter_expression string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getFilterExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createContentFilteredTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`

**Status:** `done` — createContentFilteredTopic compiles the same expression with ContentFilter.compile into the predicate and returns it from getFilterExpression. The test expects "value > 50" and checks that a reader on the ContentFilteredTopic receives only the sample with value 99.0. The public constructor of ContentFilteredTopicImpl takes the text and the predicate separately (QosLifecycleTest::contentFilteredTopicDropsNonMatching); the agreement holds for the path through the participant. (commit `cf0d29f7efb`)
### §2 ContentFilteredTopic: get_related_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51 (`dds-1.2.pdf`), table
ContentFilteredTopic — "get_related_topic: Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getRelatedTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::contentFilteredTopicDropsNonMatching`

**Status:** `done` — getRelatedTopic returns the topic given to createContentFilteredTopic; SubscriberImpl.createDataReader subscribes through that topic. The first test expects assertEquals(topic, hot.getRelatedTopic()) and that the reader receives only the matching sample from a writer on the related topic; the second checks that non-matching samples are dropped. (commit `cf0d29f7efb`)
### §2 ContentFilteredTopic: get_expression_parameters and set_expression_parameters

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.3, p. 51–52 (`dds-1.2.pdf`), table
ContentFilteredTopic — "get_expression_parameters (out: expression_parameters:
string []): ReturnCode_t; set_expression_parameters (expression_parameters:
string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic.getExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.setExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.evaluate`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::setExpressionParametersFiltersWithTheNewParametersFromThenOn`

**Status:** `done` — ContentFilteredTopicImpl.setExpressionParameters compiles the predicate again with ContentFilter.compile(filterExpression, new parameters); evaluate, and with it the readers on the ContentFilteredTopic, use it from the next sample. getExpressionParameters returns the list. Test: TopicOperationsTest::setExpressionParametersFiltersWithTheNewParametersFromThenOn (expression "level > %0", parameter 10 then 3, expected samples 15 and 5 respectively; an empty parameter set is refused and the old one stays). (commit `9579b04127c`)
### §2 MultiTopic: subscription_expression and parameters

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.4, p. 52–53 (`dds-1.2.pdf`), table
MultiTopic — "readonly: subscription_expression string;
get_expression_parameters (out: expression_parameters: string []): ReturnCode_t;
set_expression_parameters (expression_parameters: string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getSubscriptionExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.setExpressionParameters`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicCarriesItsExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theExpressionParametersAreReplaced`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::theWhereClauseUsesTheExpressionParameters`

**Status:** `done` — MultiTopic exists now (org.omg.dds.topic.MultiTopic, MultiTopicImpl, DomainParticipant.createMultiTopic). The tests check the expression, the parameter list before and after setExpressionParameters, and that a new parameter ("1000" to "2000") changes the WHERE clause of a reader. (commit `7c1ccf824a8`)
### §2 TopicListener: on_inconsistent_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.5, p. 53 (`dds-1.2.pdf`), table
TopicListener — "on_inconsistent_topic (the_topic: Topic, status:
InconsistentTopicStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicListener.java` (`TopicListener.onInconsistentTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicAdapter.java` (`TopicAdapter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener`

**Status:** `done` — TopicImpl.onInconsistentTopic counts, triggers the status and calls the TopicListener with an InconsistentTopicEventImpl outside the topic's lock: the own listener if it has the status in its mask, else the DomainParticipantListener (ListenerUp.forTopic). Tests: TopicOperationsTest::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener (TopicAdapter, source and status of the event), ::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener. (commit `deff69a0210`)
### §2 TypeSupport: register_type and get_type_name

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.3.6, p. 53–54 (`dds-1.2.pdf`), table
TypeSupport — "register_type (participant: DomainParticipant, type_name:
string): ReturnCode_t; get_type_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/RegisteredNameTypeSupport.java` (`RegisteredNameTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass`

**Status:** `done` — ServiceEnvironmentImpl.ZeroSPI.newTypeSupport creates the codec under the registered name (RegisteredNameTypeSupport.named); Topic.getTypeName, writer, reader, InProcessBus and SEDP thus use one name. Tests: TopicOperationsTest::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName (writer and reader of two participants pair under "my::Named" and exchange a sample, the DCPSPublication data carry "my::Named", a topic of the same name made from the class has "java::lang::String" and does not pair), ::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass. (commit `9579b04127c`)

---

## §2 Programming interfaces of [DDS] 1.2: Publisher

### §2 Publisher: create_datawriter and delete_datawriter

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "create_datawriter (a_topic: Topic, qos: QosPolicy [], a_listener:
DataWriterListener, mask: StatusKind []): DataWriter; delete_datawriter
(a_datawriter: DataWriter): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterListenerSetAtCreationSeesTheMatchOfEnable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aDefaultWriterQosThatIsNotConsistentIsRefusedAndTheOldOneStays`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::closeContainedEntitiesClosesTheWritersAndWithdrawsThemFromTheBuiltinTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedWriterAndReaderAreNotListedOrFoundAnyMore`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aClosedEntityRefusesToCreateContainedEntities`

**Status:** `done` — createDataWriter has the forms topic, topic with DataWriterQos and topic with DataWriterQos, DataWriterListener and status set; the listener is set before enable, so an existing reader is reported as onPublicationMatched. DataWriterImpl.close removes the writer from PublisherImpl.writers and from DCPSPublication (close hook). createDataWriter refuses an inconsistent DataWriterQos with InconsistentPolicyException. Tests: ListenerPropagationTest::aWriterListenerSetAtCreationSeesTheMatchOfEnable, PublisherOperationsTest::aDefaultWriterQosThatIsNotConsistentIsRefusedAndTheOldOneStays, ::closeContainedEntitiesClosesTheWritersAndWithdrawsThemFromTheBuiltinTopic, CloseCleanupTest::aClosedWriterAndReaderAreNotListedOrFoundAnyMore, PubSubLoopbackTest::singleWriterSingleReaderRoundTrip, BuiltinTopicQosTest::theMatchedEntitiesAreDescribedWithTheirQos. (commit `9426325d8b2`)
### §2 Publisher: lookup_datawriter

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "lookup_datawriter (topic_name: string): DataWriter"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.lookupDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.lookupDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::lookupDataWriterFindsByNameAndByTopicAndForgetsClosedWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedWriterAndReaderAreNotListedOrFoundAnyMore`

**Status:** `done` — PublisherImpl.lookupDataWriter(String) and (Topic) search the list of created writers for the topic name and return null if none matches; a closed writer is removed from the list (close hook). Test: PublisherOperationsTest::lookupDataWriterFindsByNameAndByTopicAndForgetsClosedWriters (found by name and by topic, null for an unknown name and another topic, null after close); CloseCleanupTest::aClosedWriterAndReaderAreNotListedOrFoundAnyMore. (commit `b594326a2a0`)
### §2 Publisher: suspend_publications and resume_publications

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "suspend_publications: ReturnCode_t; resume_publications:
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.resumePublications`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::resumePublicationsNeedsASuspendPublicationsAndTheCallsNest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::writesWhileSuspendedAreDeliveredAsBefore`

**Status:** `done` — PublisherImpl.suspendPublications counts (the calls nest), resumePublications without a matching suspendPublications throws PreconditionNotMetException (DDS 1.4 §2.2.2.4.1.9, PSM Table 7.1); suspend is a hint, the writers deliver unchanged. Tests: PublisherOperationsTest::resumePublicationsNeedsASuspendPublicationsAndTheCallsNest (pair, nesting, exception, AlreadyClosedException after close), ::writesWhileSuspendedAreDeliveredAsBefore. (commit `deff69a0210`)
### §2 Publisher: begin_coherent_changes and end_coherent_changes

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "begin_coherent_changes: ReturnCode_t; end_coherent_changes:
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.beginCoherentChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.beginCoherentChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.endCoherentChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.deferCoherent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.atomically`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/CoherentHold.java` (`CoherentHold`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/CoherentSetParameter.java` (`CoherentSetParameter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.endCoherentSet`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::endCoherentChangesNeedsABeginAndTheCallsNest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aCoherentSetIsDeliveredTogetherWhenThePresentationAsksForCoherentAccess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::withoutCoherentAccessTheSamplesOfASetAreDeliveredAtOnce`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoherentSetAtomicityTest.java::aReaderOnAnotherThreadNeverSeesPartOfASet`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::theParameterValueIsASequenceNumberInTheByteOrderOfTheSubmessage`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::theEndOfASetIsADataWithoutSerializedDataAndWithoutInlineQos`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::theSamplesOfASetAreHeldAndReleasedByTheFirstSampleOutsideIt`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::aSetWithAMissingSampleIsNotReleasedAndSaysSo`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::aCoherentSetOfARemoteWriterReachesACoherentAccessReaderAsAWhole`

**Status:** `done` — beginCoherentChanges/endCoherentChanges nest (`PublisherImpl.coherentDepth`), endCoherentChanges without begin throws PreconditionNotMetException. When the PRESENTATION of the publisher has coherent_access, the publisher holds back the deliveries of its writers (`PublisherImpl.deferCoherent`) and delivers them at the last end in write order as one step (`InProcessBus.atomically`): every read and take of a reader holds the other side of the same gate, so a reader on another thread sees all samples of a set or none (`CoherentSetAtomicityTest`; without the gate it fails). Over RTPS the samples of a set carry `PID_COHERENT_SET` (0x0056) with the sequence number of the first sample of the writer in the set (DDSI-RTPS 2.5 §8.7.6, §9.6.4.2; octets against `coherent_set_bytes` of the Rust implementation), the end is a DATA without data and without inline QoS (Table 9.22, Example 3). A reader of a subscriber with coherent_access holds the samples of a remote set until it is complete (`CoherentHold`) and passes them on together; an incomplete set (a sample is missing) is not delivered and is named in the log (§8.7.6). Expected towards Cyclone DDS, Fast DDS and OpenDDS: they read PID 0x0056 and act per §8.7.6; a DATA without data at the end of a set is provided by Table 9.22. Measured is only the ZeroDDS peer. The group set (access_scope GROUP, PID_GROUP_COHERENT_SET, end-coherent-set DATA of all writers of the publisher) is not built; it is in the open item on PRESENTATION. (commit `f221214b649`)
### §2 Publisher: wait_for_acknowledgments

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/ReliableWriterState.java` (`ReliableWriterState.awaitAcknowledged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitAcknowledgments`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::waitForAcknowledgmentsReturnsAtOnceForReadersOfTheProcess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aSampleTheRemoteReaderAcknowledgedEndsTheWait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aSampleNobodyAcknowledgesIsATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aSampleNobodyAcknowledgedTimesOutAfterTheWait`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::anAckNackThatReachesThePastTheSampleAcknowledgesIt`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aWaiterIsReleasedByTheAckNackOfAnotherThread`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::everyReaderThatReceivedTheSampleHasToAcknowledgeIt`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aReaderThatLeavesIsNotWaitedFor`

**Status:** `done` — PublisherImpl.waitForAcknowledgments waits in turn for the publisher's enabled writers with a common deadline; DataWriterImpl.awaitAcknowledgments, for a writer with an RTPS connection, waits through RtpsParticipant.WireWriter.waitForAcknowledgments on ReliableWriterState.awaitAcknowledged (the history is empty when every reader has acknowledged by ACKNACK) and throws TimeoutException when the deadline passes. Readers in the same process have the sample when write returns (synchronous delivery). A negative maxWait is IllegalArgumentException, a closed publisher AlreadyClosedException. Tests: PublisherOperationsTest::waitForAcknowledgmentsReturnsAtOnceForReadersOfTheProcess, WriterWireAckTest::aSampleTheRemoteReaderAcknowledgedEndsTheWait and ::aSampleNobodyAcknowledgesIsATimeoutException (publisher and writer form), WriterAckStateTest. (commit `deff69a0210`)
### §2 Publisher: get_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic`

**Status:** `done` — PublisherImpl.getParent returns the DomainParticipant that DomainParticipantImpl.createPublisher passes at creation; DataWriterImpl.getParent the publisher. Test: PublisherOperationsTest::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic (assertSame). (commit `cf0d29f7efb`)
### §2 Publisher: delete_contained_entities

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57 (`dds-1.2.pdf`), table
Publisher — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::closeContainedEntitiesClosesTheWritersAndWithdrawsThemFromTheBuiltinTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::closeContainedEntitiesEmptiesPublisherAndSubscriber`

**Status:** `done` — PublisherImpl.closeContainedEntities closes every writer; its close hook removes it from PublisherImpl.writers and withdraws it from DCPSPublication. The publisher stays open and accepts new writers. Tests: PublisherOperationsTest::closeContainedEntitiesClosesTheWritersAndWithdrawsThemFromTheBuiltinTopic (writer closed, lookupDataWriter null, getDataWriters empty, containsEntity false, the built-in reader of another participant sees NOT_ALIVE_NO_WRITERS), CloseCleanupTest::closeContainedEntitiesEmptiesPublisherAndSubscriber. (commit `b594326a2a0`)
### §2 Publisher: set_default_datawriter_qos and get_default_datawriter_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 57–58 (`dds-1.2.pdf`), table
Publisher — "set_default_datawriter_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datawriter_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.setDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.setDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::theDefaultWriterQosIsTheQosOfTheWritersCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aDefaultWriterQosThatIsNotConsistentIsRefusedAndTheOldOneStays`

**Status:** `done` — PublisherImpl.setDefaultDataWriterQos checks consistency (QosRules.checkConsistent) and throws InconsistentPolicyException without changing the default; createDataWriter(Topic) uses the default set, a QoS passed to the call takes precedence, and createDataWriter checks the QoS as well. Tests: PublisherOperationsTest::theDefaultWriterQosIsTheQosOfTheWritersCreatedNext, ::aDefaultWriterQosThatIsNotConsistentIsRefusedAndTheOldOneStays. (commit `deff69a0210`)
### §2 Publisher: copy_from_topic_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.1, p. 58 (`dds-1.2.pdf`), table
Publisher — "copy_from_topic_qos (inout: a_datawriter_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.copyFromTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.copyFromTopicQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::copyFromTopicQosLetsThePoliciesOfTheTopicReplaceThoseOfTheWriter`

**Status:** `done` — PublisherImpl.copyFromTopicQos takes every policy of the TopicQos that a DataWriter knows into the DataWriterQos and replaces the value of the writer QoS (DDS 1.4 §2.2.2.4.1.17); policies the TopicQos does not have (USER_DATA, OWNERSHIP_STRENGTH) stay those of the writer QoS, the QoS passed is unchanged, and the result is not checked for consistency. Test: PublisherOperationsTest::copyFromTopicQosLetsThePoliciesOfTheTopicReplaceThoseOfTheWriter (DEADLINE 5 s and BEST_EFFORT of the writer become DEADLINE 7 s and RELIABLE of the topic, with a topic holding default values infinite and BEST_EFFORT). (commit `deff69a0210`)
### §2 PublisherListener

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.3, p. 71–72 (`dds-1.2.pdf`) —
"PublisherListener: no attributes, no operations (inherits the operations of
DataWriterListener)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterThatAWriterListenerTakesIsNotPassedUp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant`

**Status:** `done` — DataWriterImpl.listenerFor passes a status the writer's listener does not take to the PublisherListener of the PublisherImpl (ListenerUp.forWriter) and then to the one of the DomainParticipant. Tests: PublisherOperationsTest::aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher (source of the event the writer, status with total_count 1 and policy Reliability), ::aWriterThatAWriterListenerTakesIsNotPassedUp, ListenerPropagationTest::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant. (commit `9426325d8b2`)

---

## §2 Programming interfaces of [DDS] 1.2: DataWriter

### §2 DataWriter: register_instance and register_instance_w_timestamp

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "register_instance (instance: Data): InstanceHandle_t;
register_instance_w_timestamp (instance: Data, timestamp: Time_t):
InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.registerInstanceAt`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::registerAndLookupInstance`

**Status:** `done` — registerInstance(T) returns a handle formed by InstanceKey.toHandle; the forms with Time and with long and TimeUnit use the timestamp for the liveliness time (DataWriterImpl.registerInstanceAt). Registering sends nothing to the readers (DDS 1.4 §2.2.2.4.2.5 does not require it). Tests: WriterHandleTest::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey (all three forms, the same handle when registering again), QosLifecycleTest::registerAndLookupInstance. (commit `2ad2eb1f086`)
### §2 DataWriter: unregister_instance and unregister_instance_w_timestamp

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "unregister_instance (instance: Data, handle: InstanceHandle_t):
ReturnCode_t; unregister_instance_w_timestamp (instance: Data, handle:
InstanceHandle_t, timestamp: Time_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterAtHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onMessage`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::unregisterByHandleOrBySampleMakesTheReaderSeeNoWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::aWriterDisposesAndUnregistersTheInstancesItRegistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::unregisterMarksInstanceNotAliveNoWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites`

**Status:** `done` — unregisterInstance(InstanceHandle, ...) resolves the instance through DataWriterImpl.resolve from the handle or, for HANDLE_NIL, from the sample and uses the key of the registration, which the reader looks up. A handle of an instance other than the sample's is PreconditionNotMetException, a handle of no instance of the writer (also one already unregistered) IllegalArgumentException (BAD_PARAMETER). Tests: WriterHandleTest::unregisterByHandleOrBySampleMakesTheReaderSeeNoWriters (reader sees NOT_ALIVE_NO_WRITERS for both instances), InstanceHandleTest::aWriterDisposesAndUnregistersTheInstancesItRegistered, QosLifecycleTest::unregisterMarksInstanceNotAliveNoWriters, ::closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites. (commit `eb7dddf3680`)
### §2 DataWriter: get_key_value

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getKeyValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::getKeyValueReturnsTheInstanceOfTheHandle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::getKeyValueRefusesAnUnkeyedTypeAndAnUnknownHandle`

**Status:** `done` — getKeyValue(T, InstanceHandle) and getKeyValue(InstanceHandle) return a copy of the last sample written for the instance, so the key fields are set; a type without a key is PreconditionNotMetException, a handle of no instance of the writer IllegalArgumentException. Tests: InstanceHandleTest::getKeyValueReturnsTheInstanceOfTheHandle (key 2 for the registered handle, lookupInstance returns the handle), WriterHandleTest::getKeyValueRefusesAnUnkeyedTypeAndAnUnknownHandle. (commit `2ad2eb1f086`)
### §2 DataWriter: lookup_instance

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.lookupInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.lookupInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::registerAndLookupInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey`

**Status:** `done` — lookupInstance(T) returns the handle of the registered or written instance and InstanceHandle.NIL for an unknown one; the form with ModifiableInstanceHandle fills and returns the handle passed. Tests: QosLifecycleTest::registerAndLookupInstance, WriterHandleTest::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey (the key names the instance, form with ModifiableInstanceHandle, unknown key returns the nil handle). (commit `2ad2eb1f086`)
### §2 DataWriter: write and write_w_timestamp

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 62 (`dds-1.2.pdf`), table
DataWriter — "write (data: Data, handle: InstanceHandle_t): ReturnCode_t;
write_w_timestamp (data: Data, handle: InstanceHandle_t, source_timestamp:
Time_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.writeAt`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::everyWriteOverloadCarriesItsSourceTimestampToTheSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::anUnkeyedTypeIsWrittenWithTheNilHandle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::largeByteArrayGrowsEncoderBuffer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::writeOnAClosedWriterThrowsAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNotEnabledWriterRefusesToWrite`

**Status:** `done` — DataWriter has six write overloads (without, with Time, with long and TimeUnit, each with and without InstanceHandle); the timestamp is the source timestamp of the sample at the reader. Tests: WriterHandleTest::everyWriteOverloadCarriesItsSourceTimestampToTheSample (five forms, timestamp and instance handle on the sample), ::anUnkeyedTypeIsWrittenWithTheNilHandle, PubSubLoopbackTest::singleWriterSingleReaderRoundTrip, QuickstartApiTest::largeByteArrayGrowsEncoderBuffer, ExceptionMappingTest::writeOnAClosedWriterThrowsAlreadyClosed, ::aNotEnabledWriterRefusesToWrite. (commit `2ad2eb1f086`)
### §2 DataWriter: write with handle HANDLE_NIL

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2.11, p. 67 (`dds-1.2.pdf`) — "The
special value HANDLE_NIL can be used for the parameter handle. This indicates
that the identity of the instance should be automatically deduced from the
instance_data (by means of the key)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.keyOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::aHandleGivenWithAWriteIsTheHandleOfTheSamplesInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::anUnkeyedTypeIsWrittenWithTheNilHandle`

**Status:** `done` — write(T, InstanceHandle, ...) checks the handle (DataWriterImpl.checkHandle): HANDLE_NIL is allowed, the handle of the sample's instance too, the handle of another instance of the writer is PreconditionNotMetException, a handle of no instance of the writer IllegalArgumentException (BAD_PARAMETER, §2.2.2.4.2.11). Tests: InstanceHandleTest::aHandleGivenWithAWriteIsTheHandleOfTheSamplesInstance (HANDLE_NIL not checked, registered handle yes, another registered handle PreconditionNotMetException), WriterHandleTest::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed (unknown handle IllegalArgumentException for write and dispose). (commit `eb7dddf3680`)
### §2 DataWriter: dispose and dispose_w_timestamp

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t;
dispose_w_timestamp (data: Data, handle: InstanceHandle_t, timestamp: Time_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.disposeAtHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::aWriterDisposesAndUnregistersTheInstancesItRegistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::disposeMarksInstanceNotAliveDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::disposedThenWrittenIsRebornAlive`

**Status:** `done` — dispose(InstanceHandle, ...) and dispose(T, ...) set the instance at the reader to NOT_ALIVE_DISPOSED; the handle forms resolve through DataWriterImpl.resolve (handle of the registration or HANDLE_NIL with a sample) and hit the instance the reader finds by the key. Tests: WriterHandleTest::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed (forms with Time and with long and TimeUnit), InstanceHandleTest::aWriterDisposesAndUnregistersTheInstancesItRegistered, QosLifecycleTest::disposeMarksInstanceNotAliveDisposed, ::disposedThenWrittenIsRebornAlive. (commit `eb7dddf3680`)
### §2 DataWriter: dispose with handle

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2.13, p. 69 (`dds-1.2.pdf`) — "The
constraints on the values of the handle parameter and the corresponding error
behavior are the same specified for the unregister_instance operation (Section
7.1.2.4.2.7)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.disposeAtHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::aWriterDisposesAndUnregistersTheInstancesItRegistered`

**Status:** `done` — dispose(InstanceHandle, T, ...) reports PreconditionNotMetException for the handle of an instance other than the sample's and IllegalArgumentException (BAD_PARAMETER) for a handle the writer does not know (DataWriterImpl.resolve); dispose(InstanceHandle, T) uses the handle. Tests: WriterHandleTest::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed, InstanceHandleTest::aWriterDisposesAndUnregistersTheInstancesItRegistered (neither handle nor sample: IllegalArgumentException). (commit `eb7dddf3680`)
### §2 DataWriter: wait_for_acknowledgments

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.waitForAcknowledgments`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::aWriterWhoseReadersAreInTheProcessIsAcknowledgedWhenWriteReturns`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::aWriterWithoutReadersHasNothingOutstanding`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::anArgumentThatIsNotADurationIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::aClosedWriterAndAWriterThatIsNotEnabledRefuseTheWait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aSampleTheRemoteReaderAcknowledgedEndsTheWait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aSampleNobodyAcknowledgesIsATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aSampleNobodyAcknowledgedTimesOutAfterTheWait`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::anAckNackThatReachesThePastTheSampleAcknowledgesIt`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aWaiterIsReleasedByTheAckNackOfAnotherThread`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::everyReaderThatReceivedTheSampleHasToAcknowledgeIt`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::aReaderThatLeavesIsNotWaitedFor`

**Status:** `done` — DataWriterImpl.waitForAcknowledgments(Duration) and (long, TimeUnit) call awaitAcknowledgments: for readers in the same process the call returns at once, for readers over RTPS it waits for the acknowledgments in ReliableWriterState and throws TimeoutException when maxWait passes; null and a negative wait are IllegalArgumentException, a closed writer AlreadyClosedException, a writer that is not enabled NotEnabledException. Tests: WriterWaitAckTest (four cases), WriterWireAckTest (acknowledgment and timeout over RTPS), WriterAckStateTest (history, ACKNACK, loss of a peer). (commit `deff69a0210`)
### §2 DataWriter: get_liveliness_lost_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_liveliness_lost_status (out: status: LivelinessLostStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessLostFiresWithoutPolling`

**Status:** `done` — The lease is monitored (LeaseMonitor, DataWriterImpl.evaluateLiveliness): a writer with MANUAL_BY_TOPIC and a finite lease that does not renew it counts LIVELINESS_LOST once and is not alive afterwards; assertLiveliness makes it alive again. getLivelinessLostStatus returns total_count and total_count_change; the change is reset when the listener is called or when the status is read. Tests: WriterStatusTest::aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt (listener call, status read, no further count while not alive, a new loss after assertLiveliness), LeaseTest::livelinessLostFiresWithoutPolling. (commit `e72453fce1f`)
### §2 DataWriter: get_offered_deadline_missed_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_offered_deadline_missed_status (out: status:
OfferedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::offeredDeadlineMissedFiresWithoutPolling`

**Status:** `done` — The writer checks DEADLINE per instance (evaluateOfferedDeadline, lease timer); getOfferedDeadlineMissedStatus returns total_count, total_count_change and last_instance_handle. Test: WriterStatusTest::aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt (deadline 30 ms, one write; total_count >= 1, on the first query total_count_change = total_count, last_instance_handle = lookupInstance, on the second query the change = the difference), LeaseTest::offeredDeadlineMissedFiresWithoutPolling. (commit `e72453fce1f`)
### §2 DataWriter: get_offered_incompatible_qos_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_offered_incompatible_qos_status (out: status:
OfferedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getOfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::anIncompatibleReaderIsCountedWithTheRefusedPolicy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::incompatibleReliabilityDoesNotConnect`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged compares the QoS requested by the reader with the one offered (QosCompatibility), does not pair on incompatibility and counts OFFERED_INCOMPATIBLE_QOS with the first refusing policy and a count per policy. Tests: WriterStatusTest::anIncompatibleReaderIsCountedWithTheRefusedPolicy (total_count 1, last_policy_class Reliability, exactly one QosPolicyCount for Reliability with count 1, no match, listener status), QosCompatibilityTest::incompatibleReliabilityDoesNotConnect. (commit `e72453fce1f`)
### §2 DataWriter: get_publication_matched_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_publication_matched_status (out: status:
PublicationMatchedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getPublicationMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getPublicationMatchedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::thePublicationMatchedStatusCountsAndNamesTheLastSubscription`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::statusIsReadableWithoutAListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`

**Status:** `done` — DataWriterImpl keeps total_count, current_count and their change values; the change values are reset when read or when the listener is called. Test: WriterStatusTest::thePublicationMatchedStatusCountsAndNamesTheLastSubscription (first reader: 1/1/1/1; reading again: changes 0; second reader came and went: total_count 2, change 1, current_count 1, change 0; last reader gone: current_count 0, change -1, total_count_change 0), ListenerTest::statusIsReadableWithoutAListener, ::writerListenerFiresOnPublicationMatched. (commit `deff69a0210`)
### §2 DataWriter: last_subscription_handle of PUBLICATION_MATCHED

**Spec:** §2, p. 11; [DDS] 1.2 IDL, p. 160 (`dds-1.2.pdf`) — "struct
PublicationMatchedStatus { [...] InstanceHandle_t last_subscription_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/PublicationMatchedStatus.java` (`PublicationMatchedStatus.getLastSubscriptionHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.readPublicationMatchedStatusLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::thePublicationMatchedStatusCountsAndNamesTheLastSubscription`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged remembers the handle of the reader that last changed the status (match or loss), and readPublicationMatchedStatusLocked puts it in as last_subscription_handle. Test: WriterStatusTest::thePublicationMatchedStatusCountsAndNamesTheLastSubscription (the handle is the reader's getInstanceHandle and an element of getMatchedSubscriptions; after the loss it stays that of the reader that left). (commit `deff69a0210`)
### §2 DataWriter: get_topic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_topic: Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic`

**Status:** `done` — DataWriterImpl.getTopic returns the topic from the constructor; PublisherImpl.lookupDataWriter uses it. Test: PublisherOperationsTest::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic (assertSame). (commit `830064b55c7`)
### §2 DataWriter: get_publisher

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_publisher: Publisher"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic`

**Status:** `done` — DataWriterImpl.getParent returns the Publisher that PublisherImpl.createDataWriter passes to the constructor. Test: PublisherOperationsTest::aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic (assertSame). (commit `830064b55c7`)
### §2 DataWriter: assert_liveliness

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::assertLivelinessOfTheWriterKeepsALeaseFromRunningOut`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt`

**Status:** `done` — DataWriterImpl.assertLiveliness makes the writer alive and renews the lease; with LIVELINESS MANUAL_BY_PARTICIPANT it renews the lease of all writers of this kind in the participant; on a closed writer it is AlreadyClosedException. Tests: WriterStatusTest::assertLivelinessOfTheWriterKeepsALeaseFromRunningOut (lease 1500 ms, renewed every 20 ms over 3 s: no loss; then a loss), ::aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt. (commit `deff69a0210`)
### §2 DataWriter: get_matched_subscription_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63, §7.1.2.4.2.23, p. 70
(`dds-1.2.pdf`) — "get_matched_subscription_data (inout: subscription_data:
SubscriptionBuiltinTopicData, subscription_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getMatchedSubscriptionData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNullArgumentIsABadParameter`

**Status:** `done` — The method returns the built-in data of a matched reader (locally from the reader, remotely from the SEDP announcement). The first test expects the topic name "Chatter" and the reader's UserData and IllegalArgumentException for a handle that is not a matched subscription; the second expects IllegalArgumentException for null. The branch for remote readers has no call in these tests. (commit `830064b55c7`)
### §2 DataWriter: get_matched_subscriptions

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.2, p. 63 (`dds-1.2.pdf`), table
DataWriter — "get_matched_subscriptions (inout: subscription_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getMatchedSubscriptions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::cleanupRemovesSubscription`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::closingTheReaderEndsTheJoin`

**Status:** `done` — The method returns the handles of the local matched readers and of the readers over the RTPS path. The tests expect size 1 with a handle that getMatchedSubscriptionData resolves, size 0 after the reader is closed, and size 1 per writer for the readers of a MultiTopic join. The branch for remote readers has no call in these tests. (commit `830064b55c7`)

---

## §2 Programming interfaces of [DDS] 1.2: DataWriterListener

### §2 DataWriterListener: on_liveliness_lost

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_liveliness_lost (the_writer: DataWriter, status:
LivelinessLostStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onLivelinessLost`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessLostFiresWithoutPolling`

**Status:** `done` — DataWriterListener declares onLivelinessLost(LivelinessLostEvent<TYPE>); evaluateLiveliness calls it outside the writer's monitor when the lease of a MANUAL_BY_TOPIC writer expires. The test writes once, asserts no more and expects the callback with total_count >= 1 within 5 s without polling the status. (commit `830064b55c7`)
### §2 DataWriterListener: on_offered_deadline_missed

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_offered_deadline_missed (the_writer: DataWriter,
status: OfferedDeadlineMissedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onOfferedDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::offeredDeadlineMissedFiresWithoutPolling`

**Status:** `done` — DataWriterListener declares onOfferedDeadlineMissed(OfferedDeadlineMissedEvent<TYPE>); evaluateOfferedDeadline calls it outside the writer's monitor. The test writes once with a 30 ms deadline and expects the callback with total_count >= 1 within 5 s without polling the status. (commit `830064b55c7`)
### §2 DataWriterListener: on_offered_incompatible_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_offered_incompatible_qos (the_writer: DataWriter,
status: OfferedIncompatibleQosStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onOfferedIncompatibleQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::anIncompatibleReaderIsCountedWithTheRefusedPolicy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterThatAWriterListenerTakesIsNotPassedUp`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged calls onOfferedIncompatibleQos of the listener with the status when the QoS requested by a reader is incompatible. Tests: WriterStatusTest::anIncompatibleReaderIsCountedWithTheRefusedPolicy (DataWriterAdapter overrides onOfferedIncompatibleQos), PublisherOperationsTest::aWriterThatAWriterListenerTakesIsNotPassedUp (only the writer listener is called). (commit `e72453fce1f`)
### §2 DataWriterListener: on_publication_matched

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.4.4, p. 72 (`dds-1.2.pdf`), table
DataWriterListener — "on_publication_matched (the_writer: DataWriter, status:
PublicationMatchedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onPublicationMatched`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`

**Status:** `done` — DataWriterListener declares onPublicationMatched(PublicationMatchedEvent<TYPE>); onReaderPresenceChanged calls it when a reader appears and disappears. The test expects one call per event (1 after the reader, 2 after its close), current_count 1 and then 0, total_count 1, and the writer itself as the source of the event. (commit `830064b55c7`)

---

## §2 Programming interfaces of [DDS] 1.2: Subscriber

### §2 Subscriber: create_datareader and delete_datareader

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80 (`dds-1.2.pdf`), table
Subscriber — "create_datareader (a_topic: TopicDescription, qos: QosPolicy [],
a_listener: DataReaderListener, mask: StatusKind []): DataReader;
delete_datareader (a_datareader: DataReader): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedWriterAndReaderAreNotListedOrFoundAnyMore`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::aReaderOnTheMultiTopicReceivesTheMatchingSamples`

**Status:** `done` — createDataReader has the forms without argument, with DataReaderQos and with listener and status set for every TopicDescription; the listener is set before enable, so match and replay from enable reach it. DataReaderImpl.close removes the reader from SubscriberImpl.readers and withdraws it from DCPSSubscription (close hook, SubscriberImpl.readerGone). A reader on a ContentFilteredTopic or a MultiTopic holds the description (SubscriberImpl.usedBy). Tests: ListenerPropagationTest::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders (reader with listener, existing writer), ParticipantEntityTest::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater, CloseCleanupTest::aClosedWriterAndReaderAreNotListedOrFoundAnyMore, ContentFilterTest, MultiTopicTest. (commit `9426325d8b2`)
### §2 Subscriber: lookup_datareader

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80 (`dds-1.2.pdf`), table
Subscriber — "lookup_datareader (topic_name: string): DataReader"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.lookupDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.lookupDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::lookupDataReaderFindsByNameByTopicAndByFilteredTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::aClosedWriterAndReaderAreNotListedOrFoundAnyMore`

**Status:** `done` — SubscriberImpl.lookupDataReader(String) and (TopicDescription) search the list of created readers for the name of the reader's TopicDescription (for a reader on a ContentFilteredTopic its name); a closed reader is removed from the list. Tests: ParticipantSubscriberTest::lookupDataReaderFindsByNameByTopicAndByFilteredTopic (found, null for an unknown name and for the topic under the ContentFilteredTopic, null after close), CloseCleanupTest::aClosedWriterAndReaderAreNotListedOrFoundAnyMore. (commit `b594326a2a0`)
### §2 Subscriber: begin_access and end_access

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 80–81 (`dds-1.2.pdf`), table
Subscriber — "begin_access: ReturnCode_t; end_access: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.beginAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.beginAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.endAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.checkGroupAccess`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::endAccessNeedsABeginAccessAndTheCallsNest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::withAccessScopeGroupTheSamplesAreAccessedBetweenBeginAndEndAccessOnly`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::withTheDefaultAccessScopeNoBeginAccessIsNeeded`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::aReadOfAnotherThreadWaitsForEndAccess`

**Status:** `done` — SubscriberImpl.beginAccess holds a ReentrantLock of the subscriber, which read and take of the readers take as well; the calls nest, a thread that holds the access reads without waiting, others wait. endAccess without beginAccess is PreconditionNotMetException (DDS 1.4 §2.2.2.5.2.9). With PRESENTATION access_scope GROUP, read and take of the readers and getDataReaders of the subscriber outside begin_access are refused with PreconditionNotMetException (SubscriberImpl.checkGroupAccess); the readers of a MultiTopic join are exempt. Tests: ParticipantSubscriberTest::endAccessNeedsABeginAccessAndTheCallsNest, ::withAccessScopeGroupTheSamplesAreAccessedBetweenBeginAndEndAccessOnly, ::withTheDefaultAccessScopeNoBeginAccessIsNeeded, ::aReadOfAnotherThreadWaitsForEndAccess. (commit `deff69a0210`)
### §2 Subscriber: get_datareaders

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "get_datareaders (out: readers: DataReader [], sample_states:
SampleStateKind [], view_states: ViewStateKind [], instance_states:
InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.getDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.hasMatchingSample`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::getDataReadersWithADataStateListsTheReadersThatHoldSuchSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::closingTheReaderEndsTheJoin`

**Status:** `done` — getDataReaders(Collection) returns the subscriber's readers, getDataReaders(Collection, DataState) those that hold a sample in the chosen sample, view and instance states (DataReaderImpl.hasMatchingSample). Test: ParticipantSubscriberTest::getDataReadersWithADataStateListsTheReadersThatHoldSuchSamples (empty before the write, only the reader of the written topic afterwards, empty after take; both readers without a DataState), MultiTopicJoinTest::closingTheReaderEndsTheJoin. (commit `8a5744821fd`)
### §2 Subscriber: notify_datareaders

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "notify_datareaders: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.notifyDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.notifyDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::dataOnReadersGoesToTheSubscriberListenerAndNotifyDataReadersPassesItToTheReadersWithNewData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::withoutASubscriberListenerTheReadersListenersAreCalledAndDataOnReadersStaysChanged`

**Status:** `done` — `SubscriberImpl.notifyDataReaders` calls `DataReaderImpl.notifyDataAvailable` on every reader. That calls `onDataAvailable` on the reader listener only when the DATA_AVAILABLE status of the reader is changed (no read/take since the arrival) and does not go through the listener of the subscriber. `dataOnReadersGoesToTheSubscriberListener...` takes the sample from one of two readers; after `notifyDataReaders` the other reader counts one call, the first none. The second test checks that a reader gets no call after a read. (commit `313af57405c`)
### §2 Subscriber: get_participant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::theParentOfASubscriberIsItsParticipantAndTheParentOfAReaderItsSubscriber`

**Status:** `done` — `SubscriberImpl.getParent` returns the `DomainParticipant` the subscriber was created from. The test expects the creating participant with `assertSame`. (commit `8a5744821fd`)
### §2 Subscriber: delete_contained_entities

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::closeContainedEntitiesClosesTheReadersAndWithdrawsThemFromTheBuiltinTopic`

**Status:** `done` — `SubscriberImpl.closeContainedEntities` calls `close()` on every reader; the close hook of the reader removes it from `readers` and calls `Discovery.readerClosed`. The operation now checks with `ensureOpen` that the subscriber is open. The test watches the built-in topic DCPSSubscription from a second participant of the domain: after the call both readers are closed, `getDataReaders` is empty, `lookupDataReader` returns `null`, the subscriber is open, and two samples without data with NOT_ALIVE_NO_WRITERS arrive. (commit `313af57405c`)
### §2 Subscriber: set_default_datareader_qos and get_default_datareader_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "set_default_datareader_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datareader_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.setDefaultDataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getDefaultDataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::theDefaultReaderQosIsWhatCreateDataReaderUses`

**Status:** `done` — `getDefaultDataReaderQos` returns the defaults before it is set (HISTORY KEEP_LAST, depth 1). After `setDefaultDataReaderQos(q)` the reader created with `createDataReader(topic)` has that QoS (KEEP_ALL). (commit `8a5744821fd`)
### §2 Subscriber: copy_from_topic_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.2, p. 81 (`dds-1.2.pdf`), table
Subscriber — "copy_from_topic_qos (inout: a_datareader_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.copyFromTopicQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::copyFromTopicQosReplacesThePoliciesTheTopicAndTheReaderShare`

**Status:** `done` — `SubscriberImpl.copyFromTopicQos` replaces every policy of the reader QoS that the topic QoS also has (`QosPolicy.ForDataReader`) with the topic value, even if the reader value differs from the default (DDS 1.4 §2.2.2.5.2.17). Policies only the reader has (UserData) stay, the QoS passed in is unchanged. The test sets HISTORY KEEP_LAST 5 and DURABILITY TRANSIENT_LOCAL on the reader, KEEP_ALL and VOLATILE on the topic, and expects the topic values. (commit `313af57405c`)
### §2 SubscriberListener: on_data_on_readers

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.6, p. 104 (`dds-1.2.pdf`), table
SubscriberListener — "on_data_on_readers (the_subscriber: Subscriber): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl (EntityBase.getListener)`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener.onDataOnReaders`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::dataOnReadersGoesToTheSubscriberListenerAndNotifyDataReadersPassesItToTheReadersWithNewData`

**Status:** `done` — `DataReaderImpl.fireDataAvailable` calls `onDataOnReaders` with a `DataOnReadersEventImpl` (source: the subscriber) when the subscriber (or the domain participant) has a listener with DATA_ON_READERS, instead of `onDataAvailable` of the reader. The subscriber has the status flag DATA_ON_READERS (`SubscriberImpl.dataOnReadersChanged`); calling the listener and read/take on a reader of the subscriber reset it. The test sets a `SubscriberAdapter` and two reader listeners and expects calls only on the subscriber listener. (commit `313af57405c`)

---

## §2 Programming interfaces of [DDS] 1.2: DataReader

### §2 DataReader: read

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.read`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::readMarksSamplesRead`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::readReturnsAFreshCopyNotTheReadersObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::readAndTakeOnAClosedReaderThrowAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theSelectorOfReadAndTakeCarriesTheExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::maxSamplesLimitsReadAndTakeAndUnlimitedIsAcceptedAsAValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDataStateSelectsOnSampleViewAndInstanceStates`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::theIteratorWalksTheSamplesInBothDirections`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::readSelectorAndIntFormsReturnTheSameSamplesAsTheListForm`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::destinationOrderBySourceTimestampKeepsTheSamplesOfAnInstanceInTimestampOrder`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::orderedAccessWithScopeTopicReturnsTheSamplesOfAllInstancesInOneOrder`

**Status:** `done` — `read()`, `read(int)`, `read(Selector)` (Sample.Iterator) and `read(List)`, `read(List, Selector)` (copy into the list) apply `maxSamples` and the three state sets in `collectLocked`; `read` marks the samples READ. With DESTINATION_ORDER BY_SOURCE_TIMESTAMP the samples of an instance are ordered by timestamp (`insertOrdered`); with PRESENTATION ordered_access and scope TOPIC or GROUP `collectLocked` returns the samples of all instances in one order by (timestamp, arrival), `maxSamples` cuts that order. Tested: staying in the reader, READ on the second read, copy, AlreadyClosedException, content filter, `read(int)`, `maxSamples`, the state sets, the content of the iterator and both orders. (commit `313af57405c`)
### §2 DataReader: take

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "take (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.instances`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::readAndTakeOnAClosedReaderThrowAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceWithoutLiveWritersAndWithoutSamplesIsForgottenByTheReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::aDisposedInstanceStaysDisposedWhenItsLastWriterUnregisters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::maxSamplesLimitsReadAndTakeAndUnlimitedIsAcceptedAsAValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::anInstanceHandleLimitsReadAndTakeToThatInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::takeReturnsTheSamplesInTheIteratorAndRemovesThemFromTheReader`

**Status:** `done` — `take` in its four forms removes the returned samples from the reader. An instance without live writers and without samples is removed from `instances` (`DataReaderImpl.reclaim`, DDS 1.4 §2.2.2.5.1.8); `lookupInstance` then returns the NIL handle, a later sample is an instance never seen. An instance NOT_ALIVE_DISPOSED stays so when the last writer unregisters. Tested: order, removal, `take(int)`/`maxSamples`, state sets, the iterator form and forgetting the instance. (commit `313af57405c`)
### §2 DataReader: state masks for read and take

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "sample_states: SampleStateKind [], view_states: ViewStateKind [],
instance_states: InstanceStateKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.dataState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataStateImpl.java` (`DataStateImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDataStateSelectsOnSampleViewAndInstanceStates`

**Status:** `done` — `read`/`take` carry the masks through `Selector.dataState(Subscriber.DataState)`; `collectLocked` filters on `sampleStates`, `viewStates` and `instanceStates` (all without `dataState`, `getDataState()` then returns `DataStateImpl.any`). The test brings eight readers into the same state (instance 1 read, instance 3 disposed, instance 2 untouched) and queries one each with READ, NOT_READ, NOT_NEW, NEW, NOT_ALIVE_DISPOSED, ALIVE and NOT_ALIVE_NO_WRITERS (expected counts derived from the state) and `take` with READ. (commit `313af57405c`)
### §2 DataReader: read_w_condition and take_w_condition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read_w_condition (inout: data_values: Data [], inout:
sample_infos: SampleInfo [], max_samples: long, a_condition: ReadCondition):
ReturnCode_t; take_w_condition (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.condition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::aReadConditionOfTheReaderSelectsItsStatesWithTheSelector`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::aQueryConditionOfTheReaderSelectsByItsExpressionAndTheNextInstanceKeepsIt`

**Status:** `done` — `SelectorImpl.condition(ReadCondition)` (a ZeroDDS extension, not in `DataReader.Selector`) binds a condition of the reader to the selector; `getCondition()` returns it. `collectLocked` intersects its state sets and, for a `QueryCondition`, its expression with the current parameters with the other settings of the selector. A condition of another reader gives `PreconditionNotMetException`, a closed one `AlreadyClosedException`. Tested for `read` and `take`. (commit `313af57405c`)
### §2 DataReader: read_next_sample and take_next_sample

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 86 (`dds-1.2.pdf`), table
DataReader — "read_next_sample (inout: data_value: Data, inout: sample_info:
SampleInfo): ReturnCode_t; take_next_sample (inout: data_value: Data, inout:
sample_info: SampleInfo): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.readNextSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.takeNextSample`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::readNextSampleReturnsTheNextSampleNotYetReadAndTakeNextSampleRemovesIt`

**Status:** `done` — `readNextSample` and `takeNextSample` select with a selector with `maxSamples(1)` and SampleState NOT_READ (`DataReaderImpl.nextSampleSelector`), copy the result into the passed `SampleImpl` by `copyInto` and return `false` without a sample (the passed sample is then unchanged). The test calls `readNextSample` three times: two different samples, then `false`; `takeNextSample` removes the sample. (commit `313af57405c`)
### §2 DataReader: read_instance and take_instance

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 87 (`dds-1.2.pdf`), table
DataReader — "read_instance (..., a_handle: InstanceHandle_t, sample_states,
view_states, instance_states): ReturnCode_t; take_instance (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.instance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::anInstanceHandleLimitsReadAndTakeToThatInstance`

**Status:** `done` — `select().instance(handle)` limits `read`/`take` to the instance with that handle (comparison in `collectLocked`). A handle without an instance yields an empty collection (the specification permits BAD_PARAMETER but does not require it). The test reads and takes the two samples of instance 2 of three instances; the samples carry its handle, `take` removes only these. (commit `313af57405c`)
### §2 DataReader: read_next_instance and take_next_instance

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 87 (`dds-1.2.pdf`), table
DataReader — "read_next_instance (..., previous_handle: InstanceHandle_t,
sample_states, view_states, instance_states): ReturnCode_t; take_next_instance
(...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.nextInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::nextInstanceWalksTheInstancesInTheOrderOfTheirHandles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::nextInstanceSkipsInstancesWithoutSamplesInTheSelectedStates`

**Status:** `done` — With `Selector.nextInstance(true)` `collectLocked` returns the samples of the instance with the smallest handle greater than that of `instance(h)` (from the smallest without `instance`) that has matching samples, and only that instance. The handles are ordered by `InstanceHandle.compareTo` (unsigned, byte by byte). The test writes the keys 3, 1, 2 and walks them as 1, 2, 3, then empty; `take` takes only the first instance; instances without a sample in the selected state are skipped. (commit `313af57405c`)
### §2 DataReader: read_next_instance_w_condition and take_next_instance_w_condition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "read_next_instance_w_condition (..., previous_handle:
InstanceHandle_t, a_condition: ReadCondition): ReturnCode_t;
take_next_instance_w_condition (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::aQueryConditionOfTheReaderSelectsByItsExpressionAndTheNextInstanceKeepsIt`

**Status:** `done` — Condition and previous handle combine in one selector: `condition(c).instance(h).nextInstance(true)`. The test reads with a `QueryCondition` "seq > %0" from instance 1 on and gets the samples of instance 2 with seq 1; changed parameters change the result; `take` with the condition removes only the selected samples. (commit `313af57405c`)
### §2 DataReader: return_loan

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "return_loan (inout: data_values: Data [], inout: sample_infos:
SampleInfo []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::closeEndsTheLoanAndClosingAgainIsHarmless`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::theIteratorIsReadOnly`

**Status:** `done` — `Sample.Iterator.close()` maps `return_loan` (`SampleIterator.close`); the call throws nothing, also the second time (Closeable). `read` and `take` copy the data (`deepCopy`), nothing is loaned; the samples stay readable after `close`. The name `returnLoan` does not exist in the OMG API (`close()`). (commit `897b827da64`)
### §2 DataReader: get_key_value

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::getKeyValueReturnsASampleOfTheInstanceThatHasTheKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::getKeyValueSetsTheKeyFieldsOfTheHolderAndLeavesTheOthers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::getKeyValueOfAHandleTheReaderDoesNotKnowIsAnIllegalArgument`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::getKeyValueOnATopicWithoutAKeyIsPreconditionNotMet`

**Status:** `done` — The reader keeps one sample per instance (`Instance.keyData`). For a type of the Java type representation (`ReflectionTypeSupport`) `getKeyValue(keyHolder, handle)` sets the key fields of the holder and leaves the others; for a type without writable fields (record, hand-written support) the result is a copy of that sample instead of the holder. An unknown or `null` handle gives `IllegalArgumentException`, a type without key `PreconditionNotMetException`. `getKeyValue(InstanceHandle)` returns the copy. (commit `313af57405c`)
### §2 DataReader: lookup_instance

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.lookupInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::lookupInstanceIsNilBeforeTheInstanceIsKnownAndTheHandleOfTheSampleAfterwards`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::lookupInstanceWithAHandleFillsItAndWithoutOneReturnsANewObject`

**Status:** `done` — `lookupInstance(T)` returns the handle of a known instance (equal to `Sample.getInstanceHandle()`) or the NIL handle, before the first sample and after the instance is forgotten (`reclaim`). `lookupInstance(handle, key)` fills the handle passed in, for `null` it returns a new object; a `null` key gives `IllegalArgumentException`. (commit `313af57405c`)
### §2 DataReader: create_readcondition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "create_readcondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind []):
ReadCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createReadCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.hasMatchingSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DdsAsyncTest.java::waitForSamplesAsyncFalseOnTimeout`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates`

**Status:** `done` — `createReadCondition(DataState)` returns a `ReadConditionImpl` with the state sets as of its creation (a copy); the trigger value is true while the reader holds a sample in those three sets (`anySample`). `aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates` checks NOT_READ and READ on a WaitSet: empty false (timeout), true after write (the condition is in the active set), the values swap after read, false after take. The condition is kept by the reader and closed by `close`/`closeContainedEntities`. (commit `313af57405c`)
### §2 DataReader: create_querycondition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "create_querycondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind [],
query_expression: string, query_parameters: string []): QueryCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createQueryCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theQueryConditionOfTheReaderTriggersOnAMatchingSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::aConditionKnowsTheFilterThatTriggersIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aQueryConditionNeedsOneSampleThatIsInTheStatesAndSatisfiesTheExpression`

**Status:** `done` — Both overloads of `createQueryCondition` exist. The trigger value is true when one sample is both in the state sets and satisfies the expression (`anySample(state, predicate)`); a sample without data satisfies no expression. The test (overload with `DataState`) has a read sample that does not satisfy the expression and an unread one that does: the trigger is false, true after read. (commit `313af57405c`)
### §2 DataReader: delete_readcondition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 88 (`dds-1.2.pdf`), table
DataReader — "delete_readcondition (a_condition: ReadCondition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aClosedConditionIsGoneAndAClosedConditionOnAWaitSetNeverTriggers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::closeContainedEntitiesClosesTheConditionsOfTheReaderAndNotTheReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::closingTheReaderClosesItsConditions`

**Status:** `done` — The reader keeps its conditions (`DataReaderImpl.register`); `ReadConditionImpl.close` removes the condition there and marks it closed, a second `close` has no effect. `getTriggerValue` and `QueryCondition.setQueryParameters` on a closed condition give `AlreadyClosedException`, a WaitSet skips it. A condition that does not belong to this reader (PRECONDITION_NOT_MET) cannot be expressed by `close()`; the selector reports it (see the item on read_w_condition). (commit `313af57405c`)
### §2 DataReader: get_liveliness_changed_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_liveliness_changed_status (out: status:
LivelinessChangedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.countLivelinessLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireReader.remoteLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertOnWire`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessChangedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::livelinessChangedNamesTheWriterWhoseLivelinessChangedAndCountsTheChanges`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::anIncompatibleWriterIsNotCountedAsAliveOrNotAlive`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderRemoteLivelinessTest.java::aRemoteWriterIsAliveWithinItsLeaseAndAliveAgainAfterAnAssertion`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/LivelinessHeartbeatTest.java::theLivelinessFlagIsTheThirdLowestBitAndIndependentOfFinal`

**Status:** `done` — `alive_count`, `not_alive_count` and both change values are computed from the writers in the same process (`matchedWriters`: PARTITION overlaps, QoS compatible; `countLivelinessLocked`) and from the connected writers of other processes. A remote writer is alive as long as the time since its last sign is within the lease it announced in SEDP; without an announced liveliness it counts as AUTOMATIC with an infinite lease. Signs are a DATA of the writer, a HEARTBEAT with the L flag (DDSI-RTPS 2.5 §8.3.7.5; `DataWriter.assertLiveliness` and the assertion at participant level for MANUAL_BY_PARTICIPANT send it to the connected readers) and, for AUTOMATIC, every message of its participant (SPDP). The reader learns when a writer is alive again after its lease ran out (`WireReader.setLivelinessListener`) and raises `onLivelinessChanged`. The built-in endpoints of the Writer Liveliness Protocol (`DCPSParticipantMessage`, §8.4.13) are not built; ZeroDDS does not announce them in SPDP, so a foreign participant does not send them. Expected: Cyclone DDS, Fast DDS and OpenDDS keep AUTOMATIC writers alive through SPDP and evaluate the L flag of the HEARTBEAT; MANUAL_BY_PARTICIPANT of a foreign writer reaches ZeroDDS only through DATA and HEARTBEAT. (commit `4fcfe2dd339`)
### §2 DataReader: last_publication_handle of LIVELINESS_CHANGED

**Spec:** §2, p. 11; [DDS] 1.2 IDL, p. 159–160 (`dds-1.2.pdf`) — "struct
LivelinessChangedStatus { [...] InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessChangedStatus.java` (`LivelinessChangedStatus.getLastPublicationHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::livelinessChangedNamesTheWriterWhoseLivelinessChangedAndCountsTheChanges`

**Status:** `done` — `last_publication_handle` of LIVELINESS_CHANGED is the handle of the writer whose liveliness changed last (`DataReaderImpl.lastLivelinessHandle`, set in `countLivelinessLocked` when `isAlive` changes and in `onWriterPresenceChanged` when a writer goes); `getLivelinessChangedStatus` and `evaluateLiveliness` pass it on. The test expects `writer.getInstanceHandle()` on appearing and on lease expiry. (commit `313af57405c`)
### §2 DataReader: get_requested_deadline_missed_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_requested_deadline_missed_status (out: status:
RequestedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::infiniteDeadlineNeverMisses`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::requestedDeadlineMissedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::requestedDeadlineMissedNamesTheInstanceThatMissedLast`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::theChangeOfRequestedDeadlineMissedStartsAgainWhenTheStatusIsRead`

**Status:** `done` — Misses are computed per instance from the wall clock (once per elapsed period, from the arrival of the last sample) and reported asynchronously by the reader lease. Tested: `total_count` and `total_count_change`, no miss with an infinite period, `onRequestedDeadlineMissed`, `last_instance_handle` with two instances (equal to the handle of instance 2, which is waited for) and the change value 0 on the second read (period 1500 ms, the next expiry is far after the read). (commit `313af57405c`)
### §2 DataReader: get_requested_incompatible_qos_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_requested_incompatible_qos_status (out: status:
RequestedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePoliciesWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::incompatibleReliabilityDoesNotConnect`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::requestedIncompatibleQosCountsTheRefusingPolicyAndResetsItsChange`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::everyPolicyThatRefusesTheConnectionIsCountedAndOneConnectionIsOneInTotal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::thePresentationOfTheSubscriberMustBeOfferedByThePublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aWriterThatOffersLessThanRequestedIsReportedToTheListener`

**Status:** `done` — On connecting, the reader checks the offered QoS with `QosCompatibility.incompatiblePoliciesWith`: Reliability, Durability, Deadline, Liveliness, Ownership, DestinationOrder, LatencyBudget and PRESENTATION (the policy of the subscriber, requested by the reader, that of the publisher, offered by the writer: lower scope, or coherent/ordered not offered). On incompatibility there is no match and no delivery; one connection counts once in `total_count` and once in the `QosPolicyCount` of each refusing policy; `last_policy_id` is one of them. `onRequestedIncompatibleQos` is called. Tested: one and three policies, the reset of the change, PRESENTATION, the listener. (commit `b531522ca09`)
### §2 DataReader: get_sample_lost_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_sample_lost_status (out: status: SampleLostStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.noteSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::aGapInTheSequenceNumbersOfAWriterIsSampleLost`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::sampleLostIsReportedToTheListener`

**Status:** `done` — `DataReaderImpl.noteSequence` compares the sequence number of a sample from the RTPS path (`deliverWire`) per writer GUID with the last one seen; a gap counts in `lostTotal`, `getSampleLostStatus` reports total and change and resets the change, `onSampleLost` is called. The test delivers sequence numbers 1, 4, 5 of one writer and 10 of a second: total 2, change 2, then 0. The in-process bus loses no samples. (commit `313af57405c`)
### §2 DataReader: get_sample_rejected_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_sample_rejected_status (out: status: SampleRejectedStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleRejectedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::aSampleOfANewInstanceBeyondMaxInstancesIsRejectedWithThatReasonAndInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::aSampleBeyondMaxSamplesPerInstanceIsRejectedWithThatReasonAndInstance`

**Status:** `done` — `DataReaderImpl.rejectedByLimits` applies RESOURCE_LIMITS (max_samples, max_instances, max_samples_per_instance), counts `total_count`, sets reason and instance and fires `onSampleRejected`. Tested: all three reasons with `last_reason` and `last_instance_handle` (the handle of the key derived by hand from the 16 octets), the change value and the listener. (commit `313af57405c`)
### §2 DataReader: get_subscription_matched_status

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_subscription_matched_status (out: status:
SubscriptionMatchedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::statusIsReadableWithoutAListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::nonOverlappingPartitionIsNotAMatch`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::subscriptionMatchedNamesTheWriterThatConnectedOrLeftLast`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::aWriterInAnotherPartitionOrWithLessQosThanRequestedIsNotMatched`

**Status:** `done` — `total_count`, `current_count` and both change values are kept as writers connect and disconnect; writers with non-overlapping PARTITION or incompatible QoS do not count; `last_publication_handle` is the handle of the writer that connected or disconnected last. The tests check counts, changes, listener events and the handles. (commit `313af57405c`)
### §2 DataReader: last_publication_handle of SUBSCRIPTION_MATCHED

**Spec:** §2, p. 11; [DDS] 1.2 IDL, p. 160–161 (`dds-1.2.pdf`) — "struct
SubscriptionMatchedStatus { [...] InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.readSubscriptionMatchedStatusLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::subscriptionMatchedNamesTheWriterThatConnectedOrLeftLast`

**Status:** `done` — `readSubscriptionMatchedStatusLocked` passes `DataReaderImpl.lastMatchedHandle`: the handle (`WriterPresence.writerHandle`) of the writer that connected or disconnected last. The test expects, after the first writer, the second and after closing the first, its `getInstanceHandle()` each time and checks that `getMatchedPublications` contains the handles. (commit `313af57405c`)
### §2 DataReader: get_topicdescription

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_topicdescription: TopicDescription"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.describedBy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::theDescriptionOfAReaderIsTheTopicOrTheContentFilteredTopicItWasCreatedWith`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theReaderReportsTheMultiTopicAsItsDescription`

**Status:** `done` — `getTopicDescription` returns the description the reader was created with: for a ContentFilteredTopic and a MultiTopic `SubscriberImpl.createDataReader` sets it through `describedBy`, otherwise the topic. Topic, ContentFilteredTopic (`assertSame`) and MultiTopic are tested. (commit `830064b55c7`)
### §2 DataReader: get_subscriber

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_subscriber: Subscriber"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::theParentOfASubscriberIsItsParticipantAndTheParentOfAReaderItsSubscriber`

**Status:** `done` — `DataReaderImpl.getParent` returns the `Subscriber` that created the reader. The test expects it with `assertSame`. (commit `830064b55c7`)
### §2 DataReader: delete_contained_entities

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::closeContainedEntitiesClosesTheConditionsOfTheReaderAndNotTheReader`

**Status:** `done` — `DataReaderImpl.closeContainedEntities` closes all conditions of the reader created with `createReadCondition`/`createQueryCondition` (list `conditions`); the reader stays open and can create new conditions. The test checks `AlreadyClosedException` on both conditions, then writing, reading and a new condition. (commit `313af57405c`)
### §2 DataReader: wait_for_historical_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "wait_for_historical_data (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.waitForHistoricalData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireReader.historicalDataReceived`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::aReaderThatIsNotOnTheWireWaitsForNoHistoricalData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::aLateReaderGetsTheHistoricalDataOfATransientLocalWriterBeforeWaitForHistoricalDataReturns`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::theCallReturnsOnceTheAnnouncedSamplesHaveArrived`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::theCallIsATimeoutExceptionWhileTheWriterAnnouncedNothing`

**Status:** `done` — In the same process the samples of a TRANSIENT_LOCAL writer go to the reader in `enable()` (`replayTo`); both overloads of `waitForHistoricalData` then return at once, a negative value gives `IllegalArgumentException`. With an RTPS reader of non-VOLATILE durability the call waits on `WireReader.historicalDataReceived`: every connected reliable, non-VOLATILE remote writer has announced its last sequence number in a HEARTBEAT, the reader has received up to it, and no received message is still on its way to the reader (`deliveriesInFlight`). After `maxWait` a `TimeoutException` follows. Both RTPS branches are tested with a second RTPS participant. (commit `e6b419f65f1`)
### §2 DataReader: get_matched_publication_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89, §7.1.2.5.3.33, p. 102
(`dds-1.2.pdf`) — "get_matched_publication_data (inout: publication_data:
PublicationBuiltinTopicData, publication_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublicationData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::aRemoteWriterIsDescribedByTheMatchedPublicationData`

**Status:** `done` — For a writer in the same process `getMatchedPublicationData` returns the built-in data of the writer, for a remote writer that of the SEDP announcement; an unknown handle throws `IllegalArgumentException`. The test with a writer of a second RTPS participant checks the topic name, type name and OwnershipStrength of the SEDP announcement; the test on the local writer checks the topic name, durability, OwnershipStrength and the exception for the NIL handle. (commit `4664a24a4e3`)
### §2 DataReader: get_matched_publications

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.3, p. 89 (`dds-1.2.pdf`), table
DataReader — "get_matched_publications (inout: publication_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublications`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::subscriptionMatchedNamesTheWriterThatConnectedOrLeftLast`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::aWriterInAnotherPartitionOrWithLessQosThanRequestedIsNotMatched`

**Status:** `done` — `getMatchedPublications` returns the handles of the connected writers in the process (`matchedWriters` with `writerHandle()`) and of those connected over RTPS (`wire.matchedEndpoints`). Tested: size 1, use of the element in `getMatchedPublicationData`, the set shrinks after `writer.close()`, a writer in another partition and one with incompatible QoS are not in it. Writers over RTPS are untested. (commit `313af57405c`)

---

## §2 Programming interfaces of [DDS] 1.2: SampleInfo

### §2 SampleInfo: sample_state

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103, §7.1.2.5.1.2, p. 75
(`dds-1.2.pdf`) — "sample_state: SampleStateKind"; "READ indicates that the
DataReader has already accessed that sample by means of read."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSampleState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::readMarksSamplesRead`

**Status:** `done` — `toSample` returns the state of the sample before the access and then marks it READ in the reader. The test reads the same sample twice and expects NOT_READ the first time and READ the second time. The statement "always NOT_READ" no longer applies. (commit `830064b55c7`)
### §2 SampleInfo: view_state

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103, §7.1.2.5.1.8, p. 77
(`dds-1.2.pdf`) — "view_state: ViewStateKind"; "NEW indicates that either this
is the first time that the DataReader has ever accessed samples of that instance
[...] The view_state is therefore the same for all samples in the returned
collection that refer to the same instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getViewState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::viewStateIsNewUntilFirstAccess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::allSamplesOfAnInstanceInOneCollectionHaveTheSameViewStateAndTheSampleStateOfBeforeTheRead`

**Status:** `done` — `view_state` is fixed once per instance when the collection is assembled (`Pick.view`); NOT_NEW is set only after the collection. The test (KEEP_ALL, two writes, one read): both samples NEW and NOT_READ, NOT_NEW and READ on the second read; an instance reborn after dispose is NEW again. (commit `313af57405c`)
### §2 SampleInfo: instance_state

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "instance_state: InstanceStateKind"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getInstanceState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onMessage`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/InstanceState.java` (`InstanceState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::disposeMarksInstanceNotAliveDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::unregisterMarksInstanceNotAliveNoWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::disposedThenWrittenIsRebornAlive`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites`

**Status:** `done` — The reader keeps `ALIVE`, `NOT_ALIVE_DISPOSED` and `NOT_ALIVE_NO_WRITERS` per instance (`Instance.state`); `onMessage` sets them on dispose, unregister and write. The tests check NOT_ALIVE_DISPOSED after dispose, NOT_ALIVE_NO_WRITERS after unregister and after the last writer closes (not while a second writer is still writing), and ALIVE after another write. The snapshot property is in the following item. (commit `830064b55c7`)
### §2 SampleInfo: instance_state as the state at reading

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.1.3, p. 75 (`dds-1.2.pdf`) — "The
instance_state available in the SampleInfo is a snapshot of the instance_state
of the instance at the time the collection was obtained (i.e., at the time read
or take was called). The instance_state is therefore the same for all samples in
the returned collection that refer to the same instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::disposeSnapshotsInstanceStateForAllSamples`

**Status:** `done` — `toSample` reads `instance_state` from the state of the instance at the time of `read`/`take`, not from the time of arrival. The test (KEEP_ALL) writes two samples, calls dispose and expects NOT_ALIVE_DISPOSED in all three returned samples. The statement "data sample with ALIVE" no longer applies. (commit `830064b55c7`)
### §2 SampleInfo: disposed_generation_count

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "disposed_generation_count: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getDisposedGenerationCount`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceDisposedAndWrittenAgainCountsADisposedGeneration`

**Status:** `done` — The instance keeps `disposedGeneration`; `onWrite` increments it when the instance changes from NOT_ALIVE_DISPOSED to ALIVE; each sample carries the count as of its arrival and `SampleImpl.of` passes it on. The test: write, dispose, write gives 0, 0 (sample without data) and 1. (commit `313af57405c`)
### §2 SampleInfo: no_writers_generation_count

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "no_writers_generation_count: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getNoWritersGenerationCount`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceWithoutWritersWrittenAgainCountsANoWritersGeneration`

**Status:** `done` — The instance keeps `noWritersGeneration`; `onWrite` increments it when the instance changes from NOT_ALIVE_NO_WRITERS to ALIVE. The test: write, unregister, write gives 0, 0 and 1. (commit `313af57405c`)
### §2 SampleInfo: sample_rank

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "sample_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSampleRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::theRanksAreMeasuredWithinTheSamplesOfOneInstanceInTheCollection`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceDisposedAndWrittenAgainCountsADisposedGeneration`

**Status:** `done` — After the selection `collectLocked` counts, per sample, the following samples of the same instance in the collection (`sample_rank`). Tested: two instances with two samples each (1, 0, 1, 0), three samples of one instance (2, 1, 0), a collection cut to one sample (0) and the order across instances with ordered_access. (commit `313af57405c`)
### §2 SampleInfo: generation_rank

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "generation_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getGenerationRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceDisposedAndWrittenAgainCountsADisposedGeneration`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceWithoutWritersWrittenAgainCountsANoWritersGeneration`

**Status:** `done` — `generation_rank` is the sum of the generation counts of the last sample of the instance in the collection minus that of the sample (DDS 1.4 §2.2.2.5.1.6). The tests expect 1, 1, 0 for the samples before, at and after the rebirth. (commit `313af57405c`)
### §2 SampleInfo: absolute_generation_rank

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "absolute_generation_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getAbsoluteGenerationRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceDisposedAndWrittenAgainCountsADisposedGeneration`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceWithoutLiveWritersAndWithoutSamplesIsForgottenByTheReader`

**Status:** `done` — `absolute_generation_rank` is the sum of the generation counts of the instance in the reader (the most recent sample received) minus that of the sample. The first test expects 1, 1, 0; the second that a forgotten and restarted instance has 0 again. (commit `313af57405c`)
### §2 SampleInfo: source_timestamp

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "source_timestamp: Time_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.deliverWire`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSourceTimestamp`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsMessage.java` (`RtpsMessage.decode`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/SerializedSample.java` (`SerializedSample.sourceTimestampMillis`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::theSourceTimestampIsTheOneTheWriterGave`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::theSourceTimestampAndTheWriterGuidOfTheDataReachTheSample`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::theInfoTimestampThatPrecedesADataSubmessageIsItsSourceTimestamp`

**Status:** `done` — In the process the reader takes the timestamp of the writer (`Message.sourceTimestampMillis`). Over RTPS `RtpsMessage.decode` reads the INFO_TS before DATA and DATA_FRAG (Time_t, fraction in 2^-32 s; InvalidateFlag), `SerializedSample.sourceTimestampMillis` carries it to `deliverWire`; without INFO_TS the arrival time applies. Tested: both write forms in the process, the value 77000 through `deliverWire`, the decoding 1700000000 s + 0x80000000/2^32 s = 1700000000500 ms (derived by hand) and the absence (-1). The sending part of the Java participant does not write the timestamp of the application into INFO_TS. (commit `313af57405c`)
### §2 SampleInfo: instance_handle

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "instance_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InstanceKey.java` (`InstanceKey.toHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::theInstanceHandleOfASampleIsTheOneTheReaderAndTheWriterLookUp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::aKeyLongerThanSixteenOctetsKeepsItsOwnInstanceHandle`

**Status:** `done` — `InstanceKey.toHandle` maps a key of up to 16 octets to these octets (zero padded), a longer one to its MD5 digest (DDSI-RTPS 2.5 §9.6.3.8); two keys that the earlier XOR fold mapped alike have different handles. Tested: the handle of a sample equals `Reader.lookupInstance` and `Writer.lookupInstance`, two 17-octet keys with the same fold give different handles. (commit `897b827da64`)
### §2 SampleInfo: publication_handle

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103 (`dds-1.2.pdf`), table
SampleInfo — "publication_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.deliverWire`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getPublicationHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::thePublicationHandleNamesTheWriterThatIsMatchedWithTheReader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::theSourceTimestampAndTheWriterGuidOfTheDataReachTheSample`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::aSampleWithoutAnIdentityHasANilPublicationHandle`

**Status:** `done` — In the process `publication_handle` is the handle of the writer (equal to an element of `getMatchedPublications`). Over RTPS `deliverWire` forms it from the writer GUID of the DATA (`Values.of(guid)`, as `getMatchedPublications` names the endpoint GUID); without an identity it is NIL. (commit `313af57405c`)
### §2 SampleInfo: valid_data

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.5, p. 103, §7.1.2.5.1.4, p. 76
(`dds-1.2.pdf`) — "valid_data: boolean"; "If the flag is set to FALSE, the
DataSample contains no Data."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::disposingAConstituentDisposesTheResultInstance`

**Status:** `done` — `markState` creates a sample without data for dispose and unregister; `Sample.getData()` is then `null` (§7.7.2). On a MultiTopic reader (a `DataReaderImpl`) the test reads a sample with `getData() == null` and NOT_ALIVE_DISPOSED after dispose. The statement that no test checks `null` no longer applies; the unregister path is not tested on its own. (commit `830064b55c7`)

---

## §2 Programming interfaces of [DDS] 1.2: DataReaderListener

### §2 DataReaderListener: on_data_available

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_data_available (the_reader: DataReader): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onDataAvailable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::clearingTheListenerStopsDelivery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::aCallbackDoesNotHoldTheReaderMonitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::withoutASubscriberListenerTheReadersListenersAreCalledAndDataOnReadersStaysChanged`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aStatusConditionTriggersOnlyOnTheStatusesItIsEnabledFor`

**Status:** `done` — `fireDataAvailable` sets DATA_AVAILABLE (`triggerStatus`) and calls `onDataAvailable` per arriving sample and per sample without data (dispose, unregister) with the reader as source. If the subscriber (or the domain participant) has a listener with DATA_ON_READERS, the call goes to its `onDataOnReaders` instead. The tests check the count, the source, no call after `setListener(null)`, that the call does not hold the reader monitor, and that a StatusCondition on DATA_AVAILABLE is true after write and false after take. (commit `313af57405c`)
### §2 DataReaderListener: on_sample_rejected

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_sample_rejected (the_reader: DataReader, status:
SampleRejectedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSampleRejected`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleRejectedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener`

**Status:** `done` — `DataReaderImpl.onWrite` calls `rejectedByLimits`; a rejected sample counts in `SampleRejectedStatusImpl`, `fireRejectedAndLost` calls `onSampleRejected` outside the monitor. `ReaderStatusTest::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener` uses a reader with max_samples=1: the second write triggers the call with reason REJECTED_BY_SAMPLES_LIMIT, the reader keeps the first sample. (commit `830064b55c7`)
### §2 DataReaderListener: on_liveliness_changed

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_liveliness_changed (the_reader: DataReader, status:
LivelinessChangedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onLivelinessChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessChangedFiresWithoutPolling`

**Status:** `done` — `DataReaderImpl.evaluateLiveliness` hands the status to `onLivelinessChanged` outside the monitor, triggered by the reader's lease, without the application polling the status. The test sets a listener, writes once to a MANUAL_BY_TOPIC writer with a 30 ms lease and waits for the callback with `getNotAliveCount() >= 1`. No test checks the counters `alive_count_change` and `not_alive_count_change`. (commit `830064b55c7`)
### §2 DataReaderListener: on_requested_deadline_missed

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_requested_deadline_missed (the_reader: DataReader,
status: RequestedDeadlineMissedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onRequestedDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::requestedDeadlineMissedFiresWithoutPolling`

**Status:** `done` — `DataReaderImpl.fireDeadlineMissed` calls `onRequestedDeadlineMissed` with the status outside the monitor, triggered by the lease, without the application polling. The test writes one sample with a 30 ms deadline and waits for the callback with `getTotalCount() >= 1`. The values of `last_instance_handle` and `total_count_change` are not checked. (commit `830064b55c7`)
### §2 DataReaderListener: on_requested_incompatible_qos

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_requested_incompatible_qos (the_reader: DataReader,
status: RequestedIncompatibleQosStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onRequestedIncompatibleQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.recordRequestedIncompatibleLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aWriterThatOffersLessThanRequestedIsReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::requestedIncompatibleQosCountsTheRefusingPolicyAndResetsItsChange`

**Status:** `done` — `DataReaderImpl.onWriterPresenceChanged` calls `onRequestedIncompatibleQos` outside the monitor when a writer with incompatible QoS appears. The test with a RELIABLE reader and a BEST_EFFORT writer expects one call with `getLastPolicyClass() == Reliability.class` and no matched writer. (commit `830064b55c7`)
### §2 DataReaderListener: on_subscription_matched

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_subscription_matched (the_reader: DataReader, status:
SubscriptionMatchedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSubscriptionMatched`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::nonOverlappingPartitionIsNotAMatch`

**Status:** `done` — `DataReaderImpl.onWriterPresenceChanged` calls `onSubscriptionMatched` when a writer appears and disappears, with the reader as source and the status. The first test checks call counts 1 and 2, `currentCount` 1 and 0, `totalCount` 1, and that the callback hands over the reader itself. The second checks that a writer in another partition causes no call. (commit `830064b55c7`)
### §2 DataReaderListener: on_sample_lost

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.7, p. 105 (`dds-1.2.pdf`), table
DataReaderListener — "on_sample_lost (the_reader: DataReader, status:
SampleLostStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSampleLost`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::sampleLostIsReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::aGapInTheSequenceNumbersOfAWriterIsSampleLost`

**Status:** `done` — `DataReaderImpl.deliverWire` reports the gap in the sequence numbers of a writer to `onSampleLost` through `noteSequence` and `fireRejectedAndLost` (outside the monitor). The test delivers sequence numbers 1 and 3 and expects one call with total 1 and change 1; an older sequence number triggers no further call. (commit `313af57405c`)

---

## §2 Programming interfaces of [DDS] 1.2: conditions and WaitSet

### §2 Condition: get_trigger_value

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.7, p. 31–32 (`dds-1.2.pdf`), table
Condition — "get_trigger_value: boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Condition.java` (`Condition.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getTriggerValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theQueryConditionOfTheReaderTriggersOnAMatchingSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aQueryConditionNeedsOneSampleThatIsInTheStatesAndSatisfiesTheExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aStatusConditionTriggersOnlyOnTheStatusesItIsEnabledFor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`

**Status:** `done` — `ReadConditionImpl.getTriggerValue` (state masks, for the QueryCondition also the expression on the same sample), `StatusConditionImpl.getTriggerValue` (changed status flags of the enabled statuses) and `GuardConditionImpl.getTriggerValue` are tested. (commit `313af57405c`)
### §2 WaitSet: creation without a factory

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`) — "WaitSet has
no factory. It is created as an object directly by the natural means in each
language binding (e.g., using “new” in C++ or Java)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newWaitSet`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theQueryConditionOfTheReaderTriggersOnAMatchingSample`

**Status:** `done` — `WaitSet.newWaitSet(env)` creates a `WaitSetImpl` directly through `env.getSPI().newWaitSet()`, without an entity factory. The first test checks that the SPI returns an object; the second creates a WaitSet through `WaitSet.newWaitSet(reader.getEnvironment())` and attaches and detaches a condition. (commit `2601f4bf35a`)
### §2 WaitSet: attach_condition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table WaitSet
— "attach_condition (a_condition: Condition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.attachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.attachCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait`

**Status:** `done` — `WaitSetImpl.attachCondition` records the condition (attaching twice has no effect) and registers the wait set with a `GuardConditionImpl`; status changes and arriving samples wake all wait sets through `WaitSetImpl.conditionChanged`. Tested: attaching, `getConditions`, waiting on guard, read and status conditions. (commit `2601f4bf35a`)
### §2 WaitSet: detach_condition

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table WaitSet
— "detach_condition (a_condition: Condition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.detachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.detachCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait`

**Status:** `done` — `WaitSetImpl.detachCondition` removes the condition and unregisters it from the `GuardConditionImpl`; a condition that is not attached gives `PreconditionNotMetException`. The test sets a detached guard condition to true: the wait does not end (timeout), after attaching again at once. A closed condition counts as not triggered in the wait set. (commit `313af57405c`)
### §2 WaitSet: wait

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table WaitSet
— "wait (inout: active_conditions: Condition [], timeout: Duration_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.await`),
`crates/java-omgdds/java/src/main/java/org/zerodds/DdsAsync.java` (`DdsAsync.awaitSamples`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aSecondThreadWaitingOnTheSameWaitSetIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aWaitWithATimeoutEndsInATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`

**Status:** `done` — `WaitSetImpl.await` waits until an attached condition is true; guard conditions wake through `signal`, status and read conditions through `WaitSetImpl.conditionChanged` (from `triggerStatus` and on arriving samples). A second waiting thread gives `PreconditionNotMetException`, time running out `TimeoutException`. Tested: waking by `setTriggerValue` and by a write from another thread. (commit `2601f4bf35a`)
### §2 WaitSet: result of wait

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table WaitSet
— "inout: active_conditions: Condition []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.await`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait`

**Status:** `done` — `WaitSetImpl.await` puts all attached conditions with trigger value true into the collection passed in (cleared first). The tests check the size and `assertSame` on the expected condition, also with two conditions on the wait set of which one is false. (commit `2601f4bf35a`)
### §2 WaitSet: get_conditions

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.6, p. 30 (`dds-1.2.pdf`), table WaitSet
— "get_conditions (inout: attached_conditions: Condition []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.getConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.getConditions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`

**Status:** `done` — `getConditions` returns a copy of the attached conditions. The test attaches a ReadCondition and a GuardCondition (two entries), detaches one (one entry, the detached one is missing). (commit `313af57405c`)
### §2 GuardCondition: creation without a factory, initial value FALSE

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.8, p. 32 (`dds-1.2.pdf`) —
"GuardCondition has no factory. It is created as an object directly by the
natural means in each language binding (e.g., using “new” in C++ or Java). When
first created the trigger_value is set to FALSE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`

**Status:** `done` — `GuardCondition.newGuardCondition(env)` returns a `GuardConditionImpl` without an entity factory; the test expects `getTriggerValue() == false` right after creation. (commit `2601f4bf35a`)
### §2 GuardCondition: set_trigger_value

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.8, p. 32 (`dds-1.2.pdf`), table
GuardCondition — "set_trigger_value (value: boolean): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.setTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.setTriggerValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait`

**Status:** `done` — `GuardConditionImpl.setTriggerValue` sets the value and wakes the attached wait sets (`WaitSetImpl.attachCondition` registers them). The test sets the value from a second thread; the blocked wait ends and reports the condition. (commit `2601f4bf35a`)
### §2 StatusCondition: set_enabled_statuses and get_enabled_statuses

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.9, p. 32 (`dds-1.2.pdf`), table
StatusCondition — "set_enabled_statuses (mask: StatusKind []): ReturnCode_t;
get_enabled_statuses: StatusKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.getEnabledStatuses`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.setEnabledStatuses`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getEnabledStatuses`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aStatusConditionTriggersOnlyOnTheStatusesItIsEnabledFor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`

**Status:** `done` — The default of `getEnabledStatuses` is the set of all statuses (`Status.allStatuses`); `setEnabledStatuses` restricts the trigger value to the chosen statuses. The test: SUBSCRIPTION_MATCHED is changed, false after restricting to DATA_AVAILABLE, true after write, false after take. (commit `313af57405c`)
### §2 StatusCondition: get_entity

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.1.9, p. 32 (`dds-1.2.pdf`), table
StatusCondition — "get_entity: Entity"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aStatusConditionTriggersOnlyOnTheStatusesItIsEnabledFor`

**Status:** `done` — `EntityBase.getStatusCondition().getParent()` is the entity; both tests expect the reader with `assertSame`. (commit `e72453fce1f`)
### §2 ReadCondition: get_datareader

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.8, p. 106 (`dds-1.2.pdf`), table
ReadCondition — "get_datareader: DataReader"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createReadCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates`

**Status:** `done` — `ReadConditionImpl.getParent` returns the reader that created the condition; both tests expect it with `assertSame`. (commit `e72453fce1f`)
### §2 ReadCondition: get_sample_state_mask, get_view_state_mask, get_instance_state_mask

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.8, p. 106 (`dds-1.2.pdf`), table
ReadCondition — "get_sample_state_mask: SampleStateKind []; get_view_state_mask:
ViewStateKind []; get_instance_state_mask: InstanceStateKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition.getSampleStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getSampleStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getViewStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getInstanceStates`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`

**Status:** `done` — `createReadCondition` returns exactly the state sets of the `DataState` passed in (a copy as of creation); the trigger value follows them: NOT_READ true until the first read, READ afterwards. (commit `313af57405c`)
### §2 QueryCondition: get_query_expression

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.9, p. 107 (`dds-1.2.pdf`), table
QueryCondition — "get_query_expression: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/QueryCondition.java` (`QueryCondition.getQueryExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl.getQueryExpression`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::aConditionKnowsTheFilterThatTriggersIt`

**Status:** `done` — `QueryConditionImpl.getQueryExpression` returns the expression passed to `DataReaderImpl.createQueryCondition`. The test creates a condition with `"value > %0"` and checks that the returned expression is equal. (commit `cf0d29f7efb`)
### §2 QueryCondition: get_query_parameters and set_query_parameters

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.2.5.9, p. 107 (`dds-1.2.pdf`), table
QueryCondition — "get_query_parameters (out: query_parameters: string []):
ReturnCode_t; set_query_parameters (query_parameters: string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/QueryCondition.java` (`QueryCondition.getQueryParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl.setQueryParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createQueryCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::theQueryParametersAreTheCurrentOnesWhenTheTriggerValueIsEvaluated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::aQueryConditionNeedsOneSampleThatIsInTheStatesAndSatisfiesTheExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::aQueryConditionOfTheReaderSelectsByItsExpressionAndTheNextInstanceKeepsIt`

**Status:** `done` — The trigger value of a QueryCondition compiles expression and current parameters on every evaluation (`createQueryCondition`), so `setQueryParameters` takes effect at once and wakes wait sets. The test: `seq > %0` with 5 and seq 3 false, with 2 true, with 3 false. (commit `313af57405c`)

---

## §2 Programming interfaces of [DDS] 1.2: built-in topics

### §2 Built-in topics: DCPSParticipant

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.5, p. 148 (`dds-1.2.pdf`) —
"DCPSParticipant: key BuiltinTopicKey_t; user_data UserDataQosPolicy"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ParticipantBuiltinTopicData.java` (`ParticipantBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ParticipantBuiltinTopicDataImpl.java` (`ParticipantBuiltinTopicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.participant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::aBuiltinReaderHasTheEntitiesThatExistedBeforeAndNoneOfItsOwnParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::theBuiltinReaderSeesTheUserDataChangeOfAnotherParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantBuiltinDataTest.java::aParticipantWithUserDataRoundTripsItsKeyAndUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredParticipantDataCarriesTheUserData`

**Status:** `done` — `ParticipantBuiltinTopicData` has `getKey` and `getUserData`; `BuiltinData.participant` fills them from SPDP, `BuiltinTopicWire` encodes them. A participant with non-empty USER_DATA appears with that value in the reader on DCPSParticipant and in getDiscoveredParticipantData. Tests: ParticipantDiscoveryTest::aBuiltinReaderHasTheEntitiesThatExistedBeforeAndNoneOfItsOwnParticipant (USER_DATA {1, 2} in the reader of the built-in subscriber), ::theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData (getDiscoveredParticipantData), ::theBuiltinReaderSeesTheUserDataChangeOfAnotherParticipant (setQos announces new data), ParticipantBuiltinDataTest::aParticipantWithUserDataRoundTripsItsKeyAndUserData (wire round trip). (commit `deff69a0210`)
### §2 Built-in topics: DCPSTopic

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.5, p. 148 (`dds-1.2.pdf`) — "DCPSTopic: key,
name, type_name, durability, durability_service, deadline, latency_budget,
liveliness, reliability, transport_priority, lifespan, destination_order,
history, resource_limits, ownership, topic_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicBuiltinTopicData.java` (`TopicBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.topic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantBuiltinDataTest.java::aTopicWithEveryPolicyOffItsDefaultRoundTripsTheKeyAndThePolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredTopicDataCarriesTheTopicQos`

**Status:** `done` — The interface has all fields of the spec (key, name, type_name, durability, durability_service, deadline, latency_budget, liveliness, reliability, transport_priority, lifespan, destination_order, history, resource_limits, ownership, topic_data), `BuiltinData.topic` fills them. ParticipantBuiltinDataTest::aTopicWithEveryPolicyOffItsDefaultRoundTripsTheKeyAndThePolicies sets latency_budget 2 s, liveliness MANUAL_BY_PARTICIPANT with lease 6 s, lifespan 20 s, destination_order BY_SOURCE_TIMESTAMP, ownership EXCLUSIVE and reliability RELIABLE with max_blocking_time 3 s, encodes through BuiltinTopicWire and decodes; key, name, type name and each of these policies are checked. BuiltinTopicQosTest::everyBuiltinDataTypeRoundTripsItsQos covers name, durability, durability_service, deadline, history, resource_limits, transport_priority and topic_data. (commit `2601f4bf35a`)
### §2 Built-in topics: DCPSPublication

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.5, p. 149 (`dds-1.2.pdf`) —
"DCPSPublication: key, participant_key, topic_name, type_name, durability,
durability_service, deadline, latency_budget, liveliness, reliability, lifespan,
user_data, ownership, ownership_strength, destination_order, presentation,
partition, topic_data, group_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.publication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aBuiltinReaderSeesTheQosOfTheAnnouncedWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantBuiltinDataTest.java::aPublicationKeepsItsOwnDurabilityServiceAndTheTopicDataOfItsTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`

**Status:** `done` — All fields of the spec are in the interface and in `BuiltinData.publication`. The wire test checks parameter IDs and bytes for durability, deadline, latency_budget, liveliness, reliability, lifespan, user_data, ownership, ownership_strength, destination_order, partition, group_data, presentation, topic_name, type_name and both GUIDs; the round trip compares all fields after decoding, the reader test reads differing values from the built-in reader. ParticipantBuiltinDataTest::aPublicationKeepsItsOwnDurabilityServiceAndTheTopicDataOfItsTopic checks durability_service of the writer (history_depth 9, max_samples 80) and topic_data of the topic ({7, 7, 7}) after the round trip. (commit `2601f4bf35a`)
### §2 Built-in topics: DCPSSubscription

**Spec:** §2, p. 11; [DDS] 1.2 §7.1.5, p. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, topic_name, type_name, durability,
deadline, latency_budget, liveliness, reliability, ownership, destination_order,
user_data, time_based_filter, presentation, partition, topic_data, group_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/SubscriptionBuiltinTopicData.java` (`SubscriptionBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.subscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantBuiltinDataTest.java::aSubscriptionWithEveryPolicyOffItsDefaultRoundTripsTheKeysTheTypeAndThePolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::sedpOfAnEndpointWithoutQosParametersTakesTheDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`

**Status:** `done` — The interface and `BuiltinData.subscription` contain all fields of the spec. ParticipantBuiltinDataTest::aSubscriptionWithEveryPolicyOffItsDefaultRoundTripsTheKeysTheTypeAndThePolicies sets deadline 3 s, latency_budget 1 s, ownership EXCLUSIVE, destination_order BY_SOURCE_TIMESTAMP on the reader and group_data, presentation (scope TOPIC, coherent and ordered) and partition on the subscriber and checks after encoding and decoding key, participant_key, topic_name, type_name and each of these policies. BuiltinTopicQosTest::everyBuiltinDataTypeRoundTripsItsQos covers durability, liveliness, reliability, time_based_filter, user_data, partition and topic_data. (commit `2601f4bf35a`)

---

## §3 References

### §3.1 [DDS] version 1.2

**Spec:** §3.1, p. 11 — "[DDS] Data Distribution Service for Real-Time Systems
Specification, version 1.2 (OMG document formal/2007-01-01)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Reference. The items under §2 cite [DDS] 1.2 (`.agent/spec/dds-1.2.txt`). Comments in the Java code cite DDS 1.4 (e.g. `ListenerTest`: "DDS 1.4 §2.2.4.1", `DataReaderImpl`); ZeroDDS itself follows DDS 1.4 (`.agent/spec/dds-dcps-1.4.txt`).
### §3.1 [DDS-CCM] 1.0 Beta 1

**Spec:** §3.1, p. 11 — "[DDS-CCM] DDS for Lightweight CCM, version 1.0 Beta 1
(OMG document ptc/2009-02-02)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:** —

**Status:** `n/a (informative)` — Reference. The Java binding requires nothing of its own from [DDS-CCM]; `QosProviderImpl` mentions the XML QoS libraries of [DDS-CCM] in its class comment. No comparison with Beta 1 was made. (commit `cf0d29f7efb`)
### §3.1 [DDS-XTypes] 1.0 Beta 1

**Spec:** §3.1, p. 11 — "[DDS-XTypes] Extensible and Dynamic Topic Types for
DDS, version 1.0 Beta 1 (OMG document ptc/2010-05-12)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Extensibility.java` (`Extensibility.Kind`)

**Tests:** —

**Status:** `n/a (informative)` — Reference. Where needed, the comparison is with XTypes 1.3 (`.agent/spec/dds-xtypes-1.3.txt`). `org.omg.dds.type.Extensibility.Kind` still names the extensibility `EXTENSIBLE_EXTENSIBILITY`.
### §3.1 [Java-MAP] IDL to Java 1.3

**Spec:** §3.1, p. 11 — "[Java-MAP] IDL to Java Language Mapping, Version 1.3
(OMG document formal/2008-01-11)"

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`adapter::java`)

**Tests:** —

**Status:** `n/a (informative)` — Reference. The repository holds IDL4 to Java 1.0 (`.agent/spec/idl4-java-1.0.txt`, coverage in `docs/spec-coverage/idl4-java-1.0.md`). For public fields the Java codegen cites "§1.5.10 of the OMG mapping" (`crates/lang-rules/src/adapter/java.rs`, comment on public fields).
### §3.1 [Java-Lang] The Java Language Specification, Third Edition

**Spec:** §3.1, p. 12 — "[Java-Lang] The Java Language Specification, Third
Edition, published by Addison Wesley in 2005 with ISBN 0321246780"

**Repo:**
`crates/java-omgdds/java/pom.xml` (`properties/profile java8`),
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` (`PubSubLoopbackTest.Reading`)

**Tests:** —

**Status:** `n/a (informative)` — Reference to Java 5. The module compiles with `maven.compiler.source` 17 and, in the profile `java8`, with release 8 (`crates/java-omgdds/java/pom.xml`); the tests need Java 17 (records, e.g. `PubSubLoopbackTest.Reading`). (commit `57f4d99f545`)
### §3.1 [XML] 1.1, Second Edition

**Spec:** §3.1, p. 12 — "[XML] Extensible Markup Language (XML), version 1.1,
Second Edition (W3C recommendation, August 2006)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:** —

**Status:** `n/a (informative)` — Reference for the XML QoS profiles (§7.3.5.4). The Java binding reads XML: `QosProviderImpl` parses through `javax.xml.parsers.DocumentBuilderFactory` with external DTD and schema access disabled. Which XML version applies was not checked. (commit `cf0d29f7efb`)
### §3.2 [JMS] 1.1 (non-normative)

**Spec:** §3.2, p. 12 — "The following non-normative references are provided for
informational purposes. • [JMS] Java Message Service Specification, version 1.1
(Sun Microsystems, http://java.sun.com/products/jms/ docs.html)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.

---

## §4 Terms and Definitions

### §4 Data-Centric Publish-Subscribe (DCPS)

**Spec:** §4, p. 12 — "The mandatory portion of the DDS specification used to
provide the functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Data Distribution Service (DDS)

**Spec:** §4, p. 12 — "An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness and
reliability. It is independent of implementation languages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Data Local Reconstruction Layer

**Spec:** §4, p. 12 — "The optional portion of the DDS specification used to
provide the functionality required for an application for direct access to data
exchanged at the DCPS layer. This latter builds upon the DCPS layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Java Archive (JAR)

**Spec:** §4, p. 12 — "A zip file, whose name ends in the suffix .jar, that
contains the compiled Java class files and other artifacts that comprise a Java
library."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Java Runtime Environment (JRE)

**Spec:** §4, p. 12 — "The environment within which Java applications execute.
The JRE consists of an executing instance of a JVM, a set of class libraries,
and potentially other components."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Java Virtual Machine (JVM)

**Spec:** §4, p. 12 — "An abstract computing machine capable of executing
interpreted and/or compiled Java byte code. JVM implementations typically take
the form of executables that run as processes under operating systems, but this
style of implementation is not mandatory."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Platform-Independent Model (PIM)

**Spec:** §4, p. 12 — "An abstract definition of a facility, often expressed
with the aid of formal or semi-formal modeling languages such as OMG UML, that
does not depend on any particular implementation technology."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.
### §4 Platform-Specific Model (PSM)

**Spec:** §4, p. 13 — "A concrete definition of a facility, typically based on a
corresponding PIM, in which all implementation-specific dependencies have been
resolved."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Term definition.

---

## §5 Symbols

### §5 No symbols

**Spec:** §5, p. 13 — "This specification does not define any symbols or
abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — No requirement.

---

## §6 Additional Information

### §6.1 No change to other OMG specs

**Spec:** §6.1, p. 13 — "This specification does not extend or modify any
existing OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Statement about the specification.
### §6.2 Dependency on Java SE 5, further constraints by the implementation

**Spec:** §6.2, p. 13 — "This specification depends on version 5 of the Java
Standard Edition platform. Service implementations may impose additional
constraints; the nature and scope of these, if any, are unspecified."

**Repo:**
`crates/java-omgdds/java/pom.xml` (`properties/profile java8`)

**Tests:** —

**Status:** `n/a (informative)` — Statement about the specification and permission for further constraints. ZeroDDS compiles with Java 17 or, in the profile `java8`, with release 8 (`crates/java-omgdds/java/pom.xml`); Java 5 to 7 is not a target. (commit `57f4d99f545`)
### §6.2 Rationale: Java SE instead of Java EE or Java ME

**Spec:** §6.2, p. 13 — "Design Rationale (non-normative) As of the publication
of this specification, Java SE remains the predominant platform for the
development and deployment of DDS Java applications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §6.3 Acknowledgements

**Spec:** §6.3, p. 13 — "The following companies submitted this specification: •
Real-Time Innovations, Inc. (RTI) • PrismTech"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgement.

---

## §7.1 Specification Organization

### §7.1 Organized by the modules of [DDS]

**Spec:** §7.1, p. 15 — "This specification is organized according to the module
defined by the DDS specification and the types and operations defined within
them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Organisation of the specification.

---

## §7.2 General Concerns and Conventions

### §7.2 Cross-cutting definitions

**Spec:** §7.2, p. 15 — "This sub clause defines those elements of this
specification that cut across multiple DDS modules."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction; the provisions follow in §7.2.1 to §7.2.6.
### §7.2.1 Packages with the prefix `org.omg.dds`

**Spec:** §7.2.1, p. 15 — "This PSM is defined in a set of Java packages, the
names of each beginning with the prefix org.omg.dds. Each of these contains a
Java interface or abstract class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedStringTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedBytesTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/TypeObject.java` (`TypeObject`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::ddsObjectCarriesTheOneOperationTheClauseNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreBuiltinKeyedTest.java::keyedStringHasTheDelimitedCdr2Layout`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreBuiltinKeyedTest.java::keyedBytesHasTheDelimitedCdr2Layout`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreBuiltinKeyedTest.java::aTopicOfKeyedStringCarriesTheSamplesOfTwoInstances`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreBuiltinKeyedTest.java::aTopicOfKeyedBytesCarriesTheOctets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreBuiltinKeyedTest.java::theKeyHashIsTheMd5OfTheBigEndianKey`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::everyInterfaceOfThePackageIsImplementedAndAnObjectOfItComesBackOverTheWire`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`

**Status:** `done` — The packages `org.omg.dds.*` contain, for every type of the DDS modules, an interface or abstract class of the OMG source (184 files; `CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource` compares them with the SHA-256 of the source). KeyedString and KeyedBytes are topic types: `TypeSupportResolver.resolve` returns `BuiltinTypeSupports.KeyedStringTypeSupport` and `KeyedBytesTypeSupport` (XTypes 1.3 Annex E: appendable, `@key string key`); `CoreBuiltinKeyedTest` writes and reads one topic each with two instances, the key hash octets are derived from XTypes 1.3 §7.6.8 (MD5 over the key in big-endian, not checked against another implementation). The types of `org.omg.dds.type.typeobject` are implemented as objects (`TypeObjectModel`) and reachable through `getType()` of the built-in data. (commit `6d31a402794`)
### §7.2.1 One interface or abstract class per type

**Spec:** §7.2.1, p. 15 — "Each of these contains a Java interface or abstract
class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSObject.java` (`DDSObject`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::thePsmSampleIsAnInterface`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::everyTypeOfTheOmgApiIsAnInterfaceOrAbstractExceptTheConcreteOnesOfTheOmgSource`

**Status:** `done` — CoreApiShapeTest checks by reflection for the packages core, core.policy, core.status, core.event, domain, pub, sub, topic, type, type.builtin, type.dynamic and type.typeobject that every public type is an interface, enum, annotation or abstract class. The concrete classes of the OMG source are exempt: `ServiceConfigurationException`, `ServiceInitializationException`, the six `*Adapter` classes, `TypeId`, `TypeKind.Values`, `DataRepresentation.Id` and the `MemberId` classes. The package `org.omg.dds.rpc` is not covered. (commit `e72453fce1f`)
### §7.2.1 Entities, value types, policies and statuses as an interface or abstract class

**Spec:** §7.2.1, p. 15 — "Each of these contains a Java interface or abstract
class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::entitiesValueTypesPoliciesAndStatusesAreInterfacesOrAbstractClasses`

**Status:** `done` — CoreApiShapeTest checks that DomainParticipant, DomainParticipantFactory, Publisher, Subscriber, Topic, DataWriter, DataReader are interfaces or abstract classes, Time, Duration and InstanceHandle abstract classes, QosPolicy and the policies interfaces, LivelinessChangedStatus and PublicationMatchedStatus abstract classes. (commit `2601f4bf35a`)
### §7.2.1 All packages in one JAR `omgdds.jar`

**Spec:** §7.2.1, p. 15 — "All of these packages, and the types within them, are
packaged into a single JAR file, omgdds.jar (see Annex A: Java JAR Library
File)."

**Repo:**
`crates/java-omgdds/java/pom.xml`

**Tests:** —

**Status:** `n/a (informative)` — Describes the artefact of the OMG (Annex A: `omgdds.jar`). ZeroDDS builds `org.zerodds:omgdds` (`crates/java-omgdds/java/pom.xml`, no `finalName`, so `omgdds-<version>.jar`; read from the POM, not built). The JAR holds the classes under `org.omg.dds`, `org.zerodds` and, through `build-helper-maven-plugin`, the sources of `crates/idl-java/runtime` and `runtime/java/src/main/java`. Annex A permits shipping the implementation in the same JAR. (commit `57f4d99f545`)
### §7.2.1 The service implements the abstract types concretely

**Spec:** §7.2.1, p. 15 — "All those types that are abstract—including
interfaces and abstract classes—are intended to be implemented concretely by the
Service implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectBuilder.java` (`TypeObjectBuilder`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::everyInterfaceOfThePackageIsImplementedAndAnObjectOfItComesBackOverTheWire`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::theObjectsAreSerializable`

**Status:** `done` — `ServiceEnvironmentImpl` and the classes under `org/zerodds` (`DomainParticipantFactoryImpl`, `DomainParticipantImpl`, `PublisherImpl`, `SubscriberImpl`, `TopicImpl`, `DataWriterImpl`, `DataReaderImpl`, `SampleImpl`, `TimeImpl`, `DurationImpl`, `InstanceHandleImpl`, `WaitSetImpl`) implement the abstract types. `QuickstartApiTest.byteArrayQuickstartRoundTrip` creates participant, topic, writer and reader through the environment and reads a `Sample` back. The 26 interfaces under `org/omg/dds/type/typeobject` (`TypeObject`, `StructureType`, `UnionType`, …) are implemented by `TypeObjectModel`, over a TypeObject of XTypes 1.1 Annex B that is taken from the IDL of the annex in `idl/typeobject` and translated with `idlc` into an XCDR1 codec; `TypeObjectModelTest` builds a library with a type of every kind, sends it through the codec and finds objects of all 26 interfaces again. (commit `6d31a402794`)
### §7.2.1 Additional properties in subtypes of the implementation permitted

**Spec:** §7.2.1, p. 15 — "In addition, the subtypes defined by the
implementation may expose additional implementation-specific properties and
operations; however, the nature of these, if any, is undefined."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission. The implementation classes live under `org.zerodds` (e.g. `DataReaderImpl.isClosed`, the methods of the internal interface `InProcessBus.Endpoint`). The rule on where extensions go is in the item "§7.2.6 No extensions in `org.omg.dds` or sub-packages".
### §7.2.1 Rationale: several packages

**Spec:** §7.2.1, p. 15 — "Design Rationale (non-normative) This PSM divides the
types it defines into multiple packages, rather than colocating them in a single
package [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.2.2 Accept value types of another implementation

**Spec:** §7.2.2, p. 15 — "It shall be possible to pass an instance of any value
type (see 7.2.3) created by one DDS implementation to a method implemented by
another. For example, the method DataWriter.write optionally accepts an argument
of type InstanceHandle; this object may have been created by the same DDS
implementation that created the DataWriter or by another DDS implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl.bytesOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InstanceKey.java` (`InstanceKey.of`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aHandleOfAnotherImplementationIsAcceptedEverywhere`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aNilHandleOfAnotherImplementationIsTheNilHandle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aTimeOfAnotherImplementationIsComparedByValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aDurationOfAnotherImplementationIsAccepted`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aTimeOfAnotherImplementationIsTheSourceTimestamp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aForeignHandleThatNamesNoInstanceOfTheWriterIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aNilHandleOfAnotherImplementationNamesTheInstanceOfTheSample`

**Status:** `done` — `InstanceHandleImpl.bytesOf` accepts an `InstanceHandle` of another implementation: nil through `isNil()` gives 16 zero bytes, otherwise the first 16 bytes of the SHA-256 over class name and Java serialization (fallback `toString()`); equal foreign values give equal bytes, `equals` and `compareTo` are total. `ModifiableTimeImpl.copyFrom` and the writer operations accept foreign values (`Time` and `Duration` through `getTime`/`getDuration`). `DataWriterImpl.resolve` and `checkHandle` test nil through `isNil()`, a foreign handle without an instance gives `IllegalArgumentException` (DDS 1.4 §2.2.2.4.2.11: invalid handle, BAD_PARAMETER; `aForeignHandleThatNamesNoInstanceOfTheWriterIsABadParameter`), a handle of another instance than that of the sample gives `PreconditionNotMetException`. The foreign classes in the test are written against the abstract classes of the OMG API. (commit `d9fb6db1612`)
### §7.2.2 Write samples of a foreign DataReader with the own DataWriter

**Spec:** §7.2.2, p. 15 — "It shall be possible to read or take samples from a
DataReader provided by one DDS implementation and immediately write them using a
DataWriter provided by another DDS implementation, provided that the samples are
of a DDS type compatible with that DataWriter."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aSampleOfAnotherImplementationIsWrittenWithItsTimestampAndHandle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aSampleOfAnotherImplementationIsDisposedAndUnregistered`

**Status:** `done` — CoreForeignValuesTest implements a `Sample` and a `ModifiableInstanceHandle` of another implementation and passes `getData()`, `getSourceTimestamp()` and `getInstanceHandle()` to `write`, `dispose` and `unregisterInstance` of a ZeroDDS `DataWriter`. The reader sees the timestamp in milliseconds from the foreign value (1 700 000 000 123 ms from 1 700 000 000 123 000 000 ns), `dispose` gives `NOT_ALIVE_DISPOSED`. A foreign handle that names no instance of the writer gives `PreconditionNotMetException`. (commit `6479f670a5b`)
### §7.2.2 Copying when passing between implementations

**Spec:** §7.2.2, p. 15 — "Note that passing an object from one implementation
to another may incur a performance cost, as the “receiving” implementation may
have to copy the object in question before operating on it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Note.
### §7.2.2 Foreign concrete types: exception or undefined behaviour permitted

**Spec:** §7.2.2, p. 16 — "Otherwise, unless elsewhere noted in this
specification, a Service implementation may raise an exception or behave in an
undefined way if it encounters a concrete type defined by another party."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission. ZeroDDS uses it: `PublisherImpl.codecOf` throws `UnsupportedOperationException` for a `TypeSupport` of another implementation, `InstanceHandleImpl.bytesOf` throws `IllegalArgumentException`.
### §7.2.3 Types without a PIM factory: abstract class with static factory methods `new<ClassName>`

**Spec:** §7.2.3, p. 16 — "These latter types—those without PIM-defined factory
construction methods—serve as their own factories. Each is represented as an
abstract class with one or more static factory methods. These methods are named
according to the convention new<ClassName> in order to resemble constructor
invocations and are amenable to use with the Java 5 static import facility."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time.newTime`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Duration.java` (`Duration.newDuration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/InstanceHandle.java` (`InstanceHandle.newInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider.newQosProvider`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString.newKeyedString`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes.newKeyedBytes`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theQueryConditionOfTheReaderTriggersOnAMatchingSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::theStaticFactoryMethodsOfTheAbstractValueTypesGoThroughTheSpi`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aNewInstanceHandleIsNilAndIsFilledByCopyFrom`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aTypeSupportIsRegisteredUnderTheFullyQualifiedNameOfTheType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::keyedStringAndKeyedBytesMutatorsReturnTheObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aQosProviderOfAnUnsupportedUriSchemeIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aNegativeInstantInAnyUnitIsInvalid`

**Status:** `done` — The static factory methods go through the SPI: `Time.newTime`, `Duration.newDuration`, `InstanceHandle.newInstanceHandle`, `QosProvider.newQosProvider`, `TypeSupport.newTypeSupport`, `KeyedString.newKeyedString` and `KeyedBytes.newKeyedBytes`; CoreSpiTest and CoreValuesTest call each and check value and environment. `TypeSupport.newTypeSupport(Class, env)` in `org/omg/dds/type/TypeSupport.java` registered the type under `type.getClass().getName()` (`java.lang.Class`); the defect was in the OMG source, the file now uses `type.getName()` (PSM 7.5.1). (commit `814bf0d160c`)
### §7.2.3 PIM deletion operations as `close` on the product

**Spec:** §7.2.3, p. 16 — "This PSM maps the factory deletion methods of the DDS
PIM (e.g., DomainParticipant.delete_publisher) to close methods on the “product”
interfaces themselves (e.g., Publisher.close)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::writeOnAClosedWriterThrowsAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::readAndTakeOnAClosedReaderThrowAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aClosedEntityRefusesToCreateContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::aClosedContentFilteredTopicRefusesToCreateAReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::aClosedMultiTopicRefusesToCreateAReader`

**Status:** `done` — `ContentFilteredTopicImpl.close` and `MultiTopicImpl.close` record the closed state (`isClosed`); `SubscriberImpl.createDataReader` refuses a closed description with `AlreadyClosedException`. A `ContentFilteredTopic` of another implementation now gives `UnsupportedOperationException` (before: `AlreadyClosedException`). (commit `6a6f325cdf3`)
### §7.2.3 Closing closes the contained objects

**Spec:** §7.2.3, p. 16 — "Closing an Entity implicitly closes all of its
contained objects, if any. For example, closing a Publisher also closes all of
its contained DataWriters."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::publisherCloseClosesItsWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::closeContainedEntitiesClosesThemWithoutClosingTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::closingASubscriberClosesItsReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::closingTheParticipantClosesTheReadersAndWritersInIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::aClosedContentFilteredTopicRefusesToCreateAReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreCloseTest.java::aClosedMultiTopicRefusesToCreateAReader`

**Status:** `done` — CoreCloseTest closes a `Subscriber` and a `DomainParticipant` and checks with `Qos.isClosed` that their `DataReader` objects (and the `DataWriter`) are closed; `ContentFilteredTopic.close` and `MultiTopic.close` act through the closed flag. (commit `6a6f325cdf3`)
### §7.2.3 Rationale: the `close` pattern

**Spec:** §7.2.3, p. 16 — "Design Rationale (non-normative) The close
destruction design pattern is intended to be familiar to those developers who
have used java.io stream APIs and/or [JMS] [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.2.3 Recommendation to call `close`

**Spec:** §7.2.3, p. 16 — "Users of this PSM are recommended to call close once
they are finished using such heavyweight objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Recommendation to applications.
### §7.2.3 Automatic closing of objects no longer in use permitted

**Spec:** §7.2.3, p. 16 — "In addition, implementations may automatically close
objects that the JRE deems to be no longer in use—for example, they may call
close() in an Object.finalize() override—subject to the following restrictions:"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.retain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.retain`)

**Tests:** —

**Status:** `n/a (informative)` — Permission. ZeroDDS closes nothing automatically: under `crates/java-omgdds/java/src/main/java` there is neither `finalize` nor `Cleaner` (text search; the only hit for `WeakReference` is `GuardConditionImpl`, which closes nothing with it). A `DataReader` that the application drops without `close()` stays registered at the `InProcessBus` (read from the code, not measured). (commit `5920d2fbe32`)
### §7.2.3 An object directly referenced is still in use

**Spec:** §7.2.3, p. 16 — "• Any object to which the application has a direct
reference is still in use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Restriction on automatic closing, which ZeroDDS does not perform (previous item).
### §7.2.3 An entity with a listener is still in use

**Spec:** §7.2.3, p. 16 — "• Any entity with a non-null listener is still in
use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Restriction on automatic closing, which ZeroDDS does not perform.
### §7.2.3 An explicitly retained object is still in use

**Spec:** §7.2.3, p. 16 — "• Any object that has been explicitly retained is
still in use."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.retain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.retain`)

**Tests:** —

**Status:** `n/a (informative)` — Restriction on automatic closing, which ZeroDDS does not perform. `Entity.retain()` exists; `EntityBase.retain` has no effect because nothing is closed automatically. (commit `5920d2fbe32`)
### §7.2.3 The creator of an object in use is still in use

**Spec:** §7.2.3, p. 16 — "• The creator of any object that is still in use is
itself still in use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Restriction on automatic closing, which ZeroDDS does not perform.
### §7.2.4 DataReader and DataWriter operations reentrant

**Spec:** §7.2.4, p. 16 — "All DataReader and DataWriter operations shall be
reentrant."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::aCallbackDoesNotHoldTheReaderMonitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::aCallbackDoesNotHoldTheWriterMonitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::concurrentEnableRegistersTheEndpointOnce`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::severalThreadsWriteTakeAndRegisterOnOneWriterAndReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::closingAWriterWhileOthersWriteLeavesOnlyAlreadyClosed`

**Status:** `done` — CoreConcurrencyTest calls `write`, `registerInstance`, `take` and `close` from several threads on one `DataWriter` and `DataReader`: four writer threads of 50 samples each, two registration threads, one `take` thread; the sum of the samples taken is 200. A second test closes the writer while four threads write and one thread calls `take`; only `AlreadyClosedException` remains. The code is unchanged (the `qos` field is `volatile`). (commit `171fae60383`)
### §7.2.4 Topic, Publisher, Subscriber and DomainParticipant operations reentrant, except `close`

**Spec:** §7.2.4, p. 16 — "All Topic (and other TopicDescription extension
interfaces), Publisher, Subscriber, and DomainParticipant operations shall be
reentrant with the exception that close may not be called on a given object
concurrently with any other call of any method on that object or on any
contained object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::severalThreadsCreateEntitiesAndSetTheQosOfOnePublisherAndSubscriber`

**Status:** `done` — The `qos` field of `PublisherImpl`, `SubscriberImpl`, `TopicImpl`, `DataWriterImpl` and `DataReaderImpl` is `volatile`. CoreConcurrencyTest creates writer, reader and topic ten times each from eight threads on one publisher, subscriber and participant while setting the QoS of publisher and subscriber; all 240 objects differ, the QoS left over is that of one of the threads. (commit `171fae60383`)
### §7.2.4 ServiceEnvironment and DomainParticipantFactory operations reentrant, except `DomainParticipantFactory.close`

**Spec:** §7.2.4, p. 16 — "All ServiceEnvironment and DomainParticipantFactory
operations shall be reentrant with the exception that
DomainParticipantFactory.close may not be called on a given object concurrently
with any other call of any method on that object or on any contained object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::severalThreadsGetOneParticipantFactoryOfAnEnvironment`

**Status:** `done` — `ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory` creates the factory with double-checked locking (`volatile` field, `synchronized`). CoreSpiTest calls `DomainParticipantFactory.getInstance(env)` in 20 rounds from eight threads started together each and checks that exactly one instance comes back. (commit `814bf0d160c`)
### §7.2.4 WaitSet and Condition operations reentrant

**Spec:** §7.2.4, p. 16 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception that
their close methods may not be called on a given object concurrently with any
other call of any method on that object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.setTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aSecondThreadWaitingOnTheSameWaitSetIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::conditionsAreAttachedAndDetachedWhileAThreadWaits`

**Status:** `done` — CoreConcurrencyTest attaches and detaches conditions, calls `getConditions` and `StatusCondition.setEnabledStatuses` in 100 rounds while another thread waits in `waitForConditions`; then `GuardCondition.setTriggerValue(true)` ends the wait and the active set contains the condition. The code is unchanged. (commit `76136ba8789`)
### §7.2.4 In a listener call only the entity that raised it

**Spec:** §7.2.4, p. 17 — "Code within a DDS listener callback may not safely
call any method on any DDS Entity but the one on which the status change
occurred."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:** —

**Status:** `n/a (informative)` — Restriction on the application code inside the callback; the specification asks nothing of the implementation here. ZeroDDS calls the callbacks without the monitor of the reader or writer (`DataReaderImpl.fireDataAvailable`), so calls from the callback remain possible; `ReentrancyTest.aCallbackDoesNotHoldTheReaderMonitor` checks this for `close()` from another thread. (commit `5920d2fbe32`)
### §7.2.4 Methods of value types may be non-reentrant

**Spec:** §7.2.4, p. 17 — "Any method of any value type may be non-reentrant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission.
### §7.2.4 Stronger guarantees permitted

**Spec:** §7.2.4, p. 17 — "A vendor may choose to provide stronger guarantees
than the rules above, but if so, those guarantees are unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission.
### §7.2.4 Rationale: `close` and listeners

**Spec:** §7.2.4, p. 17 — "Design Rationale (non-normative) • Objects that are
likely to “own” mutexes within their implementation need not permit close
invocations currently with other method invocations. [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.2.5 Names in camelCase

**Spec:** §7.2.5, p. 17 — "This PSM maps the underscore-formatted names of the
DDS PIM and IDL PSM (such as get_qos) into conventional Java “camel-case” names
(such as getQos)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.lookupInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getRequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getSubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.createParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::registerAndLookupInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::statusIsReadableWithoutAListener`

**Status:** `done` — The operations under `org/omg/dds` carry the PIM names in camel case; a search for method names with an underscore finds none. The tests call `registerInstance`, `lookupInstance`, `getRequestedDeadlineMissedStatus`, `getSubscriptionMatchedStatus`, `getPublicationMatchedStatus` and `createParticipant`. (commit `e0d499173ed`)
### §7.2.5 Mutators `set<PropertyName>` return the object

**Spec:** §7.2.5, p. 17 — "Properties defined by the DDS PIM are expressed as
sets of accessor and mutator methods. [...] Mutators are named set<PropertyName>
(for example, the mutator for a property “Foo” would be named setFoo.) They take
a single argument—the new value of the property—and return the enclosing object
in order to facilitate method chaining."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.setInt32Value`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString.setKey`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes.setValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes.KeyedStringImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::keyedStringAndKeyedBytesMutatorsReturnTheObject`

**Status:** `done` — CoreSpiTest chains `KeyedString.setKey(..).setValue(..)` and `KeyedBytes.setKey(..).setValue(..)` (both return the object) and checks copy and `copyFrom` behavior. The `void` mutators follow the OMG source (Annex B), which deviates from the text of 7.2.5. (commit `2601f4bf35a`)
### §7.2.5 Accessor `get<PropertyName>()` for unmodifiable values and references

**Spec:** §7.2.5, p. 17 — "Accessors for properties that are either of
unmodifiable objects (such as those of primitive types, primitive box types, or
strings) or pointers to the internal state of an object are named
get<PropertyName>. (For example, the accessor for an integer property “Foo”
would be named getFoo.) They take no arguments."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDomainId`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic.getName`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`

**Status:** `done` — The accessors for strings, numbers and references are named `get<PropertyName>` and take no argument (`DomainParticipant.getDomainId`, `Topic.getName`, `DataReader.getListener`, `Sample.getData`; declarations read). `ListenerTest.readerListenerFiresOnMatchAndOnEverySample` calls `getListener`, `getCurrentCount` and `getTotalCount`, `QuickstartApiTest.byteArrayQuickstartRoundTrip` calls `getData`. (commit `2601f4bf35a`)
### §7.2.5 Accessor `get<PropertyName>()` for policies, statuses, time and samples

**Spec:** §7.2.5, p. 17 — "Accessors for properties that are either of
unmodifiable objects [...] are named get<PropertyName>."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History.getKind`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History.getDepth`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus.getTotalCount`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time.getTime`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::timeFromMillisRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::statusIsReadableWithoutAListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::anRpcSampleIsAPsmSample`

**Status:** `done` — Policies, statuses, `Time`, `Duration`, `InstanceHandle` and `Sample` have accessors `get<PropertyName>` (`History.getKind`, `getDepth`, `SubscriptionMatchedStatus.getTotalCount`, `getCurrentCountChange`, `Time.getTime`, `Sample.getSampleState`, `getViewState`, `getInstanceState`, `getInstanceHandle`, `getSourceTimestamp`). The tests call these accessors and check the values. (commit `2601f4bf35a`)
### §7.2.5 Accessor for mutable properties with a pre-allocated object

**Spec:** §7.2.5, p. 17 — "Accessors for properties that are of mutable types,
and that may change asynchronously after they are retrieved, are named
get<PropertyName>. They take a pre-allocated object of the property type as
their first argument, the contents of which shall be overwritten by the method.
To facilitate method chaining, these methods also return a reference to this
argument. [...] An Entity’s status is an example of a property of this kind."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getSubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/KeyFields.java` (`KeyFields.copy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java` (`TopicTypeSupport.copyKey`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::getCurrentTimeFillsTheContainerOrReturnsANewOne`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::aWriterFillsTheKeyFieldsOfTheHolderOnly`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::aReaderFillsTheKeyFieldsOfTheHolderOnly`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::aHandleOfNoInstanceIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::lookupInstanceFillsTheHandleItIsGiven`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::theKeyHolderOfABuiltInKeyedTypeKeepsItsValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::aTypeWithoutAKeyHasNoKeyValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::lookupInstanceFillsAModifiableHandleOfAnotherImplementation`

**Status:** `done` — `DataWriterImpl.getKeyValue(T, InstanceHandle)` and `DataReaderImpl.getKeyValue(T, InstanceHandle)` overwrite only the key fields of `keyHolder` and return it (`TopicTypeSupport.copyKey`, default `KeyFields.copy` through `@Key`; without a `@Key` field all fields are copied; KeyedString/KeyedBytes copy `key`). The reader remembers the first sample with data for each instance (`Instance.keySample`). A handle without an instance gives `IllegalArgumentException`. `lookupInstance(ModifiableInstanceHandle, T)` fills the handle it is given, a foreign one too. (commit `d1d8ec15e5a`)
### §7.2.6 Extensions through own interfaces from the factory methods permitted

**Spec:** §7.2.6, p. 17 — "Implementation-specific extensions to the types
specified by this PSM are by definition unspecified. However, implementations
may provide such a capability by providing extended implementation-specific
interfaces and returning instances of these interfaces from the specified
factory methods."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission. ZeroDDS returns classes under `org.zerodds` from the factory methods (e.g. `DomainParticipantImpl`), without public additional interfaces of its own.
### §7.2.6 No extensions in `org.omg.dds` or its sub-packages

**Spec:** §7.2.6, p. 17 — "Implementations shall not place their extensions, if
any, in any interface or class in the package org.omg.dds or in any other
package whose name begins with that prefix."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java` (`RequesterImpl`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/ServerImpl.java` (`ServerImpl`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java` (`ZeroDdsParticipant`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java` (`RequesterImpl`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java` (`ReplierImpl`),
`runtime/java/src/main/java/org/zerodds/ddsrpc/ServerImpl.java` (`ServerImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsEqualTheOmgSourceExceptForTheRecordedDeviations`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/lang-rules/tests/java_psm_mapping.rs::an_interface_under_the_psm_mapping_compiles`

**Status:** `done` — The files under `crates/java-omgdds/java/src/main/java/org/omg/dds` are the files of the OMG source, without an exception; `CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource` checks the set of files and their SHA-256 on every run without the source being in the working tree. The binding puts no class of its own in `org.omg.dds`. The implementation classes of DDS-RPC (`RequesterImpl`, `ReplierImpl`, `ServerImpl`, `ClientParamsImpl`, `ServiceParamsImpl`, `RequesterParamsImpl`, `ReplierParamsImpl`, `Matching`, `ZeroDdsParticipant`, `RawTypeSupport`) are in `org.zerodds.ddsrpc` (`runtime/java/src/main/java/org/zerodds/ddsrpc`); the name `org.zerodds.rpc` belongs to the IDL-Java runtime. The interfaces and factory methods in `org.omg.dds.rpc` (`RequesterParams.create`, `RPCRuntime`) call them; the RPC code generator writes `extends org.zerodds.ddsrpc.RequesterImpl<…>` (`interface_java.rs`, `RPC_IMPL`). The tests of the RPC areas (`java_rpc_runtime`, `java_enhanced_codec`, `type_pair_java`, `interface_bindings`, `java_psm_mapping`) read the classes from the new path. (commit `0c7617b5c5b`)

---

## §7.3 Infrastructure Module

### §7.3 Two packages `org.omg.dds.core` and `org.omg.dds.core.policy`

**Spec:** §7.3, p. 18 — "This PSM realizes the Infrastructure Module from the
DDS specification with two packages: org.omg.dds.core and
org.omg.dds.core.policy. The latter contains all QoS policy classes, since a
given QoS policy may apply to multiple DDS Entity types. The former contains all
other Infrastructure types, including for example Entity and Condition base
interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::ddsObjectCarriesTheOneOperationTheClauseNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`

**Status:** `done` — `org.omg.dds.core` holds `DDSObject`, `Entity`, `ServiceEnvironment`, `InstanceHandle`, `Time`, `Duration`, `Condition`, `WaitSet`, `GuardCondition`, `StatusCondition`, `EntityQos`, `QosProvider` and the exceptions; `org.omg.dds.core.policy` holds the policies with `PolicyFactory`. The statuses are in `org.omg.dds.core.status`, the events in `org.omg.dds.core.event`, as in the OMG sources. The tests import and use the types from these packages; `RpcInheritanceTest` checks the package name of `DDSObject`. (commit `2601f4bf35a`)
### §7.3 Rationale: two packages, the name "core"

**Spec:** §7.3, p. 18 — "Design Rationale (non-normative) These two packages
have been made distinct from one another for two reasons: [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.3.1 `ServiceEnvironment` as the root of all objects, creation through an internal SPI

**Spec:** §7.3.1, p. 18 — "A ServiceEnvironment object represents an
instantiation of a Service implementation within a JVM. It is the “root” for all
other DDS objects and assists in their creation by means of an internal
service-provider interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getInstanceRejectsNullEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aNewInstanceHandleIsNilAndIsFilledByCopyFrom`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::theInvalidTimeAndANegativeTimeOfTheSpiAreInvalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aQosProviderOfAnUnsupportedUriSchemeIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aTypeSupportIsRegisteredUnderTheFullyQualifiedNameOfTheType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::keyedStringAndKeyedBytesMutatorsReturnTheObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::everyEntityAnswersWithTheEnvironmentItWasCreatedIn`

**Status:** `done` — CoreSpiTest calls `newInstanceHandle`, `invalidTime`, `newQosProvider`, `newTypeSupport`, `newKeyedString` and `newKeyedBytes` of the SPI and checks with `assertSame(env, x.getEnvironment())` participant, publisher, subscriber, topic, writer, reader, status condition, time and sample of an environment made by `new ServiceEnvironmentImpl()`. `DataWriterImpl` and `DataReaderImpl` take the environment of their parent (before: `Values.getDefaultEnvironment()`). (commit `6479f670a5b`)
### §7.3.1 All stateful types implement `DDSObject` (`getEnvironment`)

**Spec:** §7.3.1, p. 18 — "All stateful types in this PSM implement an interface
DDSObject, through a getServiceEnvironment method on which they can provide
access to the ServiceEnvironment from which they are ultimately derived."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSObject.java` (`DDSObject.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::ddsObjectCarriesTheOneOperationTheClauseNames`

**Status:** `done` — The operation is named `getEnvironment` in the OMG sources. `DDSObject` has exactly this one operation; the test checks name, return type `ServiceEnvironment`, parameter count 0 and the package `org.omg.dds.core`. Which types extend the interface is covered by the item "§7.3.1 Entities, factory and value types implement `DDSObject`". (commit `2601f4bf35a`)
### §7.3.1 Entities, factory and value types implement `DDSObject`

**Spec:** §7.3.1, p. 18 — "All stateful types in this PSM implement an interface
DDSObject [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getEnvironment`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl.getEnvironment`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::policiesKeepTheirValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aStatusKeepsItsCounts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::everyEntityAnswersWithTheEnvironmentItWasCreatedIn`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::ddsExceptionIsADdsObjectAndUnchecked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::anExceptionBelongsToTheEnvironmentItWasCreatedIn`

**Status:** `done` — `DDSException` has `implements DDSObject` like the OMG source; the concrete exceptions in `org.zerodds.core` return the environment given at construction or the default environment from `getEnvironment()`. CoreSpiTest checks `assertSame(env, x.getEnvironment())` for participant, publisher, subscriber, topic, writer and reader; CoreExceptionsTest checks it for the exceptions. (commit `16354365df6`)
### §7.3.1 `ServiceEnvironment` implements `DDSObject` and returns `this`

**Spec:** §7.3.1, p. 18 — "(ServiceEnvironment itself implements this interface;
a ServiceEnvironment always returns this from its getServiceEnvironment
operation.)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::aServiceEnvironmentReturnsItself`

**Status:** `done` — `ServiceEnvironment implements DDSObject`, `getEnvironment()` is `final` and returns `this`. The test checks assignability to `DDSObject` and that a subclass (`RPCRuntime`) returns itself. (commit `2601f4bf35a`)
### §7.3.1 Possible without static state, several implementations side by side

**Spec:** §7.3.1, p. 18 — "The ServiceEnvironment class allows implementations
to avoid the presence of static state, if desired. It also allows multiple DDS
implementations—or multiple versions of the “same” implementation—to potentially
coexist within the same Java run-time environment."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission and purpose. ZeroDDS holds static state: `ServiceEnvironmentImpl.DEFAULT`, `InProcessBus.INSTANCE`, `DurabilityService.INSTANCE`, `RtpsParticipant.BY_DOMAIN`, `ReflectionTypeSupport.CACHE`. Two `ServiceEnvironment`s in the same JVM therefore share bus and durability data (read from the code, not measured). The `DomainParticipantFactory`, in contrast, is separate per environment (item "§7.3.1 `ServiceEnvironment` as root of all objects").
### §7.3.1 The application's first step: create a `ServiceEnvironment`

**Spec:** §7.3.1, p. 18 — "A DDS application’s first step is to instantiate a
ServiceEnvironment, which represents the DDS implementation that it will use.
From there, it can create all of its additional DDS objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Describes the flow of an application. In ZeroDDS the step is required: `DomainParticipantFactory.getInstance(ServiceEnvironment)` has no counterpart without an argument in the OMG sources and throws a `NullPointerException` for `null` (`QuickstartApiTest.getInstanceRejectsNullEnvironment`).
### §7.3.1 `ServiceEnvironment` is abstract

**Spec:** §7.3.1, p. 18 — "The ServiceEnvironment class is abstract."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::theServiceEnvironmentIsAbstract`

**Status:** `done` — CoreSpiTest checks `Modifier.isAbstract(ServiceEnvironment.class.getModifiers())`, that `getSPI` is abstract and that `ServiceEnvironmentImpl` is a concrete subclass. (commit `2601f4bf35a`)
### §7.3.1 `createInstance` looks up the subclass through a system property

**Spec:** §7.3.1, p. 18 — "To avoid compile-time dependencies on concrete
ServiceEnvironment implementations, an application can instantiate a
ServiceEnvironment by means of a static createInstance method on the
ServiceEnvironment class. This method looks up a concrete ServiceEnvironment
subclass using a Java system property containing the name of that subclass."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.createInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentRejectsUnknownClass`

**Status:** `done` — `createInstance(ClassLoader)` reads the system property `org.omg.dds.serviceClassName`; `createInstance(String, Map, ClassLoader)` reads a freely named property and throws `ServiceConfigurationException` if it is missing or the class cannot be loaded. The file is byte-identical to the OMG source. The first test sets the property to `org.zerodds.ServiceEnvironmentImpl` and gets a usable environment, the second checks the unknown class; no test covers a missing property. (commit `2601f4bf35a`)
### §7.3.1 Subclass with its own name from the implementation

**Spec:** §7.3.1, p. 18 — "This subclass must be provided by implementers and
will therefore have an implementation-specific name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`

**Status:** `done` — `org.zerodds.ServiceEnvironmentImpl extends ServiceEnvironment` has a public no-argument constructor and one with `Map`. The test loads the class by this name through `createInstance` and uses the result; it does not check the runtime type explicitly. (commit `96bccc52ca9`)
### §7.3.1 Factory method for DynamicTypeFactory

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: DynamicTypeFactory, WaitSet, GuardCondition, TypeSupport,
Time, Duration, and InstanceHandle."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.getTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getTypeFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aTypeDescriptorCarriesTheTypesItNames`

**Status:** `done` — `ServiceProviderInterface.getTypeFactory` returns the `DynamicTypeFactoryImpl` of the environment, `DynamicTypeFactory.getInstance(env)` calls it. The first test calls `getTypeFactory().getPrimitiveType(INT_32_TYPE)`, the second creates a sequence type through `DynamicTypeFactory.getInstance(ENV)`. (commit `96bccc52ca9`)
### §7.3.1 Factory method for WaitSet

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] WaitSet [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aWaitWithATimeoutEndsInATimeoutException`

**Status:** `done` — `ServiceProviderInterface.newWaitSet` and `WaitSet.newWaitSet(env)` return a `WaitSetImpl`. The first test checks that `newWaitSet()` returns an object; `WaitSetTest` uses `WaitSet.newWaitSet(ENV)` and checks attaching, detaching, waiting with a time limit (`TimeoutException`) and being woken by a condition. (commit `76136ba8789`)
### §7.3.1 Factory method for GuardCondition

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] GuardCondition [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`

**Status:** `done` — `ServiceEnvironment.ServiceProviderInterface.newGuardCondition` returns a `GuardConditionImpl`; `GuardCondition.newGuardCondition(env)` delegates to it. The tests check that the factory returns an object, that the condition is `false` at first and that setting it from another thread ends a blocked `WaitSet`. (commit `76136ba8789`)
### §7.3.1 Factory method for TypeSupport

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] TypeSupport [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aTypeSupportIsRegisteredUnderTheFullyQualifiedNameOfTheType`

**Status:** `done` — `org/omg/dds/type/TypeSupport.newTypeSupport(Class, ServiceEnvironment)` calls the source with `type.getClass().getName()`, which is "java.lang.Class" for every type; `ServiceEnvironmentImpl.newTypeSupport` reads that name as "no name given" and registers the type under `type.getName()`. CoreSpiTest creates a TypeSupport through `TypeSupport.newTypeSupport` and `ServiceProviderInterface.newTypeSupport` and checks the name (`[B` for `byte[]`, the given name otherwise). The file is that of the OMG source (`CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`). (commit `814bf0d160c`)
### §7.3.1 Factory method for Time

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] Time [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newTime`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.invalidTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.split`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.shift`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::getCurrentTimeFillsTheContainerOrReturnsANewOne`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aNegativeInstantInAnyUnitIsInvalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::theInvalidTimeEqualsItselfAndOrdersBeforeEveryValidTime`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::addAndSubtractMoveTheInstantAndKeepItNormalised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::theInvalidTimeAndANegativeTimeOfTheSpiAreInvalid`

**Status:** `done` — `TimeImpl.split` returns the invalid instant (seconds -1, nanoseconds 0) for every negative value in every unit; `ModifiableTimeImpl.add/subtract` compute through `TimeImpl.shift` with normalization and an invalid result before the epoch. Tests: `newTime(-5, u)` invalid for all `TimeUnit` values, `invalidTime()` invalid, `newTime(0, NANOSECONDS)` valid. (commit `d9fb6db1612`)
### §7.3.1 Factory method for Duration

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] Duration [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newDuration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.infiniteDuration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.zeroDuration`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::durationInfiniteRecognized`

**Status:** `done` — `newDuration(long, TimeUnit)`, `infiniteDuration()` and `zeroDuration()` return `DurationImpl` objects of the environment; `DurationImpl.of` maps the magnitude `Long.MAX_VALUE` to infinite. The tests check `newDuration(7, SECONDS)` after serialization, `isInfinite()` and `isZero()` of the two factories, and `getDuration(SECONDS) == Long.MAX_VALUE` for infinite. (commit `8a5744821fd`)
### §7.3.1 Factory method for InstanceHandle

**Spec:** §7.3.1, p. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] and InstanceHandle."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.nilHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newInstanceHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::instanceHandleNilDetected`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aNewInstanceHandleIsNilAndIsFilledByCopyFrom`

**Status:** `done` — CoreSpiTest creates a `ModifiableInstanceHandle` through `newInstanceHandle()`, checks `isNil()`, fills it with `copyFrom` (value 42 in the fourth byte) and checks equality, not nil and a different object for each call. (commit `8a5744821fd`)
### §7.3.1 Helper functions `allStatusKinds` and `noStatusKinds`

**Spec:** §7.3.1, p. 18 — "It also provides helper functions allStatuses and
noStatuses to create special instances of Status objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.allStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.noStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.allStatusKinds`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`

**Status:** `done` — The helper functions are named `allStatusKinds()` and `noStatusKinds()` and sit on the `ServiceProviderInterface`. `allStatusKinds` holds the 13 status classes (`ServiceEnvironmentImpl.allStatusKinds`), `noStatusKinds` is empty. The test checks size 13 and the empty set; it does not check the membership of the single classes. (commit `96bccc52ca9`)
### §7.3.2 ReturnCode_t as Java exceptions

**Spec:** §7.3.2, p. 18 — "The PSM maps the ReturnCode_t type from the DDS PIM
into a combination of standard Java exceptions (where their semantics match
those expressed in the PIM) and new exception classes defined by this PSM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/AlreadyClosedException.java` (`AlreadyClosedException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/NotEnabledException.java` (`NotEnabledException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/PreconditionNotMetException.java` (`PreconditionNotMetException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::writeOnAClosedWriterThrowsAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNotEnabledWriterRefusesToWrite`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNullArgumentIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aWaitWithATimeoutEndsInATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredParticipantDataCarriesTheUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::theServiceProvidesAConcreteClassForEveryTable71ExceptionAndForRetcodeError`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::aSampleTheEncoderRejectsFailsWithRetcodeError`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::anEncodingBeyondTheLimitFailsWithOutOfResources`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEnabledWriterRefusesToChangeAnImmutablePolicy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aHistoryDepthAboveMaxSamplesPerInstanceIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aTopicOfAnotherImplementationIsUnsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::resumePublicationsHasToMatchASuspend`

**Status:** `done` — The mapping of Table 7.1 through exception classes (`org.omg.dds.core.*Exception`, concrete classes in `org.zerodds.core`) has a test for every row: ERROR, UNSUPPORTED, IMMUTABLE_POLICY, INCONSISTENT_POLICY, OUT_OF_RESOURCES, PRECONDITION_NOT_MET, ALREADY_DELETED, NOT_ENABLED, BAD_PARAMETER. ILLEGAL_OPERATION has the class and the reflection test but no operation that raises it (DDS 1.4 names none). TIMEOUT: see the item on RETCODE_TIMEOUT (partial). (commit `16354365df6`)
### §7.3.2 All exceptions unchecked except `TimeoutException`

**Spec:** §7.3.2, p. 18 — "With the exception of
java.util.concurrent.TimeoutException, all exceptions are unchecked (that is,
they extend java.lang.RuntimeException directly or indirectly)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceConfigurationException.java` (`ServiceConfigurationException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceInitializationException.java` (`ServiceInitializationException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::everyPsmExceptionIsAnUncheckedDdsException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::onlyTimeoutExceptionIsChecked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::ddsExceptionIsADdsObjectAndUnchecked`

**Status:** `done` — CoreExceptionsTest checks for all seven classes of Table 7.1, `DDSException`, `ServiceConfigurationException` and `ServiceInitializationException` that they extend `RuntimeException`, and that `TimeoutException` (`java.util.concurrent`) is checked and declared by `WaitSet.waitForConditions`, `DataWriter.write` and `DomainParticipant.findTopic`. (commit `2930862439a`)
### §7.3.2 Own exceptions based on `DDSException` in `org.omg.dds.core`, abstract

**Spec:** §7.3.2, p. 19 — "The exception classes defined by this PSM extend the
base class DDSException. All of the PSM-defined exception classes are defined in
the package org.omg.dds.core. All of these classes are abstract so as not to
specify the representation of state; implementations shall provide concrete
implementations."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/AlreadyClosedException.java` (`AlreadyClosedException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/IllegalOperationException.java` (`IllegalOperationException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/NotEnabledException.java` (`NotEnabledException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/PreconditionNotMetException.java` (`PreconditionNotMetException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ImmutablePolicyException.java` (`ImmutablePolicyException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/InconsistentPolicyException.java` (`InconsistentPolicyException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/OutOfResourcesException.java` (`OutOfResourcesException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::everyPsmExceptionIsAnUncheckedDdsException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::everyTable71ExceptionIsAnAbstractDdsExceptionOfOrgOmgDdsCore`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::theExceptionsKeepTheSerialVersionUidOfTheOmgSource`

**Status:** `done` — CoreExceptionsTest checks by reflection that the seven classes of Table 7.1 are abstract, lie in `org.omg.dds.core` and extend `DDSException` directly, that `DDSException` is abstract and that `org.zerodds.core` has a concrete class for each. The `serialVersionUID` of all exceptions of the OMG API now equals the OMG source (before: `-1L` to `-5L` on five classes and another value on `ServiceInitializationException`). (commit `16354365df6`)
### §7.3.2 Table 7.1: RETCODE_OK

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_OK | Normal return; no exception."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — `DataWriter.write` is `void`; a successful call returns without an exception. The test writes two samples without an exception and takes both at the reader. (commit `9426325d8b2`)
### §7.3.2 Table 7.1: RETCODE_NO_DATA

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_NO_DATA | An informational state
(e.g., a Boolean result) attached to a normal return; no exception."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::volatileDoesNotReplayToLateJoiner`

**Status:** `done` — Without samples `take(List)` returns an empty list without an exception; there is no `ReturnCode.NO_DATA`. The test takes at a running reader without data and checks length 0. (commit `9426325d8b2`)
### §7.3.2 Table 7.1: RETCODE_ERROR

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_ERROR | DDSException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/ErrorException.java` (`ErrorException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.serialize`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::aSampleTheEncoderRejectsFailsWithRetcodeError`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::theServiceProvidesAConcreteClassForEveryTable71ExceptionAndForRetcodeError`

**Status:** `done` — `org.zerodds.core.ErrorException` is the concrete class of `DDSException` (RETCODE_ERROR). `DataWriterImpl.serialize` reports an encoder that rejects the sample (`XcdrException`) as `ErrorException` with the cause. Tests: the class is concrete with the superclass `DDSException`; a writer with such a type support throws `ErrorException` and stays usable. (commit `6479f670a5b`)
### §7.3.2 Table 7.1: RETCODE_BAD_PARAMETER

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_BAD_PARAMETER |
java.lang.IllegalArgumentException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/HistoryImpl.java` (`HistoryImpl.withDepth`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::historyDepthMustBeNonNegative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::instanceHandleRejectsWrongLength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNullArgumentIsABadParameter`

**Status:** `done` — A negative `History` depth, a handle of the wrong length and a `null` handle in `getMatchedSubscriptionData` throw `IllegalArgumentException`. The three tests check exactly this exception; further tests check it for invalid filter expressions, type descriptors and MultiTopic expressions (`ContentFilterTest`, `DynamicTypeReflectionTest`, `MultiTopicTest`). (commit `9426325d8b2`)
### §7.3.2 Table 7.1: RETCODE_BAD_PARAMETER for `null`

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_BAD_PARAMETER |
java.lang.IllegalArgumentException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublicationData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/Args.java` (`Args.require`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNullArgumentIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getInstanceRejectsNullEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::aForeignHandleThatNamesNoInstanceOfTheWriterIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/NullArgumentTest.java::aNullArgumentIsIllegalArgumentExceptionAndNeverNullPointerException`

**Status:** `done` — `getMatchedSubscriptionData(null)` and `getMatchedPublicationData(null)` throw `IllegalArgumentException`. `DomainParticipantFactory.getInstance(null)` throws `NullPointerException` (source of the OMG API: `env.getSPI()`); `QuickstartApiTest.getInstanceRejectsNullEnvironment` fixes that. A handle that names no instance of the writer is `IllegalArgumentException` (`CoreForeignValuesTest::aForeignHandleThatNamesNoInstanceOfTheWriterIsABadParameter`, DDS 1.4 §2.2.2.4.2.11). Every operation of DomainParticipant, Publisher, Subscriber, Topic, DataWriter and DataReader with an object argument throws `IllegalArgumentException` (Table 7.1: RETCODE_BAD_PARAMETER) for `null` and no `NullPointerException` (`org.zerodds.core.Args.require` at the entry of the operations: names, types, QoS, topics, handles, timestamps, units, conditions, samples). `null` keeps a meaning where DDS or the PSM gives it one: a listener and a status set of "none", a key holder or a list that is created, the nil handle in `write`, `dispose` and `unregisterInstance`, `null` as the selector of `read` and `take`. `NullArgumentTest` calls through reflection every operation of these six interfaces with `null` for each parameter (109 calls) and demands `IllegalArgumentException`. (commit `e552b0cbaf4`)
### §7.3.2 Table 7.1: RETCODE_TIMEOUT

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_TIMEOUT |
java.util.concurrent.TimeoutException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.findTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.waitForHistoricalData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitWireHistorySpace`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.awaitHistorySpace`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.awaitHistoricalData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aWaitWithATimeoutEndsInATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::findTopicReturnsTheTopicAndTimesOutOtherwise`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aSampleNobodyAcknowledgesIsATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::aWriteBeyondMaxSamplesWaitsForAcknowledgmentsAndTimesOut`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::theCallIsATimeoutExceptionWhileTheWriterAnnouncedNothing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::keepAllWithMaxSamplesPerInstanceTimesOutForTransientLocal`

**Status:** `done` — `java.util.concurrent.TimeoutException` is thrown by `WaitSetImpl.waitForConditions(Duration)`, `DomainParticipantImpl.findTopic`, `DataWriterImpl.waitForAcknowledgments` and `PublisherImpl.waitForAcknowledgments` (readers in other processes acknowledge with ACKNACK), `DataReaderImpl.waitForHistoricalData` (RTPS branch) and by `write` when RESOURCE_LIMITS are reached and `max_blocking_time` runs out: for a TRANSIENT_LOCAL writer on the samples it keeps, for a reliable KEEP_ALL writer with `max_samples` on the samples that readers in other processes have not acknowledged yet (`WireWriter.awaitHistorySpace`). A BEST_EFFORT writer does not wait and reports `OutOfResourcesException`. (commit `e6b419f65f1`)
### §7.3.2 Table 7.1: RETCODE_UNSUPPORTED

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_UNSUPPORTED |
java.lang.UnsupportedOperationException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.codecOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aTopicOfAnotherImplementationIsUnsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aTopicDescriptionOfAnotherImplementationIsUnsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::aQosProviderOfAnUnsupportedUriSchemeIsRefused`

**Status:** `done` — `UnsupportedOperationException` is tested for a URI scheme other than `file` (`QosProviderImpl`), for a topic and a topic description of another implementation (`PublisherImpl.codecOf`, `SubscriberImpl.codecOf`) and for a content-filtered topic of another implementation. `SubscriberImpl.createDataReader` threw `AlreadyClosedException` for the latter; it is now `UnsupportedOperationException`. (commit `611135ecef4`)
### §7.3.2 Table 7.1: RETCODE_ALREADY_DELETED

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_ALREADY_DELETED |
AlreadyClosedException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/AlreadyClosedException.java` (`AlreadyClosedException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::writeOnAClosedWriterThrowsAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::readAndTakeOnAClosedReaderThrowAlreadyClosed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aClosedEntityRefusesToCreateContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantFactoryTest.java::createPublisherOnClosedParticipantThrows`

**Status:** `done` — After `close()`, `write`, `read`, `take`, `createDataWriter`, `createDataReader`, `createPublisher` and `createTopic` throw `AlreadyClosedException` (subclass of `org.omg.dds.core.AlreadyClosedException`). The tests check the exception for closed writers, readers, publishers, subscribers and participants. (commit `9426325d8b2`)
### §7.3.2 Table 7.1: RETCODE_ILLEGAL_OPERATION

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_ILLEGAL_OPERATION |
IllegalOperationException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.endCoherentChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.endAccess`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::resumePublicationsHasToMatchASuspend`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::endCoherentChangesHasToMatchABeginAndTheCallsNest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::endAccessHasToMatchABeginAccess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::theServiceProvidesAConcreteClassForEveryTable71ExceptionAndForRetcodeError`

**Status:** `done` — Per DDS 1.4 2.2.2.4.1.9, 2.2.2.4.1.11 and 2.2.2.5.2.9, `resume_publications` without `suspend_publications`, `end_coherent_changes` without `begin_coherent_changes` and `end_access` without `begin_access` are PRECONDITION_NOT_MET, and `suspend_publications` and `begin_coherent_changes` can be nested; the code threw `IllegalOperationException`. `PublisherImpl` now counts the depth and throws `PreconditionNotMetException`, `SubscriberImpl.endAccess` likewise. ILLEGAL_OPERATION: the class `org.zerodds.core.IllegalOperationException` and the mapping exist and are tested by reflection; no operation of the binding raises it, DDS 1.4 names none. (commit `611135ecef4`)
### §7.3.2 Table 7.1: RETCODE_NOT_ENABLED

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_NOT_ENABLED | NotEnabledException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.ensureEnabled`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/NotEnabledException.java` (`NotEnabledException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aNotEnabledWriterRefusesToWrite`

**Status:** `done` — A `DataWriter` that is not enabled throws `NotEnabledException` from `write`, `dispose` and `unregisterInstance`, a `DataReader` that is not enabled from `read` and `take` (`DataReaderImpl.ensureEnabled`). The test checks `write` on a writer without `enable`. `dispose`, `unregisterInstance` and the reader are not tested. (commit `9426325d8b2`)
### §7.3.2 Table 7.1: RETCODE_PRECONDITION_NOT_MET

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_PRECONDITION_NOT_MET |
PreconditionNotMetException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.detachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipantData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/PreconditionNotMetException.java` (`PreconditionNotMetException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredParticipantDataCarriesTheUserData`

**Status:** `done` — `PreconditionNotMetException` is thrown when a condition is detached from a `WaitSet` it is not attached to, for `getDiscoveredParticipantData` with an unknown handle and for `getKeyValue` on a type without a key. The two tests check the first two cases. (commit `2930862439a`)
### §7.3.2 Table 7.1: RETCODE_IMMUTABLE_POLICY

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_IMMUTABLE_POLICY |
ImmutablePolicyException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkChange`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEnabledWriterRefusesToChangeAnImmutablePolicy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEnabledWriterAcceptsTheChangeOfAChangeablePolicy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEnabledReaderRefusesImmutableAndAcceptsChangeablePolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEnabledTopicRefusesImmutableAndAcceptsChangeablePolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::anEntityThatIsNotEnabledAcceptsAnyChange`

**Status:** `done` — `QosRules.checkChange` checks in `DataWriterImpl.setQos`, `DataReaderImpl.setQos` and `TopicImpl.setQos` of an enabled object only the policies with Changeable = No (DDS 1.4 2.2.3; plus DATA_REPRESENTATION and TYPE_CONSISTENCY_ENFORCEMENT per XTypes 1.3): DURABILITY, DURABILITY_SERVICE, PRESENTATION, OWNERSHIP, LIVELINESS, RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS. A change of TRANSPORT_PRIORITY, USER_DATA or TIME_BASED_FILTER is accepted. An object that is not enabled accepts any change. `PublisherImpl.setQos`, `SubscriberImpl.setQos` and `DomainParticipantImpl.setQos` check with `QosRules.checkChange` as well. (commit `e788a4434b4`)
### §7.3.2 Table 7.1: RETCODE_INCONSISTENT_POLICY

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_INCONSISTENT_POLICY |
InconsistentPolicyException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InconsistentPolicyException.java` (`InconsistentPolicyException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aHistoryDepthAboveMaxSamplesPerInstanceIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::maxSamplesBelowMaxSamplesPerInstanceIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aMinimumSeparationAboveTheDeadlinePeriodIsInconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::aTopicWithAnInconsistentQosIsRefused`

**Status:** `done` — `QosRules.checkConsistent` is called on creation (constructor of `TopicImpl`, `DataWriterImpl`, `DataReaderImpl`) and in `setQos` and throws `InconsistentPolicyException` for a History KEEP_LAST depth above `max_samples_per_instance`, `max_samples` below `max_samples_per_instance` and `minimum_separation` above the deadline period. A limit of `LENGTH_UNLIMITED` and an infinite deadline period are not compared. Publisher, subscriber and participant QoS contain none of these policies. The tests check creation and `setQos` for each rule, that the refused QoS does not replace the old one, and the boundary of equality. (commit `e788a4434b4`)
### §7.3.2 Table 7.1: RETCODE_OUT_OF_RESOURCES

**Spec:** §7.3.2, p. 19, Table 7.1 — "RETCODE_OUT_OF_RESOURCES |
OutOfResourcesException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/OutOfResourcesException.java` (`OutOfResourcesException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.serialize`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::anEncodingBeyondTheLimitFailsWithOutOfResources`

**Status:** `done` — `DataWriterImpl.serialize(support, data, maxBytes)` grows the buffer up to `MAX_SAMPLE_BYTES` (256 MiB) and above that throws `OutOfResourcesException` with the `BufferOverflowException` as cause. The test sets the limit to 1024 bytes: 1000 bytes fit (result length 1000), 2000 bytes give the exception. (commit `6479f670a5b`)
### §7.3.2 Exceptions instead of `nil` for operations that return an object

**Spec:** §7.3.2, p. 19 — "In addition, this PSM permits implementations to
throw exceptions to indicate errors in operations that in the PIM return an
object reference."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::aClosedEntityRefusesToCreateContainedEntities`

**Status:** `n/a (informative)` — A permission, not an obligation. ZeroDDS uses it: `create*` throws `AlreadyClosedException` on a closed creator (`ExceptionMappingTest.aClosedEntityRefusesToCreateContainedEntities`) and `IllegalArgumentException` for invalid input. (commit `9426325d8b2`)
### §7.3.2 Rationale: fault and contingency

**Spec:** §7.3.2, p. 19 — "Design Rationale (non-normative) • This PSM uses
checked and unchecked exceptions according to the following rationale: [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.3.3 Value types implement `org.omg.dds.core.Value`

**Spec:** §7.3.3, p. 20 — "All DDS types with value semantics implement the
interface org.omg.dds.core.Value. These include QoS, QoS policy, status, time,
and other types."

**Repo:** —

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::timesAreSerializableAndKeepTheirValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::durationsAreSerializableAndKeepTheirValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::handlesAreSerializableAndKeepTheirOctets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::timeDurationAndHandlesAreCopiedByValue`

**Status:** `done` — The normative archive `omgdds_src.zip` contains no `org.omg.dds.core.Value` (the files of `org/omg/dds/core` are listed in Annex B); by §2 the source prevails when text and source conflict, and a new type in `org.omg.dds` would be an extension under §7.2.6. The types with value semantics (Time, Duration, InstanceHandle, QoS, QoS policy, status) have the properties §7.3.3 attributes to the interface: they are `Serializable` and `Cloneable` (`clone()` returns a copy by value), `equals` and `hashCode` follow the state, and `copyFrom` overwrites the state of the target in the Modifiable types (`ValueCopyTest`, `CoreValuesTest`). (commit `c286150d0e5`)
### §7.3.3 `Value` extends `Cloneable` and `Serializable`

**Spec:** §7.3.3, p. 20 — "The Value interface extends the standard Java SE
interfaces java.lang.Cloneable and java.io.Serializable, allowing objects of
implementing types to be copied by value as well as serialized and deserialized
using built-in Java mechanisms."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Duration.java` (`Duration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/InstanceHandle.java` (`InstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/QosPolicy.java` (`QosPolicy`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/Status.java` (`Status`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.clone`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.clone`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::policiesKeepTheirValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aStatusKeepsItsCounts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::everyPolicyOfThePolicyFactoryIsCopiedByValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::qosAndStatusObjectsAreCopiedByValue`

**Status:** `done` — `Time`, `Duration`, `InstanceHandle`, `QosPolicy`, `Status` and `EntityQos` are `Serializable` (the tests serialize time, duration, policies and a status and compare the value after the return) and their classes in `org.zerodds` are `Cloneable` with a public `clone()`: `ValueCopyTest` copies time, duration, handle, every policy of the `PolicyFactory`, the QoS classes and the statuses of entities and checks that the copy is equal but another object and that the copy of a `ModifiableTime` does not follow later changes of the source. The interfaces of the OMG source themselves do not extend `Cloneable` (no `Value`, see the item before). (commit `c286150d0e5`)
### §7.3.3 `copyFrom` overwrites the state of the target

**Spec:** §7.3.3, p. 20 — "It defines a method copyFrom that accepts a source
object of the same type as the object itself. This method overwrites the state
of the target object (“this”) with the state of the argument object. It is
similar to clone but does not require allocating a new object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ModifiableTime.java` (`ModifiableTime.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ModifiableInstanceHandle.java` (`ModifiableInstanceHandle.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/ModifiableTimeImpl.java` (`ModifiableTimeImpl.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicKeyImpl.java` (`BuiltinTopicKeyImpl.copyFrom`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::copyFromOverwritesTheStateAndCopiesAnInvalidSourceAsInvalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::copyFromOverwritesTheOctetsAndAnEarlierCopyKeepsItsValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aBuiltinTopicKeyCopiesItsWordsAndIsBuiltFromAHandleBigEndian`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::aNullHandleIsABadParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::keyedStringAndKeyedBytesMutatorsReturnTheObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::copyFromOverwritesTheWholeStateOfThePublicationData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::copyFromOverwritesTheWholeStateOfTheSubscriptionData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::copyFromOverwritesTheWholeStateOfTheTopicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::copyFromOverwritesTheStateOfTheParticipantData`

**Status:** `done` — `ModifiableTimeImpl.copyFrom` copies an invalid source as an invalid time and rejects `null` with `IllegalArgumentException`; `ModifiableInstanceHandleImpl.copyFrom` accepts foreign values. `copyFrom` of the four built-in topic data types overwrites the whole state of the target, also what only this implementation holds (the TypeInformation and TypeObject octets, which are cleared for a foreign source); `BuiltinDataCopyFromTest` compares target and source by their serialization as built-in data. Also tested are `ModifiableTime` (valid, invalid, foreign), `ModifiableInstanceHandle`, `BuiltinTopicKey` and `KeyedString`. (commit `4e65c3aae80`)
### §7.3.3 `equals` and `hashCode` on time, handle, policies and QoS

**Spec:** §7.3.3, p. 20 — "Value implementers are also expected to override
their inherited implementations of Object.equals and Object.hashCode in order to
enforce value semantics."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReliabilityImpl.java` (`ReliabilityImpl.equals`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::policiesKeepTheirValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::equalTimesHaveEqualHashCodeAndCompareAsEqual`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::equalDurationsHaveEqualHashCodeAndOrderByLength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::handlesWithTheSameOctetsAreEqualAndHaveTheSameHashCode`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::equalPoliciesHaveEqualHashCodes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::equalQosHaveEqualHashCodes`

**Status:** `done` — CoreValuesTest and CoreStatusPolicyTest check that equal Time, Duration, InstanceHandle, policy and entity QoS objects have equal `hashCode` (every factory method of `PolicyFactory` called twice, equal and same hash). `equals` across implementation boundaries (foreign Time, Duration, handle) is tested as well. The status classes are covered in the item on equals and hashCode of statuses. (commit `8a5744821fd`)
### §7.3.3 `equals` and `hashCode` on status

**Spec:** §7.3.3, p. 20 — "Value implementers are also expected to override
their inherited implementations of Object.equals and Object.hashCode in order to
enforce value semantics."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedIncompatibleQosStatusImpl.java` (`OfferedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/PublicationMatchedStatusImpl.java` (`PublicationMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessChangedStatusImpl.java` (`LivelinessChangedStatusImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aStatusKeepsItsCounts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::everyStatusComparesByItsCountersAndHandles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::statusesWithoutACounterAreEqualWithinTheirKindOnly`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::theHandleOfAStatusIsComparedByValueNotByIdentity`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::theIncompatiblePoliciesOfAStatusCannotBeChanged`

**Status:** `done` — All 13 status classes in `org.zerodds.core.status` override `equals` and `hashCode` over counters, handles, policy class and policy sets; `getPolicies()` of the two incompatible-QoS statuses returns an unmodifiable set. CoreStatusPolicyTest compares for each class equal values (equal, same hash) and one differing value (not equal); handle objects of different identity with the same bytes give equal statuses. (commit `63e0d55e146`)
### §7.3.3 QoS policies immutable

**Spec:** §7.3.3, p. 20 — "QoS policy objects are immutable. New policy objects
can be created from existing policy objects by using the QoS DSL described in
sub clause 7.3.5.3."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PartitionImpl.java` (`PartitionImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/UserDataImpl.java` (`UserDataImpl.getValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::userDataKeepsACopyOfTheArrayItWasGivenAndHandsOutCopies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::topicDataAndGroupDataHandOutCopies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::withValueLeavesTheOriginalUnchanged`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::aPartitionCannotBeChangedThroughItsNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::withPolicyLeavesTheOriginalQosUnchanged`

**Status:** `done` — `UserDataImpl`, `TopicDataImpl` and `GroupDataImpl` copy the array in the constructor and in `getValue`; `PartitionImpl.getName` returns an unmodifiable view. CoreStatusPolicyTest checks that a changed source or returned array does not change the value, that `withValue`/`withName`/`withPolicy` leave the original unchanged and that `Partition.getName().add` gives an `UnsupportedOperationException`. (commit `63e0d55e146`)
### §7.3.3 The term value type

**Spec:** §7.3.3, p. 20, footnote 2 — "The term “value type” refers to any data
type for which object identity is considered to be established solely based on
the state of the objects of that type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definition of a term.
### §7.3.4 Time_t and Duration_t as value types `Time` and `Duration`

**Spec:** §7.3.4, p. 20 — "This PSM maps the DDS Time_t and Duration_t types
into the value types Time and Duration respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Duration.java` (`Duration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::timeFromMillisRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::durationInfiniteRecognized`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::timeAndDurationKeepTheirValueAndGetAnEnvironment`

**Status:** `done` — `Time` and `Duration` are classes in `org.omg.dds.core` with seconds and nanoseconds, value semantics (`equals`, `hashCode`, `Comparable`, `Serializable`) and the implementations `TimeImpl` and `DurationImpl`. The tests check the conversion from milliseconds, the infinite duration and the value comparison after serialization. The common base `Value` is covered by the item "§7.3.3 Value types implement `org.omg.dds.core.Value`". (commit `8a5744821fd`)
### §7.3.4 Magnitude in units of `TimeUnit`

**Spec:** §7.3.4, p. 20 — "These classes can provide their magnitude using a
variety of units (expressed using java.util.concurrent.TimeUnit)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.getTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.getDuration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::timeFromMillisRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aBuiltinReaderSeesTheQosOfTheAnnouncedWriter`

**Status:** `done` — `Time.getTime(TimeUnit)`, `Time.getRemainder(TimeUnit, TimeUnit)` and `Duration.getDuration(TimeUnit)` return the magnitude in the chosen unit. One test checks `getTime` in seconds and milliseconds and `getRemainder` in nanoseconds; a second checks `getDuration(SECONDS)` on two policies (5 and 7). (commit `8a5744821fd`)
### §7.3.4 Rationale: names without `_t`

**Spec:** §7.3.4, p. 20 — "Design Rationale (non-normative) The names of these
types omit the underscore and ‘t’ characters from the ends of their names."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.3.5 Policies based on `org.omg.dds.core.policy.QosPolicy`

**Spec:** §7.3.5, p. 20 — "This PSM represents the former with the base
interface org.omg.dds.core.policy.QosPolicy and the latter with the base
interface org.omg.dds.core.EntityQos."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/QosPolicy.java` (`QosPolicy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::qosCompatibilityReliableReaderRequiresReliableWriter`

**Status:** `done` — `org.omg.dds.core.policy.QosPolicy` is the base of all policy interfaces (`Reliability`, `History`, `UserData` …), with the marker interfaces `ForDataReader`, `ForDataWriter` and so on. The tests compile and use the policies through `EntityQos<P extends QosPolicy>` (`get(Reliability.class)`, `withPolicy`). (commit `2601f4bf35a`)
### §7.3.5 QoS of an entity based on `org.omg.dds.core.EntityQos`

**Spec:** §7.3.5, p. 20 — "[...] and the latter with the base interface
org.omg.dds.core.EntityQos."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`

**Status:** `done` — `org.omg.dds.core.EntityQos` is the base of `DomainParticipantQos`, `PublisherQos`, `SubscriberQos`, `TopicQos`, `DataWriterQos` and `DataReaderQos`; `QosProfile` no longer exists. The tests read policies with `get(Class)` from reader, topic and writer QoS and compare QoS objects with `assertEquals`. (commit `96bccc52ca9`)
### §7.3.5.1 Table 7.2: policy as an interface with Java Bean properties

**Spec:** §7.3.5.1, p. 21, Table 7.2 — "QoS policy structure containing the
state of an instance of that policy. | QoS policy interface extending
org.omg.dds.core.policy.QosPolicy. Each policy provides Java Bean-style
properties."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Durability.java` (`Durability`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`

**Status:** `done` — The policies are interfaces extending `QosPolicy` with Java Bean properties (`getKind`, `getDepth`, `getMaxBlockingTime`, `withKind`, …); `Durability` is an interface with `getKind` and the enum `Durability.Kind`. The test reads `getKind` and `getDepth` on reader, topic and writer QoS. (commit `2601f4bf35a`)
### §7.3.5.1 Table 7.2: policy ID as `Class<? extends QosPolicy>`

**Spec:** §7.3.5.1, p. 21, Table 7.2 — "Unique QoS policy ID, represented by an
instance of the enumeration QosPolicyId_t. | Java platform provides “Class
object,” which uniquely identifies a QoS policy. The id will be represented by
an object of Class<? extends QosPolicy> (for example, Class<Reliability>)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.get`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::qosCompatibilityReliableReaderRequiresReliableWriter`

**Status:** `done` — The ID of a policy is its `Class` object: `EntityQos` is a `Map<Class<? extends P>, P>` with `get(Class)`, and `QosCompatibility.incompatiblePolicyWith` returns the refusing policy as a class. The tests read `get(Reliability.class)` and check `Reliability.class` as the result of `incompatiblePolicyWith`. (commit `e0d499173ed`)
### §7.3.5.1 Table 7.2: policy name through reflection

**Spec:** §7.3.5.1, p. 21, Table 7.2 — "Unique QoS policy name, represented by a
string property QosPolicy.name. | Java reflection provides the necessary
capability to obtain name of a QoSPolicy class."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/QosPolicy.java` (`QosPolicy`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::thePolicyNameIsTheSimpleNameOfItsClass`

**Status:** `done` — CoreStatusPolicyTest checks for 22 policy names (UserData, TopicData, GroupData, TransportPriority, Lifespan, Durability, Presentation, Deadline, LatencyBudget, Ownership, OwnershipStrength, Liveliness, TimeBasedFilter, Partition, Reliability, DestinationOrder, History, ResourceLimits, EntityFactory, WriterDataLifecycle, ReaderDataLifecycle, DurabilityService) that `Class.getSimpleName()` equals the name, the class is a `QosPolicy` interface and the method of the same name in `PolicyFactory` (except TopicData, Presentation: no factory method there) returns that type. (commit `2601f4bf35a`)
### §7.3.5.1 `PolicyFactory` creates new policies

**Spec:** §7.3.5.1, p. 21 — "The org.omg.dds.core.policy.PolicyFactory interface
allows creation of new default-initiated policy objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.getPolicyFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`

**Status:** `done` — `org.omg.dds.core.policy.PolicyFactory` creates one object per policy with default values (`Reliability()`, `History()`, `UserData()` …); `PolicyFactoryImpl` implements it, `ServiceProviderInterface.getPolicyFactory` and `EntityQos.getPolicyFactory` return the factory. The test checks that the factory returns `Reliability()`; the QoS tests create all policies through it. (commit `2601f4bf35a`)
### §7.3.5.1 Defaults of the PolicyFactory unspecified

**Spec:** §7.3.5.1, p. 21 — "The default state of the newly created policy
objects via the PolicyFactory interface is unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — No requirement on the values.
### §7.3.5.1 Rationale: set policies explicitly

**Spec:** §7.3.5.1, p. 21 — "Design Rationale (non-normative) In general, to
ensure portability, programmers are encouraged to explicitly specify policy
parameters using the QoS DSL."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.
### §7.3.5.1 USER_DATA

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 109 (`dds-1.2.pdf`) — "USER_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/UserData.java` (`UserData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/UserDataImpl.java` (`UserDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantQosImpl.java` (`DomainParticipantQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataReaderQosImpl.java` (`DataReaderQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredParticipantDataCarriesTheUserData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aBuiltinReaderSeesTheQosOfTheAnnouncedWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theMatchedEntitiesAreDescribedWithTheirQos`

**Status:** `done` — `UserData` is a policy with `getValue()` (octet sequence) and `withValue(byte[], int, int)`; it belongs to the DomainParticipant, DataWriter and DataReader QoS and is empty by default. The tests check length 0 at the participant and that the writer's value `{1,2,3}` and the reader's `{4}` are reported to the other side. (commit `96bccc52ca9`)
### §7.3.5.1 TOPIC_DATA

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) —
"TOPIC_DATA | A sequence of octets: “value” | [...] The default value is an
empty (zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TopicData.java` (`TopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TopicDataImpl.java` (`TopicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/TopicQosImpl.java` (`TopicQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTopicPublisherAndSubscriberQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theQosOfEveryEntityKindCarriesTheValuesOfTheDocument`

**Status:** `done` — `TopicData` is a policy with `getValue()` and `withValue(byte[], int, int)` in the topic QoS; `PolicyFactoryImpl.TopicData` and `TopicQosImpl.defaults` return an empty sequence. The test in `BuiltinTopicQosTest` sets `{5,6,7,8,9}` and checks its arrival in the built-in topic data over SEDP; `QosTypesTest::theDefaultsOfTopicPublisherAndSubscriberQos` checks the default "empty", `QosProviderTest` reads TOPIC_DATA from base64.
### §7.3.5.1 GROUP_DATA

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) —
"GROUP_DATA | A sequence of octets: “value” | [...] The default value is an
empty (zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/GroupData.java` (`GroupData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/GroupDataImpl.java` (`GroupDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/PublisherQosImpl.java` (`PublisherQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/SubscriberQosImpl.java` (`SubscriberQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aBuiltinReaderSeesTheQosOfTheAnnouncedWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTopicPublisherAndSubscriberQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData`

**Status:** `done` — `GroupData` is a policy with `getValue()` and `withValue(byte[], int, int)` in the publisher and subscriber QoS; `PolicyFactoryImpl.GroupData` returns an empty sequence. `BuiltinTopicQosTest` sets `{9,8}` at the publisher and checks its arrival in the built-in topic data and the SEDP round trip; `QosTypesTest::theDefaultsOfTopicPublisherAndSubscriberQos` checks the default "empty" on publisher and subscriber QoS, `QosTypesTest::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData` the set value on both.
### §7.3.5.1 DURABILITY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 110 (`dds-1.2.pdf`) —
"DURABILITY | A “kind”: VOLATILE, TRANSIENT_LOCAL, TRANSIENT, or PERSISTENT"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Durability.java` (`Durability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DurabilityImpl.java` (`DurabilityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.retainForDurability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityJournal.java` (`DurabilityJournal`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::volatileDoesNotReplayToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::transientSurvivesTheWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::transientIsClearedWhenTheParticipantCloses`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::persistentSurvivesTheWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::persistentSurvivesTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::durabilityOfferedMustBeAtLeastRequestedInTheOrderVolatileToPersistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::aTransientLocalReaderIsRefusedByAVolatileWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::persistentSamplesOutliveTheProcess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::transientSamplesAreNotWrittenAndDoNotOutliveTheProcess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::withoutADirectoryPersistentLivesAsLongAsTheProcess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::aDisposedInstanceIsToldDisposedAfterTheRestartUntilItsDelayHasPassed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::aDisposedInstanceWithoutDelayIsForgottenAfterTheRestartAndStaysForgotten`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::aRecordThatWasCutOffAtTheEndOfTheJournalIsIgnored`

**Status:** `done` — `Durability` is a policy interface with `Durability.Kind` (VOLATILE, TRANSIENT_LOCAL, TRANSIENT, PERSISTENT). The tests check: TRANSIENT_LOCAL replays to a reader created later, VOLATILE does not, TRANSIENT survives the writer and ends with the participant, PERSISTENT survives writer and participant. The ordering in the match (`QosCompatibility`, offered >= requested) is checked by `QosRequestedOfferedTest::durabilityOfferedMustBeAtLeastRequestedInTheOrderVolatileToPersistent` for all 16 pairs, and `aTransientLocalReaderIsRefusedByAVolatileWriter` checks that the pair does not connect and records REQUESTED_INCOMPATIBLE_QOS. PERSISTENT has permanent storage when the system property `zerodds.durability.dir` names a directory: `org.zerodds.internal.DurabilityJournal` writes a journal per (domain, topic, type), `DurabilityService` replays it on the first access to the topic, checked in `QosPersistentStoreTest` (restart through `DurabilityService.reset`, history depth, disposed instance with and without delay, cut-off last record, TRANSIENT is not written). Without the property PERSISTENT lives in the memory of the process like TRANSIENT. (commit `f6f38cbacb2`)
### §7.3.5.1 DURABILITY_SERVICE

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 111 (`dds-1.2.pdf`) —
"DURABILITY_SERVICE | A duration "service_cleanup_delay", a HistoryQosPolicyKind
"history_kind", and three integers: history_depth, max_samples, max_instances,
max_samples_per_instance"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/DurabilityService.java` (`DurabilityService`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DurabilityServiceImpl.java` (`DurabilityServiceImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DurabilityService`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.retainForDurability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService.retain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService.disposed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService.unregistered`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultOwnershipStrengthAndDurabilityServiceOfTheFactoryAreTheTableValues`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::theHistoryDepthOfThePolicyDecidesWhatALateJoinerGets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::keepAllInTheStoreKeepsEverySampleUpToTheLimit`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::aSampleBeyondMaxSamplesPerInstanceIsRefusedAndNotDelivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::aNewInstanceBeyondMaxInstancesIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::maxSamplesBoundsTheSamplesOfAllInstancesTogether`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::aDisposedInstanceStaysInTheStoreForTheCleanupDelayAndIsToldDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::withNoCleanupDelayADisposedInstanceWithoutWritersIsForgottenAtOnce`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::anInstanceDisposedWhileAWriterStillWritesItIsKeptDisposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::aWriteAfterTheDisposeMakesTheInstanceAliveInTheStore`

**Status:** `done` — `DurabilityService` is a policy with `getServiceCleanupDelay`, `getHistoryKind`, `getHistoryDepth`, `getMaxSamples`, `getMaxInstances`, `getMaxSamplesPerInstance` and the `with…` methods in topic and DataWriter QoS. The factory defaults are now delay 0, KEEP_LAST depth 1 and `LENGTH_UNLIMITED` for the three limits (`QosTypesTest::theDefaultsOfTheDataWriterQos`). `DataWriterImpl.retainForDurability` passes writer id, policy and timestamp to `org.zerodds.internal.DurabilityService`: the history depth and the limits of the store come from this policy, not from the HISTORY of the writer; a sample over a limit raises `OutOfResourcesException` before it is delivered. `service_cleanup_delay` decides when the store forgets an instance: after dispose, when no writer writes it any more and the delay has elapsed since both (DDS 1.4 §2.2.3.4); until then it tells a later reader the instance is NOT_ALIVE_DISPOSED. The clock can be replaced in tests (`DurabilityService.useClock`). Tested in `QosDurabilityServiceTest`. (commit `d7588ee703d`)
### §7.3.5.1 PRESENTATION

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 111–112 (`dds-1.2.pdf`) —
"PRESENTATION | An “access_scope”: INSTANCE, TOPIC, GROUP And two booleans:
“coherent_access” “ordered_access”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Presentation.java` (`Presentation`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PresentationImpl.java` (`PresentationImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.Presentation`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherQos.java` (`PublisherQos.getPresentation`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberQos.java` (`SubscriberQos.getPresentation`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.withPresentation`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PresentationImpl.java` (`PresentationImpl.requestedOfferedContract`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.atomically`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/GroupOrder.java` (`GroupOrder`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.groupStamp`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::presentationOfferedScopeMustBeAtLeastRequestedAndCoherentAndOrderedMustBeOffered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::aSubscriberThatRequestsTopicScopeDoesNotMatchAPublisherThatOffersInstanceScope`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::aSubscriberAndAPublisherWithTheSamePresentationMatch`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoherentSetAtomicityTest.java::aReaderOnAnotherThreadNeverSeesPartOfASet`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::orderedAccessWithScopeTopicReturnsTheSamplesOfAllInstancesInOneOrder`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::aCoherentSetOfARemoteWriterReachesACoherentAccessReaderAsAWhole`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::getDataReadersIsAListInTheOrderOfWritingAndAReadGivesOneSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::samplesOfTwoRemoteWritersOfOnePublisherArriveInTheOrderOfTheGroupSequenceNumber`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::aGroupCoherentSetOfTwoRemoteWritersArrivesAsAWholeAfterTheEndMarkers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::aGroupPublisherWithTwoWritersSendsItsSamplesAndSetsToAPeer`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/GroupOrderTest.java::aGroupCoherentSetIsHandedOnTogetherWhenItsEndHasCome`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/GroupOrderTest.java::withTheTimeoutOptionAMissingNumberIsSkippedAndAnIncompleteSetIsNotDelivered`

**Status:** `done` — `Presentation` has `getAccessScope` (INSTANCE, TOPIC, GROUP), `isCoherentAccess` and `isOrderedAccess`; default INSTANCE/false/false (`PolicyFactoryImpl.Presentation`). Compatibility is matched: `PresentationImpl.requestedOfferedContract` demands an offered scope >= the requested one and, when `coherent_access` or `ordered_access` is requested, the same property offered; `QosCompatibility` includes the PRESENTATION of the publisher and subscriber of the two endpoints in the match (`withPresentation`), the violation is in REQUESTED_/OFFERED_INCOMPATIBLE_QOS (`QosRequestedOfferedTest`, all nine scope pairs, coherent, ordered). `coherent_access` acts: the sets between `begin_coherent_changes` and `end_coherent_changes` reach a reader on another thread as a whole and, over RTPS, with `PID_COHERENT_SET`. `ordered_access` with access_scope TOPIC puts the samples of all instances of a reader in one order (`orderedAccessWithScopeTopicReturnsTheSamplesOfAllInstancesInOneOrder`). With access_scope GROUP and `ordered_access`, `get_datareaders` is a list in which every reader appears as often as it holds samples, in the order of writing (source timestamp for BY_SOURCE_TIMESTAMP, else arrival over all readers of the process), and `read` and `take` return at most one sample (DDS 1.4 §2.2.2.5.2.10, §2.2.2.5.3.8). Over RTPS every sample of a writer of a publisher with GROUP carries `PID_GROUP_SEQ_NUM` (running over all writers of the publisher), `PID_WRITER_GROUP_INFO` (`GroupDigest_t`: the leading four octets of the MD5 over the CDR big-endian sequence of the entity ids of the writers, sorted by their value as little-endian integer, `GroupDigest.of`) and, in a set, `PID_GROUP_COHERENT_SET`; every writer sends the End Coherent Set DATA at the end of the set (DDSI-RTPS 2.5 §8.7.5, §8.7.6, §9.3.2.5, §9.6.4.3 to §9.6.4.5). The writers of a publisher with GROUP send HEARTBEATs with the GroupInfoFlag (0x08): `currentGSN` (the largest group number given out), `firstGSN` and `lastGSN` of the writer, and `writerSet` and `secureWriterSet` (`RtpsMessage.Builder.heartbeatGroup`, DDSI-RTPS 2.5 §8.3.7.5, §9.4.5.7, §9.4.2.19); a writer without a sample sends them too (empty range). The option `zerodds.rtps.omit-heartbeat-group-digests` (Rust: `wire.omit_heartbeat_group_digests`, default off) leaves out `writerSet` and `secureWriterSet`. A reader of a subscriber with GROUP hands on the samples of the writers of a remote publisher in the order of the group number and a group set together when its end has come (`GroupOrder`). If the number n is missing, it counts as absent per §8.7.5 when a HEARTBEAT reports `currentGSN >= n` and every counted writer (matched, RELIABLE, of the group by `writerSet`) says it is none of its own: it delivered a sample with a larger group number, or its HEARTBEAT with `currentGSN >= n` has a range `[firstGSN, lastGSN]` that does not hold n; a writer not yet heard from holds the order. A set with a number given up that way is not delivered (§8.7.6). The timeout rule is a named option, off by default: `zerodds.rtps.group-timeout-millis` (a number that does not arrive within that time is skipped). Tests: `GroupOrderTest` (the rule, the timeout option off and on, a set) and `GroupAccessTest` (two writers over RTPS: number 11 belongs to a writer without a matched reader and is given up by the HEARTBEATs of the two matched writers; without a HEARTBEAT of the second writer it stays missing). The group number is assigned and the sample entered in the history of its writer under one lock (`PublisherImpl.stampAndWrite`); the HEARTBEAT reads `currentGSN` under the same lock (`currentGroupStamp`) and so announces only numbers whose change is entered (`GroupHeartbeatLockTest`). Expected towards Cyclone DDS, Fast DDS and OpenDDS: they read `PID_GROUP_SEQ_NUM`, `PID_GROUP_COHERENT_SET` and `PID_WRITER_GROUP_INFO` per §8.7.5; measured is only the ZeroDDS peer. (commits `35d47828b1e`, `c82a99bcd0a`, `1a7bd9cc493`, `6c7f75add81`)
### §7.3.5.1 DEADLINE

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 112 (`dds-1.2.pdf`) — "DEADLINE
| A duration “period”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Deadline.java` (`Deadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DeadlineImpl.java` (`DeadlineImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateDeadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::infiniteDeadlineNeverMisses`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::rxoDeadlineCompatibility`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::requestedDeadlineMissedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::offeredDeadlineMissedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::deadlineOfferedPeriodMustNotExceedRequested`

**Status:** `done` — `Deadline.getPeriod` defaults to infinite (`PolicyFactoryImpl.Deadline`). Reader and writer count misses (`DataReaderImpl.evaluateDeadline`, `DataWriterImpl.evaluateOfferedDeadline`); the tests check REQUESTED_DEADLINE_MISSED at 50 ms, that infinite never misses, OFFERED_DEADLINE_MISSED, the listener callback without polling, and compatibility (offered <= requested) in `rxoDeadlineCompatibility`. `QosRequestedOfferedTest::deadlineOfferedPeriodMustNotExceedRequested` checks compatibility on pairs including the infinite default.
### §7.3.5.1 LATENCY_BUDGET

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) —
"LATENCY_BUDGET | A duration “duration” | [...] The default value of the
duration is zero"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/LatencyBudget.java` (`LatencyBudget`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/LatencyBudgetImpl.java` (`LatencyBudgetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.LatencyBudget`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicies`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::latencyBudgetOfferedMustNotExceedRequested`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`

**Status:** `done` — `LatencyBudget.getDuration` defaults to 0 (`PolicyFactoryImpl.LatencyBudget`, checked in `QosTypesTest::theDefaultsOfTheDataWriterQos`); the value 2 s is written on PID_LATENCY_BUDGET and read back in the round trip. `QosCompatibility.incompatiblePolicies` checks LATENCY_BUDGET (offered <= requested, `LatencyBudgetImpl.requestedOfferedContract`); `QosRequestedOfferedTest::latencyBudgetOfferedMustNotExceedRequested` checks longer, equal, shorter and the default. Per DDS 1.4 §2.2.3.7 the policy is a hint; it does not affect delivery. (commit `f4803bc097c`)
### §7.3.5.1 OWNERSHIP

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) — "OWNERSHIP
| A “kind” SHARED EXCLUSIVE"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Ownership.java` (`Ownership`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/OwnershipImpl.java` (`OwnershipImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::exclusiveOwnershipArbitratesToHighestStrength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::sharedOwnershipDeliversFromAllWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::ownershipKindMustMatch`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::ownershipKindsMustBeEqualInBothDirections`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::aSharedReaderAndAnExclusiveWriterDoNotMatchAndBothSideRecordIt`

**Status:** `done` — `Ownership.getKind` (SHARED, EXCLUSIVE); the reader arbitrates only when reader and writer are EXCLUSIVE (`DataReaderImpl.onWrite`). DDS 1.4 §2.2.3.9 requires the same kind: `OwnershipImpl.requestedOfferedContract` returns 0 for an equal and 1 for a different kind, `QosCompatibility` reports OWNERSHIP as incompatible in both directions. Tested: `QosRequestedOfferedTest::ownershipKindsMustBeEqualInBothDirections` through `QosCompatibility.isCompatible`, `aSharedReaderAndAnExclusiveWriterDoNotMatchAndBothSideRecordIt` with reader and writer (no sample, REQUESTED_ and OFFERED_INCOMPATIBLE_QOS with Ownership), `QosLifecycleTest::ownershipKindMustMatch`. (commit `f4803bc097c`)
### §7.3.5.1 OWNERSHIP_STRENGTH

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 113 (`dds-1.2.pdf`) —
"OWNERSHIP_ STRENGTH | An integer “value”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/OwnershipStrength.java` (`OwnershipStrength`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/OwnershipStrengthImpl.java` (`OwnershipStrengthImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.OwnershipStrength`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::exclusiveOwnershipArbitratesToHighestStrength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aBuiltinReaderSeesTheQosOfTheAnnouncedWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultOwnershipStrengthAndDurabilityServiceOfTheFactoryAreTheTableValues`

**Status:** `done` — `OwnershipStrength.getValue`; with EXCLUSIVE the reader accepts only samples of the writer with the highest strength (1 against 10), the value 7 travels through built-in topic data. `PolicyFactoryImpl.OwnershipStrength()` now creates the value 0 (DDS 1.4 §2.2.3, table of the policies); `DataWriterQosImpl.defaults` takes the 0. Checked in `QosTypesTest::theDefaultsOfTheDataWriterQos` and `theDefaultOwnershipStrengthAndDurabilityServiceOfTheFactoryAreTheTableValues`. (commit `f4803bc097c`)
### §7.3.5.1 LIVELINESS

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 114 (`dds-1.2.pdf`) —
"LIVELINESS | A “kind”: AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC and a
duration “lease_duration”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Liveliness.java` (`Liveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/LivelinessImpl.java` (`LivelinessImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.renewManualByParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.renewLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/QosParameters.java` (`QosParameters.incompatibility`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessRxoKindAndLease`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessLostFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessChangedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::theParticipantAssertsTheLivelinessOfItsManualByParticipantWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::theParticipantDoesNotAssertTheLivelinessOfAManualByTopicWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::anAssertOnAnyWriterOfTheParticipantAssertsTheManualByParticipantWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::aWriteByAnyWriterOfTheParticipantAssertsTheManualByParticipantWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::theWritersOfAnotherParticipantAreNotAsserted`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::theDefaultLivelinessIsAutomaticWithAnInfiniteLease`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::livelinessOfferedKindMustBeAtLeastRequestedAndTheLeaseNotLonger`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::theWireSideRefusesAnOfferedLivelinessKindBelowTheRequestedOne`

**Status:** `done` — `Liveliness` with kind and `lease_duration`; default AUTOMATIC/infinite (`PolicyFactoryImpl.Liveliness`, checked in `QosLivelinessTest::theDefaultLivelinessIsAutomaticWithAnInfiniteLease`). The lease is monitored: the writer reports LIVELINESS_LOST, the reader LIVELINESS_CHANGED, both without a status poll (`LeaseTest`). Compatibility: offered kind >= requested (AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC) and offered lease <= requested (`QosRequestedOfferedTest::livelinessOfferedKindMustBeAtLeastRequestedAndTheLeaseNotLonger`); the wire side `QosParameters.incompatibility` compared the kind the wrong way round and is corrected (`theWireSideRefusesAnOfferedLivelinessKindBelowTheRequestedOne`). `DomainParticipantImpl.assertLiveliness` now renews all DataWriters of the participant with MANUAL_BY_PARTICIPANT; likewise every write and every `assert_liveliness` of a writer of the participant renews them (DDS 1.4 §2.2.3.11); a MANUAL_BY_TOPIC writer and the writers of another participant are left alone (`QosLivelinessTest`). (commit `c62d3fc35c0`)
### §7.3.5.1 TIME_BASED_FILTER

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 115 (`dds-1.2.pdf`) —
"TIME_BASED_ FILTER | A duration "minimum_ separation" | [...] default
minimum_separation=0"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TimeBasedFilter.java` (`TimeBasedFilter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TimeBasedFilterImpl.java` (`TimeBasedFilterImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.TimeBasedFilter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/TimeBasedFilterState.java` (`TimeBasedFilterState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aReaderWithMinimumSeparationTakesOneSamplePerInstanceInThatTime`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::theNewestSampleHeldBackIsDeliveredWhenTheSeparationHasPassed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::withoutMinimumSeparationEverySampleIsDelivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataReaderQos`

**Status:** `done` — `TimeBasedFilter.getMinimumSeparation` defaults to 0; the value 1 s runs through the round trip of the built-in subscription data. `DataReaderImpl.onWrite` now evaluates `minimum_separation` per instance (`org.zerodds.internal.TimeBasedFilterState`): the first sample of an instance is accepted, a further one within the separation is held back, and the newest held-back sample is delivered when the separation has passed (on access and by the reader's sweep), so that the last change reaches the reader (DDS 1.4 §2.2.3.12). The time is the reception time at the reader. DEADLINE < minimum_separation is `InconsistentPolicyException` (`QosSetQosTest::aDeadlineShorterThanTheMinimumSeparationIsInconsistent`). Tested in `QosReaderEffectsTest` (one instance, several instances, late delivery of the last sample, default 0). (commit `d7588ee703d`)
### §7.3.5.1 PARTITION

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 115 (`dds-1.2.pdf`) — "PARTITION
| A list of strings “name”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Partition.java` (`Partition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PartitionImpl.java` (`PartitionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/QosParameters.java` (`QosParameters.partitionsOverlap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionIsolatesNonOverlappingEndpoints`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionMatchesOnOverlap`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionWildcardMatches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::nonOverlappingPartitionIsNotAMatch`

**Status:** `done` — `Partition.getName` (set of names, default `""`) is part of `PublisherQos` and `SubscriberQos`. The tests check separation for A against B, delivery on overlap (`A,X` against `X`), wildcards (`sensor.*` against `sensor.temp`) and no match reported to the listener.
### §7.3.5.1 RELIABILITY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 115–116 (`dds-1.2.pdf`) —
"RELIABILITY | A “kind”: RELIABLE, BEST_EFFORT and a duration
“max_blocking_time” | [...] The default max_blocking_time=100ms."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReliabilityImpl.java` (`ReliabilityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DataWriterReliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitResources`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::qosCompatibilityReliableReaderRequiresReliableWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::rxoIncompatibleReliabilityDetected`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::incompatibleReliabilityDoesNotConnect`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aWriterThatOffersLessThanRequestedIsReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::reliabilityIsComparedByKindAndNotByMaxBlockingTime`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aWriteToANewInstanceBeyondMaxInstancesTimesOutUntilAnInstanceIsUnregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aBlockedWriteContinuesWhenAnotherThreadFreesTheInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aBestEffortWriterDoesNotWaitAndIsOutOfResources`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`

**Status:** `done` — Kind (BEST_EFFORT, RELIABLE) and `max_blocking_time` (default 100 ms, `QosTypesTest::theDefaultsOfTheDataWriterQos`). `ReliabilityImpl.requestedOfferedContract` compares the kind only (DDS 1.4 §2.2.3.14): a RELIABLE reader with a longer `max_blocking_time` than the writer stays compatible (`QosRequestedOfferedTest::reliabilityIsComparedByKindAndNotByMaxBlockingTime`); a RELIABLE reader against a BEST_EFFORT writer is not connected (`CoreTypesTest`, `QosCompatibilityTest`, `ReaderStatusTest`). When a RESOURCE_LIMITS limit is reached (no free instance, the history for TRANSIENT_LOCAL) `DataWriterImpl.write` waits at most `max_blocking_time` and then raises `TimeoutException`; a BEST_EFFORT writer does not wait and raises `OutOfResourcesException` (`QosResourceLimitsTest`). A writer without a limit never waits. (commit `d7588ee703d`)
### §7.3.5.1 TRANSPORT_PRIORITY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) —
"TRANSPORT_ PRIORITY | An integer “value”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TransportPriority.java` (`TransportPriority`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TransportPriorityImpl.java` (`TransportPriorityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.TransportPriority`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicQos.java` (`TopicQos.getTransportPriority`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos.getTransportPriority`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`

**Status:** `done` — `TransportPriority.getValue` (int, default 0) in `TopicQos` and `DataWriterQos`; the test sets 3 on a `TopicQos` and reads it back after the round trip of the built-in topic data. DDS 1.2 §7.1.3.15 calls the policy a hint; there is no effect on the transport (in the code only read in `QosParameters` and the built-in data).
### §7.3.5.1 LIFESPAN

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) — "LIFESPAN
| A duration “duration”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Lifespan.java` (`Lifespan`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/LifespanImpl.java` (`LifespanImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.Lifespan`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.Message.lifespanMillis`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.purgeLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aSampleWhoseLifespanHasPassedWhenItArrivesIsNotDelivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aStoredSampleDisappearsWhenItsLifespanPasses`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::anInfiniteLifespanNeverExpires`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aLateJoinerIsNotReplayedAnExpiredSampleOfATransientLocalWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::durableSamplesKeepTheirSourceTimestampAndReachTheReaderWithItsLifespan`

**Status:** `done` — `Lifespan.getDuration` (default infinite); 30 s are written on PID_LIFESPAN and read back in the round trip. The writer carries its validity duration in every message (`InProcessBus.Message.lifespanMillis`); `DataReaderImpl` discards a sample whose source timestamp plus duration has passed on arrival and removes expired samples before every access. `DataWriterImpl.replayTo` does not hand expired samples to a late reader and passes the original source timestamp on in a replay; the same holds for TRANSIENT and PERSISTENT through `org.zerodds.internal.DurabilityService`. Tested in `QosReaderEffectsTest` and `QosDurabilityServiceTest`. (commit `d7588ee703d`)
### §7.3.5.1 DESTINATION_ORDER

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 116 (`dds-1.2.pdf`) —
"DESTINATION_ ORDER | A “kind”: BY_ RECEPTION_ TIMESTAMP, BY_SOURCE_ TIMESTAMP"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/DestinationOrder.java` (`DestinationOrder`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DestinationOrderImpl.java` (`DestinationOrderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DestinationOrder`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::bySourceTimestampASampleOlderThanTheNewestOfTheInstanceIsOrderedBeforeIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::byReceptionTimestampTheSourceTimestampDoesNotDecide`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::destinationOrderOfferedMustBeAtLeastRequested`

**Status:** `done` — `DestinationOrder.getKind` (BY_RECEPTION_TIMESTAMP, BY_SOURCE_TIMESTAMP; default BY_RECEPTION_TIMESTAMP); BY_SOURCE_TIMESTAMP is written on PID_DESTINATION_ORDER and read back in the round trip. With BY_SOURCE_TIMESTAMP `DataReaderImpl` orders a sample of an instance whose source timestamp is smaller than the newest one of the instance before the newer ones (DDS 1.4 §2.2.2.5.3.8: the samples of an instance appear in the order of their source timestamps, smaller first); none is lost, equal timestamps stay in arrival order, other instances are untouched, with BY_RECEPTION_TIMESTAMP the source timestamp does not decide. `QosCompatibility.incompatiblePolicies` checks DESTINATION_ORDER (offered >= requested, BY_RECEPTION_TIMESTAMP < BY_SOURCE_TIMESTAMP). Tested in `QosReaderEffectsTest` and `QosRequestedOfferedTest::destinationOrderOfferedMustBeAtLeastRequested`. (commit `d7588ee703d`)
### §7.3.5.1 HISTORY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 117–118 (`dds-1.2.pdf`) —
"HISTORY | A “kind”: KEEP_LAST, KEEP_ALL And an optional integer “depth” | [...]
KEEP_LAST is the default kind. The default value of depth is 1."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/HistoryImpl.java` (`HistoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.retainCapped`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastCapsPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastIsPerInstanceNotGlobal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::historyDepthMustBeNonNegative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aWriterIsNotCreatedWithPoliciesThatContradictEachOther`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::theBoundariesOfTheConsistencyRulesAreConsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aTopicIsNotCreatedWithPoliciesThatContradictEachOther`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::aDefaultQosThatContradictsItselfIsRefusedAndTheDefaultStays`

**Status:** `done` — Kind (KEEP_LAST, KEEP_ALL) and depth; default KEEP_LAST/1. The tests check capping per instance in the reader, that the depth is not global, replay by the writer (KEEP_LAST 1, TRANSIENT_LOCAL), the default and rejection of a negative depth; KEEP_ALL is used through `readerKeepAll`. The rule `depth <= max_samples_per_instance` (DDS 1.4 §2.2.3.18) and the two rules of RESOURCE_LIMITS and of DURABILITY_SERVICE are checked by `QosRules.checkConsistent` when a writer, reader or topic is created, in `setQos` and in `setDefault…Qos`; a violation is `InconsistentPolicyException`. The boundary cases (depth equal to the limit, KEEP_ALL without a depth comparison, `LENGTH_UNLIMITED`) are valid. Tested in `QosSetQosTest`, `QosDefaultQosTest`. (commit `2e8078b94b5`)
### §7.3.5.1 Defaults of RELIABILITY and HISTORY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 116–118 (`dds-1.2.pdf`) —
"RELIABLE [...] This is the default value for DataWriters. BEST_EFFORT [...]
This is the default value for DataReaders and Topics."; "KEEP_LAST is the
default kind. The default value of depth is 1."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DataWriterReliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DataReaderReliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DataReaderHistory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataReaderQosImpl.java` (`DataReaderQosImpl.defaults`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl.defaults`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/TopicQosImpl.java` (`TopicQosImpl.defaults`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataReaderQos`

**Status:** `done` — Defaults today: RELIABILITY RELIABLE for `DataWriterQosImpl.defaults`, BEST_EFFORT for `DataReaderQosImpl.defaults` and `TopicQosImpl.defaults`, HISTORY KEEP_LAST with depth 1. `reliabilityAndHistoryQosDefaults` checks these values on reader, topic and writer QoS; `SubscriberImpl` and `PublisherImpl` use these defaults for entities without their own QoS.
### §7.3.5.1 RESOURCE_LIMITS

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 118–119 (`dds-1.2.pdf`) —
"RESOURCE_ LIMITS | Three integers: max_samples, max_instances,
max_samples_per_instance | [...] By default, LENGTH_UNLIMITED."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ResourceLimits.java` (`ResourceLimits`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ResourceLimitsImpl.java` (`ResourceLimitsImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitResources`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aReaderRejectsAnInstanceBeyondMaxInstances`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aKeepAllReaderRejectsASampleBeyondMaxSamplesPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::theDefaultLimitsAreUnlimitedAndRejectNothing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aWriteToANewInstanceBeyondMaxInstancesTimesOutUntilAnInstanceIsUnregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aBlockedWriteContinuesWhenAnotherThreadFreesTheInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::aBestEffortWriterDoesNotWaitAndIsOutOfResources`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::keepAllWithMaxSamplesPerInstanceTimesOutForTransientLocal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::maxSamplesBoundsTheHistoryOfAllInstancesTogether`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::keepLastReplacesTheOldestSampleOfTheInstanceAndNeedsNoNewResource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::withNoLimitAWriterNeverWaits`

**Status:** `done` — `ResourceLimits` with `max_samples`, `max_instances`, `max_samples_per_instance`, default `LENGTH_UNLIMITED`. The reader rejects samples beyond the three limits (`DataReaderImpl.rejectedByLimits`, SAMPLE_REJECTED): checked for `max_samples` (REJECTED_BY_SAMPLES_LIMIT), `max_instances` (REJECTED_BY_INSTANCES_LIMIT), `max_samples_per_instance` (REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT, KEEP_ALL) and the default. A RELIABLE writer waits up to `max_blocking_time` and then raises `TimeoutException` (BEST_EFFORT: `OutOfResourcesException` at once) when a sample would need a new instance beyond `max_instances` or, for the history kept for TRANSIENT_LOCAL, go beyond `max_samples` or `max_samples_per_instance` (`DataWriterImpl.awaitResources`); unregister and dispose free the resources and wake the waiting writer. With KEEP_LAST the new sample replaces the oldest of the instance and needs no new resource. A VOLATILE writer keeps no samples, only its instances are bounded. The rules against HISTORY are under HISTORY (`QosRules`). (commit `d7588ee703d`)
### §7.3.5.1 ENTITY_FACTORY

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 119 (`dds-1.2.pdf`) — "ENTITY_
FACTORY | A boolean: “autoenable_ created_ entities” | [...] By default, TRUE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/EntityFactory.java` (`EntityFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/EntityFactoryImpl.java` (`EntityFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.enableIfFactorySays`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::entityFactoryDecidesWhetherWhatIsCreatedIsEnabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aPublisherWithAutoEnableFalseCreatesDisabledWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aPublisherWithAutoEnableTrueCreatesEnabledWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::theChangeOfTheFactoryPolicyAffectsOnlyTheEntitiesCreatedAfterIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::enablingThePublisherEnablesTheWritersItCreatedWhileDisabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::enablingThePublisherLeavesTheWritersDisabledWhenItsPolicyIsFalse`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aWriterCannotBeEnabledBeforeItsPublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aSubscriberWithAutoEnableFalseCreatesDisabledReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::enablingTheSubscriberEnablesTheReadersItCreatedWhileDisabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::theParticipantFactoryDecidesWhetherTheParticipantsItCreatesAreEnabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::theParticipantPolicyDecidesWhetherItsPublishersSubscribersAndTopicsAreEnabled`

**Status:** `done` — `EntityFactory.isAutoEnableCreatedEntities`, default true. Publisher and subscriber read `entity_factory` of their QoS: `PublisherImpl.createDataWriter` and `SubscriberImpl.createDataReader` enable the new entity only if the publisher or subscriber itself is enabled and the policy says true (DDS 1.4 §2.2.3.20, §2.2.2.1.1.7). `enable` on a publisher or subscriber enables the children that were disabled so far if the policy says so; `enable` on a writer or reader whose publisher or subscriber is not enabled raises `PreconditionNotMetException`. A change of the policy affects only entities created later. For the participant the policy of its factory QoS applies (`DomainParticipantFactoryImpl.autoEnable`), for publisher, subscriber and topic that of the participant (`enableIfFactorySays`). Tested in `QosEntityFactoryTest`. (commit `2e8078b94b5`)
### §7.3.5.1 WRITER_DATA_LIFECYCLE

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 119–120 (`dds-1.2.pdf`) —
"WRITER_DATA_ LIFECYCLE | A boolean: “autodispose_ unregistered_ instances” |
[...] By default, TRUE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/WriterDataLifecycle.java` (`WriterDataLifecycle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/WriterDataLifecycleImpl.java` (`WriterDataLifecycleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterKey`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::theDefaultIsToDisposeWhatIsUnregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::unregisterDisposesTheInstanceWhenTheFlagIsTrue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::unregisterOnlyLeavesNoWritersWhenTheFlagIsFalse`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::anInstanceDisposedByTheApplicationStaysDisposedWhenItIsUnregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::deletingTheWriterDisposesItsInstancesWhenTheFlagIsTrue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::deletingTheWriterOnlyLeavesNoWritersWhenTheFlagIsFalse`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::aWriteAfterTheDisposeBringsTheInstanceBackToAlive`

**Status:** `done` — `WriterDataLifecycle` (`isAutDisposeUnregisteredInstances`, default true) in the DataWriter QoS. `DataWriterImpl.unregisterKey` (used by `unregisterInstance`, the handle path and `close()`) publishes DISPOSE first and then UNREGISTER when true, as if the application had called `dispose` before unregistering (DDS 1.4 §2.2.3.21); with false only UNREGISTER. The reader leaves an instance marked NOT_ALIVE_DISPOSED in that state when the last writer unregisters. With the default the result of an unregister is therefore NOT_ALIVE_DISPOSED, with false NOT_ALIVE_NO_WRITERS; `QosLifecycleTest::unregisterMarksInstanceNotAliveNoWriters` and `closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites` now use false, as do `DurabilityServiceTest` and `MultiTopicJoinTest` where they expect NO_WRITERS or surviving data. The built-in topics (`Builtin.writer`) use false and keep reporting NOT_ALIVE_NO_WRITERS. Tested in `QosWriterDataLifecycleTest`. (commit `d7588ee703d`)
### §7.3.5.1 READER_DATA_LIFECYCLE

**Spec:** §7.3.5.1, p. 21; [DDS] 1.2 §7.1.3, p. 120 (`dds-1.2.pdf`) —
"READER_DATA_ LIFECYCLE | Two durations “autopurge_ nowriter_ samples_delay” and
“autopurge_ disposed_ samples_delay” | [...] By default, infinite."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ReaderDataLifecycle.java` (`ReaderDataLifecycle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReaderDataLifecycleImpl.java` (`ReaderDataLifecycleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.purgeLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::byDefaultAnInstanceWithoutWritersKeepsItsSamplesUntilTheyAreTaken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aZeroDelayPurgesTheInstanceWithoutWritersWithItsUntakenSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aZeroDelayPurgesTheSamplesOfADisposedInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::theDelayOfTheOtherStateDoesNotPurge`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::aFiniteDelayPurgesWhenItHasPassedWithoutAnyAccess`

**Status:** `done` — `ReaderDataLifecycle` (`getAutoPurgeNoWriterSamplesDelay`, `getAutoPurgeDisposedSamplesDelay`, default infinite) in the DataReader QoS. `DataReaderImpl.purgeLocked` runs before every access and in the reader's sweep: after `autopurge_nowriter_samples_delay` since NOT_ALIVE_NO_WRITERS it removes the instance with all untaken samples, after `autopurge_disposed_samples_delay` since NOT_ALIVE_DISPOSED all samples of the instance (DDS 1.4 §2.2.3.22). Infinite never purges; the delay of one state does not act on the other. Tested in `QosReaderEffectsTest` (default, delay 0 for both states, delay of the other state, finite delay without access). (commit `d7588ee703d`)
### §7.3.5.2 Entity QoS as an interface based on `EntityQos`, direct access to the policies

**Spec:** §7.3.5.2, p. 21 — "Each Entity QoS (e.g., DataReaderQos) is an
interface extending org.omg.dds.core.EntityQos. These sub-interfaces provide
direct access to their policies as in the IDL PSM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`

**Status:** `done` — Seven interfaces (`DomainParticipantFactoryQos`, `DomainParticipantQos`, `TopicQos`, `PublisherQos`, `SubscriberQos`, `DataWriterQos`, `DataReaderQos`) extend `EntityQos<...>` and have one accessor per policy (`getReliability`, `getHistory`, `getPartition`, ...). The tests read policies directly (`DataReaderQos.getHistory`, `TopicQos.getReliability`, `DataWriterQos.getReliability`, `PublisherQos.getPartition`). `QosProfile` no longer exists.
### §7.3.5.2 DomainParticipantFactoryQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantFactoryQos { EntityFactoryQosPolicy entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactoryQos.java` (`DomainParticipantFactoryQos.getEntityFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantFactoryQosImpl.java` (`DomainParticipantFactoryQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::theParticipantFactoryDecidesWhetherTheParticipantsItCreatesAreEnabled`

**Status:** `done` — `DomainParticipantFactoryQos` has `getEntityFactory`; `DomainParticipantFactoryImpl.autoEnable` reads the value in `create_participant`. `QosEntityFactoryTest::theParticipantFactoryDecidesWhetherTheParticipantsItCreatesAreEnabled` sets a factory QoS with ENTITY_FACTORY false, reads `getEntityFactory()` back, checks that a created participant is not enabled (`DomainParticipantImpl.isEnabled`), that `enable` enables it, and restores the QoS. `DomainParticipantFactoryImpl.setQos` checks the QoS like every `setQos`.
### §7.3.5.2 DomainParticipantQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantQos { UserDataQosPolicy user_data; EntityFactoryQosPolicy
entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantQos.java` (`DomainParticipantQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantQosImpl.java` (`DomainParticipantQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::entityFactoryDecidesWhetherWhatIsCreatedIsEnabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::aParticipantAndTheFactoryTakeNewUserDataAndEntityFactory`

**Status:** `done` — `DomainParticipantQos` has `getUserData` and `getEntityFactory`; a participant returns its QoS through `getQos`. `factoryQosAndDefaultParticipantQos` sets USER_DATA `mine` as default and checks that `createParticipant(...).getQos()` equals the QoS set; `entityFactoryDecidesWhetherWhatIsCreatedIsEnabled` checks the effect of ENTITY_FACTORY false.
### §7.3.5.2 TopicQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 167–168 (`dds-1.2.pdf`) — "struct
TopicQos { topic_data; durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; ownership; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicQos.java` (`TopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/TopicQosImpl.java` (`TopicQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theTopicQosAnswersWhatItWasGivenForEveryPolicyOfTable72`

**Status:** `done` — `TopicQos` has the 13 policies of the IDL (`topic_data` to `ownership`) and `getRepresentation` and `getTypeConsistency`; `TopicQosImpl.defaults` sets all of them. Tested are Reliability, Durability, History, ResourceLimits, TransportPriority, Deadline, DurabilityService and TopicData (defaults and the round trip of the built-in topic data) and, with default and changed value, LatencyBudget, Liveliness, DestinationOrder, Lifespan and Ownership in `QosTypesTest::theTopicQosAnswersWhatItWasGivenForEveryPolicyOfTable72`.
### §7.3.5.2 PublisherQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 168 (`dds-1.2.pdf`) — "struct
PublisherQos { PresentationQosPolicy presentation; PartitionQosPolicy partition;
GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherQos.java` (`PublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/PublisherQosImpl.java` (`PublisherQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionIsolatesNonOverlappingEndpoints`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionMatchesOnOverlap`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aPublisherWithAutoEnableFalseCreatesDisabledWriters`

**Status:** `done` — `PublisherQos` has `getPresentation`, `getPartition`, `getGroupData`, `getEntityFactory`. PARTITION affects matching (`QosLifecycleTest`), GROUP_DATA and PRESENTATION travel through built-in topic data and the wire. ENTITY_FACTORY at the publisher decides whether `createDataWriter` enables the writer (`QosEntityFactoryTest`); PRESENTATION takes part in the match of the endpoints (`QosRequestedOfferedTest`), the effect of coherent_access and ordered_access on delivery is missing (item on PRESENTATION). `QosTypesTest::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData` checks the accessors with set values. (commit `2e8078b94b5`)
### §7.3.5.2 SubscriberQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 169 (`dds-1.2.pdf`) — "struct
SubscriberQos { PresentationQosPolicy presentation; PartitionQosPolicy
partition; GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory;
};"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberQos.java` (`SubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/SubscriberQosImpl.java` (`SubscriberQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionIsolatesNonOverlappingEndpoints`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionWildcardMatches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::nonOverlappingPartitionIsNotAMatch`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::aSubscriberWithAutoEnableFalseCreatesDisabledReaders`

**Status:** `done` — `SubscriberQos` has `getPresentation`, `getPartition`, `getGroupData`, `getEntityFactory`. PARTITION is tested (separation, wildcards, no match reported to the listener, round trip of the subscription data). PRESENTATION, GROUP_DATA and ENTITY_FACTORY at the subscriber are checked by `QosTypesTest::thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData`; `SubscriberImpl.createDataReader` enables the reader only if `entity_factory` says so (`QosEntityFactoryTest`). (commit `2e8078b94b5`)
### §7.3.5.2 DataWriterQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 168 (`dds-1.2.pdf`) — "struct
DataWriterQos { durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; user_data; ownership; ownership_strength;
writer_data_lifecycle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastCapsPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::exclusiveOwnershipArbitratesToHighestStrength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDataWriterQosAnswersWhatItWasGivenForTheRemainingPolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataWriterQos`

**Status:** `done` — `DataWriterQos` has the 15 policies of the IDL and `getRepresentation` and `getTypeConsistency`; `DataWriterQosImpl.defaults` sets all of them. Tested are Durability, Deadline, LatencyBudget, Liveliness, Reliability, DestinationOrder, History, Lifespan, UserData, Ownership and OwnershipStrength and ResourceLimits, TransportPriority, DurabilityService and WriterDataLifecycle (`QosTypesTest::theDataWriterQosAnswersWhatItWasGivenForTheRemainingPolicies`, defaults in `theDefaultsOfTheDataWriterQos`). WRITER_DATA_LIFECYCLE acts on unregister and close (`QosWriterDataLifecycleTest`).
### §7.3.5.2 DataReaderQos

**Spec:** §7.3.5.2, p. 21; [DDS] 1.2 IDL, p. 168–169 (`dds-1.2.pdf`) — "struct
DataReaderQos { durability; deadline; latency_budget; liveliness; reliability;
destination_order; history; resource_limits; user_data; ownership;
time_based_filter; reader_data_lifecycle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataReaderQosImpl.java` (`DataReaderQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastCapsPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::exclusiveOwnershipArbitratesToHighestStrength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDataReaderQosAnswersWhatItWasGivenForTheRemainingPolicies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theDefaultsOfTheDataReaderQos`

**Status:** `done` — `DataReaderQos` has the 12 policies of the IDL (`durability` to `reader_data_lifecycle`) and `getRepresentation` and `getTypeConsistency`; `DataReaderQosImpl.defaults` sets all of them. Tested are Durability, Deadline, Liveliness, Reliability, History, ResourceLimits, Ownership, UserData and TimeBasedFilter and LatencyBudget, DestinationOrder and ReaderDataLifecycle (`QosTypesTest::theDataReaderQosAnswersWhatItWasGivenForTheRemainingPolicies`, defaults in `theDefaultsOfTheDataReaderQos`). OwnershipStrength and Partition are not part of the reader QoS.
### §7.3.5.2 Access to the policies through `java.util.Map`

**Spec:** §7.3.5.2, p. 21 — "However, the base interface also provides for
generic access using the java.util.Map interface. This interface allows
applications to look up policies by ID and to iterate over them in a generic
way, including vendor-specific extension policies, without introducing
compile-time dependencies on vendor-specific APIs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.get`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.entrySet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.policiesOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::reliabilityAndHistoryQosDefaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::rxoDeadlineCompatibility`

**Status:** `done` — `EntityQos<P>` extends `java.util.Map<Class<? extends P>, P>` with `get(Class<POLICY>)`; `AbstractEntityQos` provides `get` and an unmodifiable `entrySet`. `reliabilityAndHistoryQosDefaults` looks up with `reader.get(Reliability.class)`; `rxoDeadlineCompatibility` iterates all entries of the QoS through `QosCompatibility.policiesOf`.
### §7.3.5.2 QoS not directly creatable, only from `getQos` or `QosProvider`

**Spec:** §7.3.5.2, p. 21 — "QoS objects cannot be created directly. They can be
either retrieved from an entity (e.g., DataReader) using the getQoS method or
looked up using a string identifier using the QoSProvider interface. QoS objects
can be cloned from pre-existing state maintained by the Service implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl.defaults`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::entitiesAreCreatedFromTheQosOfAProvider`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultDataWriterQosOfThePublisherIsTheQosOfTheWritersItCreatesNext`

**Status:** `done` — In the OMG API the QoS interfaces have no constructor; the constructors of the `*QosImpl` are private. QoS from entities and factories (`getQos`, `getDefaultPublisherQos`, `getDefaultParticipantQos`), deriving with `withPolicy` and the route through `QosProvider` are tested (`QosProviderTest::entitiesAreCreatedFromTheQosOfAProvider`, `QosDefaultQosTest`). The static method `*QosImpl.defaults(environment)` is public so that the tests (`Qos.writer()`, `Qos.reader()`) and `QosProviderImpl` can create default QoS; it is not a method of the OMG API.
### §7.3.5.2 QoS from entities and QosProvider immutable, changes through the QoS DSL

**Spec:** §7.3.5.2, p. 21 — "QoS objects as returned by Entities and QoSProvider
shall be immutable; applications shall never observe them to change.
Applications that wish to modify QoS values must use the “QoS DSL” supported by
the QoS and Policy classes."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theMapOfAQosCannotBeChanged`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::aQosFromAnEntityIsNotChangedByDerivingFromIt`

**Status:** `done` — `AbstractEntityQos` copies the policies into a map of its own, `entrySet` is unmodifiable, `AbstractMap.put`, `remove`, `clear`, `putAll`, `keySet().remove`, `values().clear` and the iterator `remove` raise `UnsupportedOperationException`; the policies have `final` fields, `withPolicy` creates a new object. Tested for all seven QoS kinds in `QosTypesTest::theMapOfAQosCannotBeChanged`; that the QoS of an entity stays unchanged after `withPolicy` and that the original is the same object is checked by `withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas` and `aQosFromAnEntityIsNotChangedByDerivingFromIt`.
### §7.3.5.3 Changes disallowed, new QoS and policies through a fluent DSL

**Spec:** §7.3.5.3, p. 21 — "Modifying QoS objects and their constituent
policies is disallowed but a QoS DSL shall support creation of new QoS objects
and policies from the existing objects using Java fluent interface design."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicies`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withPoliciesAppliesSeveralPoliciesAtOnce`

**Status:** `done` — The DSL exists: QoS (`withPolicy`, `withPolicies`) and policies (`withKeepLast`, `withManualByTopic().withLeaseDuration(...)`) return a new object and chain; the values act in behaviour (`transientLocalReplaysToLateJoiner`, `livelinessChangedReflectsWriterPresence`). That the original stays unchanged and another object comes back is checked by `QosTypesTest::withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas`, `everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource` and `withPoliciesAppliesSeveralPoliciesAtOnce`.
### §7.3.5.3 QoS classes with `withPolicy` and `withPolicies`

**Spec:** §7.3.5.3, p. 21 — "QoS classes shall provide withPolicy and
withPolicies methods that accept one or more policy objects to create a new QoS
object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.withPolicies`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicies`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withPoliciesAppliesSeveralPoliciesAtOnce`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::aPolicyThatIsNoPolicyOfTheQosIsRefused`

**Status:** `done` — `EntityQos.withPolicy(P)` and `withPolicies(P...)` exist in `EntityQos` and `AbstractEntityQos`; `withPolicy` is used in many tests. `QosTypesTest::withPoliciesAppliesSeveralPoliciesAtOnce` checks `withPolicies` with several policies and with none; `aPolicyThatIsNoPolicyOfTheQosIsRefused` checks the `IllegalArgumentException` of `AbstractEntityQos.policyKey` for a policy that names itself only as a reader policy and is none of the policies of Table 7.2.
### §7.3.5.3 Policy classes with `with` methods

**Spec:** §7.3.5.3, p. 21 — "Policy classes shall provide with methods to
specify policy parameters and to create new policy objects from the existing
ones."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Liveliness.java` (`Liveliness`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ResourceLimits.java` (`ResourceLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::historyDepthMustBeNonNegative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::policiesWithTheSameValueAreEqualAndHaveTheSameHashCode`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::theRequestOfferedPoliciesCompareByTheirOrder`

**Status:** `done` — All policy interfaces have `with*` methods for their parameters (`Liveliness.withKind`/`withLeaseDuration`, `Reliability.withMaxBlockingTime`, `History.withDepth`, `ResourceLimits.withMaxSamples` ...). `thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913` builds policies this way and checks the values byte by byte on the wire; `historyDepthMustBeNonNegative` checks the value check in `withDepth`.
### §7.3.5.3 Every `with` creates a new object, chaining

**Spec:** §7.3.5.3, p. 21 — "Each with method call will create a new policy
object because the target object of the method call is immutable. The with
methods shall support method chaining (QoS DSL)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Liveliness.java` (`Liveliness.withLeaseDuration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withCallsChainOnePolicyIntoTheNext`

**Status:** `done` — Chaining is tested (two `withPolicy` in a row; `Liveliness().withManualByTopic().withLeaseDuration(...)`), and the chained values act. That every `with` creates a new object and leaves the target unchanged is checked by `QosTypesTest::withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas` (`assertNotSame`, the source keeps its old value) and `everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource` for Reliability, History, ResourceLimits, Deadline, Partition, UserData, Presentation and Durability; `withCallsChainOnePolicyIntoTheNext` checks chaining.
### §7.3.5.3 Example `pf.ResourceLimits().withMaxSamples(P).withMaxInstances(Q)`

**Spec:** §7.3.5.3, p. 22 — "Example (non-normative) PolicyFactory pf = … //
object policy factory reference ResourceLimits rl =
pf.ResourceLimits().withMaxSamples(P).withMaxInstances(Q);"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory.ResourceLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::everyBuiltinDataTypeRoundTripsItsQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::withCallsChainOnePolicyIntoTheNext`

**Status:** `n/a (informative)` — Example (non-normative). The construction is possible with the current API: `PolicyFactory.ResourceLimits().withMaxSamples(..).withMaxInstances(..)`; `everyBuiltinDataTypeRoundTripsItsQos` calls it. The method is named `ResourceLimits()` as in the OMG sources. `QosTypesTest::withCallsChainOnePolicyIntoTheNext` checks the chain with the values 20, 6 and `LENGTH_UNLIMITED`.
### §7.3.5.3 Rationale: `getQos` without a copy

**Spec:** §7.3.5.3, p. 22 — "Design Rationale (non-normative) • The getQos
operation can operate maximally efficiently: it need not allocate any memory or
perform any copies. [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`)

**Tests:** —

**Status:** `n/a (informative)` — Design rationale (non-normative). `getQos` returns the stored immutable object without a copy; `DomainParticipantOperationsTest::factoryQosAndDefaultParticipantQos` checks with `assertSame` that `getQos` returns the object that was set.
### §7.3.5.4 `org.omg.dds.core.QosProvider` returns QoS from library and profile

**Spec:** §7.3.5.4, p. 22 — "The org.omg.dds.core.QosProvider interface allows
Entity’s Qos to be obtained from the names of QoS library and profile."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theQosOfEveryEntityKindCarriesTheValuesOfTheDocument`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theWriterAndReaderQosCarryTheValuesOfTheDocumentAndKeepTheDefaultsOfWhatItLeavesOut`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aProfileWithoutAnElementOfAKindHasNoQosOfThatKind`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::theFactoryQosHasItsOwnElement`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::anIdCanNameAnotherProfileOrAnElementOfIt`

**Status:** `done` — `QosProvider` (abstract class) has `getDomainParticipantFactoryQos`, `getDomainParticipantQos`, `getTopicQos`, `getSubscriberQos`, `getPublisherQos`, `getDataReaderQos`, `getDataWriterQos` each without and with `id`; `QosProviderImpl` reads the XML library. `QosProviderTest` writes a library to a file, loads it through `QosProvider.newQosProvider(uri, profile, env)` and reads the QoS of each entity kind (without and with `id`), with the values of the document and the defaults for what it leaves out. An `id` is the `name` of the element, `library::profile` or `library::profile::name`; a profile without an element of the kind or an unknown `id` is `IllegalArgumentException`. (commit `8c975167054`)
### §7.3.5.4 Source as a URI, prefix `file://` mandatory

**Spec:** §7.3.5.4, p. 22 — "The Qos library source is provided as a uniform
resource identifier (URI). Conforming implementation must support “file://”
prefix. For instance, “file:///path/to/qos/library.”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.read`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aFileUriIsReadAndTheProviderIsObtainedFromTheEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::anotherSchemeIsUnsupported`

**Status:** `done` — `QosProviderImpl.read` accepts the scheme `file` (`Paths.get(URI)`) and raises `UnsupportedOperationException` for every other scheme, also for none. `QosProviderTest::aFileUriIsReadAndTheProviderIsObtainedFromTheEnvironment` loads a `file:///…` URI, `anotherSchemeIsUnsupported` checks `http`, `ftp` and `data`. (commit `8c975167054`)
### §7.3.5.4 `QosProvider` from `ServiceEnvironment.newQosProvider(uri, profile)`

**Spec:** §7.3.5.4, p. 22 — "An instance of QosProvider is obtained from the
ServiceEnvironment. For example, serviceEnv.newQosProvider(String uri, String
profile);"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider.newQosProvider`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newQosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aFileUriIsReadAndTheProviderIsObtainedFromTheEnvironment`

**Status:** `done` — The call `serviceEnv.newQosProvider(uri, profile)` of the example exists as `QosProvider.newQosProvider(uri, profile, env)`, which delegates to `env.getSPI().newQosProvider(uri, profile)` (`ServiceProviderInterface`, implemented in `ServiceEnvironmentImpl`); `ServiceEnvironment` itself has no such method (as in the OMG sources). `QosProviderTest::aFileUriIsReadAndTheProviderIsObtainedFromTheEnvironment` calls both routes and compares the QoS.
### §7.3.5.4 `uri` in standard syntax, `profile` unique within the document

**Spec:** §7.3.5.4, p. 22 — "The uri parameter uses the standard uri syntax. The
profile parameter identifies a uniquely identified profile in the document
referred by the uri."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.read`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.selectProfile`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aUriThatIsNoUriAndAFileThatIsNotThereAreRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aProfileIsSelectedByQualifiedNameByUniqueNameOrAsTheFirst`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::aProfileNameThatTwoProfilesShareIsNotUnique`

**Status:** `done` — `QosProviderImpl` forms `new URI(uri)` (error: `IllegalArgumentException`) and selects the profile as `library::profile`, as a name that is unique in the file or, for `null`, the first one. An unknown profile is `IllegalArgumentException`; so is a name two profiles carry, and two profiles of one name in a library are already refused when reading. Tested in `QosProviderTest::aUriThatIsNoUriAndAFileThatIsNotThereAreRefused`, `aProfileIsSelectedByQualifiedNameByUniqueNameOrAsTheFirst`, `aProfileNameThatTwoProfilesShareIsNotUnique`. (commit `8c975167054`)
### §7.3.5.4 Factories create entities and set defaults from QoS objects or QosProvider

**Spec:** §7.3.5.4, p. 22 — "Each Entity factory interface
DomainParticipantFactory, DomainParticipant, Publisher, and Subscriber provides
methods to create new “product” Entities and to set their default QoS based on
QoS objects created programmatically or obtained through QosProvider."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setDefaultParticipantQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::factoryQosAndDefaultParticipantQos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastCapsPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultTopicQosOfTheParticipantIsTheQosOfTheTopicsItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultSubscriberQosOfTheParticipantIsTheQosOfTheSubscribersItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultPublisherQosOfTheParticipantIsTheQosOfThePublishersItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultDataWriterQosOfThePublisherIsTheQosOfTheWritersItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultDataReaderQosOfTheSubscriberIsTheQosOfTheReadersItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::theDefaultParticipantQosOfTheFactoryIsTheQosOfTheParticipantsItCreatesNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::aDefaultQosThatContradictsItselfIsRefusedAndTheDefaultStays`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::entitiesAreCreatedFromTheQosOfAProvider`

**Status:** `done` — The factories `DomainParticipantFactory`, `DomainParticipant`, `Publisher` and `Subscriber` create entities with a QoS and have `setDefault…Qos`/`getDefault…Qos`; the default is checked for consistency. Tested in `QosDefaultQosTest`: `setDefaultTopicQos`, `setDefaultSubscriberQos`, `setDefaultPublisherQos`, `Publisher.setDefaultDataWriterQos`, `Subscriber.setDefaultDataReaderQos`, `DomainParticipantFactory.setDefaultParticipantQos` and the refusal of a self-contradicting default; `QosProviderTest::entitiesAreCreatedFromTheQosOfAProvider` creates entities from provider QoS. (commit `2e8078b94b5`)
### §7.3.6 All entities extend `Entity`

**Spec:** §7.3.6, p. 22 — "As in the DDS PIM, all Entity interfaces
extend—directly or indirectly—the interface Entity."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DomainEntity.java` (`DomainEntity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::publisherCloseClosesItsWriters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::closeContainedEntitiesClosesThemWithoutClosingTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::everyEntityExtendsEntityAndEveryOneButTheParticipantExtendsDomainEntity`

**Status:** `done` — `DomainParticipant` extends `Entity`; `Publisher`, `Subscriber`, `DataWriter`, `DataReader` and `Topic` extend `DomainEntity`, which extends `Entity` (read). The test uses `Qos.isClosed(Entity<?, ?>)` with DomainParticipant, Publisher and DataWriter, which compiles only if they are `Entity`; no test covers the inheritance of the other three. This is a pure signature requirement. `QosEntityBasicsTest::everyEntityExtendsEntityAndEveryOneButTheParticipantExtendsDomainEntity` checks the inheritance of all six types with `isAssignableFrom`.
### §7.3.6 `Entity` generic over QoS and listener type

**Spec:** §7.3.6, p. 22 — "In this PSM, this interface is generic; it is
parameterized by the Entity’s QoS and listener types. These parameters allow
applications to call common operations like getQos or getListener in a type-safe
way while still working with Entities polymorphically."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::defaultQosIsTheQosOfWhatIsCreatedNext`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::clearingTheListenerStopsDelivery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::getQosAndGetListenerKeepTheirTypeOnTheSubinterfaces`

**Status:** `done` — `Entity<LISTENER extends EventListener, QOS extends EntityQos<?>>` with `getQos()` and `getListener()`; the sub-interfaces bind the types (`Publisher extends DomainEntity<PublisherListener, PublisherQos>`). `defaultQosIsTheQosOfWhatIsCreatedNext` reads `createPublisher().getQos()` as `PublisherQos`, `clearingTheListenerStopsDelivery` reads `getListener()` of a DataReader; both compile only with the type parameters.
### §7.3.6 `Entity` extends `java.io.Closeable`

**Spec:** §7.3.6, p. 22 — "The Entity interface extends java.io.Closeable
interface to support specific new language constructs (e.g., Java 7
try-with-resources) for dealing with all Entities polymorphically."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::entityExtendsCloseable`

**Status:** `done` — `Entity extends Closeable, DDSObject` (`Entity.java`). `QosEntityBasicsTest::entityExtendsCloseable` checks `Closeable.class.isAssignableFrom(Entity.class)` and holds publisher, subscriber, topic, DataWriter and DataReader as `java.io.Closeable`, closes them through it and checks the state with `Qos.isClosed`.
### §7.3.6 Entities other than DomainParticipant extend `DomainEntity` with `getParent`

**Spec:** §7.3.6, p. 22 — "Also as in the DDS PIM, Entities other than
DomainParticipant extend the interface DomainEntity. These Entities provide
operations to get the creating parent Entity; in this PSM, this operation is the
polymorphic DomainEntity.getParent."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DomainEntity.java` (`DomainEntity.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::theParentOfAnEntityIsTheEntityThatCreatedIt`

**Status:** `done` — `DomainEntity.getParent` exists; publisher, subscriber and topic return the participant, DataWriter the publisher, DataReader the subscriber. `QosEntityBasicsTest::theParentOfAnEntityIsTheEntityThatCreatedIt` checks all five relations with `assertSame`.
### §7.3.7 Statuses, listeners, conditions and wait sets

**Spec:** §7.3.7, p. 22 — "This sub clause describes the objects pertaining to
the status changes of DDS Entities: the Status types themselves, listeners,
conditions, and wait sets."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Introduction without a requirement of its own.
### §7.3.7.1 Every status as an abstract class based on `org.omg.dds.core.Status`, which extends `EventObject`

**Spec:** §7.3.7.1, p. 23 — "This PSM represents each status identified by the
DDS PIM as an abstract class extending org.omg.dds.core.Status, which in turn
extends java.util.EventObject."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/Status.java` (`Status`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/StatusChangedEvent.java` (`StatusChangedEvent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aStatusKeepsItsCounts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::theThirteenStatusKindsAreClassesOfTheStatusBase`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::theEventThatCarriesAStatusIsAnEventObjectWithTheEntityAsSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`

**Status:** `done` — All 13 statuses are abstract classes with the base `org.omg.dds.core.status.Status` (`QosEntityBasicsTest::theThirteenStatusKindsAreClassesOfTheStatusBase`, also `Serializable`). The text says `Status` extends `EventObject`; the source `omgdds_src.zip`, which prevails under §2 when they conflict, declares `Status` as `abstract class Status implements Serializable, DDSObject` without `EventObject`, and `org/omg/dds` stays equal to the source (`CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`). The `java.util.EventObject` is carried by `StatusChangedEvent`: it has `getSource()` (the entity) and `getStatus()`; `QosEntityBasicsTest::theEventThatCarriesAStatusIsAnEventObjectWithTheEntityAsSource` checks that on a PublicationMatchedEvent. (commit `bf330e251b4`)
### §7.3.7.1 Status kinds as `Class` objects, masks as a `Set`

**Spec:** §7.3.7.1, p. 23 — "This PSM represents status kinds using the
java.lang.Class instances of the corresponding status classes and status masks
as java.util.Sets of such status classes."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.allStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.setEnabledStatuses`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`

**Status:** `done` — Status kinds are `Class<? extends Status>`, masks are `Set`/`Collection` of them: `Entity.getStatusChanges`, `setListener(listener, statuses)`, `StatusCondition.setEnabledStatuses`, `allStatusKinds`/`noStatusKinds`. The tests read `getStatusChanges().contains(DataAvailableStatus.class)`, set and read the enabled statuses of a `StatusCondition`, and check `allStatusKinds().size() == 13` and `noStatusKinds().isEmpty()`.
### §7.3.7.1 Statuses in callbacks reusable, applications copy

**Spec:** §7.3.7.1, p. 23 — "Status objects passed to listeners in callbacks may
be pooled and reused by the implementation. Therefore, applications that wish to
retain these objects—or any objects found within them, such as instance
handles—for later use outside of the callback are responsible for copying them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission for the implementation and duty of the application; there is no requirement on the code that could be met. ZeroDDS creates a new status object per callback (`DataReaderImpl`: `new RequestedDeadlineMissedStatusImpl(...)` among others, read).
### §7.3.7.1 InconsistentTopicStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 132, IDL p. 159
(`dds-1.2.pdf`) — "struct InconsistentTopicStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/InconsistentTopicStatus.java` (`InconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/InconsistentTopicStatusImpl.java` (`InconsistentTopicStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::topicsOfTheSameTypeOrInAnotherDomainAreConsistent`

**Status:** `done` — Two topics of the same name and different type names in one domain count INCONSISTENT_TOPIC (TopicImpl.onInconsistentTopic) and call the TopicListener; the test states both.
### §7.3.7.1 SampleLostStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 159
(`dds-1.2.pdf`) — "struct SampleLostStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SampleLostStatus.java` (`SampleLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SampleLostStatusImpl.java` (`SampleLostStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.noteSequence`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::aGapInTheSequenceNumbersOfAWriterIsSampleLost`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::sampleLostIsReportedToTheListener`

**Status:** `done` — `DataReaderImpl.noteSequence` counts gaps in the sequence numbers per writer; `SampleLostStatus` reports `getTotalCount` and `getTotalCountChange` (reset on read), `onSampleLost` the event. Both tests deliver samples through `deliverWire`. (commit `313af57405c`)
### §7.3.7.1 SampleRejectedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 159
(`dds-1.2.pdf`) — "struct SampleRejectedStatus { long total_count; long
total_count_change; SampleRejectedStatusKind last_reason; InstanceHandle_t
last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SampleRejectedStatus.java` (`SampleRejectedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SampleRejectedStatusImpl.java` (`SampleRejectedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::aSampleOfANewInstanceBeyondMaxInstancesIsRejectedWithThatReasonAndInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::aSampleBeyondMaxSamplesPerInstanceIsRejectedWithThatReasonAndInstance`

**Status:** `done` — `ReaderLimitsTest` checks the reasons REJECTED_BY_INSTANCES_LIMIT and REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT with `getLastInstanceHandle` (derived by hand from the key) and the change value; `ReaderStatusTest` checks REJECTED_BY_SAMPLES_LIMIT and the listener. (commit `313af57405c`)
### §7.3.7.1 LivelinessLostStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 159
(`dds-1.2.pdf`) — "struct LivelinessLostStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessLostStatus.java` (`LivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessLostStatusImpl.java` (`LivelinessLostStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessLostFiresWithoutPolling`

**Status:** `done` — DataWriterImpl.getLivelinessLostStatus counts the loss once and resets the change when read; the test checks the counts, the change and the listener.
### §7.3.7.1 LivelinessChangedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 159–160
(`dds-1.2.pdf`) — "struct LivelinessChangedStatus { long alive_count; long
not_alive_count; long alive_count_change; long not_alive_count_change;
InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessChangedStatus.java` (`LivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessChangedStatusImpl.java` (`LivelinessChangedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessChangedReflectsWriterPresence`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::livelinessChangedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::livelinessChangedNamesTheWriterWhoseLivelinessChangedAndCountsTheChanges`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::anIncompatibleWriterIsNotCountedAsAliveOrNotAlive`

**Status:** `done` — `DataReaderImpl.getLivelinessChangedStatus` and the listener path set `lastPublicationHandle` to the handle of the writer whose liveliness changed last. `ReaderStatusDetailTest` checks `getLastPublicationHandle`, `getAliveCountChange` (+1, then -1) and `getNotAliveCountChange` (+1). (commit `313af57405c`)
### §7.3.7.1 OfferedDeadlineMissedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160
(`dds-1.2.pdf`) — "struct OfferedDeadlineMissedStatus { long total_count; long
total_count_change; InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/OfferedDeadlineMissedStatus.java` (`OfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedDeadlineMissedStatusImpl.java` (`OfferedDeadlineMissedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::offeredDeadlineMissedFiresWithoutPolling`

**Status:** `done` — DataWriterImpl.getOfferedDeadlineMissedStatus returns total_count, total_count_change and last_instance_handle; the test checks all three and the listener.
### §7.3.7.1 RequestedDeadlineMissedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160
(`dds-1.2.pdf`) — "struct RequestedDeadlineMissedStatus { long total_count; long
total_count_change; InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/RequestedDeadlineMissedStatus.java` (`RequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/RequestedDeadlineMissedStatusImpl.java` (`RequestedDeadlineMissedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::requestedDeadlineMissedFiresWithoutPolling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::requestedDeadlineMissedNamesTheInstanceThatMissedLast`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::theChangeOfRequestedDeadlineMissedStartsAgainWhenTheStatusIsRead`

**Status:** `done` — `ReaderStatusDetailTest` checks `getLastInstanceHandle` with two instances and the change value 0 on the second read. (commit `313af57405c`)
### §7.3.7.1 OfferedIncompatibleQosStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160
(`dds-1.2.pdf`) — "struct OfferedIncompatibleQosStatus { long total_count; long
total_count_change; QosPolicyId_t last_policy_id; QosPolicyCountSeq policies;
};"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/OfferedIncompatibleQosStatus.java` (`OfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedIncompatibleQosStatusImpl.java` (`OfferedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::anIncompatibleReaderIsCountedWithTheRefusedPolicy`

**Status:** `done` — DataWriterImpl.getOfferedIncompatibleQosStatus counts the refused reader with last_policy_id and the policy list and calls onOfferedIncompatibleQos; the test checks the count, the policy and the listener.
### §7.3.7.1 RequestedIncompatibleQosStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160
(`dds-1.2.pdf`) — "struct RequestedIncompatibleQosStatus { long total_count;
long total_count_change; QosPolicyId_t last_policy_id; QosPolicyCountSeq
policies; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/RequestedIncompatibleQosStatus.java` (`RequestedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/RequestedIncompatibleQosStatusImpl.java` (`RequestedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aWriterThatOffersLessThanRequestedIsReportedToTheListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::incompatibleReliabilityDoesNotConnect`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::requestedIncompatibleQosCountsTheRefusingPolicyAndResetsItsChange`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::everyPolicyThatRefusesTheConnectionIsCountedAndOneConnectionIsOneInTotal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::thePresentationOfTheSubscriberMustBeOfferedByThePublisher`

**Status:** `done` — `ReaderStatusDetailTest` checks `getPolicies` (one `QosPolicyCount` per refusing policy with count 1), `getTotalCountChange` and `getLastPolicyClass`; `ReaderStatusTest` the event. (commit `b531522ca09`)
### §7.3.7.1 PublicationMatchedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160
(`dds-1.2.pdf`) — "struct PublicationMatchedStatus { long total_count; long
total_count_change; long current_count; long current_count_change;
InstanceHandle_t last_subscription_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/PublicationMatchedStatus.java` (`PublicationMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/PublicationMatchedStatusImpl.java` (`PublicationMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getPublicationMatchedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::thePublicationMatchedStatusCountsAndNamesTheLastSubscription`

**Status:** `done` — DataWriterImpl.readPublicationMatchedStatusLocked returns last_subscription_handle of the reader matched last; the test compares it with getMatchedSubscriptions.
### §7.3.7.1 SubscriptionMatchedStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133, IDL p. 160–161
(`dds-1.2.pdf`) — "struct SubscriptionMatchedStatus { long total_count; long
total_count_change; long current_count; long current_count_change;
InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SubscriptionMatchedStatusImpl.java` (`SubscriptionMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSubscriptionMatchedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::statusIsReadableWithoutAListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::subscriptionMatchedNamesTheWriterThatConnectedOrLeftLast`

**Status:** `done` — `DataReaderImpl.getSubscriptionMatchedStatus` and the listener path set `lastPublicationHandle`; `ReaderStatusDetailTest` checks the handle and `getTotalCountChange` and `getCurrentCountChange` (also -1 after a writer closes). (commit `313af57405c`)
### §7.3.7.1 DataAvailableStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133 (`dds-1.2.pdf`) —
"DataReader | DATA_AVAILABLE | New information is available."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/DataAvailableStatus.java` (`DataAvailableStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/DataAvailableStatusImpl.java` (`DataAvailableStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/DataAvailableEvent.java` (`DataAvailableEvent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken`

**Status:** `done` — DataAvailableStatus is now a class; DataReaderListener.onDataAvailable receives a DataAvailableEvent with getStatus() and getSource(). ListenerTest checks one call per sample with the reader itself as source; ReaderStatusTest checks that getStatusChanges() contains DataAvailableStatus after a write and no longer after take(). (commit `b594326a2a0`)
### §7.3.7.1 DataOnReadersStatus

**Spec:** §7.3.7.1, p. 23; [DDS] 1.2 §7.1.4.1, p. 133 (`dds-1.2.pdf`) —
"Subscriber | DATA_ON_READERS | New information is available."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/DataOnReadersStatus.java` (`DataOnReadersStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/DataOnReadersStatusImpl.java` (`DataOnReadersStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/DataOnReadersEvent.java` (`DataOnReadersEvent`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener.onDataOnReaders`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::dataOnReadersGoesToTheSubscriberListenerAndNotifyDataReadersPassesItToTheReadersWithNewData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::withoutASubscriberListenerTheReadersListenersAreCalledAndDataOnReadersStaysChanged`

**Status:** `done` — `DataReaderImpl.fireDataAvailable` sets the flag DATA_ON_READERS on the subscriber (`SubscriberImpl.dataOnReadersChanged`) and calls `SubscriberListener.onDataOnReaders` with a `DataOnReadersEvent`. Without a listener the flag stays in `getStatusChanges()` until a reader of the subscriber is read or taken; with a listener it is reset after the call. (commit `313af57405c`)
### §7.3.7.2 Listeners extend `java.util.EventListener`

**Spec:** §7.3.7.2, p. 23 — "This PSM maps the Listener interface from the DDS
PIM to the empty marker interface java.util.EventListener interface defined by
the Java SE standard library."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicListener.java` (`TopicListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener`

**Status:** `done` — TopicListener, PublisherListener, SubscriberListener and DomainParticipantListener are called and pass on statuses they do not take; the three tests check Publisher, Subscriber (DATA_ON_READERS too), Participant and Topic.
### §7.3.7.2 Adapter classes with empty methods, named with "Adapter"

**Spec:** §7.3.7.2, p. 23 — "For each listener sub-interface (e.g.,
DataWriterListener), this PSM provides a concrete implementation of that
interface in which all methods have empty implementations. These concrete
classes are named like the listener interfaces they implement, but with the word
“Listener” replaced by “Adapter.”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderAdapter.java` (`DataReaderAdapter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterAdapter.java` (`DataWriterAdapter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicAdapter.java` (`TopicAdapter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherAdapter.java` (`PublisherAdapter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberAdapter.java` (`SubscriberAdapter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantAdapter.java` (`DomainParticipantAdapter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::aWriterListenerThatDoesNotTakeAStatusLeavesItToThePublisher`

**Status:** `done` — Six adapter classes (DataReaderAdapter, DataWriterAdapter, TopicAdapter, PublisherAdapter, SubscriberAdapter, DomainParticipantAdapter) implement their listener interface with empty methods. The tests override single methods: TopicAdapter (TopicOperationsTest), PublisherAdapter (ParticipantEntityTest, PublisherOperationsTest), SubscriberAdapter (ParticipantEntityTest, ListenerPropagationTest), DomainParticipantAdapter (ParticipantEntityTest, TopicOperationsTest, PublisherOperationsTest), DataReaderAdapter and DataWriterAdapter (ListenerTest, ParticipantSubscriberTest). (commit `e72453fce1f`)
### §7.3.7.2 Callback without the entity, source through the status

**Spec:** §7.3.7.2, p. 23 — "In the DDS PIM, each listener callback receives two
arguments: the Entity, the status of which has changed, and the new value of
that status. In this PSM, the former is unnecessary and is omitted: it is
available through the read-only Source property of the status object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/StatusChangedEvent.java` (`StatusChangedEvent.getSource`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`

**Status:** `done` — Each callback has exactly one argument, a StatusChangedEvent (extends java.util.EventObject) with getSource() and getStatus(). The tests read the entity through status.getSource() and assertSame it to their own reader or writer. The source is on the event, not on the Status class. (commit `e72453fce1f`)
### §7.3.7.2 Rationale: listener and adapter

**Spec:** §7.3.7.2, p. 23 — "Design Rationale (non-normative) The listener +
adapter design pattern is consistent with that used in the standard AWT and
Swing UI libraries and elsewhere."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Design rationale (non-normative); no requirement that can be met.
### §7.3.7.2 Lower-level listeners generic: TopicListener, DataReaderListener, DataWriterListener

**Spec:** §7.3.7.2, p. 23 — "The former category includes TopicListener,
DataReaderListener, and DataWriterListener. These classes are generic; their
type parameters match that of the Entities on which they are set. This
convention allows applications to read and write data within the context of a
callback in a statically type-safe way."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicListener.java` (`TopicListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicAdapter.java` (`TopicAdapter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::readerListenerFiresOnMatchAndOnEverySample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::writerListenerFiresOnPublicationMatched`

**Status:** `done` — DataReaderListener<TYPE>, DataWriterListener<TYPE> and TopicListener<TYPE> are parameterized by the data type; the events carry the same parameter. TopicImpl calls onInconsistentTopic (TopicImpl.onInconsistentTopic). Tests: TopicOperationsTest::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener (TopicAdapter<String>, getSource() as Topic<String> without a cast), ListenerTest::readerListenerFiresOnMatchAndOnEverySample, ::writerListenerFiresOnPublicationMatched. (commit `deff69a0210`)
### §7.3.7.2 Higher-level listeners with the wildcard `?`, no inheritance to the lower-level ones

**Spec:** §7.3.7.2, p. 23 — "The latter category includes PublisherListener,
SubscriberListener, and DomainParticipantListener. The Topics, DataReaders, and
DataWriters passed to these listeners’ callbacks are parameterized with the
generic wildcard ‘?’. Because of this difference between these listeners and
those in the former category, there are no inheritance relationships between
these categories, unlike in the PIM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::createParticipantWithAListenerAndAMaskReachesItFromTheWritersOfTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::aWriterWithoutListenerAndAPublisherWithoutOnePassItToTheParticipantListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders`

**Status:** `done` — PublisherListener, SubscriberListener and DomainParticipantListener take events with a wildcard (e.g. PublicationMatchedEvent<?>); DomainParticipantListener extends PublisherListener and SubscriberListener. The classes PublisherImpl, SubscriberImpl and DomainParticipantImpl store the listener in EntityBase, and DataWriterImpl, DataReaderImpl and TopicImpl call it through ListenerUp. Tests: PublisherAdapter with onPublicationMatched and onOfferedIncompatibleQos (ParticipantEntityTest::createPublisherWithAListenerSeesTheMatchOfAWriterCreatedLater, PublisherOperationsTest::aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher), SubscriberAdapter with onSubscriptionMatched (ParticipantEntityTest::createSubscriberWithAListenerSeesTheMatchOfAReaderCreatedLater) and onDataOnReaders (ListenerPropagationTest), DomainParticipantAdapter with onPublicationMatched, onOfferedIncompatibleQos and onInconsistentTopic (ParticipantEntityTest, PublisherOperationsTest, TopicOperationsTest). (commit `9426325d8b2`)
### §7.3.7.3 Conditions based on `org.omg.dds.core.Condition`

**Spec:** §7.3.7.3, p. 23 — "Conditions extend the base interface
org.omg.dds.core.Condition."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Condition.java` (`Condition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet`

**Status:** `done` — org.omg.dds.core.Condition with getTriggerValue; GuardCondition, StatusCondition and ReadCondition extend it. The tests attach all three kinds through attachCondition(Condition) to a WaitSet and read getTriggerValue and the list of active conditions. (commit `2601f4bf35a`)
### §7.3.7.3 `StatusCondition` generic with `getParent`

**Spec:** §7.3.7.3, p. 23 — "The interface StatusCondition, which extends
Condition, is a generic interface with a type parameter that is the type of the
Entity to which it belongs. This type parameter allows its getParent method to
be both polymorphic and type safe."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent`

**Status:** `done` — StatusCondition<ENTITY extends Entity<?, ?>> extends Condition and has ENTITY getParent(). The test types StatusCondition<DataReader<byte[]>> without a cast and checks assertSame(reader, condition.getParent()). It also checks triggering by SUBSCRIPTION_MATCHED and switching off through setEnabledStatuses. (commit `e72453fce1f`)
### §7.3.7.4 WaitSet based on `org.omg.dds.core.WaitSet`

**Spec:** §7.3.7.4, p. 23 — "Wait sets extend the base interface
org.omg.dds.core.WaitSet."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aSecondThreadWaitingOnTheSameWaitSetIsRefused`

**Status:** `done` — org.omg.dds.core.WaitSet is an abstract class with newWaitSet(ServiceEnvironment), attachCondition, detachCondition, getConditions and waitForConditions. The tests check attaching (a second attach has no effect), detaching (PreconditionNotMetException for a condition not attached) and the refusal of a second waiting thread. (commit `2601f4bf35a`)
### §7.3.7.4 `wait` as `waitForConditions`

**Spec:** §7.3.7.4, p. 24 — "Therefore, this PSM maps the DDS PIM wait operation
to the more explicit method name waitForConditions."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aWaitWithATimeoutEndsInATimeoutException`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported`

**Status:** `done` — WaitSet has six overloads of waitForConditions (no arguments, with a list of active conditions, with a timeout as Duration or long/TimeUnit); there is no method named wait. The tests check TimeoutException after 30 ms, blocking without a timeout until a GuardCondition is set, and the triggered condition returned in the passed list. (commit `2601f4bf35a`)

---

## §7.4 Domain Module

### §7.4 Package `org.omg.dds.domain`

**Spec:** §7.4, p. 24 — "This PSM realizes the Domain Module from the DDS
specification with the package org.omg.dds.domain. This package contains
DomainParticipant, DomainParticipantFactory, and so forth."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — The package org.omg.dds.domain contains DomainParticipant, DomainParticipantFactory, DomainParticipantListener, DomainParticipantAdapter, DomainParticipantQos and DomainParticipantFactoryQos. The test creates a DomainParticipant through DomainParticipantFactory and writes and reads with it. (commit `8bdac843874`)
### §7.4.1 DomainParticipantFactory a singleton per ServiceEnvironment

**Spec:** §7.4.1, p. 24 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton. An instance of this interface can be obtained
by passing that ServiceEnvironment to the factory’s getInstance method."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getInstanceRejectsNullEnvironment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::threadsThatAskForTheFactoryOfANewEnvironmentTogetherGetTheSameOne`

**Status:** `done` — getInstance(ServiceEnvironment) returns env.getSPI().getParticipantFactory(); ZeroSPI holds one DomainParticipantFactoryImpl per environment, created under a lock on first access. Tests: QuickstartApiTest::serviceEnvironmentBootstrapBySystemProperty (assertSame for the same environment, assertNotSame against the factory of another), ::getInstanceRejectsNullEnvironment, ParticipantFactoryEnvironmentTest::threadsThatAskForTheFactoryOfANewEnvironmentTogetherGetTheSameOne (eight threads, 25 rounds). (commit `9579b04127c`)
### §7.4.1 A factory of its own per ServiceEnvironment

**Spec:** §7.4.1, p. 24 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable`

**Status:** `done` — DomainParticipantFactoryImpl is created with its ServiceEnvironment (getEnvironment), keeps its own table of participants (lookupParticipant) and its own default QoS. Tests: QuickstartApiTest::serviceEnvironmentBootstrapBySystemProperty (assertNotSame between the factories of two environments), ParticipantFactoryEnvironmentTest::everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable (lookupParticipant of the other factory returns null). The InProcessBus is a shared singleton: the participants of both environments in a domain discover each other (same test). (commit `cf0d29f7efb`)
### §7.4.2 DomainParticipant as the interface `org.omg.dds.domain.DomainParticipant`

**Spec:** §7.4.2, p. 24 — "This PSM represents the DomainParticipant classifier
from the DDS PIM with the interface org.omg.dds.domain.DomainParticipant."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::findTopicReturnsTheTopicAndTimesOutOtherwise`

**Status:** `done` — org.omg.dds.domain.DomainParticipant is now an interface (extends Entity); the implementation is org.zerodds.domain.DomainParticipantImpl. The tests use DomainParticipant only as the interface type the factory returns. (commit `8bdac843874`)

---

## §7.5 Topic Module

### §7.5 Packages `org.omg.dds.type` and `org.omg.dds.topic`

**Spec:** §7.5, p. 24 — "This PSM realizes the Topic Module from the DDS
specification with the packages org.omg.dds.type and org.omg.dds.topic."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`

**Status:** `done` — Both packages exist: org.omg.dds.topic (Topic, TopicDescription, ContentFilteredTopic, MultiTopic, listeners, adapters, built-in data) and org.omg.dds.type (TypeSupport, annotations such as Key, TypeKind, dynamic.*). PubSubLoopbackTest uses Topic; DynamicTopicTest uses org.omg.dds.type.Key, TypeKind and the types from org.omg.dds.type.dynamic. (commit `5e1603d1c72`)
### §7.5.1 One class per type based on `org.omg.dds.type.TypeSupport`

**Spec:** §7.5.1, p. 24 — "As in the DDS PIM, each type to be published or
subscribed with DDS is represented by a class extending
org.omg.dds.type.TypeSupport."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription.getTypeSupport`),
`tools/idlc/src/java_support.rs` (`java_support::support`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicMadeFromAClassHasATypeSupportOfThatClass`,
`tools/idlc/tests/java_typesupport_base.rs::generated_type_supports_extend_the_psm_type_support`

**Status:** `done` — `org.omg.dds.type.TypeSupport` is an abstract class (`newData`, `getType`, `getTypeName`); `TypeSupportImpl` extends it, and `TopicImpl` holds an instance (`getTypeSupport`). `TopicOperationsTest::aTopicMadeFromAClassHasATypeSupportOfThatClass` checks that for a topic made from a class. The `Support_<tag>` classes of `ZdTypeSupports.java` that `tools/idlc/src/java_support.rs` generates extend `org.omg.dds.type.TypeSupport<…>` (with `getEnvironment`, `newData`, `getType`) and keep implementing `org.zerodds.cdr.TopicTypeSupport`; `java_typesupport_base` generates the Java for XCDR1 and XCDR2, compiles it with `javac -Xlint:all -Werror` against the sources of the binding and runs a program that uses the class through the base class (run on the Linux test host with a JDK). (commit `ac51b5643ec`)
### §7.5.1 Static operation `newTypeSupport` with a class and an optional name

**Spec:** §7.5.1, p. 24 — "Applications obtain instances of these interfaces by
calling the static base class operation newTypeSupport, passing this method the
Java Class object of the type they wish to support and optionally a name under
which that type should be registered."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName`

**Status:** `done` — TypeSupport.newTypeSupport(Class, ServiceEnvironment) and newTypeSupport(Class, String, ServiceEnvironment) lead through env.getSPI().newTypeSupport to a TypeSupportImpl. Tests: TopicOperationsTest::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass (getType, getTypeName, newData, a topic made from it, exchange of a sample), ::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName (form with a name). (commit `9579b04127c`)
### §7.5.1 Without a name, under the fully qualified name of the class

**Spec:** §7.5.1, p. 24 — "If no such name is provided, the type shall be
registered under the fully qualified name of the provided Class."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.getTypeName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass`

**Status:** `done` — In the OMG source newTypeSupport(Class, ServiceEnvironment) passes type.getClass().getName() ("java.lang.Class") as the name; org/omg/dds stays unchanged because the OMG source is normative. ZeroSPI.newTypeSupport (ServiceEnvironmentImpl) treats "java.lang.Class" for a type other than Class itself as "no name given" and registers under type.getName(), the fully qualified name of the class (§7.5.1). A type an application registers explicitly under "java.lang.Class" is thus not possible. Test: TopicOperationsTest::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass (getTypeName = String.class.getName(), also at the topic). (commit `9579b04127c`)
### §7.5.1 TypeSupport to `createTopic` instead of `register_type`

**Spec:** §7.5.1, p. 24 — "This PSM instead asks applications to instantiate
each TypeSupport object with a name and then provide that TypeSupport itself to
the create_topic method."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName`

**Status:** `done` — DomainParticipant.createTopic(String, TypeSupport<TYPE>) and the form with QoS, listener and status set create a TopicImpl that holds the TypeSupport (DomainParticipantImpl.newTopic); there is no register_type operation. Tests: TopicOperationsTest::newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass (getTypeName and getTypeSupport of the topic are those of the TypeSupport, writer and reader exchange a sample), ::aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName. (commit `deff69a0210`)
### §7.5.1 Rationale: static type safety

**Spec:** §7.5.1, p. 24 — "Design Rationale (non-normative) By requiring the
application to pass an instance of the generic TypeSupport interface to the
createTopic method, this PSM maintains unbroken static type safety all the way
from type registration to data publication or reception."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Design rationale (non-normative); no requirement that can be met.
### §7.5.2 Topic adds only the access to INCONSISTENT_TOPIC

**Spec:** §7.5.2, p. 24 — "The Topic interface adds only a single operation to
the set of those it inherits from its TopicDescription and DomainEntity
super-types: an accessor for the inconsistent topic status."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::topicsOfTheSameTypeOrInAnotherDomainAreConsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent`

**Status:** `done` — Topic extends TopicDescription and DomainEntity and adds getInconsistentTopicStatus; TopicImpl.onInconsistentTopic is called by DomainParticipantImpl.checkTopicConsistency and remoteEndpointSeen. Tests: TopicOperationsTest::aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener (getInconsistentTopicStatus: total_count 1, change 1 on the first and 0 on the second call), ::topicsOfTheSameTypeOrInAnotherDomainAreConsistent, ParticipantWireDiscoveryTest::aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent. (commit `deff69a0210`)
### §7.5.2 Topic generic over the data type

**Spec:** §7.5.2, p. 24–25 — "Topic—like all TopicDescriptions, and like
DataReader and DataWriter—is a generic interface with a type parameter that
identifies the type of the data with which it is associated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — Topic<TYPE> and TopicDescription<TYPE> are parameterized by the data type, like DataReader<TYPE> and DataWriter<TYPE>. The test uses Topic<Reading> to create DataWriter<Reading> and DataReader<Reading>. (commit `5e1603d1c72`)
### §7.5.2 Type safety from creation to writing and reading

**Spec:** §7.5.2, p. 25 — "Although Topic provides no type-specific operations,
its type parameter preserves type safety from Topic creation (actually all the
way from type registration) through data publication and/or subscription."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — createTopic(String, Class<TYPE>) returns Topic<TYPE>; Publisher.createDataWriter(Topic<TYPE>) returns DataWriter<TYPE>; Subscriber.createDataReader(TopicDescription<TYPE>) returns DataReader<TYPE>. QuickstartApiTest.singleArgFactoryReflectsTypedBean writes a Temperature object and reads Sample<Temperature>.getData().getCelsius() without a cast. (commit `426005cc641`)
### §7.5.2 TopicDescription extends `java.io.Closeable`

**Spec:** §7.5.2, p. 25 — "TopicDescription interface extends java.io.Closeable
to support specific new language constructs (e.g., Java try-with-resources) for
dealing with this interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/MultiTopic.java` (`MultiTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::participantCloseClosesContainedEntities`

**Status:** `done` — TopicDescription<TYPE> extends Closeable and DDSObject; ContentFilteredTopic and MultiTopic close themselves (entry removed from the participant's list, topics they hold released). Tests: TopicOperationsTest::aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds (try-with-resources for a ContentFilteredTopic and a MultiTopic, lookupTopicDescription null afterwards), ::closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant, ::closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause, CloseCascadeTest::participantCloseClosesContainedEntities. (commit `9579b04127c`)
### §7.5.3 ContentFilteredTopic and MultiTopic generic

**Spec:** §7.5.3, p. 25 — "ContentFilteredTopic and MultiTopic are generic
interfaces with type parameters that identify the types of the data with which
they are associated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/MultiTopic.java` (`MultiTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theContentFilteredTopicOfTheParticipantFiltersTheSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::theMultiTopicCarriesItsExpression`

**Status:** `done` — ContentFilteredTopic<TYPE> and MultiTopic<TYPE> are interfaces with a type parameter (both extend TopicDescription<TYPE>). ContentFilterTest uses ContentFilteredTopic<Reading> and checks that a reader on it receives only the sample with value > 50; MultiTopicTest uses MultiTopic<Reading> with expression and parameters. (commit `5e1603d1c72`)
### §7.5.3 The type parameter of a ContentFilteredTopic may be a supertype

**Spec:** §7.5.3, p. 25 — "Note that the type parameter of a
ContentFilteredTopic does not need to match that of its related Topic exactly;
it can be any supertype. For example, if the user-defined type Bar extends the
user-defined type Foo, a ContentFilteredTopic<Foo> can wrap a Topic<Bar>."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic.getRelatedTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createContentFilteredTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::aContentFilteredTopicOfASupertypeFiltersTheSamplesOfTheSubtypeByTheSupertypeFields`

**Status:** `done` — createContentFilteredTopic takes Topic<? extends TYPE>, getRelatedTopic returns Topic<? extends TYPE>; the filter evaluates the fields of the supertype on samples of the subtype (ContentFilter uses the class of the sample). Test: TopicOperationsTest::aContentFilteredTopicOfASupertypeFiltersTheSamplesOfTheSubtypeByTheSupertypeFields (ContentFilteredTopic<Foo> over Topic<Bar>, Bar extends Foo; the reader receives only the Bar with level > 5, as a Bar with its field label). (commit `5e1603d1c72`)
### §7.5.4 Data types of the built-in topics in `org.omg.dds.topic`, accessors only

**Spec:** §7.5.4, p. 25 — "The data types pertaining to the DDS built-in
discovery topics are contained in the package org.omg.dds.topic as well. These
types provide only accessors for their state, not mutators, to reflect the
read-only (from an application’s point of view) nature of discovery."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ParticipantBuiltinTopicData.java` (`ParticipantBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicBuiltinTopicData.java` (`TopicBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/SubscriptionBuiltinTopicData.java` (`SubscriptionBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/BuiltinTopicKey.java` (`BuiltinTopicKey`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantOmgApiTest.java::theApiIsTheOmgSourceCodeNotCountingCommentsAndWhiteSpace`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::getDiscoveredTopicDataCarriesTheTopicQos`

**Status:** `done` — The four interfaces and BuiltinTopicKey in org.omg.dds.topic have getters for their state and, like the OMG source, copyFrom(src). DDS-Java-PSM 1.0 clause 2 makes the OMG source files normative next to the PDF and decisive on conflict; the interfaces equal the OMG source except for comments and white space (ParticipantOmgApiTest::theApiIsTheOmgSourceCodeNotCountingCommentsAndWhiteSpace), so copyFrom belongs to the normative API. Setters exist only in the classes under org.zerodds.topic. The tests read the data as samples of the built-in readers. (commit `8a4d4ba74d3`)

---

## §7.6 Publication Module

### §7.6 Package `org.omg.dds.pub`

**Spec:** §7.6, p. 25 — "This PSM realizes the Publication Module from the DDS
specification with the package org.omg.dds.pub."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — The package org.omg.dds.pub contains Publisher, DataWriter, PublisherListener, DataWriterListener, PublisherAdapter, DataWriterAdapter, PublisherQos and DataWriterQos. The test uses Publisher and DataWriter from this package. (commit `426005cc641`)
### §7.6 Rationale: the name "pub"

**Spec:** §7.6, p. 25 — "Design Rationale (non-normative) The term “pub” has
been preferred to the longer “publication” for the sake of brevity [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Design rationale (non-normative); no requirement that can be met.
### §7.6.1 Publisher as the interface `org.omg.dds.pub.Publisher`

**Spec:** §7.6.1, p. 25 — "Publishers are represented by instances of the
org.omg.dds.pub.Publisher interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::containsEntityWalksTheContainment`

**Status:** `done` — org.omg.dds.pub.Publisher is an interface (extends DomainEntity<PublisherListener, PublisherQos>); PublisherImpl implements it. The tests hold the result of DomainParticipant.createPublisher as a Publisher and use createDataWriter on it. (commit `426005cc641`)
### §7.6.1 Overload `lookupDataWriter(Topic)`

**Spec:** §7.6.1, p. 25 — "In addition to the methods defined for this interface
by [DDS], it additionally provides a lookupDataWriter overload that acts on the
basis of a Topic object rather than solely on the topic’s name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.lookupDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.lookupDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::lookupDataWriterFindsByNameAndByTopicAndForgetsClosedWriters`

**Status:** `done` — Publisher has lookupDataWriter(String topicName) and lookupDataWriter(Topic<TYPE>); PublisherImpl returns the first writer with the topic name, or null (the Topic overload compares the name). Test: PublisherOperationsTest::lookupDataWriterFindsByNameAndByTopicAndForgetsClosedWriters (both overloads for an existing and a missing topic, null after close). (commit `426005cc641`)
### §7.6.2 DataWriter as the generic interface `org.omg.dds.pub.DataWriter`

**Spec:** §7.6.2, p. 25 — "DataWriters are represented by instances of the
org.omg.dds.pub.DataWriter interface. This is a generic interface, parameterized
by the type of the data samples to be written by a given writer."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — org.omg.dds.pub.DataWriter<TYPE> is now an interface (extends DomainEntity<DataWriterListener<TYPE>, DataWriterQos>), parameterized by the data type; DataWriterImpl implements it. The test uses DataWriter<Reading> to write. (commit `db1b9a760ba`)
### §7.6.2 No FooDataWriter, all type-specific operations on DataWriter

**Spec:** §7.6.2, p. 25 — "This PSM makes no such distinction: Java’s generic
wildcard syntax (DataWriter<?>) makes it possible to express all type-specific
DataWriter operations on the DataWriter interface itself; there is no
FooDataWriter."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.dispose`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.lookupInstance`),
`tools/idlc/src/java_support.rs` (`support`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::registerAndLookupInstance`

**Status:** `done` — write, registerInstance, unregisterInstance, dispose, getKeyValue and lookupInstance are on DataWriter<TYPE>; there is no FooDataWriter. tools/idlc/src/java_support.rs generates only ZdTypeSupports, no writer per type (read, not covered by a test). The tests write through DataWriter<Reading> and check registerInstance and lookupInstance (handle not nil, nil for an unknown key). (commit `db1b9a760ba`)
### §7.6.2 Timestamps and handles as overloads

**Spec:** §7.6.2, p. 25–26 — "Java does; therefore, the provision of timestamps
and/or instance handles is optional and is handled by means of method overloads.
For example, the write method provides the following overloads: one accepting a
data sample only, another accepting a sample and an instance handle, and another
accepting both of these as well as a timestamp."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.dispose`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::everyWriteOverloadCarriesItsSourceTimestampToTheSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed`

**Status:** `done` — DataWriter has write, registerInstance, unregisterInstance and dispose, each with the overloads sample, sample with Time or long/TimeUnit, sample with InstanceHandle, sample with InstanceHandle and timestamp. DataWriterImpl uses the timestamp in milliseconds as the source timestamp and checks the handle against the sample's instance. Tests: WriterHandleTest::everyWriteOverloadCarriesItsSourceTimestampToTheSample (timestamp and instance handle on the reader's sample for five forms), ::registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey, ::disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed (forms with Time and long/TimeUnit). (commit `eb7dddf3680`)
### §7.6.2 For unkeyed types a nil handle

**Spec:** §7.6.2, p. 26 — "Users of DataWriters of unkeyed types may choose to
call the overloads that accept instance handle arguments; if they do, the handle
argument must be a nil handle (as explained in the DDS PIM)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::anUnkeyedTypeIsWrittenWithTheNilHandle`

**Status:** `done` — The overloads with InstanceHandle (write, registerInstance, dispose, unregisterInstance) exist; DataWriterImpl.write(T, InstanceHandle, ...) allows HANDLE_NIL and deduces the instance from the key (for a type without a key, the one instance). Test: WriterHandleTest::anUnkeyedTypeIsWrittenWithTheNilHandle (byte[] topic, three forms with the nil handle, all three samples at the reader, timestamp of the forms with Time and long/TimeUnit). (commit `eb7dddf3680`)

---

## §7.7 Subscription Module

### §7.7 Package `org.omg.dds.sub`

**Spec:** §7.7, p. 26 — "This PSM realizes the Subscription Module from the DDS
specification with the package org.omg.dds.sub."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/QueryCondition.java` (`QueryCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — The package `org.omg.dds.sub` contains `Subscriber`, `DataReader`, `Sample`, `SubscriberListener`, `DataReaderListener`, `ReadCondition` and `QueryCondition`. The test uses `Subscriber`, `DataReader` and `Sample` from this package. The old statement that `SubscriberListener` and the conditions are missing no longer holds. (commit `426005cc641`)
### §7.7 Rationale: the name "sub"

**Spec:** §7.7, p. 26 — "Design Rationale (non-normative) The term “sub” has
been preferred to the longer “subscription” for the sake of brevity [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative (rationale for the package name).
### §7.7.1 Subscriber as the interface `org.omg.dds.sub.Subscriber`

**Spec:** §7.7.1, p. 26 — "Subscribers are represented by instances of the
org.omg.dds.sub.Subscriber interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — `Subscriber` is `public interface Subscriber`; `SubscriberImpl` implements it. The test holds the result of `participant.createSubscriber()` in a variable of type `Subscriber` and creates the reader through it. The old statement (final class with a public constructor) no longer holds. (commit `426005cc641`)
### §7.7.1 Overload `lookupDataReader(TopicDescription)`

**Spec:** §7.7.1, p. 26 — "In addition to the methods defined for this interface
by [DDS], it additionally provides a lookupDataReader overload that acts on the
basis of a TopicDescription object rather than solely on the topic description’s
name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.lookupDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.lookupDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::lookupDataReaderFindsTheReaderByTopicNameAndByTopicDescription`

**Status:** `done` — `lookupDataReader(String)` and `lookupDataReader(TopicDescription)` find the reader by name and by description, return `null` for an unknown topic and after the reader is closed; `null` as description gives `IllegalArgumentException`. (commit `426005cc641`)
### §7.7.2 A sample as one object of data and metadata, interface `org.omg.dds.sub.Sample`

**Spec:** §7.7.2, p. 26 — "This PSM follows the guidance of the DDS PIM rather
than of the IDL PSM: it represents data samples as single objects that
incorporate both data and metadata. Each sample is represented by an instance of
the org.omg.dds.sub.Sample interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::thePsmSampleIsAnInterface`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — `Sample<TYPE>` is an interface that carries `getData` and the metadata (`getSampleState`, `getViewState`, `getInstanceState` and others) in one object; `SampleImpl` implements it. `thePsmSampleIsAnInterface` checks `Sample.class.isInterface()` and that it is not final. The second test reads data from the samples that `take` returns. (commit `5e1603d1c72`)
### §7.7.2 `getData` returns the data

**Spec:** §7.7.2, p. 26 — "It provides its data via a getData method [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::readReturnsAFreshCopyNotTheReadersObject`

**Status:** `done` — `byteArrayQuickstartRoundTrip` writes `"hello".getBytes()` and reads it back through `Sample.getData()`; `readReturnsAFreshCopyNotTheReadersObject` checks the value `getData().value == 42`. `QuickstartApiTest.getDataAliasesData` compares `getData()` with itself and proves nothing. (commit `5e1603d1c72`)
### §7.7.2 `getData` returns `null` without valid data

**Spec:** §7.7.2, p. 26 — "if there is no valid data (corresponding to a false
value for SampleInfo.valid_data in the IDL PSM), this operation returns null."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::disposingAConstituentDisposesTheResultInstance`

**Status:** `done` — `DataReaderImpl.markState` adds a sample with `null` data for NOT_ALIVE_DISPOSED and NOT_ALIVE_NO_WRITERS. The test triggers such a sample in the MultiTopic reader by disposing a constituent topic and checks `assertNull(getData())` and `NOT_ALIVE_DISPOSED`. No test checks `getData() == null` on a reader of a plain topic, nor for NOT_ALIVE_NO_WRITERS. (commit `9426325d8b2`)
### §7.7.2 Metadata as read-only Java Bean properties

**Spec:** §7.7.2, p. 26 — "It provides its metadata (corresponding to the other
SampleInfo properties in the IDL PSM) as read-only Java-Bean-style properties."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getViewState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getInstanceState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSourceTimestamp`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getPublicationHandle`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::readMarksSamplesRead`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::viewStateIsNewUntilFirstAccess`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SampleStateTest.java::disposeSnapshotsInstanceStateForAllSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceDisposedAndWrittenAgainCountsADisposedGeneration`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::anInstanceWithoutWritersWrittenAgainCountsANoWritersGeneration`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::theRanksAreMeasuredWithinTheSamplesOfOneInstanceInTheCollection`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::theSourceTimestampIsTheOneTheWriterGave`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::thePublicationHandleNamesTheWriterThatIsMatchedWithTheReader`

**Status:** `done` — `DataReaderImpl.toSample` and `SampleImpl.of` supply disposed_generation_count, no_writers_generation_count, sample_rank, generation_rank and absolute_generation_rank from the counters and the collection (instead of the constants 0 and -1), plus source_timestamp and publication_handle. The tests in `ReaderSampleInfoTest` check all seven values. (commit `313af57405c`)
### §7.7.2 `Sample.Iterator` extends `ListIterator`, read-only

**Spec:** §7.7.2, p. 26 — "The Sample interface also defines a nested interface:
Sample.Iterator, an iterator that extends java.util.ListIterator. An iterator of
this type provides read-only access to an ordered series of samples of a single
type; such iterators are used by the DataReader read and take methods (see
below)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.Iterator`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.remove`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.set`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.add`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::theIteratorIsReadOnly`

**Status:** `done` — `SampleIterator.remove`, `set` and `add` throw `UnsupportedOperationException` (DDS-Java-PSM 1.0 §7.7.2). The test calls all three on an iterator from `DataReader.read()` and checks that the collection is unchanged. (commit `897b827da64`)
### §7.7.3 DataReader as the generic interface `org.omg.dds.sub.DataReader`

**Spec:** §7.7.3, p. 26 — "DataReaders are represented by instances of the
org.omg.dds.sub.DataReader interface. This is a generic interface, parameterized
by the type of the data samples to be read by a given reader."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`

**Status:** `done` — `DataReader<TYPE>` is `public interface DataReader<TYPE>`; `DataReaderImpl` implements it. The test uses `DataReader<Reading>` and reads `Sample<Reading>` of the data type `Reading`. The old statement (final class) no longer holds. (commit `db1b9a760ba`)
### §7.7.3 No FooDataReader, all type-specific operations on DataReader

**Spec:** §7.7.3, p. 26 — "This PSM makes no such distinction: Java’s generic
wildcard syntax (DataReader<?>) makes it possible to express all type-specific
DataReader operations on the DataReader interface itself; there is no
FooDataReader."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.read`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.take`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.lookupInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::getKeyValueReturnsASampleOfTheInstanceThatHasTheKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::lookupInstanceIsNilBeforeTheInstanceIsKnownAndTheHandleOfTheSampleAfterwards`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::readNextSampleReturnsTheNextSampleNotYetReadAndTakeNextSampleRemovesIt`

**Status:** `done` — All type-specific operations (`read`, `take`, `lookupInstance`, `getKeyValue`, `readNextSample`, `takeNextSample`) are on `DataReader`; `getKeyValue` sets the key fields (see the item on get_key_value) and all are tested. (commit `313af57405c`)
### §7.7.3 Two flavours of read and take: loan with `Sample.Iterator`, copy into an application `List`

**Spec:** §7.7.3, p. 26 — "The DataReader interface provides a number of read
and take method overloads. In addition to the distinction between read vs. take
semantics (as defined in the DDS PIM), these operations come in two “flavors.”
One that loans samples from a Service pool and returns a Sample.Iterator and
another that deeply copies into an application-provided java.util.List"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.read`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::readReturnsAFreshCopyNotTheReadersObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::takeReturnsAFreshCopyNotTheReadersObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::theIteratorWalksTheSamplesInBothDirections`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::takeReturnsTheSamplesInTheIteratorAndRemovesThemFromTheReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::readSelectorAndIntFormsReturnTheSameSamplesAsTheListForm`

**Status:** `done` — `read()`, `take()`, `read(int)`, `take(Selector)` return a `Sample.Iterator`: content and order, change of direction, READ after read, removal after take are tested. (commit `313af57405c`)
### §7.7.3 `returnLoan` on the iterator, the iterator is Closeable

**Spec:** §7.7.3, p. 27 — "Applications that read or take loans must eventually
return those loans; this PSM maps the return_loan operation from the DDS PIM to
an operation returnLoan on the Sample.Iterator. Moreover, the iterator
implements the Java.io.Closeable interface so that try-with-resources construct
can be used in Java 7."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.Iterator.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::closeEndsTheLoanAndClosingAgainIsHarmless`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::theIteratorWalksTheSamplesInBothDirections`

**Status:** `done` — `SampleIterator.close` no longer throws on the second call (Closeable, Sample.Iterator: close without a loan has no effect); try-with-resources over `read()` and `take()` is tested. The OMG API names the operation `close`; the name `returnLoan` of the PSM text does not exist there (org.omg.dds is not changed). (commit `897b827da64`)
### §7.7.3 Copy: list with any number of samples, afterwards the requested number

**Spec:** §7.7.3, p. 27 — "Applications that read or take copies may provide to
the Service destination Lists with any number of Samples already in them
(including empty Lists). Regardless of the number of Samples already in the list
when the method is called, when it returns, the List shall contain the number of
Samples requested by the application (or fewer, if fewer were available)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.read`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.read`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::takeReturnsAFreshCopyNotTheReadersObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theCopyFlavourReplacesWhatTheListHeldAndAllocatesAListForNull`

**Status:** `done` — `read(List)`, `take(List)`, `read(List, Selector)`, `take(List, Selector)` and `Selector.read(List)`/`take(List)` replace the content of the list with the returned samples (a long list is trimmed, a short one extended); `null` gives a new list. The number is `maxSamples` of the selector (DDS-Java-PSM §7.7.3: the number requested), not the length of the list. (commit `313af57405c`)
### §7.7.3 Copy may overwrite the samples passed in

**Spec:** §7.7.3, p. 27 — "The Service implementation may—for example, in order
to avoid object allocations—elect to overwrite the contents of any Samples that
are passed into it by invocations of these methods."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission for the implementation ("may"); it requires nothing. `DataReaderImpl.toSample` does not overwrite passed samples but creates new ones.
### §7.7.3 `DataReader.Selector` encapsulates the selection criteria

**Spec:** §7.7.3, p. 27 — "Instead of overloading several operation variants
that accept large numbers of infrequently used parameters, a DataReader.Selector
is provided to encapsulate various selection criteria (for example, sets of
sample, instance, and view states)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theSelectorOfReadAndTakeCarriesTheExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::anInstanceHandleLimitsReadAndTakeToThatInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::nextInstanceWalksTheInstancesInTheOrderOfTheirHandles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDataStateSelectsOnSampleViewAndInstanceStates`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::maxSamplesLimitsReadAndTakeAndUnlimitedIsAcceptedAsAValue`

**Status:** `done` — Selector.instance, nextInstance, dataState, Content and maxSamples are tested through `read(Selector)`/`take(Selector)`; nextInstance returns the next instance by handle (also together with instance(h)). (commit `313af57405c`)
### §7.7.3 `DataReader.select` returns a selector with the defaults

**Spec:** §7.7.3, p. 27 — "DataReader.select method returns a Selector object,
which encapsulates the default selection criteria."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.select`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.select`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theSelectorOfReadAndTakeCarriesTheExpression`

**Status:** `done` — `DataReaderImpl.select` returns a `SelectorImpl` bound to the reader. The test calls `reader.select().Content(...)` and reads from the same reader through `selector.read(got)`. The individual defaults of the selector are in the following items. (commit `db1b9a760ba`)
### §7.7.3 Selector default: `instanceHandle=null`

**Spec:** §7.7.3, p. 27 — "For portability, the default state of the Selector
object is defined as instanceHandle=null, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.getInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`

**Status:** `done` — `select().getInstance()` is `null`; `read()` with the default selector returns the samples of all instances (three instances, six samples). (commit `313af57405c`)
### §7.7.3 Selector default: `nextInstance=false`

**Spec:** §7.7.3, p. 27 — "[...] nextInstance=false, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.retrieveNextInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.retrieveNextInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`

**Status:** `done` — `select().retrieveNextInstance()` is `false`. (commit `313af57405c`)
### §7.7.3 Selector default: `dataState=any`

**Spec:** §7.7.3, p. 27 — "[...] dataState=any, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getDataState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.sampleStates`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`

**Status:** `done` — `SelectorImpl.getDataState()` returns `DataStateImpl.any(environment)` (all sample, view and instance states) when not set; the test compares with `assertEquals`. (commit `897b827da64`)
### §7.7.3 Selector default: `queryExpression=null`

**Spec:** §7.7.3, p. 27 — "[...] queryExpression=null, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getQueryExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theSelectorOfReadAndTakeCarriesTheExpression`

**Status:** `done` — `select().getQueryExpression()` is `null` and `read()` without an expression returns all samples; with `Content(...)` `collectLocked` filters. (commit `313af57405c`)
### §7.7.3 Selector default: `maxSamples=unlimited`

**Spec:** §7.7.3, p. 27 — "[...] and maxSamples=unlimited."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.maxSamples`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getMaxSamples`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::maxSamplesLimitsReadAndTakeAndUnlimitedIsAcceptedAsAValue`

**Status:** `done` — `SelectorImpl.maxSamples` accepts `ResourceLimits.LENGTH_UNLIMITED` and values from 0, other negative values give `IllegalArgumentException`; `getMaxSamples()` returns `LENGTH_UNLIMITED` by default. `read(Selector)`/`take(Selector)` limit to the count. (commit `897b827da64`)
### §7.7.3 Selector as a fluent interface with its own read and take

**Spec:** §7.7.3, p. 27 — "Selector provides fluent interface to modify the
default selection parameters. For convenience, Selector provides read and take
methods."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.read`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.take`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::theSelectorOfReadAndTakeCarriesTheExpression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theDefaultSelectorSelectsEverything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::anInstanceHandleLimitsReadAndTakeToThatInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::nextInstanceWalksTheInstancesInTheOrderOfTheirHandles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::theCopyFlavourReplacesWhatTheListHeldAndAllocatesAListForNull`

**Status:** `done` — The selector is fluent (`instance`, `nextInstance`, `dataState`, `Content`, `maxSamples` return it) and has `read()`, `read(List)`, `take()`, `take(List)`; chained calls are tested. (commit `313af57405c`)

---

## §7.8 Extensible and Dynamic Topic Types Module

### §7.8 Types of the TypeObject representation in `org.omg.dds.type.typeobject`

**Spec:** §7.8, p. 27 — "Types pertaining to TypeObject Type Representations are
defined in the package org.omg.dds.type.typeobject."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/TypeObject.java` (`TypeObject`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel.of`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectCodec.java` (`TypeObjectCodec`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectBuilder.java` (`TypeObjectBuilder.build`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjects.java` (`TypeObjects`),
`crates/java-omgdds/idl/typeobject/dds_xtypes_11_typeobject.idl`,
`crates/java-omgdds/scripts/generate-typeobject.sh`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::theMinimalTypeObjectHasTheOctetsAnnexBImplies`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::aJavaClassBecomesAStructureTypeWithItsMembers`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::theTypeIdIsTheLowerHalfOfTheMd5OfTheBigEndianSerialization`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::aChangeOfTheTypeChangesItsId`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::theEncodedTypeObjectIsDecodedToTheSameTypeObject`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::bytesThatAreNoTypeObjectGiveNoType`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::everyInterfaceOfThePackageIsImplementedAndAnObjectOfItComesBackOverTheWire`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::memberIdsAreNestedFinalClassesOfIntConstants`

**Status:** `done` — The package `org.omg.dds.type.typeobject` equals the OMG source file by file and is implemented. The interfaces mirror the TypeObject library of DDS-XTypes 1.1 Annex B (formal/2012-11-10, in the spec cache `.agent/spec/dds-xtypes-1.1.txt`); `idl/typeobject/dds_xtypes_11_typeobject.idl` takes the IDL of the annex in the annotation syntax of IDL 4 (`@extensibility` instead of `@Extensibility(...)`, `@id(<value>)`, `@external` instead of `@Shared`, `bitmask` instead of the `@BitSet` enumeration) with four changes that touch no value and no id: the type `_TypeId` is `TypeId` in Java; the member `annotation` is `_annotation` because `annotation` is a keyword in IDL 4; `Bound` is `CollectionBound` because `bound` and `Bound` are one identifier in IDL; `MapType`, `SequenceType` and `StringType` are `@mutable` like their base `CollectionType` (the annex says EXTENSIBLE, but the members have ids). The annex gives the enumerator `MODULE_ELEMENT` the value 22, which `STRUCTURE_TYPE_ELEMENT` also has; here it is 24. From the IDL `scripts/generate-typeobject.sh` generates the data types and the codec with `idlc` (XCDR1) (`org/zerodds/typeobject/generated`). The TypeId of a constructed type is the lower half of the MD5 of the big-endian serialization of the type (§7.3.4.1.2; the annex leaves open what the own `type_id` holds while hashing: `NO_TYPE` here). `TypeObjectBuilder` writes the TypeObject of a `DynamicType` (kind, name, extensibility, base type, members with id, key, optional, union labels, enumeration constants, bounds), `TypeObjectModel` puts the interfaces of the package on a decoded TypeObject, `TypeObjectToDynamic` maps back to the `DynamicType`. The interfaces have integers as type ids; the mapping is in `TypeObjectModel`: primitive types have the `TypeId` constant of the PSM, constructed ones `0x10000` plus the index in the library. An annotation usage is read but not written (the list of usages is empty); a bit set names no bits; a union has no discriminator type (the annex has no member for it). (commit `6d31a402794`)
### §7.8 Dynamic Language Binding in `org.omg.dds.type.dynamic`

**Spec:** §7.8, p. 27 — "Types pertaining to the Dynamic Language Binding are
defined in the package org.omg.dds.type.dynamic."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeMember.java` (`DynamicTypeMember`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicDataFactory.java` (`DynamicDataFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/TypeDescriptor.java` (`TypeDescriptor`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/MemberDescriptor.java` (`MemberDescriptor`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/AnnotationDescriptor.java` (`AnnotationDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::everyJavaPrimitiveHasTheDdsTypeOfTable81`

**Status:** `done` — The package `org.omg.dds.type.dynamic` contains `DynamicType`, `DynamicTypeMember`, `DynamicData`, both factories and the three descriptors (file by file equal to the OMG sources). `DynamicTopicTest` creates type and data through these interfaces and moves `DynamicData` through a writer and reader; `DynamicTypeReflectionTest` reads `DynamicType` from classes. The old statement (package missing, `org.zerodds.cdr.DynamicType` outside) no longer holds. (commit `2601f4bf35a`)
### §7.8 `TypeKind` in `org.omg.dds.type`

**Spec:** §7.8, p. 27 — "The TypeKind enumeration, which pertains to both of the
above, is defined in the package org.omg.dds.type."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeKind.java` (`TypeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::everyJavaPrimitiveHasTheDdsTypeOfTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`

**Status:** `done` — `org.omg.dds.type.TypeKind` exists (equal to the OMG source) with the kinds from `NO_TYPE` to `ANNOTATION_TYPE`, including the unsigned types, `CHAR_32_TYPE`, `FLOAT_128_TYPE`, enumeration, bit set, union and array. The tests compare `DynamicType.getKind()` with values of this enum. The old text (`DynamicType.Kind` with twelve values) describes the legacy `org.zerodds.cdr.DynamicType`. (commit `2601f4bf35a`)
### §7.8 Built-in types in `org.omg.dds.type.builtin`

**Spec:** §7.8, p. 27 — "The built-in types are defined in the package
org.omg.dds.type.builtin."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringIsAModifiableValueType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringCopyFromCloneAndEquals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringSurvivesJavaSerialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesIsAModifiableValueType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesCopyFromCloneAndEquals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesSurvivesJavaSerialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedStringTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedBytesTopicHasOneInstancePerKey`

**Status:** `done` — The package `org.omg.dds.type.builtin` exists with `KeyedString` and `KeyedBytes`; `BuiltinTypes` contains `KeyedStringImpl` and `KeyedBytesImpl`, and `ServiceEnvironment.getSPI().newKeyedString()`/`newKeyedBytes()` create them. `TypesBuiltinTest` checks `setKey`/`setValue` (the object is returned, the byte array is copied on set and get, the range form of `setValue`), `copyFrom`, `clone`, `equals`/`hashCode` and Java serialization, and one writer and one reader on a topic of the type. The TypeSupports of the types are in `BuiltinTypeSupports` (items on `DDS::KeyedString` and `DDS::KeyedBytes`). (commit `f8f6ba67b54`)
### §7.8 Extensions by [DDS-XTypes] within the built-in topic data types

**Spec:** §7.8, p. 27 — "Extensions by [DDS-XTypes] to types defined by [DDS]
(such as the built-in topic data types) are contained within those types."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData.getEquivalentTypeName`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData.getBaseTypeName`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData.getType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData.getRepresentation`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData.getTypeConsistency`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/PublicationBuiltinTopicDataImpl.java` (`PublicationBuiltinTopicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.describeType`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjects.java` (`TypeObjects.decodeOrNull`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theTypeMembersAreOnTheirParameterIdsAndRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::theTypeMembersOfTheOtherTwoBuiltinTopicsRoundTripToo`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::sedpCarriesTheTypeInformationAndTheTypeNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::thePublicationDataCarriesTheRepresentationAndTheTypeConsistencyOfItsWriter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::anExactTypeConsistencyForbidsCoercionOnTheWire`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::theSubscriptionDataCarriesTheRepresentationAndTheTypeConsistencyOfItsReader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::theTopicDataCarriesTheRepresentationAndTheTypeConsistencyOfItsTopic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::aRemoteEndpointAnnouncesTheRepresentationAndTheTypeConsistencyInSedp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::anEndpointThatStatesNeitherHasTheDefaultsOfTheEntityQos`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::theBuiltinDataDescribesItsTypeFromTheAnnouncedBytes`

**Status:** `done` — The data types of the built-in topics (`PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`, `TopicBuiltinTopicData`) carry `getEquivalentTypeName`, `getBaseTypeName`, `getType`, `getRepresentation` and `getTypeConsistency`. Tested are `equivalentTypeName`, `baseTypeName` and the type information octets over wire and SEDP (`BuiltinTopicQosTest`) and `getRepresentation` and `getTypeConsistency` from the QoS of local entities, on the parameter ids 0x0073 and 0x0074 in the serialization of the built-in topic data, after reading back and from a SEDP announcement (`TypesBuiltinTopicDataTest`); `EXACT_TYPE`, `EXACT_NAME` are written as DISALLOW_TYPE_COERCION (0), `ASSIGNABLE` and `DECLARED` as ALLOW_TYPE_COERCION (1). `getType()` returns the TypeObject of XTypes 1.1 (PID 0x0072, `@id(0x0072) type`): for local entities from the `DynamicType` of the type support (`TypeObjects.announce` in `BuiltinData.describeType`), for remote ones from the announced octets (`TypeObjects.decodeOrNull`); octets that are no TypeObject give `null`, and `getTypeObjectBytes()` keeps them. The mapping between the versions: on the wire XTypes 1.3 holds (the TypeInformation 0x0075 stays what the peers read), the parameter 0x0072 carries the TypeObject of version 1.1 of Annex B, which the Java PSM interfaces mirror. (commit `6d31a402794`)
### §7.8.1 Parts of the Dynamic Language Binding

**Spec:** §7.8.1, p. 27 — "The Dynamic Language Binding, as defined by
[DDS-XTypes], consists of DynamicType, DynamicTypeMember, DynamicData, their
respective factories, and several “descriptor” value types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Enumeration of the components; the requirements are in §7.8.1.1 to §7.8.1.5.
### §7.8.1.1 DynamicTypeFactory is a per-ServiceEnvironment singleton (`DynamicTypeFactory.getInstance(env)`)

**Spec:** §7.8.1.1, p. 27 — "This abstract factory is a per-ServiceEnvironment
singleton."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentCreatesTheStatusAndConditionFactories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::theFactoryIsASingletonPerServiceEnvironment`

**Status:** `done` — `DynamicTypeFactory.getInstance(env)` returns `env.getSPI().getTypeFactory()`; `ServiceEnvironmentImpl.getSPI` creates exactly one SPI with one `DynamicTypeFactoryImpl` per environment. `theFactoryIsASingletonPerServiceEnvironment` checks that two calls return the same object, that it is the one of the SPI, that a second `ServiceEnvironmentImpl` has another factory (with other primitive types) and that this one is stable in turn. (commit `86402eeaf1b`)
### §7.8.1.1 No `delete_instance` on `DynamicTypeFactory`, the Service manages the life cycle

**Spec:** §7.8.1.1, p. 27 — "The static delete_instance operations defined in
[DDS-XTypes] have been omitted in this PSM; the Service shall manage the life
cycles of the factory."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::thereIsNoDeleteInstanceAndTheServiceManagesTheLifeCycle`

**Status:** `done` — `org.omg.dds.type.dynamic.DynamicTypeFactory` and `DynamicTypeFactoryImpl` have no `deleteInstance` or similar operation; the life cycle follows the `ServiceEnvironment`. `thereIsNoDeleteInstanceAndTheServiceManagesTheLifeCycle` checks both classes by reflection for methods with `delete` in the name and that `getInstance` is the only static method of the abstract factory. (commit `86402eeaf1b`)
### §7.8.1.2 DynamicTypeSupport omitted

**Spec:** §7.8.1.2, p. 27 — "The interface DynamicTypeSupport defined by
[DDS-XTypes] does not provide any capability beyond what the generic TypeSupport
interface provided by this PSM already provides. Therefore, it has been omitted
from this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The spec describes that `DynamicTypeSupport` is omitted from this PSM; `org.omg.dds.type` and `org.omg.dds.type.dynamic` contain no such interface (directory listing read). The passage requires nothing that can be fulfilled.
### §7.8.1.3 DynamicType and DynamicTypeMember as interfaces following the mapping rules of this document

**Spec:** §7.8.1.3, p. 28 — "These interfaces are expressed in this PSM
according to the mapping rules expressed elsewhere in this document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeMember.java` (`DynamicTypeMember`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::theKeyAnnotationMakesTheTopicKeyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::membersAreFoundByIdAndByName`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::addMembersAddsEachDescriptorAndReturnsTheMembersOfTheType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::annotationsOfATypeAreReadByIndexAndByName`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::annotationsOfAMemberAreAListInTheOrderTheyWereSet`

**Status:** `done` — `DynamicType` and `DynamicTypeMember` are interfaces in `org.omg.dds.type.dynamic` with `getDescriptor`, `getName`, `getKind`, `getMember`, `getAllMembers`, the annotation operations and `addMember`; `DynamicTypeImpl` and `DynamicTypeMemberImpl` implement them. `TypesDynamicTypeTest` checks `getMember(int)`/`getMember(String)` (unknown ones too), `getAllMembers()`, `getAllMembers(int...)`, `getAllMembers(String...)`, `addMembers`, `getAnnotation(int)`/`getAnnotation(String)`, `getAllAnnotations`, `getAnnotations(int...)`/`getAnnotations(String...)`, `setAnnotation(s)` and on `DynamicTypeMember` `getId`, `getName`, `getDescriptor`, `getAnnotations`. `equals` and `clone` are in the item on §7.8.1.3 `equals`/`clone`. The legacy `org.zerodds.cdr.DynamicType` still exists alongside. (commit `86402eeaf1b`)
### §7.8.1.3 Result returned directly instead of an in-out parameter and ReturnCode_t

**Spec:** §7.8.1.3, p. 28 — "Operations that provide their result as an in-out
value in their first parameter and return DDS::ReturnCode_t have been changed
such that they instead return their results directly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.getDescriptor`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.getMember`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getPrimitiveType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.createType`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::everyJavaPrimitiveHasTheDdsTypeOfTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`

**Status:** `done` — The operations of `DynamicType`, `DynamicTypeMember` and the factory return their result as the return value (`getDescriptor()`, `getMember(...)`, `createType(...)`, `getPrimitiveType(...)`); there is no in-out parameter and no `ReturnCode_t`. The tests chain these calls, e.g. `type.getMember(name).getDescriptor().getType().getKind()`. (commit `2601f4bf35a`)
### §7.8.1.3 `equals` and `clone` as `Object.equals` and `Object.clone`

**Spec:** §7.8.1.3, p. 28 — "The equals and clone operations on these types have
been mapped to overrides of the Java-standard Object.equals and Object.clone,
respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.clone`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeDescriptorImpl.java` (`TypeDescriptorImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/MemberDescriptorImpl.java` (`MemberDescriptorImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/AnnotationDescriptorImpl.java` (`AnnotationDescriptorImpl.equals`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::twoTypesWithTheSamePropertiesAreEqual`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::membersAreEqualWhenTheirTypeDescriptorAndAnnotationsAre`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::cloneIsAnEqualTypeThatSharesNoMemberWithTheOriginal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::typeDescriptorsAreEqualWhenAllTheirPropertiesAre`

**Status:** `done` — `DynamicTypeImpl.equals`/`hashCode` compare, after XTypes 1.3 §7.5.2.8.4 (Table 54), the descriptor, the members (by id) and the annotations; `DynamicTypeMemberImpl.equals` compares, after §7.5.2.6.3, the descriptor, the annotations and the descriptor of the type the member belongs to; the descriptors compare all properties of Tables 50 and 53 and §7.5.2.3.2. `DynamicTypeImpl.clone()` returns an equal type that shares no member (changes to the copy or the original do not reach the other). The PSM has no `clone` on `DynamicTypeMember`. Tested in `TypesDynamicTypeTest`. (commit `86402eeaf1b`)
### §7.8.1.3 DynamicTypeMember as a reference type from `DynamicType.addMember`

**Spec:** §7.8.1.3, p. 28 — "DynamicTypeMember is a reference type, and
instances of it are obtained from DynamicType.addMember. This change avoids the
need to provide an additional factory method for DynamicTypeMember instances."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.addMember`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.addMember`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::theKeyAnnotationMakesTheTopicKeyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aStructureWithANestedStructureAndAnArray`

**Status:** `done` — `DynamicTypeImpl.addMember(MemberDescriptor)` adds the `DynamicTypeMember` to the type and returns that same object; `DynamicTypeFactory` has no factory method for members (only `newMemberDescriptor`). In `theKeyAnnotationMakesTheTopicKeyed` the test calls `setAnnotation(Key)` on the member returned by `addMember`, and the codec of the type is keyed afterwards (`isKeyed`). This shows that the return value is the member of the type. (commit `96bccc52ca9`)
### §7.8.1.3 `getAnnotations` instead of `get_annotation_count` and `get_annotation`, list not modifiable

**Spec:** §7.8.1.3, p. 28 — "On each type, the operations get_annotation_count
and get_annotation (by index) have been unified into a single getAnnotations
method that returns a list of annotations. The lists returned from these methods
shall not be modifiable."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeMember.java` (`DynamicTypeMember.getAnnotations`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl.getAnnotations`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.getAllAnnotations`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::theListsAndMapsOfAnnotationsCannotBeModified`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::annotationsOfAMemberAreAListInTheOrderTheyWereSet`

**Status:** `done` — `DynamicTypeMember.getAnnotations()` returns an unmodifiable list (`Collections.unmodifiableList` over a copy); `DynamicType.getAllAnnotations()`, `getAnnotations(int...)` and `getAnnotations(String...)` return unmodifiable maps. `theListsAndMapsOfAnnotationsCannotBeModified` checks `add`/`clear`/`remove`/`put` for `UnsupportedOperationException` and that the type is unchanged afterwards. `DynamicType` has `getAllAnnotations()` and the two `getAnnotations` forms instead of a single list method. The maps of `getAllMembers` stay modifiable copies (the specification names only the annotation lists). (commit `86402eeaf1b`)
### §7.8.1.3 Additional factory method `createType(Class<?>)` by §8

**Spec:** §7.8.1.3, p. 28 — "In addition to the methods specified by
[DDS-XTypes], DynamicTypeFactory provides one additional factory method:
createType(Class<?>). This method shall inspect the given type reflectively in
accordance with the Java Type Representation (see Clause 8) and instantiate an
equivalent DynamicType object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.createType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.getInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::everyJavaPrimitiveHasTheDdsTypeOfTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::aJavaTypeWithoutAKindIsRefused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::createTypeReflectsStructure`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::createTypeIsReachedThroughTheImplementationOfTheFactory`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::aDerivedClassIsAStructureWithTheSuperclassAsBaseType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::theBytesOfADerivedDynamicTypeAreThoseOfTheClass`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`

**Status:** `done` — `DynamicTypeFactoryImpl.createType(Class<?>)` maps a class by the Java Type Representation (Table 8.1, enumeration, structure, union, collections, map, annotations, superclass as base type, only the fields `java.io.Serializable` assigns) to a `DynamicType` and rejects classes without a kind with `IllegalArgumentException`; `DynamicTypeReflectionTest` and `TypesDynamicTypeTest` check that. The method is not in `org.omg.dds.type.dynamic.DynamicTypeFactory`: the source `omgdds_src.zip`, which prevails under §2 when they conflict, does not have it, and `org/omg/dds` stays equal to the source (`CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`). Access: `DynamicTypeFactoryImpl.getInstance(env)` returns the same factory as `DynamicTypeFactory.getInstance(env)` with the type of the implementation (`createTypeIsReachedThroughTheImplementationOfTheFactory`). The legacy `org.zerodds.cdr.DynamicTypeFactory.createType` returns the old `org.zerodds.cdr.DynamicType`. (commit `bf330e251b4`)
### §7.8.1.4 DynamicData as an interface following the mapping rules of this document

**Spec:** §7.8.1.4, p. 28 — "This interface is expressed in this PSM according
to the mapping rules expressed elsewhere in this document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicDataFactory.java` (`DynamicDataFactory.newDynamicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aStructureWithANestedStructureAndAnArray`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::everyPrimitiveKindIsReadBackWhatWasSet`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aSmallerSignedKindIsPromotedToTheLargerAccessor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aValueOutsideTheRangeOfTheMemberIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::descriptorsAndMemberIdsFollowTheType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::membersByNameAreTheMembersById`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::clearingANonOptionalMemberSetsItToItsDefault`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::clearingAnOptionalMemberRemovesIt`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::clearAllValuesClearsEveryMemberAndClearNonkeyValuesKeepsTheKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aLoanedStructureIsTheNestedValueAndChangesToItAreChangesToTheSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::theLoanIsReturnedOnlyOnceAndOnlyBySampleItCameFrom`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aValueThatDoesNotExistIsNotLoaned`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aLoanedPrimitiveIsWrittenBackOnReturn`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::cloneIsAnEqualSampleThatSharesNoValue`

**Status:** `done` — `DynamicData` is an interface in `org.omg.dds.type.dynamic`; `DynamicDataImpl` implements it, `DynamicDataFactory.newDynamicData` creates it. `TypesDynamicDataTest` checks the accessors for Int16, Int32, Int64, Float32, Float64, Char, Byte, Boolean and String (directly and after encoding/decoding), the promotion to the larger accessor, `getDescriptors`, `getMemberIdByName`, `getMemberIdAtIndex`, `clearValue` (not optional: default value; optional: removed), `clearAllValues`, `clearNonkeyValues` (key values stay), `loanValue`/`returnLoanedValue` (after XTypes 1.3 §7.5.2.11.7: without a copy, one loan at a time, `null` for missing values, `PreconditionNotMetException` for a foreign return; primitives under `MEMBER_ID_INVALID` with write-back) and `clone`. The accessors through writer, reader and codec are checked by `DynamicTopicTest`. (commit `86402eeaf1b`)
### §7.8.1.4 Result returned directly instead of an in-out parameter

**Spec:** §7.8.1.4, p. 28 — "Operations that provide their result as an in-out
value in their first parameter and return DDS::ReturnCode_t have been changed
such that they instead return their results directly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getInt32Value`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getStringValue`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getFloat64Values`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aStructureWithANestedStructureAndAnArray`

**Status:** `done` — The read accessors of `DynamicData` return the value (`getInt32Value` as `int`, `getStringValue` as `String`, `getComplexValue` as `DynamicData`); the array forms fill the passed array and return the count. The tests read values from the sample read and compare them. There is no `ReturnCode_t` and no in-out parameter for the result. (commit `ad26d2a5345`)
### §7.8.1.4 `equals` and `clone` as `Object.equals` and `Object.clone`

**Spec:** §7.8.1.4, p. 28 — "The equals and clone operations on these types have
been mapped to overrides of the Java-standard Object.equals and Object.clone,
respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.clone`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.hashCode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::cloneIsAnEqualSampleThatSharesNoValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aClonedArrayValueIsACopy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::sampleAreEqualWhenTypeAndValuesAreWithoutRegardToTheOrderOfTheMembers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aNonOptionalMemberOmittedByOneSampleEqualsItsDefaultInTheOther`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::nestedValuesAreComparedByValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::samplesWithTheSameFloat128ValueAreEqualWhateverTheScale`

**Status:** `done` — `DynamicDataImpl.clone()` copies the values deeply (nested `DynamicData`, arrays, lists and maps); `equals`/`hashCode` compare, after XTypes 1.3 §7.5.2.11.5, the types (equal by value) and the values without regard to the order of the members; a non-optional member that one sample omits equals the default value in the other; `BigDecimal` is compared by value (1.5 equals 1.50). Tested in `TypesDynamicDataTest`. (commit `86402eeaf1b`)
### §7.8.1.4 No methods for unsigned types, access through same-size or next-larger types

**Spec:** §7.8.1.4, p. 28 — "Methods dealing with unsigned integer types have
been omitted. Applications may access unsigned data using the signed type of the
same size (e.g., UInt32 becomes Int32), which preserves bitwise representation
but not logical value, or by using the signed type one size up (e.g., UInt32
becomes Int64), which preserves logical value but not representation [...]. In
the case of UInt64, the “type one size up” is java.math.BigInteger."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getInt32Value`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getBigIntegerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.getInt64Value`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.getBigIntegerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.setBigIntegerValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::anUnsignedKindIsReadAsTheSignedKindOfTheSameSizeAndAsTheOneSizeUp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::anUnsignedKindIsSetThroughTheOneSizeUpAsALogicalValue`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::unsignedMembersAreEncodedAndDecodedWithTheirFullRange`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aSequenceOfUnsigned64IsReadAsBigIntegers`

**Status:** `done` — `DynamicData` has no methods for unsigned types. `DynamicDataTypeSupport` holds UINT_16 as `Short`, UINT_32 as `Integer`, UINT_64 as `Long` (bit pattern). The accessors follow the kind of the member: accessors of the same size (`getInt16Value`, `getInt32Value`, `getInt64Value`) give the bit pattern, the next larger accessor the logical value (UInt16 as `int`, UInt32 as `long`, UInt64 as `getBigIntegerValue`). The setters take the bit pattern for the same size and, for a larger accessor, the logical value with a range check (`IllegalArgumentException`). `getBigIntegerValues`/`setBigIntegerValues` handle UINT_64 sequences. `TypesDynamicDataTest` checks this and the encoding/decoding of UINT_16, UINT_32 and UINT_64 (20 octets, `10 00 00 00 FF FF 00 00 FF FF FF FF FF…`). Sequences of UINT_16 and UINT_32 can be read through `getInt32Values`/`getInt64Values` only as the bit pattern of the same size. (commit `86402eeaf1b`)
### §7.8.1.4 Float128 as `java.math.BigDecimal`

**Spec:** §7.8.1.4, p. 28 — "The 128-bit Float128 type has been represented
using java.math.BigDecimal."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getBigDecimalValue`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.setBigDecimalValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aFloat128MemberIsABigDecimalOnTheWireAndBack`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::aFloat128MemberKeepsADigitStringADoubleCannotHold`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::samplesWithTheSameFloat128ValueAreEqualWhateverTheScale`

**Status:** `done` — `DynamicData` has `getBigDecimalValue`/`setBigDecimalValue` and `...Values` with `List<BigDecimal>`; `DynamicDataTypeSupport` writes and reads FLOAT_128_TYPE through `Float128.fromBigDecimal` and `toBigDecimal`. `TypesDynamicDataTest` sets a FLOAT_128 member (1.5: octets `10 00 00 00` and `00…00 80 FF 3F` little-endian; 2^100+1, which a `double` cannot hold; 0; −0.25), encodes, decodes and reads it back. The code generator still maps `long double` to `org.zerodds.wire.Float128` (`crates/lang-rules/src/adapter/java.rs`), independent of `DynamicData`. (commit `86402eeaf1b`)
### §7.8.1.5 Descriptors immutable, new ones from existing ones

**Spec:** §7.8.1.5, p. 28 — "This specification defines three descriptor
interfaces. The instances of descriptor interfaces are immutable and therefore,
provide methods to create new descriptor objects from the existing ones."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeDescriptorImpl.java` (`TypeDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/MemberDescriptorImpl.java` (`MemberDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/AnnotationDescriptorImpl.java` (`AnnotationDescriptorImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::aTypeDescriptorWithLeavesTheOriginalAsItWas`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::typeDescriptorsAreEqualWhenAllTheirPropertiesAre`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::aMemberDescriptorWithLeavesTheOriginalAsItWas`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::anAnnotationDescriptorWithLeavesTheOriginalAsItWas`

**Status:** `done` — The `with...` methods of all three descriptors copy and change the copy. `TypeDescriptorImpl.getBound` and `MemberDescriptorImpl.getLabel` return unmodifiable lists; `AnnotationDescriptorImpl.getAllValue` returns a copy. `TypesDynamicTypeTest` checks for each descriptor that `with...` leaves the original unchanged, that the lists cannot be modified and that descriptors are equal by all properties (`equals`/`hashCode`). (commit `86402eeaf1b`)
### §7.8.1.5 AnnotationDescriptor

**Spec:** §7.8.1.5, p. 28 — "• AnnotationDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/AnnotationDescriptor.java` (`AnnotationDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/AnnotationDescriptorImpl.java` (`AnnotationDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newAnnotationDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::theKeyAnnotationMakesTheTopicKeyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::theExtensibilityAnnotationChoosesTheFraming`

**Status:** `done` — `AnnotationDescriptor` exists as an interface, `DynamicTypeFactory.newAnnotationDescriptor` creates it. The tests use it to build `Key` and `Extensibility` (`withType`, `withValue`) and check their effect on the codec. (commit `96bccc52ca9`)
### §7.8.1.5 MemberDescriptor

**Spec:** §7.8.1.5, p. 28 — "• MemberDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/MemberDescriptor.java` (`MemberDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/MemberDescriptorImpl.java` (`MemberDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newMemberDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aStructureWithANestedStructureAndAnArray`

**Status:** `done` — `MemberDescriptor` exists as an interface, `DynamicTypeFactory.newMemberDescriptor` creates it. The tests set `withName`, `withId`, `withType` and `withLabel` and so create members of a structure and a union that the codec encodes and decodes. (commit `96bccc52ca9`)
### §7.8.1.5 TypeDescriptor

**Spec:** §7.8.1.5, p. 28 — "• TypeDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/TypeDescriptor.java` (`TypeDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeDescriptorImpl.java` (`TypeDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newTypeDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::aTypeDescriptorCarriesTheTypesItNames`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`

**Status:** `done` — `TypeDescriptor` exists as an interface, `DynamicTypeFactory.newTypeDescriptor` creates it. `aTypeDescriptorCarriesTheTypesItNames` checks `getKind` and `getElementType` after serialization; the union test sets `withKind`, `withName` and `withDiscriminatorType`. (commit `96bccc52ca9`)
### §7.8.2 Four built-in types of [DDS-XTypes]

**Spec:** §7.8.2, p. 29 — "[DDS-XTypes] specifies four built-in types:
DDS::String, DDS::Bytes, DDS::KeyedString, and DDS::KeyedBytes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Enumeration of the four built-in types of [DDS-XTypes]; the mappings are in the following items.
### §7.8.2 DDS::String as `java.lang.String`

**Spec:** §7.8.2, p. 29 — "• DDS::String is mapped to java.lang.String."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.isValueType`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.StringTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::stringIsTheBuiltInTypeDdsString`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aStringTopicIsDdsStringAndMovesSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aTruncatedOrInconsistentStringIsRefused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionFindsOnlyWhatWasCreated`

**Status:** `done` — `TypeSupportResolver.resolve(String.class)` returns `BuiltinTypeSupports.StringTypeSupport`: type name `DDS::String`, `APPENDABLE` (XTypes 1.3 Annex E: `struct _String { string value; }`), unchanged for `String` as a member of a structure. For example `"hi"` is `07 00 00 00 03 00 00 00 68 69 00` (DHEADER 7, length 3, characters, NUL), in big-endian with the words reversed; the string follows the setting of the item on narrow characters (truncation by default, UTF-8 as a named setting). `TypesBuiltinTest` checks the type name, the octets in both byte orders, the empty string, the refusal of truncated data and of `null`, and writes and reads a `String` sample over a topic. `DomainParticipantOperationsTest::lookupTopicDescriptionFindsOnlyWhatWasCreated` now expects `DDS::String` instead of `java::lang::String`. (commit `f8f6ba67b54`)
### §7.8.2 DDS::Bytes as `byte[]`

**Spec:** §7.8.2, p. 29 — "• DDS::Bytes is mapped to byte[]."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::typeNameIsDdsBytes`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::encodeFramesTheBytes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::largeByteArrayGrowsEncoderBuffer`

**Status:** `done` — `TypeSupportResolver.resolve(byte[].class)` returns `ByteArrayTypeSupport` with type name `DDS::Bytes` and APPENDABLE framing (DHEADER, length, octets). `typeNameIsDdsBytes` and `encodeFramesTheBytes` check the name and the octets (`07 00 00 00 03 00 00 00 01 02 03`); the two quickstart tests write and read 5 and 4096 bytes through a `Topic<byte[]>`. (commit `2601f4bf35a`)
### §7.8.2 `byte[]` topics as DDS::Bytes: type name and representation

**Spec:** §7.8.2, p. 29 — "[DDS-XTypes] specifies four built-in types:
DDS::String, DDS::Bytes, DDS::KeyedString, and DDS::KeyedBytes. [...] •
DDS::Bytes is mapped to byte[]."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.encode`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.decode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::typeNameIsDdsBytes`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::encodeFramesTheBytes`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::decodeUnframesTheBytes`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::bigEndianFramingFollowsTheChosenByteOrder`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::truncatedOrInconsistentFramingIsRefused`

**Status:** `done` — `getTypeName()` returns `DDS::Bytes`, `getExtensibility()` APPENDABLE; `encode` writes the DHEADER (length + 4), the sequence length and the octets, `decode` checks both and throws `XcdrException` on a mismatch. This matches XTypes 1.3 Annex E (`@extensibility(APPENDABLE) struct Bytes { ByteSeq value; }`). The tests check the name, the octets in little and big endian and the refusal of truncated or inconsistent framing. (commit `2601f4bf35a`)
### §7.8.2 DDS::KeyedString as a modifiable value type

**Spec:** §7.8.2, p. 29 — "• DDS::KeyedString and DDS::KeyedBytes are mapped to
modifiable value type interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes.KeyedStringImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newKeyedString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedStringTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.newData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringIsAModifiableValueType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringCopyFromCloneAndEquals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringSurvivesJavaSerialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedStringIsTheBuiltInTypeDdsKeyedString`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::theKeyHashOfAKeyedStringIsTheMd5OfItsKeyInBigEndian`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedStringTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedStringTopicSampleIsANewValueObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aTruncatedKeyedStringIsRefused`

**Status:** `done` — `KeyedString` and `KeyedStringImpl` are a modifiable value type (`setKey`/`setValue` return the object, `copyFrom`, `clone`, `equals`, `hashCode`, Java serialization without the environment). `TypeSupportResolver.resolve` returns `BuiltinTypeSupports.KeyedStringTypeSupport` for `KeyedString` and every subclass: type name `DDS::KeyedString`, `APPENDABLE`, `key` as the key. Example: key `k`, value `vv` is `0F 00 00 00 02 00 00 00 6B 00 00 00 03 00 00 00 76 76 00` (the length of the second string is 4-aligned). The key hash is the MD5 of the big-endian serialization of the key (an unbounded string has no maximum size ≤ 16 octets; XTypes 1.3 §7.6.8, DDSI-RTPS 2.5 §9.6.3.8): `302cf0a69dae0d4732bd21ea86fec48d` for `k`. `TypeSupportImpl.newData` creates the sample through the SPI. Tested in `TypesBuiltinTest`, including a writer and reader with one instance per key. (commit `f8f6ba67b54`)
### §7.8.2 DDS::KeyedBytes as a modifiable value type

**Spec:** §7.8.2, p. 29 — "• DDS::KeyedString and DDS::KeyedBytes are mapped to
modifiable value type interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes.KeyedBytesImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newKeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedBytesTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.newData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesIsAModifiableValueType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesCopyFromCloneAndEquals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesSurvivesJavaSerialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::keyedBytesIsTheBuiltInTypeDdsKeyedBytes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::theKeyHashOfKeyedBytesIsTheMd5OfItsKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedBytesTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedBytesTopicSampleIsANewValueObject`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aTruncatedKeyedBytesIsRefused`

**Status:** `done` — `KeyedBytes` and `KeyedBytesImpl` are a modifiable value type (`setValue(byte[])` and `setValue(byte[], int, int)` copy, `getValue` returns a copy, `copyFrom`, `clone`, `equals`, `hashCode`, Java serialization). `TypeSupportResolver.resolve` returns `BuiltinTypeSupports.KeyedBytesTypeSupport` for `KeyedBytes`: type name `DDS::KeyedBytes`, `APPENDABLE`, `key` as the key. Example: key `k`, value `01 02 03` is `0F 00 00 00 02 00 00 00 6B 00 00 00 03 00 00 00 01 02 03`. The key hash is the MD5 of the key as for `KeyedString`. Tested in `TypesBuiltinTest`, including a writer and reader with one instance per key. (commit `f8f6ba67b54`)
### §7.8.2 DataReader and DataWriter for the built-in types

**Spec:** §7.8.2, p. 29 — "The generic DataReader and DataWriter interfaces are
applicable to the built-in types."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aStringTopicIsDdsStringAndMovesSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedStringTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedBytesTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::bytesAreStillTheBuiltInTypeDdsBytes`

**Status:** `done` — One `DataWriter` and one `DataReader` each on a topic of `byte[]` (`DDS::Bytes`, `ByteArrayTypeSupport`), `String` (`DDS::String`), `KeyedString` and `KeyedBytes` write and read samples: `byteArrayQuickstartRoundTrip` and the tests `aStringTopicIsDdsStringAndMovesSamples`, `aKeyedStringTopicHasOneInstancePerKey`, `aKeyedBytesTopicHasOneInstancePerKey` in `TypesBuiltinTest`. (commit `f8f6ba67b54`)
### §7.8.2 Generic `createDataReader` and `createDataWriter` for the built-in types

**Spec:** §7.8.2, p. 29 — "Subscriber and Publisher provide generic
createDataReader and createDataWriter methods to create datareader and
datawriter for the built-in types, respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aStringTopicIsDdsStringAndMovesSamples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedStringTopicHasOneInstancePerKey`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::aKeyedBytesTopicHasOneInstancePerKey`

**Status:** `done` — `Publisher.createDataWriter(Topic<TYPE>)` and `Subscriber.createDataReader(TopicDescription<TYPE>)` are generic; the TypeSupport comes from the topic, which `TypeSupportResolver.resolve` fills with the built-in TypeSupport for `byte[]`, `String`, `KeyedString` and `KeyedBytes`. The tests use the overloads with the topic only (`byteArrayQuickstartRoundTrip`, `TypesBuiltinTest`). (commit `f8f6ba67b54`)
### §7.8.3 Types of TypeObject as value types according to the rules of this document

**Spec:** §7.8.3, p. 29 — "The types in this package are expressed as modifiable
value types according to the mapping rules expressed elsewhere in this
document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/generated/ZdTypes.java` (`ZdTypes`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::equalStateIsEqualObjects`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::theObjectsAreSerializable`

**Status:** `done` — The interfaces in `org.omg.dds.type.typeobject` of the OMG source have only read methods (the Javadoc of `TypeObject` and `Member` calls the objects immutable); the source prevails under §2, and `org/omg/dds` stays equal to it. The objects `TypeObjectModel` returns are value types per §7.3.3: `Serializable`, with `equals` and `hashCode` by state (`TypeObjectModelTest::equalStateIsEqualObjects`); a TypeObject is changed through the mutable data classes of the codec (`ZdTypes.DDS.*`: public fields, `TypeObjectCodec.encode`/`decode`) and `TypeObjectModel.of`. The changes of the mapping from §7.8.3 (constants in interfaces, `MemberId` classes) are tested (`TypesTypeObjectConstantsTest`). (commit `6d31a402794`)
### §7.8.3 Top-level constants inside interfaces, e.g. `Member.MEMBER_ID_INVALID`

**Spec:** §7.8.3, p. 29 — "• Top-level constants are moved into related
interfaces, for example: Member.MEMBER_ID_INVALID."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/Member.java` (`Member.MEMBER_ID_INVALID`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::theConstantsOfTheTopLevelAreMovedIntoTheInterfaces`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::theInterfacesTheConstantsLiveInAreTheOnesOfTheTopLevelTypes`

**Status:** `done` — `Member.MEMBER_ID_INVALID` is declared as the `int` constant `0x0FFFFFFF` in the interface `Member`; the value is the largest of the 28-bit range of member ids (XTypes 1.3 §7.2.2.4.4.4.4) and equals `MEMBER_ID_INVALID` in `crates/types/src/dynamic/descriptor.rs`. Further constants in interfaces: `CollectionType.UNBOUNDED_COLLECTION` (0) and the type ids in `TypeId` (equal to the `TypeKind.Values`). `TypesTypeObjectConstantsTest` reads them. (commit `e742bcfc4ad`)
### §7.8.3 Member IDs as nested final classes with `int` constants

**Spec:** §7.8.3, p. 29 — "Enumerations of member ID values are nested final
classes within the interfaces for which they provide the member’s IDs. These
classes have constant integer fields, for example:
MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/MapType.java` (`MapType.MemberId`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/Type.java` (`Type.MemberId`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::memberIdsAreNestedFinalClassesOfIntConstants`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::theMemberIdsAreThoseOfAnnexBOfXTypes11`

**Status:** `done` — The interfaces in `org.omg.dds.type.typeobject` each carry a nested final class `MemberId` with a private constructor and `int` constants, e.g. `MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID = 201`. `TypesTypeObjectConstantsTest` checks for 15 interfaces the names and values of the constants against the OMG source, that the class is final, public and static, has no public constructor, and that the values are exactly the `@ID` values of the getters of the same interface. `TypeObjectTest::theMemberIdsAreThoseOfAnnexBOfXTypes11` compares the values with the enumerations of DDS-XTypes 1.1 Annex B (`TypeMemberId` 0, 1; `StructureTypeMemberId` 100, 101; `MapTypeMemberId` 200, 201; …), which are in the spec cache now; the values agree. The source has in `CollectionType.MemberId` only `ELEMENT_TYPE_COLLECTIONTYPE_MEMBER_ID`; the constant `ELEMENT_SHARED_COLLECTIONTYPE_MEMBER_ID` (101) of the annex is not there, and `org/omg/dds` stays equal to the source. (commit `6d31a402794`)

---

## §8.1 General

### §8.1 Publish Java objects without code generation and without the Dynamic Language Binding

**Spec:** §8.1, p. 31 — "The Java Type Representation defined in this clause
provides a means for Java developers to publish and subscribe to DDS topics
typed by plain Java objects without resorting to code generation or the
reflective style of the Dynamic Language Binding."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — For a class without a generated TypeSupport, `TypeSupportResolver.resolve` falls back to `ReflectionTypeSupport`. `singleArgFactoryReflectsTypedBean` writes and reads a bean with an `int` and a `String` field through `createDataWriter(Topic)`/`createDataReader(Topic)` without IDL and without the Dynamic Language Binding; `pointMatchesV2` and `mixedIsByteIdenticalToTypedPath` check the octets against the generated path. (commit `8a5744821fd`)
### §8.1 At the same time a language binding, meant for run time

**Spec:** §8.1, p. 31 — "By its very nature as an expression of the Java
programming language, this Type Representation implicitly and simultaneously
defines a Language Binding for DDS types. [...] Therefore, this Type
Representation is intended for the run-time use of implementations of this PSM.
While this specification does not preclude Service implementations from using
this Type Representation for other purposes [...] such uses are non-normative
and unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Classification of the type representation as a language binding for run-time use; the specification requires nothing testable here.
### §8.1 Every `Serializable` type can be published

**Spec:** §8.1, p. 31 — "Any Java type that implements Serializable (directly or
indirectly) shall be available for publishing and/or subscribing over DDS as
defined below."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.refuseJdkType`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JdkValueTypes.java` (`JdkValueTypes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JdkSerializedValues.java` (`JdkSerializedValues`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JdkTopicTypeSupport.java` (`JdkTopicTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theMembersOfTheBaseClassComeFirst`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSerializableClassOfTheApplicationIsATopicType`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSerializableClassOfTheJdkIsPublishedInTheFormOfItsKind`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theTypesOfTable81AreNotRefusedAsTypesOfTheJdk`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSerializableClassOfTheJdkIsPublishedInTheFormOfItsKind`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theFormsOfDateAndUuidAreTheirTable81Parts`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::listsAndArraysOfJdkValuesAreSequences`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aJdkClassIsATopicTypeItself`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theOctetsOfAJdkValueNeverMakeAnotherClass`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aValueTheClassRefusesIsRefusedOnReading`

**Status:** `done` — `ReflectionTypeSupport` accepts every class and does not check `Serializable`; `serialVersionUID` is static and no member. A `Serializable` class of the application is published (`aSerializableClassOfTheApplicationIsATopicType`: octets `04 00 00 00 03 00 00 00 61 62 00`), with the rules of the Java Type Representation including the restriction of `java.io.Serializable` for superclasses. The `Serializable` classes of the JDK without an entry in Table 8.1 can be published, as the type of a topic, a field, a sequence element or an array component: `java.util.Date` (int64 milliseconds), `UUID` (two int64), `URI`, `Locale` and `Currency` (a string each) and `java.time.Instant`, `Duration`, `Period`, `LocalDate`, `LocalTime`, `LocalDateTime`, `OffsetDateTime`, `ZonedDateTime`, `ZoneOffset`, `Year`, `YearMonth`, `MonthDay` as a structure of Table 8.1 types (`JdkValueTypes`, the fields in the Javadoc of the class); every other concrete `Serializable` class of the JDK (such as `java.util.BitSet`) as its Java serialization, a `sequence<octet>` (`JdkSerializedValues`; only Java programs read it). Reading never makes another class: the stream may name only the declared class and classes of `java.lang` and may be at most 1 MiB long, else `XcdrException` (`theOctetsOfAJdkValueNeverMakeAnotherClass`); so does a value the class refuses (month 13). A JDK class that is not `Serializable` or not concrete is rejected when the type is created with an `XcdrException` that names the class, the field, Table 8.1 and a way out. A `DynamicType` (`createType`) and a TypeObject do not know these JDK classes; a type with such a field announces no TypeObject. (commit `341cadadc01`)
### §8.1 DDS serialization not equal to the JRE serialization

**Spec:** §8.1, p. 31 — "Note that the DDS serialization of a type will not
generally be the same as the JRE serialization of the same type, even if the
type designer's specification of which data to serialize can be shared between
these two mechanisms."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — A remark that DDS serialization differs from JRE serialization. `ReflectionTypeSupport` writes XCDR2 through `org.zerodds.wire.Writer` and does not consider `writeObject`, `readObject` or `serialPersistentFields`.

---

## §8.2 Default Mappings

### §8.2 Table 8.1: `int`, `java.lang.Integer` → INT32

**Spec:** §8.2, p. 31, Table 8.1 — "INT, JAVA.LANG.INTEGER | INT32"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqPrimitiveMatchesV5`

**Status:** `done` — `int` becomes INT32 (`Mixed.i` against the `Writer` path in the first test, octets `01 00 00 00 …` in `pointMatchesV2`); `Integer` as an element of `List<Integer>` in the second test. A field of the wrapper type holding `null` without `@Optional` throws `XcdrException` (`ReflectionTypeSupport.encodeMember`); the wrapper class has no test of its own (read from the code, not measured). (commit `8a5744821fd`)
### §8.2 Table 8.1: `short`, `java.lang.Short` → INT16

**Spec:** §8.2, p. 31, Table 8.1 — "SHORT, JAVA.LANG.SHORT | INT16"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — `mixedIsByteIdenticalToTypedPath` checks `short` as INT16 against the `Writer` path; `Short` takes the same branch in `encodeValue`/`decodeValue` and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `long`, `java.lang.Long` → INT64

**Spec:** §8.2, p. 31, Table 8.1 — "LONG, JAVA.LANG.LONG | INT64"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mutableLongRoundtrips`

**Status:** `done` — Both tests check `long` as INT64 (octets against the `Writer` path, round trip of a negative value in a MUTABLE type); `Long` takes the same branch and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `float`, `java.lang.Float` → FLOAT32

**Spec:** §8.2, p. 31, Table 8.1 — "FLOAT, JAVA.LANG.FLOAT | FLOAT32"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — `mixedIsByteIdenticalToTypedPath` checks `float` (2.5f) as FLOAT32 against the `Writer` path; `Float` takes the same branch and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `double`, `java.lang.Double` → FLOAT64

**Spec:** §8.2, p. 31, Table 8.1 — "DOUBLE, JAVA.LANG.DOUBLE | FLOAT64"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::sensorMatchesV8`

**Status:** `done` — Both tests check `double` as FLOAT64; `sensorMatchesV8` compares the octets `1F 85 EB 51 B8 1E 09 40` for 3.14. `Double` takes the same branch and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `char`, `java.lang.Character` → CHAR8

**Spec:** §8.2, p. 31, Table 8.1 — "CHAR, JAVA.LANG.CHARACTER | CHAR8"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`runtime/java/src/main/java/org/zerodds/wire/Writer.java` (`Writer.writeChar`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — `mixedIsByteIdenticalToTypedPath` checks `'Z'` as CHAR8 against the `Writer` path. `Character` takes the same branch. A character above U+00FF throws `WireException.OutOfRange` (`Writer.writeChar`; without `@SerializeAs` the exception is not converted to `XcdrException`) (read from the code, not measured). (commit `8a5744821fd`)
### §8.2 Table 8.1: `byte`, `java.lang.Byte` → BYTE

**Spec:** §8.2, p. 31, Table 8.1 — "BYTE, JAVA.LANG.BYTE | BYTE"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — `mixedIsByteIdenticalToTypedPath` checks `(byte) 0xAB` as BYTE; `Byte` takes the same branch and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `boolean`, `java.lang.Boolean` → BOOLEAN

**Spec:** §8.2, p. 31, Table 8.1 — "BOOLEAN, JAVA.LANG.BOOLEAN | BOOLEAN"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — `mixedIsByteIdenticalToTypedPath` checks `true` as BOOLEAN; `Boolean` takes the same branch and has no test of its own. (commit `8a5744821fd`)
### §8.2 Table 8.1: `java.lang.String` → STRING<CHAR8>

**Spec:** §8.2, p. 31, Table 8.1 — "JAVA.LANG.STRING | STRING<CHAR8>"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::stringMatchesV4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aNarrowStringIsTruncatedToItsLowOctetByDefault`

**Status:** `done` — `stringMatchesV4` checks the octets of `"hello"` (`06 00 00 00 68 65 6C 6C 6F 00`), `aNarrowStringIsTruncatedToItsLowOctetByDefault` those of `"été"` and `"AŁ"` (one octet per character, length including the terminator). The default for characters above U+007F is the truncation of §8.5.1, UTF-8 is the named setting `zerodds.java.narrow-string=utf8`; see the item "§8.5.1 Narrow characters: each Java character truncated to its least-significant byte". (commit `8a5744821fd`)
### §8.2 Table 8.1: `java.util.Map` → map

**Spec:** §8.2, p. 31, Table 8.1 — "java.util.Map | map"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeMap`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mapRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aMapIsADheaderACountAndTheEntries`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aMapIsReadIntoTheMapClassTheFieldDeclares`

**Status:** `done` — `mapRoundtrips`: `Map<String, Integer>` goes there and back, the order is kept. `aMapIsADheaderACountAndTheEntries` checks the octets (DHEADER, count, key/value pairs: `10 00 00 00 01 00 00 00 02 00 00 00 61 00 00 00 01 00 00 00`). The field has to be declared with type arguments, otherwise `elementClass` throws an `XcdrException`. (commit `a0d7584d6a4`)
### §8.2 Table 8.1: `java.lang.Collection` and array → sequence

**Spec:** §8.2, p. 31, Table 8.1 — "java.lang.Collection, array | sequence"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.collection`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqPrimitiveMatchesV5`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqStringMatchesV6`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::arrayRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::aSequenceIsWrittenForEveryCollection`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSequenceIsReadIntoTheCollectionTheFieldDeclares`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::emptyCollectionsAreReadIntoTheirClassesToo`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aCollectionTypeNoStandardClassFillsIsRefusedWhenRead`

**Status:** `done` — `List`, `Set`, `Queue`, `Deque`, `SortedSet`, `NavigableSet`, `EnumSet` and arrays become sequences; the tests check octets (V-5, V-6) and round trips. When reading, `ReflectionTypeSupport.collection` builds the class of the field: for an interface or abstract class the first of `ArrayList`, `LinkedHashSet`, `TreeSet`, `ArrayDeque`, `LinkedBlockingDeque` that it is (`Collection`/`List` `ArrayList`, `Set` `LinkedHashSet`, `SortedSet`/`NavigableSet` `TreeSet`, `Queue`/`Deque` `ArrayDeque`); `EnumSet` over the element class; a concrete class through its no-argument constructor. A field type that no standard class fills is an `XcdrException`, not another collection. `TypesJavaRepresentationTest::aSequenceIsReadIntoTheCollectionTheFieldDeclares` checks class and order for ten field types. (commit `8154449fb7b`)
### §8.2 Table 8.1: `java.lang.Object` → Structure

**Spec:** §8.2, p. 31, Table 8.1 — "java.lang.Object | Structure"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::nestedFinalInline`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`

**Status:** `done` — A class becomes a structure, nested classes become nested structures; `pointMatchesV2` and `nestedFinalInline` check the octets (FINAL without a DHEADER, 16 bytes for two `Point`). (commit `8a5744821fd`)
### §8.2 `@SerializeAs` changes the defaults per type and per field, `value()` of type `TypeKind`

**Spec:** §8.2, p. 32 — "A type designer may modify these defaults on a
type-by-type and/or field-by-field basis by applying the annotation
org.omg.dds.type.SerializeAs: public @interface SerializeAs { public TypeKind
value(); … }"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/SerializeAs.java` (`SerializeAs`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInALongKeepsTheLogicalValue`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aKindTable82DoesNotListForTheTypeIsRefused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aWideStringIsMarkedWithSerializeAs`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aKindOtherThanStructureOrUnionOnAClassIsRefused`

**Status:** `done` — `org.omg.dds.type.SerializeAs` has `TypeKind value()` (plus `collectionElementKind`, `mapKeyElementKind`, `bound`). On a field it changes the default (`JavaRepresentation.ofField`; the tests check UInt32, a combination Table 8.2 does not allow and a wide string); on a class `STRUCTURE_TYPE` or `UNION_TYPE` applies (`introspectUnion`; `aUnionIsItsDiscriminatorAndTheSelectedMember` checks the octets), other kinds on a class are refused by `introspectUnion` with `XcdrException` (`aKindOtherThanStructureOrUnionOnAClassIsRefused`). The annotation on methods has no effect. (commit `a0d7584d6a4`)

---

## §8.3 Metadata

### §8.3 IDL metadata as equivalent Java annotations

**Spec:** §8.3, p. 32 — "The type system metadata represented with built-in
annotations in the IDL Type Representation (such as @Key, @ID) shall be
represented by equivalent Java annotations unless otherwise noted."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.annotation`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Key.java` (`Key`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/ID.java` (`ID`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::sensorMatchesV8`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theTypeSystemAnnotationsAreRead`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::onlyTheTypeSystemAnnotationsCount`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::anAnnotationOverridesTheDefault`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theIdOfAMemberIsItsIdOnTheWireAndAnAbsentOptionalMemberHasNone`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theAnnotationsAreMembersOfTheDynamicTypeToo`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::annotationsOfAnotherPackageWithTheNamesOfTheTypeSystemAreIgnored`

**Status:** `done` — `@Key` (`sensorMatchesV8`, `theTypeSystemAnnotationsAreRead`) and `@Extensibility` (`anAnnotationOverridesTheDefault`) from `org.omg.dds.type` are read, foreign annotations of the same name are not. `ReflectionTypeSupport.annotation` compares the simple name without regard to case, so `org.omg.dds.type.ID` counts as `Id`. `theIdOfAMemberIsItsIdOnTheWireAndAnAbsentOptionalMemberHasNone` checks the octets on a MUTABLE class with `@ID(5)`, `@Optional @ID(9)` and `@MustUnderstand @ID(12)`: DHEADER, EMHEADERs `0x40000005`, `0x40000009` and `0xC000000C` (M flag for `@MustUnderstand`) each with a NEXTINT, no EMHEADER for an absent optional member; `theAnnotationsAreMembersOfTheDynamicTypeToo` the annotations in `createType`. `@Shared`, `@Nested`, `@Verbatim` and `@Value` have no effect on the wire form. (commit `8154449fb7b`)
### §8.3 Annotations in the package `org.omg.dds.type`

**Spec:** §8.3, p. 32 — "These annotations are in the package org.omg.dds.type."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Key.java` (`Key`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/ID.java` (`ID`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Optional.java` (`Optional`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/MustUnderstand.java` (`MustUnderstand`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Extensibility.java` (`Extensibility`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/SerializeAs.java` (`SerializeAs`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theTypeSystemAnnotationsAreRead`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInALongKeepsTheLogicalValue`

**Status:** `done` — The package `org.omg.dds.type` contains `Key`, `ID`, `Optional`, `MustUnderstand`, `Extensibility`, `Shared`, `Nested`, `Verbatim`, `Value`, `BitBound`, `BitSet`, `SerializeAs`, `UnionDiscriminator`, `UnionMember`. The tests import and use `Key`, `ID`, `Extensibility` and `SerializeAs` from this package (name requirement). `ReflectionTypeSupport.annotation` reads them from `org.omg.dds.type` and from `org.zerodds.types`. (commit `2601f4bf35a`)
### §8.3 Annotations on classes and fields, no specified behaviour on interfaces and methods

**Spec:** §8.3, p. 32 — "The annotations in this package logically govern the
behavior of concrete classes, not of polymorphic interfaces. As such, they may
be applied to classes or to their fields, as appropriate. Interface designers
wishing to document the DDS serialization of a type may additionally apply them
to interfaces or to property accessor and/or mutator methods; however, they have
no specified behavior in such cases."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::anAnnotationOverridesTheDefault`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::sensorMatchesV8`

**Status:** `done` — `introspect` reads annotations of the class (`@Extensibility`, `anAnnotationOverridesTheDefault`) and of its fields (`@Key`, `sensorMatchesV8`). Annotations on methods are not evaluated, which is what the specification leaves open for that case ("no specified behavior"). (commit `8a5744821fd`)

---

## §8.4 Primitive Types

### §8.4 Defaults by Table 8.2, changes through `@SerializeAs`

**Spec:** §8.4, p. 32 — "By default, Java primitive types are mapped to DDS
primitive types as defined in Table 8.2. The @SerializeAs annotation may be used
to modify these mappings as follows."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInALongKeepsTheLogicalValue`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aKindTable82DoesNotListForTheTypeIsRefused`

**Status:** `done` — The defaults of Table 8.2 apply without an annotation (`mixedIsByteIdenticalToTypedPath`). `@SerializeAs` changes them per field (`uint32HeldInALongKeepsTheLogicalValue`), and `JavaRepresentation.check` refuses combinations Table 8.2 does not list (`aKindTable82DoesNotListForTheTypeIsRefused`). (commit `b20252504bf`)
### §8.4 Table 8.2: Int32 from `int`, `Integer`

**Spec:** §8.4, p. 32, Table 8.2 — "Int32 | int, java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aKindTable82DoesNotListForTheTypeIsRefused`

**Status:** `done` — `int` and `Integer` are the default for Int32 (`mixedIsByteIdenticalToTypedPath` for `int`, `Integer` as a list element in `seqPrimitiveMatchesV5`); `@SerializeAs(INT_32_TYPE)` on a `long` is refused (`aKindTable82DoesNotListForTheTypeIsRefused`). (commit `b20252504bf`)
### §8.4 Table 8.2: UInt32 from `int`, `long`, `Integer`, `Long`

**Spec:** §8.4, p. 32, Table 8.2 — "UInt32 | int, long, java.lang.Integer,
java.lang.Long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInALongKeepsTheLogicalValue`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInAnIntKeepsTheBitPattern`

**Status:** `done` — `@SerializeAs(UINT_32_TYPE)` on `long` keeps the logical value (4,000,000,000 becomes `00 28 6B EE`, 2^32 and −1 are `XcdrException`); on `int` the bit pattern stays (−1 becomes `FF FF FF FF`). `check` allows `Integer` and `Long`, without a test of its own. (commit `b20252504bf`)
### §8.4 Table 8.2: Int16 from `short`, `Short`

**Spec:** §8.4, p. 32, Table 8.2 — "Int16 | short, java.lang.Short"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mixedIsByteIdenticalToTypedPath` checks `short` as Int16 against the `Writer` path. (commit `8a5744821fd`)
### §8.4 Table 8.2: UInt16 from `short`, `int`, `Short`, `Integer`

**Spec:** §8.4, p. 32, Table 8.2 — "UInt16 | short, int, java.lang.Short,
java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint16IntKeepsTheValueAndShortTheBitPattern`

**Status:** `done` — `@SerializeAs(UINT_16_TYPE)`: on `int` the value stays (65535 becomes `FF FF`, 65536 is `XcdrException`), on `short` the bit pattern (−2 becomes `FE FF`). The test checks the octets and the round trip; `check` allows `Short` and `Integer`, without a test of its own. (commit `b20252504bf`)
### §8.4 Table 8.2: Int64 from `long`, `Long`

**Spec:** §8.4, p. 32, Table 8.2 — "Int64 | long, java.lang.Long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mutableLongRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mutableLongRoundtrips` and `mixedIsByteIdenticalToTypedPath` check `long` as Int64. (commit `8a5744821fd`)
### §8.4 Table 8.2: UInt64 from `long`, `Long`, `BigInteger`

**Spec:** §8.4, p. 32, Table 8.2 — "UInt64 | long, java.lang.Long,
java.math.BigInteger"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint64TakesABigIntegerOrALong`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aBigIntegerWithoutAnnotationIsUInt64AndIsRangeChecked`

**Status:** `done` — `@SerializeAs(UINT_64_TYPE)` takes `BigInteger` (2^64−1 becomes eight `FF`, 2^64 is `XcdrException`) or `long` (bit pattern); without the annotation `BigInteger` is UInt64. In the default branch `ReflectionTypeSupport.encodeValue` a `BigInteger` outside 0 to 2^64−1 is refused with `XcdrException` and not cut by `longValue()`. `aBigIntegerWithoutAnnotationIsUInt64AndIsRangeChecked` checks 2^64−1 (eight `FF`), 0, 2^63 (`00…80`), −1, 2^64 and 2^100. (commit `8154449fb7b`)
### §8.4 Table 8.2: Float32 from `float`, `Float`

**Spec:** §8.4, p. 32, Table 8.2 — "Float32 | float, java.lang.Float"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mixedIsByteIdenticalToTypedPath` checks `float` as Float32 against the `Writer` path. (commit `8a5744821fd`)
### §8.4 Table 8.2: Float64 from `double`, `Double`

**Spec:** §8.4, p. 32, Table 8.2 — "Float64 | double, java.lang.Double"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::sensorMatchesV8`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `sensorMatchesV8` and `mixedIsByteIdenticalToTypedPath` check `double` as Float64. (commit `8a5744821fd`)
### §8.4 Table 8.2: Float128 from `double`, `Double`, `BigDecimal`

**Spec:** §8.4, p. 32, Table 8.2 — "Float128 | double, java.lang.Double,
java.math.BigDecimal"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::float128HeldInADouble`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`

**Status:** `done` — `@SerializeAs(FLOAT_128_TYPE)` on `double`: `float128HeldInADouble` checks the 16 octets for 1.0 (exponent `3FFF`) and the round trip. `BigDecimal` is Float128 without an annotation; `theKindsOfTable81RoundTrip` checks the round trip of 1.25 (numeric value, not scale), no octets. `check` allows `Double`, without a test of its own. (commit `b20252504bf`)
### §8.4 Table 8.2: Byte from `byte`, `Byte`

**Spec:** §8.4, p. 32, Table 8.2 — "Byte | byte, java.lang.Byte"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mixedIsByteIdenticalToTypedPath` checks `byte` as Byte. (commit `8a5744821fd`)
### §8.4 Table 8.2: Boolean from `boolean`, `Boolean`

**Spec:** §8.4, p. 32, Table 8.2 — "Boolean | boolean, java.lang.Boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mixedIsByteIdenticalToTypedPath` checks `boolean` as Boolean. (commit `8a5744821fd`)
### §8.4 Table 8.2: Char8 from `char`, `Character`

**Spec:** §8.4, p. 32, Table 8.2 — "Char8 | char, java.lang.Character"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`

**Status:** `done` — Default; `mixedIsByteIdenticalToTypedPath` checks `char` as Char8. (commit `8a5744821fd`)
### §8.4 Table 8.2: Char32 from `char`, `int`, `Character`, `Integer`

**Spec:** §8.4, p. 32, Table 8.2 — "Char32 | char, int, java.lang.Character,
java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::char32IsAFourOctetCodePoint`

**Status:** `done` — `@SerializeAs(CHAR_32_TYPE)` on `int`: code point U+1F600 becomes `00 F6 01 00`, round trip. `check` allows `char`, `Character` and `Integer`, without a test of its own; a value above U+10FFFF is `XcdrException`. (commit `b20252504bf`)
### §8.4 Unsigned values: keep the representation or the logical value

**Spec:** §8.4, p. 33 — "The DDS Type System ([DDS-XTypes]) defines unsigned
integer types; the Java type system does not. As a result, this Type
Representation must map unsigned values to “equivalent” signed types. Type
designers have two choices, reflected in the table above: • Preserve
representation [...] • Preserve logical value [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInALongKeepsTheLogicalValue`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint32HeldInAnIntKeepsTheBitPattern`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint16IntKeepsTheValueAndShortTheBitPattern`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::uint64TakesABigIntegerOrALong`

**Status:** `done` — Both choices are implemented and checked: Java types of the same size keep the bit pattern (`int` for UInt32, `short` for UInt16, `long` for UInt64), larger Java types the logical value (`long`, `int`, `BigInteger`), with `XcdrException` outside the range. (commit `b20252504bf`)

---

## §8.5 Collections

### §8.5 Three kinds of collections

**Spec:** §8.5, p. 33 — "[DDS-XTypes] recognizes three categories of
collections: strings (variable-length lists of narrow or wide characters),
sequences (variable-length lists of any single element type), and maps
(homogeneously typed key-value mappings)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Classification of the three collection categories of XTypes; the specification requires nothing testable here.
### §8.5.1 DDS strings as Java `String`

**Spec:** §8.5.1, p. 33 — "DDS strings, whether of narrow or wide characters,
are represented by Java String objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeString`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::stringMatchesV4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqStringMatchesV6`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aWideStringIsMarkedWithSerializeAs`

**Status:** `done` — A `String` field becomes a narrow string and back (`stringMatchesV4`, `seqStringMatchesV6`); with `@SerializeAs(STRING_TYPE, collectionElementKind = CHAR_32_TYPE)` a wide one (`aWideStringIsMarkedWithSerializeAs`: `06 00 00 00 E9 00 74 00 E9 00`). (commit `8a5744821fd`)
### §8.5.1 Narrow characters: every Java character truncated to its least-significant byte

**Spec:** §8.5.1, p. 33 — "• If a string is to be of narrow characters (the
default), each Java character shall be truncated to its least-significant byte."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.read`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::theTwoFormsAgreeOnTheCharactersBelowU0080`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::bothFormsOfANarrowStringAreWrittenByTheSameTypeSupport`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aNarrowStringIsTruncatedToItsLowOctetByDefault`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::theUtf8SettingWritesTheFormOfXTypes13`

**Status:** `done` — The default is the form of the clause: every character truncated to its least-significant byte (`aNarrowStringIsTruncatedToItsLowOctetByDefault`: `"AŁ"` becomes `41 41`, `"été"` becomes `E9 74 E9`); read back, one character per octet returns. The system property `zerodds.java.narrow-string=utf8` selects the UTF-8 form of XTypes 1.3 (§7.2.2.2.1.2.4, §7.4.1.1.2; `theUtf8SettingWritesTheFormOfXTypes13`: `"é"` becomes `C3 A9`, as in the interoperability corpus); on reading a missing NUL and invalid UTF-8 are rejected. `bothFormsOfANarrowStringAreWrittenByTheSameTypeSupport` writes the same instance of `ReflectionTypeSupport` one after the other in both forms (`E9` and `C3 A9`). Both forms are the same for characters up to U+007F. Peers that read the strings as UTF-8 (Cyclone DDS, Fast DDS, OpenDDS) need the setting `utf8` as soon as a character above U+007F occurs; the same holds for the built-in types `DDS::String` and `DDS::KeyedString`. The setting holds per Java process. (commit `db45fb6257d`)
### §8.5.1 Wide characters with `@SerializeAs`, one DDS character per code point

**Spec:** §8.5.1, p. 33 — "• If a string is to be of wide characters (in which
case it must be so marked with @SerializeAs), each Java code point shall become
a single DDS wide character."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/WideStrings.java` (`WideStrings`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aWideStringIsMarkedWithSerializeAs`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aNarrowStringIsTruncatedToItsLowOctetByDefault`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::char32IsAFourOctetCodePoint`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aCodePointAboveU10000OfAWideStringIsOneCharacter`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::theUtf16SettingWritesTheWstringOfXTypes13`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aWideStringRefusesWhatIsNoCodePoint`

**Status:** `done` — A string with `@SerializeAs(STRING_TYPE, collectionElementKind = CHAR_32_TYPE)` is a string of 32-bit characters, one character per code point (`WideStrings`): the length in octets (4n), then n unsigned 32-bit values without a terminator (`aWideStringIsMarkedWithSerializeAs`: `"été"` is `0C 00 00 00 E9 00 00 00 74 00 00 00 E9 00 00 00`). `aCodePointAboveU10000OfAWideStringIsOneCharacter` checks `"a𝄞"` (U+1D11E, two `char`): `08 00 00 00 61 00 00 00 1E D1 01 00`; a code point above U+FFFF is one character, not a surrogate pair. A value that is no code point is rejected on reading. The system property `zerodds.java.wide-string=utf16` selects the wstring of XTypes 1.3 §7.4.1.1.2, a sequence of UTF-16 units in which such a code point is two units (`theUtf16SettingWritesTheWstringOfXTypes13`: `06 00 00 00 61 00 34 D8 1E DD`; interoperability corpus `string-adjacent-widths`); peers that read a `wstring` need this setting. Without the annotation the string is narrow (see the item before). (commit `db45fb6257d`)
### §8.5.2 Every `java.util.Map` as a DDS map, unless marked with `@SerializeAs`

**Spec:** §8.5.2, p. 33 — "Any object whose class implements the interface
java.util.Map shall be considered a DDS map unless marked otherwise with
@SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.decodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.map`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mapRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::nestedGenericsAreFollowed`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aMapIsReadIntoTheMapClassTheFieldDeclares`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aMapMarkedAsAMapKeepsItsBound`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aMapIsADheaderACountAndTheEntries`

**Status:** `done` — A field whose declared type implements `Map` becomes a DDS map (`ReflectionTypeSupport.encodeValue`); `JavaRepresentation.check` allows only `Map` for `MAP_TYPE` and refuses every other kind on a `Map` with `XcdrException`. When reading, `ReflectionTypeSupport.map` (also for `JavaRepresentation.decodeMap`) builds the declared class: `Map` a `LinkedHashMap`, `SortedMap`/`NavigableMap` a `TreeMap`, `ConcurrentMap` a `ConcurrentHashMap`, `EnumMap` over the key class, a concrete class through its constructor. `aMapIsReadIntoTheMapClassTheFieldDeclares` checks seven field types, among them a field with `@SerializeAs(MAP_TYPE)`; `aMapMarkedAsAMapKeepsItsBound` the bound. (commit `8154449fb7b`)
### §8.5.3 Every `java.util.Collection` as a DDS sequence

**Spec:** §8.5.3, p. 33 — "Any object whose class implements the interface
java.util.Collection shall be considered DDS sequences unless marked otherwise
with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.collection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.typeOfClass`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::aSequenceIsWrittenForEveryCollection`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSequenceIsReadIntoTheCollectionTheFieldDeclares`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::emptyCollectionsAreReadIntoTheirClassesToo`

**Status:** `done` — Every `Collection` becomes a sequence (`encodeValue`, branch `Collection.class.isAssignableFrom`); when reading, `ReflectionTypeSupport.collection` builds the declared class (see the item on Table 8.1 `Collection`): `ArrayList`, `LinkedHashSet`, `TreeSet`, `ArrayDeque`, `EnumSet`, or the concrete class. `aSequenceIsReadIntoTheCollectionTheFieldDeclares` checks one field each of `Queue`, `Deque`, `SortedSet`, `NavigableSet`, `TreeSet`, `EnumSet`, `LinkedList`, `Collection`, `Set` and `List`. `createType` returns `SEQUENCE_TYPE` for `Collection<String>`. (commit `8154449fb7b`)
### §8.5.3 For a `List` the order of the list

**Spec:** §8.5.3, p. 33 — "If the class implements java.util.List, the order of
the elements in the sequence shall correspond exactly to the order of the
elements in the list."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.asList`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqPrimitiveMatchesV5`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::seqStringMatchesV6`

**Status:** `done` — `asList` takes the elements of a `List` in their order and `encodeSequence` writes them so. `seqPrimitiveMatchesV5` compares the bytes for [1,2,3], `seqStringMatchesV6` for ["a","bc"]; both also check the list read back. (commit `8a5744821fd`)
### §8.5.3 Otherwise the order of the iterator

**Spec:** §8.5.3, p. 33 — "Otherwise, the order of the elements in the sequence
shall correspond to that returned by the collection’s iterator."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.asList`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aSetIsWrittenInTheOrderOfItsIterator`

**Status:** `done` — `asList` copies a `Collection` with `new ArrayList<>(collection)`, i.e. in the order of its iterator. `aSetIsWrittenInTheOrderOfItsIterator` writes a `LinkedHashSet` ("b", "a") and a `TreeSet` with the same elements and compares all 46 octets: the first set writes `b` before `a`, the second `a` before `b`. (commit `8154449fb7b`)
### §8.5.3 Arrays as DDS sequences

**Spec:** §8.5.3, p. 33 — "Objects of array types shall be considered DDS
sequences unless marked otherwise with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::arrayRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::anArrayOfPrimitivesIsASequenceWithoutAnElementDheader`

**Status:** `done` — An array type goes through `encodeSequence` (length, elements) in `encodeValue` and comes back as an array through `listToArray`. `arrayRoundtrips` writes and reads an `int[]`; `anArrayOfPrimitivesIsASequenceWithoutAnElementDheader` checks the octets of `{1, 2}` (`02 00 00 00 01 00 00 00 02 00 00 00`); `createType` returns `SEQUENCE_TYPE` with `FLOAT_64_TYPE` as element for `double[]`. (commit `a0d7584d6a4`)
### §8.5.3 Collection or array as a DDS array with `@SerializeAs`

**Spec:** §8.5.3, p. 33 — "Any Java collection or array may be designated as a
DDS array with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeCollection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.decodeCollection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.typeOfClass`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::anArrayHasNoLengthAndASequenceMayNameItsElementKind`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::anArrayOfSeveralDimensionsIsWrittenRowMajor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::serializeAsChoosesTheKindOfTheMember`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aCollectionMarkedAsAnArrayHasAFixedLengthAndNoLengthWord`

**Status:** `done` — `@SerializeAs(ARRAY_TYPE, bound = ...)` on a Java array is tested: `int[3]` and `int[2][3]` are written row by row without a length, a wrong number of elements is refused with `XcdrException`, `createType` returns `ARRAY_TYPE` with the bounds. On a `Collection`, `aCollectionMarkedAsAnArrayHasAFixedLengthAndNoLengthWord` checks a `List<Integer>` with `bound = {3}`: octets `01 00 00 00 02 00 00 00 03 00 00 00` without a length, a list of two or four elements is refused, an `ArrayList` is read. (commit `a0d7584d6a4`)
### §8.5.3 Rationale: arrays are references

**Spec:** §8.5.3, p. 33 — "Design Rationale (non-normative) Objects of array
types must receive special care, because a Java array—like any Java object—is
stored by reference only. [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.

---

## §8.6 Aggregated Types

### §8.6 Two kinds: structures and unions

**Spec:** §8.6, p. 34 — "[DDS-XTypes] recognizes two kinds of aggregated types:
structures and unions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Classification in the specification.
### §8.6 No-argument constructor for non-nested types, also non-public

**Spec:** §8.6, p. 34 — "Any DDS type that is not a nested type (in the sense of
that word defined by [DDS-XTypes], as indicated in this Type Representation by
the annotation @Nested) must define a no-argument constructor for use by the
Service implementation. Service implementations shall have the capability to
invoke this constructor reflectively, even if it is not public."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiateCanonical`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::fieldsAndTheConstructorAreReachedWhateverTheirVisibility`

**Status:** `done` — `instantiate` calls `getDeclaredConstructor()` with `setAccessible(true)`. In `pointMatchesV2`, `Point` has a no-argument constructor that is visible only in its package and is read. If the constructor is missing, `instantiateCanonical` takes the canonical constructor (record `Mixed` in `mixedIsByteIdenticalToTypedPath`). `@Nested` is not evaluated in `ReflectionTypeSupport`. (commit `a0d7584d6a4`)
### §8.6 Fields of the class in the order of `getDeclaredFields`

**Spec:** §8.6, p. 34 — "The fields in the DDS structured type shall correspond
to those of the Java class. Their order shall be that returned by the method
java.lang.reflect.Class.getDeclaredFields."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mixedIsByteIdenticalToTypedPath`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theMembersOfTheBaseClassComeFirst`

**Status:** `done` — `fieldsOf` returns the fields of each class in the order of `getDeclaredFields()`, those of the superclasses before the class's own (item for §8.6.1.1). `mixedIsByteIdenticalToTypedPath` compares eight fields with the fixed order of the Writer, `pointMatchesV2` two fields with the bytes of the reference vector. (commit `8a5744821fd`)
### §8.6 Static and transient fields omitted

**Spec:** §8.6, p. 34 — "Static and/or transient fields shall be omitted."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::staticAndTransientFieldsCarryNoMember`

**Status:** `done` — `fieldsOf` skips static, transient and synthetic fields. `staticAndTransientFieldsCarryNoMember` checks a class with a static, a transient and a normal field: the octets hold only the normal one (`05 00 00 00`), when reading the transient field keeps the value from the constructor, `createType` lists only the normal member. (commit `8154449fb7b`)
### §8.6 Read and set fields regardless of their access level

**Spec:** §8.6, p. 34 — "Service implementations shall have the capability to
get and set the values of fields reflectively regardless of their declared
access level (e.g., public, protected, private)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.get`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::fieldsAndTheConstructorAreReachedWhateverTheirVisibility`

**Status:** `done` — `fieldsOf` calls `setAccessible(true)` on each field; `get` and `instantiate` read and set through `Field`. `singleArgFactoryReflectsTypedBean` writes and reads a class with `private` fields over a topic, `pointMatchesV2` one with fields without a modifier, `theKindsOfTable81RoundTrip` one with `public` fields, `fieldsAndTheConstructorAreReachedWhateverTheirVisibility` one with a `private`, a `protected`, a package and a `public` field. (commit `a0d7584d6a4`)
### §8.6 Not required: a SecurityManager prevents access

**Spec:** §8.6, p. 34 — "Service implementations need not address the following
cases: • A Java Security Manager (java.lang.SecurityManager) prevents privileged
access to a non-public field or constructor."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`)

**Tests:** —

**Status:** `n/a (informative)` — Exception to the obligation. `fieldsOf` turns a `SecurityException` or `InaccessibleObjectException` raised on access into an `XcdrException` naming the field.
### §8.6 Not required: final fields

**Spec:** §8.6, p. 34 — "• A field that is neither static nor transient is
declared final, preventing its value from being modified."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`)

**Tests:** —

**Status:** `n/a (informative)` — Exception to the obligation. `instantiate` sets fields through `Field.set` after `setAccessible`; for a record without a no-argument constructor it takes the canonical constructor (`mixedIsByteIdenticalToTypedPath` reads a record).
### §8.6 Not required: cycles of references

**Spec:** §8.6, p. 34 — "• Object references form a cycle. (Cycles are not
permitted by the DDS Type System.)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:** —

**Status:** `n/a (informative)` — Exception to the obligation. `DynamicTypeFactoryImpl.reflect` reports a class that contains itself through its fields as `IllegalArgumentException`; no test. An object graph with a cycle leads to unbounded recursion in `encodeStruct` (read from the code, not measured).
### §8.6.1 Every class that is neither a collection nor a map as a structure

**Spec:** §8.6.1, p. 34 — "Every Java class that is not a collection or map
shall be considered a structure by default."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.infoOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::nestedFinalInline`

**Status:** `done` — A class that is not a primitive, `String`, enumeration, array, `Collection` or `Map` in `encodeValue` and `decodeValue` goes through `infoOf` and `encodeStruct` as a structure. `nestedFinalInline` writes and reads a `Line` of two nested `Point` values inline (16 bytes, no DHEADER). (commit `8a5744821fd`)
### §8.6.1.1 Class extension as structure inheritance

**Spec:** §8.6.1.1, p. 34 — "Java class extension shall map to structure
inheritance in the DDS Type System [DDS-XTypes], subject to the restrictions
documented by the java.io.Serializable interface, such as those pertaining to
non-Serializable base types."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.baseTypeNames`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theMembersOfTheBaseClassComeFirst`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aLocalWriterDescribesTheBaseTypesOfItsType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::aDerivedClassIsAStructureWithTheSuperclassAsBaseType`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::theBytesOfADerivedDynamicTypeAreThoseOfTheClass`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theFieldsOfANonSerializableBaseOfASerializableClassAreNotPartOfTheType`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theDynamicTypeOfThatClassHasNoBaseType`

**Status:** `done` — `fieldsOf` puts the fields of the superclasses before the own ones; `theMembersOfTheBaseClassComeFirst` reads and writes a derived class with the base fields first (12 bytes: the DHEADER of the EXTENSIBLE class, two int32). `baseTypeNames` returns the names of the superclasses, `aLocalWriterDescribesTheBaseTypesOfItsType` checks them in the built-in topic data. `DynamicTypeFactoryImpl.reflect` sets the superclass (below `Object`) as base type and lists only the own members (`aDerivedClassIsAStructureWithTheSuperclassAsBaseType`); `theBytesOfADerivedDynamicTypeAreThoseOfTheClass` compares the octets of `DynamicData` with those of the class. The restriction of `java.io.Serializable` holds: in a `Serializable` class the fields of a superclass that is not `Serializable` have no member and no base type (`ReflectionTypeSupport.serializesFieldsOf`); the superclass is initialized by its own constructor on reading. A class that is not `Serializable` keeps all fields. (commit `f8598d5770f`)
### §8.6.1.2 FINAL when extending `Object` directly and `final`, or explicitly

**Spec:** §8.6.1.2, p. 34 — "• FINAL: If the class extends java.lang.Object
directly and is final, or if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::extensibilityFollowsTheClassShape`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::pointMatchesV2`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::createTypeReflectsStructure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::finalCanBeSaidOnAClassThatIsNotFinal`

**Status:** `done` — `defaultExtensibility` returns FINAL for a final class that extends `Object` directly. `extensibilityFollowsTheClassShape` checks that and that a non-final or derived class is not FINAL; `pointMatchesV2` checks the bytes without a DHEADER. Explicit `@Extensibility(FINAL_EXTENSIBILITY)` on a non-final class is mapped to FINAL by `mapExtensibility`; `finalCanBeSaidOnAClassThatIsNotFinal` checks `getExtensibility()`, the octets without a DHEADER (`01 00 00 00 02 00 00 00`) and the extensibility annotation in `createType`. (commit `8154449fb7b`)
### §8.6.1.2 EXTENSIBLE as the default in all other cases

**Spec:** §8.6.1.2, p. 34 — "• EXTENSIBLE: In all other cases, by default, or if
explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::extensibilityFollowsTheClassShape`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::aClassThatIsNotFinalIsWrittenWithADheader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::extensibleIsAppendable`

**Status:** `done` — `defaultExtensibility` returns `ExtensibilityKind.APPENDABLE` for every class that is not final or does not extend `Object` directly (EXTENSIBLE of §8.6.1.2; XTypes 1.3 calls the kind APPENDABLE, XCDR2 writes it with a DHEADER). `extensibilityFollowsTheClassShape` checks `OpenType` and `DerivedType`; `aClassThatIsNotFinalIsWrittenWithADheader` the octets `08 00 00 00 01 00 00 00 02 00 00 00`. (commit `8154449fb7b`)
### §8.6.1.2 EXTENSIBLE explicitly

**Spec:** §8.6.1.2, p. 34 — "• EXTENSIBLE: [...] or if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::extensibleIsAppendable`

**Status:** `done` — `mapExtensibility` maps `EXTENSIBLE_EXTENSIBILITY` to `ExtensibilityKind.APPENDABLE`. `extensibleIsAppendable` sets `@Extensibility(EXTENSIBLE_EXTENSIBILITY)`, checks `getExtensibility()` and the bytes (DHEADER 8, two `int`) and reads them back. (commit `8154449fb7b`)
### §8.6.1.2 MUTABLE only explicitly

**Spec:** §8.6.1.2, p. 34 — "• MUTABLE: Only if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mutableRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::mutableLongRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::anAnnotationOverridesTheDefault`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::extensibilityFollowsTheClassShape`

**Status:** `done` — `defaultExtensibility` never returns MUTABLE; `mapExtensibility` returns it for `@Extensibility(MUTABLE_EXTENSIBILITY)`. `anAnnotationOverridesTheDefault` checks MUTABLE with the annotation, `extensibilityFollowsTheClassShape` that the default does not yield it; `mutableRoundtrips` and `mutableLongRoundtrips` write and read (DHEADER and an EMHEADER per member). (commit `8a5744821fd`)
### §8.6.2 Union through `@SerializeAs`

**Spec:** §8.6.2, p. 34 — "Any class may be annotated as a union with
@SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aMutableUnionCarriesItsDiscriminatorAsMemberZero`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aDiscriminatorThatSelectsNoMemberIsWrittenAlone`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::aUnionHasItsDiscriminatorTypeAndLabels`

**Status:** `done` — A class with `@SerializeAs(UNION_TYPE)` becomes a union: the discriminator, then the selected member. `aUnionIsItsDiscriminatorAndTheSelectedMember` checks the bytes and the read, `aMutableUnionCarriesItsDiscriminatorAsMemberZero` the MUTABLE form, and `aUnionHasItsDiscriminatorTypeAndLabels` the `UNION_TYPE` description from `createType`. (commit `8a5744821fd`)
### §8.6.2 Exactly one field with `@UnionDiscriminator`

**Spec:** §8.6.2, p. 34 — "• Such a class must annotate exactly one field to be
the discriminator with @UnionDiscriminator."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aMisplacedUnionAnnotationIsRefused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`

**Status:** `done` — `introspectUnion` requires exactly one field with `@UnionDiscriminator`; with none or two it throws `XcdrException`. `aMisplacedUnionAnnotationIsRefused` checks the classes `NoDiscriminator` and `TwoDiscriminators` (and `NotAUnion`, `UnmarkedMember`, `DuplicateLabel`). (commit `8a5744821fd`)
### §8.6.2 The other fields with `@UnionMember` and the discriminator value

**Spec:** §8.6.2, p. 34 — "• All other fields that are not transient or static
must be annotated with @UnionMember, which shall identify the discriminator
value associated with that field."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.selectedMember`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aMisplacedUnionAnnotationIsRefused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aUnionIsItsDiscriminatorAndTheSelectedMember`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aDiscriminatorThatSelectsNoMemberIsWrittenAlone`

**Status:** `done` — `introspectUnion` requires `@UnionMember(discriminatorValue = ...)` on every field except the discriminator (static and transient fields are already excluded) and rejects duplicate values. `aMisplacedUnionAnnotationIsRefused` checks `UnmarkedMember` and `DuplicateLabel`; `aUnionIsItsDiscriminatorAndTheSelectedMember` selects the member by value, `aDiscriminatorThatSelectsNoMemberIsWrittenAlone` a value with no member. (commit `8a5744821fd`)

---

## §8.7 Enumerations and Bit Sets

### §8.7 Java `enum` as a DDS enumeration

**Spec:** §8.7, p. 35 — "By default, any Java enumeration class will be
considered to be a DDS enumeration."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.enumConstant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.enumeration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::theKindsOfTable81RoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::enumerationsStructuresCollectionsAndMapsFollowTable81`

**Status:** `done` — A Java `enum` is written as an enumeration (ordinal as `int32`) and read through `enumConstant`; a value outside the constants is an `XcdrException`. `theKindsOfTable81RoundTrip` writes and reads `Colour.BLUE`; `createType` yields `ENUMERATION_TYPE` with three members. No test checks the bytes of the enumeration. (commit `8a5744821fd`)
### §8.7 Enumeration annotatable as a bit set, its objects annotated as well

**Spec:** §8.7, p. 35 — "As in IDL, a type that is syntactically an enumeration
may be annotated as a bit set type. In this case, objects of these types must
also be annotated in order to be serialized correctly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.bitBound`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::theDefaultBitBoundIs32AndABitSetFieldNamesItsEnumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aMisplacedBitSetAnnotationIsRefused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::anEnumSetMarkedBitSetIsAMaskWithTheBitBoundOfItsEnumeration`

**Status:** `done` — An enumeration with `@BitSet` (and `@BitBound`, default 32) is a bit set type; `ofField` requires `@BitSet` on the member too and refuses a member of the bit set type without the annotation with `XcdrException`. `theDefaultBitBoundIs32AndABitSetFieldNamesItsEnumeration` writes a member `Wide one` (four octets, bit of constant C); `aMisplacedBitSetAnnotationIsRefused` checks `UnannotatedBitSetMember`. (commit `b20252504bf`)
### §8.7 `EnumSet` and `java.util.BitSet` with `@BitSet` as a bit set

**Spec:** §8.7, p. 35 — "A type member of type java.util.EnumSet or
java.util.BitSet will be serialized as a bit set if marked with @BitSet."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.constantsOfBits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::anEnumSetMarkedBitSetIsAMaskWithTheBitBoundOfItsEnumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::theDefaultBitBoundIs32AndABitSetFieldNamesItsEnumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aBitWithoutAConstantIsRefusedOnDecode`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::aCollectionOfBitSetValuesIsASequenceOfBitSets`

**Status:** `done` — A member of type `EnumSet` or `java.util.BitSet` with `@BitSet` is written as a bit set. `anEnumSetMarkedBitSetIsAMaskWithTheBitBoundOfItsEnumeration` checks one octet 0x05 for READ and EXEC, `theDefaultBitBoundIs32AndABitSetFieldNamesItsEnumeration` a `java.util.BitSet` with `elementType` (0x0a in four octets), `aBitWithoutAConstantIsRefusedOnDecode` bits without a constant. `bits` refuses more than 64 bits with `XcdrException`. (commit `b20252504bf`)

---

## §8.8 Modules

### §8.8 Segments of the package name as modules

**Spec:** §8.8, p. 35 — "Each segment of a Java type’s package name shall
correspond to a module in the DDS Type System [DDS-XTypes]. For example, a class
com.acme.project.TheClass would be in the nested modules com::acme::project."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.baseTypeNames`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionFindsOnlyWhatWasCreated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::aLocalWriterDescribesTheBaseTypesOfItsType`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::thePackageOfAClassIsTheModulesOfTheTypeName`

**Status:** `done` — `introspect` forms the type name from the class name with `::` instead of `.` and `$`. `thePackageOfAClassIsTheModulesOfTheTypeName` checks `com::acme::project::TypesTheClass` for a class in the package `com.acme.project` (as a topic and on the TypeSupport) and `org::zerodds::cdr::TypesJavaRepresentationTest::Skipped` for a nested class; `aLocalWriterDescribesTheBaseTypesOfItsType` the name of a nested superclass in `getBaseTypeName`. `String` has the name `DDS::String` (item on `DDS::String`). (commit `8154449fb7b`)

---

## §8.9 Annotations

### §8.9 Java annotation types not mapped by default

**Spec:** §8.9, p. 35 — "This Type Representation ignores Java annotation types
by default."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.annotation`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::onlyTheTypeSystemAnnotationsCount`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::anAnnotationTypeIsRepresentedOnlyWhenMarked`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::annotationsOfAnotherPackageWithTheNamesOfTheTypeSystemAreIgnored`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::anIdAndAMustUnderstandOfAnotherPackageDoNotReachTheEmheader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::anOptionalOfAnotherPackageDoesNotMakeAMemberOptional`

**Status:** `done` — `annotation` recognizes only annotations from `org.omg.dds.type` (and `org.zerodds.types`); foreign annotations with the same simple name do not count. `onlyTheTypeSystemAnnotationsCount` uses `ForeignKey` and `ForeignId`; the new tests use annotations `Key`, `ID`, `Optional`, `MustUnderstand` and `Extensibility` from the package `org.zerodds.cdr.foreign`, with the same simple names: a foreign `Key` does not make the type keyed, a foreign `Extensibility("MUTABLE")` does not make the class MUTABLE, a foreign `ID(77)` does not change the member id, a foreign `MustUnderstand` does not set the M flag, a foreign `Optional` does not make a member optional. `anAnnotationTypeIsRepresentedOnlyWhenMarked` checks that `createType` refuses an unmarked annotation type. (commit `8154449fb7b`)
### §8.9 Annotations in the type system only with `@SerializeAs`

**Spec:** §8.9, p. 35 — "Java annotations that are intended to be represented
explicitly within the DDS Type System must be so annotated with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflectAnnotationType`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::anAnnotationTypeIsRepresentedOnlyWhenMarked`

**Status:** `done` — `reflectAnnotationType` maps an annotation type only with `@SerializeAs(ANNOTATION_TYPE)`, with one member per element and its default value. The test checks `ANNOTATION_TYPE`, the member kinds `STRING_TYPE` and `INT_32_TYPE`, the default "none" and the rejection of the unmarked type. (commit `74eb250de33`)

---

## §9.1 Type Mapping

### §9.1 Type system by [DDS-XTypes], other aspects by its Plain Language Binding

**Spec:** §9.1, p. 37 — "The type system for DDS topic types is defined by the
Extensible and Dynamic Topic Types for DDS specification [DDS-XTypes]. This sub
clause defines the set of rules to be used in order to map abstract DDS topic
types into Java types that can be used by application programmers. Those aspects
of the DDS Type System that are not addressed below are as specified in the
Plain Language Binding as defined by [DDS- XTypes] (which in turn is defined in
terms of an IDL-to-Java mapping [Java-MAP])."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`JavaMapping`)

**Tests:** —

**Status:** `n/a (informative)` — Names the sources of the rules. The Java generator has two mappings: `--java-mapping psm` (default, Clause 9) and `--java-mapping omg` (IDL to Java, public fields); see the items for §9.1.1 and §9.1.2. (commit `f8b91f3fdc1`)
### §9.1.1 Aggregation types as a final Java class

**Spec:** §9.1.1, p. 37 — "DDS aggregation types shall be mapped to a final Java
class."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`data_class`),
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::the_psm_types_and_codec_compile_without_a_warning`

**Status:** `done` — Both mappings generate `public static final class` per structure. Under `JavaMapping::Psm` the test checks `public static final class Point {` and `Track {` in the generated text; `the_psm_types_and_codec_compile_without_a_warning` compiles the output with `javac`. The default of `idlc generate --lang java` is `psm`, Clause 9 (`the_java_mapping_defaults_to_psm_and_omg_is_selectable`); `--java-mapping omg` selects IDL to Java. (commit `f8b91f3fdc1`)
### §9.1.1 Attributes encapsulated

**Spec:** §9.1.1, p. 37 — "Contained attributes shall be encapsulated."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`field`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Under `JavaMapping::Psm` every member is a `private` field with accessors. The first test checks `private int x;` and that `public int x;` is absent; the second that the codec accesses through `getX()`/`setX(...)` and no direct field access remains. Under `omg` (`--java-mapping omg`) the fields stay public. (commit `f8b91f3fdc1`)
### §9.1.1 Java Bean accessors

**Spec:** §9.1.1, p. 37 — "Java Bean style accessors shall be provided."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`getter_name`),
`crates/lang-rules/src/adapter/java.rs` (`setter_name`),
`crates/lang-rules/src/adapter/java.rs` (`boolean_getter_name`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::a_union_keeps_its_discriminator_as_the_property_discriminator`

**Status:** `done` — Under `JavaMapping::Psm` the generator writes `getX()` and `setX(int v)`, for `boolean` also `isOn()`, and for a union `getDiscriminator()`/`setDiscriminator`. The tests check the generated text; `the_psm_types_and_codec_compile_without_a_warning` compiles it. Under `omg` there are no accessors. (commit `f8b91f3fdc1`)
### §9.1.1 Special rules for `boolean` properties permitted

**Spec:** §9.1.1, p. 37 — "Special mapping rules for boolean properties are
allowed."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`boolean_getter_name`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`

**Status:** `n/a (informative)` — A permission. The generator uses it: `isOn()` and `getOn()` for a `boolean` member, checked in the test for §9.1.1. (commit `f8b91f3fdc1`)
### §9.1.1 Internal state private

**Spec:** §9.1.1, p. 37 — "The representation of internal state shall be
private."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`field`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::a_union_keeps_its_discriminator_as_the_property_discriminator`

**Status:** `done` — Under `JavaMapping::Psm` the fields and the discriminator of a union are `private` (`private int x;`, `private int _d;`); the tests check the text, and that `public int x;` is absent. (commit `f8b91f3fdc1`)
### §9.1.2 Unbounded sequences as `Collection<E>`, encapsulated, with Java Bean accessors

**Spec:** §9.1.2, p. 37 — "Unbounded DDS sequences are mapped to Collection<E>
interface. The state is encapsulated and getters/setters are provided through
bean style property accessors."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`),
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`,
`crates/lang-rules/tests/java_psm_mapping.rs::the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Under `JavaMapping::Psm` `sequence<long>` becomes a `private java.util.Collection<java.lang.Integer>` with `getSamples()` and `setSamples(...)`; nested sequences are `Collection<Collection<String>>`. The test checks the text, the second test the codec through `Sequences`; `omg` (`--java-mapping omg`) still maps to `T[]`. (commit `f8b91f3fdc1`)
### §9.1.2 Bounded sequences as Java arrays

**Spec:** §9.1.2, p. 37 — "Bounded sequences and arrays are mapped to Java
arrays."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`,
`crates/lang-rules/tests/java_psm_mapping.rs::the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Under `JavaMapping::Psm` `sequence<long, 4>` stays `private int[] bounded;`; `type_at` chooses `Collection` only for `Bound::Unbounded`. The test checks the text, the codec trims with `Sequences.first(v, 4)`. (commit `f8b91f3fdc1`)
### §9.1.2 Arrays as Java arrays

**Spec:** §9.1.2, p. 37 — "Bounded sequences and arrays are mapped to Java
arrays."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`)

**Tests:**
`crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`,
`crates/lang-rules/tests/java_psm_mapping.rs::an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`

**Status:** `done` — `type_at`, branch `TypeKind::Array`: `long a[3]` becomes `int[]`, `long g[2][3]` becomes `int[][]`. `a_sequence_and_an_array_are_both_java_arrays` checks that under `omg`; under `psm` the second test checks `private ZdTypes.demo.Point[] vicinity;` and an array of sequences as `Collection<Integer>[]`. (commit `f8b91f3fdc1`)

---

## §9.2 Example

### §9.2 Example `Point` and `RadarTrack`

**Spec:** §9.2, p. 38–39 — "Example (non-normative) [...] public final class
Point { Point(); Point(int x, int y, java.lang.Integer z); int getX(); void
setX(int32 v); [...] }; public class final RadarTrack { [...]
java.util.Collection<byte> getPlot(); [...] };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. It is not compilable Java (`int32`, `public class final`, `Collection<byte>`, `};`). `idlc generate --lang java --java-mapping psm` generates final classes with private fields, Bean accessors and `Collection<E>` for unbounded sequences. (commit `f8b91f3fdc1`)

---

## Annex A Java JAR Library File

### Annex A JAR `omgdds.jar` with all classes of the PSM

**Spec:** Annex A, p. 41 — "In addition to this document, this specification
includes a Java Archive (JAR) library, omgdds.jar. This library contains
compiled Java *.class files for all of the classes and interfaces specified by
this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Describes the artefact of the OMG. `crates/java-omgdds/java/pom.xml` builds the JAR `org.zerodds:omgdds` with the classes under `org/omg/dds`, whose sources equal the OMG sources except for 11 of 184 files (see the Annex A and B items).
### Annex A Compile against `omgdds.jar`, run against any implementation

**Spec:** Annex A, p. 41 — "This library comprises the compile-time portion of
this specification: users shall be able to compile their PSM-compliant code
against this library and then deploy the result against any conformant
implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/pom.xml`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::ddsExceptionIsADdsObjectAsInTheOmgSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::noClassOfTheApiPackageRefersToTheImplementation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::theExceptionsAreAbstractAndHaveTheSerialVersionUidOfTheOmgSource`

**Status:** `done` — `org/omg/dds/core/DDSException.java` implements `DDSObject` like the OMG source; the seven exception classes in `org.zerodds.core` answer `getEnvironment()` (the default environment of the implementation). A comparison of the files under `org/omg/dds` with `.agent/omg-java/srcJava` gives files equal byte by byte except `EntityQos` (additional `@SuppressWarnings` and a comment) and three files with other line endings (`DomainParticipantFactory`, `DataWriter`, `DataReader`). `TypesPsmApiTest` checks `DDSObject` on `DDSException`, the `serialVersionUID` values and abstract classes of the ten exceptions and that no class in `org.omg.dds` (without `rpc`) refers to `org/zerodds`, so it can run against another implementation. The tests of the module are themselves clients of the classes of `org.omg.dds`. (commit `10e722e2838`)
### Annex A `omgdds` next to the implementation or in one library

**Spec:** Annex A, p. 41 — "Distributors of binary implementations of this PSM
may elect to distribute the omgdds library alongside their implementation
libraries or to package both the contents of omgdds.jar and their implementation
into a single library."

**Repo:**
`crates/java-omgdds/java/pom.xml`

**Tests:** —

**Status:** `n/a (informative)` — A permission for the distributor. ZeroDDS packs the classes of `org/omg/dds` and the implementation in one JAR (`org.zerodds:omgdds`, `crates/java-omgdds/java/pom.xml`). (commit `57f4d99f545`)

---

## Annex B Java Source Code

### Annex B Sources in `omgdds_src.zip`, normative for interfaces and comments

**Spec:** Annex B, p. 43 — "In addition to this document, this specification
includes the Java source code to all of the classes and interfaces specified by
this PSM in the zip archive omgdds_src.zip. This source code, in the directory
srcJava within the archive, corresponds to the binary distribution found in the
library omgdds.jar and is also normative with respect to both its programming
interfaces and its embedded documentation comments."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/pom.xml`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::theExceptionsAreAbstractAndHaveTheSerialVersionUidOfTheOmgSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::ddsExceptionIsADdsObjectAsInTheOmgSource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource`

**Status:** `done` — The ten exception classes under `org/omg/dds/core` are the files of the OMG sources (comments, `serialVersionUID`, `DDSException implements DDSObject`). All 184 files under `org/omg/dds` equal the source character by character (line ends read as LF), comments included: `type/TypeSupport.java` and `core/EntityQos.java` are the files of the source (the mapping of the name `java.lang.Class` to the name of the class is in `ServiceEnvironmentImpl.newTypeSupport`; the warning "possible heap pollution" on `EntityQos.withPolicies(P...)` is switched off by the compile options of the module: `-Xlint:all,-unchecked,-options -Werror` for the main sources in `pom.xml`). `CoreApiShapeTest::theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource` compares the files on every run with the SHA-256 of the source (`src/test/resources/omgdds-srcjava.sha256`, from `omgdds_src.zip`) and demands that none differs. `TypesPsmApiTest` checks the `serialVersionUID` values and that `ServiceConfigurationException` and `ServiceInitializationException` are plain `RuntimeException`s in the source. (commit `418458fe062`)
### Annex B Examples, `build.xml` and project files non-normative

**Spec:** Annex B, p. 43 — "For the convenience of both implementers and
application developers, the archive contains additional files that are neither
API source code nor documentation. These file are non-normative and include:
[...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative.

---

## Audit-Status

447 done / 0 partial / 0 open / 79 n/a (informative) / 0 n/a (rejected).

Test run: the tests of the second pass were only compiled on the Mac (`mvn test-compile` with and without `-Pjava8`, `javac -Xlint:all -Werror`); the run on the Linux test host is pending.

