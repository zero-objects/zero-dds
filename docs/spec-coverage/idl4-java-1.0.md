# IDL4 to Java Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (OMG formal/2021-08-01)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Die Abbildung ist `crates/lang-rules/src/emit/idl4_java/`,
ausgewählt mit `zerodds-idlc generate --lang java --java-mapping idl4`
(`--java-naming idl|java`, `--java-platform dds|corba`,
`--java-corba-module`). Komponenten, Homes, Ports, Konnektoren und
Template-Modul-Instanzen gehen über ihr intermediate IDL
(`crates/lang-rules/src/emit/intermediate_idl.rs`, `crates/ccm`). Die
Laufzeitklassen, die die Spec nennt, liegen in
`runtime/java/src/main/java/org/omg/type/` und, für die CORBA-Plattform,
`runtime/java/corba/org/omg/CORBA/`; das Pseudo-IDL des
CORBA-Moduls ist `runtime/corba/CORBA_PIDL.idl`. Jeder Test übersetzt das
erzeugte Java zusammen mit der Laufzeit mit `javac --release 8
-Xlint:all -Werror` und führt ein Prüfprogramm aus, dessen Erwartungen
der Spec-Text sind.

Der Standard von `--lang java` bleibt die DDS-Java-PSM-Bindung mit dem
XCDR2-Codec (`crates/lang-rules/src/adapter/java.rs`,
`docs/spec-coverage/zerodds-xcdr2-java-1.0.md`); sie folgt an mehreren
Stellen ausdrücklich nicht dieser Spec und ist hier nicht Gegenstand.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`, `tools/idlc/src/main.rs`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_6_a_value_type_is_an_abstract_and_a_concrete_class`, `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — `zerodds-idlc generate --lang java --java-mapping idl4` bildet IDL nach Klausel 7 und 8 auf Java ab: ein Paket je Modul, eine Datei je Klasse, Komponenten, Homes, Ports, Konnektoren und Template-Modul-Instanzen über ihr intermediate IDL; jeder Test übersetzt die erzeugten Klassen mit `javac --release 8 -Xlint:all -Werror` und prüft sie per Reflexion und Ausführung. Der Standard von `--lang java` bleibt die DDS-Java-PSM-Bindung (Entscheidung, siehe Kontext).

### 1.2 Nutzung moderner Java-Sprach-Features

**Spec:** §1, S. 1 — "The language mapping makes use of modern Java
language features as appropriate and natural."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Scope-Aussage ohne prüfbares Soll.

---

## §2 Conformance

### 2.1 Implementation: IDL -> Java Source per §7

**Spec:** §2, S. 1 — "A conformant implementation shall transform
IDL input into Java source code output as specified in clause 7."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_2_a_union_is_a_final_class_with_discriminator_accessors_and_modifiers`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — die Abbildung `--java-mapping idl4` schreibt Java-Quelltext nach Klausel 7; die Tests übersetzen und führen ihn aus.

### 2.2 User: Anwendungscode läuft cross-Implementation portabel

**Spec:** §2, S. 1 — "Application source code that conforms to this
specification makes use of the Java data types and API's as defined
in clause 7. [...] Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage; kein
Implementierungs-Soll.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, S. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung.

### 3.2 [IDL4] IDL 4.2 (2018)

**Spec:** §3, S. 1 — "[IDL4] OMG IDL 4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung; das
Frontend steht in `crates/idl-parse/` und `crates/idl-lower/`.

### 3.3 [J2SE 8.0] Java SE 8 Edition (2015)

**Spec:** §3, S. 1 — "[J2SE 8.0] Java SE 8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung.

### 3.4 [JavaBeans] JavaBeans 1.01

**Spec:** §3, S. 1 — "[JavaBeans] JavaBeans 1.01."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung; die
Naming-Konventionen sind in §7.1.1.2 befundet.

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, S. 1 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.2 Camel Case (Lower Camel Case)

**Spec:** §4, S. 2 — "Camel Case [...] the variation of Camel Case
commonly known as Lower Camel Case, where the first letter is not
capitalized."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; die Anwendung ist
in §7.1.1.2.2 befundet.

### 4.3 Java (general-purpose programming language)

**Spec:** §4, S. 2 — "Java is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.4 Language Mapping

**Spec:** §4, S. 2 — "An association of elements in one language to
elements in another language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, S. 2 — "Pascal Case, also known as Upper Camel Case,
[...] 'these are my words' would be 'TheseAreMyWords'."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; die Anwendung ist
in §7.1.1.2.1 befundet.

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: CCM, CORBA, DDS, J2SE, IDL, OMG

**Spec:** §5 Tab.5.1, S. 2 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §6 Additional Information

### 6.1 Alternative zur existierenden OMG IDL-Java-Mapping-Spec

**Spec:** §6.1, S. 3 — "This specification is an alternative to the
existing OMG IDL to Java Mapping specification; it is distinct in
that it provides a mapping for the constructs of IDL4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

### 6.2 Acknowledgments (ADLINK, RTI, Twin Oaks)

**Spec:** §6.2, S. 3 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.1 General

### 7.1.1.0 Naming-Schemes: IDL vs. Java; selektiert via @java_mapping

**Spec:** §7.1.1, S. 5 — "This specification defines two naming
schemes [...] The @java_mapping annotation defined in Clause 8.1
provides a mechanism to select the appropriate naming scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_java_mapping_reaches_the_model_parameter_by_parameter`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — die beiden Schemata sind `Naming::Idl` und `Naming::Java`; `--java-naming` wählt das Schema des Laufs, `@java_mapping(apply_naming_convention = ...)` überschreibt es je Modul und Deklaration.

### 7.1.1.0 Name-Kollision mit Reserved-Names: Underscore-Präfix

**Spec:** §7.1.1, S. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending an underscore ('_') to the mapped name."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::escape`, `crates/lang-rules/src/emit/idl4_java/names.rs::escape_type`, `crates/lang-rules/src/emit/idl4_java/names.rs::escape_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — ein abgebildeter Name, der ein Java-Keyword oder -Literal, `Constants` oder eine Methode von `java.lang.Object` ist, bekommt ein vorangestelltes `_` (`_class`, `_Constants`, `_hashCode`), unter beiden Schemata.

### 7.1.1.1 IDL Naming Scheme: Names ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::type_name`, `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::variable`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — unter `Naming::Idl` bleiben Typ-, Member-, Konstanten- und Modulnamen unverändert; die Tests prüfen die IDL-Schreibweise in Klassen, Accessoren und Paketen.

### 7.1.1.2 Java Naming Scheme: JavaBeans-1.01-Konventionen

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — unter `Naming::Java` folgen Typen Pascal Case, Member und Methoden Camel Case, Accessoren `get<Name>`/`set<Name>`, Konstanten und Enumeratoren Großbuchstaben, Pakete Kleinbuchstaben (Tabelle 8.1).

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, S. 5-6 — "The first letter after each
underscore shall be capitalized and all underscores shall be removed.
The first letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case_is_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`

**Status:** `done` — alle 14 Beispiele der Klausel ergeben die Schreibweise, die sie nennt.

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, S. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case_is_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — alle 15 Beispiele der Klausel ergeben die Schreibweise, die sie nennt. Wo ein späteres Beispiel der Spezifikation seiner Regel widerspricht (`longLongVariable` in §7.2.4.3.1), gilt die Regel (`crates/lang-rules/src/emit/idl4_java/names.rs`, Moduldoku).

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, S. 6 — "Every letter shall be capitalized and
all underscores shall be removed."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::all_uppercase`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::upper_and_lower_case_are_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — die Beispiele der Klausel; Konstanten im Container und Enumeratoren unter dem Java-Schema stehen in Großbuchstaben.

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, S. 6-7 — "Every letter shall be in lower case
and all underscores shall be removed."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::all_lowercase`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::upper_and_lower_case_are_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — die Beispiele der Klausel; das Modul `MY_MATH` ist unter dem Java-Schema das Paket `my_math`.

### 7.1.1.3 Suffixes (z.B. Abstract); Underscore-Präfix bei Kollision

**Spec:** §7.1.1.3, S. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::escape_type`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — eine Struktur `FooAbstract` wird `_FooAbstract`; `Abstract` allein bleibt, weil es nicht auf das Suffix endet, sondern es ist.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, S. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords; literals (true/false/null); java.lang.Object
methods (clone/equals/getClass/hashCode/notify/notifyAll/finalize/
toString/wait)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::JAVA_KEYWORDS`, `crates/lang-rules/src/emit/idl4_java/names.rs::JAVA_LITERALS`, `crates/lang-rules/src/emit/idl4_java/names.rs::OBJECT_METHODS`, `crates/lang-rules/src/emit/idl4_java/names.rs::CONSTANTS_CLASS`, `crates/lang-rules/src/emit/idl4_java/names.rs::ABSTRACT_SUFFIX`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — reserviert sind die 50 Keywords der Klausel, `true`/`false`/`null`, `<type>Abstract`, `Constants` und die neun Methoden von `java.lang.Object`.

### 7.1.3 Holder<E>-Klasse für inout/out-Parameter

**Spec:** §7.1.3, S. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. [...] `package org.omg.type; public class Holder<E>
{ public E value; }`."

**Repo:** `runtime/java/src/main/java/org/omg/type/Holder.java`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::operation_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — `org.omg.type.Holder<E>` mit dem öffentlichen Feld `value`; `out`- und `inout`-Parameter sind `Holder<Boxed>`, und der Test ruft eine Implementierung, die über die Holder antwortet.

### 7.1.4 Tab.7.1: Java-Sprachversionen pro Feature

**Spec:** §7.1.4, S. 8 — Tab.7.1 listet J2SE 5.0 für Enumerations,
Generics, Annotation Type-Declaration; Java SE 8.0 für Annotation
Type-Use.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Tab.7.1 beschreibt die Sprachversionen
der Spec; sie ist keine eigene Mapping-Anforderung.

### 7.1.5 Code Examples Notation `{...}` (informativ)

**Spec:** §7.1.5, S. 8 — "the notation {...} is used in describing
Java code [...] generated code is specific to a particular vendor's
implementation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 9 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.2 IDL Module -> Java Package

**Spec:** §7.2.2, S. 9 — "An IDL module is mapped to a Java package
with the same name. [...] IDL declarations not enclosed in any
modules are mapped to classes or interfaces in the (unnamed) Java
global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::package_of`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Gen::file`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — ein Modul ist ein Paket gleichen Namens, eine Datei je öffentlicher Klasse im Verzeichnis des Pakets; Deklarationen außerhalb jedes Moduls stehen im unbenannten Paket.

### 7.2.3 Default Mapping: IDL Constant -> public final class mit value-Field

**Spec:** §7.2.3, S. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — eine Konstante ist eine `public final class` ihres Namens mit `public static final <T> value`, das Beispiel der Klausel unter beiden Schemata.

### 7.2.3.1 Alternative Mapping: Constants-Container-Klasse

**Spec:** §7.2.3.1, S. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`

**Status:** `done` — `@java_mapping(constants_container = ...)` sammelt die Konstanten eines Scopes in einer Klasse, `Constants` als Vorgabe, oder im angegebenen Namen.

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, S. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — Tabelle 7.2 vollständig: `int8`/`uint8`/`octet` `byte`, `short`/`unsigned short` `short`, `long`/`unsigned long` `int`, `long long`/`unsigned long long` `long`.

### 7.2.4.1.1 Unsigned-Typen nach Tab.7.2

**Spec:** §7.2.4.1.1 Tab.7.2, S. 11 — "uint8 -> byte; unsigned short/uint16 -> short; unsigned long/uint32 -> int; unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — die unsigned Typen haben den Java-Typ gleicher Breite (Tabelle 7.2, `promote_integer_width` FALSE als Vorgabe); ihr Bitmuster bleibt erhalten.

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3)

**Spec:** §7.2.4.1.2 Tab.7.3, S. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `float` `float`, `double` `double`, `long double` `java.math.BigDecimal`.

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, S. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `char` ist `char`.

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, S. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `wchar` ist `char`.

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, S. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::java_value`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `boolean` ist `boolean`; die Konstanten `TRUE` und `FALSE` werden `true` und `false`.

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, S. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `octet` ist `byte`.

### 7.2.4.2.1.1 Sequences of Basic Types -> type-specific Interfaces

**Spec:** §7.2.4.2.1.1 Tab.7.4, S. 12 — type-specific Sequence-Interfaces
in `org.omg.type` mit `extends java.util.List<Boxed>`.

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::specific_sequence`, `runtime/java/src/main/java/org/omg/type/`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — eine Sequenz eines Basistyps ist die Klasse aus Tabelle 7.4 in `org.omg.type` (`BooleanSeq` bis `BigDecimalSeq`), eine `java.util.List` des geboxten Typs mit den Methoden der Klausel. Die Klausel nennt sie Interfaces mit Konstruktoren; ein Interface hat keinen Konstruktor, also sind es Klassen, und `prim get(int)` neben `Boxed get(int)` ist in Java nicht deklarierbar (JLS 8.4.8.3) — beides steht in den Klassen dokumentiert.

### 7.2.4.2.1.1 Bounds-Checking auf bounded Sequences

**Spec:** §7.2.4.2.1.1, S. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — der Modifier einer beschränkten Sequenz wirft `java.lang.IndexOutOfBoundsException` für zu viele Elemente.

### 7.2.4.2.1.2 Sequences of non-Basic Types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, S. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — eine Sequenz jedes anderen Typs ist `java.util.List<E>` des geboxten Elementtyps.

### 7.2.4.2.2 IDL string -> java.lang.String + IndexOutOfBoundsException

**Spec:** §7.2.4.2.2, S. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `string` ist `java.lang.String`; der Modifier wirft `IndexOutOfBoundsException` für eine überschrittene Schranke und für ein Zeichen außerhalb von `char`.

### 7.2.4.2.3 IDL wstring -> java.lang.String

**Spec:** §7.2.4.2.3, S. 14 — analog §7.2.4.2.2.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `wstring` ist `java.lang.String`; die Schranke wird im Modifier geprüft.

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException


**Spec:** §7.2.4.2.4, S. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `fixed<P,S>` ist `java.math.BigDecimal`; ein Wert mit mehr Stellen als `P` oder `S` erlauben wirft `java.lang.ArithmeticException`.

### 7.2.4.3.1 IDL struct -> Java public class + Serializable + Bean-Accessoren + Konstruktoren

**Spec:** §7.2.4.3.1, S. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor per member, public
modifier per member, public all-values constructor, public default
constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_the_default_constructor_initialises_every_member`

**Status:** `done` — eine öffentliche Klasse, `java.io.Serializable`, private Felder, Accessor und Modifier je Member, Default- und All-Values-Konstruktor, das Beispiel der Klausel unter beiden Schemata.

### 7.2.4.3.1 Default-Constructor: Members initialisieren

**Spec:** §7.2.4.3.1, S. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string; Arrays - declared size with element defaults;
Sequences - zero-length; Others - default constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::initial`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_the_default_constructor_initialises_every_member`

**Status:** `done` — Primitive behalten Javas Vorgabe, Strings sind leer, Arrays haben ihre Größe mit vorkonstruierten Elementen, Sequenzen sind leer, alles andere kommt aus seinem Default-Konstruktor.

### 7.2.4.3.1 Accessor/Modifier-Naming pro Naming-Scheme

**Spec:** §7.2.4.3.1, S. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::getter`, `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::setter`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`

**Status:** `done` — `get_<name>`/`set_<name>` unter dem IDL-Schema, `get<Name>`/`set<Name>` unter dem Java-Schema.

### 7.2.4.3.2 IDL union -> Java public final class + Discriminator-Accessor

**Spec:** §7.2.4.3.2, S. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public accessor for
discriminator [...] public accessor/modifier per member [...]"

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_2_a_union_is_a_final_class_with_discriminator_accessors_and_modifiers`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — die Beispiele `U1` und `U2`: `public final class`, Diskriminator-Accessor, Accessor und Modifier je Member, Modifier mit Diskriminator für mehrere Labels und den Default, `__default()`, `IllegalStateException` und `IllegalArgumentException` wie die Klausel sie nennt.

### 7.2.4.3.3 IDL enum -> Java public enum + valueOf(int)


**Spec:** §7.2.4.3.3, S. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name [...] private member to hold the
value [...] static helper method valueOf(int)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — ein Java-`enum` mit `@value`, `getValue()` und `valueOf(int)`, das für einen unbekannten Wert eine RuntimeException wirft.

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_4_a_recursive_type_names_its_own_class`

**Status:** `done` — eine Struktur und eine Union, die eine Sequenz ihrer selbst halten, sind Klassen, deren Member ihre eigene Klasse nennen; der Test baut verschachtelte Werte.

### 7.2.4.4 IDL Array -> Java Array + IndexOutOfBoundsException

**Spec:** §7.2.4.4, S. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_4_an_array_is_a_java_array_and_its_bounds_are_checked`

**Status:** `done` — ein Java-Array des Elementtyps, das Beispiel `S2`; der Modifier wirft `IndexOutOfBoundsException` für eine falsche Länge.

### 7.2.4.5 IDL native -> kein definiertes Mapping

**Spec:** §7.2.4.5, S. 20 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.4.6 IDL typedef -> kein Java-Type; Use replaced; Annotations propagieren

**Spec:** §7.2.4.6, S. 20 — "Java does not have a typedef construct
[...] The use of an IDL typedef type shall be replaced with the
type referenced by the typedef type. This rule shall apply
recursively. Annotations on an IDL typedef shall be applied to
uses of the typedef in other type declarations."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::type_declaration`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`, `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_15_4_2_an_annotation_prefixing_a_typedefs_type_annotates_the_typedef`

**Status:** `done` — ein `typedef` erzeugt keine Klasse, jede Verwendung trägt den Zieltyp, und `@max(100)` am `typedef` prüft der Modifier jeder Verwendung, auch als Sequenzelement.

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, S. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `runtime/java/src/main/java/org/omg/type/Any.java`, `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_3_any_is_org_omg_type_any`

**Status:** `done` — `any` ist `org.omg.type.Any` mit Einfügen, Auslesen und dem Typ des Werts; auf der CORBA-Plattform `org.omg.CORBA.Any` (A.1.4).

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface

**Spec:** §7.4, S. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. [...] Each operation
defined in the IDL interface shall map to a method in the Java
interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::interface`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::attribute_methods`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::operation_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_6_the_interface_annotations_leave_the_java_unchanged`

**Status:** `done` — jedes Interface ist ein `public interface`, Basisinterfaces werden `extends`, Attribute Accessor und Modifier (ohne Modifier für `readonly`), Operationen Methoden mit Parametern in IDL-Reihenfolge, `raises` ein `throws`.

### 7.4.1 IDL exception -> Java class extends RuntimeException

**Spec:** §7.4.1, S. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members [...] following the
rules of the IDL struct mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_1_an_exception_extends_user_exception_and_members_name_the_annexs_classes`

**Status:** `done` — eine Exception ist eine Klasse, die `java.lang.RuntimeException` erweitert, mit Membern, Accessoren und Konstruktoren einer Struktur; auf der CORBA-Plattform erweitert sie `org.omg.CORBA.UserException` (A.1.1).

### 7.4.2 Interface Forward-Declaration: kein Java-Mapping

**Spec:** §7.4.2, S. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::top_level_interface`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — eine Vorwärtsdeklaration erzeugt keine Datei; die Definition hat ihre eine.

---

## §7.5 Interfaces – Full

### 7.5 Embedded Type-/Const-/Exception-Decls im Interface-Body

**Spec:** §7.5, S. 23 — "The embedded elements shall be mapped to a
public declaration within the scope of the Java interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::embedded_declarations`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_5_declarations_embedded_in_an_interface_are_nested_in_it`

**Status:** `done` — Typen, Konstanten und Exceptions im Rumpf eines Interface sind öffentliche Deklarationen im Java-Interface, das Beispiel `FullInterface`.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java-Klassen (Abstract + concrete)

**Spec:** §7.6, S. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name [...] private members are
protected [...] Each valuetype initializer is mapped to a method
returning void."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::valuetype`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_6_a_value_type_is_an_abstract_and_a_concrete_class`

**Status:** `done` — die Beispiele `VT1` und `VT2`: die abstrakte Klasse `<Name>Abstract` mit öffentlichem Zustand, `protected` privatem Zustand, abstrakten Operationen und Initialisierern (`void`), die Klasse `<Name>`, Basis und unterstütztes Interface.

---

## §7.7 CORBA-Specific – Interfaces

### 7.7 CORBA-Specific -> Annex A.1

**Spec:** §7.7, S. 25 — "CORBA-specific mappings are defined in
clause A.1 of Annex A."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.8 CORBA-Specific – Value Types

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.9 Components – Basic

### 7.9 Components Basic -> intermediate IDL

**Spec:** §7.9, S. 25 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::transform_component`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`

**Status:** `done` — eine Komponente wird ihr äquivalentes Interface nach CCM 4.0 §6 und dieses nach §7.4: `extends Components.CCMObject`, `provide_`, `connect_`/`disconnect_`/`get_connection(s)_`, `subscribe_`/`unsubscribe_`, `get_consumer_`, Attribute; der Ereignistyp wird Wertetyp und `<E>Consumer`. Die `Components`-Deklarationen kommen aus dem IDL, das die Anwendung einbindet (Test: Transkription von CCM 4.0 §6).

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, S. 25 — analog §7.9.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::transform_home`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces`

**Status:** `done` — ein Home wird `<Home>Explicit` (erweitert `Components.CCMHome`, Factory-Operationen mit `CreateFailure`), `<Home>Implicit` (erweitert `Components.KeylessCCMHome`, `create()`) und `<Home>`, das beide erweitert.

---

## §7.11 CCM-Specific

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.12 Components – Ports and Connectors

### 7.12 Ports/Connectors -> intermediate IDL

**Spec:** §7.12, S. 25 — analog §7.9.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::expand_extended_ports`, `crates/ccm/src/transform.rs::transform_connector`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`, `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_12_a_connector_maps_through_its_ports`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type`

**Status:** `done` — ein erweiterter Port wird die Basis-Ports seines Port Types (`<port>_<basic port>`), ein `mirrorport` mit vertauschten Facets und Receptacles; ein Konnektor wird über seine Ports wie eine Komponente abgebildet, seine Attribute wie die eines Interface.

---

## §7.13 Template Modules

### 7.13 Template-Module Instances -> intermediate IDL

**Spec:** §7.13, S. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** `crates/idl-lower/src/lower.rs::template_module_inst`, `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_13_a_template_module_instance_is_a_package_of_its_own`

**Status:** `done` — eine Instanz `module Buffer<long, 4> LongBuffer;` ist das Paket `LongBuffer` mit den Deklarationen der Vorlage, `T` durch `long` und `Capacity` durch `4` ersetzt; die Vorlage selbst erzeugt nichts.

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> Java extends

**Spec:** §7.14.1, S. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class [...] all values constructor [...] first parameter a
non-null instance of the base struct's Java class."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_1_a_derived_structure_extends_its_base_class`

**Status:** `done` — die abgeleitete Klasse erweitert die der Basis, und ihr All-Values-Konstruktor nimmt als ersten Parameter eine Instanz der Basisklasse, das Beispiel `S5 : S1`.

### 7.14.2 Union-Discriminator: int8/uint8/wchar/octet zusätzlich erlaubt

**Spec:** §7.14.2, S. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — `int8`, `uint8`, `wchar` und `octet` diskriminieren eine Union wie §7.2.4.3.2 sie abbildet.

### 7.14.3.1 IDL map -> java.util.Map<K,V>


**Spec:** §7.14.3.1, S. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::map_boxed`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_1_a_map_is_a_java_util_map_of_table_7_5_types`

**Status:** `done` — eine Map ist `java.util.Map` der geboxten Typen aus Tabelle 7.5, eine beschränkte wird geprüft, das Beispiel `S4`.

### 7.14.3.2 IDL bitset -> Java public class mit Bitfield-Accessoren

**Spec:** §7.14.3.2, S. 28 — "An IDL bitset shall map to Java as a
public class with the same name. [...] Default member type: boolean
if size is 1, octet 2-8, unsigned short 9-16, unsigned long 17-32,
unsigned long long 33-64."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitset`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_2_a_bit_set_is_a_class_with_bit_field_accessors`

**Status:** `done` — eine öffentliche Klasse mit Accessor und Modifier je benanntem Bitfeld, ohne Typangabe der kleinste Typ der Klausel (`boolean`, `byte`, `short`, `int`, `long`), das Beispiel `MyBitset`.

### 7.14.3.3 IDL bitmask -> Java enum + java.util.BitSet

**Spec:** §7.14.3.3, S. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be the
IDL bitmask name with the Flags suffix appended. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`

**Status:** `done` — ein Java-`enum` `<Name>Flags` mit einer Konstante je Flag nach `@position`, ein Member ist `java.util.BitSet`, und eine Überschreitung von `@bit_bound` wirft `IndexOutOfBoundsException`.

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact auf Java-Mapping

**Spec:** §7.15, S. 29 — "No impact to the Java language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> Java @interface + <Name>Group

**Spec:** §7.16.1, S. 29-30 — "An IDL annotation type [...] shall be
represented by Java annotation types <AnnotationName> and
<AnnotationName>Group."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::annotation_type`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_16_a_user_annotation_is_a_java_annotation_type_and_its_group`

**Status:** `done` — eine Annotation ist ein Java-Annotationstyp mit ihren Membern und Defaults und `<Name>Group`, Retention RUNTIME.

### 7.16.2 Single-Instance/Multi-Instance-Anwendung

**Spec:** §7.16.2, S. 30 — "the corresponding Java element shall be
annotated with the mapped Java annotation [...] For multiple
instances, the Group-suffix annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::user_annotations`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_16_a_user_annotation_is_a_java_annotation_type_and_its_group`

**Status:** `done` — ein Element mit einer Anwendung trägt die Annotation, eines mit mehreren die Group; per Reflexion zur Laufzeit gelesen.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6

**Spec:** §7.17.1 Tab.7.6, S. 32 — Impact pro General-Purpose-
Annotation (`@id`, `@autoid`, `@optional`, `@position`, `@value`,
`@extensibility`/`@final`/`@mutable`/`@appendable`).

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::enumeration`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@optional` boxt einen Basistyp, `@value` und `@position` setzen Enum- und Flag-Werte; `@id`, `@autoid` und die Extensibility-Annotationen lassen die Klasse unverändert.

### 7.17.2 Data Modeling Tab.7.7

**Spec:** §7.17.2 Tab.7.7, S. 32 — Impact von `@key`,
`@must_understand`, `@default_literal`.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@default_literal` ist der Wert des Default-Konstruktors; `@key` und `@must_understand` lassen die Klasse unverändert.

### 7.17.3 Units and Ranges Tab.7.8

**Spec:** §7.17.3 Tab.7.8, S. 32-33 — `@default` im Default-
Constructor; `@range`/`@min`/`@max` lösen im Setter
`IllegalArgumentException` aus; `@unit` ohne Wirkung.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::range_checks`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@default` ist der Wert des Default-Konstruktors, `@range`/`@min`/`@max` prüft der Modifier mit `IllegalArgumentException`, `@unit` lässt die Klasse unverändert.

### 7.17.4 Data Implementation Tab.7.9

**Spec:** §7.17.4 Tab.7.9, S. 33 — `@bit_bound` (Bitmask),
`@external` (boxed), `@nested` (kein Impact).

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@bit_bound` begrenzt die Bitmask, `@external` boxt einen Basistyp, `@nested` lässt die Klasse unverändert.

### 7.17.5 Code Generation Tab.7.10: @verbatim

**Spec:** §7.17.5 Tab.7.10, S. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_5_verbatim_text_is_copied_for_java_and_star_only`

**Status:** `done` — Text mit `language` `java` oder `*` steht an der angegebenen Stelle, Text für eine andere Sprache nicht; der Compiler bestätigt die eingefügten Deklarationen.

### 7.17.6 Interfaces Tab.7.11

**Spec:** §7.17.6 Tab.7.11, S. 33 — `@service`, `@oneway`, `@ami`;
der Impact ist middleware-spezifisch.

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::interface`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_6_the_interface_annotations_leave_the_java_unchanged`

**Status:** `done` — `@service`, `@oneway` und `@ami` haben laut Tabelle 7.11 eine middleware-spezifische Wirkung; die Abbildung erzeugt mit ihnen dasselbe Java wie ohne sie.

---

## §8 IDL to Java Language Mapping Annotations

### 8.1.0 @java_mapping-Annotation Definition

**Spec:** §8.1, S. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** `crates/ast/src/facet/mapping.rs`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_java_mapping_reaches_the_model_parameter_by_parameter`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_an_application_that_does_not_fit_the_declaration_is_an_error`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_the_settings_survive_a_snapshot`

**Status:** `done` — `@java_mapping` ist eingebaut, wird gegen die Deklaration der Klausel geprüft (`IDL0007`) und erreicht das Modell als `Facets::mappings`, auch durch einen Snapshot.

### 8.1.1 apply_naming_convention Tab.8.1

**Spec:** §8.1.1 Tab.8.1, S. 35-36 — die Tabelle listet 17
IDL-Konstrukt-Typen mit IDL_NAMING vs. JAVA_NAMING.

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — beide Spalten der Tabelle 8.1, je Konstrukt; eine Einstellung an einer Deklaration gewinnt über die ihres Moduls.

### 8.1.2 constants_container-Parameter

**Spec:** §8.1.2, S. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`

**Status:** `done` — der Parameter schaltet die alternative Abbildung ein, mit dem Klassennamen, den er nennt.

### 8.1.3 promote_integer_width-Parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, S. 37-38 — "whether IDL unsigned integers
shall be mapped to a Java primitive type of the same size (FALSE,
default) or to a bigger type (TRUE)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — FALSE (Vorgabe) gibt Tabelle 7.2, TRUE die breiteren Typen aus Tabelle 8.2 (`uint8` `short`, `uint16` `int`, `uint32` `long`, `uint64` `java.math.BigInteger`).

### 8.1.4 string_type-Parameter

**Spec:** §8.1.4, S. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — `java.lang.String` als Vorgabe; `@java_mapping(string_type = "java.lang.StringBuilder")` ändert den Typ von `string` und `wstring`.

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, S. 39+ — CORBA-spezifische Anpassungen.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::Platform`, `crates/lang-rules/src/emit/idl4_java/mod.rs::package_of`, `runtime/java/corba/org/omg/CORBA/`, `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_2_the_corba_module_is_org_omg_corba_under_the_idl_naming_scheme`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_1_an_exception_extends_user_exception_and_members_name_the_annexs_classes`

**Status:** `done` — `--java-platform corba`: Exceptions erweitern `org.omg.CORBA.UserException`, `TypeCode`, `Object` und `Any` sind die Klassen der Annex mit dem Verhalten aus CORBA 3.3 Teil 1 §8.11.1, der CORBA-Modul ist `org.omg.CORBA` unter dem IDL-Schema; `--java-corba-module` schreibt dessen Java aus `orb.idl` und dem Pseudo-IDL. Native Typen des CORBA-Moduls sind `java.lang.Object` (Entscheidung, die Spezifikation nennt keine Form).

---

## Audit-Status

66 done / 0 partial / 0 open / 0 blocked / 22 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9310 von `main` (`python3 ci/coverage_results.py --document idl4-java-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-ccm --all-targets` (Job `merge-rust-ccm`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (Job `merge-rust-idl-lower`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 4 Tests grün.
* `python3 ci/run_integration.py merge-codegen-java` (Job `merge-codegen-java`) — 35 Tests grün.

45 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
