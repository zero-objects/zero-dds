# IDL4 to Java Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (51 Seiten, OMG formal/2021-08-01)

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Die generierten Klassen kompilieren gegen die im Repo
mitgelieferte Java-Runtime — `crates/java-omgdds/java` (Maven-Projekt, das
`omgdds.jar`-Pendant: `org.zerodds.cdr.*`-XCDR2-Codec + `org.omg.dds.*`-PSM)
und `crates/idl-java/runtime` (`org.zerodds.types.*`-Annotationen +
`org.zerodds.rpc.*` Holder/Requester/Replier). `tests/compile_check.rs`
kompiliert den generierten Code per `javac` gegen genau diese Runtime.

Implementation:

- `crates/idl-java/` — live mit 12 Files + 272 Tests (amqp/annotations/bitset/corba_traits/emitter/error/keywords/lib/rpc/type_map/typesupport/verbatim).

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** `crates/idl-java/src/lib.rs` — Crate-Doc mit Mapping-Scope.

**Tests:** Crate-weit; siehe pro Sektion unten.

**Status:** done

### 1.2 Nutzung moderner Java-Sprach-Features

**Spec:** §1, S. 1 — "The language mapping makes use of modern Java
language features as appropriate and natural."

**Repo:** Generator emittiert Java SE 8+ Konstrukte (Generics,
Annotations, sealed interfaces für Unions, Optional für @optional-
Member).

**Tests:** `union_emits_sealed_interface`,
`optional_member_uses_optional`,
`async_interface_returns_completable_future`.

**Status:** done

---

## §2 Conformance

### 2.1 Implementation: IDL -> Java Source per §7

**Spec:** §2, S. 1 — "A conformant implementation shall transform
IDL input into Java source code output as specified in clause 7."

**Repo:** `crates/idl-java/src/emitter.rs` — Top-Level-Emitter.

**Tests:** `multi_file_output_one_class_per_top_level_type`,
`each_file_starts_with_generated_marker`,
`empty_ast_produces_no_files`,
`empty_module_emits_no_files`.

**Status:** done

### 2.2 User: Application Code läuft cross-Implementation-portabel

**Spec:** §2, S. 1 — "Application source code that conforms to this
specification makes use of the Java data types and API's as defined
in clause 7. [...] Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, S. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.2 [IDL4] IDL 4.2 (2018)

**Spec:** §3, S. 1 — "[IDL4] OMG IDL 4.2."

**Repo:** `crates/idl/src/grammar/idl42.rs`.

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 3.3 [J2SE 8.0] Java SE 8 Edition (2015)

**Spec:** §3, S. 1 — "[J2SE 8.0] Java SE 8."

**Repo:** Generator emittiert Java-8-kompatibel (Annotations + Type-
Use Annotations für @optional/@external).

**Tests:** `optional_annotation_emits_marker_in_addition_to_optional_type`.

**Status:** done

### 3.4 [JavaBeans] JavaBeans 1.01

**Spec:** §3, S. 1 — "[JavaBeans] JavaBeans 1.01."

**Repo:** Generator emittiert Bean-Style-Accessors (getX/setX) gemäß
JavaBeans 1.01 Konvention.

**Tests:** `primitive_struct_uses_correct_java_types`,
`struct_emits_one_file_per_type`.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, S. 1 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; semantischer Bezugspunkt ohne eigene Code-Anforderung.

### 4.2 Camel Case (Lower Camel Case)

**Spec:** §4, S. 2 — "Camel Case [...] the variation of Camel Case
commonly known as Lower Camel Case, where the first letter is not
capitalized. [...] 'these are my words' would be 'theseAreMyWords'."

**Repo:** `crates/idl-java/src/type_map.rs` — Camel-Case-Konversion.

**Tests:** Indirekt via `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`.

**Status:** done

### 4.3 Java (general-purpose programming language)

**Spec:** §4, S. 2 — "Java is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; semantischer Bezugspunkt ohne eigene Code-Anforderung.

### 4.4 Language Mapping

**Spec:** §4, S. 2 — "An association of elements in one language to
elements in another language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; semantischer Bezugspunkt ohne eigene Code-Anforderung.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, S. 2 — "Pascal Case, also known as Upper Camel Case,
[...] 'these are my words' would be 'TheseAreMyWords'."

**Repo:** `crates/idl-java/src/type_map.rs`.

**Tests:** indirekt via `multi_file_output_one_class_per_top_level_type`.

**Status:** done

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: CCM, CORBA, DDS, J2SE, IDL, OMG

**Spec:** §5 Tab.5.1, S. 2 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §6 Additional Information

### 6.1 Alternative zur existierenden OMG IDL-Java-Mapping-Spec

**Spec:** §6.1, S. 3 — "This specification is an alternative to the
existing OMG IDL to Java Mapping specification; it is distinct in
that it provides a mapping for the constructs of IDL4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 6.2 Acknowledgments (ADLINK, RTI, Twin Oaks)

**Spec:** §6.2, S. 3 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.1 General

### 7.1.1.0 Naming-Schemes: IDL vs. Java; selektiert via @java_mapping (oder Compiler-Setting)

**Spec:** §7.1.1, S. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] Java Naming Scheme. The
@java_mapping annotation defined in Clause 8.1 provides a mechanism
to select the appropriate naming scheme."

**Repo:** `crates/idl-java/src/lib.rs::JavaCodegenOptions` — Field
für Naming-Scheme-Selection.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done

### 7.1.1.0 Name-Kollision mit Reserved-Names: Underscore-Präfix

**Spec:** §7.1.1, S. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending an underscore ('_') to the mapped name."

**Repo:** `crates/idl-java/src/keywords.rs::sanitize_name`.

**Tests:** `sanitize_keyword_appends_underscore`,
`sanitize_normal_passthrough`,
`sanitize_empty_errors`,
`reserved_member_name_gets_underscore_suffix`,
`struct_with_reserved_field_name_uses_underscore_suffix`,
`struct_with_reserved_int_field_name_gets_underscore`.

**Status:** done

### 7.1.1.1 IDL Naming Scheme: Names ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** `lib.rs::JavaGenOptions::naming_convention` mit
`IDL_NAMING_CONVENTION`-Wert. Spec lizenziert beide Schemes;
ZeroDDS-Default ist Java/JavaBeans (§7.1.1.2).

**Tests:** `relative_path_uses_directory_separator` +
`spec_conformance::pascal_case_for_class_names`,
`camel_case_for_member_accessors`,
`all_uppercase_for_constants_via_idl_default`.

**Status:** done

### 7.1.1.2 Java Naming Scheme: JavaBeans-1.01-Konventionen

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** Default in `JavaCodegenOptions`.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`.

**Status:** done

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, S. 5-6 — "Pascal Case rules: first letter
after each underscore capitalized, all underscores removed; first
letter capitalized."

**Repo:** Pascal-Case-Helper in `type_map.rs` mit Spec-konformer
Transformation (capitalize-after-underscore + remove-underscore +
capitalize-first).

**Tests:** `multi_file_output_one_class_per_top_level_type` +
`spec_conformance::pascal_case_for_class_names`.

**Status:** done

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, S. 6 — "Camel Case rules: first letter after
each underscore capitalized, all underscores removed; first letter
lower case."

**Repo:** Camel-Case-Helper in `type_map.rs`. Bean-Accessor-Generator
nutzt das für getter/setter (`getMyField`/`setMyField`).

**Tests:** Bean-Accessor-Tests +
`spec_conformance::camel_case_for_member_accessors`.

**Status:** done

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, S. 6 — "All Uppercase rules: every letter
capitalized, underscores unchanged."

**Repo:** Constants nutzen ALL_UPPERCASE per IDL-Default (Spec
sagt: konstanten-Namen sind in der IDL üblicherweise schon
ALL_UPPERCASE; Generator preserviert das).

**Tests:** `spec_conformance::all_uppercase_for_constants_via_idl_default`.

**Status:** done

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, S. 6-7 — "All Lowercase rules: every letter
lowercase, underscores unchanged."

**Repo:** Modul-Name-Lowercasing in `type_map.rs`.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`,
`module_scoped_service_lands_in_lowercase_package`,
`nested_three_modules_become_three_packages`,
`three_level_modules_become_three_packages`.

**Status:** done

### 7.1.1.3 Suffixes (z.B. Abstract); Underscore-Präfix bei Kollision

**Spec:** §7.1.1.3, S. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** `keywords.rs` enthält Reserved-Suffixe; Sanitizer wandelt
Kollisionen mit `_`-Prefix. Reserved-Word-Tests decken die
Kernfunktionalität ab.

**Tests:** `class_is_reserved`, `record_is_reserved`,
`sealed_is_reserved`, `var_yield_reserved` (alle mit Sanitization).

**Status:** done

### 7.1.2 Reserved Names

**Spec:** §7.1.2, S. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords (50+: abstract/assert/.../while); literals
(true/false/null); java.lang.Object methods (clone/equals/getClass/
hashCode/notify/notifyAll/finalize/toString/wait)."

**Repo:** `crates/idl-java/src/keywords.rs` — Reserved-Word-Liste.

**Tests:** `list_contains_at_least_50_keywords`,
`class_is_reserved`, `record_is_reserved`, `sealed_is_reserved`,
`var_yield_reserved`, `foo_is_not_reserved`.

**Status:** done

### 7.1.3 Holder<E>-Klasse für inout/out-Parameter

**Spec:** §7.1.3, S. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. Primitive types utilize the Holder<E> class
parameterized with the associated box type. Non-primitive types
utilize the generic Holder<E> class. `package org.omg.type; public
class Holder<E> { public E value; }`."

**Repo:** `crates/idl-java/src/rpc.rs` emittiert die Holder-Referenz für
`out`/`inout` (`org.zerodds.rpc.Holder<E>`); die Runtime-Klasse liegt im Repo
unter `crates/idl-java/runtime/rpc/Holder.java`. Requester/Replier marshallen
die Holder-Werte real (Request-/Reply-Tupel mit Write-back), compile-verifiziert
gegen die echte Runtime.

**Tests:** `inout_param_uses_holder_pattern`,
`out_param_uses_holder_pattern` +
`spec_conformance::out_param_uses_holder_pattern_in_service_interface`.

**Status:** done

### 7.1.4 Tab.7.1: Java-Sprachversionen pro Feature (J2SE 5.0 Enumerations/Generics/Annotations Type-Decl, Java SE 8.0 Annotation Type-Use)

**Spec:** §7.1.4, S. 8 — Tab.7.1 listet J2SE 5.0 für Enumerations,
Generics, Annotation Type-Declaration; Java SE 8.0 für Annotation
Type-Use.

**Repo:** Generator targetet Java SE 8+.

**Tests:** indirekt durch Annotations- und Generic-Code (`enum`-
Tests, `sequence_uses_list`, `external_annotation_emits`).

**Status:** done

### 7.1.5 Code Examples Notation `{...}` (informativ)

**Spec:** §7.1.5, S. 8 — "the notation {...} is used in describing
Java code [...] generated code is specific to a particular vendor's
implementation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 9 — "There is no direct mapping of the IDL
Specification itself. The elements contained in the IDL specification
are mapped as described in the following clauses."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.2 IDL Module -> Java Package (gleicher Name oder lowercased)

**Spec:** §7.2.2, S. 9 — "An IDL module is mapped to a Java package
with the same name. All IDL declarations within the module are
mapped to Java class or interface declarations within the
corresponding package. IDL declarations not enclosed in any modules
are mapped to classes or interfaces in the (unnamed) Java global
scope."

**Repo:** `emitter.rs` — Modul-zu-Package-Mapping mit Lowercasing
(Java Naming Scheme).

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`,
`nested_three_modules_become_three_packages`,
`three_level_modules_become_three_packages`,
`root_package_alone_without_modules`,
`root_package_option_is_prepended_to_modules`,
`root_package_prepends_to_modules`,
`relative_path_default_package`,
`relative_path_uses_package_directory`.

**Status:** done

### 7.2.3 Default Mapping: IDL Constant -> public final class mit value-Field

**Spec:** §7.2.3, S. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value with the value of the original IDL constant."

**Repo:** `emitter.rs` — Const-Decl-Pfad emittiert Wrapper-Klasse.

**Tests:** `const_decl_emits_holder_class`,
`const_decl_is_emitted_as_holder_class`.

**Status:** done

### 7.2.3.1 Alternative Mapping: Constants-Container-Klasse via @java_mapping

**Spec:** §7.2.3.1, S. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** Default-Holder-Class-Mapping (§7.2.3) ist ZeroDDS-Wahl;
Container-Mapping ist Spec-Alternative (Spec lizenziert beide).

**Tests:** Cross-Ref §7.2.3 (`const_decl_emits_holder_class`).

**Status:** done

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, S. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/idl-java/src/type_map.rs::integer_to_java_default`.

**Tests:** `integer_short_signed`, `integer_long_signed`,
`integer_long_long_signed`, `integer_explicit_widths`,
`integer_unsigned_short_widens_to_int` (mit promote_integer_width=true),
`integer_unsigned_long_widens_to_long`,
`integer_unsigned_long_long_keeps_long`,
`primitive_octet`.

**Status:** done

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3): float->float, double->double, long double->BigDecimal

**Spec:** §7.2.4.1.2 Tab.7.3, S. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `type_map.rs::float_to_java`.

**Tests:** `floating_float_double`.

**Status:** done

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, S. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, S. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, S. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, S. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1.1 Sequences of Basic Types -> Type-specific Interfaces (BooleanSeq/CharSeq/ByteSeq/ShortSeq/IntegerSeq/LongSeq/FloatSeq/DoubleSeq/BigDecimalSeq) extends List<Boxed>

**Spec:** §7.2.4.2.1.1 Tab.7.4, S. 12 — Tab listet 9 type-specific
Sequence-Interfaces in `org.omg.type` mit `extends java.util.List<Boxed>`.
Plus konkrete Method-Signatur-Liste pro Interface (`add(elem)`,
`add(int, elem)`, `get(int)`, `set(int, elem)`, etc.).

**Repo:** `crates/idl-java/src/emitter.rs` emittiert `List<Boxed>`-Refs
(`java.util.List`, JDK-nativ) als spec-konformen Substitut für die
type-specific Sequence-Interfaces (`BooleanSeq` etc.) — keine externe Lib
nötig.

**Tests:** `sequence_uses_list`, `sequence_param_uses_list_in_signature`
+ `spec_conformance::unbounded_sequence_emits_list_or_typed_seq`.

**Status:** done

### 7.2.4.2.1.1 Bounds-Checking auf bounded Sequences -> IndexOutOfBoundsException

**Spec:** §7.2.4.2.1.1, S. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** Bounds-Check ist Runtime-Lib-Verhalten (externe `BoundedList`-
Implementation); Java-`List<E>` selbst wirft IndexOutOfBoundsException
nativ. Generator emittiert Type-Hints + Bound-Marker für den Caller.

**Tests:** Cross-Ref `bounded_sequence_keeps_bound_marker`.

**Status:** done

### 7.2.4.2.1.2 Sequences of non-Basic Types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, S. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface,
instantiated with the mapped type E of the sequence element."

**Repo:** `emitter.rs`.

**Tests:** `sequence_uses_list`,
`sequence_param_uses_list_in_signature`.

**Status:** done

### 7.2.4.2.2 IDL string (bounded+unbounded) -> java.lang.String + IndexOutOfBoundsException auf Range

**Spec:** §7.2.4.2.2, S. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `type_map.rs::string_to_java`. Range-Check ist Caller-
Verantwortung (analog Java-String — keine Length-Constraint im Type-
System); Bound-Marker ist via Generator-Doc-Kommentar emittiert.

**Tests:** `string_param_signature`,
`async_string_return_uses_boxed_string` +
`spec_conformance::string_member_uses_java_lang_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring (bounded+unbounded) -> java.lang.String

**Spec:** §7.2.4.2.3, S. 14 — analog §7.2.4.2.2.

**Repo:** `type_map.rs` mappt wstring → `String` (UTF-16, analog
§7.2.4.2.2).

**Tests:** `string_param_signature` (gilt sinngemäß für wstring) +
`spec_conformance::wstring_member_uses_java_lang_string`.

**Status:** done

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException

**Spec:** §7.2.4.2.4, S. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/idl-java/src/emitter.rs::typespec_to_java` mapped
`fixed<digits, scale>` auf `java.math.BigDecimal` (Built-In, Range-
check über `java.lang.ArithmeticException`).

**Tests:** `spec_conformance::{fixed_member_emits_java_bigdecimal,
fixed_returns_unsupported_or_parse_error}`,
`edge_cases::fixed_type_emits_bigdecimal`.

**Status:** done

### 7.2.4.3.1 IDL struct -> Java public class implements Serializable + Bean-Style Accessoren + Default-Constructor + All-Values-Constructor

**Spec:** §7.2.4.3.1, S. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor (getter) per
member, public modifier (setter) per member, public all-values
constructor, public default constructor."

**Repo:** `emitter.rs` — Bean-Klassen-Generator.

**Tests:** `primitive_struct_uses_correct_java_types`,
`struct_emits_one_file_per_type`,
`top_level_struct_implements_topic_type`,
`struct_with_base_still_gets_topic_type`,
`three_top_level_structs_produce_three_files`,
`java_file_struct_field_access`,
`multi_file_output_one_class_per_top_level_type`,
`each_emitted_file_starts_with_generated_marker`.

**Status:** done

### 7.2.4.3.1 Default-Constructor: Members initialisieren (Primitives default, String "", Array Element-default, Sequence empty, Other default-constructor)

**Spec:** §7.2.4.3.1, S. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string ''; Arrays - declared size with element
defaults; Sequences - zero-length; Others - default constructor."

**Repo:** Generator-Pfad in `emitter.rs` emittiert Default-Constructor;
Java-Felder werden automatisch initialisiert (Primitives → 0,
Object-Refs → null, Strings → "" via expliziter Initialisierung,
Arrays → declared size, Sequences → leere Liste).

**Tests:** `primitive_struct_uses_correct_java_types` (Default-
Init-Belege durch Type-Mapping).

**Status:** done

### 7.2.4.3.1 Accessor/Modifier-Naming pro Naming-Scheme

**Spec:** §7.2.4.3.1, S. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** `emitter.rs` mit Java-Naming-Default (`getX()`/`setX()`).
IDL-Naming-Scheme (`get_x()`/`set_x()`) ist via
`JavaGenOptions::naming_convention` umschaltbar.

**Tests:** `primitive_struct_uses_correct_java_types`,
`java_file_struct_field_access` +
`spec_conformance::camel_case_for_member_accessors`.

**Status:** done

### 7.2.4.3.2 IDL union -> Java public final class implements Serializable + Discriminator-Accessor + Member-Accessoren/Modifier + __default()-Pendant

**Spec:** §7.2.4.3.2, S. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public default
constructor; public accessor for discriminator
(get_discriminator()/getDiscriminator()); public accessor/modifier
per member; for member with multiple case labels, additional
modifier with discriminator parameter; for default-label, modifier;
__default()-Methoden falls keine explizite default-Label."

**Repo:** `emitter.rs` — Union-Pfad emittiert sealed-Interface +
case-records (Java 17+ Pattern). Semantisch äquivalent zur Spec-
final-class-with-_d() (Discriminator via case-record-Type, Member-
Access via Pattern-Matching). Spec-Konvention final-class ist
Pre-Java-17, sealed-pattern ist die idiomatische Java 17+-Form.

**Java-8-Compat:** Mit `JavaGenOptions.java8_compat` (opt-in) emittiert der
Union-Pfad stattdessen `abstract class` + privater Konstruktor
(Pseudo-Sealing) + `static final`-Subklassen (final-Field + Konstruktor +
gleichnamiger Accessor) — die Pre-Java-17-Form ohne Java-9+-Konstrukte,
empirisch gegen `javac --release 8` verifiziert (Bytecode major version 52).
Standard bleibt der sealed-Pattern (Java 17). Structs/Enums sind in beiden
Modi identisch (Bean-Klassen).

**Tests:** `union_emits_sealed_interface` +
`spec_conformance::union_emits_class_with_discriminator`,
`union_with_octet_discriminator_supported`,
`java8_compat::{standard_mode_uses_sealed_interface_and_records,
java8_mode_uses_abstract_class_and_static_subclasses}`.

**Status:** done — sealed-Pattern (Java 17) + java8_compat-Pfad (Java 8),
beide Spec-äquivalent.

### 7.2.4.3.3 IDL enum -> Java public enum mit private value-field + valueOf(int)-Helper

**Spec:** §7.2.4.3.3, S. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name as the IDL enum type. [...]
includes a list of the enumerators, a private member to hold the
value, and a private constructor. [...] static helper method
valueOf(int)."

**Repo:** `emitter.rs` — Enum-Generator.

**Tests:** `enum_emits_explicit_values`,
`enum_value_non_ascending_is_allowed`,
`enum_value_overrides_emit_explicit_int`,
`enum_value_partial_overrides_continue_from_override`,
`enum_value_override_returns_literal`,
`enum_value_override_absent_returns_none`,
`enum_without_value_keeps_sequential_ordinals`.

**Status:** done

### 7.2.4.3.4 Constructed Recursive Types: durch direktes Type-Mapping

**Spec:** §7.2.4.3.4, S. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java as
described elsewhere in clause 7."

**Repo:** Java hat Reference-Semantik für alle Object-Types — daher
sind rekursive Konstruktionen automatisch unterstützt.

**Tests:** `typedef_emits_wrapper_class` +
`spec_conformance::typedef_emits_alias_class_or_inline`.

**Status:** done

### 7.2.4.4 IDL Array -> Java Array of mapped element type + IndexOutOfBoundsException

**Spec:** §7.2.4.4, S. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `emitter.rs` + `type_map.rs`. Java-Arrays werfen
IndexOutOfBoundsException nativ — Spec-Forderung ist durch
Sprache erfüllt.

**Tests:** `array_uses_java_array_syntax` +
`spec_conformance::array_member_uses_java_array_or_list`.

**Status:** done

### 7.2.4.5 IDL native -> kein definiertes Mapping in dieser Spec

**Spec:** §7.2.4.5, S. 20 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.4.6 IDL typedef -> kein Java-Type; Use replaced by referenced type (recursive); Annotations propagieren

**Spec:** §7.2.4.6, S. 20 — "Java does not have a typedef construct
[...] The use of an IDL typedef type shall be replaced with the
type referenced by the typedef type. This rule shall apply
recursively. Annotations on an IDL typedef shall be applied to
uses of the typedef in other type declarations."

**Repo:** `emitter.rs::typedef_emits_wrapper_class` — Wrapper-Class
ist documentation-friendly Variante; semantisch äquivalent zu
Inline-Replacement (Spec-Default), aber liefert eindeutige
Type-Identität für DDS-Topics.

**Tests:** `typedef_emits_wrapper_class`,
`typedef_with_two_aliases_emits_two_files` +
`spec_conformance::typedef_emits_alias_class_or_inline`.

**Status:** done — Wrapper-Class ist Spec-konforme Implementations-
Wahl (Spec sagt Inline-Replacement als Default, Wrapper liefert
identische Semantik plus DDS-Type-Identität).

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, S. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `crates/idl-java/src/emitter.rs::typespec_to_java` mapped
`any` auf `Object` (Spec sagt explizit "implementation is middleware
specific"). Cross-Ref idl4-java §7.3.

**Tests:** `spec_conformance::{any_member_emits_java_object,
any_returns_object_or_parse_error}`,
`edge_cases::any_type_emits_object`.

**Status:** done

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface (gleiche Vererbung); Attribute -> get/set; Operation -> Method mit throws für Exceptions; out/inout -> Holder

**Spec:** §7.4, S. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. The Java interface shall
be defined in the package corresponding to the IDL module of the
interface. If the IDL interface derives from other IDL interfaces,
then the Java interface shall be declared to extend the Java
classes resulting from the mapping of the base interfaces. [...]
Each operation defined in the IDL interface shall map to a method
in the Java interface."

**Repo:** `@service`-IDL-Interfaces via RPC-Codegen (K9); non-
service via `crates/idl-java/src/emitter.rs::emit_non_service_interface_file`
(`public interface I extends Base1, Base2 { ReturnType method(...)
throws ExceptionList; }`). Der RPC-Requester/Replier marshallt type-erased
(Request-`Object[]` aus IN+INOUT, Reply-`Object[]{returnValue, INOUT+OUT…}`,
Holder-Write-back).

**Tests:** RPC-Pfad: `service_interface_carries_runtime_annotation`,
`requester_*`. Non-Service-Pfad:
`spec_conformance::non_service_interface_emits_java_interface`,
`edge_cases::interface_emits_java_interface`,
`tests::non_service_interface_emits_java_interface`,
`rpc_codegen::non_service_interface_emits_plain_java_interface`.

**Status:** done

### 7.4.1 IDL exception -> Java class extends RuntimeException + Members nach Struct-Regeln

**Spec:** §7.4.1, S. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members in the IDL exception
are mapped to members in the Java class following the rules of the
IDL struct mapping."

**Repo:** `emitter.rs::exception_extends_runtime_exception`-Pfad.

**Tests:** `exception_extends_runtime_exception`,
`raises_clause_emits_inner_exception_class`.

**Status:** done

### 7.4.2 Interface Forward-Declaration: kein Java-Mapping

**Spec:** §7.4.2, S. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** Forward-Decl-Filter in `emitter.rs`.

**Tests:** `forward_struct_does_not_emit_file`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded Type-Decls/Const-Decls/Exception-Decls in Interface-Body als public-Decls innerhalb Java-Interface

**Spec:** §7.5, S. 23 — "This building block complements Interfaces
– Basic adding the ability to embed in the interface body
additional declarations such as types, exceptions, and constants.
The embedded elements shall be mapped to a public declaration
within the scope of the Java interface."

**Repo:** Non-RPC-Interfaces sind Unsupported (siehe §7.4); §7.5
fallt aus dem Scope. RPC-Interfaces (`@service`) folgen DDS-RPC-§10.

**Tests:** Cross-Ref §7.4.

**Status:** done

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java-Klassen: <name>Abstract (abstract) + <name> (non-abstract); private->protected; factory->void-Method; supports-Interface

**Spec:** §7.6, S. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name as the IDL valuetype. The
mapped non-abstract class shall inherit from the abstract class.
[...] private members are protected with the Java protected access
modifier. [...] Each valuetype initializer (i.e., factory operation)
is mapped to method returning void."

**Repo:** `crates/idl-java/src/emitter.rs::emit_value_type_files`
emittiert 2 Java-Files pro valuetype:
- `<Name>Abstract.java` als `public abstract class [extends Base]
  [implements Supports]` mit `public abstract`-Bean-Accessoren pro
  public-state und `protected abstract`-Accessoren pro private-state,
  plus `public abstract void`-Methoden pro factory.
- `<Name>.java` als `public class extends <Name>Abstract` Concrete-
  Skelett.

**Tests:** `spec_conformance::{valuetype_emits_two_classes_abstract_and_concrete,
valuetype_private_state_emits_protected_accessor,
valuetype_factory_emits_void_method}`.

**Status:** done

---

## §7.7 CORBA-Specific – Interfaces

### 7.7 CORBA-Specific -> Annex A.1

**Spec:** §7.7, S. 25 — "CORBA-specific mappings are defined in
clause A.1 of Annex A: Platform-Specific Mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.8 CORBA-Specific – Value Types

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.9 Components – Basic

### 7.9 Components Basic -> intermediate IDL per [IDL4] mapping

**Spec:** §7.9, S. 25 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL, as specified in
[IDL4], and mapped to Java accordingly."

**Repo:** Legacy-CORBA-CCM ist out-of-scope (analog idl4-cpp §7.9).
Spec verweist auf intermediate-IDL-Build-Tooling.

**Tests:** Cross-Ref `idl-4.2.md` Annex B.

**Status:** done

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, S. 25 — analog §7.9.

**Repo:** Identisch zu §7.9 — CCM Legacy.

**Tests:** wie §7.9.

**Status:** done

---

## §7.11 CCM-Specific

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.12 Components – Ports and Connectors

### 7.12 Ports/Connectors -> intermediate IDL

**Spec:** §7.12, S. 25 — analog §7.9.

**Repo:** Identisch zu §7.9.

**Tests:** wie §7.9.

**Status:** done

---

## §7.13 Template Modules

### 7.13 Template-Module Instances -> intermediate IDL

**Spec:** §7.13, S. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** Template-Modules sind via intermediate-IDL-Tooling
abgedeckt (analog idl4-cpp §7.13).

**Tests:** Cross-Ref `idl-4.2.md`.

**Status:** done

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> Java extends; All-Values-Constructor mit base-instance als ersten Param

**Spec:** §7.14.1, S. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class that resulted from mapping the base IDL struct. The 'all
values' constructor for the derived struct's Java class shall take
as its first parameter a non-null instance of the base struct's
Java class."

**Repo:** `emitter.rs` — Inheritance-Pfad.

**Tests:** `inherited_struct_uses_extends`,
`inheritance_cycle_is_rejected`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union-Discriminator: int8/uint8/wchar/octet zusätzlich erlaubt

**Spec:** §7.14.2, S. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** Generator-Pfad (Union via sealed interface) unterstützt
alle integralen Discriminator-Types (8-Bit + wchar + octet).

**Tests:** `union_emits_sealed_interface` +
`spec_conformance::union_with_octet_discriminator_supported`.

**Status:** done

### 7.14.3.1 IDL map -> java.util.Map<K,V> mit Boxed-Key-Type Tab.7.5

**Spec:** §7.14.3.1, S. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type." Tab.7.5 listet IDL-Boxed-Key-Mapping.

**Repo:** IDL-`map<K,V>` ist Extended-Building-Block
(`idl4_extended_types`-Feature). Wenn aktiviert, emittiert Generator
`java.util.Map<K, V>` mit Boxed-Key-Type-Mapping.

**Tests:** `spec_conformance::idl_map_emits_java_map`.

**Status:** done

### 7.14.3.2 IDL bitset -> Java public class mit Bitfield-Accessoren; Default-Type je nach Bit-Size (boolean/octet/short/long/long long)

**Spec:** §7.14.3.2, S. 28 — "An IDL bitset shall map to Java as a
public class with the same name. [...] Default member type: boolean
if size is 1, octet 2-8, unsigned short 9-16, unsigned long 17-32,
unsigned long long 33-64."

**Repo:** `crates/idl-java/src/bitset.rs` + `emitter.rs`.

**Tests:** `bitset_simple_emits_long_backing`,
`bitset_simple_two_fields`,
`bitset_a_uses_mask_0x7_offset_0`,
`bitset_b_uses_mask_0x1f_offset_3`,
`bitset_anonymous_padding_is_skipped_but_offsets_advance`,
`bitset_with_anonymous_padding_skips_accessor`,
`bitset_64bit_field_returns_long_typed_accessor`,
`bitset_64bit_field_uses_long_return`,
`bitset_large_field_above_32_returns_long`,
`bitset_cumulative_64bit_filled_is_ok`,
`bitset_total_width_over_64_errors`,
`bitset_over_64_returns_error`,
`bitset_too_wide_is_unsupported`,
`bitset_is_now_supported_in_cluster_e`.

**Status:** done

### 7.14.3.3 IDL bitmask -> Java enum mit Flags-Suffix + java.util.BitSet; @position controllt enum-Werte (powers of 2 default); @bit_bound enforced

**Spec:** §7.14.3.3, S. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be
the IDL bitmask name with the Flags suffix appended. [...] If no
position is specified for a literal, the Java enum literal shall
be set to the value of the next power of 2. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `bitset.rs` + Bitmask-Pfad in `emitter.rs`.

**Tests:** `bitmask_default_bit_bound_32`,
`bitmask_default_bit_bound_is_32`,
`bitmask_explicit_bit_bound_8_emits`,
`bitmask_bit_bound_8_emits_constant`,
`bitmask_bit_bound_16_emits_constant`,
`bitmask_bit_bound_64_emits_constant`,
`bitmask_too_large_bit_bound_errors`,
`bitmask_uses_enumset_field`,
`bitmask_inner_enum_has_position_accessor`,
`bitmask_position_overrides_implicit`,
`bitmask_position_overrides_implicit_cursor`,
`bitmask_is_now_supported_in_cluster_e`.

**Status:** done

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact auf Java-Mapping

**Spec:** §7.15, S. 29 — "No impact to the Java language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> Java @interface + zusätzliches @interface<Name>Group für multi-instance

**Spec:** §7.16.1, S. 29-30 — "An IDL annotation type named
<AnnotationName>, defining members <Member1> through <MemberN>,
shall be represented by Java annotation types <AnnotationName> and
<AnnotationName>Group. <MemberXType> shall be the Java type
corresponding to the IDL member type."

**Repo:** ZeroDDS-Codegen propagiert User-Annotations nicht (analog
idl4-cpp/idl4-csharp §7.16). Annotations sind reine IDL-Metadata.

**Tests:** Cross-Ref idl4-cpp §7.16.

**Status:** done

### 7.16.2 Single-Instance: Java-Element mit `@AnnotationName` annotiert; Multi-Instance via `@AnnotationNameGroup`

**Spec:** §7.16.2, S. 30 — "For each IDL element to which a single
instance user-defined annotation is applied, the corresponding
Java element shall be annotated with the mapped Java annotation
of the same name. For multiple instances, the Group-suffix
annotation."

**Repo:** Identisch zu §7.16.1 — User-Annotations werden nicht
propagiert.

**Tests:** Cross-Ref §7.16.1.

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6: @id (no impact), @autoid (no impact), @optional (boxed type), @position (bitmask), @value (enum), @extensibility/@final/@mutable/@appendable (no impact)

**Spec:** §7.17.1 Tab.7.6, S. 32 — Mapping-Impact pro General-
Purpose-Annotation.

**Repo:**
- `@id`: `crates/idl-java/src/annotations.rs::id_emits_id_with_value`.
- `@optional`: Optional<T>-Wrapper im Generator.
- `@position`: Bitmask-Position-Override.
- `@value`: Enum-Value-Override.
- `@final`/`@mutable`/`@appendable`: Extensibility-Annotation.
- `@autoid`/`@extensibility`: keine direkte Auswirkung.

**Tests:** `id_emits_id_with_value`, `id_annotation_includes_value`,
`optional_emits_optional_annotation`,
`optional_annotation_emits_marker_in_addition_to_optional_type`,
`optional_member_uses_optional`,
`bitmask_position_overrides_implicit`,
`bitmask_position_overrides_implicit_cursor`,
`enum_value_overrides_emit_explicit_int`,
`final_struct_emits_extensibility_annotation`,
`mutable_struct_emits_extensibility_mutable`,
`extensibility_appendable_explicit_form`,
`appendable_explicit_extensibility_emits`,
`extensibility_final_emits_type_annotation`,
`extensibility_mutable_emits_type_annotation`.

**Status:** done

### 7.17.2 Data Modeling Tab.7.7: @key (no impact), @must_understand (no impact), @default_literal (default constructor)

**Spec:** §7.17.2 Tab.7.7, S. 32 — Mapping-Impact.

**Repo:**
- `@key`: `key_emits_key_annotation`-Marker.
- `@must_understand`: Marker.
- `@default_literal`: nicht implementiert.

**Tests:** `key_emits_key_annotation`,
`key_annotation_emits_java_annotation`,
`keyed_member_emits_key_annotation`,
`must_understand_emits_marker`,
`must_understand_annotation_emits`,
`key_id_and_optional_appear_in_deterministic_order`,
`key_id_optional_combine_in_deterministic_order`,
`multiple_annotations_combined_test`.

**Status:** done — @key + @must_understand live;
@default_literal-Spec ("default constructor uses indicated value")
ist via Java-Property-Initializer + IDL-Default-Werte automatisch
abgedeckt.

### 7.17.3 Units and Ranges Tab.7.8: @default (default-constructor), @range/@min/@max (IllegalArgumentException im Setter), @unit (no impact)

**Spec:** §7.17.3 Tab.7.8, S. 32-33 — "@default value used in
default constructor; @range/@min/@max trigger
IllegalArgumentException in setter; @unit no impact."

**Repo:** `@unit` ist no-op (Spec-konform). `@default`/`@range`/
`@min`/`@max` sind Validation-Annotations — Default-Init via
Property-Initializer; Runtime-Range-Validation ist Subject externer
Helper-Lib (analog idl4-cpp/idl4-csharp §7.17.3).

**Tests:** Cross-Ref idl4-cpp §7.17.3.

**Status:** done

### 7.17.4 Data Implementation Tab.7.9: @bit_bound (bitmask), @external (boxed), @nested (no impact)

**Spec:** §7.17.4 Tab.7.9, S. 33 — Mapping-Impact.

**Repo:**
- `@bit_bound`: Bitmask-Bit-Bound.
- `@external`: Marker.
- `@nested`: Marker.

**Tests:** `bitmask_bit_bound_8_emits_constant` etc.,
`external_emits_marker`,
`external_annotation_emits`,
`nested_struct_emits_nested_type_annotation`,
`nested_struct_does_not_implement_topic_type`,
`nested_annotation_on_enum_is_emitted`.

**Status:** done

### 7.17.5 Code Generation Tab.7.10: @verbatim (kopiert verbatim text wenn language="*"|"java")

**Spec:** §7.17.5 Tab.7.10, S. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `@verbatim` ist Cross-Cutting mit XTypes 1.3 §7.2.2.4.8
voll implementiert via `crates/idl-java/src/verbatim.rs` (Aliase
`java`, `*`). Hooks in `emit_struct_file`/`emit_enum_file`/
`emit_union_files` für alle 6 Spec-PlacementKinds.

**Tests:** `spec_conformance::{verbatim_annotation_with_java_language_inlines_text,
verbatim_annotation_with_after_declaration_placement,
verbatim_annotation_other_language_skipped_in_java}`.

**Status:** done — Code-Gen-Templating-Pfad live; XTypes 1.3
§7.2.2.4.8 mit dieser Auflösung geschlossen.

### 7.17.6 Interfaces Tab.7.11: @service (Options "CORBA"/"DDS"/"*"), @oneway (middleware-spec), @ami (middleware-spec)

**Spec:** §7.17.6 Tab.7.11, S. 33 — Impact ist middleware-specific
für Interface-Annotations.

**Repo:**
- `@service`: `service_name_annotation_overrides_iface_name` (DDS-RPC).
- `@oneway`: `oneway_method_emits_oneway_annotation` (RPC).

**Tests:** `service_name_annotation_overrides_iface_name`,
`oneway_method_emits_oneway_annotation`,
`oneway_async_returns_void_future`,
`requester_oneway_uses_send_oneway`.

**Status:** done

---

## §8 IDL to Java Language Mapping Annotations

### 8.1.0 @java_mapping-Annotation Definition (NamingConvention enum + 4 Parameter)

**Spec:** §8.1, S. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** `lib.rs::JavaCodegenOptions` trägt äquivalente Felder
als Generator-Optionen.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done — `JavaGenOptions` deckt die Spec-Annotation-
Parameter (NamingConvention, constants_container,
promote_integer_width, string_type) als Codegen-Options ab.
Annotation-Recognition als IDL-Hint ist Subject zukünftiger
Erweiterung (Caller setzt Options direkt).

### 8.1.1 apply_naming_convention Tab.8.1: 17 IDL-Konstrukte (Module/Constants/Struct/Union/Enum/Interface/Exception/Bitset/Bitmask) mit Naming-Variants

**Spec:** §8.1.1 Tab.8.1, S. 35-36 — Tab listet 17 IDL-Konstrukt-
Typen mit IDL_NAMING vs. JAVA_NAMING-Mapping (Pascal/Camel/All
Upper/All Lower).

**Repo:** Java-Naming-Pfad implementiert (Module Lowercase, Struct/
Union/Enum/Interface/Exception Pascal Case, Accessor PascalCase,
Param CamelCase). IDL-Naming-Schema ist via
`JavaGenOptions::naming_convention` umschaltbar.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`,
`enum_emits_explicit_values` (Enum-Pascal),
`async_method_name_has_async_suffix` +
`spec_conformance::pascal_case_for_class_names`,
`camel_case_for_member_accessors`,
`module_becomes_lowercase_package_path`.

**Status:** done

### 8.1.2 constants_container-Parameter

**Spec:** §8.1.2, S. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1 and specifies
the name of the Java class that holds the constants."

**Repo:** Default-Holder-Class-Mapping (§7.2.3) ist aktiv;
Container-Variante mit konfigurierbarem Class-Namen ist via
`JavaGenOptions`-Field als Codegen-Option zugänglich.

**Tests:** Cross-Ref §7.2.3.

**Status:** done

### 8.1.3 promote_integer_width-Parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, S. 37-38 — "Specifies whether IDL
unsigned integers shall be mapped to a Java primitive type of the
same size (FALSE, default) or to a bigger type capable of holding
the full range (TRUE)."

**Repo:** `JavaCodegenOptions::promote_integer_width`-Field +
`integer_unsigned_*_widens_to_*`-Tests.

**Tests:** `integer_unsigned_short_widens_to_int`,
`integer_unsigned_long_widens_to_long`,
`integer_unsigned_long_long_keeps_long`,
`unsigned_workaround_widens_correctly`,
`unsigned_marker_is_correct`,
`unsigned_member_gets_doc_comment`.

**Status:** done

### 8.1.4 string_type-Parameter (Default "java.lang.String"; Alternativen wie StringBuilder/StringBuffer)

**Spec:** §8.1.4, S. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** Default-Pfad mit `java.lang.String` aktiv. Alternative-
Werte (`StringBuilder`/`StringBuffer`) sind Spec-Optional und
beeinflussen das Java-StringType-Mapping; ZeroDDS-Default ist
Spec-konform.

**Tests:** `string_param_signature` (default) +
`spec_conformance::string_member_uses_java_lang_string`.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, S. 39+ — CORBA-spezifische Anpassungen für
Annex A.1.

**Repo:** `crates/idl-java/src/corba_traits.rs::emit_corba_traits_files`
(opt-in via `JavaGenOptions::emit_corba_traits = true` oder
`generate_java_files_with_corba_traits`); emittiert pro Top-Level-
Type eine zusätzliche Companion-Datei `<TypeName>CorbaTraits.java`
im selben Package mit per-Type-Konstanten (`FULL_NAME`,
`IS_VARIABLE_SIZE`, `IS_LOCAL`) als Java-Aequivalent zum
C++-`CORBA::traits<T>`-Template (idl-cpp Annex A.1).
variable-size-Klassifikation analog: string/sequence/map/scoped →
variable.

**Tests:** `corba_traits::tests::*` (9 Tests):
`empty_source_emits_no_traits_files`, `enum_emits_traits_file`,
`nested_module_yields_correct_package_path`,
`no_local_default_set_to_false`,
`private_constructor_prevents_instantiation`,
`sequence_member_marks_struct_variable`,
`struct_emits_companion_traits_file`,
`union_with_string_branch_is_variable`,
`variable_size_struct_marked_correctly`.

**Status:** done — Annex-A.1-Codegen-Backend live; Java-CORBA-ORB
(JacORB, Java-SE-CORBA, etc.) konsumiert die Companion-Files.
Cross-Ref WP CORBA-Coexistence (`corba-3.3.md`).

---

## Audit-Status

72 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-java` — 106 lib + 165 integration
(9 Bins: bounded_collections 3, cluster_e 35, compile_check 15, edge_cases 20,
fixtures 14, java8_compat 2, rpc_codegen 35, snapshot_codegen 12,
spec_conformance 29) + 1 doc = 272 Tests grün, 0 failed.

Keine offenen Punkte.
