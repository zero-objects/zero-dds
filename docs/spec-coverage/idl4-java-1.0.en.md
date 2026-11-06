# IDL4 to Java Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (OMG formal/2021-08-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** The mapping is `crates/lang-rules/src/emit/idl4_java/`,
selected with `zerodds-idlc generate --lang java --java-mapping idl4`
(`--java-naming idl|java`, `--java-platform dds|corba`,
`--java-corba-module`). Components, homes, ports, connectors and template
module instances go through their intermediate IDL
(`crates/lang-rules/src/emit/intermediate_idl.rs`, `crates/ccm`). The
runtime classes the spec names are in
`runtime/java/src/main/java/org/omg/type/` and, for the CORBA platform,
`runtime/java/corba/org/omg/CORBA/`; the pseudo IDL of the CORBA
module is `runtime/corba/CORBA_PIDL.idl`. Every test compiles the
generated Java together with the runtime with `javac --release 8
-Xlint:all -Werror` and runs a check program whose expectations are the
spec text.

The default of `--lang java` stays the DDS-Java-PSM binding with the XCDR2
codec (`crates/lang-rules/src/adapter/java.rs`,
`docs/spec-coverage/zerodds-xcdr2-java-1.0.md`); in several places it
deliberately does not follow this spec and is not the subject here.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`, `tools/idlc/src/main.rs`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_6_a_value_type_is_an_abstract_and_a_concrete_class`, `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — `zerodds-idlc generate --lang java --java-mapping idl4` maps IDL to Java by clauses 7 and 8: a package per module, a file per class, components, homes, ports, connectors and template module instances through their intermediate IDL; every test compiles the generated classes with `javac --release 8 -Xlint:all -Werror` and checks them by reflection and by running them. The default of `--lang java` stays the DDS-Java-PSM binding (a decision, see the context).

### 1.2 Use of modern Java language features

**Spec:** §1, p. 1 — "The language mapping makes use of modern Java
language features as appropriate and natural."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — scope statement without a checkable requirement.

---

## §2 Conformance

### 2.1 Implementation: IDL -> Java source per §7

**Spec:** §2, p. 1 — "A conformant implementation shall transform
IDL input into Java source code output as specified in clause 7."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_2_a_union_is_a_final_class_with_discriminator_accessors_and_modifiers`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — the `--java-mapping idl4` mapping writes Java source by clause 7; the tests compile and run it.

### 2.2 User: application code runs cross-implementation portable

**Spec:** §2, p. 1 — "Application source code that conforms to this
specification makes use of the Java data types and API's as defined
in clause 7. [...] Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement; no implementation obligation.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, p. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference, not a requirement.

### 3.2 [IDL4] IDL 4.2 (2018)

**Spec:** §3, p. 1 — "[IDL4] OMG IDL 4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference, not a requirement; the
frontend stands in `crates/idl-parse/` and `crates/idl-lower/`.

### 3.3 [J2SE 8.0] Java SE 8 Edition (2015)

**Spec:** §3, p. 1 — "[J2SE 8.0] Java SE 8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference, not a requirement.

### 3.4 [JavaBeans] JavaBeans 1.01

**Spec:** §3, p. 1 — "[JavaBeans] JavaBeans 1.01."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference, not a requirement; the
naming conventions are audited in §7.1.1.2.

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, p. 1 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition.

### 4.2 Camel Case (Lower Camel Case)

**Spec:** §4, p. 2 — "Camel Case [...] the variation of Camel Case
commonly known as Lower Camel Case, where the first letter is not
capitalized."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; its application is
audited in §7.1.1.2.2.

### 4.3 Java (general-purpose programming language)

**Spec:** §4, p. 2 — "Java is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition.

### 4.4 Language Mapping

**Spec:** §4, p. 2 — "An association of elements in one language to
elements in another language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, p. 2 — "Pascal Case, also known as Upper Camel Case,
[...] 'these are my words' would be 'TheseAreMyWords'."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; its application is
audited in §7.1.1.2.1.

---

## §5 Symbols (Tab.5.1)

### 5.1 Acronyms: CCM, CORBA, DDS, J2SE, IDL, OMG

**Spec:** §5 Tab.5.1, p. 2 — acronym list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §6 Additional Information

### 6.1 Alternative to the existing OMG IDL-Java mapping spec

**Spec:** §6.1, p. 3 — "This specification is an alternative to the
existing OMG IDL to Java Mapping specification; it is distinct in
that it provides a mapping for the constructs of IDL4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

### 6.2 Acknowledgments (ADLINK, RTI, Twin Oaks)

**Spec:** §6.2, p. 3 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.1 General

### 7.1.1.0 Naming schemes: IDL vs. Java; selected via @java_mapping

**Spec:** §7.1.1, p. 5 — "This specification defines two naming
schemes [...] The @java_mapping annotation defined in Clause 8.1
provides a mechanism to select the appropriate naming scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_java_mapping_reaches_the_model_parameter_by_parameter`, `tools/idlc/src/main.rs::idl4java_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — the two schemes are `Naming::Idl` and `Naming::Java`; `--java-naming` selects the run's scheme and `@java_mapping(apply_naming_convention = ...)` overrides it per module and declaration.

### 7.1.1.0 Name collision with reserved names: underscore prefix

**Spec:** §7.1.1, p. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending an underscore ('_') to the mapped name."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::escape`, `crates/lang-rules/src/emit/idl4_java/names.rs::escape_type`, `crates/lang-rules/src/emit/idl4_java/names.rs::escape_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — a mapped name that is a Java keyword or literal, `Constants` or a method of `java.lang.Object` takes a leading `_` (`_class`, `_Constants`, `_hashCode`), under both schemes.

### 7.1.1.1 IDL Naming Scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::type_name`, `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::variable`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — under `Naming::Idl` type, member, constant and module names stay unchanged; the tests check the IDL spelling in classes, accessors and packages.

### 7.1.1.2 Java Naming Scheme: JavaBeans 1.01 conventions

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — under `Naming::Java` types follow Pascal case, members and methods camel case, accessors `get<Name>`/`set<Name>`, constants and enumerators upper case, packages lower case (Table 8.1).

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, p. 5-6 — "The first letter after each
underscore shall be capitalized and all underscores shall be removed.
The first letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case_is_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`

**Status:** `done` — all 14 examples of the clause give the spelling it names.

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, p. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case_is_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — all 15 examples of the clause give the spelling it names. Where a later example of the specification contradicts its rule (`longLongVariable` in §7.2.4.3.1), the rule holds (`crates/lang-rules/src/emit/idl4_java/names.rs`, module doc).

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, p. 6 — "Every letter shall be capitalized and
all underscores shall be removed."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::all_uppercase`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::upper_and_lower_case_are_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — the clause's examples; constants in the container and enumerators under the Java scheme are upper case.

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, p. 6-7 — "Every letter shall be in lower case
and all underscores shall be removed."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::all_lowercase`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::upper_and_lower_case_are_the_clause_examples`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — the clause's examples; the module `MY_MATH` is the package `my_math` under the Java scheme.

### 7.1.1.3 Suffixes (e.g. Abstract); underscore prefix on collision

**Spec:** §7.1.1.3, p. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::escape_type`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — a structure `FooAbstract` is `_FooAbstract`; `Abstract` alone stays, as it is the suffix rather than ending in it.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, p. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords; literals (true/false/null); java.lang.Object
methods (clone/equals/getClass/hashCode/notify/notifyAll/finalize/
toString/wait)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::JAVA_KEYWORDS`, `crates/lang-rules/src/emit/idl4_java/names.rs::JAVA_LITERALS`, `crates/lang-rules/src/emit/idl4_java/names.rs::OBJECT_METHODS`, `crates/lang-rules/src/emit/idl4_java/names.rs::CONSTANTS_CLASS`, `crates/lang-rules/src/emit/idl4_java/names.rs::ABSTRACT_SUFFIX`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_1_2_reserved_names_take_a_leading_underscore`, `crates/lang-rules/src/emit/idl4_java/names.rs::reserved_names_take_a_leading_underscore`

**Status:** `done` — reserved are the clause's 50 keywords, `true`/`false`/`null`, `<type>Abstract`, `Constants` and the nine methods of `java.lang.Object`.

### 7.1.3 Holder<E> class for inout/out parameters

**Spec:** §7.1.3, p. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. [...] `package org.omg.type; public class Holder<E>
{ public E value; }`."

**Repo:** `runtime/java/src/main/java/org/omg/type/Holder.java`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::operation_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — `org.omg.type.Holder<E>` with the public field `value`; `out` and `inout` parameters are `Holder<Boxed>`, and the test calls an implementation that answers through the holders.

### 7.1.4 Tab.7.1: Java language versions per feature

**Spec:** §7.1.4, p. 8 — Tab.7.1 lists J2SE 5.0 for enumerations,
generics, annotation type-declaration; Java SE 8.0 for annotation
type-use.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Tab.7.1 describes the spec's language
versions; it is not a mapping requirement of its own.

### 7.1.5 Code Examples Notation `{...}` (informative)

**Spec:** §7.1.5, p. 8 — "the notation {...} is used in describing
Java code [...] generated code is specific to a particular vendor's
implementation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (no direct mapping)

**Spec:** §7.2.1, p. 9 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — meta statement on the spec structure.

### 7.2.2 IDL module -> Java package

**Spec:** §7.2.2, p. 9 — "An IDL module is mapped to a Java package
with the same name. [...] IDL declarations not enclosed in any
modules are mapped to classes or interfaces in the (unnamed) Java
global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::package_of`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Gen::file`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — a module is a package of the same name, one file per public class in the package's directory; declarations outside any module are in the unnamed package.

### 7.2.3 Default mapping: IDL constant -> public final class with a value field

**Spec:** §7.2.3, p. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value`

**Status:** `done` — a constant is a `public final class` of its name with `public static final <T> value`, the clause's example under both schemes.

### 7.2.3.1 Alternative mapping: Constants container class

**Spec:** §7.2.3.1, p. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`

**Status:** `done` — `@java_mapping(constants_container = ...)` collects a scope's constants in one class, `Constants` by default, or under the name given.

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, p. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — Table 7.2 in full: `int8`/`uint8`/`octet` `byte`, `short`/`unsigned short` `short`, `long`/`unsigned long` `int`, `long long`/`unsigned long long` `long`.

### 7.2.4.1.1 Unsigned types per Tab.7.2

**Spec:** §7.2.4.1.1 Tab.7.2, p. 11 — "uint8 -> byte; unsigned short/uint16 -> short; unsigned long/uint32 -> int; unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — the unsigned types take the Java type of the same width (Table 7.2, `promote_integer_width` FALSE by default); their bit pattern is kept.

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3)

**Spec:** §7.2.4.1.2 Tab.7.3, p. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `float` `float`, `double` `double`, `long double` `java.math.BigDecimal`.

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, p. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `char` is `char`.

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, p. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `wchar` is `char`.

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, p. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::primitive`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::java_value`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `boolean` is `boolean`; the constants `TRUE` and `FALSE` become `true` and `false`.

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, p. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `octet` is `byte`.

### 7.2.4.2.1.1 Sequences of basic types -> type-specific interfaces

**Spec:** §7.2.4.2.1.1 Tab.7.4, p. 12 — type-specific sequence interfaces
in `org.omg.type` with `extends java.util.List<Boxed>`.

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::specific_sequence`, `runtime/java/src/main/java/org/omg/type/`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — a sequence of a basic type is the Table 7.4 class in `org.omg.type` (`BooleanSeq` to `BigDecimalSeq`), a `java.util.List` of the boxed type with the clause's methods. The clause calls them interfaces with constructors; an interface has no constructor, so they are classes, and `prim get(int)` beside `Boxed get(int)` cannot be declared in Java (JLS 8.4.8.3) — both are documented in the classes.

### 7.2.4.2.1.1 Bounds checking on bounded sequences

**Spec:** §7.2.4.2.1.1, p. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — the modifier of a bounded sequence throws `java.lang.IndexOutOfBoundsException` for too many elements.

### 7.2.4.2.1.2 Sequences of non-basic types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, p. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked`

**Status:** `done` — a sequence of any other type is `java.util.List<E>` of the boxed element type.

### 7.2.4.2.2 IDL string -> java.lang.String + IndexOutOfBoundsException

**Spec:** §7.2.4.2.2, p. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `string` is `java.lang.String`; the modifier throws `IndexOutOfBoundsException` for an exceeded bound and for a character outside `char`.

### 7.2.4.2.3 IDL wstring -> java.lang.String

**Spec:** §7.2.4.2.3, p. 14 — analogous to §7.2.4.2.2.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `wstring` is `java.lang.String`; the bound is checked in the modifier.

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException


**Spec:** §7.2.4.2.4, p. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers`

**Status:** `done` — `fixed<P,S>` is `java.math.BigDecimal`; a value with more digits than `P` or `S` admit throws `java.lang.ArithmeticException`.

### 7.2.4.3.1 IDL struct -> Java public class + Serializable + bean accessors + constructors

**Spec:** §7.2.4.3.1, p. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor per member, public
modifier per member, public all-values constructor, public default
constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_the_default_constructor_initialises_every_member`

**Status:** `done` — a public class, `java.io.Serializable`, private fields, an accessor and a modifier per member, the default and the all values constructor, the clause's example under both schemes.

### 7.2.4.3.1 Default constructor: initialise members

**Spec:** §7.2.4.3.1, p. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string; Arrays - declared size with element defaults;
Sequences - zero-length; Others - default constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::initial`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_the_default_constructor_initialises_every_member`

**Status:** `done` — primitives keep Java's default, strings are empty, arrays have their size with default-constructed elements, sequences are empty, everything else comes from its default constructor.

### 7.2.4.3.1 Accessor/modifier naming per naming scheme

**Spec:** §7.2.4.3.1, p. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::getter`, `crates/lang-rules/src/emit/idl4_java/names.rs::Naming::setter`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`

**Status:** `done` — `get_<name>`/`set_<name>` under the IDL scheme, `get<Name>`/`set<Name>` under the Java scheme.

### 7.2.4.3.2 IDL union -> Java public final class + discriminator accessor

**Spec:** §7.2.4.3.2, p. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public accessor for
discriminator [...] public accessor/modifier per member [...]"

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_2_a_union_is_a_final_class_with_discriminator_accessors_and_modifiers`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — the examples `U1` and `U2`: `public final class`, the discriminator accessor, an accessor and a modifier per member, a modifier with the discriminator for several labels and for the default, `__default()`, `IllegalStateException` and `IllegalArgumentException` as the clause names them.

### 7.2.4.3.3 IDL enum -> Java public enum + valueOf(int)


**Spec:** §7.2.4.3.3, p. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name [...] private member to hold the
value [...] static helper method valueOf(int)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`

**Status:** `done` — a Java `enum` with `@value`, `getValue()` and `valueOf(int)`, which throws a RuntimeException for an unknown value.

### 7.2.4.3.4 Constructed recursive types

**Spec:** §7.2.4.3.4, p. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_4_a_recursive_type_names_its_own_class`

**Status:** `done` — a structure and a union holding a sequence of themselves are classes whose members name their own class; the test builds nested values.

### 7.2.4.4 IDL array -> Java array + IndexOutOfBoundsException

**Spec:** §7.2.4.4, p. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_4_an_array_is_a_java_array_and_its_bounds_are_checked`

**Status:** `done` — a Java array of the element type, the example `S2`; the modifier throws `IndexOutOfBoundsException` for a wrong length.

### 7.2.4.5 IDL native -> no defined mapping

**Spec:** §7.2.4.5, p. 20 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — meta statement on the spec structure.

### 7.2.4.6 IDL typedef -> no Java type; use replaced; annotations propagate

**Spec:** §7.2.4.6, p. 20 — "Java does not have a typedef construct
[...] The use of an IDL typedef type shall be replaced with the
type referenced by the typedef type. This rule shall apply
recursively. Annotations on an IDL typedef shall be applied to
uses of the typedef in other type declarations."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::type_declaration`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`, `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_15_4_2_an_annotation_prefixing_a_typedefs_type_annotates_the_typedef`

**Status:** `done` — a `typedef` produces no class, every use carries the target type, and `@max(100)` on the `typedef` is checked by the modifier of every use, as a sequence element too.

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, p. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `runtime/java/src/main/java/org/omg/type/Any.java`, `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_3_any_is_org_omg_type_any`

**Status:** `done` — `any` is `org.omg.type.Any` with insert, extract and the type of the value; on the CORBA platform `org.omg.CORBA.Any` (A.1.4).

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface

**Spec:** §7.4, p. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. [...] Each operation
defined in the IDL interface shall map to a method in the Java
interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::interface`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::attribute_methods`, `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::operation_method`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_6_the_interface_annotations_leave_the_java_unchanged`

**Status:** `done` — every interface is a `public interface`, base interfaces become `extends`, attributes an accessor and a modifier (no modifier for `readonly`), operations methods with their parameters in IDL order, `raises` a `throws`.

### 7.4.1 IDL exception -> Java class extends RuntimeException

**Spec:** §7.4.1, p. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members [...] following the
rules of the IDL struct mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_1_an_exception_extends_user_exception_and_members_name_the_annexs_classes`

**Status:** `done` — an exception is a class extending `java.lang.RuntimeException` with the members, accessors and constructors of a structure; on the CORBA platform it extends `org.omg.CORBA.UserException` (A.1.1).

### 7.4.2 Interface forward declaration: no Java mapping

**Spec:** §7.4.2, p. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::top_level_interface`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — a forward declaration produces no file; the definition has its one.

---

## §7.5 Interfaces – Full

### 7.5 Embedded type/const/exception decls in the interface body

**Spec:** §7.5, p. 23 — "The embedded elements shall be mapped to a
public declaration within the scope of the Java interface."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::embedded_declarations`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_5_declarations_embedded_in_an_interface_are_nested_in_it`

**Status:** `done` — types, constants and exceptions in an interface body are public declarations within the Java interface, the example `FullInterface`.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java classes (Abstract + concrete)

**Spec:** §7.6, p. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name [...] private members are
protected [...] Each valuetype initializer is mapped to a method
returning void."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::valuetype`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_6_a_value_type_is_an_abstract_and_a_concrete_class`

**Status:** `done` — the examples `VT1` and `VT2`: the abstract class `<Name>Abstract` with public state, private state `protected`, abstract operations and initializers (`void`), the class `<Name>`, the base and the supported interface.

---

## §7.7 CORBA-Specific – Interfaces

### 7.7 CORBA-Specific -> Annex A.1

**Spec:** §7.7, p. 25 — "CORBA-specific mappings are defined in
clause A.1 of Annex A."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.8 CORBA-Specific – Value Types

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, p. 25 — analogous to §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.9 Components – Basic

### 7.9 Components Basic -> intermediate IDL

**Spec:** §7.9, p. 25 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL."

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::transform_component`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`

**Status:** `done` — a component becomes its equivalent interface by CCM 4.0 §6 and that by §7.4: `extends Components.CCMObject`, `provide_`, `connect_`/`disconnect_`/`get_connection(s)_`, `subscribe_`/`unsubscribe_`, `get_consumer_`, attributes; the event type becomes a value type and `<E>Consumer`. The `Components` declarations come from the IDL the application includes (test: a transcription of CCM 4.0 §6).

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, p. 25 — analogous to §7.9.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::transform_home`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces`

**Status:** `done` — a home becomes `<Home>Explicit` (extends `Components.CCMHome`, factory operations raising `CreateFailure`), `<Home>Implicit` (extends `Components.KeylessCCMHome`, `create()`) and `<Home>` extending both.

---

## §7.11 CCM-Specific

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, p. 25 — analogous to §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.12 Components – Ports and Connectors

### 7.12 Ports/Connectors -> intermediate IDL

**Spec:** §7.12, p. 25 — analogous to §7.9.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::intermediate_idl`, `crates/ccm/src/transform.rs::expand_extended_ports`, `crates/ccm/src/transform.rs::transform_connector`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_9_and_7_12_a_component_is_its_equivalent_interface_in_java`, `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_12_a_connector_maps_through_its_ports`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type`

**Status:** `done` — an extended port becomes the basic ports of its port type (`<port>_<basic port>`), a `mirrorport` with facets and receptacles swapped; a connector maps through its ports as a component does, its attributes as an interface's.

---

## §7.13 Template Modules

### 7.13 Template-module instances -> intermediate IDL

**Spec:** §7.13, p. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** `crates/idl-lower/src/lower.rs::template_module_inst`, `crates/lang-rules/src/emit/idl4_java/mod.rs::generate`

**Tests:** `crates/lang-rules/tests/java_idl4_components.rs::idl4java_7_13_a_template_module_instance_is_a_package_of_its_own`

**Status:** `done` — an instance `module Buffer<long, 4> LongBuffer;` is the package `LongBuffer` with the template's declarations, `T` replaced by `long` and `Capacity` by `4`; the template itself produces nothing.

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> Java extends

**Spec:** §7.14.1, p. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class [...] all values constructor [...] first parameter a
non-null instance of the base struct's Java class."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_1_a_derived_structure_extends_its_base_class`

**Status:** `done` — the derived class extends the base's, and its all values constructor takes an instance of the base class as its first parameter, the example `S5 : S1`.

### 7.14.2 Union discriminator: int8/uint8/wchar/octet additionally allowed

**Spec:** §7.14.2, p. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — `int8`, `uint8`, `wchar` and `octet` discriminate a union as §7.2.4.3.2 maps it.

### 7.14.3.1 IDL map -> java.util.Map<K,V>


**Spec:** §7.14.3.1, p. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::map_boxed`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::checks`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_1_a_map_is_a_java_util_map_of_table_7_5_types`

**Status:** `done` — a map is `java.util.Map` of the boxed types of Table 7.5, a bounded one is checked, the example `S4`.

### 7.14.3.2 IDL bitset -> Java public class with bitfield accessors

**Spec:** §7.14.3.2, p. 28 — "An IDL bitset shall map to Java as a
public class with the same name. [...] Default member type: boolean
if size is 1, octet 2-8, unsigned short 9-16, unsigned long 17-32,
unsigned long long 33-64."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitset`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_2_a_bit_set_is_a_class_with_bit_field_accessors`

**Status:** `done` — a public class with an accessor and a modifier per named bit field, without a type the clause's smallest (`boolean`, `byte`, `short`, `int`, `long`), the example `MyBitset`.

### 7.14.3.3 IDL bitmask -> Java enum + java.util.BitSet

**Spec:** §7.14.3.3, p. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be the
IDL bitmask name with the Flags suffix appended. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`

**Status:** `done` — a Java `enum` `<Name>Flags` with a constant per flag after `@position`, a member is a `java.util.BitSet`, and exceeding `@bit_bound` throws `IndexOutOfBoundsException`.

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: no impact on Java mapping

**Spec:** §7.15, p. 29 — "No impact to the Java language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> Java @interface + <Name>Group

**Spec:** §7.16.1, p. 29-30 — "An IDL annotation type [...] shall be
represented by Java annotation types <AnnotationName> and
<AnnotationName>Group."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::annotation_type`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_16_a_user_annotation_is_a_java_annotation_type_and_its_group`

**Status:** `done` — an annotation is a Java annotation type with its members and defaults and `<Name>Group`, retention RUNTIME.

### 7.16.2 Single-instance/multi-instance application

**Spec:** §7.16.2, p. 30 — "the corresponding Java element shall be
annotated with the mapped Java annotation [...] For multiple
instances, the Group-suffix annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::user_annotations`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_16_a_user_annotation_is_a_java_annotation_type_and_its_group`

**Status:** `done` — an element with one application carries the annotation, one with several the group; read by reflection at run time.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6

**Spec:** §7.17.1 Tab.7.6, p. 32 — impact per general-purpose
annotation (`@id`, `@autoid`, `@optional`, `@position`, `@value`,
`@extensibility`/`@final`/`@mutable`/`@appendable`).

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::enumeration`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@optional` boxes a basic type, `@value` and `@position` set enum and flag values; `@id`, `@autoid` and the extensibility annotations leave the class unchanged.

### 7.17.2 Data Modeling Tab.7.7

**Spec:** §7.17.2 Tab.7.7, p. 32 — impact of `@key`,
`@must_understand`, `@default_literal`.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@default_literal` is the default constructor's value; `@key` and `@must_understand` leave the class unchanged.

### 7.17.3 Units and Ranges Tab.7.8

**Spec:** §7.17.3 Tab.7.8, p. 32-33 — `@default` in the default
constructor; `@range`/`@min`/`@max` raise `IllegalArgumentException` in
the setter; `@unit` has no effect.

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::range_checks`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@default` is the default constructor's value, `@range`/`@min`/`@max` are checked by the modifier with `IllegalArgumentException`, `@unit` leaves the class unchanged.

### 7.17.4 Data Implementation Tab.7.9

**Spec:** §7.17.4 Tab.7.9, p. 33 — `@bit_bound` (bitmask),
`@external` (boxed), `@nested` (no impact).

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — `@bit_bound` bounds the bit mask, `@external` boxes a basic type, `@nested` leaves the class unchanged.

### 7.17.5 Code Generation Tab.7.10: @verbatim

**Spec:** §7.17.5 Tab.7.10, p. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::Gen::structure`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_5_verbatim_text_is_copied_for_java_and_star_only`

**Status:** `done` — text with `language` `java` or `*` is at the place given, text for another language is not; the compiler confirms the inserted declarations.

### 7.17.6 Interfaces Tab.7.11

**Spec:** §7.17.6 Tab.7.11, p. 33 — `@service`, `@oneway`, `@ami`;
the impact is middleware-specific.

**Repo:** `crates/lang-rules/src/emit/idl4_java/behaviour.rs::Gen::interface`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_17_6_the_interface_annotations_leave_the_java_unchanged`

**Status:** `done` — `@service`, `@oneway` and `@ami` have a middleware-specific impact by Table 7.11; the mapping writes the same Java with them as without them.

---

## §8 IDL to Java Language Mapping Annotations

### 8.1.0 @java_mapping annotation definition

**Spec:** §8.1, p. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** `crates/ast/src/facet/mapping.rs`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_java_mapping_reaches_the_model_parameter_by_parameter`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_an_application_that_does_not_fit_the_declaration_is_an_error`, `crates/idl-lower/tests/mapping_annotations.rs::idl4java_8_1_the_settings_survive_a_snapshot`

**Status:** `done` — `@java_mapping` is built in, checked against the clause's declaration (`IDL0007`), and reaches the model as `Facets::mappings`, through a snapshot too.

### 8.1.1 apply_naming_convention Tab.8.1

**Spec:** §8.1.1 Tab.8.1, p. 35-36 — the table lists 17
IDL-construct types with IDL_NAMING vs. JAVA_NAMING.

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws`

**Status:** `done` — both columns of Table 8.1, construct by construct; a setting on a declaration wins over its module's.

### 8.1.2 constants_container parameter

**Spec:** §8.1.2, p. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1."

**Repo:** `crates/lang-rules/src/emit/idl4_java/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope`

**Status:** `done` — the parameter switches the alternative mapping on, with the class name it gives.

### 8.1.3 promote_integer_width parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, p. 37-38 — "whether IDL unsigned integers
shall be mapped to a Java primitive type of the same size (FALSE,
default) or to a bigger type (TRUE)."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::integer`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`, `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — FALSE (the default) gives Table 7.2, TRUE the wider types of Table 8.2 (`uint8` `short`, `uint16` `int`, `uint32` `long`, `uint64` `java.math.BigInteger`).

### 8.1.4 string_type parameter

**Spec:** §8.1.4, p. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** `crates/lang-rules/src/emit/idl4_java/types.rs::Gen::java_type_of`, `crates/lang-rules/src/emit/idl4_java/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/java_idl4_mapping.rs::idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type`

**Status:** `done` — `java.lang.String` by default; `@java_mapping(string_type = "java.lang.StringBuilder")` changes the type of `string` and `wstring`.

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, p. 39+ — CORBA-specific adaptations.

**Repo:** `crates/lang-rules/src/emit/idl4_java/mod.rs::Platform`, `crates/lang-rules/src/emit/idl4_java/mod.rs::package_of`, `runtime/java/corba/org/omg/CORBA/`, `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_2_the_corba_module_is_org_omg_corba_under_the_idl_naming_scheme`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `crates/lang-rules/tests/java_idl4_corba.rs::idl4java_a_1_1_an_exception_extends_user_exception_and_members_name_the_annexs_classes`

**Status:** `done` — `--java-platform corba`: exceptions extend `org.omg.CORBA.UserException`, `TypeCode`, `Object` and `Any` are the annex's classes with the behaviour of CORBA 3.3 part 1 §8.11.1, the CORBA module is `org.omg.CORBA` under the IDL scheme; `--java-corba-module` writes its Java from `orb.idl` and the pseudo IDL. Native types of the CORBA module are `java.lang.Object` (a decision; the specification names no form).

---

## Audit status

66 done / 0 partial / 0 open / 0 blocked / 22 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9310 of `main` (`python3 ci/coverage_results.py --document idl4-java-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-ccm --all-targets` (job `merge-rust-ccm`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (job `merge-rust-idl-lower`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 4 tests green.
* `python3 ci/run_integration.py merge-codegen-java` (job `merge-codegen-java`) — 35 tests green.

45 cited tests green, 0 not green.

Open items: none.
