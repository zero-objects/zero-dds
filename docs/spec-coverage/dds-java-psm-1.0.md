# DDS-Java 1.0 (Java 5 Language PSM for DDS) — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-java-psm-1.0.pdf` (44 Seiten, OMG
formal/2013-11-02, November 2013) —
[OMG DDS-Java 1.0](https://www.omg.org/spec/DDS-Java/1.0/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben ohne weiteren Zusatz sind PDF-Seiten dieser PDF (PDF-Seite =
gedruckte Seite + 10; die Titelseite ist PDF-Seite 1, das Vorwort gedruckt iii
und iv auf PDF-Seite 9 und 10). Jeder normative Satz von §1 bis §9 und der
Annexe, jede Zeile von Table 7.1, Table 7.2, Table 8.1 und Table 8.2, jede
Factory-Methode, die §7.3.1 dem `ServiceEnvironment` gibt, und jede Vorgabe des
`DataReader.Selector` ist ein eigenes Item. §2 verlangt alle
Programmierschnittstellen der Konformitätsstufen von [DDS]; [DDS] ist nach §3
die Version 1.2 (`internal/standards/cache/omg/dds-1.2.pdf`, PDF-Seite =
gedruckte Seite + 12). Die Operationen der Klassen von DCPS ([DDS] 1.2 §7.1.2,
S. 25–107) und die eingebauten Topics (§7.1.5, S. 147–150) sind deshalb je ein
Item unter §2; §7.3.7.1 verlangt je Status von [DDS] eine Klasse ([DDS] 1.2
§7.1.4.1, S. 132–133, IDL S. 159–161) und §7.3.5.1 je Policy eine Schnittstelle
([DDS] 1.2 §7.1.3, S. 108–120), je Klasse ein Item; §7.3.5.2 je Entity-QoS eine
Schnittstelle ([DDS] 1.2 IDL, S. 167–169), je QoS ein Item. Operationen, die
sich nur durch `_w_timestamp` unterscheiden, und Paare aus `set_` und `get_`
sind ein Item. `done` steht nur, wo Code und Test die Anforderung abdecken; ein
zitierter Test in Java nennt Datei und Testmethode als `datei::methode`, ein
Test in Rust die Testfunktion als `datei::modul::name`. Eine Fundstelle im Repo
nennt Datei und Symbol, keine Zeilennummer. Eine Anforderung an einen
Namen, ein Paket oder eine Signatur gilt als durch einen Test abgedeckt, wenn
der Test gegen genau diesen Namen übersetzt wird; eine Eigenschaft, die beim
Übersetzen nicht auffällt (`abstract`, `final`, eine Elternklasse), braucht
einen Test, der sie prüft. Jede Aussage über Verhalten ist aus dem Code und den Tests gelesen. Die
Tests, die der zweite Durchgang geschrieben hat, wurden auf dem Mac nur
übersetzt; sie laufen auf dem Linux-Testhost. Wo Code eine Anforderung
abbildet, aber kein Test sie prüft, ist das Item `open` und die Notiz nennt
die Codestelle.

**Kontext:** DDS-Java 1.0 ist in ZeroDDS über diese Teile verteilt:

- `crates/java-omgdds/java/` — das Maven-Modul `org.zerodds:omgdds`
  (`crates/java-omgdds/java/pom.xml`, Java 17, Profil `java8` für Java 8) mit
  den Klassen unter `org.omg.dds.core`, `org.omg.dds.core.policy`,
  `org.omg.dds.core.status`, `org.omg.dds.domain`, `org.omg.dds.topic`,
  `org.omg.dds.pub` und `org.omg.dds.sub`, der Implementierung unter
  `org.zerodds.*` und den JUnit-5-Tests unter `src/test/java`. Die Bindung ist
  ein eigener Stack in Java. Sie ruft das C-API (`crates/zerodds-c-api`) nicht
  auf und lädt keine native Bibliothek (kein JNI, keine FFM;
  `grep -rn "loadLibrary\|java.lang.foreign" crates/java-omgdds/java/src`
  findet nichts). Samples gehen im selben Prozess über
  `org.zerodds.internal.InProcessBus` an die Reader; mit
  `-Dzerodds.rtps.enable=true` oder `ZERODDS_RTPS=1` zusätzlich über den
  RTPS-Stack in Java unter `org.zerodds.rtps` (`DdsWireBridge.enabled`).
- `crates/idl-java/runtime/` — die Annotationen `org.zerodds.types.Key`, `Id`,
  `Optional`, `MustUnderstand`, `External`, `Nested`, `Shared`, `Extensibility`
  und die Schnittstelle `org.omg.dds.topic.TopicType`; das POM übersetzt sie
  über `build-helper-maven-plugin` in dieselbe JAR.
- `runtime/java/` — der Codec `org.zerodds.wire.Writer`/`Reader`, den
  `org.zerodds.cdr.ReflectionTypeSupport` und die erzeugten Typen benutzen; das
  POM übersetzt ihn ebenfalls in die JAR.
- Der Java-Codegen: `zerodds-idlc generate --lang java`
  (`tools/idlc/src/generate.rs`, `tools/idlc/src/java_support.rs`) über
  `crates/lang-rules` (`adapter/java.rs`, `emit/java.rs`). Er erzeugt die
  Topic-Typen in einer Klasse `ZdTypes` und je Typ eine
  `ZdTypeSupports.Support_<tag>`, die `org.zerodds.cdr.TopicTypeSupport<T>`
  implementiert.

Die Abbildung von IDL auf Java nach IDL4 to Java steht in
[`idl4-java-1.0.md`](idl4-java-1.0.md), die Architektur des Moduls in
[`zerodds-java-omgdds-1.0.md`](zerodds-java-omgdds-1.0.md), der XCDR2-Vertrag
`org.zerodds.cdr.TopicTypeSupport` in
[`zerodds-xcdr2-java-1.0.md`](zerodds-xcdr2-java-1.0.md), die Endpoint-API in
[`zerodds-endpoint-java-1.0.md`](zerodds-endpoint-java-1.0.md). Hier stehen die
DDS-API in Java nach DDS-Java 1.0 und die Regeln, die DDS-Java selbst an die
Typen stellt (§8, §9). `org.omg.dds.rpc` unter `runtime/java/` gehört zu DDS-RPC
1.0 und ist nicht Gegenstand dieses Dokuments; ebenso der RTPS-Stack unter
`org.zerodds.rtps`.

Nach §2 sind die PDF, die JAR `omgdds.jar` und die Quelldateien der OMG
normativ, bei einem Widerspruch gelten JAR und Quellen. Das Archiv
(Titelseite, Annex B) liegt im Cache (`internal/standards/cache/omg/dds-java-psm-1.0-omgdds_src.zip`);
alle Dateien unter `org/omg/dds` des Moduls außer `EntityQos.java` (zusätzliches
`@SuppressWarnings`) stimmen mit den Quellen überein; `CoreApiShapeTest` prüft das bei jedem
Lauf gegen die SHA-256 der Quelldateien (`src/test/resources/omgdds-srcjava.sha256`).

**Offene Punkte:** keine.

---

## §1 Scope

### §1 PSM nur für DCPS, ohne DLRL

**Spec:** §1, S. 11 — "This specification defines a platform-specific model
(PSM) for the OMG Data Distribution Service for Real-Time Systems (DDS). It
specifies an API only for the Data-Centric Publish-Subscribe (DCPS) portion of
that specification; it does not address the Data Local Reconstruction Layer
(DLRL)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Umfang der Spec, keine erfüllbare Anforderung. Unter crates/java-omgdds/java/src/main/java/org/omg/dds liegen die Pakete core, domain, pub, sub, topic und type; ein Paket dlrl gibt es nicht. DLRL gehört zu DLRL 1.2 (tools/idlc/src/dlrl.rs).
### §1 Umfang: die APIs von [DDS-XTypes]

**Spec:** §1, S. 11 — "In addition, it encompasses (a) the DDS APIs introduced
by [DDS-XTypes] [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §2 (Stufe Extensible and Dynamic Types) und §7.8. Die Pakete org.omg.dds.type, org.omg.dds.type.dynamic, org.omg.dds.type.typeobject und org.omg.dds.type.builtin sind im Code vorhanden; die Anforderungen stehen im Item „§2 Stufe Extensible and Dynamic Types“.
### §1 Umfang: eine API für QoS-Bibliotheken und -Profile nach [DDS-CCM]

**Spec:** §1, S. 11 — "[...] and (b) an API to specifying QoS libraries and
profiles such as were specified by [DDS-CCM]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §2 und §7.3.5.4. Die API ist org.omg.dds.core.QosProvider, die Implementierung org.zerodds.core.qos.QosProviderImpl; die Anforderungen stehen im Item „§2 XML-QoS-Profile“.
### §1 Java Type Representation ohne Beschreibung in einer anderen Sprache

**Spec:** §1, S. 11 — "This specification also defines a means of publishing and
subscribing Java objects with DDS-the Java Type Representation-without first
describing the types of those objects in another language, such as XML or OMG
IDL."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §8. Umgesetzt durch org.zerodds.cdr.ReflectionTypeSupport und TypeSupportResolver.resolve (Klasse ohne generierten Code wird per Reflexion abgebildet); die Prüfung steht im Item „§2 Mindestens eine Type Representation“.

---

## §2 Conformance

### §2 PDF, JAR und Quelldateien normativ, bei Widerspruch JAR und Quellen

**Spec:** §2, S. 11; Titelseite, S. 1 — "This specification consists of this
document as well as a Java jar library file and the source files that generated
it, identified on the cover page (all are normative). In the event of a conflict
between them, the latter shall prevail."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/IllegalOperationException.java` (`IllegalOperationException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_an_exception_belongs_to_the_environment_it_was_created_in`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_the_exceptions_keep_the_serial_version_uid_of_the_omg_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`

**Status:** `done` — DDSException implementiert DDSObject (org/omg/dds/core/DDSException.java), wie die OMG-Quelle; die sieben Ausnahmeklassen in org.zerodds.core liefern mit getEnvironment die Standardumgebung. Die serialVersionUID von sechs Ausnahmeklassen ist die der OMG-Quelle. ParticipantOmgApiTest prüft DDSObject und die serialVersionUID. CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source vergleicht jede Datei von org/omg/dds mit dem SHA-256 der OMG-Quelle (Zeilenenden als LF), ohne Abweichung; auch EntityQos.java ist die Datei der OMG-Quelle. (Commit `7f102b80f83`)
### §2 Konformität parallel zu den Stufen von [DDS]

**Spec:** §2, S. 11; [DDS] 1.2 §2, S. 14–15 (`dds-1.2.pdf`) — "Conformance to
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_participant_before_the_writer_exists_keeps_its_data_and_its_match_away`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_refuses_a_change_of_an_immutable_policy_and_keeps_its_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ConformanceClaimTest.java::javapsm_2_the_readme_names_the_conformance_levels_of_clause2`

**Status:** `done` — Die Konformitätsstufen von Klausel 2 sind in `crates/java-omgdds/README.md` genannt: Minimum, Content-Subscription, Persistence und Ownership Profile sowie Extensible and Dynamic Types, ohne das Object Model Profile (DLRL); `ConformanceClaimTest` prüft die Aussage. Die Schnittstellen dieser Stufen sind in den Items dieses Dokuments einzeln belegt: Entity (set_qos mit IMMUTABLE_POLICY und INCONSISTENT_POLICY, Listener-Weitergabe, StatusCondition), DomainParticipant (ignore_*, closeContainedEntities), die Zuweisbarkeit der Typen, die Coherent Sets, die Lebendigkeit und die Wartefunktionen auch gegenüber Peers in anderen Prozessen. Was in einzelnen Operationen offen ist, steht als eigenes offenes Item. (Commit `2de26533680`)
### §2 Object Model Profile (DLRL) ausgenommen

**Spec:** §2, S. 11 — "The one exception to this rule is the Object Model
Profile, which includes in part the Data Local Reconstruction Layer (DLRL); DLRL
is outside of the scope of this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abgrenzung durch die Spec. Das Object Model Profile umfasst DLRL (im Code nicht vorhanden) und access_scope = GROUP von PRESENTATION; die Policy PRESENTATION gibt es als org.omg.dds.core.policy.Presentation, ihre Wirkung wurde hier nicht geprüft.
### §2 Stufe Extensible and Dynamic Types

**Spec:** §2, S. 11 — "In addition to the conformance levels defined in the DDS
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_7_8_sedp_carries_the_type_information_on_its_parameter_id`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::javapsm_2_a_writer_and_a_reader_of_different_type_names_do_not_connect`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_an_appended_member_is_assignable_in_both_directions`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_a_member_of_the_same_id_and_another_type_is_not_assignable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_different_extensibility_is_not_assignable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_a_derived_type_is_declared_as_its_base`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_an_assignable_reader_takes_the_samples_of_another_type_name`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_an_exact_reader_does_not_take_them`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_an_assignable_reader_does_not_take_a_clashing_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypeCoercionTest.java::javapsm_2_a_declared_reader_takes_a_derived_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_2_an_assignable_reader_takes_a_remote_writer_of_another_type_name_by_its_type_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_2_an_exact_reader_does_not_take_that_remote_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_2_a_writer_takes_a_remote_reader_of_another_type_name_that_allows_coercion_and_has_an_assignable_type`

**Status:** `done` — Die Typkonsistenz wirkt beim Abgleich von Reader und Writer. EXACT_TYPE_TYPE_CONSISTENCY und EXACT_NAME_TYPE_CONSISTENCY verlangen denselben Typnamen (`InProcessBus` führt je Domäne, Topicname und Typname einen Kanal). Ein Reader mit ASSIGNABLE_TYPE_CONSISTENCY nimmt Writer eines anderen Typnamens am selben Topic, wenn `TypeAssignability.assignableFrom` (die Regeln von XTypes 1.3 §7.2.4.4: Alias aufgelöst, gleiche Primitive, Zeichenbreite bei Strings, Array-Grenzen gleich, Elemente „stark zuweisbar“, Enumerationen mit gleichen Namen und Werten, Strukturen nach Member-IDs und -Namen, Key-, Optional- und Must-Understand-Regeln, bei APPENDABLE und FINAL die Member an gleicher Stelle) den Typ des Writers als zuweisbar ausweist; mit DECLARED_TYPE_CONSISTENCY nimmt er nur Writer, deren Typ den seinen als Basistyp nennt (`TypeAssignability.declared`). Im selben Prozess gibt `InProcessBus` die Samples über Verbindungen (`Link`) weiter. Über RTPS entscheidet der Reader aus dem TypeObject (PID 0x0072), das der Writer ankündigt (`TypeObjectToDynamic`); der Writer nimmt einen Reader anderen Typnamens, der in SEDP `ALLOW_TYPE_COERCION` ankündigt (ASSIGNABLE und DECLARED werden so geschrieben, EXACT als `DISALLOW`) und dessen TypeObject von seinem Typ aus zuweisbar ist. Ohne angekündigtes TypeObject lässt sich ein Peer nicht beurteilen und wird nicht verbunden. `@ignore_literal_names` und die Regeln der Union-Etiketten je Etikett gehören zu XTypes 1.3 §7.2.4.4 und sind nicht ausgewertet. (Commit `178c642cbc7`)
### §2 XML-QoS-Profile: Operationen vollständig oder UnsupportedOperationException

**Spec:** §2, S. 11 — "This PSM furthermore defines methods to create Entities
and to set their QoS based on the XML QoS libraries and profiles defined by the
DDS for Lightweight CCM specification. Implementations that support these XML
QoS profiles shall implement these operations fully; other implementations shall
throw java.lang.UnsupportedOperationException."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.setQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_qos_of_every_entity_kind_carries_the_values_of_the_document`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_writer_and_reader_qos_carry_the_values_of_the_document_and_keep_the_defaults_of_what_it_leaves_out`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_profile_inherits_from_its_base_and_overrides_field_by_field`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_another_scheme_than_file_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_2_entity_set_qos_with_an_uri_and_a_profile_applies_the_qos_of_that_profile`

**Status:** `done` — QosProviderImpl liest DDS-XML-1.0-Bibliotheken (`dds-xml_qos_definitions.xsd`) von `file://`-URIs: `<qos_library>`, `<qos_profile name base_name>`, sieben Entity-QoS-Elemente, die 22 Policy-Elemente der Schema-Datei mit deren Literalen (`RELIABLE_RELIABILITY_QOS`), Dauer als `<sec>`/`<nanosec>` mit `DURATION_INFINITY`, `LENGTH_UNLIMITED`, base64 für Octet-Folgen, Partition als `<name><element>`. Vererbung über `base_name` auf Profil- und auf Element-Ebene, Zyklus, später definierte Basis und fehlende Basis sind `IllegalArgumentException`. Jedes andere URI-Schema löst `UnsupportedOperationException` aus. Unbekannte Elemente, Felder, Literale und fehlerhafte Werte sind `IllegalArgumentException`, nichts wird ohne Meldung übergangen. `Entity.setQos(String, String)` ruft den Provider über `EntityBase.setQos` mit dem ersten Argument als URI. Der frühere Leser erwartete ein Wurzelelement je Entity-Element und Java-Enum-Namen und las damit kein DDS-XML-Dokument. Getestet in `QosProviderTest` (alle Entity-Arten, Vererbung, Fehlerfälle, anderes Schema, `setQos(uri, profile)` an allen sechs Entitäten). (Commit `a68b0889e7a`)
### §2 Mindestens eine Type Representation von [DDS-XTypes] oder §8

**Spec:** §2, S. 11 — "Finally, any conformant implementation must support at
least one of the OMG-specified Type Representations defined by [DDS-XTypes]
and/or in the Java Type Representation section of this specification (Clause
8)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`tools/idlc/src/java_support.rs` (`source`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`

**Status:** `done` — Die Java Type Representation ist umgesetzt: TypeSupportResolver.resolve bildet eine Klasse ohne generierten Code über ReflectionTypeSupport ab. javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure vergleicht die kodierten Bytes einer Bean mit dem Wire-Vektor (01 00 00 00 FE FF FF FF) und dekodiert zurück; TypesJavaRepresentationTest::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked, ::javapsm_8_4_a_big_decimal_without_annotation_is_a_float128 und ::javapsm_8_2_an_enum_field_is_its_ordinal_as_an_int32 prüfen die Oktette von BigInteger, BigDecimal und Enum; javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code schreibt und liest eine Bean über Topic, Writer und Reader. Die IDL Type Representation kommt über tools/idlc/src/java_support.rs in die erzeugten Typen. (Commit `819bfcb46cb`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: Entity

### §2 Entity: set_qos und get_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 25–26 (`dds-1.2.pdf`), Tabelle
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_takes_new_values_of_the_changeable_policies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_refuses_a_change_of_an_immutable_policy_and_keeps_its_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_setting_the_same_value_of_an_immutable_policy_is_no_change`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_reader_takes_a_time_based_filter_and_refuses_another_history`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_deadline_shorter_than_the_minimum_separation_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_topic_takes_topic_data_and_refuses_another_reliability`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_publisher_takes_a_partition_and_group_data_and_refuses_another_presentation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_subscriber_takes_a_partition_and_refuses_another_presentation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_participant_and_the_factory_take_new_user_data_and_entity_factory`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_before_enable_any_policy_changes_and_after_enable_the_immutable_ones_do_not`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::javapsm_2_a_publisher_that_changes_its_partition_ends_and_starts_matches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::javapsm_2_a_subscriber_that_changes_its_partition_ends_and_starts_matches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPartitionChangeTest.java::javapsm_2_a_change_that_keeps_the_overlap_is_no_match_event`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosBuiltinUpdateTest.java::javapsm_2_a_changed_qos_of_a_writer_and_of_its_publisher_is_a_new_sample_on_the_publication_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosBuiltinUpdateTest.java::javapsm_2_a_changed_qos_of_a_reader_and_of_its_subscriber_is_a_new_sample_on_the_subscription_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_2_a_changed_qos_of_a_writer_reaches_the_reader_of_another_process_over_sedp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinQosAnnouncementTest.java::javapsm_2_a_changed_topic_qos_is_a_new_sample_of_the_topic_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_history_depth_above_max_samples_per_instance_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_builtin_reader_sees_the_user_data_change_of_another_participant`

**Status:** `done` — `getQos` ist an allen sechs Entitäten vorhanden. `setQos` prüft über `org.zerodds.core.qos.QosRules.checkChange` zuerst die Verträglichkeit der Policies (`InconsistentPolicyException`, auch wenn zugleich eine unveränderliche Policy geändert wird) und nach `enable` die Policies, deren Spalte „Changeable“ in DDS 1.4 §2.2.3 „No“ ist (DURABILITY, DURABILITY_SERVICE, PRESENTATION, OWNERSHIP, LIVELINESS, RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS, DATA_REPRESENTATION, TYPE_CONSISTENCY_ENFORCEMENT; `ImmutablePolicyException`); ein gleicher Wert ist keine Änderung; bei beiden Ausnahmen bleibt die QoS unverändert. Das gilt für DataWriter, DataReader, Topic, Publisher, Subscriber, DomainParticipant und DomainParticipantFactory. Nach einer Änderung von DEADLINE oder LIVELINESS laufen die Fristen neu an; eine geänderte PARTITION lässt Writer und Reader neu matchen. Die geänderte QoS eines Writers, Readers, Publishers, Subscribers, Topics oder Participants ist ein neues Sample derselben Instanz in „DCPSPublication“, „DCPSSubscription“, „DCPSTopic“ und „DCPSParticipant“, und Writer und Reader, die über RTPS verbunden sind, kündigen sie erneut in SEDP an (`WireWriter.updateQos`, `WireReader.updateQos`); der Peer gleicht danach neu ab. Die SEDP-Writer halten je Endpunkt und Peer nur die letzte Ankündigung (KEEP_LAST 1), sodass erneute Ankündigungen die Historie nicht füllen. (Commit `447490d2b6d`)
### §2 Entity: set_listener und get_listener

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle Entity
— "abstract set_listener (a_listener: Listener, mask: StatusKind []):
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_a_writer_listener_that_does_not_take_a_status_leaves_it_to_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_an_empty_mask_takes_no_status_and_a_null_mask_takes_all`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_passes_offered_incompatible_qos_to_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_and_a_publisher_without_one_pass_it_to_the_participant_listener`

**Status:** `done` — Die Listener von DomainParticipant, Publisher, Subscriber und Topic werden aufgerufen. Ein Status geht an den Listener der Entität, wenn dieser den Status in seiner Maske hat (null = alle, leer = keine), sonst an den der enthaltenden Entität: DataWriter an Publisher und DomainParticipant, DataReader an Subscriber und DomainParticipant, Topic an DomainParticipant (ListenerUp, EntityBase.ownListener, DataWriterImpl.listenerFor, TopicImpl.listenerFor). Tests: Weitergabe vom Writer zu Publisher und Participant (ListenerPropagationTest), Statusmaske eines Writer-Listeners und leere und null-Maske (ParticipantSubscriberTest), INCONSISTENT_TOPIC an Topic- und Participant-Listener (TopicOperationsTest), OFFERED_INCOMPATIBLE_QOS an Publisher- und Participant-Listener (PublisherOperationsTest). (Commit `f31b6fa11b5`)
### §2 Entity: Statusmaske und Listener beim Erzeugen

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26, §7.1.2.2.1, S. 34–35
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_listener_set_at_creation_sees_the_match_of_enable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_topic_with_a_listener_and_a_mask_keeps_them`

**Status:** `done` — Die create*-Operationen mit Listener und Statusmenge setzen den Listener vor enable: DomainParticipantImpl.newPublisher, newSubscriber und newTopic, PublisherImpl.createDataWriter, SubscriberImpl.createDataReader; DomainParticipantFactoryImpl.createParticipant setzt ihn vor dem Eintragen des Participants. Ein SUBSCRIPTION_MATCHED oder PUBLICATION_MATCHED einer vorhandenen Gegenstelle erreicht damit den beim Erzeugen gesetzten Listener. Tests: Writer und Reader mit Listener bei vorhandener Gegenstelle (ListenerPropagationTest), Publisher und Subscriber mit Listener und später erzeugtem Writer oder Reader, Participant mit Listener und Maske, Topic mit Listener (ParticipantEntityTest). (Commit `cd8f3d67ea9`)
### §2 Entity: enable

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle Entity
— "enable: ReturnCode_t"

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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_publisher_with_auto_enable_false_creates_disabled_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_reader_of_a_disabled_subscriber_is_disabled_and_enabling_the_subscriber_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_subscriber_with_auto_enable_false_creates_disabled_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::javapsm_7_2_4_concurrent_enable_registers_the_endpoint_once`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_not_enabled_writer_refuses_to_write`

**Status:** `done` — enable folgt DDS 1.4 §2.2.2.1.1.7. Eine Entität, deren Factory nicht aktiviert ist, wird deaktiviert angelegt, und enable auf ihr ist PreconditionNotMetException (PublisherImpl.enable, SubscriberImpl.enable, DataWriterImpl.enable, DataReaderImpl.enable). DomainParticipantImpl.enable, PublisherImpl.enable und SubscriberImpl.enable aktivieren die bisher erzeugten Kinder, wenn ENTITY_FACTORY autoenable_created_entities gilt; createDataWriter und createDataReader aktivieren nur bei aktiviertem Publisher/Subscriber und dessen autoenable_created_entities. Der eingebaute Publisher und Subscriber werden mit enableBuiltin aktiviert. Tests: Factory mit autoenable = false (Participant und Kinder deaktiviert, PreconditionNotMetException, Aktivierung durch enable), Writer eines deaktivierten Publishers, Publisher mit autoenable = false, dieselben Fälle für Reader und Subscriber (ParticipantEntityTest). (Commit `cd8f3d67ea9`)
### §2 Entity: get_statuscondition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle Entity
— "get_statuscondition: StatusCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_status_is_reset_when_it_is_read_and_data_available_when_the_samples_are_taken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_the_status_changes_of_a_writer_list_the_pending_statuses_until_they_are_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_an_inconsistent_topic_triggers_its_status_condition`

**Status:** `done` — getStatusCondition liefert eine StatusConditionImpl, deren Trigger-Wert aus den von triggerStatus gesetzten und vom Lesen des jeweiligen Status gelöschten Statusarten folgt. DataReaderImpl setzt DATA_AVAILABLE, SAMPLE_REJECTED, SAMPLE_LOST, SUBSCRIPTION_MATCHED, REQUESTED_INCOMPATIBLE_QOS und REQUESTED_DEADLINE_MISSED über triggerStatus, DataWriterImpl PUBLICATION_MATCHED, OFFERED_INCOMPATIBLE_QOS, OFFERED_DEADLINE_MISSED und LIVELINESS_LOST, TopicImpl INCONSISTENT_TOPIC. Tests: WaitSet mit StatusCondition eines Readers (WaitSetTest), Reset durch Lesen (ReaderStatusTest), StatusCondition eines Writers und eines Topics (WriterStatusTest, TopicOperationsTest). (Commit `d23eff980f3`)
### §2 Entity: get_status_changes

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle Entity
— "get_status_changes: StatusKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.triggerStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_status_is_reset_when_it_is_read_and_data_available_when_the_samples_are_taken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_the_status_changes_of_a_writer_list_the_pending_statuses_until_they_are_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_missed_offered_deadline_is_counted_with_the_instance_that_missed_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`

**Status:** `done` — EntityBase.getStatusChanges liefert die Menge der ausgelösten Statusarten, ohne sie zu leeren; der Status wird beim Lesen des jeweiligen Status (get<Status>Status, Lesen und Nehmen für DATA_AVAILABLE) zurückgesetzt (EntityBase.clearStatus). Tests: Reader nach Match, Lesen des Status und Take (ReaderStatusTest), Writer nach Match und Lesen (WriterStatusTest), Topic nach INCONSISTENT_TOPIC (TopicOperationsTest). Deadline-Status: WriterStatusTest::javapsm_2_a_missed_offered_deadline_is_counted_with_the_instance_that_missed_it wartet mit getStatusChanges auf OfferedDeadlineMissedStatus. (Commit `166d5873d4a`)
### §2 Entity: get_instance_handle

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.1, S. 26 (`dds-1.2.pdf`), Tabelle Entity
— "get_instance_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/Values.java` (`Values.ofUuid`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_contains_entity_walks_the_containment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_topic_data_carries_the_topic_qos`

**Status:** `done` — Jede Entität hält einen im Konstruktor aus einer UUID gebildeten, unveränderlichen Handle (EntityBase). javapsm_2_contains_entity_walks_the_containment übergibt die Handles von Publisher, Writer, Subscriber, Reader und Topic an containsEntity (alle wahr, Nil-Handle falsch); der Schlüssel in den Daten des eingebauten Participant-Topics ist gleich dem getInstanceHandle des anderen Participants, und getDiscoveredTopicData findet ein Topic über dessen Handle. Die Eindeutigkeit folgt aus der UUID und wird nicht unmittelbar geprüft. (Commit `d23eff980f3`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DomainParticipantFactory

### §2 DomainParticipantFactory: create_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "create_participant (domain_id: DomainId_t, qos_list:
QosPolicy [], a_listener: DomainParticipantListener, mask: StatusKind []):
DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.createParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.createParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant`

**Status:** `done` — createParticipant(int, DomainParticipantQos, DomainParticipantListener, Collection) setzt Listener und Statusmenge vor dem Eintragen des Participants in die Factory (DomainParticipantFactoryImpl.createParticipant). ParticipantEntityTest::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant ruft die Form mit Listener und Maske {PUBLICATION_MATCHED} und prüft, dass PUBLICATION_MATCHED eines Writers ohne eigenen Listener den Participant-Listener erreicht und SUBSCRIPTION_MATCHED, das nicht in der Maske steht, nicht. (Commit `2e0b194caf5`)
### §2 DomainParticipantFactory: ein neuer Participant je Aufruf

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2.1, S. 46 (`dds-1.2.pdf`) — "This
operation creates a new DomainParticipant object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.createParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.register`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantFactoryTest.java::javapsm_2_create_participant_creates_a_new_participant_each_call`

**Status:** `done` — createParticipant legt je Aufruf ein neues DomainParticipantImpl an und trägt es in eine Liste je Domain ein (DomainParticipantFactoryImpl.register). Der Test erzeugt zwei Participants in derselben Domain und prüft assertNotSame; der Hinweis auf computeIfAbsent im bisherigen Item ist überholt. (Commit `2e0b194caf5`)
### §2 DomainParticipantFactory: delete_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45, §7.1.2.2.2.2, S. 46
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_the_transient_data_of_a_domain_survives_the_close_of_a_participant_while_another_one_is_open`

**Status:** `done` — DomainParticipantImpl.close schließt die enthaltenen Entitäten (§7.2.3), nimmt den Participant aus der Factory und aus der Tabelle aller Participants des Prozesses und räumt die TRANSIENT-Daten der Domain nur, wenn danach kein Participant der Domain mehr offen ist (DomainParticipantFactoryImpl.participantsInDomain, DurabilityService.clearDomain). DomainParticipantFactory hat keine deleteParticipant-Operation (OMG-API). ParticipantEntityTest::javapsm_2_the_transient_data_of_a_domain_survives_the_close_of_a_participant_while_another_one_is_open schreibt TRANSIENT-Daten, schließt den Writer und einen von zwei Participants (Daten bleiben), prüft lookupParticipant, schließt den zweiten (lookupParticipant liefert null, ein neuer Reader erhält nichts). (Commit `cd8f3d67ea9`)
### §2 DomainParticipantFactory: get_instance

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45, §7.1.2.2.2.3, S. 46
(`dds-1.2.pdf`) — "(static) get_instance: DomainParticipantFactory"; "it will
return the same DomainParticipantFactory instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_2_4_several_threads_get_one_participant_factory_of_an_environment`

**Status:** `done` — ZeroSPI.getParticipantFactory legt die Factory mit volatile-Feld und unter Sperre an (ServiceEnvironmentImpl). CoreSpiTest::javapsm_7_2_4_several_threads_get_one_participant_factory_of_an_environment lässt acht Threads gleichzeitig (CyclicBarrier) die Factory einer neuen Umgebung anfordern, 25 Runden, und erwartet dasselbe Objekt; die bisherigen Tests prüfen assertSame bei zwei Aufrufen, assertNotSame zwischen zwei Umgebungen und die NullPointerException bei null. (Commit `36e8fd3f496`)
### §2 DomainParticipantFactory: lookup_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "lookup_participant (domain_id: DomainId_t):
DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.lookupParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.lookupParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_participant_finds_one_of_the_domain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_the_transient_data_of_a_domain_survives_the_close_of_a_participant_while_another_one_is_open`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table`

**Status:** `done` — lookupParticipant(int) liefert den ersten Participant der Domain aus der Tabelle der Factory, sonst null; nach close ist der Participant aus der Tabelle genommen. Tests: DomainParticipantOperationsTest::javapsm_2_lookup_participant_finds_one_of_the_domain (assertSame, andere Domain null); ParticipantEntityTest::javapsm_2_the_transient_data_of_a_domain_survives_the_close_of_a_participant_while_another_one_is_open (nach dem Schließen null); ParticipantFactoryEnvironmentTest::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table (die Factory einer anderen Umgebung findet den Participant nicht). (Commit `2e0b194caf5`)
### §2 DomainParticipantFactory: set_default_participant_qos und get_default_participant_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "set_default_participant_qos (qos_list: QosPolicy
[]): ReturnCode_t; get_default_participant_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setDefaultParticipantQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.setDefaultParticipantQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_default_qos_whose_policies_are_not_consistent_is_refused_and_keeps_the_old_one`

**Status:** `done` — getDefaultParticipantQos liefert den gesetzten Wert; ein danach mit createParticipant(int) erzeugter Participant hat genau dieses QoS (assertEquals auf getQos). DDS 1.4 verlangt für set_default_participant_qos die Prüfung auf Konsistenz; für die Policies eines DomainParticipantQos (USER_DATA, ENTITY_FACTORY) nennt §2.2.3 keine Regel, die zwei Werte inkonsistent macht, die Prüfung ist hier also leer. Die Default-Operationen von Publisher, Subscriber, Topic und Writer prüfen mit QosRules.checkConsistent (ParticipantEntityTest, PublisherOperationsTest). (Commit `2e0b194caf5`)
### §2 DomainParticipantFactory: get_qos und set_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.2, S. 45 (`dds-1.2.pdf`), Tabelle
DomainParticipantFactory — "get_qos (out: qos_list: QosPolicy []): ReturnCode_t;
set_qos (qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled`

**Status:** `done` — getQos und setQos der Factory sind vorhanden; DomainParticipantFactoryImpl.autoEnable wertet ENTITY_FACTORY aus. ParticipantEntityTest::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled setzt ein Factory-QoS mit autoEnableCreatedEntities = false, erzeugt einen Participant, erwartet isEnabled() = false, die Ablehnung von enable auf seinen Kindern mit PreconditionNotMetException und nach enable() die aktivierten Kinder; das Factory-QoS wird im finally zurückgesetzt. (Commit `2e0b194caf5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DomainParticipant

### §2 DomainParticipant: create_publisher und delete_publisher

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 34–35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_publisher (qos_list: QosPolicy [], a_listener:
PublisherListener, mask: StatusKind []): Publisher; delete_publisher
(a_publisher: Publisher): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createPublisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_publisher_and_subscriber_leave_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_publisher_close_closes_its_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later`

**Status:** `done` — createPublisher() hat die Formen ohne Argument, mit PublisherQos und mit Listener und Statusmenge; der Listener wird vor enable gesetzt (DomainParticipantImpl.newPublisher). Ein geschlossener Publisher verlässt die Liste des Participants über den Close-Hook (containsEntity liefert danach false). Tests: PublisherQos des Aufrufs und Default (DomainParticipantOperationsTest, ParticipantEntityTest), Überladung mit Listener (ParticipantEntityTest::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later), containsEntity nach close (CloseCleanupTest::javapsm_2_a_closed_publisher_and_subscriber_leave_the_participant), Schließen der Writer (CloseCascadeTest). (Commit `28e25c198d1`)
### §2 DomainParticipant: create_subscriber und delete_subscriber

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_subscriber (qos_list: QosPolicy [], a_listener:
SubscriberListener, mask: StatusKind []): Subscriber; delete_subscriber
(a_subscriber: Subscriber): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_closing_a_subscriber_closes_its_readers_and_leaves_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_publisher_and_subscriber_leave_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`

**Status:** `done` — createSubscriber() hat die Formen ohne Argument, mit SubscriberQos und mit Listener und Statusmenge; der Listener wird vor enable gesetzt (DomainParticipantImpl.newSubscriber). SubscriberImpl.close schließt die DataReader, und der Subscriber verlässt die Liste des Participants. Tests: ParticipantSubscriberTest::javapsm_2_closing_a_subscriber_closes_its_readers_and_leaves_the_participant (Reader geschlossen, read wirft AlreadyClosedException, containsEntity false, createDataReader wirft AlreadyClosedException), ParticipantEntityTest::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later. (Commit `28e25c198d1`)
### §2 DomainParticipant: create_topic und delete_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_topic (topic_name: string, type_name: string,
qos_list: QosPolicy [], a_listener: TopicListener, mask: StatusKind []): Topic;
delete_topic (a_topic: Topic): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_closed_topic_is_gone_from_lookup_find_and_discovery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_topic_with_a_listener_and_a_mask_keeps_them`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_topic_in_use_cannot_be_closed_until_its_writers_and_readers_are`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`

**Status:** `done` — createTopic mit Class, TypeSupport und DynamicType, mit und ohne TopicQos, Listener und Statusmenge; der Listener wird vor enable gesetzt (DomainParticipantImpl.newTopic). TopicImpl.close wirft PreconditionNotMetException, solange DataWriter, DataReader, ContentFilteredTopic oder MultiTopic das Topic nutzen (TopicImpl.use/release, von ContentFilteredTopicImpl und MultiTopicImpl mitgezählt), und das Topic verlässt über den Close-Hook die Liste des Participants und das DCPSTopic. Tests: TypeSupport-Formen (TopicOperationsTest::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class, ::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name), übergebenes TopicQos (QosDefaultQosTest::javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next), Listener (ParticipantEntityTest::javapsm_2_create_topic_with_a_listener_and_a_mask_keeps_them), close mit Nutzern und danach (CloseCleanupTest, TopicOperationsTest). (Commit `36e8fd3f496`)
### §2 DomainParticipant: create_contentfilteredtopic und delete_contentfilteredtopic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_set_expression_parameters_filters_with_the_new_parameters_from_then_on`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_filter_takes_its_parameters_by_position`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_a_malformed_expression_is_rejected`

**Status:** `done` — ContentFilteredTopicImpl.setExpressionParameters übersetzt den Ausdruck mit ContentFilter.compile und den neuen Parametern neu; ein Parametersatz, den der Ausdruck nicht aufnimmt (fehlendes %n), wird mit IllegalArgumentException abgelehnt und der alte bleibt. close wirft PreconditionNotMetException, solange ein Reader das Topic liest, gibt das zugehörige Topic frei und entfernt den Eintrag aus der Liste des Participants (lookupTopicDescription liefert null); ein geschlossenes Topic ist für neue Reader AlreadyClosedException. Tests: TopicOperationsTest::javapsm_2_set_expression_parameters_filters_with_the_new_parameters_from_then_on, ::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant, ::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends, ::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant (lookupTopicDescription). (Commit `36e8fd3f496`)
### §2 DomainParticipant: create_multitopic und delete_multitopic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "create_multitopic (name: string, type_name: string,
subscription_expression: string, expression_parameters: string []): MultiTopic;
delete_multitopic (a_multitopic: MultiTopic): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createMultiTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createMultiTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicEngine.java` (`MultiTopicEngine`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_delete_contained_entities_removes_every_topic_description_from_the_lookups`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_a_reader_on_the_multi_topic_receives_the_matching_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_2_the_annex_b_example_joins_projects_renames_and_filters`

**Status:** `done` — MultiTopicImpl.close wirft PreconditionNotMetException, solange ein Reader das MultiTopic liest, gibt die Topics der FROM-Klausel frei (MultiTopicImpl.bind hält sie) und entfernt den Eintrag aus der Liste des Participants; ein geschlossenes MultiTopic ist für neue Reader AlreadyClosedException. Tests: TopicOperationsTest::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause, ::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends, ::javapsm_2_delete_contained_entities_removes_every_topic_description_from_the_lookups. (Commit `36e8fd3f496`)
### §2 DomainParticipant: find_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 35–36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "find_topic (topic_name: string, timeout: Duration_t):
Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.findTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.findTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.awaitTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_find_topic_waits_for_a_topic_another_thread_creates_and_times_out_otherwise`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_find_topic_returns_the_topic_and_times_out_otherwise`

**Status:** `done` — findTopic(String, Duration) und findTopic(String, long, TimeUnit) liefern ein vom Participant erzeugtes Topic, warten sonst bis zum Timeout auf eines, das ein anderer Thread erzeugt, und werfen TimeoutException. DDS 1.4 §2.2.2.2.1.11 überlässt der Implementierung, entfernt erzeugte Topics zugänglich zu machen; hier gibt es sie nicht. Tests: TopicOperationsTest::javapsm_2_find_topic_waits_for_a_topic_another_thread_creates_and_times_out_otherwise (ein zweiter Thread erzeugt das Topic nach 200 ms, findTopic mit 30 s Grenze liefert es; ein unbekannter Name läuft nach 300 ms in die TimeoutException, Mindestdauer 290 ms), DomainParticipantOperationsTest::javapsm_2_find_topic_returns_the_topic_and_times_out_otherwise. (Commit `819bfcb46cb`)
### §2 DomainParticipant: lookup_topicdescription

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "lookup_topicdescription (name: string): TopicDescription"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.lookupTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.lookupTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.allTopicDescriptions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_topic_description_finds_only_what_was_created`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_is_a_topic_description_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_participant_creates_the_four_builtin_topics`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`

**Status:** `done` — lookupTopicDescription durchsucht Topics, ContentFilteredTopics und MultiTopics des Participants; geschlossene Beschreibungen sind aus den Listen genommen. Tests: Topic mit assertSame, Name und Typname und unbekannter Name null (DomainParticipantOperationsTest), MultiTopic (MultiTopicTest), eingebaute Topics (BuiltinTopicsTest), ContentFilteredTopic und null nach close (TopicOperationsTest::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant, ::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant). (Commit `819bfcb46cb`)
### §2 DomainParticipant: get_builtin_subscriber

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_builtin_subscriber: Subscriber"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getBuiltinSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getBuiltinSubscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/Ignores.java` (`Ignores.pairIgnored`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_builtin_subscriber_holds_one_reader_per_builtin_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_a_builtin_reader_has_the_entities_that_existed_before_and_none_of_its_own_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_the_builtin_reader_of_a_participant_gives_no_data_about_its_own_entities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_get_builtin_subscriber_is_the_same_object_every_time`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`

**Status:** `done` — getBuiltinSubscriber legt einmal einen aktivierten Subscriber mit je einem DataReader für DCPSParticipant, DCPSPublication, DCPSSubscription und DCPSTopic an (DURABILITY TRANSIENT_LOCAL, RELIABILITY RELIABLE, HISTORY KEEP_LAST 1, DDS 1.4 §2.2.5); lookupDataReader(Topicname) findet sie. Die eingebauten Writer bieten TRANSIENT_LOCAL (Builtin.writer), sodass ein später angelegter Reader die bereits bekannten Entitäten erhält. Die eingebauten Reader liefern keine Daten über Entitäten des eigenen Participants (Ignores.pairIgnored, §2.2.5) und werden selbst nicht auf DCPSSubscription angekündigt. Tests: ParticipantDiscoveryTest::javapsm_2_the_builtin_subscriber_holds_one_reader_per_builtin_topic, ::javapsm_2_a_builtin_reader_has_the_entities_that_existed_before_and_none_of_its_own_participant, ParticipantIgnoreTest::javapsm_2_the_builtin_reader_of_a_participant_gives_no_data_about_its_own_entities. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: ignore_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_participant_before_the_writer_exists_keeps_its_data_and_its_match_away`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_participant_after_the_match_ends_it_and_stops_the_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_an_ignored_participant_does_not_ignore_the_other_way`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_participant_of_another_process_is_discovered_and_can_be_ignored`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_participant_ends_the_match_with_its_readers_and_keeps_it_ended`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_discovery_lists_the_participants_of_the_domain_and_ignore_hides_one`

**Status:** `done` — ignoreParticipant trägt den Handle in die Ignorier-Mengen des Participants ein (Ignores.of(...).ignoreParticipant) und gleicht die Paare neu ab (InProcessBus.rematchIgnored): kein Endpunkt des ignorierten Participants wird mit einem Endpunkt dieses Participants gepaart, bestehende Paarungen enden, und die Daten kommen nicht an. Für den RTPS-Pfad trägt RtpsParticipant.ignoreParticipant das GUID-Präfix ein und entfernt Übereinstimmungen; ein späteres SEDP-Announcement paart nicht neu. getDiscoveredParticipants lässt den Handle aus, getDiscoveredParticipantData lehnt ihn mit PreconditionNotMetException ab. Tests: ParticipantIgnoreTest::javapsm_2_ignore_participant_before_the_writer_exists_keeps_its_data_and_its_match_away, ::javapsm_2_ignore_participant_after_the_match_ends_it_and_stops_the_data, ::javapsm_2_an_ignored_participant_does_not_ignore_the_other_way; RTPS: WriterWireIgnoreTest::javapsm_2_ignore_participant_ends_the_match_with_its_readers_and_keeps_it_ended, ParticipantWireDiscoveryTest::javapsm_2_a_participant_of_another_process_is_discovered_and_can_be_ignored. Der RTPS-Participant eines Prozesses je Domain ist allen Participants der Domain gemeinsam, ein Ignorieren dort gilt für alle. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: ignore_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_topic (handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/Ignores.java` (`Ignores.Rules.ignoreTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_topic_separates_the_topic_of_the_ignored_handle_from_the_endpoints_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_topic_ends_the_match_with_the_readers_of_the_topic`

**Status:** `done` — ignoreTopic löst den Topic-Handle in einen Topicnamen auf (eigene Topics, Topics anderer Participants der Domain, aus SEDP bekannte Topics) und trägt den Namen ein: keine Veröffentlichung und kein Abonnement des Topics wird mit einem Endpunkt dieses Participants gepaart, bestehende Paarungen enden. Das Topic fehlt in getDiscoveredTopics, getDiscoveredTopicData lehnt den Handle mit PreconditionNotMetException ab. RTPS: RtpsParticipant.ignoreTopic. Tests: ParticipantIgnoreTest::javapsm_2_ignore_topic_separates_the_topic_of_the_ignored_handle_from_the_endpoints_of_the_participant; RTPS: WriterWireIgnoreTest::javapsm_2_ignore_topic_ends_the_match_with_the_readers_of_the_topic. Ein Handle, der zum Zeitpunkt des Aufrufs keinem bekannten Topic gehört, wird nur gemerkt und wirkt nicht auf das Matching. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: ignore_publication

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_publication (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignorePublication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignorePublication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreEndpoint`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_publication_stops_the_data_of_one_writer_only`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_subscription_ends_the_match_with_one_reader`

**Status:** `done` — ignorePublication trägt den Writer-Handle ein: kein Sample dieses Writers erreicht einen Reader dieses Participants, und der Writer wird mit keinem Reader dieses Participants gepaart (bestehende Paarung endet). RTPS: RtpsParticipant.ignoreEndpoint mit der Endpunkt-GUID. Tests: ParticipantIgnoreTest::javapsm_2_ignore_publication_stops_the_data_of_one_writer_only (ein zweiter Writer desselben Participants liefert weiter); RTPS: WriterWireIgnoreTest::javapsm_2_ignore_subscription_ends_the_match_with_one_reader. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: ignore_subscription

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "ignore_subscription (handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.ignoreSubscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.ignoreSubscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.ignoreEndpoint`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_subscription_stops_the_data_to_one_reader_only`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_subscription_ends_the_match_with_one_reader`

**Status:** `done` — ignoreSubscription trägt den Reader-Handle ein: kein Writer dieses Participants paart sich mit diesem Reader oder sendet ihm Daten (bestehende Paarung endet, getMatchedSubscriptions und PUBLICATION_MATCHED folgen). RTPS: RtpsParticipant.ignoreEndpoint. Tests: ParticipantIgnoreTest::javapsm_2_ignore_subscription_stops_the_data_to_one_reader_only (ein zweiter Reader erhält weiter); RTPS: WriterWireIgnoreTest::javapsm_2_ignore_subscription_ends_the_match_with_one_reader. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: get_domain_id

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_domain_id: DomainId_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDomainId`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDomainId`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_get_domain_id_is_the_domain_the_participant_was_created_in`

**Status:** `done` — getDomainId() gibt die Domain zurück, mit der der Participant erzeugt wurde (Feld domainId in DomainParticipantImpl). ParticipantEntityTest::javapsm_2_get_domain_id_is_the_domain_the_participant_was_created_in erzeugt Participants in den Domains 1200 und 1201 und vergleicht. (Commit `2e0b194caf5`)
### §2 DomainParticipant: delete_contained_entities

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_delete_contained_entities_removes_every_topic_description_from_the_lookups`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_participant_with_filtered_topics_multi_topics_and_readers_does_not_fail`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_close_contained_entities_closes_them_without_closing_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`

**Status:** `done` — closeContainedEntities schließt Publisher, Subscriber, ContentFilteredTopics, MultiTopics, Topics und den eingebauten Subscriber und lässt den Participant offen; die geschlossenen Entitäten verlassen über Close-Hooks die Listen des Participants. Tests: TopicOperationsTest::javapsm_2_delete_contained_entities_removes_every_topic_description_from_the_lookups (lookupTopicDescription liefert danach null, getDiscoveredTopics leer, containsEntity false), DomainParticipantOperationsTest::javapsm_2_close_contained_entities_closes_them_without_closing_the_participant, CloseCascadeTest::javapsm_7_2_3_participant_close_closes_contained_entities, TopicOperationsTest::javapsm_2_closing_a_participant_with_filtered_topics_multi_topics_and_readers_does_not_fail. (Commit `36e8fd3f496`)
### §2 DomainParticipant: assert_liveliness

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertedByParticipant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.livelinessAsserted`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_the_participant_asserts_the_liveliness_of_its_manual_by_participant_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_a_write_by_any_writer_of_the_participant_asserts_the_manual_by_participant_writers`

**Status:** `done` — DomainParticipantImpl.assertLiveliness erneuert die Lease der Writer des Participants mit LIVELINESS MANUAL_BY_PARTICIPANT (DataWriterImpl.assertedByParticipant); ein write oder assertLiveliness eines Writers dieser Art erneuert sie für die übrigen (PublisherImpl.writerAsserted, DomainParticipantImpl.livelinessAsserted), Writer mit MANUAL_BY_TOPIC bleiben unberührt. Tests: QosLivelinessTest::javapsm_7_3_5_1_the_participant_asserts_the_liveliness_of_its_manual_by_participant_writers, ::javapsm_7_3_5_1_the_participant_does_not_assert_the_liveliness_of_a_manual_by_topic_writer, ::javapsm_7_3_5_1_a_write_by_any_writer_of_the_participant_asserts_the_manual_by_participant_writers. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: set_default_publisher_qos und get_default_publisher_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_publisher_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_publisher_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createPublisher`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`

**Status:** `done` — setDefaultPublisherQos prüft die Konsistenz (QosRules.checkConsistent, INCONSISTENT_POLICY); die Policies eines PublisherQos (GROUP_DATA, PARTITION, PRESENTATION, ENTITY_FACTORY) haben nach DDS 1.4 §2.2.3 keine Regel gegeneinander. Der Test setzt ein PublisherQos mit PARTITION „mine“, liest es zurück und prüft, dass createPublisher().getQos() es hat. (Commit `2e0b194caf5`)
### §2 DomainParticipant: set_default_subscriber_qos und get_default_subscriber_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_subscriber_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_subscriber_qos (out: qos_list: QosPolicy []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultSubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultSubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createSubscriber`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_subscriber_qos_of_the_participant_is_the_qos_of_the_subscribers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`

**Status:** `done` — DomainParticipantImpl.createSubscriber() nimmt defaultSubscriberQos. QosDefaultQosTest::javapsm_7_3_5_4_the_default_subscriber_qos_of_the_participant_is_the_qos_of_the_subscribers_it_creates_next setzt ein SubscriberQos mit GROUP_DATA, liest es zurück und prüft, dass createSubscriber().getQos() gleich dem gesetzten QoS ist. (Commit `2e0b194caf5`)
### §2 DomainParticipant: set_default_topic_qos und get_default_topic_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "set_default_topic_qos (qos_list: QosPolicy []):
ReturnCode_t; get_default_topic_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.setDefaultTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_default_qos_whose_policies_are_not_consistent_is_refused_and_keeps_the_old_one`

**Status:** `done` — createTopic ohne QoS nimmt defaultTopicQos; setDefaultTopicQos prüft die Konsistenz (HISTORY depth gegen RESOURCE_LIMITS max_samples_per_instance, max_samples gegen max_samples_per_instance). QosDefaultQosTest::javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next prüft, dass createTopic(...).getQos() das gesetzte Default hat; ParticipantEntityTest::javapsm_2_a_default_qos_whose_policies_are_not_consistent_is_refused_and_keeps_the_old_one prüft INCONSISTENT_POLICY für beide Regeln und dass das alte Default bleibt. (Commit `2e0b194caf5`)
### §2 DomainParticipant: get_discovered_participants

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_participants (inout: participant_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredParticipants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipants`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.participantsIn`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin.attachRemote`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_a_closed_participant_is_not_discovered_any_more`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_participant_of_another_process_is_discovered_and_can_be_ignored`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_discovery_lists_the_participants_of_the_domain_and_ignore_hides_one`

**Status:** `done` — getDiscoveredParticipants liefert die Handles der Participants der Domain in diesem Prozess (aller Umgebungen, eigener eingeschlossen) und der von SPDP gemeldeten (RtpsParticipant, Handle aus participantGuid), ohne die ignorierten; ein geschlossener Participant fehlt. Tests: ParticipantDiscoveryTest::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data, ::javapsm_2_a_closed_participant_is_not_discovered_any_more, ParticipantFactoryEnvironmentTest::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table (Participants zweier Umgebungen), ParticipantWireDiscoveryTest::javapsm_2_a_participant_of_another_process_is_discovered_and_can_be_ignored (SPDP), ParticipantIgnoreTest. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: get_discovered_participant_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 36–37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_participant_data (inout: participant_data:
ParticipantBuiltinTopicData, participant_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredParticipantData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipantData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_participant_before_the_writer_exists_keeps_its_data_and_its_match_away`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_participant_data_carries_the_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_participant_of_another_process_is_discovered_and_can_be_ignored`

**Status:** `done` — getDiscoveredParticipantData liefert Schlüssel und USER_DATA eines Participants der Domain aus dem Prozess oder aus SPDP; ein ignorierter Handle und ein unbekannter Handle sind PreconditionNotMetException. Tests: ParticipantDiscoveryTest::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data (nicht leeres USER_DATA, Participant einer anderen Domain), ParticipantIgnoreTest::javapsm_2_ignore_participant_before_the_writer_exists_keeps_its_data_and_its_match_away (ignorierter Handle), BuiltinTopicsTest::javapsm_2_get_discovered_participant_data_carries_the_user_data (Nil-Handle), ParticipantWireDiscoveryTest (Schlüssel eines entfernten Participants). (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: get_discovered_topics

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_topics (inout: topic_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredTopics`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredTopics`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_topics_of_other_participants_of_the_domain_are_discovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_a_topic_of_an_ignored_participant_is_not_discovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_topic_separates_the_topic_of_the_ignored_handle_from_the_endpoints_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_topic_data_carries_the_topic_qos`

**Status:** `done` — getDiscoveredTopics liefert die Handles der Topics aller Participants der Domain in diesem Prozess und der aus SEDP bekannten Topics (Handle aus Name und Typname), ohne eingebaute Topics, ignorierte Topics und Topics ignorierter Participants. Tests: ParticipantDiscoveryTest::javapsm_2_the_topics_of_other_participants_of_the_domain_are_discovered, ::javapsm_2_a_topic_of_an_ignored_participant_is_not_discovered, ParticipantIgnoreTest::javapsm_2_ignore_topic_separates_the_topic_of_the_ignored_handle_from_the_endpoints_of_the_participant, ParticipantWireDiscoveryTest::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent, BuiltinTopicsTest::javapsm_2_get_discovered_topic_data_carries_the_topic_qos. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: get_discovered_topic_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_discovered_topic_data (inout: topic_data:
TopicBuiltinTopicData, topic_handle: InstanceHandle_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getDiscoveredTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/Builtin.java` (`Builtin.topicDataOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_topics_of_other_participants_of_the_domain_are_discovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_a_topic_of_an_ignored_participant_is_not_discovered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantIgnoreTest.java::javapsm_2_ignore_topic_separates_the_topic_of_the_ignored_handle_from_the_endpoints_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_topic_data_carries_the_topic_qos`

**Status:** `done` — getDiscoveredTopicData sucht den Handle bei den Topics aller Participants der Domain im Prozess und bei den aus SEDP bekannten Topics; ein ignorierter oder unbekannter Handle ist PreconditionNotMetException. Tests: ParticipantDiscoveryTest::javapsm_2_the_topics_of_other_participants_of_the_domain_are_discovered (Name, Typname, RELIABILITY gleich dem Wert des Topics, Ausnahme für einen Participant-Handle, Verschwinden nach close), ::javapsm_2_a_topic_of_an_ignored_participant_is_not_discovered, ParticipantIgnoreTest (ignoriertes Topic), ParticipantWireDiscoveryTest (Topic aus SEDP), BuiltinTopicsTest::javapsm_2_get_discovered_topic_data_carries_the_topic_qos. (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: contains_entity

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "contains_entity (a_handle: InstanceHandle_t): boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.containsEntity`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.containsEntity`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_contains_entity_walks_the_containment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_builtin_subscriber_holds_one_reader_per_builtin_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_publisher_and_subscriber_leave_the_participant`

**Status:** `done` — containsEntity durchsucht rekursiv Publisher, Subscriber, deren DataWriter und DataReader, Topics, den eingebauten Subscriber und dessen Reader; geschlossene Entitäten sind nicht mehr enthalten. Tests: DomainParticipantOperationsTest::javapsm_2_contains_entity_walks_the_containment (Handles von Publisher, Writer, Subscriber, Reader, Topic wahr, Nil-Handle falsch), ParticipantDiscoveryTest::javapsm_2_the_builtin_subscriber_holds_one_reader_per_builtin_topic (eingebaute Reader), CloseCleanupTest::javapsm_2_a_closed_publisher_and_subscriber_leave_the_participant (nach close falsch). (Commit `cd8f3d67ea9`)
### §2 DomainParticipant: get_current_time

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.1, S. 37 (`dds-1.2.pdf`), Tabelle
DomainParticipant — "get_current_time (inout: current_time: Time_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getCurrentTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_get_current_time_fills_the_container_or_returns_a_new_one`

**Status:** `done` — getCurrentTime() liefert ein neues unveränderliches Time, getCurrentTime(ModifiableTime) füllt den Container und gibt ihn zurück (assertSame); beide nehmen System.currentTimeMillis(), die Quelle, mit der DataWriterImpl Samples stempelt. Der Test prüft beide Formen mit einem Wert größer 0 Sekunden. (Commit `819bfcb46cb`)
### §2 DomainParticipantListener

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.2.3, S. 47–48 (`dds-1.2.pdf`) — "The
DomainParticipantListener interface extends all other Listener interfaces and
has no additional operation beyond the ones defined by the more general
listeners."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.setListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_and_a_publisher_without_one_pass_it_to_the_participant_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant`

**Status:** `done` — DataWriterImpl und DataReaderImpl reichen einen Status ohne eigenen Listener (oder ohne den Status in dessen Maske) an Publisher beziehungsweise Subscriber und dann an den DomainParticipant weiter (ListenerUp); der Listener der Entität bekommt ihn zuerst. TopicImpl reicht INCONSISTENT_TOPIC an den DomainParticipant. DomainParticipantListener erweitert PublisherListener und SubscriberListener und trägt onInconsistentTopic. Tests: ListenerPropagationTest::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant, PublisherOperationsTest::javapsm_2_a_writer_without_listener_and_a_publisher_without_one_pass_it_to_the_participant_listener, TopicOperationsTest::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener, ParticipantEntityTest::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant. (Commit `f31b6fa11b5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: Topic-Klassen

### §2 TopicDescription: get_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_carries_its_expression`

**Status:** `done` — TopicDescription.getParent liefert den DomainParticipant: TopicImpl hält ihn, ContentFilteredTopicImpl fragt das zugehörige Topic, MultiTopicImpl hält ihn. Tests: TopicOperationsTest::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant (Topic und ContentFilteredTopic, assertSame), MultiTopicTest::javapsm_2_the_multi_topic_carries_its_expression (MultiTopic). (Commit `819bfcb46cb`)
### §2 TopicDescription: get_type_name

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "readonly: type_name string; get_type_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getTypeName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_topic_description_finds_only_what_was_created`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_carries_its_expression`

**Status:** `done` — Topic.getTypeName liefert den Namen des TypeSupport, ContentFilteredTopicImpl.getTypeName den des zugehörigen Topics. Tests: TopicOperationsTest::javapsm_2_a_topic_and_a_content_filtered_topic_know_their_name_type_and_participant (Topic und ContentFilteredTopic gleich), DomainParticipantOperationsTest::javapsm_2_lookup_topic_description_finds_only_what_was_created („java::lang::String“), DynamicTopicTest (DynamicType „DynReading“), MultiTopicTest::javapsm_2_the_multi_topic_carries_its_expression. (Commit `819bfcb46cb`)
### §2 TopicDescription: get_name

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.1, S. 49–50 (`dds-1.2.pdf`), Tabelle
TopicDescription — "readonly: name string; get_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_topic_description_finds_only_what_was_created`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_carries_its_expression`

**Status:** `done` — Die drei Implementierungen geben den Namen aus createTopic, createContentFilteredTopic und createMultiTopic zurück. Die Tests erwarten "Chatter" (Topic), "Hot" (ContentFilteredTopic) und "Hot" (MultiTopic). (Commit `9267b77a38a`)
### §2 Topic: get_inconsistent_topic_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.2, S. 50 (`dds-1.2.pdf`), Tabelle Topic
— "get_inconsistent_topic_status (out: status: InconsistentTopicStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.checkTopicConsistency`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.remoteEndpointSeen`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_topics_of_the_same_type_or_in_another_domain_are_consistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent`

**Status:** `done` — DomainParticipantImpl.checkTopicConsistency erkennt beim Erzeugen eines Topics ein Topic gleichen Namens mit anderem Typnamen bei einem anderen Participant der Domain im Prozess, DomainParticipantImpl.remoteEndpointSeen bei einem Endpunkt aus SEDP; beide Seiten rufen TopicImpl.onInconsistentTopic. getInconsistentTopicStatus liefert total_count und total_count_change und setzt die Änderung zurück. Tests: TopicOperationsTest::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener (total_count 1 auf beiden Topics, Änderung zurückgesetzt), ::javapsm_2_topics_of_the_same_type_or_in_another_domain_are_consistent (kein Zähler bei gleichem Typ, anderer Domain, anderem Namen), ParticipantWireDiscoveryTest::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent (SEDP). (Commit `cd8f3d67ea9`)
### §2 ContentFilteredTopic: filter_expression

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51–52 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "readonly: filter_expression string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getFilterExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createContentFilteredTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`

**Status:** `done` — createContentFilteredTopic übersetzt denselben Ausdruck mit ContentFilter.compile in das Prädikat und gibt ihn über getFilterExpression zurück. Der Test erwartet "value > 50" und prüft, dass ein Reader auf dem ContentFilteredTopic nur das Sample mit value 99.0 erhält. Der öffentliche Konstruktor von ContentFilteredTopicImpl nimmt Text und Prädikat getrennt (ContentFilterTest::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples); die Übereinstimmung gilt für den Weg über den Participant. (Commit `2e0b194caf5`)
### §2 ContentFilteredTopic: get_related_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "get_related_topic: Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.getRelatedTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`


**Status:** `done` — getRelatedTopic liefert das Topic aus createContentFilteredTopic; SubscriberImpl.createDataReader abonniert über dieses Topic. Der erste Test erwartet assertEquals(topic, hot.getRelatedTopic()) und dass der Reader nur das passende Sample von einem Writer auf dem zugehörigen Topic erhält; der zweite prüft das Verwerfen nicht passender Samples. (Commit `2e0b194caf5`)
### §2 ContentFilteredTopic: get_expression_parameters und set_expression_parameters

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.3, S. 51–52 (`dds-1.2.pdf`), Tabelle
ContentFilteredTopic — "get_expression_parameters (out: expression_parameters:
string []): ReturnCode_t; set_expression_parameters (expression_parameters:
string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic.getExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.setExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ContentFilteredTopicImpl.java` (`ContentFilteredTopicImpl.evaluate`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_set_expression_parameters_filters_with_the_new_parameters_from_then_on`

**Status:** `done` — ContentFilteredTopicImpl.setExpressionParameters übersetzt das Prädikat mit ContentFilter.compile(filterExpression, neueParameter) neu; evaluate und damit die Reader auf dem ContentFilteredTopic nutzen es ab dem nächsten Sample. getExpressionParameters liefert die Liste. Test: TopicOperationsTest::javapsm_2_set_expression_parameters_filters_with_the_new_parameters_from_then_on (Ausdruck „level > %0“, Parameter 10 dann 3, erwartete Samples 15 beziehungsweise 5; ein leerer Parametersatz wird abgelehnt und der alte bleibt). (Commit `36e8fd3f496`)
### §2 MultiTopic: subscription_expression und Parameter

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.4, S. 52–53 (`dds-1.2.pdf`), Tabelle
MultiTopic — "readonly: subscription_expression string;
get_expression_parameters (out: expression_parameters: string []): ReturnCode_t;
set_expression_parameters (expression_parameters: string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getSubscriptionExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.getExpressionParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/MultiTopicImpl.java` (`MultiTopicImpl.setExpressionParameters`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_carries_its_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_expression_parameters_are_replaced`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_2_the_where_clause_uses_the_expression_parameters`

**Status:** `done` — MultiTopic ist heute vorhanden (org.omg.dds.topic.MultiTopic, MultiTopicImpl, DomainParticipant.createMultiTopic). Die Tests prüfen den Ausdruck, die Parameterliste vor und nach setExpressionParameters und dass ein neuer Parameter ("1000" auf "2000") die WHERE-Klausel eines Readers ändert. (Commit `fd437c122cf`)
### §2 TopicListener: on_inconsistent_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.5, S. 53 (`dds-1.2.pdf`), Tabelle
TopicListener — "on_inconsistent_topic (the_topic: Topic, status:
InconsistentTopicStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicListener.java` (`TopicListener.onInconsistentTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicAdapter.java` (`TopicAdapter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener`

**Status:** `done` — TopicImpl.onInconsistentTopic zählt, löst den Status aus und ruft den TopicListener mit einem InconsistentTopicEventImpl außerhalb der Topic-Sperre: den eigenen Listener, wenn er den Status in der Maske hat, sonst den DomainParticipantListener (ListenerUp.forTopic). Tests: TopicOperationsTest::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener (TopicAdapter, Quelle und Status des Events), ::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener. (Commit `cd8f3d67ea9`)
### §2 TypeSupport: register_type und get_type_name

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.3.6, S. 53–54 (`dds-1.2.pdf`), Tabelle
TypeSupport — "register_type (participant: DomainParticipant, type_name:
string): ReturnCode_t; get_type_name: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/RegisteredNameTypeSupport.java` (`RegisteredNameTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class`

**Status:** `done` — ServiceEnvironmentImpl.ZeroSPI.newTypeSupport legt den Codec unter dem registrierten Namen an (RegisteredNameTypeSupport.named); Topic.getTypeName, Writer, Reader, InProcessBus und SEDP verwenden damit einen Namen. Tests: TopicOperationsTest::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name (Writer und Reader zweier Participants paaren sich unter „my::Named“ und tauschen ein Sample aus, die DCPSPublication-Daten tragen „my::Named“, ein Topic gleichen Namens aus der Klasse hat „java::lang::String“ und paart nicht), ::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class. (Commit `36e8fd3f496`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: Publisher

### §2 Publisher: create_datawriter und delete_datawriter

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "create_datawriter (a_topic: Topic, qos: QosPolicy [], a_listener:
DataWriterListener, mask: StatusKind []): DataWriter; delete_datawriter
(a_datawriter: DataWriter): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_listener_set_at_creation_sees_the_match_of_enable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_close_contained_entities_closes_the_writers_and_withdraws_them_from_the_builtin_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`

**Status:** `done` — createDataWriter hat die Formen Topic, Topic mit DataWriterQos und Topic mit DataWriterQos, DataWriterListener und Statusmenge; der Listener wird vor enable gesetzt, sodass ein vorhandener Reader als onPublicationMatched gemeldet wird. DataWriterImpl.close nimmt den Writer aus PublisherImpl.writers und aus DCPSPublication (Close-Hook). createDataWriter lehnt ein inkonsistentes DataWriterQos mit InconsistentPolicyException ab. Tests: ListenerPropagationTest::javapsm_2_a_writer_listener_set_at_creation_sees_the_match_of_enable, QosDefaultQosTest::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays, PublisherOperationsTest::javapsm_2_close_contained_entities_closes_the_writers_and_withdraws_them_from_the_builtin_topic, CloseCleanupTest::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more, PubSubLoopbackTest::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order, BuiltinTopicQosTest::javapsm_2_the_matched_entities_are_described_with_their_qos. (Commit `f31b6fa11b5`)
### §2 Publisher: lookup_datawriter

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "lookup_datawriter (topic_name: string): DataWriter"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.lookupDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.lookupDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_lookup_data_writer_finds_by_name_and_by_topic_and_forgets_closed_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more`

**Status:** `done` — PublisherImpl.lookupDataWriter(String) und (Topic) suchen in der Liste der erzeugten Writer nach dem Topicnamen und liefern null, wenn keiner passt; ein geschlossener Writer ist aus der Liste genommen (Close-Hook). Test: PublisherOperationsTest::javapsm_2_lookup_data_writer_finds_by_name_and_by_topic_and_forgets_closed_writers (Fund über Name und Topic, null für unbekannten Namen und anderes Topic, null nach close); CloseCleanupTest::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more. (Commit `28e25c198d1`)
### §2 Publisher: suspend_publications und resume_publications

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "suspend_publications: ReturnCode_t; resume_publications:
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.resumePublications`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_resume_publications_needs_a_suspend_publications_and_the_calls_nest`

**Status:** `done` — PublisherImpl.suspendPublications zählt (die Aufrufe verschachteln), resumePublications ohne passendes suspendPublications wirft PreconditionNotMetException (DDS 1.4 §2.2.2.4.1.9, PSM Table 7.1); suspend ist ein Hinweis, die Writer liefern unverändert. Tests: PublisherOperationsTest::javapsm_2_resume_publications_needs_a_suspend_publications_and_the_calls_nest (Paar, Verschachtelung, Ausnahme, AlreadyClosedException nach close). (Commit `cd8f3d67ea9`)
### §2 Publisher: begin_coherent_changes und end_coherent_changes

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_end_coherent_changes_needs_a_begin_and_the_calls_nest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_coherent_set_is_delivered_together_when_the_presentation_asks_for_coherent_access`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_without_coherent_access_the_samples_of_a_set_are_delivered_at_once`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoherentSetAtomicityTest.java::javapsm_2_a_reader_on_another_thread_never_sees_part_of_a_set`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::rtps_9_6_4_2_the_coherent_set_parameter_is_a_sequence_number_in_the_byte_order_of_the_submessage`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::rtps_8_7_6_the_end_of_a_coherent_set_is_a_data_without_serialized_data_and_without_inline_qos`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::rtps_8_7_6_the_samples_of_a_coherent_set_are_held_until_the_first_sample_outside_it`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/CoherentSetWireTest.java::rtps_8_7_6_a_coherent_set_with_a_missing_sample_is_not_released`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_7_3_5_1_a_coherent_set_of_a_remote_writer_reaches_a_coherent_access_reader_as_a_whole`

**Status:** `done` — beginCoherentChanges/endCoherentChanges nesten (`PublisherImpl.coherentDepth`), endCoherentChanges ohne begin wirft PreconditionNotMetException. Hat die PRESENTATION des Publishers coherent_access, hält der Publisher die Zustellungen seiner Writer zurück (`PublisherImpl.deferCoherent`) und liefert sie beim letzten end in Schreibreihenfolge als einen Schritt (`InProcessBus.atomically`): jedes read und take eines Readers hält die Gegenseite desselben Tores, sodass ein Leser in einem anderen Thread alle Samples eines Satzes sieht oder keines (`CoherentSetAtomicityTest`; ohne das Tor schlägt er fehl). Über RTPS tragen die Samples eines Satzes `PID_COHERENT_SET` (0x0056) mit der Sequenznummer des ersten Samples des Writers im Satz (DDSI-RTPS 2.5 §8.7.6, §9.6.4.2; Oktette gegen `coherent_set_bytes` der Rust-Implementierung), das Ende ist ein DATA ohne Daten und ohne Inline-QoS (Tabelle 9.22, Beispiel 3). Ein Reader eines Subscribers mit coherent_access hält die Samples eines entfernten Satzes, bis er vollständig ist (`CoherentHold`), und gibt sie zusammen weiter; ein unvollständiger Satz (ein Sample fehlt) wird nicht zugestellt und im Log genannt (§8.7.6). Erwartung gegenüber Cyclone DDS, Fast DDS und OpenDDS: sie lesen PID 0x0056 und verhalten sich nach §8.7.6; ein DATA ohne Daten am Ende eines Satzes ist nach Tabelle 9.22 vorgesehen. Gemessen ist nur die Gegenstelle in ZeroDDS. Der Gruppen-Satz (access_scope GROUP, PID_GROUP_COHERENT_SET, End-Coherent-Set-DATA aller Writer des Publishers) ist nicht gebaut; er steht im offenen Item zu PRESENTATION. (Commit `178c642cbc7`)
### §2 Publisher: wait_for_acknowledgments

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/ReliableWriterState.java` (`ReliableWriterState.awaitAcknowledged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitAcknowledgments`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_wait_for_acknowledgments_returns_at_once_for_readers_of_the_process`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_2_a_sample_the_remote_reader_acknowledged_ends_the_wait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_2_a_sample_nobody_acknowledges_is_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_wait_for_acknowledgments_times_out_when_nobody_acknowledged`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_an_acknack_beyond_the_sample_acknowledges_it`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_a_waiter_is_released_by_the_acknack_of_another_thread`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_every_reader_that_received_the_sample_has_to_acknowledge_it`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_a_reader_that_leaves_is_not_waited_for`

**Status:** `done` — PublisherImpl.waitForAcknowledgments wartet nacheinander auf die aktivierten Writer des Publishers mit einer gemeinsamen Frist; DataWriterImpl.awaitAcknowledgments wartet bei einem Writer mit RTPS-Anbindung über RtpsParticipant.WireWriter.waitForAcknowledgments auf ReliableWriterState.awaitAcknowledged (die Historie ist leer, wenn alle Reader per ACKNACK bestätigt haben) und wirft TimeoutException, wenn die Frist abläuft. Reader im selben Prozess haben das Sample bei der Rückkehr von write (synchrone Zustellung). Ein negatives maxWait ist IllegalArgumentException, ein geschlossener Publisher AlreadyClosedException. Tests: PublisherOperationsTest::javapsm_2_wait_for_acknowledgments_returns_at_once_for_readers_of_the_process, WriterWireAckTest::javapsm_2_a_sample_the_remote_reader_acknowledged_ends_the_wait und ::javapsm_2_a_sample_nobody_acknowledges_is_a_timeout_exception (Publisher- und Writer-Form), WriterAckStateTest. (Commit `cd8f3d67ea9`)
### §2 Publisher: get_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — PublisherImpl.getParent liefert den DomainParticipant, den DomainParticipantImpl.createPublisher beim Erzeugen übergibt; DataWriterImpl.getParent den Publisher. Test: QosEntityBasicsTest::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it (assertSame). (Commit `2e0b194caf5`)
### §2 Publisher: delete_contained_entities

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57 (`dds-1.2.pdf`), Tabelle
Publisher — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_close_contained_entities_closes_the_writers_and_withdraws_them_from_the_builtin_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_close_contained_entities_empties_publisher_and_subscriber`

**Status:** `done` — PublisherImpl.closeContainedEntities schließt jeden Writer; dessen Close-Hook nimmt ihn aus PublisherImpl.writers und zieht ihn aus DCPSPublication zurück. Der Publisher bleibt offen und nimmt neue Writer an. Tests: PublisherOperationsTest::javapsm_2_close_contained_entities_closes_the_writers_and_withdraws_them_from_the_builtin_topic (Writer geschlossen, lookupDataWriter null, getDataWriters leer, containsEntity false, der eingebaute Reader eines anderen Participants sieht NOT_ALIVE_NO_WRITERS), CloseCleanupTest::javapsm_2_close_contained_entities_empties_publisher_and_subscriber. (Commit `28e25c198d1`)
### §2 Publisher: set_default_datawriter_qos und get_default_datawriter_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 57–58 (`dds-1.2.pdf`), Tabelle
Publisher — "set_default_datawriter_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datawriter_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.setDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.setDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_data_writer_qos_of_the_publisher_is_the_qos_of_the_writers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays`

**Status:** `done` — PublisherImpl.setDefaultDataWriterQos prüft die Konsistenz (QosRules.checkConsistent) und wirft InconsistentPolicyException, ohne das Default zu ändern; createDataWriter(Topic) verwendet das gesetzte Default, ein dem Aufruf übergebenes QoS hat Vorrang, und createDataWriter prüft das QoS ebenfalls. Tests: QosDefaultQosTest::javapsm_7_3_5_4_the_default_data_writer_qos_of_the_publisher_is_the_qos_of_the_writers_it_creates_next, QosDefaultQosTest::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays. (Commit `cd8f3d67ea9`)
### §2 Publisher: copy_from_topic_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.1, S. 58 (`dds-1.2.pdf`), Tabelle
Publisher — "copy_from_topic_qos (inout: a_datawriter_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.copyFromTopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.copyFromTopicQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_copy_from_topic_qos_lets_the_policies_of_the_topic_replace_those_of_the_writer`

**Status:** `done` — PublisherImpl.copyFromTopicQos übernimmt jede Policy des TopicQos, die ein DataWriter kennt, in das DataWriterQos und ersetzt dabei den Wert des Writer-QoS (DDS 1.4 §2.2.2.4.1.17); Policies, die das TopicQos nicht hat (USER_DATA, OWNERSHIP_STRENGTH), bleiben die des Writer-QoS, das übergebene QoS bleibt unverändert, und das Ergebnis wird nicht auf Konsistenz geprüft. Test: PublisherOperationsTest::javapsm_2_copy_from_topic_qos_lets_the_policies_of_the_topic_replace_those_of_the_writer (DEADLINE 5 s und BEST_EFFORT des Writers werden zu DEADLINE 7 s und RELIABLE des Topics, bei einem Topic mit Default-Werten zu unendlich und BEST_EFFORT). (Commit `cd8f3d67ea9`)
### §2 PublisherListener

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.3, S. 71–72 (`dds-1.2.pdf`) —
"PublisherListener: no attributes, no operations (inherits the operations of
DataWriterListener)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_passes_offered_incompatible_qos_to_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_that_a_writer_listener_takes_is_not_passed_up`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant`

**Status:** `done` — DataWriterImpl.listenerFor reicht einen Status, den der Listener des Writers nicht nimmt, an den PublisherListener des PublisherImpl (ListenerUp.forWriter) und dann an den des DomainParticipant. Tests: PublisherOperationsTest::javapsm_2_a_writer_without_listener_passes_offered_incompatible_qos_to_the_publisher (Quelle des Events der Writer, Status mit total_count 1 und Policy Reliability), ::javapsm_2_a_writer_that_a_writer_listener_takes_is_not_passed_up, ListenerPropagationTest::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant. (Commit `f31b6fa11b5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DataWriter

### §2 DataWriter: register_instance und register_instance_w_timestamp

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "register_instance (instance: Data): InstanceHandle_t;
register_instance_w_timestamp (instance: Data, timestamp: Time_t):
InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.registerInstanceAt`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key`


**Status:** `done` — registerInstance(T) liefert einen von InstanceKey.toHandle gebildeten Handle; die Formen mit Time und mit long und TimeUnit verwenden den Zeitstempel für die Liveliness-Zeit (DataWriterImpl.registerInstanceAt). Das Registrieren sendet nichts an die Reader (DDS 1.4 §2.2.2.4.2.5 verlangt es nicht). Test: WriterHandleTest::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key (alle drei Formen, derselbe Handle bei erneutem Registrieren). (Commit `56f40fabd6c`)
### §2 DataWriter: unregister_instance und unregister_instance_w_timestamp

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "unregister_instance (instance: Data, handle: InstanceHandle_t):
ReturnCode_t; unregister_instance_w_timestamp (instance: Data, handle:
InstanceHandle_t, timestamp: Time_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterAtHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onMessage`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_unregister_by_handle_or_by_sample_makes_the_reader_see_no_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_unregister_only_leaves_no_writers_when_the_flag_is_false`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_2_closing_one_writer_does_not_mark_no_writers_while_another_writes`

**Status:** `done` — unregisterInstance(InstanceHandle, ...) löst die Instanz über DataWriterImpl.resolve aus dem Handle oder, bei HANDLE_NIL, aus dem Sample auf und verwendet den Schlüssel der Registrierung, den der Reader sucht. Ein Handle einer anderen Instanz als der des Samples ist PreconditionNotMetException, ein Handle keiner Instanz des Writers (auch ein bereits abgemeldeter) IllegalArgumentException (BAD_PARAMETER). Tests: WriterHandleTest::javapsm_2_unregister_by_handle_or_by_sample_makes_the_reader_see_no_writers (Reader sieht NOT_ALIVE_NO_WRITERS für beide Instanzen), InstanceHandleTest::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered, QosWriterDataLifecycleTest::javapsm_7_3_5_1_unregister_only_leaves_no_writers_when_the_flag_is_false, ::javapsm_2_closing_one_writer_does_not_mark_no_writers_while_another_writes. (Commit `d70d1f69c5d`)
### §2 DataWriter: get_key_value

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getKeyValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_writer_fills_the_key_fields_of_the_holder_only`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_type_without_a_key_has_no_key_value`

**Status:** `done` — getKeyValue(T, InstanceHandle) und getKeyValue(InstanceHandle) liefern eine Kopie des letzten für die Instanz geschriebenen Samples, sodass die Schlüsselfelder gesetzt sind; ein Typ ohne Schlüssel ist PreconditionNotMetException, ein Handle keiner Instanz des Writers IllegalArgumentException. Tests: CoreKeyValueTest::javapsm_7_2_5_a_writer_fills_the_key_fields_of_the_holder_only, CoreKeyValueTest::javapsm_7_2_5_a_type_without_a_key_has_no_key_value. (Commit `56f40fabd6c`)
### §2 DataWriter: lookup_instance

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.lookupInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.lookupInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key`


**Status:** `done` — lookupInstance(T) liefert den Handle der registrierten oder geschriebenen Instanz und InstanceHandle.NIL für eine unbekannte; die Form mit ModifiableInstanceHandle füllt und liefert den übergebenen Handle. Test: WriterHandleTest::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key (Schlüssel bestimmt die Instanz, Form mit ModifiableInstanceHandle, unbekannter Schlüssel liefert den Nil-Handle). (Commit `56f40fabd6c`)
### §2 DataWriter: write und write_w_timestamp

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 62 (`dds-1.2.pdf`), Tabelle
DataWriter — "write (data: Data, handle: InstanceHandle_t): ReturnCode_t;
write_w_timestamp (data: Data, handle: InstanceHandle_t, source_timestamp:
Time_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.writeAt`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_7_6_2_every_write_overload_carries_its_source_timestamp_to_the_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_7_6_2_an_unkeyed_type_is_written_with_the_nil_handle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_of_4096_octets_arrives_whole`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_write_on_a_closed_writer_throws_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_not_enabled_writer_refuses_to_write`

**Status:** `done` — DataWriter hat sechs write-Überladungen (ohne, mit Time, mit long und TimeUnit, je mit und ohne InstanceHandle); der Zeitstempel ist der Source-Zeitstempel des Samples beim Reader. Tests: WriterHandleTest::javapsm_7_6_2_every_write_overload_carries_its_source_timestamp_to_the_sample (fünf Formen, Zeitstempel und Instanz-Handle am Sample), ::javapsm_7_6_2_an_unkeyed_type_is_written_with_the_nil_handle, PubSubLoopbackTest::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order, QuickstartApiTest::javapsm_7_8_2_a_byte_array_of_4096_octets_arrives_whole, ExceptionMappingTest::javapsm_7_3_2_write_on_a_closed_writer_throws_already_closed, ::javapsm_7_3_2_a_not_enabled_writer_refuses_to_write. (Commit `56f40fabd6c`)
### §2 DataWriter: write mit Handle HANDLE_NIL

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2.11, S. 67 (`dds-1.2.pdf`) — "The
special value HANDLE_NIL can be used for the parameter handle. This indicates
that the identity of the instance should be automatically deduced from the
instance_data (by means of the key)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.keyOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_7_6_2_an_unkeyed_type_is_written_with_the_nil_handle`

**Status:** `done` — write(T, InstanceHandle, ...) prüft den Handle (DataWriterImpl.checkHandle): HANDLE_NIL ist zulässig, der Handle der Instanz des Samples ebenfalls, der Handle einer anderen Instanz des Writers ist PreconditionNotMetException, ein Handle keiner Instanz des Writers IllegalArgumentException (BAD_PARAMETER, §2.2.2.4.2.11). Test: WriterHandleTest::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed prüft DataWriterImpl.checkHandle über dispose (HANDLE_NIL mit Sample, registrierter Handle, fremder registrierter Handle PreconditionNotMetException, unbekannter Handle IllegalArgumentException); write mit Handle prüft kein eigener Test. (Commit `d70d1f69c5d`)
### §2 DataWriter: dispose und dispose_w_timestamp

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "dispose (data: Data, handle: InstanceHandle_t): ReturnCode_t;
dispose_w_timestamp (data: Data, handle: InstanceHandle_t, timestamp: Time_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.disposeAtHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_a_write_after_the_dispose_brings_the_instance_back_to_alive`

**Status:** `done` — dispose(InstanceHandle, ...) und dispose(T, ...) setzen die Instanz beim Reader auf NOT_ALIVE_DISPOSED; die Handle-Formen lösen über DataWriterImpl.resolve auf (Handle der Registrierung oder HANDLE_NIL mit Sample) und treffen die Instanz, die der Reader über den Schlüssel findet. Tests: WriterHandleTest::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed (Formen mit Time und mit long und TimeUnit), InstanceHandleTest::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered, QosWriterDataLifecycleTest::javapsm_7_3_5_1_a_write_after_the_dispose_brings_the_instance_back_to_alive. (Commit `d70d1f69c5d`)
### §2 DataWriter: dispose mit Handle

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2.13, S. 69 (`dds-1.2.pdf`) — "The
constraints on the values of the handle parameter and the corresponding error
behavior are the same specified for the unregister_instance operation (Section
7.1.2.4.2.7)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.dispose`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.disposeAtHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`

**Status:** `done` — dispose(InstanceHandle, T, ...) meldet PreconditionNotMetException für den Handle einer anderen Instanz als der des Samples und IllegalArgumentException (BAD_PARAMETER) für einen Handle, den der Writer nicht kennt (DataWriterImpl.resolve); dispose(InstanceHandle, T) verwendet den Handle. Tests: WriterHandleTest::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed, InstanceHandleTest::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered (kein Handle und kein Sample: IllegalArgumentException). (Commit `d70d1f69c5d`)
### §2 DataWriter: wait_for_acknowledgments

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "wait_for_acknowledgments (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.waitForAcknowledgments`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.waitForAcknowledgments`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::javapsm_2_a_writer_whose_readers_are_in_the_process_is_acknowledged_when_write_returns`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::javapsm_2_a_writer_without_readers_has_nothing_outstanding`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::javapsm_2_an_argument_that_is_not_a_duration_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWaitAckTest.java::javapsm_2_a_closed_writer_and_a_writer_that_is_not_enabled_refuse_the_wait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_2_a_sample_the_remote_reader_acknowledged_ends_the_wait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_2_a_sample_nobody_acknowledges_is_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_wait_for_acknowledgments_times_out_when_nobody_acknowledged`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_an_acknack_beyond_the_sample_acknowledges_it`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_a_waiter_is_released_by_the_acknack_of_another_thread`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_every_reader_that_received_the_sample_has_to_acknowledge_it`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterAckStateTest.java::javapsm_2_a_reader_that_leaves_is_not_waited_for`

**Status:** `done` — DataWriterImpl.waitForAcknowledgments(Duration) und (long, TimeUnit) rufen awaitAcknowledgments: für Reader im selben Prozess kehrt der Aufruf sofort zurück, für Reader über RTPS wartet er auf die Bestätigungen aus ReliableWriterState und wirft TimeoutException, wenn maxWait abläuft; null und negative Frist sind IllegalArgumentException, ein geschlossener Writer AlreadyClosedException, ein nicht aktivierter NotEnabledException. Tests: WriterWaitAckTest (vier Fälle), WriterWireAckTest (Bestätigung und Timeout über RTPS), WriterAckStateTest (Historie, ACKNACK, Peer-Verlust). (Commit `cd8f3d67ea9`)
### §2 DataWriter: get_liveliness_lost_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_liveliness_lost_status (out: status: LivelinessLostStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_writer_that_does_not_assert_its_liveliness_loses_it_once_and_asserting_restores_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_lost_fires_without_polling`

**Status:** `done` — Die Lease wird überwacht (LeaseMonitor, DataWriterImpl.evaluateLiveliness): ein Writer mit MANUAL_BY_TOPIC und endlicher Lease, der sie nicht erneuert, zählt LIVELINESS_LOST einmal und ist danach nicht mehr lebendig; assertLiveliness macht ihn wieder lebendig. getLivelinessLostStatus liefert total_count und total_count_change; die Änderung wird beim Rufen des Listeners oder beim Lesen zurückgesetzt. Tests: WriterStatusTest::javapsm_2_a_writer_that_does_not_assert_its_liveliness_loses_it_once_and_asserting_restores_it (Listener-Aufruf, Status gelesen, kein weiterer Zähler solange nicht lebendig, nach assertLiveliness erneuter Verlust), LeaseTest::javapsm_2_liveliness_lost_fires_without_polling. (Commit `d23eff980f3`)
### §2 DataWriter: get_offered_deadline_missed_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_offered_deadline_missed_status (out: status:
OfferedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_missed_offered_deadline_is_counted_with_the_instance_that_missed_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_offered_deadline_missed_fires_without_polling`

**Status:** `done` — Der Writer prüft DEADLINE je Instanz (evaluateOfferedDeadline, Lease-Timer); getOfferedDeadlineMissedStatus liefert total_count, total_count_change und last_instance_handle. Test: WriterStatusTest::javapsm_2_a_missed_offered_deadline_is_counted_with_the_instance_that_missed_it (Deadline 30 ms, ein Write; total_count >= 1, bei der ersten Abfrage total_count_change = total_count, last_instance_handle = lookupInstance, bei der zweiten Abfrage Änderung = Differenz), LeaseTest::javapsm_2_offered_deadline_missed_fires_without_polling. (Commit `d23eff980f3`)
### §2 DataWriter: get_offered_incompatible_qos_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_offered_incompatible_qos_status (out: status:
OfferedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getOfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_an_incompatible_reader_is_counted_with_the_refused_policy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::javapsm_7_3_5_1_a_reliable_reader_and_a_best_effort_writer_do_not_connect_and_both_record_it`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged vergleicht die vom Reader angefragte mit der angebotenen QoS (QosCompatibility), paart bei Unverträglichkeit nicht und zählt OFFERED_INCOMPATIBLE_QOS mit der ersten verweigernden Policy und einer Zählung je Policy. Tests: WriterStatusTest::javapsm_2_an_incompatible_reader_is_counted_with_the_refused_policy (total_count 1, last_policy_class Reliability, genau ein QosPolicyCount für Reliability mit Zähler 1, kein Match, Listener-Status), QosCompatibilityTest::javapsm_7_3_5_1_a_reliable_reader_and_a_best_effort_writer_do_not_connect_and_both_record_it. (Commit `d23eff980f3`)
### §2 DataWriter: get_publication_matched_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_publication_matched_status (out: status:
PublicationMatchedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getPublicationMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getPublicationMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onWireReaderMatch`), `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.MatchListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_the_publication_matched_status_counts_and_names_the_last_subscription`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_status_is_readable_without_a_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`,
`tests/integration/tests/bindings_ffi.rs::dcps_2_2_4_1_publication_matched_counts_a_reader_of_another_process`

**Status:** `done` — DataWriterImpl führt total_count, current_count und deren Änderungswerte; die Änderungswerte werden beim Lesen oder beim Rufen des Listeners zurückgesetzt. Test: WriterStatusTest::javapsm_2_the_publication_matched_status_counts_and_names_the_last_subscription (erster Reader: 1/1/1/1; erneutes Lesen: Änderungen 0; zweiter Reader gekommen und gegangen: total_count 2, Änderung 1, current_count 1, Änderung 0; letzter Reader gegangen: current_count 0, Änderung -1, total_count_change 0), ListenerTest::javapsm_2_status_is_readable_without_a_listener, ::javapsm_2_writer_listener_fires_on_publication_matched. Ein Reader eines anderen Prozesses, über SEDP gematcht, zählt ebenso (`onWireReaderMatch`); der Test gegen einen Rust-Reader (Job `integration-java-psm`) erwartet 1/1/1/1 beim Match und current_count 0 mit Änderung -1 nach `ignoreSubscription`. (Commit `cd8f3d67ea9`)
### §2 DataWriter: last_subscription_handle von PUBLICATION_MATCHED

**Spec:** §2, S. 11; [DDS] 1.2 IDL, S. 160 (`dds-1.2.pdf`) — "struct
PublicationMatchedStatus { [...] InstanceHandle_t last_subscription_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/PublicationMatchedStatus.java` (`PublicationMatchedStatus.getLastSubscriptionHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.readPublicationMatchedStatusLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_the_publication_matched_status_counts_and_names_the_last_subscription`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged merkt den Handle des Readers, der den Status zuletzt änderte (Match oder Verlust), und readPublicationMatchedStatusLocked setzt ihn als last_subscription_handle ein. Test: WriterStatusTest::javapsm_2_the_publication_matched_status_counts_and_names_the_last_subscription (der Handle ist getInstanceHandle des Readers und ein Element von getMatchedSubscriptions; nach dem Verlust bleibt er der des gegangenen Readers). (Commit `cd8f3d67ea9`)
### §2 DataWriter: get_topic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_topic: Topic"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — DataWriterImpl.getTopic liefert das Topic aus dem Konstruktor; PublisherImpl.lookupDataWriter nutzt es. Test: QosEntityBasicsTest::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it (assertSame). (Commit `a2a9a67acb5`)
### §2 DataWriter: get_publisher

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_publisher: Publisher"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — DataWriterImpl.getParent liefert den Publisher, den PublisherImpl.createDataWriter an den Konstruktor übergibt. Test: QosEntityBasicsTest::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it (assertSame). (Commit `a2a9a67acb5`)
### §2 DataWriter: assert_liveliness

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "assert_liveliness: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_writer_that_does_not_assert_its_liveliness_loses_it_once_and_asserting_restores_it`


**Status:** `done` — DataWriterImpl.assertLiveliness setzt den Writer lebendig und erneuert die Lease; bei LIVELINESS MANUAL_BY_PARTICIPANT erneuert sie die Lease aller Writer dieser Art im Participant; auf einem geschlossenen Writer ist sie AlreadyClosedException. Tests: WriterStatusTest::javapsm_2_a_writer_that_does_not_assert_its_liveliness_loses_it_once_and_asserting_restores_it (ohne assertLiveliness geht die Liveliness einmal verloren, assertLiveliness stellt sie wieder her). (Commit `cd8f3d67ea9`)
### §2 DataWriter: get_matched_subscription_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63, §7.1.2.4.2.23, S. 70
(`dds-1.2.pdf`) — "get_matched_subscription_data (inout: subscription_data:
SubscriptionBuiltinTopicData, subscription_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getMatchedSubscriptionData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_null_argument_is_a_bad_parameter`

**Status:** `done` — Die Methode liefert die Builtin-Daten eines verbundenen Readers (lokal aus dem Reader, entfernt aus der SEDP-Ankündigung). Der erste Test erwartet Topicname "Chatter" und das UserData des Readers und IllegalArgumentException für einen Handle, der keine verbundene Subscription ist; der zweite erwartet IllegalArgumentException für null. Der Zweig für entfernte Reader hat in diesen Tests keinen Aufruf. (Commit `a2a9a67acb5`)
### §2 DataWriter: get_matched_subscriptions

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.2, S. 63 (`dds-1.2.pdf`), Tabelle
DataWriter — "get_matched_subscriptions (inout: subscription_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.getMatchedSubscriptions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_closed_reader_is_no_subscription_of_the_writer_any_more`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_2_closing_the_reader_ends_the_join`

**Status:** `done` — Die Methode liefert die Handles der lokalen verbundenen Reader und der Reader über die RTPS-Strecke. Die Tests erwarten Größe 1 mit einem Handle, den getMatchedSubscriptionData auflöst, Größe 0 nach dem Schließen des Readers und Größe 1 je Writer für die Reader eines MultiTopic-Joins. Der Zweig für entfernte Reader hat in diesen Tests keinen Aufruf. (Commit `a2a9a67acb5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DataWriterListener

### §2 DataWriterListener: on_liveliness_lost

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_liveliness_lost (the_writer: DataWriter, status:
LivelinessLostStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onLivelinessLost`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_lost_fires_without_polling`

**Status:** `done` — DataWriterListener deklariert onLivelinessLost(LivelinessLostEvent<TYPE>); evaluateLiveliness ruft es außerhalb des Writer-Monitors, wenn die Lease eines MANUAL_BY_TOPIC-Writers abläuft. Der Test schreibt einmal, bestätigt nicht mehr und erwartet innerhalb von 5 s den Callback mit total_count >= 1 ohne Abfrage des Status. (Commit `a2a9a67acb5`)
### §2 DataWriterListener: on_offered_deadline_missed

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_offered_deadline_missed (the_writer: DataWriter,
status: OfferedDeadlineMissedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onOfferedDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_offered_deadline_missed_fires_without_polling`

**Status:** `done` — DataWriterListener deklariert onOfferedDeadlineMissed(OfferedDeadlineMissedEvent<TYPE>); evaluateOfferedDeadline ruft es außerhalb des Writer-Monitors. Der Test schreibt einmal bei 30 ms Deadline und erwartet innerhalb von 5 s den Callback mit total_count >= 1 ohne Abfrage des Status. (Commit `a2a9a67acb5`)
### §2 DataWriterListener: on_offered_incompatible_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_offered_incompatible_qos (the_writer: DataWriter,
status: OfferedIncompatibleQosStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onOfferedIncompatibleQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_an_incompatible_reader_is_counted_with_the_refused_policy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_that_a_writer_listener_takes_is_not_passed_up`

**Status:** `done` — DataWriterImpl.onReaderPresenceChanged ruft onOfferedIncompatibleQos des Listeners mit dem Status, wenn die vom Reader angefragte QoS unverträglich ist. Tests: WriterStatusTest::javapsm_2_an_incompatible_reader_is_counted_with_the_refused_policy (DataWriterAdapter überschreibt onOfferedIncompatibleQos), PublisherOperationsTest::javapsm_2_a_writer_that_a_writer_listener_takes_is_not_passed_up (nur der Writer-Listener wird gerufen). (Commit `d23eff980f3`)
### §2 DataWriterListener: on_publication_matched

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.4.4, S. 72 (`dds-1.2.pdf`), Tabelle
DataWriterListener — "on_publication_matched (the_writer: DataWriter, status:
PublicationMatchedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener.onPublicationMatched`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onReaderPresenceChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.onWireReaderMatch`), `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.MatchListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`,
`tests/integration/tests/bindings_ffi.rs::dcps_2_2_4_1_publication_matched_counts_a_reader_of_another_process`

**Status:** `done` — DataWriterListener deklariert onPublicationMatched(PublicationMatchedEvent<TYPE>); onReaderPresenceChanged ruft ihn beim Erscheinen und Verschwinden eines Readers. Der Test erwartet einen Aufruf je Ereignis (1 nach dem Reader, 2 nach dessen close), current_count 1 und danach 0, total_count 1 und als Quelle des Events den Writer selbst. Für einen Reader eines anderen Prozesses ruft `onWireReaderMatch` den Listener ebenso, auf einem Thread des RTPS-Participants in der Reihenfolge der Änderungen; der Test gegen einen Rust-Reader erwartet die zwei Aufrufe mit 1/1/1/1 und 1/0/0/-1. (Commit `a2a9a67acb5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: Subscriber

### §2 Subscriber: create_datareader und delete_datareader

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80 (`dds-1.2.pdf`), Tabelle
Subscriber — "create_datareader (a_topic: TopicDescription, qos: QosPolicy [],
a_listener: DataReaderListener, mask: StatusKind []): DataReader;
delete_datareader (a_datareader: DataReader): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_a_reader_on_the_multi_topic_receives_the_matching_samples`

**Status:** `done` — createDataReader hat für jede TopicDescription die Formen ohne Argument, mit DataReaderQos und mit Listener und Statusmenge; der Listener wird vor enable gesetzt, sodass Match und Replay aus enable ihn erreichen. DataReaderImpl.close nimmt den Reader aus SubscriberImpl.readers und zieht ihn aus DCPSSubscription zurück (Close-Hook, SubscriberImpl.readerGone). Ein Reader auf einer ContentFilteredTopic oder einem MultiTopic hält die Beschreibung (SubscriberImpl.usedBy). Tests: ListenerPropagationTest::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers (Reader mit Listener, vorhandener Writer), ParticipantEntityTest::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later, CloseCleanupTest::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more, ContentFilterTest, MultiTopicTest. (Commit `f31b6fa11b5`)
### §2 Subscriber: lookup_datareader

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80 (`dds-1.2.pdf`), Tabelle
Subscriber — "lookup_datareader (topic_name: string): DataReader"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.lookupDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.lookupDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_lookup_data_reader_finds_by_name_by_topic_and_by_filtered_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCleanupTest.java::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more`

**Status:** `done` — SubscriberImpl.lookupDataReader(String) und (TopicDescription) suchen in der Liste der erzeugten Reader nach dem Namen der TopicDescription des Readers (bei einem Reader auf einer ContentFilteredTopic deren Name); ein geschlossener Reader ist aus der Liste genommen. Tests: ParticipantSubscriberTest::javapsm_2_lookup_data_reader_finds_by_name_by_topic_and_by_filtered_topic (Fund, null für unbekannten Namen und für das Topic unter der ContentFilteredTopic, null nach close), CloseCleanupTest::javapsm_2_a_closed_writer_and_reader_are_not_listed_or_found_any_more. (Commit `28e25c198d1`)
### §2 Subscriber: begin_access und end_access

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 80–81 (`dds-1.2.pdf`), Tabelle
Subscriber — "begin_access: ReturnCode_t; end_access: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.beginAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.beginAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.endAccess`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.checkGroupAccess`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_end_access_needs_a_begin_access_and_the_calls_nest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_with_access_scope_group_the_samples_are_accessed_between_begin_access_and_end_access`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_with_the_default_access_scope_no_begin_access_is_needed`

**Status:** `done` — SubscriberImpl.beginAccess hält eine ReentrantLock des Subscribers, die read und take der Reader ebenfalls nehmen; die Aufrufe verschachteln, ein Thread, der den Zugriff hält, liest ohne Warten, andere warten. endAccess ohne beginAccess ist PreconditionNotMetException (DDS 1.4 §2.2.2.5.2.9). Mit PRESENTATION access_scope GROUP lehnen read und take der Reader und getDataReaders des Subscribers außerhalb von begin_access mit PreconditionNotMetException ab (SubscriberImpl.checkGroupAccess); die Reader eines MultiTopic-Joins sind ausgenommen. Tests: ParticipantSubscriberTest::javapsm_2_end_access_needs_a_begin_access_and_the_calls_nest, ReaderSubscriberTest::javapsm_2_with_access_scope_group_the_samples_are_accessed_between_begin_access_and_end_access, ::javapsm_2_with_the_default_access_scope_no_begin_access_is_needed. (Commit `cd8f3d67ea9`)
### §2 Subscriber: get_datareaders

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "get_datareaders (out: readers: DataReader [], sample_states:
SampleStateKind [], view_states: ViewStateKind [], instance_states:
InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.getDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.hasMatchingSample`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_get_data_readers_with_a_data_state_lists_the_readers_that_hold_such_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_2_closing_the_reader_ends_the_join`

**Status:** `done` — getDataReaders(Collection) liefert die Reader des Subscribers, getDataReaders(Collection, DataState) die, die ein Sample in den gewählten Sample-, View- und Instance-Zuständen halten (DataReaderImpl.hasMatchingSample). Test: ParticipantSubscriberTest::javapsm_2_get_data_readers_with_a_data_state_lists_the_readers_that_hold_such_samples (leer vor dem Write, nur der Reader des geschriebenen Topics danach, leer nach take; beide Reader ohne DataState), MultiTopicJoinTest::javapsm_2_closing_the_reader_ends_the_join. (Commit `819bfcb46cb`)
### §2 Subscriber: notify_datareaders

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "notify_datareaders: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.notifyDataReaders`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.notifyDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_data_on_readers_goes_to_the_subscriber_listener_and_notify_data_readers_passes_it_to_the_readers_with_new_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_without_a_subscriber_listener_the_readers_listeners_are_called_and_data_on_readers_stays_changed`

**Status:** `done` — `SubscriberImpl.notifyDataReaders` ruft an jedem Reader `DataReaderImpl.notifyDataAvailable`. Das ruft `onDataAvailable` des Reader-Listeners nur, wenn der Status DATA_AVAILABLE des Reader geändert ist (seit dem Eintreffen kein read/take) und geht nicht über den Listener des Subscribers. `dataOnReadersGoesToTheSubscriberListener...` nimmt an einem von zwei Readern ein Sample; nach `notifyDataReaders` zählt der andere Reader einen Aufruf, der erste keinen. Der zweite Test prüft, dass ein Reader nach read keinen Aufruf erhält. (Commit `e1b55167947`)
### §2 Subscriber: get_participant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "get_participant: DomainParticipant"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — `SubscriberImpl.getParent` gibt den `DomainParticipant` zurück, mit dem der Subscriber erzeugt wurde. Der Test erwartet mit `assertSame` den Participant, der den Subscriber erzeugt hat. (Commit `819bfcb46cb`)
### §2 Subscriber: delete_contained_entities

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_close_contained_entities_closes_the_readers_and_withdraws_them_from_the_builtin_topic`

**Status:** `done` — `SubscriberImpl.closeContainedEntities` ruft `close()` an jedem Reader; der Close-Hook des Readers entfernt ihn aus `readers` und ruft `Discovery.readerClosed` auf. Die Operation prüft jetzt mit `ensureOpen`, ob der Subscriber offen ist. Der Test beobachtet an einem zweiten Participant der Domäne den Built-in-Topic DCPSSubscription: nach dem Aufruf sind beide Reader geschlossen, `getDataReaders` ist leer, `lookupDataReader` liefert `null`, der Subscriber ist offen und es kommen zwei Samples ohne Daten mit NOT_ALIVE_NO_WRITERS. (Commit `e1b55167947`)
### §2 Subscriber: set_default_datareader_qos und get_default_datareader_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "set_default_datareader_qos (qos_list: QosPolicy []): ReturnCode_t;
get_default_datareader_qos (out: qos_list: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.setDefaultDataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.getDefaultDataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_data_reader_qos_of_the_subscriber_is_the_qos_of_the_readers_it_creates_next`

**Status:** `done` — `getDefaultDataReaderQos` liefert vor dem Setzen die Standardwerte (HISTORY KEEP_LAST, Tiefe 1). Nach `setDefaultDataReaderQos(q)` hat der mit `createDataReader(topic)` erzeugte Reader dieses QoS (KEEP_ALL). (Commit `819bfcb46cb`)
### §2 Subscriber: copy_from_topic_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.2, S. 81 (`dds-1.2.pdf`), Tabelle
Subscriber — "copy_from_topic_qos (inout: a_datareader_qos: QosPolicy [],
a_topic_qos: QosPolicy []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.copyFromTopicQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_copy_from_topic_qos_replaces_the_policies_the_topic_and_the_reader_share`

**Status:** `done` — `SubscriberImpl.copyFromTopicQos` ersetzt jede Policy des Reader-QoS, die das Topic-QoS auch hat (`QosPolicy.ForDataReader`), durch den Wert des Topic, auch wenn der Reader-Wert vom Standard abweicht (DDS 1.4 §2.2.2.5.2.17). Policies, die nur der Reader hat (UserData), bleiben, das übergebene QoS bleibt unverändert. Der Test setzt am Reader HISTORY KEEP_LAST 5 und DURABILITY TRANSIENT_LOCAL, am Topic KEEP_ALL und VOLATILE und erwartet die Werte des Topic. (Commit `e1b55167947`)
### §2 SubscriberListener: on_data_on_readers

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.6, S. 104 (`dds-1.2.pdf`), Tabelle
SubscriberListener — "on_data_on_readers (the_subscriber: Subscriber): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl (EntityBase.getListener)`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener.onDataOnReaders`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_data_on_readers_goes_to_the_subscriber_listener_and_notify_data_readers_passes_it_to_the_readers_with_new_data`

**Status:** `done` — `DataReaderImpl.fireDataAvailable` ruft, wenn der Subscriber (oder der DomainParticipant) einen Listener mit DATA_ON_READERS hat, `onDataOnReaders` mit einem `DataOnReadersEventImpl` (Quelle: der Subscriber) und nicht `onDataAvailable` des Readers. Der Subscriber hat das Status-Flag DATA_ON_READERS (`SubscriberImpl.dataOnReadersChanged`); der Aufruf des Listeners und read/take an einem Reader des Subscribers setzen es zurück. Der Test setzt einen `SubscriberAdapter` und zwei Reader-Listener und erwartet Aufrufe nur am Subscriber-Listener. (Commit `e1b55167947`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DataReader

### §2 DataReader: read

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.read`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_all_samples_of_an_instance_in_one_collection_have_the_same_view_state_and_the_sample_state_of_before_the_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::javapsm_7_7_3_read_returns_a_fresh_copy_not_the_readers_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_read_and_take_on_a_closed_reader_throw_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_7_7_3_the_selector_of_read_and_take_carries_the_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_max_samples_limits_read_and_take_and_unlimited_is_accepted_as_a_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_the_data_state_selects_on_sample_view_and_instance_states`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_the_iterator_walks_the_samples_in_both_directions`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_read_selector_and_int_forms_return_the_same_samples_as_the_list_form`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_by_source_timestamp_a_sample_older_than_the_newest_of_the_instance_is_ordered_before_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_ordered_access_with_scope_topic_returns_the_samples_of_all_instances_in_one_order`

**Status:** `done` — `read()`, `read(int)`, `read(Selector)` (Sample.Iterator) und `read(List)`, `read(List, Selector)` (Kopie in die Liste) wenden `maxSamples` und die drei Zustandsmengen in `collectLocked` an; `read` setzt die Samples auf READ. Mit DESTINATION_ORDER BY_SOURCE_TIMESTAMP stehen die Samples einer Instanz nach Zeitstempel geordnet (`insertOrdered`); mit PRESENTATION ordered_access und Scope TOPIC oder GROUP gibt `collectLocked` die Samples aller Instanzen in einer Reihenfolge nach (Zeitstempel, Eintreffen) zurück, `maxSamples` schneidet diese Reihenfolge. Getestet sind das Verbleiben im Reader, READ beim zweiten read, Kopie, AlreadyClosedException, Inhaltsfilter, `read(int)`, `maxSamples`, die Zustandsmengen, der Inhalt des Iterators und beide Reihenfolgen. (Commit `e1b55167947`)
### §2 DataReader: take

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "take (inout: data_values: Data [], inout: sample_infos: SampleInfo
[], max_samples: long, sample_states: SampleStateKind [], view_states:
ViewStateKind [], instance_states: InstanceStateKind []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.instances`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_read_and_take_on_a_closed_reader_throw_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_without_live_writers_and_without_samples_is_forgotten_by_the_reader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_a_disposed_instance_stays_disposed_when_its_last_writer_unregisters`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_max_samples_limits_read_and_take_and_unlimited_is_accepted_as_a_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_an_instance_handle_limits_read_and_take_to_that_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_take_returns_the_samples_in_the_iterator_and_removes_them_from_the_reader`

**Status:** `done` — `take` in den vier Formen entfernt die gelieferten Samples aus dem Reader. Eine Instanz ohne lebende Writer und ohne Samples wird aus `instances` entfernt (`DataReaderImpl.reclaim`, DDS 1.4 §2.2.2.5.1.8); `lookupInstance` liefert danach den NIL-Handle, ein späteres Sample ist eine nie gesehene Instanz. Eine Instanz NOT_ALIVE_DISPOSED bleibt es, wenn der letzte Writer sich abmeldet. Getestet sind Reihenfolge, Entfernen, `take(int)`/`maxSamples`, Zustandsmengen, Iterator-Form und das Vergessen der Instanz. (Commit `e1b55167947`)
### §2 DataReader: Zustandsmasken bei read und take

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "sample_states: SampleStateKind [], view_states: ViewStateKind [],
instance_states: InstanceStateKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.dataState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataStateImpl.java` (`DataStateImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_the_data_state_selects_on_sample_view_and_instance_states`

**Status:** `done` — `read`/`take` tragen die Masken über `Selector.dataState(Subscriber.DataState)`; `collectLocked` filtert nach `sampleStates`, `viewStates` und `instanceStates` (ohne `dataState` alle, `getDataState()` liefert dann `DataStateImpl.any`). Der Test bringt acht Reader in denselben Zustand (Instanz 1 gelesen, Instanz 3 disposed, Instanz 2 unberührt) und fragt je einen mit READ, NOT_READ, NOT_NEW, NEW, NOT_ALIVE_DISPOSED, ALIVE und NOT_ALIVE_NO_WRITERS ab (erwartete Anzahlen aus dem Zustand abgeleitet) sowie `take` mit READ. (Commit `e1b55167947`)
### §2 DataReader: read_w_condition und take_w_condition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read_w_condition (inout: data_values: Data [], inout:
sample_infos: SampleInfo [], max_samples: long, a_condition: ReadCondition):
ReturnCode_t; take_w_condition (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.condition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_a_read_condition_of_the_reader_selects_its_states_with_the_selector`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_a_query_condition_of_the_reader_selects_by_its_expression_and_the_next_instance_keeps_it`

**Status:** `done` — `SelectorImpl.condition(ReadCondition)` (Erweiterung von ZeroDDS, nicht in `DataReader.Selector`) bindet eine Bedingung des Readers an den Selector; `getCondition()` liefert sie. `collectLocked` schneidet deren Zustandsmengen und, bei einer `QueryCondition`, deren Ausdruck mit den aktuellen Parametern mit den übrigen Vorgaben des Selectors. Eine Bedingung eines anderen Readers ergibt `PreconditionNotMetException`, eine geschlossene `AlreadyClosedException`. Getestet für `read` und `take`. (Commit `e1b55167947`)
### §2 DataReader: read_next_sample und take_next_sample

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 86 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_sample (inout: data_value: Data, inout: sample_info:
SampleInfo): ReturnCode_t; take_next_sample (inout: data_value: Data, inout:
sample_info: SampleInfo): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.readNextSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.takeNextSample`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_read_next_sample_returns_the_next_sample_not_yet_read_and_take_next_sample_removes_it`

**Status:** `done` — `readNextSample` und `takeNextSample` wählen mit einem Selector mit `maxSamples(1)` und SampleState NOT_READ (`DataReaderImpl.nextSampleSelector`), kopieren das Ergebnis per `copyInto` in das übergebene `SampleImpl` und liefern `false` ohne Sample (das übergebene Sample bleibt dann unverändert). Der Test ruft `readNextSample` dreimal auf: zwei verschiedene Samples, dann `false`; `takeNextSample` entfernt das Sample. (Commit `e1b55167947`)
### §2 DataReader: read_instance und take_instance

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 87 (`dds-1.2.pdf`), Tabelle
DataReader — "read_instance (..., a_handle: InstanceHandle_t, sample_states,
view_states, instance_states): ReturnCode_t; take_instance (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.instance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_an_instance_handle_limits_read_and_take_to_that_instance`

**Status:** `done` — `select().instance(handle)` begrenzt `read`/`take` auf die Instanz mit diesem Handle (Vergleich in `collectLocked`). Ein Handle ohne Instanz ergibt eine leere Sammlung (die Spec lässt BAD_PARAMETER zu, verlangt es nicht). Der Test liest und nimmt die zwei Samples der Instanz 2 von drei Instanzen; die Samples tragen deren Handle, `take` entfernt nur diese. (Commit `e1b55167947`)
### §2 DataReader: read_next_instance und take_next_instance

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 87 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_instance (..., previous_handle: InstanceHandle_t,
sample_states, view_states, instance_states): ReturnCode_t; take_next_instance
(...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.nextInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_next_instance_walks_the_instances_in_the_order_of_their_handles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_next_instance_skips_instances_without_samples_in_the_selected_states`

**Status:** `done` — Mit `Selector.nextInstance(true)` liefert `collectLocked` die Samples der Instanz mit dem kleinsten Handle, der größer als der von `instance(h)` ist (ohne `instance` ab dem kleinsten) und die passende Samples hat, und nur dieser Instanz. Die Handles sind nach `InstanceHandle.compareTo` (vorzeichenlos Byte für Byte) geordnet. Der Test schreibt die Schlüssel 3, 1, 2 und durchläuft sie als 1, 2, 3, danach leer; `take` nimmt nur die erste Instanz; Instanzen ohne Sample im gewählten Zustand werden übersprungen. (Commit `e1b55167947`)
### §2 DataReader: read_next_instance_w_condition und take_next_instance_w_condition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "read_next_instance_w_condition (..., previous_handle:
InstanceHandle_t, a_condition: ReadCondition): ReturnCode_t;
take_next_instance_w_condition (...): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_a_query_condition_of_the_reader_selects_by_its_expression_and_the_next_instance_keeps_it`

**Status:** `done` — Bedingung und vorheriger Handle lassen sich in einem Selector verbinden: `condition(c).instance(h).nextInstance(true)`. Der Test liest mit einer `QueryCondition` "seq > %0" ab Instanz 1 und erhält die Samples der Instanz 2 mit seq 1; geänderte Parameter ändern das Ergebnis; `take` mit der Bedingung entfernt nur die gewählten Samples. (Commit `e1b55167947`)
### §2 DataReader: return_loan

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "return_loan (inout: data_values: Data [], inout: sample_infos:
SampleInfo []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_close_ends_the_loan_and_closing_again_is_harmless`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_2_the_iterator_is_read_only`

**Status:** `done` — `Sample.Iterator.close()` bildet `return_loan` ab (`SampleIterator.close`); der Aufruf wirft nichts, auch beim zweiten Mal (Closeable). `read` und `take` kopieren die Daten (`deepCopy`), es wird nichts verliehen; die Samples bleiben nach `close` lesbar. Der Name `returnLoan` ist in der OMG-API nicht vorhanden (`close()`). (Commit `e0f89cfd194`)
### §2 DataReader: get_key_value

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "get_key_value (inout: key_holder: Data, handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::javapsm_2_get_key_value_returns_a_sample_of_the_instance_that_has_the_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_reader_fills_the_key_fields_of_the_holder_only`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_handle_of_no_instance_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_type_without_a_key_has_no_key_value`

**Status:** `done` — Der Reader hält je Instanz ein Sample (`Instance.keyData`). `getKeyValue(keyHolder, handle)` setzt bei einem Typ der Java-Typdarstellung (`ReflectionTypeSupport`) die Schlüsselfelder des Halters und lässt die übrigen; bei einem Typ ohne schreibbare Felder (Record, handgeschriebene Unterstützung) ist das Ergebnis eine Kopie dieses Samples statt des Halters. Ein unbekannter oder `null`-Handle ergibt `IllegalArgumentException`, ein Typ ohne Schlüssel `PreconditionNotMetException`. `getKeyValue(InstanceHandle)` liefert die Kopie. (Commit `e1b55167947`)
### §2 DataReader: lookup_instance

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "lookup_instance (instance: Data): InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.lookupInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::javapsm_2_lookup_instance_is_nil_before_the_instance_is_known_and_the_handle_of_the_sample_afterwards`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::javapsm_2_lookup_instance_with_a_handle_fills_it_and_without_one_returns_a_new_object`

**Status:** `done` — `lookupInstance(T)` liefert den Handle einer bekannten Instanz (gleich `Sample.getInstanceHandle()`) oder den NIL-Handle, vor dem ersten Sample und nach dem Vergessen der Instanz (`reclaim`). `lookupInstance(handle, key)` füllt den übergebenen Handle, bei `null` liefert es ein neues Objekt; ein `null`-Schlüssel ergibt `IllegalArgumentException`. (Commit `e1b55167947`)
### §2 DataReader: create_readcondition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "create_readcondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind []):
ReadCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createReadCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.hasMatchingSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DdsAsyncTest.java::async_2_2_wait_for_samples_async_completes_false_on_timeout`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states`

**Status:** `done` — `createReadCondition(DataState)` liefert eine `ReadConditionImpl` mit den Zustandsmengen zum Zeitpunkt des Erzeugens (Kopie); der Triggerwert ist wahr, solange der Reader ein Sample in diesen drei Mengen hält (`anySample`). `javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states` prüft an einem WaitSet NOT_READ und READ: leer falsch (Timeout), nach write wahr (die Bedingung steht in der aktiven Menge), nach read wechseln die Werte, nach take falsch. Die Bedingung wird im Reader geführt und von `close`/`closeContainedEntities` geschlossen. (Commit `e1b55167947`)
### §2 DataReader: create_querycondition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "create_querycondition (sample_states: SampleStateKind [],
view_states: ViewStateKind [], instance_states: InstanceStateKind [],
query_expression: string, query_parameters: string []): QueryCondition"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createQueryCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_query_condition_of_the_reader_triggers_on_a_matching_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_a_condition_knows_the_filter_that_triggers_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_query_condition_needs_one_sample_that_is_in_the_states_and_satisfies_the_expression`

**Status:** `done` — Beide Überladungen von `createQueryCondition` sind vorhanden. Der Triggerwert ist wahr, wenn ein Sample sowohl in den Zustandsmengen liegt als auch den Ausdruck erfüllt (`anySample(state, predicate)`); ein Sample ohne Daten erfüllt keinen Ausdruck. Der Test (Überladung mit `DataState`) hat ein gelesenes Sample, das den Ausdruck nicht erfüllt, und ein ungelesenes, das ihn erfüllt: Trigger falsch, nach read wahr. (Commit `e1b55167947`)
### §2 DataReader: delete_readcondition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 88 (`dds-1.2.pdf`), Tabelle
DataReader — "delete_readcondition (a_condition: ReadCondition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.closeContainedEntities`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_closed_condition_is_gone_and_a_closed_condition_on_a_wait_set_never_triggers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_close_contained_entities_closes_the_conditions_of_the_reader_and_not_the_reader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_closing_the_reader_closes_its_conditions`

**Status:** `done` — Der Reader führt seine Bedingungen (`DataReaderImpl.register`); `ReadConditionImpl.close` entfernt die Bedingung dort und markiert sie als geschlossen, ein zweites `close` hat keine Wirkung. `getTriggerValue` und `QueryCondition.setQueryParameters` an einer geschlossenen Bedingung ergeben `AlreadyClosedException`, ein WaitSet überspringt sie. Eine Bedingung, die nicht zu diesem Reader gehört (PRECONDITION_NOT_MET), kann `close()` nicht ausdrücken; der Selector meldet sie (siehe das Item zu read_w_condition). (Commit `e1b55167947`)
### §2 DataReader: get_liveliness_changed_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_liveliness_changed_status (out: status:
LivelinessChangedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.countLivelinessLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireReader.remoteLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireWriter.assertLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.assertOnWire`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_liveliness_changed_names_the_writer_whose_liveliness_changed_and_counts_the_changes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_an_incompatible_writer_is_not_counted_as_alive_or_not_alive`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderRemoteLivelinessTest.java::javapsm_2_a_remote_writer_is_alive_within_its_lease_and_alive_again_after_an_assertion`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/LivelinessHeartbeatTest.java::rtps_9_4_5_7_the_liveliness_flag_is_the_third_lowest_bit_and_independent_of_final`

**Status:** `done` — `alive_count`, `not_alive_count` und beide Änderungswerte werden aus den Writern im selben Prozess (`matchedWriters`: PARTITION überlappt, QoS verträglich; `countLivelinessLocked`) und aus den verbundenen Writern anderer Prozesse berechnet. Ein entfernter Writer lebt, solange die Zeit seit dem letzten Lebenszeichen innerhalb der Lease liegt, die er in SEDP angekündigt hat; ohne angekündigte Lebendigkeit gilt AUTOMATIC mit unendlicher Lease. Lebenszeichen sind ein DATA des Writers, ein HEARTBEAT mit dem L-Flag (DDSI-RTPS 2.5 §8.3.7.5; `DataWriter.assertLiveliness` und die Zusicherung der Participant-Ebene für MANUAL_BY_PARTICIPANT senden ihn an die verbundenen Reader) und bei AUTOMATIC jede Nachricht seines Participants (SPDP). Der Reader erfährt, wenn ein Writer nach Ablauf der Lease wieder lebt (`WireReader.setLivelinessListener`), und löst `onLivelinessChanged` aus. Nicht gebaut sind die eingebauten Endpunkte des Writer Liveliness Protocol (`DCPSParticipantMessage`, §8.4.13); ZeroDDS kündigt sie in SPDP nicht an, ein fremder Participant schickt sie deshalb nicht. Erwartung: Cyclone DDS, Fast DDS und OpenDDS halten AUTOMATIC-Writer über SPDP am Leben und werten das L-Flag des HEARTBEAT; MANUAL_BY_PARTICIPANT eines fremden Writers erreicht ZeroDDS nur über DATA und HEARTBEAT. (Commit `16f61d4789d`)
### §2 DataReader: last_publication_handle von LIVELINESS_CHANGED

**Spec:** §2, S. 11; [DDS] 1.2 IDL, S. 159–160 (`dds-1.2.pdf`) — "struct
LivelinessChangedStatus { [...] InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessChangedStatus.java` (`LivelinessChangedStatus.getLastPublicationHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_liveliness_changed_names_the_writer_whose_liveliness_changed_and_counts_the_changes`

**Status:** `done` — `last_publication_handle` von LIVELINESS_CHANGED ist der Handle des Writers, dessen Lebendigkeit sich zuletzt geändert hat (`DataReaderImpl.lastLivelinessHandle`, gesetzt in `countLivelinessLocked` beim Wechsel von `isAlive` und in `onWriterPresenceChanged` beim Verschwinden); `getLivelinessChangedStatus` und `evaluateLiveliness` geben ihn weiter. Der Test erwartet bei Erscheinen und bei Ablauf des Lease `writer.getInstanceHandle()`. (Commit `e1b55167947`)
### §2 DataReader: get_requested_deadline_missed_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_requested_deadline_missed_status (out: status:
RequestedDeadlineMissedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_an_infinite_deadline_is_never_missed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_requested_deadline_missed_names_the_instance_that_missed_last`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_the_change_of_requested_deadline_missed_starts_again_when_the_status_is_read`

**Status:** `done` — Die Versäumnisse werden je Instanz aus der Wanduhr berechnet (je abgelaufener Periode einmal, ab dem Eintreffen des letzten Samples) und vom Lease des Readers asynchron gemeldet. Getestet: `total_count` und `total_count_change`, Ausbleiben bei unendlicher Periode, `onRequestedDeadlineMissed`, `last_instance_handle` bei zwei Instanzen (gleich dem Handle der Instanz 2, auf die gewartet wird) und der Änderungswert 0 beim zweiten Lesen (Periode 1500 ms, der nächste Ablauf liegt weit nach dem Lesen). (Commit `e1b55167947`)
### §2 DataReader: get_requested_incompatible_qos_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_requested_incompatible_qos_status (out: status:
RequestedIncompatibleQosStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePoliciesWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::javapsm_7_3_5_1_a_reliable_reader_and_a_best_effort_writer_do_not_connect_and_both_record_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_requested_incompatible_qos_counts_the_refusing_policy_and_resets_its_change`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_every_policy_that_refuses_the_connection_is_counted_and_one_connection_is_one_in_total`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_the_presentation_of_the_subscriber_must_be_offered_by_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_writer_that_offers_less_than_requested_is_reported_to_the_listener`

**Status:** `done` — Beim Verbinden prüft der Reader die angebotene QoS mit `QosCompatibility.incompatiblePoliciesWith`: Reliability, Durability, Deadline, Liveliness, Ownership, DestinationOrder, LatencyBudget und PRESENTATION (die Policy des Subscribers, vom Reader gefordert, die des Publishers, vom Writer angeboten: Scope niedriger oder coherent/ordered nicht angeboten). Bei Unverträglichkeit entsteht kein Match und keine Zustellung; eine Verbindung zählt einmal in `total_count` und je verweigernder Policy einmal im `QosPolicyCount`; `last_policy_id` ist eine davon. `onRequestedIncompatibleQos` wird aufgerufen. Getestet: eine und drei Policies, die Rücksetzung der Änderung, PRESENTATION, der Listener. (Commit `d17c5f0c379`)
### §2 DataReader: get_sample_lost_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_sample_lost_status (out: status: SampleLostStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.noteSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_a_gap_in_the_sequence_numbers_of_a_writer_is_sample_lost`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_sample_lost_is_reported_to_the_listener`

**Status:** `done` — `DataReaderImpl.noteSequence` vergleicht die Sequenznummer eines Samples vom RTPS-Pfad (`deliverWire`) je Writer-GUID mit der zuletzt gesehenen; eine Lücke zählt in `lostTotal`, `getSampleLostStatus` meldet Gesamtzahl und Änderung und setzt die Änderung zurück, `onSampleLost` wird aufgerufen. Der Test liefert Sequenznummern 1, 4, 5 eines Writers und 10 eines zweiten: Gesamtzahl 2, Änderung 2, danach 0. Der Bus im Prozess verliert keine Samples. (Commit `e1b55167947`)
### §2 DataReader: get_sample_rejected_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_sample_rejected_status (out: status: SampleRejectedStatus):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleRejectedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_of_a_new_instance_beyond_max_instances_is_rejected_with_that_reason_and_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_beyond_max_samples_per_instance_is_rejected_with_that_reason_and_instance`

**Status:** `done` — `DataReaderImpl.rejectedByLimits` wendet RESOURCE_LIMITS (max_samples, max_instances, max_samples_per_instance) an, zählt `total_count`, setzt Grund und Instanz und löst `onSampleRejected` aus. Getestet: alle drei Gründe mit `last_reason` und `last_instance_handle` (der Handle des Schlüssels aus den 16 Oktetten von Hand abgeleitet), der Änderungswert und der Listener. (Commit `e1b55167947`)
### §2 DataReader: get_subscription_matched_status

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_subscription_matched_status (out: status:
SubscriptionMatchedStatus): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWireWriterMatch`), `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.MatchListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_status_is_readable_without_a_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_subscription_matched_names_the_writer_that_connected_or_left_last`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_a_writer_in_another_partition_or_with_less_qos_than_requested_is_not_matched`,
`tests/integration/tests/bindings_ffi.rs::dcps_2_2_4_1_subscription_matched_counts_a_writer_of_another_process`

**Status:** `done` — `total_count`, `current_count` und beide Änderungswerte werden mit dem Verbinden und Lösen von Writern geführt; Writer mit nicht überlappender PARTITION oder unverträglicher QoS zählen nicht; `last_publication_handle` ist der Handle des zuletzt verbundenen oder gelösten Writers. Die Tests prüfen Zählungen, Änderungen, Listener-Ereignisse und die Handles. Ein Writer eines anderen Prozesses, über SEDP gematcht, zählt ebenso (`onWireWriterMatch`); der Test gegen einen Rust-Writer (Job `integration-java-psm`) erwartet 1/1/1/1 beim Match und current_count 0 mit Änderung -1 nach `ignorePublication`. (Commit `e1b55167947`)
### §2 DataReader: last_publication_handle von SUBSCRIPTION_MATCHED

**Spec:** §2, S. 11; [DDS] 1.2 IDL, S. 160–161 (`dds-1.2.pdf`) — "struct
SubscriptionMatchedStatus { [...] InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.readSubscriptionMatchedStatusLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_subscription_matched_names_the_writer_that_connected_or_left_last`

**Status:** `done` — `readSubscriptionMatchedStatusLocked` übergibt `DataReaderImpl.lastMatchedHandle`: den Handle (`WriterPresence.writerHandle`) des Writers, der zuletzt verbunden oder gelöst wurde. Der Test erwartet nach dem ersten Writer, dem zweiten und nach dem Schließen des ersten jeweils dessen `getInstanceHandle()` und prüft, dass `getMatchedPublications` die Handles enthält. (Commit `e1b55167947`)
### §2 DataReader: get_topicdescription

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_topicdescription: TopicDescription"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getTopicDescription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.describedBy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_the_description_of_a_reader_is_the_topic_or_the_content_filtered_topic_it_was_created_with`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_reader_reports_the_multi_topic_as_its_description`

**Status:** `done` — `getTopicDescription` liefert die Beschreibung, mit der der Reader erzeugt wurde: bei ContentFilteredTopic und MultiTopic setzt `SubscriberImpl.createDataReader` sie über `describedBy`, sonst das Topic. Getestet sind Topic, ContentFilteredTopic (`assertSame`) und MultiTopic. (Commit `a2a9a67acb5`)
### §2 DataReader: get_subscriber

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_subscriber: Subscriber"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getParent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — `DataReaderImpl.getParent` gibt den `Subscriber` zurück, der den Reader erzeugt hat. Der Test erwartet ihn mit `assertSame`. (Commit `a2a9a67acb5`)
### §2 DataReader: delete_contained_entities

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "delete_contained_entities: ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_close_contained_entities_closes_the_conditions_of_the_reader_and_not_the_reader`

**Status:** `done` — `DataReaderImpl.closeContainedEntities` schließt alle mit `createReadCondition`/`createQueryCondition` erzeugten Bedingungen des Readers (Liste `conditions`); der Reader bleibt offen und kann neue Bedingungen erzeugen. Der Test prüft `AlreadyClosedException` an beiden Bedingungen, danach Schreiben, Lesen und eine neue Bedingung. (Commit `e1b55167947`)
### §2 DataReader: wait_for_historical_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "wait_for_historical_data (max_wait: Duration_t): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.waitForHistoricalData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.WireReader.historicalDataReceived`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_a_reader_that_is_not_on_the_wire_waits_for_no_historical_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_late_reader_gets_the_historical_data_of_a_transient_local_writer_before_wait_for_historical_data_returns`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::javapsm_2_the_call_returns_once_the_announced_samples_have_arrived`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::javapsm_2_the_call_is_a_timeout_exception_while_the_writer_announced_nothing`

**Status:** `done` — Im selben Prozess gehen die Samples eines TRANSIENT_LOCAL-Writers in `enable()` an den Reader (`replayTo`); beide Überladungen von `waitForHistoricalData` kehren dann sofort zurück, ein negativer Wert ergibt `IllegalArgumentException`. Beim RTPS-Reader mit nicht-VOLATILE Durability wartet der Aufruf auf `WireReader.historicalDataReceived`: jeder verbundene zuverlässige, nicht-VOLATILE entfernte Writer hat per HEARTBEAT seine letzte Sequenznummer gemeldet, der Reader hat bis dahin empfangen, und keine empfangene Nachricht ist noch auf dem Weg zum Reader (`deliveriesInFlight`). Nach Ablauf von `maxWait` folgt `TimeoutException`. Die beiden RTPS-Zweige sind mit einem zweiten RTPS-Teilnehmer getestet. (Commit `1d0691890fa`)
### §2 DataReader: get_matched_publication_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89, §7.1.2.5.3.33, S. 102
(`dds-1.2.pdf`) — "get_matched_publication_data (inout: publication_data:
PublicationBuiltinTopicData, publication_handle: InstanceHandle_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublicationData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_2_a_remote_writer_is_described_by_the_matched_publication_data`

**Status:** `done` — Für einen Writer im selben Prozess liefert `getMatchedPublicationData` die Built-in-Daten des Writers, für einen entfernten Writer die der SEDP-Meldung; ein unbekannter Handle wirft `IllegalArgumentException`. Der Test mit einem Writer eines zweiten RTPS-Teilnehmers prüft Topic-Name, Typname und OwnershipStrength der SEDP-Meldung; der Test am lokalen Writer prüft Topic-Name, Durability, OwnershipStrength und die Ausnahme für den NIL-Handle. (Commit `447490d2b6d`)
### §2 DataReader: get_matched_publications

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.3, S. 89 (`dds-1.2.pdf`), Tabelle
DataReader — "get_matched_publications (inout: publication_handles:
InstanceHandle_t []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublications`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_subscription_matched_names_the_writer_that_connected_or_left_last`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_a_writer_in_another_partition_or_with_less_qos_than_requested_is_not_matched`

**Status:** `done` — `getMatchedPublications` liefert die Handles der verbundenen Writer im Prozess (`matchedWriters` mit `writerHandle()`) und der über RTPS verbundenen (`wire.matchedEndpoints`). Getestet: Größe 1, Verwendung des Elements in `getMatchedPublicationData`, die Menge schrumpft nach `writer.close()`, ein Writer in anderer Partition und einer mit unverträglicher QoS fehlen darin. Writer über RTPS sind ungetestet. (Commit `e1b55167947`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: SampleInfo

### §2 SampleInfo: sample_state

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103, §7.1.2.5.1.2, S. 75
(`dds-1.2.pdf`) — "sample_state: SampleStateKind"; "READ indicates that the
DataReader has already accessed that sample by means of read."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSampleState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_all_samples_of_an_instance_in_one_collection_have_the_same_view_state_and_the_sample_state_of_before_the_read`

**Status:** `done` — `toSample` liefert den Zustand des Samples vor dem Zugriff und setzt es danach im Reader auf READ. Der Test liest dasselbe Sample zweimal und erwartet NOT_READ beim ersten, READ beim zweiten Mal. Die Angabe „immer NOT_READ“ gilt nicht mehr. (Commit `a2a9a67acb5`)
### §2 SampleInfo: view_state

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103, §7.1.2.5.1.8, S. 77
(`dds-1.2.pdf`) — "view_state: ViewStateKind"; "NEW indicates that either this
is the first time that the DataReader has ever accessed samples of that instance
[...] The view_state is therefore the same for all samples in the returned
collection that refer to the same instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getViewState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_all_samples_of_an_instance_in_one_collection_have_the_same_view_state_and_the_sample_state_of_before_the_read`


**Status:** `done` — `view_state` ist je Instanz einmal beim Zusammenstellen der Sammlung festgehalten (`Pick.view`); NOT_NEW wird erst nach der Sammlung gesetzt. Der Test (KEEP_ALL, zwei write, ein read): beide Samples NEW und NOT_READ, beim zweiten read NOT_NEW und READ; eine nach dispose neu belebte Instanz ist wieder NEW. (Commit `e1b55167947`)
### §2 SampleInfo: instance_state

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "instance_state: InstanceStateKind"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getInstanceState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onMessage`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/InstanceState.java` (`InstanceState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_unregister_only_leaves_no_writers_when_the_flag_is_false`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_a_write_after_the_dispose_brings_the_instance_back_to_alive`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_2_closing_one_writer_does_not_mark_no_writers_while_another_writes`

**Status:** `done` — Der Reader führt `ALIVE`, `NOT_ALIVE_DISPOSED` und `NOT_ALIVE_NO_WRITERS` je Instanz (`Instance.state`); `onMessage` setzt sie bei dispose, unregister und write. Die Tests prüfen NOT_ALIVE_DISPOSED nach dispose, NOT_ALIVE_NO_WRITERS nach unregister und nach dem Schließen des letzten Writers (nicht, solange ein zweiter Writer schreibt) und ALIVE nach erneutem write. Die Snapshot-Eigenschaft steht im folgenden Item. (Commit `a2a9a67acb5`)
### §2 SampleInfo: instance_state als Zustand beim Lesen

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.1.3, S. 75 (`dds-1.2.pdf`) — "The
instance_state available in the SampleInfo is a snapshot of the instance_state
of the instance at the time the collection was obtained (i.e., at the time read
or take was called). The instance_state is therefore the same for all samples in
the returned collection that refer to the same instance."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`

**Status:** `done` — `toSample` liest `instance_state` aus dem Zustand der Instanz zum Zeitpunkt von `read`/`take`, nicht aus dem Zeitpunkt des Eintreffens. Der Test (KEEP_ALL) schreibt zwei Samples, ruft dispose auf und erwartet in allen drei gelieferten Samples NOT_ALIVE_DISPOSED. Die Angabe „Daten-Sample mit ALIVE“ gilt nicht mehr. (Commit `a2a9a67acb5`)
### §2 SampleInfo: disposed_generation_count

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "disposed_generation_count: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getDisposedGenerationCount`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_disposed_and_written_again_counts_a_disposed_generation`

**Status:** `done` — Die Instanz führt `disposedGeneration`; `onWrite` erhöht den Zähler, wenn die Instanz von NOT_ALIVE_DISPOSED auf ALIVE wechselt; jedes Sample trägt den Zähler zum Zeitpunkt des Eintreffens und `SampleImpl.of` gibt ihn weiter. Der Test: write, dispose, write ergibt 0, 0 (Sample ohne Daten) und 1. (Commit `e1b55167947`)
### §2 SampleInfo: no_writers_generation_count

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "no_writers_generation_count: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getNoWritersGenerationCount`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_without_writers_written_again_counts_a_no_writers_generation`

**Status:** `done` — Die Instanz führt `noWritersGeneration`; `onWrite` erhöht den Zähler beim Wechsel von NOT_ALIVE_NO_WRITERS auf ALIVE. Der Test: write, unregister, write ergibt 0, 0 und 1. (Commit `e1b55167947`)
### §2 SampleInfo: sample_rank

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "sample_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSampleRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_ranks_are_measured_within_the_samples_of_one_instance_in_the_collection`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_disposed_and_written_again_counts_a_disposed_generation`

**Status:** `done` — `collectLocked` zählt nach der Auswahl je Sample die folgenden Samples derselben Instanz in der Sammlung (`sample_rank`). Getestet: zwei Instanzen mit je zwei Samples (1, 0, 1, 0), drei Samples einer Instanz (2, 1, 0), eine auf ein Sample gekürzte Sammlung (0) und die Reihenfolge über Instanzen bei ordered_access. (Commit `e1b55167947`)
### §2 SampleInfo: generation_rank

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "generation_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getGenerationRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_disposed_and_written_again_counts_a_disposed_generation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_without_writers_written_again_counts_a_no_writers_generation`

**Status:** `done` — `generation_rank` ist die Summe der Generationszähler des letzten Samples der Instanz in der Sammlung minus die des Samples (DDS 1.4 §2.2.2.5.1.6). Die Tests erwarten 1, 1, 0 für die Samples vor, bei und nach der Wiederbelebung. (Commit `e1b55167947`)
### §2 SampleInfo: absolute_generation_rank

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "absolute_generation_rank: long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getAbsoluteGenerationRank`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_disposed_and_written_again_counts_a_disposed_generation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_without_live_writers_and_without_samples_is_forgotten_by_the_reader`

**Status:** `done` — `absolute_generation_rank` ist die Summe der Generationszähler der Instanz im Reader (das jüngste empfangene Sample) minus die des Samples. Der erste Test erwartet 1, 1, 0; der zweite, dass eine vergessene und neu begonnene Instanz wieder 0 hat. (Commit `e1b55167947`)
### §2 SampleInfo: source_timestamp

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "source_timestamp: Time_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.deliverWire`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getSourceTimestamp`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsMessage.java` (`RtpsMessage.decode`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/SerializedSample.java` (`SerializedSample.sourceTimestampMillis`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_source_timestamp_is_the_one_the_writer_gave`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_the_source_timestamp_and_the_writer_guid_of_a_data_reach_the_sample_info`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::rtps_8_3_7_9_the_info_timestamp_before_a_data_is_its_source_timestamp`

**Status:** `done` — Im Prozess übernimmt der Reader den Zeitstempel des Writers (`Message.sourceTimestampMillis`). Über RTPS liest `RtpsMessage.decode` den INFO_TS vor DATA und DATA_FRAG (Time_t, Bruchteil in 2^-32 s; InvalidateFlag), `SerializedSample.sourceTimestampMillis` trägt ihn bis `deliverWire`; ohne INFO_TS gilt die Ankunftszeit. Getestet: beide Schreibformen im Prozess, der Wert 77000 über `deliverWire`, die Dekodierung 1700000000 s + 0x80000000/2^32 s = 1700000000500 ms (von Hand abgeleitet) und das Fehlen (-1). Der sendende Teil des Java-Teilnehmers schreibt den Zeitstempel der Anwendung nicht in INFO_TS. (Commit `e1b55167947`)
### §2 SampleInfo: instance_handle

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "instance_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.toSample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InstanceKey.java` (`InstanceKey.toHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_instance_handle_of_a_sample_is_the_one_the_reader_and_the_writer_look_up`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_a_key_longer_than_sixteen_octets_keeps_its_own_instance_handle`

**Status:** `done` — `InstanceKey.toHandle` bildet einen Schlüssel bis 16 Oktette auf diese Oktette (null aufgefüllt), einen längeren auf seinen MD5-Wert ab (DDSI-RTPS 2.5 §9.6.3.8); zwei Schlüssel, die der frühere XOR-Fold gleich abbildete, haben verschiedene Handles. Getestet: Sample-Handle gleich `Reader.lookupInstance` und `Writer.lookupInstance`, zwei 17-Oktett-Schlüssel mit gleichem Fold, die verschiedene Handles ergeben. (Commit `e0f89cfd194`)
### §2 SampleInfo: publication_handle

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103 (`dds-1.2.pdf`), Tabelle
SampleInfo — "publication_handle: InstanceHandle_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.deliverWire`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getPublicationHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_publication_handle_names_the_writer_that_is_matched_with_the_reader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_the_source_timestamp_and_the_writer_guid_of_a_data_reach_the_sample_info`


**Status:** `done` — Im Prozess ist `publication_handle` der Handle des Writers (gleich einem Element von `getMatchedPublications`). Über RTPS bildet `deliverWire` ihn aus der Writer-GUID der DATA (`Values.of(guid)`, wie `getMatchedPublications` die Endpoint-GUID nennt); ohne Identität ist er NIL. (Commit `e1b55167947`)
### §2 SampleInfo: valid_data

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.5, S. 103, §7.1.2.5.1.4, S. 76
(`dds-1.2.pdf`) — "valid_data: boolean"; "If the flag is set to FALSE, the
DataSample contains no Data."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_7_7_2_disposing_a_constituent_disposes_the_result_instance`

**Status:** `done` — `markState` legt für dispose und unregister ein Sample ohne Daten an; `Sample.getData()` ist dann `null` (§7.7.2). Der Test liest an einem MultiTopic-Reader (ein `DataReaderImpl`) nach dispose ein Sample mit `getData() == null` und NOT_ALIVE_DISPOSED. Die Angabe „kein Test prüft null“ gilt nicht mehr; der Pfad für unregister ist nicht eigens getestet. (Commit `a2a9a67acb5`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: DataReaderListener

### §2 DataReaderListener: on_data_available

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_data_available (the_reader: DataReader): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onDataAvailable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_clearing_the_listener_stops_delivery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::javapsm_7_2_4_a_callback_does_not_hold_the_reader_monitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_without_a_subscriber_listener_the_readers_listeners_are_called_and_data_on_readers_stays_changed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_status_condition_triggers_only_on_the_statuses_it_is_enabled_for`

**Status:** `done` — `fireDataAvailable` setzt DATA_AVAILABLE (`triggerStatus`) und ruft `onDataAvailable` je eingetroffenem Sample und je Sample ohne Daten (dispose, unregister) mit dem Reader als Quelle. Hat der Subscriber (oder der DomainParticipant) einen Listener mit DATA_ON_READERS, geht der Aufruf stattdessen an dessen `onDataOnReaders`. Die Tests prüfen Anzahl, Quelle, das Ausbleiben nach `setListener(null)`, dass der Aufruf den Monitor des Readers nicht hält, und dass eine StatusCondition auf DATA_AVAILABLE nach write wahr und nach take falsch ist. (Commit `e1b55167947`)
### §2 DataReaderListener: on_sample_rejected

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_sample_rejected (the_reader: DataReader, status:
SampleRejectedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSampleRejected`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleRejectedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener`

**Status:** `done` — `DataReaderImpl.onWrite` ruft `rejectedByLimits`; ein abgelehntes Sample zählt in `SampleRejectedStatusImpl`, `fireRejectedAndLost` ruft `onSampleRejected` außerhalb des Monitors. `ReaderStatusTest::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener` nutzt einen Reader mit max_samples=1: das zweite write löst den Aufruf mit Grund REJECTED_BY_SAMPLES_LIMIT aus, der Reader behält das erste Sample. (Commit `a2a9a67acb5`)
### §2 DataReaderListener: on_liveliness_changed

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_liveliness_changed (the_reader: DataReader, status:
LivelinessChangedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onLivelinessChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateLiveliness`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`

**Status:** `done` — `DataReaderImpl.evaluateLiveliness` übergibt den Status außerhalb des Monitors an `onLivelinessChanged`, ausgelöst vom Lease des Readers, ohne dass die Anwendung den Status abfragt. Der Test setzt einen Listener, schreibt einmal an einen MANUAL_BY_TOPIC-Writer mit 30 ms Lease und wartet auf den Callback mit `getNotAliveCount() >= 1`. Die Zähler `alive_count_change` und `not_alive_count_change` prüft kein Test. (Commit `a2a9a67acb5`)
### §2 DataReaderListener: on_requested_deadline_missed

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_requested_deadline_missed (the_reader: DataReader,
status: RequestedDeadlineMissedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onRequestedDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDeadlineMissed`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`

**Status:** `done` — `DataReaderImpl.fireDeadlineMissed` ruft `onRequestedDeadlineMissed` mit dem Status außerhalb des Monitors auf, ausgelöst vom Lease, ohne Abfrage durch die Anwendung. Der Test schreibt ein Sample bei 30 ms Deadline und wartet auf den Callback mit `getTotalCount() >= 1`. Der Wert von `last_instance_handle` und `total_count_change` ist nicht geprüft. (Commit `a2a9a67acb5`)
### §2 DataReaderListener: on_requested_incompatible_qos

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_requested_incompatible_qos (the_reader: DataReader,
status: RequestedIncompatibleQosStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onRequestedIncompatibleQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.recordRequestedIncompatibleLocked`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_writer_that_offers_less_than_requested_is_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_requested_incompatible_qos_counts_the_refusing_policy_and_resets_its_change`

**Status:** `done` — `DataReaderImpl.onWriterPresenceChanged` ruft `onRequestedIncompatibleQos` außerhalb des Monitors, wenn ein Writer mit unverträglicher QoS erscheint. Der Test mit RELIABLE-Reader und BEST_EFFORT-Writer erwartet einen Aufruf mit `getLastPolicyClass() == Reliability.class` und keinem verbundenen Writer. (Commit `a2a9a67acb5`)
### §2 DataReaderListener: on_subscription_matched

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_subscription_matched (the_reader: DataReader, status:
SubscriptionMatchedStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSubscriptionMatched`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWriterPresenceChanged`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWireWriterMatch`), `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`RtpsParticipant.MatchListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`tests/integration/tests/bindings_ffi.rs::dcps_2_2_4_1_subscription_matched_counts_a_writer_of_another_process`

**Status:** `done` — `DataReaderImpl.onWriterPresenceChanged` ruft `onSubscriptionMatched` beim Erscheinen und Verschwinden eines Writers mit dem Reader als Quelle und dem Status. Der erste Test prüft Aufrufzahl 1 bzw. 2, `currentCount` 1 bzw. 0, `totalCount` 1 und dass der Callback den eigenen Reader liefert. Der zweite prüft, dass ein Writer einer anderen Partition keinen Aufruf auslöst. Für einen Writer eines anderen Prozesses ruft `onWireWriterMatch` den Listener ebenso, auf einem Thread des RTPS-Participants in der Reihenfolge der Änderungen; der Test gegen einen Rust-Writer erwartet die zwei Aufrufe mit 1/1/1/1 und 1/0/0/-1. (Commit `a2a9a67acb5`)
### §2 DataReaderListener: on_sample_lost

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.7, S. 105 (`dds-1.2.pdf`), Tabelle
DataReaderListener — "on_sample_lost (the_reader: DataReader, status:
SampleLostStatus): void"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener.onSampleLost`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_sample_lost_is_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_a_gap_in_the_sequence_numbers_of_a_writer_is_sample_lost`

**Status:** `done` — `DataReaderImpl.deliverWire` meldet über `noteSequence` und `fireRejectedAndLost` die Lücke in den Sequenznummern eines Writers an `onSampleLost` (außerhalb des Monitors). Der Test liefert die Sequenznummern 1 und 3 und erwartet einen Aufruf mit Gesamtzahl 1 und Änderung 1; eine ältere Sequenznummer löst keinen weiteren aus. (Commit `e1b55167947`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: Conditions und WaitSet

### §2 Condition: get_trigger_value

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.7, S. 31–32 (`dds-1.2.pdf`), Tabelle
Condition — "get_trigger_value: boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Condition.java` (`Condition.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.getTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getTriggerValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_query_condition_of_the_reader_triggers_on_a_matching_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_query_condition_needs_one_sample_that_is_in_the_states_and_satisfies_the_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_status_condition_triggers_only_on_the_statuses_it_is_enabled_for`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`

**Status:** `done` — `ReadConditionImpl.getTriggerValue` (Zustandsmasken, bei der QueryCondition zusätzlich Ausdruck am selben Sample), `StatusConditionImpl.getTriggerValue` (geänderte Statusflags der aktivierten Statuses) und `GuardConditionImpl.getTriggerValue` sind getestet. (Commit `e1b55167947`)
### §2 WaitSet: Erzeugen ohne Factory

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`) — "WaitSet has
no factory. It is created as an object directly by the natural means in each
language binding (e.g., using “new” in C++ or Java)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newWaitSet`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_query_condition_of_the_reader_triggers_on_a_matching_sample`

**Status:** `done` — `WaitSet.newWaitSet(env)` erzeugt über `env.getSPI().newWaitSet()` direkt ein `WaitSetImpl`, ohne Entity-Factory. Der erste Test prüft, dass der SPI ein Objekt liefert; der zweite erzeugt über `WaitSet.newWaitSet(reader.getEnvironment())` ein WaitSet und hängt eine Condition an und ab. (Commit `5041698cee3`)
### §2 WaitSet: attach_condition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "attach_condition (a_condition: Condition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.attachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.attachCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_wait_set_lists_what_is_attached_and_a_detached_condition_does_not_end_a_wait`

**Status:** `done` — `WaitSetImpl.attachCondition` nimmt die Bedingung auf (mehrfaches Anhängen wirkt nicht) und meldet sie bei einer `GuardConditionImpl` an; Statusänderungen und Sample-Eingang wecken über `WaitSetImpl.conditionChanged` alle WaitSets. Getestet: Anhängen, `getConditions`, Warten an Guard-, Read- und StatusCondition. (Commit `5041698cee3`)
### §2 WaitSet: detach_condition

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "detach_condition (a_condition: Condition): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.detachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.detachCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_wait_set_lists_what_is_attached_and_a_detached_condition_does_not_end_a_wait`

**Status:** `done` — `WaitSetImpl.detachCondition` entfernt die Bedingung und meldet sie bei der `GuardConditionImpl` ab; eine nicht angehängte Bedingung ergibt `PreconditionNotMetException`. Der Test setzt eine abgehängte Guard-Bedingung auf wahr: das Warten endet nicht (Timeout), nach erneutem Anhängen sofort. Eine geschlossene Bedingung zählt im WaitSet als nicht ausgelöst. (Commit `e1b55167947`)
### §2 WaitSet: wait

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "wait (inout: active_conditions: Condition [], timeout: Duration_t):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.await`),
`crates/java-omgdds/java/src/main/java/org/zerodds/DdsAsync.java` (`DdsAsync.awaitSamples`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_second_thread_waiting_on_the_same_wait_set_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_wait_with_a_timeout_ends_in_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`

**Status:** `done` — `WaitSetImpl.await` wartet, bis eine angehängte Bedingung wahr ist; Guard-Bedingungen wecken über `signal`, Status- und Read-Bedingungen über `WaitSetImpl.conditionChanged` (aus `triggerStatus` und beim Sample-Eingang). Ein zweiter wartender Thread ergibt `PreconditionNotMetException`, ein Ablauf der Zeit `TimeoutException`. Getestet: Wecken durch `setTriggerValue` und durch ein write aus einem anderen Thread. (Commit `5041698cee3`)
### §2 WaitSet: Ergebnis von wait

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "inout: active_conditions: Condition []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.await`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_wait_set_lists_what_is_attached_and_a_detached_condition_does_not_end_a_wait`

**Status:** `done` — `WaitSetImpl.await` legt alle angehängten Bedingungen mit Triggerwert wahr in die übergebene Sammlung (zuvor geleert). Die Tests prüfen die Größe und `assertSame` auf die erwartete Bedingung, auch mit zwei Bedingungen am WaitSet, von denen eine falsch ist. (Commit `5041698cee3`)
### §2 WaitSet: get_conditions

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.6, S. 30 (`dds-1.2.pdf`), Tabelle
WaitSet — "get_conditions (inout: attached_conditions: Condition []):
ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.getConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.getConditions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_wait_set_lists_what_is_attached_and_a_detached_condition_does_not_end_a_wait`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`

**Status:** `done` — `getConditions` liefert eine Kopie der angehängten Bedingungen. Der Test hängt eine ReadCondition und eine GuardCondition an (zwei Einträge), hängt eine ab (ein Eintrag, die abgehängte fehlt). (Commit `e1b55167947`)
### §2 GuardCondition: Erzeugen ohne Factory, Anfangswert FALSE

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.8, S. 32 (`dds-1.2.pdf`) —
"GuardCondition has no factory. It is created as an object directly by the
natural means in each language binding (e.g., using “new” in C++ or Java). When
first created the trigger_value is set to FALSE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`

**Status:** `done` — `GuardCondition.newGuardCondition(env)` liefert eine `GuardConditionImpl` ohne Entity-Factory; der Test erwartet `getTriggerValue() == false` unmittelbar nach dem Erzeugen. (Commit `5041698cee3`)
### §2 GuardCondition: set_trigger_value

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.8, S. 32 (`dds-1.2.pdf`), Tabelle
GuardCondition — "set_trigger_value (value: boolean): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.setTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.setTriggerValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_wait_set_lists_what_is_attached_and_a_detached_condition_does_not_end_a_wait`

**Status:** `done` — `GuardConditionImpl.setTriggerValue` setzt den Wert und weckt die angehängten WaitSets (`WaitSetImpl.attachCondition` meldet sie an). Der Test setzt den Wert aus einem zweiten Thread; das blockierte Warten endet und meldet die Bedingung. (Commit `5041698cee3`)
### §2 StatusCondition: set_enabled_statuses und get_enabled_statuses

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.9, S. 32 (`dds-1.2.pdf`), Tabelle
StatusCondition — "set_enabled_statuses (mask: StatusKind []): ReturnCode_t;
get_enabled_statuses: StatusKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.getEnabledStatuses`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.setEnabledStatuses`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getEnabledStatuses`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_status_condition_triggers_only_on_the_statuses_it_is_enabled_for`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`

**Status:** `done` — Die Voreinstellung von `getEnabledStatuses` ist die Menge aller Statuses (`Status.allStatuses`); `setEnabledStatuses` beschränkt den Triggerwert auf die gewählten Statuses. Der Test: SUBSCRIPTION_MATCHED ist geändert, nach Beschränkung auf DATA_AVAILABLE falsch, nach write wahr, nach take falsch. (Commit `e1b55167947`)
### §2 StatusCondition: get_entity

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.1.9, S. 32 (`dds-1.2.pdf`), Tabelle
StatusCondition — "get_entity: Entity"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getStatusCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_status_condition_triggers_only_on_the_statuses_it_is_enabled_for`

**Status:** `done` — `EntityBase.getStatusCondition().getParent()` ist die Entity; beide Tests erwarten mit `assertSame` den Reader. (Commit `d23eff980f3`)
### §2 ReadCondition: get_datareader

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.8, S. 106 (`dds-1.2.pdf`), Tabelle
ReadCondition — "get_datareader: DataReader"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getParent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createReadCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states`

**Status:** `done` — `ReadConditionImpl.getParent` liefert den Reader, der die Bedingung erzeugt hat; beide Tests erwarten ihn mit `assertSame`. (Commit `d23eff980f3`)
### §2 ReadCondition: get_sample_state_mask, get_view_state_mask, get_instance_state_mask

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.8, S. 106 (`dds-1.2.pdf`), Tabelle
ReadCondition — "get_sample_state_mask: SampleStateKind []; get_view_state_mask:
ViewStateKind []; get_instance_state_mask: InstanceStateKind []"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition.getSampleStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getSampleStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getViewStates`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl.getInstanceStates`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_read_condition_is_true_while_the_reader_holds_a_sample_in_its_states`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`

**Status:** `done` — `createReadCondition` liefert genau die Zustandsmengen des übergebenen `DataState` (Kopie zum Zeitpunkt des Erzeugens); der Triggerwert folgt ihnen: NOT_READ wahr bis zum ersten read, READ danach. (Commit `e1b55167947`)
### §2 QueryCondition: get_query_expression

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.9, S. 107 (`dds-1.2.pdf`), Tabelle
QueryCondition — "get_query_expression: string"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/QueryCondition.java` (`QueryCondition.getQueryExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl.getQueryExpression`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_a_condition_knows_the_filter_that_triggers_it`

**Status:** `done` — `QueryConditionImpl.getQueryExpression` liefert den bei `DataReaderImpl.createQueryCondition` übergebenen Ausdruck. Der Test erzeugt eine Condition mit `"value > %0"` und prüft Gleichheit des zurückgegebenen Ausdrucks. (Commit `2e0b194caf5`)
### §2 QueryCondition: get_query_parameters und set_query_parameters

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.2.5.9, S. 107 (`dds-1.2.pdf`), Tabelle
QueryCondition — "get_query_parameters (out: query_parameters: string []):
ReturnCode_t; set_query_parameters (query_parameters: string []): ReturnCode_t"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/QueryCondition.java` (`QueryCondition.getQueryParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/QueryConditionImpl.java` (`QueryConditionImpl.setQueryParameters`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.createQueryCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_the_query_parameters_are_the_current_ones_when_the_trigger_value_is_evaluated`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderConditionTest.java::javapsm_2_a_query_condition_needs_one_sample_that_is_in_the_states_and_satisfies_the_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_a_query_condition_of_the_reader_selects_by_its_expression_and_the_next_instance_keeps_it`

**Status:** `done` — Der Triggerwert einer QueryCondition übersetzt Ausdruck und aktuelle Parameter bei jeder Auswertung (`createQueryCondition`), `setQueryParameters` wirkt also sofort und weckt WaitSets. Der Test: `seq > %0` mit 5 und seq 3 falsch, mit 2 wahr, mit 3 falsch. (Commit `e1b55167947`)

---

## §2 Programmierschnittstellen von [DDS] 1.2: eingebaute Topics

### §2 Eingebaute Topics: DCPSParticipant

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.5, S. 148 (`dds-1.2.pdf`) —
"DCPSParticipant: key BuiltinTopicKey_t; user_data UserDataQosPolicy"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ParticipantBuiltinTopicData.java` (`ParticipantBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/ParticipantBuiltinTopicDataImpl.java` (`ParticipantBuiltinTopicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.participant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_a_builtin_reader_has_the_entities_that_existed_before_and_none_of_its_own_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantDiscoveryTest.java::javapsm_2_the_builtin_reader_sees_the_user_data_change_of_another_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_participant_data_carries_the_user_data`

**Status:** `done` — `ParticipantBuiltinTopicData` hat `getKey` und `getUserData`; `BuiltinData.participant` füllt sie aus SPDP, `BuiltinTopicWire` kodiert sie. Ein Participant mit nicht leerem USER_DATA erscheint mit diesem Wert im Reader auf DCPSParticipant und in getDiscoveredParticipantData. Tests: ParticipantDiscoveryTest::javapsm_2_a_builtin_reader_has_the_entities_that_existed_before_and_none_of_its_own_participant (USER_DATA {1, 2} im Reader des eingebauten Subscribers), ::javapsm_2_the_discovered_participants_are_the_other_participants_of_the_domain_and_their_user_data (getDiscoveredParticipantData), ::javapsm_2_the_builtin_reader_sees_the_user_data_change_of_another_participant (setQos meldet neue Daten). (Commit `cd8f3d67ea9`)
### §2 Eingebaute Topics: DCPSTopic

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.5, S. 148 (`dds-1.2.pdf`) — "DCPSTopic: key,
name, type_name, durability, durability_service, deadline, latency_budget,
liveliness, reliability, transport_priority, lifespan, destination_order,
history, resource_limits, ownership, topic_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicBuiltinTopicData.java` (`TopicBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.topic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_topic_data_carries_the_topic_qos`

**Status:** `done` — Die Schnittstelle hat alle Felder der Spec (key, name, type_name, durability, durability_service, deadline, latency_budget, liveliness, reliability, transport_priority, lifespan, destination_order, history, resource_limits, ownership, topic_data), `BuiltinData.topic` füllt sie. BuiltinTopicQosTest::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13 prüft die Oktette von name, durability, durability_service, deadline, latency_budget, liveliness, reliability, transport_priority, lifespan, destination_order, history, resource_limits, ownership und topic_data auf ihren Parameter-IDs und liest sie zurück. (Commit `5041698cee3`)
### §2 Eingebaute Topics: DCPSPublication

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.5, S. 149 (`dds-1.2.pdf`) —
"DCPSPublication: key, participant_key, topic_name, type_name, durability,
durability_service, deadline, latency_budget, liveliness, reliability, lifespan,
user_data, ownership, ownership_strength, destination_order, presentation,
partition, topic_data, group_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/PublicationBuiltinTopicData.java` (`PublicationBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.publication`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`

**Status:** `done` — Alle Felder der Spec stehen in der Schnittstelle und in `BuiltinData.publication`. Der Wire-Test prüft Parameter-IDs und Bytes für durability, deadline, latency_budget, liveliness, reliability, lifespan, user_data, ownership, ownership_strength, destination_order, partition, group_data, presentation, topic_name, type_name und beide GUIDs; der Rundlauf vergleicht alle Felder nach dem Dekodieren, der Reader-Test liest abweichende Werte aus dem eingebauten Reader. durability_service und topic_data der Publikation prüft kein Test. (Commit `5041698cee3`)
### §2 Eingebaute Topics: DCPSSubscription

**Spec:** §2, S. 11; [DDS] 1.2 §7.1.5, S. 150 (`dds-1.2.pdf`) —
"DCPSSubscription: key, participant_key, topic_name, type_name, durability,
deadline, latency_budget, liveliness, reliability, ownership, destination_order,
user_data, time_based_filter, presentation, partition, topic_data, group_data"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/SubscriptionBuiltinTopicData.java` (`SubscriptionBuiltinTopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinData.java` (`BuiltinData.subscription`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicWire.java` (`BuiltinTopicWire.encode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_sedp_of_an_endpoint_without_qos_parameters_takes_the_defaults`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`

**Status:** `done` — Die Schnittstelle und `BuiltinData.subscription` enthalten alle Felder der Spec. BuiltinTopicQosTest::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13 prüft die Oktette von durability, deadline, latency_budget, liveliness, ownership, destination_order, time_based_filter, user_data, partition, group_data, presentation und topic_data auf ihren Parameter-IDs und liest den Schlüssel und einen Teil der Policies zurück. (Commit `5041698cee3`)

---

## §3 References

### §3.1 [DDS] Version 1.2

**Spec:** §3.1, S. 11 — "[DDS] Data Distribution Service for Real-Time Systems
Specification, version 1.2 (OMG document formal/2007-01-01)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis. Die Items unter §2 zitieren [DDS] 1.2 (`.agent/spec/dds-1.2.txt`). Die Kommentare im Java-Code zitieren DDS 1.4 (z. B. `ListenerTest`: "DDS 1.4 §2.2.4.1", `DataReaderImpl`); ZeroDDS selbst folgt DDS 1.4 (`.agent/spec/dds-dcps-1.4.txt`).
### §3.1 [DDS-CCM] 1.0 Beta 1

**Spec:** §3.1, S. 11 — "[DDS-CCM] DDS for Lightweight CCM, version 1.0 Beta 1
(OMG document ptc/2009-02-02)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:** —

**Status:** `n/a (informative)` — Verweis. Die Java-Bindung verlangt von [DDS-CCM] nichts Eigenes; `QosProviderImpl` erwähnt die XML-QoS-Bibliotheken von [DDS-CCM] im Klassenkommentar. Ein Vergleich mit der Beta 1 wurde nicht durchgeführt. (Commit `2e0b194caf5`)
### §3.1 [DDS-XTypes] 1.0 Beta 1

**Spec:** §3.1, S. 11 — "[DDS-XTypes] Extensible and Dynamic Topic Types for
DDS, version 1.0 Beta 1 (OMG document ptc/2010-05-12)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Extensibility.java` (`Extensibility.Kind`)

**Tests:** —

**Status:** `n/a (informative)` — Verweis. Verglichen wird, wo nötig, mit XTypes 1.3 (`.agent/spec/dds-xtypes-1.3.txt`). `org.omg.dds.type.Extensibility.Kind` benennt die Erweiterbarkeit weiter `EXTENSIBLE_EXTENSIBILITY`.
### §3.1 [Java-MAP] IDL to Java 1.3

**Spec:** §3.1, S. 11 — "[Java-MAP] IDL to Java Language Mapping, Version 1.3
(OMG document formal/2008-01-11)"

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`adapter::java`)

**Tests:** —

**Status:** `n/a (informative)` — Verweis. Im Repo liegt IDL4 to Java 1.0 (`.agent/spec/idl4-java-1.0.txt`, Coverage in `docs/spec-coverage/idl4-java-1.0.md`). Der Java-Codegen beruft sich für öffentliche Felder auf "§1.5.10 of the OMG mapping" (`crates/lang-rules/src/adapter/java.rs`, Kommentar zu den öffentlichen Feldern).
### §3.1 [Java-Lang] The Java Language Specification, Third Edition

**Spec:** §3.1, S. 12 — "[Java-Lang] The Java Language Specification, Third
Edition, published by Addison Wesley in 2005 with ISBN 0321246780"

**Repo:**
`crates/java-omgdds/java/pom.xml` (`properties/profile java8`),
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java` (`PubSubLoopbackTest.Reading`)

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf Java 5. Das Modul übersetzt mit `maven.compiler.source` 17 und im Profil `java8` mit Release 8 (`crates/java-omgdds/java/pom.xml`); die Tests brauchen Java 17 (Records, z. B. `PubSubLoopbackTest.Reading`). (Commit `5536e2abcac`)
### §3.1 [XML] 1.1, Second Edition

**Spec:** §3.1, S. 12 — "[XML] Extensible Markup Language (XML), version 1.1,
Second Edition (W3C recommendation, August 2006)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:** —

**Status:** `n/a (informative)` — Verweis für die XML-QoS-Profile (§7.3.5.4). Die Java-Bindung liest XML: `QosProviderImpl` parst über `javax.xml.parsers.DocumentBuilderFactory` mit abgeschaltetem externem DTD- und Schema-Zugriff. Welche XML-Version dabei gilt, wurde nicht geprüft. (Commit `2e0b194caf5`)
### §3.2 [JMS] 1.1 (nicht normativ)

**Spec:** §3.2, S. 12 — "The following non-normative references are provided for
informational purposes. • [JMS] Java Message Service Specification, version 1.1
(Sun Microsystems, http://java.sun.com/products/jms/ docs.html)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.

---

## §4 Terms and Definitions

### §4 Data-Centric Publish-Subscribe (DCPS)

**Spec:** §4, S. 12 — "The mandatory portion of the DDS specification used to
provide the functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Data Distribution Service (DDS)

**Spec:** §4, S. 12 — "An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness and
reliability. It is independent of implementation languages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Data Local Reconstruction Layer

**Spec:** §4, S. 12 — "The optional portion of the DDS specification used to
provide the functionality required for an application for direct access to data
exchanged at the DCPS layer. This latter builds upon the DCPS layer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Java Archive (JAR)

**Spec:** §4, S. 12 — "A zip file, whose name ends in the suffix .jar, that
contains the compiled Java class files and other artifacts that comprise a Java
library."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Java Runtime Environment (JRE)

**Spec:** §4, S. 12 — "The environment within which Java applications execute.
The JRE consists of an executing instance of a JVM, a set of class libraries,
and potentially other components."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Java Virtual Machine (JVM)

**Spec:** §4, S. 12 — "An abstract computing machine capable of executing
interpreted and/or compiled Java byte code. JVM implementations typically take
the form of executables that run as processes under operating systems, but this
style of implementation is not mandatory."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Platform-Independent Model (PIM)

**Spec:** §4, S. 12 — "An abstract definition of a facility, often expressed
with the aid of formal or semi-formal modeling languages such as OMG UML, that
does not depend on any particular implementation technology."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §4 Platform-Specific Model (PSM)

**Spec:** §4, S. 13 — "A concrete definition of a facility, typically based on a
corresponding PIM, in which all implementation-specific dependencies have been
resolved."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.

---

## §5 Symbols

### §5 Keine Symbole

**Spec:** §5, S. 13 — "This specification does not define any symbols or
abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Keine Anforderung.

---

## §6 Additional Information

### §6.1 Keine Änderung anderer OMG-Specs

**Spec:** §6.1, S. 13 — "This specification does not extend or modify any
existing OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage über die Spec.
### §6.2 Abhängigkeit von Java SE 5, weitere Vorgaben der Implementierung

**Spec:** §6.2, S. 13 — "This specification depends on version 5 of the Java
Standard Edition platform. Service implementations may impose additional
constraints; the nature and scope of these, if any, are unspecified."

**Repo:**
`crates/java-omgdds/java/pom.xml` (`properties/profile java8`)

**Tests:** —

**Status:** `n/a (informative)` — Aussage über die Spec und Erlaubnis zu weiteren Vorgaben. ZeroDDS übersetzt mit Java 17 oder im Profil `java8` mit Release 8 (`crates/java-omgdds/java/pom.xml`); Java 5 bis 7 ist nicht Ziel. (Commit `5536e2abcac`)
### §6.2 Begründung: Java SE statt Java EE oder Java ME

**Spec:** §6.2, S. 13 — "Design Rationale (non-normative) As of the publication
of this specification, Java SE remains the predominant platform for the
development and deployment of DDS Java applications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §6.3 Acknowledgements

**Spec:** §6.3, S. 13 — "The following companies submitted this specification: •
Real-Time Innovations, Inc. (RTI) • PrismTech"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7.1 Specification Organization

### §7.1 Gliederung nach den Modulen von [DDS]

**Spec:** §7.1, S. 15 — "This specification is organized according to the module
defined by the DDS specification and the types and operations defined within
them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung der Spec.

---

## §7.2 General Concerns and Conventions

### §7.2 Übergreifende Festlegungen

**Spec:** §7.2, S. 15 — "This sub clause defines those elements of this
specification that cut across multiple DDS modules."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Festlegungen folgen in §7.2.1 bis §7.2.6.
### §7.2.1 Pakete mit dem Präfix `org.omg.dds`

**Spec:** §7.2.1, S. 15 — "This PSM is defined in a set of Java packages, the
names of each beginning with the prefix org.omg.dds. Each of these contains a
Java interface or abstract class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedStringTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedBytesTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/TypeObject.java` (`TypeObject`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_3_1_dds_object_carries_the_one_operation_the_clause_names`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_the_built_in_type_dds_keyed_string`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_is_the_built_in_type_dds_keyed_bytes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_the_key_hash_of_a_keyed_string_is_the_md5_of_its_key_in_big_endian`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_2_1_every_interface_of_the_type_object_package_is_implemented`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`

**Status:** `done` — Die Pakete `org.omg.dds.*` enthalten für jeden Typ der DDS-Module eine Schnittstelle oder abstrakte Klasse der OMG-Quelle (184 Dateien; `CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source` vergleicht sie mit den SHA-256 der Quelle). KeyedString und KeyedBytes sind Themen-Typen: `TypeSupportResolver.resolve` liefert `BuiltinTypeSupports.KeyedStringTypeSupport` und `KeyedBytesTypeSupport` (XTypes 1.3 Anhang E: appendable, `@key string key`); `CoreBuiltinKeyedTest` schreibt und liest je ein Thema mit zwei Instanzen, die KeyHash-Oktette sind aus XTypes 1.3 §7.6.8 abgeleitet (MD5 über den Schlüssel in Big-Endian, nicht gegen eine andere Implementierung geprüft). Die Typen von `org.omg.dds.type.typeobject` sind als Objekte umgesetzt (`TypeObjectModel`) und über `getType()` der Built-in-Daten erreichbar. (Commit `b4bf3fa978e`)
### §7.2.1 Je Typ eine Schnittstelle oder abstrakte Klasse

**Spec:** §7.2.1, S. 15 — "Each of these contains a Java interface or abstract
class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSObject.java` (`DDSObject`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_7_2_the_psm_sample_is_an_interface`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_7_2_1_every_type_of_the_omg_api_is_an_interface_or_abstract_except_the_concrete_ones_of_the_omg_source`

**Status:** `done` — CoreApiShapeTest prüft per Reflexion für die Pakete core, core.policy, core.status, core.event, domain, pub, sub, topic, type, type.builtin, type.dynamic und type.typeobject, dass jeder öffentliche Typ Schnittstelle, Enum, Annotation oder abstrakte Klasse ist. Ausgenommen sind die konkreten Klassen der OMG-Quelle: `ServiceConfigurationException`, `ServiceInitializationException`, die sechs `*Adapter`, `TypeId`, `TypeKind.Values`, `DataRepresentation.Id` und die `MemberId`-Klassen. Das Paket `org.omg.dds.rpc` ist nicht erfasst. (Commit `d23eff980f3`)
### §7.2.1 Entitäten, Werttypen, Policies und Status als Schnittstelle oder abstrakte Klasse

**Spec:** §7.2.1, S. 15 — "Each of these contains a Java interface or abstract
class for each type in the corresponding DDS module."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_7_2_1_every_type_of_the_omg_api_is_an_interface_or_abstract_except_the_concrete_ones_of_the_omg_source`

**Status:** `done` — CoreApiShapeTest prüft, dass DomainParticipant, DomainParticipantFactory, Publisher, Subscriber, Topic, DataWriter, DataReader Schnittstellen oder abstrakte Klassen sind, Time, Duration und InstanceHandle abstrakte Klassen, QosPolicy und die Policies Schnittstellen, LivelinessChangedStatus und PublicationMatchedStatus abstrakte Klassen. (Commit `5041698cee3`)
### §7.2.1 Alle Pakete in einer JAR `omgdds.jar`

**Spec:** §7.2.1, S. 15 — "All of these packages, and the types within them, are
packaged into a single JAR file, omgdds.jar (see Annex A: Java JAR Library
File)."

**Repo:**
`crates/java-omgdds/java/pom.xml`

**Tests:** —

**Status:** `n/a (informative)` — Beschreibt das Artefakt der OMG (Annex A: `omgdds.jar`). ZeroDDS baut `org.zerodds:omgdds` (`crates/java-omgdds/java/pom.xml`, ohne `finalName`, also `omgdds-<Version>.jar`; aus der POM gelesen, nicht gebaut). Das JAR enthält die Klassen unter `org.omg.dds`, `org.zerodds` und über `build-helper-maven-plugin` die Quellen aus `crates/idl-java/runtime` und `runtime/java/src/main/java`. Annex A erlaubt, die Implementierung im selben JAR auszuliefern. (Commit `5536e2abcac`)
### §7.2.1 Abstrakte Typen implementiert der Service konkret

**Spec:** §7.2.1, S. 15 — "All those types that are abstract—including
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_2_1_every_interface_of_the_type_object_package_is_implemented`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_8_3_the_type_objects_are_serializable`

**Status:** `done` — `ServiceEnvironmentImpl` und die Klassen unter `org/zerodds` (`DomainParticipantFactoryImpl`, `DomainParticipantImpl`, `PublisherImpl`, `SubscriberImpl`, `TopicImpl`, `DataWriterImpl`, `DataReaderImpl`, `SampleImpl`, `TimeImpl`, `DurationImpl`, `InstanceHandleImpl`, `WaitSetImpl`) setzen die abstrakten Typen um. `QuickstartApiTest.javapsm_7_8_2_a_byte_array_topic_moves_the_octets` erzeugt über die Umgebung Participant, Topic, Writer und Reader und liest ein `Sample` zurück. Die 26 Schnittstellen unter `org/omg/dds/type/typeobject` (`TypeObject`, `StructureType`, `UnionType`, …) setzt `TypeObjectModel` um, über ein TypeObject von XTypes 1.1 Anhang B, das in `idl/typeobject` aus dem IDL des Anhangs übernommen und mit `idlc` in einen XCDR1-Codec übersetzt ist; `TypeObjectModelTest` baut eine Bibliothek mit einem Typ jeder Art, schickt sie durch den Codec und findet Objekte aller 26 Schnittstellen wieder. (Commit `b4bf3fa978e`)
### §7.2.1 Eigene Eigenschaften in Untertypen der Implementierung erlaubt

**Spec:** §7.2.1, S. 15 — "In addition, the subtypes defined by the
implementation may expose additional implementation-specific properties and
operations; however, the nature of these, if any, is undefined."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. Die Implementierungsklassen liegen unter `org.zerodds` (z. B. `DataReaderImpl.isClosed`, die Methoden der internen Schnittstelle `InProcessBus.Endpoint`). Die Regel zum Ort der Erweiterungen steht im Item „§7.2.6 Keine Erweiterungen in `org.omg.dds` oder Unterpaketen“.
### §7.2.1 Begründung: mehrere Pakete

**Spec:** §7.2.1, S. 15 — "Design Rationale (non-normative) This PSM divides the
types it defines into multiple packages, rather than colocating them in a single
package [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.2.2 Werttypen einer anderen Implementierung annehmen

**Spec:** §7.2.2, S. 15 — "It shall be possible to pass an instance of any value
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_2_2_a_handle_of_another_implementation_is_accepted_everywhere`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_2_2_a_nil_handle_of_another_implementation_is_the_nil_handle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_2_2_a_time_of_another_implementation_is_compared_by_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_2_2_a_duration_of_another_implementation_is_accepted`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_time_of_another_implementation_is_the_source_timestamp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_foreign_handle_that_names_no_instance_of_the_writer_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_nil_handle_of_another_implementation_names_the_instance_of_the_sample`

**Status:** `done` — `InstanceHandleImpl.bytesOf` nimmt einen `InstanceHandle` einer anderen Implementierung an: nil über `isNil()` gleich 16 Nullbytes, sonst die ersten 16 Bytes von SHA-256 über Klassenname und Java-Serialisierung (Ersatz `toString()`); gleiche Fremdwerte ergeben gleiche Bytes, `equals` und `compareTo` sind total. `ModifiableTimeImpl.copyFrom` und die Writer-Operationen nehmen Fremdwerte an (`Time` und `Duration` über `getTime`/`getDuration`). `DataWriterImpl.resolve` und `checkHandle` prüfen die Nil-Eigenschaft über `isNil()`, ein Fremdhandle ohne Instanz ergibt `IllegalArgumentException` (DDS 1.4 §2.2.2.4.2.11: ungültiger Handle, BAD_PARAMETER; `javapsm_7_2_2_a_foreign_handle_that_names_no_instance_of_the_writer_is_a_bad_parameter`), ein Handle einer anderen Instanz als der des Samples `PreconditionNotMetException`. Die Fremdklassen im Test sind gegen die abstrakten Klassen der OMG-API geschrieben. (Commit `4c28bc52254`)
### §7.2.2 Samples eines fremden DataReader mit dem eigenen DataWriter schreiben

**Spec:** §7.2.2, S. 15 — "It shall be possible to read or take samples from a
DataReader provided by one DDS implementation and immediately write them using a
DataWriter provided by another DDS implementation, provided that the samples are
of a DDS type compatible with that DataWriter."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_sample_of_another_implementation_is_written_with_its_timestamp_and_handle`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_sample_of_another_implementation_is_disposed_and_unregistered`

**Status:** `done` — CoreForeignValuesTest implementiert ein `Sample` und ein `ModifiableInstanceHandle` einer anderen Implementierung und übergibt `getData()`, `getSourceTimestamp()` und `getInstanceHandle()` an `write`, `dispose` und `unregisterInstance` eines ZeroDDS-`DataWriter`. Der Reader sieht den Zeitstempel in Millisekunden aus dem Fremdwert (1 700 000 000 123 ms aus 1 700 000 000 123 000 000 ns), `dispose` ergibt `NOT_ALIVE_DISPOSED`. Ein Fremdhandle, das keine Instanz des Writers benennt, ergibt `PreconditionNotMetException`. (Commit `26684105d79`)
### §7.2.2 Kopieren beim Übergeben zwischen Implementierungen

**Spec:** §7.2.2, S. 15 — "Note that passing an object from one implementation
to another may incur a performance cost, as the “receiving” implementation may
have to copy the object in question before operating on it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.
### §7.2.2 Fremde konkrete Typen: Ausnahme oder undefiniertes Verhalten erlaubt

**Spec:** §7.2.2, S. 16 — "Otherwise, unless elsewhere noted in this
specification, a Service implementation may raise an exception or behave in an
undefined way if it encounters a concrete type defined by another party."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. ZeroDDS macht davon Gebrauch: `PublisherImpl.codecOf` wirft `UnsupportedOperationException` für ein `TypeSupport` einer anderen Implementierung, `InstanceHandleImpl.bytesOf` wirft `IllegalArgumentException`.
### §7.2.3 Typen ohne Factory im PIM: abstrakte Klasse mit statischen Factory-Methoden `new<ClassName>`

**Spec:** §7.2.3, S. 16 — "These latter types—those without PIM-defined factory
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_query_condition_of_the_reader_triggers_on_a_matching_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_2_3_the_static_factory_methods_of_the_abstract_value_types_go_through_the_spi`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_new_instance_handle_is_nil_and_is_filled_by_copy_from`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_type_support_is_registered_under_the_fully_qualified_name_of_the_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_another_scheme_than_file_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_a_negative_instant_in_any_unit_is_invalid`

**Status:** `done` — Die statischen Fabrikmethoden gehen über den SPI: `Time.newTime`, `Duration.newDuration`, `InstanceHandle.newInstanceHandle`, `QosProvider.newQosProvider`, `TypeSupport.newTypeSupport`, `KeyedString.newKeyedString` und `KeyedBytes.newKeyedBytes`; CoreSpiTest und CoreValuesTest rufen jede auf und prüfen Wert und Umgebung. `TypeSupport.newTypeSupport(Class, env)` in `org/omg/dds/type/TypeSupport.java` registrierte den Typ unter `type.getClass().getName()` (`java.lang.Class`); der Fehler stand in der OMG-Quelle, die Datei verwendet jetzt `type.getName()` (PSM §7.5.1). (Commit `7431a7d599c`)
### §7.2.3 Löschoperationen des PIM als `close` am Produkt

**Spec:** §7.2.3, S. 16 — "This PSM maps the factory deletion methods of the DDS
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_write_on_a_closed_writer_throws_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_read_and_take_on_a_closed_reader_throw_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause`

**Status:** `done` — `ContentFilteredTopicImpl.close` und `MultiTopicImpl.close` merken sich den geschlossenen Zustand (`isClosed`); `SubscriberImpl.createDataReader` verweigert eine geschlossene Beschreibung mit `AlreadyClosedException`. Eine `ContentFilteredTopic` einer anderen Implementierung ergibt jetzt `UnsupportedOperationException` (vorher `AlreadyClosedException`). (Commit `0eb300902f5`)
### §7.2.3 Schließen schließt die enthaltenen Objekte

**Spec:** §7.2.3, S. 16 — "Closing an Entity implicitly closes all of its
contained objects, if any. For example, closing a Publisher also closes all of
its contained DataWriters."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.closeContainedEntities`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_publisher_close_closes_its_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_close_contained_entities_closes_them_without_closing_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_closing_a_subscriber_closes_its_readers_and_leaves_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause`

**Status:** `done` — CoreCloseTest schließt einen `Subscriber` und einen `DomainParticipant` und prüft mit `Qos.isClosed`, dass ihre `DataReader` (und der `DataWriter`) geschlossen sind; `ContentFilteredTopic.close` und `MultiTopic.close` wirken über das Geschlossen-Merkmal. (Commit `0eb300902f5`)
### §7.2.3 Begründung: Muster `close`

**Spec:** §7.2.3, S. 16 — "Design Rationale (non-normative) The close
destruction design pattern is intended to be familiar to those developers who
have used java.io stream APIs and/or [JMS] [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.2.3 Empfehlung, `close` aufzurufen

**Spec:** §7.2.3, S. 16 — "Users of this PSM are recommended to call close once
they are finished using such heavyweight objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung an Anwendungen.
### §7.2.3 Automatisches Schließen nicht mehr benutzter Objekte erlaubt

**Spec:** §7.2.3, S. 16 — "In addition, implementations may automatically close
objects that the JRE deems to be no longer in use—for example, they may call
close() in an Object.finalize() override—subject to the following restrictions:"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.retain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.retain`)

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. ZeroDDS schließt nichts automatisch: unter `crates/java-omgdds/java/src/main/java` gibt es weder `finalize` noch `Cleaner` (Textsuche; der einzige Treffer für `WeakReference` ist `GuardConditionImpl`, die damit nichts schließt). Ein `DataReader`, den die Anwendung ohne `close()` fallen lässt, bleibt am `InProcessBus` angemeldet (aus dem Code gelesen, nicht gemessen). (Commit `166d5873d4a`)
### §7.2.3 Direkt referenziertes Objekt bleibt in Gebrauch

**Spec:** §7.2.3, S. 16 — "• Any object to which the application has a direct
reference is still in use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung des automatischen Schließens, das ZeroDDS nicht ausführt (Item davor).
### §7.2.3 Entität mit Listener bleibt in Gebrauch

**Spec:** §7.2.3, S. 16 — "• Any entity with a non-null listener is still in
use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung des automatischen Schließens, das ZeroDDS nicht ausführt.
### §7.2.3 Ausdrücklich gehaltenes Objekt bleibt in Gebrauch

**Spec:** §7.2.3, S. 16 — "• Any object that has been explicitly retained is
still in use."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.retain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.retain`)

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung des automatischen Schließens, das ZeroDDS nicht ausführt. `Entity.retain()` ist vorhanden; `EntityBase.retain` hat keine Wirkung, weil nichts automatisch geschlossen wird. (Commit `166d5873d4a`)
### §7.2.3 Ersteller eines benutzten Objekts bleibt in Gebrauch

**Spec:** §7.2.3, S. 16 — "• The creator of any object that is still in use is
itself still in use."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung des automatischen Schließens, das ZeroDDS nicht ausführt.
### §7.2.4 Operationen von DataReader und DataWriter reentrant

**Spec:** §7.2.4, S. 16 — "All DataReader and DataWriter operations shall be
reentrant."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.enable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::javapsm_7_2_4_a_callback_does_not_hold_the_reader_monitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::javapsm_7_2_4_a_callback_does_not_hold_the_writer_monitor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReentrancyTest.java::javapsm_7_2_4_concurrent_enable_registers_the_endpoint_once`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::javapsm_7_2_4_several_threads_write_take_and_register_on_one_writer_and_reader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::javapsm_7_2_4_closing_a_writer_while_others_write_leaves_only_already_closed`

**Status:** `done` — CoreConcurrencyTest ruft `write`, `registerInstance`, `take` und `close` aus mehreren Threads an einem `DataWriter` und `DataReader` auf: vier Writer-Threads zu je 50 Samples, zwei Registrierungs-Threads, ein `take`-Thread; die Summe der entnommenen Samples ist 200. Ein zweiter Test schließt den Writer, während vier Threads schreiben und ein Thread `take` aufruft; es bleibt nur `AlreadyClosedException`. Der Code ist unverändert (das `qos`-Feld ist `volatile`). (Commit `ac048fe0dc7`)
### §7.2.4 Operationen von Topic, Publisher, Subscriber und DomainParticipant reentrant, außer `close`

**Spec:** §7.2.4, S. 16 — "All Topic (and other TopicDescription extension
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::javapsm_7_2_4_several_threads_create_entities_and_set_the_qos_of_one_publisher_and_subscriber`

**Status:** `done` — Das Feld `qos` von `PublisherImpl`, `SubscriberImpl`, `TopicImpl`, `DataWriterImpl` und `DataReaderImpl` ist `volatile`. CoreConcurrencyTest erzeugt aus acht Threads je zehnmal Writer, Reader und Topic an einem Publisher, Subscriber und Participant und setzt dabei das QoS von Publisher und Subscriber; alle 240 Objekte sind verschieden, das übrig gebliebene QoS ist das eines der Threads. (Commit `ac048fe0dc7`)
### §7.2.4 Operationen von ServiceEnvironment und DomainParticipantFactory reentrant, außer `DomainParticipantFactory.close`

**Spec:** §7.2.4, S. 16 — "All ServiceEnvironment and DomainParticipantFactory
operations shall be reentrant with the exception that
DomainParticipantFactory.close may not be called on a given object concurrently
with any other call of any method on that object or on any contained object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_2_4_several_threads_get_one_participant_factory_of_an_environment`

**Status:** `done` — `ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory` legt die Factory mit double-checked locking an (`volatile`-Feld, `synchronized`). CoreSpiTest ruft `DomainParticipantFactory.getInstance(env)` in 20 Runden aus je acht gleichzeitig gestarteten Threads auf und prüft, dass genau eine Instanz zurückkommt. (Commit `7431a7d599c`)
### §7.2.4 Operationen von WaitSet und Condition reentrant

**Spec:** §7.2.4, S. 16 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception that
their close methods may not be called on a given object concurrently with any
other call of any method on that object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl.setTriggerValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/ReadConditionImpl.java` (`ReadConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_second_thread_waiting_on_the_same_wait_set_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreConcurrencyTest.java::javapsm_7_2_4_conditions_are_attached_and_detached_while_a_thread_waits`

**Status:** `done` — CoreConcurrencyTest hängt Conditions an und ab, ruft `getConditions` und `StatusCondition.setEnabledStatuses` in 100 Runden auf, während ein anderer Thread in `waitForConditions` wartet; danach löst `GuardCondition.setTriggerValue(true)` das Warten aus und die aktive Menge enthält die Condition. Der Code ist unverändert. (Commit `fc054ce9926`)
### §7.2.4 Im Listener nur die auslösende Entität aufrufen

**Spec:** §7.2.4, S. 17 — "Code within a DDS listener callback may not safely
call any method on any DDS Entity but the one on which the status change
occurred."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:** —

**Status:** `n/a (informative)` — Einschränkung für den Code der Anwendung im Callback; die Spec verlangt hier nichts von der Implementierung. ZeroDDS ruft die Callbacks ohne Monitor des Readers oder Writers auf (`DataReaderImpl.fireDataAvailable`), sodass Aufrufe aus dem Callback zulässig bleiben; `ReentrancyTest.javapsm_7_2_4_a_callback_does_not_hold_the_reader_monitor` prüft das für `close()` aus einem anderen Thread. (Commit `166d5873d4a`)
### §7.2.4 Methoden von Werttypen dürfen nicht reentrant sein

**Spec:** §7.2.4, S. 17 — "Any method of any value type may be non-reentrant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.
### §7.2.4 Stärkere Zusagen erlaubt

**Spec:** §7.2.4, S. 17 — "A vendor may choose to provide stronger guarantees
than the rules above, but if so, those guarantees are unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.
### §7.2.4 Begründung: `close` und Listener

**Spec:** §7.2.4, S. 17 — "Design Rationale (non-normative) • Objects that are
likely to “own” mutexes within their implementation need not permit close
invocations currently with other method invocations. [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.2.5 Namen in camelCase

**Spec:** §7.2.5, S. 17 — "This PSM maps the underscore-formatted names of the
DDS PIM and IDL PSM (such as get_qos) into conventional Java “camel-case” names
(such as getQos)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.registerInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.lookupInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getRequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.getSubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.createParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_status_is_readable_without_a_listener`

**Status:** `done` — Die Operationen unter `org/omg/dds` tragen die PIM-Namen in camelCase; eine Suche nach Methodennamen mit Unterstrich findet keine. Die Tests rufen `registerInstance`, `lookupInstance`, `getRequestedDeadlineMissedStatus`, `getSubscriptionMatchedStatus`, `getPublicationMatchedStatus` und `createParticipant` auf. (Commit `c060b572340`)
### §7.2.5 Mutatoren `set<PropertyName>` geben das Objekt zurück

**Spec:** §7.2.5, S. 17 — "Properties defined by the DDS PIM are expressed as
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`

**Status:** `done` — CoreSpiTest verkettet `KeyedString.setKey(..).setValue(..)` und `KeyedBytes.setKey(..).setValue(..)` (beide liefern das Objekt zurück) und prüft Kopie- und `copyFrom`-Verhalten. Die `void`-Mutatoren folgen der OMG-Quelle (Annex B), die vom Text des §7.2.5 abweicht. (Commit `5041698cee3`)
### §7.2.5 Accessor `get<PropertyName>()` für unveränderliche Werte und Verweise

**Spec:** §7.2.5, S. 17 — "Accessors for properties that are either of
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`

**Status:** `done` — Die Accessoren für Zeichenketten, Zahlen und Verweise heißen `get<PropertyName>` und nehmen kein Argument (`DomainParticipant.getDomainId`, `Topic.getName`, `DataReader.getListener`, `Sample.getData`; Deklarationen gelesen). `ListenerTest.javapsm_2_reader_listener_fires_on_match_and_on_every_sample` ruft `getListener`, `getCurrentCount` und `getTotalCount`, `QuickstartApiTest.javapsm_7_8_2_a_byte_array_topic_moves_the_octets` `getData`. (Commit `5041698cee3`)
### §7.2.5 Accessor `get<PropertyName>()` für Policies, Status, Zeit und Samples

**Spec:** §7.2.5, S. 17 — "Accessors for properties that are either of
unmodifiable objects [...] are named get<PropertyName>."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History.getKind`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History.getDepth`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus.getTotalCount`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time.getTime`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getSampleState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_status_is_readable_without_a_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::rpc_7_11_2_2_6_an_rpc_sample_is_a_psm_sample`

**Status:** `done` — Policies, Status, `Time`, `Duration`, `InstanceHandle` und `Sample` haben Accessoren `get<PropertyName>` (`History.getKind`, `getDepth`, `SubscriptionMatchedStatus.getTotalCount`, `getCurrentCountChange`, `Time.getTime`, `Sample.getSampleState`, `getViewState`, `getInstanceState`, `getInstanceHandle`, `getSourceTimestamp`). Die Tests rufen diese Accessoren auf und prüfen die Werte. (Commit `5041698cee3`)
### §7.2.5 Accessor für veränderliche Eigenschaften mit vorab angelegtem Objekt

**Spec:** §7.2.5, S. 17 — "Accessors for properties that are of mutable types,
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_get_current_time_fills_the_container_or_returns_a_new_one`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_writer_fills_the_key_fields_of_the_holder_only`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_reader_fills_the_key_fields_of_the_holder_only`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_handle_of_no_instance_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_lookup_instance_fills_the_handle_it_is_given`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_the_key_holder_of_a_built_in_keyed_type_keeps_its_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreKeyValueTest.java::javapsm_7_2_5_a_type_without_a_key_has_no_key_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_5_lookup_instance_fills_a_modifiable_handle_of_another_implementation`

**Status:** `done` — `DataWriterImpl.getKeyValue(T, InstanceHandle)` und `DataReaderImpl.getKeyValue(T, InstanceHandle)` überschreiben nur die Schlüsselfelder von `keyHolder` und geben ihn zurück (`TopicTypeSupport.copyKey`, Standard `KeyFields.copy` über `@Key`; ohne `@Key`-Feld werden alle Felder kopiert; KeyedString/KeyedBytes kopieren `key`). Der Reader merkt sich je Instanz das erste Sample mit Daten (`Instance.keySample`). Ein Handle ohne Instanz ergibt `IllegalArgumentException`. `lookupInstance(ModifiableInstanceHandle, T)` füllt das übergebene Handle, auch ein fremdes. (Commit `f8e64a1ab3d`)
### §7.2.6 Erweiterungen über eigene Schnittstellen aus den Factory-Methoden erlaubt

**Spec:** §7.2.6, S. 17 — "Implementation-specific extensions to the types
specified by this PSM are by definition unspecified. However, implementations
may provide such a capability by providing extended implementation-specific
interfaces and returning instances of these interfaces from the specified
factory methods."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. ZeroDDS liefert aus den Factory-Methoden Klassen unter `org.zerodds` (z. B. `DomainParticipantImpl`), ohne eigene öffentliche Zusatzschnittstellen.
### §7.2.6 Keine Erweiterungen in `org.omg.dds` oder Unterpaketen

**Spec:** §7.2.6, S. 17 — "Implementations shall not place their extensions, if
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`,
`crates/rpc/tests/java_rpc_runtime.rs::rpc_7_2_1_the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_7_2_6_an_interface_under_the_psm_mapping_compiles`

**Status:** `done` — Die Dateien unter `crates/java-omgdds/java/src/main/java/org/omg/dds` sind die Dateien der OMG-Quelle, ohne Ausnahme; `CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source` prüft die Menge der Dateien und ihren SHA-256 bei jedem Lauf, ohne dass die Quelle im Arbeitsbaum liegen muss. Die Bindung legt keine eigene Klasse in `org.omg.dds` ab. Die Implementierungsklassen von DDS-RPC (`RequesterImpl`, `ReplierImpl`, `ServerImpl`, `ClientParamsImpl`, `ServiceParamsImpl`, `RequesterParamsImpl`, `ReplierParamsImpl`, `Matching`, `ZeroDdsParticipant`, `RawTypeSupport`) liegen in `org.zerodds.ddsrpc` (`runtime/java/src/main/java/org/zerodds/ddsrpc`); der Name `org.zerodds.rpc` gehört der IDL-Java-Laufzeit. Die Schnittstellen und Fabrikmethoden in `org.omg.dds.rpc` (`RequesterParams.create`, `RPCRuntime`) rufen sie auf; der RPC-Codegenerator schreibt `extends org.zerodds.ddsrpc.RequesterImpl<…>` (`interface_java.rs`, `RPC_IMPL`). Die Tests der RPC-Bereiche (`java_rpc_runtime`, `java_enhanced_codec`, `type_pair_java`, `interface_bindings`, `java_psm_mapping`) lesen die Klassen aus dem neuen Pfad. (Commit `ed38a660164`)

---

## §7.3 Infrastructure Module

### §7.3 Zwei Pakete `org.omg.dds.core` und `org.omg.dds.core.policy`

**Spec:** §7.3, S. 18 — "This PSM realizes the Infrastructure Module from the
DDS specification with two packages: org.omg.dds.core and
org.omg.dds.core.policy. The latter contains all QoS policy classes, since a
given QoS policy may apply to multiple DDS Entity types. The former contains all
other Infrastructure types, including for example Entity and Condition base
interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_3_1_dds_object_carries_the_one_operation_the_clause_names`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`

**Status:** `done` — `org.omg.dds.core` enthält `DDSObject`, `Entity`, `ServiceEnvironment`, `InstanceHandle`, `Time`, `Duration`, `Condition`, `WaitSet`, `GuardCondition`, `StatusCondition`, `EntityQos`, `QosProvider` und die Ausnahmen; `org.omg.dds.core.policy` die Policies mit `PolicyFactory`. Die Status liegen in `org.omg.dds.core.status`, die Ereignisse in `org.omg.dds.core.event`, wie in den OMG-Quellen. Die Tests importieren und benutzen die Typen aus diesen Paketen; `RpcInheritanceTest` prüft den Paketnamen von `DDSObject`. (Commit `5041698cee3`)
### §7.3 Begründung: zwei Pakete, Name „core“

**Spec:** §7.3, S. 18 — "Design Rationale (non-normative) These two packages
have been made distinct from one another for two reasons: [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.3.1 `ServiceEnvironment` als Wurzel aller Objekte, Erzeugen über ein internes SPI

**Spec:** §7.3.1, S. 18 — "A ServiceEnvironment object represents an
instantiation of a Service implementation within a JVM. It is the “root” for all
other DDS objects and assists in their creation by means of an internal
service-provider interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_new_instance_handle_is_nil_and_is_filled_by_copy_from`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_the_invalid_time_and_a_negative_time_of_the_spi_are_invalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_another_scheme_than_file_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_type_support_is_registered_under_the_fully_qualified_name_of_the_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_every_entity_answers_with_the_environment_it_was_created_in`

**Status:** `done` — CoreSpiTest ruft `newInstanceHandle`, `invalidTime`, `newQosProvider`, `newTypeSupport`, `newKeyedString` und `newKeyedBytes` des SPI auf und prüft mit `assertSame(env, x.getEnvironment())` Participant, Publisher, Subscriber, Topic, Writer, Reader, Status-Condition, Zeit und Sample einer von `new ServiceEnvironmentImpl()` erzeugten Umgebung. `DataWriterImpl` und `DataReaderImpl` übernehmen die Umgebung ihres Parent (vorher `Values.getDefaultEnvironment()`). (Commit `26684105d79`)
### §7.3.1 Alle zustandsbehafteten Typen implementieren `DDSObject` (`getEnvironment`)

**Spec:** §7.3.1, S. 18 — "All stateful types in this PSM implement an interface
DDSObject, through a getServiceEnvironment method on which they can provide
access to the ServiceEnvironment from which they are ultimately derived."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSObject.java` (`DDSObject.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_3_1_dds_object_carries_the_one_operation_the_clause_names`

**Status:** `done` — Die Operation heißt in den OMG-Quellen `getEnvironment`. `DDSObject` hat genau diese eine Operation; der Test prüft Name, Rückgabetyp `ServiceEnvironment`, Parameterzahl 0 und das Paket `org.omg.dds.core`. Welche Typen die Schnittstelle erweitern, behandelt das Item „§7.3.1 Entitäten, Factory und Werttypen implementieren `DDSObject`“. (Commit `5041698cee3`)
### §7.3.1 Entitäten, Factory und Werttypen implementieren `DDSObject`

**Spec:** §7.3.1, S. 18 — "All stateful types in this PSM implement an interface
DDSObject [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getEnvironment`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl.getEnvironment`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_policies_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_status_is_serializable_and_keeps_its_counts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_every_entity_answers_with_the_environment_it_was_created_in`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_dds_exception_is_a_dds_object_and_unchecked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_an_exception_belongs_to_the_environment_it_was_created_in`

**Status:** `done` — `DDSException` hat `implements DDSObject` wie die OMG-Quelle; die konkreten Ausnahmen in `org.zerodds.core` liefern über `getEnvironment()` die beim Erzeugen übergebene oder die Standard-Umgebung. CoreSpiTest prüft `assertSame(env, x.getEnvironment())` für Participant, Publisher, Subscriber, Topic, Writer und Reader; CoreExceptionsTest prüft es für die Ausnahmen. (Commit `b1b698778d6`)
### §7.3.1 `ServiceEnvironment` implementiert `DDSObject` und gibt `this` zurück

**Spec:** §7.3.1, S. 18 — "(ServiceEnvironment itself implements this interface;
a ServiceEnvironment always returns this from its getServiceEnvironment
operation.)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.getEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_3_1_a_service_environment_returns_itself`

**Status:** `done` — `ServiceEnvironment implements DDSObject`, `getEnvironment()` ist `final` und gibt `this` zurück. Der Test prüft die Zuweisbarkeit an `DDSObject` und dass eine Unterklasse (`RPCRuntime`) sich selbst zurückgibt. (Commit `5041698cee3`)
### §7.3.1 Ohne statischen Zustand möglich, mehrere Implementierungen nebeneinander

**Spec:** §7.3.1, S. 18 — "The ServiceEnvironment class allows implementations
to avoid the presence of static state, if desired. It also allows multiple DDS
implementations—or multiple versions of the “same” implementation—to potentially
coexist within the same Java run-time environment."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis und Zweck. ZeroDDS hält statischen Zustand: `ServiceEnvironmentImpl.DEFAULT`, `InProcessBus.INSTANCE`, `DurabilityService.INSTANCE`, `RtpsParticipant.BY_DOMAIN`, `ReflectionTypeSupport.CACHE`. Zwei `ServiceEnvironment` im selben JVM teilen damit Bus und Durability-Daten (aus dem Code gelesen, nicht gemessen). `DomainParticipantFactory` ist dagegen je Umgebung getrennt (Item „§7.3.1 `ServiceEnvironment` als Wurzel aller Objekte“).
### §7.3.1 Erster Schritt der Anwendung: ein `ServiceEnvironment` anlegen

**Spec:** §7.3.1, S. 18 — "A DDS application’s first step is to instantiate a
ServiceEnvironment, which represents the DDS implementation that it will use.
From there, it can create all of its additional DDS objects."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibt den Ablauf einer Anwendung. Bei ZeroDDS ist der Schritt nötig: `DomainParticipantFactory.getInstance(ServiceEnvironment)` hat in den OMG-Quellen kein Gegenstück ohne Argument und wirft bei `null` eine `NullPointerException` (`QuickstartApiTest.javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception`).
### §7.3.1 `ServiceEnvironment` ist abstrakt

**Spec:** §7.3.1, S. 18 — "The ServiceEnvironment class is abstract."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_the_service_environment_is_abstract`

**Status:** `done` — CoreSpiTest prüft `Modifier.isAbstract(ServiceEnvironment.class.getModifiers())`, dass `getSPI` abstrakt ist und dass `ServiceEnvironmentImpl` eine konkrete Unterklasse ist. (Commit `5041698cee3`)
### §7.3.1 `createInstance` sucht die Unterklasse über eine Systemeigenschaft

**Spec:** §7.3.1, S. 18 — "To avoid compile-time dependencies on concrete
ServiceEnvironment implementations, an application can instantiate a
ServiceEnvironment by means of a static createInstance method on the
ServiceEnvironment class. This method looks up a concrete ServiceEnvironment
subclass using a Java system property containing the name of that subclass."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.createInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_an_unknown_service_class_is_a_service_configuration_exception`

**Status:** `done` — `createInstance(ClassLoader)` liest die Systemeigenschaft `org.omg.dds.serviceClassName`; `createInstance(String, Map, ClassLoader)` liest eine frei benannte Eigenschaft und wirft `ServiceConfigurationException`, wenn sie fehlt oder die Klasse nicht ladbar ist. Die Datei ist byte-gleich zur OMG-Quelle. Der erste Test setzt die Eigenschaft auf `org.zerodds.ServiceEnvironmentImpl` und erhält eine nutzbare Umgebung, der zweite prüft die unbekannte Klasse; den Fall einer fehlenden Eigenschaft prüft kein Test. (Commit `5041698cee3`)
### §7.3.1 Unterklasse mit eigenem Namen von der Implementierung

**Spec:** §7.3.1, S. 18 — "This subclass must be provided by implementers and
will therefore have an implementation-specific name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`

**Status:** `done` — `org.zerodds.ServiceEnvironmentImpl extends ServiceEnvironment` hat einen öffentlichen Konstruktor ohne Argument und einen mit `Map`. Der Test lädt die Klasse über diesen Namen mit `createInstance` und benutzt das Ergebnis; er prüft den Laufzeittyp nicht ausdrücklich. (Commit `7d991374fd4`)
### §7.3.1 Factory-Methode für DynamicTypeFactory

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: DynamicTypeFactory, WaitSet, GuardCondition, TypeSupport,
Time, Duration, and InstanceHandle."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.getTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getTypeFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_type_descriptor_is_serializable_and_carries_the_types_it_names`

**Status:** `done` — `ServiceProviderInterface.getTypeFactory` liefert die `DynamicTypeFactoryImpl` der Umgebung, `DynamicTypeFactory.getInstance(env)` ruft sie. Der erste Test ruft `getTypeFactory().getPrimitiveType(INT_32_TYPE)`, der zweite legt über `DynamicTypeFactory.getInstance(ENV)` einen Sequenztyp an. (Commit `7d991374fd4`)
### §7.3.1 Factory-Methode für WaitSet

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] WaitSet [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newWaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_wait_with_a_timeout_ends_in_a_timeout_exception`

**Status:** `done` — `ServiceProviderInterface.newWaitSet` und `WaitSet.newWaitSet(env)` liefern ein `WaitSetImpl`. Der erste Test prüft, dass `newWaitSet()` ein Objekt liefert; `WaitSetTest` benutzt `WaitSet.newWaitSet(ENV)` und prüft Anhängen, Abhängen, Warten mit Zeitgrenze (`TimeoutException`) und Aufwecken durch eine Bedingung. (Commit `fc054ce9926`)
### §7.3.1 Factory-Methode für GuardCondition

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] GuardCondition [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition.newGuardCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/GuardConditionImpl.java` (`GuardConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`

**Status:** `done` — `ServiceEnvironment.ServiceProviderInterface.newGuardCondition` liefert ein `GuardConditionImpl`; `GuardCondition.newGuardCondition(env)` delegiert dorthin. Die Tests prüfen, dass die Fabrik ein Objekt liefert, dass die Bedingung zunächst `false` ist und dass ein Setzen aus einem anderen Thread einen blockierten `WaitSet` beendet. (Commit `fc054ce9926`)
### §7.3.1 Factory-Methode für TypeSupport

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] TypeSupport [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_type_support_is_registered_under_the_fully_qualified_name_of_the_type`

**Status:** `done` — `org/omg/dds/type/TypeSupport.newTypeSupport(Class, ServiceEnvironment)` ruft die Quelle mit `type.getClass().getName()` auf, das ist „java.lang.Class“ für jeden Typ; `ServiceEnvironmentImpl.newTypeSupport` liest diesen Namen als „kein Name angegeben“ und registriert den Typ unter `type.getName()`. CoreSpiTest erzeugt ein TypeSupport über `TypeSupport.newTypeSupport` und `ServiceProviderInterface.newTypeSupport` und prüft den Namen (`[B` für `byte[]`, der angegebene Name sonst). Die Datei ist die der OMG-Quelle (`CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`). (Commit `7431a7d599c`)
### §7.3.1 Factory-Methode für Time

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] Time [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newTime`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.invalidTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.split`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.shift`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_get_current_time_fills_the_container_or_returns_a_new_one`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_a_negative_instant_in_any_unit_is_invalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_the_invalid_time_equals_itself_and_orders_before_every_valid_time`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_add_and_subtract_move_the_instant_and_keep_it_normalised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_the_invalid_time_and_a_negative_time_of_the_spi_are_invalid`

**Status:** `done` — `TimeImpl.split` liefert für jeden negativen Wert in jeder Einheit den ungültigen Zeitpunkt (Sekunden -1, Nanosekunden 0); `ModifiableTimeImpl.add/subtract` rechnen über `TimeImpl.shift` mit Normalisierung und Ergebnis ungültig vor der Epoche. Tests: `newTime(-5, u)` für alle `TimeUnit`-Werte ungültig, `invalidTime()` ungültig, `newTime(0, NANOSECONDS)` gültig. (Commit `4c28bc52254`)
### §7.3.1 Factory-Methode für Duration

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] Duration [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newDuration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.infiniteDuration`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.zeroDuration`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.of`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_the_infinite_duration_is_the_largest_and_long_max_in_any_unit_is_infinite`

**Status:** `done` — `newDuration(long, TimeUnit)`, `infiniteDuration()` und `zeroDuration()` liefern `DurationImpl` der Umgebung; `DurationImpl.of` setzt die Größe `Long.MAX_VALUE` auf unendlich. Die Tests prüfen `newDuration(7, SECONDS)` nach Serialisierung, `isInfinite()` und `isZero()` der beiden Fabriken sowie `getDuration(SECONDS) == Long.MAX_VALUE` für unendlich. (Commit `819bfcb46cb`)
### §7.3.1 Factory-Methode für InstanceHandle

**Spec:** §7.3.1, S. 18 — "ServiceEnvironment provides factory methods for the
following objects: [...] and InstanceHandle."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newInstanceHandle`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.nilHandle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newInstanceHandle`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_handles_with_the_same_octets_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_3_1_a_new_instance_handle_is_nil_and_is_filled_by_copy_from`

**Status:** `done` — CoreSpiTest erzeugt über `newInstanceHandle()` ein `ModifiableInstanceHandle`, prüft `isNil()`, füllt es mit `copyFrom` (Wert 42 im vierten Byte) und prüft Gleichheit, Nicht-Nil und verschiedene Objekte je Aufruf. (Commit `819bfcb46cb`)
### §7.3.1 Hilfsfunktionen `allStatusKinds` und `noStatusKinds`

**Spec:** §7.3.1, S. 18 — "It also provides helper functions allStatuses and
noStatuses to create special instances of Status objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.allStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.noStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.allStatusKinds`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`

**Status:** `done` — Die Hilfsfunktionen heißen `allStatusKinds()` und `noStatusKinds()` und liegen im `ServiceProviderInterface`. `allStatusKinds` enthält die 13 Statusklassen (`ServiceEnvironmentImpl.allStatusKinds`), `noStatusKinds` ist leer. Der Test prüft Größe 13 und die leere Menge; die Zugehörigkeit der einzelnen Klassen prüft er nicht. (Commit `7d991374fd4`)
### §7.3.2 ReturnCode_t als Java-Ausnahmen

**Spec:** §7.3.2, S. 18 — "The PSM maps the ReturnCode_t type from the DDS PIM
into a combination of standard Java exceptions (where their semantics match
those expressed in the PIM) and new exception classes defined by this PSM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/AlreadyClosedException.java` (`AlreadyClosedException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/NotEnabledException.java` (`NotEnabledException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/PreconditionNotMetException.java` (`PreconditionNotMetException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_write_on_a_closed_writer_throws_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_not_enabled_writer_refuses_to_write`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_null_argument_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_wait_with_a_timeout_ends_in_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_participant_data_carries_the_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_the_service_provides_a_concrete_class_for_every_table_7_1_exception_and_for_retcode_error`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_a_sample_the_encoder_rejects_fails_with_retcode_error`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_an_encoding_beyond_the_limit_fails_with_out_of_resources`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_refuses_a_change_of_an_immutable_policy_and_keeps_its_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_history_depth_above_max_samples_per_instance_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_topic_of_another_implementation_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_resume_publications_needs_a_suspend_publications_and_the_calls_nest`

**Status:** `done` — Die Zuordnung der Tabelle 7.1 über Ausnahmeklassen (`org.omg.dds.core.*Exception`, konkrete Klassen in `org.zerodds.core`) ist für alle Zeilen mit einem Test belegt: ERROR, UNSUPPORTED, IMMUTABLE_POLICY, INCONSISTENT_POLICY, OUT_OF_RESOURCES, PRECONDITION_NOT_MET, ALREADY_DELETED, NOT_ENABLED, BAD_PARAMETER. ILLEGAL_OPERATION hat die Klasse und den Reflexionstest, aber keine Operation, die sie auslöst (DDS 1.4 nennt keine). TIMEOUT: siehe Item „RETCODE_TIMEOUT“ (partial). (Commit `b1b698778d6`)
### §7.3.2 Alle Ausnahmen ungeprüft außer `TimeoutException`

**Spec:** §7.3.2, S. 18 — "With the exception of
java.util.concurrent.TimeoutException, all exceptions are unchecked (that is,
they extend java.lang.RuntimeException directly or indirectly)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceConfigurationException.java` (`ServiceConfigurationException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceInitializationException.java` (`ServiceInitializationException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_only_timeout_exception_is_checked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_dds_exception_is_a_dds_object_and_unchecked`

**Status:** `done` — CoreExceptionsTest prüft für alle sieben Klassen der Tabelle 7.1, `DDSException`, `ServiceConfigurationException` und `ServiceInitializationException`, dass sie `RuntimeException` erweitern, und dass `TimeoutException` (`java.util.concurrent`) geprüft ist und von `WaitSet.waitForConditions`, `DataWriter.write` und `DomainParticipant.findTopic` deklariert wird. (Commit `7f18142b5e1`)
### §7.3.2 Eigene Ausnahmen mit Basis `DDSException` in `org.omg.dds.core`, abstrakt

**Spec:** §7.3.2, S. 19 — "The exception classes defined by this PSM extend the
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_only_timeout_exception_is_checked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_every_table_7_1_exception_is_an_abstract_dds_exception_of_org_omg_dds_core`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_the_exceptions_keep_the_serial_version_uid_of_the_omg_source`

**Status:** `done` — CoreExceptionsTest prüft per Reflexion, dass die sieben Klassen der Tabelle 7.1 abstrakt sind, in `org.omg.dds.core` liegen und `DDSException` direkt erweitern, dass `DDSException` abstrakt ist und dass `org.zerodds.core` zu jeder eine konkrete Klasse hat. Die `serialVersionUID` aller Ausnahmen der OMG-API entspricht jetzt der OMG-Quelle (vorher `-1L` bis `-5L` an fünf Klassen und ein anderer Wert an `ServiceInitializationException`). (Commit `b1b698778d6`)
### §7.3.2 Table 7.1: RETCODE_OK

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_OK | Normal return; no exception."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — `DataWriter.write` ist `void`; ein erfolgreicher Aufruf kehrt ohne Ausnahme zurück. Der Test schreibt zwei Samples ohne Ausnahme und nimmt beide am Reader. (Commit `f31b6fa11b5`)
### §7.3.2 Table 7.1: RETCODE_NO_DATA

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_NO_DATA | An informational state
(e.g., a Boolean result) attached to a normal return; no exception."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_volatile_does_not_replay_to_a_late_joiner`

**Status:** `done` — Ohne Samples liefert `take(List)` eine leere Liste ohne Ausnahme; es gibt kein `ReturnCode.NO_DATA`. Der Test nimmt an einem laufenden Reader ohne Daten und prüft Länge 0. (Commit `f31b6fa11b5`)
### §7.3.2 Table 7.1: RETCODE_ERROR

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_ERROR | DDSException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/ErrorException.java` (`ErrorException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.serialize`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_a_sample_the_encoder_rejects_fails_with_retcode_error`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_the_service_provides_a_concrete_class_for_every_table_7_1_exception_and_for_retcode_error`

**Status:** `done` — `org.zerodds.core.ErrorException` ist die konkrete Klasse zu `DDSException` (RETCODE_ERROR). `DataWriterImpl.serialize` meldet einen Encoder, der das Sample ablehnt (`XcdrException`), als `ErrorException` mit der Ursache. Die Tests: Klasse konkret mit Oberklasse `DDSException`; ein Writer mit einem solchen Typ-Support wirft `ErrorException` und bleibt benutzbar. (Commit `26684105d79`)
### §7.3.2 Table 7.1: RETCODE_BAD_PARAMETER

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_BAD_PARAMETER |
java.lang.IllegalArgumentException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/HistoryImpl.java` (`HistoryImpl.withDepth`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_5_3_history_depth_must_be_non_negative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_null_argument_is_a_bad_parameter`

**Status:** `done` — Eine negative `History`-Tiefe, ein Handle falscher Länge und ein `null`-Handle bei `getMatchedSubscriptionData` werfen `IllegalArgumentException`. Die drei Tests prüfen genau diese Ausnahme; weitere Tests prüfen sie für ungültige Filterausdrücke, Typdeskriptoren und MultiTopic-Ausdrücke (`ContentFilterTest`, `DynamicTypeReflectionTest`, `MultiTopicTest`). (Commit `f31b6fa11b5`)
### §7.3.2 Table 7.1: RETCODE_BAD_PARAMETER bei `null`

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_BAD_PARAMETER |
java.lang.IllegalArgumentException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getMatchedSubscriptionData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getMatchedPublicationData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/Args.java` (`Args.require`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_null_argument_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreForeignValuesTest.java::javapsm_7_2_2_a_foreign_handle_that_names_no_instance_of_the_writer_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/NullArgumentTest.java::javapsm_7_3_2_a_null_argument_is_illegal_argument_exception_and_never_null_pointer_exception`

**Status:** `done` — `getMatchedSubscriptionData(null)` und `getMatchedPublicationData(null)` werfen `IllegalArgumentException`. `DomainParticipantFactory.getInstance(null)` wirft `NullPointerException` (Quelle der OMG-API: `env.getSPI()`); `QuickstartApiTest.javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception` legt das fest. Ein Handle, der keine Instanz des Writers benennt, ist `IllegalArgumentException` (`CoreForeignValuesTest::javapsm_7_2_2_a_foreign_handle_that_names_no_instance_of_the_writer_is_a_bad_parameter`, DDS 1.4 §2.2.2.4.2.11). Jede Operation von DomainParticipant, Publisher, Subscriber, Topic, DataWriter und DataReader mit einem Objektargument wirft bei `null` `IllegalArgumentException` (Table 7.1: RETCODE_BAD_PARAMETER) und keine `NullPointerException` (`org.zerodds.core.Args.require` am Eingang der Operationen: Namen, Typen, QoS, Topics, Handles, Zeitstempel, Einheiten, Bedingungen, Samples). `null` behält eine Bedeutung, wo DDS oder die PSM sie vergibt: Listener und Statusmenge „keine“, ein Schlüsselhalter oder eine Liste, die neu angelegt werden, der Nil-Handle bei `write`, `dispose` und `unregisterInstance`, `null` als Selektor von `read` und `take`. `NullArgumentTest` ruft über Reflection jede Operation dieser sechs Schnittstellen je Parameter mit `null` auf (109 Aufrufe) und verlangt `IllegalArgumentException`. (Commit `eb7377e1f7e`)
### §7.3.2 Table 7.1: RETCODE_TIMEOUT

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_TIMEOUT |
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_wait_with_a_timeout_ends_in_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_find_topic_returns_the_topic_and_times_out_otherwise`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_2_a_sample_nobody_acknowledges_is_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterWireAckTest.java::javapsm_7_3_2_a_write_beyond_max_samples_waits_for_acknowledgments_and_times_out`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderWireHistoricalDataTest.java::javapsm_2_the_call_is_a_timeout_exception_while_the_writer_announced_nothing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_keep_all_with_max_samples_per_instance_times_out_for_transient_local`

**Status:** `done` — `java.util.concurrent.TimeoutException` wird geworfen von `WaitSetImpl.waitForConditions(Duration)`, `DomainParticipantImpl.findTopic`, `DataWriterImpl.waitForAcknowledgments` und `PublisherImpl.waitForAcknowledgments` (Reader in anderen Prozessen quittieren per ACKNACK), `DataReaderImpl.waitForHistoricalData` (RTPS-Zweig) und von `write`, wenn RESOURCE_LIMITS erreicht sind und `max_blocking_time` abläuft: bei einem TRANSIENT_LOCAL-Writer an den behaltenen Samples, bei einem zuverlässigen KEEP_ALL-Writer mit `max_samples` an den Samples, die Reader in anderen Prozessen noch nicht quittiert haben (`WireWriter.awaitHistorySpace`). Ein BEST_EFFORT-Writer wartet nicht und meldet `OutOfResourcesException`. (Commit `1d0691890fa`)
### §7.3.2 Table 7.1: RETCODE_UNSUPPORTED

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_UNSUPPORTED |
java.lang.UnsupportedOperationException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.codecOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_topic_of_another_implementation_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_topic_description_of_another_implementation_is_unsupported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_another_scheme_than_file_is_unsupported`

**Status:** `done` — `UnsupportedOperationException` ist für ein anderes URI-Schema als `file` (`QosProviderImpl`), für ein Topic und für eine TopicDescription einer anderen Implementierung (`PublisherImpl.codecOf`, `SubscriberImpl.codecOf`) und für eine ContentFilteredTopic einer anderen Implementierung getestet. `SubscriberImpl.createDataReader` warf für Letztere `AlreadyClosedException`; es ist jetzt `UnsupportedOperationException`. (Commit `b315020c678`)
### §7.3.2 Table 7.1: RETCODE_ALREADY_DELETED

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_ALREADY_DELETED |
AlreadyClosedException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.take`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/AlreadyClosedException.java` (`AlreadyClosedException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_write_on_a_closed_writer_throws_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_read_and_take_on_a_closed_reader_throw_already_closed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`


**Status:** `done` — Nach `close()` werfen `write`, `read`, `take`, `createDataWriter`, `createDataReader`, `createPublisher` und `createTopic` `AlreadyClosedException` (Unterklasse von `org.omg.dds.core.AlreadyClosedException`). Die Tests prüfen die Ausnahme für geschlossene Writer, Reader, Publisher, Subscriber und Participants. (Commit `f31b6fa11b5`)
### §7.3.2 Table 7.1: RETCODE_ILLEGAL_OPERATION

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_ILLEGAL_OPERATION |
IllegalOperationException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.suspendPublications`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.endCoherentChanges`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.endAccess`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_resume_publications_needs_a_suspend_publications_and_the_calls_nest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_end_coherent_changes_needs_a_begin_and_the_calls_nest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_end_access_needs_a_begin_access_and_the_calls_nest`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_the_service_provides_a_concrete_class_for_every_table_7_1_exception_and_for_retcode_error`

**Status:** `done` — Nach DDS 1.4 §2.2.2.4.1.9, §2.2.2.4.1.11 und §2.2.2.5.2.9 sind `resume_publications` ohne `suspend_publications`, `end_coherent_changes` ohne `begin_coherent_changes` und `end_access` ohne `begin_access` PRECONDITION_NOT_MET, und `suspend_publications` wie `begin_coherent_changes` lassen sich schachteln; der Code warf `IllegalOperationException`. `PublisherImpl` zählt jetzt die Tiefe und wirft `PreconditionNotMetException`, `SubscriberImpl.endAccess` ebenso. ILLEGAL_OPERATION: Klasse `org.zerodds.core.IllegalOperationException` und Zuordnung sind vorhanden und per Reflexion getestet; keine Operation des Bindings löst sie aus, DDS 1.4 nennt keine. (Commit `b315020c678`)
### §7.3.2 Table 7.1: RETCODE_NOT_ENABLED

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_NOT_ENABLED | NotEnabledException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.ensureEnabled`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/NotEnabledException.java` (`NotEnabledException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::javapsm_7_3_2_a_not_enabled_writer_refuses_to_write`

**Status:** `done` — Ein nicht aktivierter `DataWriter` wirft bei `write`, `dispose` und `unregisterInstance` `NotEnabledException`, ein nicht aktivierter `DataReader` bei `read` und `take` (`DataReaderImpl.ensureEnabled`). Der Test prüft `write` an einem Writer ohne `enable`. `dispose`, `unregisterInstance` und der Reader sind nicht getestet. (Commit `f31b6fa11b5`)
### §7.3.2 Table 7.1: RETCODE_PRECONDITION_NOT_MET

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_PRECONDITION_NOT_MET |
PreconditionNotMetException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl.detachCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getDiscoveredParticipantData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getKeyValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/PreconditionNotMetException.java` (`PreconditionNotMetException`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_participant_data_carries_the_user_data`

**Status:** `done` — `PreconditionNotMetException` wird geworfen, wenn eine Bedingung vom `WaitSet` gelöst wird, an dem sie nicht hängt, bei `getDiscoveredParticipantData` mit unbekanntem Handle und bei `getKeyValue` an einem Typ ohne Schlüssel. Die zwei Tests prüfen die ersten beiden Fälle. (Commit `7f18142b5e1`)
### §7.3.2 Table 7.1: RETCODE_IMMUTABLE_POLICY

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_IMMUTABLE_POLICY |
ImmutablePolicyException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.setQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkChange`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_refuses_a_change_of_an_immutable_policy_and_keeps_its_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_writer_takes_new_values_of_the_changeable_policies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_reader_takes_a_time_based_filter_and_refuses_another_history`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_an_enabled_topic_takes_topic_data_and_refuses_another_reliability`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_before_enable_any_policy_changes_and_after_enable_the_immutable_ones_do_not`

**Status:** `done` — `QosRules.checkChange` prüft bei `DataWriterImpl.setQos`, `DataReaderImpl.setQos` und `TopicImpl.setQos` eines aktivierten Objekts nur die Policies mit Changeable = No (DDS 1.4 §2.2.3; dazu DATA_REPRESENTATION und TYPE_CONSISTENCY_ENFORCEMENT nach XTypes 1.3): DURABILITY, DURABILITY_SERVICE, PRESENTATION, OWNERSHIP, LIVELINESS, RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS. Eine Änderung von TRANSPORT_PRIORITY, USER_DATA oder TIME_BASED_FILTER wird übernommen. Ein nicht aktiviertes Objekt nimmt jede Änderung an. `PublisherImpl.setQos`, `SubscriberImpl.setQos` und `DomainParticipantImpl.setQos` prüfen ebenfalls mit `QosRules.checkChange`. (Commit `a9c313e5fcb`)
### §7.3.2 Table 7.1: RETCODE_INCONSISTENT_POLICY

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_INCONSISTENT_POLICY |
InconsistentPolicyException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InconsistentPolicyException.java` (`InconsistentPolicyException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_a_history_depth_above_max_samples_per_instance_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreQosRulesTest.java::javapsm_7_3_2_max_samples_below_max_samples_per_instance_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_deadline_shorter_than_the_minimum_separation_is_inconsistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_7_3_5_1_a_topic_is_not_created_with_policies_that_contradict_each_other`

**Status:** `done` — `QosRules.checkConsistent` wird beim Erzeugen (Konstruktor von `TopicImpl`, `DataWriterImpl`, `DataReaderImpl`) und in `setQos` aufgerufen und wirft `InconsistentPolicyException` bei History-KEEP_LAST-Tiefe über `max_samples_per_instance`, `max_samples` unter `max_samples_per_instance` und `minimum_separation` über der Deadline-Periode. Ein Grenzwert `LENGTH_UNLIMITED` und eine unendliche Deadline-Periode werden nicht verglichen. Publisher-, Subscriber- und Participant-QoS enthalten keine dieser Policies. Der Test prüft je Regel Erzeugen und `setQos`, dass das abgelehnte QoS das alte nicht ersetzt, und den Grenzfall der Gleichheit. (Commit `a9c313e5fcb`)
### §7.3.2 Table 7.1: RETCODE_OUT_OF_RESOURCES

**Spec:** §7.3.2, S. 19, Table 7.1 — "RETCODE_OUT_OF_RESOURCES |
OutOfResourcesException"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/OutOfResourcesException.java` (`OutOfResourcesException`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.serialize`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_an_encoding_beyond_the_limit_fails_with_out_of_resources`

**Status:** `done` — `DataWriterImpl.serialize(support, data, maxBytes)` wächst den Puffer bis `MAX_SAMPLE_BYTES` (256 MiB) und wirft darüber `OutOfResourcesException` mit der `BufferOverflowException` als Ursache. Der Test setzt die Grenze auf 1024 Bytes: 1000 Bytes passen (Ergebnislänge 1000), 2000 Bytes ergeben die Ausnahme. (Commit `26684105d79`)
### §7.3.2 Ausnahmen statt `nil` bei Operationen, die ein Objekt liefern

**Spec:** §7.3.2, S. 19 — "In addition, this PSM permits implementations to
throw exceptions to indicate errors in operations that in the PIM return an
object reference."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`

**Status:** `n/a (informative)` — Erlaubnis, keine Pflicht. ZeroDDS macht davon Gebrauch: `create*` wirft `AlreadyClosedException` an einem geschlossenen Ersteller (`CloseCascadeTest::javapsm_7_2_3_participant_close_closes_contained_entities`) und `IllegalArgumentException` bei ungültigen Angaben. (Commit `f31b6fa11b5`)
### §7.3.2 Begründung: Fehler und Kontingenz

**Spec:** §7.3.2, S. 19 — "Design Rationale (non-normative) • This PSM uses
checked and unchecked exceptions according to the following rationale: [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.3.3 Werttypen implementieren `org.omg.dds.core.Value`

**Spec:** §7.3.3, S. 20 — "All DDS types with value semantics implement the
interface org.omg.dds.core.Value. These include QoS, QoS policy, status, time,
and other types."

**Repo:** —

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_durations_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_handles_are_serializable_and_keep_their_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::javapsm_7_3_3_time_duration_and_handles_are_copied_by_value`

**Status:** `done` — Das normative Archiv `omgdds_src.zip` enthält kein `org.omg.dds.core.Value` (die Dateien von `org/omg/dds/core` sind in Annex B aufgeführt); nach §2 gilt bei einem Widerspruch zwischen Text und Quellen die Quelle, und ein neuer Typ in `org.omg.dds` wäre eine Erweiterung nach §7.2.6. Die Typen mit Wertsemantik (Time, Duration, InstanceHandle, QoS, QoS-Policy, Status) haben die Eigenschaften, die §7.3.3 dem Interface zuschreibt: sie sind `Serializable` und `Cloneable` (`clone()` liefert eine Kopie nach Wert), `equals` und `hashCode` folgen dem Zustand, und `copyFrom` überschreibt bei den Modifiable-Typen den Zustand des Ziels (`ValueCopyTest`, `CoreValuesTest`). (Commit `265da1a816c`)
### §7.3.3 `Value` erweitert `Cloneable` und `Serializable`

**Spec:** §7.3.3, S. 20 — "The Value interface extends the standard Java SE
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_policies_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_status_is_serializable_and_keeps_its_counts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::javapsm_7_3_3_every_policy_of_the_policy_factory_is_copied_by_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ValueCopyTest.java::javapsm_7_3_3_qos_and_status_objects_are_copied_by_value`

**Status:** `done` — `Time`, `Duration`, `InstanceHandle`, `QosPolicy`, `Status` und `EntityQos` sind `Serializable` (die Tests serialisieren Zeit, Dauer, Policies und einen Status und vergleichen den Wert nach der Rückkehr) und ihre Klassen in `org.zerodds` sind `Cloneable` mit öffentlichem `clone()`: `ValueCopyTest` kopiert Zeit, Dauer, Handle, jede Policy der `PolicyFactory`, die QoS-Klassen und Status von Entitäten und prüft, dass die Kopie gleich, aber ein anderes Objekt ist und dass die Kopie einer `ModifiableTime` den späteren Änderungen der Quelle nicht folgt. Die Schnittstellen der OMG-Quelle selbst erweitern `Cloneable` nicht (kein `Value`, siehe das Item davor). (Commit `265da1a816c`)
### §7.3.3 `copyFrom` überschreibt den Zustand des Ziels

**Spec:** §7.3.3, S. 20 — "It defines a method copyFrom that accepts a source
object of the same type as the object itself. This method overwrites the state
of the target object (“this”) with the state of the argument object. It is
similar to clone but does not require allocating a new object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ModifiableTime.java` (`ModifiableTime.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ModifiableInstanceHandle.java` (`ModifiableInstanceHandle.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/ModifiableTimeImpl.java` (`ModifiableTimeImpl.copyFrom`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/BuiltinTopicKeyImpl.java` (`BuiltinTopicKeyImpl.copyFrom`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_copy_from_overwrites_the_state_and_copies_an_invalid_source_as_invalid`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_copy_from_overwrites_the_octets_and_an_earlier_copy_keeps_its_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_a_builtin_topic_key_copies_its_words_and_is_built_from_a_handle_big_endian`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_a_null_handle_is_a_bad_parameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::javapsm_7_3_3_copy_from_overwrites_the_whole_state_of_the_publication_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::javapsm_7_3_3_copy_from_overwrites_the_whole_state_of_the_subscription_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::javapsm_7_3_3_copy_from_overwrites_the_whole_state_of_the_topic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinDataCopyFromTest.java::javapsm_7_3_3_copy_from_overwrites_the_state_of_the_participant_data`

**Status:** `done` — `ModifiableTimeImpl.copyFrom` kopiert eine ungültige Quelle als ungültige Zeit und lehnt `null` mit `IllegalArgumentException` ab; `ModifiableInstanceHandleImpl.copyFrom` nimmt Fremdwerte an. `copyFrom` der vier Built-in-Topic-Datentypen überschreibt den ganzen Zustand des Ziels, auch das, was nur diese Implementierung hält (TypeInformation und TypeObject-Oktette, die bei einer fremden Quelle gelöscht werden); `BuiltinDataCopyFromTest` vergleicht Ziel und Quelle nach der Serialisierung als Built-in-Daten. Getestet sind außerdem `ModifiableTime` (gültig, ungültig, fremd), `ModifiableInstanceHandle`, `BuiltinTopicKey` und `KeyedString`. (Commit `ecd51bcff6a`)
### §7.3.3 `equals` und `hashCode` an Zeit, Handle, Policies und QoS

**Spec:** §7.3.3, S. 20 — "Value implementers are also expected to override
their inherited implementations of Object.equals and Object.hashCode in order to
enforce value semantics."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/InstanceHandleImpl.java` (`InstanceHandleImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReliabilityImpl.java` (`ReliabilityImpl.equals`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_policies_are_serializable_and_keep_their_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_equal_times_have_equal_hash_code_and_compare_as_equal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_equal_durations_have_equal_hash_code_and_order_by_length`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_handles_with_the_same_octets_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_3_policies_with_the_same_value_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_3_qos_with_the_same_policies_are_equal_and_have_the_same_hash_code`

**Status:** `done` — CoreValuesTest und CoreStatusPolicyTest prüfen, dass gleiche Time-, Duration-, InstanceHandle-, Policy- und Entity-QoS-Objekte gleiche `hashCode` haben (alle Fabrikmethoden von `PolicyFactory` je zweimal erzeugt, gleich und gleicher Hash). `equals` über Implementierungsgrenzen (Fremd-Time, -Duration, -Handle) ist ebenfalls getestet. Die Status-Klassen sind im Item „equals und hashCode an Status“ abgedeckt. (Commit `819bfcb46cb`)
### §7.3.3 `equals` und `hashCode` an Status

**Spec:** §7.3.3, S. 20 — "Value implementers are also expected to override
their inherited implementations of Object.equals and Object.hashCode in order to
enforce value semantics."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedIncompatibleQosStatusImpl.java` (`OfferedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/PublicationMatchedStatusImpl.java` (`PublicationMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessChangedStatusImpl.java` (`LivelinessChangedStatusImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_status_is_serializable_and_keeps_its_counts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_every_status_compares_by_its_counters_and_handles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_statuses_without_a_counter_are_equal_within_their_kind_only`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_the_handle_of_a_status_is_compared_by_value_not_by_identity`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_the_incompatible_policies_of_a_status_cannot_be_changed`

**Status:** `done` — Alle 13 Status-Klassen in `org.zerodds.core.status` überschreiben `equals` und `hashCode` über Zähler, Handles, Policy-Klasse und Policy-Mengen; `getPolicies()` der beiden Incompatible-QoS-Status liefert eine nicht änderbare Menge. CoreStatusPolicyTest vergleicht je Klasse gleiche Werte (gleich, gleicher Hash) und einen abweichenden Wert (ungleich); Handle-Objekte verschiedener Identität mit gleichen Bytes ergeben gleiche Status. (Commit `12325622fcf`)
### §7.3.3 QoS-Policies unveränderlich

**Spec:** §7.3.3, S. 20 — "QoS policy objects are immutable. New policy objects
can be created from existing policy objects by using the QoS DSL described in
sub clause 7.3.5.3."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PartitionImpl.java` (`PartitionImpl.getName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/UserDataImpl.java` (`UserDataImpl.getValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_user_data_keeps_a_copy_of_the_array_it_was_given_and_hands_out_copies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_topic_data_and_group_data_hand_out_copies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_with_value_leaves_the_original_unchanged`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_3_a_partition_cannot_be_changed_through_its_names`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was`

**Status:** `done` — `UserDataImpl`, `TopicDataImpl` und `GroupDataImpl` kopieren das Array im Konstruktor und in `getValue`; `PartitionImpl.getName` liefert eine nicht änderbare Sicht. CoreStatusPolicyTest prüft, dass ein verändertes Quell- oder Rückgabe-Array den Wert nicht ändert, dass `withValue`/`withName`/`withPolicy` das Original unverändert lassen und dass `Partition.getName().add` eine `UnsupportedOperationException` ergibt. (Commit `12325622fcf`)
### §7.3.3 Begriff Werttyp

**Spec:** §7.3.3, S. 20, Fußnote 2 — "The term “value type” refers to any data
type for which object identity is considered to be established solely based on
the state of the objects of that type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff.
### §7.3.4 Time_t und Duration_t als Werttypen `Time` und `Duration`

**Spec:** §7.3.4, S. 20 — "This PSM maps the DDS Time_t and Duration_t types
into the value types Time and Duration respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Time.java` (`Time`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Duration.java` (`Duration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_the_infinite_duration_is_the_largest_and_long_max_in_any_unit_is_infinite`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_times_are_serializable_and_keep_their_value`

**Status:** `done` — `Time` und `Duration` sind Klassen in `org.omg.dds.core` mit Sekunden und Nanosekunden, Wertsemantik (`equals`, `hashCode`, `Comparable`, `Serializable`) und den Umsetzungen `TimeImpl` und `DurationImpl`. Die Tests prüfen die Umrechnung aus Millisekunden, die unendliche Dauer und den Wertvergleich nach Serialisierung. Die gemeinsame Basis `Value` betrifft das Item „§7.3.3 Werttypen implementieren `org.omg.dds.core.Value`“. (Commit `819bfcb46cb`)
### §7.3.4 Größe in Einheiten von `TimeUnit`

**Spec:** §7.3.4, S. 20 — "These classes can provide their magnitude using a
variety of units (expressed using java.util.concurrent.TimeUnit)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/TimeImpl.java` (`TimeImpl.getTime`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/DurationImpl.java` (`DurationImpl.getDuration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer`

**Status:** `done` — `Time.getTime(TimeUnit)`, `Time.getRemainder(TimeUnit, TimeUnit)` und `Duration.getDuration(TimeUnit)` geben die Größe in der gewählten Einheit zurück. Der Test prüft `getTime` in Sekunden und Millisekunden und `getRemainder` in Nanosekunden; ein zweiter prüft `getDuration(SECONDS)` an zwei Policies (5 und 7). (Commit `819bfcb46cb`)
### §7.3.4 Begründung: Namen ohne `_t`

**Spec:** §7.3.4, S. 20 — "Design Rationale (non-normative) The names of these
types omit the underscore and ‘t’ characters from the ends of their names."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.3.5 Policies mit Basis `org.omg.dds.core.policy.QosPolicy`

**Spec:** §7.3.5, S. 20 — "This PSM represents the former with the base
interface org.omg.dds.core.policy.QosPolicy and the latter with the base
interface org.omg.dds.core.EntityQos."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/QosPolicy.java` (`QosPolicy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`

**Status:** `done` — `org.omg.dds.core.policy.QosPolicy` ist die Basis aller Policy-Schnittstellen (`Reliability`, `History`, `UserData` …), mit den Markerschnittstellen `ForDataReader`, `ForDataWriter` usw. Die Tests übersetzen und benutzen die Policies über `EntityQos<P extends QosPolicy>` (`get(Reliability.class)`, `withPolicy`). (Commit `5041698cee3`)
### §7.3.5 QoS einer Entität mit Basis `org.omg.dds.core.EntityQos`

**Spec:** §7.3.5, S. 20 — "[...] and the latter with the base interface
org.omg.dds.core.EntityQos."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`

**Status:** `done` — `org.omg.dds.core.EntityQos` ist die Basis von `DomainParticipantQos`, `PublisherQos`, `SubscriberQos`, `TopicQos`, `DataWriterQos` und `DataReaderQos`; `QosProfile` gibt es nicht mehr. Die Tests lesen Policies mit `get(Class)` aus Reader-, Topic- und Writer-QoS und vergleichen QoS-Objekte mit `assertEquals`. (Commit `7d991374fd4`)
### §7.3.5.1 Table 7.2: Policy als Schnittstelle mit Eigenschaften nach Java Beans

**Spec:** §7.3.5.1, S. 21, Table 7.2 — "QoS policy structure containing the
state of an instance of that policy. | QoS policy interface extending
org.omg.dds.core.policy.QosPolicy. Each policy provides Java Bean-style
properties."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Durability.java` (`Durability`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`

**Status:** `done` — Die Policies sind Schnittstellen, die `QosPolicy` erweitern, mit Eigenschaften nach Java Beans (`getKind`, `getDepth`, `getMaxBlockingTime`, `withKind`, …); `Durability` ist eine Schnittstelle mit `getKind` und der Enum `Durability.Kind`. Der Test liest `getKind` und `getDepth` an Reader-, Topic- und Writer-QoS. (Commit `5041698cee3`)
### §7.3.5.1 Table 7.2: ID einer Policy als `Class<? extends QosPolicy>`

**Spec:** §7.3.5.1, S. 21, Table 7.2 — "Unique QoS policy ID, represented by an
instance of the enumeration QosPolicyId_t. | Java platform provides “Class
object,” which uniquely identifies a QoS policy. The id will be represented by
an object of Class<? extends QosPolicy> (for example, Class<Reliability>)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.get`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`

**Status:** `done` — Die ID einer Policy ist ihr `Class`-Objekt: `EntityQos` ist eine `Map<Class<? extends P>, P>` mit `get(Class)`, `QosCompatibility.incompatiblePolicyWith` liefert die ablehnende Policy als Klasse. Die Tests lesen `get(Reliability.class)` und prüfen `Reliability.class` als Ergebnis von `incompatiblePolicyWith`. (Commit `c060b572340`)
### §7.3.5.1 Table 7.2: Name einer Policy über Reflection

**Spec:** §7.3.5.1, S. 21, Table 7.2 — "Unique QoS policy name, represented by a
string property QosPolicy.name. | Java reflection provides the necessary
capability to obtain name of a QoSPolicy class."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/QosPolicy.java` (`QosPolicy`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreStatusPolicyTest.java::javapsm_7_3_5_1_the_policy_name_is_the_simple_name_of_its_class`

**Status:** `done` — CoreStatusPolicyTest prüft für 22 Policy-Namen (UserData, TopicData, GroupData, TransportPriority, Lifespan, Durability, Presentation, Deadline, LatencyBudget, Ownership, OwnershipStrength, Liveliness, TimeBasedFilter, Partition, Reliability, DestinationOrder, History, ResourceLimits, EntityFactory, WriterDataLifecycle, ReaderDataLifecycle, DurabilityService), dass `Class.getSimpleName()` dem Namen entspricht, die Klasse eine `QosPolicy`-Schnittstelle ist und die Methode gleichen Namens in `PolicyFactory` (außer TopicData, Presentation: dort ohne Fabrikmethode) diesen Typ liefert. (Commit `5041698cee3`)
### §7.3.5.1 `PolicyFactory` erzeugt neue Policies

**Spec:** §7.3.5.1, S. 21 — "The org.omg.dds.core.policy.PolicyFactory interface
allows creation of new default-initiated policy objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.getPolicyFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`

**Status:** `done` — `org.omg.dds.core.policy.PolicyFactory` erzeugt je Policy ein Objekt mit Vorgabewerten (`Reliability()`, `History()`, `UserData()` …); `PolicyFactoryImpl` setzt es um, `ServiceProviderInterface.getPolicyFactory` und `EntityQos.getPolicyFactory` liefern die Fabrik. Der Test prüft, dass die Fabrik `Reliability()` liefert; die QoS-Tests erzeugen alle Policies darüber. (Commit `5041698cee3`)
### §7.3.5.1 Vorgaben der PolicyFactory nicht festgelegt

**Spec:** §7.3.5.1, S. 21 — "The default state of the newly created policy
objects via the PolicyFactory interface is unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Keine Anforderung an die Werte.
### §7.3.5.1 Begründung: Policies ausdrücklich setzen

**Spec:** §7.3.5.1, S. 21 — "Design Rationale (non-normative) In general, to
ensure portability, programmers are encouraged to explicitly specify policy
parameters using the QoS DSL."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.
### §7.3.5.1 USER_DATA

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 109 (`dds-1.2.pdf`) — "USER_DATA
| A sequence of octets: “value” | [...] The default value is an empty
(zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/UserData.java` (`UserData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/UserDataImpl.java` (`UserDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantQosImpl.java` (`DomainParticipantQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataReaderQosImpl.java` (`DataReaderQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_participant_data_carries_the_user_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_matched_entities_are_described_with_their_qos`

**Status:** `done` — `UserData` ist eine Policy mit `getValue()` (Oktettfolge) und `withValue(byte[], int, int)`; sie gehört zu DomainParticipant-, DataWriter- und DataReader-QoS und ist standardmäßig leer. Die Tests prüfen die Länge 0 beim Participant und dass die Werte `{1,2,3}` des Writers und `{4}` des Readers an die Gegenseite gemeldet werden. (Commit `7d991374fd4`)
### §7.3.5.1 TOPIC_DATA

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) —
"TOPIC_DATA | A sequence of octets: “value” | [...] The default value is an
empty (zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TopicData.java` (`TopicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TopicDataImpl.java` (`TopicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/TopicQosImpl.java` (`TopicQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_qos_of_every_entity_kind_carries_the_values_of_the_document`

**Status:** `done` — `TopicData` ist eine Policy mit `getValue()` und `withValue(byte[], int, int)` in der Topic-QoS; `PolicyFactoryImpl.TopicData` und `TopicQosImpl.defaults` liefern eine leere Folge. Der Test in `BuiltinTopicQosTest` setzt `{5,6,7,8,9}` und prüft die Ankunft in den Built-in-Topic-Daten über SEDP; `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos` prüft die Vorgabe „leer“, `QosProviderTest` liest TOPIC_DATA aus base64.
### §7.3.5.1 GROUP_DATA

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) —
"GROUP_DATA | A sequence of octets: “value” | [...] The default value is an
empty (zero-sized) sequence."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/GroupData.java` (`GroupData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/GroupDataImpl.java` (`GroupDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/PublisherQosImpl.java` (`PublisherQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/SubscriberQosImpl.java` (`SubscriberQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data`

**Status:** `done` — `GroupData` ist eine Policy mit `getValue()` und `withValue(byte[], int, int)` in Publisher- und Subscriber-QoS; `PolicyFactoryImpl.GroupData` liefert eine leere Folge. `BuiltinTopicQosTest` setzt `{9,8}` am Publisher und prüft die Ankunft in den Built-in-Topic-Daten und die SEDP-Hin- und Rückumwandlung; `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos` prüft die Vorgabe „leer“ an Publisher- und Subscriber-QoS, `QosTypesTest::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data` den gesetzten Wert an beiden.
### §7.3.5.1 DURABILITY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 110 (`dds-1.2.pdf`) —
"DURABILITY | A “kind”: VOLATILE, TRANSIENT_LOCAL, TRANSIENT, or PERSISTENT"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Durability.java` (`Durability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DurabilityImpl.java` (`DurabilityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityService.java` (`DurabilityService`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.retainForDurability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/DurabilityJournal.java` (`DurabilityJournal`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_volatile_does_not_replay_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::javapsm_7_3_5_1_transient_survives_the_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::javapsm_7_3_5_1_persistent_survives_the_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DurabilityServiceTest.java::javapsm_7_3_5_1_persistent_survives_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_durability_offered_must_be_at_least_requested_in_the_order_volatile_to_persistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_a_transient_local_reader_is_refused_by_a_volatile_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::javapsm_7_3_5_1_persistent_samples_outlive_the_process`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::javapsm_7_3_5_1_a_disposed_instance_is_told_disposed_after_the_restart_until_its_delay_has_passed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosPersistentStoreTest.java::javapsm_7_3_5_1_a_disposed_instance_without_delay_is_forgotten_after_the_restart_and_stays_forgotten`

**Status:** `done` — `Durability` ist eine Policy-Schnittstelle mit `Durability.Kind` (VOLATILE, TRANSIENT_LOCAL, TRANSIENT, PERSISTENT). Die Tests prüfen: TRANSIENT_LOCAL liefert einem später erzeugten Reader nach, VOLATILE nicht, TRANSIENT überlebt den Writer und endet mit dem Participant, PERSISTENT überlebt Writer und Participant. Die Ordnung beim Abgleich (`QosCompatibility`, angeboten >= gefordert) prüft `QosRequestedOfferedTest::javapsm_7_3_5_1_durability_offered_must_be_at_least_requested_in_the_order_volatile_to_persistent` für alle 16 Paare, und `javapsm_7_3_5_1_a_transient_local_reader_is_refused_by_a_volatile_writer` den Verbindungsabbruch mit REQUESTED_INCOMPATIBLE_QOS. PERSISTENT hat dauerhaften Speicher, wenn die Systemeigenschaft `zerodds.durability.dir` ein Verzeichnis nennt: `org.zerodds.internal.DurabilityJournal` schreibt je (Domäne, Topic, Typ) ein Journal, `DurabilityService` spielt es beim ersten Zugriff auf das Topic ein, geprüft in `QosPersistentStoreTest` (Neustart über `DurabilityService.reset`, Verlaufstiefe, entsorgte Instanz mit und ohne Verzögerung, abgeschnittener letzter Eintrag, TRANSIENT wird nicht geschrieben). Ohne die Eigenschaft liegt PERSISTENT im Arbeitsspeicher des Prozesses wie TRANSIENT. (Commit `d0fb00b1341`)
### §7.3.5.1 DURABILITY_SERVICE

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 111 (`dds-1.2.pdf`) —
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_default_ownership_strength_and_durability_service_of_the_factory_are_the_table_values`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_the_history_depth_of_the_policy_decides_what_a_late_joiner_gets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_keep_all_in_the_store_keeps_every_sample_up_to_the_limit`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_a_sample_beyond_max_samples_per_instance_is_refused_and_not_delivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_a_new_instance_beyond_max_instances_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_max_samples_bounds_the_samples_of_all_instances_together`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_a_disposed_instance_stays_in_the_store_for_the_cleanup_delay_and_is_told_disposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_with_no_cleanup_delay_a_disposed_instance_without_writers_is_forgotten_at_once`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_an_instance_disposed_while_a_writer_still_writes_it_is_kept_disposed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_a_write_after_the_dispose_makes_the_instance_alive_in_the_store`

**Status:** `done` — `DurabilityService` ist eine Policy mit `getServiceCleanupDelay`, `getHistoryKind`, `getHistoryDepth`, `getMaxSamples`, `getMaxInstances`, `getMaxSamplesPerInstance` und den `with…`-Methoden in Topic- und DataWriter-QoS. Die Vorgaben der Fabrik sind jetzt Verzögerung 0, KEEP_LAST Tiefe 1 und `LENGTH_UNLIMITED` für die drei Grenzen (`QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`). `DataWriterImpl.retainForDurability` übergibt Writer-Kennung, Policy und Zeitstempel an `org.zerodds.internal.DurabilityService`: Verlaufstiefe und Grenzen des Speichers kommen aus dieser Policy, nicht aus der HISTORY des Writers; ein Sample über einer Grenze löst `OutOfResourcesException` aus, bevor es zugestellt wird. `service_cleanup_delay` bestimmt, wann der Speicher eine Instanz vergisst: nach dispose, wenn kein Writer sie mehr schreibt und die Verzögerung seit beidem abgelaufen ist (DDS 1.4 §2.2.3.4); bis dahin meldet er einem späteren Reader die Instanz als NOT_ALIVE_DISPOSED. Die Uhr ist in den Tests ersetzbar (`DurabilityService.useClock`). Getestet in `QosDurabilityServiceTest`. (Commit `3389cd624cf`)
### §7.3.5.1 PRESENTATION

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 111–112 (`dds-1.2.pdf`) —
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_presentation_offered_scope_must_be_at_least_requested_and_coherent_and_ordered_must_be_offered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_the_presentation_of_the_subscriber_must_be_offered_by_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoherentSetAtomicityTest.java::javapsm_2_a_reader_on_another_thread_never_sees_part_of_a_set`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_ordered_access_with_scope_topic_returns_the_samples_of_all_instances_in_one_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WireEntityBehaviourTest.java::javapsm_7_3_5_1_a_coherent_set_of_a_remote_writer_reaches_a_coherent_access_reader_as_a_whole`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::javapsm_7_3_5_1_get_data_readers_is_a_list_in_the_order_of_writing_and_a_read_gives_one_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::javapsm_7_3_5_1_samples_of_two_remote_writers_of_one_publisher_arrive_in_the_order_of_the_group_sequence_number`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::javapsm_7_3_5_1_a_group_coherent_set_of_two_remote_writers_arrives_as_a_whole_after_the_end_markers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/GroupAccessTest.java::javapsm_7_3_5_1_a_group_publisher_with_two_writers_sends_its_samples_and_sets_to_a_peer`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/GroupOrderTest.java::rtps_8_7_6_a_group_coherent_set_is_handed_on_together_when_its_end_has_come`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/GroupOrderTest.java::rtps_8_7_6_a_group_coherent_set_with_a_number_given_up_is_not_delivered`

**Status:** `done` — `Presentation` hat `getAccessScope` (INSTANCE, TOPIC, GROUP), `isCoherentAccess` und `isOrderedAccess`; Vorgabe INSTANCE/false/false (`PolicyFactoryImpl.Presentation`). Die Verträglichkeit wird abgeglichen: `PresentationImpl.requestedOfferedContract` verlangt angebotenen Umfang >= geforderter und bei geforderter `coherent_access` oder `ordered_access` die angebotene gleiche Eigenschaft; `QosCompatibility` nimmt die PRESENTATION von Publisher und Subscriber der beiden Endpunkte in den Abgleich (`withPresentation`), die Verletzung steht in REQUESTED_/OFFERED_INCOMPATIBLE_QOS (`QosRequestedOfferedTest`, alle neun Umfangspaare, coherent, ordered). `coherent_access` wirkt: die Mengen zwischen `begin_coherent_changes` und `end_coherent_changes` erreichen einen Reader in einem anderen Thread als Ganzes und, über RTPS, mit `PID_COHERENT_SET`. `ordered_access` mit access_scope TOPIC ordnet die Samples aller Instanzen eines Readers in einer Reihenfolge (`javapsm_2_ordered_access_with_scope_topic_returns_the_samples_of_all_instances_in_one_order`). Mit access_scope GROUP ist `get_datareaders` bei `ordered_access` eine Liste, in der jeder Reader so oft vorkommt, wie er Samples hält, in der Reihenfolge des Schreibens (Quellzeitstempel bei BY_SOURCE_TIMESTAMP, sonst Ankunft über alle Reader des Prozesses), und `read` und `take` liefern höchstens ein Sample (DDS 1.4 §2.2.2.5.2.10, §2.2.2.5.3.8). Über RTPS trägt jedes Sample eines Writers eines Publishers mit GROUP `PID_GROUP_SEQ_NUM` (fortlaufend über alle Writer des Publishers), `PID_WRITER_GROUP_INFO` (`GroupDigest_t`: die ersten vier Oktette des MD5 über die CDR-Big-Endian-Folge der Entity-Ids der Writer, sortiert nach dem Wert als Little-Endian-Integer, `GroupDigest.of`) und in einem Satz `PID_GROUP_COHERENT_SET`; jeder Writer sendet am Ende des Satzes das End-Coherent-Set-DATA (DDSI-RTPS 2.5 §8.7.5, §8.7.6, §9.3.2.5, §9.6.4.3 bis §9.6.4.5). Die Writer eines Publishers mit GROUP senden HEARTBEATs mit dem GroupInfoFlag (0x08): `currentGSN` (größte vergebene Gruppennummer), `firstGSN` und `lastGSN` des Writers sowie `writerSet` und `secureWriterSet` (`RtpsMessage.Builder.heartbeatGroup`, DDSI-RTPS 2.5 §8.3.7.5, §9.4.5.7, §9.4.2.19); ein Writer ohne Sample sendet sie ebenfalls (leerer Bereich). Die Option `zerodds.rtps.omit-heartbeat-group-digests` (Rust: `wire.omit_heartbeat_group_digests`, Vorgabe aus) lässt `writerSet` und `secureWriterSet` weg. Ein Reader eines Subscribers mit GROUP gibt die Samples der Writer eines entfernten Publishers in der Reihenfolge der Gruppennummer weiter und einen Gruppensatz zusammen, wenn sein Ende da ist (`GroupOrder`). Fehlt die Nummer n, gilt sie nach §8.7.5 als nicht vorhanden, wenn ein HEARTBEAT `currentGSN >= n` meldet und jeder gezählte Writer (zugeordnet, RELIABLE, der Gruppe nach `writerSet`) sie nicht als seine ausweist: er hat ein Sample mit größerer Gruppennummer geliefert, oder sein HEARTBEAT mit `currentGSN >= n` hat einen Bereich `[firstGSN, lastGSN]`, der n nicht enthält; ein Writer, von dem noch nichts gehört wurde, hält die Reihenfolge an. Ein Satz mit einer so aufgegebenen Nummer wird nicht zugestellt (§8.7.6). Die Zeitregel ist eine benannte Option, aus in der Vorgabe: `zerodds.rtps.group-timeout-millis` (eine Nummer, die in dieser Zeit nicht eintrifft, wird übersprungen). Tests: `GroupOrderTest` (Regel, Zeitoption aus und an, Satz) und `GroupAccessTest` (zwei Writer über RTPS: Nummer 11 gehört einem Writer ohne zugeordneten Reader und wird durch die HEARTBEATs der beiden zugeordneten Writer aufgegeben; ohne HEARTBEAT des zweiten Writers bleibt sie fehlend). Die Gruppennummer wird vergeben und das Sample in die Historie des Writers eingetragen unter einer Sperre (`PublisherImpl.stampAndWrite`); das HEARTBEAT liest `currentGSN` unter derselben Sperre (`currentGroupStamp`) und meldet damit nur Nummern, deren Änderung eingetragen ist (`GroupHeartbeatLockTest`). Erwartung gegenüber Cyclone DDS, Fast DDS und OpenDDS: sie lesen `PID_GROUP_SEQ_NUM`, `PID_GROUP_COHERENT_SET` und `PID_WRITER_GROUP_INFO` nach §8.7.5; gemessen ist nur die Gegenstelle in ZeroDDS. (Commits `95d9d5eee83`, `1a2d89d1696`, `56280b5d10c`, `5d99860e61a`)
### §7.3.5.1 DEADLINE

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 112 (`dds-1.2.pdf`) — "DEADLINE
| A duration “period”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Deadline.java` (`Deadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DeadlineImpl.java` (`DeadlineImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.evaluateDeadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_an_infinite_deadline_is_never_missed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_deadline_offered_period_must_not_exceed_requested`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_offered_deadline_missed_fires_without_polling`


**Status:** `done` — `Deadline.getPeriod` mit Vorgabe unendlich (`PolicyFactoryImpl.Deadline`). Reader und Writer zählen Versäumnisse (`DataReaderImpl.evaluateDeadline`, `DataWriterImpl.evaluateOfferedDeadline`); die Tests prüfen REQUESTED_DEADLINE_MISSED mit 50 ms, dass unendlich nie versäumt, OFFERED_DEADLINE_MISSED und die Listener-Meldung ohne Abfrage sowie die Verträglichkeit (angeboten <= gefordert) in `QosRequestedOfferedTest::javapsm_7_3_5_1_deadline_offered_period_must_not_exceed_requested`.
### §7.3.5.1 LATENCY_BUDGET

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) —
"LATENCY_BUDGET | A duration “duration” | [...] The default value of the
duration is zero"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/LatencyBudget.java` (`LatencyBudget`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/LatencyBudgetImpl.java` (`LatencyBudgetImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.LatencyBudget`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicies`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_latency_budget_offered_must_not_exceed_requested`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`

**Status:** `done` — `LatencyBudget.getDuration` mit Vorgabe 0 (`PolicyFactoryImpl.LatencyBudget`, geprüft in `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`); der Wert 2 s wird auf PID_LATENCY_BUDGET geschrieben und im Rundlauf gelesen. `QosCompatibility.incompatiblePolicies` prüft LATENCY_BUDGET (angeboten <= gefordert, `LatencyBudgetImpl.requestedOfferedContract`); `QosRequestedOfferedTest::javapsm_7_3_5_1_latency_budget_offered_must_not_exceed_requested` prüft länger, gleich, kürzer und die Vorgabe. Die Policy ist nach DDS 1.4 §2.2.3.7 ein Hinweis; auf die Zustellung wirkt sie nicht. (Commit `df63a3a7869`)
### §7.3.5.1 OWNERSHIP

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) — "OWNERSHIP
| A “kind” SHARED EXCLUSIVE"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Ownership.java` (`Ownership`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/OwnershipImpl.java` (`OwnershipImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_shared_ownership_delivers_from_all_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_ownership_kinds_must_be_equal_in_both_directions`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_a_shared_reader_and_an_exclusive_writer_do_not_match_and_both_sides_record_it`

**Status:** `done` — `Ownership.getKind` (SHARED, EXCLUSIVE); der Reader arbitriert nur, wenn Reader und Writer EXCLUSIVE sind (`DataReaderImpl.onWrite`). DDS 1.4 §2.2.3.9 verlangt dieselbe Art: `OwnershipImpl.requestedOfferedContract` liefert 0 bei gleicher und 1 bei verschiedener Art, `QosCompatibility` meldet OWNERSHIP in beiden Richtungen als unverträglich. Getestet: `QosRequestedOfferedTest::javapsm_7_3_5_1_ownership_kinds_must_be_equal_in_both_directions` über `QosCompatibility.isCompatible`, `javapsm_7_3_5_1_a_shared_reader_and_an_exclusive_writer_do_not_match_and_both_sides_record_it` mit Reader und Writer (kein Sample, REQUESTED_ und OFFERED_INCOMPATIBLE_QOS mit Ownership). (Commit `df63a3a7869`)
### §7.3.5.1 OWNERSHIP_STRENGTH

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 113 (`dds-1.2.pdf`) —
"OWNERSHIP_ STRENGTH | An integer “value”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/OwnershipStrength.java` (`OwnershipStrength`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/OwnershipStrengthImpl.java` (`OwnershipStrengthImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.OwnershipStrength`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_default_ownership_strength_and_durability_service_of_the_factory_are_the_table_values`

**Status:** `done` — `OwnershipStrength.getValue`; der Reader nimmt bei EXCLUSIVE nur Samples des Writers mit der höchsten Stärke an (1 gegen 10), der Wert 7 läuft über Built-in-Topic-Daten. `PolicyFactoryImpl.OwnershipStrength()` erzeugt jetzt den Wert 0 (DDS 1.4 §2.2.3, Tabelle der Policies); `DataWriterQosImpl.defaults` übernimmt die 0. Geprüft in `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos` und `javapsm_7_3_5_1_the_default_ownership_strength_and_durability_service_of_the_factory_are_the_table_values`. (Commit `df63a3a7869`)
### §7.3.5.1 LIVELINESS

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 114 (`dds-1.2.pdf`) —
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_liveliness_offered_kind_must_be_at_least_requested_and_the_lease_not_longer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_lost_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_the_participant_asserts_the_liveliness_of_its_manual_by_participant_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_the_participant_does_not_assert_the_liveliness_of_a_manual_by_topic_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_an_assert_on_any_writer_of_the_participant_asserts_the_manual_by_participant_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_a_write_by_any_writer_of_the_participant_asserts_the_manual_by_participant_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLivelinessTest.java::javapsm_7_3_5_1_the_writers_of_another_participant_are_not_asserted`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_the_wire_side_refuses_an_offered_liveliness_kind_below_the_requested_one`

**Status:** `done` — `Liveliness` mit Art und `lease_duration`; Vorgabe AUTOMATIC/unendlich (`PolicyFactoryImpl.Liveliness`, geprüft in `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`). Die Lease wird überwacht: der Writer meldet LIVELINESS_LOST, der Reader LIVELINESS_CHANGED, jeweils ohne Statusabfrage (`LeaseTest`). Verträglichkeit: angebotene Art >= geforderte (AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC) und angebotene Lease <= geforderte (`QosRequestedOfferedTest::javapsm_7_3_5_1_liveliness_offered_kind_must_be_at_least_requested_and_the_lease_not_longer`); die Leitungsseite `QosParameters.incompatibility` verglich die Art umgekehrt und ist korrigiert (`javapsm_7_3_5_1_the_wire_side_refuses_an_offered_liveliness_kind_below_the_requested_one`). `DomainParticipantImpl.assertLiveliness` erneuert jetzt alle DataWriter des Participants mit MANUAL_BY_PARTICIPANT; ebenso erneuert jedes Schreiben und jedes `assert_liveliness` eines Writers des Participants diese Writer (DDS 1.4 §2.2.3.11); ein MANUAL_BY_TOPIC-Writer und die Writer eines anderen Participants bleiben unberührt (`QosLivelinessTest`). (Commit `6fa88c2bd3b`)
### §7.3.5.1 TIME_BASED_FILTER

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 115 (`dds-1.2.pdf`) —
"TIME_BASED_ FILTER | A duration "minimum_ separation" | [...] default
minimum_separation=0"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TimeBasedFilter.java` (`TimeBasedFilter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TimeBasedFilterImpl.java` (`TimeBasedFilterImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.TimeBasedFilter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/TimeBasedFilterState.java` (`TimeBasedFilterState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_reader_with_minimum_separation_takes_one_sample_per_instance_in_that_time`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_the_newest_sample_held_back_is_delivered_when_the_separation_has_passed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_without_minimum_separation_every_sample_is_delivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_reader_qos`

**Status:** `done` — `TimeBasedFilter.getMinimumSeparation` mit Vorgabe 0; der Wert 1 s läuft im Rundlauf der Built-in-Subscription-Daten. `DataReaderImpl.onWrite` wertet `minimum_separation` je Instanz aus (`org.zerodds.internal.TimeBasedFilterState`): das erste Sample einer Instanz wird angenommen, ein weiteres innerhalb der Trennung wird zurückgehalten, und das jüngste zurückgehaltene Sample wird nach Ablauf der Trennung zugestellt (beim Zugriff und durch den Sweep des Readers), sodass die letzte Änderung den Reader erreicht (DDS 1.4 §2.2.3.12). Die Zeit ist die Empfangszeit am Reader. DEADLINE < minimum_separation ist `InconsistentPolicyException` (`QosSetQosTest::javapsm_2_a_deadline_shorter_than_the_minimum_separation_is_inconsistent`). Getestet in `QosReaderEffectsTest` (eine Instanz, mehrere Instanzen, Nachlieferung des letzten Samples, Vorgabe 0). (Commit `3389cd624cf`)
### §7.3.5.1 PARTITION

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 115 (`dds-1.2.pdf`) — "PARTITION
| A list of strings “name”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Partition.java` (`Partition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PartitionImpl.java` (`PartitionImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/QosParameters.java` (`QosParameters.partitionsOverlap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_partitions_match_on_an_overlap`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_a_partition_wildcard_matches`


**Status:** `done` — `Partition.getName` (Menge von Namen, Vorgabe `""`) steht in `PublisherQos` und `SubscriberQos`. Die Tests prüfen Trennung bei A gegen B, Zustellung bei Überschneidung (`A,X` gegen `X`), Platzhalter (`sensor.*` gegen `sensor.temp`) und die Meldung ohne Match im Listener.
### §7.3.5.1 RELIABILITY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 115–116 (`dds-1.2.pdf`) —
"RELIABILITY | A “kind”: RELIABLE, BEST_EFFORT and a duration
“max_blocking_time” | [...] The default max_blocking_time=100ms."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReliabilityImpl.java` (`ReliabilityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosCompatibility.java` (`QosCompatibility.incompatiblePolicyWith`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DataWriterReliability`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitResources`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::javapsm_7_3_5_1_a_reliable_reader_and_a_best_effort_writer_do_not_connect_and_both_record_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_writer_that_offers_less_than_requested_is_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_write_to_a_new_instance_beyond_max_instances_times_out_until_an_instance_is_unregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_blocked_write_continues_when_another_thread_frees_the_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_best_effort_writer_does_not_wait_and_is_out_of_resources`


**Status:** `done` — Art (BEST_EFFORT, RELIABLE) und `max_blocking_time` (Vorgabe 100 ms, `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`). `ReliabilityImpl.requestedOfferedContract` vergleicht nur die Art (DDS 1.4 §2.2.3.14): ein RELIABLE-Reader mit längerer `max_blocking_time` als der Writer bleibt verträglich (`QosRequestedOfferedTest::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`); RELIABLE-Reader gegen BEST_EFFORT-Writer wird nicht verbunden (`CoreTypesTest`, `QosCompatibilityTest`, `ReaderStatusTest`). `DataWriterImpl.write` wartet bei erreichter Grenze von RESOURCE_LIMITS (nicht freie Instanz, Verlauf für TRANSIENT_LOCAL) höchstens `max_blocking_time` und löst dann `TimeoutException` aus; ein BEST_EFFORT-Writer wartet nicht und löst `OutOfResourcesException` aus (`QosResourceLimitsTest`). Ein Writer ohne Grenze wartet nie. (Commit `3389cd624cf`)
### §7.3.5.1 TRANSPORT_PRIORITY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) —
"TRANSPORT_ PRIORITY | An integer “value”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/TransportPriority.java` (`TransportPriority`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/TransportPriorityImpl.java` (`TransportPriorityImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.TransportPriority`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicQos.java` (`TopicQos.getTransportPriority`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos.getTransportPriority`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`

**Status:** `done` — `TransportPriority.getValue` (int, Vorgabe 0) in `TopicQos` und `DataWriterQos`; der Test setzt 3 an einem `TopicQos` und liest ihn nach dem Rundlauf der Built-in-Topic-Daten wieder. DDS 1.2 §7.1.3.15 nennt die Policy einen Hinweis; ein Einfluss auf den Transport ist nicht vorhanden (im Code nur in `QosParameters` und den Built-in-Daten gelesen).
### §7.3.5.1 LIFESPAN

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) — "LIFESPAN
| A duration “duration”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Lifespan.java` (`Lifespan`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/LifespanImpl.java` (`LifespanImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.Lifespan`),
`crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java` (`InProcessBus.Message.lifespanMillis`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.purgeLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_sample_whose_lifespan_has_passed_when_it_arrives_is_not_delivered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_stored_sample_disappears_when_its_lifespan_passes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_an_infinite_lifespan_never_expires`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_late_joiner_is_not_replayed_an_expired_sample_of_a_transient_local_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDurabilityServiceTest.java::javapsm_7_3_5_1_durable_samples_keep_their_source_timestamp_and_reach_the_reader_with_its_lifespan`

**Status:** `done` — `Lifespan.getDuration` (Vorgabe unendlich); 30 s werden auf PID_LIFESPAN geschrieben und im Rundlauf gelesen. Der Writer trägt seine Gültigkeitsdauer in jede Nachricht (`InProcessBus.Message.lifespanMillis`); `DataReaderImpl` verwirft ein Sample, dessen Quellzeitstempel plus Dauer beim Eintreffen vorbei ist, und entfernt abgelaufene Samples vor jedem Zugriff. `DataWriterImpl.replayTo` stellt abgelaufene Samples einem späten Reader nicht zu und gibt beim Nachliefern den ursprünglichen Quellzeitstempel mit; das gilt ebenso für TRANSIENT und PERSISTENT über `org.zerodds.internal.DurabilityService`. Getestet in `QosReaderEffectsTest` und `QosDurabilityServiceTest`. (Commit `3389cd624cf`)
### §7.3.5.1 DESTINATION_ORDER

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 116 (`dds-1.2.pdf`) —
"DESTINATION_ ORDER | A “kind”: BY_ RECEPTION_ TIMESTAMP, BY_SOURCE_ TIMESTAMP"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/DestinationOrder.java` (`DestinationOrder`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/DestinationOrderImpl.java` (`DestinationOrderImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/PolicyFactoryImpl.java` (`PolicyFactoryImpl.DestinationOrder`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_by_source_timestamp_a_sample_older_than_the_newest_of_the_instance_is_ordered_before_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_by_reception_timestamp_the_source_timestamp_does_not_decide`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_destination_order_offered_must_be_at_least_requested`

**Status:** `done` — `DestinationOrder.getKind` (BY_RECEPTION_TIMESTAMP, BY_SOURCE_TIMESTAMP; Vorgabe BY_RECEPTION_TIMESTAMP); BY_SOURCE_TIMESTAMP wird auf PID_DESTINATION_ORDER geschrieben und im Rundlauf gelesen. Bei BY_SOURCE_TIMESTAMP ordnet `DataReaderImpl` ein Sample einer Instanz, dessen Quellzeitstempel kleiner ist als der jüngste der Instanz, vor die jüngeren ein (DDS 1.4 §2.2.2.5.3.8: die Samples einer Instanz erscheinen in der Reihenfolge der Quellzeitstempel, kleinere zuerst); keines geht verloren, gleiche Zeitstempel bleiben in Ankunftsreihenfolge, andere Instanzen sind unberührt, bei BY_RECEPTION_TIMESTAMP entscheidet der Quellzeitstempel nicht. `QosCompatibility.incompatiblePolicies` prüft DESTINATION_ORDER (angeboten >= gefordert, BY_RECEPTION_TIMESTAMP < BY_SOURCE_TIMESTAMP). Getestet in `QosReaderEffectsTest` und `QosRequestedOfferedTest::javapsm_7_3_5_1_destination_order_offered_must_be_at_least_requested`. (Commit `3389cd624cf`)
### §7.3.5.1 HISTORY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 117–118 (`dds-1.2.pdf`) —
"HISTORY | A “kind”: KEEP_LAST, KEEP_ALL And an optional integer “depth” | [...]
KEEP_LAST is the default kind. The default value of depth is 1."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/HistoryImpl.java` (`HistoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.onWrite`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.retainCapped`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosRules.java` (`QosRules.checkConsistent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_is_per_instance_not_global`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_5_3_history_depth_must_be_non_negative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_7_3_5_1_a_writer_is_not_created_with_policies_that_contradict_each_other`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_7_3_5_1_the_boundaries_of_the_consistency_rules_are_consistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_7_3_5_1_a_topic_is_not_created_with_policies_that_contradict_each_other`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays`

**Status:** `done` — Art (KEEP_LAST, KEEP_ALL) und Tiefe; Vorgabe KEEP_LAST/1. Die Tests prüfen die Kappung je Instanz im Reader, dass die Tiefe nicht global gilt, die Wiedergabe des Writers (KEEP_LAST 1, TRANSIENT_LOCAL), die Vorgabe und die Ablehnung negativer Tiefe; KEEP_ALL wird über `readerKeepAll` genutzt. Die Regel `depth <= max_samples_per_instance` (DDS 1.4 §2.2.3.18) und die zwei Regeln der RESOURCE_LIMITS und von DURABILITY_SERVICE prüft `QosRules.checkConsistent` beim Erzeugen von Writer, Reader und Topic, bei `setQos` und bei `setDefault…Qos`; ein Verstoß ist `InconsistentPolicyException`. Die Grenzfälle (Tiefe gleich Grenze, KEEP_ALL ohne Tiefenvergleich, `LENGTH_UNLIMITED`) sind gültig. Getestet in `QosSetQosTest`, `QosDefaultQosTest`. (Commit `8bc25e5d19b`)
### §7.3.5.1 Vorgaben von RELIABILITY und HISTORY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 116–118 (`dds-1.2.pdf`) —
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_reader_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos`

**Status:** `done` — Vorgaben heute: RELIABILITY RELIABLE für `DataWriterQosImpl.defaults`, BEST_EFFORT für `DataReaderQosImpl.defaults` und `TopicQosImpl.defaults`, HISTORY KEEP_LAST mit Tiefe 1. `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`, `::javapsm_7_3_5_1_the_defaults_of_the_data_reader_qos` und `::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos` prüfen diese Werte an Writer-, Reader- und Topic-QoS; `SubscriberImpl` und `PublisherImpl` nehmen diese Defaults für Entitäten ohne eigene QoS.
### §7.3.5.1 RESOURCE_LIMITS

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 118–119 (`dds-1.2.pdf`) —
"RESOURCE_ LIMITS | Three integers: max_samples, max_instances,
max_samples_per_instance | [...] By default, LENGTH_UNLIMITED."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ResourceLimits.java` (`ResourceLimits`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ResourceLimitsImpl.java` (`ResourceLimitsImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.awaitResources`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_of_a_new_instance_beyond_max_instances_is_rejected_with_that_reason_and_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_beyond_max_samples_per_instance_is_rejected_with_that_reason_and_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_the_default_limits_are_unlimited_and_reject_nothing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_write_to_a_new_instance_beyond_max_instances_times_out_until_an_instance_is_unregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_blocked_write_continues_when_another_thread_frees_the_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_a_best_effort_writer_does_not_wait_and_is_out_of_resources`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_keep_all_with_max_samples_per_instance_times_out_for_transient_local`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_max_samples_bounds_the_history_of_all_instances_together`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_keep_last_replaces_the_oldest_sample_of_the_instance_and_needs_no_new_resource`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosResourceLimitsTest.java::javapsm_7_3_5_1_with_no_limit_a_writer_never_waits`

**Status:** `done` — `ResourceLimits` mit `max_samples`, `max_instances`, `max_samples_per_instance`, Vorgabe `LENGTH_UNLIMITED`. Der Reader lehnt Samples über den drei Grenzen ab (`DataReaderImpl.rejectedByLimits`, SAMPLE_REJECTED): geprüft für `max_samples` (REJECTED_BY_SAMPLES_LIMIT), `max_instances` (REJECTED_BY_INSTANCES_LIMIT), `max_samples_per_instance` (REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT, KEEP_ALL) und die Vorgabe. Der Writer wartet bei RELIABLE bis `max_blocking_time` und löst dann `TimeoutException` aus (BEST_EFFORT: sofort `OutOfResourcesException`), wenn ein Sample eine neue Instanz über `max_instances` oder den für TRANSIENT_LOCAL behaltenen Verlauf über `max_samples` oder `max_samples_per_instance` brächte (`DataWriterImpl.awaitResources`); unregister und dispose geben die Mittel frei und wecken den wartenden Schreiber. Bei KEEP_LAST ersetzt das neue Sample das älteste der Instanz und braucht kein neues Mittel. Ein VOLATILE-Writer behält keine Samples, nur seine Instanzen werden begrenzt. Die Regeln gegen HISTORY stehen unter HISTORY (`QosRules`). (Commit `3389cd624cf`)
### §7.3.5.1 ENTITY_FACTORY

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 119 (`dds-1.2.pdf`) — "ENTITY_
FACTORY | A boolean: “autoenable_ created_ entities” | [...] By default, TRUE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/EntityFactory.java` (`EntityFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/EntityFactoryImpl.java` (`EntityFactoryImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.enableIfFactorySays`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.createDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_publisher_with_auto_enable_false_creates_disabled_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_the_change_of_the_factory_policy_affects_only_the_entities_created_after_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_enabling_the_publisher_leaves_the_writers_disabled_when_its_policy_is_false`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_subscriber_with_auto_enable_false_creates_disabled_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_reader_of_a_disabled_subscriber_is_disabled_and_enabling_the_subscriber_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_the_participant_policy_decides_whether_its_publishers_subscribers_and_topics_are_enabled`

**Status:** `done` — `EntityFactory.isAutoEnableCreatedEntities`, Vorgabe true. Publisher und Subscriber lesen `entity_factory` ihrer QoS: `PublisherImpl.createDataWriter` und `SubscriberImpl.createDataReader` aktivieren die neue Entität nur, wenn der Publisher beziehungsweise Subscriber selbst aktiviert ist und die Policy true sagt (DDS 1.4 §2.2.3.20, §2.2.2.1.1.7). `enable` am Publisher oder Subscriber aktiviert die bis dahin deaktivierten Kinder, wenn die Policy es sagt; `enable` an einem Writer oder Reader, dessen Publisher oder Subscriber nicht aktiviert ist, löst `PreconditionNotMetException` aus. Eine Änderung der Policy wirkt nur auf später erzeugte Entitäten. Beim Participant gilt die Policy seines Factory-QoS (`DomainParticipantFactoryImpl.autoEnable`), bei Publisher, Subscriber und Topic die des Participants (`enableIfFactorySays`). Getestet in `QosEntityFactoryTest`. (Commit `8bc25e5d19b`)
### §7.3.5.1 WRITER_DATA_LIFECYCLE

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 119–120 (`dds-1.2.pdf`) —
"WRITER_DATA_ LIFECYCLE | A boolean: “autodispose_ unregistered_ instances” |
[...] By default, TRUE."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/WriterDataLifecycle.java` (`WriterDataLifecycle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/WriterDataLifecycleImpl.java` (`WriterDataLifecycleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.unregisterKey`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_unregister_disposes_the_instance_when_the_flag_is_true`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_unregister_only_leaves_no_writers_when_the_flag_is_false`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_an_instance_disposed_by_the_application_stays_disposed_when_it_is_unregistered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_deleting_the_writer_disposes_its_instances_when_the_flag_is_true`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_deleting_the_writer_only_leaves_no_writers_when_the_flag_is_false`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosWriterDataLifecycleTest.java::javapsm_7_3_5_1_a_write_after_the_dispose_brings_the_instance_back_to_alive`

**Status:** `done` — `WriterDataLifecycle` (`isAutDisposeUnregisteredInstances`, Vorgabe true) in der DataWriter-QoS. `DataWriterImpl.unregisterKey` (von `unregisterInstance`, dem Handle-Pfad und `close()` benutzt) veröffentlicht bei true zuerst DISPOSE und dann UNREGISTER, als hätte die Anwendung vor dem Abmelden `dispose` gerufen (DDS 1.4 §2.2.3.21); bei false nur UNREGISTER. Der Reader lässt eine als NOT_ALIVE_DISPOSED markierte Instanz beim Abmelden des letzten Writers in diesem Zustand. Mit der Vorgabe ist das Ergebnis eines Unregister also NOT_ALIVE_DISPOSED, mit false NOT_ALIVE_NO_WRITERS; `QosWriterDataLifecycleTest::javapsm_7_3_5_1_unregister_only_leaves_no_writers_when_the_flag_is_false` und `javapsm_2_closing_one_writer_does_not_mark_no_writers_while_another_writes` verwenden jetzt false, `DurabilityServiceTest` und `MultiTopicJoinTest` ebenso, wo sie NO_WRITERS beziehungsweise überlebende Daten erwarten. Die eingebauten Topics (`Builtin.writer`) verwenden false und melden weiter NOT_ALIVE_NO_WRITERS. Getestet in `QosWriterDataLifecycleTest`. (Commit `3389cd624cf`)
### §7.3.5.1 READER_DATA_LIFECYCLE

**Spec:** §7.3.5.1, S. 21; [DDS] 1.2 §7.1.3, S. 120 (`dds-1.2.pdf`) —
"READER_DATA_ LIFECYCLE | Two durations “autopurge_ nowriter_ samples_delay” and
“autopurge_ disposed_ samples_delay” | [...] By default, infinite."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ReaderDataLifecycle.java` (`ReaderDataLifecycle`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/policy/ReaderDataLifecycleImpl.java` (`ReaderDataLifecycleImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.purgeLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_by_default_an_instance_without_writers_keeps_its_samples_until_they_are_taken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_zero_delay_purges_the_instance_without_writers_with_its_untaken_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_zero_delay_purges_the_samples_of_a_disposed_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_the_delay_of_the_other_state_does_not_purge`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosReaderEffectsTest.java::javapsm_7_3_5_1_a_finite_delay_purges_when_it_has_passed_without_any_access`

**Status:** `done` — `ReaderDataLifecycle` (`getAutoPurgeNoWriterSamplesDelay`, `getAutoPurgeDisposedSamplesDelay`, Vorgabe unendlich) in der DataReader-QoS. `DataReaderImpl.purgeLocked` läuft vor jedem Zugriff und im Sweep des Readers: nach `autopurge_nowriter_samples_delay` seit NOT_ALIVE_NO_WRITERS entfernt es die Instanz mit allen ungenommenen Samples, nach `autopurge_disposed_samples_delay` seit NOT_ALIVE_DISPOSED alle Samples der Instanz (DDS 1.4 §2.2.3.22). Unendlich löscht nie; die Verzögerung des einen Zustands wirkt nicht auf den anderen. Getestet in `QosReaderEffectsTest` (Vorgabe, Verzögerung 0 für beide Zustände, Verzögerung des anderen Zustands, endliche Verzögerung ohne Zugriff). (Commit `3389cd624cf`)
### §7.3.5.2 Entity-QoS als Schnittstelle mit Basis `EntityQos`, direkter Zugriff auf die Policies

**Spec:** §7.3.5.2, S. 21 — "Each Entity QoS (e.g., DataReaderQos) is an
interface extending org.omg.dds.core.EntityQos. These sub-interfaces provide
direct access to their policies as in the IDL PSM."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`


**Status:** `done` — Sieben Schnittstellen (`DomainParticipantFactoryQos`, `DomainParticipantQos`, `TopicQos`, `PublisherQos`, `SubscriberQos`, `DataWriterQos`, `DataReaderQos`) erweitern `EntityQos<...>` und haben je Policy einen Zugriff (`getReliability`, `getHistory`, `getPartition`, ...). Die Tests lesen Policies direkt (`DataReaderQos.getHistory`, `TopicQos.getReliability`, `DataWriterQos.getReliability`, `PublisherQos.getPartition`). `QosProfile` gibt es nicht mehr.
### §7.3.5.2 DomainParticipantFactoryQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantFactoryQos { EntityFactoryQosPolicy entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactoryQos.java` (`DomainParticipantFactoryQos.getEntityFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantFactoryQosImpl.java` (`DomainParticipantFactoryQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl.autoEnable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled`

**Status:** `done` — `DomainParticipantFactoryQos` hat `getEntityFactory`; `DomainParticipantFactoryImpl.autoEnable` liest den Wert bei `create_participant`. `ParticipantEntityTest::javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled` setzt eine Factory-QoS mit ENTITY_FACTORY false, liest `getEntityFactory()` zurück, prüft dass ein erzeugter Participant nicht aktiviert ist (`DomainParticipantImpl.isEnabled`), dass `enable` ihn aktiviert, und stellt die QoS wieder her. `DomainParticipantFactoryImpl.setQos` prüft die QoS wie jedes `setQos`.
### §7.3.5.2 DomainParticipantQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 167 (`dds-1.2.pdf`) — "struct
DomainParticipantQos { UserDataQosPolicy user_data; EntityFactoryQosPolicy
entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantQos.java` (`DomainParticipantQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DomainParticipantQosImpl.java` (`DomainParticipantQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.getQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosSetQosTest.java::javapsm_2_a_participant_and_the_factory_take_new_user_data_and_entity_factory`

**Status:** `done` — `DomainParticipantQos` hat `getUserData` und `getEntityFactory`; ein Participant liefert seine QoS über `getQos`. `javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read` setzt USER_DATA `mine` als Default und prüft, dass `createParticipant(...).getQos()` gleich der gesetzten QoS ist; `ParticipantEntityTest::javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it` prüft die Wirkung von ENTITY_FACTORY false.
### §7.3.5.2 TopicQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 167–168 (`dds-1.2.pdf`) — "struct
TopicQos { topic_data; durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; ownership; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicQos.java` (`TopicQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/TopicQosImpl.java` (`TopicQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_topic_qos_answers_what_it_was_given_for_every_policy_of_table_7_2`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_topic_publisher_and_subscriber_qos`

**Status:** `done` — `TopicQos` hat die 13 Policies der IDL (`topic_data` bis `ownership`) und dazu `getRepresentation` und `getTypeConsistency`; `TopicQosImpl.defaults` setzt alle. Getestet werden Reliability, Durability, History, ResourceLimits, TransportPriority, Deadline, DurabilityService und TopicData (Vorgaben und Rundlauf der Built-in-Topic-Daten) sowie LatencyBudget, Liveliness, DestinationOrder, Lifespan und Ownership mit Vorgabe und geändertem Wert in `QosTypesTest::javapsm_7_3_5_2_the_topic_qos_answers_what_it_was_given_for_every_policy_of_table_7_2`.
### §7.3.5.2 PublisherQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 168 (`dds-1.2.pdf`) — "struct
PublisherQos { PresentationQosPolicy presentation; PartitionQosPolicy partition;
GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherQos.java` (`PublisherQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/PublisherQosImpl.java` (`PublisherQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_partitions_match_on_an_overlap`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_publisher_with_auto_enable_false_creates_disabled_writers`

**Status:** `done` — `PublisherQos` hat `getPresentation`, `getPartition`, `getGroupData`, `getEntityFactory`. PARTITION wirkt auf das Matching (`QosLifecycleTest`), GROUP_DATA und PRESENTATION laufen über Built-in-Topic-Daten und die Leitung. ENTITY_FACTORY am Publisher bestimmt, ob `createDataWriter` den Writer aktiviert (`QosEntityFactoryTest`); PRESENTATION geht in das Matching der Endpunkte ein (`QosRequestedOfferedTest`), die Wirkung von coherent_access und ordered_access auf die Zustellung fehlt (Item zu PRESENTATION). Die Zugriffe mit gesetzten Werten prüft `QosTypesTest::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data`. (Commit `8bc25e5d19b`)
### §7.3.5.2 SubscriberQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 169 (`dds-1.2.pdf`) — "struct
SubscriberQos { PresentationQosPolicy presentation; PartitionQosPolicy
partition; GroupDataQosPolicy group_data; EntityFactoryQosPolicy entity_factory;
};"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberQos.java` (`SubscriberQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/SubscriberQosImpl.java` (`SubscriberQosImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_a_partition_wildcard_matches`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityFactoryTest.java::javapsm_7_3_5_1_a_subscriber_with_auto_enable_false_creates_disabled_readers`

**Status:** `done` — `SubscriberQos` hat `getPresentation`, `getPartition`, `getGroupData`, `getEntityFactory`. PARTITION ist getestet (Trennung, Platzhalter, kein Match im Listener, Rundlauf der Subscription-Daten). PRESENTATION, GROUP_DATA und ENTITY_FACTORY am Subscriber prüft `QosTypesTest::javapsm_7_3_5_2_the_publisher_and_subscriber_qos_answer_presentation_entity_factory_and_group_data`; `SubscriberImpl.createDataReader` aktiviert den Reader nur, wenn `entity_factory` es sagt (`QosEntityFactoryTest`). (Commit `8bc25e5d19b`)
### §7.3.5.2 DataWriterQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 168 (`dds-1.2.pdf`) — "struct
DataWriterQos { durability; durability_service; deadline; latency_budget;
liveliness; reliability; destination_order; history; resource_limits;
transport_priority; lifespan; user_data; ownership; ownership_strength;
writer_data_lifecycle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterQos.java` (`DataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_data_writer_qos_answers_what_it_was_given_for_the_remaining_policies`


**Status:** `done` — `DataWriterQos` hat die 15 Policies der IDL und dazu `getRepresentation` und `getTypeConsistency`; `DataWriterQosImpl.defaults` setzt alle. Getestet sind Durability, Deadline, LatencyBudget, Liveliness, Reliability, DestinationOrder, History, Lifespan, UserData, Ownership und OwnershipStrength sowie ResourceLimits, TransportPriority, DurabilityService und WriterDataLifecycle (`QosTypesTest::javapsm_7_3_5_2_the_data_writer_qos_answers_what_it_was_given_for_the_remaining_policies`, Vorgaben in `javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`). WRITER_DATA_LIFECYCLE wirkt auf unregister und close (`QosWriterDataLifecycleTest`).
### §7.3.5.2 DataReaderQos

**Spec:** §7.3.5.2, S. 21; [DDS] 1.2 IDL, S. 168–169 (`dds-1.2.pdf`) — "struct
DataReaderQos { durability; deadline; latency_budget; liveliness; reliability;
destination_order; history; resource_limits; user_data; ownership;
time_based_filter; reader_data_lifecycle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderQos.java` (`DataReaderQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataReaderQosImpl.java` (`DataReaderQosImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_data_reader_qos_answers_what_it_was_given_for_the_remaining_policies`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_reader_qos`

**Status:** `done` — `DataReaderQos` hat die 12 Policies der IDL (`durability` bis `reader_data_lifecycle`) und dazu `getRepresentation` und `getTypeConsistency`; `DataReaderQosImpl.defaults` setzt alle. Getestet sind Durability, Deadline, Liveliness, Reliability, History, ResourceLimits, Ownership, UserData und TimeBasedFilter sowie LatencyBudget, DestinationOrder und ReaderDataLifecycle (`QosTypesTest::javapsm_7_3_5_2_the_data_reader_qos_answers_what_it_was_given_for_the_remaining_policies`, Vorgaben in `javapsm_7_3_5_1_the_defaults_of_the_data_reader_qos`). OwnershipStrength und Partition gehören nicht zur QoS des Readers.
### §7.3.5.2 Zugriff auf die Policies über `java.util.Map`

**Spec:** §7.3.5.2, S. 21 — "However, the base interface also provides for
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_deadline_offered_period_must_not_exceed_requested`

**Status:** `done` — `EntityQos<P>` erweitert `java.util.Map<Class<? extends P>, P>` mit `get(Class<POLICY>)`; `AbstractEntityQos` liefert `get` und eine nicht änderbare `entrySet`. `QosTypesTest::javapsm_7_3_5_1_the_defaults_of_the_data_writer_qos` sucht per `reader.get(Reliability.class)`; `QosRequestedOfferedTest::javapsm_7_3_5_1_deadline_offered_period_must_not_exceed_requested` läuft über `QosCompatibility.policiesOf` durch alle Einträge der QoS.
### §7.3.5.2 QoS nicht direkt erzeugbar, nur aus `getQos` oder `QosProvider`

**Spec:** §7.3.5.2, S. 21 — "QoS objects cannot be created directly. They can be
either retrieved from an entity (e.g., DataReader) using the getQoS method or
looked up using a string identifier using the QoSProvider interface. QoS objects
can be cloned from pre-existing state maintained by the Service implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/DataWriterQosImpl.java` (`DataWriterQosImpl.defaults`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.getDefaultDataWriterQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_entities_are_created_from_the_qos_of_a_provider`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_data_writer_qos_of_the_publisher_is_the_qos_of_the_writers_it_creates_next`

**Status:** `done` — In der OMG-API haben die QoS-Schnittstellen keinen Konstruktor; die Konstruktoren der `*QosImpl` sind privat. QoS aus Entitäten und Fabriken (`getQos`, `getDefaultPublisherQos`, `getDefaultParticipantQos`), das Ableiten per `withPolicy` und der Weg über `QosProvider` sind getestet (`QosProviderTest::javapsm_7_3_5_4_entities_are_created_from_the_qos_of_a_provider`, `QosDefaultQosTest`). Die statische Methode `*QosImpl.defaults(environment)` ist öffentlich, damit die Tests (`Qos.writer()`, `Qos.reader()`) und `QosProviderImpl` Vorgabe-QoS anlegen; sie ist keine Methode der OMG-API.
### §7.3.5.2 QoS aus Entitäten und QosProvider unveränderlich, Ändern über die QoS-DSL

**Spec:** §7.3.5.2, S. 21 — "QoS objects as returned by Entities and QoSProvider
shall be immutable; applications shall never observe them to change.
Applications that wish to modify QoS values must use the “QoS DSL” supported by
the QoS and Policy classes."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_the_map_of_a_qos_cannot_be_changed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_2_a_qos_from_an_entity_is_not_changed_by_deriving_from_it`

**Status:** `done` — `AbstractEntityQos` kopiert die Policies in eine eigene Map, `entrySet` ist nicht änderbar, `AbstractMap.put`, `remove`, `clear`, `putAll`, `keySet().remove`, `values().clear` und der Iterator-`remove` lösen `UnsupportedOperationException` aus; die Policies haben `final`-Felder, `withPolicy` erzeugt ein neues Objekt. Getestet für alle sieben QoS-Arten in `QosTypesTest::javapsm_7_3_5_2_the_map_of_a_qos_cannot_be_changed`; dass die QoS einer Entität nach `withPolicy` unverändert bleibt und das Original dasselbe Objekt ist, prüfen `javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was` und `javapsm_7_3_5_2_a_qos_from_an_entity_is_not_changed_by_deriving_from_it`.
### §7.3.5.3 Ändern verboten, neue QoS und Policies über eine fluent DSL

**Spec:** §7.3.5.3, S. 21 — "Modifying QoS objects and their constituent
policies is disallowed but a QoS DSL shall support creation of new QoS objects
and policies from the existing objects using Java fluent interface design."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicies`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_every_with_of_a_policy_returns_a_new_object_and_leaves_the_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policies_applies_several_policies_at_once`

**Status:** `done` — Die DSL ist vorhanden: QoS (`withPolicy`, `withPolicies`) und Policies (`withKeepLast`, `withManualByTopic().withLeaseDuration(...)`) liefern ein neues Objekt und lassen sich verketten; die Werte wirken im Verhalten (`javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`, `LeaseTest::javapsm_2_liveliness_changed_fires_without_polling`). Dass das Original unverändert bleibt und ein anderes Objekt zurückkommt, prüfen `QosTypesTest::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was`, `javapsm_7_3_5_3_every_with_of_a_policy_returns_a_new_object_and_leaves_the_source` und `javapsm_7_3_5_3_with_policies_applies_several_policies_at_once`.
### §7.3.5.3 QoS-Klassen mit `withPolicy` und `withPolicies`

**Spec:** §7.3.5.3, S. 21 — "QoS classes shall provide withPolicy and
withPolicies methods that accept one or more policy objects to create a new QoS
object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos.withPolicies`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicies`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policies_applies_several_policies_at_once`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_a_policy_that_is_no_policy_of_the_qos_is_refused`

**Status:** `done` — `EntityQos.withPolicy(P)` und `withPolicies(P...)` sind in `EntityQos` und `AbstractEntityQos` vorhanden; `withPolicy` wird in vielen Tests benutzt. `QosTypesTest::javapsm_7_3_5_3_with_policies_applies_several_policies_at_once` prüft `withPolicies` mit mehreren Policies und ohne; `javapsm_7_3_5_3_a_policy_that_is_no_policy_of_the_qos_is_refused` prüft die `IllegalArgumentException` von `AbstractEntityQos.policyKey` für eine Policy, die sich nur als Policy für Reader ausweist und keine der Policies aus Tabelle 7.2 ist.
### §7.3.5.3 Policy-Klassen mit `with`-Methoden

**Spec:** §7.3.5.3, S. 21 — "Policy classes shall provide with methods to
specify policy parameters and to create new policy objects from the existing
ones."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Liveliness.java` (`Liveliness`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Reliability.java` (`Reliability`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/History.java` (`History`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/ResourceLimits.java` (`ResourceLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_5_3_history_depth_must_be_non_negative`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_every_with_of_a_policy_returns_a_new_object_and_leaves_the_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_3_policies_with_the_same_value_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_the_request_offered_policies_compare_by_their_order`

**Status:** `done` — Alle Policy-Schnittstellen haben `with*`-Methoden für ihre Parameter (`Liveliness.withKind`/`withLeaseDuration`, `Reliability.withMaxBlockingTime`, `History.withDepth`, `ResourceLimits.withMaxSamples` ...). `javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13` erzeugt Policies so und prüft die Werte byteweise auf der Leitung; `javapsm_7_3_5_3_history_depth_must_be_non_negative` prüft die Wertprüfung in `withDepth`.
### §7.3.5.3 Jedes `with` erzeugt ein neues Objekt, Verkettung

**Spec:** §7.3.5.3, S. 21 — "Each with method call will create a new policy
object because the target object of the method call is immutable. The with
methods shall support method chaining (QoS DSL)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/AbstractEntityQos.java` (`AbstractEntityQos.withPolicy`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/Liveliness.java` (`Liveliness.withLeaseDuration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_every_with_of_a_policy_returns_a_new_object_and_leaves_the_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_calls_chain_one_policy_into_the_next`

**Status:** `done` — Verkettung ist getestet (zwei `withPolicy` hintereinander; `Liveliness().withManualByTopic().withLeaseDuration(...)`), und die verketteten Werte wirken. Dass jedes `with` ein neues Objekt erzeugt und das Ziel unverändert lässt, prüfen `QosTypesTest::javapsm_7_3_5_3_with_policy_returns_another_qos_and_leaves_the_source_as_it_was` (`assertNotSame`, Quelle behält den alten Wert) und `javapsm_7_3_5_3_every_with_of_a_policy_returns_a_new_object_and_leaves_the_source` für Reliability, History, ResourceLimits, Deadline, Partition, UserData, Presentation und Durability; `javapsm_7_3_5_3_with_calls_chain_one_policy_into_the_next` die Verkettung.
### §7.3.5.3 Beispiel `pf.ResourceLimits().withMaxSamples(P).withMaxInstances(Q)`

**Spec:** §7.3.5.3, S. 22 — "Example (non-normative) PolicyFactory pf = … //
object policy factory reference ResourceLimits rl =
pf.ResourceLimits().withMaxSamples(P).withMaxInstances(Q);"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/policy/PolicyFactory.java` (`PolicyFactory.ResourceLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosTypesTest.java::javapsm_7_3_5_3_with_calls_chain_one_policy_into_the_next`

**Status:** `n/a (informative)` — Beispiel (nicht normativ). Der Aufbau ist mit der heutigen API möglich: `PolicyFactory.ResourceLimits().withMaxSamples(..).withMaxInstances(..)`; `javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13` ruft ihn auf. Die Methode heißt `ResourceLimits()` wie in den OMG-Quellen. Die Verkettung prüft `QosTypesTest::javapsm_7_3_5_3_with_calls_chain_one_policy_into_the_next` mit den Werten 20, 6 und `LENGTH_UNLIMITED`.
### §7.3.5.3 Begründung: `getQos` ohne Kopie

**Spec:** §7.3.5.3, S. 22 — "Design Rationale (non-normative) • The getQos
operation can operate maximally efficiently: it need not allocate any memory or
perform any copies. [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/EntityBase.java` (`EntityBase.getQos`)

**Tests:** —

**Status:** `n/a (informative)` — Begründung (nicht normativ). `getQos` gibt das gespeicherte, unveränderliche Objekt ohne Kopie zurück; `DomainParticipantOperationsTest::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read` prüft per `assertSame`, dass `getQos` das gesetzte Objekt liefert.
### §7.3.5.4 `org.omg.dds.core.QosProvider` liefert QoS aus Bibliothek und Profil

**Spec:** §7.3.5.4, S. 22 — "The org.omg.dds.core.QosProvider interface allows
Entity’s Qos to be obtained from the names of QoS library and profile."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_qos_of_every_entity_kind_carries_the_values_of_the_document`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_writer_and_reader_qos_carry_the_values_of_the_document_and_keep_the_defaults_of_what_it_leaves_out`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_profile_without_an_element_of_a_kind_has_no_qos_of_that_kind`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_the_factory_qos_has_its_own_element`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_an_id_can_name_another_profile_or_an_element_of_it`

**Status:** `done` — `QosProvider` (abstrakte Klasse) hat `getDomainParticipantFactoryQos`, `getDomainParticipantQos`, `getTopicQos`, `getSubscriberQos`, `getPublisherQos`, `getDataReaderQos`, `getDataWriterQos` jeweils ohne und mit `id`; `QosProviderImpl` liest die XML-Bibliothek. `QosProviderTest` schreibt eine Bibliothek in eine Datei, lädt sie über `QosProvider.newQosProvider(uri, profile, env)` und liest die QoS jeder Entity-Art (ohne und mit `id`), mit den Werten des Dokuments und den Vorgaben für Ausgelassenes. Eine `id` ist der `name` des Elements, `Bibliothek::Profil` oder `Bibliothek::Profil::name`; ein Profil ohne Element der Art oder eine unbekannte `id` ist `IllegalArgumentException`. (Commit `a68b0889e7a`)
### §7.3.5.4 Quelle als URI, Präfix `file://` Pflicht

**Spec:** §7.3.5.4, S. 22 — "The Qos library source is provided as a uniform
resource identifier (URI). Conforming implementation must support “file://”
prefix. For instance, “file:///path/to/qos/library.”"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.read`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_file_uri_is_read_and_the_provider_is_obtained_from_the_environment`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_another_scheme_than_file_is_unsupported`

**Status:** `done` — `QosProviderImpl.read` akzeptiert das Schema `file` (`Paths.get(URI)`) und löst für jedes andere Schema, auch ohne Schema, `UnsupportedOperationException` aus. `QosProviderTest::javapsm_7_3_5_4_a_file_uri_is_read_and_the_provider_is_obtained_from_the_environment` lädt eine `file:///…`-URI, `javapsm_7_3_5_4_another_scheme_than_file_is_unsupported` prüft `http`, `ftp` und `data`. (Commit `a68b0889e7a`)
### §7.3.5.4 `QosProvider` aus `ServiceEnvironment.newQosProvider(uri, profile)`

**Spec:** §7.3.5.4, S. 22 — "An instance of QosProvider is obtained from the
ServiceEnvironment. For example, serviceEnv.newQosProvider(String uri, String
profile);"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java` (`QosProvider.newQosProvider`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.newQosProvider`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_file_uri_is_read_and_the_provider_is_obtained_from_the_environment`

**Status:** `done` — Der Aufruf `serviceEnv.newQosProvider(uri, profile)` des Beispiels gibt es als `QosProvider.newQosProvider(uri, profile, env)`, das an `env.getSPI().newQosProvider(uri, profile)` delegiert (`ServiceProviderInterface`, Umsetzung in `ServiceEnvironmentImpl`); `ServiceEnvironment` selbst hat keine solche Methode (wie in den OMG-Quellen). `QosProviderTest::javapsm_7_3_5_4_a_file_uri_is_read_and_the_provider_is_obtained_from_the_environment` ruft beide Wege auf und vergleicht die QoS.
### §7.3.5.4 `uri` in Standardsyntax, `profile` eindeutig im Dokument

**Spec:** §7.3.5.4, S. 22 — "The uri parameter uses the standard uri syntax. The
profile parameter identifies a uniquely identified profile in the document
referred by the uri."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.read`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java` (`QosProviderImpl.selectProfile`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_uri_that_is_no_uri_and_a_file_that_is_not_there_are_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_profile_is_selected_by_qualified_name_by_unique_name_or_as_the_first`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_a_profile_name_that_two_profiles_share_is_not_unique`

**Status:** `done` — `QosProviderImpl` bildet `new URI(uri)` (Fehler: `IllegalArgumentException`) und wählt das Profil als `Bibliothek::Profil`, als in der Datei eindeutigen Namen oder, bei `null`, das erste. Ein unbekanntes Profil ist `IllegalArgumentException`; ein Name, den zwei Profile tragen, ebenso, und zwei gleichnamige Profile in einer Bibliothek weist schon das Lesen zurück. Getestet in `QosProviderTest::javapsm_7_3_5_4_a_uri_that_is_no_uri_and_a_file_that_is_not_there_are_refused`, `javapsm_7_3_5_4_a_profile_is_selected_by_qualified_name_by_unique_name_or_as_the_first`, `javapsm_7_3_5_4_a_profile_name_that_two_profiles_share_is_not_unique`. (Commit `a68b0889e7a`)
### §7.3.5.4 Factories erzeugen Entitäten und setzen Vorgaben aus QoS-Objekten oder QosProvider

**Spec:** §7.3.5.4, S. 22 — "Each Entity factory interface
DomainParticipantFactory, DomainParticipant, Publisher, and Subscriber provides
methods to create new “product” Entities and to set their default QoS based on
QoS objects created programmatically or obtained through QosProvider."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.setDefaultPublisherQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.setDefaultParticipantQos`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_subscriber_qos_of_the_participant_is_the_qos_of_the_subscribers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_data_writer_qos_of_the_publisher_is_the_qos_of_the_writers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_data_reader_qos_of_the_subscriber_is_the_qos_of_the_readers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosProviderTest.java::javapsm_7_3_5_4_entities_are_created_from_the_qos_of_a_provider`

**Status:** `done` — Die Fabriken `DomainParticipantFactory`, `DomainParticipant`, `Publisher` und `Subscriber` erzeugen Entitäten mit einer QoS und haben `setDefault…Qos`/`getDefault…Qos`; die Vorgabe wird auf Verträglichkeit geprüft. Getestet in `QosDefaultQosTest`: `setDefaultTopicQos`, `setDefaultSubscriberQos`, `setDefaultPublisherQos`, `Publisher.setDefaultDataWriterQos`, `Subscriber.setDefaultDataReaderQos`, `DomainParticipantFactory.setDefaultParticipantQos` und die Ablehnung einer widersprüchlichen Vorgabe; `QosProviderTest::javapsm_7_3_5_4_entities_are_created_from_the_qos_of_a_provider` erzeugt Entitäten aus Provider-QoS. (Commit `8bc25e5d19b`)
### §7.3.6 Alle Entitäten erweitern `Entity`

**Spec:** §7.3.6, S. 22 — "As in the DDS PIM, all Entity interfaces
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_publisher_close_closes_its_writers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_close_contained_entities_closes_them_without_closing_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_every_entity_extends_entity_and_every_one_but_the_participant_extends_domain_entity`

**Status:** `done` — `DomainParticipant` erweitert `Entity`; `Publisher`, `Subscriber`, `DataWriter`, `DataReader` und `Topic` erweitern `DomainEntity`, das `Entity` erweitert (gelesen). Der Test benutzt `Qos.isClosed(Entity<?, ?>)` mit DomainParticipant, Publisher und DataWriter, was nur übersetzt, wenn diese `Entity` sind; die Vererbung der anderen drei prüft kein Test. Es ist eine reine Signaturforderung. `QosEntityBasicsTest::javapsm_7_3_6_every_entity_extends_entity_and_every_one_but_the_participant_extends_domain_entity` prüft die Vererbung aller sechs Typen mit `isAssignableFrom`.
### §7.3.6 `Entity` generisch über QoS- und Listener-Typ

**Spec:** §7.3.6, S. 22 — "In this PSM, this interface is generic; it is
parameterized by the Entity’s QoS and listener types. These parameters allow
applications to call common operations like getQos or getListener in a type-safe
way while still working with Entities polymorphically."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getQos`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosDefaultQosTest.java::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_clearing_the_listener_stops_delivery`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_get_qos_and_get_listener_keep_their_type_on_the_subinterfaces`

**Status:** `done` — `Entity<LISTENER extends EventListener, QOS extends EntityQos<?>>` mit `getQos()` und `getListener()`; die Unterschnittstellen binden die Typen (`Publisher extends DomainEntity<PublisherListener, PublisherQos>`). `QosDefaultQosTest::javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next` liest `createPublisher().getQos()` als `PublisherQos`, `javapsm_2_clearing_the_listener_stops_delivery` liest `getListener()` eines DataReader; beide übersetzen nur mit den Typparametern.
### §7.3.6 `Entity` erweitert `java.io.Closeable`

**Spec:** §7.3.6, S. 22 — "The Entity interface extends java.io.Closeable
interface to support specific new language constructs (e.g., Java 7
try-with-resources) for dealing with all Entities polymorphically."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_entity_extends_closeable`

**Status:** `done` — `Entity extends Closeable, DDSObject` (`Entity.java`). `QosEntityBasicsTest::javapsm_7_3_6_entity_extends_closeable` prüft `Closeable.class.isAssignableFrom(Entity.class)` und hält Publisher, Subscriber, Topic, DataWriter und DataReader als `java.io.Closeable`, schließt sie darüber und prüft den Zustand mit `Qos.isClosed`.
### §7.3.6 Entitäten außer DomainParticipant erweitern `DomainEntity` mit `getParent`

**Spec:** §7.3.6, S. 22 — "Also as in the DDS PIM, Entities other than
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it`

**Status:** `done` — `DomainEntity.getParent` existiert; Publisher, Subscriber und Topic liefern den Participant, DataWriter den Publisher, DataReader den Subscriber. `QosEntityBasicsTest::javapsm_7_3_6_the_parent_of_an_entity_is_the_entity_that_created_it` prüft mit `assertSame` alle fünf Beziehungen.
### §7.3.7 Status, Listener, Bedingungen und WaitSets

**Spec:** §7.3.7, S. 22 — "This sub clause describes the objects pertaining to
the status changes of DDS Entities: the Status types themselves, listeners,
conditions, and wait sets."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung ohne eigene Anforderung.
### §7.3.7.1 Jeder Status als abstrakte Klasse mit Basis `org.omg.dds.core.Status`, die `EventObject` erweitert

**Spec:** §7.3.7.1, S. 23 — "This PSM represents each status identified by the
DDS PIM as an abstract class extending org.omg.dds.core.Status, which in turn
extends java.util.EventObject."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/Status.java` (`Status`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/StatusChangedEvent.java` (`StatusChangedEvent`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_status_is_serializable_and_keeps_its_counts`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_7_1_the_thirteen_status_kinds_are_classes_of_the_status_base`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosEntityBasicsTest.java::javapsm_7_3_7_1_the_event_that_carries_a_status_is_an_event_object_with_the_entity_as_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`

**Status:** `done` — Alle 13 Status sind abstrakte Klassen mit Basis `org.omg.dds.core.status.Status` (`QosEntityBasicsTest::javapsm_7_3_7_1_the_thirteen_status_kinds_are_classes_of_the_status_base`, auch `Serializable`). Der Text sagt, `Status` erweitere `EventObject`; die Quelle `omgdds_src.zip`, die nach §2 bei einem Widerspruch gilt, erklärt `Status` als `abstract class Status implements Serializable, DDSObject` ohne `EventObject`, und `org/omg/dds` bleibt gleich der Quelle (`CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`). Das `java.util.EventObject` trägt `StatusChangedEvent`: es hat `getSource()` (die Entität) und `getStatus()`; `QosEntityBasicsTest::javapsm_7_3_7_1_the_event_that_carries_a_status_is_an_event_object_with_the_entity_as_source` prüft das an einem PublicationMatchedEvent. (Commit `72b36ae5db2`)
### §7.3.7.1 Statusarten als `Class`-Objekte, Masken als `Set`

**Spec:** §7.3.7.1, S. 23 — "This PSM represents status kinds using the
java.lang.Class instances of the corresponding status classes and status masks
as java.util.Sets of such status classes."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.getStatusChanges`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Entity.java` (`Entity.setListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/ServiceEnvironment.java` (`ServiceEnvironment.ServiceProviderInterface.allStatusKinds`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition.setEnabledStatuses`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_status_is_reset_when_it_is_read_and_data_available_when_the_samples_are_taken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`

**Status:** `done` — Statusarten sind `Class<? extends Status>`, Masken `Set`/`Collection` davon: `Entity.getStatusChanges`, `setListener(listener, statuses)`, `StatusCondition.setEnabledStatuses`, `allStatusKinds`/`noStatusKinds`. Die Tests lesen `getStatusChanges().contains(DataAvailableStatus.class)`, setzen und lesen die aktivierten Status einer `StatusCondition` und prüfen `allStatusKinds().size() == 13` und `noStatusKinds().isEmpty()`.
### §7.3.7.1 Status in Callbacks wiederverwendbar, Anwendungen kopieren

**Spec:** §7.3.7.1, S. 23 — "Status objects passed to listeners in callbacks may
be pooled and reused by the implementation. Therefore, applications that wish to
retain these objects—or any objects found within them, such as instance
handles—for later use outside of the callback are responsible for copying them."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis für die Umsetzung und Pflicht der Anwendung; eine erfüllbare Anforderung an den Code besteht nicht. ZeroDDS erzeugt je Callback ein neues Statusobjekt (`DataReaderImpl`: `new RequestedDeadlineMissedStatusImpl(...)` u. a., gelesen).
### §7.3.7.1 InconsistentTopicStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 132, IDL S. 159
(`dds-1.2.pdf`) — "struct InconsistentTopicStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/InconsistentTopicStatus.java` (`InconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/InconsistentTopicStatusImpl.java` (`InconsistentTopicStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.onInconsistentTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_topics_of_the_same_type_or_in_another_domain_are_consistent`

**Status:** `done` — Zwei Topics gleichen Namens mit verschiedenem Typnamen in einer Domain zählen INCONSISTENT_TOPIC (TopicImpl.onInconsistentTopic) und rufen den TopicListener; der Test legt beides fest.
### §7.3.7.1 SampleLostStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 159
(`dds-1.2.pdf`) — "struct SampleLostStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SampleLostStatus.java` (`SampleLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SampleLostStatusImpl.java` (`SampleLostStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.noteSequence`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSampleLostStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_a_gap_in_the_sequence_numbers_of_a_writer_is_sample_lost`,
`crates/java-omgdds/java/src/test/java/org/zerodds/sub/ReaderWireTest.java::javapsm_2_sample_lost_is_reported_to_the_listener`

**Status:** `done` — `DataReaderImpl.noteSequence` zählt Lücken in den Sequenznummern je Writer; `SampleLostStatus` meldet `getTotalCount` und `getTotalCountChange` (Rücksetzen beim Lesen), `onSampleLost` das Ereignis. Beide Tests liefern Samples über `deliverWire`. (Commit `e1b55167947`)
### §7.3.7.1 SampleRejectedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 159
(`dds-1.2.pdf`) — "struct SampleRejectedStatus { long total_count; long
total_count_change; SampleRejectedStatusKind last_reason; InstanceHandle_t
last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SampleRejectedStatus.java` (`SampleRejectedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SampleRejectedStatusImpl.java` (`SampleRejectedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.rejectedByLimits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_sample_beyond_max_samples_is_rejected_and_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_of_a_new_instance_beyond_max_instances_is_rejected_with_that_reason_and_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderLimitsTest.java::javapsm_2_a_sample_beyond_max_samples_per_instance_is_rejected_with_that_reason_and_instance`

**Status:** `done` — `ReaderLimitsTest` prüft die Gründe REJECTED_BY_INSTANCES_LIMIT und REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT mit `getLastInstanceHandle` (von Hand aus dem Schlüssel abgeleitet) und den Änderungswert; `ReaderStatusTest` prüft REJECTED_BY_SAMPLES_LIMIT und den Listener. (Commit `e1b55167947`)
### §7.3.7.1 LivelinessLostStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 159
(`dds-1.2.pdf`) — "struct LivelinessLostStatus { long total_count; long
total_count_change; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessLostStatus.java` (`LivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessLostStatusImpl.java` (`LivelinessLostStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getLivelinessLostStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateLiveliness`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_writer_that_does_not_assert_its_liveliness_loses_it_once_and_asserting_restores_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_lost_fires_without_polling`

**Status:** `done` — DataWriterImpl.getLivelinessLostStatus zählt den Verlust einmal und setzt die Änderung beim Lesen zurück; der Test prüft Zähler, Änderung und Listener.
### §7.3.7.1 LivelinessChangedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 159–160
(`dds-1.2.pdf`) — "struct LivelinessChangedStatus { long alive_count; long
not_alive_count; long alive_count_change; long not_alive_count_change;
InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/LivelinessChangedStatus.java` (`LivelinessChangedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/LivelinessChangedStatusImpl.java` (`LivelinessChangedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getLivelinessChangedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_liveliness_changed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_liveliness_changed_names_the_writer_whose_liveliness_changed_and_counts_the_changes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_an_incompatible_writer_is_not_counted_as_alive_or_not_alive`

**Status:** `done` — `DataReaderImpl.getLivelinessChangedStatus` und der Listener-Pfad setzen `lastPublicationHandle` auf den Handle des Writers, dessen Lebendigkeit sich zuletzt geändert hat. `ReaderStatusDetailTest` prüft `getLastPublicationHandle`, `getAliveCountChange` (+1, dann -1) und `getNotAliveCountChange` (+1). (Commit `e1b55167947`)
### §7.3.7.1 OfferedDeadlineMissedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160
(`dds-1.2.pdf`) — "struct OfferedDeadlineMissedStatus { long total_count; long
total_count_change; InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/OfferedDeadlineMissedStatus.java` (`OfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedDeadlineMissedStatusImpl.java` (`OfferedDeadlineMissedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.evaluateOfferedDeadline`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_a_missed_offered_deadline_is_counted_with_the_instance_that_missed_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_offered_deadline_missed_fires_without_polling`

**Status:** `done` — DataWriterImpl.getOfferedDeadlineMissedStatus liefert total_count, total_count_change und last_instance_handle; der Test prüft alle drei und den Listener.
### §7.3.7.1 RequestedDeadlineMissedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160
(`dds-1.2.pdf`) — "struct RequestedDeadlineMissedStatus { long total_count; long
total_count_change; InstanceHandle_t last_instance_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/RequestedDeadlineMissedStatus.java` (`RequestedDeadlineMissedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/RequestedDeadlineMissedStatusImpl.java` (`RequestedDeadlineMissedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedDeadlineMissedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_requested_deadline_missed_names_the_instance_that_missed_last`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_the_change_of_requested_deadline_missed_starts_again_when_the_status_is_read`

**Status:** `done` — `ReaderStatusDetailTest` prüft `getLastInstanceHandle` bei zwei Instanzen und den Änderungswert 0 beim zweiten Lesen. (Commit `e1b55167947`)
### §7.3.7.1 OfferedIncompatibleQosStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160
(`dds-1.2.pdf`) — "struct OfferedIncompatibleQosStatus { long total_count; long
total_count_change; QosPolicyId_t last_policy_id; QosPolicyCountSeq policies;
};"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/OfferedIncompatibleQosStatus.java` (`OfferedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/OfferedIncompatibleQosStatusImpl.java` (`OfferedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getOfferedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_an_incompatible_reader_is_counted_with_the_refused_policy`

**Status:** `done` — DataWriterImpl.getOfferedIncompatibleQosStatus zählt den abgelehnten Reader mit last_policy_id und der Policy-Liste und ruft onOfferedIncompatibleQos; der Test prüft Zähler, Policy und Listener.
### §7.3.7.1 RequestedIncompatibleQosStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160
(`dds-1.2.pdf`) — "struct RequestedIncompatibleQosStatus { long total_count;
long total_count_change; QosPolicyId_t last_policy_id; QosPolicyCountSeq
policies; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/RequestedIncompatibleQosStatus.java` (`RequestedIncompatibleQosStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/RequestedIncompatibleQosStatusImpl.java` (`RequestedIncompatibleQosStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getRequestedIncompatibleQosStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_writer_that_offers_less_than_requested_is_reported_to_the_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosCompatibilityTest.java::javapsm_7_3_5_1_a_reliable_reader_and_a_best_effort_writer_do_not_connect_and_both_record_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_requested_incompatible_qos_counts_the_refusing_policy_and_resets_its_change`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_every_policy_that_refuses_the_connection_is_counted_and_one_connection_is_one_in_total`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_the_presentation_of_the_subscriber_must_be_offered_by_the_publisher`

**Status:** `done` — `ReaderStatusDetailTest` prüft `getPolicies` (ein `QosPolicyCount` je verweigernder Policy mit Zähler 1), `getTotalCountChange` und `getLastPolicyClass`; `ReaderStatusTest` das Ereignis. (Commit `d17c5f0c379`)
### §7.3.7.1 PublicationMatchedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160
(`dds-1.2.pdf`) — "struct PublicationMatchedStatus { long total_count; long
total_count_change; long current_count; long current_count_change;
InstanceHandle_t last_subscription_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/PublicationMatchedStatus.java` (`PublicationMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/PublicationMatchedStatusImpl.java` (`PublicationMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.getPublicationMatchedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterStatusTest.java::javapsm_2_the_publication_matched_status_counts_and_names_the_last_subscription`

**Status:** `done` — DataWriterImpl.readPublicationMatchedStatusLocked liefert last_subscription_handle des zuletzt verbundenen Readers; der Test vergleicht ihn mit getMatchedSubscriptions.
### §7.3.7.1 SubscriptionMatchedStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133, IDL S. 160–161
(`dds-1.2.pdf`) — "struct SubscriptionMatchedStatus { long total_count; long
total_count_change; long current_count; long current_count_change;
InstanceHandle_t last_publication_handle; };"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/SubscriptionMatchedStatus.java` (`SubscriptionMatchedStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/SubscriptionMatchedStatusImpl.java` (`SubscriptionMatchedStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.getSubscriptionMatchedStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_status_is_readable_without_a_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_status_is_reset_when_it_is_read_and_data_available_when_the_samples_are_taken`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusDetailTest.java::javapsm_2_subscription_matched_names_the_writer_that_connected_or_left_last`

**Status:** `done` — `DataReaderImpl.getSubscriptionMatchedStatus` und der Listener-Pfad setzen `lastPublicationHandle`; `ReaderStatusDetailTest` prüft den Handle sowie `getTotalCountChange` und `getCurrentCountChange` (auch -1 nach dem Schließen eines Writers). (Commit `e1b55167947`)
### §7.3.7.1 DataAvailableStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133 (`dds-1.2.pdf`) —
"DataReader | DATA_AVAILABLE | New information is available."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/DataAvailableStatus.java` (`DataAvailableStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/DataAvailableStatusImpl.java` (`DataAvailableStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/DataAvailableEvent.java` (`DataAvailableEvent`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.fireDataAvailable`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderStatusTest.java::javapsm_2_a_status_is_reset_when_it_is_read_and_data_available_when_the_samples_are_taken`

**Status:** `done` — DataAvailableStatus ist heute eine Klasse; DataReaderListener.onDataAvailable erhält ein DataAvailableEvent mit getStatus() und getSource(). ListenerTest prüft einen Aufruf je Sample mit dem eigenen Reader als Quelle; ReaderStatusTest prüft, dass getStatusChanges() DataAvailableStatus nach dem Schreiben enthält und nach take() nicht mehr. (Commit `28e25c198d1`)
### §7.3.7.1 DataOnReadersStatus

**Spec:** §7.3.7.1, S. 23; [DDS] 1.2 §7.1.4.1, S. 133 (`dds-1.2.pdf`) —
"Subscriber | DATA_ON_READERS | New information is available."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/status/DataOnReadersStatus.java` (`DataOnReadersStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/status/DataOnReadersStatusImpl.java` (`DataOnReadersStatusImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/DataOnReadersEvent.java` (`DataOnReadersEvent`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/SubscriberListener.java` (`SubscriberListener.onDataOnReaders`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_data_on_readers_goes_to_the_subscriber_listener_and_notify_data_readers_passes_it_to_the_readers_with_new_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSubscriberTest.java::javapsm_2_without_a_subscriber_listener_the_readers_listeners_are_called_and_data_on_readers_stays_changed`

**Status:** `done` — `DataReaderImpl.fireDataAvailable` setzt das Flag DATA_ON_READERS am Subscriber (`SubscriberImpl.dataOnReadersChanged`) und ruft `SubscriberListener.onDataOnReaders` mit einem `DataOnReadersEvent`. Ohne Listener bleibt das Flag in `getStatusChanges()`, bis ein Reader des Subscribers gelesen oder genommen wird; mit Listener ist es nach dem Aufruf zurückgesetzt. (Commit `e1b55167947`)
### §7.3.7.2 Listener erweitern `java.util.EventListener`

**Spec:** §7.3.7.2, S. 23 — "This PSM maps the Listener interface from the DDS
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_writer_without_listener_passes_the_match_to_the_publisher_and_then_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener`

**Status:** `done` — TopicListener, PublisherListener, SubscriberListener und DomainParticipantListener werden aufgerufen und geben nicht übernommene Status nach oben weiter; die drei Tests prüfen Publisher, Subscriber (auch DATA_ON_READERS), Participant und Topic.
### §7.3.7.2 Adapter-Klassen mit leeren Methoden, Name mit „Adapter“

**Spec:** §7.3.7.2, S. 23 — "For each listener sub-interface (e.g.,
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_a_writer_listener_that_does_not_take_a_status_leaves_it_to_the_publisher`

**Status:** `done` — Sechs Adapter-Klassen (DataReaderAdapter, DataWriterAdapter, TopicAdapter, PublisherAdapter, SubscriberAdapter, DomainParticipantAdapter) implementieren ihr Listener-Interface mit leeren Methoden. Die Tests überschreiben einzelne Methoden: TopicAdapter (TopicOperationsTest), PublisherAdapter (ParticipantEntityTest, PublisherOperationsTest), SubscriberAdapter (ParticipantEntityTest, ListenerPropagationTest), DomainParticipantAdapter (ParticipantEntityTest, TopicOperationsTest, PublisherOperationsTest), DataReaderAdapter und DataWriterAdapter (ListenerTest, ParticipantSubscriberTest). (Commit `d23eff980f3`)
### §7.3.7.2 Callback ohne Entität, Quelle über den Status

**Spec:** §7.3.7.2, S. 23 — "In the DDS PIM, each listener callback receives two
arguments: the Entity, the status of which has changed, and the new value of
that status. In this PSM, the former is unnecessary and is omitted: it is
available through the read-only Source property of the status object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java` (`DataReaderListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/event/StatusChangedEvent.java` (`StatusChangedEvent.getSource`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`

**Status:** `done` — Jeder Callback hat genau ein Argument, ein StatusChangedEvent (erweitert java.util.EventObject) mit getSource() und getStatus(). Die Tests lesen die Entität über status.getSource() und prüfen assertSame auf den eigenen Reader bzw. Writer. Die Quelle liegt am Event, nicht an der Status-Klasse. (Commit `d23eff980f3`)
### §7.3.7.2 Begründung: Listener und Adapter

**Spec:** §7.3.7.2, S. 23 — "Design Rationale (non-normative) The listener +
adapter design pattern is consistent with that used in the standard AWT and
Swing UI libraries and elsewhere."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung (nicht normativ); keine erfüllbare Anforderung.
### §7.3.7.2 Niedere Listener generisch: TopicListener, DataReaderListener, DataWriterListener

**Spec:** §7.3.7.2, S. 23 — "The former category includes TopicListener,
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_reader_listener_fires_on_match_and_on_every_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_2_writer_listener_fires_on_publication_matched`

**Status:** `done` — DataReaderListener<TYPE>, DataWriterListener<TYPE> und TopicListener<TYPE> sind über den Datentyp parametrisiert; die Events tragen denselben Parameter. TopicImpl ruft onInconsistentTopic (TopicImpl.onInconsistentTopic). Tests: TopicOperationsTest::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener (TopicAdapter<String>, getSource() als Topic<String> ohne Umwandlung), ListenerTest::javapsm_2_reader_listener_fires_on_match_and_on_every_sample, ::javapsm_2_writer_listener_fires_on_publication_matched. (Commit `cd8f3d67ea9`)
### §7.3.7.2 Höhere Listener mit Platzhalter `?`, ohne Vererbung zu den niederen

**Spec:** §7.3.7.2, S. 23 — "The latter category includes PublisherListener,
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantEntityTest.java::javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_passes_offered_incompatible_qos_to_the_publisher`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_a_writer_without_listener_and_a_publisher_without_one_pass_it_to_the_participant_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_without_listener_passes_inconsistent_topic_to_the_participant_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerPropagationTest.java::javapsm_2_a_reader_listener_set_at_creation_sees_the_match_of_enable_and_the_subscriber_gets_data_on_readers`

**Status:** `done` — PublisherListener, SubscriberListener und DomainParticipantListener nehmen Events mit Platzhalter (z. B. PublicationMatchedEvent<?>); DomainParticipantListener erweitert PublisherListener und SubscriberListener. Die Klassen PublisherImpl, SubscriberImpl und DomainParticipantImpl speichern den Listener in EntityBase, und DataWriterImpl, DataReaderImpl und TopicImpl rufen ihn über ListenerUp auf. Tests: PublisherAdapter mit onPublicationMatched und onOfferedIncompatibleQos (ParticipantEntityTest::javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later, PublisherOperationsTest::javapsm_2_a_writer_without_listener_passes_offered_incompatible_qos_to_the_publisher), SubscriberAdapter mit onSubscriptionMatched (ParticipantEntityTest::javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later) und onDataOnReaders (ListenerPropagationTest), DomainParticipantAdapter mit onPublicationMatched, onOfferedIncompatibleQos und onInconsistentTopic (ParticipantEntityTest, PublisherOperationsTest, TopicOperationsTest). (Commit `f31b6fa11b5`)
### §7.3.7.3 Bedingungen mit Basis `org.omg.dds.core.Condition`

**Spec:** §7.3.7.3, S. 23 — "Conditions extend the base interface
org.omg.dds.core.Condition."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/Condition.java` (`Condition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/GuardCondition.java` (`GuardCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/ReadCondition.java` (`ReadCondition`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_read_condition_follows_the_states_of_the_samples_and_wakes_the_wait_set`

**Status:** `done` — org.omg.dds.core.Condition mit getTriggerValue; GuardCondition, StatusCondition und ReadCondition erweitern sie. Die Tests hängen alle drei Arten über attachCondition(Condition) an ein WaitSet und lesen getTriggerValue und die Liste der aktiven Conditions. (Commit `5041698cee3`)
### §7.3.7.3 `StatusCondition` generisch mit `getParent`

**Spec:** §7.3.7.3, S. 23 — "The interface StatusCondition, which extends
Condition, is a generic interface with a type parameter that is the type of the
Entity to which it belongs. This type parameter allows its getParent method to
be both polymorphic and type safe."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/StatusCondition.java` (`StatusCondition`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/StatusConditionImpl.java` (`StatusConditionImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_status_condition_triggers_on_the_status_it_is_enabled_for_and_has_its_entity_as_parent`

**Status:** `done` — StatusCondition<ENTITY extends Entity<?, ?>> erweitert Condition und hat ENTITY getParent(). Der Test typisiert StatusCondition<DataReader<byte[]>> ohne Umwandlung und prüft assertSame(reader, condition.getParent()). Er prüft außerdem Auslösen durch SUBSCRIPTION_MATCHED und Abschalten über setEnabledStatuses. (Commit `d23eff980f3`)
### §7.3.7.4 WaitSet mit Basis `org.omg.dds.core.WaitSet`

**Spec:** §7.3.7.4, S. 23 — "Wait sets extend the base interface
org.omg.dds.core.WaitSet."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_is_false_at_first_and_the_wait_set_knows_what_is_attached`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_second_thread_waiting_on_the_same_wait_set_is_refused`

**Status:** `done` — org.omg.dds.core.WaitSet ist eine abstrakte Klasse mit newWaitSet(ServiceEnvironment), attachCondition, detachCondition, getConditions und waitForConditions. Die Tests prüfen Anhängen (doppeltes Anhängen ohne Wirkung), Abhängen (PreconditionNotMetException bei nicht angehängter Condition) und die Zurückweisung eines zweiten wartenden Threads. (Commit `5041698cee3`)
### §7.3.7.4 `wait` als `waitForConditions`

**Spec:** §7.3.7.4, S. 24 — "Therefore, this PSM maps the DDS PIM wait operation
to the more explicit method name waitForConditions."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/WaitSet.java` (`WaitSet.waitForConditions`),
`crates/java-omgdds/java/src/main/java/org/zerodds/core/condition/WaitSetImpl.java` (`WaitSetImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_wait_with_a_timeout_ends_in_a_timeout_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WaitSetTest.java::javapsm_2_a_guard_condition_set_from_another_thread_ends_a_blocked_wait_and_is_reported`

**Status:** `done` — WaitSet hat sechs Überladungen waitForConditions (ohne Argumente, mit Liste aktiver Conditions, mit Timeout als Duration oder long/TimeUnit); eine Methode wait gibt es nicht. Die Tests prüfen TimeoutException nach 30 ms, das Blockieren ohne Timeout bis zum Setzen einer GuardCondition und die Rückgabe der ausgelösten Condition in der übergebenen Liste. (Commit `5041698cee3`)

---

## §7.4 Domain Module

### §7.4 Paket `org.omg.dds.domain`

**Spec:** §7.4, S. 24 — "This PSM realizes the Domain Module from the DDS
specification with the package org.omg.dds.domain. This package contains
DomainParticipant, DomainParticipantFactory, and so forth."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantListener.java` (`DomainParticipantListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — Das Paket org.omg.dds.domain enthält DomainParticipant, DomainParticipantFactory, DomainParticipantListener, DomainParticipantAdapter, DomainParticipantQos und DomainParticipantFactoryQos. Der Test erzeugt über DomainParticipantFactory einen DomainParticipant und führt damit Schreiben und Lesen aus. (Commit `0840f9ed436`)
### §7.4.1 DomainParticipantFactory je ServiceEnvironment ein Singleton

**Spec:** §7.4.1, S. 24 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton. An instance of this interface can be obtained
by passing that ServiceEnvironment to the factory’s getInstance method."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java` (`DomainParticipantFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreSpiTest.java::javapsm_7_2_4_several_threads_get_one_participant_factory_of_an_environment`

**Status:** `done` — getInstance(ServiceEnvironment) liefert env.getSPI().getParticipantFactory(); ZeroSPI hält je Umgebung eine DomainParticipantFactoryImpl, die beim ersten Zugriff unter Sperre angelegt wird. Tests: QuickstartApiTest::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property (assertSame für dieselbe Umgebung, assertNotSame gegen die Factory einer anderen), ::javapsm_7_4_1_get_instance_of_a_null_environment_is_a_null_pointer_exception, CoreSpiTest::javapsm_7_2_4_several_threads_get_one_participant_factory_of_an_environment (acht Threads, 25 Runden). (Commit `36e8fd3f496`)
### §7.4.1 Eigene Factory je ServiceEnvironment

**Spec:** §7.4.1, S. 24 — "The DomainParticipantFactory is a
per-ServiceEnvironment singleton."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.getParticipantFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantFactoryImpl.java` (`DomainParticipantFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantFactoryEnvironmentTest.java::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table`

**Status:** `done` — DomainParticipantFactoryImpl wird mit ihrer ServiceEnvironment angelegt (getEnvironment), führt eine eigene Tabelle der Participants (lookupParticipant) und eigene Default-QoS. Tests: QuickstartApiTest::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property (assertNotSame zwischen den Factories zweier Umgebungen), ParticipantFactoryEnvironmentTest::javapsm_7_4_1_every_environment_has_its_own_factory_and_its_own_participant_table (lookupParticipant der anderen Factory liefert null). Der InProcessBus ist ein gemeinsamer Singleton: die Participants beider Umgebungen in einer Domain entdecken sich (derselbe Test). (Commit `2e0b194caf5`)
### §7.4.2 DomainParticipant als Schnittstelle `org.omg.dds.domain.DomainParticipant`

**Spec:** §7.4.2, S. 24 — "This PSM represents the DomainParticipant classifier
from the DDS PIM with the interface org.omg.dds.domain.DomainParticipant."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_find_topic_returns_the_topic_and_times_out_otherwise`

**Status:** `done` — org.omg.dds.domain.DomainParticipant ist heute eine Schnittstelle (extends Entity); die Implementierung ist org.zerodds.domain.DomainParticipantImpl. Die Tests verwenden DomainParticipant nur als Schnittstellentyp, den die Factory liefert. (Commit `0840f9ed436`)

---

## §7.5 Topic Module

### §7.5 Pakete `org.omg.dds.type` und `org.omg.dds.topic`

**Spec:** §7.5, S. 24 — "This PSM realizes the Topic Module from the DDS
specification with the packages org.omg.dds.type and org.omg.dds.topic."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`

**Status:** `done` — Beide Pakete existieren: org.omg.dds.topic (Topic, TopicDescription, ContentFilteredTopic, MultiTopic, Listener, Adapter, Built-in-Daten) und org.omg.dds.type (TypeSupport, Annotationen wie Key, TypeKind, dynamic.*). PubSubLoopbackTest verwendet Topic; DynamicTopicTest verwendet org.omg.dds.type.Key, TypeKind und die Typen aus org.omg.dds.type.dynamic. (Commit `9267b77a38a`)
### §7.5.1 Je Typ eine Klasse mit Basis `org.omg.dds.type.TypeSupport`

**Spec:** §7.5.1, S. 24 — "As in the DDS PIM, each type to be published or
subscribed with DDS is represented by a class extending
org.omg.dds.type.TypeSupport."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription.getTypeSupport`),
`tools/idlc/src/java_support.rs` (`java_support::support`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_a_topic_made_from_a_class_has_a_type_support_of_that_class`,
`tools/idlc/tests/java_typesupport_base.rs::javapsm_7_5_1_generated_type_supports_extend_the_psm_type_support`

**Status:** `done` — `org.omg.dds.type.TypeSupport` ist eine abstrakte Klasse (`newData`, `getType`, `getTypeName`); `TypeSupportImpl` erweitert sie, und `TopicImpl` hält eine Instanz (`getTypeSupport`). `TopicOperationsTest::javapsm_7_5_1_a_topic_made_from_a_class_has_a_type_support_of_that_class` prüft das für ein Topic aus einer Klasse. Die von `tools/idlc/src/java_support.rs` erzeugten `Support_<tag>` in `ZdTypeSupports.java` erweitern `org.omg.dds.type.TypeSupport<…>` (mit `getEnvironment`, `newData`, `getType`) und implementieren weiter `org.zerodds.cdr.TopicTypeSupport`; `java_typesupport_base` erzeugt das Java für XCDR1 und XCDR2, übersetzt es mit `javac -Xlint:all -Werror` gegen die Quellen der Bindung und führt ein Programm aus, das die Klasse über die Basisklasse benutzt (Lauf auf dem Linux-Testhost mit JDK). (Commit `8aecff4864f`)
### §7.5.1 Statische Operation `newTypeSupport` mit Klasse und optionalem Namen

**Spec:** §7.5.1, S. 24 — "Applications obtain instances of these interfaces by
calling the static base class operation newTypeSupport, passing this method the
Java Class object of the type they wish to support and optionally a name under
which that type should be registered."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.ZeroSPI.newTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name`

**Status:** `done` — TypeSupport.newTypeSupport(Class, ServiceEnvironment) und newTypeSupport(Class, String, ServiceEnvironment) führen über env.getSPI().newTypeSupport zu einem TypeSupportImpl. Tests: TopicOperationsTest::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class (getType, getTypeName, newData, ein Topic daraus, Austausch eines Samples), ::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name (Form mit Namen). (Commit `36e8fd3f496`)
### §7.5.1 Ohne Namen unter dem voll qualifizierten Namen der Klasse

**Spec:** §7.5.1, S. 24 — "If no such name is provided, the type shall be
registered under the fully qualified name of the provided Class."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeSupport.java` (`TypeSupport.newTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.getTypeName`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class`

**Status:** `done` — newTypeSupport(Class, ServiceEnvironment) übergibt in der OMG-Quelle type.getClass().getName() („java.lang.Class“) als Namen; org/omg/dds bleibt unverändert, weil die OMG-Quelle normativ ist. ZeroSPI.newTypeSupport (ServiceEnvironmentImpl) behandelt „java.lang.Class“ für einen anderen Typ als Class selbst als „kein Name angegeben“ und registriert unter type.getName(), dem voll qualifizierten Namen der Klasse (§7.5.1). Ein Typ, den eine Anwendung ausdrücklich unter „java.lang.Class“ anmeldet, ist damit nicht möglich. Test: TopicOperationsTest::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class (getTypeName = String.class.getName(), auch am Topic). (Commit `36e8fd3f496`)
### §7.5.1 TypeSupport an `createTopic` statt `register_type`

**Spec:** §7.5.1, S. 24 — "This PSM instead asks applications to instantiate
each TypeSupport object with a name and then provide that TypeSupport itself to
the create_topic method."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name`

**Status:** `done` — DomainParticipant.createTopic(String, TypeSupport<TYPE>) und die Form mit QoS, Listener und Statusmenge erzeugen ein TopicImpl, das das TypeSupport hält (DomainParticipantImpl.newTopic); es gibt keine register_type-Operation. Tests: TopicOperationsTest::javapsm_7_5_1_new_type_support_without_a_name_registers_under_the_name_of_the_class (getTypeName und getTypeSupport des Topics sind die des TypeSupport, Writer und Reader tauschen ein Sample), ::javapsm_7_5_1_a_type_support_registered_under_a_name_gives_the_topic_its_writers_and_its_announcement_that_name. (Commit `cd8f3d67ea9`)
### §7.5.1 Begründung: statische Typsicherheit

**Spec:** §7.5.1, S. 24 — "Design Rationale (non-normative) By requiring the
application to pass an instance of the generic TypeSupport interface to the
createTopic method, this PSM maintains unbroken static type safety all the way
from type registration to data publication or reception."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung (nicht normativ); keine erfüllbare Anforderung.
### §7.5.2 Topic fügt nur den Zugriff auf INCONSISTENT_TOPIC hinzu

**Spec:** §7.5.2, S. 24 — "The Topic interface adds only a single operation to
the set of those it inherits from its TopicDescription and DomainEntity
super-types: an accessor for the inconsistent topic status."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic.getInconsistentTopicStatus`),
`crates/java-omgdds/java/src/main/java/org/zerodds/topic/TopicImpl.java` (`TopicImpl.getInconsistentTopicStatus`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_topics_of_the_same_type_or_in_another_domain_are_consistent`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantWireDiscoveryTest.java::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent`

**Status:** `done` — Topic erweitert TopicDescription und DomainEntity und fügt getInconsistentTopicStatus hinzu; TopicImpl.onInconsistentTopic wird von DomainParticipantImpl.checkTopicConsistency und remoteEndpointSeen gerufen. Tests: TopicOperationsTest::javapsm_2_a_topic_of_the_same_name_and_another_type_in_the_domain_is_inconsistent_and_tells_the_topic_listener (getInconsistentTopicStatus: total_count 1, Änderung 1 beim ersten und 0 beim zweiten Aufruf), ::javapsm_2_topics_of_the_same_type_or_in_another_domain_are_consistent, ParticipantWireDiscoveryTest::javapsm_2_a_topic_of_another_process_is_discovered_and_a_type_that_differs_is_inconsistent. (Commit `cd8f3d67ea9`)
### §7.5.2 Topic generisch über den Datentyp

**Spec:** §7.5.2, S. 24–25 — "Topic—like all TopicDescriptions, and like
DataReader and DataWriter—is a generic interface with a type parameter that
identifies the type of the data with which it is associated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — Topic<TYPE> und TopicDescription<TYPE> sind über den Datentyp parametrisiert, wie DataReader<TYPE> und DataWriter<TYPE>. Der Test verwendet Topic<Reading>, um DataWriter<Reading> und DataReader<Reading> zu erzeugen. (Commit `9267b77a38a`)
### §7.5.2 Typsicherheit von der Erzeugung bis zu Schreiben und Lesen

**Spec:** §7.5.2, S. 25 — "Although Topic provides no type-specific operations,
its type parameter preserves type safety from Topic creation (actually all the
way from type registration) through data publication and/or subscription."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — createTopic(String, Class<TYPE>) liefert Topic<TYPE>; Publisher.createDataWriter(Topic<TYPE>) liefert DataWriter<TYPE>; Subscriber.createDataReader(TopicDescription<TYPE>) liefert DataReader<TYPE>. QuickstartApiTest.javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code schreibt ein Temperature-Objekt und liest über Sample<Temperature>.getData().getCelsius() ohne Umwandlung. (Commit `a91e0f632bb`)
### §7.5.2 TopicDescription erweitert `java.io.Closeable`

**Spec:** §7.5.2, S. 25 — "TopicDescription interface extends java.io.Closeable
to support specific new language constructs (e.g., Java try-with-resources) for
dealing with this interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/TopicDescription.java` (`TopicDescription`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` (`Topic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/MultiTopic.java` (`MultiTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CloseCascadeTest.java::javapsm_7_2_3_participant_close_closes_contained_entities`

**Status:** `done` — TopicDescription<TYPE> erweitert Closeable und DDSObject; ContentFilteredTopic und MultiTopic schließen sich selbst (Eintrag aus der Liste des Participants, Freigabe der gehaltenen Topics). Tests: TopicOperationsTest::javapsm_7_5_2_a_topic_description_is_closeable_and_leaves_the_participant_when_the_resource_ends (try-with-resources für eine ContentFilteredTopic und ein MultiTopic, danach lookupTopicDescription null), ::javapsm_2_closing_a_content_filtered_topic_needs_its_readers_gone_and_removes_it_from_the_participant, ::javapsm_2_closing_a_multi_topic_needs_its_readers_gone_and_frees_the_topics_of_its_from_clause, CloseCascadeTest::javapsm_7_2_3_participant_close_closes_contained_entities. (Commit `36e8fd3f496`)
### §7.5.3 ContentFilteredTopic und MultiTopic generisch

**Spec:** §7.5.3, S. 25 — "ContentFilteredTopic and MultiTopic are generic
interfaces with type parameters that identify the types of the data with which
they are associated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/MultiTopic.java` (`MultiTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_2_the_content_filtered_topic_of_the_participant_filters_the_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicTest.java::javapsm_2_the_multi_topic_carries_its_expression`

**Status:** `done` — ContentFilteredTopic<TYPE> und MultiTopic<TYPE> sind Schnittstellen mit Typparameter (beide erweitern TopicDescription<TYPE>). ContentFilterTest verwendet ContentFilteredTopic<Reading> und prüft, dass ein Reader darauf nur das Sample mit value > 50 erhält; MultiTopicTest verwendet MultiTopic<Reading> mit Ausdruck und Parametern. (Commit `9267b77a38a`)
### §7.5.3 Typparameter der ContentFilteredTopic darf ein Obertyp sein

**Spec:** §7.5.3, S. 25 — "Note that the type parameter of a
ContentFilteredTopic does not need to match that of its related Topic exactly;
it can be any supertype. For example, if the user-defined type Bar extends the
user-defined type Foo, a ContentFilteredTopic<Foo> can wrap a Topic<Bar>."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` (`DomainParticipant.createContentFilteredTopic`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/topic/ContentFilteredTopic.java` (`ContentFilteredTopic.getRelatedTopic`),
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java` (`DomainParticipantImpl.createContentFilteredTopic`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TopicOperationsTest.java::javapsm_7_5_3_a_content_filtered_topic_of_a_supertype_filters_the_samples_of_the_subtype_by_the_supertype_fields`

**Status:** `done` — createContentFilteredTopic nimmt Topic<? extends TYPE>, getRelatedTopic liefert Topic<? extends TYPE>; der Filter wertet die Felder des Obertyps an Samples des Untertyps aus (ContentFilter nutzt die Klasse des Samples). Test: TopicOperationsTest::javapsm_7_5_3_a_content_filtered_topic_of_a_supertype_filters_the_samples_of_the_subtype_by_the_supertype_fields (ContentFilteredTopic<Foo> über Topic<Bar>, Bar extends Foo; der Reader erhält nur das Bar mit level > 5, als Bar mit dem Feld label). (Commit `9267b77a38a`)
### §7.5.4 Datentypen der eingebauten Topics in `org.omg.dds.topic`, nur Accessoren

**Spec:** §7.5.4, S. 25 — "The data types pertaining to the DDS built-in
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_the_builtin_reader_of_a_participant_sees_the_entities_of_the_domain`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicsTest.java::javapsm_2_get_discovered_topic_data_carries_the_topic_qos`

**Status:** `done` — Die vier Schnittstellen und BuiltinTopicKey in org.omg.dds.topic haben Getter für ihren Zustand und, wie die OMG-Quelle, copyFrom(src). DDS-Java-PSM 1.0 Klausel 2 macht die OMG-Quelldateien neben dem PDF normativ und bei Widerspruch maßgeblich; die Schnittstellen sind die Dateien der OMG-Quelle (CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source, SHA-256 je Datei), copyFrom gehört damit zur normativen API. Setter gibt es nur in den Klassen unter org.zerodds.topic. Die Tests lesen die Daten als Samples der eingebauten Reader. (Commit `7f102b80f83`)

---

## §7.6 Publication Module

### §7.6 Paket `org.omg.dds.pub`

**Spec:** §7.6, S. 25 — "This PSM realizes the Publication Module from the DDS
specification with the package org.omg.dds.pub."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/`,
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriterListener.java` (`DataWriterListener`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/PublisherListener.java` (`PublisherListener`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — Das Paket org.omg.dds.pub enthält Publisher, DataWriter, PublisherListener, DataWriterListener, PublisherAdapter, DataWriterAdapter, PublisherQos und DataWriterQos. Der Test verwendet Publisher und DataWriter aus diesem Paket. (Commit `a91e0f632bb`)
### §7.6 Begründung: Name „pub“

**Spec:** §7.6, S. 25 — "Design Rationale (non-normative) The term “pub” has
been preferred to the longer “publication” for the sake of brevity [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung (nicht normativ); keine erfüllbare Anforderung.
### §7.6.1 Publisher als Schnittstelle `org.omg.dds.pub.Publisher`

**Spec:** §7.6.1, S. 25 — "Publishers are represented by instances of the
org.omg.dds.pub.Publisher interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_contains_entity_walks_the_containment`

**Status:** `done` — org.omg.dds.pub.Publisher ist eine Schnittstelle (extends DomainEntity<PublisherListener, PublisherQos>); PublisherImpl implementiert sie. Die Tests halten das Ergebnis von DomainParticipant.createPublisher als Publisher und verwenden createDataWriter darauf. (Commit `a91e0f632bb`)
### §7.6.1 Überladung `lookupDataWriter(Topic)`

**Spec:** §7.6.1, S. 25 — "In addition to the methods defined for this interface
by [DDS], it additionally provides a lookupDataWriter overload that acts on the
basis of a Topic object rather than solely on the topic’s name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.lookupDataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/PublisherImpl.java` (`PublisherImpl.lookupDataWriter`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PublisherOperationsTest.java::javapsm_2_lookup_data_writer_finds_by_name_and_by_topic_and_forgets_closed_writers`

**Status:** `done` — Publisher hat lookupDataWriter(String topicName) und lookupDataWriter(Topic<TYPE>); PublisherImpl liefert den ersten Writer mit dem Topicnamen oder null (die Topic-Überladung vergleicht den Namen). Test: PublisherOperationsTest::javapsm_2_lookup_data_writer_finds_by_name_and_by_topic_and_forgets_closed_writers (beide Überladungen für ein vorhandenes und ein fehlendes Topic, null nach close). (Commit `a91e0f632bb`)
### §7.6.2 DataWriter als generische Schnittstelle `org.omg.dds.pub.DataWriter`

**Spec:** §7.6.2, S. 25 — "DataWriters are represented by instances of the
org.omg.dds.pub.DataWriter interface. This is a generic interface, parameterized
by the type of the data samples to be written by a given writer."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — org.omg.dds.pub.DataWriter<TYPE> ist heute eine Schnittstelle (extends DomainEntity<DataWriterListener<TYPE>, DataWriterQos>), parametrisiert über den Datentyp; DataWriterImpl implementiert sie. Der Test verwendet DataWriter<Reading> zum Schreiben. (Commit `5eb3cb8ca5b`)
### §7.6.2 Kein FooDataWriter, alle typspezifischen Operationen an DataWriter

**Spec:** §7.6.2, S. 25 — "This PSM makes no such distinction: Java’s generic
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key`

**Status:** `done` — write, registerInstance, unregisterInstance, dispose, getKeyValue und lookupInstance stehen an DataWriter<TYPE>; es gibt keinen FooDataWriter. tools/idlc/src/java_support.rs erzeugt nur ZdTypeSupports, keinen Writer je Typ (gelesen, nicht durch Test belegt). Die Tests schreiben über DataWriter<Reading> und prüfen registerInstance und lookupInstance (Handle ungleich nil, nil für unbekannten Schlüssel). (Commit `5eb3cb8ca5b`)
### §7.6.2 Zeitstempel und Handles als Überladungen

**Spec:** §7.6.2, S. 25–26 — "Java does; therefore, the provision of timestamps
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_7_6_2_every_write_overload_carries_its_source_timestamp_to_the_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed`

**Status:** `done` — DataWriter hat write, registerInstance, unregisterInstance und dispose je mit den Überladungen Sample, Sample mit Time oder long/TimeUnit, Sample mit InstanceHandle, Sample mit InstanceHandle und Zeitstempel. DataWriterImpl verwendet den Zeitstempel in Millisekunden als Source-Zeitstempel und prüft den Handle gegen die Instanz des Samples. Tests: WriterHandleTest::javapsm_7_6_2_every_write_overload_carries_its_source_timestamp_to_the_sample (Zeitstempel und Instanz-Handle am Sample des Readers für fünf Formen), ::javapsm_2_register_instance_takes_a_timestamp_and_lookup_instance_finds_the_instance_by_its_key, ::javapsm_2_dispose_by_handle_or_by_sample_makes_the_reader_see_the_instance_disposed (Formen mit Time und long/TimeUnit). (Commit `d70d1f69c5d`)
### §7.6.2 Bei ungeschlüsselten Typen ein nil-Handle

**Spec:** §7.6.2, S. 26 — "Users of DataWriters of unkeyed types may choose to
call the overloads that accept instance handle arguments; if they do, the handle
argument must be a nil handle (as explained in the DDS PIM)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java` (`DataWriterImpl.write`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/WriterHandleTest.java::javapsm_7_6_2_an_unkeyed_type_is_written_with_the_nil_handle`

**Status:** `done` — Die Überladungen mit InstanceHandle (write, registerInstance, dispose, unregisterInstance) sind vorhanden; DataWriterImpl.write(T, InstanceHandle, ...) lässt HANDLE_NIL zu und leitet die Instanz aus dem Schlüssel ab (für einen Typ ohne Schlüssel die eine Instanz). Test: WriterHandleTest::javapsm_7_6_2_an_unkeyed_type_is_written_with_the_nil_handle (byte[]-Topic, drei Formen mit Nil-Handle, alle drei Samples beim Reader, Zeitstempel der Formen mit Time und long/TimeUnit). (Commit `d70d1f69c5d`)

---

## §7.7 Subscription Module

### §7.7 Paket `org.omg.dds.sub`

**Spec:** §7.7, S. 26 — "This PSM realizes the Subscription Module from the DDS
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — Das Paket `org.omg.dds.sub` enthält `Subscriber`, `DataReader`, `Sample`, `SubscriberListener`, `DataReaderListener`, `ReadCondition` und `QueryCondition`. Der Test benutzt `Subscriber`, `DataReader` und `Sample` aus diesem Paket. Die Angabe im alten Text, `SubscriberListener` und die Bedingungen fehlten, trifft nicht mehr zu. (Commit `a91e0f632bb`)
### §7.7 Begründung: Name „sub“

**Spec:** §7.7, S. 26 — "Design Rationale (non-normative) The term “sub” has
been preferred to the longer “subscription” for the sake of brevity [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ (Begründung für den Paketnamen).
### §7.7.1 Subscriber als Schnittstelle `org.omg.dds.sub.Subscriber`

**Spec:** §7.7.1, S. 26 — "Subscribers are represented by instances of the
org.omg.dds.sub.Subscriber interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — `Subscriber` ist `public interface Subscriber`; `SubscriberImpl` implementiert sie. Der Test hält das Ergebnis von `participant.createSubscriber()` in einer Variablen vom Typ `Subscriber` und erzeugt darüber den Reader. Die Angabe im alten Text (finale Klasse mit öffentlichem Konstruktor) trifft nicht mehr zu. (Commit `a91e0f632bb`)
### §7.7.1 Überladung `lookupDataReader(TopicDescription)`

**Spec:** §7.7.1, S. 26 — "In addition to the methods defined for this interface
by [DDS], it additionally provides a lookupDataReader overload that acts on the
basis of a TopicDescription object rather than solely on the topic description’s
name."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.lookupDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SubscriberImpl.java` (`SubscriberImpl.lookupDataReader`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ParticipantSubscriberTest.java::javapsm_2_lookup_data_reader_finds_by_name_by_topic_and_by_filtered_topic`

**Status:** `done` — `lookupDataReader(String)` und `lookupDataReader(TopicDescription)` finden den Reader nach Name und nach Beschreibung, liefern `null` für ein unbekanntes Topic und nach dem Schließen des Readers; `null` als Beschreibung ergibt `IllegalArgumentException`. (Commit `a91e0f632bb`)
### §7.7.2 Sample als ein Objekt aus Daten und Metadaten, Schnittstelle `org.omg.dds.sub.Sample`

**Spec:** §7.7.2, S. 26 — "This PSM follows the guidance of the DDS PIM rather
than of the IDL PSM: it represents data samples as single objects that
incorporate both data and metadata. Each sample is represented by an instance of
the org.omg.dds.sub.Sample interface."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::javapsm_7_7_2_the_psm_sample_is_an_interface`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — `Sample<TYPE>` ist eine Schnittstelle mit `getData` und den Metadaten (`getSampleState`, `getViewState`, `getInstanceState` u. a.) in einem Objekt; `SampleImpl` implementiert sie. `javapsm_7_7_2_the_psm_sample_is_an_interface` prüft `Sample.class.isInterface()` und dass sie nicht final ist. Der zweite Test liest Daten aus den von `take` gelieferten Samples. (Commit `9267b77a38a`)
### §7.7.2 `getData` liefert die Daten

**Spec:** §7.7.2, S. 26 — "It provides its data via a getData method [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleImpl.java` (`SampleImpl.getData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::javapsm_7_7_3_read_returns_a_fresh_copy_not_the_readers_object`

**Status:** `done` — `javapsm_7_8_2_a_byte_array_topic_moves_the_octets` schreibt `"hello".getBytes()` und liest sie über `Sample.getData()` zurück; `javapsm_7_7_3_read_returns_a_fresh_copy_not_the_readers_object` prüft den Wert `getData().value == 42`. (Commit `9267b77a38a`)
### §7.7.2 `getData` liefert `null` ohne gültige Daten

**Spec:** §7.7.2, S. 26 — "if there is no valid data (corresponding to a false
value for SampleInfo.valid_data in the IDL PSM), this operation returns null."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.getData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.markState`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/MultiTopicJoinTest.java::javapsm_7_7_2_disposing_a_constituent_disposes_the_result_instance`

**Status:** `done` — `DataReaderImpl.markState` legt für NOT_ALIVE_DISPOSED und NOT_ALIVE_NO_WRITERS ein Sample mit `null` als Daten an. Der Test löst über ein Dispose eines Teil-Topics ein solches Sample im Reader des MultiTopic aus und prüft `assertNull(getData())` und `NOT_ALIVE_DISPOSED`. Für einen Reader eines gewöhnlichen Topics prüft kein Test `getData() == null`, ebenso nicht für NOT_ALIVE_NO_WRITERS. (Commit `f31b6fa11b5`)
### §7.7.2 Metadaten als nur lesbare Eigenschaften nach Java Beans

**Spec:** §7.7.2, S. 26 — "It provides its metadata (corresponding to the other
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_all_samples_of_an_instance_in_one_collection_have_the_same_view_state_and_the_sample_state_of_before_the_read`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/InstanceHandleTest.java::javapsm_2_a_writer_disposes_and_unregisters_the_instances_it_registered`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_disposed_and_written_again_counts_a_disposed_generation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_an_instance_without_writers_written_again_counts_a_no_writers_generation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_ranks_are_measured_within_the_samples_of_one_instance_in_the_collection`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_source_timestamp_is_the_one_the_writer_gave`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSampleInfoTest.java::javapsm_2_the_publication_handle_names_the_writer_that_is_matched_with_the_reader`

**Status:** `done` — `DataReaderImpl.toSample` und `SampleImpl.of` liefern disposed_generation_count, no_writers_generation_count, sample_rank, generation_rank und absolute_generation_rank aus den Zählern und der Sammlung (statt der Konstanten 0 und -1), dazu source_timestamp und publication_handle. Die Tests in `ReaderSampleInfoTest` prüfen alle sieben Werte. (Commit `e1b55167947`)
### §7.7.2 `Sample.Iterator` erweitert `ListIterator`, nur lesend

**Spec:** §7.7.2, S. 26 — "The Sample interface also defines a nested interface:
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_2_the_iterator_is_read_only`

**Status:** `done` — `SampleIterator.remove`, `set` und `add` werfen `UnsupportedOperationException` (DDS-Java-PSM 1.0 §7.7.2). Der Test ruft alle drei an einem Iterator von `DataReader.read()` auf und prüft, dass die Sammlung unverändert bleibt. (Commit `e0f89cfd194`)
### §7.7.3 DataReader als generische Schnittstelle `org.omg.dds.sub.DataReader`

**Spec:** §7.7.3, S. 26 — "DataReaders are represented by instances of the
org.omg.dds.sub.DataReader interface. This is a generic interface, parameterized
by the type of the data samples to be read by a given reader."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`

**Status:** `done` — `DataReader<TYPE>` ist `public interface DataReader<TYPE>`; `DataReaderImpl` implementiert sie. Der Test benutzt `DataReader<Reading>` und liest `Sample<Reading>` mit dem Datentyp `Reading`. Die Angabe im alten Text (finale Klasse) trifft nicht mehr zu. (Commit `5eb3cb8ca5b`)
### §7.7.3 Kein FooDataReader, alle typspezifischen Operationen an DataReader

**Spec:** §7.7.3, S. 26 — "This PSM makes no such distinction: Java’s generic
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::javapsm_2_get_key_value_returns_a_sample_of_the_instance_that_has_the_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderKeyTest.java::javapsm_2_lookup_instance_is_nil_before_the_instance_is_known_and_the_handle_of_the_sample_afterwards`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_read_next_sample_returns_the_next_sample_not_yet_read_and_take_next_sample_removes_it`

**Status:** `done` — Alle typspezifischen Operationen (`read`, `take`, `lookupInstance`, `getKeyValue`, `readNextSample`, `takeNextSample`) stehen an `DataReader`; `getKeyValue` setzt die Schlüsselfelder (siehe das Item zu get_key_value) und alle sind getestet. (Commit `e1b55167947`)
### §7.7.3 Zwei Arten von read und take: Loan mit `Sample.Iterator`, Kopie in eine `List` der Anwendung

**Spec:** §7.7.3, S. 26 — "The DataReader interface provides a number of read
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::javapsm_7_7_3_read_returns_a_fresh_copy_not_the_readers_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_the_iterator_walks_the_samples_in_both_directions`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_take_returns_the_samples_in_the_iterator_and_removes_them_from_the_reader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_read_selector_and_int_forms_return_the_same_samples_as_the_list_form`

**Status:** `done` — `read()`, `take()`, `read(int)`, `take(Selector)` liefern einen `Sample.Iterator`: Inhalt und Reihenfolge, Richtungswechsel, READ nach read, Entfernen nach take sind getestet. (Commit `e1b55167947`)
### §7.7.3 `returnLoan` am Iterator, Iterator ist Closeable

**Spec:** §7.7.3, S. 27 — "Applications that read or take loans must eventually
return those loans; this PSM maps the return_loan operation from the DDS PIM to
an operation returnLoan on the Sample.Iterator. Moreover, the iterator
implements the Java.io.Closeable interface so that try-with-resources construct
can be used in Java 7."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java` (`Sample.Iterator.close`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SampleIterator.java` (`SampleIterator.close`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_close_ends_the_loan_and_closing_again_is_harmless`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderIteratorTest.java::javapsm_7_7_3_the_iterator_walks_the_samples_in_both_directions`

**Status:** `done` — `SampleIterator.close` wirft beim zweiten Aufruf nichts mehr (Closeable, Sample.Iterator: close ohne Leihgabe ist ohne Wirkung); try-with-resources über `read()` und `take()` ist getestet. Die OMG-API nennt die Operation `close`; der Name `returnLoan` des PSM-Texts ist dort nicht vorhanden (org.omg.dds wird nicht geändert). (Commit `e0f89cfd194`)
### §7.7.3 Kopie: Liste mit beliebig vielen Samples, danach die angeforderte Zahl

**Spec:** §7.7.3, S. 27 — "Applications that read or take copies may provide to
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReadCopyTest.java::javapsm_7_7_3_read_returns_a_fresh_copy_not_the_readers_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_copy_flavour_replaces_what_the_list_held_and_allocates_a_list_for_null`

**Status:** `done` — `read(List)`, `take(List)`, `read(List, Selector)`, `take(List, Selector)` und `Selector.read(List)`/`take(List)` ersetzen den Inhalt der Liste durch die gelieferten Samples (zu lange Listen werden gekürzt, kurze erweitert); `null` ergibt eine neue Liste. Die Zahl ist `maxSamples` des Selectors (DDS-Java-PSM §7.7.3: die angeforderte Zahl), nicht die Länge der Liste. (Commit `e1b55167947`)
### §7.7.3 Kopie darf übergebene Samples überschreiben

**Spec:** §7.7.3, S. 27 — "The Service implementation may—for example, in order
to avoid object allocations—elect to overwrite the contents of any Samples that
are passed into it by invocations of these methods."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis für die Implementierung ("may"); die Anforderung fordert nichts. `DataReaderImpl.toSample` überschreibt keine übergebenen Samples, sondern legt neue an.
### §7.7.3 `DataReader.Selector` kapselt die Auswahlkriterien

**Spec:** §7.7.3, S. 27 — "Instead of overloading several operation variants
that accept large numbers of infrequently used parameters, a DataReader.Selector
is provided to encapsulate various selection criteria (for example, sets of
sample, instance, and view states)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_7_7_3_the_selector_of_read_and_take_carries_the_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_an_instance_handle_limits_read_and_take_to_that_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_next_instance_walks_the_instances_in_the_order_of_their_handles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_the_data_state_selects_on_sample_view_and_instance_states`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_max_samples_limits_read_and_take_and_unlimited_is_accepted_as_a_value`

**Status:** `done` — Selector.instance, nextInstance, dataState, Content und maxSamples sind über `read(Selector)`/`take(Selector)` getestet; nextInstance liefert die nächste Instanz nach Handle (auch zusammen mit instance(h)). (Commit `e1b55167947`)
### §7.7.3 `DataReader.select` liefert einen Selector mit den Vorgaben

**Spec:** §7.7.3, S. 27 — "DataReader.select method returns a Selector object,
which encapsulates the default selection criteria."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.select`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.select`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_7_7_3_the_selector_of_read_and_take_carries_the_expression`

**Status:** `done` — `DataReaderImpl.select` gibt ein `SelectorImpl` zurück, das an den Reader gebunden ist. Der Test ruft `reader.select().Content(...)` auf und liest über `selector.read(got)` aus demselben Reader. Die einzelnen Vorgaben des Selector stehen in den folgenden Items. (Commit `5eb3cb8ca5b`)
### §7.7.3 Vorgabe des Selector: `instanceHandle=null`

**Spec:** §7.7.3, S. 27 — "For portability, the default state of the Selector
object is defined as instanceHandle=null, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.getInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`

**Status:** `done` — `select().getInstance()` ist `null`; `read()` mit dem Standard-Selector liefert die Samples aller Instanzen (drei Instanzen, sechs Samples). (Commit `e1b55167947`)
### §7.7.3 Vorgabe des Selector: `nextInstance=false`

**Spec:** §7.7.3, S. 27 — "[...] nextInstance=false, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.retrieveNextInstance`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.retrieveNextInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`

**Status:** `done` — `select().retrieveNextInstance()` ist `false`. (Commit `e1b55167947`)
### §7.7.3 Vorgabe des Selector: `dataState=any`

**Spec:** §7.7.3, S. 27 — "[...] dataState=any, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getDataState`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.sampleStates`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`

**Status:** `done` — `SelectorImpl.getDataState()` liefert ohne Setzen `DataStateImpl.any(environment)` (alle Sample-, View- und Instanzzustände); der Test vergleicht mit `assertEquals`. (Commit `e0f89cfd194`)
### §7.7.3 Vorgabe des Selector: `queryExpression=null`

**Spec:** §7.7.3, S. 27 — "[...] queryExpression=null, [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getQueryExpression`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java` (`DataReaderImpl.collectLocked`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_7_7_3_the_selector_of_read_and_take_carries_the_expression`

**Status:** `done` — `select().getQueryExpression()` ist `null` und `read()` ohne Ausdruck liefert alle Samples; mit `Content(...)` filtert `collectLocked`. (Commit `e1b55167947`)
### §7.7.3 Vorgabe des Selector: `maxSamples=unlimited`

**Spec:** §7.7.3, S. 27 — "[...] and maxSamples=unlimited."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.maxSamples`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl.getMaxSamples`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_max_samples_limits_read_and_take_and_unlimited_is_accepted_as_a_value`

**Status:** `done` — `SelectorImpl.maxSamples` akzeptiert `ResourceLimits.LENGTH_UNLIMITED` und Werte ab 0, andere negative Werte ergeben `IllegalArgumentException`; `getMaxSamples()` liefert standardmäßig `LENGTH_UNLIMITED`. `read(Selector)`/`take(Selector)` begrenzen auf die Zahl. (Commit `e0f89cfd194`)
### §7.7.3 Selector als fluent Schnittstelle mit eigenem read und take

**Spec:** §7.7.3, S. 27 — "Selector provides fluent interface to modify the
default selection parameters. For convenience, Selector provides read and take
methods."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector`),
`crates/java-omgdds/java/src/main/java/org/zerodds/sub/SelectorImpl.java` (`SelectorImpl`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.read`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader.Selector.take`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/ContentFilterTest.java::javapsm_7_7_3_the_selector_of_read_and_take_carries_the_expression`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_default_selector_selects_everything`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_an_instance_handle_limits_read_and_take_to_that_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_2_next_instance_walks_the_instances_in_the_order_of_their_handles`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ReaderSelectorTest.java::javapsm_7_7_3_the_copy_flavour_replaces_what_the_list_held_and_allocates_a_list_for_null`

**Status:** `done` — Der Selector ist fluent (`instance`, `nextInstance`, `dataState`, `Content`, `maxSamples` liefern ihn zurück) und hat `read()`, `read(List)`, `take()`, `take(List)`; verkettete Aufrufe sind getestet. (Commit `e1b55167947`)

---

## §7.8 Extensible and Dynamic Topic Types Module

### §7.8 Typen der TypeObject-Darstellung in `org.omg.dds.type.typeobject`

**Spec:** §7.8, S. 27 — "Types pertaining to TypeObject Type Representations are
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
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::javapsm_7_8_the_type_object_of_xtypes_1_1_annex_b_is_a_pl_cdr_parameter_list`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::javapsm_7_8_a_java_class_becomes_a_structure_type_with_its_members`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::javapsm_7_8_the_builtin_data_describes_its_type_from_the_announced_bytes`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_2_1_every_interface_of_the_type_object_package_is_implemented`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::javapsm_7_8_3_member_ids_are_nested_final_classes_of_int_constants`

**Status:** `done` — Das Paket `org.omg.dds.type.typeobject` stimmt dateiweise mit der OMG-Quelle überein und ist umgesetzt. Die Schnittstellen bilden die TypeObject-Bibliothek von DDS-XTypes 1.1 Anhang B ab (formal/2012-11-10, im Spec-Cache `.agent/spec/dds-xtypes-1.1.txt`); `idl/typeobject/dds_xtypes_11_typeobject.idl` übernimmt das IDL des Anhangs in der Annotationssyntax von IDL 4 (`@extensibility` statt `@Extensibility(...)`, `@id(<Wert>)`, `@external` statt `@Shared`, `bitmask` statt `@BitSet`-Aufzählung) mit vier Änderungen, die keinen Wert und keine Kennung betreffen: der Typ `_TypeId` heißt in Java `TypeId`; das Member `annotation` heißt `_annotation`, weil `annotation` in IDL 4 ein Schlüsselwort ist; `Bound` heißt `CollectionBound`, weil `bound` und `Bound` in IDL ein Bezeichner sind; `MapType`, `SequenceType` und `StringType` sind `@mutable` wie ihre Basis `CollectionType` (der Anhang schreibt EXTENSIBLE, die Member haben aber Kennungen). Der Anhang vergibt dem Enumerator `MODULE_ELEMENT` den Wert 22, den auch `STRUCTURE_TYPE_ELEMENT` hat; hier ist es 24. Aus dem IDL erzeugt `scripts/generate-typeobject.sh` mit `idlc` (XCDR1) Datentypen und Codec (`org/zerodds/typeobject/generated`). Die TypeId eines konstruierten Typs ist die niederwertige Hälfte des MD5 der Big-Endian-Serialisierung des Typs (§7.3.4.1.2; der Anhang lässt offen, was die eigene `type_id` beim Hashen trägt: hier `NO_TYPE`). `TypeObjectBuilder` schreibt das TypeObject eines `DynamicType` (Art, Name, Extensibility, Basistyp, Member mit Kennung, Key, Optional, Union-Etiketten, Enumerationskonstanten, Grenzen), `TypeObjectModel` setzt die Schnittstellen des Pakets auf ein gelesenes TypeObject, `TypeObjectToDynamic` bildet den `DynamicType` zurück. Die Schnittstellen haben ganze Zahlen als Typkennung; die Abbildung steht in `TypeObjectModel`: primitive Typen haben die `TypeId`-Konstante der PSM, konstruierte `0x10000` plus Index in der Bibliothek. Ein Annotationsgebrauch wird gelesen, aber nicht geschrieben (die Liste der Gebrauche ist leer); eine Bitmenge nennt keine Bits; eine Union kennt keinen Diskriminator-Typ (der Anhang hat kein Member dafür). (Commit `b4bf3fa978e`)
### §7.8 Dynamic Language Binding in `org.omg.dds.type.dynamic`

**Spec:** §7.8, S. 27 — "Types pertaining to the Dynamic Language Binding are
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_every_java_primitive_has_the_dds_type_of_table_8_1`

**Status:** `done` — Das Paket `org.omg.dds.type.dynamic` enthält `DynamicType`, `DynamicTypeMember`, `DynamicData`, beide Factories und die drei Deskriptoren (dateiweise gleich den OMG-Quellen). `DynamicTopicTest` erzeugt Typ und Daten über diese Schnittstellen und bewegt `DynamicData` durch Writer und Reader; `DynamicTypeReflectionTest` liest `DynamicType` aus Klassen. Die Angabe im alten Text (Paket fehlt, `org.zerodds.cdr.DynamicType` außerhalb) trifft nicht mehr zu. (Commit `5041698cee3`)
### §7.8 `TypeKind` in `org.omg.dds.type`

**Spec:** §7.8, S. 27 — "The TypeKind enumeration, which pertains to both of the
above, is defined in the package org.omg.dds.type."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/TypeKind.java` (`TypeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_every_java_primitive_has_the_dds_type_of_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`

**Status:** `done` — `org.omg.dds.type.TypeKind` ist vorhanden (gleich der OMG-Quelle) mit den Arten von `NO_TYPE` bis `ANNOTATION_TYPE`, darunter die vorzeichenlosen Typen, `CHAR_32_TYPE`, `FLOAT_128_TYPE`, Aufzählung, Bitmenge, Union und Array. Die Tests vergleichen `DynamicType.getKind()` mit Werten dieses Enums. Der alte Text (`DynamicType.Kind` mit zwölf Werten) beschreibt den Altbestand `org.zerodds.cdr.DynamicType`. (Commit `5041698cee3`)
### §7.8 Eingebaute Typen in `org.omg.dds.type.builtin`

**Spec:** §7.8, S. 27 — "The built-in types are defined in the package
org.omg.dds.type.builtin."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_copy_from_clone_and_equals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_survives_java_serialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_copy_from_clone_and_equals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_survives_java_serialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key`

**Status:** `done` — Das Paket `org.omg.dds.type.builtin` ist vorhanden mit `KeyedString` und `KeyedBytes`; `BuiltinTypes` enthält `KeyedStringImpl` und `KeyedBytesImpl`, `ServiceEnvironment.getSPI().newKeyedString()`/`newKeyedBytes()` erzeugen sie. `TypesBuiltinTest` prüft `setKey`/`setValue` (Rückgabe des Objekts, Kopie des Byte-Arrays beim Setzen und Lesen, Bereichsform von `setValue`), `copyFrom`, `clone`, `equals`/`hashCode` und die Java-Serialisierung, dazu je einen Writer und Reader auf einem Topic des Typs. Die TypeSupports der Typen stehen in `BuiltinTypeSupports` (Items zu `DDS::KeyedString` und `DDS::KeyedBytes`). (Commit `ec44921ff1d`)
### §7.8 Erweiterungen von [DDS-XTypes] in den Datentypen der eingebauten Topics

**Spec:** §7.8, S. 27 — "Extensions by [DDS-XTypes] to types defined by [DDS]
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_7_8_the_type_members_are_on_their_parameter_ids`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_7_8_sedp_carries_the_type_information_on_its_parameter_id`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::javapsm_7_8_the_publication_data_carries_the_representation_and_the_type_consistency_of_its_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::javapsm_7_8_an_exact_type_consistency_forbids_coercion_on_the_wire`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::javapsm_7_8_the_subscription_data_carries_the_representation_and_the_type_consistency_of_its_reader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::javapsm_7_8_the_topic_data_carries_the_representation_and_the_type_consistency_of_its_topic`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::xtypes_7_6_3_2_a_remote_endpoint_announces_its_representation_and_type_consistency_in_sedp`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTopicDataTest.java::javapsm_7_8_an_endpoint_that_states_neither_has_the_defaults_of_the_entity_qos`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::javapsm_7_8_the_builtin_data_describes_its_type_from_the_announced_bytes`

**Status:** `done` — Die Datentypen der eingebauten Topics (`PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`, `TopicBuiltinTopicData`) tragen `getEquivalentTypeName`, `getBaseTypeName`, `getType`, `getRepresentation` und `getTypeConsistency`. Getestet sind `equivalentTypeName`, `baseTypeName` und die Type-Information-Oktette über Wire und SEDP (`BuiltinTopicQosTest`) sowie `getRepresentation` und `getTypeConsistency` aus der QoS lokaler Entities, auf den Parameter-IDs 0x0073 und 0x0074 in der Serialisierung der Built-in-Topic-Daten, nach dem Rücklesen und aus einer SEDP-Ankündigung (`TypesBuiltinTopicDataTest`); `EXACT_TYPE`, `EXACT_NAME` werden als DISALLOW_TYPE_COERCION (0), `ASSIGNABLE` und `DECLARED` als ALLOW_TYPE_COERCION (1) geschrieben. `getType()` liefert das TypeObject nach XTypes 1.1 (PID 0x0072, `@id(0x0072) type`): für lokale Entitäten aus dem `DynamicType` des TypeSupport (`TypeObjects.announce` in `BuiltinData.describeType`), für entfernte aus den angekündigten Oktetten (`TypeObjects.decodeOrNull`); Oktette, die kein TypeObject sind, geben `null`, und `getTypeObjectBytes()` behält sie. Die Abbildung zwischen den Fassungen: auf dem Draht gilt XTypes 1.3 (TypeInformation 0x0075 bleibt die Angabe, die die Peers lesen), der Parameter 0x0072 trägt das TypeObject der Fassung 1.1 nach Anhang B, das die Java-PSM-Schnittstellen abbilden. (Commit `b4bf3fa978e`)
### §7.8.1 Bestandteile des Dynamic Language Binding

**Spec:** §7.8.1, S. 27 — "The Dynamic Language Binding, as defined by
[DDS-XTypes], consists of DynamicType, DynamicTypeMember, DynamicData, their
respective factories, and several “descriptor” value types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung der Bestandteile; die Anforderungen stehen in §7.8.1.1 bis §7.8.1.5.
### §7.8.1.1 DynamicTypeFactory je ServiceEnvironment ein Singleton (`DynamicTypeFactory.getInstance(env)`)

**Spec:** §7.8.1.1, S. 27 — "This abstract factory is a per-ServiceEnvironment
singleton."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getInstance`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.getSPI`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_creates_the_status_and_condition_factories`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_1_the_factory_is_a_singleton_per_service_environment`

**Status:** `done` — `DynamicTypeFactory.getInstance(env)` liefert `env.getSPI().getTypeFactory()`; `ServiceEnvironmentImpl.getSPI` legt je Environment genau einen SPI mit einer `DynamicTypeFactoryImpl` an. `javapsm_7_8_1_1_the_factory_is_a_singleton_per_service_environment` prüft, dass zwei Aufrufe dasselbe Objekt liefern, dass es das der SPI ist, dass ein zweites `ServiceEnvironmentImpl` eine andere Factory hat (mit anderen Primitivtypen) und dass diese ihrerseits stabil ist. (Commit `a25948e911c`)
### §7.8.1.1 Kein `delete_instance` an `DynamicTypeFactory`, der Service verwaltet die Lebensdauer

**Spec:** §7.8.1.1, S. 27 — "The static delete_instance operations defined in
[DDS-XTypes] have been omitted in this PSM; the Service shall manage the life
cycles of the factory."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_1_there_is_no_delete_instance_and_the_service_manages_the_life_cycle`

**Status:** `done` — `org.omg.dds.type.dynamic.DynamicTypeFactory` und `DynamicTypeFactoryImpl` haben keine Operation `deleteInstance` oder ähnliche; die Lebensdauer richtet sich nach dem `ServiceEnvironment`. `javapsm_7_8_1_1_there_is_no_delete_instance_and_the_service_manages_the_life_cycle` prüft per Reflection beide Klassen auf Methoden mit `delete` im Namen und dass `getInstance` die einzige statische Methode der abstrakten Factory ist. (Commit `a25948e911c`)
### §7.8.1.2 DynamicTypeSupport entfällt

**Spec:** §7.8.1.2, S. 27 — "The interface DynamicTypeSupport defined by
[DDS-XTypes] does not provide any capability beyond what the generic TypeSupport
interface provided by this PSM already provides. Therefore, it has been omitted
from this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Spec beschreibt, dass `DynamicTypeSupport` in dieser PSM entfällt; `org.omg.dds.type` und `org.omg.dds.type.dynamic` enthalten keine solche Schnittstelle (Verzeichnisliste gelesen). Die Stelle verlangt nichts Erfüllbares.
### §7.8.1.3 DynamicType und DynamicTypeMember als Schnittstellen nach den Regeln dieses Dokuments

**Spec:** §7.8.1.3, S. 28 — "These interfaces are expressed in this PSM
according to the mapping rules expressed elsewhere in this document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeMember.java` (`DynamicTypeMember`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_5_the_key_annotation_makes_the_topic_keyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_members_are_found_by_id_and_by_name`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_add_members_adds_each_descriptor_and_returns_the_members_of_the_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_annotations_of_a_type_are_read_by_index_and_by_name`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_annotations_of_a_member_are_a_list_in_the_order_they_were_set`

**Status:** `done` — `DynamicType` und `DynamicTypeMember` sind Schnittstellen in `org.omg.dds.type.dynamic` mit `getDescriptor`, `getName`, `getKind`, `getMember`, `getAllMembers`, den Annotationsoperationen und `addMember`; `DynamicTypeImpl` und `DynamicTypeMemberImpl` setzen sie um. `TypesDynamicTypeTest` prüft `getMember(int)`/`getMember(String)` (auch unbekannte), `getAllMembers()`, `getAllMembers(int...)`, `getAllMembers(String...)`, `addMembers`, `getAnnotation(int)`/`getAnnotation(String)`, `getAllAnnotations`, `getAnnotations(int...)`/`getAnnotations(String...)`, `setAnnotation(s)` und an `DynamicTypeMember` `getId`, `getName`, `getDescriptor`, `getAnnotations`. `equals` und `clone` stehen im Item zu §7.8.1.3 `equals`/`clone`. Der Altbestand `org.zerodds.cdr.DynamicType` ist daneben noch vorhanden. (Commit `a25948e911c`)
### §7.8.1.3 Ergebnis direkt als Rückgabewert statt über in-out-Parameter und ReturnCode_t

**Spec:** §7.8.1.3, S. 28 — "Operations that provide their result as an in-out
value in their first parameter and return DDS::ReturnCode_t have been changed
such that they instead return their results directly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.getDescriptor`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.getMember`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.getPrimitiveType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory.createType`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_every_java_primitive_has_the_dds_type_of_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`

**Status:** `done` — Die Operationen von `DynamicType`, `DynamicTypeMember` und der Factory liefern ihr Ergebnis als Rückgabewert (`getDescriptor()`, `getMember(...)`, `createType(...)`, `getPrimitiveType(...)`); es gibt keinen in-out-Parameter und kein `ReturnCode_t`. Die Tests verketten diese Aufrufe, z. B. `type.getMember(name).getDescriptor().getType().getKind()`. (Commit `5041698cee3`)
### §7.8.1.3 `equals` und `clone` als `Object.equals` und `Object.clone`

**Spec:** §7.8.1.3, S. 28 — "The equals and clone operations on these types have
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_two_types_with_the_same_properties_are_equal`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_members_are_equal_when_their_type_descriptor_and_annotations_are`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_clone_is_an_equal_type_that_shares_no_member_with_the_original`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_5_type_descriptors_are_equal_when_all_their_properties_are`

**Status:** `done` — `DynamicTypeImpl.equals`/`hashCode` vergleichen nach XTypes 1.3 §7.5.2.8.4 (Table 54) Deskriptor, Mitglieder (nach Id) und Annotationen; `DynamicTypeMemberImpl.equals` nach §7.5.2.6.3 Deskriptor, Annotationen und den Deskriptor des Typs, zu dem das Mitglied gehört; die Deskriptoren vergleichen alle Eigenschaften der Tabellen 50 und 53 und §7.5.2.3.2. `DynamicTypeImpl.clone()` liefert einen gleichen Typ ohne gemeinsame Mitglieder (Änderungen an Kopie oder Original wirken nicht auf das andere). Die PSM hat an `DynamicTypeMember` kein `clone`. Getestet in `TypesDynamicTypeTest`. (Commit `a25948e911c`)
### §7.8.1.3 DynamicTypeMember als Referenztyp aus `DynamicType.addMember`

**Spec:** §7.8.1.3, S. 28 — "DynamicTypeMember is a reference type, and
instances of it are obtained from DynamicType.addMember. This change avoids the
need to provide an additional factory method for DynamicTypeMember instances."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicType.java` (`DynamicType.addMember`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.addMember`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_5_the_key_annotation_makes_the_topic_keyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_structure_with_a_nested_structure_and_arrays_has_the_octets_of_xtypes_7_4`

**Status:** `done` — `DynamicTypeImpl.addMember(MemberDescriptor)` legt das `DynamicTypeMember` im Typ an und gibt dasselbe Objekt zurück; `DynamicTypeFactory` hat keine Factory-Methode für Mitglieder (nur `newMemberDescriptor`). In `javapsm_7_8_1_5_the_key_annotation_makes_the_topic_keyed` setzt der Test `setAnnotation(Key)` auf dem von `addMember` gelieferten Mitglied, und der Codec des Typs ist danach verschlüsselt (`isKeyed`). Das beweist, dass der Rückgabewert das Mitglied des Typs ist. (Commit `7d991374fd4`)
### §7.8.1.3 `getAnnotations` statt `get_annotation_count` und `get_annotation`, Liste nicht änderbar

**Spec:** §7.8.1.3, S. 28 — "On each type, the operations get_annotation_count
and get_annotation (by index) have been unified into a single getAnnotations
method that returns a list of annotations. The lists returned from these methods
shall not be modifiable."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeMember.java` (`DynamicTypeMember.getAnnotations`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeMemberImpl.java` (`DynamicTypeMemberImpl.getAnnotations`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeImpl.java` (`DynamicTypeImpl.getAllAnnotations`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_the_lists_and_maps_of_annotations_cannot_be_modified`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_annotations_of_a_member_are_a_list_in_the_order_they_were_set`

**Status:** `done` — `DynamicTypeMember.getAnnotations()` liefert eine nicht änderbare Liste (`Collections.unmodifiableList` über eine Kopie); `DynamicType.getAllAnnotations()`, `getAnnotations(int...)` und `getAnnotations(String...)` liefern nicht änderbare Maps. `javapsm_7_8_1_3_the_lists_and_maps_of_annotations_cannot_be_modified` prüft `add`/`clear`/`remove`/`put` auf `UnsupportedOperationException` und dass der Typ danach unverändert ist. `DynamicType` hat statt einer einzelnen Listenmethode `getAllAnnotations()` und die beiden `getAnnotations`-Formen. Die Maps von `getAllMembers` bleiben änderbare Kopien (die Spec nennt nur die Annotationslisten). (Commit `a25948e911c`)
### §7.8.1.3 Zusätzliche Factory-Methode `createType(Class<?>)` nach §8

**Spec:** §7.8.1.3, S. 28 — "In addition to the methods specified by
[DDS-XTypes], DynamicTypeFactory provides one additional factory method:
createType(Class<?>). This method shall inspect the given type reflectively in
accordance with the Java Type Representation (see Clause 8) and instantiate an
equivalent DynamicType object."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.createType`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicTypeFactory.java` (`DynamicTypeFactory`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.getInstance`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_every_java_primitive_has_the_dds_type_of_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_7_8_1_3_a_java_type_without_a_kind_is_refused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_7_8_1_3_create_type_reflects_member_kinds_nesting_and_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_3_create_type_is_reached_through_the_implementation_of_the_factory`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_8_6_1_1_a_derived_class_is_a_structure_with_the_superclass_as_base_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_8_6_1_1_the_members_of_the_base_type_come_first_in_the_octets_of_a_derived_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`

**Status:** `done` — `DynamicTypeFactoryImpl.createType(Class<?>)` bildet eine Klasse nach der Java Type Representation (Tabelle 8.1, Aufzählung, Struktur, Union, Sammlungen, Map, Annotationen, Oberklasse als Basistyp, nur die Felder, die `java.io.Serializable` zuweist) auf einen `DynamicType` ab und weist Klassen ohne Art mit `IllegalArgumentException` zurück; `DynamicTypeReflectionTest` und `TypesDynamicTypeTest` prüfen das. Die Methode steht nicht in `org.omg.dds.type.dynamic.DynamicTypeFactory`: die Quelle `omgdds_src.zip`, die nach §2 bei einem Widerspruch gilt, hat sie nicht, und `org/omg/dds` bleibt gleich der Quelle (`CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`). Zugang: `DynamicTypeFactoryImpl.getInstance(env)` liefert dieselbe Factory wie `DynamicTypeFactory.getInstance(env)` mit dem Typ der Implementierung (`javapsm_7_8_1_3_create_type_is_reached_through_the_implementation_of_the_factory`). Der Altbestand `org.zerodds.cdr.DynamicTypeFactory.createType` liefert den alten `org.zerodds.cdr.DynamicType`. (Commit `72b36ae5db2`)
### §7.8.1.4 DynamicData als Schnittstelle nach den Regeln dieses Dokuments

**Spec:** §7.8.1.4, S. 28 — "This interface is expressed in this PSM according
to the mapping rules expressed elsewhere in this document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicDataFactory.java` (`DynamicDataFactory.newDynamicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_structure_with_a_nested_structure_and_arrays_has_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_union_is_its_discriminator_and_the_selected_member`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_every_primitive_kind_is_read_back_what_was_set`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_smaller_signed_kind_is_promoted_to_the_larger_accessor`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_value_outside_the_range_of_the_member_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_descriptors_and_member_ids_follow_the_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_members_by_name_are_the_members_by_id`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_clearing_a_non_optional_member_sets_it_to_its_default`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_clearing_an_optional_member_removes_it`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_clear_all_values_clears_every_member_and_clear_nonkey_values_keeps_the_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_loaned_structure_is_the_nested_value_and_changes_to_it_are_changes_to_the_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_the_loan_is_returned_only_once_and_only_by_sample_it_came_from`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_value_that_does_not_exist_is_not_loaned`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_loaned_primitive_is_written_back_on_return`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_clone_is_an_equal_sample_that_shares_no_value`

**Status:** `done` — `DynamicData` ist eine Schnittstelle in `org.omg.dds.type.dynamic`; `DynamicDataImpl` setzt sie um, `DynamicDataFactory.newDynamicData` erzeugt sie. `TypesDynamicDataTest` prüft die Zugriffe für Int16, Int32, Int64, Float32, Float64, Char, Byte, Boolean und String (direkt und nach Kodieren/Dekodieren), die Hochstufung auf den größeren Zugriff, `getDescriptors`, `getMemberIdByName`, `getMemberIdAtIndex`, `clearValue` (nicht optional: Vorgabewert; optional: entfernt), `clearAllValues`, `clearNonkeyValues` (Schlüsselwerte bleiben), `loanValue`/`returnLoanedValue` (nach XTypes 1.3 §7.5.2.11.7: ohne Kopie, eine Leihe zugleich, `null` für fehlende Werte, `PreconditionNotMetException` bei fremder Rückgabe; Primitive unter `MEMBER_ID_INVALID` mit Rückschreiben) und `clone`. Die Zugriffe über Writer, Reader und Codec prüfen `DynamicTopicTest`. (Commit `a25948e911c`)
### §7.8.1.4 Ergebnis direkt statt über einen in-out-Parameter

**Spec:** §7.8.1.4, S. 28 — "Operations that provide their result as an in-out
value in their first parameter and return DDS::ReturnCode_t have been changed
such that they instead return their results directly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getInt32Value`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getStringValue`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getFloat64Values`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_5_1_a_writer_and_a_reader_on_the_topic_of_a_dynamic_type_move_dynamic_data`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_structure_with_a_nested_structure_and_arrays_has_the_octets_of_xtypes_7_4`

**Status:** `done` — Die Lesezugriffe von `DynamicData` geben den Wert zurück (`getInt32Value` als `int`, `getStringValue` als `String`, `getComplexValue` als `DynamicData`); die Feldformen füllen das übergebene Array und geben die Anzahl zurück. Die Tests lesen so Werte aus dem gelesenen Sample und vergleichen sie. Ein `ReturnCode_t` und ein in-out-Parameter für das Ergebnis kommen nicht vor. (Commit `d2ca428c7b8`)
### §7.8.1.4 `equals` und `clone` als `Object.equals` und `Object.clone`

**Spec:** §7.8.1.4, S. 28 — "The equals and clone operations on these types have
been mapped to overrides of the Java-standard Object.equals and Object.clone,
respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.clone`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.equals`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataImpl.java` (`DynamicDataImpl.hashCode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_clone_is_an_equal_sample_that_shares_no_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_cloned_array_value_is_a_copy`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_sample_are_equal_when_type_and_values_are_without_regard_to_the_order_of_the_members`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_non_optional_member_omitted_by_one_sample_equals_its_default_in_the_other`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_nested_values_are_compared_by_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_samples_with_the_same_float128_value_are_equal_whatever_the_scale`

**Status:** `done` — `DynamicDataImpl.clone()` kopiert die Werte tief (verschachtelte `DynamicData`, Arrays, Listen und Maps); `equals`/`hashCode` vergleichen nach XTypes 1.3 §7.5.2.11.5 die Typen (gleich nach Wert) und die Werte ohne Rücksicht auf die Reihenfolge der Mitglieder; ein nicht optionales Mitglied, das ein Muster auslässt, ist gleich dem Vorgabewert im anderen; `BigDecimal` wird nach Wert verglichen (1.5 gleich 1.50). Getestet in `TypesDynamicDataTest`. (Commit `a25948e911c`)
### §7.8.1.4 Keine Methoden für vorzeichenlose Typen, Zugriff über gleich große oder nächstgrößere Typen

**Spec:** §7.8.1.4, S. 28 — "Methods dealing with unsigned integer types have
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_an_unsigned_kind_is_read_as_the_signed_kind_of_the_same_size_and_as_the_one_size_up`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_an_unsigned_kind_is_set_through_the_one_size_up_as_a_logical_value`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_unsigned_members_are_encoded_and_decoded_with_their_full_range`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_sequence_of_unsigned64_is_read_as_big_integers`

**Status:** `done` — `DynamicData` hat keine Methoden für vorzeichenlose Typen. `DynamicDataTypeSupport` hält UINT_16 als `Short`, UINT_32 als `Integer`, UINT_64 als `Long` (Bitmuster). Die Zugriffe richten sich nach der Art des Mitglieds: gleich große Zugriffe (`getInt16Value`, `getInt32Value`, `getInt64Value`) geben das Bitmuster, der nächstgrößere Zugriff den logischen Wert (UInt16 als `int`, UInt32 als `long`, UInt64 als `getBigIntegerValue`). Die Setzer nehmen bei gleicher Größe das Bitmuster, bei größerem Zugriff den logischen Wert mit Bereichsprüfung (`IllegalArgumentException`). `getBigIntegerValues`/`setBigIntegerValues` behandeln UINT_64-Folgen. `TypesDynamicDataTest` prüft dies und das Kodieren/Dekodieren von UINT_16, UINT_32 und UINT_64 (20 Oktette, `10 00 00 00 FF FF 00 00 FF FF FF FF FF…`). Folgen von UINT_16 und UINT_32 sind über `getInt32Values`/`getInt64Values` nur als Bitmuster der gleichen Größe lesbar. (Commit `a25948e911c`)
### §7.8.1.4 Float128 als `java.math.BigDecimal`

**Spec:** §7.8.1.4, S. 28 — "The 128-bit Float128 type has been represented
using java.math.BigDecimal."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.getBigDecimalValue`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/DynamicData.java` (`DynamicData.setBigDecimalValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicDataTypeSupport.java` (`DynamicDataTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_float128_member_is_a_big_decimal_on_the_wire_and_back`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_a_float128_member_keeps_a_value_a_double_cannot_hold`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicDataTest.java::javapsm_7_8_1_4_samples_with_the_same_float128_value_are_equal_whatever_the_scale`

**Status:** `done` — `DynamicData` hat `getBigDecimalValue`/`setBigDecimalValue` und `...Values` mit `List<BigDecimal>`; `DynamicDataTypeSupport` schreibt und liest FLOAT_128_TYPE über `Float128.fromBigDecimal` und `toBigDecimal`. `TypesDynamicDataTest` setzt ein FLOAT_128-Mitglied (1.5: Oktette `10 00 00 00` und `00…00 80 FF 3F` Little-Endian; 2^100+1, das ein `double` nicht hält; 0; −0.25), kodiert, dekodiert und liest es zurück. Der Codegen bildet `long double` weiter auf `org.zerodds.wire.Float128` ab (`crates/lang-rules/src/adapter/java.rs`), unabhängig von `DynamicData`. (Commit `a25948e911c`)
### §7.8.1.5 Deskriptoren unveränderlich, neue aus bestehenden

**Spec:** §7.8.1.5, S. 28 — "This specification defines three descriptor
interfaces. The instances of descriptor interfaces are immutable and therefore,
provide methods to create new descriptor objects from the existing ones."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeDescriptorImpl.java` (`TypeDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/MemberDescriptorImpl.java` (`MemberDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/AnnotationDescriptorImpl.java` (`AnnotationDescriptorImpl`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_5_a_type_descriptor_with_leaves_the_original_as_it_was`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_5_type_descriptors_are_equal_when_all_their_properties_are`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_5_a_member_descriptor_with_leaves_the_original_as_it_was`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_7_8_1_5_an_annotation_descriptor_with_leaves_the_original_as_it_was`

**Status:** `done` — Die `with...`-Methoden aller drei Deskriptoren kopieren und ändern die Kopie. `TypeDescriptorImpl.getBound` und `MemberDescriptorImpl.getLabel` geben nicht änderbare Listen zurück; `AnnotationDescriptorImpl.getAllValue` gibt eine Kopie. `TypesDynamicTypeTest` prüft je Deskriptor, dass `with...` das Original unverändert lässt, dass die Listen nicht änderbar sind und dass die Deskriptoren nach allen Eigenschaften gleich sind (`equals`/`hashCode`). (Commit `a25948e911c`)
### §7.8.1.5 AnnotationDescriptor

**Spec:** §7.8.1.5, S. 28 — "• AnnotationDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/AnnotationDescriptor.java` (`AnnotationDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/AnnotationDescriptorImpl.java` (`AnnotationDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newAnnotationDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_5_the_key_annotation_makes_the_topic_keyed`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_5_the_extensibility_annotation_chooses_the_framing`

**Status:** `done` — `AnnotationDescriptor` ist als Schnittstelle vorhanden, `DynamicTypeFactory.newAnnotationDescriptor` erzeugt sie. Die Tests bauen damit `Key` und `Extensibility` (`withType`, `withValue`) und prüfen deren Wirkung auf den Codec. (Commit `7d991374fd4`)
### §7.8.1.5 MemberDescriptor

**Spec:** §7.8.1.5, S. 28 — "• MemberDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/MemberDescriptor.java` (`MemberDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/MemberDescriptorImpl.java` (`MemberDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newMemberDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_union_is_its_discriminator_and_the_selected_member`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_structure_with_a_nested_structure_and_arrays_has_the_octets_of_xtypes_7_4`

**Status:** `done` — `MemberDescriptor` ist als Schnittstelle vorhanden, `DynamicTypeFactory.newMemberDescriptor` erzeugt sie. Die Tests setzen `withName`, `withId`, `withType` und `withLabel` und legen damit Mitglieder von Struktur und Union an, die der Codec kodiert und dekodiert. (Commit `7d991374fd4`)
### §7.8.1.5 TypeDescriptor

**Spec:** §7.8.1.5, S. 28 — "• TypeDescriptor"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/dynamic/TypeDescriptor.java` (`TypeDescriptor`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeDescriptorImpl.java` (`TypeDescriptorImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.newTypeDescriptor`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/SerializationTest.java::javapsm_7_3_3_a_type_descriptor_is_serializable_and_carries_the_types_it_names`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTopicTest.java::javapsm_7_8_1_4_a_union_is_its_discriminator_and_the_selected_member`

**Status:** `done` — `TypeDescriptor` ist als Schnittstelle vorhanden, `DynamicTypeFactory.newTypeDescriptor` erzeugt sie. `javapsm_7_3_3_a_type_descriptor_is_serializable_and_carries_the_types_it_names` prüft `getKind` und `getElementType` nach einer Serialisierung; der Union-Test setzt `withKind`, `withName` und `withDiscriminatorType`. (Commit `7d991374fd4`)
### §7.8.2 Vier eingebaute Typen von [DDS-XTypes]

**Spec:** §7.8.2, S. 29 — "[DDS-XTypes] specifies four built-in types:
DDS::String, DDS::Bytes, DDS::KeyedString, and DDS::KeyedBytes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung der vier eingebauten Typen von [DDS-XTypes]; die Abbildungen stehen in den folgenden Items.
### §7.8.2 DDS::String als `java.lang.String`

**Spec:** §7.8.2, S. 29 — "• DDS::String is mapped to java.lang.String."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.isValueType`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.StringTypeSupport`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_string_is_the_built_in_type_dds_string`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_string_topic_is_dds_string_and_moves_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_truncated_or_inconsistent_string_is_refused`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_topic_description_finds_only_what_was_created`

**Status:** `done` — `TypeSupportResolver.resolve(String.class)` liefert `BuiltinTypeSupports.StringTypeSupport`: Typname `DDS::String`, `APPENDABLE` (XTypes 1.3 Annex E: `struct _String { string value; }`), unverändert für `String` als Feld einer Struktur. Ein Beispiel: `"hi"` ist `07 00 00 00 03 00 00 00 68 69 00` (DHEADER 7, Länge 3, Zeichen, NUL), in Big-Endian mit umgekehrten Wörtern; die Zeichenkette folgt der Einstellung des Item zu schmalen Zeichen (Kürzung als Vorgabe, UTF-8 als benannte Einstellung). `TypesBuiltinTest` prüft Typname, Oktette in beiden Byte-Reihenfolgen, die leere Zeichenkette, die Ablehnung abgeschnittener Daten und `null`, und schreibt und liest ein `String`-Muster über ein Topic. `DomainParticipantOperationsTest::javapsm_2_lookup_topic_description_finds_only_what_was_created` erwartet jetzt `DDS::String` statt `java::lang::String`. (Commit `ec44921ff1d`)
### §7.8.2 DDS::Bytes als `byte[]`

**Spec:** §7.8.2, S. 29 — "• DDS::Bytes is mapped to byte[]."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_byte_array_topic_has_the_type_dds_bytes_and_is_appendable`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_byte_array_is_written_as_dheader_length_and_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_of_4096_octets_arrives_whole`

**Status:** `done` — `TypeSupportResolver.resolve(byte[].class)` liefert `ByteArrayTypeSupport` mit Typname `DDS::Bytes` und APPENDABLE-Rahmen (DHEADER, Länge, Oktette). `javapsm_7_8_2_a_byte_array_topic_has_the_type_dds_bytes_and_is_appendable` und `javapsm_7_8_2_a_byte_array_is_written_as_dheader_length_and_octets` prüfen Namen und Oktette (`07 00 00 00 03 00 00 00 01 02 03`); die beiden Quickstart-Tests schreiben und lesen 5 und 4096 Bytes über ein `Topic<byte[]>`. (Commit `5041698cee3`)
### §7.8.2 `byte[]`-Topics als DDS::Bytes: Typname und Darstellung

**Spec:** §7.8.2, S. 29 — "[DDS-XTypes] specifies four built-in types:
DDS::String, DDS::Bytes, DDS::KeyedString, and DDS::KeyedBytes. [...] •
DDS::Bytes is mapped to byte[]."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.getTypeName`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.encode`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport.decode`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_byte_array_topic_has_the_type_dds_bytes_and_is_appendable`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_byte_array_is_written_as_dheader_length_and_octets`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_byte_array_is_read_from_dheader_length_and_octets`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ByteArrayTypeSupportTest.java::javapsm_7_8_2_a_big_endian_byte_array_has_big_endian_dheader_and_length`


**Status:** `done` — `getTypeName()` gibt `DDS::Bytes` zurück, `getExtensibility()` APPENDABLE; `encode` schreibt DHEADER (Länge + 4), Folgenlänge und Oktette, `decode` prüft beides und wirft `XcdrException` bei Abweichung. Das entspricht XTypes 1.3 Annex E (`@extensibility(APPENDABLE) struct Bytes { ByteSeq value; }`). Die Tests prüfen Namen, Oktette in Little- und Big-Endian und die Ablehnung von abgeschnittenem oder inkonsistentem Rahmen. (Commit `5041698cee3`)
### §7.8.2 DDS::KeyedString als änderbarer Werttyp

**Spec:** §7.8.2, S. 29 — "• DDS::KeyedString and DDS::KeyedBytes are mapped to
modifiable value type interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedString.java` (`KeyedString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes.KeyedStringImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newKeyedString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedStringTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.newData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_copy_from_clone_and_equals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_survives_java_serialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_string_is_the_built_in_type_dds_keyed_string`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_the_key_hash_of_a_keyed_string_is_the_md5_of_its_key_in_big_endian`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_sample_is_a_new_value_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_truncated_keyed_string_is_refused`

**Status:** `done` — `KeyedString` und `KeyedStringImpl` sind ein änderbarer Werttyp (`setKey`/`setValue` geben das Objekt zurück, `copyFrom`, `clone`, `equals`, `hashCode`, Java-Serialisierung ohne Environment). `TypeSupportResolver.resolve` liefert für `KeyedString` und jede Unterklasse `BuiltinTypeSupports.KeyedStringTypeSupport`: Typname `DDS::KeyedString`, `APPENDABLE`, `key` als Schlüssel. Beispiel: Schlüssel `k`, Wert `vv` sind `0F 00 00 00 02 00 00 00 6B 00 00 00 03 00 00 00 76 76 00` (die Länge des zweiten Strings ist auf 4 ausgerichtet). Der KeyHash ist der MD5 der Big-Endian-Serialisierung des Schlüssels (ein unbegrenzter String hat keine größte Größe ≤ 16 Oktette; XTypes 1.3 §7.6.8, DDSI-RTPS 2.5 §9.6.3.8): `302cf0a69dae0d4732bd21ea86fec48d` für `k`. `TypeSupportImpl.newData` erzeugt das Muster über die SPI. Getestet in `TypesBuiltinTest`, einschließlich eines Writers und Readers mit einer Instanz je Schlüssel. (Commit `ec44921ff1d`)
### §7.8.2 DDS::KeyedBytes als änderbarer Werttyp

**Spec:** §7.8.2, S. 29 — "• DDS::KeyedString and DDS::KeyedBytes are mapped to
modifiable value type interfaces."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/builtin/KeyedBytes.java` (`KeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypes.java` (`BuiltinTypes.KeyedBytesImpl`),
`crates/java-omgdds/java/src/main/java/org/zerodds/ServiceEnvironmentImpl.java` (`ServiceEnvironmentImpl.newKeyedBytes`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports.KeyedBytesTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportImpl.java` (`TypeSupportImpl.newData`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_is_a_modifiable_value_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_copy_from_clone_and_equals`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_survives_java_serialization`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_keyed_bytes_is_the_built_in_type_dds_keyed_bytes`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_the_key_hash_of_keyed_bytes_is_the_md5_of_its_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_sample_is_a_new_value_object`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_truncated_keyed_bytes_is_refused`

**Status:** `done` — `KeyedBytes` und `KeyedBytesImpl` sind ein änderbarer Werttyp (`setValue(byte[])` und `setValue(byte[], int, int)` kopieren, `getValue` gibt eine Kopie, `copyFrom`, `clone`, `equals`, `hashCode`, Java-Serialisierung). `TypeSupportResolver.resolve` liefert für `KeyedBytes` `BuiltinTypeSupports.KeyedBytesTypeSupport`: Typname `DDS::KeyedBytes`, `APPENDABLE`, `key` als Schlüssel. Beispiel: Schlüssel `k`, Wert `01 02 03` sind `0F 00 00 00 02 00 00 00 6B 00 00 00 03 00 00 00 01 02 03`. Der KeyHash ist der MD5 des Schlüssels wie bei `KeyedString`. Getestet in `TypesBuiltinTest`, einschließlich eines Writers und Readers mit einer Instanz je Schlüssel. (Commit `ec44921ff1d`)
### §7.8.2 DataReader und DataWriter für die eingebauten Typen

**Spec:** §7.8.2, S. 29 — "The generic DataReader and DataWriter interfaces are
applicable to the built-in types."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java` (`DataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` (`DataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ByteArrayTypeSupport.java` (`ByteArrayTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/BuiltinTypeSupports.java` (`BuiltinTypeSupports`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_string_topic_is_dds_string_and_moves_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_bytes_are_still_the_built_in_type_dds_bytes`

**Status:** `done` — Je ein `DataWriter` und `DataReader` auf einem Topic von `byte[]` (`DDS::Bytes`, `ByteArrayTypeSupport`), `String` (`DDS::String`), `KeyedString` und `KeyedBytes` schreiben und lesen Muster: `javapsm_7_8_2_a_byte_array_topic_moves_the_octets` und die Tests `javapsm_7_8_2_a_string_topic_is_dds_string_and_moves_samples`, `javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`, `javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key` in `TypesBuiltinTest`. (Commit `ec44921ff1d`)
### §7.8.2 Generisches `createDataReader` und `createDataWriter` für die eingebauten Typen

**Spec:** §7.8.2, S. 29 — "Subscriber and Publisher provide generic
createDataReader and createDataWriter methods to create datareader and
datawriter for the built-in types, respectively."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` (`Publisher.createDataWriter`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` (`Subscriber.createDataReader`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_string_topic_is_dds_string_and_moves_samples`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_string_topic_has_one_instance_per_key`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java::javapsm_7_8_2_a_keyed_bytes_topic_has_one_instance_per_key`

**Status:** `done` — `Publisher.createDataWriter(Topic<TYPE>)` und `Subscriber.createDataReader(TopicDescription<TYPE>)` sind generisch; der TypeSupport kommt aus dem Topic, das `TypeSupportResolver.resolve` für `byte[]`, `String`, `KeyedString` und `KeyedBytes` mit dem eingebauten TypeSupport füllt. Die Tests benutzen die Überladungen mit nur dem Topic (`javapsm_7_8_2_a_byte_array_topic_moves_the_octets`, `TypesBuiltinTest`). (Commit `ec44921ff1d`)
### §7.8.3 Typen von TypeObject als Werttypen nach den Regeln dieses Dokuments

**Spec:** §7.8.3, S. 29 — "The types in this package are expressed as modifiable
value types according to the mapping rules expressed elsewhere in this
document."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/TypeObjectModel.java` (`TypeObjectModel`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/generated/ZdTypes.java` (`ZdTypes`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_8_3_type_objects_of_equal_state_are_equal`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectModelTest.java::javapsm_7_8_3_the_type_objects_are_serializable`

**Status:** `done` — Die Schnittstellen in `org.omg.dds.type.typeobject` der OMG-Quelle haben nur Lese-Methoden (die Javadoc von `TypeObject` und `Member` nennt die Objekte unveränderlich); die Quelle gilt nach §2, und `org/omg/dds` bleibt gleich ihr. Die Objekte, die `TypeObjectModel` liefert, sind Werttypen nach §7.3.3: `Serializable`, mit `equals` und `hashCode` nach dem Zustand (`TypeObjectModelTest::javapsm_7_8_3_type_objects_of_equal_state_are_equal`); verändert wird ein TypeObject über die veränderlichen Datenklassen des Codecs (`ZdTypes.DDS.*`: öffentliche Felder, `TypeObjectCodec.encode`/`decode`) und `TypeObjectModel.of`. Die Änderungen der Abbildung aus §7.8.3 (Konstanten in Schnittstellen, `MemberId`-Klassen) sind getestet (`TypesTypeObjectConstantsTest`). (Commit `b4bf3fa978e`)
### §7.8.3 Konstanten der obersten Ebene in Schnittstellen, z. B. `Member.MEMBER_ID_INVALID`

**Spec:** §7.8.3, S. 29 — "• Top-level constants are moved into related
interfaces, for example: Member.MEMBER_ID_INVALID."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/Member.java` (`Member.MEMBER_ID_INVALID`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::javapsm_7_8_3_the_constants_of_the_top_level_are_moved_into_the_interfaces`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::javapsm_7_8_3_the_interfaces_the_constants_live_in_are_the_ones_of_the_top_level_types`

**Status:** `done` — `Member.MEMBER_ID_INVALID` ist als `int`-Konstante `0x0FFFFFFF` in der Schnittstelle `Member` deklariert; der Wert ist der größte Wert des 28-Bit-Bereichs der Member-IDs (XTypes 1.3 §7.2.2.4.4.4.4) und gleich `MEMBER_ID_INVALID` in `crates/types/src/dynamic/descriptor.rs`. Weitere Konstanten in Schnittstellen: `CollectionType.UNBOUNDED_COLLECTION` (0) und die Typ-IDs in `TypeId` (gleich den `TypeKind.Values`). `TypesTypeObjectConstantsTest` liest sie. (Commit `ffbb2d554ca`)
### §7.8.3 Member-IDs als verschachtelte finale Klassen mit `int`-Konstanten

**Spec:** §7.8.3, S. 29 — "Enumerations of member ID values are nested final
classes within the interfaces for which they provide the member’s IDs. These
classes have constant integer fields, for example:
MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/MapType.java` (`MapType.MemberId`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/typeobject/Type.java` (`Type.MemberId`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesTypeObjectConstantsTest.java::javapsm_7_8_3_member_ids_are_nested_final_classes_of_int_constants`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typeobject/TypeObjectTest.java::javapsm_7_8_3_the_member_ids_are_those_of_annex_b_of_xtypes_1_1`

**Status:** `done` — Die Schnittstellen in `org.omg.dds.type.typeobject` tragen je eine verschachtelte finale Klasse `MemberId` mit privatem Konstruktor und `int`-Konstanten, z. B. `MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID = 201`. `TypesTypeObjectConstantsTest` prüft für 15 Schnittstellen Namen und Werte der Konstanten gegen die OMG-Quelle, dass die Klasse final, öffentlich und statisch ist, keinen öffentlichen Konstruktor hat, und dass die Werte genau die `@ID`-Werte der Getter derselben Schnittstelle sind. `TypeObjectTest::javapsm_7_8_3_the_member_ids_are_those_of_annex_b_of_xtypes_1_1` vergleicht die Werte mit den Aufzählungen von DDS-XTypes 1.1 Anhang B (`TypeMemberId` 0, 1; `StructureTypeMemberId` 100, 101; `MapTypeMemberId` 200, 201; …), die jetzt im Spec-Cache liegen; die Werte stimmen. Die Quelle hat in `CollectionType.MemberId` nur `ELEMENT_TYPE_COLLECTIONTYPE_MEMBER_ID`; die Konstante `ELEMENT_SHARED_COLLECTIONTYPE_MEMBER_ID` (101) des Anhangs fehlt dort, und `org/omg/dds` bleibt gleich der Quelle. (Commit `b4bf3fa978e`)

---

## §8.1 General

### §8.1 Java-Objekte ohne Codegenerierung und ohne Dynamic Language Binding veröffentlichen

**Spec:** §8.1, S. 31 — "The Java Type Representation defined in this clause
provides a means for Java developers to publish and subscribe to DDS topics
typed by plain Java objects without resorting to code generation or the
reflective style of the Dynamic Language Binding."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java` (`TypeSupportResolver.resolve`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `TypeSupportResolver.resolve` fällt für eine Klasse ohne erzeugten TypeSupport auf `ReflectionTypeSupport` zurück. `javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code` schreibt und liest ein Bean mit `int`- und `String`-Feld über `createDataWriter(Topic)`/`createDataReader(Topic)` ohne IDL und ohne Dynamic Language Binding; `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` und `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüfen die Oktette gegen den erzeugten Pfad. (Commit `819bfcb46cb`)
### §8.1 Zugleich ein Language Binding, für die Laufzeit gedacht

**Spec:** §8.1, S. 31 — "By its very nature as an expression of the Java
programming language, this Type Representation implicitly and simultaneously
defines a Language Binding for DDS types. [...] Therefore, this Type
Representation is intended for the run-time use of implementations of this PSM.
While this specification does not preclude Service implementations from using
this Type Representation for other purposes [...] such uses are non-normative
and unspecified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung der Typdarstellung als Language Binding für die Laufzeit; die Spec verlangt hier nichts Prüfbares.
### §8.1 Jeder `Serializable`-Typ lässt sich veröffentlichen

**Spec:** §8.1, S. 31 — "Any Java type that implements Serializable (directly or
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
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_1_the_members_of_the_base_class_come_first`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_a_serializable_class_of_the_application_is_a_topic_type`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_a_serializable_class_of_the_jdk_is_published_and_read_back`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_the_types_of_table_8_1_are_not_refused_as_types_of_the_jdk`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_a_date_is_its_milliseconds_and_a_uuid_its_two_halves`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_a_jdk_class_is_a_topic_type_itself`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_the_octets_of_a_jdk_value_never_make_another_class`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_1_a_value_the_class_refuses_is_refused_on_reading`

**Status:** `done` — `ReflectionTypeSupport` nimmt jede Klasse an und prüft `Serializable` nicht; `serialVersionUID` ist statisch und kein Mitglied. Eine `Serializable`-Klasse der Anwendung wird veröffentlicht (`javapsm_8_1_a_serializable_class_of_the_application_is_a_topic_type`: Oktette `04 00 00 00 03 00 00 00 61 62 00`), mit den Regeln der Java Type Representation einschließlich der Einschränkung von `java.io.Serializable` für Oberklassen. Die `Serializable`-Klassen des JDK ohne Eintrag in Table 8.1 sind veröffentlichbar, als Typ eines Topics, eines Felds, eines Folgenelements oder einer Array-Komponente: `java.util.Date` (int64 Millisekunden), `UUID` (zwei int64), `URI`, `Locale` und `Currency` (je ein String) und `java.time.Instant`, `Duration`, `Period`, `LocalDate`, `LocalTime`, `LocalDateTime`, `OffsetDateTime`, `ZonedDateTime`, `ZoneOffset`, `Year`, `YearMonth`, `MonthDay` als Struktur aus Typen der Table 8.1 (`JdkValueTypes`, die Felder in der Javadoc der Klasse); jede andere konkrete `Serializable`-Klasse des JDK (etwa `java.util.BitSet`) als ihre Java-Serialisierung, eine `sequence<octet>` (`JdkSerializedValues`; nur Java-Programme lesen sie). Beim Lesen entsteht nie eine andere Klasse: der Strom darf nur die deklarierte Klasse und Klassen aus `java.lang` nennen und höchstens 1 MiB lang sein, sonst `XcdrException` (`javapsm_8_1_the_octets_of_a_jdk_value_never_make_another_class`); ein Wert, den die Klasse ablehnt (Monat 13), ebenso. Eine Klasse des JDK, die nicht `Serializable` oder nicht konkret ist, wird beim Anlegen des Typs mit einer `XcdrException` abgelehnt, die Klasse, Feld, Table 8.1 und einen Ausweg nennt. Ein `DynamicType` (`createType`) und ein TypeObject kennen diese JDK-Klassen nicht; ein Typ mit einem solchen Feld kündigt kein TypeObject an. (Commit `bf27136ca70`)
### §8.1 DDS-Serialisierung nicht gleich der Serialisierung der JRE

**Spec:** §8.1, S. 31 — "Note that the DDS serialization of a type will not
generally be the same as the JRE serialization of the same type, even if the
type designer's specification of which data to serialize can be shared between
these two mechanisms."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis, dass die DDS-Serialisierung von der JRE-Serialisierung abweicht. `ReflectionTypeSupport` schreibt XCDR2 über `org.zerodds.wire.Writer` und beachtet `writeObject`, `readObject` und `serialPersistentFields` nicht.

---

## §8.2 Default Mappings

### §8.2 Table 8.1: `int`, `java.lang.Integer` → INT32

**Spec:** §8.2, S. 31, Table 8.1 — "INT, JAVA.LANG.INTEGER | INT32"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list`

**Status:** `done` — `int` wird INT32 (`Mixed.i` gegen den `Writer`-Pfad im ersten Test, Oktette `01 00 00 00 …` in `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`); `Integer` als Element von `List<Integer>` im zweiten Test. Ein Feld des Hüllklassen-Typs mit `null` ohne `@Optional` wirft `XcdrException` (`ReflectionTypeSupport.encodeMember`); die Hüllklasse selbst hat keinen eigenen Test (aus dem Code gelesen, nicht gemessen). (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `short`, `java.lang.Short` → INT16

**Spec:** §8.2, S. 31, Table 8.1 — "SHORT, JAVA.LANG.SHORT | INT16"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `short` als INT16 gegen den `Writer`-Pfad; `Short` geht denselben Zweig in `encodeValue`/`decodeValue` und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `long`, `java.lang.Long` → INT64

**Spec:** §8.2, S. 31, Table 8.1 — "LONG, JAVA.LANG.LONG | INT64"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four`

**Status:** `done` — Beide Tests prüfen `long` als INT64 (Oktette gegen den `Writer`-Pfad, Rundlauf mit negativem Wert in einem MUTABLE-Typ); `Long` geht denselben Zweig und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `float`, `java.lang.Float` → FLOAT32

**Spec:** §8.2, S. 31, Table 8.1 — "FLOAT, JAVA.LANG.FLOAT | FLOAT32"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `float` (2.5f) als FLOAT32 gegen den `Writer`-Pfad; `Float` geht denselben Zweig und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `double`, `java.lang.Double` → FLOAT64

**Spec:** §8.2, S. 31, Table 8.1 — "DOUBLE, JAVA.LANG.DOUBLE | FLOAT64"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`

**Status:** `done` — Beide Tests prüfen `double` als FLOAT64; `xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded` vergleicht die Oktette `1F 85 EB 51 B8 1E 09 40` für 3,14. `Double` geht denselben Zweig und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `char`, `java.lang.Character` → CHAR8

**Spec:** §8.2, S. 31, Table 8.1 — "CHAR, JAVA.LANG.CHARACTER | CHAR8"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`runtime/java/src/main/java/org/zerodds/wire/Writer.java` (`Writer.writeChar`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `'Z'` als CHAR8 gegen den `Writer`-Pfad. `Character` geht denselben Zweig. Ein Zeichen über U+00FF wirft `WireException.OutOfRange` (`Writer.writeChar`; ohne `@SerializeAs` wird die Ausnahme nicht in `XcdrException` umgewandelt) (aus dem Code gelesen, nicht gemessen). (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `byte`, `java.lang.Byte` → BYTE

**Spec:** §8.2, S. 31, Table 8.1 — "BYTE, JAVA.LANG.BYTE | BYTE"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `(byte) 0xAB` als BYTE; `Byte` geht denselben Zweig und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `boolean`, `java.lang.Boolean` → BOOLEAN

**Spec:** §8.2, S. 31, Table 8.1 — "BOOLEAN, JAVA.LANG.BOOLEAN | BOOLEAN"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `true` als BOOLEAN; `Boolean` geht denselben Zweig und hat keinen eigenen Test. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `java.lang.String` → STRING<CHAR8>

**Spec:** §8.2, S. 31, Table 8.1 — "JAVA.LANG.STRING | STRING<CHAR8>"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`


**Status:** `done` — `SerializeAsTest::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default` prüft die Oktette von `"été"` und `"AŁ"` (je Zeichen ein Oktett, Länge einschließlich Terminator). Die Vorgabe für Zeichen über U+007F ist die Kürzung nach §8.5.1, UTF-8 die benannte Einstellung `zerodds.java.narrow-string=utf8`; siehe das Item „§8.5.1 Schmale Zeichen: jedes Java-Zeichen auf das niederwertige Byte gekürzt“. (Commit `819bfcb46cb`)
### §8.2 Table 8.1: `java.util.Map` → map

**Spec:** §8.2, S. 31, Table 8.1 — "java.util.Map | map"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeMap`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_2_a_map_is_a_dheader_a_count_and_the_entries`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_2_a_map_is_read_into_the_map_class_the_field_declares`

**Status:** `done` — `TypesJavaRepresentationTest::javapsm_8_5_2_a_map_is_a_dheader_a_count_and_the_entries` prüft die Oktette (DHEADER, Anzahl, Schlüssel/Wert-Paare: `10 00 00 00 01 00 00 00 02 00 00 00 61 00 00 00 01 00 00 00`). Das Feld muss mit Typargumenten deklariert sein, sonst wirft `elementClass` eine `XcdrException`. (Commit `4aa407a112d`)
### §8.2 Table 8.1: `java.lang.Collection` und Array → sequence

**Spec:** §8.2, S. 31, Table 8.1 — "java.lang.Collection, array | sequence"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.collection`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_an_array_of_primitives_is_a_sequence_without_a_dheader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_sequence_is_read_into_the_collection_the_field_declares`


**Status:** `done` — `List`, `Set`, `Queue`, `Deque`, `SortedSet`, `NavigableSet`, `EnumSet` und Arrays werden Folgen; die Tests prüfen Oktette (V-5, V-6) und Rundläufe. Beim Lesen baut `ReflectionTypeSupport.collection` die Klasse des Feldes: für eine Schnittstelle oder abstrakte Klasse die erste von `ArrayList`, `LinkedHashSet`, `TreeSet`, `ArrayDeque`, `LinkedBlockingDeque`, die sie ist (`Collection`/`List` `ArrayList`, `Set` `LinkedHashSet`, `SortedSet`/`NavigableSet` `TreeSet`, `Queue`/`Deque` `ArrayDeque`); `EnumSet` über die Elementklasse; eine konkrete Klasse über ihren Konstruktor ohne Argumente. Ein Feldtyp, den keine Standardklasse füllt, ist eine `XcdrException`, keine andere Sammlung. `TypesJavaRepresentationTest::javapsm_8_5_3_a_sequence_is_read_into_the_collection_the_field_declares` prüft Klasse und Reihenfolge für zehn Feldtypen. (Commit `f0649c27acc`)
### §8.2 Table 8.1: `java.lang.Object` → Structure

**Spec:** §8.2, S. 31, Table 8.1 — "java.lang.Object | Structure"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_6_1_a_nested_final_structure_is_written_inline`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`

**Status:** `done` — Eine Klasse wird eine Struktur, verschachtelte Klassen werden verschachtelte Strukturen; `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` und `javapsm_8_6_1_a_nested_final_structure_is_written_inline` prüfen die Oktette (FINAL ohne DHEADER, 16 Bytes für zwei `Point`). (Commit `819bfcb46cb`)
### §8.2 `@SerializeAs` ändert die Vorgaben je Typ und je Feld, `value()` vom Typ `TypeKind`

**Spec:** §8.2, S. 32 — "A type designer may modify these defaults on a
type-by-type and/or field-by-field basis by applying the annotation
org.omg.dds.type.SerializeAs: public @interface SerializeAs { public TypeKind
value(); … }"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/SerializeAs.java` (`SerializeAs`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_kind_table_8_2_does_not_list_for_the_type_is_refused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_wide_string_is_marked_with_serialize_as`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_2_a_kind_other_than_structure_or_union_on_a_class_is_refused`

**Status:** `done` — `org.omg.dds.type.SerializeAs` hat `TypeKind value()` (dazu `collectionElementKind`, `mapKeyElementKind`, `bound`). An einem Feld ändert sie die Vorgabe (`JavaRepresentation.ofField`; die Tests prüfen UInt32, eine nach Table 8.2 unzulässige Kombination und eine breite Zeichenkette); an einer Klasse gilt `STRUCTURE_TYPE` oder `UNION_TYPE` (`introspectUnion`; `javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member` prüft die Oktette), andere Arten an einer Klasse lehnt `introspectUnion` mit `XcdrException` ab (`javapsm_8_2_a_kind_other_than_structure_or_union_on_a_class_is_refused`). Die Annotation an Methoden bleibt ohne Wirkung. (Commit `4aa407a112d`)

---

## §8.3 Metadata

### §8.3 Metadaten der IDL als gleichwertige Java-Annotationen

**Spec:** §8.3, S. 32 — "The type system metadata represented with built-in
annotations in the IDL Type Representation (such as @Key, @ID) shall be
represented by equivalent Java annotations unless otherwise noted."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.annotation`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Key.java` (`Key`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/ID.java` (`ID`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_3_the_key_annotation_of_the_type_system_makes_the_key_holder`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_9_annotations_of_another_package_with_the_names_of_the_type_system_are_ignored`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_3_the_id_of_a_member_is_its_id_on_the_wire_and_an_absent_optional_member_has_none`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_3_the_annotations_are_members_of_the_dynamic_type_too`


**Status:** `done` — `@Key` (`xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`, `javapsm_8_3_the_key_annotation_of_the_type_system_makes_the_key_holder`) und `@Extensibility` (`javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation`) aus `org.omg.dds.type` werden gelesen, fremde Annotationen gleichen Namens nicht. `ReflectionTypeSupport.annotation` vergleicht den einfachen Namen ohne Beachtung der Groß-/Kleinschreibung, so dass `org.omg.dds.type.ID` als `Id` gilt. `javapsm_8_3_the_id_of_a_member_is_its_id_on_the_wire_and_an_absent_optional_member_has_none` prüft an einer MUTABLE-Klasse mit `@ID(5)`, `@Optional @ID(9)` und `@MustUnderstand @ID(12)` die Oktette: DHEADER, je EMHEADER `0x40000005`, `0x40000009` und `0xC000000C` (M-Flag bei `@MustUnderstand`) mit NEXTINT, kein EMHEADER für ein fehlendes optionales Mitglied; `javapsm_8_3_the_annotations_are_members_of_the_dynamic_type_too` die Annotationen in `createType`. `@Shared`, `@Nested`, `@Verbatim` und `@Value` haben keine Wirkung auf die Wire-Form. (Commit `f0649c27acc`)
### §8.3 Annotationen im Paket `org.omg.dds.type`

**Spec:** §8.3, S. 32 — "These annotations are in the package org.omg.dds.type."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Key.java` (`Key`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/ID.java` (`ID`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Optional.java` (`Optional`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/MustUnderstand.java` (`MustUnderstand`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/Extensibility.java` (`Extensibility`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/type/SerializeAs.java` (`SerializeAs`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_3_the_key_annotation_of_the_type_system_makes_the_key_holder`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`

**Status:** `done` — Das Paket `org.omg.dds.type` enthält `Key`, `ID`, `Optional`, `MustUnderstand`, `Extensibility`, `Shared`, `Nested`, `Verbatim`, `Value`, `BitBound`, `BitSet`, `SerializeAs`, `UnionDiscriminator`, `UnionMember`. Die Tests importieren und benutzen `Key`, `ID`, `Extensibility` und `SerializeAs` aus diesem Paket (Namensforderung). `ReflectionTypeSupport.annotation` liest sie aus `org.omg.dds.type` und aus `org.zerodds.types`. (Commit `5041698cee3`)
### §8.3 Annotationen an Klassen und Feldern, an Schnittstellen und Methoden ohne festgelegtes Verhalten

**Spec:** §8.3, S. 32 — "The annotations in this package logically govern the
behavior of concrete classes, not of polymorphic interfaces. As such, they may
be applied to classes or to their fields, as appropriate. Interface designers
wishing to document the DDS serialization of a type may additionally apply them
to interfaces or to property accessor and/or mutator methods; however, they have
no specified behavior in such cases."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`

**Status:** `done` — `introspect` liest Annotationen der Klasse (`@Extensibility`, `javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation`) und ihrer Felder (`@Key`, `xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`). Annotationen an Methoden werden nicht ausgewertet, wie die Spec es für diesen Fall offen lässt („no specified behavior“). (Commit `819bfcb46cb`)

---

## §8.4 Primitive Types

### §8.4 Vorgaben nach Table 8.2, Änderung über `@SerializeAs`

**Spec:** §8.4, S. 32 — "By default, Java primitive types are mapped to DDS
primitive types as defined in Table 8.2. The @SerializeAs annotation may be used
to modify these mappings as follows."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_kind_table_8_2_does_not_list_for_the_type_is_refused`

**Status:** `done` — Die Vorgaben von Table 8.2 gelten ohne Annotation (`javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`). `@SerializeAs` ändert sie je Feld (`javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`), und `JavaRepresentation.check` lehnt Kombinationen ab, die Table 8.2 nicht aufführt (`javapsm_8_4_a_kind_table_8_2_does_not_list_for_the_type_is_refused`). (Commit `874bc5ac9fe`)
### §8.4 Table 8.2: Int32 aus `int`, `Integer`

**Spec:** §8.4, S. 32, Table 8.2 — "Int32 | int, java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_kind_table_8_2_does_not_list_for_the_type_is_refused`

**Status:** `done` — `int` und `Integer` sind die Vorgabe für Int32 (`javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` für `int`, `Integer` als Listenelement in `javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list`); `@SerializeAs(INT_32_TYPE)` an einem `long` wird abgelehnt (`javapsm_8_4_a_kind_table_8_2_does_not_list_for_the_type_is_refused`). (Commit `874bc5ac9fe`)
### §8.4 Table 8.2: UInt32 aus `int`, `long`, `Integer`, `Long`

**Spec:** §8.4, S. 32, Table 8.2 — "UInt32 | int, long, java.lang.Integer,
java.lang.Long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_an_int_keeps_the_bit_pattern`

**Status:** `done` — `@SerializeAs(UINT_32_TYPE)` an `long` behält den logischen Wert (4 000 000 000 wird `00 28 6B EE`, 2^32 und −1 sind `XcdrException`); an `int` bleibt das Bitmuster (−1 wird `FF FF FF FF`). `Integer` und `Long` erlaubt `check`, ohne eigenen Test. (Commit `874bc5ac9fe`)
### §8.4 Table 8.2: Int16 aus `short`, `Short`

**Spec:** §8.4, S. 32, Table 8.2 — "Int16 | short, java.lang.Short"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `short` als Int16 gegen den `Writer`-Pfad. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: UInt16 aus `short`, `int`, `Short`, `Integer`

**Spec:** §8.4, S. 32, Table 8.2 — "UInt16 | short, int, java.lang.Short,
java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint16_in_an_int_keeps_the_value_and_in_a_short_the_bit_pattern`

**Status:** `done` — `@SerializeAs(UINT_16_TYPE)`: an `int` bleibt der Wert (65535 wird `FF FF`, 65536 ist `XcdrException`), an `short` das Bitmuster (−2 wird `FE FF`). Der Test prüft die Oktette und den Rundlauf; `Short` und `Integer` erlaubt `check`, ohne eigenen Test. (Commit `874bc5ac9fe`)
### §8.4 Table 8.2: Int64 aus `long`, `Long`

**Spec:** §8.4, S. 32, Table 8.2 — "Int64 | long, java.lang.Long"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four` und `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüfen `long` als Int64. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: UInt64 aus `long`, `Long`, `BigInteger`

**Spec:** §8.4, S. 32, Table 8.2 — "UInt64 | long, java.lang.Long,
java.math.BigInteger"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint64_takes_a_big_integer_or_a_long`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked`


**Status:** `done` — `@SerializeAs(UINT_64_TYPE)` nimmt `BigInteger` (2^64−1 wird acht `FF`, 2^64 ist `XcdrException`) oder `long` (Bitmuster); ohne Annotation ist `BigInteger` UInt64. Im Vorgabezweig `ReflectionTypeSupport.encodeValue` wird ein `BigInteger` außerhalb 0 bis 2^64−1 mit `XcdrException` abgelehnt und nicht über `longValue()` gekürzt. `javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked` prüft 2^64−1 (acht `FF`), 0, 2^63 (`00…80`), −1, 2^64 und 2^100. (Commit `f0649c27acc`)
### §8.4 Table 8.2: Float32 aus `float`, `Float`

**Spec:** §8.4, S. 32, Table 8.2 — "Float32 | float, java.lang.Float"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `float` als Float32 gegen den `Writer`-Pfad. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: Float64 aus `double`, `Double`

**Spec:** §8.4, S. 32, Table 8.2 — "Float64 | double, java.lang.Double"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded` und `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüfen `double` als Float64. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: Float128 aus `double`, `Double`, `BigDecimal`

**Spec:** §8.4, S. 32, Table 8.2 — "Float128 | double, java.lang.Double,
java.math.BigDecimal"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_float128_held_in_a_double_is_binary128`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_4_a_big_decimal_without_annotation_is_a_float128`

**Status:** `done` — `@SerializeAs(FLOAT_128_TYPE)` an `double`: `javapsm_8_4_a_float128_held_in_a_double_is_binary128` prüft die 16 Oktette für 1,0 (Exponent `3FFF`) und den Rundlauf. `BigDecimal` ist ohne Annotation Float128; `TypesJavaRepresentationTest::javapsm_8_4_a_big_decimal_without_annotation_is_a_float128` prüft die 16 Oktette von 1,25 und den Zahlenwert nach dem Lesen. `Double` erlaubt `check`, ohne eigenen Test. (Commit `874bc5ac9fe`)
### §8.4 Table 8.2: Byte aus `byte`, `Byte`

**Spec:** §8.4, S. 32, Table 8.2 — "Byte | byte, java.lang.Byte"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `byte` als Byte. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: Boolean aus `boolean`, `Boolean`

**Spec:** §8.4, S. 32, Table 8.2 — "Boolean | boolean, java.lang.Boolean"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `boolean` als Boolean. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: Char8 aus `char`, `Character`

**Spec:** §8.4, S. 32, Table 8.2 — "Char8 | char, java.lang.Character"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`

**Status:** `done` — Vorgabe; `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` prüft `char` als Char8. (Commit `819bfcb46cb`)
### §8.4 Table 8.2: Char32 aus `char`, `int`, `Character`, `Integer`

**Spec:** §8.4, S. 32, Table 8.2 — "Char32 | char, int, java.lang.Character,
java.lang.Integer"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_char32_is_a_four_octet_code_point`

**Status:** `done` — `@SerializeAs(CHAR_32_TYPE)` an `int`: Codepunkt U+1F600 wird `00 F6 01 00`, Rundlauf. `char`, `Character` und `Integer` erlaubt `check`, ohne eigenen Test; ein Wert über U+10FFFF ist `XcdrException`. (Commit `874bc5ac9fe`)
### §8.4 Vorzeichenlose Werte: Darstellung oder logischer Wert erhalten

**Spec:** §8.4, S. 33 — "The DDS Type System ([DDS-XTypes]) defines unsigned
integer types; the Java type system does not. As a result, this Type
Representation must map unsigned values to “equivalent” signed types. Type
designers have two choices, reflected in the table above: • Preserve
representation [...] • Preserve logical value [...]"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeKind`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_a_long_keeps_the_logical_value`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint32_held_in_an_int_keeps_the_bit_pattern`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint16_in_an_int_keeps_the_value_and_in_a_short_the_bit_pattern`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_uint64_takes_a_big_integer_or_a_long`

**Status:** `done` — Beide Wahlmöglichkeiten sind umgesetzt und geprüft: gleich große Java-Typen behalten das Bitmuster (`int` für UInt32, `short` für UInt16, `long` für UInt64), größere Java-Typen den logischen Wert (`long`, `int`, `BigInteger`), mit `XcdrException` außerhalb des Bereichs. (Commit `874bc5ac9fe`)

---

## §8.5 Collections

### §8.5 Drei Arten von Sammlungen

**Spec:** §8.5, S. 33 — "[DDS-XTypes] recognizes three categories of
collections: strings (variable-length lists of narrow or wide characters),
sequences (variable-length lists of any single element type), and maps
(homogeneously typed key-value mappings)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung der drei Sammlungsarten von XTypes; die Spec verlangt hier nichts Prüfbares.
### §8.5.1 DDS-Strings als Java-`String`

**Spec:** §8.5.1, S. 33 — "DDS strings, whether of narrow or wide characters,
are represented by Java String objects."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeString`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_wide_string_is_marked_with_serialize_as`

**Status:** `done` — Ein Feld `String` wird eine schmale Zeichenkette und zurück (`SerializeAsTest::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`, `TypesJavaRepresentationTest::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`); mit `@SerializeAs(STRING_TYPE, collectionElementKind = CHAR_32_TYPE)` eine breite (`javapsm_8_5_1_a_wide_string_is_marked_with_serialize_as`: `06 00 00 00 E9 00 74 00 E9 00`). (Commit `819bfcb46cb`)
### §8.5.1 Schmale Zeichen: jedes Java-Zeichen auf das niederwertige Byte gekürzt

**Spec:** §8.5.1, S. 33 — "• If a string is to be of narrow characters (the
default), each Java character shall be truncated to its least-significant byte."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.read`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::xtypes_7_4_1_1_2_the_utf8_setting_writes_a_string_as_utf8`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_1_both_forms_of_a_narrow_string_are_written_by_the_same_type_support`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`


**Status:** `done` — Die Vorgabe ist die Form der Klausel: jedes Zeichen auf das niederwertige Byte gekürzt (`javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`: `"AŁ"` wird `41 41`, `"été"` wird `E9 74 E9`), gelesen kommt ein Zeichen je Oktett zurück. Die Systemeigenschaft `zerodds.java.narrow-string=utf8` wählt die UTF-8-Form von XTypes 1.3 (§7.2.2.2.1.2.4, §7.4.1.1.2; `xtypes_7_4_1_1_2_the_utf8_setting_writes_a_string_as_utf8`: `"é"` wird `C3 A9`, wie im Interop-Korpus); beim Lesen werden ein fehlendes NUL und ungültiges UTF-8 abgelehnt. `javapsm_8_5_1_both_forms_of_a_narrow_string_are_written_by_the_same_type_support` schreibt dieselbe Instanz von `ReflectionTypeSupport` nacheinander in beiden Formen (`E9` und `C3 A9`). Beide Formen sind für Zeichen bis U+007F gleich. Peers, die die Strings als UTF-8 lesen (Cyclone DDS, Fast DDS, OpenDDS), brauchen die Einstellung `utf8`, sobald ein Zeichen über U+007F vorkommt; dasselbe gilt für die eingebauten Typen `DDS::String` und `DDS::KeyedString`. Die Einstellung gilt je Java-Prozess. (Commit `b1e7b4d0338`)
### §8.5.1 Breite Zeichen mit `@SerializeAs`, je Code Point ein DDS-Zeichen

**Spec:** §8.5.1, S. 33 — "• If a string is to be of wide characters (in which
case it must be so marked with @SerializeAs), each Java code point shall become
a single DDS wide character."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeString`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/NarrowStrings.java` (`NarrowStrings.write`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/WideStrings.java` (`WideStrings`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_wide_string_is_marked_with_serialize_as`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_1_a_narrow_string_is_truncated_to_its_low_octet_by_default`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_4_a_char32_is_a_four_octet_code_point`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_1_a_code_point_above_u10000_of_a_wide_string_is_one_character`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::xtypes_7_4_1_1_2_the_utf16_setting_writes_a_wstring_as_utf16_code_units`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_1_a_wide_string_refuses_what_is_no_code_point`

**Status:** `done` — Ein String mit `@SerializeAs(STRING_TYPE, collectionElementKind = CHAR_32_TYPE)` ist ein String aus 32-Bit-Zeichen, ein Zeichen je Code Point (`WideStrings`): die Länge in Oktetten (4n), dann n vorzeichenlose 32-Bit-Werte ohne Abschlusszeichen (`javapsm_8_5_1_a_wide_string_is_marked_with_serialize_as`: `"été"` ist `0C 00 00 00 E9 00 00 00 74 00 00 00 E9 00 00 00`). `javapsm_8_5_1_a_code_point_above_u10000_of_a_wide_string_is_one_character` prüft `"a𝄞"` (U+1D11E, zwei `char`): `08 00 00 00 61 00 00 00 1E D1 01 00`; ein Code Point über U+FFFF ist ein Zeichen und kein Surrogatpaar. Ein Wert, der kein Code Point ist, wird beim Lesen abgelehnt. Die Systemeigenschaft `zerodds.java.wide-string=utf16` wählt den wstring von XTypes 1.3 §7.4.1.1.2, eine Folge von UTF-16-Einheiten, in der ein solcher Code Point zwei Einheiten ist (`xtypes_7_4_1_1_2_the_utf16_setting_writes_a_wstring_as_utf16_code_units`: `06 00 00 00 61 00 34 D8 1E DD`; Interop-Korpus `string-adjacent-widths`); Gegenstellen, die einen `wstring` lesen, brauchen diese Einstellung. Ohne die Annotation ist der String schmal (siehe das Item davor). (Commit `b1e7b4d0338`)
### §8.5.2 Jede `java.util.Map` als DDS-Map, außer mit `@SerializeAs`

**Spec:** §8.5.2, S. 33 — "Any object whose class implements the interface
java.util.Map shall be considered a DDS map unless marked otherwise with
@SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.decodeMap`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.map`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_2_a_map_is_a_dheader_a_count_and_the_entries`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_5_2_nested_generics_are_followed`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_2_a_map_is_read_into_the_map_class_the_field_declares`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_2_a_map_marked_as_a_map_keeps_its_bound`


**Status:** `done` — Ein Feld, dessen deklarierter Typ `Map` implementiert, wird eine DDS-Map (`ReflectionTypeSupport.encodeValue`); `JavaRepresentation.check` lässt für `MAP_TYPE` nur `Map` zu und verweigert jede andere Art auf einer `Map` mit `XcdrException`. Beim Lesen baut `ReflectionTypeSupport.map` (auch für `JavaRepresentation.decodeMap`) die deklarierte Klasse: `Map` ein `LinkedHashMap`, `SortedMap`/`NavigableMap` ein `TreeMap`, `ConcurrentMap` ein `ConcurrentHashMap`, `EnumMap` über die Schlüsselklasse, eine konkrete Klasse über ihren Konstruktor. `javapsm_8_5_2_a_map_is_read_into_the_map_class_the_field_declares` prüft sieben Feldtypen, darunter ein Feld mit `@SerializeAs(MAP_TYPE)`; `javapsm_8_5_2_a_map_marked_as_a_map_keeps_its_bound` die Grenze. (Commit `f0649c27acc`)
### §8.5.3 Jede `java.util.Collection` als DDS-Folge

**Spec:** §8.5.3, S. 33 — "Any object whose class implements the interface
java.util.Collection shall be considered DDS sequences unless marked otherwise
with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.collection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.typeOfClass`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_sequence_is_read_into_the_collection_the_field_declares`


**Status:** `done` — Jede `Collection` wird eine Folge (`encodeValue`, Zweig `Collection.class.isAssignableFrom`); beim Lesen baut `ReflectionTypeSupport.collection` die deklarierte Klasse (siehe das Item zu Table 8.1 `Collection`): `ArrayList`, `LinkedHashSet`, `TreeSet`, `ArrayDeque`, `EnumSet`, oder die konkrete Klasse. `javapsm_8_5_3_a_sequence_is_read_into_the_collection_the_field_declares` prüft je ein Feld von `Queue`, `Deque`, `SortedSet`, `NavigableSet`, `TreeSet`, `EnumSet`, `LinkedList`, `Collection`, `Set` und `List`. `createType` liefert für `Collection<String>` `SEQUENCE_TYPE`. (Commit `f0649c27acc`)
### §8.5.3 Bei `List` die Reihenfolge der Liste

**Spec:** §8.5.3, S. 33 — "If the class implements java.util.List, the order of
the elements in the sequence shall correspond exactly to the order of the
elements in the list."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.asList`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`

**Status:** `done` — `asList` übernimmt die Elemente einer `List` in ihrer Reihenfolge, `encodeSequence` schreibt sie so. `javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list` vergleicht die Bytes für [1,2,3], `TypesJavaRepresentationTest::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator` für eine Menge in der Reihenfolge ihres Iterators. (Commit `819bfcb46cb`)
### §8.5.3 Sonst die Reihenfolge des Iterators

**Spec:** §8.5.3, S. 33 — "Otherwise, the order of the elements in the sequence
shall correspond to that returned by the collection’s iterator."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.asList`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator`

**Status:** `done` — `asList` kopiert eine `Collection` mit `new ArrayList<>(collection)`, also in der Reihenfolge ihres Iterators. `javapsm_8_5_3_a_set_is_written_in_the_order_of_its_iterator` schreibt ein `LinkedHashSet` ("b", "a") und ein `TreeSet` mit denselben Elementen und vergleicht alle 46 Oktette: das erste Set schreibt `b` vor `a`, das zweite `a` vor `b`. (Commit `f0649c27acc`)
### §8.5.3 Arrays als DDS-Folgen

**Spec:** §8.5.3, S. 33 — "Objects of array types shall be considered DDS
sequences unless marked otherwise with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeSequence`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_an_array_of_primitives_is_a_sequence_without_a_dheader`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`


**Status:** `done` — Ein Array-Typ geht in `encodeValue` über `encodeSequence` (Länge, Elemente) und kommt über `listToArray` als Array zurück. `TypesJavaRepresentationTest::javapsm_8_5_3_an_array_of_primitives_is_a_sequence_without_a_dheader` prüft die Oktette von `{1, 2}` (`02 00 00 00 01 00 00 00 02 00 00 00`) und liest das `int[]` zurück; `createType` liefert für `double[]` `SEQUENCE_TYPE` mit `FLOAT_64_TYPE` als Element. (Commit `4aa407a112d`)
### §8.5.3 Collection oder Array als DDS-Array mit `@SerializeAs`

**Spec:** §8.5.3, S. 33 — "Any Java collection or array may be designated as a
DDS array with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.check`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.encodeCollection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.decodeCollection`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.typeOfClass`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_3_an_array_has_no_length_and_a_sequence_may_name_its_element_kind`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_5_3_an_array_of_several_dimensions_is_written_row_major`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_4_serialize_as_chooses_the_kind_of_the_member`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_5_3_a_collection_marked_as_an_array_has_a_fixed_length_and_no_length_word`

**Status:** `done` — `@SerializeAs(ARRAY_TYPE, bound = ...)` auf einem Java-Array ist getestet: `int[3]` und `int[2][3]` werden ohne Länge zeilenweise geschrieben, eine falsche Elementzahl wird mit `XcdrException` abgelehnt, `createType` liefert `ARRAY_TYPE` mit den Grenzen. Auf einer `Collection` prüft `javapsm_8_5_3_a_collection_marked_as_an_array_has_a_fixed_length_and_no_length_word` eine `List<Integer>` mit `bound = {3}`: Oktette `01 00 00 00 02 00 00 00 03 00 00 00` ohne Länge, eine Liste mit zwei oder vier Elementen wird abgelehnt, gelesen wird eine `ArrayList`. (Commit `4aa407a112d`)
### §8.5.3 Begründung: Arrays sind Verweise

**Spec:** §8.5.3, S. 33 — "Design Rationale (non-normative) Objects of array
types must receive special care, because a Java array—like any Java object—is
stored by reference only. [...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.

---

## §8.6 Aggregated Types

### §8.6 Zwei Arten: Strukturen und Unions

**Spec:** §8.6, S. 34 — "[DDS-XTypes] recognizes two kinds of aggregated types:
structures and unions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung der Spec.
### §8.6 Konstruktor ohne Argumente für nicht verschachtelte Typen, auch nicht öffentlich

**Spec:** §8.6, S. 34 — "Any DDS type that is not a nested type (in the sense of
that word defined by [DDS-XTypes], as indicated in this Type Representation by
the annotation @Nested) must define a no-argument constructor for use by the
Service implementation. Service implementations shall have the capability to
invoke this constructor reflectively, even if it is not public."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiateCanonical`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_6_fields_and_the_constructor_are_reached_whatever_their_visibility`

**Status:** `done` — `instantiate` ruft `getDeclaredConstructor()` mit `setAccessible(true)` auf. In `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` hat `Point` einen Konstruktor ohne Argumente, der nur im Paket sichtbar ist, und wird gelesen. Fehlt der Konstruktor, nimmt `instantiateCanonical` den kanonischen Konstruktor (Record `Mixed` in `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`). `@Nested` wird in `ReflectionTypeSupport` nicht ausgewertet. (Commit `4aa407a112d`)
### §8.6 Felder der Klasse in der Reihenfolge von `getDeclaredFields`

**Spec:** §8.6, S. 34 — "The fields in the DDS structured type shall correspond
to those of the Java class. Their order shall be that returned by the method
java.lang.reflect.Class.getDeclaredFields."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_1_the_members_of_the_base_class_come_first`

**Status:** `done` — `fieldsOf` liefert je Klasse die Felder in der Reihenfolge von `getDeclaredFields()`, die Felder der Oberklassen vor den eigenen (Item zu §8.6.1.1). `javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` vergleicht acht Felder mit der festen Reihenfolge des Writers, `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` zwei Felder mit den Bytes des Referenzvektors. (Commit `819bfcb46cb`)
### §8.6 Statische und transiente Felder weggelassen

**Spec:** §8.6, S. 34 — "Static and/or transient fields shall be omitted."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_6_static_and_transient_fields_carry_no_member`

**Status:** `done` — `fieldsOf` überspringt statische, transiente und synthetische Felder. `javapsm_8_6_static_and_transient_fields_carry_no_member` prüft eine Klasse mit einem statischen, einem transienten und einem normalen Feld: die Oktette enthalten nur das normale (`05 00 00 00`), beim Lesen behält das transiente Feld den Wert des Konstruktors, `createType` listet nur das normale Mitglied. (Commit `f0649c27acc`)
### §8.6 Felder unabhängig von der Sichtbarkeit lesen und setzen

**Spec:** §8.6, S. 34 — "Service implementations shall have the capability to
get and set the values of fields reflectively regardless of their declared
access level (e.g., public, protected, private)."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.get`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_6_fields_and_the_constructor_are_reached_whatever_their_visibility`

**Status:** `done` — `fieldsOf` ruft `setAccessible(true)` je Feld auf; `get` und `instantiate` lesen und setzen über `Field`. `javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code` schreibt und liest eine Klasse mit `private`-Feldern über ein Topic, `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` eine mit Feldern ohne Modifikator, `TypesJavaRepresentationTest::javapsm_8_4_a_big_integer_without_annotation_is_a_uint64_and_is_range_checked` eine mit `public`-Feldern, `javapsm_8_6_fields_and_the_constructor_are_reached_whatever_their_visibility` eine mit `private`, `protected`, Paket- und `public`-Feld. (Commit `4aa407a112d`)
### §8.6 Nicht verlangt: SecurityManager verhindert den Zugriff

**Spec:** §8.6, S. 34 — "Service implementations need not address the following
cases: • A Java Security Manager (java.lang.SecurityManager) prevents privileged
access to a non-public field or constructor."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`)

**Tests:** —

**Status:** `n/a (informative)` — Ausnahme von der Pflicht. `fieldsOf` wandelt eine `SecurityException` oder `InaccessibleObjectException` beim Zugriff in eine `XcdrException` mit dem Feldnamen.
### §8.6 Nicht verlangt: finale Felder

**Spec:** §8.6, S. 34 — "• A field that is neither static nor transient is
declared final, preventing its value from being modified."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.instantiate`)

**Tests:** —

**Status:** `n/a (informative)` — Ausnahme von der Pflicht. `instantiate` setzt Felder über `Field.set` nach `setAccessible`; für einen Record ohne Konstruktor ohne Argumente nimmt es den kanonischen Konstruktor (`javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4` liest einen Record).
### §8.6 Nicht verlangt: Zyklen von Verweisen

**Spec:** §8.6, S. 34 — "• Object references form a cycle. (Cycles are not
permitted by the DDS Type System.)"

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:** —

**Status:** `n/a (informative)` — Ausnahme von der Pflicht. `DynamicTypeFactoryImpl.reflect` meldet eine Klasse, die sich über ihre Felder selbst enthält, als `IllegalArgumentException`; kein Test. Ein Objektgraph mit Zyklus führt in `encodeStruct` zu unbegrenzter Rekursion (aus dem Code gelesen, nicht gemessen).
### §8.6.1 Jede Klasse, die keine Collection und keine Map ist, als Struktur

**Spec:** §8.6.1, S. 34 — "Every Java class that is not a collection or map
shall be considered a structure by default."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.infoOf`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_6_1_a_nested_final_structure_is_written_inline`

**Status:** `done` — Eine Klasse, die in `encodeValue` und `decodeValue` weder primitiv, `String`, Aufzählung, Array, `Collection` noch `Map` ist, geht über `infoOf` und `encodeStruct` als Struktur. `javapsm_8_6_1_a_nested_final_structure_is_written_inline` schreibt und liest eine `Line` aus zwei verschachtelten `Point` inline (16 Bytes, ohne DHEADER). (Commit `819bfcb46cb`)
### §8.6.1.1 Ableitung als Vererbung von Strukturen

**Spec:** §8.6.1.1, S. 34 — "Java class extension shall map to structure
inheritance in the DDS Type System [DDS-XTypes], subject to the restrictions
documented by the java.io.Serializable interface, such as those pertaining to
non-Serializable base types."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.fieldsOf`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.baseTypeNames`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_1_the_members_of_the_base_class_come_first`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_8_6_1_1_a_local_writer_describes_the_base_types_of_its_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_8_6_1_1_a_derived_class_is_a_structure_with_the_superclass_as_base_type`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesDynamicTypeTest.java::javapsm_8_6_1_1_the_members_of_the_base_type_come_first_in_the_octets_of_a_derived_type`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_1_the_fields_of_a_base_that_is_not_serializable_are_no_members`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_1_the_dynamic_type_of_a_class_with_a_base_that_is_not_serializable_has_no_base_type`

**Status:** `done` — `fieldsOf` stellt die Felder der Oberklassen vor die eigenen; `javapsm_8_6_1_1_the_members_of_the_base_class_come_first` liest und schreibt eine abgeleitete Klasse mit den Basisfeldern zuerst (12 Bytes: DHEADER der EXTENSIBLE-Klasse, zwei int32). `baseTypeNames` liefert die Namen der Oberklassen, `javapsm_8_6_1_1_a_local_writer_describes_the_base_types_of_its_type` prüft sie in den Built-in-Topic-Daten. `DynamicTypeFactoryImpl.reflect` setzt die Oberklasse (unterhalb von `Object`) als Basistyp und listet nur die eigenen Mitglieder (`javapsm_8_6_1_1_a_derived_class_is_a_structure_with_the_superclass_as_base_type`); `javapsm_8_6_1_1_the_members_of_the_base_type_come_first_in_the_octets_of_a_derived_type` vergleicht die Oktette von `DynamicData` mit denen der Klasse. Die Einschränkung von `java.io.Serializable` gilt: bei einer `Serializable`-Klasse haben die Felder einer Oberklasse, die nicht `Serializable` ist, kein Mitglied und keinen Basistyp (`ReflectionTypeSupport.serializesFieldsOf`); die Oberklasse wird beim Lesen durch ihren eigenen Konstruktor belegt. Eine Klasse, die nicht `Serializable` ist, behält alle Felder. (Commit `2de26533680`)
### §8.6.1.2 FINAL bei direkter Ableitung von `Object` und `final`, oder ausdrücklich

**Spec:** §8.6.1.2, S. 34 — "• FINAL: If the class extends java.lang.Object
directly and is final, or if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_7_8_1_3_create_type_reflects_member_kinds_nesting_and_key`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_6_1_2_final_can_be_said_on_a_class_that_is_not_final`

**Status:** `done` — `defaultExtensibility` liefert FINAL für eine finale Klasse, die direkt von `Object` erbt. `javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible` prüft das und dass eine nicht finale oder abgeleitete Klasse nicht FINAL ist; `javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure` prüft die Bytes ohne DHEADER. Ausdrückliches `@Extensibility(FINAL_EXTENSIBILITY)` auf einer nicht finalen Klasse bildet `mapExtensibility` auf FINAL ab; `javapsm_8_6_1_2_final_can_be_said_on_a_class_that_is_not_final` prüft `getExtensibility()`, die Oktette ohne DHEADER (`01 00 00 00 02 00 00 00`) und die Extensibility-Annotation in `createType`. (Commit `f0649c27acc`)
### §8.6.1.2 EXTENSIBLE als Vorgabe in allen anderen Fällen

**Spec:** §8.6.1.2, S. 34 — "• EXTENSIBLE: In all other cases, by default, or if
explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_6_1_2_a_class_that_is_not_final_is_written_with_a_dheader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_6_1_2_extensible_is_written_as_appendable_with_a_dheader`

**Status:** `done` — `defaultExtensibility` liefert für jede Klasse, die nicht final ist oder nicht direkt von `Object` erbt, `ExtensibilityKind.APPENDABLE` (EXTENSIBLE von §8.6.1.2; XTypes 1.3 nennt die Art APPENDABLE, XCDR2 schreibt sie mit DHEADER). `javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible` prüft `OpenType` und `DerivedType`; `javapsm_8_6_1_2_a_class_that_is_not_final_is_written_with_a_dheader` die Oktette `08 00 00 00 01 00 00 00 02 00 00 00`. (Commit `f0649c27acc`)
### §8.6.1.2 EXTENSIBLE ausdrücklich

**Spec:** §8.6.1.2, S. 34 — "• EXTENSIBLE: [...] or if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::javapsm_8_6_1_2_extensible_is_written_as_appendable_with_a_dheader`

**Status:** `done` — `mapExtensibility` bildet `EXTENSIBLE_EXTENSIBILITY` auf `ExtensibilityKind.APPENDABLE` ab. `javapsm_8_6_1_2_extensible_is_written_as_appendable_with_a_dheader` setzt `@Extensibility(EXTENSIBLE_EXTENSIBILITY)`, prüft `getExtensibility()` und die Bytes (DHEADER 8, zwei `int`) und liest sie zurück. (Commit `f0649c27acc`)
### §8.6.1.2 MUTABLE nur ausdrücklich

**Spec:** §8.6.1.2, S. 34 — "• MUTABLE: Only if explicitly indicated."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.defaultExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.mapExtensibility`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeStruct`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_3_the_id_of_a_member_is_its_id_on_the_wire_and_an_absent_optional_member_has_none`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/ReflectionTypeSupportTest.java::xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/JavaTypeRepresentationTest.java::javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible`

**Status:** `done` — `defaultExtensibility` liefert nie MUTABLE; `mapExtensibility` liefert es für `@Extensibility(MUTABLE_EXTENSIBILITY)`. `javapsm_8_6_1_2_mutable_is_only_said_by_the_extensibility_annotation` prüft MUTABLE mit der Annotation, `javapsm_8_6_1_2_a_final_class_under_object_is_final_and_every_other_class_extensible` dass die Vorgabe es nicht ergibt; `TypesJavaRepresentationTest::javapsm_8_3_the_id_of_a_member_is_its_id_on_the_wire_and_an_absent_optional_member_has_none` und `xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four` schreiben und lesen (DHEADER und EMHEADER je Mitglied). (Commit `819bfcb46cb`)
### §8.6.2 Union über `@SerializeAs`

**Spec:** §8.6.2, S. 34 — "Any class may be annotated as a union with
@SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::xtypes_7_4_3_5_3_a_mutable_union_carries_its_discriminator_as_member_zero_and_the_cases_from_one`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_discriminator_that_selects_no_member_is_written_alone`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_6_2_a_union_has_its_discriminator_type_and_labels`

**Status:** `done` — Eine Klasse mit `@SerializeAs(UNION_TYPE)` wird eine Union: Diskriminator, dann das gewählte Mitglied. `javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member` prüft die Bytes und das Lesen, `xtypes_7_4_3_5_3_a_mutable_union_carries_its_discriminator_as_member_zero_and_the_cases_from_one` die MUTABLE-Form, `javapsm_8_6_2_a_union_has_its_discriminator_type_and_labels` die `UNION_TYPE`-Beschreibung aus `createType`. (Commit `819bfcb46cb`)
### §8.6.2 Genau ein Feld mit `@UnionDiscriminator`

**Spec:** §8.6.2, S. 34 — "• Such a class must annotate exactly one field to be
the discriminator with @UnionDiscriminator."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_misplaced_union_annotation_is_refused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member`

**Status:** `done` — `introspectUnion` verlangt genau ein Feld mit `@UnionDiscriminator`; ohne eines oder mit zweien wirft es `XcdrException`. `javapsm_8_6_2_a_misplaced_union_annotation_is_refused` prüft die Klassen `NoDiscriminator` und `TwoDiscriminators` (und `NotAUnion`, `UnmarkedMember`, `DuplicateLabel`). (Commit `819bfcb46cb`)
### §8.6.2 Übrige Felder mit `@UnionMember` und dem Wert des Diskriminators

**Spec:** §8.6.2, S. 34 — "• All other fields that are not transient or static
must be annotated with @UnionMember, which shall identify the discriminator
value associated with that field."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspectUnion`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.selectedMember`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_misplaced_union_annotation_is_refused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_6_2_a_discriminator_that_selects_no_member_is_written_alone`

**Status:** `done` — `introspectUnion` verlangt `@UnionMember(discriminatorValue = ...)` an jedem Feld außer dem Diskriminator (statische und transiente Felder sind schon vorher ausgeschieden) und weist doppelte Werte ab. `javapsm_8_6_2_a_misplaced_union_annotation_is_refused` prüft `UnmarkedMember` und `DuplicateLabel`; `javapsm_8_6_2_a_union_is_its_discriminator_and_the_selected_member` wählt das Mitglied über den Wert, `javapsm_8_6_2_a_discriminator_that_selects_no_member_is_written_alone` einen Wert ohne Mitglied. (Commit `819bfcb46cb`)

---

## §8.7 Enumerations and Bit Sets

### §8.7 Java-`enum` als DDS-Aufzählung

**Spec:** §8.7, S. 35 — "By default, any Java enumeration class will be
considered to be a DDS enumeration."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.encodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.decodeValue`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.enumConstant`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.enumeration`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_2_enumerations_structures_collections_and_maps_follow_table_8_1`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_2_an_enum_field_is_its_ordinal_as_an_int32`

**Status:** `done` — Eine Java-`enum` wird als Aufzählung geschrieben (Ordinalwert als `int32`) und über `enumConstant` gelesen; ein Wert außerhalb der Konstanten ist eine `XcdrException`. `TypesJavaRepresentationTest::javapsm_8_2_an_enum_field_is_its_ordinal_as_an_int32` prüft die Oktette von `Colour.BLUE` (`02 00 00 00`), das Lesen von 1 und die Ablehnung von 3; `createType` liefert `ENUMERATION_TYPE` mit drei Mitgliedern. (Commit `819bfcb46cb`)
### §8.7 Aufzählung als Bitmenge annotierbar, die Objekte ebenfalls annotiert

**Spec:** §8.7, S. 35 — "As in IDL, a type that is syntactically an enumeration
may be annotated as a bit set type. In this case, objects of these types must
also be annotated in order to be serialized correctly."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.bitBound`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_the_default_bit_bound_is_32_and_a_bit_set_field_names_its_enumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_a_misplaced_bit_set_annotation_is_refused`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_an_enum_set_marked_bit_set_is_a_mask_with_the_bit_bound_of_its_enumeration`

**Status:** `done` — Eine Aufzählung mit `@BitSet` (und `@BitBound`, Vorgabe 32) ist ein Bitset-Typ; `ofField` verlangt `@BitSet` auch am Mitglied und verweigert ein Mitglied vom Bitset-Typ ohne die Annotation mit `XcdrException`. `javapsm_8_7_the_default_bit_bound_is_32_and_a_bit_set_field_names_its_enumeration` schreibt ein Mitglied `Wide one` (vier Oktette, Bit der Konstante C), `javapsm_8_7_a_misplaced_bit_set_annotation_is_refused` prüft `UnannotatedBitSetMember`. (Commit `874bc5ac9fe`)
### §8.7 `EnumSet` und `java.util.BitSet` mit `@BitSet` als Bitmenge

**Spec:** §8.7, S. 35 — "A type member of type java.util.EnumSet or
java.util.BitSet will be serialized as a bit set if marked with @BitSet."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.ofField`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/JavaRepresentation.java` (`JavaRepresentation.constantsOfBits`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_an_enum_set_marked_bit_set_is_a_mask_with_the_bit_bound_of_its_enumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_the_default_bit_bound_is_32_and_a_bit_set_field_names_its_enumeration`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_a_bit_without_a_constant_is_refused_on_decode`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/SerializeAsTest.java::javapsm_8_7_a_collection_of_bit_set_values_is_a_sequence_of_bit_sets`

**Status:** `done` — Ein Mitglied vom Typ `EnumSet` oder `java.util.BitSet` mit `@BitSet` wird als Bitset geschrieben. `javapsm_8_7_an_enum_set_marked_bit_set_is_a_mask_with_the_bit_bound_of_its_enumeration` prüft ein Oktett 0x05 für READ und EXEC, `javapsm_8_7_the_default_bit_bound_is_32_and_a_bit_set_field_names_its_enumeration` ein `java.util.BitSet` mit `elementType` (0x0a in vier Oktetten), `javapsm_8_7_a_bit_without_a_constant_is_refused_on_decode` Bits ohne Konstante. Mehr als 64 Bits verweigert `bits` mit `XcdrException`. (Commit `874bc5ac9fe`)

---

## §8.8 Modules

### §8.8 Segmente des Paketnamens als Module

**Spec:** §8.8, S. 35 — "Each segment of a Java type’s package name shall
correspond to a module in the DDS Type System [DDS-XTypes]. For example, a class
com.acme.project.TheClass would be in the nested modules com::acme::project."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.baseTypeNames`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::javapsm_2_lookup_topic_description_finds_only_what_was_created`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/BuiltinTopicQosTest.java::javapsm_8_6_1_1_a_local_writer_describes_the_base_types_of_its_type`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_8_the_package_of_a_class_is_the_modules_of_the_type_name`

**Status:** `done` — `introspect` bildet den Typnamen aus dem Klassennamen mit `::` statt `.` und `$`. `javapsm_8_8_the_package_of_a_class_is_the_modules_of_the_type_name` prüft `com::acme::project::TypesTheClass` für eine Klasse im Paket `com.acme.project` (als Topic und am TypeSupport) und `org::zerodds::cdr::TypesJavaRepresentationTest::Skipped` für eine verschachtelte Klasse; `javapsm_8_6_1_1_a_local_writer_describes_the_base_types_of_its_type` den Namen einer verschachtelten Oberklasse in `getBaseTypeName`. `String` hat den Namen `DDS::String` (Item zu `DDS::String`). (Commit `f0649c27acc`)

---

## §8.9 Annotations

### §8.9 Java-Annotationstypen werden standardmäßig nicht abgebildet

**Spec:** §8.9, S. 35 — "This Type Representation ignores Java annotation types
by default."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.annotation`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/ReflectionTypeSupport.java` (`ReflectionTypeSupport.introspect`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflect`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_9_annotations_of_another_package_with_the_names_of_the_type_system_are_ignored`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_9_an_annotation_type_is_represented_only_when_marked`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_9_an_id_and_a_must_understand_of_another_package_do_not_reach_the_emheader`,
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/TypesJavaRepresentationTest.java::javapsm_8_9_an_optional_of_another_package_does_not_make_a_member_optional`

**Status:** `done` — `annotation` erkennt nur Annotationen aus `org.omg.dds.type` (und `org.zerodds.types`); fremde Annotationen mit gleichem einfachen Namen zählen nicht. `TypesJavaRepresentationTest::javapsm_8_9_annotations_of_another_package_with_the_names_of_the_type_system_are_ignored` und die Tests daneben verwenden Annotationen `Key`, `ID`, `Optional`, `MustUnderstand` und `Extensibility` aus dem Paket `org.zerodds.cdr.foreign`, mit denselben einfachen Namen: Ein fremdes `Key` macht den Typ nicht keyed, ein fremdes `Extensibility("MUTABLE")` die Klasse nicht MUTABLE, ein fremdes `ID(77)` ändert die Member-ID nicht, ein fremdes `MustUnderstand` setzt das M-Flag nicht, ein fremdes `Optional` macht ein Mitglied nicht optional. `javapsm_8_9_an_annotation_type_is_represented_only_when_marked` prüft, dass `createType` einen nicht markierten Annotationstyp ablehnt. (Commit `f0649c27acc`)
### §8.9 Annotationen im Typsystem nur mit `@SerializeAs`

**Spec:** §8.9, S. 35 — "Java annotations that are intended to be represented
explicitly within the DDS Type System must be so annotated with @SerializeAs."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/DynamicTypeFactoryImpl.java` (`DynamicTypeFactoryImpl.reflectAnnotationType`)

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/DynamicTypeReflectionTest.java::javapsm_8_9_an_annotation_type_is_represented_only_when_marked`

**Status:** `done` — `reflectAnnotationType` bildet einen Annotationstyp nur mit `@SerializeAs(ANNOTATION_TYPE)` ab, mit einem Mitglied je Element und dessen Vorgabewert. Der Test prüft `ANNOTATION_TYPE`, die Mitgliedsarten `STRING_TYPE` und `INT_32_TYPE`, den Vorgabewert "none" und die Ablehnung des nicht markierten Typs. (Commit `36094b9d871`)

---

## §9.1 Type Mapping

### §9.1 Typsystem nach [DDS-XTypes], übrige Aspekte nach dessen Plain Language Binding

**Spec:** §9.1, S. 37 — "The type system for DDS topic types is defined by the
Extensible and Dynamic Topic Types for DDS specification [DDS-XTypes]. This sub
clause defines the set of rules to be used in order to map abstract DDS topic
types into Java types that can be used by application programmers. Those aspects
of the DDS Type System that are not addressed below are as specified in the
Plain Language Binding as defined by [DDS- XTypes] (which in turn is defined in
terms of an IDL-to-Java mapping [Java-MAP])."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`JavaMapping`)

**Tests:** —

**Status:** `n/a (informative)` — Nennt die Quellen der Regeln. Der Java-Generator hat zwei Abbildungen: `--java-mapping psm` (Vorgabe, Clause 9) und `--java-mapping omg` (IDL to Java, öffentliche Felder); siehe die Items zu §9.1.1 und §9.1.2. (Commit `5e078003c16`)
### §9.1.1 Aggregattypen als finale Java-Klasse

**Spec:** §9.1.1, S. 37 — "DDS aggregation types shall be mapped to a final Java
class."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`data_class`),
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_the_psm_types_and_codec_compile_without_a_warning`

**Status:** `done` — Beide Abbildungen erzeugen `public static final class` je Struktur. Der Test prüft unter `JavaMapping::Psm` `public static final class Point {` und `Track {` im erzeugten Text; `javapsm_9_1_1_the_psm_types_and_codec_compile_without_a_warning` übersetzt die Ausgabe mit `javac`. Die Vorgabe von `idlc generate --lang java` ist `psm`, Clause 9 (`the_java_mapping_defaults_to_psm_and_omg_is_selectable`); `--java-mapping omg` wählt IDL to Java. (Commit `5e078003c16`)
### §9.1.1 Attribute gekapselt

**Spec:** §9.1.1, S. 37 — "Contained attributes shall be encapsulated."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`field`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Unter `JavaMapping::Psm` ist jedes Mitglied ein `private`-Feld mit Zugriffen. Der erste Test prüft `private int x;` und dass `public int x;` fehlt; der zweite, dass der Codec über `getX()`/`setX(...)` zugreift und kein direkter Feldzugriff bleibt. Unter `omg` (`--java-mapping omg`) bleiben die Felder öffentlich. (Commit `5e078003c16`)
### §9.1.1 Zugriffe nach Java Beans

**Spec:** §9.1.1, S. 37 — "Java Bean style accessors shall be provided."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`getter_name`),
`crates/lang-rules/src/adapter/java.rs` (`setter_name`),
`crates/lang-rules/src/adapter/java.rs` (`boolean_getter_name`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_a_union_keeps_its_discriminator_as_the_property_discriminator`

**Status:** `done` — Unter `JavaMapping::Psm` erzeugt der Generator `getX()` und `setX(int v)`, bei `boolean` zusätzlich `isOn()`, und für eine Union `getDiscriminator()`/`setDiscriminator`. Die Tests prüfen den erzeugten Text; `javapsm_9_1_1_the_psm_types_and_codec_compile_without_a_warning` übersetzt ihn. Unter `omg` gibt es keine Zugriffe. (Commit `5e078003c16`)
### §9.1.1 Sonderregeln für `boolean`-Eigenschaften erlaubt

**Spec:** §9.1.1, S. 37 — "Special mapping rules for boolean properties are
allowed."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`boolean_getter_name`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`

**Status:** `n/a (informative)` — Erlaubnis. Der Generator macht davon Gebrauch: `isOn()` und `getOn()` für ein `boolean`-Mitglied, geprüft im Test zu §9.1.1. (Commit `5e078003c16`)
### §9.1.1 Innerer Zustand privat

**Spec:** §9.1.1, S. 37 — "The representation of internal state shall be
private."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`),
`crates/lang-rules/src/adapter/java.rs` (`field`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_a_union_keeps_its_discriminator_as_the_property_discriminator`

**Status:** `done` — Unter `JavaMapping::Psm` sind Felder und der Diskriminator einer Union `private` (`private int x;`, `private int _d;`); die Tests prüfen den Text, und dass `public int x;` fehlt. (Commit `5e078003c16`)
### §9.1.2 Unbegrenzte Folgen als `Collection<E>`, gekapselt, mit Zugriffen nach Java Beans

**Spec:** §9.1.2, S. 37 — "Unbounded DDS sequences are mapped to Collection<E>
interface. The state is encapsulated and getters/setters are provided through
bean style property accessors."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`),
`crates/lang-rules/src/adapter/java.rs` (`encapsulate`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_2_an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Unter `JavaMapping::Psm` wird `sequence<long>` ein `private java.util.Collection<java.lang.Integer>` mit `getSamples()` und `setSamples(...)`; geschachtelte Folgen sind `Collection<Collection<String>>`. Der Test prüft den Text, der zweite Test den Codec über `Sequences`; `omg` (`--java-mapping omg`) bildet unverändert auf `T[]` ab. (Commit `5e078003c16`)
### §9.1.2 Begrenzte Folgen als Java-Arrays

**Spec:** §9.1.2, S. 37 — "Bounded sequences and arrays are mapped to Java
arrays."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`)

**Tests:**
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_2_an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_1_the_codec_reads_and_writes_through_the_accessors`

**Status:** `done` — Unter `JavaMapping::Psm` bleibt `sequence<long, 4>` ein `private int[] bounded;`; `type_at` wählt `Collection` nur für `Bound::Unbounded`. Der Test prüft den Text, der Codec kürzt mit `Sequences.first(v, 4)`. (Commit `5e078003c16`)
### §9.1.2 Arrays als Java-Arrays

**Spec:** §9.1.2, S. 37 — "Bounded sequences and arrays are mapped to Java
arrays."

**Repo:**
`crates/lang-rules/src/adapter/java.rs` (`type_at`)

**Tests:**
`crates/lang-rules/src/adapter/java.rs::tests::javapsm_9_1_2_a_sequence_and_an_array_are_both_java_arrays`,
`crates/lang-rules/tests/java_psm_mapping.rs::javapsm_9_1_2_an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`

**Status:** `done` — `type_at`, Zweig `TypeKind::Array`: `long a[3]` wird `int[]`, `long g[2][3]` wird `int[][]`. `javapsm_9_1_2_a_sequence_and_an_array_are_both_java_arrays` prüft das unter `omg`; unter `psm` prüft der zweite Test `private ZdTypes.demo.Point[] vicinity;` und ein Array von Folgen als `Collection<Integer>[]`. (Commit `5e078003c16`)

---

## §9.2 Example

### §9.2 Beispiel `Point` und `RadarTrack`

**Spec:** §9.2, S. 38–39 — "Example (non-normative) [...] public final class
Point { Point(); Point(int x, int y, java.lang.Integer z); int getX(); void
setX(int32 v); [...] }; public class final RadarTrack { [...]
java.util.Collection<byte> getPlot(); [...] };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es ist kein übersetzbares Java (`int32`, `public class final`, `Collection<byte>`, `};`). `idlc generate --lang java --java-mapping psm` erzeugt für IDL finale Klassen mit privaten Feldern, Bean-Zugriffen und `Collection<E>` für unbegrenzte Folgen. (Commit `5e078003c16`)

---

## Annex A Java JAR Library File

### Annex A JAR `omgdds.jar` mit allen Klassen der PSM

**Spec:** Annex A, S. 41 — "In addition to this document, this specification
includes a Java Archive (JAR) library, omgdds.jar. This library contains
compiled Java *.class files for all of the classes and interfaces specified by
this PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibt das Artefakt der OMG. `crates/java-omgdds/java/pom.xml` baut die JAR `org.zerodds:omgdds` mit den Klassen unter `org/omg/dds`, deren Quellen bis auf 11 von 184 Dateien den OMG-Quellen entsprechen (siehe Items zu Annex A und B).
### Annex A Übersetzen gegen `omgdds.jar`, Ausführen gegen jede Implementierung

**Spec:** Annex A, S. 41 — "This library comprises the compile-time portion of
this specification: users shall be able to compile their PSM-compliant code
against this library and then deploy the result against any conformant
implementation."

**Repo:**
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/DDSException.java` (`DDSException`),
`crates/java-omgdds/java/src/main/java/org/omg/dds/core/EntityQos.java` (`EntityQos`),
`crates/java-omgdds/java/pom.xml`

**Tests:**
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_dds_exception_is_a_dds_object_and_unchecked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesPsmApiTest.java::javapsm_a_no_class_of_the_api_package_refers_to_the_implementation`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_every_table_7_1_exception_is_an_abstract_dds_exception_of_org_omg_dds_core`

**Status:** `done` — `org/omg/dds/core/DDSException.java` implementiert wie die OMG-Quelle `DDSObject`; die sieben Ausnahmeklassen in `org.zerodds.core` antworten mit `getEnvironment()` (das Standard-Environment der Implementierung). Ein Vergleich der Dateien unter `org/omg/dds` mit `.agent/omg-java/srcJava` ergibt nach Bytes gleiche Dateien bis auf `EntityQos` (zusätzlich `@SuppressWarnings` und ein Kommentar) und drei Dateien mit anderen Zeilenenden (`DomainParticipantFactory`, `DataWriter`, `DataReader`). `TypesPsmApiTest` prüft `DDSObject` an `DDSException`, die `serialVersionUID`-Werte und abstrakte Klassen der zehn Ausnahmen und dass keine Klasse in `org.omg.dds` (ohne `rpc`) auf `org/zerodds` verweist, also gegen eine andere Implementierung laufen kann. Die Tests des Moduls sind selbst Clients der Klassen von `org.omg.dds`. (Commit `63098240e0c`)
### Annex A `omgdds` neben der Implementierung oder in einer gemeinsamen Bibliothek

**Spec:** Annex A, S. 41 — "Distributors of binary implementations of this PSM
may elect to distribute the omgdds library alongside their implementation
libraries or to package both the contents of omgdds.jar and their implementation
into a single library."

**Repo:**
`crates/java-omgdds/java/pom.xml`

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis für den Verteiler. ZeroDDS packt die Klassen von `org/omg/dds` und die Implementierung in eine JAR (`org.zerodds:omgdds`, `crates/java-omgdds/java/pom.xml`). (Commit `5536e2abcac`)

---

## Annex B Java Source Code

### Annex B Quellen in `omgdds_src.zip`, normativ für Schnittstellen und Kommentare

**Spec:** Annex B, S. 43 — "In addition to this document, this specification
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_every_table_7_1_exception_is_an_abstract_dds_exception_of_org_omg_dds_core`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_1_dds_exception_is_a_dds_object_and_unchecked`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreApiShapeTest.java::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source`

**Status:** `done` — Die zehn Ausnahmeklassen unter `org/omg/dds/core` sind die Dateien der OMG-Quellen (Kommentare, `serialVersionUID`, `DDSException implements DDSObject`). Alle 184 Dateien unter `org/omg/dds` stimmen nach Zeichen mit der Quelle überein (Zeilenenden als LF gelesen), einschließlich der Kommentare: `type/TypeSupport.java` und `core/EntityQos.java` sind die Dateien der Quelle (die Abbildung des Namens `java.lang.Class` auf den Namen der Klasse steht in `ServiceEnvironmentImpl.newTypeSupport`; die Warnung „possible heap pollution“ an `EntityQos.withPolicies(P...)` ist durch die Übersetzungsoptionen des Moduls ausgeschaltet: `-Xlint:all,-unchecked,-options -Werror` für die Hauptquellen in `pom.xml`). `CoreApiShapeTest::javapsm_b_the_files_of_org_omg_dds_have_the_digests_of_the_omg_source` vergleicht die Dateien bei jedem Lauf mit den SHA-256 der Quelle (`src/test/resources/omgdds-srcjava.sha256`, aus `omgdds_src.zip`) und verlangt, dass keine abweicht. `TypesPsmApiTest` prüft die `serialVersionUID`-Werte und, dass `ServiceConfigurationException` und `ServiceInitializationException` in der Quelle einfache `RuntimeException`s sind. (Commit `53910e766ac`)
### Annex B Beispiele, `build.xml` und Projektdateien nicht normativ

**Spec:** Annex B, S. 43 — "For the convenience of both implementers and
application developers, the archive contains additional files that are neither
API source code nor documentation. These file are non-normative and include:
[...]"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Nicht normativ.

---

## Audit-Status

447 done / 0 partial / 0 open / 0 blocked / 79 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document dds-java-psm-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 1 Test grün.
* `python3 ci/run_integration.py merge-codegen-java` (Job `merge-codegen-java`) — 8 Tests grün.
* `python3 ci/run_integration.py merge-java-omgdds` (Job `merge-java-omgdds`) — 642 Tests grün.

651 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
