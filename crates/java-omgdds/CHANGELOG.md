# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Notes
- `DomainParticipantImpl.lookupTopicDescription(name, index)` (DDS-XTypes 1.3 §7.6.4.2). A topic of an existing
  name with other policies than the topics of that name is refused with `InconsistentPolicyException`
  (§7.6.4.1), and the topic that was there counts an inconsistent topic.
- This binding has no dependency on `crates/zerodds-c-api`: there is no
  `System.loadLibrary`, no JNI and no call into a `zerodds_c_*` function in
  `crates/java-omgdds/java/src`, `runtime/java/src` or
  `crates/idl-java/runtime`. Over RTPS the Java stack (`org.zerodds.rtps`)
  talks to the Rust stack on the wire; the C API is not in between, so a
  change there cannot reach this API.

### Changed

- A reliable reader takes a user DATA or GAP only from a writer it has matched (DDSI-RTPS 2.5
  §8.4.12). One addressed to it before the match went into its receive state and was dropped, and
  the next ACKNACK acknowledged it: a request a Rust client pushed to a service still fetching the
  client's types was never answered (`rpc-rust-e2e` `java_peer`, the Rust client of the base
  interface). It is dropped before the receive state now, and the writer's HEARTBEAT after the
  match brings it.
- The TypeLookup codecs are regenerated with the current `zerodds-idlc`: octets are Java `byte`s,
  enumerations classes (`RemoteExceptionCode`), and the XCDR1 codecs are `ZdCodec.Xcdr1`; the
  service writes and reads each FINAL envelope as its header and its call or return. The
  generator gives an `octet` constant above 127 the byte of its bits, `(byte) 0xF1` for `0xF1`
  (CORBA 3.3 part 3 §1.5.2), where it refused it.

- The TypeLookup service writes CDR2_LE with the encoding version 2 by default
  (`TypeLookupService.Encoding.XCDR2`, DDS-XTypes 1.4 Beta 1 §7.6.3.3.3) and CDR_LE with version 1
  (codec `ZdCodec.Xcdr1` generated from the same IDL) under the property
  `zerodds.wire.type_lookup_encoding=xcdr1` or the variable `ZERODDS_WIRE_TYPE_LOOKUP_ENCODING`. A received sample is read in the encoding its identifier
  names and without the padding its options count (XTypes 1.3 §7.6.3.1.2); a sent one is padded to
  four and says so.
- `zerodds-idlc generate --lang java` defaults to `--java-mapping psm` (DDS-Java-PSM 1.0
  Clause 9); `--java-mapping omg`, the IDL-to-Java mapping, is the named option. The
  scripts, gates and samples that read the public fields pass `--java-mapping omg`.
- The RTPS timings default to DDSI-RTPS 2.5: SPDP period 30 s (9.6.2.4), reader
  heartbeatResponseDelay 500 ms, writer nackResponseDelay 200 ms; a new participant is
  answered by unicast SPDP at once. System properties `zerodds.rtps.spdp-period-millis`,
  `zerodds.rtps.heartbeat-response-delay-millis`, `zerodds.rtps.nack-response-delay-millis` and
  `zerodds.rtps.heartbeat-period-millis` (no standard default; 1000) set other values.
- The send history of a reliable writer is unlimited, as RESOURCE_LIMITS is; the former
  bound of 256 samples and 16 MiB is the option `zerodds.rtps.send-history-max-samples` and
  `zerodds.rtps.send-history-max-bytes`.
- `RtpsParticipant`'s `Reliability.maxBlockingTime` default is 100 ms (DDS 1.4 2.2.3.14).

### Added
- `DataWriterImpl.writeSerializedIdentified` writes octets that are already the sample,
  `DataReaderImpl.takeSerialized` takes samples with the octets they arrived as
  (`SampleImpl.serialized()`) for a type support that retains them
  (`TopicTypeSupport.retainsSerialized()`), and `TypeSupportResolver` makes the type support
  of a DDS-RPC topic type from its class (`org.zerodds.ddsrpc.RpcTopicTypes`): the DDS-RPC
  binding hands the application its entities typed by the topic type (DDS-RPC 1.0 §7.11.2).
- `RtpsParticipant.closeDomain(int)`: closes the participant `get(int)` made for a domain. The
  wire tests lease their domain from the band of `crates/test-domains`
  (`org.zerodds.testing.TestDomains`, held to the Rust constants by
  `crates/test-domains/tests/java_lease_agrees.rs`) instead of the fixed domains 227 to 232 and 7,
  which lay in that band, and close the domain's participant before the lease goes.
- Pairing by TypeInformation (DDS-XTypes 1.3 §7.6.3.4.2) for the endpoints whose type support
  answers `TopicTypeSupport.matchesByTypeInformation()`, which the topic types of the DDS-RPC
  Enhanced mapping do (`RawTypeSupport` with a `TypeDescription`, DDS-RPC 1.0 §7.5.1.2.6): equal
  COMPLETE or MINIMAL identifiers match; otherwise the reader's type has to be assignable from the
  writer's, judged on the COMPLETE objects of the own registry and those the peer serves on a
  `getTypes` request (`org.zerodds.typelookup.CompleteTypes`, `org.zerodds.rtps.TypeConsistency`).
  The endpoints of the peer are matched again when the reply came; an object is asked of a peer
  three times at most, and an object whose serialization does not hash to its identifier is not
  kept. Without TypeInformation on one side the type names decide. The other topics of the PSM
  keep the type-name match and the coercion of `TypeConsistencyEnforcement`.
- `TypeAssignability`: two structures without members are assignable (§7.2.4.1, as
  `crates/types`); `int8` and `uint8` are not an octet, a bitmask is not a bit set.
- `access_scope` GROUP: `get_datareaders` of a subscriber with `ordered_access` is a
  list in the order of writing and a read returns one sample; over RTPS the group
  sequence number (`PID_GROUP_SEQ_NUM`) orders the samples of the writers of a
  publisher and a group coherent set (`PID_GROUP_COHERENT_SET`, End Coherent Set DATA)
  arrives as a whole. The writers send HEARTBEATs with the GroupInfoFlag
  (`currentGSN`, `firstGSN`, `lastGSN`, `writerSet`; option
  `zerodds.rtps.omit-heartbeat-group-digests`), and the reader gives up a missing
  number from them and its discovered writers (DDSI-RTPS 2.5 8.7.5); a timeout is
  only the option `zerodds.rtps.group-timeout-millis` (off by default).
- `Serializable` classes of the JDK without an entry in Table 8.1 are published:
  `java.util.Date`, `UUID`, `URI`, `Locale`, `Currency` and the `java.time` values in
  forms of Table 8.1 types, any other concrete one as its Java serialization (a
  `sequence<octet>`, read with an allowlist of classes).
- A `null` argument of the operations of participant, publisher, subscriber, topic,
  writer and reader is `IllegalArgumentException` (`org.zerodds.core.Args`).

- The TypeObject of DDS-XTypes 1.1 Annex B in `org.omg.dds.type.typeobject`:
  the IDL of the annex (`idl/typeobject`), a codec `idlc` generates from it
  (`scripts/generate-typeobject.sh`), implementations of the 26 interfaces
  (`TypeObjectModel`), the TypeObject of a `DynamicType` with the TypeIds of
  §7.3.4.1.2 (`TypeObjectBuilder`) and back (`TypeObjectToDynamic`). `getType()`
  of the built-in topic data returns it; PID 0x0072 carries it on SEDP.
- Type coercion: a reader with DECLARED or ASSIGNABLE type consistency takes
  writers of another type name when the type is assignable (XTypes 1.3
  §7.2.4.4, `TypeAssignability`), in the process and over RTPS.
- Coherent sets: the samples of a set reach readers of other threads as a
  whole, carry `PID_COHERENT_SET` over RTPS, and a coherent-access reader holds
  a remote set until it is complete (DDSI-RTPS 2.5 §8.7.6).
- Liveliness of writers in other processes: DATA, a HEARTBEAT with the L flag
  and, for AUTOMATIC, the participant keep a remote writer alive within the
  lease it announced; `LIVELINESS_CHANGED` counts it.
- A changed QoS of a writer or reader is announced again over SEDP; the SEDP
  writers keep only the last announcement of each endpoint and peer.
- A write beyond `max_samples` waits for the acknowledgments of readers in
  other processes and ends with `TimeoutException` after `max_blocking_time`.
- `Time`, `Duration`, `InstanceHandle`, the QoS policies, the statuses and the
  QoS are `Cloneable` and copy by value.

### Changed
- Narrow strings are truncated to their low octet and wide strings are one 32-bit
  character per code point by default (DDS-Java-PSM 1.0 §8.5.1); UTF-8
  (`-Dzerodds.java.narrow-string=utf8`) and the UTF-16 wstring
  (`-Dzerodds.java.wide-string=utf16`) are named settings for peers that read
  XTypes 1.3 strings.
- The DDS-RPC implementation classes moved from `org.omg.dds.rpc` to
  `org.zerodds.ddsrpc`; `EntityQos.java` is the file of the OMG source, the lint
  category `unchecked` is off for the main sources in `pom.xml`.
- An optional member of a FINAL or APPENDABLE structure is preceded by its
  `is_present` boolean (XTypes 1.3 §7.4.3.5.2).
- `set_qos` refuses an inconsistent set as such before it looks at immutable
  policies.
- BY_SOURCE_TIMESTAMP orders a late sample of an instance before the newer ones
  instead of dropping it.
- The built-in writers offer TRANSIENT_LOCAL again (the built-in readers
  request it); a TypeSupport keeps its registered name; narrow strings are read
  strictly (NUL terminator, UTF-8).
- `type/TypeSupport.java` is the file of the OMG source again; the files of
  `org.omg.dds` are compared with the SHA-256 of the source on every test run.
- The classes `QosChanges`, `internal.QosRules` and `BuiltinKeyedTypeSupport`
  are gone: one set of QoS rules (`core.qos.QosRules`) and one set of built-in
  type supports (`BuiltinTypeSupports`).
- The Serializable restriction of §8.6.1.1: the fields of a base class that is
  not Serializable are not part of the type of a Serializable class.

### Fixed
- A reliable writer repairs at the locator of the reader proxy, the one its samples went to, and
  not at the UDP source of the ACKNACK or NACK_FRAG (DDSI-RTPS 2.5 §8.4.9.2.8; §8.3.4.1 gives the
  reply locator of a received message no port). A reader that sends from another socket than it
  announced, as the Rust runtime does, never got the repair: a request a Java client wrote before
  the Rust service had matched its writer was lost (capture: DATA SN 1 to the ACKNACK's source port
  after each ACKNACK). Test `WriterRepairLocatorTest`.
- The shipped main sources compile with `-source 8` / `--release 8` again
  (the `java8` profile): `instanceof` patterns, a `record` and the Java 9+
  library calls (`List.of`, `List.copyOf`, `Set.of`, `String.isBlank`,
  `Path.of`, `Arrays.compareUnsigned`, `AccessibleObject.canAccess`) were
  used in about forty files.
- `javac -Xlint:all -Werror` over the Java binding is warning-free. The
  value types of §7.3.3 are `Serializable`; their `ServiceEnvironment` is
  `transient` and a deserialized object belongs to the default environment
  of the implementation, collection fields have a serializable declared
  type, `DynamicTypeImpl` and `DynamicTypeMemberImpl` are `Serializable`
  because the descriptors name them, and the declaration of
  `EntityQos.withPolicies(P...)` (normative signature) suppresses the
  heap-pollution warning an interface method cannot annotate away.
- The Java Type Representation of DDS-Java-PSM 1.0 §8 in the reflective
  type support (`ReflectionTypeSupport`, the path a plain Java bean takes
  without generated code):
  - §8.6.1.2: the extensibility was FINAL for every class. It is now FINAL
    for a final class that extends `java.lang.Object` directly and
    EXTENSIBLE in every other case; MUTABLE still only on an explicit
    annotation. `ExtensibilityKind` gained the `EXTENSIBLE` value, and
    EXTENSIBLE is PLAIN_CDR2 like FINAL — the two differ in what a receiver
    does with a member it does not know, not in the bytes.
  - §8.6.1.1: only the fields a class declares itself were mapped, so the
    fields of a base class were lost without a word. Java class extension
    maps to structure inheritance, and the members of the base class are now
    written before the ones of the class itself.
  - Table 8.1, §8.4, §8.5.3: `java.util.Collection` (not just `List`) is a
    sequence, `java.math.BigInteger` is a UInt64, `java.math.BigDecimal` a
    Float128 and a Java enum an enumeration. Before, all four fell into the
    "nested aggregate" branch, and reading them failed — `setAccessible` on
    a field of `java.math` is refused under Java 17, and an enum has no
    no-argument constructor.
  - §8.3/§8.9: an annotation was matched by its simple name, so a `@Key`
    or an `@Id` of any other package became metadata of the type system — an
    `@Id` without a `value()` ended in a `NullPointerException`. Only the
    annotations of `org.omg.dds.type` and of the ZeroDDS IDL-Java runtime
    (`org.zerodds.types`) are read now; everything else is ignored, as §8.9
    says. An `@Id` whose value is not a number is refused with a message
    naming the field.

### Removed
- `org.omg.dds.topic.TopicType` (`crates/idl-java/runtime/TopicType.java`).
  DDS-Java-PSM 1.0 §7.2.6 ("Implementations shall not place their extensions
  […] in any interface or class in the package org.omg.dds or in any other
  package whose name begins with that prefix") forbids it, nothing
  referenced it, and the annotation bridge of the IDL-Java runtime lives in
  `org.zerodds.types` / `org.zerodds.rpc` anyway. With it, every type of the
  published `org.omg.dds` API is a type of the specification.

### Added
- The generator selects the Java mapping (`zerodds-idlc generate --lang java
  --java-mapping omg|psm`, `zerodds_lang_rules::adapter::java::JavaMapping`).
  `psm` is Clause 9 of DDS-Java-PSM 1.0: an aggregation type (structure,
  union, bit set, exception) is a final class with private state, a
  no-argument constructor and Bean accessors, `isX()` too for a `boolean`
  (§9.1.1); an unbounded sequence is `java.util.Collection<E>`, a bounded one
  and an array a Java array (§9.1.2). The codec reads and writes through the
  accessors and calls the new `org.zerodds.wire.Sequences` for the one thing
  it cannot decide by type (count, index, allocate, fill and trim an array or a
  `Collection`; overload resolution chooses). `omg`, the IDL-to-Java mapping,
  stays the default and its output is unchanged; the endpoint SDK and the
  `java-typelookup` gate use it. The shipped TypeLookup types keep it for the
  gate. An `interface` is refused under `psm` by name. Generated output of
  the corpus compiles under both mappings with `javac -Xlint:all` without a
  warning.
- `@SerializeAs` (DDS-Java-PSM 1.0 §8.2) in the reflective mapping: the DDS
  kind of a field in place of the default of Table 8.1 — `UINT_16`,
  `UINT_32`, `UINT_64`, `FLOAT_128`, `CHAR_32` and the other primitive kinds
  on the Java types Table 8.2 lists (an unsigned kind in the signed type of
  the same size keeps the bit pattern, in the next larger one the logical
  value and refuses a value outside the range), a wide string
  (`STRING_TYPE` with `collectionElementKind = CHAR_32_TYPE`), bounded
  strings, sequences and maps (`bound`), arrays with their dimensions
  (`ARRAY_TYPE`, also for a nested Java array), the kind of the elements and
  keys of a collection or map, and bit sets. A combination Table 8.2 does not
  list is an `XcdrException` when the type is first introspected.
- Unions (§8.6.2): a class annotated `@SerializeAs(UNION_TYPE)` with one
  `@UnionDiscriminator` field and `@UnionMember(discriminatorValue = ...)` on
  every other field is written as the discriminator and the member it
  selects (FINAL, EXTENSIBLE, MUTABLE; a discriminator that selects no member
  is written alone). A misplaced annotation, a missing or second
  discriminator, an unannotated field or two members with one value are
  refused.
- Built-in topic data, type members (XTypes 1.3 §7.6.3.2.2): the
  TypeInformation of the entity's type is on the wire as `PID_TYPE_INFORMATION`
  (0x0075) in the built-in topic samples and, as before, in SEDP; the
  XTypes 1.1 `type` is kept as the octets announced and forwarded on 0x0072
  (no Java type support produces one; `getType()` stays `null`).
  `equivalentTypeName` and `baseTypeName` are sequences of strings on the
  vendor-specific parameters 0x8003 and 0x8004: the Java PSM gives them
  `@ID(0x0075)` and `@ID(0x0076)`, and 0x0075 is `type_information` in XTypes
  1.3. `baseTypeName` is filled from the superclasses of a `Class` topic type
  (and the base types of a `DynamicType`); no Java type states equivalent
  type names. The parameter ids of the QoS policies were compared with the
  table of parameter ids in DDSI-RTPS 2.5 §9.6.3 (numbered 9.18 and 9.19 in
  the 2.5 text): all 23 that the Java code uses agree (checked by a script over
  the table text); 0x0072 to 0x0075 are those of XTypes 1.3 §7.6.3.1,
  §7.6.3.2.2 and §7.6.3.4.3.
- DDS-RPC 1.0 §7.8.2 on the DDS Java PSM binding: `DataWriterImpl.writeIdentified(data,
  related)` returns the identity the sample was sent under (the GUID of the writer and
  its sequence number) and sends the identity it relates to as
  `PID_RELATED_SAMPLE_IDENTITY` (0x0083) in the inline QoS of the DATA, in a reliable
  writer's history and in the first fragment of a fragmented sample; inside the process
  it travels on the message. `SerializedSample.identity()` and `relatedIdentity()` and
  `SampleImpl.identity()` and `relatedIdentity()` give both to the reader.
  `runtime/java/.../rpc/zerodds/ZeroDdsParticipant` is written over this API (`RawTypeSupport`
  is a `TopicTypeSupport<byte[]>` that adds no framing), so the Enhanced mapping runs on
  it and a replier with `RelatedIdentity.REQUIRED` no longer fails.
- `DynamicTypeFactoryImpl.createType(Class)` maps a member by Table 8.1
  (`int` INT_32, `short` INT_16, `long` INT_64, `float` FLOAT_32, `double`
  FLOAT_64, `char` CHAR_8, `byte` BYTE, `boolean` BOOLEAN, boxed types alike).
  Before, every member was built by a recursive `createType(member.type)` and a
  primitive became a structure without members. Strings, enumerations,
  `Map`, `Collection` and arrays (element types from the type arguments),
  nested structures, unions (§8.6.2), `@SerializeAs` overrides and the
  annotations `Extensibility`, `Key`, `Optional` and `MustUnderstand` are
  reflected; an annotation type marked `@SerializeAs(ANNOTATION_TYPE)` is an
  annotation type (§8.9). A member Java type with no kind is an
  `IllegalArgumentException`.
- `@BitSet` (§8.7): an enumeration annotated `@BitSet` is a bit set type
  (constant n is bit n, `@BitBound`, default 32, gives the width); a member of
  `java.util.EnumSet` or `java.util.BitSet` marked `@BitSet` is a bit mask of
  its constants, a member of the bit set enumeration marked `@BitSet` is one
  constant. An unmarked member of a bit set type, and bits on the wire without
  a constant, are an `XcdrException`.
- `@SerializeAs(ALIAS_TYPE)` and `@SerializeAs(ANNOTATION_TYPE)` on a member
  stay refused, with the reason in the message: Clause 8 gives no Java type
  for an alias (Table 8.2, §8.5 to §8.7 list the kinds that have one), and §8.9
  applies `ANNOTATION_TYPE` to a Java annotation type, which has no data.
- Narrow strings: a `String<Char8>` is written as UTF-8 (DDS-XTypes 1.3
  §7.2.2.2.1.2.4 and §7.4.1.1.2; the binding transforms the Java `String` to and
  from UTF-8), the form of the interoperability corpus
  (`tests/interop-corpus`: `été` is `c3 a9 74 c3 a9`) and of the generated
  code (`ReflectionTypeSupport`, `DynamicDataTypeSupport`). DDS-Java-PSM 1.0
  §8.5.1 ("each Java character shall be truncated to its least-significant
  byte", `e9 74 e9`) predates XTypes 1.3 and is the named setting
  `-Dzerodds.java.narrow-string=truncate`. For characters up to U+007F the two
  forms are the same. IDL `char` is one octet and refuses a character above
  U+00FF in both.
- `createTopic(String, DynamicType)` and the form with QoS and listener work.
  PSM 1.0 types the result `Topic<DynamicType>`, its samples are
  `DynamicData`; the signatures stay, and `DynamicTopics.dataTopic(topic)`
  (the one place that converts the parameterization, the type parameter is
  erased) gives the `Topic<DynamicData>` that `createDataWriter` and
  `createDataReader` take; `DomainParticipantImpl.createDynamicTopic` does
  both steps. The new `DynamicDataTypeSupport` is the XCDR2 codec of a
  `DynamicType`: structures (with base types; FINAL, EXTENSIBLE, APPENDABLE,
  MUTABLE through the `Extensibility` annotation), unions, sequences, arrays,
  maps, strings, enumerations, bit sets and every primitive kind, with the
  same bytes as `ReflectionTypeSupport` for the corresponding Java class,
  keys by the `Key` annotation of a member. Before, both forms threw
  `UnsupportedOperationException`.
- The built-in topic data carries the QoS of DDS 1.4 §2.2.5 and is read from
  and written to discovery as DDSI-RTPS 2.5 §9.6.2 (Table 9.13) defines.
  The wire form of the four built-in topics was the identifying members
  only (key, participant key, names, USER_DATA); it is now the `PL_CDR_LE`
  ParameterList with every member of the structure on its ParameterId
  (`org.zerodds.topic.BuiltinTopicWire`, `BuiltinQos`, `BuiltinData`). SEDP
  announces the QoS of each DataWriter and DataReader (reliability and
  durability kinds were all it said), SPDP the USER_DATA of the participant,
  and a policy absent from an announcement is the default of the entity kind
  (`org.zerodds.rtps.QosParameters`). `RtpsParticipant` matches a local
  endpoint with a remote one only if the request-offered rules of §2.2.3
  hold, reports what it discovers to `DiscoveryListener`s, and the built-in
  topics of a participant show the remote participants and endpoints with
  their QoS. `getMatchedSubscriptionData` and `getMatchedPublicationData`
  return the data of the matched entity (BAD_PARAMETER for a handle that is
  not matched) where they threw `PreconditionNotMetException`.
  XTypes 1.3 has two TypeConsistencyKind values, PSM 1.0 four: the exact
  and declared kinds are sent as DISALLOW_TYPE_COERCION, the assignable kind
  as ALLOW_TYPE_COERCION.
- `MultiTopic` with the subscription expression of DDS-DCPS 1.4 §2.2.2.3.4
  and Annex B.2 (`TopicExpression`): `SELECT <fields | *> FROM <topic>
  [ NATURAL JOIN <topic> ... ] [ WHERE <condition> ] [;]`, all three spellings
  of the join keyword, parenthesized join items, quoted topic names, `AS`
  aliases and the juxtaposed alias. A DataReader on a MultiTopic reads the
  topics of the FROM clause through readers of its own (on its Subscriber,
  with its QoS), joins the current samples of all their instances on the
  members the topics share by name (the SQL natural join; with no shared
  member a cross product), projects and renames the fields, applies the
  WHERE clause to the joined row (member names or aliases) and delivers the
  result through an ordinary reader. The result instance is disposed when a
  constituting instance is disposed, and has no writers when one has none
  and none is disposed (§2.2.2.3.4). The system property
  `zerodds.multitopic.join=key` restricts the join to the members that are
  keys in every topic that has them. The expression is checked when the
  MultiTopic is created (syntax, the topics exist, the join members have one
  type, every field exists and fits the member of the result type that takes
  it); a fault is an `IllegalArgumentException`. `ORDER BY` is not part of a
  topic expression in Annex B and is a syntax error.
  `getTopicDescription()` of a reader of a MultiTopic or a
  ContentFilteredTopic now returns that description, not the topic under it.
- Content filter expressions: the comparison operands of Annex B,
  `FIELDNAME RelOp FIELDNAME` and `Parameter RelOp FIELDNAME`.
- Content filter expressions (DDS 1.2 §2.2.2.5.3.8, DDSI-RTPS 2.5
  §9.4.1.1). `org.zerodds.sub.ContentFilter` compiles an expression into a
  predicate over a deserialized sample: the comparison operators, `AND`,
  `OR`, `NOT`, parentheses, `LIKE` (with `ESCAPE`), `BETWEEN`, `IN`,
  `IS [NOT] NULL` and the `%n` expression parameters, with a dotted member
  path resolved through `getX()`, `isX()`, a no-argument method named like
  the member or a field, so beans and records both work. The predicate
  drives `DomainParticipant.createContentFilteredTopic` (which raised
  `UnsupportedOperationException` before), the trigger value of
  `DataReader.createQueryCondition` and the query expression of a read/take
  `Selector` (which did the same).
- The built-in topics of DDS 1.2 §2.2.1.3.1 / DDSI-RTPS 2.5 §9.3.1.2. A
  `DomainParticipant` now creates `DCPSParticipant`, `DCPSPublication`,
  `DCPSSubscription` and `DCPSTopic` for itself, each with its
  `DDS::DCPS…` type, and announces itself, its topics, its DataWriters and
  its DataReaders on them through built-in writers, so that an application
  reading them through `getBuiltinSubscriber()` sees what the domain
  contains. `createTopic` with a built-in name returns the built-in topic of
  that type (DDS 1.2 §2.2.1.3.1) and refuses another type. The four
  built-in topic structures are implemented in `org.zerodds.topic`
  (`BuiltinTopicKeyImpl`, `ParticipantBuiltinTopicDataImpl`,
  `TopicBuiltinTopicDataImpl`, `PublicationBuiltinTopicDataImpl`,
  `SubscriptionBuiltinTopicDataImpl`); their wire form carries the
  identifying members — key, participant key, topic name, type name and
  USER_DATA octets — because the QoS of a discovered entity is read from the
  live entity through `getDiscoveredParticipantData`,
  `getDiscoveredTopicData`, `getMatchedPublicationData` and
  `getMatchedSubscriptionData`, which build the full structure. Closing a
  participant withdraws everything it announced before its own entities go
  away.

### Changed
- `org.omg.dds.domain.DomainParticipant` and
  `org.omg.dds.domain.DomainParticipantFactory` are the types of the
  specification — the interface and the abstract class of OMG DDS Java-PSM
  1.0 §7.4 — and ZeroDDS implements them in
  `org.zerodds.domain.DomainParticipantImpl` and
  `org.zerodds.domain.DomainParticipantFactoryImpl`. The DCPS operations of
  DDS 1.2 §2.2.1.2 that the binding lacked are there: `createPublisher`/
  `createSubscriber`/`createTopic` with a listener and a status mask,
  `findTopic` (with a timeout), `lookupTopicDescription`,
  `closeContainedEntities`, `containsEntity`, `getCurrentTime` (with and
  without a container), `assertLiveliness`, the default Publisher,
  Subscriber and Topic QoS, `getBuiltinSubscriber`, `ignoreParticipant`/
  `ignoreTopic`/`ignorePublication`/`ignoreSubscription`,
  `getDiscoveredParticipants`, the factory's own QoS and its default
  participant QoS. `createTopic` now also takes an explicit
  `TypeSupport`; `createTopic(String, DynamicType, TypeSupport)` works
  through it. ENTITY_FACTORY decides whether `create_*` enables what it
  creates (DDS 1.2 §7.1.3.7). **Breaking:** there is no longer a
  zero-argument `DomainParticipantFactory.getInstance()` — §7.4.1 names the
  environment, so a call becomes
  `DomainParticipantFactory.getInstance(env)` with
  `env = ServiceEnvironment.createInstance(classLoader)` after
  `System.setProperty(ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY,
  "org.zerodds.ServiceEnvironmentImpl")` (§7.3.1); the factory's
  `deleteParticipant` is gone as well, `close()` is the operation that
  deletes (DDS-Java-PSM 1.0 §7.2.3). `getInstance(null)` now fails with a
  `NullPointerException`, as the normative `getInstance(env)` does.
  `createTopic(String, DynamicType)` and `createTopic(String, DynamicType,
  QoS, TopicListener, Statuses)` raise `UnsupportedOperationException`: the
  PSM 1.0 signature types the topic of a `DynamicType` as
  `Topic<DynamicType>`, while the samples of such a topic are `DynamicData`
  and a reader or writer needs a `TypeSupport<DynamicData>` over a
  `Topic<DynamicData>`, which no `createTopic` of this version produces; the
  exception names the form to use instead. The built-in topic data, the
  content filter and `MultiTopic` came with the following commits.
- `org.omg.dds.pub.DataWriter` and `org.omg.dds.sub.DataReader` are the
  interfaces of the specification; ZeroDDS implements them in
  `org.zerodds.pub.DataWriterImpl` and `org.zerodds.sub.DataReaderImpl`.
  The DCPS operations of DDS 1.2 §2.2.2.4 and §2.2.2.5 that were missing
  are there: `write`/`dispose`/`unregisterInstance`/`registerInstance` with
  an instance handle and a source timestamp (in `Time` and in
  milliseconds/`TimeUnit`), `getMatchedSubscriptions`/`getMatchedPublications`,
  `waitForAcknowledgments`, `waitForHistoricalData`,
  `getSampleLostStatus`/`getSampleRejectedStatus`, `getKeyValue`,
  `closeContainedEntities`, `createReadCondition`/`createQueryCondition`,
  `getTopicDescription` and `cast`.
  `read`/`take` now have both forms of DDS-Java-PSM 1.0 §7.7.3: a
  `Sample.Iterator` and a copy into the application's list, each with the
  selector and `maxSamples` variants, plus `readNextSample`/`takeNextSample`.
  `DdsAsync` waits on a `ReadCondition` in a `WaitSet` instead of polling
  `read()` every millisecond. Code that used `read()`/`take()` returning a
  `List` now walks a `Sample.Iterator` or calls the list flavour.
- `org.omg.dds.pub.Publisher` and `org.omg.dds.sub.Subscriber` are the
  interfaces of the specification; ZeroDDS implements them in
  `org.zerodds.pub.PublisherImpl` and `org.zerodds.sub.SubscriberImpl`. The
  operations of DDS 1.2 §7.6 and §7.7 that were missing are there:
  `createDataWriter`/`createDataReader` with a listener and a status mask,
  `lookupDataWriter`/`lookupDataReader`, `closeContainedEntities`,
  `suspendPublications`/`resumePublications`,
  `beginCoherentChanges`/`endCoherentChanges`, `waitForAcknowledgments`,
  `getDataReaders` (with and without a `DataState`), `notifyDataReaders`,
  `beginAccess`/`endAccess`, `getDefault…Qos`/`setDefault…Qos` and
  `copyFromTopicQos`. `beginAccess` holds the subscriber's readers still
  until `endAccess` (§2.2.2.2.3). The ZeroDDS form
  `createDataWriter(Topic, TopicTypeSupport, DataWriterQos)` — a writer whose
  codec the application supplies — moved to `PublisherImpl` /
  `SubscriberImpl`.
- `org.omg.dds.sub.Sample`, `org.omg.dds.topic.Topic` and
  `org.omg.dds.topic.ContentFilteredTopic` are the interfaces of the
  specification; ZeroDDS implements them in `org.zerodds.sub.SampleImpl`,
  `org.zerodds.topic.TopicImpl` and
  `org.zerodds.topic.ContentFilteredTopicImpl`. A `Sample` carries the whole
  `SampleInfo` of DDS 1.2 §2.2.2.5.1 as Java-Bean properties
  (`getData`, `getSampleState`, `getViewState`, `getInstanceState`,
  `getSourceTimestamp`, `getInstanceHandle`, `getPublicationHandle`, the
  generation counts and the three ranks) instead of the short forms
  `data()`, `sampleState()`, …; a `Topic` carries the `TypeSupport` it was
  created from, so its `getTypeName()` is the registered type name.
- The entity API is the OMG API. `org.omg.dds.core.Entity` is now the
  generic, `java.io.Closeable` interface of DDS-Java-PSM 1.0 §7.3.6 with
  `getListener`/`setListener` (with the status mask of §7.3.7.2), `getQos`/
  `setQos`, `setQos(library, profile)`, `enable`, `getStatusCondition`,
  `getStatusChanges`, `getInstanceHandle`, `close` and `retain`;
  `org.omg.dds.core.DomainEntity`, `org.omg.dds.core.StatusCondition`, the
  fourteen `org.omg.dds.core.event` types, `ReadCondition`, `QueryCondition`,
  `TopicDescription`, `MultiTopic` and the listener interfaces and adapters
  for participant, publisher, subscriber, topic, writer and reader are the
  types of the specification. Listeners now receive one event object that
  names the entity and carries the status, as §7.3.7.2 requires, and are
  told only about the statuses of their mask. ZeroDDS implements all of it
  in `org.zerodds` (`EntityBase`, `StatusConditionImpl`,
  `ReadConditionImpl`, `QueryConditionImpl`, `org.zerodds.core.event.*`).
  `DataReaderListener`/`DataWriterListener` callbacks that took the entity
  as the first argument now take the corresponding event; use
  `event.getSource()` / `event.getStatus()`.
- Errors are exceptions, not return values. DDS-Java-PSM 1.0 §7.3.2 and
  Table 7.1 map `ReturnCode_t` onto Java exceptions; the binding had its own
  `org.omg.dds.core.ReturnCode` enum (including a `NOT_ALLOWED_BY_SECURITY`
  value DDS 1.2 does not have), so a `write()` on a closed or not yet enabled
  writer returned a code the application had to remember to check — without
  the check the sample was lost silently — and `read()`/`take()` on a closed
  reader returned an empty list that could not be told apart from "no data".
  `org.omg.dds.core` now carries the abstract exception classes of the PSM
  (`DDSException` and the eight PSM subclasses, plus the two concrete
  bootstrap exceptions `ServiceConfigurationException` and
  `ServiceInitializationException`), with ZeroDDS' concrete subclasses in
  `org.zerodds.core`. `write`, `dispose`, `unregisterInstance`,
  `assertLiveliness` and `enable` return `void` and throw
  `AlreadyClosedException` / `NotEnabledException`; `read`/`take` throw
  instead of returning an empty list; a null argument throws
  `IllegalArgumentException` (RETCODE_BAD_PARAMETER) and an operation on a
  closed entity throws `AlreadyClosedException`. The `ReturnCode` enum is
  gone.

### Fixed
- Entity operations were not reentrant (DDS-Java-PSM 1.0 §7.2.4). `write()`
  held the writer's monitor while it delivered to the readers, and delivery
  held the reader's monitor while it ran the application's listener callback,
  so a callback ran with two monitors held and blocked any concurrent call on
  those entities; `enable()` was not synchronized and `closed`/`enabled` were
  read by the bus, by callbacks and by application threads without
  synchronization. Entity state changes now happen under the entity's own
  monitor, while delivery, late-join replay, bus registration and every
  listener callback run without it; `closed`/`enabled` are `volatile`;
  `enable()` is serialized and closes of a `DomainParticipant`, `Publisher`
  or `Subscriber` take effect before the contained entities are closed.
- The built-in `byte[]` topic type announced itself as `DDS::Octets` and put
  the octets on the wire without any framing. DDS-Java-PSM 1.0 §7.8.2 maps
  `byte[]` to the XTypes built-in type `DDS::Bytes`, which XTypes 1.3 Annex E
  defines as `@extensibility(APPENDABLE) struct Bytes { ByteSeq value; }`.
  `ByteArrayTypeSupport` now reports `DDS::Bytes`, reports APPENDABLE, and
  serializes the sample as a DELIMITED_CDR2 struct: a 4-byte DHEADER holding
  the serialized body size, the 4-byte `ByteSeq` length, then the octets.
  Truncated or inconsistent framing raises `XcdrException` instead of
  producing wrong data.
- `DataReader.read()`/`take()` returned the reader's own decoded sample
  objects, so an application mutating a returned sample's `getData()` changed
  the value the next `read()` returned. DDS-Java-PSM 1.0 §7.7.3 ("another
  that deeply copies into an application-provided List"). The reader now
  deep-copies each sample's data (serialize + deserialize through the
  TypeSupport) before handing it back.
- DURABILITY kinds TRANSIENT and PERSISTENT were treated like TRANSIENT_LOCAL:
  the DataWriter replayed its own retained samples and the data died with the
  writer. DDS 1.2 §7.1.3.4 requires TRANSIENT/PERSISTENT data to survive the
  DataWriter via a durability service. A new `org.zerodds.internal.DurabilityService`
  retains TRANSIENT/PERSISTENT samples beyond the writer's lifetime and replays
  them to late-joining readers; TRANSIENT data is dropped when the
  DomainParticipant closes, PERSISTENT data outlives it (in-memory, no disk in
  the pure-Java stack). TRANSIENT_LOCAL remains writer-local.
- `DataReader`/`DataWriter` evaluated `REQUESTED_DEADLINE_MISSED` and
  `LIVELINESS_CHANGED`/`LIVELINESS_LOST` only when the application polled the
  status getter; a listener never fired otherwise, and a writer that stopped
  writing stayed "alive" until close. DDS 1.2 §7.1.4.2 requires the middleware
  to alert the application asynchronously. Entities with a finite DEADLINE
  period or a finite MANUAL-mode LIVELINESS `lease_duration` now run a
  daemon-thread lease (`org.zerodds.internal.LeaseMonitor`) that raises the
  callback without a status poll. `DataWriter.isAlive()` now reflects the
  lease, and `DataWriterListener` gains `onOfferedDeadlineMissed` /
  `onLivelinessLost` (`OfferedDeadlineMissedStatus`, `LivelinessLostStatus`).

### Changed
- `org.zerodds.cdr` no longer carries its own CDR codec. `Xcdr2Writer`/
  `Xcdr2Reader` (never released) are gone; `TopicTypeSupport<T>`,
  `EndianMode`, `ExtensibilityKind`, `Md5` and `XcdrException` stay as
  the thin contract the typed-topic path bridges through.
  `zerodds-idlc --lang java` (`tools/idlc/src/java_support.rs`) emits a
  `ZdTypeSupports.Support_<tag>` companion per IDL `struct`/`union` that
  calls straight into `org.zerodds.wire.Writer`/`Reader`
  (`runtime/java`) — the one Java XCDR2 codec, exercised by five other
  language columns besides this one.
- `ReflectionTypeSupport` (the no-IDL, plain-bean fallback DDS-Java-PSM
  1.0 §8 requires) now drives the same `org.zerodds.wire` runtime
  instead of the removed `Xcdr2Writer`/`Xcdr2Reader`; its public API and
  wire shape are unchanged.
- **Fixed**: `TypeSupportResolver.resolve(Class)` — the machinery behind
  `Publisher.createDataWriter(Topic)` / `Subscriber.createDataReader(Topic)`
  — looked for a generated companion named `<FQN>TypeSupport` next to the
  topic type. `zerodds-idlc --lang java` has never emitted that shape; the
  real companion is `ZdTypeSupports.Support_<tag>`, a nested class inside a
  sibling top-level `ZdTypeSupports`, keyed by the topic type's own nesting
  under the generated `ZdTypes` container. The old convention never
  matched, so every convenience `createDataWriter`/`createDataReader` call
  silently used `ReflectionTypeSupport` even for a codegen'd type.
  `lookupGenerated` now reconstructs `<tag>` from the type's enclosing
  classes and finds the actual companion.

### Removed
- `Xcdr2WireVectorsTest`'s V-10 vector used length code 4 (a plain
  NEXTINT) for the `@mutable` type's `string` member, with the NEXTINT
  written *separately* from the string's own length prefix — DHEADER 23,
  total 27 bytes. XTypes 1.3 §7.4.3.4.2 permits that framing and length
  code 5, where the string's own length word is the NEXTINT;
  `org.zerodds.wire.Writer` (and the codegen companion built on it) use
  the compact framing (`docs/specs/zerodds-ast-1.0.md` §7.1), length code
  5 here, giving DHEADER 19 / total 23 bytes. The vector is byte-identical to
  `zerodds-idlc --lang java`'s own generated output for the equivalent
  IDL type (checked directly, not just by inspection).

### Notes
- The EMHEADER wire layout follows OMG XTypes 1.3 §7.4.3.4.5 (LC bits 30-28,
  member-id bits 27-0, in body endianness LE). The conceptual MSB hex
  notation in the spec (e.g. `20 00 00 01` for {LC=2, id=1}) corresponds to
  the in-memory u32 view; the wire output is `01 00 00 20`.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-java-omgdds`** crate as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
Native Java DDS PSM scaffolding (org.omg.dds.* package)

### Architecture
- Layer: 6 (PSMs / bindings)
- Dependencies (in): `zerodds-c-api` (foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.

